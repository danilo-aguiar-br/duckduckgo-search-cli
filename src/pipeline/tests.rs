// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests for this module.

use super::*;
use crate::types::{SearchMetadata, SelectorConfig, ZeroCause};
use std::collections::BTreeMap;
use std::time::Instant;

#[test]
fn calculate_selectors_hash_returns_16_chars() {
    let cfg = SelectorConfig::default();
    let hash = calculate_selectors_hash(&cfg);
    assert_eq!(hash.len(), 16);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn calculate_selectors_hash_is_deterministic() {
    let cfg = SelectorConfig::default();
    let h1 = calculate_selectors_hash(&cfg);
    let h2 = calculate_selectors_hash(&cfg);
    assert_eq!(h1, h2);
}

#[test]
fn combinar_deduplica_preservando_ordem_da_primeira_ocorrencia() {
    let posicionais = vec!["alfa".to_string(), "beta".to_string()];
    let from_file = vec!["beta".to_string(), "gama".to_string()];
    let de_stdin = vec!["alfa".to_string(), "delta".to_string()];
    let combinado = combine_and_dedup_queries(posicionais, from_file, de_stdin);
    assert_eq!(
        combinado,
        vec!["alfa", "beta", "gama", "delta"],
        "order must follow the first occurrence; duplicates must be removed"
    );
}

#[test]
fn combinar_remove_strings_vazias_e_apenas_espacos() {
    let posicionais = vec!["   ".to_string(), "rust".to_string(), "".to_string()];
    let from_file = vec!["\t\t".to_string(), "tokio".to_string()];
    let de_stdin = vec![];
    let combinado = combine_and_dedup_queries(posicionais, from_file, de_stdin);
    assert_eq!(combinado, vec!["rust", "tokio"]);
}

#[test]
fn combine_trims_whitespace_before_comparing() {
    let posicionais = vec!["  alfa  ".to_string()];
    let from_file = vec!["alfa".to_string()];
    let de_stdin = vec!["alfa\t".to_string()];
    let combinado = combine_and_dedup_queries(posicionais, from_file, de_stdin);
    assert_eq!(
        combinado,
        vec!["alfa"],
        "queries that are equivalent after trimming must be deduplicated"
    );
}

#[test]
fn combine_empty_returns_empty() {
    let combinado = combine_and_dedup_queries(vec![], vec![], vec![]);
    assert!(combinado.is_empty());
}

#[test]
fn read_queries_from_file_accepts_windows_lines_and_empty() {
    use std::io::Write;
    let dir = std::env::temp_dir().join("ddg_cli_iter2_queries_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("queries.txt");
    let content = "rust\r\ntokio\r\n\r\n  axum  \n\nhttp://exemplo.com\n";
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    drop(file);

    let lines = read_queries_from_file(&path).expect("should read file");
    assert_eq!(lines, vec!["rust", "tokio", "axum", "http://exemplo.com"]);
    // Cleanup best-effort.
    let _ = std::fs::remove_file(&path);
}

#[test]
fn total_results_in_single_output() {
    let output = SearchOutput {
        query: "q".into(),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: crate::types::test_timestamp(),
        region: "br-pt".into(),
        result_count: 7,
        results: vec![],
        pages_fetched: 1,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: SearchMetadata {
            execution_time_ms: 0,
            selectors_hash: "x".into(),
            retries: 0,
            retries_configured: None,
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            chrome_attempted: false,
            user_agent: "ua".into(),
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
    };
    assert_eq!(PipelineResult::Single(Box::new(output)).total_results(), 7);
}

// GAP-WS-104 v0.8.9: total_results soma news_count — news-only com
// news encontradas ⇒ exit 0; without news ⇒ exit 5.
#[test]
fn total_results_sums_news_count() {
    let output = SearchOutput {
        query: "q".into(),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: crate::types::test_timestamp(),
        region: "br-pt".into(),
        result_count: 0,
        results: vec![],
        pages_fetched: 0,
        news: Some(vec![]),
        news_count: Some(4),
        error: None,
        message: None,
        metadata: SearchMetadata {
            execution_time_ms: 0,
            selectors_hash: "x".into(),
            retries: 0,
            retries_configured: None,
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: true,
            chrome_attempted: true,
            user_agent: "ua".into(),
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
            vertical_used: Some("news".into()),
            chrome_path_resolved: None,
            chrome_channel: None,
            ..Default::default()
        },
    };
    assert_eq!(PipelineResult::Single(Box::new(output)).total_results(), 4);
}

// =====================================================================
// Fim dos testes do classificador GAP-AUD-003.
// =====================================================================

// =====================================================================
// GAP F1/F2/F5 v0.8.9 — news-only failure envelope, pre-flight gate, and the
// Chrome transport cancellation error.
// =====================================================================

fn cfg_for_vertical(pre_flight: bool, vertical: crate::types::VerticalMode) -> Config {
    let mut cfg = Config::default();
    cfg.query = crate::security::ValidatedQuery::try_new("assunto").expect("q");
    cfg.queries = vec![cfg.query.clone()];
    cfg.pre_flight = pre_flight;
    cfg.vertical = vertical;
    cfg.fetch_content = false;
    cfg
}

#[test]
fn pre_flight_aplica_somente_quando_vertical_inclui_web() {
    assert!(pre_flight_applies(&cfg_for_vertical(
        true,
        crate::types::VerticalMode::Web
    )));
    assert!(pre_flight_applies(&cfg_for_vertical(
        true,
        crate::types::VerticalMode::All
    )));
    assert!(
        !pre_flight_applies(&cfg_for_vertical(true, crate::types::VerticalMode::News)),
        "news-only must skip the pre-flight (Chrome-only, no HTTP endpoint)"
    );
    assert!(!pre_flight_applies(&cfg_for_vertical(
        false,
        crate::types::VerticalMode::Web
    )));
}

#[cfg(feature = "chrome")]
#[test]
fn news_only_chrome_failure_output_emits_structured_envelope() {
    let mut cfg = Config::default();
    cfg.query = crate::security::ValidatedQuery::try_new("assunto").expect("q");
    cfg.queries = vec![cfg.query.clone()];
    cfg.fetch_content = false;
    cfg.vertical = crate::types::VerticalMode::News;
    let err = CliError::InvalidConfig {
        message: "Chrome not detected: binary missing".to_string(),
    };
    let out = news_only_chrome_failure_output(&cfg, &err, Instant::now());
    assert_eq!(out.result_count, 0);
    assert!(out.results.is_empty());
    assert_eq!(out.news_count, Some(0));
    assert_eq!(out.news.as_ref().map(Vec::len), Some(0));
    assert_eq!(
        out.error.as_deref(),
        Some(crate::error::codes::INVALID_CONFIG)
    );
    assert!(out
        .message
        .as_deref()
        .unwrap_or_default()
        .contains("Chrome"));
    assert_eq!(out.metadata.zero_cause, Some(ZeroCause::InvalidResponse));
    assert!(out.metadata.chrome_attempted);
    assert_eq!(out.metadata.vertical_used.as_deref(), Some("news"));
    assert!(out.metadata.next_action_suggestion.is_some());
}

#[cfg(feature = "chrome")]
#[test]
fn chrome_cancelled_error_unifica_cancelled_exit_130() {
    let err = chrome_cancelled_error("news search");
    assert!(matches!(err, CliError::Cancelled));
    assert_eq!(err.error_code(), crate::error::codes::CANCELLED);
    assert_eq!(err.exit_code(), crate::error::exit_codes::CANCELLED);
    assert!(err.to_string().contains("cancelled"));
}

#[test]
fn total_results_in_multi_output_sums_all() {
    let new_output = |n: u32| SearchOutput {
        query: "q".into(),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: crate::types::test_timestamp(),
        region: "br-pt".into(),
        result_count: n,
        results: vec![],
        pages_fetched: 1,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: SearchMetadata {
            execution_time_ms: 0,
            selectors_hash: "x".into(),
            retries: 0,
            retries_configured: None,
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            chrome_attempted: false,
            user_agent: "ua".into(),
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
    };
    let multi = MultiSearchOutput {
        query_count: 3,
        timestamp: crate::types::test_timestamp(),
        parallelism: 3,
        searches: vec![new_output(2), new_output(5), new_output(0)],
        zero_cause_histogram: BTreeMap::new(),
    };
    assert_eq!(PipelineResult::Multi(Box::new(multi)).total_results(), 7);
}
