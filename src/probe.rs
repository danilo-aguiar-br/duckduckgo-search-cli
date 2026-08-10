// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (one-shot probe / probe-deep diagnostics).
// Parallelism: N/A — single probe session per invocation (GAP-COMP-003).
//! One-shot Chrome HTML probe helpers extracted from `lib` (SRP / GAP-COMP-003).

/// Neutral calibration query for probe endpoints (no operator PII).
const PROBE_CALIBRATION_QUERY: &str = "the quick brown fox jumps over the lazy dog";

/// Which probe ceiling a call site is asking for.
///
/// An enum rather than four free functions so a new ceiling cannot be added
/// without deciding, at the type level, which phase it belongs to.
#[derive(Debug, Clone, Copy)]
enum ProbeCeiling {
    /// Chrome launch during `--probe`.
    Launch,
    /// SERP navigation and DOM extraction during `--probe`.
    Extract,
    /// Chrome launch during `--probe-deep`.
    DeepLaunch,
    /// SERP navigation and DOM extraction during `--probe-deep`.
    DeepExtract,
}

/// Resolve one probe ceiling: `--timeout`, then XDG, then the compiled default.
///
/// # Why this exists
///
/// These were four inline literals — `min(30)`, `min(20)`, `min(30)`,
/// `min(25)` — sitting directly in the Chrome calls. A literal cannot be
/// found by someone tuning a slow host, cannot be documented, and cannot be
/// overridden without a rebuild, which is precisely what the product rule
/// against hardcoded policy is there to prevent.
///
/// The operator's `--timeout` still wins whenever it is SMALLER: asking for
/// less patience must always be honoured, while asking for more must not let a
/// health check block indefinitely.
fn probe_ceiling(args: &crate::cli::CliArgs, which: ProbeCeiling) -> std::time::Duration {
    use crate::types::bounded;
    let xdg = crate::runtime::load_runtime_user_config();
    let ceiling = match which {
        ProbeCeiling::Launch => xdg
            .probe_launch_timeout()
            .unwrap_or(bounded::PROBE_LAUNCH_TIMEOUT_SECONDS),
        ProbeCeiling::Extract => xdg
            .probe_extract_timeout()
            .unwrap_or(bounded::PROBE_EXTRACT_TIMEOUT_SECONDS),
        ProbeCeiling::DeepLaunch => xdg
            .probe_deep_launch_timeout()
            .unwrap_or(bounded::PROBE_DEEP_LAUNCH_TIMEOUT_SECONDS),
        ProbeCeiling::DeepExtract => xdg
            .probe_deep_extract_timeout()
            .unwrap_or(bounded::PROBE_DEEP_EXTRACT_TIMEOUT_SECONDS),
    };
    std::time::Duration::from_secs(args.timeout_seconds.min(ceiling))
}

/// Attach SSOT deep-research budget snapshot for agent preflight (CM-10).
fn attach_deep_research_budget(mut payload: serde_json::Value) -> serde_json::Value {
    if let Some(map) = payload.as_object_mut() {
        map.insert(
            "deep_research_budget".to_string(),
            crate::budget::default_product_snapshot(),
        );
    }
    payload
}

/// Builds the exact JSON value a probe envelope carries on stdout.
///
/// # Why this is public and separate from the emit
///
/// The conformance suite used to validate `serialize_for_wire(&report)` — the
/// struct alone. That is not what the binary writes: the private emit helper
/// injects `deep_research_budget` *after* serialization, so the extra key was
/// invisible to every test while `probe-output.schema.json` still carried
/// `additionalProperties: true`. Two blind spots lined up and the field went
/// undeclared for a whole release.
///
/// Exposing the construction lets a test assert the real bytes without a Chrome
/// session, which is the only reason the probe family had been exempt.
///
/// # Errors
///
/// Returns [`crate::error::CliError::InvalidConfig`] when the report cannot be
/// serialized to a JSON value.
pub fn probe_payload_value<T: serde::Serialize>(
    report: &T,
) -> Result<serde_json::Value, crate::error::CliError> {
    let value =
        serde_json::to_value(report).map_err(|e| crate::error::CliError::InvalidConfig {
            message: format!("failed to serialize probe report: {e}"),
        })?;
    Ok(attach_deep_research_budget(value))
}

/// Emit a probe report through the agent-native projector, and return an exit code.
///
/// # What this replaced, and why the signature changed
///
/// Until v1.0.5 this was `emit_probe_payload`, which serialized straight to
/// stdout and never reached [`crate::output::envelope_ops`]. Every one of the
/// nineteen probe emission sites therefore accepted `--fields`,
/// `--truncate-content`, `--limit`, `--sort`, `--dedupe-by`, `--filter` and
/// `--count-only` and did nothing with any of them: measured on v1.0.4, all
/// four operators tried returned 633 bytes against a 633-byte baseline at
/// exit 0.
///
/// Routing through the projector was only half the fix. Every call site wrote
/// `let _ = emit_probe_payload(...)`, so a REFUSAL would have been swallowed
/// just as silently as the old no-op — the flag would have gone from "accepted
/// and ignored" to "refused and ignored". Returning the exit code, and making
/// each site pass the code it wants on success, is what makes the refusal
/// reach the caller.
///
/// The process wire-keys policy is preserved: the probe is a WIRE surface and
/// has emitted Portuguese spellings under `--wire-keys pt` since before the
/// projector existed. Reduction runs first, on the English document, so
/// `--fields` paths mean the same thing in both languages.
fn emit_probe_or<T: serde::Serialize>(
    report: &T,
    shape: &crate::output::envelope_ops::EnvelopeShape,
    ok_code: i32,
) -> i32 {
    let payload = match probe_payload_value(report) {
        Ok(v) => v,
        Err(err) => {
            tracing::error!(?err, "failed to build probe payload");
            return crate::error::exit_codes::GENERIC_ERROR;
        }
    };
    let code = crate::output::emit_envelope_or_refuse(
        payload,
        shape,
        crate::output::json_pretty_enabled(),
        crate::output::KeyPolicy::ProcessWire,
    );
    if code == crate::error::exit_codes::SUCCESS {
        ok_code
    } else {
        code
    }
}

/// Emit a `--probe` report, returning `ok_code` when the write succeeded.
fn emit_probe(report: &crate::types::ProbeReport, ok_code: i32) -> i32 {
    emit_probe_or(report, &crate::output::envelope_ops::PROBE_SHAPE, ok_code)
}

/// Emit a `--probe-deep` report, returning `ok_code` when the write succeeded.
fn emit_probe_deep(report: &crate::types::ProbeDeepReport, ok_code: i32) -> i32 {
    emit_probe_or(
        report,
        &crate::output::envelope_ops::PROBE_DEEP_SHAPE,
        ok_code,
    )
}

pub(crate) async fn execute_probe_via_chrome(args: &crate::cli::CliArgs, probe_url: &str) -> i32 {
    use crate::error::exit_codes;
    use std::time::Instant;

    let ua = crate::identity::chrome_only_ua_for_platform();
    let started = Instant::now();
    let chrome_path = match crate::browser::detect_chrome(args.chrome_path.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            return emit_probe(
                &crate::types::ProbeReport::failure(
                    "html",
                    0,
                    Some(probe_url.to_string()),
                    false,
                    format!("{e}"),
                    Some(e.error_code().to_string()),
                ),
                e.exit_code(),
            );
        }
    };
    let launch = crate::browser::ChromeBrowser::launch(
        chrome_path.as_path(),
        args.proxy.as_deref(),
        probe_ceiling(args, ProbeCeiling::Launch),
        &ua,
    )
    .await;
    let mut browser = match launch {
        Ok(b) => b,
        Err(e) => {
            let elapsed = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            return emit_probe(
                &crate::types::ProbeReport::failure(
                    "html",
                    elapsed,
                    Some(probe_url.to_string()),
                    false,
                    format!("{e}"),
                    Some(e.error_code().to_string()),
                ),
                e.exit_code(),
            );
        }
    };
    let html = crate::browser::extract_html_with_chrome(
        &mut browser,
        probe_url,
        256 * 1024,
        probe_ceiling(args, ProbeCeiling::Extract),
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
            let verdict_code = if healthy {
                exit_codes::SUCCESS
            } else {
                exit_codes::RATE_LIMITED_OR_BLOCKED
            };
            emit_probe(
                &crate::types::ProbeReport::verdict(
                    "html",
                    latency_ms,
                    probe_url.to_string(),
                    healthy,
                    body.len(),
                    has_serp,
                ),
                verdict_code,
            )
        }
        Err(e) => emit_probe(
            &crate::types::ProbeReport::failure(
                "html",
                latency_ms,
                Some(probe_url.to_string()),
                false,
                format!("{e}"),
                None,
            ),
            exit_codes::GENERIC_ERROR,
        ),
    }
}

pub(crate) async fn execute_probe(args: &crate::cli::CliArgs) -> i32 {
    use crate::error::exit_codes;
    #[cfg(feature = "http-test-harness")]
    use std::time::Instant;

    // GAP-WS-113: probe must use Chrome in production (no reqwest health lies).
    if let Err(e) = crate::chrome_policy::require_chrome_transport() {
        if !crate::chrome_policy::http_test_harness_active() {
            return emit_probe(
                &crate::types::ProbeReport::failure(
                    "html",
                    0,
                    None,
                    false,
                    format!("{e}"),
                    Some(e.error_code().to_string()),
                ),
                e.exit_code(),
            );
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

    // Everything below is the residual HTTP probe and is reachable ONLY under
    // the harness: without `chrome` the transport gate above already returned,
    // and with `chrome` the branch above always returns because
    // `http_test_harness_active()` compiles to a literal `false` (ADR-0029).
    #[cfg(not(feature = "http-test-harness"))]
    {
        // Fail closed rather than `unreachable!()`: if a future refactor ever
        // reaches this point, an agent gets a diagnosable envelope, not a panic.
        let _ = &probe_url;
        emit_probe(
            &crate::types::ProbeReport::failure(
                endpoint,
                0,
                None,
                false,
                "chrome transport is mandatory (GAP-WS-113); pure HTTP is not a production transport"
                    .to_string(),
                Some("CHROME_UNAVAILABLE".to_string()),
            ),
            exit_codes::INVALID_CONFIG,
        )
    }

    // Build a minimal client. Use the same UA + Accept-Language defaults
    // the main pipeline uses (no --probe-specific profile).
    // Pick a User-Agent (rotated, seeded if --seed is set) — keeps probe
    // behavior consistent with the main pipeline.
    #[cfg(feature = "http-test-harness")]
    {
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
        let client = match crate::http::build_client(
            &ua,
            args.timeout_seconds,
            &args.language,
            &args.country,
        ) {
            Ok(c) => c,
            Err(err) => {
                return emit_probe(
                    &crate::types::ProbeReport::failure(
                        endpoint,
                        0,
                        None,
                        false,
                        format!("client build failed: {err}"),
                        None,
                    ),
                    exit_codes::GENERIC_ERROR,
                );
            }
        };

        let started = Instant::now();
        let result = client.get(&probe_url).send().await;
        let latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

        match result {
            Ok(response) => {
                let status = response.status().as_u16();
                let has_set_cookie = response.headers().contains_key("set-cookie");
                // The schema types `status` as the ok/blocked verdict, not the raw
                // HTTP code; the numeric code belongs in `http_status`.
                let healthy = (200..300).contains(&status);
                let report = crate::types::ProbeReport {
                    kind: crate::types::ProbeKind::Probe,
                    status: if healthy {
                        crate::types::ProbeStatus::Ok
                    } else {
                        crate::types::ProbeStatus::Blocked
                    },
                    healthy,
                    endpoint: endpoint.to_string(),
                    http_status: Some(status),
                    latency_ms,
                    has_set_cookie,
                    url: Some(probe_url.clone()),
                    used_chrome: false,
                    chrome_attempted: false,
                    body_len: None,
                    has_result_page_signal: None,
                    error: None,
                    error_code: None,
                };
                // Probe succeeds on ANY HTTP response (even 202/403/429) — caller
                // decides what to do based on the status field.
                emit_probe(&report, exit_codes::SUCCESS)
            }
            Err(err) => emit_probe(
                &crate::types::ProbeReport::failure(
                    endpoint,
                    latency_ms,
                    Some(probe_url.clone()),
                    false,
                    format!("network error: {err}"),
                    None,
                ),
                exit_codes::GENERIC_ERROR,
            ),
        }
    }
}

/// Executes the v0.7.3 PR3 `--probe-deep` health check.
///
/// Runs one real query against the configured endpoint, reads the
/// response body, and classifies it as `captcha | ok` based on the
/// presence of Cloudflare or DDG bot-detection markers. Emits a JSON
/// report on stdout with `status`, `endpoint`, `cascade_level`,
/// `cascade_reason`, and `mitigation_suggestion` (v1.0.3 renamed the third
/// from the Portuguese `cascata_motivo`, which the published schema never
/// declared). Exits 0 on success
/// (including when the probe detected a captcha — the caller is
/// expected to act on the JSON), 1 on network failure.
///
/// GAP-WS-113: probe-deep via real Chrome DOM (CAPTCHA markers on rendered HTML).
#[cfg(feature = "chrome")]
pub(crate) async fn execute_probe_deep_via_chrome(
    args: &crate::cli::CliArgs,
    _probe_url: &str,
) -> i32 {
    use crate::error::exit_codes;
    use crate::probe_deep::{
        detect_interstitial_with_match, mitigation_suggestion_with_marker, InterstitialKind,
    };
    use std::time::Instant;

    let ua = crate::identity::chrome_only_ua_for_platform();
    let started = Instant::now();
    let chrome_path = match crate::browser::detect_chrome(args.chrome_path.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            return emit_probe_deep(
                &crate::types::ProbeDeepReport::failure(
                    "html",
                    None,
                    Some(false),
                    format!("{e}"),
                    Some(e.error_code().to_string()),
                ),
                e.exit_code(),
            );
        }
    };
    let launch = crate::browser::ChromeBrowser::launch(
        chrome_path.as_path(),
        args.proxy.as_deref(),
        probe_ceiling(args, ProbeCeiling::DeepLaunch),
        &ua,
    )
    .await;
    let mut browser = match launch {
        Ok(b) => b,
        Err(e) => {
            let elapsed = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            return emit_probe_deep(
                &crate::types::ProbeDeepReport::failure(
                    "html",
                    Some(elapsed),
                    Some(false),
                    format!("{e}"),
                    Some(e.error_code().to_string()),
                ),
                e.exit_code(),
            );
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
        probe_ceiling(args, ProbeCeiling::DeepExtract),
    )
    .await;
    if let Err(e) = browser.shutdown().await {
        tracing::error!(error = %e, "Chrome shutdown after probe-deep failed");
    }
    let latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    match html {
        Ok(body) => {
            let (marker, kind) = detect_interstitial_with_match(&body);
            let clean = kind == InterstitialKind::None;
            let report = crate::types::ProbeDeepReport {
                kind: crate::types::ProbeDeepKind::ProbeDeep,
                status: if clean {
                    crate::types::ProbeDeepStatus::Ok
                } else {
                    crate::types::ProbeDeepStatus::Captcha
                },
                endpoint: "html".to_string(),
                http_status: Some(if clean { 200 } else { 403 }),
                latency_ms: Some(latency_ms),
                cascade_level: Some(u8::from(!clean)),
                cascade_reason: Some(kind.as_str().to_string()),
                mitigation_suggestion: Some(mitigation_suggestion_with_marker(kind, marker)),
                url: Some(serp_url.clone()),
                used_chrome: Some(true),
                chrome_attempted: Some(true),
                body_len: Some(body.len()),
                error: None,
                error_code: None,
            };
            let verdict_code = if kind == InterstitialKind::None {
                exit_codes::SUCCESS
            } else {
                exit_codes::RATE_LIMITED_OR_BLOCKED
            };
            emit_probe_deep(&report, verdict_code)
        }
        Err(e) => emit_probe_deep(
            &crate::types::ProbeDeepReport::failure(
                "html",
                Some(latency_ms),
                Some(false),
                format!("{e}"),
                None,
            ),
            exit_codes::GENERIC_ERROR,
        ),
    }
}

pub(crate) async fn execute_probe_deep(args: &crate::cli::CliArgs) -> i32 {
    use crate::error::exit_codes;
    #[cfg(feature = "http-test-harness")]
    use crate::probe_deep::{
        detect_interstitial_with_match, mitigation_suggestion_with_marker, InterstitialKind,
    };
    #[cfg(feature = "http-test-harness")]
    use std::time::Instant;

    // GAP-WS-113: probe-deep requires Chrome DOM (no HTTP-only CAPTCHA miss).
    if let Err(e) = crate::chrome_policy::require_chrome_transport() {
        if !crate::chrome_policy::http_test_harness_active() {
            return emit_probe_deep(
                &crate::types::ProbeDeepReport::failure(
                    "html",
                    None,
                    Some(false),
                    format!("{e}"),
                    Some(e.error_code().to_string()),
                ),
                e.exit_code(),
            );
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

    // Residual HTTP path for http-test-harness only — see the note in
    // `execute_probe`: without the harness this point is unreachable, because
    // the transport gate or the Chrome branch above always returned first.
    #[cfg(not(feature = "http-test-harness"))]
    {
        let _ = &probe_url;
        emit_probe_deep(
            &crate::types::ProbeDeepReport::failure(
                endpoint,
                None,
                None,
                "chrome transport is mandatory (GAP-WS-113); pure HTTP is not a production transport"
                    .to_string(),
                Some("CHROME_UNAVAILABLE".to_string()),
            ),
            exit_codes::INVALID_CONFIG,
        )
    }

    #[cfg(feature = "http-test-harness")]
    {
        let ua = crate::http::select_user_agent();
        let client = match crate::http::build_client(
            &ua,
            args.timeout_seconds,
            &args.language,
            &args.country,
        ) {
            Ok(c) => c,
            Err(err) => {
                return emit_probe_deep(
                    &crate::types::ProbeDeepReport::failure(
                        endpoint,
                        None,
                        None,
                        format!("client build failed: {err}"),
                        None,
                    ),
                    exit_codes::GENERIC_ERROR,
                );
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
                let clean = kind == InterstitialKind::None;
                let report = crate::types::ProbeDeepReport {
                    kind: crate::types::ProbeDeepKind::ProbeDeep,
                    status: if clean {
                        crate::types::ProbeDeepStatus::Ok
                    } else {
                        crate::types::ProbeDeepStatus::Captcha
                    },
                    endpoint: endpoint.to_string(),
                    http_status: Some(status),
                    latency_ms: Some(latency_ms),
                    cascade_level: Some(0),
                    cascade_reason: Some(kind.as_str().to_string()),
                    mitigation_suggestion: Some(mitigation_suggestion_with_marker(kind, marker)),
                    url: Some(probe_url.to_string()),
                    used_chrome: None,
                    chrome_attempted: None,
                    body_len: None,
                    error: None,
                    error_code: None,
                };
                // B4 fix: when the probe detects a captcha / interstitial,
                // surface exit 3 (DuckDuckGo 202 block anomaly) so consumers
                // can branch on the exit code instead of parsing the JSON
                // status field. The JSON payload above already carries
                // `status: "captcha"` and the marker hint for downstream use.
                let verdict_code = if kind == InterstitialKind::None {
                    exit_codes::SUCCESS
                } else {
                    exit_codes::RATE_LIMITED_OR_BLOCKED
                };
                emit_probe_deep(&report, verdict_code)
            }
            Err(err) => emit_probe_deep(
                &crate::types::ProbeDeepReport::failure(
                    endpoint,
                    Some(latency_ms),
                    None,
                    format!("network error: {err}"),
                    None,
                ),
                exit_codes::GENERIC_ERROR,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::envelope_ops::{apply_generic, AgentOps, PROBE_DEEP_SHAPE, PROBE_SHAPE};

    /// A verdict envelope, built without a Chrome session.
    fn probe_value() -> serde_json::Value {
        probe_payload_value(&crate::types::ProbeReport::verdict(
            "html",
            120,
            "https://example.invalid/?q=x".to_string(),
            true,
            4096,
            true,
        ))
        .expect("probe payload serializes")
    }

    #[test]
    fn payload_carries_the_discriminator_under_type() {
        let v = probe_value();
        assert_eq!(v["type"], "probe", "probe lost its routing key");
    }

    #[test]
    fn payload_carries_the_budget_snapshot_the_schema_declares() {
        let v = probe_value();
        assert!(
            v.get("deep_research_budget").is_some(),
            "the emit injects this key after serialization; a test that only \
             checked the struct would never see it, which is how it went \
             undeclared for a whole release"
        );
    }

    /// The probe is rowless, so every row operator must refuse.
    ///
    /// This is the assertion that would have failed for the whole of v1.0.4,
    /// when the probe never reached the projector at all.
    #[test]
    fn row_operators_refuse_on_a_rowless_probe_envelope() {
        for ops in [
            AgentOps {
                limit: Some(1),
                ..AgentOps::default()
            },
            AgentOps {
                count_only: true,
                ..AgentOps::default()
            },
            AgentOps {
                sort: Some("status".into()),
                ..AgentOps::default()
            },
            AgentOps {
                dedupe_by: Some("status".into()),
                ..AgentOps::default()
            },
            AgentOps {
                filter: Some("status=ok".into()),
                ..AgentOps::default()
            },
        ] {
            let mut v = probe_value();
            let err = apply_generic(&mut v, &ops, &PROBE_SHAPE)
                .expect_err("a rowless envelope must refuse row operations");
            assert_eq!(err.exit_code(), crate::error::exit_codes::INVALID_CONFIG);
        }
    }

    #[test]
    fn fields_actually_reduces_the_probe_envelope() {
        let full = serde_json::to_string(&probe_value()).expect("json");
        let mut v = probe_value();
        apply_generic(
            &mut v,
            &AgentOps {
                fields: Some("status".into()),
                ..AgentOps::default()
            },
            &PROBE_SHAPE,
        )
        .expect("--fields status is honoured on the probe");
        let reduced = serde_json::to_string(&v).expect("json");
        assert!(
            reduced.len() < full.len(),
            "--fields returned {} bytes against {} — this is exactly the \
             633-against-633 measurement that survived v1.0.4",
            reduced.len(),
            full.len()
        );
        assert_eq!(v["type"], "probe", "projection dropped the routing key");
    }

    #[test]
    fn truncate_shortens_prose_but_spares_identity() {
        let mut v = probe_value();
        apply_generic(
            &mut v,
            &AgentOps {
                truncate_content: Some(4),
                ..AgentOps::default()
            },
            &PROBE_SHAPE,
        )
        .expect("--truncate-content is honoured on the probe");
        assert_eq!(v["type"], "probe", "discriminator was truncated");
        assert_eq!(v["status"], "ok", "status is routable and must survive");
        assert_eq!(
            v["url"], "https://example.invalid/?q=x",
            "a truncated URL points somewhere else, which is worse than a long one"
        );
        assert_eq!(v["endpoint"], "html", "endpoint identifies the transport");
    }

    #[test]
    fn probe_deep_spares_its_cascade_reason() {
        let mut v = probe_payload_value(&crate::types::ProbeDeepReport::failure(
            "html",
            Some(10),
            Some(true),
            "cloudflare_turnstile detected".to_string(),
            Some("BLOCKED".to_string()),
        ))
        .expect("probe-deep payload serializes");
        apply_generic(
            &mut v,
            &AgentOps {
                truncate_content: Some(3),
                ..AgentOps::default()
            },
            &PROBE_DEEP_SHAPE,
        )
        .expect("--truncate-content is honoured on probe-deep");
        assert_eq!(v["type"], "probe_deep");
        assert_eq!(
            v["error_code"], "BLOCKED",
            "an error code an agent branches on cannot be shortened"
        );
        assert!(
            v["error"].as_str().is_some_and(|s| s.chars().count() <= 3),
            "the human message IS content and should have shrunk: {:?}",
            v["error"]
        );
    }

    /// The probe is truncatable BY TYPE, even when a given envelope has no prose.
    ///
    /// # The boundary case this pins
    ///
    /// Measured on the live binary: a HEALTHY `--probe` envelope returns 632
    /// bytes with or without `--truncate-content 5`, because its only strings
    /// are `type`, `status`, `endpoint` and `url` — every one of them identity.
    /// Byte-identical output is the signature of the accepted-and-ignored
    /// defect, so the temptation is to mark the surface `without_content()`
    /// and have it refuse.
    ///
    /// That would be wrong, and this test is why: a FAILURE envelope carries
    /// `error`, which IS prose, and truncating it took the live envelope from
    /// 1024 bytes to 627. The surface can shorten something; this particular
    /// envelope simply had nothing to shorten, exactly like
    /// `--truncate-content 500` on a short document.
    ///
    /// The rule stays static — refuse where the TYPE can never carry prose,
    /// apply where it can — and both halves are measured here rather than
    /// argued in a comment.
    #[test]
    fn truncate_is_a_noop_on_a_healthy_probe_and_real_on_a_failed_one() {
        let mut healthy = probe_value();
        let before = serde_json::to_string(&healthy).expect("json").len();
        apply_generic(
            &mut healthy,
            &AgentOps {
                truncate_content: Some(5),
                ..AgentOps::default()
            },
            &PROBE_SHAPE,
        )
        .expect("truncate applies to the probe surface");
        assert_eq!(
            serde_json::to_string(&healthy).expect("json").len(),
            before,
            "a healthy probe carries only identifiers, so nothing should shrink"
        );

        let mut failed = probe_payload_value(&crate::types::ProbeReport::failure(
            "html",
            0,
            Some("https://example.invalid/".to_string()),
            false,
            "a long human sentence explaining exactly what went wrong".to_string(),
            Some("path_error".to_string()),
        ))
        .expect("failure payload serializes");
        let full = serde_json::to_string(&failed).expect("json").len();
        apply_generic(
            &mut failed,
            &AgentOps {
                truncate_content: Some(5),
                ..AgentOps::default()
            },
            &PROBE_SHAPE,
        )
        .expect("truncate applies to the probe surface");
        let cut = serde_json::to_string(&failed).expect("json").len();
        assert!(
            cut < full,
            "a failure envelope carries prose in `error`, so {cut} must be \
             below {full}; if this ever stops being true the surface really is \
             contentless and should refuse instead"
        );
        assert_eq!(
            failed["error_code"], "path_error",
            "the machine code an agent branches on is identity"
        );
    }

    /// Both probe shapes must be reachable from the published matrix.
    #[test]
    fn probe_shapes_are_published_in_the_capability_matrix() {
        for name in ["--probe", "--probe-deep"] {
            assert!(
                crate::output::envelope_ops::shape_for(name).is_some(),
                "{name} is not published in SURFACES, so an agent cannot learn \
                 what it can honour without collecting exit codes"
            );
        }
    }
}
