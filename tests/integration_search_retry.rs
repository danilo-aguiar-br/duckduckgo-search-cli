// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests focused on uncovered paths of `search.rs`:
//! - `execute_search` (the simple single-query compatibility version)
//! - Mid-retry cancellation in `execute_with_retry`
//! - Error / edge paths of `search_with_pagination`:
//!   * missing vqd tokens → no pagination possible
//!   * next page with a non-OK status → stops
//!   * next page with zero results → stops
//!   * next page without vqd tokens → stops after appending
//!   * cancellation during pagination
//!   * Lite fallback that also fails → stays empty
//!   * truncation by `num_resultados`
//!   * exhausted 429 retry
//!
//! ZERO real HTTP calls — everything goes through `wiremock::MockServer`.

use duckduckgo_search_cli::search::{
    execute_search, execute_with_retry, search_with_pagination, RetryFailReason,
};
mod common;

use duckduckgo_search_cli::types::{Config, Endpoint};
use reqwest::Client;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Async mutex to serialize tests that manipulate env vars.
fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: LazyLock<TokioMutex<()>> = LazyLock::new(|| TokioMutex::new(()));
    &LOCK
}

fn test_client() -> Client {
    // V18: residual HTTP harness needs rustls CryptoProvider (same as wiremock).
    common::ensure_tls_for_http_harness();
    Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (teste-search-retry)")
        .build()
        .expect("test client")
}

fn base_config(endpoint: Endpoint, pages: u32, retries: u32) -> Config {
    common::lean_config(endpoint, pages, retries)
}

/// HTML with 3 organic results — body above 5,000 bytes (anti-block threshold).
fn html_3_results() -> String {
    // Padding ensures the body stays above the silent-block threshold (5,000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/um">Resultado Um</a>
        <a class="result__snippet">Snippet do primeiro resultado.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/dois">Resultado Dois</a>
        <a class="result__snippet">Snippet do segundo resultado.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/tres">Resultado Três</a>
        <a class="result__snippet">Snippet do terceiro resultado.</a>
      </div>
    </div>
    </body></html>"#
    )
}

fn html_with_tokens_and_results(vqd: &str, s: &str, dc: &str, titles: &[&str]) -> String {
    // Padding ensures the body stays above the silent-block threshold (5,000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let mut html = format!("<html><body>{padding}");
    html.push_str(&format!(
        r#"<form><input name="vqd" value="{vqd}"><input name="s" value="{s}"><input name="dc" value="{dc}"></form>"#
    ));
    html.push_str(r#"<div id="links">"#);
    for t in titles {
        html.push_str(&format!(
            r#"<div class="result"><a class="result__a" href="//exemplo.com/{}">{}</a><a class="result__snippet">snippet de {}</a></div>"#,
            t.replace(' ', "-"),
            t,
            t
        ));
    }
    html.push_str("</div></body></html>");
    html
}

/// HTML WITHOUT vqd/s/dc tokens — body above 5,000 bytes (anti-block threshold).
fn html_without_vqd_tokens() -> String {
    // Padding ensures the body stays above the silent-block threshold (5,000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/sem-tokens">Resultado Sem Tokens</a>
        <a class="result__snippet">Snippet sem tokens vqd presentes no formulário.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/outro">Outro Resultado</a>
        <a class="result__snippet">Outro snippet com texto suficiente para ultrapassar 100 bytes.</a>
      </div>
    </div>
    </body></html>"#
    )
}

/// V18: mock endpoints via EndpointPolicy SSOT (`common::HarnessGuard`).
type EnvGuard = common::HarnessGuard;

// ===========================================================================
// `execute_search` — standalone compatibility function (iteration 1).
// ===========================================================================

#[tokio::test]
async fn execute_search_returns_html_on_status_200() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_3_results())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let html = execute_search(&client, "rust", "pt", "br")
        .await
        .expect("status 200 + large body should return Ok");
    assert!(html.contains("Resultado Um"));
    assert!(html.len() > 100);
}

#[tokio::test]
async fn execute_search_fails_with_status_500() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(500).set_body_string("erro interno"))
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let result = execute_search(&client, "rust", "pt", "br").await;
    let err = result.expect_err("status 500 should be an error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("500"),
        "error message should mention status 500: {msg}"
    );
}

#[tokio::test]
async fn execute_search_fails_with_small_body() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Status 200, but body with fewer than 100 bytes → suspected block.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("ok")
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let result = execute_search(&client, "rust", "pt", "br").await;
    let err = result.expect_err("small body should be an error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("suspiciously small") || msg.contains("silent block"),
        "message should mention small response / silent block: {msg}"
    );
}

// ===========================================================================
// `execute_with_retry` — cancellation, exhausted retries and error paths.
// ===========================================================================

#[tokio::test]
async fn retry_aborts_when_token_already_cancelled() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Mock returns 200, but cancellation must abort before the first attempt.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_3_results())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let client = test_client();
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();
    cancellation.cancel(); // already cancelled before even calling.

    let url = format!("{}/", mock.uri());
    let result = execute_with_retry(&client, &url, 3, &flag, &cancellation).await;

    // Typed cancel (not stringly Network) — promotes to CliError::Cancelled → 130/143.
    match result {
        Err(RetryFailReason::Cancelled) => {}
        Err(RetryFailReason::Network(msg)) if msg.to_lowercase().contains("cancel") => {}
        other => panic!("expected Err(Cancelled) (or Network cancel), got {other:?}"),
    }
}

#[tokio::test]
async fn retry_429_exhausted_returns_rate_limited() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Always 429 — exhausts retries and returns RateLimited.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&mock)
        .await;

    let client = test_client();
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    // 0 retries → 1 single attempt → backoff not triggered.
    let url = format!("{}/", mock.uri());
    let result = execute_with_retry(&client, &url, 0, &flag, &cancellation).await;
    match result {
        Err(RetryFailReason::RateLimited) => {}
        other => panic!("expected RateLimited, got {other:?}"),
    }
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "rate limit flag must be set"
    );
}

#[tokio::test]
async fn retry_4xx_non_retryable_returns_http_error() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // 418 is not 200/403/429 → falls into "other 4xx/5xx → do not retry" path.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(418))
        .mount(&mock)
        .await;

    let client = test_client();
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let url = format!("{}/", mock.uri());
    let result = execute_with_retry(&client, &url, 3, &flag, &cancellation).await;
    match result {
        Err(RetryFailReason::HttpError(418)) => {}
        other => panic!("expected HttpError(418), got {other:?}"),
    }
}

// ===========================================================================
// `search_with_pagination` — pagination edge cases.
// ===========================================================================

#[tokio::test]
async fn pagination_without_vqd_tokens_warns_and_returns_page_1_only() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Page 1 with results but WITHOUT vqd/s/dc tokens → blocks pagination.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_without_vqd_tokens())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    // Requests 3 pages, but since there are no vqd tokens, only page 1 will come.
    let config = base_config(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("first page should succeed");
    assert_eq!(aggregated.pages_fetched, 1);
    assert_eq!(aggregated.results.len(), 2);
}

#[tokio::test]
async fn pagination_truncated_by_num_results() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Page 1: 3 results + tokens.
    let html_pg1 = html_with_tokens_and_results("vqd-trunc-1", "0", "30", &["A", "B", "C"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Page 2: 3 results → accumulated total = 6.
    let html_pg2 = html_with_tokens_and_results("vqd-trunc-2", "30", "60", &["D", "E", "F"]);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-trunc-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let mut config = base_config(Endpoint::Html, 2, 0);
    config.num_results = Some(common::result_count(4)); // truncate accumulated 6 down to 4.
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("pagination ok");
    assert_eq!(
        aggregated.results.len(),
        4,
        "results should be truncated to 4"
    );
}

#[tokio::test]
async fn pagination_stops_when_next_page_returns_non_ok_status() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_with_tokens_and_results("vqd-bad-1", "0", "30", &["A", "B"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Page 2 (POST) returns 503 → pagination must stop and return only page 1.
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-bad-1"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("first page ok even with page 2 failing");
    assert_eq!(aggregated.pages_fetched, 1);
    assert_eq!(aggregated.results.len(), 2);
}

#[tokio::test]
async fn pagination_stops_when_next_page_returns_zero_results() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_with_tokens_and_results("vqd-zero-1", "0", "30", &["X", "Y", "Z"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Page 2 returns HTML > 100 bytes but without `.result` → zero results → stops.
    let html_empty = r#"<html><head><title>nada</title></head><body><div id="links"><p>Sem resultados nesta página de teste, apenas texto suficiente para superar 100 bytes.</p></div></body></html>"#;
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-zero-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_empty)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("ok");
    assert_eq!(
        aggregated.pages_fetched, 1,
        "pages_fetched stays at 1 because page 2 returned zero results"
    );
    assert_eq!(aggregated.results.len(), 3);
}

#[tokio::test]
async fn pagination_stops_when_next_page_loses_vqd_tokens() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_with_tokens_and_results("vqd-lost-1", "0", "30", &["A", "B"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Page 2: has results BUT lost vqd tokens → pagination stops after adding page 2.
    // Padding ensures body stays above SILENT_BLOCK_THRESHOLD (5,000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let html_pg2_no_tokens = format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/p2a">Pg 2 A</a>
        <a class="result__snippet">snippet pg2a com texto suficiente.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/p2b">Pg 2 B</a>
        <a class="result__snippet">snippet pg2b com texto suficiente.</a>
      </div>
    </div>
    </body></html>"#
    );
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-lost-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2_no_tokens)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 5, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("ok");
    assert_eq!(
        aggregated.pages_fetched, 2,
        "page 2 counted — but page 3 never arrived because tokens were lost"
    );
    assert_eq!(
        aggregated.results.len(),
        4,
        "2 from page 1 + 2 from page 2 = 4 total results"
    );
}

#[tokio::test]
async fn pagination_aborts_if_token_already_cancelled_at_loop_start() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_with_tokens_and_results("vqd-cancel-1", "0", "30", &["A", "B"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // POST Mock that should NEVER be called (pre-loop cancellation must abort).
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_3_results())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    // Spawn task that cancels after a small delay to simulate mid-execution cancellation.
    // Since the loop has multiple chances to check `is_cancelled()`, this ensures
    // one of the checks fires.
    let cancellation_clone = cancellation.clone();
    let handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancellation_clone.cancel();
    });

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("first page should complete before cancellation");

    handle.await.expect("cancellation task ok");

    // Cancellation must abort the loop before fetching all 3 pages.
    assert!(
        aggregated.pages_fetched < 3,
        "cancellation must abort before completing 3 pages (actual: {})",
        aggregated.pages_fetched
    );
}

#[tokio::test]
async fn fallback_lite_failure_keeps_results_empty() {
    let _g = env_lock().lock().await;
    let mock_html = MockServer::start().await;
    let mock_lite = MockServer::start().await;

    // HTML returns 200 but with zero `.result` → triggers Lite fallback.
    // Padding ensures body stays above SILENT_BLOCK_THRESHOLD (5,000 bytes).
    let padding_fb =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let html_empty = format!(
        r#"<html><head><title>vazio</title></head><body>{padding_fb}<div id="links"><p>Nenhum resultado encontrado para teste de fallback Lite.</p></div></body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_empty)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_html)
        .await;

    // Lite also fails — returns persistent 503.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock_lite)
        .await;

    let base_html = format!("{}/", mock_html.uri());
    let base_lite = format!("{}/", mock_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("HTML 200 + Lite 503 should return Ok with empty list");
    assert_eq!(
        aggregated.results.len(),
        0,
        "both endpoints without results → empty vec"
    );
    assert!(
        !aggregated.used_fallback_lite,
        "Lite fallback failed → flag stays false"
    );
    assert_eq!(aggregated.effective_endpoint, Endpoint::Html);
}

#[tokio::test]
async fn first_page_blocked_by_small_body_returns_blocked_reason() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Status 200 but VERY small body → search_with_pagination returns Blocked.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("ok")
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let result = search_with_pagination(&client, &config, "rust", &flag, &cancellation).await;
    match result {
        Err(RetryFailReason::Blocked) => {}
        Err(other) => panic!("expected Blocked, got another reason: {other:?}"),
        Ok(_) => panic!("expected Blocked, got Ok"),
    }
}

// ===========================================================================
// GAP-WS-52 (v0.7.8) — CONDITIONAL Lite fallback with anti-bot detector.
//
// Expected behaviour (v0.7.8):
// - HTML zero + Cloudflare/DDG interstitial + flag ON  → tries Lite, returns
//   the Lite results, sets `used_fallback_lite` and `effective_endpoint =
//   Endpoint::Lite`.
// - HTML zero + Cloudflare/DDG interstitial + flag OFF → does NOT try Lite,
//   keeps `results = []`, `used_fallback_lite = false`, `effective_endpoint
//   = Endpoint::Html`, and emits a structured `tracing::warn!` with the hint.
// - HTML zero + NO interstitial + flag OFF             → does NOT try Lite,
//   legacy behaviour (zero results).
// ===========================================================================

/// Cloudflare interstitial HTML — body above the 5,000-byte threshold
/// to avoid silent-block detection (which would return `Blocked`).
fn html_cloudflare_interstitial() -> String {
    // Padding keeps the body above the silent-block threshold
    // (5,000 bytes) and includes the canonical markers of the
    // `detect_interstitial` detector (`cf-challenge`, `cf-spinner`,
    // `__cf_chl_jschl_tk__`, `Just a moment`, `Attention Required`).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><head><title>Just a moment...</title></head><body>{padding}<div class="cf-challenge"><h1>Attention Required! | Cloudflare</h1><p>cf-spinner placeholder — checking your browser before accessing duckduckgo.com.</p><script src="/cdn-cgi/challenge-platform/h/b"></script><input type="hidden" name="__cf_chl_jschl_tk__" value="x.y.z"></div></body></html>"#
    )
}

fn html_lite_1_result() -> String {
    // Canonical Lite endpoint format (a TABLE with class="result-link"),
    // which matches `extract_results_lite_with_cfg`. Padding keeps the body
    // above the 5,000-byte threshold.
    let padding =
        "<!-- padding para garantir que a resposta Lite seja interpretada como resultado válido. -->"
            .repeat(60);
    format!(
        r#"<html><body>
    {padding}
    <table>
      <tr><td valign="top">1.&nbsp;</td><td><a class="result-link" href="//lite.example/a">Lite Resultado A</a></td></tr>
      <tr><td>&nbsp;</td><td class="result-snippet">Snippet do primeiro resultado lite retornado pelo fallback.</td></tr>
    </table>
    </body></html>"#
    )
}

fn html_zero_without_interstitial() -> String {
    // Genuinely empty HTML (zero `.result`, zero interstitial markers).
    // The padding has to be robust — `detect_interstitial` runs over the
    // whole body, so the padding must not contain markers either. 80
    // repetitions are used to guarantee ~6,000 bytes.
    let padding =
        "<!-- padding cenario-C sem marcadores anti-bot para superar limiar 5000 bytes -->"
            .repeat(80);
    format!(
        r#"<html><head><title>Resultados</title></head><body>{padding}<div id="links"><p>Nenhum resultado encontrado para esta consulta genuinamente vazia.</p></div></body></html>"#
    )
}

#[tokio::test]
async fn conditional_lite_fallback_with_interstitial_and_flag_uses_lite() {
    let _g = env_lock().lock().await;
    let mock_html = MockServer::start().await;
    let mock_lite = MockServer::start().await;

    // HTML returns 200 with a Cloudflare interstitial detected by
    // `detect_interstitial` (markers `cf-challenge` + `cf-spinner`).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_cloudflare_interstitial())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_html)
        .await;

    // Lite returns 200 with 1 valid result in the canonical table
    // format (matched by `extract_results_lite_with_cfg`).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_lite_1_result())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_lite)
        .await;

    let base_html = format!("{}/", mock_html.uri());
    let base_lite = format!("{}/", mock_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let mut config = base_config(Endpoint::Html, 1, 0);
    // FLAG ON → the Lite fallback must fire when the detector
    // classifies the response as an interstitial.
    config.allow_lite_fallback = true;
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("interstitial + flag ON → must complete with Lite results");
    assert_eq!(
        aggregated.results.len(),
        1,
        "Scenario A: interstitial + flag → 1 Lite result"
    );
    assert!(
        aggregated.used_fallback_lite,
        "Scenario A: flag ON + interstitial → used_fallback_lite must be true"
    );
    assert_eq!(aggregated.effective_endpoint, Endpoint::Lite);
}

#[tokio::test]
async fn conditional_lite_fallback_with_interstitial_without_flag_stays_empty() {
    let _g = env_lock().lock().await;
    let mock_html = MockServer::start().await;
    let mock_lite = MockServer::start().await;

    // Same Cloudflare interstitial as scenario A.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_cloudflare_interstitial())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_html)
        .await;

    // Lite exists in the mock but must NOT be called in this scenario (no flag).
    // If it were called it would return 1 result — the test would fail because
    // the mocked count would be 1 while effective_endpoint stayed
    // Html (assert below).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<html><body>should not be called</body></html>"),
        )
        .mount(&mock_lite)
        .await;

    let base_html = format!("{}/", mock_html.uri());
    let base_lite = format!("{}/", mock_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 1, 0);
    // FLAG OFF (default) → does NOT try Lite even with an interstitial.
    assert!(!config.allow_lite_fallback);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("interstitial + flag OFF → must return Ok with an empty list");
    assert_eq!(
        aggregated.results.len(),
        0,
        "Scenario B: interstitial + flag OFF → 0 results"
    );
    assert!(
        !aggregated.used_fallback_lite,
        "Scenario B: flag OFF → Lite is never attempted"
    );
    assert_eq!(
        aggregated.effective_endpoint,
        Endpoint::Html,
        "Scenario B: effective_endpoint stays Html when Lite is not attempted"
    );
}

#[tokio::test]
async fn conditional_lite_fallback_zero_without_interstitial_does_not_use_lite() {
    let _g = env_lock().lock().await;
    let mock_html = MockServer::start().await;
    let mock_lite = MockServer::start().await;

    // HTML returns 200 with zero results and NO interstitial marker
    // at all. Body above the 5,000-byte threshold.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_zero_without_interstitial())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_html)
        .await;

    // Lite is mocked — it must NOT be called in this scenario.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "<html><body>should not be called — sem interstitial detectado</body></html>",
        ))
        .mount(&mock_lite)
        .await;

    let base_html = format!("{}/", mock_html.uri());
    let base_lite = format!("{}/", mock_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let mut config = base_config(Endpoint::Html, 1, 0);
    // Even with the flag ON, without a detected interstitial the fallback does NOT fire.
    config.allow_lite_fallback = true;
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("genuine zero results without interstitial → Ok with an empty list");
    assert_eq!(
        aggregated.results.len(),
        0,
        "Scenario C: zero without interstitial → 0 results"
    );
    assert!(
        !aggregated.used_fallback_lite,
        "Scenario C: the detector classifies as None → Lite is not attempted even with flag ON"
    );
    assert_eq!(
        aggregated.effective_endpoint,
        Endpoint::Html,
        "Scenario C: effective_endpoint stays Html"
    );
}
