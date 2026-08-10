// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: I/O-bound orchestrator (dispatches to parallel.rs and content_fetch.rs).
// Parallelism in this module (GAP-PAR-021):
// - multi-query → parallel:: JoinSet + Semaphore
// - single-query `--vertical all` → dual multi-process Chrome (web ∥ news) when
//   chrome budget ≥ 2 and !--shared-session-verticals; else shared serial session
// Bounded mpsc channel provides backpressure between producer and consumer in streaming mode.
//! Orchestration of the CLI execution flow.
//!
//! In iteration 2, decides between single-query and multi-query flow based on
//! the number of effective queries (after combining positional + file + stdin,
//! dedup and empty-string filtering).
//!
//! - Single-query (1 query): uses the legacy `execute_single_search` flow and emits `SearchOutput`.
//! - Multi-query (>=2 queries): delegates to `parallel::execute_parallel_searches`
//!   and emits `MultiSearchOutput`.

use crate::error::CliError;
use crate::parallel;
use crate::types::{Config, MultiSearchOutput, SearchOutput};
use tokio_util::sync::CancellationToken;

#[cfg(feature = "chrome")]
mod chrome;
pub mod failure;
pub mod queries;
// Re-export failure envelopes for crate callers (parallel / lib).
#[cfg(test)]
pub(crate) use failure::chrome_cancelled_error;
#[cfg(feature = "http-test-harness")]
#[allow(unused_imports)] // re-exported for crate callers; not all used in this file
pub(crate) use failure::failure_output;
#[allow(unused_imports)] // re-exported for crate callers; not all used in this file
pub(crate) use failure::{chrome_transport_failure_output, news_only_chrome_failure_output};
pub(crate) use queries::{calculate_selectors_hash, derive_cascade_level_from_attempts};
pub use queries::{
    combine_and_dedup_queries, read_queries_from_file, read_queries_from_stdin_if_pipe,
};

/// Result emitted by the pipeline — may be a single output, aggregated multi output, or an already-emitted stream.
///
/// The `Stream` variant indicates that output was already emitted incrementally by
/// the consumer; the final `output` step MUST NOT re-emit anything. Only the
/// aggregated statistics are available for logging / exit-code decisions.
#[derive(Debug, Clone)]
pub enum PipelineResult {
    /// Single-query execution produced one output.
    Single(Box<SearchOutput>),
    /// Multi-query execution produced aggregated output.
    Multi(Box<MultiSearchOutput>),
    /// Streaming mode — output already emitted incrementally; only stats remain.
    Stream(crate::parallel::StreamStats),
}

impl PipelineResult {
    /// Total results summed across all queries (used for exit-code decisions).
    ///
    /// For `Stream` returns `successes` — a sufficient approximation for exit codes 0/5
    /// (success vs zero-results).
    pub fn total_results(&self) -> u32 {
        match self {
            // GAP-WS-104: sums `news_count` — news-only with news ⇒ exit 0;
            // news-only without news ⇒ exit 5 (legitimate zero).
            PipelineResult::Single(s) => s.result_count.saturating_add(s.news_count.unwrap_or(0)),
            PipelineResult::Multi(m) => m
                .searches
                .iter()
                .map(|b| b.result_count.saturating_add(b.news_count.unwrap_or(0)))
                .fold(0u32, |acc, v| acc.saturating_add(v)),
            PipelineResult::Stream(e) => e.successes,
        }
    }

    /// GAP-WS-092 + GAP-WS-093: populate compat fields in all inner `SearchOutputs`.
    pub fn fill_compat_fields(&mut self) {
        match self {
            PipelineResult::Single(s) => s.fill_compat_fields(),
            PipelineResult::Multi(m) => {
                for search in &mut m.searches {
                    search.fill_compat_fields();
                }
            }
            PipelineResult::Stream(_) => {}
        }
    }
}

/// Entry point for iteration 2: decides single vs multi based on `configuracoes.queries`.
///
/// `cancelamento` is the token that signals SIGINT (ctrl+c). In single-query mode
/// cancellation only affects the request via `reqwest` timeout; in multi-query mode it
/// is propagated explicitly to each task.
///
/// # Errors
///
/// Returns an error if the query list is empty, if the HTTP client cannot be built,
/// or if the underlying single-query or multi-query execution fails unrecoverably.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future propagates the cancellation
/// token to any in-flight sub-tasks, which will terminate gracefully.
pub async fn execute_pipeline(
    config: Config,
    cancellation: CancellationToken,
) -> Result<PipelineResult, CliError> {
    match config.queries.len() {
        0 => Err(CliError::InvalidConfig {
            message: "no queries to execute (list empty after filtering)".into(),
        }),
        1 => {
            if config.stream_mode {
                tracing::warn!(
                    "--stream ignored in single-query mode (only 1 effective query); \
                     emitting default aggregated output"
                );
            }
            // Clone intentional: overwrites query field for single-query compatibility.
            // Cost: ~15 String clones, executed exactly once per CLI invocation.
            let mut cfg_single = config.clone();
            cfg_single.query = cfg_single.queries[0].clone();
            let output = execute_single_search(&cfg_single, &cancellation).await?;
            persist_cookies(&cfg_single);
            Ok(PipelineResult::Single(Box::new(output)))
        }
        _ => {
            if config.stream_mode {
                return execute_pipeline_streaming(config, cancellation).await;
            }
            let queries = config.queries.clone();
            // Persist cookies after the parallel search completes, using
            // a clone of `config` because `config` is moved into the
            // search call.
            let config_for_persist = config.clone();
            let multi = parallel::execute_parallel_searches(queries, config, cancellation).await?;
            persist_cookies(&config_for_persist);
            Ok(PipelineResult::Multi(Box::new(multi)))
        }
    }
}

/// Persists the cookie jar to disk after the search completes. v0.7.3 PR2.
fn persist_cookies(config: &Config) {
    // GAP-WS-113: only the residual harness transport owns a Rust cookie jar;
    // Chrome persists its own cookies inside the one-shot profile.
    #[cfg(feature = "http-test-harness")]
    if let Some(persistent_jar) = config.persistent_jar.as_ref() {
        persistent_jar.save();
    }
    #[cfg(not(feature = "http-test-harness"))]
    let _ = config;
}

/// Performs the warm-up GET to the SERP origin to populate session cookies.
/// Failures are surfaced to the caller but never fatal; the caller logs and
/// continues. v0.7.3 PR2. Residual HTTP harness path.
///
/// URL comes from [`crate::endpoints::serp_base_url`] (env-overridable).
#[cfg(feature = "http-test-harness")]
pub(super) async fn do_warmup(client: &reqwest::Client, cfg: &Config) -> Result<(), CliError> {
    let warmup_url = crate::endpoints::serp_base_url();
    tracing::info!(url = %warmup_url, "Warming up session with cookie jar");
    let response = client
        .get(&warmup_url)
        .send()
        .await
        .map_err(|e| CliError::HttpError {
            message: format!("warm-up request to {warmup_url} failed: {e}"),
            cause: None,
        })?;
    tracing::info!(
        status = response.status().as_u16(),
        url = %warmup_url,
        "warm-up response received"
    );
    let _ = cfg;
    Ok(())
}

/// Pipeline in streaming mode — emits results as tasks complete.
///
/// The spawned consumer drains the mpsc channel and emits NDJSON/text/markdown line by line.
/// Returns `PipelineResult::Stream` at the end, indicating there is nothing left to emit.
async fn execute_pipeline_streaming(
    config: Config,
    cancellation: CancellationToken,
) -> Result<PipelineResult, CliError> {
    use crate::types::OutputFormat;
    use tokio::sync::mpsc;

    let format = config.format;
    let output_file = config.output_file.clone();
    let queries = config.queries.clone();
    // These specs were already parsed and validated in `run`, which fail-fasts
    // with exit 2 BEFORE opening a Chrome session. Re-parsing them here with
    // `.ok()` discarded the error a second time, so the stream path silently
    // dropped a reduction the non-stream path refuses outright. The mismatch is
    // masked today by that upstream gate and would surface the moment anything
    // reached the pipeline without passing it. Propagating keeps the two paths
    // saying the same thing about the same input.
    let stream_fields = config
        .agent_ops
        .fields
        .as_deref()
        .map(crate::output::FieldSet::parse)
        .transpose()?;
    let stream_filter = config
        .agent_ops
        .filter
        .as_deref()
        .map(crate::output::ResultFilter::parse)
        .transpose()?;
    // Buffer = parallelism * 2 (see concurrency::stream_channel_capacity).
    let channel_cap = crate::concurrency::stream_channel_capacity(config.parallelism.get());
    let (tx, mut rx) = mpsc::channel::<(usize, SearchOutput)>(channel_cap);

    // Spawn consumer: drains items and emits per format.
    let consumer = tokio::spawn(async move {
        let mut emitidos: u64 = 0;
        while let Some((index, mut output)) = rx.recv().await {
            // GAP-JSON-001: multi-query `--stream` actually emits lines — mark
            // agent metadata so consumers can tell request vs effective stream.
            // (Single-query path keeps stream_effective=false when the flag is ignored.)
            output.metadata.stream_requested = Some(true);
            output.metadata.stream_effective = Some(true);
            crate::output::project::apply_to_search_output(
                &mut output,
                stream_fields.as_ref(),
                stream_filter.as_ref(),
            );
            let resolved_format = match format {
                OutputFormat::Auto | OutputFormat::Json => OutputFormat::Json,
                outro => outro,
            };
            // GAP-PAR-040b: format/serde each stream item off the Tokio worker.
            let res = match resolved_format {
                OutputFormat::Json | OutputFormat::Auto => {
                    if let Some(ref fs) = stream_fields {
                        match crate::output::project::format_search_json_projected(&output, fs) {
                            Ok(line) => {
                                crate::output::emit_payload_async(line, output_file.as_deref())
                                    .await
                            }
                            Err(e) => Err(e),
                        }
                    } else {
                        crate::output::emit_ndjson_async(output, output_file.clone()).await
                    }
                }
                OutputFormat::Text => {
                    crate::output::emit_stream_text_async(index, output, output_file.clone()).await
                }
                OutputFormat::Markdown => {
                    crate::output::emit_stream_markdown_async(index, output, output_file.clone())
                        .await
                }
                // Stream + TSV: reuse text stream blocks (headered TSV is non-streaming only).
                OutputFormat::Tsv => {
                    crate::output::emit_stream_text_async(index, output, output_file.clone()).await
                }
            };
            if let Err(err) = res {
                if crate::output::is_broken_pipe(&err) {
                    // GAP-E2E-51-007: propagate BrokenPipe so lib maps exit 141
                    // (do not swallow as Ok — that yields exit 0 with partial stream).
                    tracing::info!("BrokenPipe in streaming — stopping consumer with exit 141");
                    return Err(err);
                }
                tracing::error!(?err, "failed to emit streaming item — aborting consumer");
                return Err(err);
            }
            emitidos = emitidos.saturating_add(1);
        }
        tracing::info!(emitidos, "streaming consumer finished");
        Ok::<(), CliError>(())
    });

    let stats =
        parallel::execute_parallel_searches_streaming(queries, config, cancellation, tx).await?;

    match consumer.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(err),
        Err(join_err) => {
            // GAP-PAR-013: distinguish panic vs cancel vs other JoinError.
            if join_err.is_panic() {
                tracing::error!(?join_err, "streaming consumer panicked");
            } else if join_err.is_cancelled() {
                tracing::warn!(
                    ?join_err,
                    "streaming consumer cancelled (JoinError::is_cancelled)"
                );
            } else {
                tracing::warn!(?join_err, "streaming consumer join failed");
            }
            return Err(CliError::NetworkError {
                message: format!("streaming consumer join failed: {join_err}"),
            });
        }
    }

    Ok(PipelineResult::Stream(stats))
}

mod single;
pub use single::execute_single_search;

#[cfg(feature = "chrome")]
pub use chrome::{
    execute_chrome_all_search_pub, execute_chrome_news_search,
    execute_chrome_news_search_on_browser, execute_chrome_search_pub, ChromeAllSearchOutcome,
};
#[cfg(feature = "chrome")]
pub(crate) use chrome::{
    execute_chrome_search, fill_chrome_agent_metadata, pre_flight_applies, resolved_chrome_metadata,
};

// GAP-COMP-002: pure zero-result classification lives in `zero_cause`.
// Re-export so existing `pipeline::classify_zero_result` call sites keep working.
pub use crate::zero_cause::{
    classify_zero_result, next_action_suggestion_for_zero, ZeroClassificationInputs,
};

/// Backwards-compatible alias — preserves the `execute` name used in the original `lib.rs`.
///
/// # Errors
///
/// Returns an error if the HTTP client cannot be built or if `execute_single_search`
/// fails unrecoverably (see that function's documentation for details).
///
/// # Cancel safety
///
/// This function is cancel-safe. It delegates directly to [`execute_single_search`]
/// with a fresh, never-cancelled [`CancellationToken`]; dropping the future is safe.
pub async fn execute(cfg: &Config) -> Result<SearchOutput, CliError> {
    execute_single_search(cfg, &CancellationToken::new()).await
}

/* moved to queries.rs — see pub use below */

#[cfg(test)]
mod tests;
