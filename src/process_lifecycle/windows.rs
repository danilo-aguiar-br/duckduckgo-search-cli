// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: process lifecycle (Windows terminate helpers)
//! Windows-specific process termination helpers (GAP-COMP-005r / Pass 44).
//!
//! Soundness boundary for Win32 process FFI. Safe wrappers validate PIDs and
//! HANDLEs before any `unsafe` op; each critical call is its own block.
//!
//! The module is already gated at its declaration (`#[cfg(windows)] mod windows;`
//! in `mod.rs`), so no inner `#![cfg(windows)]` is needed here.

use std::collections::{HashSet, VecDeque};

/// Exit code passed to `TerminateProcess` for reaped Chrome/helper processes.
const TERMINATE_EXIT_CODE: u32 = 1;

/// Returns `true` when `pid` is a safe target for best-effort terminate.
#[must_use]
fn is_safe_kill_target(pid: u32) -> bool {
    pid >= 2 && pid != std::process::id()
}

pub(crate) fn windows_terminate_pid(pid: u32) {
    if !is_safe_kill_target(pid) {
        return;
    }
    use windows_sys::Win32::Foundation::{CloseHandle, FALSE};
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    // SAFETY:
    // - `pid` was validated (`>= 2`, not self).
    // - `OpenProcess` takes a process id by value; no pointer arithmetic.
    // - On failure Win32 returns a null HANDLE (windows-sys 0.61: `*mut c_void`).
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, FALSE, pid) };
    if handle.is_null() {
        return;
    }
    // SAFETY:
    // - `handle` is non-null from a successful `OpenProcess` with PROCESS_TERMINATE.
    // - `TERMINATE_EXIT_CODE` is a plain u32 exit status for the target process.
    let _ = unsafe { TerminateProcess(handle, TERMINATE_EXIT_CODE) };
    // SAFETY:
    // - `handle` is an open process HANDLE we own; `CloseHandle` releases it once.
    // - Must run even if TerminateProcess fails (no leak).
    let _ = unsafe { CloseHandle(handle) };
}

pub(crate) fn windows_terminate_tree(root_pid: u32) {
    // Snapshot + kill descendants, then root.
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut children_of: std::collections::HashMap<u32, Vec<u32>> =
        std::collections::HashMap::new();

    // SAFETY:
    // - `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)` takes no user pointers.
    // - On failure the API returns `INVALID_HANDLE_VALUE` (checked immediately).
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        windows_terminate_pid(root_pid);
        return;
    }

    // SAFETY:
    // - `PROCESSENTRY32W` is a Win32 POD (`#[repr(C)]` in windows-sys): all fields
    //   are integers/fixed arrays; zeroed bit-pattern is a valid initial state.
    // - `dwSize` must be set before `Process32FirstW` (Win32 contract).
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    // SAFETY:
    // - `snap` is a valid toolhelp snapshot HANDLE (not INVALID_HANDLE_VALUE).
    // - `entry` is a stack POD with `dwSize` set; First/Next write only into it.
    let first_ok = unsafe { Process32FirstW(snap, &mut entry) } != 0;
    if first_ok {
        loop {
            children_of
                .entry(entry.th32ParentProcessID)
                .or_default()
                .push(entry.th32ProcessID);
            // SAFETY: same snapshot + entry invariants as FirstW.
            if unsafe { Process32NextW(snap, &mut entry) } == 0 {
                break;
            }
        }
    }

    // SAFETY: we own `snap` from CreateToolhelp32Snapshot; close exactly once.
    let _ = unsafe { CloseHandle(snap) };

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

/// Whether `pid` names a process that currently exists.
///
/// # Why `OpenProcess` and not a toolhelp snapshot
///
/// The caller is the `SingletonLock` recovery path, which asks about one PID at
/// a time. A snapshot walks the whole process table to answer that, while
/// `OpenProcess` with `PROCESS_QUERY_LIMITED_INFORMATION` asks the kernel
/// directly and needs no elevation — the limited right exists precisely for
/// "does this exist" questions.
///
/// A null HANDLE means the process is gone OR that access was denied. Access
/// denied still means it EXISTS, so both are treated as unknown-but-present
/// only when the error says so; anything else counts as dead.
#[must_use]
pub(crate) fn windows_pid_is_alive(pid: u32) -> bool {
    if pid < 2 {
        return false;
    }
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, FALSE};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    // SAFETY:
    // - `pid` is passed by value; no user pointers are involved.
    // - The requested right is read-only metadata access, never terminate.
    // - On failure Win32 returns a null HANDLE (windows-sys: `*mut c_void`).
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid) };
    if handle.is_null() {
        return std::io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED as i32);
    }
    // SAFETY: `handle` is non-null from a successful `OpenProcess`; close once.
    let _ = unsafe { CloseHandle(handle) };
    true
}

/// Terminate owned Chrome trees recorded by profile marker, on Windows.
///
/// # Why this is no longer empty
///
/// The previous body was a comment saying a WMI command-line scan is heavy and
/// that terminating the tree from `chrome_pid` is the primary path. Both halves
/// are true and neither covers the case this function exists for: after a
/// SIGKILL or an OOM there IS no `chrome_pid` in memory, which is the only
/// moment the fallback is ever reached. A fallback that is empty exactly when
/// it is needed is not a fallback.
///
/// The recovery does not need command lines at all. Every owned profile
/// directory is named with [`super::USER_DATA_DIR_PREFIX`], and Chromium writes
/// its own PID into `SingletonLock` inside that directory. The directory is
/// therefore already a process registry that survives our death, so the scan is
/// a cheap directory read instead of a WMI query over the whole process table.
pub(crate) fn windows_kill_by_cmdline_substring(marker: &str) {
    for pid in super::owned_profile_owner_pids(&std::env::temp_dir(), marker) {
        tracing::info!(
            pid,
            marker,
            "terminating residual owned Chrome recorded in SingletonLock"
        );
        windows_terminate_tree(pid);
    }
}
