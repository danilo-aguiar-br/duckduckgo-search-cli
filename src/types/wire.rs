// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (JSON wire DTOs — agent stdout contract)
//! Wire-format types for stdout JSON (agent contract).
//!
//! Domain runtime config lives in `types::Config` and `types::bounded`.
//! These structs are serialization boundaries only (GAP-COMP-008).
//!
//! # Wire language policy (v2.0.0 / ADR-0027 — supersedes ADR-0023 default)
//!
//! **Serialize** keys are **English** (`results`, `metadata`, `title`, …) —
//! agent-native contract for v2.x.
//!
//! **Deserialize** still accepts Portuguese `alias = "..."` spellings so
//! legacy fixtures and `--wire-keys pt` consumers can parse historical JSON.
//! Serialize does **not** emit Portuguese aliases by default.

use crate::types::{HttpUrl, RunId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use std::collections::BTreeMap;

/// Classified cause of a zero-result in the JSON envelope.
///
/// Distinguishes legitimate empty SERPs, silent DDG filters, Cloudflare
/// ghost-blocks (HTTP 200 sub-4KB without markers), explicit anti-bot pages,
/// and invalid/truncated responses. Marked `#[non_exhaustive]` so future
/// variants do not break consumers.
///
/// Wire JSON values **serialize in English kebab-case** since ADR-0027 flipped
/// the serialize default to EN. Portuguese values remain accepted on
/// **deserialize only**, via `alias` (historical ADR-0023 contract). Portuguese
/// *keys* are produced by the post-processing layer in
/// `crate::output::wire_keys` under `--wire-keys pt`, never by `serde` itself.
///
/// v1.0.3 uniformised `ghost-block` and `anti-bot`, which were the last two
/// variants still emitting snake_case while every sibling emitted kebab-case.
/// The former snake spellings stay as deserialize aliases so payloads captured
/// by agents before 1.0.3 still parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ZeroCause {
    /// Query genuinely has no results in the DDG index.
    #[serde(rename = "legitimate", alias = "legitimo")]
    Legitimate,
    /// DDG dropped the query silently without a detectable interstitial.
    #[serde(rename = "silent-filter", alias = "filtro-silencioso")]
    SilentFilter,
    /// Cloudflare served HTTP 200 with a sub-4KB body and no literal markers.
    #[serde(rename = "ghost-block", alias = "ghost_block")]
    GhostBlock,
    /// Explicit anti-bot (HTTP 202, persistent 403, CF/DDG interstitial).
    #[serde(rename = "anti-bot", alias = "anti_bot")]
    AntiBot,
    /// Invalid or truncated response (empty body, malformed JSON, proxy intercept).
    #[serde(rename = "invalid-response", alias = "resposta-invalida")]
    InvalidResponse,
    /// Decompressed body in the suspicious 5–15KB band without result-page
    /// signal or interstitial markers. Indicates likely upstream soft-block.
    /// Distinct from `Legitimate` because the body lacks result-page markers.
    #[serde(rename = "suspicious-zero-results", alias = "zero-resultados-suspeito")]
    SuspiciousZeroResults,
    /// News/`all` vertical returned no articles and no blocking interstitial.
    /// Treated as a legitimate vertical zero (exit 5). GAP-WS-104 v0.8.9.
    #[serde(rename = "vertical-no-results", alias = "vertical-sem-resultados")]
    VerticalNoResults,
}

/// Represents a single `DuckDuckGo` search result.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Result position on the page (1-indexed, already after ad filtering).
    #[serde(rename = "position", alias = "posicao")]
    pub position: u32,

    /// Result title, extracted from the `.result__a` element.
    #[serde(rename = "title", alias = "titulo")]
    pub title: String,

    /// Result URL, extracted from the `href` attribute of `.result__a`.
    /// Validated absolute `http`/`https` ([`HttpUrl`]); JSON string on the wire.
    pub url: HttpUrl,

    /// Display URL (more user-friendly), extracted from `.result__url`.
    #[serde(rename = "display_url", alias = "url_exibicao")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_url: Option<String>,

    /// Descriptive snippet for the result, extracted from `.result__snippet`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,

    /// Literal title text as rendered by `DuckDuckGo`, preserved for auditing
    /// when substitution heuristics are applied (e.g., DDG returns "Official site"
    /// for verified domains — we replace it with `display_url` and keep the
    /// original here). Absent when the title was not modified.
    #[serde(rename = "original_title", alias = "titulo_original")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_title: Option<String>,

    /// Full text content of the page (only with `--fetch-content`; not implemented in the MVP).
    #[serde(rename = "content", alias = "conteudo")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Size in characters of the extracted content (only with `--fetch-content`).
    #[serde(rename = "content_size", alias = "tamanho_conteudo")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_size: Option<u32>,

    /// Method used to extract content: `"http"` or `"chrome"` (only with `--fetch-content`).
    #[serde(
        rename = "content_extraction_method",
        alias = "metodo_extracao_conteudo"
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_extraction_method: Option<String>,
}

/// Represents a single result from the `DuckDuckGo` news vertical.
///
/// Extracted from the Chrome-rendered DOM of the `ia=news&iar=news` SERP
/// (the news module requires JavaScript hydration — see
/// `extraction::extract_news_results_with_cfg`). Only `position`, `title`
/// and `url` are guaranteed; the remaining fields depend on which selector
/// cascade strategy matched and are `Option` with `skip_serializing_if`.
/// GAP-WS-104 v0.8.9.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsResult {
    /// Result position on the news page (1-indexed).
    #[serde(rename = "position", alias = "posicao")]
    pub position: u32,

    /// Headline text.
    #[serde(rename = "title", alias = "titulo")]
    pub title: String,

    /// Article URL, resolved to the external destination.
    /// Validated absolute `http`/`https` ([`HttpUrl`]); JSON string on the wire.
    pub url: HttpUrl,

    /// Publisher/source name (e.g. "G1", "Reuters"), when extractable.
    #[serde(rename = "source", alias = "fonte")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Relative timestamp as rendered by `DuckDuckGo` (e.g. "há 2 horas",
    /// "3 hours ago"). Kept verbatim — no absolute-date conversion in the MVP.
    #[serde(rename = "relative_date", alias = "data_relativa")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_date: Option<String>,

    /// Thumbnail image URL. Protocol-relative sources (`//host/img.jpg`) are
    /// resolved to `https://host/img.jpg`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,

    /// Clean article body text for LLM consumption (readability via Chrome/CDP).
    /// Populated when content fetch is enabled (default since v0.9.8).
    #[serde(rename = "content", alias = "conteudo")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Character length of [`Self::content`].
    #[serde(rename = "content_size", alias = "tamanho_conteudo")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_size: Option<usize>,

    /// How content was extracted (`readability`, `raw`, `none`).
    #[serde(
        rename = "content_extraction_method",
        alias = "metodo_extracao_conteudo"
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_extraction_method: Option<String>,
}

/// Search execution metadata, useful for diagnostics and LLM integration.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMetadata {
    /// Total execution time in milliseconds.
    #[serde(rename = "execution_time_ms", alias = "tempo_execucao_ms")]
    pub execution_time_ms: u64,

    /// Blake3 hash (hex, first 16 characters) of the selector configuration used.
    #[serde(rename = "selectors_hash", alias = "hash_seletores")]
    pub selectors_hash: String,

    /// Number of retries performed (0 in MVP — retry not yet implemented).
    /// Number of retries actually executed by the pipeline (excludes the
    /// first attempt). `0` indicates the initial request succeeded without
    /// any retry. GAP-AUD-007 v0.8.0: renamed from `retries` and added
    /// `retries_configured` to disambiguate configured-vs-executed.
    #[serde(rename = "retries", alias = "retentativas_executadas")]
    pub retries: u32,

    /// Number of retries that the operator configured via `--retries N`.
    /// Distinguishes between "0 retries ran because the first try worked"
    /// and "0 retries ran because none was requested". `None` when the
    /// operator did not override the default. v0.8.0 GAP-AUD-007.
    #[serde(rename = "retries_configured", alias = "retentativas_configuradas")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries_configured: Option<u32>,

    /// Indicates whether the Lite endpoint was used as fallback (always `false` in MVP).
    #[serde(rename = "used_fallback_endpoint", alias = "usou_endpoint_fallback")]
    pub used_fallback_endpoint: bool,

    /// Number of parallel content fetches started (0 in MVP).
    #[serde(rename = "concurrent_fetches", alias = "fetches_simultaneos")]
    pub concurrent_fetches: u32,

    /// Successful content fetches (0 in MVP).
    #[serde(rename = "fetch_successes", alias = "sucessos_fetch")]
    pub fetch_successes: u32,

    /// Failed content fetches (0 in MVP).
    #[serde(rename = "fetch_failures", alias = "falhas_fetch")]
    pub fetch_failures: u32,

    /// Indicates whether Chrome was used (always `false` in MVP).
    #[serde(rename = "used_chrome", alias = "usou_chrome")]
    pub used_chrome: bool,

    /// Indicates whether Chrome-primary search was attempted.
    /// `true` when the `chrome` feature is enabled and the pipeline
    /// tried the Chrome path (regardless of success or failure).
    #[serde(rename = "chrome_attempted", alias = "tentou_chrome")]
    pub chrome_attempted: bool,

    /// User-Agent used during execution.
    pub user_agent: String,

    /// Identity tag actually used for the request (WS-26).
    ///
    /// Format: `<family>-<platform>-<16hex>`. This field is additive — when
    /// the WS-26 identity rotation is disabled (default in v0.6.4) it
    /// contains a synthetic tag derived from the static UA. When rotation
    /// is active, the tag reports the identity that was used for the
    /// successful response (or the last attempt on failure).
    #[serde(rename = "identity_used", alias = "identidade_usada")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_used: Option<String>,

    /// Cascade level reached during the request (0..=4). `None` when the
    /// identity rotation was not active. See `IdentityPool::rotate_on_block`.
    #[serde(rename = "cascade_level", alias = "nivel_cascata")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cascade_level: Option<u32>,

    /// Indicates whether a proxy was configured (always `false` in MVP).
    #[serde(rename = "used_proxy", alias = "usou_proxy")]
    pub used_proxy: bool,

    /// Indicates whether the pre-flight ghost-block detection was triggered.
    /// `true` when `--pre-flight` is active AND a sub-4KB body with no
    /// result-page signal was classified as `Cloudflare`. v0.7.10.
    #[serde(rename = "pre_flight_fired", alias = "pre_flight_disparado")]
    pub pre_flight_fired: bool,

    /// Whether pre-flight calibration actually ran (GAP-WS-PREFLIGHT-META-001 v0.9.9).
    /// Distinct from `pre_flight_fired` (ghost-block only).
    #[serde(rename = "pre_flight_executed", alias = "pre_flight_executado")]
    #[serde(default)]
    pub pre_flight_executed: bool,

    /// Optional status: `skipped` | `ok` | `blocked` (v0.9.9).
    #[serde(rename = "pre_flight_status")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_flight_status: Option<String>,

    /// Count of news items removed as DDG promo/chrome (agent metadata, agent contract field).
    #[serde(rename = "news_promo_filtered", alias = "news_filtradas_promo")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub news_promo_filtered: Option<u32>,

    /// Whether `--stream` was requested.
    #[serde(rename = "stream_requested", alias = "stream_solicitado")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_requested: Option<bool>,

    /// Whether stream NDJSON was actually emitted.
    #[serde(rename = "stream_effective", alias = "stream_efetivo")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_effective: Option<bool>,

    /// Classified cause of a zero-result run, when `result_count == 0`.
    ///
    /// `None` when the classifier did not run or the search returned results.
    /// Filled automatically by `zero_cause::classify_zero_result`.
    /// v0.8.0 — closes GAP-AUD-003.
    #[serde(rename = "zero_cause", alias = "causa_zero")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zero_cause: Option<ZeroCause>,
    /// Actionable next step when `zero_cause` is present.
    ///
    /// A fixed string per `ZeroCause` variant; there is no separate code field.
    /// `None` when the classifier did not run or the search returned results.
    /// v0.8.0.
    #[serde(rename = "next_action_suggestion", alias = "sugestao_proxima_acao")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action_suggestion: Option<String>,

    /// Raw bytes received from DDG before decompression.
    ///
    /// `None` when the search did not run (config error, sub-4KB body without
    /// response, or byte counters unavailable). GAP-NEW-002 v0.8.0.
    /// Lets the operator tell an empty body apart from a 14KB Cloudflare
    /// stealth-block shell without needing a debug build.
    #[serde(rename = "bytes_raw", alias = "bytes_brutos")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_raw: Option<u64>,

    /// Bytes after gzip/deflate/br decompression.
    ///
    /// `None` when decompression did not occur or byte counters are
    /// unavailable. When both this and `bytes_raw` are present, a compression
    /// ratio can be computed as `bytes_decompressed / bytes_raw`.
    /// GAP-NEW-002 v0.8.0.
    #[serde(rename = "bytes_decompressed", alias = "bytes_descomprimidos")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_decompressed: Option<u64>,

    /// Cascade level observed in the most recent probe-deep of the same
    /// process session. Cached in `Config::last_probe_cascade_level` as a
    /// cross-signal for the zero-result classifier when `--pre-flight` is not
    /// active. GAP-NEW-003 v0.8.0.
    #[serde(rename = "cascade_level_observed", alias = "cascata_nivel_observado")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cascade_level_observed: Option<u32>,

    /// Compat alias: mirrors root-level `quantidade_resultados`. GAP-WS-092.
    #[serde(rename = "result_count", alias = "quantidade_resultados")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_count_compat: Option<u32>,

    /// Compat alias: mirrors root-level `endpoint`. GAP-WS-093.
    #[serde(rename = "endpoint_used", alias = "endpoint_usado")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_used_compat: Option<String>,

    /// Vertical actually executed (`"web"`, `"news"`, or `"all"`).
    ///
    /// Since v0.9.8 the default vertical is `all`, so this field is commonly
    /// present. Omitted only when unset (`None`). v0.8.9 GAP-WS-104 /
    /// GAP-WS-AGENT-READY-001 v0.9.8.
    #[serde(rename = "vertical_used", alias = "vertical_usada")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_used: Option<String>,

    /// Absolute path of the Chrome/Chromium binary used (after shell→ELF resolve).
    /// Agent contract field — agent contract field. GAP-WS-AGENT-READY-001 v0.9.8.
    #[serde(rename = "chrome_path_resolved", alias = "chrome_path_resolvido")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chrome_path_resolved: Option<String>,

    /// Install channel: `manual|env|host|flatpak|snap`. Agent contract field.
    #[serde(rename = "chrome_channel", alias = "chrome_canal")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chrome_channel: Option<String>,

    /// Correlation id for this search run (UUID v7, hyphenated string on wire).
    /// Present on production success and failure envelopes (Pass 42).
    #[serde(rename = "run_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,

    /// Legacy / no-op flags the operator passed that the binary intentionally
    /// ignored (agent honesty — GAP-E2E-V14-ALLOW-LITE-SILENT-NOOP).
    /// Example: `["allow-lite-fallback"]` when `--allow-lite-fallback` is set
    /// under Chrome-only production (GAP-WS-113).
    #[serde(rename = "flags_ignored", alias = "flags_ignored")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags_ignored: Option<Vec<String>>,
}

impl Default for SearchMetadata {
    /// Empty/initial metadata envelope (GAP-DRY-001 / GAP-DRY-003).
    fn default() -> Self {
        Self {
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
            identity_used: None,
            cascade_level: None,
            used_proxy: false,
            pre_flight_fired: false,
            pre_flight_executed: false,
            pre_flight_status: None,
            news_promo_filtered: None,
            stream_requested: None,
            stream_effective: None,
            zero_cause: None,
            next_action_suggestion: None,
            bytes_raw: None,
            bytes_decompressed: None,
            cascade_level_observed: None,
            result_count_compat: None,
            endpoint_used_compat: None,
            vertical_used: None,
            chrome_path_resolved: None,
            chrome_channel: None,
            run_id: None,
            flags_ignored: None,
        }
    }
}

/// Complete output for a single-query search (serialized as JSON in the MVP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOutput {
    /// Original search query submitted by the user.
    pub query: String,

    /// Search engine used — always `"duckduckgo"`.
    #[serde(rename = "engine", alias = "motor")]
    pub engine: String,

    /// Endpoint used — `"html"` or `"lite"` (always `"html"` in MVP).
    pub endpoint: String,

    /// ISO-8601 (RFC 3339) timestamp of when the search was executed.
    /// Rust type is [`DateTime<Utc>`]; serde emits an RFC 3339 JSON string.
    pub timestamp: DateTime<Utc>,

    /// `kl` region code used (e.g., `"br-pt"`).
    #[serde(rename = "region", alias = "regiao")]
    pub region: String,

    /// Count of results returned after ad filtering.
    #[serde(rename = "result_count", alias = "quantidade_resultados")]
    pub result_count: u32,

    /// List of organic results.
    #[serde(rename = "results", alias = "resultados")]
    pub results: Vec<SearchResult>,

    /// Number of pages fetched (always 1 in MVP).
    #[serde(rename = "pages_fetched", alias = "paginas_buscadas")]
    pub pages_fetched: u32,

    /// Structured error code if the search partially failed (None on full success).
    #[serde(rename = "error", alias = "erro")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Additional human-readable message (used for non-fatal warnings).
    #[serde(rename = "message", alias = "mensagem")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Execution metadata.
    #[serde(rename = "metadata", alias = "metadados")]
    pub metadata: SearchMetadata,

    /// News-vertical results (`--vertical news|all`). `None` in the default
    /// `web` mode — keeps the JSON contract byte-identical to pre-v0.8.9.
    /// GAP-WS-104 v0.8.9.
    #[serde(rename = "news", alias = "noticias")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub news: Option<Vec<NewsResult>>,

    /// Count of news-vertical results after dedupe/cap. `None` in the
    /// default `web` mode. GAP-WS-104 v0.8.9.
    #[serde(rename = "news_count", alias = "quantidade_noticias")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub news_count: Option<u32>,
}

impl SearchOutput {
    /// GAP-WS-092 + GAP-WS-093: populate compat fields in metadata
    /// so `.metadados.quantidade_resultados` and `.metadados.endpoint_usado`
    /// mirror the root-level values.
    pub fn fill_compat_fields(&mut self) {
        self.metadata.result_count_compat = Some(self.result_count);
        self.metadata.endpoint_used_compat = Some(self.endpoint.clone());
        // GAP-WS-097: populate nivel_cascata from cascata_nivel_observado.
        if self.metadata.cascade_level.is_none() {
            self.metadata.cascade_level = self.metadata.cascade_level_observed;
        }
    }
}

/// Complete output for a multi-query execution (serialized as JSON).
///
/// Per section 14.1 of the specification. Each inner `SearchOutput` retains the
/// single-query format (including per-query `error`), and the root-level fields
/// aggregate metadata from the parallel execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSearchOutput {
    /// Total number of queries executed (success + failure).
    #[serde(rename = "query_count", alias = "quantidade_queries")]
    pub query_count: u32,

    /// ISO-8601 (RFC 3339) timestamp of the start of the parallel execution.
    /// Rust type is [`DateTime<Utc>`]; serde emits an RFC 3339 JSON string.
    pub timestamp: DateTime<Utc>,

    /// Effective `--parallel` value used during execution (after validation/clamp).
    #[serde(rename = "parallelism", alias = "paralelismo")]
    pub parallelism: u32,

    /// Result of each individual query, in the same order as the input queries.
    #[serde(rename = "searches", alias = "buscas")]
    pub searches: Vec<SearchOutput>,

    /// Histograma agregado de `causa_zero` em todas as sub-queries (deep-research).
    ///
    /// `BTreeMap` for stable lexicographic order in deterministic JSON output.
    /// Key is the kebab-case name of the `ZeroCause` variant; value is the count.
    /// v0.8.0.
    // v1.0.3: was `causa_zero_histogram` on both the field and the wire — the
    // last Portuguese spelling that ADR-0027 missed, so the English default
    // emitted a Portuguese key and `--wire-keys pt` emitted nothing different.
    // The alias keeps pre-1.0.3 documents deserializable.
    #[serde(rename = "zero_cause_histogram", alias = "causa_zero_histogram")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub zero_cause_histogram: BTreeMap<String, u32>,
}

/// Envelope discriminator of `--probe`, fixed at the type level.
///
/// # Why a single-variant enum and not a `String`
///
/// Until v1.0.4 both probe envelopes carried `kind: String`, assigned by hand
/// in three constructors. Nothing tied that literal to the value published in
/// `commands::schema_cmd::DISCRIMINATOR_SCHEMAS`, and the two drifted: the code
/// emitted `probe_deep` while the catalog advertised `probe-deep`, so an agent
/// routing by discriminator found no schema. A single-variant enum makes the
/// wire value a compile-time constant — `serde` emits the `rename` string and
/// refuses to deserialize anything else, which is `const` in schema terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProbeKind {
    /// The only value: matches `properties.type.const` in `probe-output.schema.json`.
    #[default]
    #[serde(rename = "probe")]
    Probe,
}

/// Envelope discriminator of `--probe-deep`, fixed at the type level.
///
/// See [`ProbeKind`] for why this is an enum rather than a `String`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProbeDeepKind {
    /// The only value: matches `properties.type.const` in `probe-deep-output.schema.json`.
    #[default]
    #[serde(rename = "probe_deep")]
    ProbeDeep,
}

/// Envelope discriminator of `doctor`, fixed at the type level.
///
/// See [`ProbeKind`] for why this is an enum rather than a `String`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DoctorKind {
    /// The only value: matches `properties.type.const` in `doctor-output.schema.json`.
    #[default]
    #[serde(rename = "doctor")]
    Doctor,
}

/// Envelope discriminator of `deep-research`, fixed at the type level.
///
/// See [`ProbeKind`] for why this is an enum rather than a `String`. This one
/// is the only discriminator in the product that travels on `kind` instead of
/// `type`; the exception is declared in `commands::schema_cmd` and asserted by
/// `discriminator_key_is_kind_only_for_deep_research`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DeepResearchKind {
    /// The only value: matches `properties.kind.const` in `deep-research-output.schema.json`.
    #[default]
    #[serde(rename = "deep_research")]
    DeepResearch,
}

/// Envelope discriminator of `commands`, fixed at the type level.
///
/// See [`ProbeKind`] for why this is an enum rather than a `String`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CommandsKind {
    /// The only value: matches `properties.type.const` in `commands-output.schema.json`.
    #[default]
    #[serde(rename = "commands")]
    Commands,
}

/// Envelope discriminator of the `schema` catalog, fixed at the type level.
///
/// See [`ProbeKind`] for why this is an enum rather than a `String`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SchemaCatalogKind {
    /// The only value: matches `properties.type.const` in `schema-catalog.schema.json`.
    #[default]
    #[serde(rename = "schema_catalog")]
    SchemaCatalog,
}

/// Envelope discriminator of `locale`, fixed at the type level.
///
/// Added in v1.0.4. The envelope already carried `strategy: "locale"`, but
/// `strategy` names HOW the locale was resolved, not WHICH envelope this is —
/// so the catalog could not route it and an agent had to recognise the shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LocaleKind {
    /// The only value: matches `properties.type.const` in `locale-output.schema.json`.
    #[default]
    #[serde(rename = "locale")]
    Locale,
}

/// Envelope discriminators of the five `config` envelopes.
///
/// Added in v1.0.4: the whole family emitted no discriminator at all, so the
/// published catalog could not route any of it and an agent had to recognise
/// five shapes by hand. One enum rather than five single-variant types because
/// the values name sibling envelopes of one family and the call sites read
/// better as `ConfigKind::ConfigGet` than as five imports.
///
/// `set` and `unset` share `ConfigMutation`: they share one schema too, keyed
/// on `action` rather than on `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigKind {
    /// `config path` — matches `config-path-output.schema.json`.
    #[serde(rename = "config_path")]
    ConfigPath,
    /// `config list` — matches `config-list-output.schema.json`.
    #[serde(rename = "config_list")]
    ConfigList,
    /// `config get` — matches `config-get-output.schema.json`.
    #[serde(rename = "config_get")]
    ConfigGet,
    /// `config set` and `config unset` — matches `config-mutation-output.schema.json`.
    #[serde(rename = "config_mutation")]
    ConfigMutation,
    /// `config effective` — matches `config-effective-output.schema.json`.
    #[serde(rename = "config_effective")]
    ConfigEffective,
}

/// Envelope discriminator of `init-config`, fixed at the type level.
///
/// Added in v1.0.4: the envelope had no discriminator at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum InitConfigKind {
    /// The only value: matches `properties.type.const` in `init-config-output.schema.json`.
    #[default]
    #[serde(rename = "init_config")]
    InitConfig,
}

/// Health verdict emitted by `--probe`.
///
/// Serializes lowercase (`ok` | `blocked` | `error`), matching the `status`
/// enum in `docs/schemas/probe-output.schema.json`. Under `--wire-keys pt`
/// the value `error` becomes `erro` in `crate::output::wire_keys`; the key
/// itself is unchanged because `status` has no Portuguese spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeStatus {
    /// SERP-capable: no interstitial and a real result page.
    Ok,
    /// Reachable but blocked — interstitial or ghost-block body.
    Blocked,
    /// The probe never reached a verdict (no Chrome, launch or transport failure).
    Error,
}

/// Classification emitted by `--probe-deep`.
///
/// Serializes lowercase (`ok` | `captcha` | `error`) per
/// `docs/schemas/probe-deep-output.schema.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeDeepStatus {
    /// Rendered DOM carried no anti-bot marker.
    Ok,
    /// An interstitial or CAPTCHA marker was detected in the rendered DOM.
    Captcha,
    /// The probe never reached a verdict (no Chrome, launch or transport failure).
    Error,
}

/// Typed `--probe` envelope (`docs/schemas/probe-output.schema.json`).
///
/// Replaces the twelve ad-hoc `serde_json::json!` literals that previously
/// built this payload by hand. Those literals emitted Portuguese keys
/// (`usou_chrome`, `tentou_chrome`) straight to stdout, bypassing
/// `crate::output::wire_keys`, and dropped the schema-required `healthy`
/// while typing `status` as an integer on every failure path.
///
/// The struct makes all three invariants compiler-enforced: `status` is an
/// enum, `healthy` is non-optional, and the English wire spelling is fixed by
/// `serde(rename)`. Portuguese output remains available through
/// `--wire-keys pt`, applied once at the emit boundary.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeReport {
    /// Envelope discriminator — always `probe`, enforced by [`ProbeKind`].
    #[serde(rename = "type")]
    pub kind: ProbeKind,

    /// Health verdict.
    pub status: ProbeStatus,

    /// `true` only when the SERP rendered and carried result-page signal.
    pub healthy: bool,

    /// Endpoint probed (`html` under the Chrome-only transport policy).
    pub endpoint: String,

    /// Numeric HTTP-like code: `200` when healthy, `0` otherwise.
    pub http_status: Option<u16>,

    /// Wall-clock latency of the probe.
    pub latency_ms: u64,

    /// Whether the response carried a `Set-Cookie` header.
    pub has_set_cookie: bool,

    /// URL actually probed (absent when the failure preceded navigation).
    pub url: Option<String>,

    /// Whether Chrome performed the navigation.
    #[serde(rename = "used_chrome", alias = "usou_chrome")]
    pub used_chrome: bool,

    /// Whether the Chrome path was attempted, successfully or not.
    #[serde(rename = "chrome_attempted", alias = "tentou_chrome")]
    pub chrome_attempted: bool,

    /// Size of the rendered body, when one was retrieved.
    pub body_len: Option<usize>,

    /// Whether the body carried a recognizable result-page signal.
    pub has_result_page_signal: Option<bool>,

    /// Human-readable failure description.
    pub error: Option<String>,

    /// Stable machine-readable failure identifier.
    pub error_code: Option<String>,
}

impl ProbeReport {
    /// Builds a failure envelope: not healthy, `status = error`, latency preserved.
    ///
    /// `error_code` is optional because the transport-level failure path has a
    /// message but no classified code.
    #[must_use]
    pub fn failure(
        endpoint: &str,
        latency_ms: u64,
        url: Option<String>,
        used_chrome: bool,
        error: String,
        error_code: Option<String>,
    ) -> Self {
        Self {
            kind: ProbeKind::Probe,
            status: ProbeStatus::Error,
            healthy: false,
            endpoint: endpoint.to_string(),
            http_status: Some(0),
            latency_ms,
            has_set_cookie: false,
            url,
            used_chrome,
            chrome_attempted: true,
            body_len: None,
            has_result_page_signal: None,
            error: Some(error),
            error_code,
        }
    }

    /// Builds a verdict envelope from a rendered body.
    #[must_use]
    pub fn verdict(
        endpoint: &str,
        latency_ms: u64,
        url: String,
        healthy: bool,
        body_len: usize,
        has_result_page_signal: bool,
    ) -> Self {
        Self {
            kind: ProbeKind::Probe,
            status: if healthy {
                ProbeStatus::Ok
            } else {
                ProbeStatus::Blocked
            },
            healthy,
            endpoint: endpoint.to_string(),
            http_status: Some(if healthy { 200 } else { 0 }),
            latency_ms,
            has_set_cookie: false,
            url: Some(url),
            used_chrome: true,
            chrome_attempted: true,
            body_len: Some(body_len),
            has_result_page_signal: Some(has_result_page_signal),
            error: None,
            error_code: None,
        }
    }
}

/// Typed `--probe-deep` envelope (`docs/schemas/probe-deep-output.schema.json`).
///
/// That schema sets `additionalProperties: false`, so the Portuguese
/// `cascata_motivo` key the previous `json!` literals emitted was a hard
/// violation rather than a cosmetic one: a strict validator rejects the whole
/// document. The English spelling `cascade_reason` is fixed here and mapped
/// back to `cascata_motivo` only under `--wire-keys pt`.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeDeepReport {
    /// Envelope discriminator — always `probe_deep`, enforced by [`ProbeDeepKind`].
    #[serde(rename = "type")]
    pub kind: ProbeDeepKind,

    /// CAPTCHA classification.
    pub status: ProbeDeepStatus,

    /// Endpoint probed (`html` under the Chrome-only transport policy).
    pub endpoint: String,

    /// HTTP status observed, when a response was produced.
    pub http_status: Option<u16>,

    /// Wall-clock latency of the probe.
    pub latency_ms: Option<u64>,

    /// Cascade level reached, `0..=4`.
    pub cascade_level: Option<u8>,

    /// Interstitial identifier, or `none` when the DOM was clean.
    #[serde(rename = "cascade_reason", alias = "cascata_motivo")]
    pub cascade_reason: Option<String>,

    /// Actionable remediation hint when `status = captcha`.
    pub mitigation_suggestion: Option<String>,

    /// URL actually probed.
    pub url: Option<String>,

    /// Whether Chrome performed the navigation.
    #[serde(rename = "used_chrome", alias = "usou_chrome")]
    pub used_chrome: Option<bool>,

    /// Whether the Chrome path was attempted, successfully or not.
    #[serde(rename = "chrome_attempted", alias = "tentou_chrome")]
    pub chrome_attempted: Option<bool>,

    /// Size of the rendered body, when one was retrieved.
    pub body_len: Option<usize>,

    /// Human-readable failure description.
    pub error: Option<String>,

    /// Stable machine-readable failure identifier.
    pub error_code: Option<String>,
}

impl ProbeDeepReport {
    /// Builds a failure envelope: `status = error`, no cascade verdict.
    #[must_use]
    pub fn failure(
        endpoint: &str,
        latency_ms: Option<u64>,
        used_chrome: Option<bool>,
        error: String,
        error_code: Option<String>,
    ) -> Self {
        Self {
            kind: ProbeDeepKind::ProbeDeep,
            status: ProbeDeepStatus::Error,
            endpoint: endpoint.to_string(),
            http_status: None,
            latency_ms,
            cascade_level: None,
            cascade_reason: None,
            mitigation_suggestion: None,
            url: None,
            used_chrome,
            chrome_attempted: used_chrome.map(|_| true),
            body_len: None,
            error: Some(error),
            error_code,
        }
    }
}

/// Thin structured error envelope, mirroring `docs/schemas/error-response.schema.json`.
///
/// # Why this is a type and not a `json!` literal
///
/// Until v1.0.3 the eleven thin-error emit sites in `run.rs` and
/// `commands::deep_research` each built their own `serde_json::json!` map and
/// wrote it with `print_line_stdout(&payload.to_string())`. That skipped
/// [`crate::output::serialize_for_wire`] entirely, so `--wire-keys pt` emitted
/// the English spellings — the exact mirror of the `--probe` defect, which
/// leaked Portuguese into the English default.
///
/// A literal map cannot be checked by the compiler and drifts on the next
/// patch. Routing every thin error through one type means the wire contract is
/// declared once, and `--wire-keys pt` is applied by construction.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinErrorResponse {
    /// Stable machine-readable error category, e.g. `invalid_config`.
    pub error: String,
    /// Human-readable explanation of the failure.
    pub message: String,
    /// Actionable remediation for an agent, when one is known.
    pub next_action_suggestion: Option<String>,
    /// Always `0` when present; emitted so a caller parsing a search
    /// invocation can read the same shape on success and on failure.
    pub result_count: Option<usize>,
    /// Always empty when present; see [`Self::result_count`].
    pub results: Option<Vec<SearchResult>>,
}

impl ThinErrorResponse {
    /// Builds the minimal envelope: just the required `error` and `message`.
    #[must_use]
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            next_action_suggestion: None,
            result_count: None,
            results: None,
        }
    }

    /// Attaches an agent-actionable remediation hint.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.next_action_suggestion = Some(suggestion.into());
        self
    }

    /// Adds the empty search shape (`result_count: 0`, `results: []`).
    ///
    /// Used on failure paths of a *search* invocation so the caller does not
    /// have to branch on envelope shape before reading `results`.
    #[must_use]
    pub fn with_search_shape(mut self) -> Self {
        self.result_count = Some(0);
        self.results = Some(Vec::new());
        self
    }
}
