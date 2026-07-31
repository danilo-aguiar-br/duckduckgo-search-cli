// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests for this module.

use super::*;

    /// Builds a `Config` with safe defaults for unit tests that do not
    /// care about every field. The body size matches the original
    /// `Config::default()` so the struct stays in sync if a field is
    /// added or removed.
    fn test_config_empty() -> Config {
        let mut cfg = Config::default();
        cfg.fetch_content = false;
        cfg.warmup_enabled = false;
        cfg.retries = crate::types::RetryBudget::try_new(2).expect("retries");
        cfg.pages = crate::types::PageCount::try_new(1).expect("pages");
        cfg
    }

    #[test]
    fn format_kl_concatenates_correctly() {
        assert_eq!(format_kl("pt", "br"), "br-pt");
        assert_eq!(format_kl("PT", "BR"), "br-pt");
        assert_eq!(format_kl("en", "us"), "us-en");
    }

    #[test]
    fn build_url_escapes_spaces_and_accents() {
        let url = build_url("endividamento brasileiro", "pt", "br");
        assert!(url.starts_with("https://html.duckduckgo.com/html/?q="));
        assert!(url.contains("endividamento%20brasileiro"));
        assert!(url.contains("&kl=br-pt"));
    }

    #[test]
    fn build_url_escapes_special_characters() {
        let url = build_url("C++ tutorial", "en", "us");
        assert!(url.contains("C%2B%2B"));
        assert!(url.contains("&kl=us-en"));
    }

    #[test]
    fn build_url_with_portuguese_accents() {
        let url = build_url("música eletrônica", "pt", "br");
        assert!(url.contains("m%C3%BAsica"));
        assert!(url.contains("eletr%C3%B4nica"));
    }

    #[test]
    fn build_search_url_adds_optional_params() {
        let url = build_search_url(
            "rust",
            "en",
            "us",
            Endpoint::Html,
            Some(TimeFilter::Week),
            SafeSearch::Strict,
        );
        assert!(url.contains("&kp=1"));
        assert!(url.contains("&df=w"));
    }

    #[test]
    fn build_search_url_omits_kp_when_moderate() {
        let url = build_search_url(
            "rust",
            "en",
            "us",
            Endpoint::Html,
            None,
            SafeSearch::Moderate,
        );
        assert!(!url.contains("&kp="));
        assert!(!url.contains("&df="));
    }

    #[test]
    fn build_search_url_lite_endpoint_uses_correct_url() {
        let url = build_search_url(
            "rust",
            "en",
            "us",
            Endpoint::Lite,
            None,
            SafeSearch::Moderate,
        );
        assert!(url.starts_with("https://lite.duckduckgo.com/lite/?"));
    }

    #[test]
    fn build_news_search_url_contains_news_vertical_params() {
        let url = build_news_search_url("rust", "pt", "br", None, SafeSearch::Moderate);
        assert!(url.starts_with("https://duckduckgo.com/?q=rust"));
        assert!(url.contains("&ia=news&iar=news"));
        assert!(url.contains("&kl=br-pt"));
        assert!(!url.contains("&kp="));
        assert!(!url.contains("&df="));
    }

    #[test]
    fn build_news_search_url_adds_optional_params() {
        let url = build_news_search_url(
            "rust",
            "en",
            "us",
            Some(TimeFilter::Day),
            SafeSearch::Strict,
        );
        assert!(url.contains("&ia=news&iar=news"));
        assert!(url.contains("&kp=1"));
        assert!(url.contains("&df=d"));
    }

    #[test]
    fn build_news_search_url_encodes_query() {
        let url = build_news_search_url("noticias brasil C++", "pt", "br", None, SafeSearch::Off);
        assert!(url.contains("noticias%20brasil%20C%2B%2B"));
        assert!(!url.contains("noticias brasil"));
    }

    #[test]
    fn build_news_search_url_respects_endpoint_policy() {
        crate::endpoints::set_endpoint_policy(crate::endpoints::EndpointPolicy {
            html: None,
            lite: None,
            serp: Some("http://127.0.0.1:9/serp/".into()),
        });
        let url = build_news_search_url("rust", "en", "us", None, SafeSearch::Moderate);
        crate::endpoints::set_endpoint_policy(crate::endpoints::EndpointPolicy::default());
        assert!(url.starts_with("http://127.0.0.1:9/serp/?q=rust"));
        assert!(url.contains("&ia=news&iar=news"));
    }

    #[test]
    fn extract_results_and_tokens_share_one_document() {
        // Combined path must match separate token extract (same fixture fields).
        let html = r#"
            <div id="links">
              <div class="result">
                <a class="result__a" href="https://example.com/a">Alpha</a>
                <a class="result__snippet">snippet alpha</a>
              </div>
            </div>
            <form>
              <input name="vqd" value="4-shared-parse">
              <input name="s" value="30">
              <input name="dc" value="31">
            </form>
        "#;
        let cfg = crate::types::SelectorConfig::default();
        let (results, tokens) = extract_results_and_pagination_tokens(html, &cfg);
        assert!(!results.is_empty(), "strategy extract should find results");
        let (vqd, s, dc) = tokens.expect("tokens from same parse");
        assert_eq!((vqd.as_str(), s.as_str(), dc.as_str()), ("4-shared-parse", "30", "31"));
        let (v2, s2, d2) = extract_pagination_tokens(html).expect("standalone tokens");
        assert_eq!(vqd, v2);
        assert_eq!(s, s2);
        assert_eq!(dc, d2);
    }

    #[test]
    fn extract_pagination_tokens_extracts_when_present() {
        let html = r#"
            <form>
              <input name="q" value="rust">
              <input name="vqd" value="4-12345678-abc">
              <input name="s" value="50">
              <input name="dc" value="51">
            </form>
        "#;
        let (vqd, s, dc) = extract_pagination_tokens(html).expect("all present");
        assert_eq!(vqd, "4-12345678-abc");
        assert_eq!(s, "50");
        assert_eq!(dc, "51");
    }

    #[test]
    fn extract_pagination_tokens_returns_none_when_absent() {
        let html = r#"<html><body>Sem inputs</body></html>"#;
        assert!(extract_pagination_tokens(html).is_none());
    }

    #[test]
    fn retry_fail_reason_is_cancellation_is_typed() {
        assert!(RetryFailReason::Cancelled.is_cancellation());
        assert!(!RetryFailReason::Network("connection reset".into()).is_cancellation());
        assert!(!RetryFailReason::Network("cancelled".into()).is_cancellation());
        assert!(!RetryFailReason::Timeout.is_cancellation());
        assert!(!RetryFailReason::Blocked.is_cancellation());
    }

    #[test]
    fn retry_fail_reason_returns_correct_error_code() {
        assert_eq!(
            RetryFailReason::Blocked.as_error_code(),
            crate::error::codes::BLOCKED
        );
        assert_eq!(
            RetryFailReason::Timeout.as_error_code(),
            crate::error::codes::TIMEOUT
        );
    }

    #[test]
    fn retry_fail_reason_is_retryable_classification() {
        assert!(RetryFailReason::RateLimited.is_retryable());
        assert!(RetryFailReason::Timeout.is_retryable());
        assert!(RetryFailReason::Network("connection reset".into()).is_retryable());
        assert!(!RetryFailReason::Cancelled.is_retryable());
        assert!(!RetryFailReason::Blocked.is_retryable());
        assert!(!RetryFailReason::HttpError(404).is_retryable());
        assert!(!RetryFailReason::HttpError(400).is_retryable());
        assert!(RetryFailReason::HttpError(503).is_retryable());
        assert!(RetryFailReason::HttpError(502).is_retryable());
        assert!(RetryFailReason::HttpError(404).is_permanent());
        assert!(!RetryFailReason::RateLimited.is_permanent());
    }

    // v0.7.9 GAP-WS-58: should_try_lite is the pure gate that decides
    // whether the Lite endpoint should be attempted after the HTML
    // endpoint returned no results. The legacy path requires
    // `cfg.allow_lite_fallback` AND a positive marker classification;
    // the new pre-flight path requires `cfg.pre_flight` AND a
    // ghost-block. Both paths converge here so the gate is testable
    // without spinning a `Client` or a `MockServer`.
    #[test]
    fn preflight_ghost_block_triggers_lite_fallback() {
        let mut cfg = test_config_empty();
        cfg.endpoint = Endpoint::Html;
        cfg.allow_lite_fallback = false;
        cfg.pre_flight = false;
        // Baseline: nothing fires.
        assert!(!should_try_lite(&cfg, InterstitialKind::None, false).0);
        assert!(!should_try_lite(&cfg, InterstitialKind::None, true).0);

        // Legacy path: flag ON + marker detected → fires.
        cfg.allow_lite_fallback = true;
        assert!(should_try_lite(&cfg, InterstitialKind::Cloudflare, false).0);
        assert!(should_try_lite(&cfg, InterstitialKind::DuckDuckGo, false).0);
        // No marker → no fallback even with legacy flag.
        assert!(!should_try_lite(&cfg, InterstitialKind::None, false).0);

        // Pre-flight path: flag ON + ghost_block → fires WITHOUT
        // `allow_lite_fallback`.
        cfg.allow_lite_fallback = false;
        cfg.pre_flight = true;
        assert!(should_try_lite(&cfg, InterstitialKind::None, true).0);
        // Pre-flight WITHOUT ghost_block → no fallback.
        assert!(!should_try_lite(&cfg, InterstitialKind::None, false).0);
    }

    // v0.7.10 P3 #9: `pre_flight_fired` flag is true ONLY when the
    // pre-flight path triggered the fallback (NOT when the legacy
    // `--allow-lite-fallback` path did).
    #[test]
    fn pre_flight_flag_in_metadata_only_when_preflight_path_fires() {
        let mut cfg = test_config_empty();
        cfg.endpoint = Endpoint::Html;

        // Legacy path → should_try=true, pre_flight_fired=false.
        cfg.allow_lite_fallback = true;
        cfg.pre_flight = false;
        let (try_lite, pre_flight_fired) =
            should_try_lite(&cfg, InterstitialKind::Cloudflare, false);
        assert!(try_lite);
        assert!(
            !pre_flight_fired,
            "legacy path must NOT set pre_flight_fired"
        );

        // Pre-flight path → should_try=true, pre_flight_fired=true.
        cfg.allow_lite_fallback = false;
        cfg.pre_flight = true;
        let (try_lite, pre_flight_fired) = should_try_lite(&cfg, InterstitialKind::None, true);
        assert!(try_lite);
        assert!(
            pre_flight_fired,
            "pre-flight path MUST set pre_flight_fired"
        );

        // No fallback → both false.
        let (try_lite, pre_flight_fired) = should_try_lite(&cfg, InterstitialKind::None, false);
        assert!(!try_lite);
        assert!(!pre_flight_fired);
    }
