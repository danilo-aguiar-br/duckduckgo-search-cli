// SPDX-License-Identifier: MIT OR Apache-2.0
//! Deep-research + news vertical integration (GAP-WS-105 / v1.0.2 harness).
//!
//! # Coverage (no real network)
//!
//! - Binary `deep-research --no-news` against wiremock under feature
//!   `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`: exit 0 and
//!   additive news fields always present (`news: []`, counts 0); sub_queries
//!   omit news keys when `--no-news`.
//! - Additive contract: v0.8.8 envelope (no news fields) still deserializes.
//! - Dual synthesis with `--no-news` for markdown / plain-text / json.
//! - Multi-query + `--vertical all` + missing Chrome path → exit 2 (fail-closed).
//!
//! # Run (wiremock binary tests need the harness feature)
//!
//! ```text
//! cargo test --test integration_deep_research_news --features chrome,http-test-harness
//! ```
//!
//! Thin flags (`--no-fetch-content --global-timeout 30 -q`) keep wall-clock
//! under 15s per binary test (GAP-TEST-NEWS-HARNESS / rules_rust_testes_sem_travar).
//! Without `http-test-harness`, wiremock binary tests are compiled out so default
//! `cargo test` never hangs on real Chrome I/O.
//!
//! Env vars are passed via `Command::env` (subprocess only) — no process-wide
//! mutation. MockServer tests use multi-thread runtime because the subprocess
//! blocks a worker while the mock answers.

use duckduckgo_search_cli::deep_research::DeepResearchOutput;
use std::process::{Command, Stdio};

#[cfg(feature = "http-test-harness")]
use std::process::Output;
#[cfg(feature = "http-test-harness")]
use wiremock::matchers::method;
#[cfg(feature = "http-test-harness")]
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_duckduckgo-search-cli")
}

/// SERP HTML with 3 organic results (same shape as `integration_wiremock`).
/// Padding exceeds the silent-block size threshold (~5000 bytes).
#[cfg(feature = "http-test-harness")]
fn html_with_3_results() -> String {
    // Must exceed SILENT_BLOCK_THRESHOLD (5000 bytes) in search.rs.
    let padding =
        "<!-- padding to exceed DuckDuckGo silent-block detection threshold. -->".repeat(80);
    format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//example.com/one">Result One</a>
        <a class="result__snippet">First result description.</a>
        <span class="result__url">example.com/one</span>
      </div>
      <div class="result">
        <a class="result__a" href="//example.com/two">Result Two</a>
        <a class="result__snippet">Second result description.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//example.com/three">Result Three</a>
        <a class="result__snippet">Third result description.</a>
      </div>
    </div>
    </body></html>"#
    )
}

#[cfg(feature = "http-test-harness")]
async fn mock_serp_with_results() -> MockServer {
    let server = MockServer::start().await;
    // Match ANY path: residual HTML SERP uses /html/ and query strings, not only `/`.
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_with_3_results())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&server)
        .await;
    server
}

/// Run binary `deep-research` against mock base URL under HTTP test harness.
///
/// Always thin: `--no-fetch-content`, short global timeout, quiet — avoids
/// content-fetch hang and real Chrome when harness is active.
#[cfg(feature = "http-test-harness")]
fn run_deep_research_bin(base: String, extra_args: Vec<String>) -> Output {
    let mut cmd = Command::new(bin_path());
    // Global flags before subcommand; thin harness workload (no content fetch).
    // Endpoint overrides are CLI/XDG policy only (GAP-SCRAPE-R2-009) — not product env.
    cmd.args([
        "-q",
        "--global-timeout",
        "30",
        "--base-url-html",
        base.as_str(),
        "--base-url-lite",
        base.as_str(),
        "--base-url-serp",
        base.as_str(),
        "deep-research",
        "--no-fetch-content",
    ]);
    cmd.args(&extra_args);
    // GAP-PROC: explicit Stdio on all streams (rules-rust-processos-externos).
    // HTTP_TEST enables residual wiremock transport (feature http-test-harness only).
    cmd.env("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.output().expect("binary must execute")
}

// ---------------------------------------------------------------------------
// 1. Additive envelope with --no-news (binary + wiremock) — harness only
// ---------------------------------------------------------------------------

#[cfg(feature = "http-test-harness")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn binary_no_news_emits_envelope_with_additive_news_fields() {
    let server = mock_serp_with_results().await;
    let base = format!("{}/", server.uri());

    let output = tokio::task::spawn_blocking(move || {
        run_deep_research_bin(
            base,
            vec![
                "rust async runtime".to_string(),
                "--no-news".to_string(),
                "--max-sub-queries".to_string(),
                "2".to_string(),
            ],
        )
    })
    .await
    .expect("subprocess join");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "--no-news with web results must exit 0; stderr: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid JSON");

    assert_eq!(json["kind"], "deep_research");
    let results = json["results"].as_array().expect("results must be array");
    assert!(
        !results.is_empty(),
        "fan-out against mock must aggregate web results"
    );

    let news = json["news"]
        .as_array()
        .expect("news must be array even with --no-news");
    assert!(news.is_empty(), "--no-news implies empty news");
    assert_eq!(
        json["news_count"].as_u64(),
        Some(0),
        "news_count must be 0 with --no-news"
    );
    assert_eq!(
        json["metadata"]["unique_news_count"].as_u64(),
        Some(0),
        "metadados.unique_news_count must be 0 with --no-news"
    );

    let sub_queries = json["metadata"]["sub_queries"]
        .as_array()
        .expect("sub_queries must be array");
    assert!(!sub_queries.is_empty(), "fan-out must emit sub-queries");
    for sq in sub_queries {
        let obj = sq.as_object().expect("sub_query must be object");
        assert!(
            !obj.contains_key("news_count"),
            "--no-news must omit news_count on sub_query: {sq}"
        );
        assert!(
            !obj.contains_key("news_unavailable"),
            "--no-news must omit news_unavailable on sub_query: {sq}"
        );
    }

    let parsed: DeepResearchOutput =
        serde_json::from_str(stdout.trim()).expect("round-trip DeepResearchOutput");
    assert!(parsed.news.is_empty());
    assert_eq!(parsed.news_count, 0);
    assert_eq!(parsed.metadata.unique_news_count, 0);
}

// ---------------------------------------------------------------------------
// 2. Additive contract: v0.8.8 envelope still deserializes (no harness)
// ---------------------------------------------------------------------------

#[test]
fn envelope_v088_without_news_fields_deserializes_with_defaults() {
    let antigo = r#"{
        "tipo": "deep_research",
        "query": "rust",
        "metadata": {
            "query_original": "rust",
            "sub_queries": [
                {
                    "texto": "rust overview",
                    "estrategia": "heuristic",
                    "status": "ok",
                    "tempo_ms": 10
                }
            ],
            "estrategia_agregacao": "rrf",
            "unique_result_count": 0,
            "tempo_total_ms": 12,
            "nivel_cascata": null
        },
        "results": []
    }"#;

    let parsed: DeepResearchOutput =
        serde_json::from_str(antigo).expect("v0.8.8 envelope must deserialize");
    assert!(parsed.news.is_empty(), "missing news becomes empty vec");
    assert_eq!(parsed.news_count, 0, "missing news_count becomes 0");
    assert_eq!(
        parsed.metadata.unique_news_count, 0,
        "missing unique_news_count becomes 0"
    );
    let sq = &parsed.metadata.sub_queries[0];
    assert!(sq.news_count.is_none());
    assert!(sq.news_unavailable.is_none());
}

// ---------------------------------------------------------------------------
// 3. Synthesis with --no-news in all three formats — harness only
// ---------------------------------------------------------------------------

#[cfg(feature = "http-test-harness")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn binary_no_news_synthesize_emits_synthesis_in_all_three_formats() {
    let server = mock_serp_with_results().await;

    for (flag, formato_esperado) in [
        ("markdown", "markdown"),
        ("plain-text", "plain_text"),
        ("json", "json"),
    ] {
        let base = format!("{}/", server.uri());
        let output = tokio::task::spawn_blocking(move || {
            run_deep_research_bin(
                base,
                vec![
                    "rust async runtime".to_string(),
                    "--no-news".to_string(),
                    "--max-sub-queries".to_string(),
                    "2".to_string(),
                    "--synthesize".to_string(),
                    "--synth-format".to_string(),
                    flag.to_string(),
                ],
            )
        })
        .await
        .expect("subprocess join");

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            output.status.code(),
            Some(0),
            "--synthesize --synth-format {flag} must exit 0; code={:?} stderr: {stderr} stdout: {stdout}",
            output.status.code()
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(stdout.trim()).expect("stdout must be valid JSON");

        let synthesis = json
            .get("synthesis")
            .unwrap_or_else(|| panic!("synthesis must be present for format {flag}"));
        assert_eq!(
            synthesis["format"].as_str(),
            Some(formato_esperado),
            "synthesis.format wrong for --synth-format {flag}"
        );
        let body = synthesis["body"]
            .as_str()
            .expect("synthesis.body must be string");
        assert!(
            !body.is_empty(),
            "synthesis.body must not be empty for format {flag}"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Fail-closed without Chrome (no harness / no network)
// ---------------------------------------------------------------------------

// GAP-WS-113: multi-query + --vertical all + missing Chrome binary => exit 2.
// Product env NO_CHROME was removed; force fail-closed via nonexistent --chrome-path.
#[test]
fn binary_multi_query_vertical_all_no_chrome_fail_closed() {
    let output = Command::new(bin_path())
        .args([
            "--vertical",
            "all",
            "-q",
            "-f",
            "json",
            "--chrome-path",
            "/nonexistent/chromium-for-gap-ws-113-test",
            "--global-timeout",
            "15",
            "rust",
            "tokio",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("binary must execute");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stderr.contains("aceita apenas UMA query"),
        "multi-query guard must stay removed (GAP-WS-105); stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "GAP-WS-113: missing Chrome path must fail exit 2; stdout={stdout} stderr={stderr}"
    );
}
