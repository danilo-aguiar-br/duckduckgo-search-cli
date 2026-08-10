// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: orchestrator (Chrome SERP transport)
//! Chrome/CDP search transport extracted from `pipeline` (GAP-COMP-007).

use super::failure::chrome_cancelled_error;
use crate::error::CliError;
use crate::types::{Config, SearchMetadata};

/// v0.8.0 Chrome-primary search path.
///
/// Launches headless Chrome via `src/browser.rs::ChromeBrowser`, navigates to the
/// `DuckDuckGo` HTML endpoint for the query, extracts the full HTML and parses
/// results using the same selectors as the reqwest path.
///
/// Returns `Err(CliError)` if Chrome is not installed, if Chrome launch times out,
/// or if the page extraction fails.
#[cfg(feature = "chrome")]
pub(crate) async fn execute_chrome_search(
    cfg: &Config,
    user_agent: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<crate::search::AggregatedSearchResult, CliError> {
    // GAP F5 v0.8.9: launch and navigation respect the cancellation token via
    // `tokio::select!` — same cancellation error as the reqwest path.
    let launched = tokio::select! {
        launched = launch_chrome_browser(cfg, user_agent) => launched,
        _ = cancellation.cancelled() => return Err(chrome_cancelled_error("launch")),
    };
    let mut browser = launched?;
    // The intermediate `Option` releases the `browser` borrow before
    // shutdown, executed in BOTH branches (completed and cancelled).
    let selected = tokio::select! {
        result = execute_chrome_web_search_on_browser(&mut browser, cfg) => Some(result),
        _ = cancellation.cancelled() => None,
    };
    // Best-effort cleanup: never mask the primary search result/error with shutdown noise.
    if let Err(err) = browser.shutdown().await {
        tracing::debug!(?err, "chrome shutdown after web search (best-effort)");
    }
    selected.unwrap_or_else(|| Err(chrome_cancelled_error("web search")))
}

/// Runs the web-vertical SERP navigation + extraction on an ALREADY-launched
/// Chrome session.
///
/// GAP-WS-104 v0.8.9: extracted from [`execute_chrome_search`] so that
/// `--vertical all` shares the SAME Chrome session (GAP-WS-077 warm-up
/// single) between web and news SERPs.
///
/// # Errors
///
/// Returns `CliError` when navigation or page extraction fails or times out.
#[cfg(feature = "chrome")]
async fn execute_chrome_web_search_on_browser(
    browser: &mut crate::browser::ChromeBrowser,
    cfg: &Config,
) -> Result<crate::search::AggregatedSearchResult, CliError> {
    use std::time::Duration;

    // GAP-WS-113: Chrome SERP always uses HTML canonical endpoint — never Lite.
    // Same browser session: optional pre-flight calibration navigation first.
    let extract_timeout = Duration::from_secs(cfg.timeout_seconds.get().min(20));
    if cfg.pre_flight && cfg.vertical.includes_web() {
        let calib = "the quick brown fox jumps over the lazy dog";
        let calib_url = crate::search::build_search_url(
            calib,
            cfg.language.as_str(),
            cfg.country.as_str(),
            crate::types::Endpoint::Html,
            cfg.time_filter,
            cfg.safe_search,
        );
        match crate::browser::extract_html_with_chrome(
            browser,
            &calib_url,
            512 * 1024,
            extract_timeout,
        )
        .await
        {
            Ok(body) => {
                let outcome = crate::probe_deep::classify_probe_outcome(&body, 200, 0);
                if !outcome.healthy {
                    return Err(CliError::Blocked);
                }
                tracing::info!(
                    body_len = body.len(),
                    "pre-flight on shared Chrome session healthy"
                );
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "pre-flight on shared Chrome session failed — continuing to real query"
                );
            }
        }
    }

    let url = crate::search::build_search_url(
        cfg.query.as_str(),
        cfg.language.as_str(),
        cfg.country.as_str(),
        crate::types::Endpoint::Html,
        cfg.time_filter,
        cfg.safe_search,
    );

    let html = crate::browser::extract_html_with_chrome(browser, &url, 256 * 1024, extract_timeout)
        .await
        .map_err(|e| {
            // GAP-E2E-V11-EXIT-TAXONOMY: transport failures stay chrome_unavailable (exit 2).
            crate::error::normalize_chrome_transport_error(match e {
                CliError::ChromeUnavailable { .. }
                | CliError::ChromeNotFound { .. }
                | CliError::ChromeDisabledByEnv => e,
                other => {
                    CliError::chrome_unavailable(format!("Chrome HTML extraction failed: {other}"))
                }
            })
        })?;

    // GAP-PAR-030: SERP parse off Tokio worker (spawn_blocking + CPU semaphore).
    let results = crate::extraction::extract_results_with_strategies_cfg_async(
        html.clone(),
        (*cfg.selectors).clone(),
    )
    .await?;

    Ok(crate::search::AggregatedSearchResult {
        results,
        first_body: html.clone(),
        pages_fetched: 1,
        attempts: 1,
        used_fallback_lite: false,
        effective_endpoint: crate::types::Endpoint::Html,
        bytes_in: html.len() as u64,
        bytes_out: html.len() as u64,
    })
}

/// GAP-WS-095 + GAP-WS-104 + GAP-E2E-V14: stable tag for the Chrome identity
/// actually used on this host. Exact pool UA equality fails after major rewrite
/// — always return the host Chrome pool tag (anti-CF #4 honesty).
///
/// Kept for fan-out / parallel call sites that still pass a UA string.
#[cfg(feature = "chrome")]
#[allow(dead_code)] // used by dual-path helpers / tests; SSOT now prefers chrome_identity_tag_for_host
pub(crate) fn identity_tag_for_chrome_ua(_chrome_ua: &str) -> Option<String> {
    Some(crate::identity::chrome_identity_tag_for_host())
}

/// Public wrapper for `execute_chrome_search` used by the parallel executor.
///
/// # Errors
///
/// Returns `CliError` when Chrome is unavailable or page extraction fails.
#[cfg(feature = "chrome")]
pub async fn execute_chrome_search_pub(
    cfg: &Config,
    user_agent: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<crate::search::AggregatedSearchResult, CliError> {
    execute_chrome_search(cfg, user_agent, cancellation).await
}

/// Outcome of a `--vertical news|all` Chrome session (GAP-WS-105 v0.8.9).
///
/// Produced by [`execute_chrome_all_search_pub`]:
/// - **Dual multi-process** (default when `--parallel ≥ 2`): two OS Chromes
///   run web and news SERPs in parallel (GAP-PAR-021; tokio docs: spawn for
///   real parallelism, not join! on one task).
/// - **Shared session**: one Chrome, web then news serial (opt-in via
///   `--shared-session-verticals` or budget &lt; 2).
///
/// Consumed by single-query pipeline and multi-query fan-out (`parallel.rs`).
#[cfg(feature = "chrome")]
#[derive(Debug)]
pub struct ChromeAllSearchOutcome {
    /// Web SERP result — `Some` only when the vertical includes web AND the
    /// Chrome web navigation succeeded. `None` ⇒ the caller decides the
    /// fallback (in `all` mode both the pipeline and the fan-out degrade the
    /// web half to the HTTP path).
    pub web: Option<crate::search::AggregatedSearchResult>,
    /// News outcome — `Ok((results, rendered body))` when the news SERP ran,
    /// even with zero results. `Err` when the Chrome launch or the news
    /// navigation failed (news is Chrome-only, with no HTTP fallback): the
    /// news-only pipeline emits a failure envelope, `all` degrades to empty
    /// news, and the fan-out reports `news` as absent.
    /// `Ok((results, body, promo_filtered))` — third field is the count of
    /// DDG promo/chrome links stripped (GAP-WS-NEWS-LIVE-001 v0.9.9).
    pub news: Result<(Vec<crate::types::NewsResult>, String, u32), CliError>,
}

/// GAP-WS-105 / GAP-PAR-021: orchestration of `--vertical news|all`.
///
/// Default multi-process dual path (web ∥ news in two Chromes) when
/// [`crate::concurrency::prefer_dual_vertical_chrome`] is true. Shared
/// serial session is the fallback (budget &lt; 2 or `--shared-session-verticals`).
///
/// Cancel: child tokens + JoinSet abort on parent cancel. JoinError panic/
/// cancel distinguished on dual drain.
///
/// # Errors
///
/// Returns `Err` only for cooperative cancellation. Launch/navigation failures
/// are reported inside [`ChromeAllSearchOutcome`].
#[cfg(feature = "chrome")]
pub async fn execute_chrome_all_search_pub(
    cfg: &Config,
    user_agent: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<ChromeAllSearchOutcome, CliError> {
    let dual = crate::concurrency::prefer_dual_vertical_chrome(
        cfg.parallelism.get(),
        crate::concurrency::DualVerticalMode::Auto,
        cfg.shared_session_verticals,
    ) && cfg.vertical.includes_web()
        && cfg.vertical.includes_news();

    tracing::info!(
        dual_chrome = dual,
        chrome_slots = crate::concurrency::chrome_slots_per_query(
            cfg.vertical.includes_web(),
            cfg.vertical.includes_news(),
            dual,
        ),
        parallelism = cfg.parallelism.get(),
        shared_session_verticals = cfg.shared_session_verticals,
        "Chrome vertical orchestration mode (GAP-PAR-021)"
    );

    if dual {
        execute_chrome_all_search_dual(cfg, user_agent, cancellation).await
    } else if cfg.vertical.includes_news() && !cfg.vertical.includes_web() {
        // GAP-E2E-51-006: news-only uses the dedicated launch path (web SERP
        // prime + Chrome re-launch retries) — same transport as dual's news
        // process, not the shared serial shell which skips web entirely.
        let news = execute_chrome_news_search(cfg, user_agent, cancellation).await;
        Ok(ChromeAllSearchOutcome { web: None, news })
    } else {
        execute_chrome_all_search_shared(cfg, user_agent, cancellation).await
    }
}

/// Dual multi-process: two independent Chrome OS processes for web and news
/// (GAP-PAR-021). Tokio docs: real parallelism requires `spawn` / JoinSet, not
/// `join!` on a single task with one `&mut browser`.
#[cfg(feature = "chrome")]
#[tracing::instrument(
    level = "info",
    skip_all,
    fields(
        dual_chrome = true,
        query = %cfg.query,
        parallelism = cfg.parallelism.get(),
        chrome_slots = 2u32,
    )
)]
async fn execute_chrome_all_search_dual(
    cfg: &Config,
    user_agent: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<ChromeAllSearchOutcome, CliError> {
    use crate::types::VerticalMode;
    use tokio::task::JoinSet;

    let mut web_cfg = cfg.clone();
    web_cfg.vertical = VerticalMode::Web;
    let mut news_cfg = cfg.clone();
    news_cfg.vertical = VerticalMode::News;

    let ua = user_agent.to_string();
    let cancel_web = cancellation.child_token();
    let cancel_news = cancellation.child_token();

    let mut set: JoinSet<(bool, Result<DualVerticalPiece, CliError>)> = JoinSet::new();

    {
        let ua = ua.clone();
        let cancel = cancel_web.clone();
        set.spawn(async move {
            let result = execute_chrome_search(&web_cfg, &ua, &cancel).await;
            (true, result.map(DualVerticalPiece::Web))
        });
    }
    {
        let ua = ua.clone();
        let cancel = cancel_news.clone();
        set.spawn(async move {
            let result = execute_chrome_news_search(&news_cfg, &ua, &cancel).await;
            (false, result.map(DualVerticalPiece::News))
        });
    }

    type NewsVerticalResult = Result<(Vec<crate::types::NewsResult>, String, u32), CliError>;
    let mut web: Option<crate::search::AggregatedSearchResult> = None;
    let mut news: Option<NewsVerticalResult> = None;

    while let Some(joined) = set.join_next().await {
        if cancellation.is_cancelled() {
            set.abort_all();
            while set.join_next().await.is_some() {}
            return Err(chrome_cancelled_error("dual vertical"));
        }
        match joined {
            Ok((is_web, Ok(DualVerticalPiece::Web(r)))) if is_web => {
                tracing::info!(
                    chrome_results = r.results.len(),
                    dual_chrome = true,
                    "Chrome dual web SERP succeeded"
                );
                web = Some(r);
            }
            Ok((is_web, Ok(DualVerticalPiece::News(n)))) if !is_web => {
                tracing::info!(
                    news_results = n.0.len(),
                    dual_chrome = true,
                    "Chrome dual news SERP succeeded"
                );
                news = Some(Ok(n));
            }
            Ok((is_web, Ok(_))) => {
                // Mismatched tag — treat as internal error (should not happen).
                tracing::error!(is_web, "dual vertical piece tag mismatch");
            }
            Ok((true, Err(err))) => {
                if matches!(err, CliError::Cancelled) {
                    set.abort_all();
                    while set.join_next().await.is_some() {}
                    return Err(err);
                }
                tracing::error!(
                    error = %err,
                    dual_chrome = true,
                    "Chrome dual web SERP failed — no HTTP fallback (GAP-WS-113)"
                );
                web = None;
            }
            Ok((false, Err(err))) => {
                if matches!(err, CliError::Cancelled) {
                    set.abort_all();
                    while set.join_next().await.is_some() {}
                    return Err(err);
                }
                tracing::warn!(
                    error = %err,
                    dual_chrome = true,
                    "Chrome dual news SERP failed — news vertical will be empty"
                );
                news = Some(Err(err));
            }
            Err(join_err) if join_err.is_cancelled() => {
                set.abort_all();
                while set.join_next().await.is_some() {}
                return Err(chrome_cancelled_error("dual vertical join cancel"));
            }
            Err(join_err) if join_err.is_panic() => {
                tracing::error!(dual_chrome = true, "dual vertical task panicked");
                set.abort_all();
                while set.join_next().await.is_some() {}
                return Err(CliError::NetworkError {
                    message: "dual vertical Chrome task panicked".into(),
                });
            }
            Err(join_err) => {
                tracing::error!(?join_err, dual_chrome = true, "dual vertical JoinError");
                set.abort_all();
                while set.join_next().await.is_some() {}
                return Err(CliError::NetworkError {
                    message: format!("dual vertical join failed: {join_err}"),
                });
            }
        }
    }

    Ok(ChromeAllSearchOutcome {
        web,
        news: news.unwrap_or_else(|| {
            Err(CliError::NetworkError {
                message: "dual news vertical produced no outcome".into(),
            })
        }),
    })
}

/// Internal dual-path piece tag (web vs news payload).
#[cfg(feature = "chrome")]
enum DualVerticalPiece {
    Web(crate::search::AggregatedSearchResult),
    News((Vec<crate::types::NewsResult>, String, u32)),
}

/// Shared single-Chrome session: web then news serial (GAP-WS-104 fallback).
#[cfg(feature = "chrome")]
async fn execute_chrome_all_search_shared(
    cfg: &Config,
    user_agent: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<ChromeAllSearchOutcome, CliError> {
    let launched = tokio::select! {
        launched = launch_chrome_browser(cfg, user_agent) => launched,
        _ = cancellation.cancelled() => return Err(chrome_cancelled_error("launch")),
    };
    let mut browser = match launched {
        Ok(browser) => browser,
        Err(err) => {
            if cfg.vertical.includes_web() {
                tracing::error!(
                    error = %err,
                    "Chrome launch failed — no HTTP fallback (GAP-WS-113)"
                );
            }
            return Ok(ChromeAllSearchOutcome {
                web: None,
                news: Err(err),
            });
        }
    };

    let mut web = None;
    if cfg.vertical.includes_web() {
        let web_result = tokio::select! {
            result = execute_chrome_web_search_on_browser(&mut browser, cfg) => Some(result),
            _ = cancellation.cancelled() => None,
        };
        let Some(web_result) = web_result else {
            // Best-effort cleanup: do not mask the primary error/result with shutdown failure.
            if let Err(err) = browser.shutdown().await {
                tracing::debug!(?err, "chrome shutdown (best-effort)");
            }
            return Err(chrome_cancelled_error("web search"));
        };
        match web_result {
            Ok(result) => {
                tracing::info!(
                    chrome_results = result.results.len(),
                    dual_chrome = false,
                    "Chrome-primary search succeeded (shared session)"
                );
                web = Some(result);
            }
            Err(err) => {
                tracing::error!(
                    error = %err,
                    "Chrome-primary web search failed — no HTTP fallback (GAP-WS-113)"
                );
                let _ = err;
            }
        }
    }

    // GAP-E2E-51-006: prime only when this browser did not already run a
    // successful web SERP (news-only, or all-mode web failure). Shared-session
    // all with web>0 already has cookies — skip the extra HTML navigation.
    let prime_session = web.is_none();
    let news_result = tokio::select! {
        result = execute_chrome_news_search_on_browser_with_prime(
            &mut browser,
            cfg,
            prime_session,
        ) => Some(result),
        _ = cancellation.cancelled() => None,
    };
    let Some(news_result) = news_result else {
        // Best-effort cleanup: do not mask the primary error/result with shutdown failure.
        if let Err(err) = browser.shutdown().await {
            tracing::debug!(?err, "chrome shutdown (best-effort)");
        }
        return Err(chrome_cancelled_error("news search"));
    };
    // Best-effort cleanup: do not mask the primary error/result with shutdown failure.
    if let Err(err) = browser.shutdown().await {
        tracing::debug!(?err, "chrome shutdown (best-effort)");
    }
    if let Err(ref err) = news_result {
        if cfg.vertical.includes_web() {
            tracing::warn!(
                error = %err,
                "News vertical failed in all mode — news vertical will be empty"
            );
        }
    }
    Ok(ChromeAllSearchOutcome {
        web,
        news: news_result,
    })
}

/// Shared Chrome launch path for the web and news verticals (GAP-WS-104).
///
/// Detects the Chrome binary (flag → env → auto-detection) and launches the
/// stealth browser with the caller-provided user agent.
#[cfg(feature = "chrome")]
pub(super) async fn launch_chrome_browser(
    cfg: &Config,
    user_agent: &str,
) -> Result<crate::browser::ChromeBrowser, CliError> {
    use crate::browser::{detect_chrome_resolved, ChromeBrowser};
    use crate::error::{
        chrome_session_retries, is_chrome_session_transient, normalize_chrome_transport_error,
    };
    use crate::retry::RetryConfig;
    use std::time::Duration;

    let resolved = detect_chrome_resolved(cfg.chrome_path.as_deref()).map_err(|e| {
        // GAP-E2E-V11-EXIT-TAXONOMY: missing binary is chrome_not_found, not invalid_config.
        CliError::chrome_not_found(format!("Chrome not detected: {e}"))
    })?;
    tracing::info!(
        path = %resolved.path.display(),
        canal = resolved.channel.as_str(),
        "launching Chrome (multi-canal resolve)"
    );
    let launch_timeout = Duration::from_secs(cfg.timeout_seconds.get().min(15));
    let proxy_url = match &cfg.proxy_config {
        crate::http::ProxyConfig::Url(u) => Some(u.as_str()),
        _ => None,
    };

    // Anti-CF / GAP-WS-074: never pair Safari/Firefox UA with Chromium TLS.
    let chrome_ua = crate::identity::coerce_chrome_user_agent(user_agent);

    // GAP-E2E-V11-CHROME-FLAKY: full session rebuild retry on transient launch/WS.
    // Each attempt creates a fresh TempDir + Xvfb + CDP pipe (one-shot safe).
    let retry_policy = RetryConfig::from_retries(chrome_session_retries());
    let total = retry_policy.total_attempts().max(1);
    let mut last_err: Option<CliError> = None;
    for attempt in 0..total {
        match ChromeBrowser::launch(&resolved.path, proxy_url, launch_timeout, &chrome_ua).await {
            Ok(browser) => {
                if attempt > 0 {
                    tracing::info!(
                        attempt,
                        total,
                        "Chrome session launch succeeded after retry"
                    );
                }
                return Ok(browser);
            }
            Err(err) => {
                let err = normalize_chrome_transport_error(err);
                tracing::warn!(
                    attempt,
                    total,
                    error = %err,
                    "Chrome session launch failed"
                );
                let transient = is_chrome_session_transient(&err);
                last_err = Some(err);
                if !transient || attempt + 1 >= total {
                    break;
                }
                let delay_ms = retry_policy.backoff_ms(attempt);
                tracing::info!(delay_ms, attempt, "backing off before Chrome session retry");
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        CliError::chrome_unavailable("chrome session launch failed with no error detail")
    }))
}

/// Best-effort chrome path/channel for agent metadata (no spawn).
///
/// Agent contract fields (`chrome_path_resolvido` / `chrome_canal`) — agent contract field.
#[cfg(feature = "chrome")]
pub(crate) fn resolved_chrome_metadata(cfg: &Config) -> (Option<String>, Option<String>) {
    match crate::browser::detect_chrome_resolved(cfg.chrome_path.as_deref()) {
        Ok(r) => (
            Some(r.path.display().to_string()),
            Some(r.channel.as_str().to_string()),
        ),
        Err(_) => (None, None),
    }
}

/// Fills `chrome_path_resolvido` + `chrome_canal` on any envelope (success or failure).
///
/// GAP-WS-AGENT-READY-001 residual R-01/R-03: single-path, fan-out, and failure
/// helpers must not leave these fields null when Chrome is detectable.
pub(crate) fn fill_chrome_agent_metadata(meta: &mut SearchMetadata, cfg: &Config) {
    // GAP-WS-META-NO-CHROME-001: never claim path/canal when policy forbids Chrome.
    if crate::chrome_policy::chrome_disabled_by_env()
        && !crate::chrome_policy::http_test_harness_active()
    {
        meta.chrome_path_resolved = None;
        meta.chrome_channel = None;
        meta.chrome_attempted = false;
        meta.used_chrome = false;
        return;
    }
    #[cfg(feature = "chrome")]
    {
        let (path, canal) = resolved_chrome_metadata(cfg);
        meta.chrome_path_resolved = path;
        meta.chrome_channel = canal;
    }
    #[cfg(not(feature = "chrome"))]
    {
        let _ = (meta, cfg);
    }
}

mod news;
pub use news::{
    execute_chrome_news_search, execute_chrome_news_search_on_browser,
    execute_chrome_news_search_on_browser_with_prime,
};

/// GAP F2 v0.8.9: pre-flight only applies when the web vertical participates
/// in execution — news vertical is Chrome-only (no HTTP endpoint to probe)
/// and a probe false positive would abort the news search without ever trying it.
pub(crate) fn pre_flight_applies(cfg: &Config) -> bool {
    cfg.pre_flight && cfg.vertical.includes_web()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
