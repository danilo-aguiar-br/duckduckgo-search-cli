// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure (URL resolve / uddg unwrap)
//! DuckDuckGo redirect URL resolution helpers.

use crate::types::HttpUrl;

/// Resolves a URL found in the `DuckDuckGo` DOM to a validated absolute HTTP(S) URL.
///
/// Handled cases:
/// 1. `//example.com/path` → `https://example.com/path` (protocol-relative).
/// 2. `/l/?uddg=<REAL_URL>&rut=...` → decodes `uddg` and returns the real URL.
/// 3. `//duckduckgo.com/l/?uddg=...` → same logic after normalisation.
/// 4. Absolute external URLs are validated as [`HttpUrl`].
/// 5. URLs on the `duckduckgo.com` domain itself (except `/l/?uddg=`) are filtered.
///
/// Returns `None` if the URL is invalid, non-http(s), or belongs to `DuckDuckGo`.
pub fn resolve_url(href: &str) -> Option<HttpUrl> {
    let href_trim = href.trim();
    if href_trim.is_empty() {
        return None;
    }

    // Case 1: protocol-relative.
    let normalized = if let Some(rest) = href_trim.strip_prefix("//") {
        format!("https://{rest}")
    } else if href_trim.starts_with('/') {
        // Case 2: relative DuckDuckGo path (e.g., "/l/?uddg=...").
        // Origin from endpoints module (env-overridable SERP base).
        format!("{}{href_trim}", crate::endpoints::ddg_origin_for_relative())
    } else {
        href_trim.to_string()
    };

    // Case 3: DuckDuckGo redirect with `uddg` parameter.
    if let Some(uddg_decoded) = extract_uddg(&normalized) {
        return HttpUrl::try_new(&uddg_decoded).ok();
    }

    // Case 4: filter URLs from DuckDuckGo itself (without uddg).
    if is_duckduckgo_url(&normalized) {
        return None;
    }

    HttpUrl::try_new(&normalized).ok()
}

/// If the URL is a `DuckDuckGo` redirect (`/l/?uddg=<REAL_URL>`), extracts and
/// URL-decodes `uddg`. Returns `None` if it is not a redirect or the parameter is absent.
pub(crate) fn extract_uddg(url: &str) -> Option<String> {
    // Search for "uddg=" in the query string.
    let idx_uddg = url.find("uddg=")?;
    let after_equals = &url[idx_uddg + "uddg=".len()..];
    // The uddg value extends to the next `&` or end of string.
    let encoded_value = match after_equals.find('&') {
        Some(end) => &after_equals[..end],
        None => after_equals,
    };
    urlencoding::decode(encoded_value)
        .ok()
        .map(|cow| cow.into_owned())
}

/// Checks whether the URL points to any subdomain of `DuckDuckGo`.
pub(crate) fn is_duckduckgo_url(url: &str) -> bool {
    let after_proto = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };
    let host = after_proto
        .split('/')
        .next()
        .unwrap_or(after_proto)
        .split('?')
        .next()
        .unwrap_or(after_proto);
    crate::endpoints::is_ddg_host(host)
}
