// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: orchestrator (Chrome SERP transport)
//! Chrome/CDP search transport extracted from `pipeline` (GAP-COMP-007).

use crate::error::CliError;
use super::failure::chrome_cancelled_error;
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
    // `tokio::select!` — mesmo erro de cancelamento do caminho reqwest.
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
        .map_err(|e| CliError::InvalidConfig {
            message: format!("Chrome HTML extraction failed: {e}"),
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

/// GAP-WS-095 + GAP-WS-104: resolve a tag de identidade correspondente ao UA
/// Chrome efetivamente usado (pool Auto). Compartilhada pelos caminhos web e
/// news|all do bloco Chrome-primary.
#[cfg(feature = "chrome")]
pub(crate) fn identity_tag_for_chrome_ua(chrome_ua: &str) -> Option<String> {
    let pool = crate::identity::IdentityPool::new(None);
    pool.iter()
        .find(|p| p.user_agent == chrome_ua)
        .map(|matched| matched.tag())
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
    /// Web SERP result — `Some` apenas quando a vertical inclui web E a
    /// Chrome web navigation succeeded. `None` ⇒ the caller decides the
    /// fallback (pipeline e fan-out degradam a web para reqwest no modo all).
    pub web: Option<crate::search::AggregatedSearchResult>,
    /// News outcome — `Ok((resultados, body renderizado))` quando a SERP
    /// news executou (mesmo com zero resultados). `Err` quando o launch do
    /// Chrome or news navigation failed (news is Chrome-only, without fallback
    /// HTTP): o pipeline news-only emite envelope de falha, o modo all
    /// degrada para news vazia e o fan-out sinaliza `noticias` ausente.
    /// `Ok((resultados, body, promo_filtradas))` — third field is count of
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
        Ok(ChromeAllSearchOutcome {
            web: None,
            news,
        })
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
async fn launch_chrome_browser(
    cfg: &Config,
    user_agent: &str,
) -> Result<crate::browser::ChromeBrowser, CliError> {
    use crate::browser::{detect_chrome_resolved, ChromeBrowser};
    use std::time::Duration;

    let resolved = detect_chrome_resolved(cfg.chrome_path.as_deref()).map_err(|e| {
        CliError::InvalidConfig {
            message: format!("Chrome not detected: {e}"),
        }
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
    ChromeBrowser::launch(
        &resolved.path,
        proxy_url,
        launch_timeout,
        user_agent,
    )
    .await
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

/// Best-effort web-SERP session prime on an already-launched Chrome.
///
/// GAP-E2E-51-006: shared-session `--vertical all` navigates the HTML web SERP
/// before news, establishing DDG cookies/session signals. Cold news-only (and
/// dual-process news Chrome) previously skipped that step and hit interstitials
/// more often. This prime mirrors the shared path without claiming fake results.
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
fn news_empty_is_retryable(results: &[crate::types::NewsResult], html: &str) -> bool {
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
    .map_err(|e| CliError::InvalidConfig {
        message: format!("Chrome news HTML extraction failed: {e}"),
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
    Err(last_err.unwrap_or_else(|| CliError::InvalidConfig {
        message: "Chrome news HTML extraction failed after retries".into(),
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

/// GAP F2 v0.8.9: pre-flight only applies when the web vertical participates
/// in execution — news vertical is Chrome-only (no HTTP endpoint to probe)
/// and a probe false positive would abort the news search without ever trying it.
pub(crate) fn pre_flight_applies(cfg: &Config) -> bool {
    cfg.pre_flight && cfg.vertical.includes_web()
}

#[cfg(all(test, feature = "chrome"))]
mod tests {
    use super::news_empty_is_retryable;
    use crate::types::NewsResult;

    fn sample_news() -> NewsResult {
        NewsResult {
            position: 1,
            title: "t".into(),
            url: crate::types::HttpUrl::for_test("https://example.com/a"),
            source: None,
            relative_date: None,
            thumbnail: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        }
    }

    #[test]
    fn non_empty_news_is_not_retryable() {
        assert!(!news_empty_is_retryable(&[sample_news()], "<html></html>"));
    }

    #[test]
    fn interstitial_empty_is_retryable() {
        let html = r#"<html><body class="anomaly-modal__mask">Unfortunately, bots use DuckDuckGo too.</body></html>"#;
        assert!(news_empty_is_retryable(&[], html));
    }

    #[test]
    fn rendered_news_shell_empty_is_legitimate_not_retryable() {
        let html = r#"<div data-testid="news-vertical"><div data-testid="no-results-message">No results</div></div>"#;
        // Long enough to avoid ghost-block threshold when no interstitial markers.
        let html = format!("{html}{}", "x".repeat(5000));
        assert!(!news_empty_is_retryable(&[], &html));
    }

    #[test]
    fn incomplete_body_without_shell_is_retryable() {
        // Long body without news shell and without interstitial markers.
        let html = format!("<html><body>{}</body></html>", "partial".repeat(800));
        assert!(news_empty_is_retryable(&[], &html));
    }

    #[test]
    fn css_only_anomaly_modal_with_no_results_is_not_retryable() {
        // Live DDG news SERP embeds `.anomaly-modal__modal` CSS rules even when
        // the vertical honestly renders no-results-message (GAP-E2E-51-006).
        let html = format!(
            r#"<style>.anomaly-modal__modal {{ border: 1px solid #000; }}</style>
            <div data-testid="news-vertical">
              <section data-testid="no-results-message">Nenhum artigo</section>
            </div>{}"#,
            "x".repeat(5000)
        );
        assert!(!news_empty_is_retryable(&[], &html));
    }
}
