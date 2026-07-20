// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (CDP navigation + DOM extract).
// Parallelism: caller holds exclusive `&mut ChromeBrowser` (serial per process).
//! SERP/content extraction helpers over an existing [`ChromeBrowser`] session.

use super::session::{apply_ua_override, ChromeBrowser};
use super::stealth::{STEALTH_PLATFORM_SCRIPT, STEALTH_SCRIPTS};
use super::{
    CONTENT_JS_SETTLE_MS, MIN_LINE_LENGTH, NEWS_POST_READY_SETTLE_MS, SERP_POLL_ATTEMPTS,
    SERP_POLL_INTERVAL_MS, SERP_POLL_MIN_BUDGET_SECS, SERP_WARMUP_BASE_MS, SERP_WARMUP_JITTER_MS,
};
use std::path::Path;
use crate::error::CliError;
use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
use std::time::Duration;

/// Shared CDP bootstrap after `new_page` (GAP-DRY-002): UA override, stealth
/// scripts, SERP origin warm-up navigation, and jittered settle delay.
async fn prepare_page_stealth(
    page: &chromiumoxide::Page,
    ua_str: &str,
    ua_major: u32,
    url: &str,
) {
    apply_ua_override(page, ua_str, ua_major).await;
    let stealth_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_SCRIPTS);
    // Best-effort: stealth inject failure must not abort SERP (CDP optional scripts).
    if let Err(err) = page.execute(stealth_cmd).await {
        tracing::debug!(?err, "stealth script inject failed (best-effort)");
    }
    let platform_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_PLATFORM_SCRIPT);
    if let Err(err) = page.execute(platform_cmd).await {
        tracing::debug!(?err, "platform script inject failed (best-effort)");
    }

    // GAP-WS-077: warm-up navigation to SERP origin (endpoints module).
    let warmup = crate::endpoints::serp_base_url();
    // Best-effort warm-up: main navigation still proceeds if origin warm-up fails.
    if let Err(err) = page.goto(warmup.as_str()).await {
        tracing::debug!(?err, "serp warm-up goto failed (best-effort)");
    }
    if let Err(err) = page.wait_for_navigation().await {
        tracing::debug!(?err, "serp warm-up wait failed (best-effort)");
    }
    tokio::time::sleep(Duration::from_millis(
        SERP_WARMUP_BASE_MS + (url.len() as u64 % SERP_WARMUP_JITTER_MS),
    ))
    .await;
}


/// Extracts raw HTML from a URL using headless Chrome with stealth injection.
///
/// Strategy:
/// 1. Opens a blank page and injects `navigator.webdriver = false` via CDP.
/// 2. Navigates to the target URL.
/// 3. Waits for navigation completion + [`super::CONTENT_JS_SETTLE_MS`] for JS rendering.
/// 4. Extracts `document.documentElement.outerHTML`.
/// 5. Truncates at `max_size` bytes and closes the page.
///
/// The `timeout` applies to the entire operation via `tokio::time::timeout`.
///
/// # Errors
///
/// Returns an error if the page cannot be opened, JS evaluation fails,
/// or the operation exceeds `timeout`.
///
/// # Cancel safety
///
/// This function is cancel-safe. The outer `tokio::time::timeout` wraps
/// the entire navigation, so dropping the future aborts the CDP session
/// and releases the browser tab.
pub async fn extract_html_with_chrome(
    browser: &mut ChromeBrowser,
    url: &str,
    max_size: usize,
    timeout: Duration,
) -> Result<String, CliError> {
    let work = async {
        // GAP-WS-109 v0.9.2: capture effective UA + major before borrowing the
        // browser mutably via `browser_mut()` (avoids borrow-checker conflict).
        let ua_str = browser.effective_ua().to_string();
        let ua_major = browser.chrome_major();

        let page = browser
            .browser_mut()
            .new_page("about:blank")
            .await
            .map_err(|e| {
                CliError::http_with_source(format!("failed to open blank page for {url:?}"), e)
            })?;

        // GAP-DRY-002: shared stealth + SERP warm-up bootstrap.
        prepare_page_stealth(&page, &ua_str, ua_major, url).await;

        // Navigate to the target URL.
        page.goto(url).await.map_err(|e| {
            CliError::http_with_source(format!("failed to navigate to {url:?}"), e)
        })?;

        // Wait for full navigation to complete (respects redirects).
        let _ = page.wait_for_navigation().await;

        // Poll for real SERP: Cloudflare may serve a JS challenge that
        // auto-resolves after a few seconds (GAP-SCRAPE-R-003 named consts).
        let mut raw_html = String::new();
        for attempt in 0..SERP_POLL_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(SERP_POLL_INTERVAL_MS)).await;
            let js_result = page
                .evaluate("document.documentElement.outerHTML")
                .await
                .map_err(|e| {
                    CliError::http_with_source(
                        format!("failed to extract outerHTML on {url:?}"),
                        e,
                    )
                })?;
            raw_html = js_result.into_value().unwrap_or_default();
            if raw_html.contains("result__a") || raw_html.contains("result__snippet") {
                tracing::info!(attempt, "SERP detected after polling");
                break;
            }
            if attempt == 15 {
                tracing::info!(
                    body_len = raw_html.len(),
                    "polling exhausted — using last HTML"
                );
            }
        }

        // Close the page immediately to release the target.
        let _ = page.close().await;

        // Truncate at byte boundary.
        if raw_html.len() > max_size {
            Ok::<String, CliError>(raw_html[..max_size].to_string())
        } else {
            Ok::<String, CliError>(raw_html)
        }
    };

    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| CliError::http_msg(format!("chrome timeout exceeded for {url:?}")))?
}

/// Polls the most recently opened page until `selector` matches in the
/// rendered DOM, or `timeout` elapses. GAP-WS-104 v0.8.9.
///
/// Returns `true` as soon as `document.querySelector(selector)` yields an
/// element, `false` on timeout or when no page is open. A `false` return is
/// NOT fatal: callers extract the last HTML anyway and let the extraction
/// cascade decide (Strategy B may still recover results).
///
/// # Cancel safety
///
/// Cancel-safe: the loop only awaits `page.evaluate` and `tokio::time::sleep`.
pub async fn wait_for_selector_with_chrome(
    browser: &mut ChromeBrowser,
    selector: &str,
    poll_interval: Duration,
    timeout: Duration,
) -> bool {
    let pages = match browser.browser_mut().pages().await {
        Ok(pages) => pages,
        Err(error) => {
            tracing::warn!(%error, "wait_for_selector: failed to list pages");
            return false;
        }
    };
    let Some(page) = pages.last() else {
        tracing::warn!("wait_for_selector: no open page to poll");
        return false;
    };
    wait_for_selector_on_page(page, selector, poll_interval, timeout).await
}

/// Core polling loop shared by [`wait_for_selector_with_chrome`] and
/// [`extract_news_html_with_chrome`]. Uses `tokio::time::sleep` between
/// attempts (never blocks the async runtime).
async fn wait_for_selector_on_page(
    page: &chromiumoxide::Page,
    selector: &str,
    poll_interval: Duration,
    timeout: Duration,
) -> bool {
    wait_for_any_selector_on_page(page, &[selector], poll_interval, timeout).await
}

/// Polls until **any** of `selectors` matches (GAP-WS-AGENT-READY-001 L-04).
///
/// News SERP React markup is fragile; waiting only on
/// `[data-react-module-id="news"]` often times out while article cards already
/// exist. Callers pass a cascade of selectors.
async fn wait_for_any_selector_on_page(
    page: &chromiumoxide::Page,
    selectors: &[&str],
    poll_interval: Duration,
    timeout: Duration,
) -> bool {
    if selectors.is_empty() {
        return false;
    }
    // Build OR of querySelector checks; escape each selector for single-quoted JS.
    let parts: Vec<String> = selectors
        .iter()
        .map(|selector| {
            let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
            format!("!!document.querySelector('{escaped}')")
        })
        .collect();
    let js = format!("({})", parts.join("||"));
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        // `Evaluation: From<&str>` — borrow the same JS each poll; no per-iter clone.
        match page.evaluate(js.as_str()).await {
            Ok(result) => {
                if result.into_value::<bool>().unwrap_or(false) {
                    return true;
                }
            }
            Err(error) => {
                tracing::trace!(%error, "wait_for_selector: evaluate failed — retrying");
            }
        }
        if tokio::time::Instant::now() + poll_interval > deadline {
            tracing::info!(
                selectors = ?selectors,
                "wait_for_selector: timeout — none of the selectors found"
            );
            return false;
        }
        tokio::time::sleep(poll_interval).await;
    }
}

/// Extracts the Chrome-rendered HTML of a news SERP (`ia=news&iar=news`),
/// polling for the React news module before reading `outerHTML`.
/// GAP-WS-104 v0.8.9.
///
/// Mirrors [`extract_html_with_chrome`] (stealth injection + GAP-WS-077
/// warm-up), but instead of polling for static web-SERP markers it polls
/// `wait_selector` — the news module only exists after JavaScript
/// hydration. On poll timeout the last HTML is still returned so the
/// extraction cascade can decide.
///
/// # Errors
///
/// Returns an error if the page cannot be opened, navigation or JS
/// evaluation fails, or the operation exceeds `timeout`.
///
/// # Cancel safety
///
/// Cancel-safe: the outer `tokio::time::timeout` wraps the entire
/// navigation, so dropping the future aborts the CDP session.
pub async fn extract_news_html_with_chrome(
    browser: &mut ChromeBrowser,
    url: &str,
    wait_selector: &str,
    poll_interval: Duration,
    max_size: usize,
    timeout: Duration,
    dump_path: Option<&Path>,
) -> Result<String, CliError> {
    let work = async {
        // GAP-WS-109 v0.9.2: capture effective UA + major before borrowing the
        // browser mutably via `browser_mut()` (avoids borrow-checker conflict).
        let ua_str = browser.effective_ua().to_string();
        let ua_major = browser.chrome_major();

        let page = browser
            .browser_mut()
            .new_page("about:blank")
            .await
            .map_err(|e| {
                CliError::http_with_source(format!("failed to open blank page for {url:?}"), e)
            })?;

        // GAP-DRY-002: shared stealth + SERP warm-up bootstrap.
        prepare_page_stealth(&page, &ua_str, ua_major, url).await;

        // Navigate to the news SERP.
        page.goto(url).await.map_err(|e| {
            CliError::http_with_source(format!("failed to navigate to {url:?}"), e)
        })?;
        let _ = page.wait_for_navigation().await;

        // Poll for React news hydration (v0.9.9):
        // Do NOT treat bare `article` / `.result__a` as ready — those appear in
        // chrome/footer and stop the poll before /news.js populates the vertical
        // (live e2e: premature extract → only promo links + no-results-message).
        let poll_budget = (timeout / 2)
            .max(Duration::from_secs(SERP_POLL_MIN_BUDGET_SECS))
            .min(timeout);
        let news_ready_selectors: &[&str] = &[
            wait_selector,
            "[data-testid=\"news-vertical\"] article",
            "[data-testid=\"news-vertical\"] a[data-testid=\"result-title-a\"]",
            "[data-react-module-id=\"news\"] article",
            "[data-testid=\"news-vertical\"] a[href*=\"uddg=\"]",
            // Terminal empty state after news API settles.
            "[data-testid=\"no-results-message\"]",
        ];
        let found =
            wait_for_any_selector_on_page(&page, news_ready_selectors, poll_interval, poll_budget)
                .await;
        if !found {
            tracing::warn!(
                primary = wait_selector,
                "news module not detected after multi-selector polling — extracting last HTML anyway"
            );
        }
        // Extra settle: news.js XHR may still paint cards after first selector match.
        tokio::time::sleep(Duration::from_millis(NEWS_POST_READY_SETTLE_MS)).await;

        let js_result = page
            .evaluate("document.documentElement.outerHTML")
            .await
            .map_err(|e| {
                CliError::http_with_source(format!("failed to extract outerHTML on {url:?}"), e)
            })?;
        let raw_html: String = js_result.into_value().unwrap_or_default();

        // Local-only debug dump (GAP-SCRAPE-R-008): CLI `--dump-news-html` path only.
        if let Some(dump_path) = dump_path {
            let path_str = dump_path.to_string_lossy();
            if !path_str.is_empty() && !path_str.contains("..") {
                if let Err(e) = std::fs::write(dump_path, &raw_html) {
                    tracing::warn!(error = %e, path = %dump_path.display(), "failed to dump news HTML");
                } else {
                    tracing::info!(
                        path = %dump_path.display(),
                        bytes = raw_html.len(),
                        "dumped news SERP HTML"
                    );
                }
            }
        }

        // Close the page immediately to release the target.
        let _ = page.close().await;

        // Truncate at a valid UTF-8 boundary (the news SERP is heavy —
        // callers pass a 1 MiB cap instead of the web-SERP 256 KiB).
        if raw_html.len() > max_size {
            let mut end = max_size;
            while end > 0 && !raw_html.is_char_boundary(end) {
                end -= 1;
            }
            Ok::<String, CliError>(raw_html[..end].to_string())
        } else {
            Ok::<String, CliError>(raw_html)
        }
    };

    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| CliError::http_msg(format!("chrome timeout exceeded for {url:?}")))?
}

/// Extracts the main text from a URL using headless Chrome.
///
/// Wrapper over [`extract_html_with_chrome`] that applies text cleaning
/// (normalizes whitespace, discards short lines, truncates at `max_size`).
///
/// # Errors
///
/// Returns an error if the page cannot be opened, JS evaluation fails,
/// or the operation exceeds `timeout`.
///
/// # Cancel safety
///
/// This function is cancel-safe. The outer `tokio::time::timeout` wraps
/// the entire navigation, so dropping the future aborts the CDP session
/// and releases the browser tab.
pub async fn extract_text_with_chrome(
    browser: &mut ChromeBrowser,
    url: &str,
    max_size: usize,
    timeout: Duration,
) -> Result<String, CliError> {
    let work = async {
        // GAP-WS-109 v0.9.2: capture effective UA + major before borrowing the
        // browser mutably via `browser_mut()` (avoids borrow-checker conflict).
        let ua_str = browser.effective_ua().to_string();
        let ua_major = browser.chrome_major();

        let page = browser
            .browser_mut()
            .new_page("about:blank")
            .await
            .map_err(|e| {
                CliError::http_with_source(format!("failed to open blank page for {url:?}"), e)
            })?;

        // Content fetch (not SERP): stealth + UA only — no SERP origin warm-up.
        apply_ua_override(&page, &ua_str, ua_major).await;
        let stealth_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_SCRIPTS);
        // Best-effort: optional CDP scripts must not abort content fetch.
        if let Err(err) = page.execute(stealth_cmd).await {
            tracing::debug!(?err, "content stealth inject failed (best-effort)");
        }
        let platform_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_PLATFORM_SCRIPT);
        if let Err(err) = page.execute(platform_cmd).await {
            tracing::debug!(?err, "content platform inject failed (best-effort)");
        }

        // Navigate to the target URL.
        page.goto(url).await.map_err(|e| {
            CliError::http_with_source(format!("failed to navigate to {url:?}"), e)
        })?;

        // Best-effort: navigation wait may race with already-settled loads.
        if let Err(err) = page.wait_for_navigation().await {
            tracing::debug!(?err, "content wait_for_navigation failed (best-effort)");
        }

        // Allow time for JS rendering (named settle policy — GAP-SCRAPE-009).
        tokio::time::sleep(Duration::from_millis(CONTENT_JS_SETTLE_MS)).await;

        let js_result = page
            .evaluate("document.body ? document.body.innerText : ''")
            .await
            .map_err(|e| {
                CliError::http_with_source(format!("failed to execute innerText on {url:?}"), e)
            })?;

        let raw_text: String = js_result.into_value().unwrap_or_default();

        // Close the page immediately to release the target.
        let _ = page.close().await;

        Ok::<String, CliError>(clean_text(&raw_text, max_size))
    };

    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| CliError::http_msg(format!("chrome timeout exceeded for {url:?}")))?
}

/// Cleans raw text: normalizes whitespace, discards short lines, truncates at `max_size`.
pub(crate) fn clean_text(raw: &str, max_size: usize) -> String {
    let lines: Vec<String> = raw
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| line.chars().count() >= MIN_LINE_LENGTH)
        .collect();
    let joined = lines.join("\n");
    truncate_at_word(&joined, max_size)
}

/// Truncates respecting word boundary. Mirrors the implementation in `content.rs`.
fn truncate_at_word(text: &str, max_size: usize) -> String {
    if max_size == 0 {
        return String::new();
    }
    let total: usize = text.chars().count();
    if total <= max_size {
        return text.to_string();
    }
    let prefix: String = text.chars().take(max_size).collect();
    if let Some(pos) = prefix.rfind(char::is_whitespace) {
        return prefix[..pos].trim_end().to_string();
    }
    prefix
}

