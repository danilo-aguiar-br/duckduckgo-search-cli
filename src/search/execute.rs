// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (single HTTP GET + body read)
//! Single-shot search execution over the residual HTTP transport.
//!
//! The aggregated result type lives in `super::aggregate` since v1.0.3 — it is
//! transport-neutral and must survive this module being gated out.

use crate::error::CliError;
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

// `AggregatedSearchResult` moved to `super::aggregate` in v1.0.3 so the Chrome
// pipeline keeps the type when this HTTP module is gated out.
