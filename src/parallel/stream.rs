// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound multi-query fan-out (JoinSet + Semaphore) — streaming mode.
//! Streaming multi-query fan-out: emit results as tasks complete.

use crate::error::CliError;
use crate::http;
use crate::types::{Config, SearchOutput};
use rand::RngExt;
use reqwest::Client;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::{
    error_output, execute_query_with_cancellation, DELAY_BASE_STAGGERED_MS, MAX_STAGGERED_JITTER_MS,
};

/// Aggregated statistics for a multi-query execution in streaming mode.
#[derive(Debug, Clone, Default)]
pub struct StreamStats {
    /// Total queries submitted.
    pub total: u32,
    /// Queries completed successfully (no `error` field).
    pub successes: u32,
    /// Queries completed with an error.
    pub errors: u32,
    /// Timestamp (RFC 3339) of execution start.
    pub start_timestamp: chrono::DateTime<chrono::Utc>,
    /// Effective parallelism.
    pub parallelism: u32,
}

/// Executes multiple queries in parallel EMITTING results via `mpsc::Sender`
/// as each task finishes. The consumer (in `pipeline`) receives the results and
/// emits NDJSON / text / markdown incrementally.
///
/// Returns `StreamStats` after all tasks have finished.
///
/// Results arrive in COMPLETION ORDER (not the order of the input queries).
/// Each sent item is `(original_index, SearchOutput)` so the consumer knows
/// which query produced each output.
///
/// # Errors
///
/// Returns an error only if the shared HTTP client cannot be built when `paginas <= 1`.
/// Individual query failures are captured in the `SearchOutput` sent through the channel.
/// If the channel receiver is dropped early, remaining tasks are aborted and the
/// function returns the statistics collected up to that point.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future stops the staggered-launch loop
/// and causes in-flight tasks to observe the cancellation token on their next checkpoint,
/// aborting gracefully without sending to the (now-dropped) channel.
#[tracing::instrument(skip_all, fields(query_count = queries.len(), parallelism = config.parallelism.get()))]
pub async fn execute_parallel_searches_streaming(
    queries: Vec<crate::security::ValidatedQuery>,
    config: Config,
    cancellation: CancellationToken,
    output_channel: mpsc::Sender<(usize, SearchOutput)>,
) -> Result<StreamStats, CliError> {
    let query_count = u32::try_from(queries.len()).unwrap_or(u32::MAX);
    let effective_parallelism = crate::concurrency::effective_concurrency(config.parallelism.get());
    let start_timestamp = crate::types::utc_now();

    tracing::info!(
        queries = query_count,
        parallel = effective_parallelism,
        cpus = crate::concurrency::cpu_count(),
        "Starting parallel multi-query streaming execution"
    );

    crate::concurrency::log_chrome_concurrency_advisory(effective_parallelism);
    let dual_vertical = crate::concurrency::prefer_dual_vertical_chrome(
        effective_parallelism,
        crate::concurrency::DualVerticalMode::Auto,
        config.shared_session_verticals,
    );
    let chrome_slots = crate::concurrency::chrome_slots_per_query(
        config.vertical.includes_web(),
        config.vertical.includes_news(),
        dual_vertical,
    );
    let semaphore = Arc::new(Semaphore::new(effective_parallelism as usize));
    let config = Arc::new(config);
    let flag_rate_limit = Arc::new(AtomicBool::new(false));

    let config_proxy = Arc::new(config.proxy_config.clone());

    let client_shared: Option<Client> = if crate::chrome_policy::http_test_harness_active()
        && config.pages.get() <= 1
    {
        http::build_client_with_proxy_and_cookies(
            &config.browser_profile,
            config.timeout_seconds.get(),
            config.language.as_str(),
            config.country.as_str(),
            &config_proxy,
            config.cookie_provider.clone(),
        )
        .map_err(|e| {
            CliError::http_with_source("failed to build shared HTTP client for streaming", e)
        })
        .map(Some)?
    } else {
        None
    };

    let mut task_set: JoinSet<(usize, SearchOutput)> = JoinSet::new();

    for (index, query) in queries.into_iter().enumerate() {
        let task_semaphore = Arc::clone(&semaphore);
        let task_config = Arc::clone(&config);
        let task_cancellation = cancellation.clone();
        let task_client = client_shared.clone();
        let flag_rate_limit_task = Arc::clone(&flag_rate_limit);
        let config_proxy_task = Arc::clone(&config_proxy);
        let task_slots = chrome_slots;

        task_set.spawn(async move {
            let jitter_ms = rand::rng().random_range(0..MAX_STAGGERED_JITTER_MS);
            let delay_total = Duration::from_millis(
                DELAY_BASE_STAGGERED_MS.saturating_mul(index as u64) + jitter_ms,
            );

            tokio::select! {
                biased;
                _ = task_cancellation.cancelled() => {
                    return (
                        index,
                        error_output(index, &CliError::Cancelled, &task_config),
                    );
                }
                _ = tokio::time::sleep(delay_total) => {}
            }

            tracing::info!(
                permits_available = task_semaphore.available_permits(),
                query_index = index,
                chrome_slots = task_slots,
                "awaiting semaphore permit(s) (streaming)"
            );
            let permit = match task_semaphore.acquire_many_owned(task_slots).await {
                Ok(p) => p,
                Err(err) => {
                    let e = CliError::NetworkError {
                        message: format!("semaphore closed: {err}"),
                    };
                    return (index, error_output(index, &e, &task_config));
                }
            };

            tracing::info!(
                query_index = index,
                chrome_slots = task_slots,
                "permit acquired (streaming)"
            );

            if task_cancellation.is_cancelled() {
                drop(permit);
                return (
                    index,
                    error_output(index, &CliError::Cancelled, &task_config),
                );
            }

            let client_result: Result<Option<Client>, CliError> = match task_client {
                Some(c) => Ok(Some(c)),
                None if crate::chrome_policy::http_test_harness_active() => {
                    http::build_client_with_proxy_and_cookies(
                        &task_config.browser_profile,
                        task_config.timeout_seconds.get(),
                        task_config.language.as_str(),
                        task_config.country.as_str(),
                        &config_proxy_task,
                        task_config.cookie_provider.clone(),
                    )
                    .map(Some)
                    .map_err(|e| {
                        CliError::http_with_source("failed to build isolated Client", e)
                    })
                }
                None => Ok(None),
            };

            let result = match client_result {
                Ok(client_opt) => {
                    execute_query_with_cancellation(
                        &query,
                        client_opt.as_ref(),
                        &task_config,
                        &flag_rate_limit_task,
                        &task_cancellation,
                    )
                    .await
                }
                Err(err) => Err(err),
            };

            drop(permit);
            match result {
                Ok(output) => (index, output),
                Err(err) => (index, error_output(index, &err, &task_config)),
            }
        });
    }

    let mut success_count: u32 = 0;
    let mut error_count: u32 = 0;

    while let Some(task_result) = task_set.join_next().await {
        match task_result {
            Ok((index, output)) => {
                if output.error.is_some() {
                    error_count = error_count.saturating_add(1);
                } else {
                    success_count = success_count.saturating_add(1);
                }
                if let Err(send_error) = output_channel.send((index, output)).await {
                    tracing::warn!(
                        ?send_error,
                        "streaming consumer closed channel — aborting send"
                    );
                    task_set.abort_all();
                    break;
                }
            }
            Err(join_err) => {
                // GAP-PAR-013: distinguish panic vs cancel vs other JoinError.
                if join_err.is_panic() {
                    tracing::error!(
                        ?join_err,
                        "task panicked in streaming — permit recovered via RAII"
                    );
                } else if join_err.is_cancelled() {
                    tracing::warn!(
                        ?join_err,
                        "task cancelled in streaming (JoinError::is_cancelled)"
                    );
                } else {
                    tracing::warn!(?join_err, "task join failed in streaming");
                }
                error_count = error_count.saturating_add(1);
            }
        }
    }

    tracing::info!(
        total = query_count,
        successes = success_count,
        errors = error_count,
        "streaming complete"
    );

    Ok(StreamStats {
        total: query_count,
        successes: success_count,
        errors: error_count,
        start_timestamp,
        parallelism: effective_parallelism,
    })
}

