// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: **mixed pipeline**
// 1. decomposition — CPU-light / sequential (heuristic templates)
// 2. fan-out — I/O-bound multi-process Chrome (JoinSet + Semaphore)
// 3. dual aggregation — CPU-light, parallel via spawn_blocking + join!
// 4. synthesis — CPU-light sequential (string build)
// Bound: `--parallel` / `--max-concurrency` on stage 2.
//! Deep research subcommand — query fan-out, aggregation, and synthesis.
//!
//! This module is the single entry point for the `deep-research` subcommand. It
//! composes four smaller modules into an 8-stage pipeline:
//!
//! 1. `decomposition::decompose` — splits the user query into 1..=`max_sub_queries`
//!    sub-queries using a heuristic, manual, or template strategy.
//! 2. `parallel::execute_query_fan_out` — fans out all sub-queries via `JoinSet`
//!    with a `Semaphore` for bounded concurrency and `CancellationToken` for
//!    graceful shutdown.
//! 3. `aggregation::aggregate` — merges the per-sub-query result lists into a
//!    single ranked list using Reciprocal Rank Fusion (RRF, K=60) or
//!    URL-canonical deduplication.
//! 4. `synthesis::synthesize_dual` — optionally combines the top-K web and
//!    news results into a Markdown/PlainText/Json report with numbered
//!    references.
//!
//! # Design notes
//!
//! - `SubQuery` carries its origin (heuristic/manual/template) and a per-query
//!   `elapsed_ms` for observability.
//! - Aggregation emits a score in `[0.0, 1.0]` — higher is better — plus a
//!   `fontes: Vec<String>` listing the sub-query texts that produced the result
//!   for traceability.
//!
//! Content fetches performed by the synthesis stage honour the same
//! per-host rate limiting, circuit breaker, and concurrency controls as
//! the rest of the binary.

mod depth;
mod news_diag;
mod run;
mod types;

#[cfg(test)]
#[path = "../deep_research_tests.rs"]
mod tests;

// ── Public API re-exports (stable paths under `crate::deep_research::*`) ──
pub use run::run_deep_research;
pub use types::{
    AggregationStrategyKind, DeepResearchArgs, DeepResearchMetadata, DeepResearchOutput,
    SubQueryOutcome, SubQueryStrategy,
};

// Private helpers re-exported into this module namespace so unit tests
// (`use super::*`) and `run` keep the same visibility surface as the
// former single-file module.
#[cfg(test)]
pub(crate) use depth::{heuristic_depth_follow_ups, is_quality_depth_subquery};
#[cfg(test)]
pub(crate) use news_diag::{sub_query_news_diagnosis, sub_query_news_fields};
// Types used by unit tests (were private imports in the former single-file module).
#[cfg(test)]
pub(crate) use crate::aggregation::AggregatedItem;
#[cfg(test)]
pub(crate) use crate::types::SearchOutput;

/// Hard upper bound on the number of sub-queries produced by decomposition.
pub const MAX_SUB_QUERIES: usize = 12;

/// Default number of sub-queries when the user does not specify.
/// Default sub-query fan-out for deep-research (v1.0.2 budget contract).
///
/// Paired with [`crate::cli::DEFAULT_FETCH_CONTENT_CAP`] so
/// `gate(defaults) ≤ DEFAULT_GLOBAL_TIMEOUT` (GAP-AUD-DR-001).
pub const DEFAULT_MAX_SUB_QUERIES: usize = 3;

/// Hard upper bound on the depth (number of reflective rounds).
pub const MAX_DEPTH: u32 = 3;

/// Sub-query outcome status when the fan-out completed without error (EN wire).
pub const SUB_QUERY_STATUS_OK: &str = "ok";
/// Sub-query outcome status when the fan-out failed (EN wire default; PT via `--wire-keys pt`).
pub const SUB_QUERY_STATUS_ERROR: &str = "error";

/// Default RRF constant (K in `1 / (K + rank)`). 60 is the de-facto literature
/// default (Cormack et al., 2009) and matches the `SQLite` `FTS5` anchor used
/// in the `GraphRAG` memory subsystem.
pub const RRF_K: u32 = 60;

// Compile-time invariants for deep-research limits.
const _: () = assert!(DEFAULT_MAX_SUB_QUERIES <= MAX_SUB_QUERIES && DEFAULT_MAX_SUB_QUERIES >= 1);
const _: () = assert!(MAX_DEPTH >= 1);
const _: () = assert!(RRF_K >= 1);
