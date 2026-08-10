// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (HTTP requests to DuckDuckGo endpoints)
//! URL construction and search request execution for `DuckDuckGo`.
//!
//! Iteration 3 adds:
//! - Pagination with `vqd` token via POST form-urlencoded.
//! - Retry with exponential backoff on 429 and UA rotation on 403.
//! - Lite endpoint (see [`crate::endpoints`]).
//! - Time filter (`df`) and safe-search (`kp`).
//! - Base URL parameterization via environment variables (for wiremock tests).
//!
//! Base URLs live in [`crate::endpoints`] (single source of truth). Env overrides:
//! CLI `--base-url-html|lite|serp` (process policy). Defaults END with a
//! slash (`/html/` and `/lite/`) because `DuckDuckGo` treats `/html` (without
//! slash) as a redirect.
//!
//! # Module layout
//!
//! - `url` — URL builders (`build_search_url`, `format_kl`, …)
//! - `retry` — `execute_with_retry` + `RetryFailReason`
//! - `execute` — single-shot `execute_search` + [`AggregatedSearchResult`]
//! - `extract` — pagination-token / SERP parse helpers
//! - `pagination` — multi-page search + Lite fallback gate

mod aggregate;
// GAP-WS-113: `execute`, `pagination` and `retry` drive the residual `reqwest`
// SERP transport. Production SERP is Chrome/CDP, so they are harness-only.
#[cfg(feature = "http-test-harness")]
mod execute;
mod extract;
#[cfg(feature = "http-test-harness")]
mod pagination;
#[cfg(feature = "http-test-harness")]
mod retry;
mod url;

// Re-export endpoint accessors so existing `search::html_base_url` call sites keep working.
pub use crate::endpoints::{html_base_url, lite_base_url, serp_base_url};

pub use aggregate::AggregatedSearchResult;
pub use extract::extract_pagination_tokens;
pub use url::{build_news_search_url, build_search_url, build_url, format_kl};

#[cfg(feature = "http-test-harness")]
pub use execute::execute_search;
#[cfg(feature = "http-test-harness")]
pub use pagination::search_with_pagination;
#[cfg(feature = "http-test-harness")]
pub use retry::{execute_with_retry, RetryFailReason, RetryResult};

// Internal helpers re-exported so `tests` can `use super::*` without public API surface.
#[cfg(test)]
pub(crate) use crate::probe_deep::InterstitialKind;
#[cfg(test)]
pub(crate) use crate::types::{Config, Endpoint, SafeSearch, TimeFilter};
#[cfg(test)]
pub(crate) use extract::extract_results_and_pagination_tokens;
#[cfg(all(test, feature = "http-test-harness"))]
pub(crate) use pagination::should_try_lite;

#[cfg(test)]
mod tests;
