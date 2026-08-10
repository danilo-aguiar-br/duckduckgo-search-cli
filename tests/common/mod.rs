// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(dead_code)]
//! Shared integration-test fixtures (v1.0.2 / GAP-TEST-COMPILE-8 DRY).
//!
//! Integration crates cannot use `#[cfg(test)]` helpers from the library.
//! Prefer these builders over hand-written `Config { ... }` literals so
//! newtype fields (`ValidatedQuery`, `HttpUrl`, bounded timeouts, `proxy_config`,
//! `run_id`) stay in sync with production types.
//!
//! # Endpoint overrides (GAP-SCRAPE-R2-009 / V18)
//!
//! Residual HTTP wiremock fixtures **must** install
//! [`duckduckgo_search_cli::endpoints::EndpointPolicy`] via
//! [`EndpointPolicyGuard`]. Product code no longer reads
//! `DUCKDUCKGO_SEARCH_CLI_BASE_URL_*` env vars — those env sets are dead and
//! cause tests to hit live DuckDuckGo (false `Blocked`).

use duckduckgo_search_cli::cli::CliIdentityProfile;
use duckduckgo_search_cli::endpoints::{set_endpoint_policy, EndpointPolicy};
use duckduckgo_search_cli::http::{create_browser_profile, ProxyConfig};
use duckduckgo_search_cli::security::ValidatedQuery;
use duckduckgo_search_cli::types::{
    Config, ContentLengthLimit, Endpoint, GlobalTimeoutSeconds, OutputFormat, PageCount,
    ParallelismDegree, PerHostLimit, RetryBudget, SafeSearch, SearchMetadata, SelectorConfig,
    SerpCountry, SerpLanguage, TimeoutSeconds, UserAgentString, VerticalMode,
};
use std::sync::Arc;

const TEST_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";

/// Install process-wide rustls `CryptoProvider` (aws-lc-rs) once per test process.
///
/// Required by residual HTTP harness (`reqwest` + `rustls-tls-webpki-roots-no-provider`).
/// Call from any integration test that builds a `reqwest::Client` (wiremock, retry, …).
/// Idempotent. Does **not** affect production Chrome CDP SERP.
pub fn ensure_tls_for_http_harness() {
    duckduckgo_search_cli::tls_bootstrap::ensure_for_tests();
}

/// Validate a query string for fixtures (panics on invalid test input).
#[must_use]
pub fn validated_query(q: &str) -> ValidatedQuery {
    ValidatedQuery::try_new(q).unwrap_or_else(|e| panic!("test query {q:?}: {e}"))
}

/// Lean web-only Config for wiremock / pipeline / retry tests.
///
/// - `fetch_content = false`, `quiet = true`, `warmup_enabled = false`
/// - `proxy_config = Disabled` (no HTTP_PROXY inheritance)
/// - bounded newtypes for timeouts / pages / retries
#[must_use]
pub fn lean_config(endpoint: Endpoint, pages: u32, retries: u32) -> Config {
    // Residual HTTP fixtures (wiremock) need rustls provider before Client::build.
    ensure_tls_for_http_harness();
    let q = validated_query("rust");
    Config {
        query: q.clone(),
        queries: vec![q],
        num_results: None,
        vertical: VerticalMode::Web,
        format: OutputFormat::Json,
        timeout_seconds: TimeoutSeconds::try_new(5).expect("timeout"),
        language: SerpLanguage::try_new("pt").expect("lang"),
        country: SerpCountry::try_new("br").expect("country"),
        verbose: 0,
        quiet: true,
        user_agent: UserAgentString::try_new(TEST_UA).expect("ua"),
        browser_profile: create_browser_profile(TEST_UA),
        parallelism: ParallelismDegree::try_new(1).expect("par"),
        pages: PageCount::try_new(pages).expect("pages"),
        retries: RetryBudget::try_new(retries).expect("retries"),
        endpoint,
        time_filter: None,
        safe_search: SafeSearch::Moderate,
        stream_mode: false,
        output_file: None,
        fetch_content: false,
        fetch_content_cap: duckduckgo_search_cli::cli::DEFAULT_FETCH_CONTENT_CAP,
        agent_ops: duckduckgo_search_cli::output::AgentOps::default(),
        max_output_bytes: None,
        max_content_length: ContentLengthLimit::try_new(10_000).expect("content"),
        proxy_config: ProxyConfig::Disabled,
        global_timeout_seconds: GlobalTimeoutSeconds::try_new(60).expect("gto"),
        match_platform_ua: false,
        per_host_limit: PerHostLimit::try_new(2).expect("host"),
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
        shared_session_verticals: false,
    }
}

/// Multi-query lean Config (parallel / pipeline tests).
#[must_use]
pub fn lean_config_queries(
    endpoint: Endpoint,
    pages: u32,
    queries: Vec<String>,
    parallelism: u32,
) -> Config {
    let qs: Vec<ValidatedQuery> = queries.iter().map(|s| validated_query(s)).collect();
    let first = qs
        .first()
        .cloned()
        .unwrap_or_else(|| validated_query("rust"));
    let mut c = lean_config(endpoint, pages, 0);
    c.query = first;
    c.queries = qs;
    c.parallelism = ParallelismDegree::try_new(parallelism).expect("par");
    c
}

/// SearchMetadata stub with stable defaults (`run_id: None`, `flags_ignored: None`).
///
/// Prefer this (or `..SearchMetadata::default()`) over full field lists so
/// additive wire fields (e.g. V15.1 `flags_ignored`) do not break harnesses.
#[must_use]
pub fn sample_metadata() -> SearchMetadata {
    SearchMetadata {
        selectors_hash: "abc123".to_string(),
        user_agent: "test-ua".to_string(),
        ..SearchMetadata::default()
    }
}

/// Build ResultCount for fixtures.
#[must_use]
pub fn result_count(n: u32) -> duckduckgo_search_cli::types::ResultCount {
    duckduckgo_search_cli::types::ResultCount::try_new(n).expect("result_count")
}

/// Parse HttpUrl for fixtures.
#[must_use]
pub fn http_url(raw: &str) -> duckduckgo_search_cli::types::HttpUrl {
    duckduckgo_search_cli::types::HttpUrl::try_new(raw).expect("http_url")
}

/// Map string queries to ValidatedQuery vec.
#[must_use]
pub fn validated_queries(qs: &[&str]) -> Vec<ValidatedQuery> {
    qs.iter().map(|s| validated_query(s)).collect()
}

/// Map owned String queries to ValidatedQuery vec.
#[must_use]
pub fn validated_queries_owned(qs: &[String]) -> Vec<ValidatedQuery> {
    qs.iter().map(|s| validated_query(s)).collect()
}

/// RAII guard: installs process-wide endpoint policy for wiremock / residual HTTP.
///
/// Restores [`EndpointPolicy::default`] on drop so tests do not leak mock URLs
/// into later cases in the same process.
///
/// # Example
///
/// ```ignore
/// let base = format!("{}/", mock.uri());
/// let _ep = common::EndpointPolicyGuard::html_and_lite(&base, &base);
/// // search_with_pagination now hits the mock, not live DDG
/// ```
#[derive(Debug)]
pub struct EndpointPolicyGuard {
    _private: (),
}

impl EndpointPolicyGuard {
    /// Install HTML + Lite (+ optional SERP) base URLs. Prefer trailing slash.
    #[must_use]
    pub fn install(html: Option<String>, lite: Option<String>, serp: Option<String>) -> Self {
        set_endpoint_policy(EndpointPolicy { html, lite, serp });
        Self { _private: () }
    }

    /// Both HTML and Lite point at the same mock base (common wiremock case).
    #[must_use]
    pub fn html_and_lite(html: &str, lite: &str) -> Self {
        Self::install(Some(html.to_string()), Some(lite.to_string()), None)
    }

    /// HTML / Lite / SERP all point at the same mock base.
    #[must_use]
    pub fn all_bases(base: &str) -> Self {
        Self::install(
            Some(base.to_string()),
            Some(base.to_string()),
            Some(base.to_string()),
        )
    }

    /// SERP-only override (news vertical URL builder).
    #[must_use]
    pub fn serp_only(serp: &str) -> Self {
        Self::install(None, None, Some(serp.to_string()))
    }
}

impl Drop for EndpointPolicyGuard {
    fn drop(&mut self) {
        set_endpoint_policy(EndpointPolicy::default());
    }
}

/// Drop-in replacement for legacy `EnvGuard` that set `DUCKDUCKGO_SEARCH_CLI_BASE_URL_*`.
///
/// - **Base URL keys** → [`set_endpoint_policy`] (SSOT after GAP-SCRAPE-R2-009).
/// - **Other keys** (e.g. `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST`) → process env for the
///   residual HTTP feature gate only (not product config).
///
/// Restores endpoint defaults and removes env keys on drop.
#[derive(Debug)]
pub struct HarnessGuard {
    env_keys: Vec<&'static str>,
    had_endpoint_override: bool,
}

impl HarnessGuard {
    /// Install from the historical `(key, value)` pairs used by wiremock fixtures.
    #[must_use]
    pub fn set(keys: &[(&'static str, String)]) -> Self {
        let mut html = None;
        let mut lite = None;
        let mut serp = None;
        let mut env_keys = Vec::new();
        for (k, v) in keys {
            match *k {
                "DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML" => html = Some(v.clone()),
                "DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE" => lite = Some(v.clone()),
                "DUCKDUCKGO_SEARCH_CLI_BASE_URL_SERP" => serp = Some(v.clone()),
                other => {
                    // SAFETY for tests only: residual harness / feature gates.
                    // Product SERP never inherits these as configuration.
                    std::env::set_var(other, v);
                    env_keys.push(other);
                }
            }
        }
        let had_endpoint_override = html.is_some() || lite.is_some() || serp.is_some();
        if had_endpoint_override {
            set_endpoint_policy(EndpointPolicy { html, lite, serp });
        }
        Self {
            env_keys,
            had_endpoint_override,
        }
    }
}

impl Drop for HarnessGuard {
    fn drop(&mut self) {
        if self.had_endpoint_override {
            set_endpoint_policy(EndpointPolicy::default());
        }
        for k in &self.env_keys {
            std::env::remove_var(k);
        }
    }
}
