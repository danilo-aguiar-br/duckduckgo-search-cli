// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light (string assembly, no I/O).
// Parallelism: sync APIs for tests; deep-research stage 4 calls via
// `concurrency::run_cpu_bound` (GAP-PAR-034) so the async worker is free.
//! Heuristic synthesis of an aggregated result list into a single report.
//!
//! Given a list of [`AggregatedItem`]s sorted by descending score, this module
//! produces a self-contained report with numbered references. Three formats
//! are supported:
//!
//! - [`SynthFormat::Markdown`] — `##`/`###` headings and `[n](url)` links.
//! - [`SynthFormat::PlainText`] — linear numbered list without markup.
//! - [`SynthFormat::Json`] — structured tree: `{ "summary": "...", "references":
//!   [{ "id": n, "url": "...", "title": "..." }] }`.
//!
//! # Token budget
//!
//! We approximate one token as four characters (the de-facto industry
//! heuristic for English text). The budget is enforced on the summary body
//! only — references are always included in full because they are
//! non-negotiable for LLM grounding.

mod budget;
mod render;

use crate::aggregation::{AggregatedItem, AggregatedNewsItem};
use serde::{Deserialize, Serialize};

use render::{
    render_json, render_json_dual, render_markdown, render_news_markdown, render_news_plain,
    render_plain,
};

pub use budget::{estimate_tokens, trim_to_budget};

/// Output format of the synthesis stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynthFormat {
    /// `## Heading` and `[n](url)` links.
    Markdown,
    /// Linear numbered list, no markup.
    PlainText,
    /// Structured JSON tree.
    Json,
}

/// Synthesised report returned by the deep-research pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SynthesizedReport {
    /// Format used to render the report.
    #[serde(rename = "format", alias = "formato")]
    pub format: SynthFormat,
    /// The report body (`Markdown`, `PlainText`, or `JSON`).
    #[serde(rename = "body", alias = "corpo")]
    pub body: String,
    /// Approximate token count of the report body (4 chars ≈ 1 token).
    #[serde(rename = "estimated_tokens", alias = "tokens_estimados")]
    pub estimated_tokens: usize,
    /// Number of references cited in the report.
    #[serde(rename = "reference_count", alias = "quantidade_referencias")]
    pub reference_count: usize,
}

/// Honest sub-query counters for synthesis prose (CLI-SYNTH-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubQuerySynthStats {
    /// Total sub-queries dispatched.
    pub total: usize,
    /// Sub-queries that completed OK.
    pub ok: usize,
    /// Sub-queries that failed / errored.
    pub error: usize,
}

/// Combines the top-K aggregated items into a synthesised report.
pub fn synthesize(
    items: &[AggregatedItem],
    original_query: &str,
    format: SynthFormat,
    budget_tokens: usize,
) -> SynthesizedReport {
    synthesize_with_stats(items, original_query, format, budget_tokens, None)
}

/// Like [`synthesize`] with explicit sub-query success counts (CLI-SYNTH-01).
pub fn synthesize_with_stats(
    items: &[AggregatedItem],
    original_query: &str,
    format: SynthFormat,
    budget_tokens: usize,
    stats: Option<SubQuerySynthStats>,
) -> SynthesizedReport {
    // Heuristic cap: never synthesise more than 20 references per report.
    let top: &[AggregatedItem] = if items.len() > 20 {
        &items[..20]
    } else {
        items
    };

    let body = match format {
        SynthFormat::Markdown => render_markdown(top, original_query, stats),
        SynthFormat::PlainText => render_plain(top, original_query),
        SynthFormat::Json => render_json(top, original_query),
    };
    let trimmed = trim_to_budget(&body, budget_tokens);
    SynthesizedReport {
        format,
        estimated_tokens: estimate_tokens(&trimmed),
        reference_count: top.len(),
        body: trimmed,
    }
}

/// Combines web and news aggregates into a single dual-section report.
///
/// With an empty `news` list this delegates to [`synthesize`], so the
/// web-only output is identical to the historical format. With news present,
/// the web section keeps the current format under ~70% of the token budget
/// and a localized "Recent news" section consumes the remaining ~30%. In the
/// [`SynthFormat::Json`] format the news enter the JSON object as a `news`
/// array instead of a text section. `reference_count` sums the web and news
/// references actually rendered (each side capped at 20). GAP-WS-105 v0.8.9.
pub fn synthesize_dual(
    web: &[AggregatedItem],
    news: &[AggregatedNewsItem],
    original_query: &str,
    format: SynthFormat,
    budget_tokens: usize,
) -> SynthesizedReport {
    if news.is_empty() {
        return synthesize_with_stats(web, original_query, format, budget_tokens, None);
    }
    let top_web: &[AggregatedItem] = if web.len() > 20 { &web[..20] } else { web };
    let top_news: &[AggregatedNewsItem] = if news.len() > 20 { &news[..20] } else { news };

    // ~70% of the budget for the web section, ~30% for the news section.
    let web_budget = budget_tokens.saturating_mul(7) / 10;
    let news_budget = budget_tokens.saturating_sub(web_budget);

    let body = match format {
        SynthFormat::Markdown => {
            let web_body =
                trim_to_budget(&render_markdown(top_web, original_query, None), web_budget);
            let news_body = trim_to_budget(&render_news_markdown(top_news), news_budget);
            format!("{web_body}\n{news_body}")
        }
        SynthFormat::PlainText => {
            let web_body = trim_to_budget(&render_plain(top_web, original_query), web_budget);
            let news_body = trim_to_budget(&render_news_plain(top_news), news_budget);
            format!("{web_body}\n{news_body}")
        }
        SynthFormat::Json => render_json_dual(top_web, top_news, original_query),
    };
    // Final guard: trimming at `budget_tokens - 1` bounds the body (including
    // the ` ...` suffix) to `budget_tokens * 4` chars, so `estimated_tokens`
    // never exceeds the budget.
    let trimmed = trim_to_budget(&body, budget_tokens.saturating_sub(1));
    SynthesizedReport {
        format,
        estimated_tokens: estimate_tokens(&trimmed),
        reference_count: top_web.len() + top_news.len(),
        body: trimmed,
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
