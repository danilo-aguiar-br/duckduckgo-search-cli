// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (HTML SERP extraction)
//! Web (html/lite) SERP result extraction.

use crate::endpoints::{HOST_DDG, URL_AD_TRACKER_PATH};
use crate::types::{SearchResult, SelectorConfig};
use scraper::Html;

use super::url::resolve_url;

mod selectors;
mod strategies;
mod text;

use selectors::{sel_tr, CompiledLiteSelectors, CompiledSelectors};
use strategies::{extract_strategy_2, extract_with_document};
use text::{SNIPPET_LIMIT, TITLE_LIMIT};

#[allow(unused_imports)]
pub(crate) use text::{join_text, normalize_text};

/// Extracts the organic results from a `DuckDuckGo` HTML page using Strategy 1.
///
/// Returns results already filtered (no ads), with resolved URLs and positions
/// numbered sequentially from 1.
///
/// If no results are found, returns an empty `Vec` (not an error — the query may simply
/// have no results; actual malformed-HTML errors are handled further up the call stack).
///
/// # Errors
///
/// This function is **infallible**: absence of organic results is expressed as an empty
/// `Vec` (`Option`-like absence), not [`crate::error::CliError`]. Callers that need a
/// domain failure use higher layers (`NoResults`, zero-cause classification).
pub fn extract_results(raw_html: &str) -> Vec<SearchResult> {
    let cfg = SelectorConfig::default();
    extract_results_with_cfg(raw_html, &cfg)
}

/// Same as `extract_results`, but accepts a custom `SelectorConfig`.
///
/// Iteration 6: allows selectors loaded from an external TOML file to be applied.
pub fn extract_results_with_cfg(raw_html: &str, cfg: &SelectorConfig) -> Vec<SearchResult> {
    let document = Html::parse_document(raw_html);
    let Some(compiled) = CompiledSelectors::compile(cfg) else {
        return Vec::new();
    };
    extract_with_document(&document, &compiled)
}

/// Applies Strategy 1 and, if it returns empty, applies Strategy 2 (semantic fallback).
///
/// Strategy 2 searches all `<a href="...">` links inside `#links` that point to
/// an external domain; for each one it extracts the link text as the title, unwraps
/// the href with `resolve_url`, and attempts to extract a snippet from the parent
/// element (looks for the ancestor with substantial text).
pub fn extract_results_with_strategies(raw_html: &str) -> Vec<SearchResult> {
    let cfg = SelectorConfig::default();
    extract_results_with_strategies_cfg(raw_html, &cfg)
}

/// Same as `extract_results_with_strategies`, but accepts external selectors.
pub fn extract_results_with_strategies_cfg(
    raw_html: &str,
    cfg: &SelectorConfig,
) -> Vec<SearchResult> {
    let document = Html::parse_document(raw_html);
    extract_results_with_strategies_on_document(&document, cfg)
}

/// Strategy 1→2 extraction against an already-parsed document.
///
/// Used when the caller also needs pagination tokens from the same parse
/// (latency: one `Html::parse_document` per page, not two).
pub fn extract_results_with_strategies_on_document(
    document: &Html,
    cfg: &SelectorConfig,
) -> Vec<SearchResult> {
    let mut results = match CompiledSelectors::compile(cfg) {
        Some(compiled) => extract_with_document(document, &compiled),
        None => Vec::new(),
    };
    if !results.is_empty() {
        return results;
    }

    // Hot path: demoted to debug — stripped in release via release_max_level_info
    // unless a debug build is used; avoids per-page format/subscriber cost.
    tracing::debug!("Strategy 1 returned empty — trying Strategy 2 (semantic fallback)");
    results = extract_strategy_2(document);
    if !results.is_empty() {
        tracing::debug!(total = results.len(), "Strategy 2 recovered results");
    }
    results
}

/// Strategy 3: extraction for the Lite endpoint (`https://lite.duckduckgo.com/lite/`).
///
/// Lite returns tabular HTML. We iterate over `<tr>` elements capturing pairs:
/// 1. `<tr>` with `<a class="result-link">` (or any `<a>` in `<td>`) → title/URL.
/// 2. The following `<tr>` with `td.result-snippet` (or a `<td>` with substantial text) → snippet.
pub fn extract_results_lite(raw_html: &str) -> Vec<SearchResult> {
    let cfg = SelectorConfig::default();
    extract_results_lite_with_cfg(raw_html, &cfg)
}

/// Same as `extract_results_lite`, but accepts external selectors.
pub fn extract_results_lite_with_cfg(raw_html: &str, cfg: &SelectorConfig) -> Vec<SearchResult> {
    let document = Html::parse_document(raw_html);
    let Some(compiled_lite) = CompiledLiteSelectors::compile(cfg) else {
        return Vec::new();
    };
    let sel_link = &compiled_lite.link;
    let sel_snippet_td = &compiled_lite.snippet_td;

    let mut results: Vec<SearchResult> = Vec::with_capacity(16);
    let mut position: u32 = 0;
    let mut pending_title: Option<(String, crate::types::HttpUrl)> = None;

    for tr in document.select(sel_tr()) {
        // Try the result link in the first <a> of the row (class result-link preferred).
        let link_candidate = tr.select(sel_link).next();
        if let Some(link) = link_candidate {
            let is_result_link = link
                .value()
                .attr("class")
                .map(|c| c.contains("result-link"))
                .unwrap_or(false);

            if is_result_link || pending_title.is_none() {
                if let Some(href) = link.value().attr("href") {
                    if let Some(resolved_url) = resolve_url(href) {
                        if resolved_url.as_str().contains(URL_AD_TRACKER_PATH) {
                            continue;
                        }
                        let raw_title = join_text(&link);
                        let title = normalize_text(&raw_title, TITLE_LIMIT);
                        if !title.is_empty() && !resolved_url.as_str().contains(HOST_DDG) {
                            // Flush any pending title without snippet.
                            if let Some((pending_t, pending_u)) = pending_title.take() {
                                position += 1;
                                results.push(SearchResult {
                                    position,
                                    title: pending_t,
                                    url: pending_u,
                                    display_url: None,
                                    snippet: None,
                                    original_title: None,
                                    content: None,
                                    content_size: None,
                                    content_extraction_method: None,
                                });
                            }
                            pending_title = Some((title, resolved_url));
                            continue;
                        }
                    }
                }
            }
        }

        // Snippet row: look for td.result-snippet or td with substantial text.
        if let Some((title, url)) = pending_title.take() {
            let snippet_text = tr
                .select(sel_snippet_td)
                .map(|td| join_text(&td))
                .find(|t| t.split_whitespace().count() > 5);
            let snippet = snippet_text.map(|t| normalize_text(&t, SNIPPET_LIMIT));

            position += 1;
            results.push(SearchResult {
                position,
                title,
                url,
                display_url: None,
                snippet,
                original_title: None,
                content: None,
                content_size: None,
                content_extraction_method: None,
            });
        }

        if results.len() >= 50 {
            break;
        }
    }

    // Final flush of any pending title.
    if let Some((title, url)) = pending_title {
        position += 1;
        results.push(SearchResult {
            position,
            title,
            url,
            display_url: None,
            snippet: None,
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        });
    }

    results
}
