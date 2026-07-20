// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (reqwest client construction and UA management)
//! `reqwest::Client` construction and User-Agent selection.
//!
//! Residual HTTP only (`http-test-harness` / tests). Production SERP uses Chrome
//! CDP (ADR-0016). The client is configured with:
//! - TLS via **rustls** + process `CryptoProvider` (`aws-lc-rs`, see
//!   [`crate::tls_bootstrap`]) and Mozilla `webpki-roots` — no OpenSSL / native-tls.
//! - Cookie store enabled (required for pagination with `vqd` token).
//! - `gzip` + `deflate` Accept-Encoding (matches enabled reqwest features; no `br`).
//! - HTTP/2 when the peer negotiates it (`http2` feature).
//! - Redirect policy limited to 5 hops with structural SSRF checks on Location.
//! - TCP_NODELAY, TCP keepalive, connect timeout, pool idle timeout.
//! - Headers that mimic a real browser with full family profile (Chrome, Firefox, Safari, Edge).
//! - Configurable total timeout.
//! - Optional HTTP/HTTPS/SOCKS5 proxy **only** via [`ProxyConfig::Url`] / `--proxy`
//!   (never silent `HTTP_PROXY` env inheritance — XDG/CLI config only).
//! - User-Agents loaded from external `user-agents.toml` OR built-in defaults.
//!
//! ## Browser Profiles (v0.6.0)
//!
//! Each loaded UA receives a [`BrowserProfile`] that encapsulates the detected family
//! (`Chrome`, `Firefox`, `Safari`, `Edge`) and generates complete Sec-Fetch headers.
//! Chrome and Edge also emit Client Hints (`Sec-CH-UA*`), exactly replicating
//! the behavior of real browsers and reducing anti-bot detection.

//!
//! ## Layout (GAP-E2E-51-011 — SRP split)
//!
//! | Submodule | Responsibility |
//! |-----------|----------------|
//! | [`profile`] | [`BrowserFamily`] / [`BrowserProfile`] / UA pool selection |
//! | [`client`] | [`ProxyConfig`] / [`ProxyUrl`] / `build_client*` |

mod client;
mod profile;

pub use client::{
    build_client, build_client_with_proxy, build_client_with_proxy_and_cookies,
    maybe_build_residual_client, ProxyConfig, ProxyUrl,
};
pub use profile::{
    create_browser_profile, detect_family, load_user_agents, select_profile_from_list,
    select_profile_from_list_seeded, select_random_user_agent, select_user_agent,
    select_user_agent_from_list, BrowserFamily, BrowserProfile,
};

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use super::client::{
        mask_proxy_url, CONNECT_TIMEOUT_SECS, POOL_IDLE_TIMEOUT_SECS, POOL_MAX_IDLE_PER_HOST,
        REDIRECT_LIMIT, TCP_KEEPALIVE_SECS,
    };
    use super::profile::{
        extract_major_version, ACCEPT_ENCODING_SUPPORTED, USER_AGENTS_DEFAULT,
    };
    use reqwest::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE};

    // --- Testes existentes ---------------------------------------------------

    #[test]
    fn choose_user_agent_returns_non_empty_string() {
        let ua = select_user_agent();
        assert!(!ua.is_empty());
    }

    #[test]
    fn choose_user_agent_returns_modern_ua_from_pool() {
        let ua = select_user_agent();
        assert!(
            USER_AGENTS_DEFAULT.contains(&ua.as_str()),
            "UA selecionado deve estar na lista padrão: {ua}"
        );
        assert!(
            ua.starts_with("Mozilla/5.0 ("),
            "UAs padrão v0.3.0 iniciam with 'Mozilla/5.0 (' (browser real): {ua}"
        );
    }

    #[test]
    fn default_pool_contains_modern_browsers_in_all_families() {
        let pool = USER_AGENTS_DEFAULT;
        assert!(pool.iter().any(|ua| ua.contains("Chrome/")));
        assert!(pool.iter().any(|ua| ua.contains("Firefox/")));
        assert!(pool.iter().any(|ua| ua.contains("Edg/")));
        assert!(pool
            .iter()
            .any(|ua| ua.contains("Safari/") && !ua.contains("Chrome/")));
    }

    #[test]
    fn default_pool_does_not_contain_removed_text_browsers() {
        for ua in USER_AGENTS_DEFAULT {
            assert!(!ua.contains("Lynx"), "UA banido detectado (Lynx): {ua}");
            assert!(!ua.contains("w3m"), "UA banido detectado (w3m): {ua}");
            assert!(
                !ua.starts_with("Links ("),
                "UA banido detectado (Links): {ua}"
            );
            assert!(!ua.contains("ELinks"), "UA banido detectado (ELinks): {ua}");
            assert!(
                !ua.starts_with("duckduckgo-search-cli"),
                "UA banido detectado (self-cli): {ua}"
            );
            assert_ne!(
                *ua, "Mozilla/5.0",
                "UA minimalista 'Mozilla/5.0' deve ter sido removido"
            );
        }
        assert!(!USER_AGENTS_DEFAULT.is_empty());
    }

    #[test]
    fn select_random_user_agent_without_exclusion_returns_valid() {
        let ua = select_random_user_agent(None);
        assert!(!ua.is_empty());
    }

    #[test]
    fn select_random_user_agent_avoids_excluded_when_possible() {
        let excluded = USER_AGENTS_DEFAULT[0];
        for _ in 0..20 {
            let ua = select_random_user_agent(Some(excluded));
            assert_ne!(ua, excluded);
            assert!(!ua.is_empty());
        }
    }

    #[test]
    fn build_client_with_valid_values_works() {
        crate::tls_bootstrap::ensure_for_tests();
        let client = build_client("Mozilla/5.0 teste", 15, "pt", "br");
        assert!(client.is_ok(), "cliente deve ser construído without erro");
    }

    #[test]
    fn accept_encoding_does_not_advertise_unsupported_br() {
        assert_eq!(ACCEPT_ENCODING_SUPPORTED, "gzip, deflate");
        assert!(!ACCEPT_ENCODING_SUPPORTED.contains("br"));
        let profile = create_browser_profile(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
        );
        let headers = profile.initial_headers("en", "us").expect("headers");
        let ae = headers
            .get(ACCEPT_ENCODING)
            .expect("Accept-Encoding present")
            .to_str()
            .expect("ascii");
        assert_eq!(ae, "gzip, deflate");
        assert!(!ae.split(',').any(|t| t.trim() == "br"));
    }

    #[test]
    fn pool_and_timeout_constants_are_positive() {
        assert!(TCP_KEEPALIVE_SECS > 0);
        assert!(CONNECT_TIMEOUT_SECS > 0);
        assert!(POOL_MAX_IDLE_PER_HOST > 0);
        assert!(POOL_IDLE_TIMEOUT_SECS > 0);
        assert!(REDIRECT_LIMIT > 0);
    }

    #[test]
    fn build_client_with_http_proxy_works() {
        crate::tls_bootstrap::ensure_for_tests();
        let profile = create_browser_profile("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36");
        let proxy = ProxyConfig::url_for_test("http://user:pass@proxy.local:8080");
        let client = build_client_with_proxy(&profile, 10, "pt", "br", &proxy);
        assert!(client.is_ok(), "client with HTTP proxy should build");
    }

    #[test]
    fn build_client_with_socks5_proxy_works() {
        crate::tls_bootstrap::ensure_for_tests();
        let profile = create_browser_profile(
            "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
        );
        let proxy = ProxyConfig::url_for_test("socks5://127.0.0.1:9050");
        let client = build_client_with_proxy(&profile, 10, "pt", "br", &proxy);
        assert!(client.is_ok(), "client with SOCKS5 should build");
    }

    #[test]
    fn build_client_with_no_proxy_works() {
        crate::tls_bootstrap::ensure_for_tests();
        let profile = create_browser_profile(
            "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
        );
        let proxy = ProxyConfig::Disabled;
        let client = build_client_with_proxy(&profile, 10, "pt", "br", &proxy);
        assert!(client.is_ok(), "client with no_proxy should build");
    }

    #[test]
    fn proxy_unset_disables_env_proxy_inheritance() {
        // GAP-TLS-009: Unset applies no_proxy — same builder path as Disabled for env.
        crate::tls_bootstrap::ensure_for_tests();
        let profile = create_browser_profile(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
        );
        let client = build_client_with_proxy(&profile, 10, "pt", "br", &ProxyConfig::Unset);
        assert!(client.is_ok(), "Unset must build a client without env proxy");
    }

    #[test]
    fn build_client_with_invalid_proxy_url_fails() {
        // Invalid raw URL is rejected at ProxyUrl::try_new (parse-don't-validate boundary).
        assert!(ProxyUrl::try_new("nao eh uma url").is_err());
        assert!(ProxyUrl::try_new("file:///etc/passwd").is_err());
        assert!(ProxyConfig::try_from_options(Some("ftp://bad"), false).is_err());
    }

    #[test]
    fn proxy_config_from_flags() {
        assert_eq!(ProxyConfig::try_from_options(None, false).unwrap(), ProxyConfig::Unset);
        assert_eq!(ProxyConfig::try_from_options(None, true).unwrap(), ProxyConfig::Disabled);
        assert_eq!(
            ProxyConfig::try_from_options(Some("http://x:9"), false).unwrap(),
            ProxyConfig::url_for_test("http://x:9")
        );
        assert_eq!(
            ProxyConfig::try_from_options(Some("http://x:9"), true).unwrap(),
            ProxyConfig::Disabled
        );
    }

    #[test]
    fn proxy_config_is_active_only_for_url() {
        assert!(!ProxyConfig::Unset.is_active());
        assert!(!ProxyConfig::Disabled.is_active());
        assert!(ProxyConfig::url_for_test("http://x").is_active());
    }

    #[test]
    fn mask_proxy_url_with_credentials() {
        let result = mask_proxy_url("http://admin:s3cret@proxy.local:8080");
        assert!(!result.contains("s3cret"), "password vazou: {result}");
        assert!(
            !result.contains("admin"),
            "username completo vazou: {result}"
        );
        assert!(
            result.contains("ad***"),
            "username mascarado ausente: {result}"
        );
        assert!(result.contains("proxy.local"));
        assert!(result.contains("8080"));
    }

    #[test]
    fn mask_proxy_url_without_credentials() {
        let result = mask_proxy_url("http://proxy.local:8080");
        assert_eq!(result, "http://proxy.local:8080");
    }

    #[test]
    fn mask_proxy_url_username_only() {
        let result = mask_proxy_url("http://user@proxy.local:3128");
        assert!(result.contains("us***"));
        assert!(!result.contains("user@"));
    }

    #[test]
    fn mask_proxy_url_malformed() {
        let result = mask_proxy_url("not-a-url");
        assert_eq!(result, "***URL_MALFORMADA***");
    }

    #[test]
    fn mask_proxy_url_socks5() {
        let result = mask_proxy_url("socks5://root:toor@127.0.0.1:1080");
        assert!(!result.contains("toor"));
        assert!(result.contains("socks5://"));
        assert!(result.contains("127.0.0.1"));
    }

    #[test]
    fn mask_proxy_url_short_username() {
        let result = mask_proxy_url("http://a:pass@proxy:80");
        assert!(result.contains("a***"));
        assert!(!result.contains("pass"));
    }

    #[test]
    fn load_user_agents_returns_at_least_one_default() {
        let agents = load_user_agents(false);
        assert!(!agents.is_empty());
        for ua in &agents {
            assert!(!ua.is_empty());
        }
    }

    #[test]
    fn choose_user_agent_from_list_returns_list_item() {
        let agents = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        for _ in 0..10 {
            let selected = select_user_agent_from_list(&agents);
            assert!(agents.contains(&selected));
        }
    }

    // --- Testes novos: BrowserProfile -----------------------------------------

    #[test]
    fn detect_family_chrome() {
        let uas_chrome = [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
        ];
        for ua in &uas_chrome {
            assert_eq!(
                detect_family(ua),
                BrowserFamily::Chrome,
                "esperado Chrome para: {ua}"
            );
        }
    }

    #[test]
    fn detect_family_edge_before_chrome() {
        // Edge UA contains "Chrome/" but must return Edge because it has "Edg/" first
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.3800.97";
        assert_eq!(detect_family(ua), BrowserFamily::Edge);
    }

    #[test]
    fn detect_family_firefox() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        assert_eq!(detect_family(ua), BrowserFamily::Firefox);
    }

    #[test]
    fn detect_family_safari() {
        // Pure Safari does not contain "Chrome/"
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15";
        assert_eq!(detect_family(ua), BrowserFamily::Safari);
    }

    #[test]
    fn extract_major_version_chrome_146() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let version = extract_major_version(ua, BrowserFamily::Chrome);
        assert_eq!(version, 146, "versão major Chrome deve ser 146");
    }

    #[test]
    fn extract_major_version_firefox_134() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        let version = extract_major_version(ua, BrowserFamily::Firefox);
        assert_eq!(version, 134, "versão major Firefox deve ser 134");
    }

    #[test]
    fn initial_chrome_headers_include_sec_fetch() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("pt", "br")
            .expect("should build headers");
        assert!(
            headers.contains_key("sec-fetch-dest"),
            "sec-fetch-dest ausente"
        );
        assert!(
            headers.contains_key("sec-fetch-mode"),
            "sec-fetch-mode ausente"
        );
        assert!(
            headers.contains_key("sec-fetch-site"),
            "sec-fetch-site ausente"
        );
        assert!(
            headers.contains_key("sec-fetch-user"),
            "sec-fetch-user ausente"
        );
    }

    #[test]
    fn initial_chrome_headers_include_client_hints() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("pt", "br")
            .expect("should build headers");
        assert!(headers.contains_key("sec-ch-ua"), "sec-ch-ua ausente");
        assert!(
            headers.contains_key("sec-ch-ua-mobile"),
            "sec-ch-ua-mobile ausente"
        );
        assert!(
            headers.contains_key("sec-ch-ua-platform"),
            "sec-ch-ua-platform ausente"
        );
    }

    #[test]
    fn initial_firefox_headers_omit_client_hints() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("pt", "br")
            .expect("should build headers");
        assert!(
            !headers.contains_key("sec-ch-ua"),
            "Firefox must NOT ter sec-ch-ua"
        );
        assert!(
            !headers.contains_key("sec-ch-ua-mobile"),
            "Firefox must NOT ter sec-ch-ua-mobile"
        );
        assert!(
            !headers.contains_key("sec-ch-ua-platform"),
            "Firefox must NOT ter sec-ch-ua-platform"
        );
    }

    #[test]
    fn pagination_headers_sec_fetch_site_same_origin() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile.pagination_headers();
        let value = headers
            .get("sec-fetch-site")
            .expect("sec-fetch-site should be present");
        assert_eq!(value.to_str().unwrap(), "same-origin");
    }

    #[test]
    fn accept_language_with_q_values_pt() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("pt", "br")
            .expect("should build headers");
        let al = headers
            .get(ACCEPT_LANGUAGE)
            .expect("Accept-Language present");
        let al_str = al.to_str().unwrap();
        assert!(al_str.contains("pt-BR"), "deve conter pt-BR: {al_str}");
        assert!(
            al_str.contains("pt;q=0.9"),
            "deve conter pt;q=0.9: {al_str}"
        );
        assert!(
            al_str.contains("en-US;q=0.8"),
            "deve conter en-US;q=0.8: {al_str}"
        );
        assert!(
            al_str.contains("en;q=0.7"),
            "deve conter en;q=0.7: {al_str}"
        );
    }

    #[test]
    fn accept_language_with_q_values_en() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("en", "us")
            .expect("should build headers");
        let al = headers
            .get(ACCEPT_LANGUAGE)
            .expect("Accept-Language present");
        let al_str = al.to_str().unwrap();
        assert_eq!(
            al_str, "en-US,en;q=0.9",
            "formato en deve ser simplificado: {al_str}"
        );
    }

    // Testes existentes atualizados para usar BrowserProfile

    #[test]
    fn default_headers_include_accept_and_language() {
        // Teste atualizado para usar BrowserProfile em vez de headers_padrao()
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("pt", "br")
            .expect("should build headers");
        let accept = headers.get(ACCEPT).expect("ACCEPT present");
        assert!(accept.to_str().unwrap().contains("text/html"));
        let al = headers
            .get(ACCEPT_LANGUAGE)
            .expect("ACCEPT_LANGUAGE present");
        assert!(al.to_str().unwrap().contains("pt-BR"));
    }

    #[test]
    fn default_headers_omit_dnt_and_referer() {
        // Empirical finding iter. 4: persistent DNT + Referer reveal automation profile.
        // Updated to use BrowserProfile.
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("en", "us")
            .expect("should build headers");
        assert!(headers.get(reqwest::header::DNT).is_none());
        assert!(headers.get(reqwest::header::REFERER).is_none());
    }
}

