// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (single HTTP GET + body read)
//! Single-shot search execution and aggregated result types.

use crate::error::CliError;
use crate::types::{Endpoint, SearchResult};
use reqwest::Client;

use super::url::build_url;

/// Byte threshold for silent block detection.
/// Real `DuckDuckGo` responses with results are 50-200KB.
/// Silent block pages are typically ~3KB.
pub(crate) const SILENT_BLOCK_THRESHOLD: usize = 5_000;

/// Executes the initial search on the configured endpoint and returns the raw HTML.
/// Compatibility version (iteration 1) — used by the simple single-query flow.
///
/// # Errors
///
/// Returns an error if the HTTP request fails, if `DuckDuckGo` returns a non-2xx
/// status, or if the response body is suspiciously small (silent block detected).
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future before `.send().await`
/// completes discards the in-flight request; dropping it before the capped
/// body read completes discards the partially-received body.
pub async fn execute_search(
    client: &Client,
    query: &str,
    idioma: &str,
    pais: &str,
) -> Result<String, CliError> {
    let url = build_url(query, idioma, pais);
    tracing::info!(url = %url, "Sending GET to the DuckDuckGo HTML endpoint");

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CliError::http_with_source(format!("failed to send GET to {url}"), e))?;

    let status = response.status();
    tracing::info!(status = %status, "HTTP response received");

    if !status.is_success() {
        return Err(CliError::http_msg(format!(
            "duckduckgo returned http {} for {:?}",
            status.as_u16(),
            query
        )));
    }

    // Propagate typed decompress/transport errors without wrapping/duplicating.
    let html = crate::decompress::response_body_string(response).await?;

    if html.len() < SILENT_BLOCK_THRESHOLD {
        tracing::warn!(
            bytes = html.len(),
            limiar = SILENT_BLOCK_THRESHOLD,
            "suspiciously small response — possible silent block"
        );
        return Err(CliError::http_msg(format!(
            "suspiciously small response ({} bytes < {} threshold) — possible silent block",
            html.len(),
            SILENT_BLOCK_THRESHOLD
        )));
    }

    tracing::info!(bytes = html.len(), "HTML received successfully");
    Ok(html)
}

/// Aggregated result of a search with pagination and potential endpoint fallback.
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
    /// v0.8.0 GAP-AUD-003: consumido por
    /// to distinguish ghost-block from legitimate zero. Not persisted on disk.
    pub first_body: String,
    /// Raw bytes received from DDG BEFORE decompression.
    /// v0.8.0 GAP-NEW-002: HTTP decompression byte counters. Allows
    /// distinguir body vazio () de shell de 14KB (stealth
    /// block do Cloudflare) sem precisar de build debug.
    pub bytes_in: u64,
    /// Bytes after gzip/deflate/br decompression.
    /// v0.8.0 GAP-NEW-002: complemento de . A taxa
    ///  indicates compression was applied.
    pub bytes_out: u64,
}
