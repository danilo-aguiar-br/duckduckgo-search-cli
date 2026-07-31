// SPDX-License-Identifier: MIT OR Apache-2.0
//! Domain types for the deep-research pipeline.

use crate::aggregation::{AggregatedItem, AggregatedNewsItem};
use crate::synthesis::{SynthFormat, SynthesizedReport};
use crate::validation;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError, ValidationErrors};

use super::{DEFAULT_MAX_SUB_QUERIES, MAX_DEPTH, MAX_SUB_QUERIES};

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
    /// Maximum number of sub-queries to produce (1..=12, default 3 in v1.0.2).
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
            // Product default ON since v0.9.8 (aligned with CLI --no-fetch-content opt-out).
            fetch_content: true,
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
    #[serde(rename = "text", alias = "texto")]
    pub text: String,
    /// Origin label — `heuristic`, `manual`, or template name.
    #[serde(rename = "strategy", alias = "estrategia")]
    pub strategy: String,
    /// Status: `ok` when results were produced, `error` otherwise (EN wire default).
    #[serde(rename = "status")]
    pub status: String,
    /// Wall-clock duration for this sub-query (milliseconds).
    #[serde(rename = "elapsed_ms", alias = "tempo_ms")]
    pub elapsed_ms: u64,
    /// Optional error message when `status == "error"`.
    #[serde(rename = "error_message", alias = "mensagem_erro", skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Number of news items returned by this sub-query's news scan. `None`
    /// when the news vertical was skipped (`--no-news`) or unavailable.
    /// GAP-WS-105 v0.8.9.
    #[serde(
        rename = "news_count",
        alias = "quantidade_noticias",
        skip_serializing_if = "Option::is_none"
    )]
    pub news_count: Option<usize>,
    /// `Some(true)` when the news scan was expected but the news vertical
    /// became unavailable mid-flight (Chrome fell and the web search degraded
    /// to HTTP). Omitted otherwise. GAP-WS-105 v0.8.9.
    #[serde(
        rename = "news_unavailable",
        alias = "news_indisponivel",
        skip_serializing_if = "Option::is_none"
    )]
    pub news_unavailable: Option<bool>,
    /// Per-sub-query zero-cause when the SERP classifier ran (CM-09).
    #[serde(
        rename = "zero_cause",
        alias = "causa_zero",
        skip_serializing_if = "Option::is_none"
    )]
    pub zero_cause: Option<crate::types::ZeroCause>,
    /// Short news failure code when `news_unavailable` is true (CM-09).
    #[serde(
        rename = "news_error",
        alias = "news_erro",
        skip_serializing_if = "Option::is_none"
    )]
    pub news_error: Option<String>,
    /// One-line agent-actionable news diagnosis, capped for tokens (CM-09).
    #[serde(
        rename = "news_diagnosis",
        alias = "news_diagnostico",
        skip_serializing_if = "Option::is_none"
    )]
    pub news_diagnosis: Option<String>,
}

/// Top-level output of the `deep-research` subcommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchOutput {
    /// Schema discriminator (always `"deep_research"`).
    #[serde(rename = "kind", alias = "tipo")]
    pub kind: String,
    /// Original user query (mirrors `SearchOutput.query` for schema parity).
    pub query: String,
    /// Run metadata (query, sub-queries, timings, etc.).
    #[serde(rename = "metadata", alias = "metadados")]
    pub metadata: DeepResearchMetadata,
    /// Aggregated evidence list (sorted by descending score).
    #[serde(rename = "results", alias = "resultados")]
    pub results: Vec<AggregatedItem>,
    /// Aggregated news list (GAP-WS-105 v0.8.9). Always serialized — empty
    /// when zero news items were found or when `--no-news` was passed.
    #[serde(rename = "news", alias = "noticias", default)]
    pub news: Vec<AggregatedNewsItem>,
    /// Number of aggregated news items. Always serialized. GAP-WS-105 v0.8.9.
    #[serde(rename = "news_count", alias = "quantidade_noticias")]
    pub news_count: usize,
    /// Optional synthesised report.
    #[serde(rename = "synthesis", alias = "sintese", skip_serializing_if = "Option::is_none")]
    pub synth: Option<SynthesizedReport>,
}

/// Run-level metadata for a deep-research execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchMetadata {
    /// Original user query.
    #[serde(rename = "original_query", alias = "query_original")]
    pub original_query: String,
    /// Sub-queries that were dispatched.
    #[serde(rename = "sub_queries")]
    pub sub_queries: Vec<SubQueryOutcome>,
    /// Aggregation strategy used.
    #[serde(rename = "aggregation_strategy", alias = "estrategia_agregacao")]
    pub aggregation_strategy: String,
    /// Number of unique results after deduplication / RRF.
    #[serde(rename = "unique_result_count", alias = "total_resultados_unicos")]
    pub unique_result_count: usize,
    /// Number of unique news items after news aggregation (parity with
    /// `total_resultados_unicos`). GAP-WS-105 v0.8.9.
    #[serde(rename = "unique_news_count", alias = "total_noticias_unicas", default)]
    pub unique_news_count: usize,
    /// End-to-end wall-clock duration (milliseconds).
    #[serde(rename = "total_time_ms", alias = "tempo_total_ms")]
    pub total_elapsed_ms: u64,
    /// Anti-bot cascade level reached during the deepest sub-query.
    #[serde(rename = "cascade_level", alias = "nivel_cascata")]
    pub cascade_level: Option<u8>,
    /// True when any sub-query used Chrome/chromiumoxide (agent contract).
    #[serde(rename = "used_chrome", alias = "usou_chrome", default)]
    pub used_chrome: bool,
    /// Resolved Chrome/Chromium binary after shell/Flatpak resolution (agent contract — agent contract field).
    #[serde(
        rename = "chrome_path_resolved",
        alias = "chrome_path_resolvido",
        skip_serializing_if = "Option::is_none"
    )]
    pub chrome_path_resolved: Option<String>,
    /// Install channel: `manual|env|host|flatpak|snap`.
    #[serde(
        rename = "chrome_channel",
        alias = "chrome_canal",
        skip_serializing_if = "Option::is_none"
    )]
    pub chrome_channel: Option<String>,
    /// Total sub-queries dispatched (agent-native partial surface).
    #[serde(rename = "sub_queries_total", default)]
    pub sub_queries_total: usize,
    /// Sub-queries that completed successfully.
    #[serde(rename = "sub_queries_ok", default)]
    pub sub_queries_ok: usize,
    /// Sub-queries that failed.
    #[serde(rename = "sub_queries_error", alias = "sub_queries_erro", default)]
    pub sub_queries_error: usize,
    /// True when any sub-query failed or status != ok (partial harvest).
    #[serde(rename = "partial", alias = "parcial", default)]
    pub partial: bool,
    /// Advisory: host Chrome process count suggests contention.
    #[serde(rename = "chrome_contention_advisory", default, skip_serializing_if = "std::ops::Not::not")]
    pub chrome_contention_advisory: bool,
}
