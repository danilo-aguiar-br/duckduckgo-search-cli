// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (HTML SERP extraction strategies)
//! Strategy 1 and Strategy 2 extraction routines over a parsed document.

use crate::endpoints::URL_AD_TRACKER_PATH;
use crate::types::SearchResult;
use scraper::{ElementRef, Html};

use super::selectors::{sel_strategy2_links, CompiledSelectors};
use super::text::{
    contains_dynamic_ad_class, join_text, normalize_text, SNIPPET_LIMIT, TITLE_LIMIT, URL_LIMIT,
};
use crate::extraction::url::resolve_url;

/// Strategy 2: semantic fallback. Searches all external `<a href>` links inside
/// the results container (`#links`) and extracts title, URL and snippet.
pub(super) fn extract_strategy_2(document: &Html) -> Vec<SearchResult> {
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
        if resolved_url.as_str().contains(URL_AD_TRACKER_PATH)
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

pub(super) fn extract_with_document(
    document: &Html,
    compiled: &CompiledSelectors,
) -> Vec<SearchResult> {
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
