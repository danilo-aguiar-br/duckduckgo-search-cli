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
/// Wire JSON values stay Portuguese kebab-case for v1.x backward compatibility
/// via explicit `#[serde(rename = "...")]` on each variant. English
/// `alias` values are **deserialize-only** (ADR-0023 / GAP-E2E-51-008).
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
    #[serde(rename = "ghost_block", alias = "ghost-block")]
    GhostBlock,
    /// Explicit anti-bot (HTTP 202, persistent 403, CF/DDG interstitial).
    #[serde(rename = "anti_bot", alias = "anti-bot")]
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
    #[serde(rename = "content_extraction_method", alias = "metodo_extracao_conteudo")]
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
    #[serde(rename = "content_extraction_method", alias = "metodo_extracao_conteudo")]
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

    /// Causa classificada do zero-result quando `result_count == 0`.
    ///
    /// `None` when the classifier did not run or the search returned results.
    /// Auto-preenchido pelo classificador causal em `zero_cause::classify_zero_result`.
    /// v0.8.0 — fecha GAP-AUD-003.
    #[serde(rename = "zero_cause", alias = "causa_zero")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zero_cause: Option<ZeroCause>,
    /// Actionable next-action suggestion when .
    ///
    /// String fixa por variante de  (sem campo  separado).
    ///  when the classifier did not run or the search returned results.
    /// v0.8.0.
    #[serde(rename = "next_action_suggestion", alias = "sugestao_proxima_acao")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action_suggestion: Option<String>,

    /// Raw bytes received from DDG before decompression.
    ///
    ///  when the search did not run (config error, sub-4KB
    /// body without response, or byte counters unavailable). GAP-NEW-002 v0.8.0.
    /// Permite ao operador distinguir entre body vazio e shell de 14KB
    /// (stealth block do Cloudflare) sem precisar de build debug.
    #[serde(rename = "bytes_raw", alias = "bytes_brutos")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_raw: Option<u64>,

    /// Bytes after gzip/deflate/br decompression.
    ///
    ///  when decompression did not occur or byte counters are unavailable.
    /// When , a
    /// compression ratio can be calculated as
    /// . GAP-NEW-002 v0.8.0.
    #[serde(rename = "bytes_decompressed", alias = "bytes_descomprimidos")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_decompressed: Option<u64>,

    /// Cascade level observed in the most recent probe-deep of the same
    /// process session. Cached in
    /// para uso como sinal cruzado pelo classificador de zero-result
    /// when  is not active. GAP-NEW-003 v0.8.0.
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
    #[serde(rename = "causa_zero_histogram")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub causa_zero_histogram: BTreeMap<String, u32>,
}
