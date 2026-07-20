// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (async DNS) + CPU (structural URL checks)
//! SSRF gate for attacker-influenced fetch URLs (SERP links / redirects).
//!
//! Shared by residual HTTP content extract **and** Chrome `--fetch-content`
//! navigation (GAP-SCRAPE-015). Skip is limited to the HTTP test harness
//! (feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`) —
//! never a product env toggle (GAP-SCRAPE-008).

use std::net::IpAddr;

/// Validates that a URL is structurally safe to fetch (SSRF protection).
///
/// Rejects non-HTTP schemes (`file://`, `ftp://`, `data:`, etc.),
/// `localhost`, and hosts that are **literal** private/loopback/link-local/
/// CGNAT/documentation/multicast addresses (IPv4 + IPv6 ULA/link-local).
///
/// Hostname targets pass the structural check; use [`url_is_safe_to_fetch`]
/// for the async DNS resolution gate (blocks rebinding to private ranges).
///
/// Public to `crate` so [`crate::http`] can apply the same filter on redirect hops.
pub(crate) fn is_safe_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };

    match parsed.scheme() {
        "http" | "https" => {}
        _ => return false,
    }

    let host = match parsed.host_str() {
        Some(h) => h,
        None => return false,
    };

    // Case-insensitive localhost / .localhost (RFC 6761).
    let host_lower = host.to_ascii_lowercase();
    if host_lower == "localhost" || host_lower.ends_with(".localhost") {
        return false;
    }

    // mDNS / link-local name — never fetch via content pipeline.
    if host_lower.ends_with(".local") {
        return false;
    }

    let host_clean = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = host_clean.parse::<IpAddr>() {
        return !is_blocked_ip(ip);
    }

    true
}

/// Returns `true` when the IP must not be contacted by `--fetch-content`.
fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_documentation()
                || is_cgnat_v4(v4)
                || v4.octets()[0] == 0 // 0.0.0.0/8
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.is_multicast()
                || is_documentation_v6(v6)
            {
                return true;
            }
            // IPv4-mapped IPv6 (::ffff:x.x.x.x) — re-check embedded v4.
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_blocked_ip(IpAddr::V4(v4));
            }
            false
        }
    }
}

/// Carrier-grade NAT (RFC 6598) `100.64.0.0/10`.
fn is_cgnat_v4(v4: std::net::Ipv4Addr) -> bool {
    let o = v4.octets();
    o[0] == 100 && (o[1] & 0b1100_0000) == 64
}

/// IPv6 documentation prefix `2001:db8::/32` (RFC 3849).
///
/// Implemented manually: `Ipv6Addr::is_documentation` is still unstable
/// (`feature(ip)` / issue #27709) on the crate MSRV.
fn is_documentation_v6(v6: std::net::Ipv6Addr) -> bool {
    let seg = v6.segments();
    seg[0] == 0x2001 && seg[1] == 0x0db8
}

/// Async SSRF gate: structural check + DNS resolution of every A/AAAA record.
///
/// Fail-closed: resolution errors reject the URL (no blind connect).
/// Skipped only when the HTTP test harness is active (GAP-SCRAPE-008).
pub(crate) async fn url_is_safe_to_fetch(url: &str) -> bool {
    if crate::chrome_policy::http_test_harness_active() {
        return true;
    }
    if !is_safe_url(url) {
        return false;
    }
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };
    let host = match parsed.host_str() {
        Some(h) => h.to_string(),
        None => return false,
    };
    // Literal IPs already validated in `is_safe_url`.
    let host_clean = host.trim_start_matches('[').trim_end_matches(']');
    if host_clean.parse::<IpAddr>().is_ok() {
        return true;
    }
    let port = parsed
        .port_or_known_default()
        .unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
    host_resolves_public(&host, port).await
}

/// Resolves `host:port` and requires **every** address to be non-blocked.
///
/// Uses Tokio async DNS (`lookup_host`) — never blocks a worker on getaddrinfo.
async fn host_resolves_public(host: &str, port: u16) -> bool {
    let addrs = match tokio::net::lookup_host((host, port)).await {
        Ok(iter) => iter,
        Err(err) => {
            tracing::debug!(host, %err, "SSRF DNS resolve failed — rejecting URL");
            return false;
        }
    };
    let mut any = false;
    for addr in addrs {
        any = true;
        if is_blocked_ip(addr.ip()) {
            tracing::warn!(
                host,
                ip = %addr.ip(),
                "SSRF: hostname resolves to blocked address — rejecting"
            );
            return false;
        }
    }
    // No addresses → fail closed.
    any
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssrf_rejects_file_scheme() {
        assert!(!is_safe_url("file:///etc/passwd"));
    }

    #[test]
    fn ssrf_rejects_ftp_scheme() {
        assert!(!is_safe_url("ftp://internal.corp/data"));
    }

    #[test]
    fn ssrf_rejects_data_scheme() {
        assert!(!is_safe_url("data:text/html,<h1>hi</h1>"));
    }

    #[test]
    fn ssrf_rejects_loopback_ipv4() {
        assert!(!is_safe_url("http://127.0.0.1/secret"));
    }

    #[test]
    fn ssrf_rejects_private_range_10() {
        assert!(!is_safe_url("http://10.0.0.1/internal"));
    }

    #[test]
    fn ssrf_rejects_private_range_192() {
        assert!(!is_safe_url("http://192.168.1.1/admin"));
    }

    #[test]
    fn ssrf_rejects_link_local() {
        assert!(!is_safe_url("http://169.254.169.254/metadata"));
    }

    #[test]
    fn ssrf_rejects_localhost() {
        assert!(!is_safe_url("http://localhost/admin"));
    }

    #[test]
    fn ssrf_accepts_public_https() {
        assert!(is_safe_url("https://www.example.com/page"));
    }

    #[test]
    fn ssrf_accepts_public_http() {
        assert!(is_safe_url("http://example.com/page"));
    }

    #[test]
    fn ssrf_rejects_ipv6_loopback() {
        assert!(!is_safe_url("http://[::1]/secret"));
    }

    #[test]
    fn ssrf_rejects_ipv6_ula() {
        assert!(!is_safe_url("http://[fd12:3456:789a::1]/internal"));
    }

    #[test]
    fn ssrf_rejects_ipv6_link_local() {
        assert!(!is_safe_url("http://[fe80::1]/local"));
    }

    #[test]
    fn ssrf_rejects_cgnat() {
        assert!(!is_safe_url("http://100.64.0.1/cgnat"));
    }

    #[test]
    fn ssrf_rejects_documentation_v4() {
        assert!(!is_safe_url("http://203.0.113.10/docs"));
    }

    #[test]
    fn ssrf_rejects_multicast() {
        assert!(!is_safe_url("http://224.0.0.1/mcast"));
    }

    #[test]
    fn ssrf_rejects_dot_local_mdns() {
        assert!(!is_safe_url("http://printer.local/status"));
    }

    #[test]
    fn ssrf_rejects_localhost_suffix() {
        assert!(!is_safe_url("http://app.localhost/admin"));
    }

    #[test]
    fn ssrf_rejects_ipv4_mapped_loopback() {
        assert!(!is_safe_url("http://[::ffff:127.0.0.1]/secret"));
    }

    #[tokio::test]
    async fn ssrf_async_rejects_localhost_resolve() {
        // Structural reject without needing DNS.
        assert!(!url_is_safe_to_fetch("http://localhost/x").await);
        assert!(!url_is_safe_to_fetch("http://127.0.0.1/x").await);
    }
}
