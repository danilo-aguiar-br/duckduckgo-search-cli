// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (string construction, no I/O)
//! DuckDuckGo search URL builders.
//!
//! Base URLs live in [`crate::endpoints`] (single source of truth). Env overrides:
//! CLI `--base-url-html|lite|serp` (process policy). Defaults END with a
//! slash (`/html/` and `/lite/`) because `DuckDuckGo` treats `/html` (without
//! slash) as a redirect.

use crate::endpoints::{html_base_url, lite_base_url, serp_base_url};
use crate::types::{Endpoint, SafeSearch, TimeFilter};

/// Builds the GET search URL with the appropriate query-string for a given endpoint.
///
/// Parameters:
/// - `q` — search query (URL-encoded).
/// - `kl` — region, format `{country}-{language}`.
/// - `kp` — safe-search (when present).
/// - `df` — time filter (when present).
pub fn build_search_url(
    query: &str,
    language: &str,
    country: &str,
    endpoint: Endpoint,
    time_filter: Option<TimeFilter>,
    safe_search: SafeSearch,
) -> String {
    let base = match endpoint {
        Endpoint::Html => html_base_url(),
        Endpoint::Lite => lite_base_url(),
    };
    let query_encoded = urlencoding::encode(query);
    let kl = format_kl(language, country);
    let mut url = String::with_capacity(base.len() + query_encoded.len() + kl.len() + 32);
    url.push_str(&base);
    url.push_str("?q=");
    url.push_str(&query_encoded);
    url.push_str("&kl=");
    url.push_str(&kl);
    if let Some(kp) = safe_search.as_param() {
        url.push_str("&kp=");
        url.push_str(kp);
    }
    if let Some(df) = time_filter {
        url.push_str("&df=");
        url.push_str(df.as_param());
    }
    url
}

/// Simplified version from iteration 1 — kept for backward compatibility with older tests.
pub fn build_url(query: &str, language: &str, country: &str) -> String {
    build_search_url(
        query,
        language,
        country,
        Endpoint::Html,
        None,
        SafeSearch::Moderate,
    )
}

/// Builds the GET URL for the `DuckDuckGo` news vertical SERP
/// (`ia=news&iar=news`). GAP-WS-104 v0.8.9.
///
/// The news vertical lives on the main SERP (`duckduckgo.com`) — the
/// html/lite endpoints have no news vertical — and requires JavaScript,
/// so this URL is only ever navigated via the Chrome transport.
///
/// Parameters mirror [`build_search_url`]: `q` (URL-encoded), `kl`
/// (region), optional `kp` (safe-search) and `df` (time filter).
pub fn build_news_search_url(
    query: &str,
    language: &str,
    country: &str,
    time_filter: Option<TimeFilter>,
    safe_search: SafeSearch,
) -> String {
    let base = serp_base_url();
    let query_encoded = urlencoding::encode(query);
    let kl = format_kl(language, country);
    let mut url = String::with_capacity(base.len() + query_encoded.len() + kl.len() + 48);
    url.push_str(&base);
    url.push_str("?q=");
    url.push_str(&query_encoded);
    url.push_str("&ia=news&iar=news");
    url.push_str("&kl=");
    url.push_str(&kl);
    if let Some(kp) = safe_search.as_param() {
        url.push_str("&kp=");
        url.push_str(kp);
    }
    if let Some(df) = time_filter {
        url.push_str("&df=");
        url.push_str(df.as_param());
    }
    url
}

/// Formats the `DuckDuckGo` `kl` parameter as `{country}-{language}` in lowercase.
///
/// `DuckDuckGo` expects `kl` with the country in lowercase, followed by a hyphen and language
/// in lowercase. Uppercase inputs are normalized.
///
/// # Exemplo
///
/// ```
/// use duckduckgo_search_cli::search::format_kl;
///
/// assert_eq!(format_kl("pt", "br"), "br-pt");
/// assert_eq!(format_kl("EN", "US"), "us-en"); // normalizes uppercase input
/// ```
// Small pure helper — `#[inline]` for monomorphization/inlining across crates; not `always`.
#[inline]
pub fn format_kl(language: &str, country: &str) -> String {
    let mut kl = String::with_capacity(country.len() + language.len() + 1);
    for ch in country.chars() {
        kl.push(ch.to_ascii_lowercase());
    }
    kl.push('-');
    for ch in language.chars() {
        kl.push(ch.to_ascii_lowercase());
    }
    kl
}
