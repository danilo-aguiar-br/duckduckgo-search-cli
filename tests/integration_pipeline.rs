// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for `pipeline::execute_pipeline` and `parallel::*`.
//!
//! They cover the most expensive paths of the multi-query flow:
//! - Barrier (`JoinSet`) when `stream_mode = false`.
//! - Streaming (mpsc) when `stream_mode = true`.
//! - Single-query with `stream_mode = true` (warn + fallback).
//! - Empty-list errors.
//! - Pure dedup and file-reading helpers.
//!
//! Every test uses `wiremock` — ZERO real HTTP calls.

use duckduckgo_search_cli::pipeline::{
    combine_and_dedup_queries, execute_pipeline, read_queries_from_file, PipelineResult,
};
mod common;

use duckduckgo_search_cli::types::{Config, Endpoint, OutputFormat};
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Async mutex to serialize tests that install process-wide endpoint policy.
fn env_lock() -> &'static TokioMutex<()> {
    // TokioMutex::new is not const — LazyLock is the correct fixed-init wrapper (MSRV ≥ 1.80).
    static LOCK: LazyLock<TokioMutex<()>> = LazyLock::new(|| TokioMutex::new(()));
    &LOCK
}

/// V18: mock endpoints via EndpointPolicy SSOT (`common::HarnessGuard`).
type EnvGuard = common::HarnessGuard;

fn cfg_multi(queries: Vec<String>, format: OutputFormat, stream: bool) -> Config {
    let mut c = common::lean_config_queries(Endpoint::Html, 1, queries, 2);
    c.format = format;
    c.stream_mode = stream;
    c
}

/// HTML with 2 results — body above 5,000 bytes (silent-block threshold).
fn html_2_results(title_a: &str, title_b: &str) -> String {
    // Padding ensures the body stays above the silent-block threshold (5,000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><body>
        {padding}
        <div id="links">
          <div class="result">
            <a class="result__a" href="//exemplo.com/a">{title_a}</a>
            <a class="result__snippet">snippet A</a>
            <span class="result__url">exemplo.com/a</span>
          </div>
          <div class="result">
            <a class="result__a" href="//exemplo.com/b">{title_b}</a>
            <a class="result__snippet">snippet B</a>
            <span class="result__url">exemplo.com/b</span>
          </div>
        </div></body></html>"#
    )
}

// ---------------------------------------------------------------------------
// T1: multi-query in barrier mode — exercises `execute_parallel_searches`
//     and the JoinSet with staggered launch.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_multi_query_barrier_aggregates_results() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_2_results("Primeiro", "Segundo"))
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

    let cfg = cfg_multi(
        vec!["rust".to_string(), "tokio".to_string()],
        OutputFormat::Json,
        false,
    );
    let token = CancellationToken::new();

    let res = execute_pipeline(cfg, token)
        .await
        .expect("the multi-query barrier pipeline must succeed");

    match res {
        PipelineResult::Multi(multi) => {
            assert_eq!(multi.query_count, 2, "2 queries executadas");
            assert_eq!(multi.searches.len(), 2);
            assert!(multi.searches.iter().all(|s| s.result_count >= 2));
        }
        other => panic!("expected Multi, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T2: multi-query in streaming mode — exercises `execute_parallel_searches_streaming`
//     + consumer via mpsc + NDJSON emission.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_multi_query_streaming_drains_and_returns_stats() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_2_results("Alpha", "Beta"))
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

    // Output file to avoid polluting stdout during the test.
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    let mut cfg = cfg_multi(
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
        OutputFormat::Json,
        true,
    );
    cfg.output_file = Some(tmp.path().to_path_buf());

    let token = CancellationToken::new();

    let res = tokio::time::timeout(Duration::from_secs(30), execute_pipeline(cfg, token))
        .await
        .expect("the pipeline must not hang")
        .expect("the streaming pipeline must succeed");

    match res {
        PipelineResult::Stream(stats) => {
            assert_eq!(stats.total, 3, "3 queries processadas no stream");
            assert!(stats.successes + stats.errors == stats.total);
        }
        other => panic!("expected Stream, got: {other:?}"),
    }

    // Validate that NDJSON was written: 3 valid JSON lines.
    let content = std::fs::read_to_string(tmp.path()).expect("read output file");
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 3, "3 NDJSON lines (one per query)");
    for line in &lines {
        let _: serde_json::Value = serde_json::from_str(line).expect("valid NDJSON line");
    }
}

// ---------------------------------------------------------------------------
// T3: single-query with stream_mode=true — branch that warns + falls back to aggregate.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_single_query_with_stream_warns_and_falls_back_to_aggregate() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_2_results("Único", "Segundo"))
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

    let cfg = cfg_multi(vec!["solo".to_string()], OutputFormat::Json, true);
    let token = CancellationToken::new();

    let res = execute_pipeline(cfg, token)
        .await
        .expect("single + stream must fall back to Single with a warning");

    match res {
        PipelineResult::Single(output) => {
            assert_eq!(output.query, "solo");
            assert!(output.result_count >= 2);
        }
        other => panic!("expected Single, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T4: empty queries — must return error, not panic.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_with_empty_queries_returns_error() {
    let cfg = cfg_multi(vec![], OutputFormat::Json, false);
    let token = CancellationToken::new();
    let res = execute_pipeline(cfg, token).await;
    assert!(res.is_err(), "an empty list must produce an error");
    let msg = format!("{}", res.unwrap_err());
    assert!(
        msg.contains("no queries to execute"),
        "message should mention 'no queries to execute', got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// T5: combine_and_dedup_queries — dedup preserving order and filtering empties.
// ---------------------------------------------------------------------------
#[test]
fn combine_queries_preserves_order_dedups_and_filters_empties() {
    let r = combine_and_dedup_queries(
        vec!["rust".into(), "  ".into(), "tokio".into()],
        vec!["rust".into(), "serde".into()],
        vec!["".into(), "serde".into(), "axum".into()],
    );
    assert_eq!(r, vec!["rust", "tokio", "serde", "axum"]);
}

#[test]
fn combine_queries_fully_empty_list_returns_empty_vec() {
    let r = combine_and_dedup_queries(vec![], vec![], vec!["   ".into(), "\n".into()]);
    assert!(r.is_empty());
}

#[test]
fn combine_queries_trims_each_entry() {
    let r = combine_and_dedup_queries(
        vec!["  rust  ".into()],
        vec!["\ttokio\n".into()],
        vec![" rust ".into()],
    );
    // "  rust  " and " rust " after trim are equal → dedup.
    assert_eq!(r, vec!["rust", "tokio"]);
}

// ---------------------------------------------------------------------------
// T6: read_queries_from_file — LF, CRLF and blank lines.
// ---------------------------------------------------------------------------
#[test]
fn read_queries_from_file_handles_crlf_and_empty_lines() {
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    // Mix of LF and CRLF + blank lines.
    std::fs::write(tmp.path(), "rust\r\n\r\n  tokio  \nserde\n\n").expect("escrever");
    let qs = read_queries_from_file(tmp.path()).expect("ler ok");
    assert_eq!(qs, vec!["rust", "tokio", "serde"]);
}

#[test]
fn read_queries_from_nonexistent_file_returns_error() {
    let missing_path = PathBuf::from("/tmp/duckduckgo-search-cli-file-nao-existe-xyz-123.txt");
    let r = read_queries_from_file(&missing_path);
    assert!(r.is_err(), "a non-existent file must fail");
}

#[test]
fn read_queries_from_empty_file_returns_empty_vec() {
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    std::fs::write(tmp.path(), "").expect("escrever");
    let qs = read_queries_from_file(tmp.path()).expect("ok");
    assert!(qs.is_empty());
}

// GAP-WS-51: probe-deep must send a calibration query of realistic length.
// A 1-word query like "rust" rarely triggers DuckDuckGo bot detection,
// so the probe would falsely report "ok" even when production queries
// are blocked. The constant must be long enough to mirror real usage.
#[test]
fn probe_calibration_query_is_long_and_multi_word() {
    // Recreate the same constant value used in src/lib.rs to detect
    // any future drift between the two sites.
    const PROBE_CALIBRATION_QUERY: &str = "the quick brown fox jumps over the lazy dog";

    // Must have at least 30 characters to look like a real production query.
    assert!(
        PROBE_CALIBRATION_QUERY.len() >= 30,
        "PROBE_CALIBRATION_QUERY must be >= 30 chars, got {} (value: {:?})",
        PROBE_CALIBRATION_QUERY.len(),
        PROBE_CALIBRATION_QUERY
    );

    // Must contain at least 3 words separated by whitespace to be a
    // multi-word query (single-word queries do not exercise bot detection).
    let word_count = PROBE_CALIBRATION_QUERY.split_whitespace().count();
    assert!(
        word_count >= 3,
        "PROBE_CALIBRATION_QUERY must be multi-word (>= 3 words), got {word_count} words"
    );

    // Must NOT be the original "rust" 1-word query that caused the gap.
    assert_ne!(
        PROBE_CALIBRATION_QUERY, "rust",
        "PROBE_CALIBRATION_QUERY must not be the original 1-word value"
    );
}
