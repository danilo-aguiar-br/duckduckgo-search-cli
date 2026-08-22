// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light (string assembly, no I/O).
//! Report renderers for the Markdown, plain-text and JSON synthesis formats.

use super::SubQuerySynthStats;
use crate::aggregation::{AggregatedItem, AggregatedNewsItem};
use serde::Serialize;

/// Renders one news item as `title — source, relative_date`, omitting the
/// metadata suffix when both `fonte` and `data_relativa` are absent.
fn news_line(item: &AggregatedNewsItem) -> String {
    let meta: Vec<&str> = item
        .source
        .as_deref()
        .into_iter()
        .chain(item.relative_date.as_deref())
        .collect();
    if meta.is_empty() {
        truncate(&item.title, 120)
    } else {
        format!("{} — {}", truncate(&item.title, 120), meta.join(", "))
    }
}

pub(super) fn render_news_markdown(items: &[AggregatedNewsItem]) -> String {
    // GAP-MEM-036: reserve for heading + ~96 bytes per news line.
    let mut s = String::with_capacity(64usize.saturating_add(items.len().saturating_mul(96)));
    s.push_str(crate::i18n::Message::SynthesisRecentNewsHeading.text(crate::i18n::language()));
    for (i, item) in items.iter().enumerate() {
        s.push_str(&format!("{}. {}\n", i + 1, news_line(item)));
    }
    s
}

pub(super) fn render_news_plain(items: &[AggregatedNewsItem]) -> String {
    let mut s = String::with_capacity(64usize.saturating_add(items.len().saturating_mul(96)));
    s.push_str(crate::i18n::Message::SynthesisRecentNewsLabel.text(crate::i18n::language()));
    for (i, item) in items.iter().enumerate() {
        s.push_str(&format!("{}. {}\n", i + 1, news_line(item)));
    }
    s
}

pub(super) fn render_json_dual(
    web: &[AggregatedItem],
    news: &[AggregatedNewsItem],
    query: &str,
) -> String {
    #[derive(Serialize)]
    struct Ref<'a> {
        id: usize,
        url: &'a str,
        title: &'a str,
        score: f64,
    }
    #[derive(Serialize)]
    struct NewsRef<'a> {
        id: usize,
        url: &'a str,
        title: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        fonte: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        data_relativa: Option<&'a str>,
        score: f64,
    }
    #[derive(Serialize)]
    struct Body<'a> {
        query: &'a str,
        summary: String,
        references: Vec<Ref<'a>>,
        news: Vec<NewsRef<'a>>,
    }
    let body = Body {
        query,
        summary: format!(
            "Aggregated {} result(s) and {} news item(s) for the deep-research query.",
            web.len(),
            news.len()
        ),
        references: web
            .iter()
            .enumerate()
            .map(|(i, item)| Ref {
                id: i + 1,
                url: item.url.as_str(),
                title: &item.title,
                // I-JSON: never emit NaN/Infinity as JSON numbers.
                score: finite_score(item.score),
            })
            .collect(),
        news: news
            .iter()
            .enumerate()
            .map(|(i, item)| NewsRef {
                id: i + 1,
                url: item.url.as_str(),
                title: &item.title,
                fonte: item.source.as_deref(),
                data_relativa: item.relative_date.as_deref(),
                score: finite_score(item.score),
            })
            .collect(),
    };
    serialize_synth_json(&body, query)
}

pub(super) fn render_markdown(
    items: &[AggregatedItem],
    query: &str,
    stats: Option<SubQuerySynthStats>,
) -> String {
    let mut s = String::new();
    s.push_str(&format!("## Deep Research: {query}\n\n"));
    s.push_str("### Summary\n\n");
    if items.is_empty() {
        s.push_str("_No results were aggregated._\n");
        return s;
    }
    let sub_line = match stats {
        Some(st) if st.total > 0 => format!(
            "Aggregated {} result(s) from {}/{} successful sub-queries ({} failed). \
The top-ranked sources are summarised below (ranked SERP summary, not an LLM essay).\n\n",
            items.len(),
            st.ok,
            st.total,
            st.error
        ),
        _ => format!(
            "Aggregated {} result(s). The top-ranked sources are summarised below \
(ranked SERP summary, not an LLM essay).\n\n",
            items.len()
        ),
    };
    s.push_str(&sub_line);
    s.push_str("### Key Findings\n\n");
    for (i, item) in items.iter().enumerate() {
        let id = i + 1;
        let snippet = item.snippet.as_deref().unwrap_or("(no snippet)");
        s.push_str(&format!(
            "{}. [{}]({}) — {}\n",
            id,
            truncate(&item.title, 80),
            item.url,
            truncate(snippet, 240)
        ));
    }
    s.push_str("\n### References\n\n");
    for (i, item) in items.iter().enumerate() {
        let id = i + 1;
        s.push_str(&format!("[{}] {}\n", id, item.url));
    }
    s
}

pub(super) fn render_plain(items: &[AggregatedItem], query: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("Deep Research: {query}\n\n"));
    if items.is_empty() {
        s.push_str("No results were aggregated.\n");
        return s;
    }
    s.push_str(&format!("Top {} result(s):\n\n", items.len()));
    for (i, item) in items.iter().enumerate() {
        let id = i + 1;
        let snippet = item.snippet.as_deref().unwrap_or("(no snippet)");
        s.push_str(&format!(
            "{}. {}\n   URL: {}\n   {}\n",
            id,
            item.title,
            item.url,
            truncate(snippet, 240)
        ));
    }
    s
}

pub(super) fn render_json(items: &[AggregatedItem], query: &str) -> String {
    #[derive(Serialize)]
    struct Ref<'a> {
        id: usize,
        url: &'a str,
        title: &'a str,
        score: f64,
    }
    #[derive(Serialize)]
    struct Body<'a> {
        query: &'a str,
        summary: String,
        references: Vec<Ref<'a>>,
    }
    let body = Body {
        query,
        summary: format!(
            "Aggregated {} result(s) for the deep-research query.",
            items.len()
        ),
        references: items
            .iter()
            .enumerate()
            .map(|(i, item)| Ref {
                id: i + 1,
                url: item.url.as_str(),
                title: &item.title,
                score: finite_score(item.score),
            })
            .collect(),
    };
    serialize_synth_json(&body, query)
}

/// RFC 8259 / I-JSON: JSON numbers must be finite. RRF scores are always finite
/// in normal aggregation; clamp non-finite values defensively before serialize.
#[inline]
pub(super) fn finite_score(score: f64) -> f64 {
    if score.is_finite() {
        score
    } else {
        0.0
    }
}

/// Pretty JSON for synthesis body. On the theoretically unreachable serialize
/// failure path, emit a **valid** minimal object (never empty `{}` without fields
/// that break the consumer contract, and never panic).
fn serialize_synth_json<T: Serialize>(body: &T, query: &str) -> String {
    match serde_json::to_string_pretty(body) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                error = %e,
                "deep-research JSON synthesis serialization failed"
            );
            // `Value` Display is infallible for these simple nodes.
            serde_json::json!({
                "query": query,
                "summary": "serialization failed",
                "references": [],
                "error": e.to_string(),
            })
            .to_string()
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if crate::text::exceeds_chars(s, max) {
        format!("{}...", crate::text::truncate_to_chars(s, max))
    } else {
        s.to_string()
    }
}
