// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative budget gate — no fan-out, no I/O, no Chrome.
//! Deep-research wall-clock budget SSOT (v1.0.2 / GAP-AUD-DR-001+ / CLI-BUDGET-*).
//!
//! # Contract
//!
//! - [`estimate_deep_research_seconds`] — contention-aware **wall-clock** lower bound.
//! - [`work_estimate_seconds`] — slot-second work accounting (transparent).
//! - [`gate_deep_research_estimate`] — lower bound × safety margin.
//! - [`validate_deep_research_budget_ex`] — fail-fast / auto-raise before Chrome.
//!
//! Defaults with `chrome_n=0` and product dual multiproc (`-p` default ≥ 2) must
//! satisfy `gate(default_args) ≤ DEFAULT_GLOBAL_TIMEOUT_SECONDS`.

mod contention;
mod estimate;
mod input;
mod print;
mod profile;
mod validate;

pub use contention::{contention_factor_percent, scale_seconds_by_percent, ContentionParams};
pub use profile::{
    apply_budget_profile, is_known_budget_profile, PROFILE_DESKTOP_CONTENDED, PROFILE_LAB,
    PROFILE_THIN,
};
pub use estimate::{
    estimate_deep_research_seconds, gate_deep_research_estimate, gated_estimate,
    input_contention_factor_percent, per_sub_query_seconds, per_sub_query_wall_seconds,
    per_sub_query_work_seconds, shell_timeout_hint, suggested_global_timeout, wall_estimate_seconds,
    work_estimate_seconds,
};
pub use input::DeepResearchBudgetInput;
pub use print::{budget_underflow_payload, default_product_snapshot, print_budget_payload};
pub use validate::{
    budget_underflow_exit_code, validate_deep_research_budget, validate_deep_research_budget_ex,
    BudgetDecision,
};

/// Re-export grace for callers that already import budget.
pub use crate::types::bounded::DEEP_RESEARCH_TIMEOUT_GRACE_SECONDS as TIMEOUT_GRACE_SECONDS;

/// Grace seconds after timeout for partial harvest (re-export name stability).
#[must_use]
pub const fn timeout_grace_seconds() -> u64 {
    crate::types::bounded::DEEP_RESEARCH_TIMEOUT_GRACE_SECONDS
}

/// Whether default product knobs fit inside the default global timeout (regression SSOT).
#[must_use]
pub fn default_deep_research_budget_ok(
    max_sub_queries: usize,
    fetch_content: bool,
    fetch_content_cap: usize,
    dual_vertical: bool,
    depth: u32,
) -> bool {
    let input = DeepResearchBudgetInput::from_cli(
        max_sub_queries,
        fetch_content,
        fetch_content_cap,
        dual_vertical,
        depth,
    );
    gated_estimate(input) <= crate::types::bounded::DEFAULT_GLOBAL_TIMEOUT_SECONDS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::DEFAULT_FETCH_CONTENT_CAP;
    use crate::deep_research::DEFAULT_MAX_SUB_QUERIES;
    use crate::types::bounded::DEFAULT_GLOBAL_TIMEOUT_SECONDS;

    #[test]
    fn default_product_knobs_fit_global_timeout_with_margin() {
        assert!(
            default_deep_research_budget_ok(
                DEFAULT_MAX_SUB_QUERIES,
                true,
                DEFAULT_FETCH_CONTENT_CAP,
                true,
                0,
            ),
            "default deep-research gated estimate must fit DEFAULT_GLOBAL_TIMEOUT"
        );
        let input = DeepResearchBudgetInput::from_cli(
            DEFAULT_MAX_SUB_QUERIES,
            true,
            DEFAULT_FETCH_CONTENT_CAP,
            true,
            0,
        );
        let gated = gated_estimate(input);
        assert!(
            gated <= DEFAULT_GLOBAL_TIMEOUT_SECONDS,
            "gated={gated} > default timeout {DEFAULT_GLOBAL_TIMEOUT_SECONDS}"
        );
        assert!(input.runtime_dual_multiproc(), "default -p must enable dual multiproc");
    }

    #[test]
    fn p1_vs_p2_runtime_dual_differs() {
        let mut p1 = DeepResearchBudgetInput::from_cli(3, true, 4, true, 0);
        p1.parallelism = 1;
        let mut p2 = p1;
        p2.parallelism = 2;
        assert!(!p1.runtime_dual_multiproc());
        assert!(p2.runtime_dual_multiproc());
        assert!(
            wall_estimate_seconds(p1) >= wall_estimate_seconds(p2),
            "sequential dual should not be cheaper wall than multiproc dual"
        );
    }

    #[test]
    fn chrome_n_high_raises_suggested() {
        let mut lab = DeepResearchBudgetInput::from_cli(3, true, 4, true, 0);
        lab.chrome_n = 0;
        let mut hot = lab;
        hot.chrome_n = 45;
        let s_lab = suggested_global_timeout(lab);
        let s_hot = suggested_global_timeout(hot);
        assert!(s_hot > s_lab, "s_hot={s_hot} s_lab={s_lab}");
        assert!(s_hot >= 200, "contended dual+fetch should suggest large GT, got {s_hot}");
        assert_eq!(input_contention_factor_percent(hot), 250);
    }

    #[test]
    fn auto_raise_proceeds_when_gt_below_gated() {
        let mut input = DeepResearchBudgetInput::from_cli(3, true, 4, true, 0);
        input.chrome_n = 45;
        let gated = gated_estimate(input);
        match validate_deep_research_budget_ex(input, 159, false, true) {
            BudgetDecision::Proceed {
                effective_global_timeout,
                auto_raised,
                ..
            } => {
                assert!(auto_raised);
                assert!(effective_global_timeout >= gated);
            }
            BudgetDecision::Reject { .. } => panic!("auto should raise, not reject"),
        }
    }

    #[test]
    fn legacy_heavy_defaults_reject_without_allow() {
        let input = DeepResearchBudgetInput::from_cli(5, true, 10, true, 0);
        let d = validate_deep_research_budget(input, 180, false);
        match d {
            BudgetDecision::Reject {
                gated_seconds,
                global_timeout_seconds,
                ..
            } => {
                assert!(gated_seconds > global_timeout_seconds);
            }
            BudgetDecision::Proceed { .. } => panic!("expected reject for heavy workload"),
        }
    }

    #[test]
    fn allow_under_budget_proceeds() {
        let input = DeepResearchBudgetInput::from_cli(5, true, 10, true, 0);
        match validate_deep_research_budget(input, 180, true) {
            BudgetDecision::Proceed {
                allow_under_budget, ..
            } => assert!(allow_under_budget),
            BudgetDecision::Reject { .. } => panic!("allow flag must proceed"),
        }
    }

    #[test]
    fn depth_increases_estimate() {
        let base = DeepResearchBudgetInput::from_cli(3, true, 4, true, 0);
        let deep = DeepResearchBudgetInput::from_cli(3, true, 4, true, 2);
        assert!(
            estimate_deep_research_seconds(deep) > estimate_deep_research_seconds(base),
            "depth must increase estimate"
        );
    }

    #[test]
    fn no_fetch_dual_is_cheap() {
        let input = DeepResearchBudgetInput::from_cli(5, false, 10, true, 0);
        // dual multiproc: serp_wall=8, waves depend on p=5; work is higher
        assert!(estimate_deep_research_seconds(input) < 120);
        assert!(gated_estimate(input) <= DEFAULT_GLOBAL_TIMEOUT_SECONDS);
    }

    #[test]
    fn gate_applies_ten_percent_margin() {
        assert_eq!(gate_deep_research_estimate(100, 10), 110);
        assert_eq!(gate_deep_research_estimate(144, 10), 159);
        assert_eq!(gate_deep_research_estimate(50, 10), 55);
    }

    #[test]
    fn no_news_cheaper_than_dual() {
        let dual = DeepResearchBudgetInput::from_cli(3, true, 4, true, 0);
        let web = DeepResearchBudgetInput::from_cli(3, true, 4, false, 0);
        assert!(wall_estimate_seconds(web) <= wall_estimate_seconds(dual));
    }
}
