// SPDX-License-Identifier: MIT OR Apache-2.0
//! Full deep-research pipeline orchestration.

use crate::aggregation::{
    aggregate, aggregate_news, AggregatedNewsItem, AggregationStrategy,
};
use crate::decomposition::{decompose, SubQuery};
use crate::error::CliError;
use crate::parallel::execute_parallel_searches;
use crate::synthesis::{
    synthesize_dual, synthesize_with_stats, SubQuerySynthStats,
};
use crate::types::{Config, SearchOutput};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use super::depth::heuristic_depth_follow_ups;
use super::news_diag::sub_query_news_diagnosis;
use super::types::{
    AggregationStrategyKind, DeepResearchArgs, DeepResearchMetadata, DeepResearchOutput,
    SubQueryOutcome,
};
use super::{RRF_K, SUB_QUERY_STATUS_ERROR, SUB_QUERY_STATUS_OK};

/// Runs the full deep-research pipeline.
///
/// # Arguments
///
/// * `args` — user-facing deep-research options.
/// * `cfg` — base [`Config`] inherited from the global CLI flags (HTTP client,
///   timeouts, proxies, etc.).
/// * `cancel` — token that signals SIGINT / global timeout.
///
/// # Errors
///
/// Returns an error only for unrecoverable setup failures (e.g. invalid
/// `sub_queries_file` when strategy is `Manual`). Per-sub-query failures are
/// captured inside the returned [`DeepResearchOutput`] rather than propagated
/// as `Err`, so partial results are always surfaced.
///
/// # Cancel safety
///
/// This function is cancel-safe. The `CancellationToken` is propagated to every
/// spawned sub-task; on cancellation, in-flight HTTP requests abort, partial
/// results are collected, and a `DeepResearchOutput` is returned with the
/// remaining sub-queries marked as `status == "error"`.
pub async fn run_deep_research(
    args: DeepResearchArgs,
    cfg: &Config,
    cancel: CancellationToken,
) -> Result<DeepResearchOutput, CliError> {
    args.validate()?;

    let start_total = Instant::now();

    // Stage 1: decompose the original query.
    let sub_queries: Vec<SubQuery> = decompose(
        &args.query,
        args.sub_query_strategy,
        args.sub_queries_file.as_deref(),
        args.max_sub_queries,
        &cancel,
        !args.no_news,
    )
    .await?;

    // Stage 2: fan out — `SubQuery.text` is already `ValidatedQuery` (GAP-TYPE-002/013).
    // JoinSet + Semaphore bound = `--parallel`. Each Chrome query is a separate OS process.
    crate::concurrency::log_chrome_concurrency_advisory(cfg.parallelism.get());
    let per_query_outputs: std::sync::Arc<Vec<SearchOutput>> = std::sync::Arc::new(
        execute_parallel_searches(
            sub_queries.iter().map(|q| q.text.clone()).collect(),
            cfg.clone(),
            cancel.clone(),
        )
        .await?
        .searches,
    );

    // Build the per-sub-query outcome report.
    let mut outcomes: Vec<SubQueryOutcome> = sub_queries
        .iter()
        .zip(per_query_outputs.iter())
        .map(|(q, o)| {
            let diag = sub_query_news_diagnosis(args.no_news, o);
            SubQueryOutcome {
                text: q.text.as_str().to_string(),
                strategy: q.strategy_label(),
                status: if o.error.is_some() {
                    SUB_QUERY_STATUS_ERROR
                } else {
                    SUB_QUERY_STATUS_OK
                }
                .to_string(),
                elapsed_ms: o.metadata.execution_time_ms,
                error: o.error.clone(),
                news_count: diag.news_count,
                news_unavailable: diag.news_unavailable,
                zero_cause: diag.zero_cause,
                news_error: diag.news_error,
                news_diagnosis: diag.news_diagnosis,
            }
        })
        .collect();

    // Stage 3: aggregate across sub-queries.
    //
    // Workload: CPU-light merge (RRF / URL dedupe) over a small N
    // (max_sub_queries ≤ 12). Web and news score spaces are independent —
    // run both in parallel on the blocking pool so the async worker is not
    // occupied. GAP-PAR-023/035: each branch acquires its own CPU permit
    // (per-branch admit via `run_cpu_bound`) so we never hold two permits
    // idle on the async task before either spawn starts. Rayon not used: N tiny.
    let aggregation_strategy = match args.aggregation {
        AggregationStrategyKind::Rrf => AggregationStrategy::Rrf(RRF_K),
        AggregationStrategyKind::DedupeByUrl => AggregationStrategy::DedupeByUrl,
    };

    let outputs_web = std::sync::Arc::clone(&per_query_outputs);
    let outputs_news = std::sync::Arc::clone(&per_query_outputs);
    let (web_res, news_res) = tokio::join!(
        crate::concurrency::run_cpu_bound({
            let outputs = std::sync::Arc::clone(&outputs_web);
            move || aggregate(outputs.as_slice(), aggregation_strategy)
        }),
        crate::concurrency::run_cpu_bound({
            let outputs = std::sync::Arc::clone(&outputs_news);
            move || aggregate_news(outputs.as_slice(), aggregation_strategy)
        }),
    );
    let aggregated = web_res.map_err(|e| match e {
        CliError::Cancelled => CliError::Cancelled,
        other => CliError::NetworkError {
            message: format!("web aggregation task failed: {other}"),
        },
    })?;
    // GAP-WS-105: aggregate the news vertical over the SAME fan-out outputs
    // (all rounds enter the merge, mirroring the web aggregation above), in
    // its own score space. Outputs with `news: None` are skipped inside
    // `aggregate_news`, so mid-flight unavailability degrades to an empty
    // list rather than an error.
    let mut aggregated_news: Vec<AggregatedNewsItem> = news_res.map_err(|e| match e {
        CliError::Cancelled => CliError::Cancelled,
        other => CliError::NetworkError {
            message: format!("news aggregation task failed: {other}"),
        },
    })?;
    let mut aggregated = aggregated;

    // Determine the deepest cascade level observed across all sub-queries.
    let mut cascade_level = per_query_outputs
        .iter()
        .filter_map(|o| o.metadata.cascade_level_observed)
        .max()
        .map(|v| v as u8);

    // Agent contract (v0.9.8 R-02): honest chrome usage + path/channel on deep envelope.
    let mut used_chrome = per_query_outputs.iter().any(|o| o.metadata.used_chrome);
    let mut per_query_outputs = per_query_outputs;

    // GAP-E2E-48-008: reflective depth — heuristic follow-up rounds (no LLM).
    // Each round mines rare terms from top-K titles/snippets, fans out additional
    // sub-queries under the remaining max_sub_queries budget, and re-aggregates.

    if args.depth > 0 && !cancel.is_cancelled() {
        let mut seen_texts: std::collections::HashSet<String> = sub_queries
            .iter()
            .map(|q| q.text.as_str().to_ascii_lowercase())
            .collect();
        seen_texts.insert(args.query.to_ascii_lowercase());

        for round in 1..=args.depth {
            if cancel.is_cancelled() {
                break;
            }
            let remaining = args
                .max_sub_queries
                .saturating_sub(seen_texts.len().saturating_sub(1))
                .clamp(1, 4);
            let follow_ups =
                heuristic_depth_follow_ups(&args.query, &aggregated, remaining, &seen_texts);
            if follow_ups.is_empty() {
                outcomes.push(SubQueryOutcome {
                    text: format!("<reflective depth={round} no-gap-terms>"),
                    strategy: "depth".to_string(),
                    status: SUB_QUERY_STATUS_OK.to_string(),
                    elapsed_ms: 0,
                    error: None,
                    news_count: None,
                    news_unavailable: None,
                    zero_cause: None,
                    news_error: None,
                    news_diagnosis: None,
                });
                continue;
            }

            let follow_validated: Vec<crate::security::ValidatedQuery> = follow_ups
                .iter()
                .filter_map(|t| crate::security::ValidatedQuery::try_new(t).ok())
                .collect();
            if follow_validated.is_empty() {
                continue;
            }
            for t in &follow_validated {
                seen_texts.insert(t.as_str().to_ascii_lowercase());
            }

            let round_start = Instant::now();
            let round_outputs = execute_parallel_searches(
                follow_validated.clone(),
                cfg.clone(),
                cancel.clone(),
            )
            .await?
            .searches;

            for (q, o) in follow_validated.iter().zip(round_outputs.iter()) {
                let diag = sub_query_news_diagnosis(args.no_news, o);
                outcomes.push(SubQueryOutcome {
                    text: q.as_str().to_string(),
                    strategy: "depth".to_string(),
                    status: if o.error.is_some() {
                        SUB_QUERY_STATUS_ERROR.to_string()
                    } else {
                        SUB_QUERY_STATUS_OK.to_string()
                    },
                    elapsed_ms: o.metadata.execution_time_ms,
                    error: o.error.clone(),
                    news_count: diag.news_count,
                    news_unavailable: diag.news_unavailable,
                    zero_cause: diag.zero_cause,
                    news_error: diag.news_error,
                    news_diagnosis: diag.news_diagnosis,
                });
            }

            // Merge round outputs into the pool and re-aggregate (CPU-bound).
            // Pre-reserve: known sizes from prior fan-out + reflection round (memory rule).
            let mut merged: Vec<SearchOutput> =
                Vec::with_capacity(per_query_outputs.len() + round_outputs.len());
            merged.extend_from_slice(per_query_outputs.as_ref());
            merged.extend(round_outputs);
            per_query_outputs = std::sync::Arc::new(merged);
            let outputs_web = std::sync::Arc::clone(&per_query_outputs);
            let outputs_news = std::sync::Arc::clone(&per_query_outputs);
            let (web_res, news_res) = tokio::join!(
                crate::concurrency::run_cpu_bound({
                    let outputs = std::sync::Arc::clone(&outputs_web);
                    move || aggregate(outputs.as_slice(), aggregation_strategy)
                }),
                crate::concurrency::run_cpu_bound({
                    let outputs = std::sync::Arc::clone(&outputs_news);
                    move || aggregate_news(outputs.as_slice(), aggregation_strategy)
                }),
            );
            if let Ok(w) = web_res {
                aggregated = w;
            }
            if let Ok(n) = news_res {
                aggregated_news = n;
            }
            used_chrome = per_query_outputs.iter().any(|o| o.metadata.used_chrome);
            cascade_level = per_query_outputs
                .iter()
                .filter_map(|o| o.metadata.cascade_level_observed)
                .max()
                .map(|v| v as u8);
            let _ = round_start;
        }
    }

    let sub_total = outcomes.len();
    let sub_ok = outcomes
        .iter()
        .filter(|o| o.status.eq_ignore_ascii_case("ok"))
        .count();
    let sub_err = sub_total.saturating_sub(sub_ok);
    let partial = sub_err > 0
        || outcomes.iter().any(|o| !o.status.eq_ignore_ascii_case(SUB_QUERY_STATUS_OK));
    let chrome_n = crate::process_count::count_chrome_like_processes() as u64;
    let chrome_contention_advisory = chrome_n
        >= crate::types::bounded::BUDGET_CONTENTION_LOW;
    let synth_stats = SubQuerySynthStats {
        total: sub_total,
        ok: sub_ok,
        error: sub_err,
    };

    // Re-run synthesis after depth rounds if requested (fresh dual report).
    let synth = if args.synthesize {
        let web = aggregated.clone();
        let news = aggregated_news.clone();
        let query = args.query.clone();
        let format = args.synth_format;
        let budget = args.budget_tokens;
        Some(
            crate::concurrency::run_cpu_bound(move || {
                if news.is_empty() {
                    synthesize_with_stats(&web, &query, format, budget, Some(synth_stats))
                } else {
                    synthesize_dual(&web, &news, &query, format, budget)
                }
            })
            .await?,
        )
    } else {
        None
    };

    let (chrome_path_resolved, chrome_channel) = {
        #[cfg(feature = "chrome")]
        {
            crate::pipeline::resolved_chrome_metadata(cfg)
        }
        #[cfg(not(feature = "chrome"))]
        {
            (None, None)
        }
    };

    Ok(DeepResearchOutput {
        kind: "deep_research".to_string(),
        query: args.query.clone(),
        metadata: DeepResearchMetadata {
            original_query: args.query,
            sub_queries: outcomes,
            aggregation_strategy: match args.aggregation {
                AggregationStrategyKind::Rrf => "rrf".to_string(),
                AggregationStrategyKind::DedupeByUrl => "dedupe_by_url".to_string(),
            },
            unique_result_count: aggregated.len(),
            unique_news_count: aggregated_news.len(),
            total_elapsed_ms: start_total.elapsed().as_millis() as u64,
            cascade_level,
            used_chrome,
            chrome_path_resolved,
            chrome_channel,
            sub_queries_total: sub_total,
            sub_queries_ok: sub_ok,
            sub_queries_error: sub_err,
            partial,
            chrome_contention_advisory,
        },
        results: aggregated,
        news_count: aggregated_news.len(),
        news: aggregated_news,
        synth,
    })
}
