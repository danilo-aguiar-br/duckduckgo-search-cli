// SPDX-License-Identifier: MIT OR Apache-2.0
// GAP-COMP-005: lives under process_lifecycle/ module directory for SRP.
// Workload: system (external process tree ownership / one-shot reap).
// Parallelism (GAP-PAR-028/031/036/037/041):
// - kill_process_tree: sibling kills via thread::scope when n_children ≥ 4
// - reap_all_registered: multi-session phased reap; single-pass multi-marker /proc
// - kill_by_cmdline: parallel /proc cmdline collect (n≥32) then parallel kill (n≥4)
// - sweep_orphan_profiles: collect owned paths then parallel remove when n ≥ 4
// Sync only (Drop / panic hook — no Tokio runtime).
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
//! - panic hook → [`ensure_oneshot_cleanup`]
//! - [`ExitReapGuard`] on `main` Drop → [`ensure_oneshot_cleanup`]
//! - explicit cleanup after global timeout / cancel paths in `lib::run`
//! - start of each run → residual Chrome kill + [`sweep_orphan_profiles`]
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
//! No product telemetry. Logs use `tracing` to stderr only.
//!
//! # Unsafe inventory (Pass 44 soundness boundary)
//!
//! | Site | Module | Ops |
//! |------|--------|-----|
//! | Process kill / PG kill | `unix` | `libc::kill` |
//! | Child `pre_exec` | `unix` | `setpgid` / `prctl` / `getppid` / `_exit` |
//! | Win32 terminate / toolhelp | `windows` | `OpenProcess` / `TerminateProcess` / Toolhelp |
//!
//! Safe wrappers validate PIDs before any `unsafe`. No transmute / `from_raw`.

// Feature gate is on `pub mod process_lifecycle` in lib.rs (avoid duplicate #![cfg]).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

// Profile prefix SSOT: [`USER_DATA_DIR_PREFIX`] (`ddg-chrome-*`).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux::*;
#[cfg(windows)]
use windows::*;

// --- Named reap-policy timings (not user config / not XDG; internal DIE policy) ---

/// Brief settle after SIGKILL so the kernel reaps the zombie / releases fds.
pub(crate) const KILL_SETTLE_MS: u64 = 20;

/// Grace between SIGTERM and SIGKILL for a process group.
///
/// Only the Unix signal path consumes it; `cfg(test)` keeps the timing
/// assertion in `tests.rs` compiling on non-Unix targets.
#[cfg(any(unix, test))]
pub(crate) const TERM_TO_KILL_GRACE_MS: u64 = 50;

/// Settle before first `remove_dir_all` of a Chrome user-data-dir (handles).
pub(crate) const PROFILE_REMOVE_SETTLE_MS: u64 = 50;

/// Wait before retrying `remove_dir_all` after a partial failure.
pub(crate) const PROFILE_REMOVE_RETRY_MS: u64 = 80;

/// Temp profile directory prefix owned by this CLI (`ddg-chrome-*`).
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
///
/// GAP-PAR-031 / 041b: multi-session path is **phased**:
/// 1. kill known Chrome trees in parallel;
/// 2. **one** `/proc` multi-marker pass (not N full scans);
/// 3. Xvfb / display / profile dirs in parallel.
pub fn reap_all_registered() {
    let sessions = std::mem::take(&mut *lock_registry());
    if sessions.is_empty() {
        return;
    }
    if sessions.len() == 1 {
        force_reap(&sessions[0]);
        return;
    }

    let workers = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(2)
        .min(sessions.len())
        .max(1);
    let chunk = sessions.len().div_ceil(workers).max(1);

    // Phase 1: known Chrome process trees (parallel).
    std::thread::scope(|scope| {
        for slice in sessions.chunks(chunk) {
            scope.spawn(move || {
                for session in slice {
                    if let Some(pid) = session.chrome_pid {
                        kill_process_tree(pid);
                    }
                }
            });
        }
    });

    // Phase 2: single-pass multi-marker /proc kill (GAP-PAR-041b).
    let markers: Vec<String> = sessions
        .iter()
        .map(|s| s.user_data_dir.to_string_lossy().into_owned())
        .filter(|m| !m.is_empty())
        .collect();
    kill_by_any_cmdline_substring(&markers);

    // Phase 3: Xvfb + display locks + profile dirs (parallel).
    std::thread::scope(|scope| {
        for slice in sessions.chunks(chunk) {
            scope.spawn(move || {
                for session in slice {
                    force_reap_xvfb_and_disk(session);
                }
            });
        }
    });
}

/// Xvfb / display / disk half of [`force_reap`] (process trees + markers already done).
fn force_reap_xvfb_and_disk(session: &SessionIds) {
    if let Some(pgid) = session.xvfb_pgid {
        kill_process_group(pgid);
    } else if let Some(pid) = session.xvfb_pid {
        kill_pid(pid);
    }
    if let Some(ref display) = session.display {
        cleanup_xvfb_display_files(display);
    }
    remove_user_data_dir(&session.user_data_dir);
    tracing::info!(
        chrome_pid = ?session.chrome_pid,
        xvfb_pid = ?session.xvfb_pid,
        user_data = %session.user_data_dir.display(),
        "force_reap_xvfb_and_disk completed for browser session"
    );
}

/// Full one-shot cleanup used on every exit path (GAP-E2E-51-001).
///
/// Order:
/// 1. [`reap_all_registered`] — sessions still in the registry
/// 2. [`kill_residual_owned_chrome`] — Chromium processes whose cmdline still
///    references a `ddg-chrome-*` profile (registry empty after partial Drop)
/// 3. [`sweep_orphan_profiles`] — remove leftover owned profile dirs on disk
///
/// Idempotent and best-effort; never panics. Safe to call from Drop, panic hook,
/// timeout/cancel paths, and end-of-`main`.
pub fn ensure_oneshot_cleanup() {
    reap_all_registered();
    kill_residual_owned_chrome();
    // Settle so SIGKILL'd Chromium releases handles / drops from /proc.
    std::thread::sleep(Duration::from_millis(KILL_SETTLE_MS));
    // Second kill pass: stream BrokenPipe can leave mid-launch helpers.
    kill_residual_owned_chrome();
    std::thread::sleep(Duration::from_millis(KILL_SETTLE_MS));
    sweep_orphan_profiles();
    // Final force: remove any remaining owned dirs (stale marker_in_use races).
    force_remove_remaining_owned_profiles();
}
/// Kill residual Chromium/Chrome processes that still hold a `ddg-chrome-*`
/// user-data-dir marker in their cmdline.
///
/// Only kills browser-like executables (avoids collateral damage to agent shells
/// or scanners whose argv merely *mentions* the prefix string).
pub fn kill_residual_owned_chrome() {
    #[cfg(target_os = "linux")]
    {
        let pids = linux_collect_numeric_pids();
        let candidates = linux_collect_pids_matching_marker(&pids, USER_DATA_DIR_PREFIX);
        let mut browser_pids = Vec::with_capacity(candidates.len());
        for pid in candidates {
            if linux_pid_looks_like_chromium(pid) {
                tracing::info!(
                    pid,
                    marker = USER_DATA_DIR_PREFIX,
                    "killing residual owned Chrome matching ddg-chrome-* marker"
                );
                browser_pids.push(pid);
            }
        }
        kill_pid_list_parallel(&browser_pids);
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        // macOS/BSD have no `/proc`, but they do not need one: every owned
        // profile records its owner PID in `SingletonLock`, and that file
        // outlives the launcher that created it.
        for pid in owned_profile_owner_pids(&std::env::temp_dir(), USER_DATA_DIR_PREFIX) {
            tracing::info!(
                pid,
                marker = USER_DATA_DIR_PREFIX,
                "killing residual owned Chrome recorded in SingletonLock"
            );
            kill_process_tree(pid);
        }
    }
    #[cfg(windows)]
    {
        windows_kill_by_cmdline_substring(USER_DATA_DIR_PREFIX);
    }
}

/// Live owner PIDs recorded inside this CLI's own profile directories.
///
/// # Why a directory scan is the portable answer
///
/// The Linux recovery path reads `/proc/*/cmdline` and matches the profile
/// marker. That file does not exist on macOS, the BSDs or Windows, and the two
/// substitutes are both bad: shelling out to `ps` breaks the pure-Rust contract,
/// and a WMI command-line query walks the whole process table to answer a
/// question about at most a handful of directories.
///
/// Chromium already solved it. Each profile holds a `SingletonLock` naming the
/// process that owns it, so the set of directories this CLI owns IS the process
/// registry, and it survives our own SIGKILL because it lives on disk.
///
/// Only live PIDs are returned, so a stale lock from a machine reboot cannot
/// send a signal to whatever unrelated process later inherited that number.
#[cfg_attr(target_os = "linux", allow(dead_code))]
fn owned_profile_owner_pids(root: &Path, prefix: &str) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut pids = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if is_forbidden_bulk_delete_name(&name_str) || !name_str.starts_with(prefix) {
            continue;
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(pid) = singleton_lock_pid(&path) {
            if pid_is_alive(pid) && pid != std::process::id() {
                pids.push(pid);
            }
        }
    }
    pids
}

/// Force-remove every owned `ddg-chrome-*` directory under `temp_dir()`.
///
/// Called after kill passes so disk one-shot holds even when `marker_in_use`
/// raced true during the first [`sweep_orphan_profiles`]. Never touches `.tmp*`
/// or `org.chromium.Chromium.*`.
///
/// # Concurrency: this DOES reap another invocation's Chrome, deliberately
///
/// Unlike [`sweep_orphan_profiles`], this pass does not consult `marker_in_use`,
/// so a second concurrent run of the CLI ends the first one's browser. That is
/// the accepted trade-off, not an oversight: the one-shot disk contract is what
/// keeps agents able to run N sequential invocations without accumulating
/// orphan profiles, and honouring the marker here would let a raced marker
/// leave one behind forever.
///
/// The boundary that matters is the PREFIX, and it is enforced above: only
/// `ddg-chrome-*` is ever removed. Reviewed and kept 2026-08-21 — an audit that
/// reads this as a bug should change the trade-off explicitly, not silently.
fn force_remove_remaining_owned_profiles() {
    let temp = std::env::temp_dir();
    let Ok(entries) = std::fs::read_dir(&temp) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if is_forbidden_bulk_delete_name(&name_str) {
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
        kill_by_cmdline_substring(&marker);
        // Extra settle: Chromium multi-process can hold files after SIGKILL.
        std::thread::sleep(Duration::from_millis(PROFILE_REMOVE_SETTLE_MS));
        remove_user_data_dir(&path);
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
            if path.exists() {
                std::thread::sleep(Duration::from_millis(PROFILE_REMOVE_RETRY_MS));
                let _ = std::fs::remove_dir_all(&path);
            }
        }
    }
}

/// RAII guard: on create, sweeps orphan `ddg-chrome-*` from prior SIGKILL/OOM;
/// on Drop, runs [`ensure_oneshot_cleanup`] (registry + residual Chrome + disk).
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
        // Kill residual Chrome first so sweep can remove profile dirs (marker_in_use).
        kill_residual_owned_chrome();
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
        ensure_oneshot_cleanup();
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
            ensure_oneshot_cleanup();
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
    std::thread::sleep(Duration::from_millis(PROFILE_REMOVE_SETTLE_MS));
    if let Err(e) = std::fs::remove_dir_all(path) {
        tracing::info!(
            path = %path.display(),
            error = %e,
            "first remove_dir_all of user-data-dir failed — retrying"
        );
        std::thread::sleep(Duration::from_millis(PROFILE_REMOVE_RETRY_MS));
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
    sweep_orphan_profiles_in(&std::env::temp_dir());
}

/// [`sweep_orphan_profiles`] against an explicit root.
///
/// # Why the root is a parameter
///
/// The sweep walks a directory shared with every other process on the host, so
/// a test that plants a fixture there is racing tools it does not control. One
/// `cargo test-all` run failed on `org.chromium.* must survive sweep` while a
/// sibling project was compiling in the same `/tmp`: the fixture was removed by
/// something that was not this sweep, and the assertion was right to fire.
///
/// `sweep_lock()` cannot help — it serialises threads inside one process, and
/// `test-all` runs twenty-five binaries at once. Naming fixtures by PID does
/// not help either, since a foreign sweeper does not read our PIDs. Giving the
/// tests their own root removes the shared surface instead of narrowing the
/// window, so the intermittent cannot recur by construction.
pub fn sweep_orphan_profiles_in(root: &Path) {
    let temp = root.to_path_buf();
    let Ok(entries) = std::fs::read_dir(&temp) else {
        return;
    };
    let mut skipped_foreign = 0u32;
    let mut to_remove: Vec<PathBuf> = Vec::new();
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
        to_remove.push(path);
    }

    // GAP-PAR-037: parallel remove when many orphans (cold-start path).
    const PARALLEL_SWEEP_THRESHOLD: usize = 4;
    let removed = if to_remove.len() >= PARALLEL_SWEEP_THRESHOLD {
        let workers = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(2)
            .min(to_remove.len())
            .max(1);
        let chunk = to_remove.len().div_ceil(workers).max(1);
        let removed = std::sync::atomic::AtomicU32::new(0);
        std::thread::scope(|scope| {
            for slice in to_remove.chunks(chunk) {
                let removed = &removed;
                scope.spawn(move || {
                    let mut local = 0u32;
                    for path in slice {
                        remove_user_data_dir(path);
                        if !path.exists() {
                            local = local.saturating_add(1);
                        }
                    }
                    removed.fetch_add(local, std::sync::atomic::Ordering::Relaxed);
                });
            }
        });
        removed.load(std::sync::atomic::Ordering::Relaxed)
    } else {
        let mut removed = 0u32;
        for path in &to_remove {
            remove_user_data_dir(path);
            if !path.exists() {
                removed = removed.saturating_add(1);
            }
        }
        removed
    };

    if removed > 0 || skipped_foreign > 0 {
        tracing::info!(
            removed,
            skipped_foreign_candidates = skipped_foreign,
            "orphan profile sweep finished (ddg-chrome-* only)"
        );
    }
}

mod kill;
use kill::{
    kill_by_any_cmdline_substring, marker_in_use, pid_is_alive, singleton_lock_alive,
    singleton_lock_pid,
};
pub use kill::{kill_by_cmdline_substring, kill_pid, kill_process_group, kill_process_tree};
// `kill_pid_list_parallel` is only reachable from the Linux `/proc` scan path.
#[cfg(target_os = "linux")]
use kill::kill_pid_list_parallel;
// Consumed only by the Linux `/proc` scanners in `linux.rs`; gate the re-export
// so non-Linux targets do not trip `-D warnings` on an unused import.
#[cfg(target_os = "linux")]
pub(crate) use kill::PROC_SCAN_PARALLEL_THRESHOLD;

/// X11 display lock path for display number `N` under [`std::env::temp_dir`].
///
/// GAP-HARD-X11-001: never hardcode `/tmp/.X{N}-lock` — honor `TMPDIR` via
/// `std::env::temp_dir()`.
#[must_use]
pub fn x11_lock_path(display_num: &str) -> PathBuf {
    std::env::temp_dir().join(format!(".X{display_num}-lock"))
}

/// X11 Unix-domain socket path for display number `N` under
/// [`std::env::temp_dir`]`/.X11-unix/`.
///
/// GAP-HARD-X11-001: never hardcode `/tmp/.X11-unix/X{N}`.
#[must_use]
pub fn x11_socket_path(display_num: &str) -> PathBuf {
    std::env::temp_dir()
        .join(".X11-unix")
        .join(format!("X{display_num}"))
}

/// Removes Xvfb lock and Unix socket for a display like `":99"`.
pub fn cleanup_xvfb_display_files(xvfb_display: &str) {
    let num = xvfb_display.trim().trim_start_matches(':');
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return;
    }
    let lock = x11_lock_path(num);
    let socket = x11_socket_path(num);
    let _ = std::fs::remove_file(&lock);
    let _ = std::fs::remove_file(&socket);
    tracing::info!(display = %xvfb_display, "cleaned Xvfb lock/socket files");
}

/// Configures a `std::process::Command` so the child becomes a process-group
/// leader and receives SIGKILL if this CLI dies (Linux PDEATHSIG).
#[cfg(unix)]
pub use unix::apply_process_group_and_pdeathsig;

/// No-op outside Unix: there is no process group nor PDEATHSIG to configure,
/// so the child inherits the platform default spawn behaviour.
#[cfg(not(unix))]
pub fn apply_process_group_and_pdeathsig(_cmd: &mut std::process::Command) {}

/// Substring search over raw `/proc` bytes.
///
/// Only the Linux `/proc` scanners consume it; `cfg(test)` keeps the unit test
/// compiling on non-Linux targets.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn bytes_contains_str(haystack: &[u8], needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let n = needle.as_bytes();
    haystack.windows(n.len()).any(|w| w == n)
}

#[cfg(test)]
mod tests;
