// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (wire error code constants)
//! Stable JSON / agent wire error codes (specification section 14.3).
//!
//! These strings are part of the agent-facing stdout contract. They stay
//! English and stable across locales (UI copy is localised separately via
//! [`crate::i18n`]).

/// HTTP-level failure (timeout, connection refused, non-2xx status).
pub const HTTP_ERROR: &str = "http_error";
/// Persistent rate limiting (HTTP 429 after exhausting retries).
pub const RATE_LIMITED: &str = "rate_limited";
/// Anti-bot blocking detected (HTTP 202 anomaly or persistent 403).
pub const BLOCKED: &str = "blocked";
/// Zero organic results across all queries.
pub const NO_RESULTS_FOUND: &str = "no_results_found";
/// Global timeout exceeded.
pub const TIMEOUT: &str = "timeout";
/// Cooperative cancel via SIGINT/SIGTERM.
pub const CANCELLED: &str = "cancelled";
/// Chrome/Chromium executable not found on the system.
pub const CHROME_NOT_FOUND: &str = "chrome_not_found";
/// Chrome transport unavailable or disabled (GAP-WS-113 Chrome-only).
pub const CHROME_UNAVAILABLE: &str = "chrome_unavailable";
/// Historical wire code for the removed product env kill-switch (GAP-WS-113 /
/// GAP-SCRAPE-R2-013). `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` is **not** read;
/// Chrome is required via feature `chrome` only.
pub const CHROME_DISABLED_BY_ENV: &str = "chrome_disabled_by_env";
/// Low-level network error (DNS, TLS, connection reset).
pub const NETWORK_ERROR: &str = "network_error";
/// Proxy configuration or connection failure.
pub const PROXY_ERROR: &str = "proxy_error";
/// Invalid CLI configuration (incompatible arguments, bad values).
pub const INVALID_CONFIG: &str = "invalid_config";
/// Output path is invalid (path traversal, system directory).
pub const PATH_ERROR: &str = "path_error";
/// Consumer closed the pipe (SIGPIPE / `BrokenPipe`).
pub const BROKEN_PIPE: &str = "broken_pipe";
/// Pipeline invariant violation — internal state reached an impossible branch.
///
/// Emitted instead of aborting the process when a code path that the type
/// system cannot prove unreachable is in fact reached. v0.8.0 — closes GAP-NEW-013.
pub const PIPELINE_INVARIANT_VIOLATION: &str = "pipeline_invariant_violation";
