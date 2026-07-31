// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests for deep_research.

use super::*;

fn empty_output() -> DeepResearchOutput {
        DeepResearchOutput {
            kind: "deep_research".to_string(),
            query: "q".to_string(),
            metadata: DeepResearchMetadata {
                original_query: "q".to_string(),
                sub_queries: vec![SubQueryOutcome {
                    text: "q aspect".to_string(),
                    strategy: "heuristic".to_string(),
                    status: "ok".to_string(),
                    elapsed_ms: 1,
                    error: None,
                    news_count: None,
                    news_unavailable: None,
                    zero_cause: None,
                    news_error: None,
                    news_diagnosis: None,
                }],
                aggregation_strategy: "rrf".to_string(),
                unique_result_count: 0,
                unique_news_count: 0,
                total_elapsed_ms: 1,
                cascade_level: None,
                used_chrome: true,
                chrome_path_resolved: Some("/usr/lib64/chromium-browser/chromium-browser".into()),
                chrome_channel: Some("host".into()),
                sub_queries_total: 1,
                sub_queries_ok: 1,
                sub_queries_error: 0,
                partial: false,
                chrome_contention_advisory: false,
            },
            results: Vec::new(),
            news: Vec::new(),
            news_count: 0,
            synth: None,
        }
    }

    #[test]
    fn envelope_always_serializes_news_fields() {
        let json = serde_json::to_value(empty_output()).expect("serializable");
        assert_eq!(json["news"], serde_json::json!([]));
        assert_eq!(json["news_count"], 0);
        assert_eq!(json["metadata"]["unique_news_count"], 0);
    }

    #[test]
    fn envelope_serializes_chrome_agent_metadata() {
        let json = serde_json::to_value(empty_output()).expect("serializable");
        assert_eq!(json["metadata"]["used_chrome"], true);
        assert_eq!(
            json["metadata"]["chrome_path_resolved"],
            "/usr/lib64/chromium-browser/chromium-browser"
        );
        assert_eq!(json["metadata"]["chrome_channel"], "host");
    }

    #[test]
    fn sub_query_omits_news_fields_when_none() {
        let json = serde_json::to_value(empty_output()).expect("serializable");
        let sq = &json["metadata"]["sub_queries"][0];
        assert!(sq.get("news_count").is_none());
        assert!(sq.get("news_unavailable").is_none());
    }

    #[test]
    fn sub_query_news_mapping_covers_all_cases() {
        assert_eq!(sub_query_news_fields(false, Some(3)), (Some(3), None));
        assert_eq!(sub_query_news_fields(false, Some(0)), (Some(0), None));
        assert_eq!(sub_query_news_fields(false, None), (None, Some(true)));
        assert_eq!(sub_query_news_fields(true, Some(3)), (None, None));
        assert_eq!(sub_query_news_fields(true, None), (None, None));
    }

    #[test]
    fn news_unavailable_diagnosis_is_rich() {
        let mut out = SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: crate::types::utc_now(),
            region: "br-pt".into(),
            result_count: 1,
            results: vec![],
            pages_fetched: 1,
            news: None,
            news_count: None,
            error: None,
            message: Some("news_vertical_unavailable:chrome_unavailable".into()),
            metadata: crate::types::SearchMetadata {
                execution_time_ms: 1,
                selectors_hash: "t".into(),
                next_action_suggestion: Some("Retry news vertical.".into()),
                ..crate::types::SearchMetadata::default()
            },
        };
        let diag = sub_query_news_diagnosis(false, &out);
        assert_eq!(diag.news_unavailable, Some(true));
        assert_eq!(diag.news_error.as_deref(), Some("chrome_unavailable"));
        assert!(diag.news_diagnosis.as_ref().is_some_and(|s| s.contains("Retry")));
        let _ = &mut out;
    }

    // ── GAP-E2E-51-012: depth reflection quality filter ────────────────────

    #[test]
    fn quality_filter_rejects_stopword_glue_rust_your() {
        // Exact regression from e2e: parent "rust" + stopword "your".
        assert!(!is_quality_depth_subquery("rust", "rust your"));
        assert!(!is_quality_depth_subquery("rust", "rust the"));
        assert!(!is_quality_depth_subquery("rust", "your rust"));
    }

    #[test]
    fn quality_filter_rejects_stopword_only_and_short() {
        assert!(!is_quality_depth_subquery("async rust", "the and or"));
        assert!(!is_quality_depth_subquery("async rust", "x"));
        assert!(!is_quality_depth_subquery("async rust", ""));
        assert!(!is_quality_depth_subquery("async rust", "   "));
    }

    #[test]
    fn quality_filter_rejects_near_duplicate_of_parent() {
        assert!(!is_quality_depth_subquery("rust async", "rust async"));
        assert!(!is_quality_depth_subquery("rust async", "Rust  Async"));
        // Parent content tokens only — no new gap term.
        assert!(!is_quality_depth_subquery(
            "rust async programming",
            "async rust programming"
        ));
        // Parent + only stopwords.
        assert!(!is_quality_depth_subquery(
            "rust async",
            "rust async your the"
        ));
    }

    #[test]
    fn quality_filter_accepts_contentful_follow_up() {
        assert!(is_quality_depth_subquery("rust", "rust tokio runtime"));
        assert!(is_quality_depth_subquery(
            "rust async",
            "rust async tokio"
        ));
        assert!(is_quality_depth_subquery(
            "machine learning",
            "machine learning transformers"
        ));
    }

    #[test]
    fn heuristic_depth_skips_junk_title_glue() {
        let items = vec![AggregatedItem {
            url: crate::types::HttpUrl::for_test("https://example.com/a"),
            title: "Your guide to the best of rust".to_string(),
            display_url: None,
            snippet: Some("With more about your journey".to_string()),
            score: 0.5,
            position: 1,
            sources: vec!["rust".to_string()],
        }];
        let seen = std::collections::HashSet::new();
        let out = heuristic_depth_follow_ups("rust", &items, 4, &seen);
        for q in &out {
            assert!(
                is_quality_depth_subquery("rust", q),
                "emitted low-quality follow-up: {q:?}"
            );
            assert!(
                !q.eq_ignore_ascii_case("rust your"),
                "regression: rust your must not fan out"
            );
        }
    }

    #[test]
    fn heuristic_depth_emits_content_term_when_available() {
        let items = vec![AggregatedItem {
            url: crate::types::HttpUrl::for_test("https://example.com/b"),
            title: "Tokio runtime internals for async Rust".to_string(),
            display_url: None,
            snippet: Some("Explore tokio multi-threaded scheduler".to_string()),
            score: 0.9,
            position: 1,
            sources: vec!["rust async".to_string()],
        }];
        let seen = std::collections::HashSet::new();
        let out = heuristic_depth_follow_ups("rust async", &items, 3, &seen);
        assert!(
            !out.is_empty(),
            "expected at least one quality follow-up from tokio/runtime terms"
        );
        assert!(out.iter().all(|q| is_quality_depth_subquery("rust async", q)));
        assert!(out.iter().any(|q| {
            let lower = q.to_ascii_lowercase();
            lower.contains("tokio") || lower.contains("runtime") || lower.contains("scheduler")
        }));
    }
