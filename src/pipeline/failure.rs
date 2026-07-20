// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure (failure envelope builders)
//! Search failure / cancel envelope constructors (GAP-COMP-006r).

use crate::error::CliError;
use crate::search;
use crate::types::{Config, SearchMetadata, SearchOutput, ZeroCause};
use std::time::Instant;

use super::calculate_selectors_hash;
use super::fill_chrome_agent_metadata;

pub(crate) fn chrome_cancelled_error(stage: &str) -> CliError {
    tracing::warn!(
        stage,
        "Chrome execution cancelled (cooperative cancel → exit 130/143)"
    );
    CliError::Cancelled
}

/// GAP F1 v0.8.9: structured envelope for Chrome failure in news-only mode.
///
/// The news vertical is Chrome-only; a launch/navigation failure must NOT
/// propagate raw `Err` to `lib.rs` (empty stdout + exit 1 would break the contract
/// that `-f json` always emits an envelope). Follows the pattern of
/// [`failure_output`]: empty results and news, `erro`/`mensagem`
/// filled and `causa_zero = resposta-invalida` (non-legitimate, thus exit 6
/// under strict mode and exit 5 under legacy opt-out `--no-zero-cause-strict`).
/// GAP-WS-113: structured envelope when Chrome transport is unavailable or fails.
///
/// Never returns a silent zero-results "legitimo" success — the `error` field is set
/// so callers map to exit 2 (invalid config / chrome unavailable) rather than exit 5.
pub(crate) fn chrome_transport_failure_output(cfg: &Config, err: &CliError, start: Instant) -> SearchOutput {
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let selectors_hash = calculate_selectors_hash(&cfg.selectors);
    let used_proxy = cfg.proxy_config.clone().is_active();
    let identity_used = crate::identity::identity_tag_for_cli_identity(cfg.identity_profile, None);
    let mut out = SearchOutput {
        query: cfg.query.as_str().to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp: crate::types::utc_now(),
        region: search::format_kl(cfg.language.as_str(), cfg.country.as_str()),
        result_count: 0,
        results: Vec::new(),
        pages_fetched: 0,
        news: None,
        news_count: None,
        error: Some(err.error_code().to_string()),
        message: Some(format!("{err}")),
        metadata: SearchMetadata {
            execution_time_ms: elapsed_ms,
            selectors_hash,
            retries: 0,
            retries_configured: Some(cfg.retries.get()),
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            // GAP-WS-META-NO-CHROME-001: do not claim an attempt when Chrome is unavailable.
            chrome_attempted: !crate::chrome_policy::chrome_disabled_by_env()
                || crate::chrome_policy::http_test_harness_active(),
            user_agent: cfg.user_agent.as_str().to_string(),
            used_proxy,
            identity_used,
            cascade_level: None,
            pre_flight_fired: false,
            pre_flight_executed: false,
            pre_flight_status: None,
            news_promo_filtered: None,
            stream_requested: if cfg.stream_mode { Some(true) } else { None },
            stream_effective: if cfg.stream_mode { Some(false) } else { None },
            zero_cause: None,
            // GAP-EN-036: agent-stable JSON is English (one-shot stdout contract).
            next_action_suggestion: Some(
                "Chrome/chromiumoxide is required (GAP-WS-113). Install Chrome or Chromium, \
                 pass --chrome-path if needed (Chrome feature is mandatory in production). \
                 Lite and pure HTTP are not success paths."
                    .to_string(),
            ),
            bytes_raw: Some(0),
            bytes_decompressed: Some(0),
            cascade_level_observed: None,
            result_count_compat: None,
            endpoint_used_compat: Some("html".to_string()),
            vertical_used: Some(cfg.vertical.as_str().to_string()),
            chrome_path_resolved: None,
            chrome_channel: None,
            run_id: Some(crate::types::RunId::generate()),
        },
    };
    fill_chrome_agent_metadata(&mut out.metadata, cfg);
    out
}

#[cfg(feature = "chrome")]
#[cold]
pub(crate) fn news_only_chrome_failure_output(cfg: &Config, err: &CliError, start: Instant) -> SearchOutput {
    let mut output =
        failure_output_from_parts(cfg, err.error_code().to_string(), format!("{err}"), start);
    output.news = Some(Vec::new());
    output.news_count = Some(0);
    output.metadata.chrome_attempted = true;
    output.metadata.zero_cause = Some(ZeroCause::InvalidResponse);
    output.metadata.next_action_suggestion = Some(
        "Chrome transport failed on the news vertical (Chrome-only, no HTTP fallback); \
         verify Chrome/Chromium install, --chrome-path and the Xvfb environment, \
         then re-run. Lite/HTTP are not success paths (GAP-WS-113)."
            .to_string(),
    );
    output
}

/// Generates a `SearchOutput` from a retry failure, preserving the structured error code
/// and partial metrics.
#[cold]
pub(crate) fn failure_output(cfg: &Config, reason: &search::RetryFailReason, start: Instant) -> SearchOutput {
    failure_output_from_parts(
        cfg,
        reason.as_error_code().to_string(),
        reason.message(),
        start,
    )
}

/// Shared core of [`failure_output`] (GAP F1 v0.8.9): builds the
/// failure envelope from already-formatted code and message, allowing
/// que o caminho Chrome news-only reutilize o mesmo esqueleto de metadados.
#[cold]
pub(crate) fn failure_output_from_parts(
    cfg: &Config,
    error_code: String,
    message: String,
    start: Instant,
) -> SearchOutput {
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = crate::types::utc_now();
    let run_id = crate::types::RunId::generate();
    let selectors_hash = calculate_selectors_hash(&cfg.selectors);
    let used_proxy = cfg.proxy_config.clone().is_active();

    // GAP-AUD-001: when the operator pins an identity via `--identity-profile`,
    // the failure envelope must report the SAME identity tag the success path
    // would have reported. `identity_tag_for_cli_identity` reuses the canonical
    // `IdentityProfile::tag()` formatter to guarantee format parity.
    let identity_used = crate::identity::identity_tag_for_cli_identity(cfg.identity_profile, None);

    let mut out = SearchOutput {
        query: cfg.query.as_str().to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: cfg.endpoint.as_str().to_string(),
        timestamp,
        region: search::format_kl(cfg.language.as_str(), cfg.country.as_str()),
        result_count: 0,
        results: Vec::new(),
        pages_fetched: 0,
        news: None,
        news_count: None,
        error: Some(error_code),
        message: Some(message),
        metadata: SearchMetadata {
            execution_time_ms: elapsed_ms,
            selectors_hash,
            retries: cfg.retries.get(),
            retries_configured: Some(cfg.retries.get()),
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            chrome_attempted: false,
            user_agent: cfg.user_agent.as_str().to_string(),
            used_proxy,
            identity_used,
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
            // GAP-WS-104: even in the failure envelope, diagnosis reports the
            // requested vertical when != web (consistency with success).
            vertical_used: Some(cfg.vertical.as_str().to_string()),
            chrome_path_resolved: None,
            chrome_channel: None,
            run_id: Some(run_id),
        },
    };
    fill_chrome_agent_metadata(&mut out.metadata, cfg);
    out
}
