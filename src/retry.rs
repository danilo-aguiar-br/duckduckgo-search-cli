// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative / pure (retry policy + delay math; no network I/O)
//! Named retry policy for DuckDuckGo HTTP reads (idempotent GET only).
//!
//! # Architectural decision (inline ADR)
//!
//! Retry is **opt-in via `--retries N`** (default 2 additional attempts) and
//! used by:
//! - DuckDuckGo **search GET** paths in [`crate::search::execute_with_retry`]
//! - Chrome **news vertical** empty/interstitial recovery (GAP-E2E-51-006) in
//!   [`crate::pipeline::chrome`] — full-jitter backoff, never fake-success
//!
//! Content fetch does not share this policy (different failure modes).
//!
//! Operations are **GET-only** against public HTML SERP endpoints, so they are
//! idempotent by HTTP semantics — no `Idempotency-Key` is required.
//!
//! # Kill switch (GAP-SCRAPE-R2-010)
//!
//! Use CLI `--disable-retry` (or `--retries 0`) to force zero retries during
//! an incident. Product environment variables are **not** read.

use rand::RngExt;
use reqwest::{Response, StatusCode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Process-wide retry kill switch installed from CLI (`--disable-retry`).
static RETRY_DISABLED: AtomicBool = AtomicBool::new(false);

/// Install process-wide retry kill switch (CLI / tests).
pub fn set_retry_disabled(disabled: bool) {
    RETRY_DISABLED.store(disabled, Ordering::SeqCst);
}

/// Default initial backoff base before full jitter (ms). Rules: 100–500 ms.
pub const DEFAULT_INITIAL_BACKOFF_MS: u64 = 200;
/// Hard cap for exponential growth (ms). Rules: 30–60 s.
pub const DEFAULT_MAX_BACKOFF_MS: u64 = 30_000;
/// Wall-clock budget for the entire retry loop (ms), independent of attempt count.
pub const DEFAULT_MAX_ELAPSED_MS: u64 = 120_000;
/// Cap for `Retry-After` values interpreted as delay (seconds → ms).
pub const RETRY_AFTER_MAX_SECS: u64 = 120;
/// Exponent clamp for `2^attempt` so delays never overflow `u64` mul.
const BACKOFF_EXPONENT_CAP: u32 = 10;

/// Named, documented retry policy for one external dependency (DDG search).
///
/// Separates policy from business logic: call sites pass a [`RetryConfig`]
/// (or build one via [`RetryConfig::from_retries`]) instead of scattering
/// magic numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryConfig {
    /// Additional attempts after the first (0 = single shot). Clamped to 0..=10.
    pub max_retries: u32,
    /// Base delay (ms) at attempt 0 before full jitter.
    pub initial_backoff_ms: u64,
    /// Truncation ceiling (ms) for exponential growth.
    pub max_backoff_ms: u64,
    /// Maximum wall-clock time for the whole loop (ms). Checked with
    /// [`std::time::Instant`] (monotonic).
    pub max_elapsed_ms: u64,
    /// When true, effective retries are forced to 0 (kill switch / opt-out).
    pub disabled: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_backoff_ms: DEFAULT_INITIAL_BACKOFF_MS,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
            max_elapsed_ms: DEFAULT_MAX_ELAPSED_MS,
            disabled: false,
        }
    }
}

impl RetryConfig {
    /// Build policy from CLI `--retries N`, applying process kill switch.
    #[must_use]
    pub fn from_retries(max_retries: u32) -> Self {
        let disabled = retry_disabled();
        Self {
            max_retries: max_retries.min(crate::cli::MAX_RETRIES),
            disabled,
            ..Self::default()
        }
    }

    /// Effective additional retries after kill switch / clamp.
    #[must_use]
    pub fn effective_retries(&self) -> u32 {
        if self.disabled {
            0
        } else {
            self.max_retries.min(crate::cli::MAX_RETRIES)
        }
    }

    /// Total attempts = effective_retries + 1 (at least 1).
    #[must_use]
    pub fn total_attempts(&self) -> u32 {
        self.effective_retries().saturating_add(1)
    }

    /// Monotonic deadline for this loop (`now + max_elapsed`).
    #[must_use]
    pub fn deadline(&self) -> Instant {
        Instant::now() + Duration::from_millis(self.max_elapsed_ms.max(1))
    }

    /// Full-jitter exponential backoff for `attempt` (0-based).
    ///
    /// Formula: `delay = random(0..=min(initial * 2^attempt, max_backoff))`.
    #[must_use]
    pub fn backoff_ms(&self, attempt: u32) -> u64 {
        calculate_backoff_ms(attempt, self.initial_backoff_ms, self.max_backoff_ms)
    }

    /// Remaining time until deadline; `Duration::ZERO` if exhausted.
    #[must_use]
    pub fn remaining(deadline: Instant) -> Duration {
        deadline.saturating_duration_since(Instant::now())
    }
}

/// Returns true when the process kill switch is active (`--disable-retry`).
#[must_use]
pub fn retry_disabled() -> bool {
    RETRY_DISABLED.load(Ordering::SeqCst)
}

/// Deprecated alias — prefer [`retry_disabled`] (no product env).
#[must_use]
#[inline]
pub fn retry_disabled_by_env() -> bool {
    retry_disabled()
}

/// Truncated exponential backoff with **full jitter** (AWS-style).
///
/// - Growth factor 2.0 (`1 << attempt`)
/// - Cap at `max_backoff_ms`
/// - Jitter: uniform sample in `0..=cap` (desynchronizes clients)
///
/// Uses saturating arithmetic; never panics on overflow.
#[must_use]
pub fn calculate_backoff_ms(attempt: u32, initial_ms: u64, max_backoff_ms: u64) -> u64 {
    let factor = 1u64 << attempt.min(BACKOFF_EXPONENT_CAP);
    let exp = initial_ms.saturating_mul(factor);
    let cap = exp.min(max_backoff_ms.max(1));
    if cap == 0 {
        return 0;
    }
    rand::rng().random_range(0..=cap)
}

/// Deterministic exponential cap (no jitter) — for tests and property checks.
#[must_use]
pub fn exponential_cap_ms(attempt: u32, initial_ms: u64, max_backoff_ms: u64) -> u64 {
    let factor = 1u64 << attempt.min(BACKOFF_EXPONENT_CAP);
    initial_ms.saturating_mul(factor).min(max_backoff_ms.max(1))
}

/// True when the HTTP status is a proven transient condition for **idempotent GET**.
///
/// Retries:
/// - `202` — DDG soft-block anomaly
/// - `403` — may clear after UA/identity rotation at a higher layer
/// - `408` — request timeout
/// - `429` — rate limit (`Retry-After` preferred)
/// - `502` / `503` / `504` — gateway / overload
///
/// Does **not** retry permanent client errors (`400`, `401`, `404`, `422`, …).
#[must_use]
pub fn http_status_is_retryable(status: StatusCode) -> bool {
    matches!(
        status.as_u16(),
        202 | 403 | 408 | 429 | 502 | 503 | 504
    )
}

/// True for statuses that honor `Retry-After` when present.
#[must_use]
pub fn status_honors_retry_after(status: StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 503)
}

/// Parse `Retry-After` as **milliseconds**, supporting delta-seconds and HTTP-date.
///
/// - Delta-seconds: clamped to [`RETRY_AFTER_MAX_SECS`]
/// - HTTP-date (RFC 2822 / IMF-fixdate): delay = `date - now`, reject if past
/// - Unparseable / absent → `None`
#[must_use]
pub fn parse_retry_after_ms(response: &Response) -> Option<u64> {
    parse_retry_after_header_value(response.headers().get("retry-after")?.to_str().ok()?)
}

/// Pure parser for a raw `Retry-After` header value (testable without HTTP).
#[must_use]
pub fn parse_retry_after_header_value(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    // Delta-seconds (preferred by most CDNs).
    if let Ok(secs) = value.parse::<u64>() {
        return Some(secs.min(RETRY_AFTER_MAX_SECS).saturating_mul(1000));
    }
    // HTTP-date (IMF-fixdate / RFC 2822).
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(value) {
        let target = dt.timestamp();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs() as i64;
        if target <= now {
            // Past or equal → do not block; caller falls back to exponential.
            return None;
        }
        let delta = (target - now) as u64;
        return Some(delta.min(RETRY_AFTER_MAX_SECS).saturating_mul(1000));
    }
    None
}

/// Sleep at most `delay_ms`, never past `deadline`. Returns `false` if no time left.
pub async fn sleep_until_deadline(delay_ms: u64, deadline: Instant) -> bool {
    let remaining = RetryConfig::remaining(deadline);
    if remaining.is_zero() {
        return false;
    }
    let sleep_for = Duration::from_millis(delay_ms).min(remaining);
    if sleep_for.is_zero() {
        return false;
    }
    tokio::time::sleep(sleep_for).await;
    true
}

/// True when the deadline has already elapsed.
#[must_use]
pub fn deadline_exceeded(deadline: Instant) -> bool {
    Instant::now() >= deadline
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_cap_grows_and_truncates() {
        assert_eq!(exponential_cap_ms(0, 200, 30_000), 200);
        assert_eq!(exponential_cap_ms(1, 200, 30_000), 400);
        assert_eq!(exponential_cap_ms(2, 200, 30_000), 800);
        assert_eq!(exponential_cap_ms(8, 200, 30_000), 30_000); // 200*256=51200 → cap
        assert_eq!(exponential_cap_ms(20, 200, 30_000), 30_000); // exponent clamp
    }

    #[test]
    fn full_jitter_stays_within_cap() {
        let max = exponential_cap_ms(3, 200, 30_000);
        for _ in 0..64 {
            let d = calculate_backoff_ms(3, 200, 30_000);
            assert!(d <= max, "jitter {d} exceeded cap {max}");
        }
    }

    #[test]
    fn parse_retry_after_delta_seconds() {
        assert_eq!(parse_retry_after_header_value("2"), Some(2_000));
        assert_eq!(parse_retry_after_header_value("0"), Some(0));
        assert_eq!(
            parse_retry_after_header_value("9999"),
            Some(RETRY_AFTER_MAX_SECS * 1000)
        );
    }

    #[test]
    fn parse_retry_after_http_date_future() {
        let future = chrono::Utc::now() + chrono::Duration::seconds(5);
        // RFC 2822 via chrono (HTTP-date compatible for our parser).
        let header = future.to_rfc2822();
        let ms = parse_retry_after_header_value(&header).expect("future HTTP-date");
        // Allow 1s slack for clock / formatting.
        assert!(ms > 0 && ms <= RETRY_AFTER_MAX_SECS * 1000);
    }

    #[test]
    fn parse_retry_after_http_date_past_is_none() {
        let past = chrono::Utc::now() - chrono::Duration::seconds(60);
        assert!(parse_retry_after_header_value(&past.to_rfc2822()).is_none());
    }

    #[test]
    fn parse_retry_after_garbage_is_none() {
        assert!(parse_retry_after_header_value("not-a-date").is_none());
        assert!(parse_retry_after_header_value("").is_none());
    }

    #[test]
    fn http_status_classification() {
        assert!(http_status_is_retryable(StatusCode::TOO_MANY_REQUESTS));
        assert!(http_status_is_retryable(StatusCode::SERVICE_UNAVAILABLE));
        assert!(http_status_is_retryable(StatusCode::BAD_GATEWAY));
        assert!(http_status_is_retryable(StatusCode::GATEWAY_TIMEOUT));
        assert!(http_status_is_retryable(StatusCode::REQUEST_TIMEOUT));
        assert!(http_status_is_retryable(StatusCode::FORBIDDEN));
        assert!(http_status_is_retryable(StatusCode::ACCEPTED));
        assert!(!http_status_is_retryable(StatusCode::NOT_FOUND));
        assert!(!http_status_is_retryable(StatusCode::BAD_REQUEST));
        assert!(!http_status_is_retryable(StatusCode::UNAUTHORIZED));
        assert!(!http_status_is_retryable(StatusCode::UNPROCESSABLE_ENTITY));
        assert!(!http_status_is_retryable(StatusCode::INTERNAL_SERVER_ERROR));
    }

    #[test]
    fn retry_config_disabled_forces_zero() {
        let mut cfg = RetryConfig::from_retries(5);
        cfg.disabled = true;
        assert_eq!(cfg.effective_retries(), 0);
        assert_eq!(cfg.total_attempts(), 1);
    }

    #[test]
    fn retry_config_clamps_to_max() {
        let cfg = RetryConfig {
            max_retries: 999,
            disabled: false,
            ..RetryConfig::default()
        };
        assert_eq!(cfg.effective_retries(), crate::cli::MAX_RETRIES);
    }

    #[test]
    fn status_honors_retry_after_only_429_503() {
        assert!(status_honors_retry_after(StatusCode::TOO_MANY_REQUESTS));
        assert!(status_honors_retry_after(StatusCode::SERVICE_UNAVAILABLE));
        assert!(!status_honors_retry_after(StatusCode::BAD_GATEWAY));
        assert!(!status_honors_retry_after(StatusCode::FORBIDDEN));
    }
}
