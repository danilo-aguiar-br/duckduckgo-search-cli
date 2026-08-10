// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests for result aggregation (SRP split from aggregation.rs).

use super::*;
use crate::types::SearchMetadata;

fn out_with(query: &str, urls: &[&str]) -> SearchOutput {
    SearchOutput {
        query: query.to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp: chrono::DateTime::parse_from_rfc3339("2026-06-07T00:00:00Z")
            .expect("fixture")
            .with_timezone(&chrono::Utc),
        region: "br-pt".to_string(),
        result_count: urls.len() as u32,
        results: urls
            .iter()
            .enumerate()
            .map(|(i, u)| crate::types::SearchResult {
                position: (i as u32) + 1,
                title: format!("title-{i}"),
                url: crate::types::HttpUrl::for_test(u),
                display_url: None,
                snippet: Some(format!("snippet-{i}")),
                original_title: None,
                content: None,
                content_size: None,
                content_extraction_method: None,
            })
            .collect(),
        pages_fetched: 1,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: SearchMetadata {
            execution_time_ms: 0,
            selectors_hash: String::new(),
            retries: 0,
            retries_configured: None,
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            chrome_attempted: false,
            user_agent: String::new(),
            used_proxy: false,
            identity_used: None,
            cascade_level: None,
            pre_flight_fired: false,
            pre_flight_executed: false,
            pre_flight_status: None,
            news_promo_filtered: None,
            stream_requested: None,
            stream_effective: None,
            zero_cause: None,
            next_action_suggestion: None,
            bytes_raw: None,
            bytes_decompressed: None,
            cascade_level_observed: None,
            result_count_compat: None,
            endpoint_used_compat: None,
            vertical_used: None,
            chrome_path_resolved: None,
            chrome_channel: None,
            ..Default::default()
        },
    }
}

#[test]
fn canonicalize_strips_utm_and_lowercases_host() {
    let a = "HTTPS://Example.com/path/?utm_source=x&b=2&a=1#frag";
    let b = "https://example.com/path?a=1&b=2";
    assert_eq!(canonicalize_url(a), canonicalize_url(b));
}

#[test]
fn canonicalize_collapses_repeated_slashes() {
    let a = "https://example.com/foo//bar///baz/";
    let b = "https://example.com/foo/bar/baz";
    assert_eq!(canonicalize_url(a), canonicalize_url(b));
}

#[test]
fn canonicalize_preserves_root() {
    assert_eq!(
        canonicalize_url("https://example.com"),
        "https://example.com/"
    );
}

#[test]
fn canonical_hash_is_stable() {
    let a = canonical_hash("https://Example.com/a?utm_source=x&b=1&a=1");
    let b = canonical_hash("https://example.com/a?a=1&b=1");
    assert_eq!(a, b);
}

#[test]
fn rrf_combines_duplicate_urls_with_combined_score() {
    let a = out_with("alpha", &["https://example.com/a", "https://example.com/b"]);
    let b = out_with("beta", &["https://example.com/a", "https://example.com/c"]);
    let merged = aggregate(&[a, b], AggregationStrategy::Rrf(60));
    // Example.com/a appears in both => highest score.
    assert_eq!(merged[0].url, "https://example.com/a");
    assert!(merged[0].score > merged[1].score);
    // Sources trace back to both sub-queries.
    assert_eq!(merged[0].sources.len(), 2);
}

#[test]
fn rrf_is_deterministic_across_calls() {
    let a = out_with("alpha", &["https://example.com/a", "https://example.com/b"]);
    let b = out_with("beta", &["https://example.com/b", "https://example.com/c"]);
    let m1 = aggregate(&[a.clone(), b.clone()], AggregationStrategy::Rrf(60));
    let m2 = aggregate(&[a, b], AggregationStrategy::Rrf(60));
    assert_eq!(m1, m2);
}

/// GAP-OS-001: when RRF scores and positions tie, order must follow URL
/// (never HashMap iteration order).
#[test]
fn rrf_tie_breaks_by_url_for_stable_stdout() {
    // Two distinct URLs both at rank 1 in different lists → identical RRF
    // score and best position 1; total order must be url ascending.
    let a = out_with("alpha", &["https://example.com/z-last"]);
    let b = out_with("beta", &["https://example.com/a-first"]);
    let merged = aggregate(&[a, b], AggregationStrategy::Rrf(60));
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].url, "https://example.com/a-first");
    assert_eq!(merged[1].url, "https://example.com/z-last");
    // Same input order reversed → same output (deterministic).
    let a2 = out_with("alpha", &["https://example.com/z-last"]);
    let b2 = out_with("beta", &["https://example.com/a-first"]);
    let merged2 = aggregate(&[b2, a2], AggregationStrategy::Rrf(60));
    assert_eq!(merged, merged2);
}

#[test]
fn dedupe_tie_breaks_by_url_when_positions_equal() {
    // First-seen keeps position 1 for both different URLs across lists.
    let a = out_with("alpha", &["https://example.com/m"]);
    let b = out_with("beta", &["https://example.com/k"]);
    let merged = aggregate(&[a, b], AggregationStrategy::DedupeByUrl);
    assert_eq!(merged.len(), 2);
    // Both have position 1 → sort by url: k before m.
    assert_eq!(merged[0].url, "https://example.com/k");
    assert_eq!(merged[1].url, "https://example.com/m");
}

#[test]
fn dedupe_keeps_first_occurrence() {
    let a = out_with("alpha", &["https://example.com/a", "https://example.com/b"]);
    let b = out_with("beta", &["https://example.com/a", "https://example.com/c"]);
    let merged = aggregate(&[a, b], AggregationStrategy::DedupeByUrl);
    assert_eq!(merged.len(), 3);
    assert_eq!(merged[0].url, "https://example.com/a");
    assert_eq!(merged[0].position, 1);
}

#[test]
fn canonicalize_handles_invalid_url_gracefully() {
    let out = canonicalize_url("not a url");
    assert_eq!(out, "not a url");
}

fn news_item(
    position: u32,
    url: &str,
    title: &str,
    relative_date: Option<&str>,
) -> crate::types::NewsResult {
    crate::types::NewsResult {
        position,
        title: title.to_string(),
        url: crate::types::HttpUrl::for_test(url),
        source: Some(format!("fonte-{position}")),
        relative_date: relative_date.map(str::to_string),
        thumbnail: None,
        content: None,
        content_size: None,
        content_extraction_method: None,
    }
}

fn news_out(query: &str, items: Vec<crate::types::NewsResult>) -> SearchOutput {
    let mut out = out_with(query, &[]);
    out.news = Some(items);
    out
}

#[test]
fn news_rrf_dedupes_by_canonical_url_and_sums_score() {
    let a = news_out(
        "alpha",
        vec![news_item(
            1,
            "https://example.com/n?utm_source=x",
            "old",
            Some("há 3 dias"),
        )],
    );
    let b = news_out(
        "beta",
        vec![news_item(
            1,
            "https://example.com/n",
            "new",
            Some("há 1 hora"),
        )],
    );
    let merged = aggregate_news(&[a, b], AggregationStrategy::Rrf(60));
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].occurrences, 2);
    let expected = 2.0 / 61.0;
    assert!((merged[0].score - expected).abs() < 1e-12);
    // Fields come from the most recent exemplar.
    assert_eq!(merged[0].title, "new");
    assert_eq!(merged[0].relative_date.as_deref(), Some("há 1 hora"));
}

#[test]
fn news_rrf_tiebreak_prefers_more_recent() {
    let a = news_out(
        "alpha",
        vec![news_item(
            1,
            "https://example.com/a",
            "a",
            Some("3 hours ago"),
        )],
    );
    let b = news_out(
        "beta",
        vec![news_item(
            1,
            "https://example.com/b",
            "b",
            Some("há 2 horas"),
        )],
    );
    let merged = aggregate_news(&[a, b], AggregationStrategy::Rrf(60));
    assert_eq!(merged.len(), 2);
    // Equal RRF scores => the more recent item ("há 2 horas") wins.
    assert_eq!(merged[0].url, "https://example.com/b");
    assert_eq!(merged[1].url, "https://example.com/a");
}

#[test]
fn news_rrf_stable_order_when_dates_do_not_parse() {
    let a = news_out(
        "alpha",
        vec![news_item(1, "https://example.com/a", "a", None)],
    );
    let b = news_out(
        "beta",
        vec![news_item(
            1,
            "https://example.com/b",
            "b",
            Some("sem formato"),
        )],
    );
    let merged = aggregate_news(&[a, b], AggregationStrategy::Rrf(60));
    assert_eq!(merged.len(), 2);
    // Equal scores, no parseable date on either side => first-seen order.
    assert_eq!(merged[0].url, "https://example.com/a");
    assert_eq!(merged[1].url, "https://example.com/b");
}

#[test]
fn news_rrf_reassigns_positions_one_to_n() {
    let a = news_out(
        "alpha",
        vec![
            news_item(1, "https://example.com/a", "a", None),
            news_item(2, "https://example.com/b", "b", None),
            news_item(3, "https://example.com/c", "c", None),
        ],
    );
    let merged = aggregate_news(&[a], AggregationStrategy::Rrf(60));
    let positions: Vec<usize> = merged.iter().map(|i| i.position).collect();
    assert_eq!(positions, vec![1, 2, 3]);
}

#[test]
fn news_rrf_ignores_outputs_without_news() {
    let a = out_with("alpha", &["https://example.com/web"]);
    let b = news_out(
        "beta",
        vec![news_item(1, "https://example.com/n", "n", None)],
    );
    let merged = aggregate_news(&[a, b], AggregationStrategy::Rrf(60));
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].url, "https://example.com/n");
}

#[test]
fn news_dedupe_keeps_first_occurrence() {
    let a = news_out(
        "alpha",
        vec![
            news_item(1, "https://example.com/a", "first", Some("há 3 dias")),
            news_item(2, "https://example.com/b", "b", None),
        ],
    );
    let b = news_out(
        "beta",
        vec![news_item(
            1,
            "https://example.com/a",
            "second",
            Some("há 1 hora"),
        )],
    );
    let merged = aggregate_news(&[a, b], AggregationStrategy::DedupeByUrl);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].title, "first");
    assert_eq!(merged[0].position, 1);
    assert_eq!(merged[1].position, 2);
}

#[test]
fn relative_date_parses_portuguese_forms() {
    assert_eq!(relative_date_to_minutes("há 2 horas"), Some(120));
    assert_eq!(relative_date_to_minutes("há 1 hora"), Some(60));
    assert_eq!(relative_date_to_minutes("há 15 min"), Some(15));
    assert_eq!(relative_date_to_minutes("há 15 minutos"), Some(15));
    assert_eq!(relative_date_to_minutes("há 3 dias"), Some(4320));
    assert_eq!(relative_date_to_minutes("ontem"), Some(1440));
}

#[test]
fn relative_date_parses_english_forms() {
    assert_eq!(relative_date_to_minutes("3 hours ago"), Some(180));
    assert_eq!(relative_date_to_minutes("15 minutes ago"), Some(15));
    assert_eq!(relative_date_to_minutes("1 day ago"), Some(1440));
    assert_eq!(relative_date_to_minutes("yesterday"), Some(1440));
}

#[test]
fn relative_date_parses_short_forms() {
    assert_eq!(relative_date_to_minutes("2h"), Some(120));
    assert_eq!(relative_date_to_minutes("15min"), Some(15));
    assert_eq!(relative_date_to_minutes("3d"), Some(4320));
}

#[test]
fn relative_date_is_case_insensitive() {
    assert_eq!(relative_date_to_minutes("Há 2 Horas"), Some(120));
    assert_eq!(relative_date_to_minutes("3 Hours Ago"), Some(180));
    assert_eq!(relative_date_to_minutes("YESTERDAY"), Some(1440));
    assert_eq!(relative_date_to_minutes("Ontem"), Some(1440));
}

#[test]
fn relative_date_returns_none_when_unparseable() {
    assert_eq!(relative_date_to_minutes(""), None);
    assert_eq!(relative_date_to_minutes("sem formato"), None);
    assert_eq!(relative_date_to_minutes("há muito tempo"), None);
    assert_eq!(relative_date_to_minutes("2 weeks ago"), None);
}

#[test]
fn relative_date_returns_none_on_multiplication_overflow() {
    assert_eq!(relative_date_to_minutes("999999999999999999 h"), None);
    assert_eq!(relative_date_to_minutes("99999999999999999 days ago"), None);
    assert_eq!(relative_date_to_minutes("há 99999999999999999 dias"), None);
}

// ---------------------------------------------------------------
// Property-based tests (proptest)
// ---------------------------------------------------------------
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// `canonicalize_url(canonicalize_url(x)) == canonicalize_url(x)` —
        /// the operation must be idempotent for any well-formed URL.
        #[test]
        fn canonicalize_is_idempotent(
            scheme in "(https|http)",
            host in "[a-z]{3,12}",
            path in "/[a-z0-9/_-]{0,30}",
            qkey in "[a-z]{1,5}",
            qval in "[a-z0-9]{1,5}",
        ) {
            let url = format!("{scheme}://{host}{path}?{qkey}={qval}");
            let once = canonicalize_url(&url);
            let twice = canonicalize_url(&once);
            prop_assert_eq!(once, twice);
        }

        /// The canonical form never contains a fragment (`#`).
        #[test]
        fn canonicalize_strips_fragment(
            host in "[a-z]{3,8}",
            fragment in "[a-zA-Z0-9]{1,12}",
        ) {
            let url = format!("https://{host}/p#{fragment}");
            let canon = canonicalize_url(&url);
            prop_assert!(!canon.contains('#'), "fragment leaked: {}", canon);
        }

        /// `utm_*`, `fbclid`, and `gclid` must always be stripped.
        #[test]
        fn canonicalize_strips_tracking_params(
            host in "[a-z]{3,8}",
            path in "/[a-z0-9]{1,10}",
        ) {
            let url = format!(
                "https://{host}{path}?utm_source=x&fbclid=y&gclid=z&keep=1"
            );
            let canon = canonicalize_url(&url);
            prop_assert!(!canon.contains("utm_"), "utm leaked: {}", canon);
            prop_assert!(!canon.contains("fbclid"), "fbclid leaked: {}", canon);
            prop_assert!(!canon.contains("gclid"), "gclid leaked: {}", canon);
            prop_assert!(canon.contains("keep=1"), "non-tracking lost: {}", canon);
        }

        /// The host is always lowercased in the canonical form.
        #[test]
        fn canonicalize_lowercases_host(
            host_part in "[A-Z]{3,8}",
            path in "/[a-z0-9]{0,8}",
        ) {
            let url = format!("https://{host_part}/{path}");
            let canon = canonicalize_url(&url);
            let lower = host_part.to_ascii_lowercase();
            prop_assert!(canon.contains(&lower), "host not lowered: {}", canon);
        }
    }
}
