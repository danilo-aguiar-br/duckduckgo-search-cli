// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (best-effort OS process enumeration; no kill).
//! Count Chrome/Chromium-like processes for doctor + deep-research budget.
//!
//! Shared SSOT for CLI-BUDGET-01 / CLI-DOC-* (v1.0.2). Never kills processes.
//! Returns `0` when the platform cannot enumerate (or probe fails).

/// Best-effort count of chrome/chromium-like processes (host + Flatpak).
///
/// Used for doctor diagnostics and contention-aware budget. Never kills
/// processes. Returns 0 when `/proc` is unavailable (non-Linux) or unreadable.
#[must_use]
pub fn count_chrome_like_processes() -> usize {
    #[cfg(target_os = "linux")]
    {
        count_linux()
    }
    #[cfg(target_os = "macos")]
    {
        count_via_pgrep()
    }
    #[cfg(target_os = "windows")]
    {
        count_windows()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        0
    }
}

#[cfg(target_os = "linux")]
fn count_linux() -> usize {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return 0;
    };
    let mut n = 0usize;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let cmdline_path = entry.path().join("cmdline");
        let Ok(raw) = std::fs::read(cmdline_path) else {
            continue;
        };
        // cmdline is NUL-separated; join with spaces for substring match.
        let cmd = String::from_utf8_lossy(&raw).replace('\0', " ");
        let lower = cmd.to_ascii_lowercase();
        if lower.contains("chrome")
            || lower.contains("chromium")
            || lower.contains("/app/extra/chrome")
        {
            n = n.saturating_add(1);
        }
    }
    n
}

#[cfg(target_os = "macos")]
fn count_via_pgrep() -> usize {
    // Best-effort: pgrep -fl chrome|chromium. Failure → 0 (factor 1.0).
    let Ok(output) = std::process::Command::new("pgrep")
        .args(["-fl", "Chrome|Chromium|Google Chrome|chromium"])
        .output()
    else {
        return 0;
    };
    if !output.status.success() && output.stdout.is_empty() {
        return 0;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

#[cfg(target_os = "windows")]
fn count_windows() -> usize {
    // Best-effort tasklist; failure → 0 (contention factor 1.0).
    let Ok(output) = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
    else {
        return 0;
    };
    if !output.status.success() {
        return 0;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| {
            let lower = l.to_ascii_lowercase();
            lower.contains("chrome.exe") || lower.contains("chromium")
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_does_not_panic() {
        let _ = count_chrome_like_processes();
    }
}
