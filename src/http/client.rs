// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (reqwest client construction)
//! Residual HTTP client construction, proxy config, and redirect policy.

use crate::error::CliError;
use reqwest::{
    redirect::Policy,
    Client,
};
use std::sync::Arc;
use std::time::Duration;

use super::profile::{create_browser_profile, BrowserProfile};

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
// Proxy configuration
// ---------------------------------------------------------------------------

/// Proxy configuration for the HTTP client.
///
/// - `Unset` → `.no_proxy()` (no env inheritance; config is CLI/`--proxy` or XDG only).
/// - `Disabled` → `.no_proxy()` — same network effect as `Unset` (explicit operator intent).
/// - `Url(u)` → `Proxy::all(u)` with basic-auth extracted from userinfo, if present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyConfig {
    /// No explicit proxy — does **not** read `HTTP_PROXY`/`HTTPS_PROXY` env vars.
    Unset,
    /// Proxy explicitly disabled via `--no-proxy`.
    Disabled,
    /// Explicit proxy URL (HTTP/HTTPS/SOCKS5) validated at the CLI boundary.
    Url(ProxyUrl),
}

/// Validated proxy URL (`http`/`https`/`socks5`/`socks5h` with a host).
///
/// Stores the parsed [`url::Url`] so residual client construction never re-parses
/// (Pass 42 / GAP-DOM-006). Field is private; no [`std::ops::Deref`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct ProxyUrl(url::Url);

impl ProxyUrl {
    /// Parse and validate a proxy URL string.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::ProxyError`] when the URL is malformed or uses a
    /// disallowed scheme / missing host.
    pub fn try_new(raw: &str) -> Result<Self, crate::error::CliError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(crate::error::CliError::ProxyError {
                message: "proxy URL is empty".into(),
            });
        }
        let parsed = url::Url::parse(trimmed).map_err(|e| crate::error::CliError::ProxyError {
            message: format!("invalid proxy URL {}: {e}", mask_proxy_url(trimmed)),
        })?;
        let scheme = parsed.scheme();
        if !matches!(scheme, "http" | "https" | "socks5" | "socks5h") {
            return Err(crate::error::CliError::ProxyError {
                message: format!(
                    "unsupported proxy scheme '{scheme}' (allowed: http, https, socks5, socks5h)"
                ),
            });
        }
        if parsed.host_str().is_none() {
            return Err(crate::error::CliError::ProxyError {
                message: format!("proxy URL missing host: {}", mask_proxy_url(trimmed)),
            });
        }
        Ok(Self(parsed))
    }

    /// Borrow the canonical string form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Borrow the validated [`url::Url`] (no re-parse).
    #[must_use]
    pub fn as_url(&self) -> &url::Url {
        &self.0
    }
}

impl std::fmt::Display for ProxyUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for ProxyUrl {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl ProxyConfig {
    /// Builds the configuration from the `--proxy` and `--no-proxy` flags.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::ProxyError`] when `--proxy` is set to an invalid URL.
    pub fn try_from_options(
        proxy: Option<&str>,
        no_proxy: bool,
    ) -> Result<Self, crate::error::CliError> {
        if no_proxy {
            return Ok(Self::Disabled);
        }
        match proxy {
            Some(u) if !u.trim().is_empty() => Ok(Self::Url(ProxyUrl::try_new(u)?)),
            _ => Ok(Self::Unset),
        }
    }

    /// Infallible constructor used by tests when the URL is known-valid.
    #[cfg(test)]
    pub fn url_for_test(raw: &str) -> Self {
        Self::Url(ProxyUrl::try_new(raw).expect("test proxy URL must be valid"))
    }

    /// Returns `true` when an explicit proxy URL is configured.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Url(_))
    }
}

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

/// Masks credentials in a proxy URL for safe use in logs and error messages.
///
/// Transforms `http://user:password@proxy:8080` into `http://us***@proxy:8080`.
/// If the URL contains no credentials, returns the safe representation without userinfo.
pub(super) fn mask_proxy_url(raw_url: &str) -> String {
    match url::Url::parse(raw_url) {
        Ok(parsed) => {
            let user = parsed.username();
            let has_password = parsed.password().is_some();

            if user.is_empty() && !has_password {
                return format!(
                    "{}://{}{}",
                    parsed.scheme(),
                    parsed.host_str().unwrap_or("?"),
                    parsed.port().map(|p| format!(":{p}")).unwrap_or_default()
                );
            }

            let masked_user = if user.len() > 2 {
                format!("{}***", &user[..2])
            } else {
                format!("{user}***")
            };

            format!(
                "{}://{}@{}{}",
                parsed.scheme(),
                masked_user,
                parsed.host_str().unwrap_or("?"),
                parsed.port().map(|p| format!(":{p}")).unwrap_or_default()
            )
        }
        Err(_) => "***URL_MALFORMADA***".to_string(),
    }
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

