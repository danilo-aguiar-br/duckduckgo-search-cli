// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: OS process kill helpers (one-shot Chrome lifecycle)
//! PID / process-group / cmdline kill helpers (SRP split from `process_lifecycle`).

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use super::linux::*;
#[cfg(windows)]
use super::windows::*;
#[cfg(unix)]
use super::unix;

/// Returns true if any process (except self) has `marker` in its cmdline.
///
/// GAP-PAR-041a: when `/proc` is large, cmdline reads run in a bounded
/// `thread::scope` with early-exit via `AtomicBool`.
pub(super) fn marker_in_use(marker: &str) -> bool {
    if marker.is_empty() {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        let pids = linux_collect_numeric_pids();
        linux_any_cmdline_contains(&pids, marker)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = marker;
        false
    }
}

/// Threshold above which `/proc` cmdline/stat reads fan out via `thread::scope`.
pub(crate) const PROC_SCAN_PARALLEL_THRESHOLD: usize = 32;

// Proc helpers (collect PIDs, cmdline markers) live in `linux` / `unix` modules.

/// Kill every process whose cmdline matches **any** marker (one `/proc` pass).
pub(super) fn kill_by_any_cmdline_substring(markers: &[String]) {
    if markers.is_empty() {
        return;
    }
    if markers.len() == 1 {
        kill_by_cmdline_substring(&markers[0]);
        return;
    }
    #[cfg(target_os = "linux")]
    {
        let pids = linux_collect_numeric_pids();
        let matches = linux_collect_pids_matching_any_marker(&pids, markers);
        kill_pid_list_parallel(&matches);
    }
    #[cfg(not(target_os = "linux"))]
    {
        for m in markers {
            kill_by_cmdline_substring(m);
        }
    }
}

/// Kill a list of PIDs; parallel when `len ≥ 4` (shared by 036/041).
pub(super) fn kill_pid_list_parallel(pids: &[u32]) {
    const PARALLEL_KILL_THRESHOLD: usize = 4;
    if pids.is_empty() {
        return;
    }
    if pids.len() >= PARALLEL_KILL_THRESHOLD {
        let workers = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(2)
            .min(pids.len())
            .max(1);
        let chunk = pids.len().div_ceil(workers).max(1);
        std::thread::scope(|scope| {
            for slice in pids.chunks(chunk) {
                scope.spawn(move || {
                    for pid in slice {
                        kill_pid(*pid);
                    }
                });
            }
        });
    } else {
        for &pid in pids {
            kill_pid(pid);
        }
    }
}

/// If `SingletonLock` exists and names a live PID, the profile is still owned.
pub(super) fn singleton_lock_alive(profile: &Path) -> bool {
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
///
/// Refuses PID 0 (POSIX process-group broadcast), PID 1 (init), and this
/// process — see [`unix::is_safe_kill_target`].
pub fn kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        unix::unix_kill_pid(pid);
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
    #[cfg(unix)]
    {
        unix::unix_kill_process_group(pgid);
    }
    #[cfg(not(unix))]
    {
        let _ = pgid;
    }
}

/// Walks descendants of `root_pid` and SIGKILLs them bottom-up, then the root.
///
/// GAP-PAR-028: when the tree is large (many Chromium helpers), sibling kills
/// run in a bounded `thread::scope` so shutdown does not stay monothread.
/// Still not Tokio (may run from Drop / panic hook without a runtime).
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
        // Kill children first (skip root until end). Bottom-up order among
        // parents is preserved by processing the reversed BFS list in chunks
        // of independent leaves-first wave; for simplicity we parallelize
        // only when many PIDs (Chrome multi-process trees).
        let children: Vec<u32> = all
            .iter()
            .copied()
            .filter(|pid| *pid != root_pid)
            .rev()
            .collect();
        const PARALLEL_REAP_THRESHOLD: usize = 4;
        if children.len() >= PARALLEL_REAP_THRESHOLD {
            let workers = std::thread::available_parallelism()
                .map(std::num::NonZeroUsize::get)
                .unwrap_or(2)
                .min(children.len())
                .max(1);
            let chunk = children.len().div_ceil(workers).max(1);
            std::thread::scope(|scope| {
                for slice in children.chunks(chunk) {
                    scope.spawn(move || {
                        for pid in slice {
                            kill_pid(*pid);
                        }
                    });
                }
            });
        } else {
            for pid in children {
                kill_pid(pid);
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
///
/// GAP-PAR-036 / 041a: collect matching PIDs (parallel cmdline reads when
/// `/proc` is large), then kill in parallel when the match set is large.
pub fn kill_by_cmdline_substring(marker: &str) {
    if marker.is_empty() {
        return;
    }
    #[cfg(target_os = "linux")]
    {
        let pids = linux_collect_numeric_pids();
        let matches = linux_collect_pids_matching_marker(&pids, marker);
        for &pid in &matches {
            tracing::info!(pid, %marker, "killing process matching user-data-dir marker");
        }
        kill_pid_list_parallel(&matches);
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
