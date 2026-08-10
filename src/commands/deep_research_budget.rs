// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light pure resolution — no I/O beyond one process-count probe.
//! Budget input and policy resolution for the `deep-research` subcommand.
//!
//! Split out of `commands::deep_research` in v1.0.3. `execute_deep_research` had
//! grown to roughly 650 lines carrying five unrelated responsibilities at once:
//! XDG merge, budget resolution, agent-ops parsing, fan-out and envelope emit.
//!
//! Budget resolution is the one piece that is genuinely self-contained — it
//! reads configuration and returns values, with no early return and no envelope
//! of its own — so it is the piece that can move without touching control flow.
//!
//! The move also removed a real defect of duplication: the seven XDG override
//! assignments were written out **twice**, verbatim, because the profile seed
//! has to be applied between them. Two copies of the same precedence rule is two
//! places to forget a new key. They are now a single function called twice.

use crate::budget::{
    budget_underflow_exit_code, budget_underflow_payload, print_budget_payload,
    validate_deep_research_budget_ex, BudgetDecision, DeepResearchBudgetInput,
};
use crate::cli::{CliArgs, DeepResearchArgs as CliDeepResearchArgs};
use crate::error::{exit_codes, CliError};
use crate::output;
use std::path::Path;

/// Budget-related policy switches resolved from CLI flags and XDG config.
#[derive(Debug, Clone, Copy)]
pub(super) struct BudgetPolicy {
    /// Proceed even when the estimate exceeds the effective global timeout.
    pub(super) allow_under: bool,
    /// Raise the global timeout automatically under Chrome contention.
    pub(super) auto_contention: bool,
}

/// Apply the per-key XDG budget overrides onto `input`.
///
/// Called twice on purpose: once before the profile seed and once after, so an
/// explicitly set key always outranks the profile it was seeded from (G4).
/// Keeping it in one function is what makes that precedence rule auditable.
fn apply_xdg_budget_overrides(
    input: &mut DeepResearchBudgetInput,
    xdg: &crate::runtime::user::UserConfig,
) {
    if let Some(s) = xdg.budget_serp_seconds() {
        input.serp_seconds = s;
    }
    if let Some(s) = xdg.budget_fetch_seconds() {
        input.fetch_seconds = s;
    }
    if let Some(m) = xdg.budget_safety_margin_percent() {
        input.margin_percent = m;
    }
    if let Some(v) = xdg.budget_contention_low() {
        input.contention.low = v;
    }
    if let Some(v) = xdg.budget_contention_high() {
        input.contention.high = v;
    }
    if let Some(v) = xdg.budget_contention_factor_mid_percent() {
        input.contention.mid_percent = v;
    }
    if let Some(v) = xdg.budget_contention_factor_high_percent() {
        input.contention.high_percent = v;
    }
}

/// Build the budget estimate input from the resolved domain args, the search
/// defaults and XDG configuration.
///
/// Precedence, highest first: explicit XDG key, then budget profile, then the
/// compiled factory default. `chrome_n` is sampled here because contention
/// scaling depends on how many Chrome-like processes are alive right now.
pub(super) fn resolve_budget_input(
    dr: &crate::deep_research::DeepResearchArgs,
    search_defaults: &CliArgs,
    xdg: &crate::runtime::user::UserConfig,
) -> DeepResearchBudgetInput {
    let mut input = DeepResearchBudgetInput::from_cli(
        dr.max_sub_queries,
        dr.fetch_content,
        search_defaults.fetch_content_cap,
        !dr.no_news,
        dr.depth,
    );
    input.parallelism = search_defaults.parallelism;
    input.chrome_n = crate::process_count::count_chrome_like_processes() as u64;
    input.shared_session_verticals = search_defaults.shared_session_verticals;

    apply_xdg_budget_overrides(&mut input, xdg);

    if let Some(profile) = xdg.budget_profile() {
        crate::budget::apply_budget_profile(&mut input, profile);
        // Explicit per-key XDG entries outrank the profile that just seeded them.
        apply_xdg_budget_overrides(&mut input, xdg);
    }

    input
}

/// Resolve the two budget policy switches.
///
/// `auto_contention` defaults to ON (agent-ready, CLI-AUTO-01): an agent hitting
/// a contended host should get a raised timeout rather than a budget refusal it
/// cannot interpret. The negative flag wins over the positive one so
/// `--no-auto-contention-budget` is always an unambiguous opt-out.
pub(super) fn resolve_budget_policy(
    args: &CliDeepResearchArgs,
    xdg: &crate::runtime::user::UserConfig,
) -> BudgetPolicy {
    let allow_under =
        args.allow_under_budget || xdg.deep_research_allow_under_budget().unwrap_or(false);

    let auto_contention = if args.no_auto_contention_budget {
        false
    } else if args.auto_contention_budget {
        true
    } else {
        xdg.deep_research_auto_contention_budget().unwrap_or(true)
    };

    BudgetPolicy {
        allow_under,
        auto_contention,
    }
}

/// Outcome of the pre-Chrome budget stage.
pub(super) enum BudgetGate {
    /// `--print-budget` ran: the payload is already emitted, this is the exit code.
    Printed(i32),
    /// The estimate does not fit the gate: the refusal envelope is already
    /// emitted, this is the exit code.
    Rejected(i32),
    /// Cleared to run. Carries the effective global timeout in seconds, which
    /// may exceed the requested one when auto-contention raised it.
    Proceed(u64),
}

/// Run `--print-budget` and the fail-fast budget gate.
///
/// # Ordering, and why it is load-bearing
///
/// CM-01b puts this stage BEFORE `require_chrome_transport`. A legacy 5×10 load
/// must fail fast with exit 2 on a host that has no Chrome binary at all,
/// because the answer "your budget cannot fit" does not depend on Chrome and an
/// agent needs it without installing one first.
///
/// # Emission contract
///
/// Both non-`Proceed` variants have ALREADY written their envelope, on stdout or
/// via `-o`. The caller must return the carried exit code and emit nothing else.
pub(super) fn run_gate(
    args: &CliDeepResearchArgs,
    search_defaults: &CliArgs,
    budget_input: DeepResearchBudgetInput,
    policy: BudgetPolicy,
    root_global_timeout_seconds: u64,
    output_file: Option<&Path>,
) -> BudgetGate {
    let BudgetPolicy {
        allow_under,
        auto_contention,
    } = policy;

    if args.print_budget {
        let decision = validate_deep_research_budget_ex(
            budget_input,
            root_global_timeout_seconds,
            allow_under,
            auto_contention,
        );
        let effective = match &decision {
            BudgetDecision::Proceed {
                effective_global_timeout,
                ..
            } => *effective_global_timeout,
            BudgetDecision::Reject {
                global_timeout_seconds,
                ..
            } => *global_timeout_seconds,
        };
        let payload = print_budget_payload(
            budget_input,
            root_global_timeout_seconds,
            allow_under,
            auto_contention,
            effective,
        );
        return BudgetGate::Printed(
            match output::emit_payload(&payload.to_string(), output_file) {
                Ok(()) => exit_codes::SUCCESS,
                Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
                Err(err) => {
                    output::emit_stderr(format!("failed to emit budget: {err}"));
                    exit_codes::GENERIC_ERROR
                }
            },
        );
    }

    // CM-01 / CLI-AUTO-01: fail-fast or auto-raise under contention.
    match validate_deep_research_budget_ex(
        budget_input,
        root_global_timeout_seconds,
        allow_under,
        auto_contention,
    ) {
        BudgetDecision::Reject {
            estimate_seconds,
            gated_seconds,
            global_timeout_seconds,
        } => {
            output::emit_stderr(crate::i18n::tf(
                crate::i18n::Message::DeepResearchBudgetUnderflow,
                &[
                    ("timeout", &global_timeout_seconds.to_string()),
                    ("gated", &gated_seconds.to_string()),
                    ("estimate", &estimate_seconds.to_string()),
                ],
            ));
            let payload = budget_underflow_payload(
                estimate_seconds,
                gated_seconds,
                global_timeout_seconds,
                budget_input,
            );
            // A write failure other than a broken pipe still reports the budget
            // refusal: the reason the run stops is the budget, not the write.
            BudgetGate::Rejected(
                match output::emit_payload(&payload.to_string(), output_file) {
                    Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
                    Ok(()) | Err(_) => budget_underflow_exit_code(),
                },
            )
        }
        BudgetDecision::Proceed {
            estimate_seconds,
            gated_seconds,
            effective_global_timeout: eff,
            allow_under_budget: warned,
            auto_raised,
        } => {
            if warned {
                output::emit_stderr(crate::i18n::tf(
                    crate::i18n::Message::DeepResearchBudgetAllowOverride,
                    &[
                        ("timeout", &root_global_timeout_seconds.to_string()),
                        ("gated", &gated_seconds.to_string()),
                        ("estimate", &estimate_seconds.to_string()),
                    ],
                ));
            }
            if auto_raised && !search_defaults.quiet {
                output::emit_stderr(format!(
                    "auto-contention-budget: raised global timeout {root_global_timeout_seconds}s → {eff}s \
(chrome_n={}, factor={}%, keep -p>=2 for dual multiproc; outer shell timeout hint: {}s)",
                    budget_input.chrome_n,
                    crate::budget::input_contention_factor_percent(budget_input),
                    crate::budget::shell_timeout_hint(budget_input)
                ));
            }
            BudgetGate::Proceed(eff)
        }
    }
}
