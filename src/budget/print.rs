// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure — agent JSON payloads for budget discovery.
//! print-budget / underflow / product snapshot JSON (CLI-PRINT-01).

use crate::budget::estimate::{
    estimate_deep_research_seconds, gate_deep_research_estimate, input_contention_factor_percent,
    shell_timeout_hint, suggested_global_timeout, wall_estimate_seconds, work_estimate_seconds,
};
use crate::budget::input::DeepResearchBudgetInput;
use crate::types::bounded::DEFAULT_GLOBAL_TIMEOUT_SECONDS;

/// Agent-stable JSON payload for budget fail-fast (stdout).
#[must_use]
pub fn budget_underflow_payload(
    estimate_seconds: u64,
    gated_seconds: u64,
    global_timeout_seconds: u64,
    input: DeepResearchBudgetInput,
) -> serde_json::Value {
    serde_json::json!({
        "error": "budget_underflow",
        "type": "deep_research_error",
        "message": format!(
            "global timeout {global_timeout_seconds}s is below gated deep-research estimate {gated_seconds}s \
(raw estimate {estimate_seconds}s, margin {}%)",
            input.margin_percent
        ),
        "estimate_seconds": estimate_seconds,
        "gated_seconds": gated_seconds,
        "global_timeout_seconds": global_timeout_seconds,
        "suggested_global_timeout": suggested_global_timeout(input),
        "shell_timeout_hint": shell_timeout_hint(input),
        "margin_percent": input.margin_percent,
        "max_sub_queries": input.max_sub_queries,
        "fetch_content": input.fetch_content,
        "fetch_content_cap": input.fetch_content_cap,
        "dual_vertical": input.dual_vertical,
        "runtime_dual_multiproc": input.runtime_dual_multiproc(),
        "parallelism": input.parallelism,
        "chrome_n": input.chrome_n,
        "contention_factor_percent": input_contention_factor_percent(input),
        "depth": input.depth,
        "next_action_suggestion":
            "Raise --global-timeout to at least suggested_global_timeout (or enable \
--auto-contention-budget), lower --max-sub-queries / --fetch-content-cap / --depth, \
pass --no-fetch-content or --no-news, keep -p>=2 for dual multiproc, or set \
--allow-under-budget (or XDG deep_research_allow_under_budget=true).",
    })
}

/// Full print-budget JSON (agent discovery; no Chrome).
#[must_use]
pub fn print_budget_payload(
    input: DeepResearchBudgetInput,
    global_timeout_seconds: u64,
    allow_under_budget: bool,
    auto_contention_budget: bool,
    effective_global_timeout: u64,
) -> serde_json::Value {
    let work = work_estimate_seconds(input);
    let wall = wall_estimate_seconds(input);
    let gated = gate_deep_research_estimate(wall, input.margin_percent);
    let suggested = suggested_global_timeout(input);
    let dual_mp = input.runtime_dual_multiproc();
    serde_json::json!({
        "type": "deep_research_budget",
        "estimate_seconds": wall,
        "work_estimate_seconds": work,
        "wall_estimate_seconds": wall,
        "gated_seconds": gated,
        "suggested_global_timeout": suggested,
        "shell_timeout_hint": shell_timeout_hint(input),
        "contention_factor_percent": input_contention_factor_percent(input),
        "chrome_n": input.chrome_n,
        "parallelism": input.parallelism,
        "slots_per_query": input.slots_per_query(),
        "dual_vertical": input.dual_vertical,
        "runtime_dual_multiproc": dual_mp,
        "global_timeout_seconds": global_timeout_seconds,
        "effective_global_timeout": effective_global_timeout,
        // budget_ok = user GT covers gated without needing auto-raise (agent contract).
        "budget_ok": global_timeout_seconds >= gated || allow_under_budget,
        "budget_ok_suggested": effective_global_timeout >= suggested
            || (auto_contention_budget && suggested > 0),
        "auto_contention_budget": auto_contention_budget,
        "margin_percent": input.margin_percent,
        "max_sub_queries": input.max_sub_queries,
        "fetch_content": input.fetch_content,
        "fetch_content_cap": input.fetch_content_cap,
        "depth": input.depth,
        "serp_seconds": input.serp_seconds,
        "fetch_seconds": input.fetch_seconds,
        "allow_under_budget": allow_under_budget,
        "default_global_timeout": DEFAULT_GLOBAL_TIMEOUT_SECONDS,
    })
}

/// Agent-stable JSON snapshot of **product defaults** for doctor / probe-deep.
#[must_use]
pub fn default_product_snapshot() -> serde_json::Value {
    use crate::cli::DEFAULT_FETCH_CONTENT_CAP;
    use crate::deep_research::DEFAULT_MAX_SUB_QUERIES;

    let input = DeepResearchBudgetInput::from_cli(
        DEFAULT_MAX_SUB_QUERIES,
        true,
        DEFAULT_FETCH_CONTENT_CAP,
        true,
        0,
    );
    let estimate_seconds = estimate_deep_research_seconds(input);
    let gated_seconds = gate_deep_research_estimate(estimate_seconds, input.margin_percent);
    serde_json::json!({
        "estimate_seconds": estimate_seconds,
        "gated_seconds": gated_seconds,
        "global_timeout_default": DEFAULT_GLOBAL_TIMEOUT_SECONDS,
        "budget_ok": gated_seconds <= DEFAULT_GLOBAL_TIMEOUT_SECONDS,
        "max_sub_queries": input.max_sub_queries,
        "fetch_content": input.fetch_content,
        "fetch_content_cap": input.fetch_content_cap,
        "dual_vertical": input.dual_vertical,
        "runtime_dual_multiproc": input.runtime_dual_multiproc(),
        "parallelism": input.parallelism,
        "depth": input.depth,
        "margin_percent": input.margin_percent,
        "serp_seconds": input.serp_seconds,
        "fetch_seconds": input.fetch_seconds,
    })
}
