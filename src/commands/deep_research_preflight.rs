// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light validation — one config read, no network, no fan-out.
//! Argument validation and agent-ops parsing for the `deep-research` subcommand.
//!
//! Split out of `commands::deep_research` in v1.0.3 (plan Fase 8). This is the
//! block that runs before anything can fail expensively: XDG merge, the trust
//! boundary on the query, the output-path check, and the fail-closed parse of
//! `--fields` / `--filter`.
//!
//! # Why these four belong together
//!
//! They share one property that nothing later in the handler has: each is a
//! pure precondition whose failure is a *user input* error, emitted as a thin
//! error envelope and mapped to an exit code, with no Chrome, no budget and no
//! in-flight guard involved yet. Grouping them makes the handler's first
//! decision explicit — either the request is well formed, or the process ends
//! here.

use crate::cli::{CliArgs, DeepResearchArgs};
use crate::error::exit_codes;
use crate::output;
use crate::runtime::user::UserConfig;
use crate::security::ValidatedQuery;
use std::path::PathBuf;

/// Everything the handler needs once the request is known to be well formed.
pub(super) struct Preflight {
    /// XDG configuration, read once and reused by the budget resolver.
    pub(super) xdg: UserConfig,
    /// Query after the trust boundary (`ValidatedQuery`), NFC-normalised.
    pub(super) validated_query: ValidatedQuery,
    /// Plain copy of the query, kept for the `--require-results` message.
    pub(super) query_for_error: String,
    /// Global `-o/--output` target, already path-validated.
    pub(super) output_file: Option<PathBuf>,
    /// Domain args mapped from clap, with query and `no_news` overridden.
    pub(super) dr: crate::deep_research::DeepResearchArgs,
}

/// Validate the request and build [`Preflight`].
///
/// # Emission contract
///
/// On failure this function has ALREADY written its own envelope — a thin error
/// on stdout for the query / `--fields` / `--filter` paths, or a plain stderr
/// line for the output-path path. The caller must return the carried exit code
/// verbatim and emit nothing further.
///
/// # Errors
///
/// Returns the exit code to propagate when the query fails the trust boundary,
/// when `-o` points at a rejected path, or when `--fields` / `--filter` carry a
/// token outside the allowlist.
pub(super) fn run(
    args: &mut DeepResearchArgs,
    search_defaults: &CliArgs,
) -> Result<Preflight, i32> {
    // Apply XDG deep knobs when CLI still at clap defaults (GAP-AUD-DR-006).
    let xdg = crate::commands::config_cmd::load_runtime_user_config();
    apply_xdg_to_deep_args(args, search_defaults, &xdg);

    let effective_no_news = args.no_news;

    // Trust boundary (GAP-SECDEV-008): deep-research must not bypass ValidatedQuery.
    // --print-budget may use a placeholder query.
    let query_raw = if args.print_budget && args.query.trim().is_empty() {
        "budget".to_string()
    } else {
        args.query.clone()
    };
    let validated_query = match ValidatedQuery::try_new(&query_raw) {
        Ok(v) => v,
        Err(e) => {
            let payload = crate::types::ThinErrorResponse::new(e.error_code(), format!("{e}"))
                .with_suggestion(
                    "Provide a non-empty query without control/bidi characters (max 2048 chars).",
                );
            let _ = output::emit_wire_line(&payload);
            return Err(e.exit_code());
        }
    };
    let query_for_error = validated_query.as_str().to_string();

    // GAP-E2E-48-006: honor global `-o` (same contract as buscar).
    let output_file = search_defaults.output_file.clone();
    if let Some(ref path) = output_file {
        if let Err(e) = crate::paths::validate_output_path(path) {
            output::emit_stderr(e.to_string());
            return Err(exit_codes::INVALID_CONFIG);
        }
    }

    // CM-15b: single map clap → domain (DRY). Override query (validated) and no_news.
    let mut dr = args.clone().into_domain();
    dr.query = validated_query.as_str().to_string();
    dr.no_news = effective_no_news;

    reject_unknown_agent_ops(args, search_defaults)?;

    Ok(Preflight {
        xdg,
        validated_query,
        query_for_error,
        output_file,
        dr,
    })
}

/// Fail closed on `--fields` / `--filter` even when `--print-budget` is set.
///
/// Accepting an unknown token here and only breaking on the full run would cost
/// an agent a whole Chrome fan-out to discover a typo, so both are parsed before
/// the budget gate rather than at the emit boundary where they are used.
///
/// # Errors
///
/// Returns the exit code after emitting the thin error envelope itself.
fn reject_unknown_agent_ops(args: &DeepResearchArgs, search_defaults: &CliArgs) -> Result<(), i32> {
    let root_fields = search_defaults.agent.fields_or_select();
    let deep_fields = args.agent.fields_or_select();
    if let Some(raw) = root_fields.as_deref().or(deep_fields.as_deref()) {
        if let Err(e) = output::FieldSet::parse(raw) {
            let payload = crate::types::ThinErrorResponse::new(e.error_code(), format!("{e}"))
                .with_suggestion("Use --fields with allowlisted PT wire keys or EN aliases (e.g. title,url or titulo,url).");
            let _ = output::emit_wire_line(&payload);
            return Err(e.exit_code());
        }
    }
    if let Some(raw) = search_defaults
        .agent
        .result_filter
        .as_deref()
        .or(args.agent.result_filter.as_deref())
    {
        if let Err(e) = output::ResultFilter::parse(raw) {
            let payload = crate::types::ThinErrorResponse::new(e.error_code(), format!("{e}"))
                .with_suggestion("Use --filter with titulo~/title~, url~, snippet~, or host:.");
            let _ = output::emit_wire_line(&payload);
            return Err(e.exit_code());
        }
    }
    Ok(())
}

/// Apply XDG deep-research knobs when CLI left clap defaults.
fn apply_xdg_to_deep_args(
    args: &mut DeepResearchArgs,
    search_defaults: &CliArgs,
    xdg: &UserConfig,
) {
    use crate::deep_research::DEFAULT_MAX_SUB_QUERIES;
    if args.max_sub_queries == DEFAULT_MAX_SUB_QUERIES {
        if let Some(n) = xdg.default_max_sub_queries() {
            args.max_sub_queries = n;
        }
    }
    if !args.allow_under_budget && xdg.deep_research_allow_under_budget() == Some(true) {
        args.allow_under_budget = true;
    }
    // fetch_content_cap lives on search_defaults (root flatten); applied in config_cmd.
    let _ = search_defaults;
}
