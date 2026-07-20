// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound + CPU (HTTP fetch + readability extraction)
//! Full text content extraction from URLs (flag `--fetch-content`).
//!
//! ## Layout (Pass 45 / GAP-SCRAPE-011 — SRP split)
//!
//! | Submodule | Responsibility |
//! |-----------|----------------|
//! | [`ssrf`] | Structural + async DNS SSRF gate (HTTP **and** Chrome) |
//! | [`encoding`] | BOM / Content-Type / meta charset / WINDOWS_1252 decode |
//! | [`readability`] | scraper-based main-content text extraction |
//! | [`http_extract`] | residual pure-HTTP fetch path (test harness) |
//!
//! Production `--fetch-content` uses Chrome (`content_fetch` + `browser::extract`).
//! Residual HTTP exists for the `http-test-harness` feature only.
//!
//! ## Scraping policy (Pass 45 / operator mandate)
//!
//! This CLI **does not** download or honor `robots.txt` (REP). It is a one-shot
//! search client with optional page enrichment for LLM agents, not a site-wide
//! polite crawler. Outbound load is still bounded by Semaphore, per-host limits,
//! jitter, circuit breaker, and HTTP retry/`Retry-After` — not by robots Crawl-delay.
//!
//! # Threat model
//!
//! - Every fetch URL is attacker-influenced (SERP links): structural SSRF filter,
//!   async DNS of all A/AAAA records, and post-redirect re-check (HTTP) plus
//!   pre-navigation gate (Chrome).
//! - Response bodies are hostile: hard byte cap, charset conversion, readability
//!   truncation; never shell out on content.

mod encoding;
mod http_extract;
mod readability;
mod ssrf;

pub use encoding::decode_to_utf8;
pub use http_extract::extract_http_content;
pub(crate) use ssrf::{is_safe_url, url_is_safe_to_fetch};
