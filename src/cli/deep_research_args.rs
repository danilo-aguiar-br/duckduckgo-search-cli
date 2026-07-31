// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (clap derive only — zero runtime I/O).
//! Clap arguments for the `deep-research` subcommand (CM-15 / GAP-E2E-51-011).
//!
//! Extracted from the root CLI module for single responsibility. Re-exported
//! via [`crate::cli`] so existing call sites stay stable.

use clap::{builder::ValueHint, ArgAction, Args, ValueEnum};
use std::path::PathBuf;

use super::{
    DEFAULT_BUDGET_TOKENS, HEADING_CONTENT, HEADING_DIAGNOSTICS, HEADING_NETWORK, HEADING_OUTPUT,
};

/// Arguments for the `deep-research` subcommand (v0.7.0).
#[derive(Debug, Clone, Args)]
pub struct DeepResearchArgs {
    /// The original user query to research.
    ///
    /// Optional when `--print-budget` is set (agent discovery / dry estimate
    /// without inventing a placeholder query — GAP-PRINT-BUDGET-QUERY).
    #[arg(
        value_name = "QUERY",
        default_value = "",
        required_unless_present = "print_budget"
    )]
    pub query: String,

    /// Maximum number of sub-queries to produce by decomposition (1..=12, default 3 in v1.0.2).
    #[arg(
        long = "max-sub-queries",
        value_name = "N",
        default_value_t = crate::deep_research::DEFAULT_MAX_SUB_QUERIES
    )]
    pub max_sub_queries: usize,

    /// Decomposition strategy: `heuristic` (5 templates, default) or `manual`.
    #[arg(
        long = "sub-query-strategy",
        value_enum,
        default_value_t = CliSubQueryStrategy::Heuristic
    )]
    pub sub_query_strategy: CliSubQueryStrategy,

    /// File with one sub-query per line (only used with `--sub-query-strategy manual`).
    #[arg(
        long = "sub-queries-file",
        value_name = "PATH",
        value_hint = ValueHint::FilePath
    )]
    pub sub_queries_file: Option<PathBuf>,

    /// Aggregation strategy: `rrf` (default, K=60) or `dedupe-by-url`.
    #[arg(
        long = "aggregate",
        value_enum,
        default_value_t = CliAggregationStrategy::Rrf
    )]
    pub aggregation: CliAggregationStrategy,

    /// Reflection depth (0..=3). 0 = single pass. Each round runs heuristic
    /// follow-up sub-queries from top titles/snippets and re-aggregates (v1.0.1).
    #[arg(long = "depth", value_name = "N", default_value_t = 0)]
    pub depth: u32,

    /// Affirms content extraction (default ON since v0.9.8; kept for scripts).
    #[arg(long = "fetch-content", action = ArgAction::SetTrue, help_heading = HEADING_CONTENT)]
    pub fetch_content: bool,

    /// Disables content extraction for deep-research (opt-out of v0.9.8 default).
    #[arg(
        long = "no-fetch-content",
        action = ArgAction::SetTrue,
        conflicts_with = "fetch_content",
        help_heading = HEADING_CONTENT
    )]
    pub no_fetch_content: bool,

    /// Produces a synthesised report at the end of the pipeline.
    #[arg(long = "synthesize", action = ArgAction::SetTrue, help_heading = HEADING_OUTPUT)]
    pub synthesize: bool,

    /// Approximate token budget for the synthesised report (default 4000).
    /// 1 token ≈ 4 characters (English text heuristic).
    #[arg(
        long = "budget-tokens",
        value_name = "N",
        default_value_t = DEFAULT_BUDGET_TOKENS as usize
    )]
    pub budget_tokens: usize,

    /// Format of the synthesised report.
    #[arg(
        long = "synth-format",
        value_enum,
        default_value_t = CliSynthFormat::Markdown
    )]
    pub synth_format: CliSynthFormat,

    /// Fail with a non-zero exit code when the fan-out aggregates zero
    /// results. Default `false` preserves v0.7.0–v0.7.9 behavior (exit 0
    /// even with an empty payload). v0.7.10 GAP-WS-1114.
    #[arg(long = "require-results", action = ArgAction::SetTrue, help_heading = HEADING_DIAGNOSTICS)]
    pub require_results: bool,

    /// Skip the news vertical in deep-research (web-only fan-out).
    /// Default is dual web+news (`all`) per sub-query (GAP-WS-105).
    #[arg(long = "no-news", action = ArgAction::SetTrue, help_heading = HEADING_NETWORK)]
    pub no_news: bool,

    /// Permit deep-research when `--global-timeout` is below the gated workload
    /// estimate (v1.0.2 / GAP-AUD-DR-001). Default is fail-fast exit 2.
    /// Also settable via XDG `deep_research_allow_under_budget`.
    #[arg(
        long = "allow-under-budget",
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub allow_under_budget: bool,

    /// Print gated budget estimate as JSON and exit 0 without launching Chrome
    /// (agent-first dry estimate; v1.0.2).
    #[arg(
        long = "print-budget",
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub print_budget: bool,

    /// Auto-raise `--global-timeout` to suggested wall under Chrome contention
    /// (CLI-AUTO-01; default ON). Also XDG `deep_research_auto_contention_budget`.
    #[arg(
        long = "auto-contention-budget",
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub auto_contention_budget: bool,

    /// Disable auto-raise of global timeout under contention.
    #[arg(
        long = "no-auto-contention-budget",
        action = ArgAction::SetTrue,
        conflicts_with = "auto_contention_budget",
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub no_auto_contention_budget: bool,

    /// Exit non-zero when any sub-query fails even if partial hits exist
    /// (GAP-DEEP-REQUIRE-ALL-SUBQUERIES / agent strict mode).
    #[arg(
        long = "require-all-sub-queries",
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub require_all_sub_queries: bool,

    // ── Shared search knobs (GAP-E2E-V11-HELP-POLLUTION / V13) ──────────────
    // Declared HERE (not as Root global) so doctor/locale help stay clean.
    // Clap allows the same long names on parent `CliArgs` (bare/buscar) and on
    // this subcommand; after-subcommand flags bind to these fields.

    /// Max results per sub-query (same contract as root `-n` / `--num`).
    #[arg(short = 'n', long = "num", value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
    pub num_results: Option<u32>,

    /// Output file (same contract as root `-o` / `--output`).
    #[arg(
        short = 'o',
        long = "output",
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        help_heading = HEADING_OUTPUT
    )]
    pub output_file: Option<PathBuf>,

    /// Manual Chrome/Chromium path (same contract as root `--chrome-path`).
    #[arg(
        long = "chrome-path",
        value_name = "PATH",
        value_hint = ValueHint::ExecutablePath,
        help_heading = HEADING_NETWORK
    )]
    pub chrome_path: Option<PathBuf>,

    /// Project result fields (agent-native; same as root `--fields`).
    #[arg(long = "fields", value_name = "LIST", help_heading = HEADING_OUTPUT)]
    pub fields: Option<String>,

    /// Alias of `--fields`.
    #[arg(
        long = "select",
        value_name = "LIST",
        conflicts_with = "fields",
        help_heading = HEADING_OUTPUT
    )]
    pub select: Option<String>,

    /// Filter result rows (agent-native; same as root `--filter`).
    #[arg(long = "filter", value_name = "EXPR", help_heading = HEADING_OUTPUT)]
    pub result_filter: Option<String>,

    /// Cap aggregated rows after filter (agent-native; same as root `--limit`).
    #[arg(
        long = "limit",
        value_name = "N",
        value_parser = clap::value_parser!(u32).range(1..),
        help_heading = HEADING_OUTPUT
    )]
    pub result_limit: Option<u32>,

    /// Sort aggregated rows (same as root `--sort`).
    #[arg(long = "sort", value_name = "KEY[:DIR]", help_heading = HEADING_OUTPUT)]
    pub sort: Option<String>,

    /// Dedupe by URL (same as root `--dedupe-by`).
    #[arg(long = "dedupe-by", value_name = "FIELD", help_heading = HEADING_OUTPUT)]
    pub dedupe_by: Option<String>,

    /// Count-only compact JSON (same as root `--count-only`).
    #[arg(long = "count-only", action = ArgAction::SetTrue, help_heading = HEADING_OUTPUT)]
    pub count_only: bool,

    /// Truncate content (same as root `--truncate-content`).
    #[arg(
        long = "truncate-content",
        value_name = "N",
        value_parser = clap::value_parser!(u32).range(1..),
        help_heading = HEADING_OUTPUT
    )]
    pub truncate_content: Option<u32>,

    /// Max stdout bytes (same as root `--max-output-bytes`).
    #[arg(
        long = "max-output-bytes",
        value_name = "N",
        value_parser = clap::value_parser!(u64).range(1..),
        help_heading = HEADING_OUTPUT
    )]
    pub max_output_bytes: Option<u64>,

    /// Proxy URL (same as root `--proxy`; no env inheritance).
    #[arg(
        long = "proxy",
        value_name = "URL",
        value_hint = ValueHint::Url,
        conflicts_with = "no_proxy",
        help_heading = HEADING_NETWORK
    )]
    pub proxy: Option<String>,

    /// Explicit no-proxy (same as root `--no-proxy`).
    #[arg(
        long = "no-proxy",
        action = ArgAction::SetTrue,
        conflicts_with = "proxy",
        help_heading = HEADING_NETWORK
    )]
    pub no_proxy: bool,
}

/// CLI wrapper for the decomposition strategy enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliSubQueryStrategy {
    /// Heuristic fan-out using 5 canonical templates (default).
    Heuristic,
    /// Read sub-queries from a file or stdin.
    Manual,
}

/// CLI wrapper for the aggregation strategy enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliAggregationStrategy {
    /// Reciprocal Rank Fusion with K=60 (default).
    Rrf,
    /// Canonical-URL deduplication, keep first occurrence.
    DedupeByUrl,
}

/// CLI wrapper for the synthesis format enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliSynthFormat {
    /// Markdown with H2/H3 headings and `[n](url)` links.
    Markdown,
    /// Linear numbered list without markup.
    PlainText,
    /// Structured JSON tree.
    Json,
}

impl From<CliSubQueryStrategy> for crate::deep_research::SubQueryStrategy {
    fn from(value: CliSubQueryStrategy) -> Self {
        match value {
            CliSubQueryStrategy::Heuristic => Self::Heuristic,
            CliSubQueryStrategy::Manual => Self::Manual,
        }
    }
}

impl From<CliAggregationStrategy> for crate::deep_research::AggregationStrategyKind {
    fn from(value: CliAggregationStrategy) -> Self {
        match value {
            CliAggregationStrategy::Rrf => Self::Rrf,
            CliAggregationStrategy::DedupeByUrl => Self::DedupeByUrl,
        }
    }
}

impl From<CliSynthFormat> for crate::synthesis::SynthFormat {
    fn from(value: CliSynthFormat) -> Self {
        match value {
            CliSynthFormat::Markdown => Self::Markdown,
            CliSynthFormat::PlainText => Self::PlainText,
            CliSynthFormat::Json => Self::Json,
        }
    }
}

/// Overlay deep-subcommand shared knobs onto root/`buscar` defaults (V13).
///
/// After-subcommand flags bind to [`DeepResearchArgs`]; pre-subcommand flags
/// still bind to root `CliArgs`. Deep wins when set.
#[must_use]
pub fn merge_deep_search_defaults(
    base: &super::CliArgs,
    deep: &DeepResearchArgs,
) -> super::CliArgs {
    let mut out = base.clone();
    if let Some(n) = deep.num_results {
        out.num_results = Some(n);
    }
    if let Some(ref path) = deep.output_file {
        out.output_file = Some(path.clone());
    }
    if let Some(ref path) = deep.chrome_path {
        out.chrome_path = Some(path.clone());
    }
    if let Some(ref fields) = deep.fields {
        out.fields = Some(fields.clone());
    }
    if let Some(ref select) = deep.select {
        out.select = Some(select.clone());
    }
    if let Some(ref filter) = deep.result_filter {
        out.result_filter = Some(filter.clone());
    }
    if let Some(n) = deep.result_limit {
        out.result_limit = Some(n);
    }
    if let Some(ref sort) = deep.sort {
        out.sort = Some(sort.clone());
    }
    if let Some(ref d) = deep.dedupe_by {
        out.dedupe_by = Some(d.clone());
    }
    if deep.count_only {
        out.count_only = true;
    }
    if let Some(n) = deep.truncate_content {
        out.truncate_content = Some(n);
    }
    if let Some(n) = deep.max_output_bytes {
        out.max_output_bytes = Some(n);
    }
    if let Some(ref proxy) = deep.proxy {
        out.proxy = Some(proxy.clone());
    }
    if deep.no_proxy {
        out.no_proxy = true;
    }
    out
}

impl DeepResearchArgs {
    /// Map clap args to the domain [`crate::deep_research::DeepResearchArgs`] (CM-15b DRY).
    ///
    /// Content fetch is ON unless `--no-fetch-content` (product default since v0.9.8).
    #[must_use]
    pub fn into_domain(self) -> crate::deep_research::DeepResearchArgs {
        crate::deep_research::DeepResearchArgs {
            query: self.query,
            max_sub_queries: self.max_sub_queries,
            sub_query_strategy: self.sub_query_strategy.into(),
            sub_queries_file: self.sub_queries_file,
            aggregation: self.aggregation.into(),
            depth: self.depth,
            fetch_content: !self.no_fetch_content,
            synthesize: self.synthesize,
            budget_tokens: self.budget_tokens,
            synth_format: self.synth_format.into(),
            no_news: self.no_news,
        }
    }
}
