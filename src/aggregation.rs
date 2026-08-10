// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound / declarative (in-memory merge, no I/O).
// Cost is O(n log n) from sort after HashMap merge; n = total SERP rows across
// sub-queries (typically tens, not millions). No rayon: coordination overhead
// exceeds work; multi-query fan-out already parallelized in parallel.rs.
// Hash maps use std DefaultHasher (not ahash/FxHash): maps are tiny and cold
// relative to Chrome/network; switching hasher without profile is premature.
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
    pub url: crate::types::HttpUrl,
    /// Title (first non-empty across duplicates is kept).
    #[serde(rename = "title", alias = "titulo")]
    pub title: String,
    /// Display URL (optional, kept from the first occurrence).
    ///
    /// Serializes English per ADR-0027; the Portuguese spelling stays as a
    /// deserialize alias. Until v1.0.4 the `rename` here was the Portuguese
    /// `url_exibicao`, so the ENGLISH default wire emitted a Portuguese key
    /// while `deep-research-output.schema.json` declared the English one under
    /// `additionalProperties: false`.
    #[serde(
        rename = "display_url",
        alias = "url_exibicao",
        skip_serializing_if = "Option::is_none"
    )]
    pub display_url: Option<String>,
    /// Optional snippet (first non-empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// Score in `[0.0, 1.0]` (1.0 = best, 0.0 = irrelevant). Higher is better.
    pub score: f64,
    /// Original position in the source sub-query (1-indexed).
    #[serde(rename = "position", alias = "posicao")]
    pub position: u32,
    /// Texts of the sub-queries that produced this result.
    #[serde(rename = "sources", alias = "fontes")]
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
    let Ok(mut url) = Url::parse(raw) else {
        return raw.to_string();
    };
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
    // `to_hex()` is already a displayable hex buffer — take the first 16
    // chars without intermediate `String` allocations.
    hash.to_hex()[..16].to_owned()
}

/// Merges a list of per-sub-query `SearchOutput` into a single ranked list.
///
/// The order of `outputs` is the order in which the sub-queries were
/// dispatched; ranks inside each output are 1-indexed.
///
/// # Why this borrows, and why the clones inside are not waste
///
/// A 2026-08-10 memory audit counted 26 `clone` calls in this file and
/// proposed taking `outputs` by value to remove them. Measuring the caller
/// refuted it. In `crate::deep_research::run` the outputs live in an
/// `Arc<Vec<SearchOutput>>` that is:
///
/// 1. handed to [`aggregate`] and [`aggregate_news`] CONCURRENTLY, under
///    `tokio::join!`, so neither can take ownership;
/// 2. read again afterwards for `used_chrome` and `cascade_level`;
/// 3. extended with the next round's outputs when `--depth` is above zero.
///
/// Consuming by value would therefore force a clone of the entire vector at
/// the call site — every row of every sub-query — to remove per-field clones
/// of a subset of those same rows. The remaining clones are structural: the
/// input is borrowed and the output owns its strings, so somebody has to
/// allocate them exactly once, and that is what happens here.
pub fn aggregate(outputs: &[SearchOutput], strategy: AggregationStrategy) -> Vec<AggregatedItem> {
    match strategy {
        AggregationStrategy::Rrf(k) => rrf_aggregate(outputs, k),
        AggregationStrategy::DedupeByUrl => dedupe_by_url(outputs),
    }
}

/// Upper bound for unique web URLs: sum of per-query result rows.
#[inline]
fn estimated_web_rows(outputs: &[SearchOutput]) -> usize {
    outputs.iter().map(|o| o.results.len()).sum()
}

/// Upper bound for unique news URLs across sub-queries.
#[inline]
fn estimated_news_rows(outputs: &[SearchOutput]) -> usize {
    outputs
        .iter()
        .map(|o| o.news.as_ref().map_or(0, Vec::len))
        .sum()
}

fn rrf_aggregate(outputs: &[SearchOutput], k: u32) -> Vec<AggregatedItem> {
    use std::collections::HashMap;

    struct Entry {
        url: crate::types::HttpUrl,
        title: String,
        display_url: Option<String>,
        snippet: Option<String>,
        score: f64,
        position: u32,
        sources: Vec<String>,
    }

    // Pre-size: unique keys ≤ total rows (avoids rehash on small multi-query merges).
    let mut map: HashMap<String, Entry> = HashMap::with_capacity(estimated_web_rows(outputs));
    for output in outputs {
        for (idx, r) in output.results.iter().enumerate() {
            let key = canonical_hash(r.url.as_str());
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

    let mut out: Vec<AggregatedItem> = Vec::with_capacity(map.len());
    out.extend(map.into_values().map(|e| AggregatedItem {
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
    }));
    // Full total order: score ↓, position ↑, url ↑ — never rely on HashMap
    // iteration (rules-rust-cli-one-shot: deterministic stdout).
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.position.cmp(&b.position))
            .then_with(|| a.url.cmp(&b.url))
    });
    out
}

fn dedupe_by_url(outputs: &[SearchOutput]) -> Vec<AggregatedItem> {
    use std::collections::HashMap;

    let mut map: HashMap<String, AggregatedItem> =
        HashMap::with_capacity(estimated_web_rows(outputs));
    for output in outputs {
        for (idx, r) in output.results.iter().enumerate() {
            let key = canonical_hash(r.url.as_str());
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
    let mut out: Vec<AggregatedItem> = Vec::with_capacity(map.len());
    out.extend(map.into_values());
    // position ↑ then url ↑ — stable total order independent of HashMap.
    out.sort_by(|a, b| a.position.cmp(&b.position).then_with(|| a.url.cmp(&b.url)));
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
    #[serde(rename = "position", alias = "posicao")]
    pub position: usize,
    /// Headline (kept from the most recent exemplar across duplicates).
    #[serde(rename = "title", alias = "titulo")]
    pub title: String,
    /// Article URL (as returned by the upstream search).
    pub url: crate::types::HttpUrl,
    /// Publisher/source name (kept from the most recent exemplar).
    ///
    /// Serializes English per ADR-0027; Portuguese kept as deserialize alias.
    #[serde(
        rename = "source",
        alias = "fonte",
        skip_serializing_if = "Option::is_none"
    )]
    pub source: Option<String>,
    /// Relative timestamp, verbatim (kept from the most recent exemplar).
    ///
    /// Serializes English per ADR-0027; Portuguese kept as deserialize alias.
    #[serde(
        rename = "relative_date",
        alias = "data_relativa",
        skip_serializing_if = "Option::is_none"
    )]
    pub relative_date: Option<String>,
    /// Thumbnail URL (kept from the most recent exemplar).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    /// Score (RRF sum for [`AggregationStrategy::Rrf`], `1.0` for
    /// [`AggregationStrategy::DedupeByUrl`]). Higher is better.
    pub score: f64,
    /// Number of sub-queries in which this news item appeared.
    #[serde(rename = "occurrences", alias = "ocorrencias")]
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

    let mut map: HashMap<String, Entry> = HashMap::with_capacity(estimated_news_rows(outputs));
    let mut next_order = 0usize;
    for output in outputs {
        let Some(news) = output.news.as_ref() else {
            continue;
        };
        for n in news {
            let key = canonical_hash(n.url.as_str());
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

    let mut entries: Vec<Entry> = Vec::with_capacity(map.len());
    entries.extend(map.into_values());
    // score ↓, recency, first-seen order, url ↑ — no HashMap-order leakage.
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
            .then_with(|| a.item.url.cmp(&b.item.url))
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

    let mut map: HashMap<String, (u32, usize, AggregatedNewsItem)> =
        HashMap::with_capacity(estimated_news_rows(outputs));
    let mut next_order = 0usize;
    for output in outputs {
        let Some(news) = output.news.as_ref() else {
            continue;
        };
        for n in news {
            let key = canonical_hash(n.url.as_str());
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
    let mut entries: Vec<(u32, usize, AggregatedNewsItem)> = Vec::with_capacity(map.len());
    entries.extend(map.into_values());
    // position ↑, insertion order ↑, url ↑ — deterministic even with HashMap.
    entries.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.url.cmp(&b.2.url))
    });
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
#[path = "aggregation_tests.rs"]
mod tests;
