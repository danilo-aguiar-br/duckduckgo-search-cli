// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (plain aggregate DTO; no I/O)
//! Transport-neutral aggregate of a completed SERP run.
//!
//! [`AggregatedSearchResult`] used to live in `search::execute`, next to the
//! `reqwest`-based fetcher. That was fine while HTTP was compiled
//! unconditionally, but the Chrome pipeline (`pipeline::chrome`) returns the very
//! same type, so gating the HTTP module behind `http-test-harness` would have
//! taken the type down with it and broken the default Chrome-only build.
//!
//! The struct itself describes *what a search produced*, not *how it was
//! fetched*, so it belongs outside either transport. It is re-exported from
//! `crate::search`, which keeps every existing call site unchanged.

use crate::types::{Endpoint, SearchResult};

/// Aggregated result of a search with pagination and potential endpoint fallback.
///
/// Produced by both transports: the Chrome/CDP pipeline in production and the
/// residual HTTP fetcher under `http-test-harness`.
#[derive(Debug)]
pub struct AggregatedSearchResult {
    /// Organic results collected across all pages.
    pub results: Vec<SearchResult>,
    /// Number of pages actually fetched.
    pub pages_fetched: u32,
    /// Whether the lite endpoint was used as fallback.
    pub used_fallback_lite: bool,
    /// Total HTTP attempts (including retries).
    pub attempts: u32,
    /// Endpoint that produced the final results.
    pub effective_endpoint: Endpoint,
    /// Raw body of the FIRST page (empty if unavailable).
    ///
    /// v0.8.0 GAP-AUD-003: consumed by the zero-result classifier to tell a
    /// ghost-block apart from a legitimate empty SERP. Not persisted on disk.
    pub first_body: String,
    /// Raw bytes received from DDG BEFORE decompression.
    ///
    /// v0.8.0 GAP-NEW-002: lets an operator distinguish an empty body from a
    /// 14KB Cloudflare stealth-block shell without a debug build.
    pub bytes_in: u64,
    /// Bytes after gzip/deflate/br decompression.
    ///
    /// v0.8.0 GAP-NEW-002: counterpart of [`Self::bytes_in`]. A ratio above one
    /// indicates compression was applied.
    pub bytes_out: u64,
}
