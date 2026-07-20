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

use crate::content_fetch;
use crate::error::CliError;
// GAP F4 v0.8.9: `extraction` is consumed only by Chrome-primary paths
// (`#[cfg(feature = "chrome")]`) — the import gate keeps the build
// `--no-default-features` sem warnings.
use crate::http;
// v0.7.10 B1 fix: removed `use crate::output;` — the early
// `print_line_stdout` call was deleted, so the import is unused.
use crate::parallel;
use crate::probe_deep;
use crate::search;
use crate::types::{
    Config, MultiSearchOutput, SearchMetadata, SearchOutput,
};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;


pub mod failure;
#[cfg(feature = "chrome")]
mod chrome;
pub mod queries;
pub(crate) use failure::{
    chrome_transport_failure_output, failure_output, news_only_chrome_failure_output,
};
#[cfg(test)]
pub(crate) use failure::{chrome_cancelled_error, failure_output_from_parts};
pub use queries::{
    combine_and_dedup_queries, read_queries_from_file, read_queries_from_stdin_if_pipe,
};
pub(crate) use queries::{calculate_selectors_hash, derive_cascade_level_from_attempts};

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
    if let Some(persistent_jar) = config.persistent_jar.as_ref() {
        persistent_jar.save();
    }
}

/// Performs the warm-up GET to the SERP origin to populate session cookies.
/// Failures are surfaced to the caller but never fatal; the caller logs and
/// continues. v0.7.3 PR2. Residual HTTP harness path.
///
/// URL comes from [`crate::endpoints::serp_base_url`] (env-overridable).
async fn do_warmup(client: &reqwest::Client, cfg: &Config) -> Result<(), CliError> {
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
            let resolved_format = match format {
                OutputFormat::Auto | OutputFormat::Json => OutputFormat::Json,
                outro => outro,
            };
            // GAP-PAR-040b: format/serde each stream item off the Tokio worker.
            let res = match resolved_format {
                OutputFormat::Json | OutputFormat::Auto => {
                    crate::output::emit_ndjson_async(output, output_file.clone()).await
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

/// Executes the full flow for a single-query search with pagination, retry and Lite fallback.
///
/// # Errors
///
/// Returns an error if the HTTP client cannot be built. Search failures (rate limit,
/// timeout, block) are captured in the returned [`SearchOutput`] error fields rather
/// than propagated as `Err`.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future aborts the in-flight HTTP
/// request; any partial pagination state is discarded without side effects.
pub async fn execute_single_search(
    cfg: &Config,
    cancellation: &CancellationToken,
) -> Result<SearchOutput, CliError> {
    // `cancellation` is wired through to chromiumoxide fallback and content_fetch
    // enrichment; reference it here to document its purpose and suppress warnings.
    let _ = cancellation;
    let start = Instant::now();

    let config_proxy = cfg.proxy_config.clone();

    // v0.7.10 GAP-WS-60 fix: when `--identity-profile` pins a specific
    // family+platform, build a fresh `BrowserProfile` from the matching
    // identity in the 12-identity pool. The original `cfg.browser_profile`
    // (built from `user-agents.toml` or embedded defaults) is discarded
    // for this single query — the session is fully pinned to the chosen
    // identity until the process exits. `Auto` (default) keeps the
    // legacy behavior (uses `cfg.browser_profile` directly).
    //
    // Also capture the effective UA + identity tag so the SearchMetadata
    // output reports what the request actually used, not the static
    // value of `cfg.user_agent` (which still reflects the original
    // `user-agents.toml` selection).
    // GAP F4 v0.8.9: reassignments of `effective_identity_tag` happen
    // only under `#[cfg(feature = "chrome")]` — same pattern as `chrome_result`.
    #[cfg_attr(not(feature = "chrome"), allow(unused_mut))]
    let (effective_profile, effective_user_agent, mut effective_identity_tag): (
        http::BrowserProfile,
        String,
        Option<String>,
    ) = match crate::identity::browser_profile_for_cli_identity(cfg.identity_profile, None) {
        Some(pinned) => {
            // Use the canonical tag from `IdentityProfile::tag()` via
            // `identity_tag_for_cli_identity` so the success path matches
            // the failure paths (`failure_output`, `error_output`).
            // Previously this was FNV-1a(UA) — same format
            // `<family>-<platform>-<16hex>` but different hex bytes; the
            // canonical seed is more stable across UA string tweaks.
            let tag = crate::identity::identity_tag_for_cli_identity(cfg.identity_profile, None)
                .unwrap_or_default();
            tracing::info!(
                identity_profile = ?cfg.identity_profile,
                pinned_ua = %pinned.user_agent,
                pinned_tag = %tag,
                "pinned to fixed identity per --identity-profile"
            );
            // Move the profile; clone only the UA string needed as a parallel field.
            let ua = pinned.user_agent.clone();
            (pinned, ua, Some(tag))
        }
        None => (cfg.browser_profile.clone(), cfg.user_agent.as_str().to_string(), None),
    };

    // GAP-TLS-014: residual reqwest Client only for http-test-harness.
    // Production Chrome-only path skips TLS pool / cookie jar construction.
    let residual_client = http::maybe_build_residual_client(
        &effective_profile,
        cfg.timeout_seconds.get(),
        cfg.language.as_str(),
        cfg.country.as_str(),
        &config_proxy,
        cfg.cookie_provider.clone(),
    )?;

    // GAP-WS-113: HTTP warm-up is residual harness-only. Production warm-up is
    // Chrome CDP (GAP-WS-077 inside browser launch / extract paths).
    if cfg.warmup_enabled {
        if let Some(ref client) = residual_client {
            if let Err(e) = do_warmup(client, cfg).await {
                tracing::warn!(error = %e, "warm-up request failed; continuing without it");
            }
        }
    }

    // v0.7.10 P5: probe-deep scheduler — when `cfg.pre_flight == true`,
    // run a minimal probe before the real search and short-circuit on
    // captcha/ghost-block so the operator does not waste a full
    // search round-trip on an already-blocked environment.
    // GAP F2 v0.8.9: o pre-flight sonda o endpoint HTML web via reqwest — um
    // sinal irrelevante (e potencialmente falso-positivo fatal) para a vertical
    // news, which is Chrome-only without HTTP fallback. A false positive would abort
    // the news search with exit 3 without ever trying it. The probe runs only when
    // execution includes the web vertical (`web` and `all`).
    if cfg.pre_flight && !pre_flight_applies(cfg) {
        tracing::info!(
            vertical = cfg.vertical.as_str(),
            "pre-flight nao se aplica a vertical news (Chrome-only, sem endpoint HTTP); probe pulado"
        );
    }
    // GAP-WS-113: production pre-flight runs on the SHARED Chrome SERP session
    // inside `execute_chrome_web_search_on_browser` (one launch per invocation).
    // Residual HTTP pre-flight remains harness-only below.
    if pre_flight_applies(cfg) {
        if let Some(ref client) = residual_client {
        let probe_started = std::time::Instant::now();
        let probe_result = client
            .post(crate::search::html_base_url())
            .form(&[("q", "the quick brown fox jumps over the lazy dog")])
            .send()
            .await;
        match probe_result {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = crate::decompress::response_body_string(response)
                    .await
                    .unwrap_or_default();
                let latency = probe_started
                    .elapsed()
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64;
                let outcome = probe_deep::classify_probe_outcome(&body, status, latency);
                if !outcome.healthy {
                    tracing::warn!(
                        marker = outcome.marker,
                        kind = outcome.kind.as_str(),
                        http_status = outcome.http_status,
                        latency_ms = outcome.latency_ms,
                        "pre-flight detected block; short-circuiting search"
                    );
                    // B1 fix: do NOT early-print via `print_line_stdout` —
                    // the caller in lib.rs already serializes the returned
                    // SearchOutput exactly once via `output::emit_result`.
                    // Printing here caused two JSON objects to be emitted
                    // back-to-back (broken pipe contract for `| jaq`).
                    // The pre-flight context (kind, marker, latency, message)
                    // travels inside the SearchOutput envelope below; the
                    // caller maps `error: Some("pre_flight_blocked")` to
                    // exit code 3 (anti-bot) instead of 0.
                    let mut pre = SearchOutput {
                        query: cfg.query.as_str().to_string(),
                        engine: "duckduckgo".to_string(),
                        endpoint: cfg.endpoint.as_str().to_string(),
                        timestamp: crate::types::utc_now(),
                        region: format!("{}-{}", cfg.country, cfg.language),
                        result_count: 0,
                        results: vec![],
                        pages_fetched: 0,
                        news: None,
                        news_count: None,
                        error: Some("pre_flight_blocked".to_string()),
                        message: Some(format!(
                            "pre-flight detected captcha/ghost-block via marker {}",
                            outcome.marker
                        )),
                        metadata: SearchMetadata {
                            execution_time_ms: outcome.latency_ms,
                            selectors_hash: "pre-flight".to_string(),
                            retries: 0,
                            retries_configured: None,
                            used_fallback_endpoint: false,
                            concurrent_fetches: 0,
                            fetch_successes: 0,
                            fetch_failures: 0,
                            used_chrome: false,
                            chrome_attempted: false,
                            user_agent: effective_user_agent.clone(),
                            used_proxy: config_proxy.is_active(),
                            identity_used: None,
                            cascade_level: None,
                            pre_flight_fired: true,
                            pre_flight_executed: true,
                            pre_flight_status: Some("blocked".into()),
                            news_promo_filtered: None,
                            stream_requested: None,
                            stream_effective: None,
                            zero_cause: None,
                            next_action_suggestion: None,
                            bytes_raw: None,
                            bytes_decompressed: None,
                            cascade_level_observed: None,
                            result_count_compat: None,
                            endpoint_used_compat: None,
                            vertical_used: Some(cfg.vertical.as_str().to_string()),
                            chrome_path_resolved: None,
                            chrome_channel: None,
                            run_id: Some(crate::types::RunId::generate()),
                        },
                    };
                    fill_chrome_agent_metadata(&mut pre.metadata, cfg);
                    return Ok(pre);
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "pre-flight request failed; continuing with real search");
            }
        }
        } // residual_client
    } // pre_flight_applies

    tracing::info!(query = %cfg.query, endpoint = cfg.endpoint.as_str(), "Executing search");

    // v0.8.0 / ADR-0016: Chrome-only production transport — native browser TLS stack
    // (avoids library TLS bot-class signatures blocked by Cloudflare; ADR-0022:
    // no synthetic hardware fingerprint spoof). Residual HTTP is harness-only.
    #[allow(unused_assignments, unused_mut)]
    let mut chrome_attempted = false;
    #[allow(unused_assignments, unused_mut)]
    let mut chrome_result_used = false;
    #[allow(unused_mut)]
    let mut chrome_result: Option<search::AggregatedSearchResult> = None;

    // GAP-WS-104 v0.8.9: resultado da vertical news (resultados + body bruto
    // rendered, consumed by zero-cause classification). `None` in
    // default web mode — the JSON contract remains byte-identical pre-v0.8.9.
    #[cfg(feature = "chrome")]
    let mut news_outcome: Option<(Vec<crate::types::NewsResult>, String, u32)> = None;

    // GAP-WS-113: production always requires Chrome. Harness may skip Chrome.
    if let Err(err) = crate::chrome_policy::require_chrome_transport() {
        if !crate::chrome_policy::http_test_harness_active() {
            return Ok(chrome_transport_failure_output(cfg, &err, start));
        }
    }

    #[cfg(feature = "chrome")]
    if !crate::chrome_policy::chrome_disabled_by_env()
        && !crate::chrome_policy::http_test_harness_active()
    {
        chrome_attempted = true;
        let chrome_ua = {
            let candidate =
                crate::identity::browser_profile_for_cli_identity(cfg.identity_profile, None)
                    .map(|p| p.user_agent) // owned profile — move the UA field
                    .unwrap_or_else(|| effective_user_agent.clone());
            crate::identity::coerce_chrome_user_agent(&candidate)
        };

        if cfg.vertical.includes_news() {
            // GAP-WS-104 / GAP-PAR-021: `--vertical news|all` via
            // `execute_chrome_all_search_pub` — dual multi-process Chromes
            // (web ∥ news) when budget ≥ 2; shared serial session when
            // `--shared-session-verticals` or `-p 1`. Multi-query pays
            // acquire_many(2) so peak Chrome OS ≤ effective (GAP-PAR-021b).
            // News is Chrome-only (`ia=news&iar=news`); cancel → exit 130/143.
            let outcome = execute_chrome_all_search_pub(cfg, &chrome_ua, cancellation).await?;
            // Chrome session was used for news and/or web (L-04 honest used_chrome).
            chrome_result_used = true;
            if let Some(result) = outcome.web {
                chrome_result = Some(result);
                // GAP-WS-095: populate identity_used from Chrome UA when Auto.
                if effective_identity_tag.is_none() {
                    effective_identity_tag = identity_tag_for_chrome_ua(&chrome_ua);
                }
            }
            match outcome.news {
                Ok(outcome_news) => news_outcome = Some(outcome_news),
                Err(err) => {
                    // Cooperative cancel must surface as exit 130/143, not as a
                    // structured zero-results envelope.
                    if matches!(err, CliError::Cancelled) {
                        return Err(err);
                    }
                    if !cfg.vertical.includes_web() {
                        // GAP F1 v0.8.9: news-only NUNCA propaga `Err` cru —
                        // `-f json` SEMPRE emite envelope JSON estruturado.
                        return Ok(news_only_chrome_failure_output(cfg, &err, start));
                    }
                    news_outcome = Some((Vec::new(), String::new(), 0));
                }
            }
        } else {
            match execute_chrome_search(cfg, &chrome_ua, cancellation).await {
                Ok(result) => {
                    tracing::info!(
                        chrome_results = result.results.len(),
                        "Chrome-primary search succeeded"
                    );
                    chrome_result = Some(result);
                    chrome_result_used = true;
                    // GAP-WS-095: populate identity_used from Chrome UA when Auto.
                    if effective_identity_tag.is_none() {
                        effective_identity_tag = identity_tag_for_chrome_ua(&chrome_ua);
                    }
                }
                Err(err) => {
                    // Cooperative cancel → Err(Cancelled) → exit 130/143.
                    if matches!(err, CliError::Cancelled) {
                        return Err(err);
                    }
                    // GAP-WS-113: never fall back to reqwest — structured failure.
                    // Pre-flight on shared session returns Blocked → pre_flight envelope.
                    if matches!(err, CliError::Blocked) {
                        tracing::warn!("Chrome shared-session pre-flight blocked (GAP-WS-113)");
                        let mut out = chrome_transport_failure_output(cfg, &err, start);
                        out.error = Some("pre_flight_blocked".to_string());
                        out.metadata.pre_flight_fired = true;
                        out.metadata.pre_flight_executed = true;
                        out.metadata.pre_flight_status = Some("blocked".into());
                        out.metadata.used_chrome = true;
                        out.metadata.chrome_attempted = true;
                        out.metadata.next_action_suggestion = Some(
                            "Pre-flight Chrome detected a block (GAP-WS-113). Wait 300s or use --proxy."
                                .to_string(),
                        );
                        return Ok(out);
                    }
                    tracing::error!(
                        error = %err,
                        "Chrome-primary search failed — HTTP fallback removed (GAP-WS-113)"
                    );
                    return Ok(chrome_transport_failure_output(cfg, &err, start));
                }
            }
        }
    }

    #[allow(unused_mut)]
    let mut agregado = if let Some(cr) = chrome_result {
        cr
    } else if !cfg.vertical.includes_web() {
        // GAP-WS-104: news-only — web pipeline intentionally empty.
        search::AggregatedSearchResult {
            results: Vec::new(),
            first_body: String::new(),
            pages_fetched: 0,
            attempts: 1,
            used_fallback_lite: false,
            effective_endpoint: crate::types::Endpoint::Html,
            bytes_in: 0,
            bytes_out: 0,
        }
    } else if let Some(ref client) = residual_client {
        // Residual HTTP path for wiremock tests only (feature http-test-harness).
        let flag_rate_limit = Arc::new(AtomicBool::new(false));
        let search_result = search::search_with_pagination(
            client,
            cfg,
            cfg.query.as_str(),
            &flag_rate_limit,
            cancellation,
        )
        .await;
        let failure_output_val = match &search_result {
            Err(reason) if reason.is_cancellation() => {
                // HTTP harness cancel → typed Cancelled → exit 130/143.
                return Err(CliError::Cancelled);
            }
            Err(reason) => Some(failure_output(cfg, reason, start)),
            Ok(_) => None,
        };
        if let Some(out) = failure_output_val {
            return Ok(out);
        }
        search_result.map_err(|reason| CliError::PipelineInvariantViolation {
            message: format!(
                "search_result reached extract_ok_path with Err after early return; reason={reason:?}"
            ),
        })?
    } else {
        // GAP-WS-113: Chrome did not produce a web result and harness is off.
        let err = CliError::InvalidConfig {
            message: "Chrome transport did not return SERP results (GAP-WS-113).                       Install Chrome/Chromium or pass --chrome-path."
                .into(),
        };
        return Ok(chrome_transport_failure_output(cfg, &err, start));
    };

    // GAP-WS-090: truncate results to --num when Chrome headed returns a full
    // page (typically 10). Without this, --num is silently ignored.
    if let Some(max) = cfg.num_results.map(|n| n.get()) {
        let max = max as usize;
        if agregado.results.len() > max {
            agregado.results.truncate(max);
        }
    }

    let quantidade = u32::try_from(agregado.results.len()).unwrap_or(u32::MAX);
    let selectors_hash = calculate_selectors_hash(&cfg.selectors);
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = crate::types::utc_now();
    let run_id = crate::types::RunId::generate();
    // Retries = attempts - 1 (the first request does not count as a retry).
    let retries_count = agregado.attempts.saturating_sub(1);

    // GAP-AUD-002 + GAP-AUD-010 v0.8.0: cascade_level_observed deve refletir
    // the cascade level actually exercised. We prefer the cache from
    // probe-deep (`cfg.last_probe_cascade_level`) when available (case
    // --pre-flight within the same process invocation). Otherwise,
    // we derive from the observable signal: 1 retry with fallback = level 1;
    // 2+ retries = level 2+. Without retries = level 0.
    let cascade_level_observed = cfg
        .last_probe_cascade_level
        .or_else(|| Some(derive_cascade_level_from_attempts(&agregado)));

    let mut metadata_val = SearchMetadata {
        execution_time_ms: elapsed_ms,
        selectors_hash,
        retries: retries_count,
        retries_configured: Some(cfg.retries.get()),
        used_fallback_endpoint: agregado.used_fallback_lite,
        concurrent_fetches: 0,
        fetch_successes: 0,
        fetch_failures: 0,
        used_chrome: chrome_result_used,
        chrome_attempted,
        user_agent: effective_user_agent.clone(),
        used_proxy: config_proxy.is_active(),
        identity_used: effective_identity_tag.clone(),
        cascade_level: None,
        pre_flight_fired: false,
        // GAP-WS-PREFLIGHT-META-001: executed when flag on and web path applies.
        pre_flight_executed: cfg.pre_flight && cfg.vertical.includes_web(),
        pre_flight_status: if cfg.pre_flight && cfg.vertical.includes_web() {
            Some("ok".into())
        } else {
            None
        },
        news_promo_filtered: None,
        stream_requested: if cfg.stream_mode { Some(true) } else { None },
        stream_effective: if cfg.stream_mode {
            Some(false) // single-query stream is ignored
        } else {
            None
        },
        zero_cause: None,
        next_action_suggestion: None,
        // GAP-NEW-002 v0.8.0: HTTP decompression byte counters. When
        // , the compression ratio is
        // . When iguais,
        // o body veio como identity (sem encoding) ou via .
        bytes_raw: Some(agregado.bytes_in),
        bytes_decompressed: Some(agregado.bytes_out),
        cascade_level_observed,
        result_count_compat: None,
        endpoint_used_compat: None,
        // GAP-WS-104: `None` no modo web default preserva o contrato JSON
        // byte-identical pre-v0.8.9 (`skip_serializing_if`).
        vertical_used: Some(cfg.vertical.as_str().to_string()),
        chrome_path_resolved: None,
        chrome_channel: None,
        run_id: Some(run_id),
    };
    fill_chrome_agent_metadata(&mut metadata_val, cfg);

    // GAP-AUD-003 v0.8.0: classificar zero-result causalmente.
    // Only runs on the zero path (`quantidade == 0`) to avoid cost on success.
    // GAP-WS-104: in news-only mode the web pipeline does not run — classification
    // de zero passa a ser responsabilidade do bloco news abaixo.
    if quantidade == 0 && cfg.vertical.includes_web() {
        let inputs = ZeroClassificationInputs {
            body: &agregado.first_body,
            pre_flight_enabled: cfg.pre_flight,
            pre_flight_fired: false,
            execution_time_ms: metadata_val.execution_time_ms,
            retries: metadata_val.retries,
            concurrent_fetches: metadata_val.concurrent_fetches,
            last_probe_cascade_level: cfg.last_probe_cascade_level,
        };
        let cause = classify_zero_result(&inputs);
        metadata_val.zero_cause = Some(cause);
        metadata_val.next_action_suggestion =
            next_action_suggestion_for_zero(cause).map(str::to_string);
    }

    // GAP-NEW-004 v0.8.0: lite auto-fallback — wire-in happens after
    // construction of  (see block below).

    let mut output = SearchOutput {
        query: cfg.query.as_str().to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: agregado.effective_endpoint.as_str().to_string(),
        timestamp,
        region: search::format_kl(cfg.language.as_str(), cfg.country.as_str()),
        result_count: quantidade,
        results: agregado.results,
        pages_fetched: agregado.pages_fetched,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: metadata_val,
    };

    // GAP-WS-104 v0.8.9: wiring of the news vertical into the envelope. Populates
    // `noticias`/`quantidade_noticias` SOMENTE quando `--vertical news|all`
    // ran (in default web mode `news_outcome` is `None` — contract
    // byte-identical). Cap `--num` with the same GAP-WS-090 web pattern.
    #[cfg(feature = "chrome")]
    if let Some((mut news_results, news_body, promo_filtered)) = news_outcome.take() {
        if let Some(max) = cfg.num_results.map(|n| n.get()) {
            let max = max as usize;
            if news_results.len() > max {
                news_results.truncate(max);
            }
        }
        let news_quantidade = u32::try_from(news_results.len()).unwrap_or(u32::MAX);
        output.news = Some(news_results);
        output.news_count = Some(news_quantidade);
        if promo_filtered > 0 {
            output.metadata.news_promo_filtered = Some(promo_filtered);
        }

        // Zero news: interstitial anti-bot no body renderizado ⇒ AntiBot;
        // otherwise ⇒ VerticalNoResults (LEGITIMATE zero ⇒ exit 5, not 6).
        // Precedence rules:
        // - modo all com web>0: sucesso segue a web — zero_cause fica None;
        // - all mode with web==0: web classification (more informative) already
        //   ran and is preserved (`zero_cause.is_none()` fails);
        // - news-only: web classification was skipped — this block decides.
        if news_quantidade == 0 && output.result_count == 0 && output.metadata.zero_cause.is_none()
        {
            let cause = if crate::probe_deep::detect_interstitial(&news_body)
                != crate::probe_deep::InterstitialKind::None
            {
                crate::types::ZeroCause::AntiBot
            } else {
                crate::types::ZeroCause::VerticalNoResults
            };
            output.metadata.zero_cause = Some(cause);
            output.metadata.next_action_suggestion =
                next_action_suggestion_for_zero(cause).map(str::to_string);
        }

        // GAP F3 v0.8.9: no modo `all` com web>0, `causa_zero` permanece `None`
        // by semantics — the field describes the envelope total zero — and the
        // JSON contract does NOT gain new fields. To avoid discarding the
        // news block diagnosis silently, the warning goes to stderr
        // via `tracing::warn` (fora do stdout JSON).
        if news_quantidade == 0
            && output.result_count > 0
            && crate::probe_deep::detect_interstitial(&news_body)
                != crate::probe_deep::InterstitialKind::None
        {
            tracing::warn!(
                next_action_suggestion = "news vertical blocked by anti-bot interstitial; \
                 web returned results. Wait 300s and re-run with --vertical news, \
                 or use --proxy to rotate the egress IP.",
                "news vertical blocked by anti-bot in all mode — empty news"
            );
        }
    }

    // GAP-WS-113: auto-fallback Lite permanently removed (was GAP-NEW-004).

    // Enriquecimento opcional via --fetch-content (iter. 5).
    // Residual Client is Some only under http-test-harness; Chrome path uses CDP.
    content_fetch::enrich_with_content(&mut output, residual_client.as_ref(), cfg, cancellation)
        .await;
    // GAP-WS-META-TIMING-001: wall clock includes content fetch.
    output.metadata.execution_time_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;

    tracing::info!(
        total = output.result_count,
        pages = output.pages_fetched,
        fallback = output.metadata.used_fallback_endpoint,
        fetch_content = cfg.fetch_content,
        fetch_successes = output.metadata.fetch_successes,
        "Search completed successfully"
    );
    Ok(output)
}

#[cfg(feature = "chrome")]
pub use chrome::{
    execute_chrome_all_search_pub, execute_chrome_news_search, execute_chrome_news_search_on_browser,
    execute_chrome_search_pub, ChromeAllSearchOutcome,
};
#[cfg(feature = "chrome")]
pub(crate) use chrome::{
    execute_chrome_search, fill_chrome_agent_metadata, identity_tag_for_chrome_ua,
    pre_flight_applies, resolved_chrome_metadata,
};


// GAP-COMP-002: pure zero-result classification lives in `zero_cause`.
// Re-export so existing `pipeline::classify_zero_result` call sites keep working.
pub use crate::zero_cause::{
    ZeroClassificationInputs, classify_zero_result, next_action_suggestion_for_zero,
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
mod tests {
    use super::*;
    use crate::types::{SelectorConfig, ZeroCause};
    use std::collections::BTreeMap;

    #[test]
    fn calculate_selectors_hash_returns_16_chars() {
        let cfg = SelectorConfig::default();
        let hash = calculate_selectors_hash(&cfg);
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn calculate_selectors_hash_is_deterministic() {
        let cfg = SelectorConfig::default();
        let h1 = calculate_selectors_hash(&cfg);
        let h2 = calculate_selectors_hash(&cfg);
        assert_eq!(h1, h2);
    }

    #[test]
    fn combinar_deduplica_preservando_ordem_da_primeira_ocorrencia() {
        let posicionais = vec!["alfa".to_string(), "beta".to_string()];
        let de_arquivo = vec!["beta".to_string(), "gama".to_string()];
        let de_stdin = vec!["alfa".to_string(), "delta".to_string()];
        let combinado = combine_and_dedup_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(
            combinado,
            vec!["alfa", "beta", "gama", "delta"],
            "ordem deve ser da primeira ocorrência; duplicatas devem ser removidas"
        );
    }

    #[test]
    fn combinar_remove_strings_vazias_e_apenas_espacos() {
        let posicionais = vec!["   ".to_string(), "rust".to_string(), "".to_string()];
        let de_arquivo = vec!["\t\t".to_string(), "tokio".to_string()];
        let de_stdin = vec![];
        let combinado = combine_and_dedup_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(combinado, vec!["rust", "tokio"]);
    }

    #[test]
    fn combine_trims_whitespace_before_comparing() {
        let posicionais = vec!["  alfa  ".to_string()];
        let de_arquivo = vec!["alfa".to_string()];
        let de_stdin = vec!["alfa\t".to_string()];
        let combinado = combine_and_dedup_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(
            combinado,
            vec!["alfa"],
            "queries equivalentes after trim devem ser deduplicadas"
        );
    }

    #[test]
    fn combine_empty_returns_empty() {
        let combinado = combine_and_dedup_queries(vec![], vec![], vec![]);
        assert!(combinado.is_empty());
    }

    #[test]
    fn read_queries_from_file_accepts_windows_lines_and_empty() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("ddg_cli_iter2_queries_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("queries.txt");
        let content = "rust\r\ntokio\r\n\r\n  axum  \n\nhttp://exemplo.com\n";
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        drop(file);

        let lines = read_queries_from_file(&path).expect("should read file");
        assert_eq!(lines, vec!["rust", "tokio", "axum", "http://exemplo.com"]);
        // Cleanup best-effort.
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn total_results_in_single_output() {
        let output = SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: crate::types::test_timestamp(),
            region: "br-pt".into(),
            result_count: 7,
            results: vec![],
            pages_fetched: 1,
            news: None,
            news_count: None,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "x".into(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                chrome_attempted: false,
                user_agent: "ua".into(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
                pre_flight_fired: false,
                pre_flight_executed: false,
                pre_flight_status: None,
                news_promo_filtered: None,
                stream_requested: None,
                stream_effective: None,
                zero_cause: None,
                next_action_suggestion: None,
                bytes_raw: None,
                bytes_decompressed: None,
                cascade_level_observed: None,
                result_count_compat: None,
                endpoint_used_compat: None,
                vertical_used: None,
                chrome_path_resolved: None,
                chrome_channel: None,
                    ..Default::default()
                },
        };
        assert_eq!(PipelineResult::Single(Box::new(output)).total_results(), 7);
    }

    // GAP-WS-104 v0.8.9: total_results soma news_count — news-only com
    // news encontradas ⇒ exit 0; without news ⇒ exit 5.
    #[test]
    fn total_results_sums_news_count() {
        let output = SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: crate::types::test_timestamp(),
            region: "br-pt".into(),
            result_count: 0,
            results: vec![],
            pages_fetched: 0,
            news: Some(vec![]),
            news_count: Some(4),
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "x".into(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: true,
                chrome_attempted: true,
                user_agent: "ua".into(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
                pre_flight_fired: false,
                pre_flight_executed: false,
                pre_flight_status: None,
                news_promo_filtered: None,
                stream_requested: None,
                stream_effective: None,
                zero_cause: None,
                next_action_suggestion: None,
                bytes_raw: None,
                bytes_decompressed: None,
                cascade_level_observed: None,
                result_count_compat: None,
                endpoint_used_compat: None,
                vertical_used: Some("news".into()),
                chrome_path_resolved: None,
                chrome_channel: None,
                    ..Default::default()
                },
        };
        assert_eq!(PipelineResult::Single(Box::new(output)).total_results(), 4);
    }

    // =====================================================================
    // Fim dos testes do classificador GAP-AUD-003.
    // =====================================================================

    // =====================================================================
    // GAP F1/F2/F5 v0.8.9 — envelope de falha news-only, gate do pre-flight
    // e erro de cancelamento do transporte Chrome.
    // =====================================================================

    fn cfg_para_vertical(pre_flight: bool, vertical: crate::types::VerticalMode) -> Config {
        let mut cfg = Config::default();
        cfg.query = crate::security::ValidatedQuery::try_new("assunto").expect("q");
        cfg.queries = vec![cfg.query.clone()];
        cfg.pre_flight = pre_flight;
        cfg.vertical = vertical;
        cfg.fetch_content = false;
        cfg
    }

    #[test]
    fn pre_flight_aplica_somente_quando_vertical_inclui_web() {
        assert!(pre_flight_applies(&cfg_para_vertical(
            true,
            crate::types::VerticalMode::Web
        )));
        assert!(pre_flight_applies(&cfg_para_vertical(
            true,
            crate::types::VerticalMode::All
        )));
        assert!(
            !pre_flight_applies(&cfg_para_vertical(true, crate::types::VerticalMode::News)),
            "news-only deve pular o pre-flight (Chrome-only, sem endpoint HTTP)"
        );
        assert!(!pre_flight_applies(&cfg_para_vertical(
            false,
            crate::types::VerticalMode::Web
        )));
    }

    #[cfg(feature = "chrome")]
    #[test]
    fn news_only_chrome_failure_output_emite_envelope_estruturado() {
        let mut cfg = Config::default();
        cfg.query = crate::security::ValidatedQuery::try_new("assunto").expect("q");
        cfg.queries = vec![cfg.query.clone()];
        cfg.fetch_content = false;
        cfg.vertical = crate::types::VerticalMode::News;
        let err = CliError::InvalidConfig {
            message: "Chrome not detected: binario ausente".to_string(),
        };
        let out = news_only_chrome_failure_output(&cfg, &err, Instant::now());
        assert_eq!(out.result_count, 0);
        assert!(out.results.is_empty());
        assert_eq!(out.news_count, Some(0));
        assert_eq!(out.news.as_ref().map(Vec::len), Some(0));
        assert_eq!(
            out.error.as_deref(),
            Some(crate::error::codes::INVALID_CONFIG)
        );
        assert!(out
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("Chrome"));
        assert_eq!(out.metadata.zero_cause, Some(ZeroCause::InvalidResponse));
        assert!(out.metadata.chrome_attempted);
        assert_eq!(out.metadata.vertical_used.as_deref(), Some("news"));
        assert!(out.metadata.next_action_suggestion.is_some());
    }

    #[cfg(feature = "chrome")]
    #[test]
    fn chrome_cancelled_error_unifica_cancelled_exit_130() {
        let err = chrome_cancelled_error("news search");
        assert!(matches!(err, CliError::Cancelled));
        assert_eq!(err.error_code(), crate::error::codes::CANCELLED);
        assert_eq!(err.exit_code(), crate::error::exit_codes::CANCELLED);
        assert!(err.to_string().contains("cancelled"));
    }

    #[test]
    fn total_results_in_multi_output_sums_all() {
        let nova_saida = |n: u32| SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: crate::types::test_timestamp(),
            region: "br-pt".into(),
            result_count: n,
            results: vec![],
            pages_fetched: 1,
            news: None,
            news_count: None,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "x".into(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                chrome_attempted: false,
                user_agent: "ua".into(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
                pre_flight_fired: false,
                pre_flight_executed: false,
                pre_flight_status: None,
                news_promo_filtered: None,
                stream_requested: None,
                stream_effective: None,
                zero_cause: None,
                next_action_suggestion: None,
                bytes_raw: None,
                bytes_decompressed: None,
                cascade_level_observed: None,
                result_count_compat: None,
                endpoint_used_compat: None,
                vertical_used: None,
                chrome_path_resolved: None,
                chrome_channel: None,
                    ..Default::default()
                },
        };
        let multi = MultiSearchOutput {
            query_count: 3,
            timestamp: crate::types::test_timestamp(),
            parallelism: 3,
            searches: vec![nova_saida(2), nova_saida(5), nova_saida(0)],
            causa_zero_histogram: BTreeMap::new(),
        };
        assert_eq!(PipelineResult::Multi(Box::new(multi)).total_results(), 7);
    }
}
