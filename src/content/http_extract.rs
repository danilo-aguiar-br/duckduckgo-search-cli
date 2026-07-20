// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound + CPU (HTTP fetch + readability off worker)
//! Residual pure-HTTP content extraction (test harness / non-Chrome path).

use super::encoding::{accept_as_html, decode_to_utf8, extract_charset};
use super::readability::{apply_readability, MIN_CONTENT_THRESHOLD};
use super::ssrf::{is_safe_url, url_is_safe_to_fetch};
use crate::error::CliError;
use reqwest::Client;
use tokio_util::sync::CancellationToken;

/// Hard cap on HTTP response body size before allocation (5 MiB).
///
/// Security bound for hostile responses — not user XDG config (GAP-SCRAPE N/A-009).
pub(crate) const MAX_BODY_BYTES: usize = 5 * 1024 * 1024;

/// Extracts the main text content from a URL via pure HTTP.
///
/// Returns:
/// - `Ok(Some((clean_text, original_size_in_bytes)))` on success.
/// - `Ok(None)` if the body is not HTML (pdf, image, etc.).
/// - `Err` on unrecoverable network/parse failure.
///
/// The returned text may be empty if extraction produced no content > 200 chars —
/// in that case the caller knows a Chrome fallback would be needed.
///
/// # Errors
///
/// Returns an error if the HTTP request fails, the response body
/// cannot be read, or the operation is cancelled via the token.
///
/// # Cancel safety
///
/// This function is cancel-safe. Each `.await` point races against
/// the cancellation token via `tokio::select!`, so dropping the
/// future does not leak resources.
pub async fn extract_http_content(
    client: &Client,
    url: &str,
    max_size: usize,
    token: &CancellationToken,
) -> Result<Option<(String, u32)>, CliError> {
    if token.is_cancelled() {
        return Err(CliError::Cancelled);
    }

    // Structural + async DNS SSRF (rules-rust network / SSRF prevention).
    if !url_is_safe_to_fetch(url).await {
        tracing::warn!(
            url,
            "URL rejected by SSRF filter — unsafe scheme, private host, or blocked DNS"
        );
        return Ok(None);
    }

    tracing::debug!(url, "starting HTTP content extraction");

    let response = tokio::select! {
        biased;
        _ = token.cancelled() => {
            return Err(CliError::Cancelled);
        }
        res = client.get(url).send() => res.map_err(|e| {
            CliError::http_with_source(format!("http request failed for {url}"), e)
        })?
    };

    // Post-redirect final URL must still pass structural SSRF (redirect policy
    // already checks hops; this is defense-in-depth for the effective URL).
    let final_url = response.url().as_str().to_string();
    if !crate::chrome_policy::http_test_harness_active() && !is_safe_url(&final_url) {
        tracing::warn!(
            url,
            final_url = %final_url,
            "final URL after redirects rejected by SSRF filter"
        );
        return Ok(None);
    }

    if !response.status().is_success() {
        tracing::debug!(url, status = %response.status(), "non-success HTTP status — discarding");
        return Ok(None);
    }

    // Charset from Content-Type before consuming the body.
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let charset = extract_charset(&content_type);

    // Capture Content-Encoding BEFORE the body is consumed.
    let encoding = response
        .headers()
        .get(reqwest::header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("identity")
        .to_ascii_lowercase();

    // Stream body with hard cap (never unbounded `bytes()` / `read_to_end`).
    let raw = tokio::select! {
        biased;
        _ = token.cancelled() => {
            return Err(CliError::Cancelled);
        }
        res = crate::decompress::read_body_capped(response, MAX_BODY_BYTES) => {
            match res {
                Ok(b) => b,
                Err(CliError::PayloadTooLarge { max, actual }) => {
                    tracing::warn!(
                        url,
                        max,
                        actual,
                        "response body exceeds size limit — skipping"
                    );
                    return Ok(None);
                }
                Err(e) => return Err(e),
            }
        }
    };

    let bytes = crate::decompress::decode_bytes(&raw, &encoding)?;

    if bytes.len() > MAX_BODY_BYTES {
        tracing::warn!(
            url,
            actual_size = bytes.len(),
            limit = MAX_BODY_BYTES,
            "downloaded body exceeds size limit — discarding"
        );
        return Ok(None);
    }

    // Content-Type gate with magic-byte sniff for generic/empty types (GAP-SCRAPE-007).
    if !accept_as_html(&content_type, &bytes) {
        tracing::debug!(url, content_type, "body is not HTML — discarding");
        return Ok(None);
    }

    let size_original = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    tracing::debug!(url, size = bytes.len(), "body downloaded");

    // Decode to UTF-8: BOM → CT charset → meta → WINDOWS_1252.
    let html_utf8 = decode_to_utf8(&bytes, charset.as_deref());

    // Parse + readability run in the blocking pool: scraper uses Rc<_> internally
    // (html5ever) and is NOT Send. GAP-PAR-017/030: central `run_cpu_bound` admits
    // via blocking_cpu_semaphore and maps JoinError panic/cancel.
    let max_size_local = max_size;
    let clean_text = crate::concurrency::run_cpu_bound(move || {
        apply_readability(&html_utf8, max_size_local)
    })
    .await?;

    if clean_text.len() < MIN_CONTENT_THRESHOLD {
        tracing::debug!(
            url,
            len = clean_text.len(),
            "extracted content below threshold — signalling possible Chrome need"
        );
        return Ok(Some((String::new(), size_original)));
    }

    tracing::debug!(url, clean_size = clean_text.len(), "extraction complete");
    Ok(Some((clean_text, size_original)))
}
