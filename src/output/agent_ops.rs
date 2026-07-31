// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light pure transform — agent-native sort/dedupe/count/truncate.
//! Post-SERP agent data ops (sort, dedupe, count-only, content truncate).
//!
//! # Pipeline order (SSOT)
//!
//! `filter → sort → dedupe → project(fields) → limit → truncate-content → emit`
//!
//! When `--count-only` is set, rows are not emitted; stdout is a compact
//! English JSON object with counts after filter/sort/dedupe (not after limit
//! unless `--limit` also applied — count uses the final row set after limit).
//!
//! # Agent-native contract
//!
//! The binary performs these ops so the LLM never needs `jq`/`sed` for
//! ordering, URL dedupe, or counting (G9/G10/G13).

use crate::aggregation::{canonicalize_url, AggregatedItem, AggregatedNewsItem};
use crate::deep_research::DeepResearchOutput;
use crate::error::CliError;
use crate::types::{MultiSearchOutput, NewsResult, SearchOutput, SearchResult};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

/// Process-wide stdout payload cap (`0` = unlimited). Set from CLI/XDG once per run.
static MAX_OUTPUT_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Install `--max-output-bytes` / XDG cap for this process (call after clap+XDG resolve).
pub fn set_process_max_output_bytes(n: Option<u64>) {
    let v = n.map(|x| x as usize).unwrap_or(0);
    MAX_OUTPUT_BYTES.store(v, AtomicOrdering::Relaxed);
}

/// Current process cap (`None` if unlimited).
#[must_use]
pub fn process_max_output_bytes() -> Option<usize> {
    let v = MAX_OUTPUT_BYTES.load(AtomicOrdering::Relaxed);
    if v == 0 {
        None
    } else {
        Some(v)
    }
}


/// Sort direction for [`SortSpec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    /// Ascending (default for position/title/url/source).
    Asc,
    /// Descending (default for score).
    Desc,
}

/// Allowlisted sort keys (agent-native `--sort`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    /// SERP / RRF position.
    Position,
    /// Result title.
    Title,
    /// Result URL string.
    Url,
    /// Aggregated RRF score (deep-research); 0 for plain SERP.
    Score,
    /// News source name.
    Source,
}

/// Parsed `--sort KEY[:asc|desc]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortSpec {
    /// Field to compare.
    pub key: SortKey,
    /// Ascending or descending.
    pub dir: SortDir,
}

impl SortSpec {
    /// Parse `position`, `title:desc`, `score`, `posicao:asc`, …
    ///
    /// # Errors
    ///
    /// Unknown key/dir → [`CliError::InvalidConfig`].
    pub fn parse(raw: &str) -> Result<Self, CliError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(CliError::InvalidConfig {
                message: "--sort must be non-empty (e.g. title, score:desc, position:asc)".into(),
            });
        }
        let (key_raw, dir_raw) = match raw.rsplit_once(':') {
            Some((k, d)) if matches!(d.to_ascii_lowercase().as_str(), "asc" | "desc") => (k, Some(d)),
            _ => (raw, None),
        };
        let key = match key_raw.trim().to_ascii_lowercase().as_str() {
            "position" | "posicao" => SortKey::Position,
            "title" | "titulo" => SortKey::Title,
            "url" => SortKey::Url,
            "score" => SortKey::Score,
            "source" | "fonte" => SortKey::Source,
            other => {
                return Err(CliError::InvalidConfig {
                    message: format!(
                        "unknown --sort key {other:?}; allowlist: position|title|url|score|source \
                         (PT aliases: posicao|titulo|fonte)"
                    ),
                });
            }
        };
        let dir = match dir_raw.map(|d| d.to_ascii_lowercase()) {
            Some(d) if d == "asc" => SortDir::Asc,
            Some(d) if d == "desc" => SortDir::Desc,
            Some(d) => {
                return Err(CliError::InvalidConfig {
                    message: format!("unknown --sort direction {d:?}; use asc|desc"),
                });
            }
            None => match key {
                SortKey::Score => SortDir::Desc,
                _ => SortDir::Asc,
            },
        };
        Ok(Self { key, dir })
    }
}

/// `--dedupe-by` target (v2: `url` only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupeBy {
    /// Canonical URL (tracking params stripped).
    Url,
}

impl DedupeBy {
    /// Parse `url` (fail-closed on anything else).
    ///
    /// # Errors
    ///
    /// Unknown target → [`CliError::InvalidConfig`].
    pub fn parse(raw: &str) -> Result<Self, CliError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "url" => Ok(Self::Url),
            other => Err(CliError::InvalidConfig {
                message: format!(
                    "unknown --dedupe-by {other:?}; only `url` is supported in v2.0.0"
                ),
            }),
        }
    }
}

/// Sort web/news rows of a single-query envelope.
pub fn apply_sort_search(output: &mut SearchOutput, sort: Option<&SortSpec>) {
    let Some(spec) = sort else {
        return;
    };
    sort_web_results(&mut output.results, *spec);
    if let Some(news) = output.news.as_mut() {
        sort_news_results(news, *spec);
    }
}

/// Sort every multi-query search.
pub fn apply_sort_multi(output: &mut MultiSearchOutput, sort: Option<&SortSpec>) {
    for search in &mut output.searches {
        apply_sort_search(search, sort);
    }
}

/// Sort deep-research aggregated web/news rows.
pub fn apply_sort_deep(output: &mut DeepResearchOutput, sort: Option<&SortSpec>) {
    let Some(spec) = sort else {
        return;
    };
    sort_aggregated(&mut output.results, *spec);
    sort_aggregated_news(&mut output.news, *spec);
}

/// Dedupe web/news rows by canonical URL (first wins after current order).
pub fn apply_dedupe_search(output: &mut SearchOutput, dedupe: Option<DedupeBy>) {
    let Some(DedupeBy::Url) = dedupe else {
        return;
    };
    dedupe_web_by_url(&mut output.results);
    output.result_count = output.results.len() as u32;
    if let Some(news) = output.news.as_mut() {
        dedupe_news_by_url(news);
        output.news_count = Some(news.len() as u32);
    }
}

/// Dedupe every multi-query search.
pub fn apply_dedupe_multi(output: &mut MultiSearchOutput, dedupe: Option<DedupeBy>) {
    for search in &mut output.searches {
        apply_dedupe_search(search, dedupe);
    }
}

/// Dedupe deep-research aggregated rows by canonical URL.
pub fn apply_dedupe_deep(output: &mut DeepResearchOutput, dedupe: Option<DedupeBy>) {
    let Some(DedupeBy::Url) = dedupe else {
        return;
    };
    dedupe_aggregated_by_url(&mut output.results);
    dedupe_aggregated_news_by_url(&mut output.news);
    output.news_count = output.news.len();
    output.metadata.unique_result_count = output.results.len();
    output.metadata.unique_news_count = output.news.len();
}

/// Truncate `content` fields to at most `n` chars (Unicode scalar safe).
pub fn apply_truncate_content_search(output: &mut SearchOutput, n: Option<usize>) {
    let Some(max) = n else {
        return;
    };
    for r in &mut output.results {
        if let Some(ref mut c) = r.content {
            truncate_in_place(c, max);
            r.content_size = Some(c.chars().count() as u32);
        }
    }
    if let Some(news) = output.news.as_mut() {
        for r in news {
            if let Some(ref mut c) = r.content {
                truncate_in_place(c, max);
                r.content_size = Some(c.chars().count());
            }
        }
    }
}

/// Truncate content on every multi-query search.
pub fn apply_truncate_content_multi(output: &mut MultiSearchOutput, n: Option<usize>) {
    for search in &mut output.searches {
        apply_truncate_content_search(search, n);
    }
}

/// Truncate is a no-op on deep aggregated rows (no content field today).
pub fn apply_truncate_content_deep(_output: &mut DeepResearchOutput, _n: Option<usize>) {}

/// Compact EN count payload for `--count-only` (single search).
#[must_use]
pub fn count_only_search(output: &SearchOutput) -> String {
    let web = output.results.len() as u64;
    let news = output.news.as_ref().map_or(0u64, |n| n.len() as u64);
    serde_json::json!({
        "count": web.saturating_add(news),
        "web": web,
        "news": news,
    })
    .to_string()
}

/// Compact EN count payload for multi-query `--count-only`.
#[must_use]
pub fn count_only_multi(output: &MultiSearchOutput) -> String {
    let mut web = 0u64;
    let mut news = 0u64;
    for s in &output.searches {
        web = web.saturating_add(s.results.len() as u64);
        news = news.saturating_add(s.news.as_ref().map_or(0, |n| n.len() as u64));
    }
    serde_json::json!({
        "count": web.saturating_add(news),
        "web": web,
        "news": news,
        "searches": output.searches.len() as u64,
    })
    .to_string()
}

/// Compact EN count payload for deep-research `--count-only`.
#[must_use]
pub fn count_only_deep(output: &DeepResearchOutput) -> String {
    let web = output.results.len() as u64;
    let news = output.news.len() as u64;
    serde_json::json!({
        "count": web.saturating_add(news),
        "web": web,
        "news": news,
    })
    .to_string()
}

/// Fail-closed when payload exceeds `--max-output-bytes` / XDG cap.
///
/// # Errors
///
/// Oversized payload → [`CliError::InvalidConfig`].
pub fn enforce_max_output_bytes(payload: &str, max: Option<usize>) -> Result<(), CliError> {
    let Some(limit) = max else {
        return Ok(());
    };
    if limit == 0 {
        return Err(CliError::InvalidConfig {
            message: "--max-output-bytes must be >= 1 when set".into(),
        });
    }
    let n = payload.len();
    if n > limit {
        return Err(CliError::InvalidConfig {
            message: format!(
                "output payload is {n} bytes; exceeds --max-output-bytes {limit} \
                 (raise the cap, use --fields/--limit/--count-only, or --truncate-content)"
            ),
        });
    }
    Ok(())
}

fn truncate_in_place(s: &mut String, max_chars: usize) {
    if s.chars().count() <= max_chars {
        return;
    }
    let cut = s
        .char_indices()
        .nth(max_chars)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    s.truncate(cut);
}

fn cmp_dir(ord: Ordering, dir: SortDir) -> Ordering {
    match dir {
        SortDir::Asc => ord,
        SortDir::Desc => ord.reverse(),
    }
}

fn sort_web_results(rows: &mut [SearchResult], spec: SortSpec) {
    rows.sort_by(|a, b| {
        let ord = match spec.key {
            SortKey::Position => a.position.cmp(&b.position),
            SortKey::Title => a.title.to_ascii_lowercase().cmp(&b.title.to_ascii_lowercase()),
            SortKey::Url => a.url.as_str().cmp(b.url.as_str()),
            SortKey::Score => Ordering::Equal,
            SortKey::Source => Ordering::Equal,
        };
        cmp_dir(ord, spec.dir).then_with(|| a.url.as_str().cmp(b.url.as_str()))
    });
}

fn sort_news_results(rows: &mut [NewsResult], spec: SortSpec) {
    rows.sort_by(|a, b| {
        let ord = match spec.key {
            SortKey::Position => a.position.cmp(&b.position),
            SortKey::Title => a.title.to_ascii_lowercase().cmp(&b.title.to_ascii_lowercase()),
            SortKey::Url => a.url.as_str().cmp(b.url.as_str()),
            SortKey::Score => Ordering::Equal,
            SortKey::Source => a
                .source
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase()
                .cmp(&b.source.as_deref().unwrap_or("").to_ascii_lowercase()),
        };
        cmp_dir(ord, spec.dir).then_with(|| a.url.as_str().cmp(b.url.as_str()))
    });
}

fn sort_aggregated(rows: &mut [AggregatedItem], spec: SortSpec) {
    rows.sort_by(|a, b| {
        let ord = match spec.key {
            SortKey::Position => a.position.cmp(&b.position),
            SortKey::Title => a.title.to_ascii_lowercase().cmp(&b.title.to_ascii_lowercase()),
            SortKey::Url => a.url.as_str().cmp(b.url.as_str()),
            SortKey::Score => a
                .score
                .partial_cmp(&b.score)
                .unwrap_or(Ordering::Equal),
            SortKey::Source => Ordering::Equal,
        };
        cmp_dir(ord, spec.dir).then_with(|| a.url.as_str().cmp(b.url.as_str()))
    });
}

fn sort_aggregated_news(rows: &mut [AggregatedNewsItem], spec: SortSpec) {
    rows.sort_by(|a, b| {
        let ord = match spec.key {
            SortKey::Position => a.position.cmp(&b.position),
            SortKey::Title => a.title.to_ascii_lowercase().cmp(&b.title.to_ascii_lowercase()),
            SortKey::Url => a.url.as_str().cmp(b.url.as_str()),
            SortKey::Score => a
                .score
                .partial_cmp(&b.score)
                .unwrap_or(Ordering::Equal),
            SortKey::Source => a
                .source
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase()
                .cmp(&b.source.as_deref().unwrap_or("").to_ascii_lowercase()),
        };
        cmp_dir(ord, spec.dir).then_with(|| a.url.as_str().cmp(b.url.as_str()))
    });
}

fn dedupe_web_by_url(rows: &mut Vec<SearchResult>) {
    let mut seen = HashSet::new();
    rows.retain(|r| seen.insert(canonicalize_url(r.url.as_str())));
}

fn dedupe_news_by_url(rows: &mut Vec<NewsResult>) {
    let mut seen = HashSet::new();
    rows.retain(|r| seen.insert(canonicalize_url(r.url.as_str())));
}

fn dedupe_aggregated_by_url(rows: &mut Vec<AggregatedItem>) {
    let mut seen = HashSet::new();
    rows.retain(|r| seen.insert(canonicalize_url(r.url.as_str())));
}

fn dedupe_aggregated_news_by_url(rows: &mut Vec<AggregatedNewsItem>) {
    let mut seen = HashSet::new();
    rows.retain(|r| seen.insert(canonicalize_url(r.url.as_str())));
}


// ── PipelineResult adapters (keep lib.rs thin) ─────────────────────────────
use crate::pipeline::PipelineResult;

/// Parse optional `--sort` raw string.
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when the operation fails.
pub fn parse_sort_opt(raw: Option<&str>) -> Result<Option<SortSpec>, CliError> {
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(SortSpec::parse(s)?)),
    }
}

/// Parse optional `--dedupe-by` raw string.
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when the dedupe key is unknown.
pub fn parse_dedupe_opt(raw: Option<&str>) -> Result<Option<DedupeBy>, CliError> {
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(DedupeBy::parse(s)?)),
    }
}

/// Sort all variants of a pipeline result.
pub fn apply_sort_pipeline(output: &mut PipelineResult, sort: Option<&SortSpec>) {
    match output {
        PipelineResult::Single(s) => apply_sort_search(s, sort),
        PipelineResult::Multi(m) => apply_sort_multi(m, sort),
        PipelineResult::Stream(_) => {}
    }
}

/// Dedupe all variants of a pipeline result.
pub fn apply_dedupe_pipeline(output: &mut PipelineResult, dedupe: Option<DedupeBy>) {
    match output {
        PipelineResult::Single(s) => apply_dedupe_search(s, dedupe),
        PipelineResult::Multi(m) => apply_dedupe_multi(m, dedupe),
        PipelineResult::Stream(_) => {}
    }
}

/// Truncate content on pipeline result rows.
pub fn apply_truncate_pipeline(output: &mut PipelineResult, n: Option<usize>) {
    match output {
        PipelineResult::Single(s) => apply_truncate_content_search(s, n),
        PipelineResult::Multi(m) => apply_truncate_content_multi(m, n),
        PipelineResult::Stream(_) => {}
    }
}

/// Compact count-only payload for a pipeline result.
#[must_use]
pub fn count_only_pipeline(output: &PipelineResult) -> String {
    match output {
        PipelineResult::Single(s) => count_only_search(s),
        PipelineResult::Multi(m) => count_only_multi(m),
        PipelineResult::Stream(_) => serde_json::json!({"count":0,"web":0,"news":0}).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{HttpUrl, SearchMetadata};

    fn web(title: &str, url: &str, pos: u32) -> SearchResult {
        SearchResult {
            position: pos,
            title: title.into(),
            url: HttpUrl::try_new(url).expect("url"),
            display_url: None,
            snippet: None,
            original_title: None,
            content: Some("abcdefghij".into()),
            content_size: Some(10),
            content_extraction_method: None,
        }
    }

    fn out(results: Vec<SearchResult>) -> SearchOutput {
        SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: crate::types::test_timestamp(),
            region: "us-en".into(),
            result_count: results.len() as u32,
            results,
            pages_fetched: 1,
            error: None,
            message: None,
            metadata: SearchMetadata {
                selectors_hash: "x".into(),
                user_agent: "ua".into(),
                ..SearchMetadata::default()
            },
            news: None,
            news_count: None,
        }
    }

    #[test]
    fn parse_sort_defaults_and_aliases() {
        let s = SortSpec::parse("title").unwrap();
        assert_eq!(s.key, SortKey::Title);
        assert_eq!(s.dir, SortDir::Asc);
        let s = SortSpec::parse("score").unwrap();
        assert_eq!(s.dir, SortDir::Desc);
        let s = SortSpec::parse("titulo:desc").unwrap();
        assert_eq!(s.key, SortKey::Title);
        assert_eq!(s.dir, SortDir::Desc);
        assert!(SortSpec::parse("nope").is_err());
    }

    #[test]
    fn sort_title_asc() {
        let mut o = out(vec![
            web("Charlie", "https://c.example/", 1),
            web("Alpha", "https://a.example/", 2),
            web("Bravo", "https://b.example/", 3),
        ]);
        apply_sort_search(&mut o, Some(&SortSpec::parse("title").unwrap()));
        assert_eq!(o.results[0].title, "Alpha");
        assert_eq!(o.results[1].title, "Bravo");
        assert_eq!(o.results[2].title, "Charlie");
    }

    #[test]
    fn dedupe_url_canonical() {
        let mut o = out(vec![
            web("A", "https://example.com/p?utm_source=x&id=1", 1),
            web("B", "https://example.com/p?id=1", 2),
            web("C", "https://other.example/", 3),
        ]);
        apply_dedupe_search(&mut o, Some(DedupeBy::Url));
        assert_eq!(o.results.len(), 2);
        assert_eq!(o.result_count, 2);
        assert_eq!(o.results[0].title, "A");
    }

    #[test]
    fn truncate_content_chars() {
        let mut o = out(vec![web("T", "https://t.example/", 1)]);
        apply_truncate_content_search(&mut o, Some(4));
        assert_eq!(o.results[0].content.as_deref(), Some("abcd"));
        assert_eq!(o.results[0].content_size, Some(4));
    }

    #[test]
    fn count_only_shape_en() {
        let o = out(vec![
            web("A", "https://a.example/", 1),
            web("B", "https://b.example/", 2),
        ]);
        let json = count_only_search(&o);
        assert!(json.contains("\"count\":2"));
        assert!(json.contains("\"web\":2"));
        assert!(json.contains("\"news\":0"));
        assert!(!json.contains("resultados"));
    }

    #[test]
    fn max_output_bytes_enforced() {
        assert!(enforce_max_output_bytes("hello", Some(10)).is_ok());
        assert!(enforce_max_output_bytes("hello world", Some(5)).is_err());
    }
}

