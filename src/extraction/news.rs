// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (news vertical extraction)
//! News vertical extraction and promo filtering.

use super::url::resolve_url;
use super::web::{join_text, normalize_text};
use crate::types::{NewsResult, NewsSelectors, SelectorConfig};
use scraper::{ElementRef, Html, Selector};
use std::sync::LazyLock;

const TITLE_LIMIT: usize = 200;
const URL_LIMIT: usize = 2000;
const SOURCE_LIMIT: usize = 120;

fn sel_news_anchors() -> &'static Selector {
    static C: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("a[href]").expect("static CSS selector 'a[href]' is valid"));
    &C
}

fn sel_news_meta_fallback() -> &'static Selector {
    static C: LazyLock<Selector> = LazyLock::new(|| {
        Selector::parse("span, time").expect("static CSS selector 'span, time' is valid")
    });
    &C
}

fn sel_news_img_fallback() -> &'static Selector {
    static C: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("img").expect("static CSS selector 'img' is valid"));
    &C
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
    static C: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("*").expect("static CSS selector '*' is valid"));
    &C
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

/// Returns true when `url` is a `DuckDuckGo` chrome/footer promotional link
/// (app store CTAs, Duck.ai, community), not a news article.
///
/// GAP-WS-NEWS-LIVE-001 / GAP-WS-NEWS-FETCH-WASTE-001 / v0.9.9.
pub fn is_ddg_promo_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    // Host / path denylist (promo UI scraped when news container is missing).
    const HOST_MARKERS: &[&str] = &[
        "apps.apple.com",
        "play.google.com",
        "duck.ai",
        "reddit.com/r/duckduckgo",
        "www.reddit.com/r/duckduckgo",
        "insideduckduckgo.substack.com",
        "spreadprivacy.com",
    ];
    for m in HOST_MARKERS {
        if lower.contains(m) {
            return true;
        }
    }
    // UTM / CT campaign markers from DDG SERP chrome.
    if lower.contains("utm_campaign=serp") || lower.contains("ct=serp-atb-serp") {
        return true;
    }
    if lower.contains("origin=funnel_playstore") {
        return true;
    }
    false
}

/// Filters promo UI URLs and reindexes 1-based positions.
///
/// Returns `(kept, removed_promo_count)`.
pub fn filter_news_results(mut results: Vec<NewsResult>) -> (Vec<NewsResult>, u32) {
    let before = results.len();
    results.retain(|r| !is_ddg_promo_url(r.url.as_str()));
    let removed = (before - results.len()) as u32;
    for (i, r) in results.iter_mut().enumerate() {
        r.position = (i + 1) as u32;
    }
    if removed > 0 {
        tracing::debug!(
            removed,
            kept = results.len(),
            "News promo URLs filtered (GAP-WS-NEWS-LIVE-001)"
        );
    }
    (results, removed)
}

/// Extracts news results from the Chrome-rendered `ia=news&iar=news` SERP.
///
/// Cascade (GAP-WS-104 / v0.9.9 NEWS-LIVE):
/// - Strategy A: configured [`NewsSelectors`] — container → article →
///   title/anchor/source/date/thumbnail.
/// - Strategy B: class-agnostic fallback **only inside a found news container**.
/// - Full-document B is still attempted for recovery, but **promo URLs are
///   always stripped** — empty is preferred over DDG chrome/footer links.
///
/// Internal `duckduckgo.com` links are filtered via [`resolve_url`]; results
/// are deduplicated by URL preserving order, with 1-indexed positions.
pub fn extract_news_results_with_cfg(raw_html: &str, cfg: &SelectorConfig) -> Vec<NewsResult> {
    extract_news_results_with_stats(raw_html, cfg).0
}

/// Same as [`extract_news_results_with_cfg`] but also returns how many promo
/// URLs were filtered (GAP-WS-NEWS-LIVE-001 v0.9.9 agent metadata).
pub fn extract_news_results_with_stats(
    raw_html: &str,
    cfg: &SelectorConfig,
) -> (Vec<NewsResult>, u32) {
    let document = Html::parse_document(raw_html);
    let compiled = CompiledNewsSelectors::compile(&cfg.news);

    // Typical SERP news block is small (≤16–32 items); pre-size like web extract.
    let mut raw: Vec<NewsResult> = Vec::with_capacity(16);

    // Prefer configured news container.
    if let Some(container) = document.select(&compiled.container).next() {
        let results = extract_news_strategy_a(&container, &compiled);
        if !results.is_empty() {
            tracing::debug!(total = results.len(), "News strategy A extracted results");
            raw = results;
        } else {
            tracing::debug!(
                "News strategy A returned empty — trying strategy B (class-agnostic)"
            );
            let fallback = extract_news_strategy_b(&container);
            if !fallback.is_empty() {
                tracing::debug!(
                    total = fallback.len(),
                    "News strategy B recovered results"
                );
                raw = fallback;
            }
        }
    } else {
        tracing::debug!(
            "News container not found — trying alternate containers then full-document B with promo filter"
        );
        // Alternate containers (DOM 2026 may not use data-react-module-id="news").
        for alt in [
            "[data-testid=\"news-vertical\"]",
            "[data-testid=\"news\"]",
            "[data-area=\"news\"]",
            "section[data-testid*=\"news\"]",
            "[data-react-module-id=\"news\"]",
        ] {
            if let Ok(sel) = Selector::parse(alt) {
                if let Some(container) = document.select(&sel).next() {
                    let results = extract_news_strategy_a(&container, &compiled);
                    if results.is_empty() {
                        let fb = extract_news_strategy_b(&container);
                        if !fb.is_empty() {
                            raw = fb;
                            break;
                        }
                    } else {
                        raw = results;
                        break;
                    }
                }
            }
        }
        if raw.is_empty() {
            // Last resort: full document B — MUST pass promo filter (v0.9.9).
            let body_sel = Selector::parse("body").ok();
            let scope = body_sel
                .as_ref()
                .and_then(|s| document.select(s).next())
                .unwrap_or_else(|| document.root_element());
            raw = extract_news_strategy_b(&scope);
            if !raw.is_empty() {
                tracing::debug!(
                    total = raw.len(),
                    "News full-document strategy B candidates before promo filter"
                );
            }
        }
    }

    let (kept, removed) = filter_news_results(raw);
    if kept.is_empty() && removed > 0 {
        tracing::warn!(
            removed,
            "News extract yielded only DDG promo/chrome links — returning empty (honest zero)"
        );
    }
    (kept, removed)
}

/// Strategy A: semantic selectors from [`NewsSelectors`].
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
        if url.as_str().len() > URL_LIMIT || seen_urls.contains(url.as_str()) {
            continue;
        }
        seen_urls.insert(url.as_str().to_owned());

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

/// Strategy B: class-agnostic fallback for obfuscated React markup.
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
        if url.as_str().len() > URL_LIMIT || seen_urls.contains(url.as_str()) {
            continue;
        }
        seen_urls.insert(url.as_str().to_owned());
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
fn first_external_url(article: &ElementRef<'_>) -> Option<crate::types::HttpUrl> {
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
pub(crate) fn news_meta_from_ancestors(
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
pub(crate) fn resolve_thumbnail_url(src: &str) -> Option<String> {
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
pub(crate) fn is_count_with_time_unit(text: &str) -> bool {
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
pub(crate) fn is_compact_relative_token(text: &str) -> bool {
    let digit_count = text.chars().take_while(char::is_ascii_digit).count();
    if digit_count == 0 {
        return false;
    }
    let unit: String = text.chars().skip(digit_count).collect();
    matches!(unit.as_str(), "s" | "m" | "min" | "h" | "d")
}

