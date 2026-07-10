// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (env flag checks; no I/O)
//! Chrome-only transport policy (GAP-WS-113).
//!
//! Always compiled — including builds with `--no-default-features` — so call sites
//! in `lib`, `pipeline`, `content_fetch`, and `parallel` can enforce Chrome-only
//! production without depending on the `chromiumoxide` stack.
//!
//! Residual pure-HTTP paths exist only when the binary is built with
//! `http-test-harness` **and** `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`.

/// Environment variable that disables Chrome at runtime.
///
/// GAP-WS-113: in production this env is **forbidden** — all network ops require
/// chromiumoxide. Residual HTTP is only available under `http-test-harness`.
pub const NO_CHROME_ENV: &str = "DUCKDUCKGO_SEARCH_CLI_NO_CHROME";

/// Env that enables residual HTTP paths when compiled with `http-test-harness`.
pub const HTTP_TEST_ENV: &str = "DUCKDUCKGO_SEARCH_CLI_HTTP_TEST";

/// Returns true when `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`.
#[must_use]
pub fn chrome_disabled_by_env() -> bool {
    std::env::var(NO_CHROME_ENV).as_deref() == Ok("1")
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
/// Fails when:
/// - the binary was built without feature `chrome`
/// - `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` (unless HTTP test harness is active)
///
/// # Errors
///
/// Returns [`CliError::InvalidConfig`] with actionable remediation text.
pub fn require_chrome_transport() -> Result<(), crate::error::CliError> {
    use crate::error::CliError;

    if http_test_harness_active() {
        return Ok(());
    }

    #[cfg(not(feature = "chrome"))]
    {
        return Err(CliError::InvalidConfig {
            message: "Chrome transport is mandatory (GAP-WS-113). Rebuild with --features chrome \
                       (default). Pure HTTP is not a production transport."
                .into(),
        });
    }

    #[cfg(feature = "chrome")]
    {
        if chrome_disabled_by_env() {
            return Err(CliError::InvalidConfig {
                message: "DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1 is forbidden (GAP-WS-113). \
                           All network operations require chromiumoxide/CDP. Install Chrome/Chromium \
                           or pass --chrome-path. Residual HTTP exists only behind the \
                           http-test-harness compile feature for wiremock tests."
                    .into(),
            });
        }
        Ok(())
    }
}
