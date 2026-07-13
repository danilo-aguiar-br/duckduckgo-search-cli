// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: system (external process tree ownership / one-shot reap)
//! One-shot ownership helpers for external process trees (GAP-WS-LIFECYCLE-001).
//!
//! Chromium is multi-process; `Child::kill` / `kill_on_drop` only target the
//! root process. Xvfb is a sibling of Chrome, not a child of it. This module
//! provides process-group kill, tree walk, and cmdline-marker sweep so a CLI
//! invocation leaves **zero** automation browsers, private displays, or
//! profile dirs behind (rules-rust-cli-one-shot + rules-rust-processos-externos).
//!
//! No remote telemetry. Logs use `tracing` to stderr only.

#![cfg(feature = "chrome")]

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
#[cfg(unix)]
use std::time::Duration;

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

/// Global registry of live sessions for best-effort panic/atexit reaping (O1).
static SESSION_REGISTRY: Mutex<Vec<SessionIds>> = Mutex::new(Vec::new());

/// Registers a session so a panic path can still attempt reaping.
pub fn register_session(session: SessionIds) {
    if let Ok(mut guard) = SESSION_REGISTRY.lock() {
        guard.push(session);
    }
}

/// Removes sessions whose `user_data_dir` matches (called after clean finalize).
pub fn unregister_session(user_data_dir: &Path) {
    if let Ok(mut guard) = SESSION_REGISTRY.lock() {
        guard.retain(|s| s.user_data_dir != user_data_dir);
    }
}

/// Best-effort reap of every registered session (panic hook / atexit).
pub fn reap_all_registered() {
    let sessions = match SESSION_REGISTRY.lock() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(_) => return,
    };
    for session in sessions {
        force_reap(&session);
    }
}

/// Installs a panic hook that reaps registered browser sessions, chaining the previous hook.
pub fn install_panic_reap_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            reap_all_registered();
            previous(info);
        }));
    });
}

/// Synchronous forced cleanup of a session (Chrome tree + Xvfb + locks).
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
    tracing::info!(
        chrome_pid = ?session.chrome_pid,
        xvfb_pid = ?session.xvfb_pid,
        user_data = %session.user_data_dir.display(),
        "force_reap completed for browser session"
    );
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
}
