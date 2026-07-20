// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: **declarative / pure** (no I/O fan-out).
//! DuckDuckGo endpoint defaults and runtime overrides (hardcode-prohibition).
//!
//! This module is the **single source of truth** for product network identity:
//! - compile-time named defaults (`const`, SCREAMING_SNAKE_CASE, explicit types)
//! - optional runtime injection via process policy (CLI / tests — **not** product env)
//! - hostnames used by filters / cookie domain / relative URL resolution
//!
//! Call sites must use these symbols instead of raw `"https://…duckduckgo.com…"`
//! string literals in production logic (rules-rust proibição de hardcode).
//!
//! # Runtime overrides (CLI / wiremock tests) — GAP-SCRAPE-R2-009
//!
//! Install once per process with [`set_endpoint_policy`]:
//! - CLI: `--base-url-html`, `--base-url-lite`, `--base-url-serp`
//! - Tests: `set_endpoint_policy(EndpointPolicy { … })` before exercising HTTP

use std::sync::Mutex;

/// Default base URL for the DuckDuckGo HTML SERP endpoint (trailing slash required).
pub const URL_ENDPOINT_HTML_DEFAULT: &str = "https://html.duckduckgo.com/html/";
/// Default base URL for the DuckDuckGo Lite endpoint (trailing slash required).
pub const URL_ENDPOINT_LITE_DEFAULT: &str = "https://lite.duckduckgo.com/lite/";
/// Default origin for the main SERP / news vertical / session warm-up.
pub const URL_SERP_DEFAULT: &str = "https://duckduckgo.com/";
/// Origin used as HTTP `Referer` on HTML pagination POSTs.
pub const URL_HTML_REFERER_DEFAULT: &str = "https://html.duckduckgo.com/";
/// Canonical cookie / relative-path origin (scheme + host, no path).
pub const URL_DDG_ORIGIN: &str = "https://duckduckgo.com";

/// Registrable domain used for cookie jar projection and host filters.
pub const HOST_DDG: &str = "duckduckgo.com";
/// HTML endpoint host.
pub const HOST_DDG_HTML: &str = "html.duckduckgo.com";
/// Lite endpoint host.
pub const HOST_DDG_LITE: &str = "lite.duckduckgo.com";
/// Suffix match for any `*.duckduckgo.com` host (includes leading dot).
pub const HOST_DDG_SUFFIX: &str = ".duckduckgo.com";

/// Process-wide endpoint overrides (CLI / tests). `None` fields use defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EndpointPolicy {
    /// Override for [`html_base_url`].
    pub html: Option<String>,
    /// Override for [`lite_base_url`].
    pub lite: Option<String>,
    /// Override for [`serp_base_url`].
    pub serp: Option<String>,
}

static ENDPOINT_POLICY: Mutex<EndpointPolicy> = Mutex::new(EndpointPolicy {
    html: None,
    lite: None,
    serp: None,
});

/// Install process-wide endpoint overrides (CLI / wiremock tests).
pub fn set_endpoint_policy(policy: EndpointPolicy) {
    if let Ok(mut guard) = ENDPOINT_POLICY.lock() {
        *guard = policy;
    }
}

/// Current endpoint policy (for tests / diagnostics).
#[must_use]
pub fn endpoint_policy() -> EndpointPolicy {
    ENDPOINT_POLICY
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
}

fn policy_snapshot() -> EndpointPolicy {
    endpoint_policy()
}

/// Effective HTML base URL (policy override or [`URL_ENDPOINT_HTML_DEFAULT`]).
#[must_use]
pub fn html_base_url() -> String {
    policy_snapshot()
        .html
        .unwrap_or_else(|| URL_ENDPOINT_HTML_DEFAULT.to_string())
}

/// Effective Lite base URL (policy override or [`URL_ENDPOINT_LITE_DEFAULT`]).
#[must_use]
pub fn lite_base_url() -> String {
    policy_snapshot()
        .lite
        .unwrap_or_else(|| URL_ENDPOINT_LITE_DEFAULT.to_string())
}

/// Effective SERP / warm-up base URL (policy override or [`URL_SERP_DEFAULT`]).
#[must_use]
pub fn serp_base_url() -> String {
    policy_snapshot()
        .serp
        .unwrap_or_else(|| URL_SERP_DEFAULT.to_string())
}

/// Referer for HTML pagination. Derived from the HTML base when possible;
/// falls back to [`URL_HTML_REFERER_DEFAULT`].
#[must_use]
pub fn html_referer() -> String {
    let base = html_base_url();
    // Prefer origin of the effective base (so wiremock overrides stay consistent).
    if let Ok(parsed) = url::Url::parse(&base) {
        if let Some(host) = parsed.host_str() {
            let scheme = parsed.scheme();
            return match parsed.port() {
                Some(port) => format!("{scheme}://{host}:{port}/"),
                None => format!("{scheme}://{host}/"),
            };
        }
    }
    URL_HTML_REFERER_DEFAULT.to_string()
}

/// Origin used to resolve root-relative DuckDuckGo paths (`/l/?uddg=…`).
///
/// Uses the SERP base URL host so test overrides apply; scheme defaults to https.
#[must_use]
pub fn ddg_origin_for_relative() -> String {
    let base = serp_base_url();
    if let Ok(parsed) = url::Url::parse(&base) {
        if let Some(host) = parsed.host_str() {
            let scheme = parsed.scheme();
            return format!("{scheme}://{host}");
        }
    }
    URL_DDG_ORIGIN.to_string()
}

/// Returns true when `host` is duckduckgo.com or a subdomain thereof.
#[must_use]
pub fn is_ddg_host(host: &str) -> bool {
    let host = host.trim();
    if host.is_empty() {
        return false;
    }
    host.eq_ignore_ascii_case(HOST_DDG)
        || host.eq_ignore_ascii_case(HOST_DDG_HTML)
        || host.eq_ignore_ascii_case(HOST_DDG_LITE)
        || (host.len() > HOST_DDG_SUFFIX.len()
            && host[host.len() - HOST_DDG_SUFFIX.len()..].eq_ignore_ascii_case(HOST_DDG_SUFFIX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_https_and_end_with_slash_where_required() {
        assert!(URL_ENDPOINT_HTML_DEFAULT.starts_with("https://"));
        assert!(URL_ENDPOINT_HTML_DEFAULT.ends_with('/'));
        assert!(URL_ENDPOINT_LITE_DEFAULT.starts_with("https://"));
        assert!(URL_ENDPOINT_LITE_DEFAULT.ends_with('/'));
        assert!(URL_SERP_DEFAULT.starts_with("https://"));
        assert!(URL_SERP_DEFAULT.ends_with('/'));
        assert!(!URL_DDG_ORIGIN.ends_with('/'));
    }

    #[test]
    fn is_ddg_host_accepts_product_hosts() {
        assert!(is_ddg_host("duckduckgo.com"));
        assert!(is_ddg_host("html.duckduckgo.com"));
        assert!(is_ddg_host("lite.duckduckgo.com"));
        assert!(is_ddg_host("external-content.duckduckgo.com"));
        assert!(!is_ddg_host("example.com"));
        assert!(!is_ddg_host("notduckduckgo.com"));
    }

    #[test]
    fn accessors_return_non_empty() {
        set_endpoint_policy(EndpointPolicy::default());
        assert!(!html_base_url().is_empty());
        assert!(!lite_base_url().is_empty());
        assert!(!serp_base_url().is_empty());
        assert!(!html_referer().is_empty());
        assert!(!ddg_origin_for_relative().is_empty());
    }

    #[test]
    fn policy_overrides_html_base() {
        set_endpoint_policy(EndpointPolicy {
            html: Some("http://127.0.0.1:9/html/".into()),
            lite: None,
            serp: None,
        });
        assert_eq!(html_base_url(), "http://127.0.0.1:9/html/");
        assert_eq!(html_referer(), "http://127.0.0.1:9/");
        set_endpoint_policy(EndpointPolicy::default());
    }
}
