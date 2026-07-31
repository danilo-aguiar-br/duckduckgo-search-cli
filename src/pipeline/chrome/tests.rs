// SPDX-License-Identifier: MIT OR Apache-2.0
//! Chrome pipeline unit tests.

use super::news::news_empty_is_retryable;
use crate::types::NewsResult;

fn sample_news() -> NewsResult {
        NewsResult {
            position: 1,
            title: "t".into(),
            url: crate::types::HttpUrl::for_test("https://example.com/a"),
            source: None,
            relative_date: None,
            thumbnail: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        }
    }

    #[test]
    fn non_empty_news_is_not_retryable() {
        assert!(!news_empty_is_retryable(&[sample_news()], "<html></html>"));
    }

    #[test]
    fn interstitial_empty_is_retryable() {
        let html = r#"<html><body class="anomaly-modal__mask">Unfortunately, bots use DuckDuckGo too.</body></html>"#;
        assert!(news_empty_is_retryable(&[], html));
    }

    #[test]
    fn rendered_news_shell_empty_is_legitimate_not_retryable() {
        let html = r#"<div data-testid="news-vertical"><div data-testid="no-results-message">No results</div></div>"#;
        // Long enough to avoid ghost-block threshold when no interstitial markers.
        let html = format!("{html}{}", "x".repeat(5000));
        assert!(!news_empty_is_retryable(&[], &html));
    }

    #[test]
    fn incomplete_body_without_shell_is_retryable() {
        // Long body without news shell and without interstitial markers.
        let html = format!("<html><body>{}</body></html>", "partial".repeat(800));
        assert!(news_empty_is_retryable(&[], &html));
    }

    #[test]
    fn css_only_anomaly_modal_with_no_results_is_not_retryable() {
        // Live DDG news SERP embeds `.anomaly-modal__modal` CSS rules even when
        // the vertical honestly renders no-results-message (GAP-E2E-51-006).
        let html = format!(
            r#"<style>.anomaly-modal__modal {{ border: 1px solid #000; }}</style>
            <div data-testid="news-vertical">
              <section data-testid="no-results-message">Nenhum artigo</section>
            </div>{}"#,
            "x".repeat(5000)
        );
        assert!(!news_empty_is_retryable(&[], &html));
    }
