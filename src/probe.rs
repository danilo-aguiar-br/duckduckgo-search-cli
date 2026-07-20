// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (one-shot probe / probe-deep diagnostics).
// Parallelism: N/A — single probe session per invocation (GAP-COMP-003).
//! One-shot Chrome HTML probe helpers extracted from `lib` (SRP / GAP-COMP-003).

/// Neutral calibration query for probe endpoints (no operator PII).
const PROBE_CALIBRATION_QUERY: &str = "the quick brown fox jumps over the lazy dog";

pub(crate) async fn execute_probe_via_chrome(args: &crate::cli::CliArgs, probe_url: &str) -> i32 {
    use crate::error::exit_codes;
    use std::time::{Duration, Instant};

    let ua = crate::identity::chrome_only_ua_for_platform();
    let started = Instant::now();
    let chrome_path = match crate::browser::detect_chrome(args.chrome_path.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            let payload = serde_json::json!({
                "type": "probe",
                "endpoint": "html",
                "status": 0u16,
                "latency_ms": 0u64,
                "has_set_cookie": false,
                "url": probe_url,
                "usou_chrome": false,
                "tentou_chrome": true,
                "error": format!("{e}"),
                "error_code": e.error_code(),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    };
    let launch = crate::browser::ChromeBrowser::launch(
        chrome_path.as_path(),
        args.proxy.as_deref(),
        Duration::from_secs(args.timeout_seconds.min(30)),
        &ua,
    )
    .await;
    let mut browser = match launch {
        Ok(b) => b,
        Err(e) => {
            let payload = serde_json::json!({
                "type": "probe",
                "endpoint": "html",
                "status": 0u16,
                "latency_ms": started.elapsed().as_millis() as u64,
                "has_set_cookie": false,
                "url": probe_url,
                "usou_chrome": false,
                "tentou_chrome": true,
                "error": format!("{e}"),
                "error_code": e.error_code(),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    };
    let html = crate::browser::extract_html_with_chrome(
        &mut browser,
        probe_url,
        256 * 1024,
        Duration::from_secs(args.timeout_seconds.min(20)),
    )
    .await;
    if let Err(e) = browser.shutdown().await {
        tracing::error!(error = %e, "Chrome shutdown after probe failed");
    }
    let latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    match html {
        Ok(body) => {
            // GAP-WS-PROBE-403-001: healthy if no interstitial AND (SERP signals
            // or body large enough that it is not a ghost block).
            let interstitial = crate::probe_deep::detect_interstitial(&body);
            let has_serp = crate::probe_deep::has_result_page_signal(&body);
            let healthy = !body.is_empty()
                && interstitial == crate::probe_deep::InterstitialKind::None
                && (has_serp || body.len() >= 4_000);
            let status_str = if healthy { "ok" } else { "blocked" };
            let payload = serde_json::json!({
                "type": "probe",
                "endpoint": "html",
                "status": status_str,
                "http_status": if healthy { 200u16 } else { 0u16 },
                "latency_ms": latency_ms,
                "has_set_cookie": false,
                "url": probe_url,
                "usou_chrome": true,
                "tentou_chrome": true,
                "body_len": body.len(),
                "healthy": healthy,
                "has_result_page_signal": has_serp,
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            if healthy {
                exit_codes::SUCCESS
            } else {
                exit_codes::RATE_LIMITED_OR_BLOCKED
            }
        }
        Err(e) => {
            let payload = serde_json::json!({
                "type": "probe",
                "endpoint": "html",
                "status": 0u16,
                "latency_ms": latency_ms,
                "has_set_cookie": false,
                "url": probe_url,
                "usou_chrome": false,
                "tentou_chrome": true,
                "error": format!("{e}"),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            exit_codes::GENERIC_ERROR
        }
    }
}

pub(crate) async fn execute_probe(args: &crate::cli::CliArgs) -> i32 {
    use crate::error::exit_codes;
    use std::time::Instant;

    // GAP-WS-113: probe must use Chrome in production (no reqwest health lies).
    if let Err(e) = crate::chrome_policy::require_chrome_transport() {
        if !crate::chrome_policy::http_test_harness_active() {
            let payload = serde_json::json!({
                "type": "probe",
                "endpoint": "html",
                "status": 0u16,
                "latency_ms": 0u64,
                "has_set_cookie": false,
                "usou_chrome": false,
                "tentou_chrome": true,
                "error": format!("{e}"),
                "error_code": e.error_code(),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    }

    let endpoint = match args.endpoint {
        crate::cli::CliEndpoint::Html => "html",
        crate::cli::CliEndpoint::Lite => "lite",
    };
    // GAP-WS-PROBE-403-001 v0.9.9: bare /html/ without q= yields short non-SERP
    // bodies that the interstitial heuristic mislabels as 403. Use the same
    // calibration query as probe-deep / pre-flight.
    let kl = format!("{}-{}", args.country, args.language);
    let probe_url = format!(
        "{}?q={}&kl={}",
        crate::search::html_base_url(),
        urlencoding::encode("the quick brown fox jumps over the lazy dog"),
        urlencoding::encode(&kl),
    );
    let _ = endpoint; // always HTML under GAP-WS-113

    // GAP-WS-113: production probe navigates via chromiumoxide (DOM-real health).
    #[cfg(feature = "chrome")]
    if !crate::chrome_policy::http_test_harness_active() {
        return execute_probe_via_chrome(args, &probe_url).await;
    }

    // Build a minimal client. Use the same UA + Accept-Language defaults
    // the main pipeline uses (no --probe-specific profile).
    // Pick a User-Agent (rotated, seeded if --seed is set) — keeps probe
    // behavior consistent with the main pipeline.
    // Pick a User-Agent (rotated, seeded if --seed is set) — keeps probe
    // behavior consistent with the main pipeline.
    let ua = match args.seed {
        Some(seed) => {
            crate::http::select_profile_from_list_seeded(
                &crate::http::load_user_agents(args.match_platform_ua),
                Some(seed),
            )
            .user_agent
        }
        None => crate::http::select_user_agent(),
    };
    let client =
        match crate::http::build_client(&ua, args.timeout_seconds, &args.language, &args.country) {
            Ok(c) => c,
            Err(err) => {
                let payload = serde_json::json!({
                    "type": "probe",
                    "endpoint": endpoint,
                    "status": 0u16,
                    "latency_ms": 0u64,
                    "has_set_cookie": false,
                    "error": format!("client build failed: {err}"),
                });
                let _ = crate::output::print_line_stdout(&payload.to_string());
                return exit_codes::GENERIC_ERROR;
            }
        };

    let started = Instant::now();
    let result = client.get(&probe_url).send().await;
    let latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

    match result {
        Ok(response) => {
            let status = response.status().as_u16();
            let has_set_cookie = response.headers().contains_key("set-cookie");
            let payload = serde_json::json!({
                "type": "probe",
                "endpoint": endpoint,
                "status": status,
                "latency_ms": latency_ms,
                "has_set_cookie": has_set_cookie,
                "url": probe_url,
            });
            // Emit single JSON object to stdout.
            if let Err(err) = crate::output::print_line_stdout(&payload.to_string()) {
                if crate::output::is_broken_pipe(&err) {
                    return exit_codes::BROKEN_PIPE;
                }
                tracing::error!(?err, "failed to emit probe report");
                return exit_codes::GENERIC_ERROR;
            }
            // Probe succeeds on ANY HTTP response (even 202/403/429) — caller
            // decides what to do based on the status field.
            exit_codes::SUCCESS
        }
        Err(err) => {
            let payload = serde_json::json!({
                "type": "probe",
                "endpoint": endpoint,
                "status": 0u16,
                "latency_ms": latency_ms,
                "has_set_cookie": false,
                "url": probe_url,
                "error": format!("network error: {err}"),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            exit_codes::GENERIC_ERROR
        }
    }
}

/// Executes the v0.7.3 PR3 `--probe-deep` health check.
///
/// Runs one real query against the configured endpoint, reads the
/// response body, and classifies it as `captcha | ok` based on the
/// presence of Cloudflare or DDG bot-detection markers. Emits a JSON
/// report on stdout with `status`, `endpoint`, `cascade_level`,
/// `cascata_motivo`, and `mitigation_suggestion`. Exits 0 on success
/// (including when the probe detected a captcha — the caller is
/// expected to act on the JSON), 1 on network failure.
///
/// GAP-WS-113: probe-deep via real Chrome DOM (CAPTCHA markers on rendered HTML).
#[cfg(feature = "chrome")]
pub(crate) async fn execute_probe_deep_via_chrome(args: &crate::cli::CliArgs, _probe_url: &str) -> i32 {
    use crate::error::exit_codes;
    use crate::probe_deep::{
        detect_interstitial_with_match, mitigation_suggestion_with_marker, InterstitialKind,
    };
    use std::time::{Duration, Instant};

    let ua = crate::identity::chrome_only_ua_for_platform();
    let started = Instant::now();
    let chrome_path = match crate::browser::detect_chrome(args.chrome_path.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            let payload = serde_json::json!({
                "type": "probe_deep",
                "endpoint": "html",
                "status": "error",
                "usou_chrome": false,
                "tentou_chrome": true,
                "error": format!("{e}"),
                "error_code": e.error_code(),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    };
    let launch = crate::browser::ChromeBrowser::launch(
        chrome_path.as_path(),
        args.proxy.as_deref(),
        Duration::from_secs(args.timeout_seconds.min(30)),
        &ua,
    )
    .await;
    let mut browser = match launch {
        Ok(b) => b,
        Err(e) => {
            let payload = serde_json::json!({
                "type": "probe_deep",
                "endpoint": "html",
                "status": "error",
                "usou_chrome": false,
                "tentou_chrome": true,
                "latency_ms": started.elapsed().as_millis() as u64,
                "error": format!("{e}"),
                "error_code": e.error_code(),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    };

    // Navigate SERP with calibration query (HTML form semantics via URL).
    let serp_url = crate::search::build_search_url(
        PROBE_CALIBRATION_QUERY,
        &args.language,
        &args.country,
        crate::types::Endpoint::Html,
        None,
        crate::types::SafeSearch::Moderate,
    );
    let html = crate::browser::extract_html_with_chrome(
        &mut browser,
        &serp_url,
        512 * 1024,
        Duration::from_secs(args.timeout_seconds.min(25)),
    )
    .await;
    if let Err(e) = browser.shutdown().await {
        tracing::error!(error = %e, "Chrome shutdown after probe-deep failed");
    }
    let latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    match html {
        Ok(body) => {
            let (marker, kind) = detect_interstitial_with_match(&body);
            let status_str = match kind {
                InterstitialKind::None => "ok",
                _ => "captcha",
            };
            let payload = serde_json::json!({
                "type": "probe_deep",
                "endpoint": "html",
                "status": status_str,
                "http_status": if kind == InterstitialKind::None { 200u16 } else { 403u16 },
                "latency_ms": latency_ms,
                "cascade_level": if kind == InterstitialKind::None { 0 } else { 1 },
                "cascata_motivo": kind.as_str(),
                "mitigation_suggestion": mitigation_suggestion_with_marker(kind, marker),
                "url": serp_url,
                "usou_chrome": true,
                "tentou_chrome": true,
                "body_len": body.len(),
            });
            if let Err(err) = crate::output::print_line_stdout(&payload.to_string()) {
                if crate::output::is_broken_pipe(&err) {
                    return exit_codes::BROKEN_PIPE;
                }
                tracing::error!(?err, "failed to emit probe_deep report");
                return exit_codes::GENERIC_ERROR;
            }
            if kind != InterstitialKind::None {
                exit_codes::RATE_LIMITED_OR_BLOCKED
            } else {
                exit_codes::SUCCESS
            }
        }
        Err(e) => {
            let payload = serde_json::json!({
                "type": "probe_deep",
                "endpoint": "html",
                "status": "error",
                "latency_ms": latency_ms,
                "usou_chrome": false,
                "tentou_chrome": true,
                "error": format!("{e}"),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            exit_codes::GENERIC_ERROR
        }
    }
}

pub(crate) async fn execute_probe_deep(args: &crate::cli::CliArgs) -> i32 {
    use crate::error::exit_codes;
    use crate::probe_deep::{
        detect_interstitial_with_match, mitigation_suggestion_with_marker, InterstitialKind,
    };
    use std::time::Instant;

    // GAP-WS-113: probe-deep requires Chrome DOM (no HTTP-only CAPTCHA miss).
    if let Err(e) = crate::chrome_policy::require_chrome_transport() {
        if !crate::chrome_policy::http_test_harness_active() {
            let payload = serde_json::json!({
                "type": "probe_deep",
                "endpoint": "html",
                "status": "error",
                "usou_chrome": false,
                "tentou_chrome": true,
                "error": format!("{e}"),
                "error_code": e.error_code(),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    }

    let endpoint = match args.endpoint {
        crate::cli::CliEndpoint::Html => "html",
        crate::cli::CliEndpoint::Lite => "lite",
    };
    let probe_url = crate::search::html_base_url(); // GAP-WS-113: always HTML

    // GAP-WS-113: production probe-deep uses Chrome DOM exclusively.
    #[cfg(feature = "chrome")]
    if !crate::chrome_policy::http_test_harness_active() {
        return execute_probe_deep_via_chrome(args, &probe_url).await;
    }

    // Residual HTTP path for http-test-harness only.
    let ua = crate::http::select_user_agent();
    let client =
        match crate::http::build_client(&ua, args.timeout_seconds, &args.language, &args.country) {
            Ok(c) => c,
            Err(err) => {
                let payload = serde_json::json!({
                    "type": "probe_deep",
                    "endpoint": endpoint,
                    "status": "error",
                    "error": format!("client build failed: {err}"),
                });
                let _ = crate::output::print_line_stdout(&payload.to_string());
                return exit_codes::GENERIC_ERROR;
            }
        };

    // Build a minimal form with just `q=`. The HTML endpoint requires
    // POST with a form body, so we send a one-field form.
    let form_data: Vec<(String, String)> =
        vec![("q".to_string(), PROBE_CALIBRATION_QUERY.to_string())];
    let started = Instant::now();
    let result = client.post(&probe_url).form(&form_data).send().await;
    let latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

    match result {
        Ok(response) => {
            let status = response.status().as_u16();
            let body = crate::decompress::response_body_string(response)
                .await
                .unwrap_or_default();
            let (marker, kind) = detect_interstitial_with_match(&body);
            let status_str = match kind {
                InterstitialKind::None => "ok",
                _ => "captcha",
            };
            let payload = serde_json::json!({
                "type": "probe_deep",
                "endpoint": endpoint,
                "status": status_str,
                "http_status": status,
                "latency_ms": latency_ms,
                "cascade_level": 0,
                "cascata_motivo": kind.as_str(),
                "mitigation_suggestion": mitigation_suggestion_with_marker(kind, marker),
                "url": probe_url,
            });
            if let Err(err) = crate::output::print_line_stdout(&payload.to_string()) {
                if crate::output::is_broken_pipe(&err) {
                    return exit_codes::BROKEN_PIPE;
                }
                tracing::error!(?err, "failed to emit probe_deep report");
                return exit_codes::GENERIC_ERROR;
            }
            // B4 fix: when the probe detects a captcha / interstitial,
            // surface exit 3 (DuckDuckGo 202 block anomaly) so consumers
            // can branch on the exit code instead of parsing the JSON
            // status field. The JSON payload above already carries
            // `status: "captcha"` and the marker hint for downstream use.
            if kind != InterstitialKind::None {
                exit_codes::RATE_LIMITED_OR_BLOCKED
            } else {
                exit_codes::SUCCESS
            }
        }
        Err(err) => {
            let payload = serde_json::json!({
                "type": "probe_deep",
                "endpoint": endpoint,
                "status": "error",
                "latency_ms": latency_ms,
                "error": format!("network error: {err}"),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            exit_codes::GENERIC_ERROR
        }
    }
}
