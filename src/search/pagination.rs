// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (paginated HTTP + optional Lite fallback)
//! Multi-page search with `vqd` pagination and Lite endpoint fallback.

use crate::endpoints;
use crate::endpoints::html_base_url;
use crate::extraction;
use crate::probe_deep::{detect_interstitial, InterstitialKind};
use crate::types::{Config, Endpoint};
use rand::RngExt;
use reqwest::Client;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use super::execute::{AggregatedSearchResult, SILENT_BLOCK_THRESHOLD};
use super::extract::extract_results_and_pagination_tokens_async;
use super::retry::{execute_with_retry, RetryFailReason};
use super::url::{build_search_url, format_kl};

/// Minimum delay between consecutive pages (ms).
/// v0.6.0: increased from 500 to 800ms to reduce anti-bot detection.
const PAGINATION_DELAY_MIN_MS: u64 = 800;
/// Maximum delay between consecutive pages (ms).
/// v0.6.0: increased from 1000 to 1500ms to reduce anti-bot detection.
const PAGINATION_DELAY_MAX_MS: u64 = 1500;

/// Decides whether `search_with_pagination` should attempt the Lite
/// endpoint after the HTML endpoint returned no results. Returns
/// `(should_try, pre_flight_fired)`:
/// - `should_try`: `true` when the gate fires (Lite fallback should be
///   attempted).
/// - `pre_flight_fired`: `true` only when the new pre-flight path
///   triggered the fallback (so callers can populate
///   `SearchMetadata::pre_flight_fired`).
///
/// v0.7.9 GAP-WS-58 + GAP-WS-59 + v0.7.10 P3: the legacy
/// v0.7.7/v0.7.8 path requires `cfg.allow_lite_fallback` AND a
/// positive marker classification. The new pre-flight path requires
/// `cfg.pre_flight` AND a ghost-block (sub-`SILENT_BLOCK_THRESHOLD`
/// body without any result-page selector). Both paths converge here
/// so the gate is a single, testable pure function.
pub(crate) fn should_try_lite(
    cfg: &Config,
    kind: InterstitialKind,
    ghost_block: bool,
) -> (bool, bool) {
    let legacy = cfg.allow_lite_fallback
        && matches!(
            kind,
            InterstitialKind::Cloudflare | InterstitialKind::DuckDuckGo
        );
    let pre_flight = cfg.pre_flight && ghost_block;
    let should_try = legacy || pre_flight;
    (should_try, pre_flight)
}

/// Runs a complete search with vqd pagination and optional fallback to Lite.
///
/// If the HTML endpoint returns zero results on the first page (via Strategies 1 and 2),
/// automatically falls back to the Lite endpoint (Strategy 3).
///
/// Returns an aggregated structure with results, related searches, pages actually
/// fetched, fallback indicator, and total attempt count.
///
/// # Errors
///
/// Returns an error (as a [`RetryFailReason`]) if the first-page request fails after
/// all retries, or if the first response is suspiciously small (silent block detected).
/// Pagination and Lite-fallback failures are logged and handled gracefully without
/// propagating an error.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future between pagination steps leaves
/// the accumulated results collected so far in an unreachable state; no partial output
/// is emitted to the caller.
pub async fn search_with_pagination(
    client: &Client,
    cfg: &Config,
    query: &str,
    flag_rate_limit: &Arc<AtomicBool>,
    cancellation: &CancellationToken,
) -> std::result::Result<AggregatedSearchResult, RetryFailReason> {
    let initial_endpoint = cfg.endpoint;
    let initial_url = build_search_url(
        query,
        cfg.language.as_str(),
        cfg.country.as_str(),
        initial_endpoint,
        cfg.time_filter,
        cfg.safe_search,
    );

    let first_result = execute_with_retry(
        client,
        &initial_url,
        cfg.retries.get(),
        flag_rate_limit,
        cancellation,
    )
    .await?;
    let mut accumulated_attempts = first_result.attempts;

    // Stream + decompress with hard wire cap (never bare `.text()` / unbounded body).
    let first_html = crate::decompress::response_body_string(first_result.response)
        .await
        .map_err(|e| RetryFailReason::Network(e.to_string()))?;

    // v0.8.0 GAP-AUD-003 / GAP-NEW-002: first_body is moved into the result at
    // the end (no early clone). Byte counters is captured while we still
    // only borrow `first_html` for extraction / interstitial checks.
    let accumulated_bytes_in: u64 = first_html.len() as u64;
    let accumulated_bytes_out: u64 = first_html.len() as u64;

    if first_html.len() < SILENT_BLOCK_THRESHOLD {
        // v0.7.9 GAP-WS-58: classify before bailing out. An empty or tiny
        // response that contains a known bot-management marker is a
        // ghost-block (HTTP 200, sub-4KB body, no result structure). The
        // detector returns `Cloudflare`/`DuckDuckGo`; the caller decides
        // what to do. Without classification, the pipeline used to bail
        // out with `RetryFailReason::Blocked` regardless of marker state,
        // which masked the v0.7.7 root cause that the new fallback gate
        // is designed to mitigate.
        let kind = detect_interstitial(&first_html);
        if matches!(
            kind,
            InterstitialKind::Cloudflare | InterstitialKind::DuckDuckGo
        ) {
            tracing::warn!(
                bytes = first_html.len(),
                limiar = SILENT_BLOCK_THRESHOLD,
                kind = kind.as_str(),
                "first page response short + interstitial markers — possible ghost block"
            );
            return Err(RetryFailReason::Blocked);
        }
        tracing::warn!(
            bytes = first_html.len(),
            limiar = SILENT_BLOCK_THRESHOLD,
            "first page response suspiciously small — possible silent block"
        );
        return Err(RetryFailReason::Blocked);
    }

    // Extract results from the first page according to the endpoint.
    // When multi-page HTML is requested, pull pagination tokens in the *same*
    // Html::parse_document (GAP-LAT: avoid a second full parse of first_html).
    // GAP-PAR-030: all SERP parses run via run_cpu_bound (not on the async worker).
    let mut first_page_tokens: Option<(String, String, String)> = None;
    let mut accumulated_results = match initial_endpoint {
        Endpoint::Html if cfg.pages.get() > 1 => {
            let (results, tokens) = extract_results_and_pagination_tokens_async(
                first_html.clone(),
                (*cfg.selectors).clone(),
            )
            .await
            .map_err(|e| RetryFailReason::Network(e.to_string()))?;
            first_page_tokens = tokens;
            results
        }
        Endpoint::Html => extraction::extract_results_with_strategies_cfg_async(
            first_html.clone(),
            (*cfg.selectors).clone(),
        )
        .await
        .map_err(|e| RetryFailReason::Network(e.to_string()))?,
        Endpoint::Lite => extraction::extract_results_lite_with_cfg_async(
            first_html.clone(),
            (*cfg.selectors).clone(),
        )
        .await
        .map_err(|e| RetryFailReason::Network(e.to_string()))?,
    };
    let mut used_fallback_lite = false;
    let mut effective_endpoint = initial_endpoint;
    let mut pages_fetched: u32 = 1;

    // Se HTML retornou zero E estamos no endpoint HTML → tentar Lite como fallback.
    // v0.7.9 GAP-WS-58: ghost_block (sub-`SILENT_BLOCK_THRESHOLD` + no result
    // signal) qualifies for fallback ONLY when `cfg.pre_flight == true`. The
    // legacy path requires `cfg.allow_lite_fallback == true` plus a positive
    // marker classification. Both paths converge in `should_try_lite`.
    let interstitial_kind = detect_interstitial(&first_html);
    let ghost_block = first_html.len() < SILENT_BLOCK_THRESHOLD
        && !crate::probe_deep::has_result_page_signal(&first_html);
    let (try_lite, pre_flight_fired) = should_try_lite(cfg, interstitial_kind, ghost_block);
    let should_attempt_lite_fallback =
        accumulated_results.is_empty() && initial_endpoint == Endpoint::Html && try_lite;

    // v0.7.10 P3: structured log so downstream pipelines (and tests) can
    // verify the pre-flight gate fired without instrumenting every call site.
    // The `metadata.pre_flight_fired` field is populated by `pipeline.rs`
    // and `parallel.rs` from the SearchOutput envelope; this log gives
    // operators immediate observability in the runtime.
    if pre_flight_fired {
        tracing::info!(
            ghost_block_bytes = first_html.len(),
            "pre-flight ghost-block detected; auto-roteando para Lite"
        );
    }

    if accumulated_results.is_empty()
        && initial_endpoint == Endpoint::Html
        && !should_attempt_lite_fallback
    {
        // Log structured suggestion only when the detector flagged
        // interstitial but the flag is disabled — so the operator
        // knows a mitigation path exists (`--allow-lite-fallback`).
        if !cfg.allow_lite_fallback
            && matches!(
                interstitial_kind,
                InterstitialKind::Cloudflare | InterstitialKind::DuckDuckGo
            )
        {
            tracing::warn!(
                kind = interstitial_kind.as_str(),
                "interstitial detected; re-run with --allow-lite-fallback to enable automatic Lite fallback"
            );
        } else {
            tracing::warn!("HTML returned zero results — no interstitial detected");
        }
    }

    if should_attempt_lite_fallback {
        tracing::warn!(
            kind = interstitial_kind.as_str(),
            "interstitial detected — trying Lite fallback"
        );
        let url_lite = build_search_url(
            query,
            cfg.language.as_str(),
            cfg.country.as_str(),
            Endpoint::Lite,
            cfg.time_filter,
            cfg.safe_search,
        );
        match execute_with_retry(
            client,
            &url_lite,
            cfg.retries.get(),
            flag_rate_limit,
            cancellation,
        )
        .await
        {
            Ok(r_lite) => {
                accumulated_attempts = accumulated_attempts.saturating_add(r_lite.attempts);
                let html_lite = crate::decompress::response_body_string(r_lite.response)
                    .await
                    .map_err(|e| RetryFailReason::Network(e.to_string()))?;
                let lite_results = extraction::extract_results_lite_with_cfg_async(
                    html_lite,
                    (*cfg.selectors).clone(),
                )
                .await
                .map_err(|e| RetryFailReason::Network(e.to_string()))?;
                if !lite_results.is_empty() {
                    accumulated_results = lite_results;
                    used_fallback_lite = true;
                    effective_endpoint = Endpoint::Lite;
                }
            }
            Err(err) => {
                tracing::warn!(?err, "Lite fallback also failed — keeping empty");
            }
        }
    }

    // vqd pagination ONLY for the HTML endpoint (Lite does not have this mechanism).
    // AND ONLY if configured for multiple pages.
    // Tokens for page 1 were extracted together with results (single parse).
    if effective_endpoint == Endpoint::Html && cfg.pages.get() > 1 && !accumulated_results.is_empty() {
        if let Some((mut vqd, mut s, mut dc)) = first_page_tokens {
            // Form identical to the hidden form returned by the DOM (discovered
            // empirically on 2026-04-14 / iteration 4): besides `q`/`s`/`dc`/`vqd`/`kl`,
            // DDG expects `nextParams` (empty), `v="l"`, `o="json"`, `api="d.js"`.
            // Built once before the loop; only variable fields (s/dc/vqd) are
            // updated per iteration via clone_from to reuse String capacity.
            let mut form_data: Vec<(String, String)> = vec![
                ("q".to_string(), query.to_string()),      // [0] fixed
                ("s".to_string(), s.clone()),              // [1] variable
                ("nextParams".to_string(), String::new()), // [2] fixed
                ("v".to_string(), "l".to_string()),        // [3] fixed
                ("o".to_string(), "json".to_string()),     // [4] fixed
                ("dc".to_string(), dc.clone()),            // [5] variable
                ("api".to_string(), "d.js".to_string()),   // [6] fixed
                ("vqd".to_string(), vqd.clone()),          // [7] variable
                ("kl".to_string(), format_kl(cfg.language.as_str(), cfg.country.as_str())), // [8] fixed
            ];

            for page_idx in 2..=cfg.pages.get() {
                if cancellation.is_cancelled() {
                    tracing::debug!("cancellation detected during pagination");
                    break;
                }

                // Delay between pages.
                let delay_ms =
                    rand::rng().random_range(PAGINATION_DELAY_MIN_MS..=PAGINATION_DELAY_MAX_MS);
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => { break; }
                    _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                }

                form_data[1].1.clone_from(&s);
                form_data[5].1.clone_from(&dc);
                form_data[7].1.clone_from(&vqd);

                let base = html_base_url();
                let response = match tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => {
                        break;
                    }
                    r = client
                        .post(&base)
                        .header(reqwest::header::REFERER, endpoints::html_referer())
                        .headers(cfg.browser_profile.pagination_headers())
                        .form(&form_data)
                        .send() => r,
                } {
                    Ok(r) => r,
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            pagina = page_idx,
                            "network error during pagination — stopping"
                        );
                        break;
                    }
                };

                if !response.status().is_success() {
                    tracing::warn!(
                        status = response.status().as_u16(),
                        pagina = page_idx,
                        "pagination returned non-success status — stopping"
                    );
                    break;
                }

                let page_html = match crate::decompress::response_body_string(response).await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!(?e, "error reading page body — stopping");
                        break;
                    }
                };

                // Check for silent block on the pagination page.
                if page_html.len() < SILENT_BLOCK_THRESHOLD {
                    tracing::warn!(
                        bytes = page_html.len(),
                        limiar = SILENT_BLOCK_THRESHOLD,
                        pagina = page_idx,
                        "pagination page suspiciously small — possible silent block"
                    );
                    break;
                }

                // Single parse: results + next-page tokens (GAP-LAT + GAP-PAR-030).
                let (new_results, next_tokens) = match extract_results_and_pagination_tokens_async(
                    page_html,
                    (*cfg.selectors).clone(),
                )
                .await
                {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(?e, "pagination parse failed — stopping");
                        break;
                    }
                };
                if new_results.is_empty() {
                    tracing::debug!(pagina = page_idx, "page returned zero results — stopping");
                    break;
                }

                // Renumber positions following the accumulated Vec.
                let offset = u32::try_from(accumulated_results.len()).unwrap_or(u32::MAX);
                for mut r in new_results {
                    r.position = offset.saturating_add(r.position);
                    accumulated_results.push(r);
                }

                pages_fetched = page_idx;

                // Update tokens for the next page; if absent, stop.
                match next_tokens {
                    Some((next_vqd, next_s, next_dc)) => {
                        vqd = next_vqd;
                        s = next_s;
                        dc = next_dc;
                    }
                    None => {
                        tracing::warn!(pagina = page_idx, "pagination tokens missing — stopping");
                        break;
                    }
                }
            }
        } else {
            tracing::warn!("vqd/s/dc tokens missing on first page — pagination not possible");
        }
    }

    // Trunca ao --num se especificado.
    if let Some(n) = cfg.num_results.map(|x| x.get()) {
        let n_usize = n as usize;
        if accumulated_results.len() > n_usize {
            accumulated_results.truncate(n_usize);
        }
    }

    Ok(AggregatedSearchResult {
        results: accumulated_results,
        pages_fetched,
        used_fallback_lite,
        attempts: accumulated_attempts,
        effective_endpoint,
        first_body: first_html, // move — sole remaining owner after all &str uses
        bytes_in: accumulated_bytes_in,
        bytes_out: accumulated_bytes_out,
    })
}
