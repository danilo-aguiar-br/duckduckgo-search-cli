// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (reqwest client construction)
//! Residual HTTP client construction, proxy config, and redirect policy.

// GAP-WS-113: this whole module is harness-only (`mod client` is gated in
// `http::mod`), so no per-item `cfg` is needed here. `ProxyConfig` / `ProxyUrl`
// live in the transport-neutral `super::proxy` because the Chrome path uses them.
use crate::error::CliError;
use reqwest::{redirect::Policy, Client};
use std::sync::Arc;
use std::time::Duration;

use super::profile::{create_browser_profile, BrowserProfile};
use super::proxy::{mask_proxy_url, ProxyConfig};

/// TCP keep-alive interval for the shared `reqwest` client (seconds).
pub(super) const TCP_KEEPALIVE_SECS: u64 = 60;
/// Connect timeout for establishing TCP+TLS (seconds).
pub(super) const CONNECT_TIMEOUT_SECS: u64 = 10;
/// Max idle connections retained per host in the client pool.
pub(super) const POOL_MAX_IDLE_PER_HOST: usize = 10;
/// How long an idle pooled connection is kept before eviction (seconds).
/// Matches reqwest's historical default of 90s; named for auditability.
pub(super) const POOL_IDLE_TIMEOUT_SECS: u64 = 90;
/// Maximum number of HTTP redirects followed by the client.
pub(super) const REDIRECT_LIMIT: usize = 5;

// ---------------------------------------------------------------------------
// Redirect policy (SSRF structural gate)
// ---------------------------------------------------------------------------

/// Redirect policy: max hops + structural URL safety on every Location target.
///
/// Blocks schemes other than `http`/`https` and literal private/loopback IPs
/// (see [`crate::content::is_safe_url`]). Hostname→private DNS rebinding is
/// handled asynchronously at content-fetch entry (cannot run inside this
/// sync redirect hook).
pub(super) fn safe_redirect_policy() -> Policy {
    Policy::custom(|attempt| {
        if attempt.previous().len() >= REDIRECT_LIMIT {
            return attempt.error(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "redirect hop limit exceeded",
            ));
        }
        // Clone before consuming `attempt` (borrowck: url() borrows attempt).
        let next = attempt.url().to_string();
        if !crate::content::is_safe_url(&next) {
            return attempt.error(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("redirect target rejected by SSRF filter: {next}"),
            ));
        }
        attempt.follow()
    })
}

// ---------------------------------------------------------------------------
// Client construction
// ---------------------------------------------------------------------------

/// Builds a `reqwest::Client` ready to make requests to `DuckDuckGo`.
///
/// # Arguments
/// * `user_agent` — User-Agent string to be sent on all requests.
/// * `timeout_secs` — total timeout (including body read).
/// * `language` — language code for the `Accept-Language` header (e.g. `"pt"`).
/// * `country` — country code for the `Accept-Language` header (e.g. `"br"`).
///
/// # Errors
/// Returns an error if the `ClientBuilder` build fails.
pub fn build_client(
    user_agent: &str,
    timeout_secs: u64,
    language: &str,
    country: &str,
) -> Result<Client, CliError> {
    let profile = create_browser_profile(user_agent);
    build_client_with_proxy(
        &profile,
        timeout_secs,
        language,
        country,
        &ProxyConfig::Unset,
    )
}

/// Builds a `reqwest::Client` with a browser profile and proxy configuration.
///
/// Uses [`BrowserProfile::initial_headers`] to generate family-specific headers,
/// including complete Sec-Fetch and Client Hints (Chrome/Edge).
///
/// # Arguments
/// * `profile` — browser profile that defines headers per family.
/// * `timeout_secs` — total timeout.
/// * `language` — language code (e.g. `"pt"`).
/// * `country` — country code (e.g. `"br"`).
/// * `proxy` — proxy configuration.
///
/// # Errors
/// Returns an error if the headers are invalid or the proxy configuration fails.
pub fn build_client_with_proxy(
    profile: &BrowserProfile,
    timeout_secs: u64,
    language: &str,
    country: &str,
    proxy: &ProxyConfig,
) -> Result<Client, CliError> {
    build_client_with_proxy_and_cookies(profile, timeout_secs, language, country, proxy, None)
}

/// Builds a `reqwest::Client` with a browser profile, proxy configuration, and
/// an optional external cookie store.
///
/// When `cookie_provider` is `Some(Arc<dyn reqwest::cookie::CookieStore>)`, the
/// client's in-memory cookie store is replaced with the supplied one. This
/// is the integration point for the [`crate::session_warmup::default_cookies_path`]
/// module: the warm-up reads the persistent jar from disk, wraps it in
/// [`crate::cookie_adapter::PersistentJar`], and passes it here so
/// the request pipeline sees the persisted session cookies.
///
/// When `cookie_provider` is `None`, the builder falls back to
/// `cookie_store(true)` (an in-memory jar that lives for the process).
///
/// # Errors
///
/// Returns `Err` if any header value contains invalid bytes, if the
/// proxy URL is malformed, or if the underlying TLS backend cannot
/// initialize (very rare on a properly configured host).
pub fn build_client_with_proxy_and_cookies(
    profile: &BrowserProfile,
    timeout_secs: u64,
    language: &str,
    country: &str,
    proxy: &ProxyConfig,
    cookie_provider: Option<Arc<reqwest::cookie::Jar>>,
) -> Result<Client, CliError> {
    let headers = profile.initial_headers(language, country)?;

    let mut builder = Client::builder()
        .user_agent(&profile.user_agent)
        .default_headers(headers)
        .tcp_nodelay(true)
        .tcp_keepalive(Duration::from_secs(TCP_KEEPALIVE_SECS))
        .pool_max_idle_per_host(POOL_MAX_IDLE_PER_HOST)
        .pool_idle_timeout(Duration::from_secs(POOL_IDLE_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        // Structural SSRF on each Location hop (literal private IPs / bad schemes).
        // Async DNS rebinding checks live in `content` for fetch-content targets.
        .redirect(safe_redirect_policy())
        .timeout(Duration::from_secs(timeout_secs));

    match cookie_provider {
        Some(provider) => {
            builder = builder.cookie_provider(provider);
        }
        None => {
            builder = builder.cookie_store(true);
        }
    }

    match proxy {
        // GAP-TLS-009: never inherit HTTP_PROXY/HTTPS_PROXY from the environment.
        // Proxy is CLI `--proxy` / XDG config only (project rule: no env config).
        ProxyConfig::Unset => {
            builder = builder.no_proxy();
        }
        ProxyConfig::Disabled => {
            builder = builder.no_proxy();
            tracing::info!("proxy explicitly disabled via --no-proxy");
        }
        ProxyConfig::Url(proxy_url) => {
            // Already validated at ProxyUrl::try_new — no re-parse (GAP-DOM-006).
            let parsed_url = proxy_url.as_url();
            let url = proxy_url.as_str();
            let user = parsed_url.username().to_string();
            let password = parsed_url
                .password()
                .map(|s| s.to_string())
                .unwrap_or_default();

            let mut proxy_rq = reqwest::Proxy::all(url).map_err(|e| CliError::ProxyError {
                message: format!(
                    "failed to configure Proxy::all({}): {e}",
                    mask_proxy_url(url)
                ),
            })?;

            if !user.is_empty() {
                proxy_rq = proxy_rq.basic_auth(&user, &password);
            }
            builder = builder.proxy(proxy_rq);
            tracing::info!(
                host = parsed_url.host_str(),
                scheme = parsed_url.scheme(),
                "proxy configured"
            );
        }
    }

    let client = builder
        .build()
        .map_err(|e| CliError::http_with_source("failed to build reqwest::Client", e))?;

    Ok(client)
}

/// Build a residual `reqwest::Client` only when the HTTP test harness is active.
///
/// Production Chrome-only paths skip Client construction (no TLS pool / cookie
/// jar / DNS state) — GAP-TLS-014 / one-shot memory.
///
/// # Errors
///
/// Propagates [`build_client_with_proxy_and_cookies`] failures when the harness
/// is active.
pub fn maybe_build_residual_client(
    profile: &BrowserProfile,
    timeout_secs: u64,
    language: &str,
    country: &str,
    proxy: &ProxyConfig,
    cookie_provider: Option<Arc<reqwest::cookie::Jar>>,
) -> Result<Option<Client>, CliError> {
    if !crate::chrome_policy::http_test_harness_active() {
        return Ok(None);
    }
    let client = build_client_with_proxy_and_cookies(
        profile,
        timeout_secs,
        language,
        country,
        proxy,
        cookie_provider,
    )?;
    Ok(Some(client))
}
