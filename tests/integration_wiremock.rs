// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests with `wiremock` — ZERO real HTTP calls.
//!
//! Each test starts a `MockServer` on a random port and sets the base URL via
//! an environment variable read by `search::html_base_url`/`lite_base_url`. The
//! variable is set and cleared inside the test itself. Every test that touches env
//! vars runs serialized (an implicit constraint of `std::env::set_var`),
//! and each test uses distinct paths (or the same `/` path) to avoid interference.

use duckduckgo_search_cli::search::{
    execute_with_retry, extract_pagination_tokens, search_with_pagination, RetryFailReason,
};
mod common;

use chrono::{TimeZone, Utc};

use duckduckgo_search_cli::types::{Config, Endpoint};
use reqwest::Client;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// `std::env::set_var` is not thread-safe; each test acquires the lock async-friendly.
fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: LazyLock<TokioMutex<()>> = LazyLock::new(|| TokioMutex::new(()));
    &LOCK
}

fn test_client() -> Client {
    // GAP-WIREMOCK-RUSTLS-PROVIDER / V17: rustls-no-provider needs explicit install.
    common::ensure_tls_for_http_harness();
    Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (teste)")
        .build()
        .expect("cliente de teste")
}

fn base_config(endpoint: Endpoint, pages: u32, retries: u32) -> Config {
    common::lean_config(endpoint, pages, retries)
}

fn html_with_3_results_class() -> String {
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
        <a class="result__snippet">Descrição do primeiro resultado.</a>
        <span class="result__url">exemplo.com/um</span>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/dois">Resultado Dois</a>
        <a class="result__snippet">Descrição do segundo resultado.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/tres">Resultado Três</a>
        <a class="result__snippet">Descrição do terceiro resultado.</a>
      </div>
      <div class="result result--ad">
        <a class="result__a" href="//anuncio.com">Anúncio Patrocinado</a>
      </div>
    </div>
    </body></html>"#
    )
}

fn html_with_vqd_tokens_and_results(vqd: &str, s: &str, dc: &str, titles: &[&str]) -> String {
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

fn html_lite_table() -> String {
    // Padding ensures the body stays above the silent-block threshold (5,000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><body>
    {padding}
    <table>
      <tr><td valign="top">1.&nbsp;</td><td><a class="result-link" href="//exemplo.com/lite1">Lite Um</a></td></tr>
      <tr><td>&nbsp;</td><td class="result-snippet">Descrição detalhada do primeiro resultado lite.</td></tr>
      <tr><td valign="top">2.&nbsp;</td><td><a class="result-link" href="//exemplo.com/lite2">Lite Dois</a></td></tr>
      <tr><td>&nbsp;</td><td class="result-snippet">Descrição detalhada do segundo resultado lite.</td></tr>
    </table>
    </body></html>"#
    )
}

/// V18: BASE_URL_* → EndpointPolicy SSOT; residual HTTP_TEST stays env (feature gate).
/// Prefer `common::HarnessGuard` over raw `std::env::set_var` for mock endpoints.
type EnvGuard = common::HarnessGuard;

// ---------------------------------------------------------------------------
// Test 1: Strategy 1 with full HTML → 3 extracted results, 0 ads.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_strategy_1_success() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_with_3_results_class())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let cfg = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &cfg, "rust", &flag, &token)
        .await
        .expect("search must succeed");

    assert_eq!(aggregated.results.len(), 3, "3 organic results");
    assert_eq!(aggregated.results[0].title, "Resultado Um");
    assert_eq!(aggregated.results[0].url, "https://exemplo.com/um");
    assert_eq!(aggregated.pages_fetched, 1);
    assert!(!aggregated.used_fallback_lite);
    assert_eq!(aggregated.effective_endpoint, Endpoint::Html);
}

// ---------------------------------------------------------------------------
// Test 2: HTML with an anti-bot interstitial + flag ON → the Lite fallback engages.
//
// v0.7.8 (GAP-WS-52): the Lite fallback is no longer unconditional on empty
// HTML. It now fires ONLY when (a) `cfg.allow_lite_fallback == true` AND
// (b) `detect_interstitial` classifies the response as Cloudflare/DDG.
// To preserve the intent of the test (the Lite fallback works), an HTML
// body with the `cf-challenge` marker (Cloudflare) is used and the flag is set.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_fallback_lite_when_html_empty() {
    let _g = env_lock().lock().await;
    let mock_server_html = MockServer::start().await;
    let mock_server_lite = MockServer::start().await;

    // HTML with a Cloudflare marker so the detector classifies it as an interstitial.
    let padding_cf =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let html_interstitial = format!(
        r#"<html><head><title>Just a moment...</title></head><body>{padding_cf}<div class="cf-challenge"><h1>Attention Required! | Cloudflare</h1></div></body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_interstitial)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server_html)
        .await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_lite_table())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server_lite)
        .await;

    let base_html = format!("{}/", mock_server_html.uri());
    let base_lite = format!("{}/", mock_server_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let mut cfg = base_config(Endpoint::Html, 1, 0);
    // v0.7.8 (GAP-WS-52): the flag must be ON for the Lite fallback to fire.
    cfg.allow_lite_fallback = true;
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &cfg, "rust", &flag, &token)
        .await
        .expect("Lite fallback must succeed");

    assert_eq!(aggregated.results.len(), 2, "2 resultados do Lite");
    assert_eq!(aggregated.results[0].title, "Lite Um");
    assert!(
        aggregated.used_fallback_lite,
        "the fallback flag must be true"
    );
    assert_eq!(aggregated.effective_endpoint, Endpoint::Lite);
}

// ---------------------------------------------------------------------------
// Test 3: retry on 429 — first 2 responses are 429, the 3rd is 200.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_retry_on_429() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Limitation: each Mock responds with the same template. To sequence responses,
    // use `up_to_n_times` on prior mocks and leave the final mock as catch-all.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_with_3_results_class())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    // 2 retries → up to 3 attempts total.
    let cfg = base_config(Endpoint::Html, 1, 2);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    // On failure, report how many requests the SERVER actually saw.
    //
    // # Why the server and not `attempts`
    //
    // This test is a known intermittent. It configures two retries, the mock
    // answers 429 twice and then succeeds, so ending in `RateLimited` means the
    // third attempt never reached the success stub. Two mechanics explain that
    // — the retry loop's wall-clock budget expiring during backoff, or a
    // request being lost before it arrives — and telling them apart needs the
    // attempt count from the FAILING path.
    //
    // `RetryFailReason` does not carry one: `attempts` lives on `RetryResult`
    // and is dropped by the `?` in `search::pagination`. Threading it into the
    // error would change a public enum that two pipeline modules match on, for
    // a diagnostic. The mock server already knows the answer, and asking it
    // costs nothing and changes no product code.
    let aggregated = match search_with_pagination(&client, &cfg, "rust", &flag, &token).await {
        Ok(aggregated) => aggregated,
        Err(reason) => {
            let seen = mock_server
                .received_requests()
                .await
                .map_or_else(|| "unavailable".to_string(), |r| r.len().to_string());
            panic!(
                "retry should have eventually succeeded, got {reason:?}.\n\
                 The mock server received {seen} request(s) for a budget of 3 \
                 attempts (2x 429 + 1 success).\n\
                 Fewer than 3 means the retry loop stopped early — its \
                 wall-clock budget expired during backoff.\n\
                 Exactly 3 means every attempt arrived and the third was still \
                 answered 429 — a stub-ordering problem, not a timing one."
            );
        }
    };

    assert_eq!(aggregated.results.len(), 3);
    assert_eq!(
        aggregated.attempts, 3,
        "expected exactly 3 attempts (2x 429 + 1 success)"
    );
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "the global rate-limit flag must have been raised"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Persistent 403 → `blocked` error after exhausting retries.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn blocked_after_retries_exhausted() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    // 1 retry → up to 2 attempts.
    let cfg = base_config(Endpoint::Html, 1, 1);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let result = execute_with_retry(
        &client,
        &format!("{}/", mock_server.uri()),
        cfg.retries.get(),
        &flag,
        &token,
    )
    .await;

    match result {
        Err(RetryFailReason::Blocked) => {}
        other => panic!("expected RetryFailReason::Blocked, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 5: vqd pagination — 3 pages, combining results.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn vqd_pagination() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Page 1 (GET) — HTML with vqd/s/dc tokens.
    let html_pg1 =
        html_with_vqd_tokens_and_results("vqd-pg1", "0", "30", &["Res Um", "Res Dois", "Res Três"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Page 2 (POST with vqd-pg1) — returns HTML with vqd-pg2.
    let html_pg2 =
        html_with_vqd_tokens_and_results("vqd-pg2", "30", "60", &["Res Quatro", "Res Cinco"]);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-pg1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(1)
        .mount(&mock_server)
        .await;

    // Page 3 (POST with vqd-pg2).
    let html_pg3 = html_with_vqd_tokens_and_results("vqd-pg3", "60", "90", &["Res Seis"]);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-pg2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg3)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let cfg = base_config(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &cfg, "rust", &flag, &token)
        .await
        .expect("pagination must work");

    assert_eq!(
        aggregated.results.len(),
        6,
        "must combine results from the 3 pages"
    );
    assert_eq!(aggregated.pages_fetched, 3);
    // Positions must be 1..=6, preserving order per page.
    for (i, r) in aggregated.results.iter().enumerate() {
        assert_eq!(r.position, (i + 1) as u32);
    }
    assert_eq!(aggregated.results[0].title, "Res Um");
    assert_eq!(aggregated.results[3].title, "Res Quatro");
    assert_eq!(aggregated.results[5].title, "Res Seis");
}

// ---------------------------------------------------------------------------
// Test 6: Ad filtering — HTML with mixed ads, only organic results returned.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn ad_filtering() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Mixed HTML: 2 organic + 2 ads (one by class, another by data-nrn).
    // Padding ensures the body stays above the silent-block threshold (5,000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let html = format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result result--ad">
        <a class="result__a" href="//anuncio1.com">Anúncio Um</a>
      </div>
      <div class="result">
        <a class="result__a" href="//organico.com/a">Orgânico A</a>
        <a class="result__snippet">Snippet A</a>
      </div>
      <div class="result" data-nrn="ad">
        <a class="result__a" href="//anuncio2.com">Anúncio Dois</a>
      </div>
      <div class="result">
        <a class="result__a" href="//organico.com/b">Orgânico B</a>
        <a class="result__snippet">Snippet B</a>
      </div>
      <div class="result">
        <a class="result__a" href="//duckduckgo.com/y.js?ad=x">Tracker</a>
      </div>
    </div>
    </body></html>"#
    );

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let cfg = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &cfg, "rust", &flag, &token)
        .await
        .expect("success");

    assert_eq!(
        aggregated.results.len(),
        2,
        "only organic results must survive the filter"
    );
    assert_eq!(aggregated.results[0].title, "Orgânico A");
    assert_eq!(aggregated.results[1].title, "Orgânico B");
    for r in &aggregated.results {
        assert!(!r.url.as_str().contains("anuncio"));
        assert!(!r.url.as_str().contains("y.js"));
    }
}

// ---------------------------------------------------------------------------
// Test 7 (v0.3.0): "Official site" heuristic — DDG renders the literal text
// "Official site" as the title for verified domains. The scraper replaces it
// with `url_exibicao` and preserves the literal in `titulo_original`.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn official_site_heuristic() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // HTML with a result that has the literal title "Official site" + .result__url.
    // Padding ensures the body stays above the silent-block threshold (5,000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let html = format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//saofidelis.rj.gov.br/">Official site</a>
        <span class="result__url">saofidelis.rj.gov.br</span>
        <a class="result__snippet">Prefeitura Municipal de São Fidélis.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/outro">Título Normal</a>
        <span class="result__url">exemplo.com</span>
        <a class="result__snippet">Snippet qualquer.</a>
      </div>
    </div>
    </body></html>"#
    );

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let cfg = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &cfg, "saofidelis", &flag, &token)
        .await
        .expect("success");

    assert_eq!(aggregated.results.len(), 2, "2 organic results expected");

    // Result 1: title replaced by url_exibicao, original preserved.
    let r1 = &aggregated.results[0];
    assert_eq!(
        r1.title, "saofidelis.rj.gov.br",
        "title must be the display URL"
    );
    assert_eq!(
        r1.original_title.as_deref(),
        Some("Official site"),
        "original_title must preserve the literal"
    );

    // Result 2: normal title → no substitution, original_title = None.
    let r2 = &aggregated.results[1];
    assert_eq!(r2.title, "Título Normal");
    assert!(
        r2.original_title.is_none(),
        "original_title must be None when there is no substitution"
    );
}

// ---------------------------------------------------------------------------
// Test 8 (v0.3.0): JSON schema NO LONGER contains `buscas_relacionadas`.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_schema_v03_without_related_searches() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    let html = html_with_vqd_tokens_and_results("v", "0", "0", &["T1", "T2"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let cfg = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &cfg, "teste", &flag, &token)
        .await
        .expect("success");

    // Serialize as JSON and confirm the field does NOT appear.
    use duckduckgo_search_cli::types::{SearchMetadata, SearchOutput};
    let output = SearchOutput {
        query: "teste".into(),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        region: "br-pt".into(),
        result_count: aggregated.results.len() as u32,
        results: aggregated.results,
        pages_fetched: 1,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: SearchMetadata {
            selectors_hash: "abc123".into(),
            user_agent: "ua".into(),
            ..SearchMetadata::default()
        },
    };
    let line = serde_json::to_string(&output).expect("serializar NDJSON");
    // A single line (no intermediate \n — \n inside the title is escaped as \n).
    assert!(
        !line.contains('\n'),
        "NDJSON must be a SINGLE line — literal \n inside content MUST be escaped"
    );
    assert!(
        !line.contains("buscas_relacionadas"),
        "v0.3.0: the JSON schema must NOT expose buscas_relacionadas"
    );
    assert!(
        !line.contains("related_searches"),
        "v0.3.0: the JSON schema must NOT expose related_searches"
    );
}

// ===================================================================
// Tests for --fetch-content pure HTTP (iteration 5).
// ===================================================================

/// Real HTML page with enough content to pass the 200-char threshold.
#[allow(dead_code)]
fn html_real_article() -> String {
    r#"<!DOCTYPE html><html><head><title>Artigo de Teste</title></head>
    <body>
      <nav><a href="/">Home</a> <a href="/about">About</a></nav>
      <article>
        <h1>Título Principal do Artigo</h1>
        <p>Este é o primeiro parágrafo do artigo com conteúdo substantivo suficiente para passar o limiar de line mínima. Contém várias frases para simular um texto real de notícia ou documentação técnica.</p>
        <p>Segundo parágrafo relevante com mais informações sobre o tema tratado no artigo. Incluímos conteúdo em português brasileiro com acentuação correta para validar a decodificação UTF-8 adequadamente.</p>
        <p>Terceiro parágrafo conclui o artigo com uma síntese dos pontos principais abordados ao longo do texto. Este conteúdo deve ser preservado integralmente pela extração readability.</p>
      </article>
      <footer>Copyright 2026 Rodapé do site</footer>
    </body></html>"#.to_string()
}

#[tokio::test]
#[cfg(feature = "http-test-harness")]
async fn fetch_content_http_extracts_real_article_via_wiremock() {
    use duckduckgo_search_cli::content::extract_http_content;

    // v1.0.3 GAP-TEST-RACE: the lock MUST be taken BEFORE mutating the global
    // environment. Setting the var first let it be wiped by another test's
    // `EnvGuard` drop while this test was still blocked on the lock, so the
    // body then ran without the harness active and failed nondeterministically.
    let _guard = env_lock().lock().await;
    // GAP-SCRAPE-008: SSRF skip only via harness (feature + HTTP_TEST), not SKIP_SSRF env.
    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1");
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/artigo"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(html_real_article().into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&server)
        .await;

    let client = test_client();
    let token = CancellationToken::new();
    let url = format!("{}/artigo", server.uri());

    let result = extract_http_content(&client, &url, 2000, &token)
        .await
        .expect("fetch must succeed");
    let (text, orig_size) = result.expect("content present");
    assert!(
        text.contains("primeiro parágrafo"),
        "must contain the first paragraph: {text:?}"
    );
    assert!(text.contains("Segundo parágrafo"));
    assert!(text.contains("Terceiro parágrafo"));
    // Nav and footer must have been removed.
    assert!(!text.contains("About"));
    assert!(!text.contains("Copyright"));
    // Tamanho original reportado > 0.
    assert!(orig_size > 0);
}

#[tokio::test]
#[cfg(feature = "http-test-harness")]
async fn fetch_content_http_rejects_non_html_content_type() {
    use duckduckgo_search_cli::content::extract_http_content;

    // v1.0.3 GAP-TEST-RACE: lock BEFORE mutating the global environment.
    let _guard = env_lock().lock().await;
    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1");
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(vec![0u8; 100], "application/pdf"))
        .mount(&server)
        .await;

    let client = test_client();
    let token = CancellationToken::new();
    let url = format!("{}/pdf", server.uri());

    let result = extract_http_content(&client, &url, 1000, &token)
        .await
        .expect("request OK");
    assert!(result.is_none(), "a non-HTML Content-Type must return None");
}

#[tokio::test]
#[cfg(feature = "http-test-harness")]
async fn fetch_content_http_decodes_latin1_correctly() {
    use duckduckgo_search_cli::content::extract_http_content;

    // v1.0.3 GAP-TEST-RACE: lock BEFORE mutating the global environment.
    let _guard = env_lock().lock().await;
    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1");
    let server = MockServer::start().await;

    // HTML in Latin-1 (ISO-8859-1) containing 'c-cedilla' (0xE7) + 'a-acute' (0xE1).
    let mut html: Vec<u8> = b"<html><body><article>".to_vec();
    // Paragraph long enough to pass the threshold (20+ chars).
    html.extend_from_slice(
        b"<p>Uma a\xe7\xe3o interessante foi realizada pelos desenvolvedores do sistema de busca que n\xe3o pode ser ignorada.</p>"
    );
    html.extend_from_slice(
        b"<p>Outro par\xe1grafo relevante com mais texto para chegar ao limiar m\xednimo de conte\xfado necess\xe1rio.</p>"
    );
    html.extend_from_slice(b"</article></body></html>");

    Mock::given(method("GET"))
        .and(path("/latin1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(html, "text/html; charset=iso-8859-1"),
        )
        .mount(&server)
        .await;

    let client = test_client();
    let token = CancellationToken::new();
    let url = format!("{}/latin1", server.uri());

    let result = extract_http_content(&client, &url, 2000, &token)
        .await
        .expect("fetch must succeed");
    let (text, _) = result.expect("content present");
    // 'acao' with accent must be present (correctly decoded from Latin-1).
    assert!(
        text.contains("ação") || text.contains("parágrafo"),
        "text must have correct UTF-8 accents: {text:?}"
    );
}

// Sanity check of the extract_pagination_tokens helper (extra integration-level
// coverage to ensure the helper is exposed and works).
#[test]
fn sanity_extract_pagination_tokens_via_public_lib() {
    let html = html_with_vqd_tokens_and_results("v1", "0", "10", &["a", "b"]);
    let (vqd, s, dc) = extract_pagination_tokens(&html).expect("tokens present");
    assert_eq!(vqd, "v1");
    assert_eq!(s, "0");
    assert_eq!(dc, "10");
}

#[test]
fn ndjson_serializes_search_output_in_valid_single_line() {
    use duckduckgo_search_cli::types::{SearchMetadata, SearchOutput, SearchResult};
    let output = SearchOutput {
        query: "rust".into(),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        region: "br-pt".into(),
        result_count: 1,
        results: vec![SearchResult {
            position: 1,
            title: "Exemplo com\nnova linha".to_string(),
            url: common::http_url("https://exemplo.com"),
            display_url: None,
            snippet: None,
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        }],
        pages_fetched: 1,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: SearchMetadata {
            execution_time_ms: 100,
            selectors_hash: "abc123".into(),
            user_agent: "ua".into(),
            ..SearchMetadata::default()
        },
    };
    let line = serde_json::to_string(&output).expect("serializar NDJSON");
    // A single line (no intermediate \n — \n inside the title is escaped as \\n).
    assert!(
        !line.contains('\n'),
        "NDJSON must be a SINGLE line — literal \n inside content MUST be escaped"
    );
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("NDJSON must be valid JSON");
    assert_eq!(parsed["query"], "rust");
    assert_eq!(parsed["result_count"], 1);
}

// ---------------------------------------------------------------------------
// Test v0.4.0 #1: default --num 15 + auto-pagination to 2 pages.
//
// Simulates the configuration that `montar_configuracoes` would produce when the user
// does NOT pass `--num` (default=15) and `--pages` is at default (1): it raises
// `paginas` to 2 and fixes `num_resultados = Some(15)`. The test verifies that
// `search_with_pagination` respects this: GETs page 1, POSTs page 2
// (with vqd), aggregates 11 + 10 = 21 results and truncates at 15.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn default_num_15_auto_paginates_to_2_pages() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Page 1 — 11 results (typical count for the first DDG page).
    let titles_pg1: Vec<String> = (1..=11).map(|i| format!("Res Pg1 {i}")).collect();
    let refs_pg1: Vec<&str> = titles_pg1.iter().map(String::as_str).collect();
    let html_pg1 = html_with_vqd_tokens_and_results("vqd-auto-pg1", "0", "30", &refs_pg1);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Page 2 — 10 additional results. The search will come via POST with vqd-auto-pg1.
    let titles_pg2: Vec<String> = (1..=10).map(|i| format!("Res Pg2 {i}")).collect();
    let refs_pg2: Vec<&str> = titles_pg2.iter().map(String::as_str).collect();
    let html_pg2 = html_with_vqd_tokens_and_results("vqd-auto-pg2", "30", "60", &refs_pg2);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-auto-pg1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(1)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    // Simulate the POST-montar_configuracoes configuration with default --num 15
    // and auto-pagination to 2 pages (new behavior in v0.4.0).
    let mut cfg = base_config(Endpoint::Html, 2, 0);
    cfg.num_results = Some(common::result_count(15));
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &cfg, "rust", &flag, &token)
        .await
        .expect("auto-pagination must work");

    assert_eq!(
        aggregated.results.len(),
        15,
        "must truncate at --num=15 after aggregating 21 results from 2 pages"
    );
    assert_eq!(
        aggregated.pages_fetched, 2,
        "must have fetched exactly 2 pages"
    );
    assert_eq!(aggregated.results[0].title, "Res Pg1 1");
    // Page 2 starts after the 11 from page 1 → positions 12..=15 come from page 2.
    assert_eq!(aggregated.results[11].title, "Res Pg2 1");
    assert_eq!(aggregated.results[14].title, "Res Pg2 4");
}

// ---------------------------------------------------------------------------
// Test v0.4.0 #2: auto-pagination RESPECTS explicit --pages from the user.
//
// When the user passes --pages 3 explicitly, the auto-pagination logic
// does NOT override it. This test simulates cfg with paginas=3 + num=15 and verifies
// that search_with_pagination runs 3 pages (and truncates at 15).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_auto_pagination_respects_explicit_pages() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Page 1 — 11 results.
    let titles_pg1: Vec<String> = (1..=11).map(|i| format!("Pg1-{i}")).collect();
    let refs_pg1: Vec<&str> = titles_pg1.iter().map(String::as_str).collect();
    let html_pg1 = html_with_vqd_tokens_and_results("v-expl-1", "0", "30", &refs_pg1);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Page 2 — 5 results.
    let titles_pg2: Vec<String> = (1..=5).map(|i| format!("Pg2-{i}")).collect();
    let refs_pg2: Vec<&str> = titles_pg2.iter().map(String::as_str).collect();
    let html_pg2 = html_with_vqd_tokens_and_results("v-expl-2", "30", "60", &refs_pg2);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=v-expl-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(1)
        .mount(&mock_server)
        .await;

    // Page 3 — 3 results.
    let titles_pg3: Vec<String> = (1..=3).map(|i| format!("Pg3-{i}")).collect();
    let refs_pg3: Vec<&str> = titles_pg3.iter().map(String::as_str).collect();
    let html_pg3 = html_with_vqd_tokens_and_results("v-expl-3", "60", "90", &refs_pg3);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=v-expl-2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg3)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    // Simulate --num 15 --pages 3 (explicit) → montar_configuracoes does NOT override.
    let mut cfg = base_config(Endpoint::Html, 3, 0);
    cfg.num_results = Some(common::result_count(15));
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &cfg, "rust", &flag, &token)
        .await
        .expect("must fetch 3 pages per the explicit --pages");

    assert_eq!(
        aggregated.pages_fetched, 3,
        "must honour the explicit --pages=3"
    );
    // 11 + 5 + 3 = 19 aggregated, truncated to 15.
    assert_eq!(
        aggregated.results.len(),
        15,
        "19 aggregated results must be truncated at num=15"
    );
    assert_eq!(aggregated.results[0].title, "Pg1-1");
    assert_eq!(aggregated.results[11].title, "Pg2-1");
}

// ---------------------------------------------------------------------------
// Test 15: HTTP 202 on the first attempt → recovers on the second with 200 OK.
// Verifies that `flag_rate_limit` was activated and result comes with success.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn status_202_recovers_on_second_attempt() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // First attempt returns 202 (DDG anti-bot anomaly).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(202).set_body_string(""))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&mock_server)
        .await;

    // Second attempt returns 200 with valid HTML.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_with_3_results_class())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    // 1 retry → up to 2 attempts total.
    let cfg = base_config(Endpoint::Html, 1, 1);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &cfg, "rust", &flag, &token)
        .await
        .expect("must recover after a 202 on the first attempt");

    assert_eq!(
        aggregated.results.len(),
        3,
        "must have extracted 3 results after recovering from the 202"
    );
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "flag_rate_limit must have been set by the HTTP 202"
    );
}

// ---------------------------------------------------------------------------
// Test 16: HTTP 202 on ALL attempts → `RetryFailReason::Blocked`.
// Verifies that `flag_rate_limit` was activated and the CLI terminates with Blocked.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_202_exhausts_retries_returns_blocked() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // All attempts return 202 (DDG never lets through).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(202).set_body_string(""))
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    // 1 retry → up to 2 attempts (exhausted at 202).
    let cfg = base_config(Endpoint::Html, 1, 1);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let result = execute_with_retry(
        &client,
        &format!("{}/", mock_server.uri()),
        cfg.retries.get(),
        &flag,
        &token,
    )
    .await;

    match result {
        Err(RetryFailReason::Blocked) => {}
        other => panic!(
            "expected RetryFailReason::Blocked after exhausting retries on 202, got {other:?}"
        ),
    }
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "flag_rate_limit must have been set by the HTTP 202"
    );
}

// ---------------------------------------------------------------------------
// WS-23 — Retry-After header test
//
// The first response advertises a 2-second `Retry-After`. The retry pipeline
// must parse and respect the delay. We bound the test with a generous
// upper-bound on elapsed time to avoid flakiness on slow CI.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_retry_after_header_respected() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // 1st response: 429 with Retry-After: 2 (seconds).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "2"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&mock_server)
        .await;

    // 2nd response: 200 with body.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_with_3_results_class())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let client = test_client();
    let cfg = base_config(Endpoint::Html, 1, 2);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let started = std::time::Instant::now();
    let aggregated = search_with_pagination(&client, &cfg, "rust", &flag, &token)
        .await
        .expect("retry must eventually succeed with Retry-After 2s");
    let elapsed_ms = started.elapsed().as_millis() as u64;

    assert_eq!(aggregated.results.len(), 3);
    assert_eq!(
        aggregated.attempts, 2,
        "must have made 2 attempts (1 failed 429 + 1 success)"
    );
    // Retry-After: 2 seconds = 2000ms minimum delay. Allow 1500ms slack for
    // jitter and CI scheduler overhead.
    assert!(
        elapsed_ms >= 1500,
        "elapsed {elapsed_ms}ms is too short — Retry-After: 2 may have been ignored"
    );
}
