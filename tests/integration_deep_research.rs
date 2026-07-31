// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for the deep-research pipeline (v1.0.2 harness).
//!
//! Offline budget/contract tests + lightweight aggregation fixtures.
//! Network SERP paths are covered by e2e/Chrome harnesses elsewhere.

use chrono::{DateTime, Utc};
use duckduckgo_search_cli::aggregation::{aggregate, AggregationStrategy};
use duckduckgo_search_cli::budget::{
    default_deep_research_budget_ok, estimate_deep_research_seconds, gated_estimate,
    validate_deep_research_budget, BudgetDecision, DeepResearchBudgetInput,
};
use duckduckgo_search_cli::cli::DEFAULT_FETCH_CONTENT_CAP;
use duckduckgo_search_cli::decomposition::{
    is_composite_query, CompositeSignal, HeuristicTemplate,
};
use duckduckgo_search_cli::deep_research::{
    AggregationStrategyKind, DeepResearchArgs, DEFAULT_MAX_SUB_QUERIES, MAX_SUB_QUERIES,
};
use duckduckgo_search_cli::synthesis::{estimate_tokens, trim_to_budget, SynthFormat};
use duckduckgo_search_cli::types::{HttpUrl, SearchMetadata, SearchOutput, SearchResult};

fn make_result(position: u32, url: &str) -> SearchResult {
    SearchResult {
        position,
        title: format!("Mock title {position}"),
        url: HttpUrl::try_new(url).expect("valid test url"),
        display_url: None,
        snippet: Some(format!("Mock snippet {position}")),
        original_title: None,
        content: None,
        content_size: None,
        content_extraction_method: None,
    }
}

fn make_metadata() -> SearchMetadata {
    // DRY: `..Default` absorbs additive wire fields (e.g. flags_ignored V15.1).
    SearchMetadata {
        execution_time_ms: 100,
        selectors_hash: "deadbeef".to_string(),
        user_agent: "test-ua".to_string(),
        ..SearchMetadata::default()
    }
}

fn make_output(query: &str, urls: &[&str]) -> SearchOutput {
    let ts: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-06-07T00:00:00Z")
        .expect("rfc3339")
        .with_timezone(&Utc);
    SearchOutput {
        query: query.to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp: ts,
        region: "us-en".to_string(),
        result_count: urls.len() as u32,
        results: urls
            .iter()
            .enumerate()
            .map(|(i, u)| make_result(i as u32 + 1, u))
            .collect(),
        pages_fetched: 1,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: make_metadata(),
    }
}

#[test]
fn budget_defaults_fit_global_timeout_v1_0_2() {
    assert!(default_deep_research_budget_ok(
        DEFAULT_MAX_SUB_QUERIES,
        true,
        DEFAULT_FETCH_CONTENT_CAP,
        true,
        0,
    ));
    let heavy = DeepResearchBudgetInput::from_cli(5, true, 10, true, 0);
    match validate_deep_research_budget(heavy, 180, false) {
        BudgetDecision::Reject { .. } => {}
        BudgetDecision::Proceed { .. } => panic!("legacy heavy workload must reject"),
    }
    let with_depth = DeepResearchBudgetInput::from_cli(3, true, 4, true, 2);
    let base = DeepResearchBudgetInput::from_cli(3, true, 4, true, 0);
    assert!(estimate_deep_research_seconds(with_depth) > estimate_deep_research_seconds(base));
    assert!(gated_estimate(base) <= 180);
}

#[test]
fn heuristic_template_news_exists() {
    assert_eq!(HeuristicTemplate::News.as_str(), "news");
    assert!(HeuristicTemplate::News.suffix().contains("news"));
    assert_eq!(HeuristicTemplate::all().len(), 6);
}

#[test]
fn max_sub_queries_default_is_three() {
    assert_eq!(DEFAULT_MAX_SUB_QUERIES, 3);
    const _: () = assert!(DEFAULT_MAX_SUB_QUERIES <= MAX_SUB_QUERIES);
    let _ = DeepResearchArgs::default();
    let _ = AggregationStrategyKind::Rrf;
    let _ = is_composite_query("rust vs go", CompositeSignal::Comparison);
    let _ = estimate_tokens("hello world");
    let _ = trim_to_budget("hello world", 10);
    let _ = SynthFormat::Markdown;
}

#[test]
fn aggregate_rrf_merges_mock_outputs() {
    let a = make_output("q", &["https://example.com/a", "https://example.com/b"]);
    let b = make_output("q", &["https://example.com/b", "https://example.com/c"]);
    let merged = aggregate(&[a, b], AggregationStrategy::Rrf(60));
    assert!(merged.len() >= 2);
}
