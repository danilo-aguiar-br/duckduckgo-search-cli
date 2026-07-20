// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: I/O-bound (page download) + multi-process Chrome CDP.
// Bottleneck: per-page navigation RTT (~300-2000ms).
// Saturated resource: Chrome OS processes + outbound connections + per-host limits.
// Parallelism:
// - global Semaphore (`--parallel` / `--max-concurrency`)
// - per-host Semaphore (stricter domain gate)
// - Chrome **pool** of independent OS processes (size = chrome_pool_size(...))
// - Nested under multi-query fan-out: pool_size = 1 (GAP-PAR-016 peak Chrome ≤ N).
// - Pool launch and shutdown use JoinSet (parallel multi-process setup/teardown).
//
// Lock / admit order (total order — never invert; GAP-PAR-018):
//   1. global fetch Semaphore (acquire_owned)
//   2. per-host Semaphore (acquire_owned)
//   3. pool free-list TokioMutex (pop/push only — no CDP under lock)
//! Parallel enrichment orchestration for `--fetch-content`.

use super::circuit::{BreakerDecision, CircuitBreakerMap};
use super::host::{extract_host, semaphore_for_host, PerHostSemaphoreMap};
use crate::content;
use crate::types::{Config, SearchOutput};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

#[cfg(feature = "chrome")]
use crate::browser::{detect_chrome, extract_text_with_chrome, ChromeBrowser, CONTENT_CHROME_TIMEOUT_SECS};
#[cfg(feature = "chrome")]
use tokio::sync::Mutex as TokioMutex;

/// Free-list of independent Chrome OS processes for multi-process content fetch.
///
/// Each admitted task pops a browser, navigates, then pushes it back. Pool size
/// equals the global admission gate so CDP work is concurrent across processes
/// (not serialised through a single `Mutex<ChromeBrowser>`).
#[cfg(feature = "chrome")]
type ChromeBrowserPool = Arc<TokioMutex<VecDeque<ChromeBrowser>>>;

/// Options for [`enrich_with_content_opts`] (nested fan-out vs standalone).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnrichOptions {
    /// Called from multi-query JoinSet tasks that already multi-process SERP.
    pub nested_in_query_fanout: bool,
}

/// Enriches search results with page content (`--fetch-content`).
///
/// Single-query convenience wrapper: full multi-process Chrome pool sized to
/// [`crate::concurrency::chrome_pool_size`]. For multi-query fan-out, use
/// [`enrich_with_content_opts`] with `nested_in_query_fanout: true`.
#[tracing::instrument(skip_all, fields(result_count = output.result_count, parallelism = config.parallelism.get()))]
pub async fn enrich_with_content(
    output: &mut SearchOutput,
    client: Option<&Client>,
    config: &Config,
    cancellation: &CancellationToken,
) {
    enrich_with_content_opts(output, client, config, cancellation, EnrichOptions::default()).await;
}

/// Enriches search results with page content, with explicit nesting options.
///
/// When `options.nested_in_query_fanout` is true, the Chrome pool is size 1 so
/// peak OS Chrome processes stay within `--parallel` across concurrent query
/// tasks (GAP-PAR-016).
#[tracing::instrument(skip_all, fields(
    result_count = output.result_count,
    parallelism = config.parallelism.get(),
    nested = options.nested_in_query_fanout
))]
pub async fn enrich_with_content_opts(
    output: &mut SearchOutput,
    client: Option<&Client>,
    config: &Config,
    cancellation: &CancellationToken,
    options: EnrichOptions,
) {
    // GAP-WS-113: fetch-content is Chrome-only in production.
    if config.fetch_content && !crate::chrome_policy::http_test_harness_active() {
        if let Err(err) = crate::chrome_policy::require_chrome_transport() {
            tracing::error!(error = %err, "fetch-content aborted — Chrome required (GAP-WS-113)");
            output.metadata.chrome_attempted = true;
            output.metadata.used_chrome = false;
            if output.error.is_none() {
                output.error = Some(err.error_code().to_string());
            }
            if output.message.is_none() {
                output.message = Some(format!("{err}"));
            }
            return;
        }
    }

    if !config.fetch_content {
        return;
    }
    // Agent-ready: fetch for web results and/or news URLs (v0.9.8 L-05).
    let has_web = !output.results.is_empty();
    let has_news = output.news.as_ref().map(|n| !n.is_empty()).unwrap_or(false);
    if !has_web && !has_news {
        return;
    }

    // GAP-SCRAPE-R-004: product cap from Config (CLI `--fetch-content-cap`).
    let fetch_cap = config.fetch_content_cap.max(1);
    let web_cap = output.results.len().min(fetch_cap);
    let news_cap = output
        .news
        .as_ref()
        .map(|n| n.len().min(fetch_cap))
        .unwrap_or(0);
    let total = web_cap + news_cap;
    tracing::info!(
        total,
        web_cap,
        news_cap,
        parallel = config.parallelism.get(),
        "starting parallel enrichment with --fetch-content"
    );

    // Global admission gate: `--parallel` / `--max-concurrency` (same policy
    // as multi-query fan-out). Per-host map below is a second, stricter gate.
    // Chrome multi-process pool size matches the gate so admitted CDP work is
    // truly concurrent (one OS process per in-flight fetch).
    let effective = crate::concurrency::effective_concurrency(config.parallelism.get());
    // GAP-PAR-016: nested multi-query forces pool_size=1 so peak Chrome ≤ effective.
    let pool_target = crate::concurrency::chrome_pool_size(
        config.parallelism.get(),
        total,
        options.nested_in_query_fanout,
    );
    crate::concurrency::log_chrome_concurrency_advisory(effective);
    if options.nested_in_query_fanout {
        tracing::debug!(
            pool_target,
            effective,
            chrome_budget = crate::concurrency::chrome_process_budget(config.parallelism.get()),
            "nested enrich under query fan-out — pool_size capped at 1 (GAP-PAR-016)"
        );
    }
    let mapa_por_host: PerHostSemaphoreMap =
        Arc::new(StdMutex::new(HashMap::with_capacity(total.clamp(1, 32))));
    let breaker: CircuitBreakerMap = CircuitBreakerMap::new();
    let per_host_limit = config.per_host_limit.get().max(1);
    let max_size = config.max_content_length.get();

    // GAP-WS-113: Chrome is the only production fetch transport. Launch a
    // multi-process pool before fan-out; HTTP residual only under harness.

    // WS-25: ProgressBar for long crawls. indicatif auto-detects TTY and
    // suppresses the bar when stderr is not a terminal (e.g. when piped to
    // a log file). The bar lives until `finish()`/`finish_and_clear()`.
    let progress = ProgressBar::new(total as u64);
    // Static template — failure is a programming error; fall back instead of panic
    // (defensive security: no `expect` on production paths).
    let style = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>4}/{len:4} {msg}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar())
    .progress_chars("##-");
    progress.set_style(style);
    progress.set_message("fetching");

    // Multi-process Chrome pool (GAP-PAR residual): free-list of independent
    // browser processes. `tokio::sync::Mutex` only protects the free-list
    // pop/push (microseconds); CDP navigation runs without holding the pool
    // lock. Contrast with the previous single-browser design that held a
    // Mutex across the entire extract await (serial CDP).
    //
    // GAP-PAR-011: launches run in a bounded JoinSet (N = pool_target ≤ 20),
    // not a serial for-await loop — cold-start scales with max(launch) not sum.
    // Xvfb display allocation is mutexed in browser.rs (GAP-PAR-015).
    #[cfg(feature = "chrome")]
    let (chrome_pool, chrome_pool_len): (Option<ChromeBrowserPool>, usize) = {
        let manual_path = config.chrome_path.as_deref();
        match detect_chrome(manual_path) {
            Ok(path) => {
                tracing::info!(
                    path = %path.display(),
                    pool_target,
                    effective,
                    nested = options.nested_in_query_fanout,
                    "Chrome detected — launching multi-process fetch pool in parallel (GAP-WS-113)"
                );
                let timeout_launch = std::time::Duration::from_secs(CONTENT_CHROME_TIMEOUT_SECS);
                let proxy = match &config.proxy_config {
                    crate::http::ProxyConfig::Url(u) => Some(u.as_str().to_string()),
                    _ => None,
                };
                let user_agent = config.user_agent.as_str().to_string();
                let mut launch_set: JoinSet<(usize, Result<ChromeBrowser, crate::error::CliError>)> =
                    JoinSet::new();
                for i in 0..pool_target {
                    if cancellation.is_cancelled() {
                        break;
                    }
                    let path = path.clone();
                    let proxy = proxy.clone();
                    let user_agent = user_agent.clone();
                    let cancel = cancellation.clone();
                    launch_set.spawn(async move {
                        if cancel.is_cancelled() {
                            return (
                                i,
                                Err(crate::error::CliError::Cancelled),
                            );
                        }
                        let result = ChromeBrowser::launch(
                            &path,
                            proxy.as_deref(),
                            timeout_launch,
                            &user_agent,
                        )
                        .await;
                        (i, result)
                    });
                }
                let mut launched: VecDeque<ChromeBrowser> = VecDeque::with_capacity(pool_target);
                while let Some(joined) = launch_set.join_next().await {
                    match joined {
                        Ok((index, Ok(browser))) => {
                            tracing::debug!(
                                index,
                                pool_size = launched.len() + 1,
                                "Chrome pool member launched"
                            );
                            launched.push_back(browser);
                        }
                        Ok((index, Err(err))) => {
                            tracing::error!(
                                ?err,
                                index,
                                launched = launched.len(),
                                "failed to launch Chrome pool member"
                            );
                        }
                        Err(join_err) => {
                            if join_err.is_panic() {
                                tracing::error!(
                                    ?join_err,
                                    "Chrome pool launch task panicked"
                                );
                            } else if join_err.is_cancelled() {
                                tracing::warn!(
                                    ?join_err,
                                    "Chrome pool launch task cancelled"
                                );
                            } else {
                                tracing::warn!(?join_err, "Chrome pool launch join failed");
                            }
                        }
                    }
                }
                let n = launched.len();
                if n == 0 {
                    tracing::error!(
                        "failed to launch any Chrome for fetch-content (GAP-WS-113) — no HTTP fallback"
                    );
                    (None, 0)
                } else {
                    if n < pool_target {
                        tracing::warn!(
                            launched = n,
                            pool_target,
                            "Chrome pool partially launched — admission gate reduced to launched size"
                        );
                    }
                    (Some(Arc::new(TokioMutex::new(launched))), n)
                }
            }
            Err(err) => {
                tracing::error!(
                    ?err,
                    "Chrome not detected for fetch-content (GAP-WS-113) — no HTTP fallback"
                );
                (None, 0)
            }
        }
    };

    #[cfg(not(feature = "chrome"))]
    {
        if config.chrome_path.is_some() {
            tracing::warn!(
                "--chrome-path provided but binary was not compiled with --features chrome — ignoring"
            );
        }
    }

    #[cfg(feature = "chrome")]
    if !crate::chrome_policy::http_test_harness_active() && chrome_pool.is_none() {
        tracing::error!("fetch-content aborted — Chrome unavailable (GAP-WS-113)");
        output.metadata.chrome_attempted = true;
        output.metadata.used_chrome = false;
        output.metadata.fetch_failures = u32::try_from(total).unwrap_or(u32::MAX);
        if output.error.is_none() {
            let err = crate::error::CliError::chrome_not_found(
                "fetch-content requires chromiumoxide (GAP-WS-113); chrome launch failed",
            );
            output.error = Some(err.error_code().to_string());
            if output.message.is_none() {
                output.message = Some(err.to_string());
            }
        } else if output.message.is_none() {
            output.message = Some(
                "fetch-content requires chromiumoxide (GAP-WS-113); chrome launch failed".into(),
            );
        }
        progress.finish_and_clear();
        return;
    }

    // Admission permits = live Chrome processes (production) or effective (harness).
    #[cfg(feature = "chrome")]
    let admit = if chrome_pool_len > 0 {
        chrome_pool_len
    } else if crate::chrome_policy::http_test_harness_active() {
        effective as usize
    } else {
        1
    };
    #[cfg(not(feature = "chrome"))]
    let admit = effective as usize;
    let semaphore = Arc::new(Semaphore::new(admit.max(1)));

    // Target: web index or news index. Payload: text, size, method.
    #[derive(Clone, Copy)]
    enum FetchKind {
        Web(usize),
        News(usize),
    }
    type FetchOutcome = (FetchKind, Option<(String, u32, String)>);
    let mut tasks: JoinSet<FetchOutcome> = JoinSet::new();

    let mut spawn_urls: Vec<(FetchKind, String)> = Vec::with_capacity(total);
    for (index, result_item) in output.results.iter().enumerate().take(web_cap) {
        spawn_urls.push((FetchKind::Web(index), result_item.url.as_str().to_owned()));
    }
    if let Some(news) = output.news.as_ref() {
        for (index, item) in news.iter().enumerate().take(news_cap) {
            // GAP-WS-NEWS-FETCH-WASTE-001: never fetch DDG promo/chrome URLs.
            if crate::extraction::is_ddg_promo_url(item.url.as_str()) {
                tracing::debug!(url = %item.url, "skipping promo news URL for content fetch");
                continue;
            }
            spawn_urls.push((FetchKind::News(index), item.url.as_str().to_owned()));
        }
    }

    for (kind, url) in spawn_urls {
        if cancellation.is_cancelled() {
            tracing::warn!("cancellation detected — aborting fetch spawns");
            break;
        }
        // Residual HTTP client is only present under http-test-harness (GAP-TLS-014).
        let task_client = client.cloned();
        let task_semaphore = Arc::clone(&semaphore);
        let mapa_task = Arc::clone(&mapa_por_host);
        let task_breaker = breaker.clone();
        let task_cancellation = cancellation.clone();

        #[cfg(feature = "chrome")]
        let pool_task: Option<ChromeBrowserPool> = chrome_pool.as_ref().map(Arc::clone);

        tasks.spawn(async move {
            tracing::debug!(
                permits_available = task_semaphore.available_permits(),
                url = %url,
                "awaiting global fetch semaphore permit"
            );
            let Ok(permit_global) = task_semaphore.acquire_owned().await else {
                return (kind, None);
            };
            if task_cancellation.is_cancelled() {
                drop(permit_global);
                return (kind, None);
            }
            let host = extract_host(&url);
            if task_breaker.check(&host) == BreakerDecision::Reject {
                drop(permit_global);
                return (kind, None);
            }
            let semaforo_host = semaphore_for_host(&mapa_task, &host, per_host_limit as usize);
            let Ok(permit_host) = semaforo_host.acquire_owned().await else {
                drop(permit_global);
                return (kind, None);
            };
            if task_cancellation.is_cancelled() {
                drop(permit_host);
                drop(permit_global);
                return (kind, None);
            }

            let harness = crate::chrome_policy::http_test_harness_active();
            // GAP-SCRAPE-015: SSRF before Chrome navigate (same gate as HTTP residual).
            if !content::url_is_safe_to_fetch(&url).await {
                tracing::warn!(
                    url = %url,
                    "URL rejected by SSRF filter before content fetch — skipping"
                );
                drop(permit_host);
                drop(permit_global);
                return (kind, None);
            }
            let outcome: FetchOutcome = {
                #[cfg(feature = "chrome")]
                if let Some(pool) = pool_task.as_ref() {
                    // Pop a dedicated Chrome OS process from the free-list.
                    // Pool Mutex is held only for pop/push (no CDP under lock).
                    let mut browser = {
                        let mut free = pool.lock().await;
                        free.pop_front()
                    };
                    let extract_outcome = if let Some(ref mut b) = browser {
                        Some(
                            extract_text_with_chrome(
                                b,
                                &url,
                                max_size,
                                std::time::Duration::from_secs(
                                    crate::browser::CONTENT_CHROME_TIMEOUT_SECS,
                                ),
                            )
                            .await,
                        )
                    } else {
                        tracing::warn!(
                            url = %url,
                            "Chrome pool empty under admission gate — unexpected"
                        );
                        None
                    };
                    // Always return the process to the free-list (or drop on panic via RAII Drop).
                    if let Some(b) = browser {
                        let mut free = pool.lock().await;
                        free.push_back(b);
                    }
                    match extract_outcome {
                        Some(Ok(text)) if !text.is_empty() => {
                            task_breaker.record_success(&host);
                            let size_cast = u32::try_from(text.len()).unwrap_or(u32::MAX);
                            drop(permit_host);
                            drop(permit_global);
                            return (kind, Some((text, size_cast, "chrome".to_string())));
                        }
                        Some(Ok(_)) => task_breaker.record_failure(&host),
                        Some(Err(_)) => task_breaker.record_failure(&host),
                        None => task_breaker.record_failure(&host),
                    }
                    if !harness {
                        drop(permit_host);
                        drop(permit_global);
                        return (kind, None);
                    }
                } else if !harness {
                    task_breaker.record_failure(&host);
                    drop(permit_host);
                    drop(permit_global);
                    return (kind, None);
                }

                if harness {
                    let Some(ref http_client) = task_client else {
                        task_breaker.record_failure(&host);
                        drop(permit_host);
                        drop(permit_global);
                        return (kind, None);
                    };
                    let result_item = content::extract_http_content(
                        http_client,
                        &url,
                        max_size,
                        &task_cancellation,
                    )
                    .await;
                    match &result_item {
                        Ok(Some((text, _))) if !text.is_empty() => {
                            task_breaker.record_success(&host)
                        }
                        Ok(_) | Err(_) => task_breaker.record_failure(&host),
                    }
                    match result_item {
                        Ok(Some((text, size))) if !text.is_empty() => {
                            (kind, Some((text, size, "http".to_string())))
                        }
                        _ => (kind, None),
                    }
                } else {
                    (kind, None)
                }
            };

            drop(permit_host);
            drop(permit_global);
            outcome
        });
    }

    let mut sucessos: u32 = 0;
    let mut falhas: u32 = 0;
    let mut usou_chrome: bool = false;

    while let Some(join_res) = tasks.join_next().await {
        match join_res {
            Ok((kind, Some((text, _size_original, method)))) if !text.is_empty() => {
                if method == "chrome" {
                    usou_chrome = true;
                }
                let actual_size = u32::try_from(text.len()).unwrap_or(u32::MAX);
                match kind {
                    FetchKind::Web(index) if index < output.results.len() => {
                        let res = &mut output.results[index];
                        res.content = Some(text);
                        res.content_size = Some(actual_size);
                        res.content_extraction_method = Some(method);
                        sucessos = sucessos.saturating_add(1);
                    }
                    FetchKind::News(index) => {
                        if let Some(news) = output.news.as_mut() {
                            if index < news.len() {
                                let item = &mut news[index];
                                item.content = Some(text);
                                item.content_size = Some(actual_size as usize);
                                item.content_extraction_method = Some(method);
                                sucessos = sucessos.saturating_add(1);
                            } else {
                                falhas = falhas.saturating_add(1);
                            }
                        } else {
                            falhas = falhas.saturating_add(1);
                        }
                    }
                    _ => {
                        falhas = falhas.saturating_add(1);
                    }
                }
            }
            Ok((_, None)) | Ok((_, Some(_))) => {
                falhas = falhas.saturating_add(1);
            }
            Err(error_join) => {
                if error_join.is_panic() {
                    tracing::error!(
                        ?error_join,
                        "fetch task panicked — permit + pool browser recovered via RAII Drop"
                    );
                } else if error_join.is_cancelled() {
                    tracing::warn!(?error_join, "fetch task cancelled (JoinError::is_cancelled)");
                } else {
                    tracing::warn!(?error_join, "fetch task join failed");
                }
                falhas = falhas.saturating_add(1);
            }
        }
        progress.inc(1);
    }

    output.metadata.concurrent_fetches = u32::try_from(total).unwrap_or(u32::MAX);
    output.metadata.fetch_successes = sucessos;
    output.metadata.fetch_failures = falhas;
    if usou_chrome {
        output.metadata.used_chrome = true;
    }

    // WS-25: close the progress bar so the cursor returns to a clean state
    // and the next prompt/print starts on a fresh line. `finish_and_clear`
    // erases the bar from the terminal instead of leaving it visible.
    progress.finish_and_clear();

    // GAP-WS-LIFECYCLE-001 L-05: JoinSet drained; drain the multi-process pool
    // and async-shutdown every Chrome (tree + Xvfb + TempDir). Never bare-drop.
    // GAP-PAR-012: shutdown members in parallel (bounded JoinSet, N = pool size).
    #[cfg(feature = "chrome")]
    if let Some(pool_arc) = chrome_pool {
        let browsers = {
            let mut free = pool_arc.lock().await;
            std::mem::take(&mut *free)
        };
        drop(pool_arc);
        let pool_n = browsers.len();
        let mut shutdown_set: JoinSet<Result<(), crate::error::CliError>> = JoinSet::new();
        for browser in browsers {
            shutdown_set.spawn(async move { browser.shutdown().await });
        }
        while let Some(joined) = shutdown_set.join_next().await {
            match joined {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    tracing::error!(
                        error = %e,
                        "Chrome pool member shutdown after fetch-content failed"
                    );
                }
                Err(join_err) => {
                    if join_err.is_panic() {
                        tracing::error!(?join_err, "Chrome pool shutdown task panicked");
                    } else if join_err.is_cancelled() {
                        tracing::warn!(?join_err, "Chrome pool shutdown task cancelled");
                    } else {
                        tracing::warn!(?join_err, "Chrome pool shutdown join failed");
                    }
                }
            }
        }
        if pool_n > 0 {
            tracing::info!(
                pool_size = pool_n,
                "Chrome multi-process pool shut down in parallel after enrichment (one-shot finalize)"
            );
        }
    }

    tracing::info!(total, sucessos, falhas, "content enrichment complete");
}

