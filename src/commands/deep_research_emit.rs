// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound serialization offloaded via `concurrency::run_cpu_bound`.
//! Result projection and envelope emission for the `deep-research` subcommand.
//!
//! Split out of `commands::deep_research` in v1.0.3 (plan Fase 8). This module
//! owns the last stage: turn one `Result<DeepResearchOutput, _>` into bytes on
//! stdout (or `-o`) and an exit code.
//!
//! # The exit-code ladder, in priority order
//!
//! The order below is the contract, and every rung exists because a previous
//! version got it wrong:
//!
//! 1. **Cancelled** wins over everything (DEEP-E2E-08 / V18). A cooperative
//!    SIGINT/SIGTERM must surface as 130/143, never as exit 5 "empty index",
//!    because an agent reading 5 would retry a run the operator just stopped.
//! 2. **`--require-results`** with zero rows.
//! 3. **`--require-all-sub-queries`** with any failed sub-query.
//! 4. Otherwise the **zero-result classification**: exit 0 when either vertical
//!    produced rows, exit 2 when empty *because* a sub-query hit a Chrome or
//!    config error, exit 5 only for a genuinely empty index.

use crate::error::{exit_codes, CliError};
use crate::output;
use crate::types::Config;
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// Parsed `--fields` / `--filter`, resolved once and reused by both the success
/// and the timeout-partial emit paths.
pub(super) struct Projection {
    /// Parsed `--fields` / `--select`.
    pub(super) fields: Option<output::FieldSet>,
    /// Parsed `--filter`.
    pub(super) filter: Option<output::ResultFilter>,
}

/// Parse the projection specs off the resolved [`Config`].
///
/// GAP-DEEP-PROJECT-FILTER: parsed once so the timeout path can project a
/// partial harvest with the same rules as a full success. Preflight already
/// rejected unknown tokens against the same allowlist, so reaching an error here
/// means the two allowlists drifted — fail closed rather than emit unprojected.
///
/// # Errors
///
/// Returns `INVALID_CONFIG` after writing the parser message to stderr.
pub(super) fn parse_projection(config: &Config) -> Result<Projection, i32> {
    let fields = match config.agent_ops.fields.as_deref() {
        Some(raw) => match output::FieldSet::parse(raw) {
            Ok(fs) => Some(fs),
            Err(err) => {
                output::emit_stderr(err.to_string());
                return Err(exit_codes::INVALID_CONFIG);
            }
        },
        None => None,
    };
    let filter = match config.agent_ops.filter.as_deref() {
        Some(raw) => match output::ResultFilter::parse(raw) {
            Ok(f) => Some(f),
            Err(err) => {
                output::emit_stderr(err.to_string());
                return Err(exit_codes::INVALID_CONFIG);
            }
        },
        None => None,
    };
    Ok(Projection { fields, filter })
}

/// Everything `finalize` needs beyond the run result itself.
pub(super) struct FinalizeContext<'a> {
    /// Resolved run configuration (agent-ops knobs live here).
    pub(super) config: &'a Config,
    /// Main's token — cancellation outranks every other exit classification.
    pub(super) cancellation: &'a CancellationToken,
    /// Parsed `--fields` / `--filter`.
    pub(super) projection: &'a Projection,
    /// Global `-o/--output` target.
    pub(super) output_file: Option<&'a Path>,
    /// `--require-results`.
    pub(super) require_results: bool,
    /// `--require-all-sub-queries`.
    pub(super) require_all_sub_queries: bool,
    /// Query text, quoted into the `--require-results` message.
    pub(super) query_for_error: &'a str,
}

/// Apply agent-ops, classify the outcome and emit the final envelope.
///
/// # Emission contract
///
/// This function always emits exactly one envelope on the success path and
/// returns the exit code; the caller runs one-shot cleanup and propagates it.
pub(super) async fn finalize(
    result: Result<crate::deep_research::DeepResearchOutput, CliError>,
    ctx: FinalizeContext<'_>,
) -> i32 {
    let mut deep_output = match result {
        Ok(o) => o,
        Err(err) => {
            output::emit_stderr(crate::i18n::error_msg(
                crate::i18n::Message::DeepResearchFailed,
                &err,
            ));
            // Signal-aware: SIGINT → 130, SIGTERM → 143 (graceful-shutdown rules).
            return crate::signals::exit_code_for_error(&err);
        }
    };

    let config = ctx.config;
    let fields = ctx.projection.fields.as_ref();

    // Agent-native order: filter → sort → dedupe → project → limit → truncate.
    let _pre = output::project::apply_to_deep_output(
        &mut deep_output,
        None,
        ctx.projection.filter.as_ref(),
    );
    let sort_spec = match config.agent_ops.sort.as_deref() {
        Some(raw) => match output::SortSpec::parse(raw) {
            Ok(s) => Some(s),
            Err(err) => {
                output::emit_stderr(err.to_string());
                return exit_codes::INVALID_CONFIG;
            }
        },
        None => None,
    };
    let dedupe = match config.agent_ops.dedupe_by.as_deref() {
        Some(raw) => match output::DedupeBy::parse(raw) {
            Ok(d) => Some(d),
            Err(err) => {
                output::emit_stderr(err.to_string());
                return exit_codes::INVALID_CONFIG;
            }
        },
        None => None,
    };
    output::apply_sort_deep(&mut deep_output, sort_spec.as_ref());
    output::apply_dedupe_deep(&mut deep_output, dedupe);
    let _ = output::project::apply_to_deep_output(&mut deep_output, fields, None);
    output::project::apply_result_limit_deep(&mut deep_output, config.agent_ops.limit);
    output::apply_truncate_content_deep(
        &mut deep_output,
        config.agent_ops.truncate_content.map(|n| n as usize),
    );

    if was_cancelled(ctx.cancellation, &deep_output) {
        // Emit partial envelope if any rows harvested; exit is signal-aware.
        let fields_for_emit = fields.cloned();
        let serialize = crate::concurrency::run_cpu_bound({
            let out = deep_output.clone();
            move || output::project::format_deep_json(&out, fields_for_emit.as_ref())
        })
        .await;
        if let Ok(Ok(json)) = serialize {
            let _ = output::emit_payload(&json, ctx.output_file);
        }
        return crate::signals::last_cancel_exit_code();
    }

    // v0.7.10 P4 / GAP-WS-1114: --require-results + zero results → non-zero.
    if ctx.require_results && deep_output.metadata.unique_result_count == 0 {
        let q = format!("{:?}", ctx.query_for_error);
        output::emit_stderr(crate::i18n::tf(
            crate::i18n::Message::DeepResearchZeroResultsRequire,
            &[("query", &q)],
        ));
        return exit_codes::GLOBAL_TIMEOUT;
    }

    if ctx.require_all_sub_queries && deep_output.metadata.sub_queries_error > 0 {
        let payload = output::sub_queries_incomplete_payload(
            deep_output.metadata.sub_queries_total,
            deep_output.metadata.sub_queries_ok,
            deep_output.metadata.sub_queries_error,
        );
        let _ = output::emit_payload(&payload.to_string(), ctx.output_file);
        // Still emit success body if hits exist? Agent strict: error JSON is enough.
        return exit_codes::INVALID_CONFIG;
    }

    let success_code = zero_result_exit_code(&deep_output);
    emit_body(deep_output, config, fields, ctx.output_file, success_code).await
}

/// True when this run was stopped cooperatively rather than finishing empty.
///
/// The token alone is not enough: a sub-query worker can observe the cancel and
/// record it as free text before aggregation returns `Ok`, so the sub-query
/// rows are inspected too. Missing that is what used to turn a SIGTERM into an
/// exit 5.
fn was_cancelled(
    cancellation: &CancellationToken,
    deep_output: &crate::deep_research::DeepResearchOutput,
) -> bool {
    cancellation.is_cancelled()
        || deep_output.metadata.sub_queries.iter().any(|s| {
            let status_cancel = s.status.eq_ignore_ascii_case("cancelled");
            let err_cancel = s.error.as_deref().is_some_and(|m| {
                let lower = m.to_ascii_lowercase();
                lower == "cancelled"
                    || lower.contains("cancel")
                    || lower.contains("sigterm")
                    || lower.contains("sigint")
            });
            let news_cancel = s.news_error.as_deref().is_some_and(|m| {
                let lower = m.to_ascii_lowercase();
                lower == "cancelled" || lower.contains("cancel")
            });
            status_cancel || err_cancel || news_cancel
        })
}

/// Exit code for a completed, non-cancelled run.
///
/// GAP-WS-105 + GAP-E2E-V11-DEEP-ZERO-EXIT5 / V12: exit 0 when either vertical
/// produced results; if empty AND any sub-query failed with a Chrome or config
/// class, exit 2 rather than exit 5 "empty index"; genuine empty → exit 5.
fn zero_result_exit_code(deep_output: &crate::deep_research::DeepResearchOutput) -> i32 {
    if !(deep_output.results.is_empty() && deep_output.news_count == 0) {
        return exit_codes::SUCCESS;
    }
    let wire_codes = deep_output.metadata.sub_queries.iter().map(|s| {
        // Prefer structured error when present; deep stores free text.
        s.error.as_deref().and_then(|msg| {
            if crate::error::chrome_classify::message_implies_chrome_or_config(msg) {
                Some(crate::error::codes::CHROME_UNAVAILABLE)
            } else if msg.to_ascii_lowercase().contains("invalid") {
                Some(crate::error::codes::INVALID_CONFIG)
            } else {
                None
            }
        })
    });
    let free_text = deep_output
        .metadata
        .sub_queries
        .iter()
        .map(|s| s.error.as_deref())
        .chain(
            deep_output
                .metadata
                .sub_queries
                .iter()
                .map(|s| s.news_error.as_deref()),
        );
    crate::error::exit_for_zero_results_with_errors(wire_codes, free_text)
}

/// Serialize and write the success body, preserving `success_code` on success.
///
/// GAP-PAR-040c + GAP-E2E-48-006: serialization runs off the async worker and
/// the write goes through the one `-o`-aware route. With `--fields` set, result
/// rows are projected through `Value` so keys like `score` / `fontes` drop.
async fn emit_body(
    deep_output: crate::deep_research::DeepResearchOutput,
    config: &Config,
    fields: Option<&output::FieldSet>,
    output_file: Option<&Path>,
    success_code: i32,
) -> i32 {
    if config.agent_ops.count_only {
        let payload = output::count_only_deep(&deep_output);
        return match output::emit_payload(&payload, output_file) {
            Ok(()) => success_code,
            Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
            Err(err) => {
                output::emit_stderr(crate::i18n::error_msg(
                    crate::i18n::Message::StdoutWriteFailed,
                    &err,
                ));
                exit_codes::GENERIC_ERROR
            }
        };
    }

    let fields_for_emit = fields.cloned();
    let serialize = crate::concurrency::run_cpu_bound(move || {
        output::project::format_deep_json(&deep_output, fields_for_emit.as_ref())
    })
    .await;
    match serialize {
        Ok(Ok(json)) => match output::emit_payload(&json, output_file) {
            Ok(()) => success_code,
            Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
            Err(err) => {
                output::emit_stderr(crate::i18n::error_msg(
                    crate::i18n::Message::StdoutWriteFailed,
                    &err,
                ));
                exit_codes::GENERIC_ERROR
            }
        },
        Ok(Err(err)) | Err(err) => {
            output::emit_stderr(crate::i18n::error_msg(
                crate::i18n::Message::DeepResearchSerializeFailed,
                &err,
            ));
            exit_codes::GENERIC_ERROR
        }
    }
}
