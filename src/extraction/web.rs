// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (HTML SERP extraction)
//! Web (html/lite) SERP result extraction.

use crate::types::{SearchResult, SelectorConfig};
use scraper::{ElementRef, Html, Selector};
use std::sync::LazyLock;

use super::url::resolve_url;

fn sel_tr() -> &'static Selector {
    static C: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("tr").expect("static CSS selector 'tr' is valid"));
    &C
}

fn sel_strategy2_links() -> &'static Selector {
    static C: LazyLock<Selector> =
        LazyLock::new(|| {
            Selector::parse("#links a[href], .result a[href]")
                .expect("static CSS selector for result links is valid")
        });
    &C
}

pub(crate) struct CompiledSelectors {
    pub result_item: Selector,
    pub ad_class: Option<Selector>,
    pub title_sel: Option<Selector>,
    pub snippet: Option<Selector>,
    pub display_url_sel: Option<Selector>,
    pub ad_classes_raw: Vec<String>,
    pub ad_attributes: Vec<(String, String)>,
    pub url_patterns: Vec<String>,
}

impl CompiledSelectors {
    pub fn compile(cfg: &SelectorConfig) -> Option<Self> {
        let result_item = match Selector::parse(&cfg.html_endpoint.result_item) {
            Ok(s) => s,
            Err(error) => {
                tracing::error!(
                    ?error,
                    selector = %cfg.html_endpoint.result_item,
                    "Result selector invalid — cannot extract"
                );
                return None;
            }
        };
        let join_ad = cfg.html_endpoint.ads_filter.ad_classes.join(", ");
        let ad_class = if join_ad.is_empty() {
            None
        } else {
            Selector::parse(&join_ad).ok()
        };
        let title_sel = Selector::parse(&cfg.html_endpoint.title_and_url).ok();
        let snippet = Selector::parse(&cfg.html_endpoint.snippet).ok();
        let display_url_sel = Selector::parse(&cfg.html_endpoint.display_url).ok();
        let ad_classes_raw = cfg
            .html_endpoint
            .ads_filter
            .ad_classes
            .iter()
            .map(|c| c.trim_start_matches('.').to_string())
            .collect();
        let ad_attributes = cfg
            .html_endpoint
            .ads_filter
            .ad_attributes
            .iter()
            .filter_map(|e| {
                let mut parts = e.splitn(2, '=');
                let key = parts.next()?.trim().to_string();
                let value = parts.next()?.trim().to_string();
                Some((key, value))
            })
            .collect();
        let url_patterns = cfg.html_endpoint.ads_filter.ad_url_patterns.clone();
        Some(Self {
            result_item,
            ad_class,
            title_sel,
            snippet,
            display_url_sel,
            ad_classes_raw,
            ad_attributes,
            url_patterns,
        })
    }
}

pub(crate) struct CompiledLiteSelectors {
    pub link: Selector,
    pub snippet_td: Selector,
}

impl CompiledLiteSelectors {
    pub fn compile(cfg: &SelectorConfig) -> Option<Self> {
        let link = Selector::parse(&cfg.lite_endpoint.result_link)
            .or_else(|_| Selector::parse("a.result-link, a"))
            .ok()?;
        let snippet_td = Selector::parse(&cfg.lite_endpoint.result_snippet)
            .or_else(|_| Selector::parse("td.result-snippet, td"))
            .ok()?;
        Some(Self { link, snippet_td })
    }
}

/// Bounded limits to prevent absurdly large payloads (section 5.4 — rule 4).
const TITLE_LIMIT: usize = 200;
const URL_LIMIT: usize = 2000;
const SNIPPET_LIMIT: usize = 500;

pub(crate) fn join_text(el: &ElementRef<'_>) -> String {
    let mut out = String::with_capacity(128);
    let mut need_space = false;
    for frag in el.text() {
        for word in frag.split_whitespace() {
            if need_space {
                out.push(' ');
            }
            out.push_str(word);
            need_space = true;
        }
    }
    out
}

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

/// Strategy 2: semantic fallback. Searches all external `<a href>` links inside
/// the results container (`#links`) and extracts title, URL and snippet.
fn extract_strategy_2(document: &Html) -> Vec<SearchResult> {
    let links_selector = sel_strategy2_links();

    let mut results = Vec::with_capacity(16);
    let mut position: u32 = 0;
    let mut seen_urls: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(16);

    for link in document.select(links_selector) {
        let href = match link.value().attr("href") {
            Some(h) if !h.is_empty() => h,
            _ => continue,
        };
        let resolved_url = match resolve_url(href) {
            Some(u) => u,
            None => continue,
        };
        if resolved_url.as_str().contains("duckduckgo.com/y.js")
            || resolved_url.as_str().len() > URL_LIMIT
        {
            continue;
        }
        // Deduplicate by URL — clone only on first sighting (not on every candidate).
        let url_key = resolved_url.as_str().to_owned();
        if seen_urls.contains(&url_key) {
            continue;
        }
        seen_urls.insert(url_key);

        let raw_title = join_text(&link);
        let title = normalize_text(&raw_title, TITLE_LIMIT);
        if title.is_empty() {
            continue;
        }

        // Look for an ancestor with substantial text to extract as snippet.
        let snippet = extract_snippet_from_ancestor(&link, &title);

        position += 1;
        results.push(SearchResult {
            position,
            title,
            url: resolved_url,
            display_url: None,
            snippet,
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        });

        // Sanity limit to avoid pages that explode the list.
        if results.len() >= 50 {
            break;
        }
    }

    results
}

/// Walks the link's ancestors looking for the first one with "substantial" text
/// (at least 40 characters distinct from the title itself).
fn extract_snippet_from_ancestor(link: &ElementRef<'_>, title: &str) -> Option<String> {
    let mut atual = link.parent();
    let mut nivel = 0;
    while let Some(no) = atual {
        nivel += 1;
        if nivel > 5 {
            break;
        }
        if let Some(el) = ElementRef::wrap(no) {
            let text = join_text(&el);
            let normalized = normalize_text(&text, SNIPPET_LIMIT);
            // Remove the title from the text to isolate the "rest" that may be a snippet.
            let without_title = normalized.replacen(title, "", 1);
            let without_title_tr = without_title.trim();
            if without_title_tr.chars().count() >= 40 {
                return Some(normalize_text(without_title_tr, SNIPPET_LIMIT));
            }
        }
        atual = no.parent();
    }
    None
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
                        if resolved_url.as_str().contains("duckduckgo.com/y.js") {
                            continue;
                        }
                        let raw_title = join_text(&link);
                        let title = normalize_text(&raw_title, TITLE_LIMIT);
                        if !title.is_empty() && !resolved_url.as_str().contains("duckduckgo.com") {
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

fn extract_with_document(document: &Html, compiled: &CompiledSelectors) -> Vec<SearchResult> {
    let mut results = Vec::with_capacity(16);
    let mut position: u32 = 0;

    for result_element in document.select(&compiled.result_item) {
        // --- Ad filter by class (descendant or element itself) ---
        if let Some(ref ad_sel) = compiled.ad_class {
            if result_element.select(ad_sel).next().is_some()
                || contains_dynamic_ad_class(&result_element, &compiled.ad_classes_raw)
            {
                tracing::trace!("Result filtered by ad class");
                continue;
            }
        }

        // --- Filter by attributes (configured key=value pairs) ---
        let mut filtered_by_attribute = false;
        for (key, value) in &compiled.ad_attributes {
            if result_element.value().attr(key.as_str()) == Some(value.as_str()) {
                tracing::trace!(attribute = %key, "Result filtered by ad attribute");
                filtered_by_attribute = true;
                break;
            }
        }
        if filtered_by_attribute {
            continue;
        }

        // --- Title + URL extraction ---
        let Some(ref title_selector) = compiled.title_sel else {
            continue;
        };
        let title_element = match result_element.select(title_selector).next() {
            Some(e) => e,
            None => {
                tracing::trace!("Result missing title element — skipping");
                continue;
            }
        };

        let raw_title = join_text(&title_element);
        let title = normalize_text(&raw_title, TITLE_LIMIT);
        if title.is_empty() {
            continue;
        }

        let raw_url = match title_element.value().attr("href") {
            Some(href) => href.to_string(),
            None => {
                tracing::trace!("Title missing href attribute — skipping");
                continue;
            }
        };
        let resolved_url = match resolve_url(&raw_url) {
            Some(u) => u,
            None => {
                tracing::trace!(url = %raw_url, "URL filtered or invalid");
                continue;
            }
        };
        // Filter by ad URL patterns (configurable).
        if compiled
            .url_patterns
            .iter()
            .any(|p| resolved_url.as_str().contains(p))
        {
            tracing::trace!(url = %resolved_url, "URL filtered by ad pattern");
            continue;
        }
        if resolved_url.as_str().len() > URL_LIMIT {
            tracing::trace!(
                size = resolved_url.as_str().len(),
                "URL exceeds limit — skipping"
            );
            continue;
        }

        // --- Snippet extraction (optional) ---
        let snippet = compiled.snippet.as_ref().and_then(|sel| {
            result_element
                .select(sel)
                .next()
                .map(|el| normalize_text(&join_text(&el), SNIPPET_LIMIT))
                .filter(|s| !s.is_empty())
        });

        // --- Display URL extraction (optional) ---
        let display_url = compiled.display_url_sel.as_ref().and_then(|sel| {
            result_element
                .select(sel)
                .next()
                .map(|el| normalize_text(&join_text(&el), URL_LIMIT))
                .filter(|s| !s.is_empty())
        });

        // --- "Official site" heuristic (v0.3.0) ---
        // DDG renders the literal "Official site" as the title for verified domains
        // (e.g., wikipedia.org, rust-lang.org). We replace it with
        // `display_url` when available and preserve the literal in
        // `original_title` for auditing.
        let (final_title, original_title) =
            apply_official_site_heuristic(title, display_url.as_deref());

        position += 1;
        results.push(SearchResult {
            position,
            title: final_title,
            url: resolved_url,
            display_url,
            snippet,
            original_title,
            content: None,
            content_size: None,
            content_extraction_method: None,
        });
    }

    tracing::debug!(
        total = results.len(),
        "Extraction complete after ad filtering"
    );
    results
}

/// Dynamic version: accepts the list of ad classes configured in the TOML file.
fn contains_dynamic_ad_class(element: &ElementRef<'_>, raw_classes: &[String]) -> bool {
    element
        .value()
        .classes()
        .any(|class| raw_classes.iter().any(|c| c == class))
}

/// Applies the "Official site" replacement heuristic (v0.3.0).
///
/// `DuckDuckGo` renders the literal text `"Official site"` (case-insensitive)
/// as the title when the result's domain is verified (e.g. rust-lang.org,
/// wikipedia.org). That title is not useful for API consumers — we replace it
/// with `url_exibicao` and preserve the literal in `original_title` for auditing.
///
/// Returns `(final_title, original_title)`:
/// - If the title matches exactly "Official site" (case-insensitive) AND a non-empty
///   `url_exibicao` exists, returns `(url_exibicao, Some("Official site"))`.
/// - Otherwise returns `(title, None)` unchanged.
fn apply_official_site_heuristic(
    title: String,
    display_url: Option<&str>,
) -> (String, Option<String>) {
    if title.eq_ignore_ascii_case("Official site") {
        if let Some(friendly_url) = display_url.map(str::trim).filter(|s| !s.is_empty()) {
            return (friendly_url.to_string(), Some(title));
        }
    }
    (title, None)
}

/// Normalises extracted text: collapses whitespace, trims and truncates at `limit` characters
/// respecting UTF-8 character boundaries.
pub(crate) fn normalize_text(raw: &str, limit: usize) -> String {
    let mut result_buf = String::with_capacity(raw.len().min(limit + 64));
    let mut needs_space = false;
    let mut chars_written: usize = 0;

    for word in raw.split_whitespace() {
        let separator = usize::from(needs_space);
        let word_len = word.chars().count();

        if chars_written + separator + word_len > limit {
            let remaining = limit.saturating_sub(chars_written + separator);
            if remaining > 0 {
                if needs_space {
                    result_buf.push(' ');
                }
                for ch in word.chars().take(remaining) {
                    result_buf.push(ch);
                }
            }
            break;
        }

        if needs_space {
            result_buf.push(' ');
            chars_written += 1;
        }
        result_buf.push_str(word);
        chars_written += word_len;
        needs_space = true;
    }

    result_buf
}

