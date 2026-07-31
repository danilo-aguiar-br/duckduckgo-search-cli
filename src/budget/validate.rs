// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure — budget gate decision (no I/O).
//! Fail-fast budget validation and auto-contention raise (CLI-AUTO-01).

use crate::budget::estimate::{estimate_deep_research_seconds, gate_deep_research_estimate};
use crate::budget::input::DeepResearchBudgetInput;
use crate::error::exit_codes;

/// Result of budget validation before Chrome launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetDecision {
    /// Timeout is sufficient (or under-budget / auto-raise allowed).
    Proceed {
        /// Raw wall-clock lower-bound seconds.
        estimate_seconds: u64,
        /// Gated seconds (with margin).
        gated_seconds: u64,
        /// Effective global timeout after optional auto-raise.
        effective_global_timeout: u64,
        /// Whether under-budget was overridden.
        allow_under_budget: bool,
        /// Whether auto-contention raised the timeout.
        auto_raised: bool,
    },
    /// Must abort with exit 2 — do not launch Chrome.
    Reject {
        /// Raw wall-clock lower-bound seconds.
        estimate_seconds: u64,
        /// Gated seconds (with margin).
        gated_seconds: u64,
        /// Configured global timeout (pre-raise).
        global_timeout_seconds: u64,
    },
}

/// Validate deep-research wall-clock budget against `--global-timeout`.
///
/// When `auto_contention_budget` is true and `global_timeout < suggested`,
/// proceeds with `effective_global_timeout = suggested` (CLI-AUTO-01).
#[must_use]
pub fn validate_deep_research_budget(
    input: DeepResearchBudgetInput,
    global_timeout_seconds: u64,
    allow_under_budget: bool,
) -> BudgetDecision {
    validate_deep_research_budget_ex(input, global_timeout_seconds, allow_under_budget, false)
}

/// Extended validate with auto-contention raise.
#[must_use]
pub fn validate_deep_research_budget_ex(
    input: DeepResearchBudgetInput,
    global_timeout_seconds: u64,
    allow_under_budget: bool,
    auto_contention_budget: bool,
) -> BudgetDecision {
    let estimate_seconds = estimate_deep_research_seconds(input);
    let gated_seconds = gate_deep_research_estimate(estimate_seconds, input.margin_percent);
    let suggested = gated_seconds;

    if auto_contention_budget && global_timeout_seconds < suggested {
        return BudgetDecision::Proceed {
            estimate_seconds,
            gated_seconds,
            effective_global_timeout: suggested,
            allow_under_budget: false,
            auto_raised: true,
        };
    }

    if global_timeout_seconds < gated_seconds && !allow_under_budget {
        BudgetDecision::Reject {
            estimate_seconds,
            gated_seconds,
            global_timeout_seconds,
        }
    } else {
        BudgetDecision::Proceed {
            estimate_seconds,
            gated_seconds,
            effective_global_timeout: global_timeout_seconds,
            allow_under_budget: allow_under_budget && global_timeout_seconds < gated_seconds,
            auto_raised: false,
        }
    }
}

/// Exit code for budget underflow (invalid config / agent contract).
#[must_use]
pub const fn budget_underflow_exit_code() -> i32 {
    exit_codes::INVALID_CONFIG
}
