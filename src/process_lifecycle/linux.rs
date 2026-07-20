// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: process lifecycle (Linux /proc helpers)
//! Linux-specific process tree and cmdline helpers (GAP-COMP-005r).
// OS gate: `#[cfg(target_os = "linux")] mod linux` in parent.

use super::{bytes_contains_str, PROC_SCAN_PARALLEL_THRESHOLD};
use std::sync::Mutex;

pub(crate) fn linux_collect_numeric_pids() -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let self_pid = std::process::id();
    let mut pids = Vec::with_capacity(256);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        if pid != self_pid {
            pids.push(pid);
        }
    }
    pids
}
pub(crate) fn linux_cmdline_contains(pid: u32, marker: &str) -> bool {
    let cmdline_path = format!("/proc/{pid}/cmdline");
    let Ok(bytes) = std::fs::read(&cmdline_path) else {
        return false;
    };
    bytes_contains_str(&bytes, marker)
}

/// True when `/proc/{pid}/cmdline` looks like a Chromium/Chrome browser process.
///
/// Used by residual `ddg-chrome-*` reaping so agent shells / scanners that only
/// *mention* the profile prefix in argv are not SIGKILL'd.
pub(crate) fn linux_pid_looks_like_chromium(pid: u32) -> bool {
    let cmdline_path = format!("/proc/{pid}/cmdline");
    let Ok(bytes) = std::fs::read(&cmdline_path) else {
        return false;
    };
    // First argv component is typically the executable path.
    let first = bytes.split(|b| *b == 0).next().unwrap_or(&[]);
    let path = String::from_utf8_lossy(first);
    let lower = path.to_ascii_lowercase();
    lower.contains("chrom") // chrome, chromium, chrome-headless, google-chrome
        || lower.ends_with("chrome.exe")
}
pub(crate) fn linux_any_cmdline_contains(pids: &[u32], marker: &str) -> bool {
    if pids.is_empty() || marker.is_empty() {
        return false;
    }
    if pids.len() < PROC_SCAN_PARALLEL_THRESHOLD {
        return pids.iter().any(|&pid| linux_cmdline_contains(pid, marker));
    }
    use std::sync::atomic::{AtomicBool, Ordering};
    let found = AtomicBool::new(false);
    let found = &found;
    let workers = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(2)
        .min(pids.len())
        .max(1);
    let chunk = pids.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        for slice in pids.chunks(chunk) {
            scope.spawn(move || {
                for &pid in slice {
                    if found.load(Ordering::Relaxed) {
                        return;
                    }
                    if linux_cmdline_contains(pid, marker) {
                        found.store(true, Ordering::Relaxed);
                        return;
                    }
                }
            });
        }
    });
    found.load(Ordering::Relaxed)
}
pub(crate) fn linux_collect_pids_matching_marker(pids: &[u32], marker: &str) -> Vec<u32> {
    if pids.is_empty() || marker.is_empty() {
        return Vec::new();
    }
    if pids.len() < PROC_SCAN_PARALLEL_THRESHOLD {
        return pids
            .iter()
            .copied()
            .filter(|&pid| linux_cmdline_contains(pid, marker))
            .collect();
    }
    let workers = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(2)
        .min(pids.len())
        .max(1);
    let chunk = pids.len().div_ceil(workers).max(1);
    let matches = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        let matches = &matches;
        for slice in pids.chunks(chunk) {
            scope.spawn(move || {
                let local: Vec<u32> = slice
                    .iter()
                    .copied()
                    .filter(|&pid| linux_cmdline_contains(pid, marker))
                    .collect();
                if !local.is_empty() {
                    matches
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .extend(local);
                }
            });
        }
    });
    matches.into_inner().unwrap_or_else(|p| p.into_inner())
}
pub(crate) fn linux_collect_pids_matching_any_marker(pids: &[u32], markers: &[String]) -> Vec<u32> {
    if pids.is_empty() || markers.is_empty() {
        return Vec::new();
    }
    if markers.len() == 1 {
        return linux_collect_pids_matching_marker(pids, &markers[0]);
    }
    let match_fn = |pid: u32| -> bool {
        let cmdline_path = format!("/proc/{pid}/cmdline");
        let Ok(bytes) = std::fs::read(&cmdline_path) else {
            return false;
        };
        markers.iter().any(|m| bytes_contains_str(&bytes, m))
    };
    if pids.len() < PROC_SCAN_PARALLEL_THRESHOLD {
        return pids.iter().copied().filter(|&pid| match_fn(pid)).collect();
    }
    let workers = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(2)
        .min(pids.len())
        .max(1);
    let chunk = pids.len().div_ceil(workers).max(1);
    let matches = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        let matches = &matches;
        for slice in pids.chunks(chunk) {
            scope.spawn(move || {
                let local: Vec<u32> = slice.iter().copied().filter(|&pid| match_fn(pid)).collect();
                if !local.is_empty() {
                    matches
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .extend(local);
                }
            });
        }
    });
    matches.into_inner().unwrap_or_else(|p| p.into_inner())
}
pub(crate) fn linux_children_of(pid: u32) -> Vec<u32> {
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
pub(crate) fn linux_ppid_from_stat(pid: u32) -> Option<u32> {
    let stat_path = format!("/proc/{pid}/stat");
    let stat = std::fs::read_to_string(stat_path).ok()?;
    // Format: pid (comm) state ppid ...
    let after_comm = stat.rsplit(')').next()?;
    let mut parts = after_comm.split_whitespace();
    let _state = parts.next()?;
    parts.next()?.parse::<u32>().ok()
}
pub(crate) fn linux_children_via_stat_scan(parent: u32) -> Vec<u32> {
    // GAP-PAR-041a: parallel stat reads when /proc is large.
    let pids = linux_collect_numeric_pids();
    if pids.len() < PROC_SCAN_PARALLEL_THRESHOLD {
        return pids
            .into_iter()
            .filter(|&pid| linux_ppid_from_stat(pid) == Some(parent))
            .collect();
    }
    let workers = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(2)
        .min(pids.len())
        .max(1);
    let chunk = pids.len().div_ceil(workers).max(1);
    let kids = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        let kids = &kids;
        for slice in pids.chunks(chunk) {
            scope.spawn(move || {
                let local: Vec<u32> = slice
                    .iter()
                    .copied()
                    .filter(|&pid| linux_ppid_from_stat(pid) == Some(parent))
                    .collect();
                if !local.is_empty() {
                    kids.lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .extend(local);
                }
            });
        }
    });
    kids.into_inner().unwrap_or_else(|p| p.into_inner())
}
