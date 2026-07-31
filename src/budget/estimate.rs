// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure CPU — wall-clock / work estimates (no Chrome).
//! Deep-research wall-clock and work estimates (CLI-BUDGET-03/04).

use crate::budget::contention::{contention_factor_percent, scale_seconds_by_percent};
use crate::budget::input::DeepResearchBudgetInput;

/// Cost of one sub-query in **wall** seconds (SERP + optional fetch, dual-aware).
///
/// When runtime dual multiproc is ON, web+news SERP share wall-clock (1× SERP)
/// and fetch verts run in parallel (1× fetch band). When dual_vertical is ON
/// but multiproc is OFF, verts are sequential (2× SERP and 2× fetch).
#[must_use]
pub fn per_sub_query_wall_seconds(input: DeepResearchBudgetInput) -> u64 {
    let dual_mp = input.runtime_dual_multiproc();
    let verts = if input.dual_vertical { 2_u64 } else { 1_u64 };
    let serp_wall = if input.dual_vertical {
        if dual_mp {
            input.serp_seconds
        } else {
            input.serp_seconds.saturating_mul(2)
        }
    } else {
        input.serp_seconds
    };
    let fetch_wall = if input.fetch_content {
        let cap = (input.fetch_content_cap as u64).max(1);
        let fetch_verts = if dual_mp { 1_u64 } else { verts };
        cap.saturating_mul(input.fetch_seconds).saturating_mul(fetch_verts)
    } else {
        0
    };
    serp_wall.saturating_add(fetch_wall)
}

/// Work (slot-seconds) lower bound — transparent accounting, not wall-clock.
///
/// Counts SERP×verts + fetch×verts even when multiproc parallelizes wall.
#[must_use]
pub fn per_sub_query_work_seconds(input: DeepResearchBudgetInput) -> u64 {
    let verts = if input.dual_vertical { 2_u64 } else { 1_u64 };
    let serp = input.serp_seconds.saturating_mul(verts);
    let fetch = if input.fetch_content {
        (input.fetch_content_cap as u64)
            .max(1)
            .saturating_mul(input.fetch_seconds)
            .saturating_mul(verts)
    } else {
        0
    };
    serp.saturating_add(fetch)
}

/// Legacy unit helper (fetch×verts, SERP 1×) — kept for callers that only pass
/// the five classic knobs. Prefer [`per_sub_query_wall_seconds`].
#[must_use]
pub fn per_sub_query_seconds(
    fetch_content: bool,
    fetch_content_cap: usize,
    dual_vertical: bool,
    serp_seconds: u64,
    fetch_seconds: u64,
) -> u64 {
    let mut input = DeepResearchBudgetInput::from_cli(1, fetch_content, fetch_content_cap, dual_vertical, 0);
    input.serp_seconds = serp_seconds;
    input.fetch_seconds = fetch_seconds;
    // Historical formula: SERP 1×, fetch × verts (used by older tests/docs).
    let verts = if dual_vertical { 2_u64 } else { 1_u64 };
    let fetch = if fetch_content {
        (fetch_content_cap as u64).max(1) * fetch_seconds * verts
    } else {
        0
    };
    serp_seconds.saturating_add(fetch)
}

fn depth_extra_seconds(input: DeepResearchBudgetInput, unit: u64) -> u64 {
    let per_round = input.max_sub_queries.clamp(1, 4) as u64;
    u64::from(input.depth)
        .saturating_mul(per_round)
        .saturating_mul(unit)
}

/// Work estimate (sum of slot-seconds) including depth.
#[must_use]
pub fn work_estimate_seconds(input: DeepResearchBudgetInput) -> u64 {
    let n = input.max_sub_queries.max(1) as u64;
    let unit = per_sub_query_work_seconds(input);
    let base = n.saturating_mul(unit);
    base.saturating_add(depth_extra_seconds(input, unit))
}

/// Wall-clock estimate: JoinSet waves × contended unit (+ depth).
///
/// Under host Chrome contention (`factor > 100%`), also floors by
/// `work_estimate × factor` so desktop hosts with dozens of Chromes are not
/// told dual+fetch “fits” in lab-ideal wall (CLI-BUDGET-01).
#[must_use]
pub fn wall_estimate_seconds(input: DeepResearchBudgetInput) -> u64 {
    let n = input.max_sub_queries.max(1) as u64;
    let unit = per_sub_query_wall_seconds(input);
    let factor = contention_factor_percent(input.chrome_n, input.contention);
    let unit_c = scale_seconds_by_percent(unit, factor);
    let slots = u64::from(input.slots_per_query().max(1));
    let chrome_budget = crate::concurrency::chrome_process_budget(input.parallelism).max(1) as u64;
    // How many sub-queries can run concurrently given slot cost.
    let query_capacity = (chrome_budget / slots).max(1);
    let waves = n.div_ceil(query_capacity).max(1);
    let base = waves.saturating_mul(unit_c);
    // Depth rounds: each may schedule up to min(4, max_sub) follow-ups.
    let depth = depth_extra_seconds(input, unit_c);
    let waves_wall = base.saturating_add(depth);

    if factor > 100 {
        // Contention floor: work × factor (honest under saturated host).
        let work_floor = scale_seconds_by_percent(work_estimate_seconds(input), factor);
        waves_wall.max(work_floor)
    } else {
        waves_wall
    }
}

/// Primary estimate used by the gate: **wall-clock** (contention-aware).
#[must_use]
pub fn estimate_deep_research_seconds(input: DeepResearchBudgetInput) -> u64 {
    wall_estimate_seconds(input)
}

/// Apply safety margin: `ceil(estimate * (100 + margin) / 100)`.
#[must_use]
pub fn gate_deep_research_estimate(estimate: u64, margin_percent: u64) -> u64 {
    if estimate == 0 {
        return 0;
    }
    let margin = margin_percent.min(100);
    let num = estimate.saturating_mul(100u64.saturating_add(margin));
    num.div_ceil(100)
}

/// Gate estimate for the given input (wall + margin).
#[must_use]
pub fn gated_estimate(input: DeepResearchBudgetInput) -> u64 {
    gate_deep_research_estimate(estimate_deep_research_seconds(input), input.margin_percent)
}

/// Suggested global timeout (gated wall). Floor can be applied by profile later.
#[must_use]
pub fn suggested_global_timeout(input: DeepResearchBudgetInput) -> u64 {
    gated_estimate(input)
}

/// Shell timeout hint: suggested + extra seconds for outer `timeout(1)`.
#[must_use]
pub fn shell_timeout_hint(input: DeepResearchBudgetInput) -> u64 {
    suggested_global_timeout(input)
        .saturating_add(crate::types::bounded::BUDGET_SHELL_TIMEOUT_HINT_EXTRA_SECONDS)
}

/// Contention factor percent for the input's chrome_n.
#[must_use]
pub fn input_contention_factor_percent(input: DeepResearchBudgetInput) -> u64 {
    contention_factor_percent(input.chrome_n, input.contention)
}
