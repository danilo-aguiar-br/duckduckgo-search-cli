// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound multi-query fan-out (JoinSet + Semaphore).
// Parallelism: bounded by `--parallel` / `--max-concurrency` (1..=20).
// CPU offload: SERP extract/emit via `concurrency::run_cpu_bound` (GAP-PAR-030/040).
//! Multi-query parallelism with `JoinSet`, `Semaphore`, staggered launch and `CancellationToken`.
//!
//! Implementation of iteration 2 per sections 4.1–4.6, 13 and 15.8 of the specification.
//!
//! Key contracts:
//! - `Semaphore` limits concurrency to the `--parallel` value (1..=20).
//! - Staggered launch adds `index * 200ms + jitter(0..300ms)` BEFORE the spawn
//!   to avoid a synchronous burst that would trigger rate-limiting.
//! - `CancellationToken` is checked between stages of each task; when SIGINT
//!   fires, in-flight tasks abort gracefully with a `cancelled` error.
//! - Failure of one task does NOT abort the entire `JoinSet`. Other tasks continue.
//!   Failed queries produce a `SearchOutput` with the `error` field filled in.
//! - Client-per-query decision (cookie jar isolation) follows section 4.3:
//!   `paginas == 1` → shared; `paginas > 1` → new Client per query.

// Workload classification: I/O-bound (HTTP scraping against DuckDuckGo).
// Bottleneck: network latency per request (~200-800ms round-trip).
// Saturated resource: outbound HTTP connections + DuckDuckGo rate limits.
// Parallelism removes the latency bottleneck; semaphore prevents connection exhaustion.
//
// Trade-offs chosen:
// - Staggered launch (200ms base + 0-300ms jitter) reduces burst throughput
//   by ~15-20% but prevents DuckDuckGo rate-limit triggers.
// - JoinSet with ordered collection: O(n) memory for all results vs streaming,
//   but preserves input order for deterministic JSON output.
// - Per-host semaphore (content_fetch.rs): limits per-domain concurrency at
//   cost of underutilizing global permits when hosts are diverse.
//
// Expected failure scenarios and their handling:
// - Rate-limit cascade: one HTTP 429 sets AtomicBool flag → all tasks add
//   random delay (500-1200ms) on next attempt. Flag uses Ordering::Relaxed
//   (best-effort, see justification in search.rs).
// - Cancel mid-flight: CancellationToken checked at 3 points — (1) before
//   staggered delay, (2) after acquire_owned, (3) inside select! on HTTP.
//   Partial results preserved; cancelled queries get error SearchOutput.
// - Consumer close (streaming): send() fails → abort_all() kills remaining
//   tasks → function returns StreamStats with consistent counters.
// - Task panic: permit recovered via RAII drop. JoinError differentiated
//   via is_panic() (logged as error) vs is_cancelled() (logged as warn).

//!
//! ## Layout (GAP-E2E-51-011 — SRP split)
//!
//! | Submodule | Responsibility |
//! |-----------|----------------|
//! | [`batch`] | Ordered multi-query fan-out ([`execute_parallel_searches`]) |
//! | [`stream`] | Streaming fan-out + [`StreamStats`] |

mod batch;
mod stream;

pub use batch::execute_parallel_searches;
pub use stream::{execute_parallel_searches_streaming, StreamStats};

use crate::content_fetch;
use crate::error::CliError;
use crate::search;
use crate::types::{Config, SearchMetadata, SearchOutput};
use reqwest::Client;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// Base delay per index (milliseconds) for staggered launch.
pub(super) const DELAY_BASE_STAGGERED_MS: u64 = 200;

/// Maximum additional jitter (milliseconds) for staggered launch.
pub(super) const MAX_STAGGERED_JITTER_MS: u64 = 300;


/// Executes ONE query with pagination, retry, Lite fallback and fetch-content (if enabled).
pub(super) async fn execute_query_with_cancellation(
    query: &crate::security::ValidatedQuery,
    client: Option<&Client>,
    config: &Config,
    flag_rate_limit: &Arc<AtomicBool>,
    cancellation: &CancellationToken,
) -> Result<SearchOutput, CliError> {
    let start = Instant::now();

    if cancellation.is_cancelled() {
        return Err(CliError::Cancelled);
    }

    tracing::info!(query = %query, endpoint = config.endpoint.as_str(), "sending request");

    // Create a copy with the query overridden for `search_with_pagination`.
    let mut cfg_task = config.clone();
    // Multi-query fan-out only uses already-validated list entries (GAP-SECDEV-009).
    cfg_task.query = query.clone();

    // GAP-WS-113: Chrome-only in fan-out. Harness may use residual HTTP below.
    // Never swallow Chrome errors with `.ok()`.
    #[cfg(feature = "chrome")]
    let (chrome_result, news_outcome) = if crate::chrome_policy::http_test_harness_active()
        || crate::chrome_policy::chrome_disabled_by_env()
    {
        (None, None)
    } else {
        let chrome_ua = {
            let candidate =
                crate::identity::browser_profile_for_cli_identity(config.identity_profile, None)
                    .map(|p| p.user_agent) // owned profile — move the UA field
                    .unwrap_or_else(|| config.user_agent.as_str().to_string());
            crate::identity::coerce_chrome_user_agent(&candidate)
        };
        if cfg_task.vertical.includes_news() {
            let outcome =
                crate::pipeline::execute_chrome_all_search_pub(&cfg_task, &chrome_ua, cancellation)
                    .await?;
            let news = match outcome.news {
                Ok(news_ok) => Some(news_ok),
                Err(err) => {
                    if !cfg_task.vertical.includes_web() {
                        return Err(err);
                    }
                    // GAP-WS-113: no HTTP degradation — news absent signals failure.
                    tracing::error!(error = %err, "news vertical failed in fan-out (no HTTP fallback)");
                    None
                }
            };
            if cfg_task.vertical.includes_web() && outcome.web.is_none() {
                return Err(CliError::InvalidConfig {
                    message:
                        "Chrome fan-out failed for web SERP (GAP-WS-113); HTTP fallback removed"
                            .into(),
                });
            }
            (outcome.web, news)
        } else {
            match crate::pipeline::execute_chrome_search_pub(&cfg_task, &chrome_ua, cancellation)
                .await
            {
                Ok(result) => (Some(result), None),
                Err(err) => {
                    // GAP-WS-113: propagate — never `.ok()` into HTTP soft-block.
                    return Err(err);
                }
            }
        }
    };
    #[cfg(not(feature = "chrome"))]
    let (chrome_result, news_outcome): (
        Option<search::AggregatedSearchResult>,
        Option<(Vec<crate::types::NewsResult>, String, u32)>,
    ) = (None, None);

    let chrome_used = chrome_result.is_some() || news_outcome.is_some();
    let chrome_attempted = cfg!(feature = "chrome")
        && !crate::chrome_policy::chrome_disabled_by_env()
        && !crate::chrome_policy::http_test_harness_active();

    let mut agregado = if let Some(cr) = chrome_result {
        cr
    } else if !config.vertical.includes_web() {
        // GAP-WS-105: news-only no fan-out — o pipeline web (Chrome e
        // reqwest) is skipped; `resultados` stays empty by contract (same
        // semantics as the single-query path in `pipeline.rs`).
        search::AggregatedSearchResult {
            results: Vec::new(),
            first_body: String::new(),
            pages_fetched: 0,
            attempts: 1,
            used_fallback_lite: false,
            effective_endpoint: config.endpoint,
            bytes_in: 0,
            bytes_out: 0,
        }
    } else if let Some(http_client) = client {
        match search::search_with_pagination(
            http_client,
            &cfg_task,
            query.as_str(),
            flag_rate_limit,
            cancellation,
        )
        .await
        {
            Ok(a) => a,
            Err(reason) => {
                let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
                let timestamp = crate::types::utc_now();
                let run_id = crate::types::RunId::generate();
                let selectors_hash = crate::pipeline::calculate_selectors_hash(&config.selectors);
                let used_proxy =
                    config.proxy_config.clone().is_active();
                let identity_used_early =
                    crate::identity::identity_tag_for_cli_identity(config.identity_profile, None);
                let mut early = SearchOutput {
                    query: query.to_string(),
                    engine: "duckduckgo".to_string(),
                    endpoint: config.endpoint.as_str().to_string(),
                    timestamp,
                    region: search::format_kl(config.language.as_str(), config.country.as_str()),
                    result_count: 0,
                    results: Vec::new(),
                    pages_fetched: 0,
                    news: None,
                    news_count: None,
                    error: Some(reason.as_error_code().to_string()),
                    message: Some(reason.message()),
                    metadata: SearchMetadata {
                        execution_time_ms: elapsed_ms,
                        selectors_hash,
                        retries: config.retries.get(),
                        retries_configured: Some(config.retries.get()),
                        used_fallback_endpoint: false,
                        concurrent_fetches: 0,
                        fetch_successes: 0,
                        fetch_failures: 0,
                        used_chrome: false,
                        chrome_attempted,
                        user_agent: config.user_agent.as_str().to_string(),
                        used_proxy,
                        identity_used: identity_used_early,
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
                        vertical_used: Some(config.vertical.as_str().to_string()),
                        chrome_path_resolved: None,
                        chrome_channel: None,
                        run_id: Some(run_id),
                    },
                };
                crate::pipeline::fill_chrome_agent_metadata(&mut early.metadata, config);
                return Ok(early);
            }
        }
    } else {
        return Err(CliError::InvalidConfig {
            message: "Chrome transport required for parallel/deep-research fan-out (GAP-WS-113); HTTP fallback removed. Install Chrome or pass --chrome-path."
                .into(),
        });
    };

    // GAP-WS-094: truncate results to --num in batch/parallel path too.
    if let Some(max) = config.num_results.map(|n| n.get()) {
        let max = max as usize;
        if agregado.results.len() > max {
            agregado.results.truncate(max);
        }
    }

    let quantidade = u32::try_from(agregado.results.len()).unwrap_or(u32::MAX);
    let selectors_hash = crate::pipeline::calculate_selectors_hash(&config.selectors);
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = crate::types::utc_now();
    let run_id = crate::types::RunId::generate();
    let retries_count = agregado.attempts.saturating_sub(1);

    let used_proxy =
        config.proxy_config.clone().is_active();
    let identity_used =
        crate::identity::identity_tag_for_cli_identity(config.identity_profile, None);
    let mut metadata_val = SearchMetadata {
        execution_time_ms: elapsed_ms,
        selectors_hash,
        retries: retries_count,
        retries_configured: Some(config.retries.get()),
        used_fallback_endpoint: agregado.used_fallback_lite,
        concurrent_fetches: 0,
        fetch_successes: 0,
        fetch_failures: 0,
        used_chrome: chrome_used,
        chrome_attempted,
        user_agent: config.user_agent.as_str().to_string(),
        used_proxy,
        identity_used,
        cascade_level: None,
        pre_flight_fired: false,
        pre_flight_executed: false,
        pre_flight_status: None,
        news_promo_filtered: None,
        stream_requested: None,
        stream_effective: None,
        zero_cause: None,
        next_action_suggestion: None,
        // GAP-NEW-002 v0.8.0: HTTP decompression byte counters.
        bytes_raw: Some(agregado.bytes_in),
        bytes_decompressed: Some(agregado.bytes_out),
        cascade_level_observed: config.last_probe_cascade_level.or_else(|| {
            Some(crate::pipeline::derive_cascade_level_from_attempts(
                &agregado,
            ))
        }),
        result_count_compat: None,
        endpoint_used_compat: None,
        // GAP-WS-105 / v0.9.8: vertical + agent chrome metadata on every envelope.
        vertical_used: Some(config.vertical.as_str().to_string()),
        chrome_path_resolved: None,
        chrome_channel: None,
        run_id: Some(run_id),
    };
    crate::pipeline::fill_chrome_agent_metadata(&mut metadata_val, config);

    // GAP-AUD-003 v0.8.0: classificar zero-result causalmente no path paralelo.
    // GAP-WS-105: in news-only mode the web pipeline does not run — the
    // web classification is skipped (same gate as single-query).
    if quantidade == 0 && config.vertical.includes_web() {
        let inputs = crate::pipeline::ZeroClassificationInputs {
            body: &agregado.first_body,
            pre_flight_enabled: config.pre_flight,
            pre_flight_fired: false,
            execution_time_ms: metadata_val.execution_time_ms,
            retries: metadata_val.retries,
            concurrent_fetches: metadata_val.concurrent_fetches,
            last_probe_cascade_level: config.last_probe_cascade_level,
        };
        let cause = crate::pipeline::classify_zero_result(&inputs);
        metadata_val.zero_cause = Some(cause);
        metadata_val.next_action_suggestion =
            crate::pipeline::next_action_suggestion_for_zero(cause).map(str::to_string);
    }

    let mut output = SearchOutput {
        query: query.to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: agregado.effective_endpoint.as_str().to_string(),
        timestamp,
        region: search::format_kl(config.language.as_str(), config.country.as_str()),
        result_count: quantidade,
        results: agregado.results,
        pages_fetched: agregado.pages_fetched,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: metadata_val,
    };

    // GAP-WS-105 v0.8.9: wiring of the news vertical into the per-sub-query envelope.
    // Popula `noticias`/`quantidade_noticias` SOMENTE quando a SERP news
    // ran (`news_outcome` is `None` in web mode and when Chrome fell
    // in flight). Cap `--num` with the same GAP-WS-090 web pattern.
    if let Some((mut news_results, _news_body, promo_filtered)) = news_outcome {
        if let Some(max) = config.num_results.map(|n| n.get()) {
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
    }

    // Optional --fetch-content enrich. Nested under multi-query fan-out so
    // chrome pool_size = 1 (GAP-PAR-016: peak Chrome OS processes ≤ effective).
    content_fetch::enrich_with_content_opts(
        &mut output,
        client,
        config,
        cancellation,
        content_fetch::EnrichOptions {
            nested_in_query_fanout: true,
        },
    )
    .await; // client: Option<&Client> — None on Chrome-only production path
    // Note: parallel path uses per-query wall time already in metadata; fetch
    // extends wall clock — re-stamp from a local Instant if available.
    // Best-effort: add nothing if no Instant; single-query path owns the full fix.

    Ok(output)
}

/// Generates a `SearchOutput` representing a failed query.
///
/// Preserves the position in the multi-query output even when an individual query failed.
#[cold]
pub(super) fn error_output(index: usize, err: &CliError, config: &Config) -> SearchOutput {
    let query_ref = config
        .queries
        .get(index)
        .map(|q| q.as_str().to_string())
        .unwrap_or_default();
    let message = format!("{err:#}");
    let timestamp = crate::types::utc_now();
    let run_id = crate::types::RunId::generate();
    let selectors_hash = crate::pipeline::calculate_selectors_hash(&config.selectors);

    // GAP-AUD-001: propagate the pinned identity tag so a multi-query
    // failure output can be correlated to the identity that was selected.
    // Reuses `IdentityProfile::tag()` via the canonical helper.
    let identity_used =
        crate::identity::identity_tag_for_cli_identity(config.identity_profile, None);

    let mut out = SearchOutput {
        query: query_ref,
        engine: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp,
        region: search::format_kl(config.language.as_str(), config.country.as_str()),
        result_count: 0,
        results: Vec::new(),
        pages_fetched: 0,
        news: None,
        news_count: None,
        error: Some(err.error_code().to_string()),
        message: Some(message),
        metadata: SearchMetadata {
            execution_time_ms: 0,
            selectors_hash,
            retries: config.retries.get(),
            retries_configured: Some(config.retries.get()),
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            chrome_attempted: false,
            user_agent: config.user_agent.as_str().to_string(),
            used_proxy: false,
            identity_used,
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
            vertical_used: Some(config.vertical.as_str().to_string()),
            chrome_path_resolved: None,
            chrome_channel: None,
            run_id: Some(run_id),
        },
    };
    crate::pipeline::fill_chrome_agent_metadata(&mut out.metadata, config);
    out
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SelectorConfig;

    fn test_config(queries: Vec<String>, parallelism: u32) -> Config {
        let vqs: Vec<crate::security::ValidatedQuery> = queries
            .iter()
            .map(|q| crate::security::ValidatedQuery::try_new(q).expect("test query"))
            .collect();
        let first = vqs
            .first()
            .cloned()
            .unwrap_or_else(|| crate::security::ValidatedQuery::try_new("q").expect("q"));
        let mut cfg = Config::default();
        cfg.query = first;
        cfg.queries = vqs;
        cfg.parallelism = crate::types::ParallelismDegree::try_new(parallelism.max(1).min(20))
            .expect("parallelism");
        cfg.pages = crate::types::PageCount::try_new(1).expect("pages");
        cfg.retries = crate::types::RetryBudget::try_new(0).expect("retries");
        cfg.fetch_content = false;
        cfg.warmup_enabled = false;
        cfg.language = crate::types::SerpLanguage::try_new("pt").expect("lang");
        cfg.country = crate::types::SerpCountry::try_new("br").expect("country");
        cfg
    }

    #[test]
    fn error_output_fills_required_fields() {
        let cfg = test_config(vec!["alfa".into(), "beta".into()], 2);
        let err = CliError::NetworkError {
            message: "synthetic test failure".into(),
        };
        let output = error_output(1, &err, &cfg);
        assert_eq!(output.query, "beta");
        assert_eq!(output.result_count, 0);
        assert!(output.results.is_empty());
        assert!(output.error.is_some());
        assert!(output.message.is_some());
        assert_eq!(output.region, "br-pt");
    }

    #[test]
    fn error_output_index_out_of_bounds_uses_empty_string() {
        let cfg = test_config(vec!["apenas uma".into()], 1);
        let err = CliError::NetworkError {
            message: "out of bounds".into(),
        };
        let output = error_output(99, &err, &cfg);
        // No query available for the index → empty string, but no panic.
        assert!(output.query.is_empty());
        assert!(output.error.is_some());
    }

    #[tokio::test]
    async fn parallel_searches_cancelled_before_spawn_returns_errors() {
        // Cancelamos ANTES de chamar, todas as tasks devem retornar falha controlada.
        let token = CancellationToken::new();
        token.cancel();
        let cfg = test_config(
            vec!["query-a".into(), "query-b".into(), "query-c".into()],
            3,
        );
        let queries = cfg.queries.clone();
        let result = execute_parallel_searches(queries, cfg, token).await;
        let output = result.expect("function should return Ok even when all fail");
        assert_eq!(output.query_count, 3);
        assert_eq!(output.searches.len(), 3);
        assert_eq!(output.parallelism, 3);
        // Todas devem estar marcadas com erro.
        for search in &output.searches {
            assert!(
                search.error.is_some(),
                "query {:?} deveria ter falhado com cancelamento",
                search.query
            );
        }
    }

    #[test]
    fn calculate_selectors_hash_returns_16_chars() {
        let cfg = SelectorConfig::default();
        let hash = crate::pipeline::calculate_selectors_hash(&cfg);
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // GAP-WS-57 (regressao): error_output.metadata.retries deve refletir
    // config.retries.get(), NAO ser hardcoded em 0. Cobre N=0, 1 e 3.
    #[test]
    fn error_output_retries_matches_config_retries_zero() {
        let mut cfg = test_config(vec!["q".into()], 1);
        cfg.retries = crate::types::RetryBudget::try_new(0).expect("retries");
        let err = CliError::NetworkError {
            message: "synthetic".into(),
        };
        let output = error_output(0, &err, &cfg);
        assert_eq!(
            output.metadata.retries, 0,
            "retries=0 no config deve propagar para metadata"
        );
    }

    #[test]
    fn error_output_retries_matches_config_retries_one() {
        let mut cfg = test_config(vec!["q".into()], 1);
        cfg.retries = crate::types::RetryBudget::try_new(1).expect("retries");
        let err = CliError::NetworkError {
            message: "synthetic".into(),
        };
        let output = error_output(0, &err, &cfg);
        assert_eq!(
            output.metadata.retries, 1,
            "retries=1 no config deve propagar para metadata (regressao GAP-WS-57)"
        );
    }

    #[test]
    fn error_output_retries_matches_config_retries_three() {
        let mut cfg = test_config(vec!["q".into()], 1);
        cfg.retries = crate::types::RetryBudget::try_new(3).expect("retries");
        let err = CliError::NetworkError {
            message: "synthetic".into(),
        };
        let output = error_output(0, &err, &cfg);
        assert_eq!(
            output.metadata.retries, 3,
            "retries=3 no config deve propagar para metadata (regressao GAP-WS-57)"
        );
    }

    #[test]
    fn calculate_selectors_hash_is_deterministic() {
        let hash_a = crate::pipeline::calculate_selectors_hash(&SelectorConfig::default());
        let hash_b = crate::pipeline::calculate_selectors_hash(&SelectorConfig::default());
        assert_eq!(hash_a, hash_b, "selector hash must be deterministic");
        assert_eq!(hash_a.len(), 16, "selector hash must be 16 hex chars (u64)");
    }

    #[test]
    fn calculate_selectors_hash_different_inputs_produce_different_hashes() {
        let hash_a = crate::pipeline::calculate_selectors_hash(&SelectorConfig::default());
        let mut other = SelectorConfig::default();
        other.html_endpoint.results_container = ".completely-different-selector".to_string();
        let hash_b = crate::pipeline::calculate_selectors_hash(&other);
        assert_ne!(
            hash_a, hash_b,
            "different selectors must produce different hashes"
        );
    }

    #[test]
    fn error_output_metadata_pre_flight_fired_defaults_false() {
        // Regression GAP-WS-59 P3: pre_flight_fired field is a bool that
        // defaults to false in error_output. When --pre-flight fires
        // and blocks, the field is set to true via a different path.
        let cfg = test_config(vec!["q".into()], 1);
        let err = CliError::NetworkError {
            message: "x".into(),
        };
        let output = error_output(0, &err, &cfg);
        assert!(
            !output.metadata.pre_flight_fired,
            "pre_flight_fired must default to false in error_output"
        );
    }
}

