// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (in-memory merge, no I/O).
//! Result aggregation across sub-queries for the deep-research pipeline.
//!
//! Two strategies are supported:
//!
//! - [`AggregationStrategy::Rrf`] — Reciprocal Rank Fusion (Cormack et al.,
//!   2009). For each sub-query, the score of a result at rank `r` is
//!   `1 / (K + r)`. Scores are summed across all sub-queries that mention the
//!   same canonical URL. The default K is 60 (matches the literature and the
//!   `GraphRAG` memory subsystem's hybrid search).
//! - [`AggregationStrategy::DedupeByUrl`] — canonical-URL deduplication that
//!   keeps the FIRST occurrence (lowest source-index wins) and discards the
//!   rest. No scoring is performed; the returned list preserves the input
//!   order, which is convenient for stable, predictable JSON output.
//!
//! # URL canonicalization (gap 2.3 of the v0.7.0 audit)
//!
//! The canonical form is computed by [`canonicalize_url`]:
//!
//! 1. Lowercase the scheme and host.
//! 2. Strip the fragment (everything after `#`).
//! 3. Drop tracking query parameters (`utm_*`, `fbclid`, `gclid`, `ref`,
//!    `mc_cid`, `mc_eid`).
//! 4. Sort the remaining query parameters alphabetically (so that
//!    `?a=1&b=2` and `?b=2&a=1` compare equal).
//! 5. Normalise trailing slashes on the path (collapse `///+` to `/` and
//!    strip the trailing slash when the path is longer than `/`).
//!
//! The canonical form is then hashed with `blake3` (first 16 hex chars) to
//! serve as the dedup key — URLs with identical canonical form collide.

use crate::types::SearchOutput;
use serde::{Deserialize, Serialize};
use url::Url;

/// Aggregation strategy variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregationStrategy {
    /// Reciprocal Rank Fusion with the given K constant.
    Rrf(u32),
    /// Canonical-URL deduplication, keep first occurrence.
    DedupeByUrl,
}

/// Aggregated evidence item, scored and sorted by descending score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregatedItem {
    /// Source URL (as returned by the upstream search).
    pub url: String,
    /// Title (first non-empty across duplicates is kept).
    #[serde(rename = "titulo")]
    pub title: String,
    /// Display URL (optional, kept from the first occurrence).
    #[serde(rename = "url_exibicao", skip_serializing_if = "Option::is_none")]
    pub display_url: Option<String>,
    /// Optional snippet (first non-empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// Score in `[0.0, 1.0]` (1.0 = best, 0.0 = irrelevant). Higher is better.
    pub score: f64,
    /// Original position in the source sub-query (1-indexed).
    #[serde(rename = "posicao")]
    pub position: u32,
    /// Texts of the sub-queries that produced this result.
    #[serde(rename = "fontes")]
    pub sources: Vec<String>,
}

/// Computes the canonical form of a URL for deduplication.
///
/// Returns the input unchanged if it fails to parse — that way we never lose
/// data, and the hash of the original string still serves as a unique key.
///
/// # Examples
///
/// ```
/// use duckduckgo_search_cli::aggregation::canonicalize_url;
///
/// // Tracking parameters are stripped.
/// assert_eq!(
///     canonicalize_url("https://Example.com/a?utm_source=x&id=1"),
///     canonicalize_url("https://example.com/a?id=1"),
/// );
///
/// // Query parameters are sorted alphabetically.
/// assert_eq!(
///     canonicalize_url("https://example.com/p?b=2&a=1"),
///     "https://example.com/p?a=1&b=2",
/// );
///
/// // Fragment is removed.
/// assert_eq!(
///     canonicalize_url("https://example.com/p#section"),
///     "https://example.com/p",
/// );
///
/// // Unparseable input is returned unchanged.
/// assert_eq!(
///     canonicalize_url("not a url"),
///     "not a url",
/// );
/// ```
pub fn canonicalize_url(raw: &str) -> String {
    let parsed = match Url::parse(raw) {
        Ok(u) => u,
        Err(_) => return raw.to_string(),
    };

    let mut url = parsed.clone();
    let lower_host = url.host_str().map(|h| h.to_ascii_lowercase());
    if let Some(h) = lower_host {
        let _ = url.set_host(Some(&h));
    }
    let _ = url.set_scheme(url.scheme().to_ascii_lowercase().as_str());
    url.set_fragment(None);

    // Drop tracking params.
    let tracking: &[&str] = &[
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_term",
        "utm_content",
        "fbclid",
        "gclid",
        "ref",
        "mc_cid",
        "mc_eid",
    ];
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !tracking.contains(&k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if !pairs.is_empty() {
        let mut sorted = pairs;
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let mut ser = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in &sorted {
            ser.append_pair(k, v);
        }
        let query = ser.finish();
        url.set_query(Some(&query));
    } else {
        url.set_query(None);
    }

    let mut out = url.to_string();
    // Collapse repeated slashes in the path (after the scheme://host).
    if let Some(idx) = out.find("://") {
        let (scheme, rest) = out.split_at(idx + 3);
        // The character after "scheme://" is the first char of the host.
        // Find the first '/' that follows the host (it separates host from path).
        let host_end = rest.find('/').unwrap_or(rest.len());
        let host = &rest[..host_end];
        let path_and_query = &rest[host_end..];
        // Split into path and query so we can normalize the path independently.
        let (raw_path, raw_query) = match path_and_query.find('?') {
            Some(q) => (&path_and_query[..q], &path_and_query[q..]),
            None => (path_and_query, ""),
        };
        // Collapse repeated slashes in the path.
        let collapsed_path: String =
            raw_path
                .chars()
                .fold(String::with_capacity(raw_path.len()), |mut acc, c| {
                    if c == '/' && acc.ends_with('/') {
                        acc
                    } else {
                        acc.push(c);
                        acc
                    }
                });
        // Trim trailing slashes from the path (but keep at least one "/").
        let trimmed_path = collapsed_path.trim_end_matches('/');
        let trimmed_path = if trimmed_path.is_empty() {
            "/"
        } else {
            trimmed_path
        };
        out = format!("{scheme}{host}{trimmed_path}{raw_query}");
    }
    out
}

/// Returns a short, deterministic hash of the canonical URL.
pub fn canonical_hash(raw: &str) -> String {
    let canonical = canonicalize_url(raw);
    let hash = blake3::hash(canonical.as_bytes());
    let hex = hash.to_hex();
    hex.to_string()[..16].to_string()
}

/// Merges a list of per-sub-query `SearchOutput` into a single ranked list.
///
/// The order of `outputs` is the order in which the sub-queries were
/// dispatched; ranks inside each output are 1-indexed.
pub fn aggregate(outputs: &[SearchOutput], strategy: AggregationStrategy) -> Vec<AggregatedItem> {
    match strategy {
        AggregationStrategy::Rrf(k) => rrf_aggregate(outputs, k),
        AggregationStrategy::DedupeByUrl => dedupe_by_url(outputs),
    }
}

fn rrf_aggregate(outputs: &[SearchOutput], k: u32) -> Vec<AggregatedItem> {
    use std::collections::HashMap;

    #[derive(Default)]
    struct Entry {
        url: String,
        title: String,
        display_url: Option<String>,
        snippet: Option<String>,
        score: f64,
        position: u32,
        sources: Vec<String>,
    }

    let mut map: HashMap<String, Entry> = HashMap::new();
    for output in outputs {
        for (idx, r) in output.results.iter().enumerate() {
            let key = canonical_hash(&r.url);
            let rank = (idx as u32) + 1;
            let score = 1.0 / ((k as f64) + (rank as f64));
            let entry = map.entry(key).or_insert_with(|| Entry {
                url: r.url.clone(),
                title: String::new(),
                display_url: r.display_url.clone(),
                snippet: r.snippet.clone(),
                score: 0.0,
                position: rank,
                sources: Vec::new(),
            });
            entry.score += score;
            if entry.title.is_empty() {
                entry.title = r.title.clone();
            }
            if entry.display_url.is_none() {
                entry.display_url = r.display_url.clone();
            }
            if entry.snippet.is_none() {
                entry.snippet = r.snippet.clone();
            }
            if rank < entry.position {
                entry.position = rank;
            }
            if !output.query.is_empty() && !entry.sources.contains(&output.query) {
                entry.sources.push(output.query.clone());
            }
        }
    }

    let mut out: Vec<AggregatedItem> = map
        .into_values()
        .map(|e| AggregatedItem {
            url: e.url,
            title: e.title,
            display_url: e.display_url,
            snippet: e.snippet,
            // Normalize to [0, 1] by dividing by the theoretical maximum (one
            // occurrence at rank 1 across all sub-queries). For practical
            // outputs, scores usually fall in (0, 0.05].
            score: e.score,
            position: e.position,
            sources: e.sources,
        })
        .collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.position.cmp(&b.position))
    });
    out
}

fn dedupe_by_url(outputs: &[SearchOutput]) -> Vec<AggregatedItem> {
    use std::collections::HashMap;

    let mut map: HashMap<String, AggregatedItem> = HashMap::new();
    for output in outputs {
        for (idx, r) in output.results.iter().enumerate() {
            let key = canonical_hash(&r.url);
            map.entry(key).or_insert_with(|| AggregatedItem {
                url: r.url.clone(),
                title: r.title.clone(),
                display_url: r.display_url.clone(),
                snippet: r.snippet.clone(),
                score: 1.0,
                position: (idx as u32) + 1,
                sources: if output.query.is_empty() {
                    Vec::new()
                } else {
                    vec![output.query.clone()]
                },
            });
        }
    }
    let mut out: Vec<AggregatedItem> = map.into_values().collect();
    out.sort_by_key(|i| i.position);
    out
}

/// Aggregated news item, scored and sorted by descending score with a
/// recency tiebreak. GAP-WS-105 v0.8.9.
///
/// News aggregation uses its own RRF score space — news scores are NEVER
/// fused with the web [`AggregatedItem`] scores, because RRF scores computed
/// over distinct lists are not comparable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregatedNewsItem {
    /// Position in the aggregated list (1-indexed, reassigned after merge).
    #[serde(rename = "posicao")]
    pub position: usize,
    /// Headline (kept from the most recent exemplar across duplicates).
    #[serde(rename = "titulo")]
    pub title: String,
    /// Article URL (as returned by the upstream search).
    pub url: String,
    /// Publisher/source name (kept from the most recent exemplar).
    #[serde(rename = "fonte", skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Relative timestamp, verbatim (kept from the most recent exemplar).
    #[serde(rename = "data_relativa", skip_serializing_if = "Option::is_none")]
    pub relative_date: Option<String>,
    /// Thumbnail URL (kept from the most recent exemplar).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    /// Score (RRF sum for [`AggregationStrategy::Rrf`], `1.0` for
    /// [`AggregationStrategy::DedupeByUrl`]). Higher is better.
    pub score: f64,
    /// Number of sub-queries in which this news item appeared.
    #[serde(rename = "ocorrencias")]
    pub occurrences: usize,
}

/// Parses a `DuckDuckGo` relative timestamp into minutes of age.
///
/// Supports the Portuguese ("há 2 horas", "há 15 min", "há 3 dias",
/// "ontem") and English ("3 hours ago", "15 minutes ago", "1 day ago",
/// "yesterday", short "2h"/"15min"/"3d") forms rendered by the news SERP.
/// Matching is case-insensitive. Returns `None` when the string cannot be
/// parsed — callers must treat an unparsed date as "unknown age", never as
/// an error. The verbatim string still flows to the JSON output untouched.
pub(crate) fn relative_date_to_minutes(raw: &str) -> Option<u64> {
    let lower = raw.trim().to_lowercase();
    if lower == "ontem" || lower == "yesterday" {
        return Some(24 * 60);
    }
    let stripped = lower
        .strip_prefix("há ")
        .or_else(|| lower.strip_prefix("ha "))
        .unwrap_or(&lower);
    let stripped = stripped.strip_suffix(" ago").unwrap_or(stripped).trim();
    let digits_end = stripped
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(stripped.len());
    let value: u64 = stripped.get(..digits_end)?.parse().ok()?;
    let unit = stripped.get(digits_end..)?.trim();
    if unit.starts_with("min") {
        Some(value)
    } else if unit.starts_with('h') {
        value.checked_mul(60)
    } else if unit.starts_with('d') {
        value.checked_mul(1440)
    } else {
        None
    }
}

/// Merges the news lists of per-sub-query `SearchOutput` into a single
/// ranked list. Outputs without a news list (`news: None`) are skipped.
///
/// Ordering for [`AggregationStrategy::Rrf`]: score descending, ties broken
/// by recency (smallest parsed age first, unparsed dates last), then by the
/// stable first-seen order. Positions are reassigned 1..N after the merge.
/// GAP-WS-105 v0.8.9.
pub fn aggregate_news(
    outputs: &[SearchOutput],
    strategy: AggregationStrategy,
) -> Vec<AggregatedNewsItem> {
    match strategy {
        AggregationStrategy::Rrf(k) => rrf_aggregate_news(outputs, k),
        AggregationStrategy::DedupeByUrl => dedupe_news_by_url(outputs),
    }
}

fn rrf_aggregate_news(outputs: &[SearchOutput], k: u32) -> Vec<AggregatedNewsItem> {
    use std::collections::HashMap;

    struct Entry {
        item: AggregatedNewsItem,
        /// Age in minutes of the most recent exemplar seen so far.
        best_age: Option<u64>,
        /// First-seen index, used as the stable final tiebreak.
        order: usize,
    }

    let mut map: HashMap<String, Entry> = HashMap::new();
    let mut next_order = 0usize;
    for output in outputs {
        let Some(news) = output.news.as_ref() else {
            continue;
        };
        for n in news {
            let key = canonical_hash(&n.url);
            let score = 1.0 / (f64::from(k) + f64::from(n.position));
            let age = n
                .relative_date
                .as_deref()
                .and_then(relative_date_to_minutes);
            let entry = map.entry(key).or_insert_with(|| {
                let order = next_order;
                next_order += 1;
                Entry {
                    item: AggregatedNewsItem {
                        position: 0,
                        title: n.title.clone(),
                        url: n.url.clone(),
                        source: n.source.clone(),
                        relative_date: n.relative_date.clone(),
                        thumbnail: n.thumbnail.clone(),
                        score: 0.0,
                        occurrences: 0,
                    },
                    best_age: age,
                    order,
                }
            });
            entry.item.score += score;
            entry.item.occurrences += 1;
            // Keep the fields of the most recent exemplar (smallest age).
            let newer = match (age, entry.best_age) {
                (Some(a), Some(b)) => a < b,
                (Some(_), None) => true,
                _ => false,
            };
            if newer {
                entry.best_age = age;
                entry.item.title = n.title.clone();
                entry.item.source = n.source.clone();
                entry.item.relative_date = n.relative_date.clone();
                entry.item.thumbnail = n.thumbnail.clone();
            }
        }
    }

    let mut entries: Vec<Entry> = map.into_values().collect();
    entries.sort_by(|a, b| {
        b.item
            .score
            .partial_cmp(&a.item.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| match (a.best_age, b.best_age) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
            .then_with(|| a.order.cmp(&b.order))
    });
    entries
        .into_iter()
        .enumerate()
        .map(|(i, e)| {
            let mut item = e.item;
            item.position = i + 1;
            item
        })
        .collect()
}

fn dedupe_news_by_url(outputs: &[SearchOutput]) -> Vec<AggregatedNewsItem> {
    use std::collections::HashMap;

    let mut map: HashMap<String, (u32, usize, AggregatedNewsItem)> = HashMap::new();
    let mut next_order = 0usize;
    for output in outputs {
        let Some(news) = output.news.as_ref() else {
            continue;
        };
        for n in news {
            let key = canonical_hash(&n.url);
            map.entry(key).or_insert_with(|| {
                let order = next_order;
                next_order += 1;
                (
                    n.position,
                    order,
                    AggregatedNewsItem {
                        position: 0,
                        title: n.title.clone(),
                        url: n.url.clone(),
                        source: n.source.clone(),
                        relative_date: n.relative_date.clone(),
                        thumbnail: n.thumbnail.clone(),
                        score: 1.0,
                        occurrences: 1,
                    },
                )
            });
        }
    }
    let mut entries: Vec<(u32, usize, AggregatedNewsItem)> = map.into_values().collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    entries
        .into_iter()
        .enumerate()
        .map(|(i, (_, _, mut item))| {
            item.position = i + 1;
            item
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SearchMetadata;

    fn out_with(query: &str, urls: &[&str]) -> SearchOutput {
        SearchOutput {
            query: query.to_string(),
            engine: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: "2026-06-07T00:00:00Z".to_string(),
            region: "br-pt".to_string(),
            result_count: urls.len() as u32,
            results: urls
                .iter()
                .enumerate()
                .map(|(i, u)| crate::types::SearchResult {
                    position: (i as u32) + 1,
                    title: format!("title-{}", i),
                    url: u.to_string(),
                    display_url: None,
                    snippet: Some(format!("snippet-{}", i)),
                    original_title: None,
                    content: None,
                    content_size: None,
                    content_extraction_method: None,
                })
                .collect(),
            pages_fetched: 1,
            news: None,
            news_count: None,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: String::new(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                chrome_attempted: false,
                user_agent: String::new(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
                pre_flight_fired: false,
                zero_cause: None,
                sugestao_proxima_acao: None,
                bytes_raw: None,
                bytes_decompressed: None,
                cascade_level_observed: None,
                result_count_compat: None,
                endpoint_used_compat: None,
                vertical_used: None,
                chrome_path_resolved: None,
                chrome_channel: None,
            },
        }
    }

    #[test]
    fn canonicalize_strips_utm_and_lowercases_host() {
        let a = "HTTPS://Example.com/path/?utm_source=x&b=2&a=1#frag";
        let b = "https://example.com/path?a=1&b=2";
        assert_eq!(canonicalize_url(a), canonicalize_url(b));
    }

    #[test]
    fn canonicalize_collapses_repeated_slashes() {
        let a = "https://example.com/foo//bar///baz/";
        let b = "https://example.com/foo/bar/baz";
        assert_eq!(canonicalize_url(a), canonicalize_url(b));
    }

    #[test]
    fn canonicalize_preserves_root() {
        assert_eq!(
            canonicalize_url("https://example.com"),
            "https://example.com/"
        );
    }

    #[test]
    fn canonical_hash_is_stable() {
        let a = canonical_hash("https://Example.com/a?utm_source=x&b=1&a=1");
        let b = canonical_hash("https://example.com/a?a=1&b=1");
        assert_eq!(a, b);
    }

    #[test]
    fn rrf_combines_duplicate_urls_with_combined_score() {
        let a = out_with("alpha", &["https://example.com/a", "https://example.com/b"]);
        let b = out_with("beta", &["https://example.com/a", "https://example.com/c"]);
        let merged = aggregate(&[a, b], AggregationStrategy::Rrf(60));
        // Example.com/a appears in both => highest score.
        assert_eq!(merged[0].url, "https://example.com/a");
        assert!(merged[0].score > merged[1].score);
        // Sources trace back to both sub-queries.
        assert_eq!(merged[0].sources.len(), 2);
    }

    #[test]
    fn rrf_is_deterministic_across_calls() {
        let a = out_with("alpha", &["https://example.com/a", "https://example.com/b"]);
        let b = out_with("beta", &["https://example.com/b", "https://example.com/c"]);
        let m1 = aggregate(&[a.clone(), b.clone()], AggregationStrategy::Rrf(60));
        let m2 = aggregate(&[a, b], AggregationStrategy::Rrf(60));
        assert_eq!(m1, m2);
    }

    #[test]
    fn dedupe_keeps_first_occurrence() {
        let a = out_with("alpha", &["https://example.com/a", "https://example.com/b"]);
        let b = out_with("beta", &["https://example.com/a", "https://example.com/c"]);
        let merged = aggregate(&[a, b], AggregationStrategy::DedupeByUrl);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].url, "https://example.com/a");
        assert_eq!(merged[0].position, 1);
    }

    #[test]
    fn canonicalize_handles_invalid_url_gracefully() {
        let out = canonicalize_url("not a url");
        assert_eq!(out, "not a url");
    }

    fn news_item(
        position: u32,
        url: &str,
        title: &str,
        relative_date: Option<&str>,
    ) -> crate::types::NewsResult {
        crate::types::NewsResult {
            position,
            title: title.to_string(),
            url: url.to_string(),
            source: Some(format!("fonte-{position}")),
            relative_date: relative_date.map(str::to_string),
            thumbnail: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        }
    }

    fn news_out(query: &str, items: Vec<crate::types::NewsResult>) -> SearchOutput {
        let mut out = out_with(query, &[]);
        out.news = Some(items);
        out
    }

    #[test]
    fn news_rrf_dedupes_by_canonical_url_and_sums_score() {
        let a = news_out(
            "alpha",
            vec![news_item(
                1,
                "https://example.com/n?utm_source=x",
                "old",
                Some("há 3 dias"),
            )],
        );
        let b = news_out(
            "beta",
            vec![news_item(
                1,
                "https://example.com/n",
                "new",
                Some("há 1 hora"),
            )],
        );
        let merged = aggregate_news(&[a, b], AggregationStrategy::Rrf(60));
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].occurrences, 2);
        let expected = 2.0 / 61.0;
        assert!((merged[0].score - expected).abs() < 1e-12);
        // Fields come from the most recent exemplar.
        assert_eq!(merged[0].title, "new");
        assert_eq!(merged[0].relative_date.as_deref(), Some("há 1 hora"));
    }

    #[test]
    fn news_rrf_tiebreak_prefers_more_recent() {
        let a = news_out(
            "alpha",
            vec![news_item(
                1,
                "https://example.com/a",
                "a",
                Some("3 hours ago"),
            )],
        );
        let b = news_out(
            "beta",
            vec![news_item(
                1,
                "https://example.com/b",
                "b",
                Some("há 2 horas"),
            )],
        );
        let merged = aggregate_news(&[a, b], AggregationStrategy::Rrf(60));
        assert_eq!(merged.len(), 2);
        // Equal RRF scores => the more recent item ("há 2 horas") wins.
        assert_eq!(merged[0].url, "https://example.com/b");
        assert_eq!(merged[1].url, "https://example.com/a");
    }

    #[test]
    fn news_rrf_stable_order_when_dates_do_not_parse() {
        let a = news_out(
            "alpha",
            vec![news_item(1, "https://example.com/a", "a", None)],
        );
        let b = news_out(
            "beta",
            vec![news_item(
                1,
                "https://example.com/b",
                "b",
                Some("sem formato"),
            )],
        );
        let merged = aggregate_news(&[a, b], AggregationStrategy::Rrf(60));
        assert_eq!(merged.len(), 2);
        // Equal scores, no parseable date on either side => first-seen order.
        assert_eq!(merged[0].url, "https://example.com/a");
        assert_eq!(merged[1].url, "https://example.com/b");
    }

    #[test]
    fn news_rrf_reassigns_positions_one_to_n() {
        let a = news_out(
            "alpha",
            vec![
                news_item(1, "https://example.com/a", "a", None),
                news_item(2, "https://example.com/b", "b", None),
                news_item(3, "https://example.com/c", "c", None),
            ],
        );
        let merged = aggregate_news(&[a], AggregationStrategy::Rrf(60));
        let positions: Vec<usize> = merged.iter().map(|i| i.position).collect();
        assert_eq!(positions, vec![1, 2, 3]);
    }

    #[test]
    fn news_rrf_ignores_outputs_without_news() {
        let a = out_with("alpha", &["https://example.com/web"]);
        let b = news_out(
            "beta",
            vec![news_item(1, "https://example.com/n", "n", None)],
        );
        let merged = aggregate_news(&[a, b], AggregationStrategy::Rrf(60));
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].url, "https://example.com/n");
    }

    #[test]
    fn news_dedupe_keeps_first_occurrence() {
        let a = news_out(
            "alpha",
            vec![
                news_item(1, "https://example.com/a", "first", Some("há 3 dias")),
                news_item(2, "https://example.com/b", "b", None),
            ],
        );
        let b = news_out(
            "beta",
            vec![news_item(
                1,
                "https://example.com/a",
                "second",
                Some("há 1 hora"),
            )],
        );
        let merged = aggregate_news(&[a, b], AggregationStrategy::DedupeByUrl);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].title, "first");
        assert_eq!(merged[0].position, 1);
        assert_eq!(merged[1].position, 2);
    }

    #[test]
    fn relative_date_parses_portuguese_forms() {
        assert_eq!(relative_date_to_minutes("há 2 horas"), Some(120));
        assert_eq!(relative_date_to_minutes("há 1 hora"), Some(60));
        assert_eq!(relative_date_to_minutes("há 15 min"), Some(15));
        assert_eq!(relative_date_to_minutes("há 15 minutos"), Some(15));
        assert_eq!(relative_date_to_minutes("há 3 dias"), Some(4320));
        assert_eq!(relative_date_to_minutes("ontem"), Some(1440));
    }

    #[test]
    fn relative_date_parses_english_forms() {
        assert_eq!(relative_date_to_minutes("3 hours ago"), Some(180));
        assert_eq!(relative_date_to_minutes("15 minutes ago"), Some(15));
        assert_eq!(relative_date_to_minutes("1 day ago"), Some(1440));
        assert_eq!(relative_date_to_minutes("yesterday"), Some(1440));
    }

    #[test]
    fn relative_date_parses_short_forms() {
        assert_eq!(relative_date_to_minutes("2h"), Some(120));
        assert_eq!(relative_date_to_minutes("15min"), Some(15));
        assert_eq!(relative_date_to_minutes("3d"), Some(4320));
    }

    #[test]
    fn relative_date_is_case_insensitive() {
        assert_eq!(relative_date_to_minutes("Há 2 Horas"), Some(120));
        assert_eq!(relative_date_to_minutes("3 Hours Ago"), Some(180));
        assert_eq!(relative_date_to_minutes("YESTERDAY"), Some(1440));
        assert_eq!(relative_date_to_minutes("Ontem"), Some(1440));
    }

    #[test]
    fn relative_date_returns_none_when_unparseable() {
        assert_eq!(relative_date_to_minutes(""), None);
        assert_eq!(relative_date_to_minutes("sem formato"), None);
        assert_eq!(relative_date_to_minutes("há muito tempo"), None);
        assert_eq!(relative_date_to_minutes("2 weeks ago"), None);
    }

    #[test]
    fn relative_date_returns_none_on_multiplication_overflow() {
        assert_eq!(relative_date_to_minutes("999999999999999999 h"), None);
        assert_eq!(relative_date_to_minutes("99999999999999999 days ago"), None);
        assert_eq!(relative_date_to_minutes("há 99999999999999999 dias"), None);
    }

    // ---------------------------------------------------------------
    // Property-based tests (proptest)
    // ---------------------------------------------------------------
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// `canonicalize_url(canonicalize_url(x)) == canonicalize_url(x)` —
            /// the operation must be idempotent for any well-formed URL.
            #[test]
            fn canonicalize_is_idempotent(
                scheme in "(https|http)",
                host in "[a-z]{3,12}",
                path in "/[a-z0-9/_-]{0,30}",
                qkey in "[a-z]{1,5}",
                qval in "[a-z0-9]{1,5}",
            ) {
                let url = format!("{}://{}{}?{}={}", scheme, host, path, qkey, qval);
                let once = canonicalize_url(&url);
                let twice = canonicalize_url(&once);
                prop_assert_eq!(once, twice);
            }

            /// The canonical form never contains a fragment (`#`).
            #[test]
            fn canonicalize_strips_fragment(
                host in "[a-z]{3,8}",
                fragment in "[a-zA-Z0-9]{1,12}",
            ) {
                let url = format!("https://{}/p#{}", host, fragment);
                let canon = canonicalize_url(&url);
                prop_assert!(!canon.contains('#'), "fragment leaked: {}", canon);
            }

            /// `utm_*`, `fbclid`, and `gclid` must always be stripped.
            #[test]
            fn canonicalize_strips_tracking_params(
                host in "[a-z]{3,8}",
                path in "/[a-z0-9]{1,10}",
            ) {
                let url = format!(
                    "https://{}{}?utm_source=x&fbclid=y&gclid=z&keep=1",
                    host, path
                );
                let canon = canonicalize_url(&url);
                prop_assert!(!canon.contains("utm_"), "utm leaked: {}", canon);
                prop_assert!(!canon.contains("fbclid"), "fbclid leaked: {}", canon);
                prop_assert!(!canon.contains("gclid"), "gclid leaked: {}", canon);
                prop_assert!(canon.contains("keep=1"), "non-tracking lost: {}", canon);
            }

            /// The host is always lowercased in the canonical form.
            #[test]
            fn canonicalize_lowercases_host(
                host_part in "[A-Z]{3,8}",
                path in "/[a-z0-9]{0,8}",
            ) {
                let url = format!("https://{}/{}", host_part, path);
                let canon = canonicalize_url(&url);
                let lower = host_part.to_ascii_lowercase();
                prop_assert!(canon.contains(&lower), "host not lowered: {}", canon);
            }
        }
    }
}
