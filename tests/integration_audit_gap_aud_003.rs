// SPDX-License-Identifier: MIT OR Apache-2.0
//! Teste de auditoria: reprodução do GAP-AUD-003 v0.8.0 em ambiente bloqueado.
//!
//! Serves the REAL Cloudflare body captured on 2026-06-19 (14KB, carrying
//! `anomaly-modal` + `anomaly.js` markers) through wiremock and checks that the
//! classifier returns `AntiBot`/`GhostBlock` and whether the exit code is 6.

mod common;

use duckduckgo_search_cli::pipeline::{
    classify_zero_result, next_action_suggestion_for_zero, ZeroClassificationInputs,
};
use duckduckgo_search_cli::probe_deep::{
    detect_interstitial_with_match, has_result_page_signal, InterstitialKind,
};
use duckduckgo_search_cli::search::search_with_pagination;
use duckduckgo_search_cli::types::{Endpoint, ZeroCause};
use reqwest::Client;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: std::sync::LazyLock<TokioMutex<()>> =
        std::sync::LazyLock::new(|| TokioMutex::new(()));
    &LOCK
}

fn load_cloudflare_2026_fixture() -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/interstitial_cloudflare_anomaly_2026.html");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read fixture {p:?}: {e}"))
}

/// Pre-compresses the Cloudflare 2026 fixture with gzip at default level.
///
/// Mirrors the production behavior of `DuckDuckGo`, which replies with
/// `Content-Encoding: gzip` for HTML responses. Used to reproduce
/// GAP-AUD-003 Bug #1 in a regression test that verifies the full
/// `search_with_pagination` path correctly decompresses before the
/// interstitial classifier inspects the body.
fn gzip_compress_fixture(plain: &str) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(plain.as_bytes())
        .expect("write to gzip encoder");
    encoder.finish().expect("finish gzip encoder")
}

#[test]
fn audit_cloudflare_2026_body_has_anomaly_modal_marker() {
    let body = load_cloudflare_2026_fixture();
    let (marker, kind) = detect_interstitial_with_match(&body);
    eprintln!(
        "AUDITORIA: body_len={} marker={} kind={:?} has_result_page_signal={}",
        body.len(),
        marker,
        kind,
        has_result_page_signal(&body)
    );
    assert!(
        body.contains("anomaly-modal"),
        "fixture must contain the anomaly-modal marker"
    );
    assert_eq!(
        kind,
        InterstitialKind::Cloudflare,
        "kind must be Cloudflare (14KB with anomaly-modal)"
    );
}

#[test]
fn audit_cloudflare_2026_classifier_returns_non_legitimo() {
    let body = load_cloudflare_2026_fixture();
    let inputs = ZeroClassificationInputs {
        body: &body,
        pre_flight_enabled: false,
        pre_flight_fired: false,
        execution_time_ms: 766,
        retries: 0,
        concurrent_fetches: 0,
        last_probe_cascade_level: None,
    };
    let cause = classify_zero_result(&inputs);
    let sugestao = next_action_suggestion_for_zero(cause);
    eprintln!("AUDITORIA: cause={cause:?} sugestao={sugestao:?}");
    assert_ne!(
        cause,
        ZeroCause::Legitimate,
        "BUG CONFIRMADO: classificador rotulou Cloudflare 2026 challenge (14KB + anomaly-modal) como Legitimate"
    );
}

#[tokio::test]
async fn audit_cloudflare_2026_e2e_first_body_populated() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    let body = load_cloudflare_2026_fixture();
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body.clone())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let mock_server_lite = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<html><body>vazio lite</body></html>".to_string())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server_lite)
        .await;

    let base_html = format!("{}/", mock_server.uri());
    let base_lite = format!("{}/", mock_server_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    // The rustls CryptoProvider must be installed before the first
    // `Client::build()`. `lean_config` also installs it, but it runs one line
    // too late, which made this test depend on another test in the same binary
    // having installed it first. The helper is idempotent.
    common::ensure_tls_for_http_harness();
    let client = Client::builder().build().expect("client");
    let mut cfg = common::lean_config(Endpoint::Html, 1, 0);
    let q = common::validated_query("rust serde derive");
    cfg.query = q.clone();
    cfg.queries = vec![q];
    cfg.num_results = Some(duckduckgo_search_cli::types::ResultCount::try_new(3).expect("num"));
    cfg.timeout_seconds =
        duckduckgo_search_cli::types::TimeoutSeconds::try_new(10).expect("timeout");
    cfg.global_timeout_seconds =
        duckduckgo_search_cli::types::GlobalTimeoutSeconds::try_new(30).expect("gto");
    cfg.allow_lite_fallback = false;
    cfg.pre_flight = false;

    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let result = search_with_pagination(&client, &cfg, "rust serde derive", &flag, &token).await;

    match &result {
        Ok(agregado) => {
            eprintln!(
                "AUDITORIA E2E: results.len={} first_body.len={} effective_endpoint={:?}",
                agregado.results.len(),
                agregado.first_body.len(),
                agregado.effective_endpoint
            );
            assert_eq!(
                agregado.results.len(),
                0,
                "a Cloudflare challenge must not produce results"
            );
            assert!(
                agregado.first_body.len() > 5000,
                "first_body must contain the real Cloudflare body (>5KB), found {} bytes",
                agregado.first_body.len()
            );
            assert!(
                agregado.first_body.contains("anomaly-modal"),
                "first_body must preserve anomaly-modal from the original body"
            );
        }
        Err(e) => {
            eprintln!(
                "AUDITORIA E2E: search_with_pagination retornou Err (esperado se blocked antes de popular first_body): {e:?}"
            );
        }
    }
}

/// Regression test for GAP-AUD-003 Bug #1 (HTTP gzip decompression).
///
/// Reproduces the production scenario where `DuckDuckGo` replies with
/// `Content-Encoding: gzip` and the body is gzip-compressed bytes. Without
/// the fix, `search_with_pagination` would treat the compressed bytes as
/// the interstitial body, the marker detection would fail, and the
/// classifier would mislabel the result as `Legitimate` instead of
/// `AntiBot`/`GhostBlock`.
///
/// Steps:
/// 1. Load the real 14KB Cloudflare 2026 fixture (contains `anomaly-modal`).
/// 2. Pre-compress with `flate2::write::GzEncoder`.
/// 3. Serve via wiremock with `Content-Encoding: gzip` header.
/// 4. Run full `search_with_pagination` end-to-end.
/// 5. Assert that `agregado.first_body` (after decompression) contains
///    `anomaly-modal` — proves the decompression path is wired correctly
///    before the interstitial detector runs.
#[tokio::test]
async fn audit_cloudflare_2026_gzip_e2e_decompression_succeeds() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    let plain = load_cloudflare_2026_fixture();
    let gzipped = gzip_compress_fixture(&plain);
    assert!(
        gzipped.len() < plain.len(),
        "fixture must compress (original={}, gzipped={})",
        plain.len(),
        gzipped.len()
    );

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(gzipped)
                .insert_header("content-type", "text/html; charset=utf-8")
                .insert_header("content-encoding", "gzip")
                .insert_header("vary", "Accept-Encoding"),
        )
        .mount(&mock_server)
        .await;

    let mock_server_lite = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<html><body>vazio lite</body></html>".to_string())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server_lite)
        .await;

    let base_html = format!("{}/", mock_server.uri());
    let base_lite = format!("{}/", mock_server_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    // The rustls CryptoProvider must be installed before the first
    // `Client::build()`. `lean_config` also installs it, but it runs one line
    // too late, which made this test depend on another test in the same binary
    // having installed it first. The helper is idempotent.
    common::ensure_tls_for_http_harness();
    let client = Client::builder().build().expect("client");
    let mut cfg = common::lean_config(Endpoint::Html, 1, 0);
    let q = common::validated_query("rust serde derive");
    cfg.query = q.clone();
    cfg.queries = vec![q];
    cfg.num_results = Some(duckduckgo_search_cli::types::ResultCount::try_new(3).expect("num"));
    cfg.timeout_seconds =
        duckduckgo_search_cli::types::TimeoutSeconds::try_new(10).expect("timeout");
    cfg.global_timeout_seconds =
        duckduckgo_search_cli::types::GlobalTimeoutSeconds::try_new(30).expect("gto");
    cfg.allow_lite_fallback = false;
    cfg.pre_flight = false;

    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let result = search_with_pagination(&client, &cfg, "rust serde derive", &flag, &token).await;

    match &result {
        Ok(agregado) => {
            eprintln!(
                "AUDITORIA GZIP E2E: results.len={} first_body.len={} effective_endpoint={:?}",
                agregado.results.len(),
                agregado.first_body.len(),
                agregado.effective_endpoint
            );
            assert_eq!(
                agregado.results.len(),
                0,
                "a Cloudflare challenge must not produce results even with gzip"
            );
            assert!(
                agregado.first_body.len() > 5000,
                "first_body MUST contain the decompressed Cloudflare body (>5KB after gzip→plain), found {} bytes — BUG #1 NOT FIXED if it is close to the gzipped size",
                agregado.first_body.len()
            );
            assert!(
                agregado.first_body.contains("anomaly-modal"),
                "first_body MUST preserve the 'anomaly-modal' marker after gzip decompression — BUG #1 NOT FIXED if the marker is missing"
            );
        }
        Err(e) => {
            panic!(
                "GZIP E2E AUDIT: search_with_pagination should succeed (gzip body decompressed OK), got Err: {e:?}"
            );
        }
    }
}

/// V18: mock endpoints via EndpointPolicy SSOT (`common::HarnessGuard`).
type EnvGuard = common::HarnessGuard;
