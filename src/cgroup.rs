// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential process-control helper (Linux cgroup opt-in).
//! Optional Linux cgroup memory limit for Chrome fan-out (multi-OS safe).
//!
//! # Contract (G6)
//!
//! - **Default OFF** — operators enable via XDG `linux_cgroup_enabled=true` and
//!   `linux_cgroup_memory_max_mb=N` (`config set`), never product env.
//! - **Linux only** — macOS/Windows return [`CgroupStatus::NotApplicable`].
//! - Does not reparent the whole host; when enabled, documents the intended
//!   `systemd-run --scope` / cgroup v2 memory.max policy for operator scripts.
//! - Doctor reports honest status; no silent failure that looks like empty SERP.

use serde::Serialize;

/// Doctor / agent-facing cgroup status (stdout JSON field, not telemetry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CgroupStatus {
    /// Feature not available on this OS.
    NotApplicable,
    /// Available but disabled in XDG (default).
    Disabled,
    /// Enabled in XDG; Linux path ready for operator wrap / future enforce.
    Enabled,
    /// Enabled but host lacks cgroup v2 / systemd-run.
    Unavailable,
}

/// Resolved cgroup policy from XDG (no env inheritance).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CgroupPolicy {
    /// Whether the operator opted in.
    pub enabled: bool,
    /// Memory max in MiB when enabled.
    pub memory_max_mb: Option<u64>,
}

impl CgroupPolicy {
    /// Parse from flat XDG string map values.
    #[must_use]
    pub fn from_xdg(enabled_raw: Option<&str>, mb_raw: Option<&str>) -> Self {
        let enabled = matches!(
            enabled_raw
                .map(|s| s.trim().to_ascii_lowercase())
                .as_deref(),
            Some("1" | "true" | "yes" | "on")
        );
        let memory_max_mb = mb_raw
            .and_then(|s| s.trim().parse().ok())
            .filter(|&n| n >= 1);
        Self {
            enabled,
            memory_max_mb,
        }
    }

    /// Status for doctor / config show.
    #[must_use]
    pub fn status(&self) -> CgroupStatus {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = self;
            CgroupStatus::NotApplicable
        }
        #[cfg(target_os = "linux")]
        {
            if !self.enabled {
                return CgroupStatus::Disabled;
            }
            // Presence of systemd-run is a soft readiness signal (not required to search).
            let has_systemd_run = std::process::Command::new("systemd-run")
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if has_systemd_run || self.memory_max_mb.is_some() {
                CgroupStatus::Enabled
            } else {
                CgroupStatus::Unavailable
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_xdg_disabled() {
        let p = CgroupPolicy::from_xdg(None, None);
        assert!(!p.enabled);
        #[cfg(target_os = "linux")]
        assert_eq!(p.status(), CgroupStatus::Disabled);
        #[cfg(not(target_os = "linux"))]
        assert_eq!(p.status(), CgroupStatus::NotApplicable);
    }

    #[test]
    fn enabled_with_mb() {
        let p = CgroupPolicy::from_xdg(Some("true"), Some("2048"));
        assert!(p.enabled);
        assert_eq!(p.memory_max_mb, Some(2048));
    }
}
