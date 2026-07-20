// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (shared data types and serde configuration)
//! Shared data types used across the application.
//!
//! Output structs (`SearchOutput`, `MultiSearchOutput`, `SearchResult`,
//! `SearchMetadata`) serialize with JSON field names preserved via
//! `#[serde(rename = "...")]` for backward compatibility.

use crate::cli::CliIdentityProfile;
use crate::http::BrowserProfile;
pub mod bounded;
pub mod http_url;
pub mod ids;
pub mod selectors;
pub mod time;
pub mod wire;
pub use bounded::{
    ContentLengthLimit, GlobalTimeoutSeconds, PageCount, ParallelismDegree, PerHostLimit,
    ResultCount, RetryBudget, SerpCountry, SerpLanguage, TimeoutSeconds, UserAgentString,
};
pub use http_url::HttpUrl;
pub use ids::RunId;
pub use selectors::{
    AdFilter, HtmlSelectors, LiteSelectors, NewsSelectors, PaginationSelectors, RelatedSelectors,
    SelectorConfig,
};
pub use time::utc_now;
#[cfg(test)]
pub use time::{test_timestamp, test_timestamp_offset};

pub use wire::{
    MultiSearchOutput, NewsResult, SearchMetadata, SearchOutput, SearchResult, ZeroCause,
};





/// `DuckDuckGo` endpoint chosen via `--endpoint`.
///
/// - `Html` (default): `https://html.duckduckgo.com/html/` with `.result` in the DOM.
/// - `Lite`: `https://lite.duckduckgo.com/lite/` with tabular layout (no JavaScript).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Endpoint {
    /// Full HTML endpoint with `.result` DOM structure.
    Html,
    /// Lightweight endpoint with tabular layout (no JavaScript).
    Lite,
}

impl Endpoint {
    /// Returns the short string used in logs and metadata output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Endpoint::Html => "html",
            Endpoint::Lite => "lite",
        }
    }
}

/// Search vertical selected via `--vertical` (GAP-WS-104 v0.8.9).
///
/// Default is **`All`** since v0.9.8 (GAP-WS-AGENT-READY-001): agent-ready dual
/// web+news. Opt out with `--vertical web`. `News` and `All` are routed
/// EXCLUSIVELY through the Chrome-primary transport — the `ia=news&iar=news`
/// SERP requires JavaScript rendering and has no HTTP fallback
/// (see `search::build_news_search_url`). `All` runs both verticals — by
/// default as two multi-process Chromes in parallel when `--parallel ≥ 2`
/// (GAP-PAR-021); opt into one shared session with `--shared-session-verticals`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum VerticalMode {
    /// Organic web results only (`--vertical web`).
    Web,
    /// News vertical only. Requires the `chrome` feature.
    News,
    /// Both verticals in the same Chrome session (best-effort news). **Default** since v0.9.8.
    #[default]
    All,
}

impl VerticalMode {
    /// `true` when this mode's pipeline must run the organic web search.
    pub fn includes_web(&self) -> bool {
        matches!(self, VerticalMode::Web | VerticalMode::All)
    }

    /// `true` when this mode's pipeline must run the news vertical.
    pub fn includes_news(&self) -> bool {
        matches!(self, VerticalMode::News | VerticalMode::All)
    }

    /// Returns the short string used in logs and metadata output
    /// (`metadados.vertical_usada`).
    pub fn as_str(&self) -> &'static str {
        match self {
            VerticalMode::Web => "web",
            VerticalMode::News => "news",
            VerticalMode::All => "all",
        }
    }
}

/// `DuckDuckGo` `df` time filter.
///
/// Values accepted by the API: `d` (day), `w` (week), `m` (month), `y` (year).
/// Absence of the parameter means "no time filter".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TimeFilter {
    /// Results from the last 24 hours.
    Day,
    /// Results from the last 7 days.
    Week,
    /// Results from the last 30 days.
    Month,
    /// Results from the last 365 days.
    Year,
}

impl TimeFilter {
    /// Returns the code accepted by the URL's `df` parameter.
    pub fn as_param(&self) -> &'static str {
        match self {
            TimeFilter::Day => "d",
            TimeFilter::Week => "w",
            TimeFilter::Month => "m",
            TimeFilter::Year => "y",
        }
    }
}

/// `DuckDuckGo` safe-search (`kp` parameter).
///
/// Accepted values: `-2` moderate (DDG default, sent as absence of the parameter),
/// `-1` off (disables filters), `1` strict (filters adult content).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SafeSearch {
    /// Disables all content filters (`kp=-1`).
    Off,
    /// DDG default — moderate filtering (no `kp` parameter sent).
    Moderate,
    /// Strict filtering of adult content (`kp=1`).
    Strict,
}

impl SafeSearch {
    /// Value for the `kp` parameter. `None` means "do not add the parameter"
    /// (equivalent to DDG's moderate default).
    pub fn as_param(&self) -> Option<&'static str> {
        match self {
            SafeSearch::Off => Some("-1"),
            SafeSearch::Moderate => None,
            SafeSearch::Strict => Some("1"),
        }
    }
}

/// Global settings derived from the CLI, passed through the pipeline.
///
/// The `query` field remains as the "active query" in single-query executions
/// (useful for the legacy flow in `pipeline::execute`). In multi-query mode, the
/// pipeline iterates over `queries` and clones this struct for each task,
/// overwriting `query` with the current iteration item.
#[derive(Clone)]
pub struct Config {
    /// "Active" query — validated at CLI boundary (GAP-SECDEV-009).
    /// In multi-query mode starts equal to the first query and is overwritten per task.
    pub query: crate::security::ValidatedQuery,
    /// Full list of queries to execute. Always contains at least 1 item.
    pub queries: Vec<crate::security::ValidatedQuery>,
    /// Desired number of results (maps to pagination logic).
    pub num_results: Option<ResultCount>,
    /// Output format chosen via `--format`.
    pub format: OutputFormat,
    /// Per-request HTTP timeout in seconds.
    pub timeout_seconds: TimeoutSeconds,
    /// Language code for DDG `kl` parameter (e.g. `"pt-br"`).
    pub language: SerpLanguage,
    /// Country code for DDG `kl` parameter (e.g. `"br"`).
    pub country: SerpCountry,
    /// Verbosity level of stderr logs (0=INFO, 1+=DEBUG, 2+=TRACE).
    /// Populated by the `-v`/`-vv`/`-vvv` repeated flag via `clap::ArgAction::Count`.
    pub verbose: u8,
    /// `--quiet` flag — suppresses non-essential stderr output.
    pub quiet: bool,
    /// Selected User-Agent string sent in HTTP headers.
    pub user_agent: UserAgentString,
    /// Full browser profile — family, version and platform derived from `user_agent`.
    /// Kept alongside the `user_agent` field (used in `SearchMetadata` and JSON output).
    pub browser_profile: BrowserProfile,
    /// Effective parallelism degree (1..=20). Informational only in single-query mode.
    pub parallelism: ParallelismDegree,
    /// Number of pages to fetch per query (1..=5).
    pub pages: PageCount,
    /// Number of retry attempts (0..=10). 0 = no retry; 2 is the default.
    pub retries: RetryBudget,
    /// Preferred endpoint (html by default; lite forces the no-JavaScript endpoint).
    pub endpoint: Endpoint,
    /// Optional time filter (`df`).
    pub time_filter: Option<TimeFilter>,
    /// Safe-search (`kp`).
    pub safe_search: SafeSearch,
    /// `--stream` flag (placeholder — not implemented in this iteration).
    pub stream_mode: bool,
    /// Optional path for writing output (instead of stdout).
    pub output_file: Option<std::path::PathBuf>,
    /// `--fetch-content` flag — enables text content extraction from result pages.
    pub fetch_content: bool,
    /// Max URLs enriched per vertical under `--fetch-content` (`--fetch-content-cap`).
    ///
    /// Agent-ready cost bound (GAP-SCRAPE-R-004). Default `10`.
    pub fetch_content_cap: usize,
    /// Value of `--max-content-length` — maximum content size in characters (1..=100000).
    pub max_content_length: ContentLengthLimit,
    /// Proxy policy (parsed once at CLI boundary — GAP-TYPE-003).
    pub proxy_config: crate::http::ProxyConfig,
    /// Value of `--global-timeout` in seconds (global timeout for the entire execution).
    pub global_timeout_seconds: GlobalTimeoutSeconds,
    /// `--match-platform-ua` flag — restricts UAs from the external config to the current platform.
    pub match_platform_ua: bool,
    /// Per-host concurrent fetch limit in `--fetch-content` mode (1..=10, default 2).
    pub per_host_limit: PerHostLimit,
    /// Optional manual path to Chrome/Chromium (`--chrome-path` flag, `chrome` feature).
    /// Without the `chrome` feature or `--fetch-content`, this value is ignored with a warning.
    pub chrome_path: Option<std::path::PathBuf>,
    /// Force headed Chrome (`--chrome-visible`). Mutually exclusive with headless force.
    ///
    /// GAP-SCRAPE-R-007: CLI primary (not product env).
    pub chrome_force_visible: bool,
    /// Force headless Chrome (`--chrome-headless`).
    pub chrome_force_headless: bool,
    /// Request private Xvfb headed mode on Linux (`--chrome-xvfb`).
    pub chrome_force_xvfb: bool,
    /// Optional path to dump news SERP HTML after Chrome extract (`--dump-news-html`).
    ///
    /// GAP-SCRAPE-R-008: CLI path, not product env.
    pub dump_news_html: Option<std::path::PathBuf>,
    /// CSS selector configuration (loaded from selectors.toml or built-in defaults).
    /// Wrapped in `Arc` for cheap cloning across concurrent tasks.
    pub selectors: std::sync::Arc<SelectorConfig>,
    /// Pre-built cookie jar for `reqwest::Client::cookie_provider`. Built by
    /// `build_config` from the persistent JSON file (or an empty jar if
    /// persistence is disabled). v0.7.3 PR2.
    pub cookie_provider: Option<std::sync::Arc<reqwest::cookie::Jar>>,
    /// Persistent jar handle used by the pipeline to save cookies back to
    /// disk after the request completes. v0.7.3 PR2.
    pub persistent_jar: Option<crate::cookie_adapter::PersistentJar>,
    /// Whether to perform the warm-up `GET https://duckduckgo.com/`
    /// before the first real query. v0.7.3 PR2.
    pub warmup_enabled: bool,
    /// Legacy flag retained for CLI backward compatibility (GAP-WS-113).
    /// Production SERP stays HTML Chrome; this is a no-op and never remediates
    /// blocks via Lite/HTTP.
    pub allow_lite_fallback: bool,
    /// Whether to enable pre-flight ghost-block / interstitial calibration on
    /// the shared Chrome SERP session (GAP-WS-113). Does not open a Lite or
    /// pure-HTTP production success path. Default `false` — opt-in via
    /// `--pre-flight`.
    pub pre_flight: bool,
    /// Selected browser identity profile from the 12-identity pool.
    /// Default `Auto` selects the adaptive cascade (rotates on block).
    /// When set to a specific family+platform tuple, the session is
    /// pinned to that single identity. v0.7.10 GAP-WS-60 fix — the
    /// flag was previously declared on the CLI but never propagated
    /// to `Config` (help-first drift).
    pub identity_profile: CliIdentityProfile,
    /// Cache of the cascade level observed in the last probe-deep
    /// successful in the same process session. Used by the classifier
    /// of zero-result as a cross-signal when  is not
    /// ativo. v0.8.0 GAP-NEW-003.
    pub last_probe_cascade_level: Option<u32>,
    /// Search vertical selected via `--vertical` (default `Web`).
    /// GAP-WS-104 v0.8.9.
    pub vertical: VerticalMode,
    /// When `true`, force one shared Chrome session for web+news (`--vertical all`)
    /// instead of dual multi-process (GAP-PAR-021). CLI: `--shared-session-verticals`.
    pub shared_session_verticals: bool,
}


/// Test/helper constructor with validated domain defaults (GAP-TYPE-018).
///
/// Prefer this over hand-written `Config { ... }` literals so newtype fields
/// stay in sync across unit tests.
#[cfg(test)]
pub fn test_config() -> Config {
    Config::default()
}

impl Default for Config {
    fn default() -> Self {
        use std::sync::Arc;
        // Placeholder query for Default only (tests/helpers). Production uses ValidatedQuery boundary.
        let placeholder = crate::security::ValidatedQuery::try_new("placeholder")
            .expect("placeholder query must validate");
        Self {
            query: placeholder.clone(),
            queries: vec![placeholder],
            num_results: None,
            format: OutputFormat::Json,
            timeout_seconds: TimeoutSeconds::try_new(30).expect("default timeout"),
            language: SerpLanguage::try_new("en").expect("default language"),
            country: SerpCountry::try_new("us").expect("default country"),
            verbose: 0,
            quiet: false,
            // Valid non-empty placeholder UA for tests/helpers (production fills real UA).
            user_agent: UserAgentString::try_new(
                "Mozilla/5.0 (compatible; duckduckgo-search-cli-test/1.0)",
            )
            .expect("default user_agent"),
            browser_profile: crate::http::BrowserProfile::default(),
            parallelism: ParallelismDegree::try_new(1).expect("default parallelism"),
            pages: PageCount::try_new(1).expect("default pages"),
            retries: RetryBudget::try_new(2).expect("default retries"),
            endpoint: Endpoint::Html,
            time_filter: None,
            safe_search: SafeSearch::Moderate,
            stream_mode: false,
            output_file: None,
            fetch_content: true,
            fetch_content_cap: crate::cli::DEFAULT_FETCH_CONTENT_CAP,
            max_content_length: ContentLengthLimit::try_new(10_000).expect("default content len"),
            proxy_config: crate::http::ProxyConfig::Unset,
            // Align with CLI default (v0.9.9 agent-ready / v1.0.0).
            global_timeout_seconds: GlobalTimeoutSeconds::try_new(
                crate::cli::DEFAULT_GLOBAL_TIMEOUT,
            )
            .expect("default global timeout"),
            match_platform_ua: false,
            per_host_limit: PerHostLimit::try_new(2).expect("default per_host"),
            chrome_path: None,
            chrome_force_visible: false,
            chrome_force_headless: false,
            chrome_force_xvfb: false,
            dump_news_html: None,
            selectors: Arc::new(SelectorConfig::default()),
            cookie_provider: None,
            persistent_jar: None,
            warmup_enabled: false,
            allow_lite_fallback: false,
            pre_flight: false,
            identity_profile: CliIdentityProfile::Auto,
            last_probe_cascade_level: None,
            vertical: VerticalMode::All,
            shared_session_verticals: false,
        }
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("query", &self.query)
            .field("endpoint", &self.endpoint)
            .field("warmup_enabled", &self.warmup_enabled)
            .field("allow_lite_fallback", &self.allow_lite_fallback)
            .field("pre_flight", &self.pre_flight)
            .field("identity_profile", &self.identity_profile)
            .field("vertical", &self.vertical)
            .finish()
    }
}

/// Output formats supported by the CLI (only `Json` is supported in the MVP).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputFormat {
    /// Structured JSON (default for pipes and LLM consumption).
    Json,
    /// Human-readable plain text.
    Text,
    /// Markdown with headers and links.
    Markdown,
    /// Tab-separated values (stable columns for agents/scripts).
    Tsv,
    /// Auto-detect: JSON when stdout is not a TTY, Text otherwise.
    Auto,
}

impl OutputFormat {
    /// Converts a `"json"|"text"|"markdown"|"tsv"|"auto"` string into the corresponding enum variant.
    pub fn from_str_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "text" => Some(Self::Text),
            "markdown" | "md" => Some(Self::Markdown),
            "tsv" => Some(Self::Tsv),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use validator::Validate;

    #[test]
    fn selector_config_default_has_result_container() {
        let cfg = SelectorConfig::default();
        assert_eq!(cfg.html_endpoint.results_container, "#links");
        assert!(cfg
            .html_endpoint
            .ads_filter
            .ad_url_patterns
            .contains(&"duckduckgo.com/y.js".to_string()));
    }

    #[test]
    fn output_format_parses_valid_variants() {
        assert_eq!(
            OutputFormat::from_str_value("json"),
            Some(OutputFormat::Json)
        );
        assert_eq!(
            OutputFormat::from_str_value("TEXT"),
            Some(OutputFormat::Text)
        );
        assert_eq!(
            OutputFormat::from_str_value("markdown"),
            Some(OutputFormat::Markdown)
        );
        assert_eq!(
            OutputFormat::from_str_value("md"),
            Some(OutputFormat::Markdown)
        );
        assert_eq!(
            OutputFormat::from_str_value("Auto"),
            Some(OutputFormat::Auto)
        );
        assert_eq!(OutputFormat::from_str_value("xml"), None);
    }

    #[test]
    fn search_output_serializes_pt_json_keys() {
        let output = SearchOutput {
            query: "teste".to_string(),
            engine: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: crate::types::test_timestamp(),
            region: "br-pt".to_string(),
            result_count: 0,
            results: vec![],
            pages_fetched: 1,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "abc123".to_string(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                chrome_attempted: false,
                user_agent: "Mozilla/5.0".to_string(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
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
                ..Default::default()
            },
            news: None,
            news_count: None,
        };
        let json = serde_json::to_string(&output).expect("serialization should work");
        // Portuguese JSON keys must be preserved (backward-compat invariant).
        assert!(json.contains("\"query\""));
        assert!(json.contains("\"quantidade_resultados\""));
        assert!(json.contains("\"tempo_execucao_ms\""));
        assert!(json.contains("\"resultados\""));
        assert!(json.contains("\"metadados\""));
        // v0.3.0 BREAKING: campo `buscas_relacionadas` removido do schema.
        assert!(!json.contains("\"buscas_relacionadas\""));
        // English Rust field names must NOT leak into JSON output.
        assert!(!json.contains("\"results_count\""));
        assert!(!json.contains("\"results\":"));
        assert!(!json.contains("\"metadata\""));
        assert!(!json.contains("\"related_searches\""));
        // GAP-WS-104 v0.8.9: default `web` mode must NOT emit the new
        // news-vertical fields at all (byte-identical contract).
        assert!(!json.contains("\"noticias\""));
        assert!(!json.contains("\"quantidade_noticias\""));
        assert!(!json.contains("\"vertical_usada\""));
    }

    #[test]
    fn vertical_mode_default_is_all_dual() {
        let mode = VerticalMode::default();
        assert_eq!(mode, VerticalMode::All);
        assert!(mode.includes_web());
        assert!(mode.includes_news());
        assert_eq!(mode.as_str(), "all");
    }

    #[test]
    fn vertical_mode_news_and_all_include_news() {
        assert!(VerticalMode::News.includes_news());
        assert!(!VerticalMode::News.includes_web());
        assert_eq!(VerticalMode::News.as_str(), "news");

        assert!(VerticalMode::All.includes_web());
        assert!(VerticalMode::All.includes_news());
        assert_eq!(VerticalMode::All.as_str(), "all");
    }

    #[test]
    fn zero_cause_vertical_sem_resultados_round_trips_kebab_case() {
        let json = serde_json::to_string(&ZeroCause::VerticalNoResults).unwrap();
        assert_eq!(json, "\"vertical-sem-resultados\"");
        let parsed: ZeroCause = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ZeroCause::VerticalNoResults);
    }

    /// ADR-0023 / GAP-E2E-51-008: serialize stays PT; deserialize accepts EN aliases.
    #[test]
    fn zero_cause_accepts_english_deserialize_aliases() {
        let legit: ZeroCause = serde_json::from_str("\"legitimate\"").unwrap();
        assert_eq!(legit, ZeroCause::Legitimate);
        // Serialize must still emit Portuguese wire values.
        assert_eq!(
            serde_json::to_string(&ZeroCause::Legitimate).unwrap(),
            "\"legitimo\""
        );
        let silent: ZeroCause = serde_json::from_str("\"silent-filter\"").unwrap();
        assert_eq!(silent, ZeroCause::SilentFilter);
        assert_eq!(
            serde_json::to_string(&silent).unwrap(),
            "\"filtro-silencioso\""
        );
    }

    #[test]
    fn search_result_accepts_english_field_aliases_on_deserialize() {
        let en = r#"{
            "position": 1,
            "title": "Example",
            "url": "https://example.com/",
            "display_url": "example.com",
            "content": "body",
            "content_size": 4
        }"#;
        let r: SearchResult = serde_json::from_str(en).expect("EN aliases deserialize");
        assert_eq!(r.position, 1);
        assert_eq!(r.title, "Example");
        assert_eq!(r.display_url.as_deref(), Some("example.com"));
        assert_eq!(r.content.as_deref(), Some("body"));
        // Serialize must keep Portuguese keys.
        let v = serde_json::to_value(&r).expect("serialize");
        assert!(v.get("posicao").is_some());
        assert!(v.get("titulo").is_some());
        assert!(v.get("position").is_none());
        assert!(v.get("title").is_none());
    }

    #[test]
    fn news_selectors_default_targets_react_module_container() {
        let cfg = SelectorConfig::default();
        assert!(
            cfg.news.container.contains("news-vertical")
                || cfg.news.container.contains("data-react-module-id"),
            "default news container should target news vertical: {}",
            cfg.news.container
        );
        assert!(cfg.news.article.contains("article"));
    }

    #[test]
    fn multi_search_output_serializes_pt_json_keys() {
        let output = MultiSearchOutput {
            query_count: 2,
            timestamp: crate::types::test_timestamp(),
            parallelism: 5,
            searches: vec![],
            causa_zero_histogram: BTreeMap::new(),
        };
        let json = serde_json::to_string(&output).expect("serialization should work");
        // Portuguese JSON keys must be preserved.
        assert!(json.contains("\"quantidade_queries\":2"));
        assert!(json.contains("\"paralelismo\":5"));
        assert!(json.contains("\"buscas\":[]"));
        // English field names must NOT appear in JSON.
        assert!(!json.contains("\"queries_count\""));
        assert!(!json.contains("\"parallel\""));
        assert!(!json.contains("\"searches\""));
    }

    #[test]
    fn default_selector_config_validates() {
        use validator::Validate;
        let cfg = SelectorConfig::default();
        cfg.validate()
            .expect("built-in SelectorConfig defaults must validate");
    }

    #[test]
    fn empty_css_selector_fails_validate() {
        use validator::Validate;
        let mut cfg = SelectorConfig::default();
        cfg.html_endpoint.results_container.clear();
        assert!(cfg.validate().is_err());
    }
}
