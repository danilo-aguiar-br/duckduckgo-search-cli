// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light pure transform (no I/O) — agent-native field projection + filter.
//! Post-pipeline projection and filter for structured SERP / deep-research output
//! (agent token budget).
//!
//! # Agent-native contract (GAP-FIELDS-PROJECT / GAP-RESULT-FILTER / GAP-DEEP-PROJECT-FILTER)
//!
//! The binary projects and filters **before** stdout so the LLM never needs
//! `jaq`/`sed` to drop dead fields or keep only URLs.
//!
//! - [`FieldSet`]: allowlisted wire keys (PT serialize names + EN aliases).
//! - [`ResultFilter`]: substring / host match on title, url, snippet.
//! - Content keys (`conteudo`, …) drive skip-fetch when omitted from `--fields`.
//! - Deep-research: [`apply_to_deep_output`] + [`format_deep_json_projected`]
//!   cover RRF rows (`score`, `fontes`, …) on the final envelope.

use crate::aggregation::{AggregatedItem, AggregatedNewsItem};
use crate::deep_research::DeepResearchOutput;
use crate::error::CliError;
use crate::types::{MultiSearchOutput, NewsResult, SearchOutput, SearchResult};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::collections::BTreeSet;

/// Canonical wire keys that count as page body content (skip-fetch when absent).
const CONTENT_KEYS: &[&str] = &["conteudo", "tamanho_conteudo", "metodo_extracao_conteudo"];

/// Allowlisted result-row keys (PT wire) plus accepted EN aliases at parse time.
///
/// Includes deep-research aggregated keys (`score`, `fontes`, `ocorrencias`) so
/// agents can project RRF rows without `jaq` (GAP-DEEP-PROJECT-FILTER).
const RESULT_KEYS: &[(&str, &str)] = &[
    ("posicao", "position"),
    ("titulo", "title"),
    ("url", "url"),
    ("url_exibicao", "display_url"),
    ("snippet", "snippet"),
    ("titulo_original", "original_title"),
    ("conteudo", "content"),
    ("tamanho_conteudo", "content_size"),
    ("metodo_extracao_conteudo", "content_extraction_method"),
    // news-only
    ("fonte", "source"),
    ("data_relativa", "relative_date"),
    ("thumbnail", "thumbnail"),
    // deep-research aggregated rows
    ("score", "score"),
    ("fontes", "sources"),
    ("ocorrencias", "occurrences"),
];

/// Selected result fields after `--fields` parse (ordered for TSV headers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSet {
    /// Canonical PT wire keys in request order (deduped).
    keys: Vec<String>,
    set: BTreeSet<String>,
}

impl FieldSet {
    /// Parse a comma-separated list (`url,titulo` or `url,title`).
    ///
    /// # Errors
    ///
    /// Empty list or unknown field name → [`CliError::InvalidConfig`].
    pub fn parse(raw: &str) -> Result<Self, CliError> {
        let mut keys = Vec::with_capacity(8);
        let mut set = BTreeSet::new();
        for part in raw.split(',') {
            let token = part.trim();
            if token.is_empty() {
                continue;
            }
            let canon = canonicalize_field(token).ok_or_else(|| CliError::InvalidConfig {
                message: format!(
                    "unknown --fields entry {token:?}; allowlist: {}",
                    allowlist_help()
                ),
            })?;
            if set.insert(canon.clone()) {
                keys.push(canon);
            }
        }
        if keys.is_empty() {
            return Err(CliError::InvalidConfig {
                message: "--fields must list at least one field (e.g. url,titulo,snippet)".into(),
            });
        }
        Ok(Self { keys, set })
    }

    /// Ordered canonical keys (TSV column order).
    #[must_use]
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// `true` when the set requests page body content fields.
    #[must_use]
    pub fn requests_content(&self) -> bool {
        CONTENT_KEYS.iter().any(|k| self.set.contains(*k))
    }

    /// `true` when `key` (canonical PT) is selected.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.set.contains(key)
    }
}

/// Post-SERP result filter (`--filter`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultFilter {
    /// Case-insensitive substring in title, url, or snippet.
    Contains(String),
    /// Case-insensitive substring in title only.
    TitleContains(String),
    /// Case-insensitive substring in url only.
    UrlContains(String),
    /// Case-insensitive substring in snippet only.
    SnippetContains(String),
    /// Host/domain contains (matched against result URL host).
    HostContains(String),
}

impl ResultFilter {
    /// Parse filter expression (fail-fast; never spends Chrome on typos).
    ///
    /// Supported:
    /// - `titulo~foo` / `title~foo`
    /// - `url~bar`
    /// - `snippet~baz`
    /// - `host:example.com`
    /// - bare `foo` → title|url|snippet contains
    ///
    /// Rejected (exit 2 / [`CliError::InvalidConfig`]):
    /// - `titulo~=foo` (SQL/jaq-style) — use `titulo~rust`
    /// - `posicao=1` / `field=value` — equality is not supported; use `field~needle`
    /// - empty expression / empty needle / unknown field name
    ///
    /// # Errors
    ///
    /// Invalid or empty expression → [`CliError::InvalidConfig`].
    pub fn parse(raw: &str) -> Result<Self, CliError> {
        let s = raw.trim();
        if s.is_empty() {
            return Err(CliError::InvalidConfig {
                message: "--filter must be non-empty (e.g. titulo~rust, host:example.com)".into(),
            });
        }
        // GAP-E2E-V19-FILTER-SYNTAX-FOOTGUN: `~=` is a common typo that used to
        // parse as field~ with needle starting with `=`, then waste a SERP and
        // collapse to exit 5 (empty index lie).
        if s.contains("~=") {
            return Err(CliError::InvalidConfig {
                message: "--filter: use field~needle (e.g. titulo~rust), not field~=needle".into(),
            });
        }
        if let Some(rest) = s.strip_prefix("host:") {
            let h = rest.trim();
            if h.is_empty() {
                return Err(CliError::InvalidConfig {
                    message: "--filter host: requires a non-empty host fragment".into(),
                });
            }
            return Ok(Self::HostContains(h.to_ascii_lowercase()));
        }
        if let Some((left, right)) = s.split_once('~') {
            let field = left.trim().to_ascii_lowercase();
            let needle = right.trim();
            if field.is_empty() {
                return Err(CliError::InvalidConfig {
                    message: "--filter: missing field before ~ (e.g. titulo~rust)".into(),
                });
            }
            if needle.is_empty() {
                return Err(CliError::InvalidConfig {
                    message: format!("--filter {field}~ requires a non-empty needle"),
                });
            }
            // Reject accidental `titulo~=x` if the ~= check above was bypassed.
            let needle = needle.trim_start_matches('=').trim();
            if needle.is_empty() || right.trim().starts_with('=') {
                return Err(CliError::InvalidConfig {
                    message: format!(
                        "--filter: use {field}~needle (not {field}~=…); needle must be non-empty"
                    ),
                });
            }
            let needle = needle.to_ascii_lowercase();
            return match field.as_str() {
                "titulo" | "title" => Ok(Self::TitleContains(needle)),
                "url" => Ok(Self::UrlContains(needle)),
                "snippet" => Ok(Self::SnippetContains(needle)),
                other => Err(CliError::InvalidConfig {
                    message: format!(
                        "unknown --filter field {other:?}; use titulo~, url~, snippet~, or host:"
                    ),
                }),
            };
        }
        // `field=value` (no ~) is not a supported operator — fail closed so
        // `posicao=1` does not become a bare substring search that burns Chrome.
        if let Some((left, right)) = s.split_once('=') {
            let field = left.trim();
            let value = right.trim();
            if !field.is_empty()
                && !value.is_empty()
                && field
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Err(CliError::InvalidConfig {
                    message: format!(
                        "--filter: equality `{field}={value}` is not supported; \
                         use field~needle (titulo~, url~, snippet~) or host:example.com"
                    ),
                });
            }
        }
        Ok(Self::Contains(s.to_ascii_lowercase()))
    }

    fn matches_web(&self, r: &SearchResult) -> bool {
        let title = r.title.to_ascii_lowercase();
        let url = r.url.as_str().to_ascii_lowercase();
        let snippet = r.snippet.as_deref().unwrap_or("").to_ascii_lowercase();
        match self {
            Self::Contains(n) => title.contains(n) || url.contains(n) || snippet.contains(n),
            Self::TitleContains(n) => title.contains(n),
            Self::UrlContains(n) => url.contains(n),
            Self::SnippetContains(n) => snippet.contains(n),
            Self::HostContains(n) => host_of(r.url.as_str()).is_some_and(|h| h.contains(n)),
        }
    }

    fn matches_news(&self, r: &NewsResult) -> bool {
        let title = r.title.to_ascii_lowercase();
        let url = r.url.as_str().to_ascii_lowercase();
        match self {
            Self::Contains(n) => title.contains(n) || url.contains(n),
            Self::TitleContains(n) => title.contains(n),
            Self::UrlContains(n) => url.contains(n),
            Self::SnippetContains(_) => false,
            Self::HostContains(n) => host_of(r.url.as_str()).is_some_and(|h| h.contains(n)),
        }
    }

    fn matches_aggregated(&self, r: &AggregatedItem) -> bool {
        let title = r.title.to_ascii_lowercase();
        let url = r.url.as_str().to_ascii_lowercase();
        let snippet = r.snippet.as_deref().unwrap_or("").to_ascii_lowercase();
        match self {
            Self::Contains(n) => title.contains(n) || url.contains(n) || snippet.contains(n),
            Self::TitleContains(n) => title.contains(n),
            Self::UrlContains(n) => url.contains(n),
            Self::SnippetContains(n) => snippet.contains(n),
            Self::HostContains(n) => host_of(r.url.as_str()).is_some_and(|h| h.contains(n)),
        }
    }

    fn matches_aggregated_news(&self, r: &AggregatedNewsItem) -> bool {
        let title = r.title.to_ascii_lowercase();
        let url = r.url.as_str().to_ascii_lowercase();
        let source = r.source.as_deref().unwrap_or("").to_ascii_lowercase();
        match self {
            Self::Contains(n) => title.contains(n) || url.contains(n) || source.contains(n),
            Self::TitleContains(n) => title.contains(n),
            Self::UrlContains(n) => url.contains(n),
            // News rows have no snippet; treat snippet~ as no-match (same as Search news).
            Self::SnippetContains(_) => false,
            Self::HostContains(n) => host_of(r.url.as_str()).is_some_and(|h| h.contains(n)),
        }
    }
}

/// Apply filter then field projection to a single-query envelope (in place).
///
/// Returns the number of result rows **before** the filter ran (web + news),
/// so callers can distinguish filter-miss from a genuine empty SERP
/// (`erro=filter_empty`, GAP-E2E-V19-FILTER-SYNTAX-FOOTGUN).
pub fn apply_to_search_output(
    output: &mut SearchOutput,
    fields: Option<&FieldSet>,
    filter: Option<&ResultFilter>,
) -> u32 {
    let pre_filter = search_row_count(output);
    if let Some(f) = filter {
        output.results.retain(|r| f.matches_web(r));
        if let Some(news) = output.news.as_mut() {
            news.retain(|r| f.matches_news(r));
            output.news_count = Some(news.len() as u32);
        }
        output.result_count = output.results.len() as u32;
    }
    if let Some(fs) = fields {
        for r in &mut output.results {
            project_web_result(r, fs);
        }
        if let Some(news) = output.news.as_mut() {
            for r in news {
                project_news_result(r, fs);
            }
        }
    }
    pre_filter
}

/// Apply filter + projection to every search in a multi-query envelope.
///
/// Returns the sum of pre-filter row counts across all searches.
pub fn apply_to_multi_output(
    output: &mut MultiSearchOutput,
    fields: Option<&FieldSet>,
    filter: Option<&ResultFilter>,
) -> u32 {
    let mut pre = 0u32;
    for search in &mut output.searches {
        pre = pre.saturating_add(apply_to_search_output(search, fields, filter));
    }
    pre
}

/// Post-SERP / post-filter row cap (agent-native `--limit`, not `-n/--num`).
///
/// `-n/--num` limits the **DDG request**; `--limit` slices the envelope the
/// agent receives so the LLM never needs `jq '.[0:N]'`.
pub fn apply_result_limit_search(output: &mut SearchOutput, limit: Option<u32>) {
    let Some(n) = limit else {
        return;
    };
    let n = n as usize;
    if output.results.len() > n {
        output.results.truncate(n);
        output.result_count = output.results.len() as u32;
    }
    if let Some(news) = output.news.as_mut() {
        if news.len() > n {
            news.truncate(n);
            output.news_count = Some(news.len() as u32);
        }
    }
}

/// Apply [`apply_result_limit_search`] to every multi-query search.
pub fn apply_result_limit_multi(output: &mut MultiSearchOutput, limit: Option<u32>) {
    for search in &mut output.searches {
        apply_result_limit_search(search, limit);
    }
}

/// Cap deep-research aggregated web/news rows after filter/project.
pub fn apply_result_limit_deep(output: &mut DeepResearchOutput, limit: Option<u32>) {
    let Some(n) = limit else {
        return;
    };
    let n = n as usize;
    if output.results.len() > n {
        output.results.truncate(n);
        output.metadata.unique_result_count = output.results.len();
    }
    if output.news.len() > n {
        output.news.truncate(n);
        output.news_count = output.news.len();
        output.metadata.unique_news_count = output.news.len();
    }
}

/// Mark a search envelope as filter-empty (SERP had rows; filter kept none).
pub fn mark_filter_empty_search(output: &mut SearchOutput) {
    output.error = Some("filter_empty".into());
    output.message = Some(
        "SERP returned results but --filter matched zero rows; relax the filter expression".into(),
    );
}

fn search_row_count(output: &SearchOutput) -> u32 {
    let news = output.news.as_ref().map_or(0, |n| n.len() as u32);
    (output.results.len() as u32).saturating_add(news)
}

/// Apply filter (and optional struct-level strip) to a deep-research envelope.
///
/// Apply filter/project to a deep-research envelope.
///
/// # Agent-native (GAP-DEEP-PROJECT-FILTER)
///
/// - `--filter` retains only matching aggregated web/news rows and updates
///   `news_count` + metadata unique counts so agents see coherent numbers.
/// - `--fields` strips optional fat fields on the structs; required keys that
///   must disappear from JSON (e.g. `score`, `fontes`) are dropped by
///   [`format_deep_json_projected`] via `serde_json::Value` key retention.
/// - When `fields` is set, the optional `synthesis` blob is dropped (token budget)
///   unless the allowlist includes a future synth key (none today).
///
/// Returns the pre-filter aggregated row count (web + news).
pub fn apply_to_deep_output(
    output: &mut DeepResearchOutput,
    fields: Option<&FieldSet>,
    filter: Option<&ResultFilter>,
) -> u32 {
    let pre_filter = (output.results.len() as u32).saturating_add(output.news.len() as u32);
    if let Some(f) = filter {
        output.results.retain(|r| f.matches_aggregated(r));
        output.news.retain(|r| f.matches_aggregated_news(r));
        output.news_count = output.news.len();
        output.metadata.unique_result_count = output.results.len();
        output.metadata.unique_news_count = output.news.len();
    }
    if fields.is_some() {
        // Synthesis is multi-kB; agents that project result rows almost never
        // want the full report in the same payload.
        output.synth = None;
    }
    if let Some(fs) = fields {
        for r in &mut output.results {
            project_aggregated_item(r, fs);
        }
        for r in &mut output.news {
            project_aggregated_news(r, fs);
        }
    }
    pre_filter
}

/// Serialize `DeepResearchOutput` as compact JSON with only selected result keys.
///
/// Top-level envelope keys stay (`tipo`, `query`, `metadados`, `resultados`,
/// `noticias`, …). Each object in `resultados` / `noticias` keeps only keys in
/// `fields` (same pattern as [`format_search_json_projected`]).
///
/// # Errors
///
/// Serde failures → [`CliError::InvalidConfig`].
pub fn format_deep_json_projected(
    output: &DeepResearchOutput,
    fields: &FieldSet,
) -> Result<String, CliError> {
    let mut value = serde_json::to_value(output).map_err(|e| CliError::InvalidConfig {
        message: format!("failed to serialize deep-research output: {e}"),
    })?;
    project_envelope_value(&mut value, fields);
    super::wire_keys::value_to_wire_string(value)
}

/// Emit deep-research JSON: projected when `fields` is set, else full compact.
///
/// # Errors
///
/// Serde / projection failures → [`CliError::InvalidConfig`].
pub fn format_deep_json(
    output: &DeepResearchOutput,
    fields: Option<&FieldSet>,
) -> Result<String, CliError> {
    match fields {
        Some(fs) => format_deep_json_projected(output, fs),
        // serialize_for_wire honors --pretty; to_wire_string is forced-compact.
        None => super::wire_keys::serialize_for_wire(output),
    }
}

fn project_aggregated_item(r: &mut AggregatedItem, fs: &FieldSet) {
    if !fs.contains("titulo") {
        r.title.clear();
    }
    if !fs.contains("url_exibicao") {
        r.display_url = None;
    }
    if !fs.contains("snippet") {
        r.snippet = None;
    }
    if !fs.contains("fontes") {
        r.sources.clear();
    }
    // score / posicao / url: required wire fields; JSON projection drops them
    // when absent from FieldSet via project_object_keys.
}

fn project_aggregated_news(r: &mut AggregatedNewsItem, fs: &FieldSet) {
    if !fs.contains("titulo") {
        r.title.clear();
    }
    if !fs.contains("fonte") {
        r.source = None;
    }
    if !fs.contains("data_relativa") {
        r.relative_date = None;
    }
    if !fs.contains("thumbnail") {
        r.thumbnail = None;
    }
}

/// Serialize `SearchOutput` as compact JSON with only selected result keys.
///
/// Top-level envelope keys stay (query, resultados, metadados, …). Each object
/// in `resultados` / `noticias` keeps only keys in `fields`.
///
/// # Errors
///
/// Serde failures → [`CliError::InvalidConfig`].
pub fn format_search_json_projected(
    output: &SearchOutput,
    fields: &FieldSet,
) -> Result<String, CliError> {
    let mut value = serde_json::to_value(output).map_err(|e| CliError::InvalidConfig {
        message: format!("failed to serialize search output: {e}"),
    })?;
    project_envelope_value(&mut value, fields);
    // value_to_wire_string honors --pretty (GAP-PRETTY-FIELDS).
    super::wire_keys::value_to_wire_string(value)
}

/// Serialize multi-query envelope with projected result rows.
///
/// # Errors
///
/// Serde failures → [`CliError::InvalidConfig`].
pub fn format_multi_json_projected(
    output: &MultiSearchOutput,
    fields: &FieldSet,
) -> Result<String, CliError> {
    let mut value = serde_json::to_value(output).map_err(|e| CliError::InvalidConfig {
        message: format!("failed to serialize multi-search output: {e}"),
    })?;
    if let Some(arr) = value.get_mut("buscas").and_then(Value::as_array_mut) {
        for item in arr {
            project_envelope_value(item, fields);
        }
    }
    // English alias path (if present in future).
    if let Some(arr) = value.get_mut("searches").and_then(Value::as_array_mut) {
        for item in arr {
            project_envelope_value(item, fields);
        }
    }
    super::wire_keys::value_to_wire_string(value)
}

/// TSV with columns = field set order (plus optional query column when multi).
pub fn format_search_tsv_projected(output: &SearchOutput, fields: &FieldSet) -> String {
    let mut buffer = String::with_capacity(64 + output.results.len().saturating_mul(80));
    buffer.push_str(&fields.keys().join("\t"));
    buffer.push('\n');
    for r in &output.results {
        write_tsv_row(&mut buffer, r, None, fields);
    }
    buffer
}

fn write_tsv_row(
    buffer: &mut String,
    r: &SearchResult,
    news: Option<&NewsResult>,
    fields: &FieldSet,
) {
    for (i, key) in fields.keys().iter().enumerate() {
        if i > 0 {
            buffer.push('\t');
        }
        let cell = if let Some(n) = news {
            news_field_str(n, key)
        } else {
            web_field_str(r, key)
        };
        push_tsv_escaped(buffer, &cell);
    }
    buffer.push('\n');
}

/// Project `resultados` / `noticias` (and EN aliases) object keys in a JSON Value.
///
/// Used by search/deep formatters and timeout partial envelopes.
pub(crate) fn project_envelope_value(value: &mut Value, fields: &FieldSet) {
    if let Some(arr) = value.get_mut("resultados").and_then(Value::as_array_mut) {
        for item in arr {
            project_object_keys(item, fields);
        }
    }
    if let Some(arr) = value.get_mut("results").and_then(Value::as_array_mut) {
        for item in arr {
            project_object_keys(item, fields);
        }
    }
    if let Some(arr) = value.get_mut("noticias").and_then(Value::as_array_mut) {
        for item in arr {
            project_object_keys(item, fields);
        }
    }
    if let Some(arr) = value.get_mut("news").and_then(Value::as_array_mut) {
        for item in arr {
            project_object_keys(item, fields);
        }
    }
}

/// Map FieldSet PT-canonical key → v2 EN wire key used on serialize.
fn wire_en_key(pt_canon: &str) -> &str {
    RESULT_KEYS
        .iter()
        .find(|(pt, _)| *pt == pt_canon)
        .map(|(_, en)| *en)
        .unwrap_or(pt_canon)
}

fn project_object_keys(value: &mut Value, fields: &FieldSet) {
    let Value::Object(map) = value else {
        return;
    };
    let mut next = Map::new();
    for key in fields.keys() {
        // v2.0.0 serialize is EN-primary; FieldSet still stores PT canon internally.
        let en = wire_en_key(key);
        if let Some(v) = map.remove(en).or_else(|| map.remove(key.as_str())) {
            next.insert(en.to_string(), v);
        }
    }
    *map = next;
}

fn project_web_result(r: &mut SearchResult, fs: &FieldSet) {
    if !fs.contains("posicao") {
        // Keep position for internal ranking; JSON projection drops it.
    }
    if !fs.contains("titulo") {
        r.title.clear();
    }
    if !fs.contains("url_exibicao") {
        r.display_url = None;
    }
    if !fs.contains("snippet") {
        r.snippet = None;
    }
    if !fs.contains("titulo_original") {
        r.original_title = None;
    }
    if !fs.contains("conteudo") {
        r.content = None;
    }
    if !fs.contains("tamanho_conteudo") {
        r.content_size = None;
    }
    if !fs.contains("metodo_extracao_conteudo") {
        r.content_extraction_method = None;
    }
}

fn project_news_result(r: &mut NewsResult, fs: &FieldSet) {
    if !fs.contains("titulo") {
        r.title.clear();
    }
    if !fs.contains("fonte") {
        r.source = None;
    }
    if !fs.contains("data_relativa") {
        r.relative_date = None;
    }
    if !fs.contains("thumbnail") {
        r.thumbnail = None;
    }
    if !fs.contains("conteudo") {
        r.content = None;
    }
    if !fs.contains("tamanho_conteudo") {
        r.content_size = None;
    }
    if !fs.contains("metodo_extracao_conteudo") {
        r.content_extraction_method = None;
    }
}

/// Borrows a web result field; allocates only for the numeric ones.
///
/// Every string field here already exists as a `String` on the row, so the
/// previous `-> String` signature copied the whole value for the sole purpose
/// of handing it to an escaper that copied it four more times. `content` is
/// routinely several kilobytes and is emitted once per row.
fn web_field_str<'a>(r: &'a SearchResult, key: &str) -> Cow<'a, str> {
    match key {
        "posicao" => Cow::Owned(r.position.to_string()),
        "titulo" => Cow::Borrowed(r.title.as_str()),
        "url" => Cow::Borrowed(r.url.as_str()),
        "url_exibicao" => optional(r.display_url.as_deref()),
        "snippet" => optional(r.snippet.as_deref()),
        "titulo_original" => optional(r.original_title.as_deref()),
        "conteudo" => optional(r.content.as_deref()),
        "tamanho_conteudo" => r
            .content_size
            .map_or(Cow::Borrowed(""), |n| Cow::Owned(n.to_string())),
        "metodo_extracao_conteudo" => optional(r.content_extraction_method.as_deref()),
        _ => Cow::Borrowed(""),
    }
}

/// Borrows a news result field; allocates only for the numeric ones.
fn news_field_str<'a>(r: &'a NewsResult, key: &str) -> Cow<'a, str> {
    match key {
        "posicao" => Cow::Owned(r.position.to_string()),
        "titulo" => Cow::Borrowed(r.title.as_str()),
        "url" => Cow::Borrowed(r.url.as_str()),
        "fonte" => optional(r.source.as_deref()),
        "data_relativa" => optional(r.relative_date.as_deref()),
        "thumbnail" => optional(r.thumbnail.as_deref()),
        "conteudo" => optional(r.content.as_deref()),
        "tamanho_conteudo" => r
            .content_size
            .map_or(Cow::Borrowed(""), |n| Cow::Owned(n.to_string())),
        "metodo_extracao_conteudo" => optional(r.content_extraction_method.as_deref()),
        _ => Cow::Borrowed(""),
    }
}

/// An absent optional field is the empty cell, borrowed rather than allocated.
#[inline]
fn optional(value: Option<&str>) -> Cow<'_, str> {
    value.map_or(Cow::Borrowed(""), Cow::Borrowed)
}

/// Characters TSV must escape. All four are ASCII, so byte and char indices agree.
const TSV_ESCAPES: [char; 4] = ['\\', '\t', '\n', '\r'];

/// Appends `raw` to `buffer`, escaping in ONE pass with no intermediate string.
///
/// The previous shape was `buffer.push_str(&tsv_escape(cell))`, where
/// `tsv_escape` chained four `replace` calls. Each `replace` allocates a fresh
/// `String` and copies the entire value into it, so a cell went through six
/// full copies before reaching the buffer: one to build it, four to escape it,
/// one to append it. A row that carries fetched page content pays that six
/// times over a payload measured in kilobytes.
///
/// Cells with nothing to escape — the overwhelming majority — now copy exactly
/// once, straight into the destination.
fn push_tsv_escaped(buffer: &mut String, raw: &str) {
    let mut rest = raw;
    while let Some(idx) = rest.find(TSV_ESCAPES) {
        buffer.push_str(&rest[..idx]);
        let (escaped, width) = match rest.as_bytes()[idx] {
            b'\\' => ("\\\\", 1),
            b'\t' => ("\\t", 1),
            b'\n' => ("\\n", 1),
            _ => ("\\r", 1),
        };
        buffer.push_str(escaped);
        rest = &rest[idx + width..];
    }
    buffer.push_str(rest);
}

fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
}

fn canonicalize_field(token: &str) -> Option<String> {
    let t = token.trim().to_ascii_lowercase();
    for (pt, en) in RESULT_KEYS {
        if t == *pt || t == *en {
            return Some((*pt).to_string());
        }
    }
    None
}

fn allowlist_help() -> String {
    RESULT_KEYS
        .iter()
        .map(|(pt, en)| {
            if pt == en {
                (*pt).to_string()
            } else {
                format!("{pt}|{en}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
