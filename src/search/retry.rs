// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (HTTP GET with retry/backoff)
//! Retry orchestration for idempotent DuckDuckGo search GETs.
//!
//! Policy lives in [`crate::retry`] (`RetryConfig`, full-jitter backoff,
//! `Retry-After` seconds + HTTP-date, kill switch). Only **GET** (idempotent)
//! — never used for non-idempotent writes.

use crate::retry::{
    deadline_exceeded, http_status_is_retryable, parse_retry_after_ms, sleep_until_deadline,
    status_honors_retry_after, RetryConfig,
};
use rand::RngExt;
use reqwest::{Client, Response, StatusCode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Specific errors returned by `execute_with_retry`.
///
/// Used so the pipeline can tag queries with structured error codes
/// instead of a generic message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryFailReason {
    /// Persistent rate limit after exhausting retries (HTTP 429).
    RateLimited,
    /// Persistent block after exhausting retries (HTTP 403).
    Blocked,
    /// Non-recoverable HTTP error (4xx/5xx status other than 403/429).
    HttpError(u16),
    /// Timeout after 1 retry attempt.
    Timeout,
    /// Generic network error.
    Network(String),
    /// Cooperative cancel (`CancellationToken` / SIGINT/SIGTERM) — typed, not stringly.
    Cancelled,
}

impl RetryFailReason {
    /// Maps to the structured error code in `error::codes`.
    pub fn as_error_code(&self) -> &'static str {
        match self {
            RetryFailReason::RateLimited => crate::error::codes::RATE_LIMITED,
            RetryFailReason::Blocked => crate::error::codes::BLOCKED,
            RetryFailReason::HttpError(_) => crate::error::codes::HTTP_ERROR,
            RetryFailReason::Timeout => crate::error::codes::TIMEOUT,
            RetryFailReason::Network(_) => crate::error::codes::NETWORK_ERROR,
            RetryFailReason::Cancelled => crate::error::codes::CANCELLED,
        }
    }

    /// Returns a human-readable failure description for logs and JSON output.
    pub fn message(&self) -> String {
        match self {
            RetryFailReason::RateLimited => "persistent rate limit (http 429)".to_string(),
            RetryFailReason::Blocked => "blocked by duckduckgo (http 403)".to_string(),
            RetryFailReason::HttpError(status) => format!("http {status} unrecoverable"),
            RetryFailReason::Timeout => "persistent timeout".to_string(),
            RetryFailReason::Network(msg) => format!("network error: {msg}"),
            RetryFailReason::Cancelled => "operation cancelled via sigint/sigterm".to_string(),
        }
    }

    /// True when the failure is cooperative cancel (`CancellationToken` / SIGINT/SIGTERM).
    ///
    /// Used by the pipeline to promote to [`crate::error::CliError::Cancelled`]
    /// (exit 130/143 via signals) instead of a zero-results envelope.
    #[must_use]
    pub fn is_cancellation(&self) -> bool {
        matches!(self, RetryFailReason::Cancelled)
    }

    /// Whether an **external** re-invocation of the CLI may help (agent guidance).
    ///
    /// Distinct from in-process retry classification: after this process has
    /// already exhausted its budget, only some failures remain worth retrying
    /// later (with a longer external backoff).
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        if self.is_cancellation() {
            return false;
        }
        match self {
            // Wait, then retry — respect rate-limit windows.
            RetryFailReason::RateLimited | RetryFailReason::Timeout => true,
            RetryFailReason::Network(_) => true,
            // Soft block needs a long external cool-down (300s+), not immediate retry.
            RetryFailReason::Blocked => false,
            RetryFailReason::HttpError(code) => http_status_is_retryable(
                StatusCode::from_u16(*code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            ),
            RetryFailReason::Cancelled => false,
        }
    }

    /// Complement of [`Self::is_retryable`] excluding cancellations.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        !self.is_retryable() && !self.is_cancellation()
    }
}

/// Result of `execute_with_retry`: either the HTTP response + total attempts, or the failure reason.
#[derive(Debug)]
pub struct RetryResult {
    /// The successful HTTP response body.
    pub response: Response,
    /// Total number of attempts made (1 = no retry needed).
    pub attempts: u32,
}

/// Executes a GET request with retry and backoff. Parameters:
/// * `client` — reqwest client (shared).
/// * `url` — full target URL.
/// * `retries` — number of additional retries (0..=10). 0 = single attempt only.
///   Overridden to 0 when `--disable-retry` process policy is active.
/// * `flag_rate_limit` — signals to other tasks that rate limiting was detected.
///
/// Policy (see [`RetryConfig`]): truncated exponential **full jitter**,
/// wall-clock `max_elapsed`, `Retry-After` (seconds + HTTP-date) on 429/503,
/// and status classification via [`http_status_is_retryable`]. Only **GET**
/// (idempotent) — never used for non-idempotent writes.
///
/// # Errors
///
/// Returns an error if all retry attempts are exhausted due to rate limiting,
/// blocking (HTTP 403 / HTTP 202), timeout, a non-recoverable HTTP status, or
/// a network failure.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future between retries prevents
/// any in-progress `tokio::time::sleep` or pending `send()` from completing,
/// leaving the HTTP connection in an unknown state that `reqwest` will close.
#[tracing::instrument(
    skip_all,
    fields(%url, max_attempts, policy = "ddg_search_get")
)]
pub async fn execute_with_retry(
    client: &Client,
    url: &str,
    retries: u32,
    flag_rate_limit: &Arc<AtomicBool>,
    cancellation: &CancellationToken,
) -> std::result::Result<RetryResult, RetryFailReason> {
    let policy = RetryConfig::from_retries(retries);
    let total_attempts = policy.total_attempts();
    tracing::Span::current().record("max_attempts", total_attempts);

    let deadline = policy.deadline();
    let mut last_reason = RetryFailReason::Network("no attempts executed".to_string());

    if policy.disabled {
        tracing::warn!("retry kill switch active (--disable-retry) — single attempt only");
    }

    for attempt in 0..total_attempts {
        if cancellation.is_cancelled() {
            return Err(RetryFailReason::Cancelled);
        }
        if deadline_exceeded(deadline) {
            tracing::warn!(
                attempt = attempt + 1,
                "retry max_elapsed budget exhausted — stopping"
            );
            return Err(last_reason);
        }

        // Global rate-limit flag set by another task → extra delay (best-effort).
        // Ordering::Relaxed: see store justification below (no multi-field invariant).
        if flag_rate_limit.load(Ordering::Relaxed) && attempt == 0 {
            let extra_ms = rand::rng().random_range(500..1200);
            tracing::debug!(
                extra_ms,
                "global rate-limit flag active — waiting before first attempt"
            );
            if !sleep_until_deadline(extra_ms, deadline).await {
                return Err(last_reason);
            }
        }

        tracing::debug!(
            attempt = attempt + 1,
            total = total_attempts,
            url = %url,
            "retry_attempt: executing GET"
        );

        let envio = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                return Err(RetryFailReason::Cancelled);
            }
            res = client.get(url).send() => res,
        };

        match envio {
            Ok(response) => {
                let status = response.status();
                // Ordering::Relaxed is sufficient for this AtomicBool flag because:
                // 1. It is a best-effort signal — a task that misses the flag simply
                //    retries and discovers the rate-limit itself.
                // 2. No correctness invariant depends on immediate cross-thread visibility.
                // 3. After the flag is set, each task independently adds random delay;
                //    eventual consistency is acceptable for this coordination pattern.
                if status.is_success() && status != StatusCode::ACCEPTED {
                    return Ok(RetryResult {
                        response,
                        attempts: attempt + 1,
                    });
                }

                // Classify permanent vs transient by status code (never by Display string).
                if !http_status_is_retryable(status) {
                    return Err(RetryFailReason::HttpError(status.as_u16()));
                }

                last_reason = match status.as_u16() {
                    429 => {
                        flag_rate_limit.store(true, Ordering::Relaxed);
                        RetryFailReason::RateLimited
                    }
                    202 | 403 => {
                        if status == StatusCode::ACCEPTED {
                            flag_rate_limit.store(true, Ordering::Relaxed);
                        }
                        RetryFailReason::Blocked
                    }
                    code => RetryFailReason::HttpError(code),
                };

                if attempt + 1 >= total_attempts {
                    return Err(last_reason);
                }

                let delay_ms = if status_honors_retry_after(status) {
                    parse_retry_after_ms(&response).unwrap_or_else(|| policy.backoff_ms(attempt))
                } else {
                    policy.backoff_ms(attempt)
                };

                tracing::warn!(
                    attempt = attempt + 1,
                    status = status.as_u16(),
                    backoff_ms = delay_ms,
                    reason = last_reason.as_error_code(),
                    "transient HTTP status — applying backoff before retry"
                );

                if !sleep_until_deadline(delay_ms, deadline).await {
                    return Err(last_reason);
                }
            }
            Err(err) => {
                if err.is_timeout() {
                    last_reason = RetryFailReason::Timeout;
                } else {
                    last_reason = RetryFailReason::Network(err.to_string());
                }

                if attempt + 1 >= total_attempts {
                    return Err(last_reason);
                }

                let delay_ms = policy.backoff_ms(attempt);
                tracing::warn!(
                    attempt = attempt + 1,
                    backoff_ms = delay_ms,
                    timeout = err.is_timeout(),
                    "transient network/timeout error — applying backoff before retry"
                );

                if !sleep_until_deadline(delay_ms, deadline).await {
                    return Err(last_reason);
                }
            }
        }
    }

    Err(last_reason)
}
