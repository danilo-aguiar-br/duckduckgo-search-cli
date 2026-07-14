// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (HTML parsing and text extraction via scraper)
//! Extraction of search results from `DuckDuckGo` HTML.
//!
//! In the MVP implements ONLY Strategy 1 (stable class selectors):
//! - Container: `#links`.
//! - Items: `.result` (multiple alternative selectors).
//! - Title + URL: `.result__a`.
//! - Snippet: `.result__snippet`.
//! - Display URL: `.result__url`.
//!
//! Ad filtering:
//! - Removes elements with class `.result--ad` or `.badge--ad`.
//! - Removes elements with attribute `data-nrn="ad"`.
//! - Removes results whose URL contains `duckduckgo.com/y.js`.
//!
//! URL resolution:
//! - Protocol-relative URLs (`//example.com`) are prefixed with `https:`.
//! - URLs containing a `DuckDuckGo` internal redirect (`/l/?uddg=...&rut=...`) are
//!   unwrapped via URL-decoding of the `uddg` parameter.
//! - URLs on the `duckduckgo.com` domain itself are filtered out.

use crate::types::{NewsResult, NewsSelectors, SearchResult, SelectorConfig};
use scraper::{ElementRef, Html, Selector};
use std::sync::OnceLock;

fn sel_tr() -> &'static Selector {
    static C: OnceLock<Selector> = OnceLock::new();
    C.get_or_init(|| Selector::parse("tr").unwrap())
}

fn sel_strategy2_links() -> &'static Selector {
    static C: OnceLock<Selector> = OnceLock::new();
    C.get_or_init(|| Selector::parse("#links a[href], .result a[href]").unwrap())
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
        let url_patterns = cfg.html_endpoint.ads_filter.ad_url_patterns.to_vec();
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

fn join_text(el: &ElementRef<'_>) -> String {
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
    let mut results = match CompiledSelectors::compile(cfg) {
        Some(compiled) => extract_with_document(&document, &compiled),
        None => Vec::new(),
    };
    if !results.is_empty() {
        return results;
    }

    tracing::info!("Strategy 1 returned empty — trying Strategy 2 (semantic fallback)");
    results = extract_strategy_2(&document);
    if !results.is_empty() {
        tracing::info!(total = results.len(), "Strategy 2 recovered results");
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
        if resolved_url.contains("duckduckgo.com/y.js") || resolved_url.len() > URL_LIMIT {
            continue;
        }
        // Deduplicate by URL.
        if !seen_urls.insert(resolved_url.clone()) {
            continue;
        }

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
    let mut pending_title: Option<(String, String)> = None;

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
                        if resolved_url.contains("duckduckgo.com/y.js") {
                            continue;
                        }
                        let raw_title = join_text(&link);
                        let title = normalize_text(&raw_title, TITLE_LIMIT);
                        if !title.is_empty() && !resolved_url.contains("duckduckgo.com") {
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
                || contem_classe_anuncio_dinamico(&result_element, &compiled.ad_classes_raw)
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
            .any(|p| resolved_url.contains(p))
        {
            tracing::trace!(url = %resolved_url, "URL filtered by ad pattern");
            continue;
        }
        if resolved_url.len() > URL_LIMIT {
            tracing::trace!(size = resolved_url.len(), "URL exceeds limit — skipping");
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

    tracing::info!(
        total = results.len(),
        "Extraction complete after ad filtering"
    );
    results
}

/// Dynamic version: accepts the list of ad classes configured in the TOML file.
fn contem_classe_anuncio_dinamico(element: &ElementRef<'_>, raw_classes: &[String]) -> bool {
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
fn normalize_text(raw: &str, limit: usize) -> String {
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

/// Resolves a URL found in the `DuckDuckGo` DOM to the final URL.
///
/// Handled cases:
/// 1. `//example.com/path` → `https://example.com/path` (protocol-relative).
/// 2. `/l/?uddg=<REAL_URL>&rut=...` → decodes `uddg` and returns the real URL.
/// 3. `//duckduckgo.com/l/?uddg=...` → same logic after normalisation.
/// 4. Absolute external URLs are returned as-is.
/// 5. URLs on the `duckduckgo.com` domain itself (except `/l/?uddg=`) are filtered.
///
/// Returns `None` if the URL is invalid or belongs to `DuckDuckGo`.
pub fn resolve_url(href: &str) -> Option<String> {
    let href_trim = href.trim();
    if href_trim.is_empty() {
        return None;
    }

    // Case 1: protocol-relative.
    let normalized = if let Some(rest) = href_trim.strip_prefix("//") {
        format!("https://{rest}")
    } else if href_trim.starts_with('/') {
        // Case 2: relative DuckDuckGo path (e.g., "/l/?uddg=...").
        format!("https://duckduckgo.com{href_trim}")
    } else {
        href_trim.to_string()
    };

    // Case 3: DuckDuckGo redirect with `uddg` parameter.
    if let Some(uddg_decoded) = extract_uddg(&normalized) {
        return Some(uddg_decoded);
    }

    // Case 4: filter URLs from DuckDuckGo itself (without uddg).
    if eh_url_duckduckgo(&normalized) {
        return None;
    }

    Some(normalized)
}

/// If the URL is a `DuckDuckGo` redirect (`/l/?uddg=<REAL_URL>`), extracts and
/// URL-decodes `uddg`. Returns `None` if it is not a redirect or the parameter is absent.
fn extract_uddg(url: &str) -> Option<String> {
    // Search for "uddg=" in the query string.
    let idx_uddg = url.find("uddg=")?;
    let after_equals = &url[idx_uddg + "uddg=".len()..];
    // The uddg value extends to the next `&` or end of string.
    let encoded_value = match after_equals.find('&') {
        Some(end) => &after_equals[..end],
        None => after_equals,
    };
    urlencoding::decode(encoded_value)
        .ok()
        .map(|cow| cow.into_owned())
}

/// Checks whether the URL points to any subdomain of `DuckDuckGo`.
fn eh_url_duckduckgo(url: &str) -> bool {
    let after_proto = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };
    let host = after_proto
        .split('/')
        .next()
        .unwrap_or(after_proto)
        .split('?')
        .next()
        .unwrap_or(after_proto);
    host.eq_ignore_ascii_case("duckduckgo.com")
        || host.eq_ignore_ascii_case("html.duckduckgo.com")
        || host.eq_ignore_ascii_case("lite.duckduckgo.com")
        || (host.len() > ".duckduckgo.com".len()
            && host[host.len() - ".duckduckgo.com".len()..].eq_ignore_ascii_case(".duckduckgo.com"))
}

// =============================================================================
// News vertical extraction (GAP-WS-104 v0.8.9)
// =============================================================================

/// Bounded limit for the publisher/source and relative-date fields.
const SOURCE_LIMIT: usize = 120;

fn sel_news_anchors() -> &'static Selector {
    static C: OnceLock<Selector> = OnceLock::new();
    C.get_or_init(|| Selector::parse("a[href]").unwrap())
}

fn sel_news_meta_fallback() -> &'static Selector {
    static C: OnceLock<Selector> = OnceLock::new();
    C.get_or_init(|| Selector::parse("span, time").unwrap())
}

fn sel_news_img_fallback() -> &'static Selector {
    static C: OnceLock<Selector> = OnceLock::new();
    C.get_or_init(|| Selector::parse("img").unwrap())
}

struct CompiledNewsSelectors {
    container: Selector,
    article: Selector,
    title: Selector,
    source: Selector,
    relative_date: Selector,
    thumbnail: Selector,
}

impl CompiledNewsSelectors {
    /// Compiles the configured news selectors, falling back per-field to
    /// the [`NewsSelectors`] defaults when a TOML-provided selector fails
    /// to parse (same graceful degradation as `CompiledSelectors::compile`,
    /// but one bad selector must not disable the whole news extractor).
    fn compile(cfg: &NewsSelectors) -> Self {
        let defaults = NewsSelectors::default();
        Self {
            container: parse_news_selector(&cfg.container, &defaults.container, "news.container"),
            article: parse_news_selector(&cfg.article, &defaults.article, "news.article"),
            title: parse_news_selector(&cfg.title, &defaults.title, "news.title"),
            source: parse_news_selector(&cfg.source, &defaults.source, "news.source"),
            relative_date: parse_news_selector(
                &cfg.relative_date,
                &defaults.relative_date,
                "news.relative_date",
            ),
            thumbnail: parse_news_selector(&cfg.thumbnail, &defaults.thumbnail, "news.thumbnail"),
        }
    }
}

/// Universal last-resort selector for [`parse_news_selector`] — only
/// reachable if a crate default fails to parse (programming error).
fn sel_news_universal_fallback() -> &'static Selector {
    static C: OnceLock<Selector> = OnceLock::new();
    C.get_or_init(|| Selector::parse("*").unwrap())
}

/// Parses `configured`; on failure logs a warning and parses the crate
/// default. Default selectors are compile-time constants validated by the
/// test suite (`news_selectors_defaults_all_compile`); should one ever fail
/// to parse, degrades to the universal selector instead of panicking (same
/// no-panic posture as `CompiledSelectors::compile` and
/// `CompiledLiteSelectors::compile`).
fn parse_news_selector(configured: &str, default_value: &str, field: &'static str) -> Selector {
    match Selector::parse(configured) {
        Ok(selector) => selector,
        Err(error) => {
            tracing::warn!(
                ?error,
                selector = %configured,
                field,
                "Invalid news selector from config — falling back to default"
            );
            match Selector::parse(default_value) {
                Ok(selector) => selector,
                Err(error) => {
                    tracing::error!(
                        ?error,
                        selector = %default_value,
                        field,
                        "Default news selector invalid — falling back to universal selector"
                    );
                    sel_news_universal_fallback().clone()
                }
            }
        }
    }
}

/// Extracts news results from the Chrome-rendered `ia=news&iar=news` SERP.
///
/// Cascade (GAP-WS-104):
/// - Estratégia A: configured [`NewsSelectors`] — container → article →
///   title/anchor/source/date/thumbnail.
/// - Estratégia B: class-agnostic fallback, applied when A yields zero AND
///   the news container is present (obfuscated React class names).
///
/// Internal `duckduckgo.com` links are filtered via [`resolve_url`]; results
/// are deduplicated by URL preserving order, with 1-indexed positions.
pub fn extract_news_results_with_cfg(raw_html: &str, cfg: &SelectorConfig) -> Vec<NewsResult> {
    let document = Html::parse_document(raw_html);
    let compiled = CompiledNewsSelectors::compile(&cfg.news);

    // Prefer configured news container; fall back to full document (L-04).
    if let Some(container) = document.select(&compiled.container).next() {
        let results = extract_news_strategy_a(&container, &compiled);
        if !results.is_empty() {
            tracing::info!(total = results.len(), "News Estratégia A extracted results");
            return results;
        }
        tracing::info!("News Estratégia A returned empty — trying Estratégia B (class-agnostic)");
        let fallback = extract_news_strategy_b(&container);
        if !fallback.is_empty() {
            tracing::info!(
                total = fallback.len(),
                "News Estratégia B recovered results"
            );
            return fallback;
        }
    } else {
        tracing::info!("News container not found — trying Estratégia B on full document (L-04)");
    }

    // Full-document Estratégia B when container missing or empty.
    let body_sel = Selector::parse("body").ok();
    let scope = body_sel
        .as_ref()
        .and_then(|s| document.select(s).next())
        .unwrap_or_else(|| document.root_element());
    let doc_fallback = extract_news_strategy_b(&scope);
    if !doc_fallback.is_empty() {
        tracing::info!(
            total = doc_fallback.len(),
            "News full-document Estratégia B recovered results"
        );
    }
    doc_fallback
}

/// Estratégia A: semantic selectors from [`NewsSelectors`].
fn extract_news_strategy_a(
    container: &ElementRef<'_>,
    compiled: &CompiledNewsSelectors,
) -> Vec<NewsResult> {
    let mut results: Vec<NewsResult> = Vec::with_capacity(16);
    let mut seen_urls: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(16);
    let mut position: u32 = 0;

    for article in container.select(&compiled.article) {
        let Some(title_element) = article.select(&compiled.title).next() else {
            tracing::trace!("News article missing title element — skipping");
            continue;
        };
        let title = normalize_text(&join_text(&title_element), TITLE_LIMIT);
        if title.is_empty() {
            continue;
        }

        let Some(url) = first_external_url(&article) else {
            tracing::trace!(title = %title, "News article without external URL — skipping");
            continue;
        };
        if url.len() > URL_LIMIT || !seen_urls.insert(url.clone()) {
            continue;
        }

        let relative_date = article
            .select(&compiled.relative_date)
            .map(|el| normalize_text(&join_text(&el), SOURCE_LIMIT))
            .find(|t| looks_like_relative_date(t));
        let source = article
            .select(&compiled.source)
            .map(|el| normalize_text(&join_text(&el), SOURCE_LIMIT))
            .find(|t| !t.is_empty() && !looks_like_relative_date(t));
        let thumbnail = article
            .select(&compiled.thumbnail)
            .filter_map(|img| img.value().attr("src"))
            .find_map(resolve_thumbnail_url);

        position += 1;
        results.push(NewsResult {
            position,
            title,
            url,
            source,
            relative_date,
            thumbnail,
            content: None,
            content_size: None,
            content_extraction_method: None,
        });

        if results.len() >= 50 {
            break;
        }
    }

    results
}

/// Estratégia B: class-agnostic fallback for obfuscated React markup.
///
/// Collects every anchor with an external href inside the news container;
/// the anchor text becomes the title. Source and relative date come from
/// `span`/`time` texts in nearby ancestors (disambiguated via
/// `looks_like_relative_date`); the thumbnail is the closest `img` in the
/// same block (best-effort).
fn extract_news_strategy_b(container: &ElementRef<'_>) -> Vec<NewsResult> {
    let mut results: Vec<NewsResult> = Vec::with_capacity(16);
    let mut seen_urls: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(16);
    let mut position: u32 = 0;

    for anchor in container.select(sel_news_anchors()) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Some(url) = resolve_url(href) else {
            continue;
        };
        if url.len() > URL_LIMIT || !seen_urls.insert(url.clone()) {
            continue;
        }
        let title = normalize_text(&join_text(&anchor), TITLE_LIMIT);
        if title.is_empty() {
            continue;
        }

        let (source, relative_date) = news_meta_from_ancestors(&anchor, &title);
        let thumbnail = news_thumbnail_from_ancestors(&anchor);

        position += 1;
        results.push(NewsResult {
            position,
            title,
            url,
            source,
            relative_date,
            thumbnail,
            content: None,
            content_size: None,
            content_extraction_method: None,
        });

        if results.len() >= 50 {
            break;
        }
    }

    results
}

/// Returns the first anchor href inside `article` that resolves to an
/// external URL (see [`resolve_url`] — filters `duckduckgo.com` internals
/// and unwraps `uddg` redirects).
fn first_external_url(article: &ElementRef<'_>) -> Option<String> {
    for anchor in article.select(sel_news_anchors()) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        if let Some(url) = resolve_url(href) {
            return Some(url);
        }
    }
    None
}

/// Walks up to 4 ancestor levels (never past the news module root) looking
/// for `span`/`time` texts: the first that matches
/// `looks_like_relative_date` becomes `data_relativa`, the first other
/// non-empty text distinct from the title becomes `fonte`. The climb
/// continues while either field is still `None` (each field keeps the
/// innermost occurrence found), so metadata split across levels — e.g.
/// `fonte` as a sibling of the anchor and `data_relativa` only in an outer
/// wrapper — is not silently dropped.
fn news_meta_from_ancestors(
    anchor: &ElementRef<'_>,
    title: &str,
) -> (Option<String>, Option<String>) {
    let mut source: Option<String> = None;
    let mut relative_date: Option<String> = None;
    let mut current = anchor.parent();
    let mut level = 0;

    while let Some(node) = current {
        level += 1;
        if level > 4 {
            break;
        }
        if let Some(el) = ElementRef::wrap(node) {
            // Never classify metadata across the whole module: spans up
            // there belong to sibling cards.
            if el.value().attr("data-react-module-id").is_some() {
                break;
            }
            for meta in el.select(sel_news_meta_fallback()) {
                let text = normalize_text(&join_text(&meta), SOURCE_LIMIT);
                if text.is_empty() || text == title {
                    continue;
                }
                if looks_like_relative_date(&text) {
                    if relative_date.is_none() {
                        relative_date = Some(text);
                    }
                } else if source.is_none() {
                    source = Some(text);
                }
                if source.is_some() && relative_date.is_some() {
                    return (source, relative_date);
                }
            }
        }
        current = node.parent();
    }

    (source, relative_date)
}

/// Walks up to 4 ancestor levels (never past the news module root) looking
/// for the closest `img` with a resolvable `src` (best-effort thumbnail).
fn news_thumbnail_from_ancestors(anchor: &ElementRef<'_>) -> Option<String> {
    let mut current = anchor.parent();
    let mut level = 0;

    while let Some(node) = current {
        level += 1;
        if level > 4 {
            break;
        }
        if let Some(el) = ElementRef::wrap(node) {
            if el.value().attr("data-react-module-id").is_some() {
                break;
            }
            let thumbnail = el
                .select(sel_news_img_fallback())
                .filter_map(|img| img.value().attr("src"))
                .find_map(resolve_thumbnail_url);
            if thumbnail.is_some() {
                return thumbnail;
            }
        }
        current = node.parent();
    }

    None
}

/// Resolves a thumbnail `src` to an absolute URL.
///
/// Unlike [`resolve_url`], this does NOT filter `duckduckgo.com` hosts:
/// news thumbnails are proxied through
/// `external-content.duckduckgo.com` by design.
fn resolve_thumbnail_url(src: &str) -> Option<String> {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("//") {
        return Some(format!("https://{rest}"));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    None
}

/// Heuristic that decides whether a short text is a relative timestamp
/// rather than a publisher name — used to disambiguate the `fonte` and
/// `data_relativa` spans, which share the same markup on the news SERP.
///
/// Recognized (case-insensitive): PT `"há N minuto(s)/hora(s)/dia(s)"` (also
/// unaccented `"ha N ..."`) and `"agora"`; EN `"N minute(s)/hour(s)/day(s)
/// ago"` and `"just now"`; compact `"2h"`, `"15min"`, `"3d"`.
pub(crate) fn looks_like_relative_date(text: &str) -> bool {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() || normalized.chars().count() > 40 {
        return false;
    }
    if matches!(
        normalized.as_str(),
        "agora" | "agora mesmo" | "just now" | "now"
    ) {
        return true;
    }
    // PT: "há N <unidade>" (tolerates unaccented "ha ").
    if let Some(rest) = normalized
        .strip_prefix("há ")
        .or_else(|| normalized.strip_prefix("ha "))
    {
        return is_count_with_time_unit(rest);
    }
    // EN: "N <unit> ago".
    if let Some(head) = normalized.strip_suffix(" ago") {
        return is_count_with_time_unit(head);
    }
    // Compact: "2h", "15min", "3d".
    is_compact_relative_token(&normalized)
}

/// `true` when `text` starts with an integer count followed by a known
/// PT/EN time-unit token (e.g. `"2 horas"`, `"15 min"`, `"3 hours"`).
fn is_count_with_time_unit(text: &str) -> bool {
    const UNITS: &[&str] = &[
        "s", "seg", "segundo", "segundos", "second", "seconds", "min", "mins", "minuto", "minutos",
        "minute", "minutes", "h", "hora", "horas", "hour", "hours", "d", "dia", "dias", "day",
        "days", "semana", "semanas", "week", "weeks", "mês", "mes", "meses", "month", "months",
    ];
    let mut parts = text.split_whitespace();
    let Some(count) = parts.next() else {
        return false;
    };
    if count.is_empty() || !count.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let Some(unit) = parts.next() else {
        return false;
    };
    UNITS.contains(&unit)
}

/// `true` for compact relative tokens: digits immediately followed by a
/// short unit (`"2h"`, `"15min"`, `"3d"`).
fn is_compact_relative_token(text: &str) -> bool {
    let digit_count = text.chars().take_while(char::is_ascii_digit).count();
    if digit_count == 0 {
        return false;
    }
    let unit: String = text.chars().skip(digit_count).collect();
    matches!(unit.as_str(), "s" | "m" | "min" | "h" | "d")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_url_prefixa_protocol_relative() {
        assert_eq!(
            resolve_url("//exemplo.com/caminho"),
            Some("https://exemplo.com/caminho".to_string())
        );
    }

    #[test]
    fn resolver_url_desencapsula_redirect_uddg() {
        let href = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexemplo.com%2Fnoticia&rut=abc123";
        let resolvida = resolve_url(href).expect("should decode uddg");
        assert_eq!(resolvida, "https://exemplo.com/noticia");
    }

    #[test]
    fn resolve_url_unwraps_uddg_with_absolute_path() {
        let href = "/l/?uddg=https%3A%2F%2Fexemplo.com%2Farticle";
        let resolvida = resolve_url(href).expect("should decode uddg");
        assert_eq!(resolvida, "https://exemplo.com/article");
    }

    #[test]
    fn resolve_url_filters_duckduckgo_without_uddg() {
        assert_eq!(resolve_url("https://duckduckgo.com/settings"), None);
        assert_eq!(resolve_url("//html.duckduckgo.com/html/?q=teste"), None);
    }

    #[test]
    fn resolver_url_mantem_absolutas_externas() {
        assert_eq!(
            resolve_url("https://exemplo.com.br/noticia"),
            Some("https://exemplo.com.br/noticia".to_string())
        );
    }

    #[test]
    fn resolve_url_returns_none_for_empty_string() {
        assert_eq!(resolve_url(""), None);
        assert_eq!(resolve_url("   "), None);
    }

    #[test]
    fn normalize_text_colapsa_whitespace() {
        assert_eq!(
            normalize_text("  olá   mundo\n\n\ttexto  ", 100),
            "olá mundo texto"
        );
    }

    #[test]
    fn normalize_text_trunca_respeitando_char_boundary() {
        let long_text = "á".repeat(300);
        let truncated = normalize_text(&long_text, 200);
        assert_eq!(truncated.chars().count(), 200);
    }

    #[test]
    fn extract_results_works_with_minimal_html() {
        let html = r#"
            <html><body>
            <div id="links">
              <div class="result">
                <a class="result__a" href="//exemplo.com/pagina">Título Exemplo</a>
                <a class="result__snippet">Esta é uma descrição de exemplo.</a>
                <span class="result__url">exemplo.com</span>
              </div>
              <div class="result result--ad">
                <a class="result__a" href="//anuncio.com">Anúncio Pago</a>
              </div>
              <div class="result">
                <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwikipedia.org%2Fwiki%2FRust">Rust</a>
                <a class="result__snippet">Linguagem de programação Rust.</a>
              </div>
            </div>
            </body></html>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 2, "deve filtrar o anúncio");
        assert_eq!(results[0].position, 1);
        assert_eq!(results[0].title, "Título Exemplo");
        assert_eq!(results[0].url, "https://exemplo.com/pagina");
        assert_eq!(
            results[0].snippet.as_deref(),
            Some("Esta é uma descrição de exemplo.")
        );
        assert_eq!(results[1].position, 2);
        assert_eq!(results[1].title, "Rust");
        assert_eq!(results[1].url, "https://wikipedia.org/wiki/Rust");
    }

    #[test]
    fn extract_results_filters_js_urls() {
        let html = r#"
            <div id="links">
              <div class="result">
                <a class="result__a" href="//duckduckgo.com/y.js?ad=1">Tracker</a>
              </div>
              <div class="result">
                <a class="result__a" href="//site-valido.com/pagina">Válido</a>
              </div>
            </div>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Válido");
    }

    #[test]
    fn extract_results_respects_data_nrn_ad_attribute() {
        let html = r#"
            <div id="links">
              <div class="result" data-nrn="ad">
                <a class="result__a" href="//anuncio.com">Patrocinado</a>
              </div>
              <div class="result" data-nrn="organic">
                <a class="result__a" href="//organico.com">Orgânico</a>
              </div>
            </div>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://organico.com");
    }

    #[test]
    fn extract_results_empty_returns_empty_vec() {
        let html = "<html><body>Sem results</body></html>";
        let results = extract_results(html);
        assert!(results.is_empty());
    }

    #[test]
    fn strategy_2_recovers_when_classes_absent() {
        let html = r#"
            <html><body>
            <div id="links">
              <div>
                <a href="//exemplo.com/artigo">Título do Artigo de Exemplo</a>
                <p>Este é o snippet descritivo do artigo que precisa ter texto suficiente para ser considerado substancial e assim ser capturado como snippet pela heurística de extração.</p>
              </div>
              <div>
                <a href="//outro-site.com/noticia">Notícia Externa Importante</a>
                <p>Descrição relevante da notícia com mais de quarenta caracteres para garantir captura pela heurística de snippet.</p>
              </div>
            </div>
            </body></html>
        "#;
        let results = extract_results_with_strategies(html);
        assert!(
            results.len() >= 2,
            "Estratégia 2 deve recuperar pelo menos 2 results"
        );
        assert_eq!(results[0].title, "Título do Artigo de Exemplo");
        assert_eq!(results[0].url, "https://exemplo.com/artigo");
    }

    #[test]
    fn strategy_2_does_not_run_if_strategy_1_worked() {
        let html = r#"
            <html><body>
            <div id="links">
              <div class="result">
                <a class="result__a" href="//valido.com">Válido via Estratégia 1</a>
                <a class="result__snippet">Snippet curto.</a>
              </div>
            </div>
            </body></html>
        "#;
        let results = extract_results_with_strategies(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Válido via Estratégia 1");
    }

    #[test]
    fn extract_results_lite_parses_duckduckgo_lite_table() {
        let html = r#"
            <html><body>
            <table>
              <tr>
                <td valign="top">1.&nbsp;</td>
                <td><a rel="nofollow" href="//exemplo.com/pagina1" class="result-link">Primeiro Resultado Lite</a></td>
              </tr>
              <tr>
                <td>&nbsp;</td>
                <td class="result-snippet">Esta é a descrição do primeiro resultado com texto suficiente para ser reconhecido.</td>
              </tr>
              <tr>
                <td valign="top">2.&nbsp;</td>
                <td><a rel="nofollow" href="//exemplo.com/pagina2" class="result-link">Segundo Resultado Lite</a></td>
              </tr>
              <tr>
                <td>&nbsp;</td>
                <td class="result-snippet">Descrição do segundo resultado com bastante texto também.</td>
              </tr>
            </table>
            </body></html>
        "#;
        let results = extract_results_lite(html);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].position, 1);
        assert_eq!(results[0].title, "Primeiro Resultado Lite");
        assert_eq!(results[0].url, "https://exemplo.com/pagina1");
        assert!(results[0].snippet.is_some());
        assert_eq!(results[1].title, "Segundo Resultado Lite");
    }

    #[test]
    fn extract_results_lite_empty_returns_empty_vec() {
        let html = "<html><body><p>Nada aqui</p></body></html>";
        let results = extract_results_lite(html);
        assert!(results.is_empty());
    }

    #[test]
    fn extract_results_with_custom_cfg_uses_alternate_selector() {
        // HTML sem `.result` original, mas com `.custom-result` — extrator default falharia.
        let html = r#"
            <div id="custom-links">
              <div class="custom-result">
                <a class="custom-title" href="//site.com/a">Título A</a>
                <span class="custom-snippet">Snippet A</span>
              </div>
              <div class="custom-result">
                <a class="custom-title" href="//site.com/b">Título B</a>
                <span class="custom-snippet">Snippet B</span>
              </div>
            </div>
        "#;

        // Default finds nothing.
        let padrao = extract_results(html);
        assert!(
            padrao.is_empty(),
            "default não deve casar com .custom-result"
        );

        // Config customizada deve funcionar.
        let mut cfg = SelectorConfig::default();
        cfg.html_endpoint.result_item = "#custom-links .custom-result".to_string();
        cfg.html_endpoint.title_and_url = ".custom-title".to_string();
        cfg.html_endpoint.snippet = ".custom-snippet".to_string();

        let results = extract_results_with_cfg(html, &cfg);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Título A");
        assert_eq!(results[1].title, "Título B");
    }

    #[test]
    fn extract_results_with_cfg_filters_custom_classes() {
        let html = r#"
            <div id="links">
              <div class="result organic">
                <a class="result__a" href="//a.com">Orgânico</a>
              </div>
              <div class="result my-custom-ad">
                <a class="result__a" href="//ad.com">Anúncio Custom</a>
              </div>
            </div>
        "#;

        let mut cfg = SelectorConfig::default();
        cfg.html_endpoint.ads_filter.ad_classes = vec![".my-custom-ad".to_string()];

        let results = extract_results_with_cfg(html, &cfg);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://a.com");
    }

    #[test]
    fn extract_results_lite_filters_duckduckgo_links() {
        let html = r#"
            <table>
              <tr><td><a href="//duckduckgo.com/about" class="result-link">Sobre DDG</a></td></tr>
              <tr><td class="result-snippet">Snippet do DDG não deve aparecer.</td></tr>
              <tr><td><a href="//externo.com/doc" class="result-link">Doc Externa</a></td></tr>
              <tr><td class="result-snippet">Descrição da documentação externa relevante.</td></tr>
            </table>
        "#;
        let results = extract_results_lite(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://externo.com/doc");
    }

    // =========================================================================
    // WS-11 — Property-based invariants for HTML parsers (stdlib only, no
    // proptest dependency in PATCH bump). Each test feeds the parser a family
    // of representative inputs and asserts invariants that MUST hold across
    // the family. These are lighter than a full proptest framework but still
    // catch regressions in the parse pipeline when inputs are fuzzed by hand.
    // =========================================================================

    /// Invariant: extracting from an empty/blank input always returns an empty
    /// Vec — no panic, no spurious rows, no NaN positions.
    #[test]
    fn ws11_invariant_empty_inputs_yield_empty_results() {
        for input in &[
            "",
            " ",
            "\n",
            "\t\t",
            "<html></html>",
            "<!-- only a comment -->",
        ] {
            let r_html = extract_results(input);
            let r_lite = extract_results_lite(input);
            assert!(
                r_html.is_empty(),
                "extract_results({input:?}) must be empty, got {} rows",
                r_html.len()
            );
            assert!(
                r_lite.is_empty(),
                "extract_results_lite({input:?}) must be empty, got {} rows",
                r_lite.len()
            );
            for r in r_html.iter().chain(r_lite.iter()) {
                assert!(
                    r.position >= 1,
                    "position must be 1-based, got {}",
                    r.position
                );
            }
        }
    }

    /// Invariant: positions are dense and 1-based (no gaps, no duplicates).
    #[test]
    fn ws11_invariant_positions_are_dense_and_one_based() {
        let html = r#"
            <div id="links">
              <div class="result"><a class="result__a" href="//a.com">A</a></div>
              <div class="result"><a class="result__a" href="//b.com">B</a></div>
              <div class="result"><a class="result__a" href="//c.com">C</a></div>
              <div class="result"><a class="result__a" href="//d.com">D</a></div>
              <div class="result"><a class="result__a" href="//e.com">E</a></div>
            </div>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 5);
        for (i, r) in results.iter().enumerate() {
            assert_eq!(
                r.position,
                (i + 1) as u32,
                "positions must be 1-based and dense"
            );
        }
    }

    /// Invariant: extracted URLs are always absolute (start with http/https)
    /// or empty. Protocol-relative `//host/path` must be promoted to `https://`.
    #[test]
    fn ws11_invariant_urls_are_normalized_to_absolute() {
        let html = r#"
            <div id="links">
              <div class="result"><a class="result__a" href="//exemplo.com/p">E</a></div>
              <div class="result"><a class="result__a" href="http://inseguro.com">I</a></div>
              <div class="result"><a class="result__a" href="https://seguro.com">S</a></div>
            </div>
        "#;
        let results = extract_results(html);
        assert!(!results.is_empty(), "must extract at least one row");
        for r in &results {
            assert!(
                r.url.starts_with("http://") || r.url.starts_with("https://") || r.url.is_empty(),
                "URL must be absolute (http/https) or empty, got {:?}",
                r.url
            );
        }
        // Protocol-relative `//` must be promoted to `https://`.
        let relative = results
            .iter()
            .find(|r| r.url.contains("exemplo.com"))
            .expect("exemplo.com must be present");
        assert!(
            relative.url.starts_with("https://"),
            "protocol-relative URL must be promoted to https, got {:?}",
            relative.url
        );
    }

    /// Invariant: re-parsing the same input yields the same output (idempotence).
    /// This catches hidden state or RNG-based drift in the parser.
    #[test]
    fn ws11_invariant_extraction_is_idempotent() {
        let html = r#"
            <div id="links">
              <div class="result"><a class="result__a" href="//a.com/1">A1</a></div>
              <div class="result"><a class="result__a" href="//a.com/2">A2</a></div>
              <div class="result"><a class="result__a" href="//a.com/3">A3</a></div>
            </div>
        "#;
        let r1 = extract_results(html);
        let r2 = extract_results(html);
        assert_eq!(r1.len(), r2.len(), "parser must be deterministic");
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.url, b.url);
            assert_eq!(a.title, b.title);
        }
    }

    /// Invariant: malformed HTML with unclosed/mismatched tags does not panic.
    /// The parser must be tolerant per the html5ever contract.
    #[test]
    fn ws11_invariant_malformed_html_does_not_panic() {
        let cases = vec![
            r#"<div id="links"><div class="result"><a class="result__a" href="//a.com">A"#,
            r#"<DIV ID=LINKS><DIV CLASS=RESULT><A CLASS=RESULT__A HREF=//A.COM>A</A>"#,
            r#"<<>><>invalid<<>>tags<<>>"#, // broken
            "<html><body>",                 // truncated
        ];
        for input in cases {
            // Must not panic.
            let _ = extract_results(input);
            let _ = extract_results_lite(input);
        }
    }

    // =========================================================================
    // GAP-WS-104 — News vertical extraction (3 fixtures + date heuristic)
    // =========================================================================

    const NEWS_FIXTURE_A: &str = include_str!("../tests/fixtures/ddg_news_serp.html");
    const NEWS_FIXTURE_OBFUSCATED: &str =
        include_str!("../tests/fixtures/ddg_news_serp_ofuscada.html");
    const NEWS_FIXTURE_EMPTY: &str = include_str!("../tests/fixtures/ddg_news_serp_vazia.html");

    #[test]
    fn extract_news_strategy_a_extracts_unique_external_articles() {
        let cfg = SelectorConfig::default();
        let results = extract_news_results_with_cfg(NEWS_FIXTURE_A, &cfg);

        // A fixture tem 6 <article>: 4 externos únicos + 1 armadilha interna
        // duckduckgo.com (descartada) + 1 URL duplicada (deduplicada).
        assert_eq!(results.len(), 4);
        assert!(
            results.iter().all(|r| !r.url.contains("duckduckgo.com")),
            "a armadilha interna duckduckgo.com deve ser descartada"
        );

        assert_eq!(results[0].position, 1);
        assert_eq!(
            results[0].title,
            "Governo anuncia novo pacote de investimentos em infraestrutura"
        );
        assert_eq!(results[0].url, "https://exemplo-veiculo-1.com/artigo-1");
        assert_eq!(results[0].source.as_deref(), Some("G1"));
        assert_eq!(results[0].relative_date.as_deref(), Some("há 2 horas"));
        let thumbnail = results[0].thumbnail.as_deref().expect("thumbnail present");
        assert!(
            thumbnail.starts_with("https://external-content.duckduckgo.com/"),
            "thumbnail protocol-relative deve virar https, got {thumbnail:?}"
        );

        // Data relativa EN no segundo card.
        assert_eq!(results[1].source.as_deref(), Some("Reuters"));
        assert_eq!(results[1].relative_date.as_deref(), Some("3 hours ago"));

        // Posições densas 1-indexed após filtro + dedupe.
        for (i, r) in results.iter().enumerate() {
            assert_eq!(r.position, (i + 1) as u32);
        }
    }

    #[test]
    fn extract_news_strategy_b_recovers_from_obfuscated_markup() {
        let cfg = SelectorConfig::default();
        let results = extract_news_results_with_cfg(NEWS_FIXTURE_OBFUSCATED, &cfg);

        // Sem <article>/<h3> e com classes 100% ofuscadas — só a Estratégia B
        // (agnóstica de classe) recupera os 3 cards do container.
        assert_eq!(results.len(), 3);
        assert_eq!(
            results[0].title,
            "Prefeitura confirma cronograma de obras no centro da cidade"
        );
        assert_eq!(results[0].url, "https://exemplo-veiculo-5.com/nota-5");
        assert_eq!(results[0].source.as_deref(), Some("Estadão"));
        assert_eq!(results[0].relative_date.as_deref(), Some("há 4 horas"));
        assert_eq!(results[1].relative_date.as_deref(), Some("2 days ago"));
        assert_eq!(results[2].source.as_deref(), Some("O Globo"));
    }

    #[test]
    fn extract_news_empty_serp_returns_empty_vec() {
        let cfg = SelectorConfig::default();
        let results = extract_news_results_with_cfg(NEWS_FIXTURE_EMPTY, &cfg);
        assert!(results.is_empty());
    }

    #[test]
    fn extract_news_without_container_returns_empty_vec() {
        let cfg = SelectorConfig::default();
        let html = "<html><body><div id=\"links\"><p>web serp</p></div></body></html>";
        assert!(extract_news_results_with_cfg(html, &cfg).is_empty());
    }

    #[test]
    fn extract_news_invalid_config_selectors_fall_back_to_defaults() {
        let mut cfg = SelectorConfig::default();
        cfg.news.container = ":::invalid:::".to_string();
        cfg.news.title = "[".to_string();
        let results = extract_news_results_with_cfg(NEWS_FIXTURE_A, &cfg);
        assert_eq!(
            results.len(),
            4,
            "seletor inválido deve cair para o default"
        );
    }

    #[test]
    fn looks_like_relative_date_matches_pt_en_and_compact_forms() {
        for s in [
            "há 2 horas",
            "há 15 min",
            "Há 1 hora",
            "ha 3 dias",
            "3 hours ago",
            "1 day ago",
            "45 minutes ago",
            "agora",
            "just now",
            "2h",
            "15min",
            "3d",
        ] {
            assert!(
                looks_like_relative_date(s),
                "{s:?} deveria ser data relativa"
            );
        }
        for s in [
            "G1",
            "Reuters",
            "Folha de S.Paulo",
            "Estadão",
            "BBC News",
            "",
            "Hamburgo",
            "há muito tempo atrás nesta cidade grande demais",
        ] {
            assert!(
                !looks_like_relative_date(s),
                "{s:?} NÃO deveria ser data relativa"
            );
        }
    }

    #[test]
    fn news_meta_from_ancestors_finds_date_above_source_level() {
        // F6: fonte irmã direta do <a> (nível 1) e data_relativa apenas num
        // wrapper externo (nível 2) — a subida deve continuar enquanto
        // qualquer um dos dois campos ainda for None.
        let html = concat!(
            "<div data-react-module-id=\"news\">",
            "<div>",
            "<div>",
            "<a href=\"https://exemplo-veiculo-9.com/nota-9\">Manchete de teste F6</a>",
            "<span>Fonte Exemplo</span>",
            "</div>",
            "<time>há 2 horas</time>",
            "</div>",
            "</div>",
        );
        let document = Html::parse_document(html);
        let anchor_sel = Selector::parse("a[href]").expect("selector de teste válido");
        let anchor = document
            .select(&anchor_sel)
            .next()
            .expect("âncora presente no HTML sintético");

        let (source, relative_date) = news_meta_from_ancestors(&anchor, "Manchete de teste F6");
        assert_eq!(source.as_deref(), Some("Fonte Exemplo"));
        assert_eq!(
            relative_date.as_deref(),
            Some("há 2 horas"),
            "data_relativa no nível 2 não pode ser perdida quando a fonte é achada no nível 1"
        );
    }

    #[test]
    fn news_selectors_defaults_all_compile() {
        // F7: garante que todos os defaults de NewsSelectors::default()
        // compilam — pré-condição do fallback sem panic de parse_news_selector.
        let defaults = NewsSelectors::default();
        for (field, value) in [
            ("container", &defaults.container),
            ("article", &defaults.article),
            ("title", &defaults.title),
            ("source", &defaults.source),
            ("relative_date", &defaults.relative_date),
            ("thumbnail", &defaults.thumbnail),
        ] {
            assert!(
                Selector::parse(value).is_ok(),
                "default news.{field} = {value:?} deve compilar"
            );
        }
    }
}
