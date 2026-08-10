// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound orchestrator — fans out via deep_research::run → parallel.rs.
// Concurrency bound: root `--parallel` / `--max-concurrency`.
//! Handler for the `deep-research` subcommand.
//!
//! # Shape of this file
//!
//! v1.0.3 (plan Fase 8) reduced `execute_deep_research` from roughly 580 lines
//! carrying five responsibilities to an orchestrator that owns exactly one: the
//! ORDER of the stages, and the timeout fence around the fan-out. The stages
//! live in four sibling modules:
//!
//! The four are private siblings, so they are named here as code spans rather
//! than intra-doc links: linking a public page to a private item is a rustdoc
//! error under `-D warnings`, and the gate is right to refuse — the reader of
//! the published docs cannot follow such a link anywhere.
//!
//! | Stage | Module (private sibling) | Owns |
//! |---|---|---|
//! | 1 | `commands::deep_research_preflight` | XDG merge, trust boundary, agent-ops parse |
//! | 2 | `commands::deep_research_budget` | `--print-budget` and the fail-fast gate |
//! | 3 | `commands::deep_research_session` | Chrome gate and [`crate::types::Config`] assembly |
//! | 4 | `commands::deep_research_emit` | projection, exit ladder, envelope |
//!
//! The order is not cosmetic. Stage 2 runs before stage 3 on purpose (CM-01b):
//! a budget refusal has to be reachable on a host with no Chrome binary.
//!
//! Every stage emits its OWN envelope on failure and hands back only an exit
//! code, so this file never has to know which of them writes what.

use crate::cli::{CliArgs, CliIdentityProfile, DeepResearchArgs};
use crate::output;
use crate::types::bounded::DEEP_RESEARCH_TIMEOUT_GRACE_SECONDS;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use super::deep_research_budget::BudgetGate;
use super::deep_research_emit::FinalizeContext;
use super::deep_research_session::SessionRequest;

/// Executes the `deep-research` subcommand (v1.0.2 budget contract).
///
/// Runs the four stages in order, fences the fan-out with the effective global
/// timeout, and guarantees one-shot Chrome cleanup on every path that got far
/// enough to launch a browser.
///
/// Honors `--global-timeout` from the root parser (GAP-WS B3) and global
/// `-o/--output` (GAP-E2E-48-006): success and timeout envelopes share
/// [`output::emit_payload`].
///
/// v1.0.2: fail-fast budget gate **before** Chrome (GAP-AUD-DR-001 / CM-01).
pub async fn execute_deep_research(
    mut args: DeepResearchArgs,
    root_global_timeout_seconds: u64,
    search_defaults: &CliArgs,
    allow_lite_fallback: bool,
    pre_flight: bool,
    identity_profile: CliIdentityProfile,
    cancellation: CancellationToken,
) -> i32 {
    use crate::deep_research::run_deep_research;

    // Stage 1 — validation. Emits its own envelope on every failure.
    let pre = match super::deep_research_preflight::run(&mut args, search_defaults) {
        Ok(p) => p,
        Err(code) => return code,
    };

    // Stage 2 — budget. Must precede the Chrome gate (CM-01b).
    let budget_input =
        super::deep_research_budget::resolve_budget_input(&pre.dr, search_defaults, &pre.xdg);
    let budget_policy = super::deep_research_budget::resolve_budget_policy(&args, &pre.xdg);
    let effective_global_timeout = match super::deep_research_budget::run_gate(
        &args,
        search_defaults,
        budget_input,
        budget_policy,
        root_global_timeout_seconds,
        pre.output_file.as_deref(),
    ) {
        BudgetGate::Printed(code) | BudgetGate::Rejected(code) => return code,
        BudgetGate::Proceed(seconds) => seconds,
    };

    // CM-05: register in-flight so SIGTERM force-exit emits agent JSON before
    // reap. Armed here, before any browser exists, so the guard covers the whole
    // remaining lifetime of the run.
    let _deep_inflight = output::DeepInFlightGuard::arm(pre.output_file.as_deref());

    // Stage 3 — Chrome gate and session.
    let config = match super::deep_research_session::build(SessionRequest {
        validated_query: &pre.validated_query,
        dr: &pre.dr,
        search_defaults,
        output_file: pre.output_file.clone(),
        root_global_timeout_seconds,
        allow_lite_fallback,
        pre_flight,
        identity_profile,
    }) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let projection = match super::deep_research_emit::parse_projection(&config) {
        Ok(p) => p,
        Err(code) => return code,
    };

    // GAP-WS-TMP-PROFILE-ORPHAN-001: use main's CancellationToken (SIGINT/SIGTERM)
    // and fence with global timeout so Chrome sessions are cancelled + reaped.
    // GAP-E2E-48-007 / CM-05: pin future so cancel can harvest partials after timeout.
    // v1.0.2 CM-05: emit envelope **before** heavy oneshot cleanup.
    let global_timeout = Duration::from_secs(effective_global_timeout);
    let deep_future = run_deep_research(pre.dr, &config, cancellation.clone());
    tokio::pin!(deep_future);

    let result = match tokio::time::timeout(global_timeout, &mut deep_future).await {
        Ok(inner) => inner,
        Err(_elapsed) => {
            cancellation.cancel();
            let exit = harvest_timeout(
                deep_future,
                root_global_timeout_seconds,
                &projection,
                pre.output_file.as_deref(),
            )
            .await;
            #[cfg(feature = "chrome")]
            crate::process_lifecycle::ensure_oneshot_cleanup();
            return exit;
        }
    };

    // Stage 4 — projection, exit ladder, envelope.
    let exit = super::deep_research_emit::finalize(
        result,
        FinalizeContext {
            config: &config,
            cancellation: &cancellation,
            projection: &projection,
            output_file: pre.output_file.as_deref(),
            require_results: args.require_results,
            require_all_sub_queries: args.require_all_sub_queries,
            query_for_error: &pre.query_for_error,
        },
    )
    .await;

    #[cfg(feature = "chrome")]
    crate::process_lifecycle::ensure_oneshot_cleanup();
    exit
}

/// Best-effort partial harvest after the global timeout fired.
///
/// The cancel token has already been tripped by the caller. This gives the
/// in-flight fan-out a bounded grace window to hand back whatever rows it
/// gathered, then emits the timeout envelope BEFORE the caller runs one-shot
/// cleanup — reaping a Chrome tree takes long enough that an agent waiting on
/// stdout would otherwise see the delay as a hang (CM-05).
async fn harvest_timeout<F>(
    deep_future: std::pin::Pin<&mut F>,
    root_global_timeout_seconds: u64,
    projection: &super::deep_research_emit::Projection,
    output_file: Option<&std::path::Path>,
) -> i32
where
    F: std::future::Future<
        Output = Result<crate::deep_research::DeepResearchOutput, crate::error::CliError>,
    >,
{
    let grace = Duration::from_secs(DEEP_RESEARCH_TIMEOUT_GRACE_SECONDS);
    let partial = match tokio::time::timeout(grace, deep_future).await {
        Ok(Ok(mut harvested)) => {
            output::project::apply_to_deep_output(
                &mut harvested,
                projection.fields.as_ref(),
                projection.filter.as_ref(),
            );
            Some(harvested)
        }
        _ => None,
    };
    output::emit_stderr(crate::i18n::deep_research_timeout_exceeded(
        root_global_timeout_seconds,
    ));
    output::emit_timeout_envelope(
        root_global_timeout_seconds,
        partial.as_ref(),
        output_file,
        projection.fields.as_ref(),
    )
    .await
}
