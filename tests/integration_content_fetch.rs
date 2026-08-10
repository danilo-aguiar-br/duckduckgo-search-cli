// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for `content_fetch::enrich_with_content` via `wiremock`.
//!
//! They cover the residual HTTP HAPPY PATH (feature `http-test-harness` +
//! `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`) that the unit tests do not exercise.
//!
//! GAP-WS-113: in production fetch-content is Chrome-only; these tests only
//! validate the wiremock harness.

#![cfg(feature = "http-test-harness")]

use duckduckgo_search_cli::content_fetch::enrich_with_content;
use duckduckgo_search_cli::types::{
    Config, Endpoint, ParallelismDegree, PerHostLimit, SearchOutput, SearchResult,
};
use reqwest::Client;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod common;

use chrono::{TimeZone, Utc};

/// Web-only Config for the content-fetch harness.
///
/// GAP-TEST-COMPILE-8: derives from [`common::lean_config`] so newtype fields
/// (`ValidatedQuery`, bounded timeouts, `proxy_config`) stay in sync with
/// production types. Only the fields this suite needs differently are
/// overridden here; everything else (timeout 5s, global timeout 60s,
/// `max_content_length` 10_000, `per_host_limit` 2, `proxy_config` disabled,
/// `warmup_enabled = false`, `quiet = true`, `VerticalMode::Web`) comes from
/// the shared builder.
/// Residual HTTP client for the harness.
///
/// GAP-WIREMOCK-RUSTLS-PROVIDER / V17: `rustls-*-no-provider` requires an
/// explicit `CryptoProvider` install before `Client::build`, otherwise reqwest
/// panics with "No provider set".
fn test_client() -> Client {
    common::ensure_tls_for_http_harness();
    Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("cliente de teste")
}

fn cfg(parallelism: u32) -> Config {
    let mut config = common::lean_config(Endpoint::Html, 1, 0);
    let q = common::validated_query("q");
    config.query = q.clone();
    config.queries = vec![q];
    config.fetch_content = true;
    config.parallelism = ParallelismDegree::try_new(parallelism).expect("parallelism");
    config
}

fn output_with_urls(urls: &[&str]) -> SearchOutput {
    let results: Vec<SearchResult> = urls
        .iter()
        .enumerate()
        .map(|(i, u)| SearchResult {
            position: (i + 1) as u32,
            title: format!("Titulo {i}"),
            url: common::http_url(u),
            display_url: None,
            snippet: Some(format!("snippet {i}")),
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        })
        .collect();
    SearchOutput {
        query: "q".into(),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: Utc
            .with_ymd_and_hms(2026, 4, 14, 0, 0, 0)
            .single()
            .expect("timestamp"),
        region: "br-pt".into(),
        result_count: results.len() as u32,
        results,
        pages_fetched: 1,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: common::sample_metadata(),
    }
}

fn article_html(title: &str) -> String {
    // Realistic HTML for readability: <article> + several long paragraphs.
    let paragraphs: Vec<String> = (0..5)
        .map(|i| {
            format!(
                "<p>Este é o parágrafo número {i} do artigo sobre {title}, \
                 com texto suficiente para ultrapassar o threshold de 200 caracteres \
                 e convencer o extrator de que há conteúdo relevante a preservar.</p>"
            )
        })
        .collect();
    format!(
        "<html><head><title>{title}</title></head><body>\
         <nav>menu</nav>\
         <article>{}</article>\
         <footer>rodapé</footer>\
         </body></html>",
        paragraphs.join("")
    )
}

// ---------------------------------------------------------------------------
// T1: happy path — 2 distinct URLs, HTTP returns article HTML → both enriched.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn enriches_two_urls_via_plain_http_and_marks_http_method() {
    // GAP-WS-113 / GAP-SCRAPE-008: residual HTTP + SSRF skip only when harness is active.
    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1");
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/a"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            article_html("Rust").into_bytes(),
            "text/html; charset=utf-8",
        ))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/b"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            article_html("Tokio").into_bytes(),
            "text/html; charset=utf-8",
        ))
        .mount(&mock)
        .await;

    let url_a = format!("{}/a", mock.uri());
    let url_b = format!("{}/b", mock.uri());
    let mut output = output_with_urls(&[&url_a, &url_b]);

    let client = test_client();
    // Force residual HTTP: nonexistent chrome path so launch fails and harness
    // falls back to reqwest (GAP-WS-113 Chrome-first does not apply without Chrome).
    let mut config = cfg(2);
    config.chrome_path = Some("/nonexistent/chrome-for-http-harness".into());
    let cancellation = CancellationToken::new();

    enrich_with_content(&mut output, Some(&client), &config, &cancellation).await;

    assert_eq!(output.metadata.concurrent_fetches, 2);
    assert_eq!(output.metadata.fetch_successes, 2);
    assert_eq!(output.metadata.fetch_failures, 0);
    for r in &output.results {
        let content = r.content.as_ref().expect("content present");
        assert!(content.len() > 100, "non-trivial content");
        assert_eq!(
            r.content_extraction_method.as_deref(),
            Some("http"),
            "method should be http under harness without Chrome"
        );
    }
    assert!(!output.metadata.used_chrome);
}

// ---------------------------------------------------------------------------
// T2: endpoint returns non-HTML content-type → must register failure, not crash.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn enriches_with_non_html_content_type_records_failure() {
    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1");
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/img"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"PNGDATA".to_vec(), "image/png"))
        .mount(&mock)
        .await;

    let url = format!("{}/img", mock.uri());
    let mut output = output_with_urls(&[&url]);

    let client = test_client();
    let mut config = cfg(1);
    config.chrome_path = Some("/nonexistent/chrome-for-http-harness".into());
    let cancellation = CancellationToken::new();

    enrich_with_content(&mut output, Some(&client), &config, &cancellation).await;

    assert_eq!(output.metadata.concurrent_fetches, 1);
    assert_eq!(output.metadata.fetch_successes, 0, "non-HTML = 0 successes");
    assert_eq!(
        output.metadata.fetch_failures, 1,
        "non-HTML counts as failure"
    );
    assert!(output.results[0].content.is_none());
    assert!(output.results[0].content_extraction_method.is_none());
}

// ---------------------------------------------------------------------------
// T3: two results on the SAME host — exercises per-host semaphore without serializing everything.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn enriches_same_host_respecting_per_host_limit() {
    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1");
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/p1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(article_html("A").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/p2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(article_html("B").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/p3"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(article_html("C").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let u1 = format!("{}/p1", mock.uri());
    let u2 = format!("{}/p2", mock.uri());
    let u3 = format!("{}/p3", mock.uri());
    let mut output = output_with_urls(&[&u1, &u2, &u3]);

    let client = test_client();
    let mut config = cfg(3);
    config.per_host_limit = PerHostLimit::try_new(2).expect("per_host_limit");
    config.chrome_path = Some("/nonexistent/chrome-for-http-harness".into());
    let cancellation = CancellationToken::new();

    enrich_with_content(&mut output, Some(&client), &config, &cancellation).await;

    assert_eq!(output.metadata.fetch_successes, 3);
    assert_eq!(output.metadata.fetch_failures, 0);
}
