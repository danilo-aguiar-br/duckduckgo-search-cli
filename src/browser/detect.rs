// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative + light process I/O (Chrome path detection / version probe).
// Parallelism: N/A — one-shot path resolution (no fan-out).
//! Chrome/Chromium binary detection, channel classification, and version probe.

use crate::error::CliError;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Installation channel for a resolved Chrome/Chromium binary (agent metadata).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeChannel {
    /// Explicit `--chrome-path` (after shell→ELF resolution when needed).
    Manual,
    /// Native package / Applications / Program Files host install.
    Host,
    /// Flatpak deploy ELF (`files/extra/chrome` or equivalent).
    Flatpak,
    /// Snap install.
    Snap,
}

impl ChromeChannel {
    /// Stable string for JSON / logs (`manual|host|flatpak|snap`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Host => "host",
            Self::Flatpak => "flatpak",
            Self::Snap => "snap",
        }
    }
}

/// Resolved Chrome executable ready for `BrowserConfigBuilder::chrome_executable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedChrome {
    /// Absolute or concrete path to a real binary (ELF/Mach-O/PE), never a shell export.
    pub path: PathBuf,
    /// How the path was obtained / which install channel it belongs to.
    pub channel: ChromeChannel,
}

/// Returns an ordered list of candidate paths for Chrome/Chromium by platform.
///
/// Order (Linux): host Google Chrome → host Chromium ELF (lib64) → Flatpak
/// deploy ELF → Flatpak exports (resolved later) → Snap. Windows resolves
/// `%ProgramFiles%`, `%ProgramFiles(x86)%`, `%LOCALAPPDATA%` (never hardcodes
/// only `C:\`) and includes Beta / Canary / Edge. macOS covers system and
/// user `Applications` plus Homebrew.
///
/// GAP-WS-AGENT-READY-001 v0.9.8: include Flatpak **deploy ELF** under
/// `…/app/<id>/current/active/files/extra/chrome`, not only shell exports.
/// GAP-MP-001 (Pass 20): full multiplatform candidate matrix.
pub fn chrome_candidate_paths() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::with_capacity(32);

    #[cfg(target_os = "linux")]
    {
        // Host Google Chrome first, then host Chromium ELF (avoid shell wrappers).
        for base in [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/google-chrome-beta",
            "/usr/bin/google-chrome-unstable",
            "/opt/google/chrome/chrome",
            "/opt/google/chrome/google-chrome",
            "/usr/lib64/chromium-browser/chromium-browser",
            "/usr/lib/chromium-browser/chromium-browser",
            "/usr/lib/chromium-browser/chrome",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/usr/local/bin/chromium",
            "/usr/local/bin/google-chrome",
            // Flatpak deploy ELF (chromiumoxide can launch these with --no-sandbox).
            "/var/lib/flatpak/app/com.google.Chrome/current/active/files/extra/chrome",
            "/var/lib/flatpak/app/org.chromium.Chromium/current/active/files/extra/chrome",
            // Exports (shell) — resolved to deploy ELF by resolve_chrome_candidate.
            "/var/lib/flatpak/exports/bin/com.google.Chrome",
            "/var/lib/flatpak/exports/bin/org.chromium.Chromium",
            "/snap/bin/chromium",
            "/snap/bin/google-chrome",
        ] {
            candidates.push(PathBuf::from(base));
        }
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join(
                ".local/share/flatpak/app/com.google.Chrome/current/active/files/extra/chrome",
            ));
            candidates.push(home.join(
                ".local/share/flatpak/app/org.chromium.Chromium/current/active/files/extra/chrome",
            ));
            candidates.push(home.join(".local/share/flatpak/exports/bin/com.google.Chrome"));
            candidates.push(home.join(".local/share/flatpak/exports/bin/org.chromium.Chromium"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        for base in [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "/opt/homebrew/bin/chromium",
            "/opt/homebrew/bin/google-chrome",
            "/usr/local/bin/chromium",
            "/usr/local/bin/google-chrome",
        ] {
            candidates.push(PathBuf::from(base));
        }
        if let Some(home) = dirs::home_dir() {
            candidates.push(
                home.join("Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            );
            candidates.push(home.join("Applications/Chromium.app/Contents/MacOS/Chromium"));
            candidates.push(
                home.join("Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
            );
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Relative install suffixes under Program Files / LocalAppData.
        // Order: stable Chrome → Beta → Canary (SxS) → Edge → Chromium.
        const RELATIVE: &[&str] = &[
            r"Google\Chrome\Application\chrome.exe",
            r"Google\Chrome Beta\Application\chrome.exe",
            r"Google\Chrome SxS\Application\chrome.exe",
            r"Microsoft\Edge\Application\msedge.exe",
            r"Chromium\Application\chrome.exe",
        ];

        // Prefer env-resolved roots (Unicode-safe via OsString) over hardcoded C:\.
        for env_key in ["PROGRAMFILES", "ProgramFiles", "PROGRAMFILES(X86)", "ProgramFiles(x86)"] {
            if let Ok(root) = std::env::var_os(env_key) {
                let root = PathBuf::from(root);
                for rel in RELATIVE {
                    candidates.push(root.join(rel));
                }
            }
        }
        if let Ok(localappdata) = std::env::var_os("LOCALAPPDATA") {
            let base = PathBuf::from(localappdata);
            candidates.push(base.join(r"Google\Chrome\Application\chrome.exe"));
            candidates.push(base.join(r"Google\Chrome Beta\Application\chrome.exe"));
            candidates.push(base.join(r"Google\Chrome SxS\Application\chrome.exe"));
            candidates.push(base.join(r"Chromium\Application\chrome.exe"));
            candidates.push(base.join(r"Microsoft\Edge\Application\msedge.exe"));
        }
        // Hardcoded fallbacks when env vars are missing (minimal Windows shells).
        for base in [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Chromium\Application\chrome.exe",
        ] {
            candidates.push(PathBuf::from(base));
        }
    }

    candidates
}

/// Infers [`ChromeChannel`] from a resolved binary path (not the original wrapper).
#[must_use]
pub fn classify_chrome_channel(path: &Path) -> ChromeChannel {
    let s = path.to_string_lossy();
    if s.contains("/flatpak/app/") || s.contains("\\flatpak\\app\\") {
        return ChromeChannel::Flatpak;
    }
    if s.starts_with("/snap/") || s.contains("/snap/bin/") {
        return ChromeChannel::Snap;
    }
    ChromeChannel::Host
}

/// Resolves a user-facing path (ELF, Flatpak export shell, or Fedora wrapper)
/// into a real browser binary path.
///
/// GAP-WS-AGENT-READY-001 / GAP-NEW-005: shell scripts are never passed to
/// chromiumoxide. Flatpak exports (`flatpak run com.google.Chrome`) map to
/// `files/extra/chrome`. Fedora `/usr/bin/chromium-browser` wrappers map to
/// lib64/lib ELF paths.
///
/// Never uses `flatpak-spawn --host` with untrusted interpolation.
#[must_use]
pub fn resolve_chrome_candidate(path: &Path) -> Option<PathBuf> {
    if is_executable_chrome_binary(path) {
        return Some(path.to_path_buf());
    }
    if !path.is_file() {
        return None;
    }
    // Try reading shell content for flatpak run / known wrappers.
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let content_lower = content.to_lowercase();

    #[cfg(target_os = "linux")]
    {
        if content_lower.contains("flatpak") && content_lower.contains("run") {
            if let Some(app_id) = extract_flatpak_app_id(&content) {
                if let Some(elf) = flatpak_deploy_chrome_elf(&app_id) {
                    if is_executable_chrome_binary(&elf) {
                        tracing::info!(
                            from = %path.display(),
                            to = %elf.display(),
                            app_id = %app_id,
                            "resolved Flatpak export shell to deploy ELF"
                        );
                        return Some(elf);
                    }
                }
            }
            // Path-based fallback from known export locations.
            let s = path.to_string_lossy();
            if s.contains("com.google.Chrome") {
                if let Some(elf) = flatpak_deploy_chrome_elf("com.google.Chrome") {
                    if is_executable_chrome_binary(&elf) {
                        return Some(elf);
                    }
                }
            }
            if s.contains("org.chromium.Chromium") {
                if let Some(elf) = flatpak_deploy_chrome_elf("org.chromium.Chromium") {
                    if is_executable_chrome_binary(&elf) {
                        return Some(elf);
                    }
                }
            }
        }

        // Fedora/RHEL chromium-browser.sh wrapper (and symlink to it).
        let s = path.to_string_lossy();
        if s.contains("chromium-browser") || content_lower.contains("chromium-browser") {
            for candidate in [
                "/usr/lib64/chromium-browser/chromium-browser",
                "/usr/lib/chromium-browser/chromium-browser",
                "/usr/lib/chromium-browser/chrome",
            ] {
                let p = PathBuf::from(candidate);
                if is_executable_chrome_binary(&p) {
                    tracing::info!(
                        from = %path.display(),
                        to = %p.display(),
                        "resolved Chromium shell wrapper to host ELF"
                    );
                    return Some(p);
                }
            }
        }
    }

    let _ = content;
    None
}

/// Parses `com.google.Chrome` / `org.chromium.Chromium` from a Flatpak export script.
#[cfg(target_os = "linux")]
fn extract_flatpak_app_id(script: &str) -> Option<String> {
    for token in script.split_whitespace() {
        if token == "com.google.Chrome" || token.starts_with("com.google.Chrome") {
            return Some("com.google.Chrome".to_string());
        }
        if token == "org.chromium.Chromium" || token.starts_with("org.chromium.Chromium") {
            return Some("org.chromium.Chromium".to_string());
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn extract_flatpak_app_id(_script: &str) -> Option<String> {
    None
}

/// Locates Flatpak deploy ELF for a given app-id (system then user install).
#[cfg(target_os = "linux")]
fn flatpak_deploy_chrome_elf(app_id: &str) -> Option<PathBuf> {
    let mut paths = Vec::new();
    paths.push(PathBuf::from(format!(
        "/var/lib/flatpak/app/{app_id}/current/active/files/extra/chrome"
    )));
    // Some Chromium Flatpaks ship as `chrome` or `chromium`.
    paths.push(PathBuf::from(format!(
        "/var/lib/flatpak/app/{app_id}/current/active/files/bin/chromium"
    )));
    paths.push(PathBuf::from(format!(
        "/var/lib/flatpak/app/{app_id}/current/active/files/bin/chrome"
    )));
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(format!(
            ".local/share/flatpak/app/{app_id}/current/active/files/extra/chrome"
        )));
        paths.push(home.join(format!(
            ".local/share/flatpak/app/{app_id}/current/active/files/bin/chromium"
        )));
        paths.push(home.join(format!(
            ".local/share/flatpak/app/{app_id}/current/active/files/bin/chrome"
        )));
    }
    paths.into_iter().find(|p| is_executable_chrome_binary(p))
}

#[cfg(not(target_os = "linux"))]
fn flatpak_deploy_chrome_elf(_app_id: &str) -> Option<PathBuf> {
    None
}

/// Detects Chrome/Chromium with multi-channel resolution (GAP-WS-AGENT-READY-001).
///
/// Resolution order (GAP-SCRAPE-R2-003/004 — no product env):
/// 1. `manual_path` (`--chrome-path`) — resolve shell wrappers; hard-fail if invalid.
/// 2. `which` on common binary names — resolve wrappers.
/// 3. [`chrome_candidate_paths`] — first resolvable real binary wins.
///
/// # Errors
///
/// Returns an error if `manual_path` cannot be resolved to a real binary, or if
/// no Chrome/Chromium executable is found on the system.
pub fn detect_chrome(manual_path: Option<&Path>) -> Result<PathBuf, CliError> {
    Ok(detect_chrome_resolved(manual_path)?.path)
}

/// Like [`detect_chrome`] but returns channel metadata for agent JSON fields.
///
/// # Errors
///
/// Same as [`detect_chrome`].
pub fn detect_chrome_resolved(manual_path: Option<&Path>) -> Result<ResolvedChrome, CliError> {
    // Layer 1: manual --chrome-path (highest priority; fail closed after resolve).
    if let Some(p) = manual_path {
        if let Some(resolved) = resolve_chrome_candidate(p) {
            let channel = ChromeChannel::Manual;
            tracing::info!(
                path = %resolved.display(),
                requested = %p.display(),
                canal = channel.as_str(),
                "Chrome found via --chrome-path"
            );
            return Ok(ResolvedChrome {
                path: resolved,
                channel,
            });
        }
        return Err(CliError::PathError {
            message: format!(
                "--chrome-path {:?} is not a valid Chrome/Chromium binary (missing, shell wrapper without resolvable ELF, or not a file). \
                 On Fedora try /usr/lib64/chromium-browser/chromium-browser. \
                 For Flatpak Chrome try the deploy ELF under \
                 /var/lib/flatpak/app/com.google.Chrome/current/active/files/extra/chrome \
                 (export scripts under flatpak/exports/bin are resolved automatically when the deploy exists).",
                p.display()
            ),
        });
    }

    // Layer 2: PATH lookup via `which` crate (platform binary names).
    #[cfg(windows)]
    let which_names: &[&str] = &[
        "chrome.exe",
        "chrome",
        "msedge.exe",
        "msedge",
        "chromium.exe",
        "chromium",
    ];
    #[cfg(not(windows))]
    let which_names: &[&str] = &[
        "google-chrome",
        "google-chrome-stable",
        "google-chrome-beta",
        "google-chrome-unstable",
        "chromium",
        "chromium-browser",
        "chrome",
        "microsoft-edge",
        "microsoft-edge-stable",
        "msedge",
    ];
    for binary_name in which_names {
        if let Ok(p) = which::which(binary_name) {
            if let Some(resolved) = resolve_chrome_candidate(&p) {
                let channel = classify_chrome_channel(&resolved);
                tracing::info!(
                    binary = binary_name,
                    path = %resolved.display(),
                    canal = channel.as_str(),
                    "Chrome found via PATH lookup (which crate)"
                );
                return Ok(ResolvedChrome {
                    path: resolved,
                    channel,
                });
            }
            tracing::debug!(
                binary = binary_name,
                path = %p.display(),
                "which crate found candidate but could not resolve to a real binary"
            );
        }
    }

    // Layer 4: platform-specific well-known installation paths.
    for candidate in chrome_candidate_paths() {
        if let Some(resolved) = resolve_chrome_candidate(&candidate) {
            let channel = classify_chrome_channel(&resolved);
            tracing::info!(
                path = %resolved.display(),
                canal = channel.as_str(),
                "Chrome found at platform-specific path"
            );
            return Ok(ResolvedChrome {
                path: resolved,
                channel,
            });
        }
    }

    Err(CliError::PathError {
        message: "Chrome/Chromium not found. Install via package manager, Flatpak (com.google.Chrome), or provide --chrome-path to a real binary (not a shell-only wrapper).".into(),
    })
}

/// v0.8.0 GAP-NEW-005: rejects shell-script wrappers (e.g. `chromium-browser.sh`)
/// which call the Rust `timeout` crate binary and kill Chrome in ~0.1s. Validates
/// that the candidate is a real ELF/Mach-O executable, not a text file.
///
/// Prefer [`resolve_chrome_candidate`] when accepting user paths — it maps known
/// shells to deploy/host ELFs before this check is applied to the result.
pub(crate) fn is_executable_chrome_binary(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let path_str = path.to_string_lossy();
    // Reject shell / batch wrappers (GAP-PROC-006 / CVE-2024-24576 BatBadBut).
    // `.bat`/`.cmd` must never be spawned with untrusted args on Windows;
    // `.sh` wrappers kill Chrome via nested `timeout` (GAP-NEW-005).
    let lower = path_str.to_ascii_lowercase();
    if lower.ends_with(".sh")
        || lower.ends_with(".bat")
        || lower.ends_with(".cmd")
        || lower.ends_with(".ps1")
    {
        tracing::debug!(
            path = %path.display(),
            "rejecting script/batch wrapper (not a native Chrome binary)"
        );
        return false;
    }
    // Verify ELF magic bytes (Linux) or Mach-O (macOS) / PE (Windows).
    // Read only the first bytes — Flatpak Chrome ELF is ~280 MiB.
    match std::fs::File::open(path).and_then(|mut f| {
        use std::io::Read;
        let mut magic = [0u8; 4];
        f.read_exact(&mut magic)?;
        Ok(magic)
    }) {
        Ok(bytes) => {
            let is_elf = &bytes[0..4] == b"\x7fELF";
            let is_macho = bytes[0..4] == [0xCF, 0xFA, 0xED, 0xFE]
                || bytes[0..4] == [0xFE, 0xED, 0xFA, 0xCE]
                || bytes[0..4] == [0xFE, 0xED, 0xFA, 0xCF]
                || bytes[0..4] == [0xCA, 0xFE, 0xBA, 0xBE];
            let is_pe = &bytes[0..2] == b"MZ";
            is_elf || is_macho || is_pe
        }
        _ => false,
    }
}


/// Indicates whether we are running inside a container or Flatpak/Snap wrapper, which
/// requires `--no-sandbox` for Chrome to work.
///
/// GAP-WS-AGENT-READY-001 v0.9.8: also true for Flatpak **deploy** ELFs under
/// `/flatpak/app/` and `files/extra/chrome` (not only export scripts).
/// GAP-MP-002 (Pass 20): also honors `$FLATPAK_ID` / `$SNAP` process sandboxes
/// and centralized container detection from [`crate::platform`].
pub fn needs_no_sandbox(chrome_path: &Path) -> bool {
    // Process-level sandboxes (CLI itself running inside Flatpak/Snap).
    if crate::platform::is_flatpak_sandbox() || crate::platform::is_snap_sandbox() {
        tracing::warn!(
            "CLI is running inside Flatpak/Snap confinement — Chrome may need --no-sandbox; \
             prefer a host package install if automation fails"
        );
        return true;
    }

    #[cfg(target_os = "linux")]
    {
        let s = chrome_path.to_string_lossy();
        if s.contains("flatpak/exports/bin")
            || s.contains("/flatpak/app/")
            || s.contains("files/extra/chrome")
            || s.starts_with("/snap/")
            || s.contains("/.var/app/")
        {
            tracing::debug!(
                path = %chrome_path.display(),
                "Chrome binary is under Flatpak/Snap path — enabling --no-sandbox"
            );
            return true;
        }
        // Containers (Docker/Podman/k8s) and root often lack user namespaces.
        if crate::platform::is_container() {
            return true;
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = chrome_path;
    }
    false
}

/// Max wall-clock budget for `chrome --version` probe (GAP-PROC-002).
///
/// Non-interactive CLI must not hang if a broken/stub binary never exits.
const CHROME_VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Hard cap on captured `--version` stdout (GAP-PROC-002 buffer bound).
const CHROME_VERSION_STDOUT_CAP: usize = 4096;

/// Detects the installed Chrome/Chromium major version via `<path> --version`
/// (GAP-WS-109 v0.9.2 + GAP-PROC-002).
///
/// Parses the first digit group after `Chrome ` or `Chromium ` in the output
/// of `--version` (e.g. `"Google Chrome 149.0.7827.201"` → `149`). Returns
/// `None` if spawn fails, the process times out, exit status is non-zero, or
/// the version is not parseable.
///
/// Process contract (rules-rust-processos-externos + security):
/// - `stdin`/`stderr` = `null` (no TTY attach / no noise merge)
/// - `stdout` = `piped` with a hard byte cap
/// - `env_clear` + minimal PATH (and Windows SystemRoot) — no inherited secrets
/// - exit status verified before treating stdout as valid
/// - wall-clock timeout with kill on expiry (no orphan probe)
pub fn detect_chrome_major_version(path: &Path) -> Option<u32> {
    use std::io::Read;
    use std::process::Stdio;

    let mut cmd = std::process::Command::new(path);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_clear()
        // Controlled cwd: never inherit a caller-controlled working directory
        // that could redirect relative library/config loads (process security rules).
        .current_dir(std::env::temp_dir());
    // Minimal env for a version probe — no proxy/credential inheritance.
    if let Some(path_env) = std::env::var_os("PATH") {
        cmd.env("PATH", path_env);
    }
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("SystemRoot") {
            cmd.env("SystemRoot", root);
        }
        if let Some(windir) = std::env::var_os("WINDIR") {
            cmd.env("WINDIR", windir);
        }
    }
    // Some Linux Chrome builds need lib path from the install prefix.
    if let Some(ld) = std::env::var_os("LD_LIBRARY_PATH") {
        cmd.env("LD_LIBRARY_PATH", ld);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            tracing::debug!(
                path = %path.display(),
                ?err,
                "chrome --version spawn failed"
            );
            return None;
        }
    };

    let mut stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    };

    // Drain stdout on a side thread so a full pipe cannot deadlock the wait loop.
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 512];
        loop {
            match stdout.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let room = CHROME_VERSION_STDOUT_CAP.saturating_sub(buf.len());
                    if room == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n.min(room)]);
                    if buf.len() >= CHROME_VERSION_STDOUT_CAP {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        buf
    });

    let deadline = std::time::Instant::now() + CHROME_VERSION_PROBE_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                tracing::warn!(
                    path = %path.display(),
                    timeout_ms = CHROME_VERSION_PROBE_TIMEOUT.as_millis() as u64,
                    "chrome --version timed out — killing probe child"
                );
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return None;
            }
            Err(err) => {
                tracing::debug!(?err, path = %path.display(), "chrome --version wait error");
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return None;
            }
        }
    };

    let out = reader.join().ok()?;
    // Deterministic exit-code gate before treating stdout as valid (PROC rules).
    if !status.success() {
        tracing::debug!(
            path = %path.display(),
            code = ?status.code(),
            "chrome --version exited non-zero — ignoring stdout"
        );
        return None;
    }

    let line = String::from_utf8_lossy(&out);
    let after = line
        .split("Chrome ")
        .nth(1)
        .or_else(|| line.split("Chromium ").nth(1))?;
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok()
}
