// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (Chrome news vertical SERP)
//! Chrome news-vertical search path (SRP split from `chrome`).

use crate::error::CliError;
use crate::types::Config;
use super::super::failure::chrome_cancelled_error;
use super::launch_chrome_browser;

#[cfg(feature = "chrome")]
async fn prime_news_session_with_web_serp(
    browser: &mut crate::browser::ChromeBrowser,
    cfg: &Config,
) -> Result<(), CliError> {
    use std::time::Duration;

    let url = crate::search::build_search_url(
        cfg.query.as_str(),
        cfg.language.as_str(),
        cfg.country.as_str(),
        crate::types::Endpoint::Html,
        cfg.time_filter,
        cfg.safe_search,
    );
    let extract_timeout = Duration::from_secs(cfg.timeout_seconds.get().min(20));
    match crate::browser::extract_html_with_chrome(browser, &url, 256 * 1024, extract_timeout)
        .await
    {
        Ok(body) => {
            tracing::info!(
                body_len = body.len(),
                "news session primed via web SERP (GAP-E2E-51-006 parity with --vertical all)"
            );
            Ok(())
        }
        Err(err) => {
            // Best-effort: still attempt the news SERP on a cold-ish session.
            tracing::warn!(
                error = %err,
                "news session prime via web SERP failed — continuing to news SERP"
            );
            Ok(())
        }
    }
}

/// True when an empty news extract should be retried (transient block /
/// incomplete hydration) rather than accepted as a legitimate zero.
///
/// NEVER treats empty as success — only decides whether another attempt is
/// warranted. Final empty+interstitial still classifies as `causa_zero=anti-bot`
/// (exit 6) upstream.
#[cfg(feature = "chrome")]
pub(super) fn news_empty_is_retryable(results: &[crate::types::NewsResult], html: &str) -> bool {
    if !results.is_empty() {
        return false;
    }
    // Terminal empty state from DDG news vertical — legitimate zero, do not retry.
    if html.contains("no-results-message") || html.contains("data-testid=\"no-results-message\"") {
        return false;
    }
    if crate::probe_deep::detect_interstitial(html) != crate::probe_deep::InterstitialKind::None {
        return true;
    }
    // Premature extract: body present but no news shell / terminal empty state.
    let has_news_shell = html.contains("news-vertical")
        || html.contains("data-react-module-id=\"news\"")
        || html.contains("no-results-message");
    !html.is_empty() && !has_news_shell
}

/// Single news SERP navigation + extract + parse (no retry).
#[cfg(feature = "chrome")]
async fn extract_news_once_on_browser(
    browser: &mut crate::browser::ChromeBrowser,
    cfg: &Config,
) -> Result<(Vec<crate::types::NewsResult>, String, u32), CliError> {
    use std::time::Duration;

    let url = crate::search::build_news_search_url(
        cfg.query.as_str(),
        cfg.language.as_str(),
        cfg.country.as_str(),
        cfg.time_filter,
        cfg.safe_search,
    );
    let extract_timeout = Duration::from_secs(cfg.timeout_seconds.get().min(20));
    let html = crate::browser::extract_news_html_with_chrome(
        browser,
        &url,
        &cfg.selectors.news.container,
        Duration::from_millis(250),
        1024 * 1024,
        extract_timeout,
        cfg.dump_news_html.as_deref(),
    )
    .await
    .map_err(|e| {
        crate::error::normalize_chrome_transport_error(match e {
            CliError::ChromeUnavailable { .. }
            | CliError::ChromeNotFound { .. }
            | CliError::ChromeDisabledByEnv => e,
            other => CliError::chrome_unavailable(format!(
                "Chrome news HTML extraction failed: {other}"
            )),
        })
    })?;

    // GAP-PAR-030: news SERP parse off Tokio worker.
    let (results, promo_filtered) = crate::extraction::extract_news_results_with_stats_async(
        html.clone(),
        (*cfg.selectors).clone(),
    )
    .await?;
    Ok((results, html, promo_filtered))
}

/// Runs the news-vertical search on an ALREADY-launched Chrome session.
/// GAP-WS-104 v0.8.9 + GAP-E2E-51-006.
///
/// Used by `--vertical all` to reuse the same browser (single GAP-WS-077
/// warm-up) after the web SERP navigation. Navigates to the
/// `ia=news&iar=news` SERP built by [`crate::search::build_news_search_url`],
/// polls the React news module (`cfg.selectors.news.container`, 250ms
/// interval) and extracts via the A→B cascade with a 1 MiB cap — the
/// hydrated news SERP is far heavier than the 256 KiB web SERP.
///
/// When `prime_session` is true (news-only / dual news Chrome / shared path
/// without a prior successful web SERP), performs a web-SERP prime first for
/// session parity with shared-session `--vertical all`.
///
/// Empty interstitial / incomplete DOM outcomes honor `--retries` with
/// full-jitter backoff ([`crate::retry::RetryConfig`]). Exhausted retries
/// return the last honest empty body — never synthetic articles.
///
/// Returns the extracted news results plus the raw rendered HTML body
/// (used upstream for zero-cause classification).
///
/// # Errors
///
/// Returns `CliError` when navigation or extraction fails or times out
/// after the configured retry budget is exhausted.
#[cfg(feature = "chrome")]
pub async fn execute_chrome_news_search_on_browser(
    browser: &mut crate::browser::ChromeBrowser,
    cfg: &Config,
) -> Result<(Vec<crate::types::NewsResult>, String, u32), CliError> {
    // Default: prime when this browser never ran web (news-only / dual news).
    // Shared-session callers that already navigated web override via
    // [`execute_chrome_news_search_on_browser_with_prime`].
    execute_chrome_news_search_on_browser_with_prime(browser, cfg, !cfg.vertical.includes_web())
        .await
}

/// Like [`execute_chrome_news_search_on_browser`] with explicit session-prime control.
#[cfg(feature = "chrome")]
pub async fn execute_chrome_news_search_on_browser_with_prime(
    browser: &mut crate::browser::ChromeBrowser,
    cfg: &Config,
    prime_session: bool,
) -> Result<(Vec<crate::types::NewsResult>, String, u32), CliError> {
    use crate::retry::{deadline_exceeded, sleep_until_deadline, RetryConfig};

    let policy = RetryConfig::from_retries(cfg.retries.get());
    let total_attempts = policy.total_attempts();
    let deadline = policy.deadline();
    let mut last_ok: Option<(Vec<crate::types::NewsResult>, String, u32)> = None;
    let mut last_err: Option<CliError> = None;

    for attempt in 0..total_attempts {
        if deadline_exceeded(deadline) {
            tracing::warn!(
                attempt = attempt + 1,
                "news vertical retry max_elapsed exhausted (GAP-E2E-51-006)"
            );
            break;
        }

        // Re-prime on every attempt when requested: fresh cookies after a
        // blocked news extract improve odds without fabricating results.
        if prime_session {
            let _ = prime_news_session_with_web_serp(browser, cfg).await;
        }

        match extract_news_once_on_browser(browser, cfg).await {
            Ok((results, html, promo_filtered)) => {
                if !results.is_empty() {
                    if attempt > 0 {
                        tracing::info!(
                            attempt = attempt + 1,
                            news_results = results.len(),
                            "news vertical recovered after retry (GAP-E2E-51-006)"
                        );
                    }
                    return Ok((results, html, promo_filtered));
                }
                if !news_empty_is_retryable(&results, &html) {
                    // Legitimate zero (rendered shell / no interstitial).
                    return Ok((results, html, promo_filtered));
                }
                tracing::warn!(
                    attempt = attempt + 1,
                    total = total_attempts,
                    body_len = html.len(),
                    "news empty/interstitial — retry with backoff (GAP-E2E-51-006); \
                     never fake-success empty"
                );
                last_ok = Some((results, html, promo_filtered));
                last_err = None;
                if attempt + 1 >= total_attempts {
                    break;
                }
                let delay_ms = policy.backoff_ms(attempt);
                if !sleep_until_deadline(delay_ms, deadline).await {
                    break;
                }
            }
            Err(err) if matches!(err, CliError::Cancelled) => return Err(err),
            Err(err) => {
                tracing::warn!(
                    attempt = attempt + 1,
                    total = total_attempts,
                    error = %err,
                    "news extract error — retry with backoff (GAP-E2E-51-006)"
                );
                last_err = Some(err);
                if attempt + 1 >= total_attempts {
                    break;
                }
                let delay_ms = policy.backoff_ms(attempt);
                if !sleep_until_deadline(delay_ms, deadline).await {
                    break;
                }
            }
        }
    }

    if let Some(ok) = last_ok {
        // Honest empty after budget: upstream classifies anti-bot → exit 6.
        return Ok(ok);
    }
    Err(last_err.unwrap_or_else(|| {
        CliError::chrome_unavailable("Chrome news HTML extraction failed after retries")
    }))
}

/// Standalone news-vertical search: launches Chrome, delegates to
/// [`execute_chrome_news_search_on_browser`] (with session prime) and shuts
/// the browser down. Used by `--vertical news` and dual-process news Chrome.
/// GAP-WS-104 v0.8.9 + GAP-E2E-51-006.
///
/// # Errors
///
/// Returns `CliError` when Chrome is unavailable, launch times out, or
/// news extraction fails after the retry budget.
#[cfg(feature = "chrome")]
pub async fn execute_chrome_news_search(
    cfg: &Config,
    user_agent: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<(Vec<crate::types::NewsResult>, String, u32), CliError> {
    use crate::retry::{deadline_exceeded, sleep_until_deadline, RetryConfig};

    // Outer loop can re-launch Chrome when interstitial persists on one profile.
    let policy = RetryConfig::from_retries(cfg.retries.get());
    let total_attempts = policy.total_attempts();
    let deadline = policy.deadline();
    let mut last_ok: Option<(Vec<crate::types::NewsResult>, String, u32)> = None;
    let mut last_err: Option<CliError> = None;

    for attempt in 0..total_attempts {
        if cancellation.is_cancelled() {
            return Err(chrome_cancelled_error("news search"));
        }
        if deadline_exceeded(deadline) {
            break;
        }

        let launched = tokio::select! {
            launched = launch_chrome_browser(cfg, user_agent) => launched,
            _ = cancellation.cancelled() => return Err(chrome_cancelled_error("launch")),
        };
        let mut browser = match launched {
            Ok(b) => b,
            Err(err) => {
                last_err = Some(err);
                if attempt + 1 >= total_attempts {
                    break;
                }
                let delay_ms = policy.backoff_ms(attempt);
                if !sleep_until_deadline(delay_ms, deadline).await {
                    break;
                }
                continue;
            }
        };

        // One extract+inner-retry budget per browser would double-count retries.
        // Here: prime + single extract per launch; outer loop owns the budget.
        let selected = tokio::select! {
            result = async {
                let _ = prime_news_session_with_web_serp(&mut browser, cfg).await;
                extract_news_once_on_browser(&mut browser, cfg).await
            } => Some(result),
            _ = cancellation.cancelled() => None,
        };
        if let Err(err) = browser.shutdown().await {
            tracing::debug!(?err, "chrome shutdown after news attempt (best-effort)");
        }
        let Some(result) = selected else {
            return Err(chrome_cancelled_error("news search"));
        };

        match result {
            Ok((results, html, promo_filtered)) => {
                if !results.is_empty() {
                    if attempt > 0 {
                        tracing::info!(
                            attempt = attempt + 1,
                            news_results = results.len(),
                            "news-only recovered after Chrome re-launch (GAP-E2E-51-006)"
                        );
                    }
                    return Ok((results, html, promo_filtered));
                }
                if !news_empty_is_retryable(&results, &html) {
                    return Ok((results, html, promo_filtered));
                }
                tracing::warn!(
                    attempt = attempt + 1,
                    total = total_attempts,
                    "news-only empty/interstitial — re-launch with backoff (GAP-E2E-51-006)"
                );
                last_ok = Some((results, html, promo_filtered));
                last_err = None;
                if attempt + 1 >= total_attempts {
                    break;
                }
                let delay_ms = policy.backoff_ms(attempt);
                if !sleep_until_deadline(delay_ms, deadline).await {
                    break;
                }
            }
            Err(err) if matches!(err, CliError::Cancelled) => return Err(err),
            Err(err) => {
                last_err = Some(err);
                if attempt + 1 >= total_attempts {
                    break;
                }
                let delay_ms = policy.backoff_ms(attempt);
                if !sleep_until_deadline(delay_ms, deadline).await {
                    break;
                }
            }
        }
    }

    if let Some(ok) = last_ok {
        return Ok(ok);
    }
    Err(last_err.unwrap_or_else(|| chrome_cancelled_error("news search")))
}
