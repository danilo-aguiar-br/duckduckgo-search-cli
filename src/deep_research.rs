// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: **mixed pipeline**
// 1. decomposition — CPU-light / sequential (heuristic templates)
// 2. fan-out — I/O-bound multi-process Chrome (JoinSet + Semaphore)
// 3. dual aggregation — CPU-light, parallel via spawn_blocking + join!
// 4. synthesis — CPU-light sequential (string build)
// Bound: `--parallel` / `--max-concurrency` on stage 2.
//! Deep research subcommand — query fan-out, aggregation, and synthesis.
//!
//! This module is the single entry point for the `deep-research` subcommand. It
//! composes four smaller modules into an 8-stage pipeline:
//!
//! 1. `decomposition::decompose` — splits the user query into 1..=`max_sub_queries`
//!    sub-queries using a heuristic, manual, or template strategy.
//! 2. `parallel::execute_query_fan_out` — fans out all sub-queries via `JoinSet`
//!    with a `Semaphore` for bounded concurrency and `CancellationToken` for
//!    graceful shutdown.
//! 3. `aggregation::aggregate` — merges the per-sub-query result lists into a
//!    single ranked list using Reciprocal Rank Fusion (RRF, K=60) or
//!    URL-canonical deduplication.
//! 4. `synthesis::synthesize_dual` — optionally combines the top-K web and
//!    news results into a Markdown/PlainText/Json report with numbered
//!    references.
//!
//! # Design notes
//!
//! - The output struct uses Portuguese JSON keys (`metadados`, `resultados`,
//!   `sintese`) to keep the public schema consistent with the rest of the CLI.
//! - `SubQuery` carries its origin (heuristic/manual/template) and a per-query
//!   `elapsed_ms` for observability.
//! - Aggregation emits a score in `[0.0, 1.0]` — higher is better — plus a
//!   `fontes: Vec<String>` listing the sub-query texts that produced the result
//!   for traceability.
//!
//! Content fetches performed by the synthesis stage honour the same
//! per-host rate limiting, circuit breaker, and concurrency controls as
//! the rest of the binary.
//!
//! # Example
//!
//! The function is the entry point called from `lib::execute_deep_research`.
//! End-to-end usage requires a fully-built [`Config`] (the CLI's
//! `lib::execute_deep_research` builds one from the global flags), the
//! operator-provided [`DeepResearchArgs`], and a [`CancellationToken`].
//!
//! ```no_run
//! use duckduckgo_search_cli::deep_research::{
//!     DeepResearchArgs, SubQueryStrategy, AggregationStrategyKind,
//! };
//! use duckduckgo_search_cli::synthesis::SynthFormat;
//!
//! let _args = DeepResearchArgs {
//!     query: "rust async".to_string(),
//!     max_sub_queries: 2,
//!     sub_query_strategy: SubQueryStrategy::Heuristic,
//!     sub_queries_file: None,
//!     aggregation: AggregationStrategyKind::Rrf,
//!     depth: 0,
//!     fetch_content: false,
//!     synthesize: false,
//!     budget_tokens: 4000,
//!     synth_format: SynthFormat::Markdown,
//!     no_news: false,
//! };
//! // See `lib::execute_deep_research` for a complete wiring example that
//! // builds a `Config` from CLI args and dispatches the pipeline.
//! ```

use crate::aggregation::{
    aggregate, aggregate_news, AggregatedItem, AggregatedNewsItem, AggregationStrategy,
};
use crate::decomposition::{decompose, SubQuery};
use crate::error::CliError;
use crate::parallel::execute_parallel_searches;
use crate::synthesis::{synthesize_dual, SynthFormat, SynthesizedReport};
use crate::types::{Config, SearchOutput};
use crate::validation;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError, ValidationErrors};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// Hard upper bound on the number of sub-queries produced by decomposition.
pub const MAX_SUB_QUERIES: usize = 12;

/// Default number of sub-queries when the user does not specify.
pub const DEFAULT_MAX_SUB_QUERIES: usize = 5;

/// Hard upper bound on the depth (number of reflective rounds).
pub const MAX_DEPTH: u32 = 3;

/// Default RRF constant (K in `1 / (K + rank)`). 60 is the de-facto literature
/// default (Cormack et al., 2009) and matches the `SQLite` `FTS5` anchor used
/// in the `GraphRAG` memory subsystem.
pub const RRF_K: u32 = 60;

// Compile-time invariants for deep-research limits.
const _: () = assert!(DEFAULT_MAX_SUB_QUERIES <= MAX_SUB_QUERIES && DEFAULT_MAX_SUB_QUERIES >= 1);
const _: () = assert!(MAX_DEPTH >= 1);
const _: () = assert!(RRF_K >= 1);

/// Strategy used to decompose the original query into sub-queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubQueryStrategy {
    /// Heuristic fan-out (5 canonical templates: aspect, comparison, timeline,
    /// opinion, cause). Default — pure local computation, no LLM cost.
    Heuristic,
    /// Pre-defined list of sub-queries from a file or stdin.
    Manual,
}

/// Strategy used to merge per-sub-query results into a single ranked list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationStrategyKind {
    /// Reciprocal Rank Fusion across sub-queries (K=60 by default).
    Rrf,
    /// Canonical-URL deduplication (keep first occurrence, drop the rest).
    DedupeByUrl,
}

/// Arguments for the `deep-research` subcommand.
#[derive(Debug, Clone)]
pub struct DeepResearchArgs {
    /// The original user query.
    pub query: String,
    /// Maximum number of sub-queries to produce (1..=12, default 5).
    pub max_sub_queries: usize,
    /// Decomposition strategy (default `Heuristic`).
    pub sub_query_strategy: SubQueryStrategy,
    /// Optional path to a file containing explicit sub-queries (one per line).
    /// Only honoured when `sub_query_strategy = Manual`.
    pub sub_queries_file: Option<std::path::PathBuf>,
    /// Aggregation strategy (default `Rrf`).
    pub aggregation: AggregationStrategyKind,
    /// Reflection depth — number of follow-up gap-filling rounds (0..=3).
    /// 0 = single pass, 1..=3 = iterative refinement using the top-ranked
    /// results to inform new sub-queries.
    pub depth: u32,
    /// When `true`, enable `--fetch-content` behaviour for the top-K results.
    pub fetch_content: bool,
    /// When `true`, produce a synthesised report (Markdown/PlainText/Json).
    pub synthesize: bool,
    /// Token budget for the synthesised report (~4 chars ≈ 1 token heuristic).
    pub budget_tokens: usize,
    /// Format of the synthesised report (only used when `synthesize` is true).
    pub synth_format: SynthFormat,
    /// GAP-WS-105 v0.8.9: when `true`, skips the news vertical (the fan-out
    /// runs web-only). Default `false` — deep-research applies the `all`
    /// vertical (web + news) to every sub-query.
    pub no_news: bool,
}

impl Default for DeepResearchArgs {
    fn default() -> Self {
        Self {
            query: String::new(),
            max_sub_queries: DEFAULT_MAX_SUB_QUERIES,
            sub_query_strategy: SubQueryStrategy::Heuristic,
            sub_queries_file: None,
            aggregation: AggregationStrategyKind::Rrf,
            depth: 0,
            fetch_content: false,
            synthesize: false,
            budget_tokens: 4000,
            synth_format: SynthFormat::Markdown,
            no_news: false,
        }
    }
}

impl DeepResearchArgs {
    /// Validates that the field ranges are within documented bounds.
    ///
    /// # Errors
    ///
    /// Returns an error string when `max_sub_queries` is zero or exceeds
    /// [`MAX_SUB_QUERIES`], or when `depth` exceeds [`MAX_DEPTH`].
    ///
    /// # Examples
    ///
    /// ```
    /// use duckduckgo_search_cli::deep_research::{
    ///     DeepResearchArgs, MAX_SUB_QUERIES, MAX_DEPTH,
    /// };
    ///
    /// // Defaults are valid.
    /// assert!(DeepResearchArgs::default().validate().is_ok());
    ///
    /// // Zero is rejected.
    /// let mut args = DeepResearchArgs::default();
    /// args.max_sub_queries = 0;
    /// assert!(args.validate().is_err());
    ///
    /// // Above MAX_SUB_QUERIES is rejected.
    /// let mut args = DeepResearchArgs::default();
    /// args.max_sub_queries = MAX_SUB_QUERIES + 1;
    /// assert!(args.validate().is_err());
    ///
    /// // Depth above MAX_DEPTH is rejected.
    /// let mut args = DeepResearchArgs::default();
    /// args.depth = MAX_DEPTH + 1;
    /// assert!(args.validate().is_err());
    /// ```
    /// CLI-facing validation. Delegates to [`Validate`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError::InvalidConfig`] when any field is out of range
    /// (max sub-queries, depth, budget, etc.).
    pub fn validate(&self) -> Result<(), crate::error::CliError> {
        Validate::validate(self).map_err(|errors| {
            validation::log_validation_errors("deep_research", &errors);
            crate::error::CliError::invalid_config(errors.to_string())
        })
    }
}

impl Validate for DeepResearchArgs {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        if self.max_sub_queries == 0 {
            let mut err = ValidationError::new("range");
            err.message = Some(
                format!(
                    "--max-sub-queries must be at least 1 (got {})",
                    self.max_sub_queries
                )
                .into(),
            );
            errors.add("max_sub_queries", err);
        } else if self.max_sub_queries > MAX_SUB_QUERIES {
            let mut err = ValidationError::new("range");
            err.message = Some(
                format!(
                    "--max-sub-queries cannot exceed {MAX_SUB_QUERIES} (got {})",
                    self.max_sub_queries
                )
                .into(),
            );
            errors.add("max_sub_queries", err);
        }
        if self.depth > MAX_DEPTH {
            let mut err = ValidationError::new("range");
            err.message = Some(
                format!("--depth cannot exceed {MAX_DEPTH} (got {})", self.depth).into(),
            );
            errors.add("depth", err);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Per-sub-query outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubQueryOutcome {
    /// The sub-query text.
    #[serde(rename = "texto", alias = "text")]
    pub text: String,
    /// Origin label — `heuristic`, `manual`, or template name.
    #[serde(rename = "estrategia", alias = "strategy")]
    pub strategy: String,
    /// Status: `ok` when results were produced, `erro` otherwise.
    #[serde(rename = "status")]
    pub status: String,
    /// Wall-clock duration for this sub-query (milliseconds).
    #[serde(rename = "tempo_ms", alias = "elapsed_ms")]
    pub elapsed_ms: u64,
    /// Optional error message when `status == "erro"`.
    #[serde(rename = "mensagem_erro", alias = "error", skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Number of news items returned by this sub-query's news scan. `None`
    /// when the news vertical was skipped (`--no-news`) or unavailable.
    /// GAP-WS-105 v0.8.9.
    #[serde(
        rename = "quantidade_noticias",
        alias = "news_count",
        skip_serializing_if = "Option::is_none"
    )]
    pub news_count: Option<usize>,
    /// `Some(true)` when the news scan was expected but the news vertical
    /// became unavailable mid-flight (Chrome fell and the web search degraded
    /// to HTTP). Omitted otherwise. GAP-WS-105 v0.8.9.
    #[serde(
        rename = "news_indisponivel",
        alias = "news_unavailable",
        skip_serializing_if = "Option::is_none"
    )]
    pub news_unavailable: Option<bool>,
}

/// Top-level output of the `deep-research` subcommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchOutput {
    /// Schema discriminator (always `"deep_research"`).
    #[serde(rename = "tipo", alias = "kind")]
    pub kind: String,
    /// Original user query (mirrors `SearchOutput.query` for schema parity).
    pub query: String,
    /// Run metadata (query, sub-queries, timings, etc.).
    #[serde(rename = "metadados", alias = "metadata")]
    pub metadata: DeepResearchMetadata,
    /// Aggregated evidence list (sorted by descending score).
    #[serde(rename = "resultados", alias = "results")]
    pub results: Vec<AggregatedItem>,
    /// Aggregated news list (GAP-WS-105 v0.8.9). Always serialized — empty
    /// when zero news items were found or when `--no-news` was passed.
    #[serde(rename = "noticias", alias = "news", default)]
    pub news: Vec<AggregatedNewsItem>,
    /// Number of aggregated news items. Always serialized. GAP-WS-105 v0.8.9.
    #[serde(rename = "quantidade_noticias", alias = "news_count", default)]
    pub news_count: usize,
    /// Optional synthesised report.
    #[serde(rename = "sintese", alias = "synth", skip_serializing_if = "Option::is_none")]
    pub synth: Option<SynthesizedReport>,
}

/// Run-level metadata for a deep-research execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchMetadata {
    /// Original user query.
    #[serde(rename = "query_original")]
    pub original_query: String,
    /// Sub-queries that were dispatched.
    #[serde(rename = "sub_queries")]
    pub sub_queries: Vec<SubQueryOutcome>,
    /// Aggregation strategy used.
    #[serde(rename = "estrategia_agregacao")]
    pub aggregation_strategy: String,
    /// Number of unique results after deduplication / RRF.
    #[serde(rename = "total_resultados_unicos")]
    pub unique_result_count: usize,
    /// Number of unique news items after news aggregation (parity with
    /// `total_resultados_unicos`). GAP-WS-105 v0.8.9.
    #[serde(rename = "total_noticias_unicas", default)]
    pub unique_news_count: usize,
    /// End-to-end wall-clock duration (milliseconds).
    #[serde(rename = "tempo_total_ms")]
    pub total_elapsed_ms: u64,
    /// Anti-bot cascade level reached during the deepest sub-query.
    #[serde(rename = "nivel_cascata")]
    pub cascade_level: Option<u8>,
    /// True when any sub-query used Chrome/chromiumoxide (agent contract).
    #[serde(rename = "usou_chrome", default)]
    pub used_chrome: bool,
    /// Resolved Chrome/Chromium binary after shell/Flatpak resolution (agent contract — agent contract field).
    #[serde(
        rename = "chrome_path_resolvido",
        skip_serializing_if = "Option::is_none"
    )]
    pub chrome_path_resolved: Option<String>,
    /// Install channel: `manual|env|host|flatpak|snap`.
    #[serde(rename = "chrome_canal", skip_serializing_if = "Option::is_none")]
    pub chrome_channel: Option<String>,
}

/// Runs the full deep-research pipeline.
///
/// # Arguments
///
/// * `args` — user-facing deep-research options.
/// * `cfg` — base [`Config`] inherited from the global CLI flags (HTTP client,
///   timeouts, proxies, etc.).
/// * `cancel` — token that signals SIGINT / global timeout.
///
/// # Errors
///
/// Returns an error only for unrecoverable setup failures (e.g. invalid
/// `sub_queries_file` when strategy is `Manual`). Per-sub-query failures are
/// captured inside the returned [`DeepResearchOutput`] rather than propagated
/// as `Err`, so partial results are always surfaced.
///
/// # Cancel safety
///
/// This function is cancel-safe. The `CancellationToken` is propagated to every
/// spawned sub-task; on cancellation, in-flight HTTP requests abort, partial
/// results are collected, and a `DeepResearchOutput` is returned with the
/// remaining sub-queries marked as `status == "erro"`.
pub async fn run_deep_research(
    args: DeepResearchArgs,
    cfg: &Config,
    cancel: CancellationToken,
) -> Result<DeepResearchOutput, CliError> {
    args.validate()?;

    let start_total = Instant::now();

    // Stage 1: decompose the original query.
    let sub_queries: Vec<SubQuery> = decompose(
        &args.query,
        args.sub_query_strategy,
        args.sub_queries_file.as_deref(),
        args.max_sub_queries,
        &cancel,
    )
    .await?;

    // Stage 2: fan out — `SubQuery.text` is already `ValidatedQuery` (GAP-TYPE-002/013).
    // JoinSet + Semaphore bound = `--parallel`. Each Chrome query is a separate OS process.
    crate::concurrency::log_chrome_concurrency_advisory(cfg.parallelism.get());
    let per_query_outputs: std::sync::Arc<Vec<SearchOutput>> = std::sync::Arc::new(
        execute_parallel_searches(
            sub_queries.iter().map(|q| q.text.clone()).collect(),
            cfg.clone(),
            cancel.clone(),
        )
        .await?
        .searches,
    );

    // Build the per-sub-query outcome report.
    let mut outcomes: Vec<SubQueryOutcome> = sub_queries
        .iter()
        .zip(per_query_outputs.iter())
        .map(|(q, o)| {
            // GAP-WS-105: `news: Some(v)` means the news scan ran (even with
            // zero items); `news: None` without --no-news signals the news
            // vertical became unavailable mid-flight.
            let (news_count, news_unavailable) =
                sub_query_news_fields(args.no_news, o.news.as_ref().map(Vec::len));
            SubQueryOutcome {
                text: q.text.as_str().to_string(),
                // `strategy_label` already returns an owned `String`.
                strategy: q.strategy_label(),
                status: if o.error.is_some() { "erro" } else { "ok" }.to_string(),
                elapsed_ms: o.metadata.execution_time_ms,
                error: o.error.clone(),
                news_count,
                news_unavailable,
            }
        })
        .collect();

    // Stage 3: aggregate across sub-queries.
    //
    // Workload: CPU-light merge (RRF / URL dedupe) over a small N
    // (max_sub_queries ≤ 12). Web and news score spaces are independent —
    // run both in parallel on the blocking pool so the async worker is not
    // occupied. GAP-PAR-023/035: each branch acquires its own CPU permit
    // (per-branch admit via `run_cpu_bound`) so we never hold two permits
    // idle on the async task before either spawn starts. Rayon not used: N tiny.
    let aggregation_strategy = match args.aggregation {
        AggregationStrategyKind::Rrf => AggregationStrategy::Rrf(RRF_K),
        AggregationStrategyKind::DedupeByUrl => AggregationStrategy::DedupeByUrl,
    };

    let outputs_web = std::sync::Arc::clone(&per_query_outputs);
    let outputs_news = std::sync::Arc::clone(&per_query_outputs);
    let (web_res, news_res) = tokio::join!(
        crate::concurrency::run_cpu_bound({
            let outputs = std::sync::Arc::clone(&outputs_web);
            move || aggregate(outputs.as_slice(), aggregation_strategy)
        }),
        crate::concurrency::run_cpu_bound({
            let outputs = std::sync::Arc::clone(&outputs_news);
            move || aggregate_news(outputs.as_slice(), aggregation_strategy)
        }),
    );
    let aggregated = web_res.map_err(|e| match e {
        CliError::Cancelled => CliError::Cancelled,
        other => CliError::NetworkError {
            message: format!("web aggregation task failed: {other}"),
        },
    })?;
    // GAP-WS-105: aggregate the news vertical over the SAME fan-out outputs
    // (all rounds enter the merge, mirroring the web aggregation above), in
    // its own score space. Outputs with `news: None` are skipped inside
    // `aggregate_news`, so mid-flight unavailability degrades to an empty
    // list rather than an error.
    let mut aggregated_news: Vec<AggregatedNewsItem> = news_res.map_err(|e| match e {
        CliError::Cancelled => CliError::Cancelled,
        other => CliError::NetworkError {
            message: format!("news aggregation task failed: {other}"),
        },
    })?;
    let mut aggregated = aggregated;

    // Determine the deepest cascade level observed across all sub-queries.
    let mut cascade_level = per_query_outputs
        .iter()
        .filter_map(|o| o.metadata.cascade_level_observed)
        .max()
        .map(|v| v as u8);

    // Agent contract (v0.9.8 R-02): honest chrome usage + path/channel on deep envelope.
    let mut used_chrome = per_query_outputs.iter().any(|o| o.metadata.used_chrome);
    let mut per_query_outputs = per_query_outputs;

    // GAP-E2E-48-008: reflective depth — heuristic follow-up rounds (no LLM).
    // Each round mines rare terms from top-K titles/snippets, fans out additional
    // sub-queries under the remaining max_sub_queries budget, and re-aggregates.

    if args.depth > 0 && !cancel.is_cancelled() {
        let mut seen_texts: std::collections::HashSet<String> = sub_queries
            .iter()
            .map(|q| q.text.as_str().to_ascii_lowercase())
            .collect();
        seen_texts.insert(args.query.to_ascii_lowercase());

        for round in 1..=args.depth {
            if cancel.is_cancelled() {
                break;
            }
            let remaining = args
                .max_sub_queries
                .saturating_sub(seen_texts.len().saturating_sub(1))
                .clamp(1, 4);
            let follow_ups =
                heuristic_depth_follow_ups(&args.query, &aggregated, remaining, &seen_texts);
            if follow_ups.is_empty() {
                outcomes.push(SubQueryOutcome {
                    text: format!("<reflective depth={round} no-gap-terms>"),
                    strategy: "depth".to_string(),
                    status: "ok".to_string(),
                    elapsed_ms: 0,
                    error: None,
                    news_count: None,
                    news_unavailable: None,
                });
                continue;
            }

            let follow_validated: Vec<crate::security::ValidatedQuery> = follow_ups
                .iter()
                .filter_map(|t| crate::security::ValidatedQuery::try_new(t).ok())
                .collect();
            if follow_validated.is_empty() {
                continue;
            }
            for t in &follow_validated {
                seen_texts.insert(t.as_str().to_ascii_lowercase());
            }

            let round_start = Instant::now();
            let round_outputs = execute_parallel_searches(
                follow_validated.clone(),
                cfg.clone(),
                cancel.clone(),
            )
            .await?
            .searches;

            for (q, o) in follow_validated.iter().zip(round_outputs.iter()) {
                let (news_count, news_unavailable) =
                    sub_query_news_fields(args.no_news, o.news.as_ref().map(Vec::len));
                outcomes.push(SubQueryOutcome {
                    text: q.as_str().to_string(),
                    strategy: "depth".to_string(),
                    status: if o.error.is_some() {
                        "erro".to_string()
                    } else {
                        "ok".to_string()
                    },
                    elapsed_ms: o.metadata.execution_time_ms,
                    error: o.error.clone(),
                    news_count,
                    news_unavailable,
                });
            }

            // Merge round outputs into the pool and re-aggregate (CPU-bound).
            let mut merged: Vec<SearchOutput> = per_query_outputs.as_ref().clone();
            merged.extend(round_outputs);
            per_query_outputs = std::sync::Arc::new(merged);
            let outputs_web = std::sync::Arc::clone(&per_query_outputs);
            let outputs_news = std::sync::Arc::clone(&per_query_outputs);
            let (web_res, news_res) = tokio::join!(
                crate::concurrency::run_cpu_bound({
                    let outputs = std::sync::Arc::clone(&outputs_web);
                    move || aggregate(outputs.as_slice(), aggregation_strategy)
                }),
                crate::concurrency::run_cpu_bound({
                    let outputs = std::sync::Arc::clone(&outputs_news);
                    move || aggregate_news(outputs.as_slice(), aggregation_strategy)
                }),
            );
            if let Ok(w) = web_res {
                aggregated = w;
            }
            if let Ok(n) = news_res {
                aggregated_news = n;
            }
            used_chrome = per_query_outputs.iter().any(|o| o.metadata.used_chrome);
            cascade_level = per_query_outputs
                .iter()
                .filter_map(|o| o.metadata.cascade_level_observed)
                .max()
                .map(|v| v as u8);
            let _ = round_start;
        }
    }

    // Re-run synthesis after depth rounds if requested (fresh dual report).
    let synth = if args.synthesize {
        let web = aggregated.clone();
        let news = aggregated_news.clone();
        let query = args.query.clone();
        let format = args.synth_format;
        let budget = args.budget_tokens;
        Some(
            crate::concurrency::run_cpu_bound(move || {
                synthesize_dual(&web, &news, &query, format, budget)
            })
            .await?,
        )
    } else {
        None
    };

    let (chrome_path_resolved, chrome_channel) = {
        #[cfg(feature = "chrome")]
        {
            crate::pipeline::resolved_chrome_metadata(cfg)
        }
        #[cfg(not(feature = "chrome"))]
        {
            (None, None)
        }
    };

    Ok(DeepResearchOutput {
        kind: "deep_research".to_string(),
        query: args.query.clone(),
        metadata: DeepResearchMetadata {
            original_query: args.query,
            sub_queries: outcomes,
            aggregation_strategy: match args.aggregation {
                AggregationStrategyKind::Rrf => "rrf".to_string(),
                AggregationStrategyKind::DedupeByUrl => "dedupe_by_url".to_string(),
            },
            unique_result_count: aggregated.len(),
            unique_news_count: aggregated_news.len(),
            total_elapsed_ms: start_total.elapsed().as_millis() as u64,
            cascade_level,
            used_chrome,
            chrome_path_resolved,
            chrome_channel,
        },
        results: aggregated,
        news_count: aggregated_news.len(),
        news: aggregated_news,
        synth,
    })
}

/// Minimum alphanumeric tokens a depth follow-up query must contain.
const MIN_DEPTH_SUBQUERY_TOKENS: usize = 2;
/// Minimum non-stopword content tokens required (rejects junk glue like "rust your").
const MIN_DEPTH_CONTENT_TOKENS: usize = 2;
/// Mined gap-term length bounds (inclusive).
const MIN_GAP_TERM_LEN: usize = 4;
const MAX_GAP_TERM_LEN: usize = 32;

/// Stopwords / glue tokens excluded from depth reflection mining and quality checks.
///
/// Includes high-frequency EN/PT function words that previously leaked into
/// low-quality sub-queries (e.g. parent `"rust"` + term `"your"` → `"rust your"`).
fn depth_stopwords() -> std::collections::HashSet<&'static str> {
    [
        // EN function / glue
        "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "with", "from", "by", "as",
        "is", "are", "was", "were", "be", "been", "being", "this", "that", "these", "those", "it",
        "its", "at", "if", "but", "not", "nor", "so", "than", "then", "too", "very", "just", "only",
        "also", "into", "over", "under", "again", "further", "once", "here", "there", "all", "any",
        "both", "each", "few", "more", "most", "other", "some", "such", "no", "own", "same", "can",
        "will", "shall", "may", "might", "must", "could", "would", "should", "does", "did", "doing",
        "done", "have", "has", "had", "having", "do", "about", "above", "below", "between", "through",
        "during", "before", "after", "without", "within", "against", "across", "along", "among",
        "around", "because", "while", "until", "unless", "although", "though", "whether",
        "you", "your", "yours", "yourself", "yourselves", "we", "our", "ours", "ourselves", "they",
        "them", "their", "theirs", "themselves", "he", "him", "his", "she", "her", "hers", "me",
        "my", "mine", "myself", "who", "whom", "whose", "which", "what", "how", "why", "when", "where",
        "http", "https", "www", "com", "org", "net", "html", "php", "asp",
        // PT function / glue
        "de", "da", "do", "dos", "das", "em", "um", "uma", "uns", "umas", "os", "as", "ao", "aos",
        "à", "às", "para", "pra", "com", "por", "pelo", "pela", "pelos", "pelas", "não", "nao",
        "que", "se", "ser", "estar", "ter", "foi", "são", "sao", "era", "eram", "será", "sera",
        "seu", "sua", "seus", "suas", "meu", "minha", "meus", "minhas", "nosso", "nossa", "nossos",
        "nossas", "eles", "elas", "ele", "ela", "você", "voce", "vocês", "voces", "nós", "nos",
        "lhe", "lhes", "este", "esta", "estes", "estas", "esse", "essa", "esses", "essas", "isso",
        "isto", "aquele", "aquela", "aqueles", "aquelas", "aquilo", "mais", "menos", "muito",
        "muita", "muitos", "muitas", "pouco", "pouca", "poucos", "poucas", "sobre", "entre",
        "depois", "antes", "quando", "onde", "como", "porque", "pois", "mas", "ou", "já", "ja",
        "ainda", "também", "tambem", "só", "so", "sem", "até", "ate", "após", "apos", "desde",
        "durante", "através", "atraves", "contra", "segundo", "cada", "todo", "toda", "todos",
        "todas", "outro", "outra", "outros", "outras", "mesmo", "mesma", "mesmos", "mesmas",
    ]
    .into_iter()
    .collect()
}

/// Split a query string into lowercase alphanumeric tokens (keeps `-` / `_` inside tokens).
fn tokenize_depth_query(s: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(8);
    for tok in s.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_') {
        if tok.is_empty() {
            continue;
        }
        out.push(tok.to_ascii_lowercase());
    }
    out
}

/// Non-stopword content tokens (min length 3 to keep short technical terms like "io").
fn content_tokens(tokens: &[String], stop: &std::collections::HashSet<&str>) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    for t in tokens {
        if t.len() >= 3 && !stop.contains(t.as_str()) {
            out.push(t.clone());
        }
    }
    out
}

/// Quality gate for reflective depth sub-queries (GAP-E2E-51-012).
///
/// Rejects:
/// - fewer than [`MIN_DEPTH_SUBQUERY_TOKENS`] tokens
/// - stopword-only / junk glue (fewer than [`MIN_DEPTH_CONTENT_TOKENS`] content tokens)
/// - near-duplicates of the parent query (no new content token vs parent)
/// - exact parent after case/whitespace normalization
fn is_quality_depth_subquery(parent: &str, candidate: &str) -> bool {
    let parent = parent.trim();
    let candidate = candidate.trim();
    if candidate.is_empty() || candidate.len() > 200 {
        return false;
    }
    let stop = depth_stopwords();
    let parent_tokens = tokenize_depth_query(parent);
    let cand_tokens = tokenize_depth_query(candidate);
    if cand_tokens.len() < MIN_DEPTH_SUBQUERY_TOKENS {
        return false;
    }
    let parent_content = content_tokens(&parent_tokens, &stop);
    let cand_content = content_tokens(&cand_tokens, &stop);
    if cand_content.len() < MIN_DEPTH_CONTENT_TOKENS {
        return false;
    }
    // Exact / whitespace-normalized duplicate of parent.
    if parent_tokens == cand_tokens {
        return false;
    }
    // Near-duplicate: every content token already present in the parent.
    let parent_set: std::collections::HashSet<&str> =
        parent_content.iter().map(String::as_str).collect();
    let has_new_content = cand_content.iter().any(|t| !parent_set.contains(t.as_str()));
    if !has_new_content {
        return false;
    }
    true
}

/// Build heuristic follow-up queries from rare tokens in top aggregated titles/snippets.
///
/// Applies [`is_quality_depth_subquery`] so junk glues like `"rust your"` never fan out.
fn heuristic_depth_follow_ups(
    original: &str,
    aggregated: &[AggregatedItem],
    limit: usize,
    seen: &std::collections::HashSet<String>,
) -> Vec<String> {
    use std::collections::HashMap;
    let stop = depth_stopwords();
    let parent_content: std::collections::HashSet<String> = content_tokens(
        &tokenize_depth_query(original),
        &stop,
    )
    .into_iter()
    .collect();

    let mut freq: HashMap<String, usize> = HashMap::with_capacity(64);
    for item in aggregated.iter().take(12) {
        let blob = format!(
            "{} {}",
            item.title,
            item.snippet.as_deref().unwrap_or("")
        );
        for tok in blob.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_') {
            let t = tok.to_ascii_lowercase();
            if t.len() < MIN_GAP_TERM_LEN
                || t.len() > MAX_GAP_TERM_LEN
                || stop.contains(t.as_str())
                || parent_content.contains(&t)
            {
                continue;
            }
            *freq.entry(t).or_insert(0) += 1;
        }
    }
    // Prefer uncommon-but-present terms (appear 1..=3 times) as gap fillers.
    let mut candidates: Vec<(usize, String)> = Vec::with_capacity(freq.len());
    for (t, c) in freq {
        if (1..=3).contains(&c) {
            candidates.push((c, t));
        }
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));

    let base = original.trim();
    let mut out = Vec::with_capacity(limit);
    for (_, term) in candidates {
        if out.len() >= limit {
            break;
        }
        let q = format!("{base} {term}");
        let key = q.to_ascii_lowercase();
        if seen.contains(&key) || seen.contains(&term) {
            continue;
        }
        if !is_quality_depth_subquery(base, &q) {
            continue;
        }
        out.push(q);
    }
    // Fallback: pair original with a content keyword from the top title.
    if out.is_empty() {
        if let Some(first) = aggregated.first() {
            let title_tokens = tokenize_depth_query(&first.title);
            let word = content_tokens(&title_tokens, &stop)
                .into_iter()
                .find(|w| w.len() >= MIN_GAP_TERM_LEN && !parent_content.contains(w))
                .unwrap_or_else(|| "overview".to_string());
            let q = format!("{base} {word}");
            let key = q.to_ascii_lowercase();
            if !seen.contains(&key) && is_quality_depth_subquery(base, &q) {
                out.push(q);
            }
        }
    }
    out
}

/// Maps a sub-query's news scan result to the [`SubQueryOutcome`] fields.
///
/// - `--no-news` set: both fields are `None` (news was never expected).
/// - `news_len = Some(n)`: the scan ran — report the count, even when zero.
/// - `news_len = None` without `--no-news`: the news vertical became
///   unavailable mid-flight — flag it. GAP-WS-105 v0.8.9.
fn sub_query_news_fields(no_news: bool, news_len: Option<usize>) -> (Option<usize>, Option<bool>) {
    if no_news {
        (None, None)
    } else {
        match news_len {
            Some(n) => (Some(n), None),
            None => (None, Some(true)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_output() -> DeepResearchOutput {
        DeepResearchOutput {
            kind: "deep_research".to_string(),
            query: "q".to_string(),
            metadata: DeepResearchMetadata {
                original_query: "q".to_string(),
                sub_queries: vec![SubQueryOutcome {
                    text: "q aspect".to_string(),
                    strategy: "heuristic".to_string(),
                    status: "ok".to_string(),
                    elapsed_ms: 1,
                    error: None,
                    news_count: None,
                    news_unavailable: None,
                }],
                aggregation_strategy: "rrf".to_string(),
                unique_result_count: 0,
                unique_news_count: 0,
                total_elapsed_ms: 1,
                cascade_level: None,
                used_chrome: true,
                chrome_path_resolved: Some("/usr/lib64/chromium-browser/chromium-browser".into()),
                chrome_channel: Some("host".into()),
            },
            results: Vec::new(),
            news: Vec::new(),
            news_count: 0,
            synth: None,
        }
    }

    #[test]
    fn envelope_always_serializes_news_fields() {
        let json = serde_json::to_value(empty_output()).expect("serializable");
        assert_eq!(json["noticias"], serde_json::json!([]));
        assert_eq!(json["quantidade_noticias"], 0);
        assert_eq!(json["metadados"]["total_noticias_unicas"], 0);
    }

    #[test]
    fn envelope_serializes_chrome_agent_metadata() {
        let json = serde_json::to_value(empty_output()).expect("serializable");
        assert_eq!(json["metadados"]["usou_chrome"], true);
        assert_eq!(
            json["metadados"]["chrome_path_resolvido"],
            "/usr/lib64/chromium-browser/chromium-browser"
        );
        assert_eq!(json["metadados"]["chrome_canal"], "host");
    }

    #[test]
    fn sub_query_omits_news_fields_when_none() {
        let json = serde_json::to_value(empty_output()).expect("serializable");
        let sq = &json["metadados"]["sub_queries"][0];
        assert!(sq.get("quantidade_noticias").is_none());
        assert!(sq.get("news_indisponivel").is_none());
    }

    #[test]
    fn sub_query_news_mapping_covers_all_cases() {
        assert_eq!(sub_query_news_fields(false, Some(3)), (Some(3), None));
        assert_eq!(sub_query_news_fields(false, Some(0)), (Some(0), None));
        assert_eq!(sub_query_news_fields(false, None), (None, Some(true)));
        assert_eq!(sub_query_news_fields(true, Some(3)), (None, None));
        assert_eq!(sub_query_news_fields(true, None), (None, None));
    }

    // ── GAP-E2E-51-012: depth reflection quality filter ────────────────────

    #[test]
    fn quality_filter_rejects_stopword_glue_rust_your() {
        // Exact regression from e2e: parent "rust" + stopword "your".
        assert!(!is_quality_depth_subquery("rust", "rust your"));
        assert!(!is_quality_depth_subquery("rust", "rust the"));
        assert!(!is_quality_depth_subquery("rust", "your rust"));
    }

    #[test]
    fn quality_filter_rejects_stopword_only_and_short() {
        assert!(!is_quality_depth_subquery("async rust", "the and or"));
        assert!(!is_quality_depth_subquery("async rust", "x"));
        assert!(!is_quality_depth_subquery("async rust", ""));
        assert!(!is_quality_depth_subquery("async rust", "   "));
    }

    #[test]
    fn quality_filter_rejects_near_duplicate_of_parent() {
        assert!(!is_quality_depth_subquery("rust async", "rust async"));
        assert!(!is_quality_depth_subquery("rust async", "Rust  Async"));
        // Parent content tokens only — no new gap term.
        assert!(!is_quality_depth_subquery(
            "rust async programming",
            "async rust programming"
        ));
        // Parent + only stopwords.
        assert!(!is_quality_depth_subquery(
            "rust async",
            "rust async your the"
        ));
    }

    #[test]
    fn quality_filter_accepts_contentful_follow_up() {
        assert!(is_quality_depth_subquery("rust", "rust tokio runtime"));
        assert!(is_quality_depth_subquery(
            "rust async",
            "rust async tokio"
        ));
        assert!(is_quality_depth_subquery(
            "machine learning",
            "machine learning transformers"
        ));
    }

    #[test]
    fn heuristic_depth_skips_junk_title_glue() {
        let items = vec![AggregatedItem {
            url: crate::types::HttpUrl::for_test("https://example.com/a"),
            title: "Your guide to the best of rust".to_string(),
            display_url: None,
            snippet: Some("With more about your journey".to_string()),
            score: 0.5,
            position: 1,
            sources: vec!["rust".to_string()],
        }];
        let seen = std::collections::HashSet::new();
        let out = heuristic_depth_follow_ups("rust", &items, 4, &seen);
        for q in &out {
            assert!(
                is_quality_depth_subquery("rust", q),
                "emitted low-quality follow-up: {q:?}"
            );
            assert!(
                !q.eq_ignore_ascii_case("rust your"),
                "regression: rust your must not fan out"
            );
        }
    }

    #[test]
    fn heuristic_depth_emits_content_term_when_available() {
        let items = vec![AggregatedItem {
            url: crate::types::HttpUrl::for_test("https://example.com/b"),
            title: "Tokio runtime internals for async Rust".to_string(),
            display_url: None,
            snippet: Some("Explore tokio multi-threaded scheduler".to_string()),
            score: 0.9,
            position: 1,
            sources: vec!["rust async".to_string()],
        }];
        let seen = std::collections::HashSet::new();
        let out = heuristic_depth_follow_ups("rust async", &items, 3, &seen);
        assert!(
            !out.is_empty(),
            "expected at least one quality follow-up from tokio/runtime terms"
        );
        assert!(out.iter().all(|q| is_quality_depth_subquery("rust async", q)));
        assert!(out.iter().any(|q| {
            let lower = q.to_ascii_lowercase();
            lower.contains("tokio") || lower.contains("runtime") || lower.contains("scheduler")
        }));
    }
}
