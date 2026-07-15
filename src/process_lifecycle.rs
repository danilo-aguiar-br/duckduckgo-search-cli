// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: system (external process tree ownership / one-shot reap)
//! One-shot ownership helpers for external process trees
//! (GAP-WS-LIFECYCLE-001 + GAP-WS-TMP-PROFILE-ORPHAN-001).
//!
//! Chromium is multi-process; `Child::kill` / `kill_on_drop` only target the
//! root process. Xvfb is a sibling of Chrome, not a child of it. This module
//! provides process-group kill, tree walk, cmdline-marker sweep, and **profile
//! directory removal** so a CLI invocation leaves **zero** automation browsers,
//! private displays, or identifiable profile dirs behind
//! (`rules-rust-cli-one-shot` + `rules-rust-processos-externos` + shutdown family).
//!
//! Safety nets (no libc `atexit` — async-signal unsafe / redundant with RAII):
//! - panic hook → [`reap_all_registered`]
//! - [`ExitReapGuard`] on `main` Drop → [`reap_all_registered`]
//! - explicit reap after global timeout / cancel paths in `lib::run`
//! - start of each run → [`sweep_orphan_profiles`] (SIGKILL/OOM residual)
//!
//! # Disk hygiene policy (hard rules — GAP-WS-TMP-PROFILE-ORPHAN-001)
//!
//! 1. **Never** auto-delete legacy generic tempdirs (names starting with `.tmp`)
//!    — they may belong to other Rust apps / tools on the host.
//! 2. **Never** auto-delete global Chromium stubs such as
//!    `org.chromium.Chromium.*` under `temp_dir` — may belong to desktop Chrome,
//!    Flatpak, or MCP debug profiles.
//! 3. **Only** auto-sweep this CLI's profiles: names starting with
//!    [`USER_DATA_DIR_PREFIX`] (`ddg-chrome-`). That is the recovery path for
//!    residual left by SIGKILL / OOM of a previous invocation.
//!
//! No remote telemetry. Logs use `tracing` to stderr only.

#![cfg(feature = "chrome")]

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// Auditable prefix for per-invocation Chrome user-data-dir under `temp_dir`.
///
/// Matches the atomwrite convention of project-scoped temp prefixes
/// (see `paths.rs` `.ddg-atomic-`). Never use the tempfile default `.tmp`.
pub const USER_DATA_DIR_PREFIX: &str = "ddg-chrome-";

/// Chromium OS temp stub prefix (sockets/cookies). **Not** owned by this CLI's
/// hygiene policy — must never be mass-deleted by [`sweep_orphan_profiles`].
pub const CHROMIUM_GLOBAL_STUB_PREFIX: &str = "org.chromium.Chromium.";

/// tempfile crate default directory name prefix. **Not** owned by this CLI —
/// must never be mass-deleted (collides with other Rust applications).
pub const FOREIGN_TEMPFILE_DIR_PREFIX: &str = ".tmp";

/// Returns `true` only when `file_name` is a profile directory owned by this CLI.
///
/// Used by [`sweep_orphan_profiles`] and as a hard guard in [`remove_user_data_dir`].
#[must_use]
pub fn is_cli_owned_profile_name(file_name: &str) -> bool {
    file_name.starts_with(USER_DATA_DIR_PREFIX)
}

/// Returns `true` when the name must **never** be bulk-deleted by this CLI.
///
/// Covers generic Rust tempdirs (`.tmp*`) and Chromium global stubs
/// (`org.chromium.Chromium.*`).
#[must_use]
pub fn is_forbidden_bulk_delete_name(file_name: &str) -> bool {
    file_name.starts_with(FOREIGN_TEMPFILE_DIR_PREFIX)
        || file_name.starts_with(CHROMIUM_GLOBAL_STUB_PREFIX)
        // Also reject bare / partial Chromium app-id style names without suffix.
        || file_name == "org.chromium.Chromium"
        || file_name.starts_with("org.chromium.Chromium")
}

/// Snapshot of a browser automation session used for forced reaping.
#[derive(Debug, Clone)]
pub struct SessionIds {
    /// Root Chromium browser process id, if launched.
    pub chrome_pid: Option<u32>,
    /// Private Xvfb process id, if started (Linux).
    pub xvfb_pid: Option<u32>,
    /// Xvfb process-group id (usually equals `xvfb_pid` after `setpgid(0,0)`).
    pub xvfb_pgid: Option<i32>,
    /// Absolute path of the per-invocation Chrome user-data-dir (kill marker).
    pub user_data_dir: PathBuf,
    /// X11 display string such as `":99"`, when Xvfb was used.
    pub display: Option<String>,
}

/// Global registry of live sessions for best-effort panic / exit-guard reaping.
static SESSION_REGISTRY: Mutex<Vec<SessionIds>> = Mutex::new(Vec::new());

/// Poison-safe lock helper (never panics; recovers poisoned mutex content).
fn lock_registry() -> std::sync::MutexGuard<'static, Vec<SessionIds>> {
    SESSION_REGISTRY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Registers a session so a panic / exit path can still attempt reaping.
pub fn register_session(session: SessionIds) {
    lock_registry().push(session);
}

/// Removes sessions whose `user_data_dir` matches (called after clean finalize).
pub fn unregister_session(user_data_dir: &Path) {
    lock_registry().retain(|s| s.user_data_dir != user_data_dir);
}

/// Best-effort reap of every registered session (panic hook / [`ExitReapGuard`] /
/// timeout path). Kills processes **and** removes profile dirs.
pub fn reap_all_registered() {
    let sessions = std::mem::take(&mut *lock_registry());
    for session in sessions {
        force_reap(&session);
    }
}

/// RAII guard: on create, sweeps orphan `ddg-chrome-*` from prior SIGKILL/OOM;
/// on Drop, reaps all registered browser sessions (process + disk).
///
/// Place at the top of `main` so:
/// - residual from a **previous** killed run is cleaned at the start of this run
/// - cooperative and abrupt Rust unwinds still attempt one-shot cleanup
///
/// Idempotent with per-session finalize. Never bulk-deletes `.tmp*` or
/// `org.chromium.Chromium.*` (see module policy).
#[derive(Debug)]
pub struct ExitReapGuard;

impl ExitReapGuard {
    /// Install start-of-run orphan sweep (SIGKILL residual of prior invocation).
    #[must_use]
    pub fn new() -> Self {
        // Policy §3: only `ddg-chrome-*`; never `.tmp*` / `org.chromium.Chromium.*`.
        sweep_orphan_profiles();
        Self
    }
}

impl Default for ExitReapGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExitReapGuard {
    fn drop(&mut self) {
        reap_all_registered();
    }
}

/// Installs a panic hook that reaps registered browser sessions, chaining the
/// previous hook. On first install also sweeps orphan `ddg-chrome-*` dirs from
/// prior SIGKILL'd invocations (best-effort, prefix-only).
pub fn install_panic_reap_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // Recover residual profiles left by SIGKILL / OOM of a previous run.
        sweep_orphan_profiles();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            reap_all_registered();
            previous(info);
        }));
    });
}

/// Synchronous forced cleanup of a session (Chrome tree + Xvfb + locks + disk).
///
/// Idempotent and best-effort: never panics.
pub fn force_reap(session: &SessionIds) {
    if let Some(pid) = session.chrome_pid {
        kill_process_tree(pid);
    }
    let marker = session.user_data_dir.to_string_lossy();
    if !marker.is_empty() {
        kill_by_cmdline_substring(&marker);
    }
    if let Some(pgid) = session.xvfb_pgid {
        kill_process_group(pgid);
    } else if let Some(pid) = session.xvfb_pid {
        kill_pid(pid);
    }
    if let Some(ref display) = session.display {
        cleanup_xvfb_display_files(display);
    }
    // Disk one-shot: remove profile after processes are dead (GAP-WS-TMP-PROFILE-ORPHAN-001).
    remove_user_data_dir(&session.user_data_dir);
    tracing::info!(
        chrome_pid = ?session.chrome_pid,
        xvfb_pid = ?session.xvfb_pid,
        user_data = %session.user_data_dir.display(),
        "force_reap completed for browser session (process + disk)"
    );
}

/// Best-effort recursive removal of a Chrome user-data-dir (idempotent).
///
/// **Hard guard:** refuses paths whose final component is not
/// [`is_cli_owned_profile_name`] — never deletes `.tmp*` or
/// `org.chromium.Chromium.*` even if a caller passes a wrong path.
///
/// Order assumed by callers: kill tree/marker first, then call this. Sleeps
/// briefly then retries once if the first `remove_dir_all` leaves residual.
pub fn remove_user_data_dir(path: &Path) {
    if path.as_os_str().is_empty() || !path.exists() {
        return;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    if !is_cli_owned_profile_name(name) {
        tracing::warn!(
            path = %path.display(),
            "refusing remove_user_data_dir — path is not a ddg-chrome-* profile \
             (policy: never bulk-delete .tmp* or org.chromium.Chromium.*)"
        );
        return;
    }
    // Brief settle so Chromium releases file handles after SIGKILL.
    std::thread::sleep(Duration::from_millis(50));
    if let Err(e) = std::fs::remove_dir_all(path) {
        tracing::info!(
            path = %path.display(),
            error = %e,
            "first remove_dir_all of user-data-dir failed — retrying"
        );
        std::thread::sleep(Duration::from_millis(80));
        if path.exists() {
            if let Err(e2) = std::fs::remove_dir_all(path) {
                tracing::warn!(
                    path = %path.display(),
                    error = %e2,
                    "user-data-dir residual after retry (best-effort; SIGKILL/OOM limit)"
                );
                return;
            }
        }
    }
    if !path.exists() {
        tracing::info!(path = %path.display(), "user-data-dir removed");
    }
}

/// Sweeps orphan **`ddg-chrome-*` only** under `std::env::temp_dir()`.
///
/// Recovery for residual left when a previous CLI was SIGKILL'd / OOM-killed
/// (destructors and panic hooks did not run). Safe concurrent runs: skips dirs
/// still referenced by a live process cmdline marker or live `SingletonLock` PID.
///
/// # Hard non-goals (enforced)
///
/// - Does **not** delete names matching [`.tmp*`](FOREIGN_TEMPFILE_DIR_PREFIX)
///   (legacy 0.9.x or other Rust apps).
/// - Does **not** delete [`org.chromium.Chromium.*`](CHROMIUM_GLOBAL_STUB_PREFIX)
///   (desktop Chrome / Flatpak / MCP).
pub fn sweep_orphan_profiles() {
    let temp = std::env::temp_dir();
    let Ok(entries) = std::fs::read_dir(&temp) else {
        return;
    };
    let mut removed = 0u32;
    let mut skipped_foreign = 0u32;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Explicit reject first (defense in depth + observability).
        if is_forbidden_bulk_delete_name(&name_str) {
            skipped_foreign = skipped_foreign.saturating_add(1);
            continue;
        }
        if !is_cli_owned_profile_name(&name_str) {
            continue;
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let marker = path.to_string_lossy();
        if marker_in_use(&marker) {
            continue;
        }
        // Optional SingletonLock check: if lock points to a live PID, skip.
        if singleton_lock_alive(&path) {
            continue;
        }
        tracing::info!(
            path = %path.display(),
            "sweeping orphan ddg-chrome profile (SIGKILL/OOM residual recovery)"
        );
        // remove_user_data_dir re-checks ownership prefix.
        remove_user_data_dir(&path);
        if !path.exists() {
            removed = removed.saturating_add(1);
        }
    }
    if removed > 0 || skipped_foreign > 0 {
        tracing::info!(
            removed,
            skipped_foreign_candidates = skipped_foreign,
            "orphan profile sweep finished (ddg-chrome-* only)"
        );
    }
}

/// Returns true if any process (except self) has `marker` in its cmdline.
fn marker_in_use(marker: &str) -> bool {
    if marker.is_empty() {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return false;
        };
        let self_pid = std::process::id();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(pid) = name.parse::<u32>() else {
                continue;
            };
            if pid == self_pid {
                continue;
            }
            let cmdline_path = format!("/proc/{pid}/cmdline");
            let Ok(bytes) = std::fs::read(&cmdline_path) else {
                continue;
            };
            if bytes_contains_str(&bytes, marker) {
                return true;
            }
        }
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = marker;
        false
    }
}

/// If `SingletonLock` exists and names a live PID, the profile is still owned.
fn singleton_lock_alive(profile: &Path) -> bool {
    let lock = profile.join("SingletonLock");
    // Chromium often uses a symlink SingletonLock → hostname-PID
    let target = if lock.is_symlink() {
        std::fs::read_link(&lock).ok()
    } else if lock.exists() {
        // Regular file: try read contents
        std::fs::read_to_string(&lock)
            .ok()
            .map(PathBuf::from)
    } else {
        return false;
    };
    let Some(target) = target else {
        return false;
    };
    let s = target.to_string_lossy();
    // Format commonly "hostname-12345"
    let pid_str = s.rsplit('-').next().unwrap_or("");
    let Ok(pid) = pid_str.parse::<u32>() else {
        return false;
    };
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        false
    }
}

/// Sends SIGKILL (or Windows terminate) to a single PID and best-effort waits.
pub fn kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        // SAFETY: kill with a concrete PID is process-scoped; ESRCH is ignored.
        let _ = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        std::thread::sleep(Duration::from_millis(20));
    }
    #[cfg(windows)]
    {
        windows_terminate_pid(pid);
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
    }
}

/// Unix: SIGTERM then SIGKILL to an entire process group. `pgid` must be > 1.
pub fn kill_process_group(pgid: i32) {
    if pgid <= 1 {
        return;
    }
    #[cfg(unix)]
    {
        // SAFETY: negative PID targets the process group; we never use pgid 1.
        let _ = unsafe { libc::kill(-pgid, libc::SIGTERM) };
        std::thread::sleep(Duration::from_millis(50));
        // SAFETY: same process-group kill with SIGKILL after grace period.
        let _ = unsafe { libc::kill(-pgid, libc::SIGKILL) };
        std::thread::sleep(Duration::from_millis(20));
    }
    #[cfg(not(unix))]
    {
        let _ = pgid;
    }
}

/// Walks descendants of `root_pid` and SIGKILLs them bottom-up, then the root.
pub fn kill_process_tree(root_pid: u32) {
    #[cfg(target_os = "linux")]
    {
        let mut all = Vec::new();
        let mut queue = VecDeque::from([root_pid]);
        let mut seen = HashSet::from([root_pid]);
        while let Some(pid) = queue.pop_front() {
            all.push(pid);
            for child in linux_children_of(pid) {
                if seen.insert(child) {
                    queue.push_back(child);
                }
            }
        }
        // Kill children first (skip root until end).
        for pid in all.iter().rev() {
            if *pid != root_pid {
                kill_pid(*pid);
            }
        }
        kill_pid(root_pid);
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        // macOS / BSD: pkill by parent is fragile; kill root and rely on marker sweep.
        kill_pid(root_pid);
    }
    #[cfg(windows)]
    {
        windows_terminate_tree(root_pid);
    }
}

/// Kills every process whose `/proc/pid/cmdline` (or Windows image cmdline)
/// contains `marker`. Marker must be a unique per-invocation path (profile dir).
pub fn kill_by_cmdline_substring(marker: &str) {
    if marker.is_empty() {
        return;
    }
    #[cfg(target_os = "linux")]
    {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(pid) = name.parse::<u32>() else {
                continue;
            };
            if pid == std::process::id() {
                continue;
            }
            let cmdline_path = format!("/proc/{pid}/cmdline");
            let Ok(bytes) = std::fs::read(&cmdline_path) else {
                continue;
            };
            // cmdline is NUL-separated; treat as opaque byte haystack.
            if bytes_contains_str(&bytes, marker) {
                tracing::info!(pid, %marker, "killing process matching user-data-dir marker");
                kill_pid(pid);
            }
        }
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        // Best-effort: `pgrep -f` is not available in pure Rust without Command;
        // root kill + tree already ran. Avoid shell injection: use /bin/ps only
        // for diagnostics is overkill — macOS relies on kill_process_tree root.
        let _ = marker;
    }
    #[cfg(windows)]
    {
        windows_kill_by_cmdline_substring(marker);
    }
}

/// Removes Xvfb lock and Unix socket for a display like `":99"`.
pub fn cleanup_xvfb_display_files(xvfb_display: &str) {
    let num = xvfb_display.trim().trim_start_matches(':');
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return;
    }
    let lock = format!("/tmp/.X{num}-lock");
    let socket = format!("/tmp/.X11-unix/X{num}");
    let _ = std::fs::remove_file(&lock);
    let _ = std::fs::remove_file(&socket);
    tracing::info!(display = %xvfb_display, "cleaned Xvfb lock/socket files");
}

/// Configures a `std::process::Command` so the child becomes a process-group
/// leader and receives SIGKILL if this CLI dies (Linux PDEATHSIG).
#[cfg(unix)]
pub fn apply_process_group_and_pdeathsig(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // Clippy wants one unsafe op per block; pre_exec closure must stay atomic.
    #[allow(clippy::multiple_unsafe_ops_per_block)]
    // SAFETY: `pre_exec` runs in the child between fork and exec. Only
    // async-signal-safe syscalls (`setpgid`, `prctl`, `getppid`, `_exit`).
    // No heap allocation. Child becomes its own process group leader and
    // receives SIGKILL if the parent dies (Linux).
    unsafe {
        cmd.pre_exec(|| {
            // New process group with pgid == pid of child.
            if libc::setpgid(0, 0) != 0 {
                // Non-fatal: continue without group leadership.
            }
            #[cfg(target_os = "linux")]
            {
                // PR_SET_PDEATHSIG = 1 — deliver SIGKILL when parent dies.
                const PR_SET_PDEATHSIG: libc::c_int = 1;
                let _ = libc::prctl(PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong, 0, 0, 0);
                // If parent already died between fork and prctl, exit now.
                if libc::getppid() == 1 {
                    libc::_exit(128 + libc::SIGKILL);
                }
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
pub fn apply_process_group_and_pdeathsig(_cmd: &mut std::process::Command) {}

fn bytes_contains_str(haystack: &[u8], needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let n = needle.as_bytes();
    haystack.windows(n.len()).any(|w| w == n)
}

#[cfg(target_os = "linux")]
fn linux_children_of(pid: u32) -> Vec<u32> {
    let path = format!("/proc/{pid}/task/{pid}/children");
    let Ok(contents) = std::fs::read_to_string(path) else {
        // Fallback: scan /proc for ppid == pid via stat (field 4).
        return linux_children_via_stat_scan(pid);
    };
    contents
        .split_whitespace()
        .filter_map(|s| s.parse::<u32>().ok())
        .collect()
}

#[cfg(target_os = "linux")]
fn linux_children_via_stat_scan(parent: u32) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut kids = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let stat_path = format!("/proc/{pid}/stat");
        let Ok(stat) = std::fs::read_to_string(stat_path) else {
            continue;
        };
        // Format: pid (comm) state ppid ...
        if let Some(after_comm) = stat.rsplit(')').next() {
            let mut parts = after_comm.split_whitespace();
            let _state = parts.next();
            if let Some(ppid_s) = parts.next() {
                if ppid_s.parse::<u32>().ok() == Some(parent) {
                    kids.push(pid);
                }
            }
        }
    }
    kids
}

#[cfg(windows)]
fn windows_terminate_pid(pid: u32) {
    use windows_sys::Win32::Foundation::{CloseHandle, FALSE};
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    // SAFETY: OpenProcess/TerminateProcess with a numeric PID from our tree.
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
        // windows-sys 0.61: HANDLE is *mut c_void (not isize/0).
        if handle.is_null() {
            return;
        }
        let _ = TerminateProcess(handle, 1);
        let _ = CloseHandle(handle);
    }
}

#[cfg(windows)]
fn windows_terminate_tree(root_pid: u32) {
    // Snapshot + kill descendants, then root.
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut children_of: std::collections::HashMap<u32, Vec<u32>> =
        std::collections::HashMap::new();

    // SAFETY: Toolhelp snapshot of processes is standard Win32 usage.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            windows_terminate_pid(root_pid);
            return;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                children_of
                    .entry(entry.th32ParentProcessID)
                    .or_default()
                    .push(entry.th32ProcessID);
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }

    let mut all = Vec::new();
    let mut queue = VecDeque::from([root_pid]);
    let mut seen = HashSet::from([root_pid]);
    while let Some(pid) = queue.pop_front() {
        all.push(pid);
        if let Some(kids) = children_of.get(&pid) {
            for &c in kids {
                if seen.insert(c) {
                    queue.push_back(c);
                }
            }
        }
    }
    for pid in all.iter().rev() {
        windows_terminate_pid(*pid);
    }
}

#[cfg(windows)]
fn windows_kill_by_cmdline_substring(_marker: &str) {
    // Full WMI cmdline scan is heavy; tree terminate from chrome_pid is primary on Windows.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn bytes_contains_marker() {
        let hay = b"chrome\0--user-data-dir=/tmp/.tmpABCDEF\0--headless\0";
        assert!(bytes_contains_str(hay, "/tmp/.tmpABCDEF"));
        assert!(!bytes_contains_str(hay, "/tmp/.tmpOTHER"));
    }

    #[cfg(unix)]
    #[test]
    fn process_group_kill_reaps_child() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        apply_process_group_and_pdeathsig(&mut cmd);
        let mut child = cmd.spawn().expect("spawn sleep");
        let pid = child.id();
        // Child is group leader: pgid == pid.
        kill_process_group(pid as i32);
        // wait should complete quickly.
        let status = child.wait().expect("wait");
        assert!(!status.success() || status.code().is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn kill_by_marker_reaps_matching_process() {
        let marker = format!("ddg-lifecycle-marker-{}", std::process::id());
        // Use env so marker appears in /proc/pid/environ OR pass as arg.
        let mut child = Command::new("sleep")
            .arg("30")
            .arg(&marker)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn");
        let pid = child.id();
        std::thread::sleep(Duration::from_millis(50));
        kill_by_cmdline_substring(&marker);
        let _ = child.wait();
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "process {pid} should be gone after marker kill"
        );
    }

    #[test]
    fn cleanup_display_rejects_garbage() {
        cleanup_xvfb_display_files("not-a-display");
        cleanup_xvfb_display_files(":");
    }

    #[test]
    fn force_reap_empty_session_does_not_panic() {
        force_reap(&SessionIds {
            chrome_pid: None,
            xvfb_pid: None,
            xvfb_pgid: None,
            user_data_dir: PathBuf::from("/tmp/.tmp-nonexistent-ddg-test"),
            display: None,
        });
    }

    #[test]
    fn force_reap_removes_profile_directory() {
        let dir = std::env::temp_dir().join(format!(
            "{USER_DATA_DIR_PREFIX}unit-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create test profile dir");
        std::fs::write(dir.join("Preferences"), b"{}").expect("write marker file");
        assert!(dir.exists());
        force_reap(&SessionIds {
            chrome_pid: None,
            xvfb_pid: None,
            xvfb_pgid: None,
            user_data_dir: dir.clone(),
            display: None,
        });
        assert!(
            !dir.exists(),
            "force_reap must remove user-data-dir {}",
            dir.display()
        );
    }

    #[test]
    fn user_data_dir_prefix_is_ddg_chrome() {
        assert_eq!(USER_DATA_DIR_PREFIX, "ddg-chrome-");
        assert!(!USER_DATA_DIR_PREFIX.starts_with('.'));
    }

    #[test]
    fn ownership_predicates_encode_hygiene_policy() {
        assert!(is_cli_owned_profile_name("ddg-chrome-AbCd12"));
        assert!(!is_cli_owned_profile_name(".tmpABCDEF"));
        assert!(!is_cli_owned_profile_name("org.chromium.Chromium.xyz"));
        assert!(is_forbidden_bulk_delete_name(".tmpABCDEF"));
        assert!(is_forbidden_bulk_delete_name("org.chromium.Chromium.BDsv2K"));
        assert!(is_forbidden_bulk_delete_name("org.chromium.Chromium"));
        assert!(!is_forbidden_bulk_delete_name("ddg-chrome-AbCd12"));
    }

    #[test]
    fn remove_user_data_dir_refuses_foreign_tmp_prefix() {
        let alien = std::env::temp_dir().join(format!(
            "{FOREIGN_TEMPFILE_DIR_PREFIX}refuse-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&alien).expect("mkdir .tmp alien");
        std::fs::write(alien.join("x"), b"1").expect("write");
        remove_user_data_dir(&alien);
        assert!(
            alien.exists(),
            "must never remove .tmp* legacy/generic tempdirs"
        );
        let _ = std::fs::remove_dir_all(&alien);
    }

    #[test]
    fn remove_user_data_dir_refuses_chromium_global_stubs() {
        let stub = std::env::temp_dir().join(format!(
            "{CHROMIUM_GLOBAL_STUB_PREFIX}stub-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&stub).expect("mkdir chromium stub");
        std::fs::write(stub.join("x"), b"1").expect("write");
        remove_user_data_dir(&stub);
        assert!(
            stub.exists(),
            "must never remove org.chromium.Chromium.* global stubs"
        );
        let _ = std::fs::remove_dir_all(&stub);
    }

    #[test]
    fn sweep_orphan_ignores_generic_tmp_prefix() {
        let alien = std::env::temp_dir().join(format!(
            "{FOREIGN_TEMPFILE_DIR_PREFIX}ddg-sweep-alien-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&alien);
        sweep_orphan_profiles();
        assert!(
            alien.exists(),
            "sweep must never remove generic .tmp dirs (third-party risk)"
        );
        let _ = std::fs::remove_dir_all(&alien);
    }

    #[test]
    fn sweep_orphan_ignores_org_chromium_global_stubs() {
        let stub = std::env::temp_dir().join(format!(
            "{CHROMIUM_GLOBAL_STUB_PREFIX}sweep-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&stub).expect("mkdir stub");
        std::fs::write(stub.join("socket"), b"").expect("write");
        sweep_orphan_profiles();
        assert!(
            stub.exists(),
            "sweep must never remove org.chromium.Chromium.* (desktop/MCP risk)"
        );
        let _ = std::fs::remove_dir_all(&stub);
    }

    #[test]
    fn sweep_orphan_removes_stale_ddg_chrome_dir() {
        let dir = std::env::temp_dir().join(format!(
            "{USER_DATA_DIR_PREFIX}orphan-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create orphan");
        std::fs::write(dir.join("Local State"), b"{}").expect("write");
        sweep_orphan_profiles();
        assert!(
            !dir.exists(),
            "sweep should remove orphan ddg-chrome-* (SIGKILL residual recovery)"
        );
    }

    #[test]
    fn sweep_leaves_foreign_while_removing_owned() {
        let pid = std::process::id();
        let foreign_tmp = std::env::temp_dir().join(format!("{FOREIGN_TEMPFILE_DIR_PREFIX}mix-{pid}"));
        let foreign_chrome =
            std::env::temp_dir().join(format!("{CHROMIUM_GLOBAL_STUB_PREFIX}mix-{pid}"));
        let owned = std::env::temp_dir().join(format!("{USER_DATA_DIR_PREFIX}mix-{pid}"));
        for p in [&foreign_tmp, &foreign_chrome, &owned] {
            std::fs::create_dir_all(p).expect("mkdir");
            std::fs::write(p.join("f"), b"1").expect("write");
        }
        sweep_orphan_profiles();
        assert!(foreign_tmp.exists(), ".tmp* must survive sweep");
        assert!(foreign_chrome.exists(), "org.chromium.* must survive sweep");
        assert!(!owned.exists(), "ddg-chrome-* orphan must be swept");
        let _ = std::fs::remove_dir_all(&foreign_tmp);
        let _ = std::fs::remove_dir_all(&foreign_chrome);
    }

    #[test]
    fn exit_reap_guard_drop_does_not_panic() {
        let guard = ExitReapGuard::new();
        drop(guard);
    }
}
