// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: orchestrator (config assembly, delegation to pipeline)
// html_root_url requires a string literal (no env!/concat! in this attr).
// Keep in sync with package.version in Cargo.toml (docs.rs deep links).
#![doc(html_root_url = "https://docs.rs/duckduckgo-search-cli/1.0.2")]
#![doc(html_playground_url = "https://play.rust-lang.org")]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::private_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::invalid_codeblock_attributes)]
#![warn(rustdoc::invalid_html_tags)]
#![warn(rustdoc::bare_urls)]
#![warn(rustdoc::redundant_explicit_links)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::multiple_unsafe_ops_per_block)]
#![deny(unsafe_op_in_unsafe_fn)]
// Unit-test-only clippy noise (const asserts, Default-then-assign fixtures, test panic helpers).
// Production `cargo clippy --lib -D warnings` stays clean without these allows.
#![cfg_attr(
    test,
    allow(
        clippy::const_is_empty,
        clippy::assertions_on_constants,
        clippy::field_reassign_with_default,
        clippy::manual_clamp,
        clippy::missing_panics_doc,
    )
)]
// Cannot `forbid(unsafe_code)`: platform FFI (Windows console, libc kill/pre_exec,
// SIGPIPE) requires minimal documented `unsafe`. Density inventory (Pass 44):
// - `signals::restore_sigpipe` — keeps SIG_IGN (Rust default) so one-shot reap runs on EPIPE (Unix)
// - `platform::init` — Win32 console UTF-8 / VTP (Windows)
// - `process_lifecycle::unix` — `libc::kill` / `pre_exec` setpgid+prctl (Unix)
// - `process_lifecycle::windows` — OpenProcess/Terminate/Toolhelp (Windows)
// Zero transmute/from_raw/static mut/union. See gaps.md §AP.
//! # duckduckgo-search-cli
//!
//! Rust CLI for searching `DuckDuckGo` via real Chrome (`chromiumoxide`/CDP), with structured JSON
//! output for LLM agents. No paid API. Production network is Chrome-only (GAP-WS-113).
//! No cache. Universal cross-platform (Linux including Alpine/NixOS/Flatpak/Snap,
//! macOS including Apple Silicon, Windows including cmd.exe and `PowerShell`).
//!
//! ## Module Structure
//!
//! | Module        | Responsibility                                               |
//! |---------------|--------------------------------------------------------------|
//! | [`cli`]       | Clap structs (command-line argument parsing).                |
//! | [`http`]      | `reqwest::Client` construction and User-Agent selection.  |
//! | [`search`]    | URL building and HTTP request to the `DuckDuckGo` endpoint.    |
//! | [`extraction`]| HTML parsing with `scraper` and ad filtering.                |
//! | [`pipeline`]  | Single/multi orchestration, deduplication and source reading.|
//! | [`parallel`]  | Multi-query fan-out with `JoinSet`, Semaphore, `CancellationToken`.|
//! | [`concurrency`] | Bounded-concurrency policy (`--parallel` / `--max-concurrency`).|
//! | [`output`]    | JSON/stdout payload + human stderr via [`output::emit_stderr`] (MP-06).|
//! | [`platform`]  | Cross-platform initialization (UTF-8 on Windows, TTY detect).|
//! | [`types`]     | Shared structs and enums.                                    |
//! | [`error`]     | Error codes and exit codes (`is_retryable` for agents).      |
//! | [`retry`]     | Named `RetryConfig`, full-jitter backoff, `Retry-After`.     |
//! | [`security`]  | Threat model + STRIDE + [`ValidatedQuery`] boundary.           |
//! | [`content`]   | SSRF + encoding + readability; residual HTTP for harness. |
//! | [`content_fetch`] | Parallel `--fetch-content` (Chrome pool + Semaphore). |
//! | [`selectors`] | Loading of external `SelectorConfig` (iter. 6).      |
//! | [`signals`]   | Cross-platform signal handlers (SIGPIPE, Ctrl+C).            |
//! | [`config_init`] | `init-config` subcommand (iter. 6).                       |
//! | [`paths`]     | Path validation and sanitization for I/O.                    |
//! | [`logging`] | Local stderr logging (developer diagnostics; no product telemetry).    |
//! | [`i18n`]      | UI locale (`en`/`pt-BR`) for human stderr; stdout stays EN.  |
//! | `browser`     | Headless Chrome cross-platform under feature `chrome` (iter.7).|
//! | [`chrome_policy`] | Always-on GAP-WS-113 Chrome-only transport policy.        |
//!
//! ## Features
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `chrome` | **yes** | Production network transport via real Chrome (`chromiumoxide`/CDP). Required for SERP, news, deep-research, probe, pre-flight, and content fetch. |
//! | `http-test-harness` | no | Residual HTTP SERP/probe paths for tests only (`DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`). Never a silent production SERP path. |
//! | `console` | no | Composes `tokio-console` with the stderr fmt layer (`RUSTFLAGS=--cfg tokio_unstable cargo run --features console`). |
//!
//! Feature-gated items are described in prose (not `#[doc(cfg(...))]`) so docs build on
//! **stable** and on docs.rs without nightly `#![feature(doc_cfg)]` (Oct 2025 `doc_auto_cfg` → `doc_cfg` merge).
//!
//! ## Scraping / robots.txt policy (Pass 45)
//!
//! This CLI **does not** fetch or honor `robots.txt` (operator product mandate). It is a
//! one-shot DuckDuckGo search client with optional page enrichment for agents — not a
//! site-wide polite crawler. Outbound load is bounded by Semaphore, per-host limits,
//! stagger jitter, circuit breaker, and HTTP `Retry-After` — not by REP Crawl-delay.
//! Attacker-influenced SERP URLs still pass the shared SSRF gate before HTTP or Chrome
//! navigation. See `gaps.md` §AQ / N/A-SCRAPE-001.
//!
//! ## Entry Point
//!
//! The public function [`run`] is called by `main.rs` and returns an exit code
//! as specified in section 17.7 of the specification.

pub mod aggregation;
pub mod chrome_policy;
pub mod cli;
pub mod concurrency;
pub mod budget;
pub mod cgroup;
pub mod process_count;
pub mod runtime;
pub mod commands;
pub mod config;
pub mod config_init;
pub mod content;
pub mod content_fetch;
pub mod cookie_adapter;
pub mod decomposition;
pub mod decompress;
pub mod deep_research;
pub mod endpoints;
pub mod error;
pub mod extraction;
pub mod http;
pub mod i18n;
pub mod identity;
pub mod output;
pub mod parallel;
pub mod paths;
pub mod pipeline;
pub mod platform;
pub mod probe_deep;
pub mod zero_cause;
pub use zero_cause::{
    set_zero_cause_strict, zero_cause_is_non_legitimate, zero_cause_strict,
};
pub mod probe;
pub mod retry;
pub mod search;
pub mod security;
pub mod selectors;
pub mod session_warmup;
pub mod signals;
pub mod synthesis;
pub mod logging;
pub(crate) use logging::initialize_logging_for_command;
/// Process-wide rustls CryptoProvider install (binary `main` + tests).
pub mod tls_bootstrap;
pub mod types;
pub mod validation;

// browser.rs declares `#![cfg(feature = "chrome")]` at the module root (line 25),
// which already excludes the entire module when the feature is off. Re-declaring
// `#[cfg(feature = "chrome")]` here is redundant and triggers clippy::duplicated_attributes.
// The previous `#[cfg_attr(docsrs, doc(cfg(...)))]` was removed in v0.6.6 because
// `doc(cfg)` is unstable and requires `#![feature(doc_cfg)]` since doc_auto_cfg
// was merged into doc_cfg in Oct 2025 (see rust-lang/rust#43781).
//
// Transport policy (require_chrome_transport / http_test_harness_active) lives in
// `chrome_policy` and is always compiled — see GAP-WS-113 / no-default-features CI.
pub mod browser;

// GAP-WS-LIFECYCLE-001: process-group / tree reap helpers (feature-gated like browser).
#[cfg(feature = "chrome")]
pub mod process_lifecycle;

// Long calibration query for probe-deep (GAP-WS-51).
//
// DuckDuckGo trata queries curtas e longas de forma diferente: queries
// de 1 palavra raramente acionam o sistema de bot detection, fazendo
// com que `--probe-deep` retorne "ok" mesmo quando uma query real de
// production would be blocked. This 43-character string ensures that
// the HTTP payload has a realistic size, replicating the real scenario.


mod run;
pub use run::run;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
