// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: process lifecycle (Unix kill / process-group / PDEATHSIG)
//! Unix soundness boundary for process termination FFI (Pass 44 / GAP-UNSAFE-012).
//!
//! All `libc::kill` / `pre_exec` / `prctl` for one-shot Chrome/Xvfb reaping live
//! here so the module is a single audit unit for undefined-behavior risk.
//!
//! # Invariants (safe wrappers enforce before any `unsafe`)
//!
//! - Never signal PID 0 (POSIX: entire caller's process group).
//! - Never signal PID 1 (init / system).
//! - Never signal this process (`std::process::id()`).
//! - Positive PIDs convert via `i32::try_from` (reject wrap → negative PG kill).
//! - Process-group kills require `pgid > 1` and use the POSIX negative-PID form.
// OS gate: `#[cfg(unix)] mod unix` in parent.

use std::time::Duration;

use super::{KILL_SETTLE_MS, TERM_TO_KILL_GRACE_MS};

/// Returns `true` when `pid` is a safe target for best-effort SIGKILL.
///
/// Rejects 0 (process-group broadcast), 1 (init), and the current process.
#[must_use]
pub(crate) fn is_safe_kill_target(pid: u32) -> bool {
    pid >= 2 && pid != std::process::id()
}

/// Convert a validated positive PID to `i32` for `libc::kill`.
///
/// Returns `None` when the value does not fit in `i32` (would wrap to a
/// negative argument and accidentally target a process group).
#[must_use]
fn pid_as_libc(pid: u32) -> Option<i32> {
    if !is_safe_kill_target(pid) {
        return None;
    }
    i32::try_from(pid).ok().filter(|&p| p > 1)
}

/// Sends SIGKILL to a single PID (best-effort; ESRCH/EPERM ignored).
pub(crate) fn unix_kill_pid(pid: u32) {
    let Some(pid_i) = pid_as_libc(pid) else {
        return;
    };
    // SAFETY:
    // - `pid_i` is > 1 and ≠ our PID (checked in `pid_as_libc` / `is_safe_kill_target`).
    // - Positive `pid_i` targets a single process (POSIX), never a process group.
    // - Best-effort reap: return value and errno (ESRCH if already gone, EPERM)
    //   are intentionally ignored — callers must not assume the process exited.
    // - No pointer ownership; `libc::kill` does not transfer memory.
    let _ = unsafe { libc::kill(pid_i, libc::SIGKILL) };
    std::thread::sleep(Duration::from_millis(KILL_SETTLE_MS));
}

/// Unix: SIGTERM then SIGKILL to an entire process group. `pgid` must be > 1.
pub(crate) fn unix_kill_process_group(pgid: i32) {
    if pgid <= 1 {
        return;
    }
    // SAFETY:
    // - Negative PID is the POSIX form for "signal this process group".
    // - `pgid > 1` (checked above) so we never target PG 0/1 or the broadcast forms.
    // - Best-effort reap: errno ignored (group may already be gone).
    let _ = unsafe { libc::kill(-pgid, libc::SIGTERM) };
    std::thread::sleep(Duration::from_millis(TERM_TO_KILL_GRACE_MS));
    // SAFETY: same process-group kill with SIGKILL after grace (see above).
    let _ = unsafe { libc::kill(-pgid, libc::SIGKILL) };
    std::thread::sleep(Duration::from_millis(KILL_SETTLE_MS));
}

/// Configures a `std::process::Command` so the child becomes a process-group
/// leader and receives SIGKILL if this CLI dies (Linux PDEATHSIG).
pub fn apply_process_group_and_pdeathsig(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // `pre_exec` must stay one atomic closure between fork and exec; multiple
    // async-signal-safe syscalls share one soundness proof (see SAFETY).
    #[allow(clippy::multiple_unsafe_ops_per_block)]
    // SAFETY:
    // - `pre_exec` runs only in the child, between `fork` and `exec`.
    // - Body uses only async-signal-safe calls: `setpgid`, `prctl`, `getppid`,
    //   `_exit` (no heap allocation, no locks, no Rust Drop).
    // - Child becomes its own process-group leader (`setpgid(0,0)`).
    // - On Linux, `PR_SET_PDEATHSIG` + SIGKILL reaps the child if the parent
    //   dies; if the parent already died (`getppid() == 1`), the child exits.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                // Non-fatal: continue without group leadership.
            }
            #[cfg(target_os = "linux")]
            {
                let _ = libc::prctl(
                    libc::PR_SET_PDEATHSIG,
                    libc::SIGKILL as libc::c_ulong,
                    0,
                    0,
                    0,
                );
                if libc::getppid() == 1 {
                    libc::_exit(128 + libc::SIGKILL);
                }
            }
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_pid_zero_one_and_self() {
        assert!(!is_safe_kill_target(0));
        assert!(!is_safe_kill_target(1));
        assert!(!is_safe_kill_target(std::process::id()));
        assert!(is_safe_kill_target(2) || std::process::id() == 2);
    }

    #[test]
    fn kill_pid_zero_one_self_are_nops() {
        // Must not signal our process group or init.
        unix_kill_pid(0);
        unix_kill_pid(1);
        unix_kill_pid(std::process::id());
    }

    #[test]
    fn kill_process_group_refuses_pgid_le_one() {
        unix_kill_process_group(0);
        unix_kill_process_group(1);
        unix_kill_process_group(-1);
    }

    #[test]
    fn pid_as_libc_rejects_unsafe_targets() {
        assert!(pid_as_libc(0).is_none());
        assert!(pid_as_libc(1).is_none());
        assert!(pid_as_libc(std::process::id()).is_none());
    }
}
