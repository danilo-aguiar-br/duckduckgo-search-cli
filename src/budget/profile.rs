// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure CPU — budget profile presets (no I/O).
//! Named budget profiles for deep-research (CLI-XDG-01 / G4).
//!
//! Profiles seed contention and unit-time knobs. Precedence after apply:
//! **CLI explicit > individual XDG knobs > profile > factory constants**.

use crate::budget::input::DeepResearchBudgetInput;

/// Canonical profile names accepted by XDG `budget_profile` / agents.
/// Lab / ideal unit times (factory contention bands).
pub const PROFILE_LAB: &str = "lab";
/// Desktop host with many Chromes — earlier contention scaling.
pub const PROFILE_DESKTOP_CONTENDED: &str = "desktop_contended";
/// Thin SERP-oriented estimates (lower unit times).
pub const PROFILE_THIN: &str = "thin";

/// Whether `name` is a known budget profile.
#[must_use]
pub fn is_known_budget_profile(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "lab" | "desktop_contended" | "thin"
    )
}

/// Apply a named profile onto a budget input (in place).
///
/// Unknown names are no-ops (caller may fail-closed at `config set`).
pub fn apply_budget_profile(input: &mut DeepResearchBudgetInput, profile: &str) {
    match profile.trim().to_ascii_lowercase().as_str() {
        "desktop_contended" => {
            // Saturate earlier and scale harder (desktop Flatpak + many Chromes).
            input.contention.low = 12;
            input.contention.high = 28;
            input.contention.mid_percent = 220;
            input.contention.high_percent = 280;
            // Slightly more conservative unit times than lab.
            input.serp_seconds = input.serp_seconds.max(10);
            input.fetch_seconds = input.fetch_seconds.max(6);
            input.margin_percent = input.margin_percent.max(15);
        }
        "thin" => {
            // SERP-only style estimates for agent discovery / light runs.
            input.serp_seconds = 5;
            input.fetch_seconds = 3;
            input.margin_percent = 10;
            input.contention.low = 30;
            input.contention.high = 50;
            input.contention.mid_percent = 150;
            input.contention.high_percent = 200;
        }
        "lab" => {
            // Explicit lab: factory contention bands (leave unit times as already set).
            input.contention = crate::budget::contention::ContentionParams::default();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::DeepResearchBudgetInput;

    #[test]
    fn desktop_contended_raises_thresholds_and_units() {
        let mut input = DeepResearchBudgetInput::from_cli(3, true, 4, true, 0);
        let lab_serp = input.serp_seconds;
        apply_budget_profile(&mut input, PROFILE_DESKTOP_CONTENDED);
        assert_eq!(input.contention.low, 12);
        assert_eq!(input.contention.high, 28);
        assert!(input.serp_seconds >= lab_serp);
        assert!(input.contention.mid_percent >= 200);
    }

    #[test]
    fn thin_lowers_unit_times() {
        let mut input = DeepResearchBudgetInput::from_cli(3, true, 4, true, 0);
        apply_budget_profile(&mut input, PROFILE_THIN);
        assert_eq!(input.serp_seconds, 5);
        assert_eq!(input.fetch_seconds, 3);
    }

    #[test]
    fn known_names() {
        assert!(is_known_budget_profile("lab"));
        assert!(is_known_budget_profile("DESKTOP_CONTENDED"));
        assert!(is_known_budget_profile("thin"));
        assert!(!is_known_budget_profile("nope"));
    }
}
