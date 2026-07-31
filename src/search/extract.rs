// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (HTML parse / CSS select)
//! Pagination-token and SERP result extraction helpers.
//!
//! Prefer single-parse paths ([`extract_results_and_pagination_tokens`]) when both
//! organic results and `vqd`/`s`/`dc` tokens are needed — a second
//! `Html::parse_document` is pure latency waste (`scraper::Html` is `!Send`,
//! so tokens must be extracted in the same step before any `.await`).

use crate::extraction;
use crate::types::SearchResult;

/// Extracts `vqd`, `s` and `dc` from the first page HTML (for pagination).
/// Returns `None` if any of the three fields is missing.
///
/// Prefer [`extract_results_and_pagination_tokens`] when results are also needed
/// from the same HTML — a second `Html::parse_document` is pure latency waste
/// (`scraper::Html` is `!Send`, so tokens must be extracted in the same step
/// before any `.await`, not held across async boundaries).
pub fn extract_pagination_tokens(html: &str) -> Option<(String, String, String)> {
    use scraper::Html;
    let doc = Html::parse_document(html);
    extract_pagination_tokens_from_doc(&doc)
}

/// Pagination tokens from an already-parsed document (zero extra parse).
fn extract_pagination_tokens_from_doc(doc: &scraper::Html) -> Option<(String, String, String)> {
    let vqd = doc
        .select(sel_vqd())
        .next()
        .and_then(|el| el.value().attr("value"))
        .map(|v| v.to_string())?;
    let s = doc
        .select(sel_s_input())
        .next()
        .and_then(|el| el.value().attr("value"))
        .map(|v| v.to_string())?;
    let dc = doc
        .select(sel_dc())
        .next()
        .and_then(|el| el.value().attr("value"))
        .map(|v| v.to_string())?;

    Some((vqd, s, dc))
}

/// One `Html::parse_document` for SERP results (strategy 1→2) **and** pagination
/// tokens. Callers that need both must use this instead of separate parses.
pub(crate) fn extract_results_and_pagination_tokens(
    html: &str,
    cfg: &crate::types::SelectorConfig,
) -> (Vec<SearchResult>, Option<(String, String, String)>) {
    use scraper::Html;
    let doc = Html::parse_document(html);
    let results = extraction::extract_results_with_strategies_on_document(&doc, cfg);
    let tokens = extract_pagination_tokens_from_doc(&doc);
    (results, tokens)
}

/// GAP-PAR-030: results + pagination tokens in one blocking parse (no double
/// `Html::parse_document`, no `Html` across `.await`).
pub(crate) async fn extract_results_and_pagination_tokens_async(
    html: String,
    cfg: crate::types::SelectorConfig,
) -> Result<(Vec<SearchResult>, Option<(String, String, String)>), crate::error::CliError> {
    crate::concurrency::run_cpu_bound(move || extract_results_and_pagination_tokens(&html, &cfg))
        .await
}

fn sel_vqd() -> &'static scraper::Selector {
    use std::sync::LazyLock;
    static C: LazyLock<scraper::Selector> = LazyLock::new(|| {
        scraper::Selector::parse("input[name='vqd']")
            .expect("static CSS selector 'input[name=vqd]' must parse")
    });
    &C
}

fn sel_s_input() -> &'static scraper::Selector {
    use std::sync::LazyLock;
    static C: LazyLock<scraper::Selector> = LazyLock::new(|| {
        scraper::Selector::parse("input[name='s']")
            .expect("static CSS selector 'input[name=s]' must parse")
    });
    &C
}

fn sel_dc() -> &'static scraper::Selector {
    use std::sync::LazyLock;
    static C: LazyLock<scraper::Selector> = LazyLock::new(|| {
        scraper::Selector::parse("input[name='dc']")
            .expect("static CSS selector 'input[name=dc]' must parse")
    });
    &C
}
