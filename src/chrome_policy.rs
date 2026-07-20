// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (policy checks; no I/O)
//! Chrome-only transport policy (GAP-WS-113).
//!
//! Always compiled — including builds with `--no-default-features` — so call sites
//! in `lib`, `pipeline`, `content_fetch`, and `parallel` can enforce Chrome-only
//! production without depending on the `chromiumoxide` stack.
//!
//! Residual pure-HTTP paths exist only when the binary is built with
//! `http-test-harness` **and** `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` (test harness only).
//!
//! GAP-SCRAPE-R2-013: product env `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` is **not**
//! read. Production always requires the `chrome` feature; disable Chrome only by
//! building without that feature.

/// Environment variable that previously disabled Chrome at runtime.
///
/// **Deprecated (GAP-SCRAPE-R2-013):** no longer read. Kept as a documented
/// symbol so older docs/tests that mention the name still compile against the
/// constant string if needed.
pub const NO_CHROME_ENV: &str = "DUCKDUCKGO_SEARCH_CLI_NO_CHROME";

/// Env that enables residual HTTP paths when compiled with `http-test-harness`.
pub const HTTP_TEST_ENV: &str = "DUCKDUCKGO_SEARCH_CLI_HTTP_TEST";

/// Always `false` — product env kill-switch removed (GAP-SCRAPE-R2-013).
///
/// Call sites keep using this predicate for `chrome_attempted` metadata; it
/// never reports “disabled by env” in production.
#[must_use]
pub fn chrome_disabled_by_env() -> bool {
    false
}

/// Residual HTTP test harness is active (feature + env).
///
/// Production binaries compile this to always `false` when the feature is off.
#[must_use]
pub fn http_test_harness_active() -> bool {
    #[cfg(feature = "http-test-harness")]
    {
        std::env::var(HTTP_TEST_ENV).as_deref() == Ok("1")
    }
    #[cfg(not(feature = "http-test-harness"))]
    {
        false
    }
}

/// GAP-WS-113: every production network operation must use chromiumoxide.
///
/// Fails when the binary was built without feature `chrome` (unless the HTTP
/// test harness is active).
///
/// # Errors
///
/// - [`crate::error::CliError::ChromeUnavailable`] when the binary was built without
///   feature `chrome` (production requires chromiumoxide).
pub fn require_chrome_transport() -> Result<(), crate::error::CliError> {
    if http_test_harness_active() {
        return Ok(());
    }

    #[cfg(not(feature = "chrome"))]
    {
        Err(crate::error::CliError::chrome_unavailable(
            "chrome transport is mandatory (GAP-WS-113); rebuild with --features chrome \
             (default); pure HTTP is not a production transport",
        ))
    }

    #[cfg(feature = "chrome")]
    {
        Ok(())
    }
}
