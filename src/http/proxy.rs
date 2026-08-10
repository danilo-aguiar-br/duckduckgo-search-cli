// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative / pure (URL parse + validation; no network I/O)
//! Transport-neutral proxy configuration.
//!
//! GAP-WS-113: `--proxy` / `--no-proxy` apply to **both** transports — the
//! Chrome subprocess (`--proxy-server`) and the residual `reqwest` client. The
//! types therefore live outside `client`, which only compiles under
//! `http-test-harness`. Split out of `client` so `no-default-features` builds
//! keep `Config::proxy_config` without pulling in `reqwest` / `rustls`.

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
    /// Returns [`crate::error::CliError::ProxyError`] when the URL is malformed
    /// or uses a disallowed scheme / missing host.
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
    /// Returns [`crate::error::CliError::ProxyError`] when `--proxy` is set to
    /// an invalid URL.
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
