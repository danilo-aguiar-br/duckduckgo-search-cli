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
            Some((k, d)) if matches!(d.to_ascii_lowercase().as_str(), "asc" | "desc") => {
                (k, Some(d))
            }
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
    sort_rows(&mut output.results, *spec);
    if let Some(news) = output.news.as_mut() {
        sort_rows(news, *spec);
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
    sort_rows(&mut output.results, *spec);
    sort_rows(&mut output.news, *spec);
}

/// Dedupe web/news rows by canonical URL (first wins after current order).
pub fn apply_dedupe_search(output: &mut SearchOutput, dedupe: Option<DedupeBy>) {
    let Some(DedupeBy::Url) = dedupe else {
        return;
    };
    dedupe_rows_by_url(&mut output.results);
    output.result_count = output.results.len() as u32;
    if let Some(news) = output.news.as_mut() {
        dedupe_rows_by_url(news);
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
    dedupe_rows_by_url(&mut output.results);
    dedupe_rows_by_url(&mut output.news);
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

/// Shorten the synthesised report; aggregated rows carry no content field.
///
/// # The empty body this replaces, and why three audits kept missing the bug
///
/// This function was `{}` for three releases. Twice an audit flagged it as an
/// accepted-and-ignored flag, and twice the refutation was accepted — the third
/// time by me, on 2026-08-10, with the reasoning written into the doc that used
/// to live here.
///
/// That reasoning was true and incomplete, which is the worst combination. It
/// said: `--truncate-content` shortens the `content` string content fetch
/// attaches to a SERP row; deep-research emits
/// [`crate::aggregation::AggregatedItem`] and
/// [`crate::aggregation::AggregatedNewsItem`]; neither declares `content`;
/// therefore there is nothing to shorten. Every clause of that is correct.
///
/// It measured the ROWS. The envelope is not the rows. `DeepResearchOutput`
/// also carries `synth: Option<SynthesizedReport>`, and
/// [`crate::synthesis::SynthesizedReport::body`] is the single largest prose
/// blob this CLI produces — the whole synthesised report, in Markdown, plain
/// text or JSON. So `deep-research --synthesize --truncate-content 40` parsed,
/// validated, arrived here and returned the entire report, at exit 0, on
/// exactly the field an agent reaches for the flag to cap.
///
/// The guard that was supposed to keep the old doc honest,
/// `deep_aggregated_rows_have_no_content_field`, could not have caught this: it
/// asserts over the two row structs, which is precisely the scope whose
/// incompleteness caused the bug. A guard that keeps its own copy of the target
/// measures the copy. `deep_truncate_shortens_the_synthesised_report` covers
/// the envelope instead.
///
/// # Why `estimated_tokens` is recomputed
///
/// The field is documented as the token count OF THE BODY. Shortening the body
/// and leaving the count would emit an envelope that contradicts itself, and a
/// caller budgeting on that number would budget for text it did not receive.
pub fn apply_truncate_content_deep(output: &mut DeepResearchOutput, n: Option<usize>) {
    let Some(max) = n else {
        return;
    };
    if let Some(synth) = output.synth.as_mut() {
        truncate_in_place(&mut synth.body, max);
        synth.estimated_tokens = crate::synthesis::estimate_tokens(&synth.body);
    }
}

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
    let cut = crate::text::truncate_to_chars(s, max_chars).len();
    s.truncate(cut);
}

fn cmp_dir(ord: Ordering, dir: SortDir) -> Ordering {
    match dir {
        SortDir::Asc => ord,
        SortDir::Desc => ord.reverse(),
    }
}

/// What a row must expose for `--sort` and `--dedupe-by` to act on it.
///
/// # Why a trait replaced eight functions
///
/// There were four sorters and four dedupers here, one per row type, each a
/// near-verbatim copy of its siblings. That shape is what let the
/// `--truncate-content` defect survive: a matrix of near-identical cells makes
/// the one empty cell look like the others at a glance, and there is no
/// compiler check that a family of copies is complete.
///
/// The accessors answer `Option` where a row type genuinely has no such
/// column. A web result carries no score and no publisher, so it returns
/// `None` and those keys compare equal for it — which is exactly what the four
/// hand-written `SortKey::Score => Ordering::Equal` arms did.
trait SortableRow {
    /// Rank within its own list, widened so `u32` and `usize` rows compare alike.
    fn sort_position(&self) -> u64;
    /// Headline; compared case-insensitively.
    fn sort_title(&self) -> &str;
    /// Canonical URL — also the total-order tiebreak and the dedupe key.
    fn sort_url(&self) -> &str;
    /// Relevance, when the row has one.
    fn sort_score(&self) -> Option<f64> {
        None
    }
    /// Publisher, when the row has one.
    fn sort_source(&self) -> Option<&str> {
        None
    }
}

impl SortableRow for SearchResult {
    fn sort_position(&self) -> u64 {
        u64::from(self.position)
    }
    fn sort_title(&self) -> &str {
        &self.title
    }
    fn sort_url(&self) -> &str {
        self.url.as_str()
    }
}

impl SortableRow for NewsResult {
    fn sort_position(&self) -> u64 {
        u64::from(self.position)
    }
    fn sort_title(&self) -> &str {
        &self.title
    }
    fn sort_url(&self) -> &str {
        self.url.as_str()
    }
    fn sort_source(&self) -> Option<&str> {
        Some(self.source.as_deref().unwrap_or(""))
    }
}

impl SortableRow for AggregatedItem {
    fn sort_position(&self) -> u64 {
        u64::from(self.position)
    }
    fn sort_title(&self) -> &str {
        &self.title
    }
    fn sort_url(&self) -> &str {
        self.url.as_str()
    }
    fn sort_score(&self) -> Option<f64> {
        Some(self.score)
    }
}

impl SortableRow for AggregatedNewsItem {
    fn sort_position(&self) -> u64 {
        self.position as u64
    }
    fn sort_title(&self) -> &str {
        &self.title
    }
    fn sort_url(&self) -> &str {
        self.url.as_str()
    }
    fn sort_score(&self) -> Option<f64> {
        Some(self.score)
    }
    fn sort_source(&self) -> Option<&str> {
        Some(self.source.as_deref().unwrap_or(""))
    }
}

/// Sorts any row family by `spec`, with URL as the total-order tiebreak.
///
/// The tiebreak is not cosmetic: without it two rows equal on the chosen key
/// would keep whatever order the upstream SERP happened to return, and a
/// one-shot CLI must produce the same bytes for the same input.
fn sort_rows<T: SortableRow>(rows: &mut [T], spec: SortSpec) {
    rows.sort_by(|a, b| {
        let ord = match spec.key {
            SortKey::Position => a.sort_position().cmp(&b.sort_position()),
            SortKey::Title => a
                .sort_title()
                .to_ascii_lowercase()
                .cmp(&b.sort_title().to_ascii_lowercase()),
            SortKey::Url => a.sort_url().cmp(b.sort_url()),
            SortKey::Score => match (a.sort_score(), b.sort_score()) {
                (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
                _ => Ordering::Equal,
            },
            SortKey::Source => match (a.sort_source(), b.sort_source()) {
                (Some(x), Some(y)) => x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase()),
                _ => Ordering::Equal,
            },
        };
        cmp_dir(ord, spec.dir).then_with(|| a.sort_url().cmp(b.sort_url()))
    });
}

/// Drops later rows whose canonical URL was already seen.
fn dedupe_rows_by_url<T: SortableRow>(rows: &mut Vec<T>) {
    let mut seen = HashSet::new();
    rows.retain(|r| seen.insert(canonicalize_url(r.sort_url())));
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

    /// Pins one premise of [`super::apply_truncate_content_deep`]: rows are
    /// contentless, so truncation has nothing to do to THEM.
    ///
    /// # Read this together with the test below it
    ///
    /// This assertion used to be the whole defence of an empty
    /// `apply_truncate_content_deep`, and it was not enough — not because it
    /// was wrong, but because its scope is the two ROW structs while the flag's
    /// scope is the ENVELOPE. `synthesis.body` sat outside it and went
    /// untruncated for three releases. See
    /// `deep_truncate_shortens_the_synthesised_report`.
    ///
    /// Kept, not deleted: the day either row struct grows a `content` field,
    /// this fails and points at the function that must learn to shorten it.
    /// A narrow guard is useful as long as nobody mistakes it for a wide one.
    #[test]
    fn deep_aggregated_rows_have_no_content_field() {
        // Every optional field is populated on purpose. `skip_serializing_if`
        // hides a `None` from serialization, so a fixture that leaves optionals
        // empty makes the key invisible to every drift ruler in the suite —
        // which is exactly how three Portuguese `rename`s survived ADR-0027.
        let web = serde_json::to_value(crate::aggregation::AggregatedItem {
            url: crate::types::HttpUrl::for_test("https://e.example/"),
            title: "t".into(),
            display_url: Some("e.example".into()),
            snippet: Some("s".into()),
            score: 1.0,
            position: 1,
            sources: vec!["q".into()],
        })
        .expect("aggregated item serializes");
        let news = serde_json::to_value(crate::aggregation::AggregatedNewsItem {
            position: 1,
            title: "t".into(),
            url: crate::types::HttpUrl::for_test("https://e.example/"),
            source: Some("Publisher".into()),
            relative_date: Some("2h ago".into()),
            thumbnail: Some("https://e.example/t.png".into()),
            score: 1.0,
            occurrences: 1,
        })
        .expect("aggregated news item serializes");

        for (label, value) in [("AggregatedItem", &web), ("AggregatedNewsItem", &news)] {
            let obj = value.as_object().expect("row serializes to an object");
            assert!(
                !obj.contains_key("content"),
                "{label} grew a `content` field, so `apply_truncate_content_deep` is no \
                 longer a correct no-op — implement it, mirroring \
                 `apply_truncate_content_search`, and update its doc comment"
            );
            assert!(
                !obj.contains_key("content_size"),
                "{label} grew a `content_size` field — see `apply_truncate_content_deep`"
            );
        }
    }

    /// A minimal deep-research envelope carrying a synthesised report.
    fn deep_with_synthesis(body: &str) -> DeepResearchOutput {
        use crate::deep_research::DeepResearchMetadata;
        DeepResearchOutput {
            kind: crate::types::DeepResearchKind::DeepResearch,
            query: "q".into(),
            metadata: DeepResearchMetadata {
                original_query: "q".into(),
                sub_queries: vec![],
                aggregation_strategy: "rrf".into(),
                unique_result_count: 0,
                unique_news_count: 0,
                total_elapsed_ms: 1,
                cascade_level: None,
                used_chrome: true,
                chrome_path_resolved: None,
                chrome_channel: None,
                sub_queries_total: 0,
                sub_queries_ok: 0,
                sub_queries_error: 0,
                partial: false,
                chrome_contention_advisory: false,
            },
            results: vec![],
            news: vec![],
            news_count: 0,
            synth: Some(crate::synthesis::SynthesizedReport {
                format: crate::synthesis::SynthFormat::Markdown,
                body: body.to_string(),
                estimated_tokens: crate::synthesis::estimate_tokens(body),
                reference_count: 0,
            }),
        }
    }

    /// `--truncate-content` must shorten the synthesised report.
    ///
    /// # The measurement that reopened a class declared closed three times
    ///
    /// `apply_truncate_content_deep` was `{}` for three releases, defended by a
    /// doc arguing that deep-research rows carry no `content` field. That is
    /// true, and it is not the envelope. Measured live on v1.0.5 against a real
    /// run of `deep-research --synthesize --synth-format markdown`: the payload
    /// was 19007 bytes and `synthesis.body` alone was 7673 characters — about
    /// forty percent of everything the caller received, returned in full at
    /// exit 0 with `--truncate-content` set.
    ///
    /// The sibling guard `deep_aggregated_rows_have_no_content_field` asserts
    /// over the two row structs, so it could never have seen this: it holds its
    /// own copy of the target and measures the copy. This one asserts on the
    /// serialized envelope, which is what the caller actually reads.
    #[test]
    fn deep_truncate_shortens_the_synthesised_report() {
        let long = "a".repeat(500);
        let mut out = deep_with_synthesis(&long);
        apply_truncate_content_deep(&mut out, Some(40));

        let synth = out.synth.as_ref().expect("synthesis survives truncation");
        assert_eq!(
            synth.body.chars().count(),
            40,
            "the report body was not capped at the requested scalar count"
        );
        assert_eq!(
            synth.estimated_tokens,
            crate::synthesis::estimate_tokens(&synth.body),
            "`estimated_tokens` is documented as the count OF THE BODY; leaving \
             the pre-truncation value emits an envelope that contradicts itself \
             and makes a caller budget for text it never received"
        );

        // The whole point is fewer bytes on the wire, not a shorter field in
        // memory. Assert on the serialized form the caller parses.
        let before = serde_json::to_string(&deep_with_synthesis(&long)).expect("serializes");
        let after = serde_json::to_string(&out).expect("serializes");
        assert!(
            after.len() < before.len(),
            "truncation did not shrink the emitted envelope: {} bytes before, \
             {} bytes after",
            before.len(),
            after.len()
        );
    }

    /// EVERY truncate entry point must shrink the SERIALIZED envelope.
    ///
    /// # The ruler the plan asked for and the last round did not build
    ///
    /// `apply_truncate_content_deep` shipped with an empty body defended by a
    /// thirty-line doc explaining why it had nothing to do. The offline matrix
    /// in `tests/integration_agent_ops_matrix.rs` measures exactly this, but
    /// only for surfaces reachable without Chrome — and `deep-research` is not
    /// one of them, which is precisely why the empty body survived three
    /// reviews.
    ///
    /// So the guard is repeated here at the type level, where every family is
    /// reachable. A new envelope type gets an `apply_truncate_content_*`
    /// function; if that function does nothing, this fails on the byte count
    /// rather than on someone noticing the body is empty.
    #[test]
    fn every_truncate_entry_point_shrinks_its_serialized_envelope() {
        let long = "z".repeat(500);

        // Single-query search: content lives on the web rows.
        let mut single = out(vec![web("t", "https://a.example/", 1)]);
        single.results[0].content = Some(long.clone());
        let before = serde_json::to_string(&single).expect("serializes");
        apply_truncate_content_search(&mut single, Some(40));
        let after = serde_json::to_string(&single).expect("serializes");
        assert!(
            after.len() < before.len(),
            "apply_truncate_content_search left the envelope at {} bytes",
            after.len()
        );

        // Multi-query: the same rows, one level down.
        let mut inner = out(vec![web("t", "https://a.example/", 1)]);
        inner.results[0].content = Some(long.clone());
        let mut multi = MultiSearchOutput {
            query_count: 1,
            timestamp: crate::types::test_timestamp(),
            parallelism: 1,
            searches: vec![inner],
            zero_cause_histogram: std::collections::BTreeMap::new(),
        };
        let before = serde_json::to_string(&multi).expect("serializes");
        apply_truncate_content_multi(&mut multi, Some(40));
        let after = serde_json::to_string(&multi).expect("serializes");
        assert!(
            after.len() < before.len(),
            "apply_truncate_content_multi left the envelope at {} bytes",
            after.len()
        );

        // Deep research: content lives in the synthesised report, not the rows.
        let mut deep = deep_with_synthesis(&long);
        let before = serde_json::to_string(&deep).expect("serializes");
        apply_truncate_content_deep(&mut deep, Some(40));
        let after = serde_json::to_string(&deep).expect("serializes");
        assert!(
            after.len() < before.len(),
            "apply_truncate_content_deep left the envelope at {} bytes",
            after.len()
        );

        // Pipeline: the dispatcher over the two search families.
        let mut piped = out(vec![web("t", "https://a.example/", 1)]);
        piped.results[0].content = Some(long);
        let mut result = crate::pipeline::PipelineResult::Single(Box::new(piped));
        let before = match &result {
            crate::pipeline::PipelineResult::Single(s) => {
                serde_json::to_string(s).expect("serializes")
            }
            _ => unreachable!("constructed as Single"),
        };
        apply_truncate_pipeline(&mut result, Some(40));
        let after = match &result {
            crate::pipeline::PipelineResult::Single(s) => {
                serde_json::to_string(s).expect("serializes")
            }
            _ => unreachable!("constructed as Single"),
        };
        assert!(
            after.len() < before.len(),
            "apply_truncate_pipeline left the envelope at {} bytes",
            after.len()
        );
    }

    /// Truncation must leave an envelope with no synthesis untouched.
    ///
    /// Guards the other direction: the fix must not invent a report, nor panic
    /// on the common case of `deep-research` without `--synthesize`.
    #[test]
    fn deep_truncate_is_inert_without_a_synthesised_report() {
        let mut out = deep_with_synthesis("body");
        out.synth = None;
        let before = serde_json::to_string(&out).expect("serializes");
        apply_truncate_content_deep(&mut out, Some(1));
        let after = serde_json::to_string(&out).expect("serializes");
        assert_eq!(
            before, after,
            "an envelope without a synthesised report must round-trip unchanged"
        );
    }

    /// ADR-0027: domain types serialize ENGLISH keys; PT is a remap at emit.
    ///
    /// Three optional fields of the aggregated rows kept a Portuguese `rename`
    /// through the whole v1.0.2 migration. Nothing caught it because
    /// `skip_serializing_if` hid them whenever the fixture left them `None`,
    /// and every fixture did. The English default wire therefore emitted
    /// `url_exibicao`, `fonte` and `data_relativa`, while
    /// `deep-research-output.schema.json` declared the English spellings under
    /// `additionalProperties: false` — so a real news row failed the contract
    /// the product publishes for it.
    #[test]
    fn aggregated_rows_serialize_english_keys_only() {
        let web = serde_json::to_value(crate::aggregation::AggregatedItem {
            url: crate::types::HttpUrl::for_test("https://e.example/"),
            title: "t".into(),
            display_url: Some("e.example".into()),
            snippet: Some("s".into()),
            score: 1.0,
            position: 1,
            sources: vec!["q".into()],
        })
        .expect("aggregated item serializes");
        let news = serde_json::to_value(crate::aggregation::AggregatedNewsItem {
            position: 1,
            title: "t".into(),
            url: crate::types::HttpUrl::for_test("https://e.example/"),
            source: Some("Publisher".into()),
            relative_date: Some("2h ago".into()),
            thumbnail: Some("https://e.example/t.png".into()),
            score: 1.0,
            occurrences: 1,
        })
        .expect("aggregated news item serializes");

        // Any key that appears as a PT spelling in the EN→PT table must never
        // be emitted by a domain type: the remap walk only runs for PT.
        for (label, value) in [("AggregatedItem", &web), ("AggregatedNewsItem", &news)] {
            let obj = value.as_object().expect("row serializes to an object");
            for key in obj.keys() {
                assert!(
                    !super::super::wire_keys::is_portuguese_wire_key(key),
                    "{label} serializes `{key}`, a Portuguese wire spelling. ADR-0027 \
                     requires domain types to emit English; PT is applied once at the \
                     emit boundary by `output::wire_keys`."
                );
            }
        }

        let web_obj = web.as_object().expect("object");
        assert!(web_obj.contains_key("display_url"));
        let news_obj = news.as_object().expect("object");
        assert!(news_obj.contains_key("source"));
        assert!(news_obj.contains_key("relative_date"));
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
