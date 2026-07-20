// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound multi-query fan-out (JoinSet + Semaphore) — batch mode.
//! Batch multi-query fan-out: collect all results, preserve input order.

use crate::error::CliError;
use crate::http;
use crate::types::{Config, MultiSearchOutput, SearchOutput};
use rand::RngExt;
use reqwest::Client;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::{
    error_output, execute_query_with_cancellation, DELAY_BASE_STAGGERED_MS, MAX_STAGGERED_JITTER_MS,
};

/// Executes multiple queries in parallel respecting the `--parallel` limit.
///
/// # Arguments
/// * `queries` — already deduplicated/filtered list of queries.
/// * `configuracoes` — configuration template (individual query will be overwritten).
/// * `cancelamento` — token that signals SIGINT / global timeout.
///
/// # Failure behaviour
/// If a query fails, its `SearchOutput` is generated with `error` filled in and
/// `results_count = 0`. The process does NOT abort other in-flight queries.
///
/// # Errors
///
/// Returns an error only if the shared HTTP client cannot be built when `paginas <= 1`.
/// Individual query failures are captured inside the returned [`MultiSearchOutput`]
/// rather than propagated as `Err`.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future signals the cancellation token
/// to all spawned tasks; each task checks the token before and after acquiring its
/// semaphore permit and terminates gracefully.
#[tracing::instrument(skip_all, fields(query_count = queries.len(), parallelism = config.parallelism.get()))]
pub async fn execute_parallel_searches(
    queries: Vec<crate::security::ValidatedQuery>,
    config: Config,
    cancellation: CancellationToken,
) -> Result<MultiSearchOutput, CliError> {
    let query_count = u32::try_from(queries.len()).unwrap_or(u32::MAX);
    let effective_parallelism = crate::concurrency::effective_concurrency(config.parallelism.get());
    let start_timestamp = crate::types::utc_now();

    tracing::info!(
        queries = query_count,
        parallel = effective_parallelism,
        pages = config.pages.get(),
        cpus = crate::concurrency::cpu_count(),
        "Starting parallel multi-query execution"
    );

    // Permit calculation: see `concurrency` module (single source of truth).
    // User flag `--parallel` / `--max-concurrency` is the hard gate; CPU/RAM
    // only emit an advisory when Chrome multi-process may thrash.
    crate::concurrency::log_chrome_concurrency_advisory(effective_parallelism);
    // GAP-PAR-021b: query Semaphore is the Chrome OS process budget. Dual
    // web+news multi-process costs 2 slots per task (acquire_many_owned).
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
    tracing::info!(
        dual_chrome = dual_vertical,
        chrome_slots,
        effective = effective_parallelism,
        "multi-query chrome slot policy (GAP-PAR-021b)"
    );
    let semaphore = Arc::new(Semaphore::new(effective_parallelism as usize));
    let config = Arc::new(config);
    let flag_rate_limit = Arc::new(AtomicBool::new(false));

    let config_proxy = Arc::new(config.proxy_config.clone());

    // Residual reqwest Client only under http-test-harness (GAP-TLS-014).
    // pages == 1 → shared; pages > 1 → isolated per task (harness only).
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
            CliError::http_with_source("failed to build shared HTTP client for multi-query", e)
        })
        .map(Some)?
    } else {
        None
    };

    let mut task_set: JoinSet<(usize, Result<SearchOutput, CliError>)> = JoinSet::new();

    for (index, query) in queries.into_iter().enumerate() {
        // Clone refs to move into the spawned task.
        let task_semaphore = Arc::clone(&semaphore);
        let task_config = Arc::clone(&config);
        let task_cancellation = cancellation.clone();
        let task_client = client_shared.clone();
        let flag_rate_limit_task = Arc::clone(&flag_rate_limit);
        let config_proxy_task = Arc::clone(&config_proxy);
        let task_slots = chrome_slots;

        task_set.spawn(async move {
            // Staggered launch: delay before acquiring permit to avoid synchronous burst.
            // (Anti-bot intentional — N/A-PAR: not a missing parallelization.)
            let jitter_ms = rand::rng().random_range(0..MAX_STAGGERED_JITTER_MS);
            let delay_total = Duration::from_millis(
                DELAY_BASE_STAGGERED_MS.saturating_mul(index as u64) + jitter_ms,
            );

            tokio::select! {
                biased;
                _ = task_cancellation.cancelled() => {
                    return (index, Err(CliError::Cancelled));
                }
                _ = tokio::time::sleep(delay_total) => {}
            }

            // Acquire chrome-budget slots (1 or 2 for dual vertical) — RAII drop.
            tracing::info!(
                permits_available = task_semaphore.available_permits(),
                query_index = index,
                chrome_slots = task_slots,
                "awaiting semaphore permit(s)"
            );
            let permit = match task_semaphore.acquire_many_owned(task_slots).await {
                Ok(p) => p,
                Err(err) => {
                    return (
                        index,
                        Err(CliError::NetworkError {
                            message: format!("semaphore closed: {err}"),
                        }),
                    );
                }
            };

            tracing::info!(
                index,
                query = %query,
                chrome_slots = task_slots,
                "permit acquired, starting task"
            );

            if task_cancellation.is_cancelled() {
                drop(permit);
                return (index, Err(CliError::Cancelled));
            }

            // Per-task residual Client (harness + pages>1 only).
            let client_result: Result<Option<Client>, CliError> = match task_client {
                Some(shared) => Ok(Some(shared)),
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
                        CliError::http_with_source(
                            "failed to build isolated Client for query",
                            e,
                        )
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
            (index, result)
        });
    }

    // Coleta todas as tasks — preservando a ordem original das queries.
    let mut ordered_results: Vec<Option<SearchOutput>> = (0..query_count).map(|_| None).collect();

    while let Some(task_result) = task_set.join_next().await {
        match task_result {
            Ok((index, Ok(output))) => {
                ordered_results[index] = Some(output);
            }
            Ok((index, Err(err))) => {
                tracing::warn!(index, ?err, "query failed, generating error SearchOutput");
                ordered_results[index] = Some(error_output(index, &err, &config));
            }
            Err(join_err) => {
                // GAP-PAR-013: distinguish panic vs cancel vs other JoinError.
                if join_err.is_panic() {
                    tracing::error!(?join_err, "task panicked — permit recovered via RAII");
                } else if join_err.is_cancelled() {
                    tracing::warn!(
                        ?join_err,
                        "task cancelled (JoinError::is_cancelled) — permit recovered via RAII"
                    );
                } else {
                    tracing::warn!(?join_err, "task join failed");
                }
                if let Some(slot) = ordered_results.iter_mut().find(|s| s.is_none()) {
                    let err = CliError::NetworkError {
                        message: format!("task join failed: {join_err}"),
                    };
                    *slot = Some(error_output(0, &err, &config));
                }
            }
        }
    }

    // Convert Option<SearchOutput> into Vec<SearchOutput> (all slots must be filled).
    let searches: Vec<SearchOutput> = ordered_results
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            slot.unwrap_or_else(|| {
                let err = CliError::NetworkError {
                    message: format!("missing result for query {index}"),
                };
                error_output(index, &err, &config)
            })
        })
        .collect();

    tracing::info!(total = searches.len(), "multi-query complete");

    // GAP-AUD-003 v0.8.0: aggregate zero-cause histogram across sub-queries.
    // BTreeMap guarantees lexicographic key order in the JSON output (deterministic).
    let mut causa_zero_histogram: BTreeMap<String, u32> = BTreeMap::new();
    for s in &searches {
        if let Some(cause) = s.metadata.zero_cause {
            let key = serde_json::to_value(cause)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| format!("{cause:?}"));
            *causa_zero_histogram.entry(key).or_insert(0) += 1;
        }
    }

    Ok(MultiSearchOutput {
        query_count,
        timestamp: start_timestamp,
        parallelism: effective_parallelism,
        searches,
        causa_zero_histogram,
    })
}


