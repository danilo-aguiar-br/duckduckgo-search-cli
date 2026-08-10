// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (pure classification of Chrome transport failures)
//! Chrome transport error classification (GAP-E2E-V11-EXIT-TAXONOMY / V12).
//!
//! Keeps wire codes and exit mapping DRY across `browser::session`,
//! `browser::extract`, `pipeline::chrome`, `lib` exit selection, and
//! `deep-research` empty-result exits.
//!
//! # Contract
//!
//! | Failure class | Wire code | Exit | Agent retry |
//! |---------------|-----------|------|-------------|
//! | not found | `chrome_not_found` | 2 | no |
//! | session transient (launch/WS/timeout) | `chrome_unavailable` | 2 | **yes** |
//! | disabled / policy | `chrome_disabled_by_env` | 2 | no |
//! | genuine empty SERP | `no_results_found` / none | 5 | no |

use std::sync::atomic::{AtomicU32, Ordering};

use super::{codes, exit_codes, CliError};

/// Default additional Chrome session launch/extract attempts after the first.
///
/// SSOT for CLI `--chrome-session-retries` and XDG `chrome_session_retries`.
/// Not a product env. Range enforced at install/validate: 0..=[`MAX_CHROME_SESSION_RETRIES`].
pub const DEFAULT_CHROME_SESSION_RETRIES: u32 = 2;

/// Hard ceiling for Chrome session retries (CLI + XDG validation).
pub const MAX_CHROME_SESSION_RETRIES: u32 = 5;

/// Process-wide Chrome session retry budget installed from CLI / XDG (no product env).
static CHROME_SESSION_RETRIES: AtomicU32 = AtomicU32::new(DEFAULT_CHROME_SESSION_RETRIES);

/// Install process-wide Chrome session retry budget (CLI / XDG / tests).
///
/// Clamped to `0..=MAX_CHROME_SESSION_RETRIES`.
pub fn set_chrome_session_retries(retries: u32) {
    CHROME_SESSION_RETRIES.store(retries.min(MAX_CHROME_SESSION_RETRIES), Ordering::SeqCst);
}

/// Current process-wide Chrome session retry budget (additional attempts after first).
#[must_use]
pub fn chrome_session_retries() -> u32 {
    CHROME_SESSION_RETRIES.load(Ordering::SeqCst)
}

/// Message prefix for launch failures (stable for logs + tests).
pub const PREFIX_LAUNCH: &str = "chrome launch failed: ";
/// Message prefix for CDP/page/navigation failures.
pub const PREFIX_CDP: &str = "chrome cdp failed: ";
/// Message prefix for extract/navigation wall-clock timeouts.
pub const PREFIX_TIMEOUT: &str = "chrome timeout: ";
/// Message prefix for WebSocket / protocol resets mid-session.
pub const PREFIX_WS: &str = "chrome ws reset: ";

/// Returns true when `message` (or Display text) looks like a transient Chrome session fault.
///
/// Used for in-process retry and for [`CliError::is_retryable`] on
/// [`CliError::ChromeUnavailable`].
#[must_use]
pub fn is_chrome_session_transient_message(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("unexpected end of stream")
        || m.contains("resetwithoutclosinghandshake")
        || m.contains("reset without closing handshake")
        || m.contains("connection reset")
        || m.contains("broken pipe")
        || m.contains("connection refused")
        || m.contains("chrome timeout")
        || m.contains("timeout exceeded")
        || m.contains("failed to launch")
        || m.contains("chrome launch failed")
        || m.contains("chrome cdp failed")
        || m.contains("chrome ws reset")
        || m.contains("websocket")
        || m.contains("ws(")
        || m.contains("handshake")
        || m.contains("channel closed")
        || m.contains("target closed")
        || m.contains("session closed")
        || m.contains("browser closed")
        || m.contains("connection error")
}

/// Maps a raw launch error into [`CliError::ChromeUnavailable`] with a stable prefix.
#[must_use]
pub fn chrome_launch_error(source: impl std::fmt::Display) -> CliError {
    CliError::chrome_unavailable(format!("{PREFIX_LAUNCH}{source}"))
}

/// Maps a CDP / page operation error into [`CliError::ChromeUnavailable`].
#[must_use]
pub fn chrome_cdp_error(
    context: impl std::fmt::Display,
    source: impl std::fmt::Display,
) -> CliError {
    let text = source.to_string();
    if is_ws_reset_message(&text) {
        return CliError::chrome_unavailable(format!("{PREFIX_WS}{context}: {text}"));
    }
    CliError::chrome_unavailable(format!("{PREFIX_CDP}{context}: {text}"))
}

/// Maps a wall-clock extract timeout into [`CliError::ChromeUnavailable`].
#[must_use]
pub fn chrome_timeout_error(url: &str) -> CliError {
    CliError::chrome_unavailable(format!("{PREFIX_TIMEOUT}exceeded for {url:?}"))
}

/// True when the error text indicates a WebSocket protocol reset.
#[must_use]
pub fn is_ws_reset_message(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("resetwithoutclosinghandshake")
        || m.contains("reset without closing handshake")
        || m.contains("websocket")
        || m.contains("ws(")
}

/// Whether a [`CliError`] is a transient Chrome session fault eligible for retry.
#[must_use]
pub fn is_chrome_session_transient(err: &CliError) -> bool {
    match err {
        CliError::ChromeUnavailable { message } => is_chrome_session_transient_message(message),
        // Legacy mis-maps still treated as transient until all call sites remapped.
        CliError::HttpError { message, .. } if is_chrome_session_transient_message(message) => true,
        CliError::InvalidConfig { message } if is_chrome_extract_mislabel(message) => true,
        _ => false,
    }
}

fn is_chrome_extract_mislabel(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("chrome html extraction failed")
        || m.contains("chrome news html extraction failed")
        || m.contains("chrome timeout")
}

/// Whether a wire `error` / `erro` string is a Chrome transport or invalid-config class.
#[must_use]
pub fn is_chrome_or_config_wire(code: Option<&str>) -> bool {
    matches!(
        code,
        Some(codes::INVALID_CONFIG)
            | Some(codes::CHROME_UNAVAILABLE)
            | Some(codes::CHROME_DISABLED_BY_ENV)
            | Some(codes::CHROME_NOT_FOUND)
    )
}

/// True when free-text error is Chrome fan-out / process slot failure (CLI-C14).
#[must_use]
pub fn message_implies_chrome_fanout(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("fan-out")
        || m.contains("fanout")
        || m.contains("chrome process")
        || m.contains("failed to launch chrome")
        || m.contains("browser launch")
        || (m.contains("slot") && m.contains("chrome"))
}

/// True when free-text (deep sub-query `mensagem_erro`) indicates Chrome/config failure.
#[must_use]
pub fn message_implies_chrome_or_config(message: &str) -> bool {
    if message_implies_chrome_fanout(message) {
        return true;
    }
    let m = message.to_ascii_lowercase();
    m.contains("chrome")
        || m.contains("chromium")
        || m.contains("cdp")
        || m.contains("invalid configuration")
        || m.contains("invalid_config")
        || m.contains("browserconfig")
}

/// Exit code when total results are zero and optional wire/message errors are known.
///
/// Prefer exit **2** for any Chrome/config class; exit **5** only for genuine empty index.
#[must_use]
pub fn exit_for_zero_results_with_errors<'a>(
    wire_codes: impl IntoIterator<Item = Option<&'a str>>,
    free_text_errors: impl IntoIterator<Item = Option<&'a str>>,
) -> i32 {
    for code in wire_codes {
        if is_chrome_or_config_wire(code) {
            return exit_codes::INVALID_CONFIG;
        }
    }
    for msg in free_text_errors.into_iter().flatten() {
        if message_implies_chrome_or_config(msg) {
            return exit_codes::INVALID_CONFIG;
        }
    }
    exit_codes::ZERO_RESULTS
}

/// Agent-stable next-step hint for Chrome transport failures (GAP-E2E-V11-SUGGESTION-GENERIC).
///
/// Distinguishes **binary missing** (`chrome_not_found` / disabled) from **session/CDP**
/// faults (`chrome_unavailable` timeout/WS/CDP/launch). Session faults must **not**
/// default to a blind “Install Chrome” — operators already have Chrome when the binary
/// was resolved; remediations are Xvfb, warm-up, XDG cookies, proxy, and `--probe-deep`.
#[must_use]
pub fn next_action_suggestion_for_chrome_error(err: &CliError) -> String {
    match err {
        CliError::ChromeNotFound { .. } => {
            "Chrome binary not found (GAP-WS-113). Install Chrome/Chromium or pass \
             --chrome-path. Lite/HTTP are not success paths."
                .to_string()
        }
        CliError::ChromeDisabledByEnv => {
            "Chrome transport unavailable (GAP-WS-113). Rebuild with --features chrome; \
             pass --chrome-path if needed. Lite/HTTP are not success paths."
                .to_string()
        }
        CliError::ChromeUnavailable { message } => {
            let m = message.to_ascii_lowercase();
            if m.contains("timeout") {
                "Chrome session timeout (GAP-WS-113). On Linux use private Xvfb (headed), \
                 warm-up on duckduckgo.com, reuse XDG cookies (no --no-warmup in real ops), \
                 try --proxy if captcha/interstitial, run doctor --probe-deep. Do not treat \
                 zero hits as empty index. Install Chrome only if binary is missing."
                    .to_string()
            } else if m.contains("ws")
                || m.contains("cdp")
                || m.contains("launch")
                || m.contains("websocket")
            {
                "Chrome session/CDP fault (GAP-WS-113). Confirm headed Chrome + stable \
                 display (Xvfb on Linux), single-flight, warm-up, XDG cookie jar, \
                 --proxy if blocked; re-run doctor --probe-deep. Install Chrome only if \
                 the binary is missing."
                    .to_string()
            } else {
                "Chrome unavailable (GAP-WS-113). Verify --chrome-path, Xvfb on Linux, \
                 warm-up, XDG cookies, --proxy; doctor --probe-deep. Lite/HTTP are not \
                 success paths."
                    .to_string()
            }
        }
        CliError::Blocked => {
            "Anti-bot block in Chrome DOM (GAP-WS-113). Wait 300s, use --proxy, keep \
             warm-up and XDG cookies; run --probe-deep. Do not fall back to Lite/HTTP."
                .to_string()
        }
        CliError::InvalidConfig { message }
            if message.to_ascii_lowercase().contains("chrome")
                || message.contains("GAP-WS-113") =>
        {
            "Chrome transport produced no SERP (GAP-WS-113). If Chrome is installed: \
             check Xvfb, warm-up, XDG cookies, --proxy, doctor --probe-deep. If missing: \
             install Chrome/Chromium or pass --chrome-path. Lite/HTTP are not success paths."
                .to_string()
        }
        _ => "Chrome/chromiumoxide required (GAP-WS-113). Verify install/--chrome-path, \
             Xvfb, warm-up, cookies; doctor --probe-deep. Lite/HTTP are not success paths."
            .to_string(),
    }
}

/// Normalizes mis-labeled Chrome failures into [`CliError::ChromeUnavailable`].
///
/// Call at boundaries that still receive legacy `HttpError` / `InvalidConfig` wraps.
#[must_use]
pub fn normalize_chrome_transport_error(err: CliError) -> CliError {
    match err {
        e @ CliError::ChromeUnavailable { .. }
        | e @ CliError::ChromeNotFound { .. }
        | e @ CliError::ChromeDisabledByEnv => e,
        CliError::HttpError { message, .. } if is_chrome_session_transient_message(&message) => {
            if message.to_ascii_lowercase().contains("timeout") {
                CliError::chrome_unavailable(format!("{PREFIX_TIMEOUT}{message}"))
            } else if is_ws_reset_message(&message) {
                CliError::chrome_unavailable(format!("{PREFIX_WS}{message}"))
            } else if message.to_ascii_lowercase().contains("launch") {
                CliError::chrome_unavailable(format!("{PREFIX_LAUNCH}{message}"))
            } else {
                CliError::chrome_unavailable(format!("{PREFIX_CDP}{message}"))
            }
        }
        CliError::InvalidConfig { message } if is_chrome_extract_mislabel(&message) => {
            CliError::chrome_unavailable(message)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_stream_end_is_transient() {
        assert!(is_chrome_session_transient_message(
            "failed to launch Chrome process: unexpected end of stream"
        ));
        let err = chrome_launch_error("unexpected end of stream");
        assert_eq!(err.error_code(), codes::CHROME_UNAVAILABLE);
        assert_eq!(err.exit_code(), exit_codes::INVALID_CONFIG);
        assert!(is_chrome_session_transient(&err));
        assert!(err.is_retryable());
    }

    #[test]
    fn ws_reset_is_transient() {
        assert!(is_chrome_session_transient_message(
            "Ws(Protocol(ResetWithoutClosingHandshake))"
        ));
        let err = chrome_cdp_error("extract", "Ws(Protocol(ResetWithoutClosingHandshake))");
        assert!(matches!(err, CliError::ChromeUnavailable { .. }));
        assert!(is_chrome_session_transient(&err));
    }

    #[test]
    fn timeout_wire_is_chrome_unavailable_not_http() {
        let err = chrome_timeout_error("https://duckduckgo.com/?q=x");
        assert_eq!(err.error_code(), codes::CHROME_UNAVAILABLE);
        assert_eq!(err.exit_code(), exit_codes::INVALID_CONFIG);
        assert!(err.is_retryable());
    }

    #[test]
    fn exit_zero_results_promotes_chrome_wire() {
        assert_eq!(
            exit_for_zero_results_with_errors([Some(codes::CHROME_UNAVAILABLE)], [None]),
            exit_codes::INVALID_CONFIG
        );
        assert_eq!(
            exit_for_zero_results_with_errors([Some(codes::HTTP_ERROR)], [None]),
            exit_codes::ZERO_RESULTS
        );
        assert_eq!(
            exit_for_zero_results_with_errors(
                [None],
                [Some("Chrome HTML extraction failed: timeout")]
            ),
            exit_codes::INVALID_CONFIG
        );
        assert_eq!(
            exit_for_zero_results_with_errors([None], [None]),
            exit_codes::ZERO_RESULTS
        );
    }

    #[test]
    fn normalize_legacy_http_launch() {
        let legacy =
            CliError::http_msg("failed to launch Chrome process: unexpected end of stream");
        let n = normalize_chrome_transport_error(legacy);
        assert_eq!(n.error_code(), codes::CHROME_UNAVAILABLE);
    }

    #[test]
    fn chrome_not_found_not_transient() {
        let err = CliError::chrome_not_found("no binary");
        assert!(!is_chrome_session_transient(&err));
        assert!(!err.is_retryable());
    }

    #[test]
    fn suggestion_not_found_mentions_install() {
        let s = next_action_suggestion_for_chrome_error(&CliError::chrome_not_found("missing"));
        assert!(
            s.contains("Install Chrome") || s.contains("--chrome-path"),
            "not-found must point at install/path, got: {s}"
        );
    }

    #[test]
    fn suggestion_timeout_is_not_blind_install_only() {
        let err = chrome_timeout_error("https://duckduckgo.com/?q=x");
        let s = next_action_suggestion_for_chrome_error(&err);
        assert!(
            s.contains("timeout") || s.contains("session"),
            "timeout suggestion must name session/timeout, got: {s}"
        );
        assert!(
            s.contains("Xvfb") || s.contains("warm-up") || s.contains("probe-deep"),
            "timeout suggestion must remediate session (Xvfb/warm-up/probe), got: {s}"
        );
        // Primary remediation must not be install-only (binary already resolved).
        assert!(
            !s.starts_with("Chrome/chromiumoxide is required")
                && !s.starts_with("Chrome binary not found"),
            "timeout must not use not-found / generic install-only template, got: {s}"
        );
        // "Install Chrome only if" secondary clause is OK; bare "Install Chrome or Chromium," is not.
        assert!(
            !s.contains("Install Chrome or Chromium,"),
            "timeout must not push install as primary fix, got: {s}"
        );
    }

    #[test]
    fn suggestion_cdp_ws_mentions_session_remediation() {
        let err = chrome_cdp_error("nav", "Ws(Protocol(ResetWithoutClosingHandshake))");
        let s = next_action_suggestion_for_chrome_error(&err);
        assert!(
            s.contains("session") || s.contains("CDP") || s.contains("Xvfb"),
            "CDP/WS suggestion must be session-oriented, got: {s}"
        );
        assert!(
            !s.contains("Install Chrome or Chromium,"),
            "CDP/WS must not push install as primary, got: {s}"
        );
    }
}
