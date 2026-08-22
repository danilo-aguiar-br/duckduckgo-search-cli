// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (string transformations, no I/O).
//! Query decomposition for deep-research fan-out.
//!
//! Splits an original user query into 1..=`max_sub_queries` sub-queries using
//! one of three strategies:
//!
//! - `Heuristic` — applies five canonical templates:
//!   aspect, comparison, timeline, opinion, cause. Pure local computation.
//! - `Manual` — reads a list of sub-queries from a file
//!   or stdin, one per line. Empty lines and `#`-comments are ignored.
//!
//! All templates are deterministic for a given input — no LLM is invoked.
//! When the heuristic strategy produces fewer templates than `max_sub_queries`,
//! the function tops up by emitting focused refinements of the original query.

mod heuristic;
mod manual;
mod templates;

use crate::error::CliError;
use std::path::Path;
use tokio_util::sync::CancellationToken;

use heuristic::heuristic_decompose;
use manual::load_manual;

pub use heuristic::{is_composite_query, CompositeSignal};
pub use templates::{HeuristicTemplate, SubQuery, SubQueryOrigin};

/// Decomposes a user query into a list of sub-queries.
///
/// # Arguments
///
/// * `query` — the original user query.
/// * `strategy` — heuristic or manual.
/// * `manual_path` — when `Some`, used as the source of manual sub-queries
///   (only honoured when `strategy == Manual`).
/// * `max_sub_queries` — upper bound on the number of returned sub-queries.
/// * `cancel` — cooperative cancellation token.
/// * `news_aware` — when `true` (dual deep-research), inject news/recency
///   template first so the news vertical is not starved by web-centric suffixes
///   (GAP-AUD-DR-003 / CM-08).
///
/// # Errors
///
/// Returns [`CliError::InvalidConfig`] when `manual_path` is `Some` but the
/// file cannot be read, or when the manual list is empty.
///
/// # Cancel safety
///
/// This function is cancel-safe — it never holds resources across `await`
/// points in a way that would leak on cancellation.
pub async fn decompose(
    query: &str,
    strategy: crate::deep_research::SubQueryStrategy,
    manual_path: Option<&Path>,
    max_sub_queries: usize,
    cancel: &CancellationToken,
    news_aware: bool,
) -> Result<Vec<SubQuery>, CliError> {
    if cancel.is_cancelled() {
        return Err(CliError::Cancelled);
    }
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(CliError::InvalidConfig {
            message: "deep-research query is empty".to_string(),
        });
    }
    if max_sub_queries == 0 {
        return Err(CliError::InvalidConfig {
            message: "max_sub_queries must be at least 1".to_string(),
        });
    }

    match strategy {
        crate::deep_research::SubQueryStrategy::Manual => {
            load_manual(manual_path, max_sub_queries).await
        }
        crate::deep_research::SubQueryStrategy::Heuristic => {
            heuristic_decompose(trimmed, max_sub_queries, news_aware)
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
