// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: subprocess I/O (Xvfb lifecycle) — Linux only for real work.
// Parallelism: display alloc serialized via tokio Mutex.
//! Private Xvfb display allocation and RAII teardown (one-shot).

use crate::process_lifecycle::apply_process_group_and_pdeathsig;
use crate::process_lifecycle::cleanup_xvfb_display_files;
#[cfg(target_os = "linux")]
use crate::process_lifecycle::{x11_lock_path, x11_socket_path};


/// Detects whether the current platform has a native display server available.
/// Linux: checks `$DISPLAY` (X11) or `$WAYLAND_DISPLAY` (Wayland).
/// macOS/Windows: always returns true (Quartz/DWM always active on desktop).
pub(crate) fn has_native_display() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(d) = std::env::var("DISPLAY") {
            if !d.is_empty() {
                return true;
            }
        }
        if let Ok(d) = std::env::var("WAYLAND_DISPLAY") {
            if !d.is_empty() {
                return true;
            }
        }
        false
    }
    #[cfg(target_os = "macos")]
    {
        true
    }
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        return false;
    }
}

/// Returns `true` when the Xvfb lock file at `path` references a PID that
/// is no longer running (stale lock from a crashed/killed Xvfb). GAP-WS-089.
#[cfg(target_os = "linux")]
pub(crate) fn is_lock_stale(path: &std::path::Path) -> bool {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let pid_str = contents.trim();
    let pid: i32 = match pid_str.parse() {
        Ok(p) if p > 0 => p,
        _ => return true,
    };
    // Safe check: /proc/{pid} existence is a non-signal probe.
    !std::path::Path::new(&format!("/proc/{pid}")).exists()
}

/// RAII guard for a private Xvfb process (L-02, L-06, L-09).
///
/// Always kills the process group / PID and removes lock/socket files on drop,
/// including when Chrome launch fails after Xvfb was already started.
pub(crate) struct XvfbGuard {
    child: std::process::Child,
    pub(crate) display: String,
    /// Process group id (== child pid when setpgid(0,0) succeeded).
    pgid: Option<i32>,
    reaped: bool,
}

impl XvfbGuard {
    pub(crate) fn session_bits(&self) -> (Option<u32>, Option<i32>, String) {
        (Some(self.child.id()), self.pgid, self.display.clone())
    }

    pub(crate) fn reap(&mut self) {
        if self.reaped {
            return;
        }
        self.reaped = true;
        if let Some(pgid) = self.pgid {
            crate::process_lifecycle::kill_process_group(pgid);
        }
        let _ = self.child.kill();
        // Drop/reap must not block indefinitely (memory/RAII rules). Prefer a
        // short try_wait poll over `Child::wait`, which can hang if the child
        // ignores SIGKILL (D-state) or was reparented oddly.
        const WAIT_BUDGET: std::time::Duration = std::time::Duration::from_millis(200);
        const POLL: std::time::Duration = std::time::Duration::from_millis(10);
        let deadline = std::time::Instant::now() + WAIT_BUDGET;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(POLL);
                }
                Ok(None) => {
                    tracing::warn!(
                        display = %self.display,
                        "Xvfb still alive after kill+{WAIT_BUDGET:?} — abandoning wait (Drop must not hang)"
                    );
                    break;
                }
                Err(err) => {
                    tracing::info!(?err, display = %self.display, "Xvfb try_wait error — continuing cleanup");
                    break;
                }
            }
        }
        cleanup_xvfb_display_files(&self.display);
        tracing::info!(display = %self.display, "Xvfb virtual display stopped");
    }
}

impl Drop for XvfbGuard {
    fn drop(&mut self) {
        self.reap();
    }
}

/// Serialises Xvfb display-number allocation across concurrent Chrome launches.
///
/// GAP-PAR-015: without this, parallel `ChromeBrowser::launch` (content-fetch
/// pool) can race on the same free `:N` and fail flaky. Tokio mutex so the
/// exclusive section can include the async readiness poll (GAP-PAR-022).
///
/// Lock order (document for rules checklist): **display_alloc →** TempDir
/// user-data → chromiumoxide Browser (never invert).
#[cfg(target_os = "linux")]
pub(crate) fn xvfb_display_alloc_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// Spawns a private Xvfb server on a free display number so Chrome can run
/// in headed mode (passing Cloudflare anti-bot) without showing a visible
/// window to the user.
///
/// Async readiness poll uses `tokio::time::sleep` (GAP-PAR-022) — never
/// `std::thread::sleep` on a Tokio worker during parallel Chrome launches.
/// Display allocation uses [`tokio::sync::Mutex`] so the exclusive pick can
/// span the readiness await without blocking the multi-thread scheduler
/// (GAP-PAR-015 lock order: display_alloc → TempDir → Browser).
///
/// Returns [`XvfbGuard`] on success, or `None` if Xvfb is not available or no
/// free display slot was found. The guard always reaps Xvfb on drop (L-06).
#[cfg(target_os = "linux")]
pub(crate) async fn spawn_virtual_display() -> Option<XvfbGuard> {
    let xvfb_path = which::which("Xvfb").ok()?;

    // GAP-PAR-015: exclusive display pick for the whole try loop (incl. await).
    let alloc = xvfb_display_alloc_lock();
    let _alloc_guard = alloc.lock().await;

    for display_num in 99..200 {
        // GAP-HARD-X11-001: resolve lock/socket via temp_dir() (not hardcoded /tmp).
        let num = display_num.to_string();
        let lock_path = x11_lock_path(&num);
        if lock_path.exists() {
            // GAP-WS-089: check if the PID in the lock file is still alive;
            // remove stale locks left by crashed/killed Xvfb processes.
            if is_lock_stale(&lock_path) {
                let _ = std::fs::remove_file(&lock_path);
                let socket_path = x11_socket_path(&num);
                let _ = std::fs::remove_file(&socket_path);
                tracing::info!(display_num, "removed stale Xvfb lock file");
            } else {
                continue;
            }
        }
        let disp = format!(":{display_num}");
        // GAP-PROC-001: explicit Stdio on every stream (rules-rust-processos-externos).
        // Silent daemon: discard all three; never inherit parent stdin (agent pipes).
        // Security: env_clear + minimal PATH — Xvfb does not need proxy/credentials.
        let mut cmd = std::process::Command::new(&xvfb_path);
        cmd.arg(&disp)
            .args(["-screen", "0", "1920x1080x24", "-nolisten", "tcp", "-ac"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .env_clear()
            .env(
                "PATH",
                std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
            );
        apply_process_group_and_pdeathsig(&mut cmd);
        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => return None,
        };
        let pgid = Some(child.id() as i32);

        // GAP-PAR-022: cooperative sleep (not std::thread::sleep).
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        if lock_path.exists() {
            tracing::info!(
                xvfb_display = %disp,
                xvfb_pid = child.id(),
                "Xvfb virtual display started (process group + PDEATHSIG)"
            );
            return Some(XvfbGuard {
                child,
                display: disp,
                pgid,
                reaped: false,
            });
        }
        // Xvfb failed to create lock — reap this attempt and try next display.
        let mut failed = XvfbGuard {
            child,
            display: disp,
            pgid,
            reaped: false,
        };
        failed.reap();
    }
    None
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)] // only invoked under the cfg(target_os = "linux") launch path
pub(crate) async fn spawn_virtual_display() -> Option<XvfbGuard> {
    None
}

/// Attempts to auto-install Xvfb on Linux when not found.
/// Shows visible messages to the user via [`crate::output::emit_stderr`]
/// (MP-06: no raw `eprintln!` outside `output`).
/// Uses `sudo -n` (non-interactive) to avoid blocking on password prompts.
#[cfg(target_os = "linux")]
pub(crate) fn try_auto_install_xvfb() {
    if which::which("Xvfb").is_ok() {
        return;
    }
    let distro = detect_linux_distro();
    let variant = detect_linux_variant();
    if variant == "immutable" {
        crate::output::emit_stderr(
            crate::i18n::Message::XvfbImmutableDistro.format(
                crate::i18n::language(),
                &[("distro", &distro)],
            ),
        );
        crate::output::emit_stderr(xvfb_manual_instruction(&distro));
        return;
    }
    let (pkg_manager, args): (&str, Vec<&str>) = match distro.as_str() {
        "fedora" | "rhel" | "centos" | "rocky" | "almalinux" => {
            ("dnf", vec!["install", "-y", "xorg-x11-server-Xvfb"])
        }
        "ubuntu" | "debian" | "linuxmint" | "pop" | "zorin" | "elementary" | "kali" => {
            ("apt-get", vec!["install", "-y", "xvfb"])
        }
        "arch" | "manjaro" | "endeavouros" | "garuda" => {
            ("pacman", vec!["-S", "--noconfirm", "xorg-server-xvfb"])
        }
        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" | "sles" => {
            ("zypper", vec!["install", "-y", "xorg-x11-server-Xvfb"])
        }
        "alpine" => ("apk", vec!["add", "xvfb"]),
        "amzn" => ("yum", vec!["install", "-y", "Xvfb"]),
        "void" => ("xbps-install", vec!["-y", "xorg-server-xvfb"]),
        "gentoo" => ("emerge", vec!["--ask=n", "x11-base/xorg-server"]),
        _ => {
            crate::output::emit_stderr(
                crate::i18n::Message::XvfbUnknownDistro.format(
                    crate::i18n::language(),
                    &[("distro", &distro)],
                ),
            );
            crate::output::emit_stderr(xvfb_manual_instruction(&distro));
            return;
        }
    };
    let install_cmd = format!("sudo {} {}", pkg_manager, args.join(" "));
    let lang = crate::i18n::language();
    crate::output::emit_stderr(crate::i18n::Message::XvfbAutoInstallAttempt.text(lang));
    crate::output::emit_stderr(format_args!("\x1b[36m  $ {install_cmd}\x1b[0m"));
    // Memory/RAII + external-process rules: every `Command::new("sudo")` with
    // `.status()` must use `Stdio::null()` so a password prompt / TTY attach
    // cannot hang the one-shot CLI. User feedback stays on emit_stderr.
    // Security: env_clear + fixed PATH so inherited proxy/credential vars
    // cannot influence the privileged install command.
    let status = std::process::Command::new("sudo")
        .arg("-n")
        .arg(pkg_manager)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .current_dir("/")
        .status();
    match status {
        Ok(s) if s.success() => {
            crate::output::emit_stderr(crate::i18n::Message::XvfbInstalledOk.text(lang));
        }
        Ok(_) => {
            crate::output::emit_stderr(crate::i18n::Message::XvfbAutoInstallFailed.text(lang));
            crate::output::emit_stderr(format_args!(
                "{}\n\x1b[36m  $ {install_cmd}\x1b[0m",
                crate::i18n::Message::XvfbInstallManually.text(lang)
            ));
        }
        Err(e) => {
            let err_s = e.to_string();
            crate::output::emit_stderr(
                crate::i18n::Message::XvfbPackageManagerFailed
                    .format(lang, &[("error", &err_s)]),
            );
            crate::output::emit_stderr(format_args!(
                "{}\n\x1b[36m  $ {install_cmd}\x1b[0m",
                crate::i18n::Message::XvfbInstallManually.text(lang)
            ));
        }
    }
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)] // only invoked under the cfg(target_os = "linux") launch path
pub(crate) fn try_auto_install_xvfb() {}

/// Returns a distro-aware instruction string for manual Xvfb installation.
#[cfg(target_os = "linux")]
pub(crate) fn xvfb_manual_instruction(distro: &str) -> String {
    let specific = match distro {
        "fedora" | "rhel" | "centos" | "rocky" | "almalinux" => {
            Some("sudo dnf install -y xorg-x11-server-Xvfb")
        }
        "ubuntu" | "debian" | "linuxmint" | "pop" | "zorin" | "elementary" | "kali" => {
            Some("sudo apt-get install -y xvfb")
        }
        "arch" | "manjaro" | "endeavouros" | "garuda" => {
            Some("sudo pacman -S --noconfirm xorg-server-xvfb")
        }
        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" | "sles" => {
            Some("sudo zypper install -y xorg-x11-server-Xvfb")
        }
        "alpine" => Some("sudo apk add xvfb"),
        "void" => Some("sudo xbps-install -y xorg-server-xvfb"),
        "gentoo" => Some("sudo emerge x11-base/xorg-server"),
        "amzn" => Some("sudo yum install -y Xvfb"),
        "nixos" => Some("nix-env -iA nixpkgs.xorg.xorgserver"),
        "guix" => Some("guix install xorg-server"),
        _ => None,
    };
    let mut msg = crate::i18n::Message::XvfbInstallManuallyFull
        .text(crate::i18n::language())
        .to_owned();
    if let Some(cmd) = specific {
        msg.push_str(&format!("\x1b[36m  $ {cmd}\x1b[0m\n"));
    } else {
        msg.push_str(
            "\x1b[36m  Fedora/RHEL:       sudo dnf install -y xorg-x11-server-Xvfb\n\
             \x1b[36m  Ubuntu/Debian:     sudo apt-get install -y xvfb\n\
             \x1b[36m  Arch/Manjaro:      sudo pacman -S --noconfirm xorg-server-xvfb\n\
             \x1b[36m  openSUSE:          sudo zypper install -y xorg-x11-server-Xvfb\n\
             \x1b[36m  Alpine:            sudo apk add xvfb\n\
             \x1b[36m  NixOS:             nix-env -iA nixpkgs.xorg.xorgserver\n\
             \x1b[36m  Silverblue:        rpm-ostree install xorg-x11-server-Xvfb && systemctl reboot\x1b[0m\n",
        );
    }
    // Fedora Silverblue/Kinoite/ostree-based
    if distro == "fedora" {
        let variant = detect_linux_variant();
        if variant == "immutable" {
            msg = crate::i18n::Message::XvfbInstallManuallyFull
                .text(crate::i18n::language())
                .to_owned();
            msg.push_str(
                "\x1b[36m  $ rpm-ostree install xorg-x11-server-Xvfb && systemctl reboot\x1b[0m\n",
            );
        }
    }
    msg
}

/// Detects immutable/NixOS/Silverblue distros where package install is non-standard.
#[cfg(target_os = "linux")]
pub(crate) fn detect_linux_variant() -> &'static str {
    let content = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let lower = content.to_lowercase();
    if lower.contains("variant_id=silverblue")
        || lower.contains("variant_id=kinoite")
        || lower.contains("variant_id=sericea")
        || lower.contains("variant_id=onyx")
    {
        return "immutable";
    }
    if lower.contains("\nid=nixos")
        || lower.contains("\nid=\"nixos\"")
        || lower.contains("\nid=guix")
        || lower.contains("\nid=\"guix\"")
    {
        return "immutable";
    }
    if std::path::Path::new("/run/ostree-booted").exists() {
        return "immutable";
    }
    "mutable"
}

/// Detects the Linux distribution by reading /etc/os-release.
/// Returns the ID field (e.g. "fedora", "ubuntu", "arch").
#[cfg(target_os = "linux")]
pub(crate) fn detect_linux_distro() -> String {
    let content = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    for line in content.lines() {
        if let Some(id) = line.strip_prefix("ID=") {
            return id.trim_matches('"').to_lowercase();
        }
    }
    "unknown".to_string()
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::process_lifecycle::{x11_lock_path, x11_socket_path};

    /// GAP-HARD-X11-001: spawn path construction must stay under `temp_dir()`.
    #[test]
    fn xvfb_lock_and_socket_paths_under_temp_dir() {
        let tmp = std::env::temp_dir();
        for num in ["99", "150", "199"] {
            let lock = x11_lock_path(num);
            let socket = x11_socket_path(num);
            assert!(
                lock.starts_with(&tmp),
                "lock {} not under {}",
                lock.display(),
                tmp.display()
            );
            assert!(
                socket.starts_with(&tmp),
                "socket {} not under {}",
                socket.display(),
                tmp.display()
            );
            assert_eq!(
                lock,
                tmp.join(format!(".X{num}-lock")),
                "lock path must be temp_dir()/.X{{n}}-lock"
            );
            assert_eq!(
                socket,
                tmp.join(".X11-unix").join(format!("X{num}")),
                "socket path must be temp_dir()/.X11-unix/X{{n}}"
            );
        }
    }
}
