// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (pure range checks over parsed argv, zero runtime).
//! Domain validators for [`CliArgs`] search flags (GAP-CLI-MOD-SPLIT).

use super::buscar_args::CliArgs;
use super::{
    MAX_CONTENT_LENGTH_LIMIT, MAX_PAGES, MAX_PARALLELISM, MAX_PER_HOST_LIMIT, MAX_RETRIES,
};

/// Shared `[1, max]` range check for numeric flags (DRY: single message SSOT).
///
/// `field_name` is the long flag spelling used verbatim in the error text.
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when `value` is zero or above `max`.
fn validate_range(value: u64, max: u64, field_name: &str) -> Result<(), crate::error::CliError> {
    if value == 0 {
        return Err(crate::error::CliError::invalid_config(format!(
            "{field_name} must be at least 1 (got {value})"
        )));
    }
    if value > max {
        return Err(crate::error::CliError::invalid_config(format!(
            "{field_name} cannot exceed {max} (got {value})"
        )));
    }
    Ok(())
}

impl CliArgs {
    /// Validates that the parallelism degree is within the range `[1, MAX_PARALLELISM]`.
    ///
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when the operation fails.
    pub fn validate_parallelism(&self) -> Result<(), crate::error::CliError> {
        validate_range(
            u64::from(self.parallelism),
            u64::from(MAX_PARALLELISM),
            "--parallel",
        )
    }

    /// Validates that the number of pages is within the range `[1, MAX_PAGES]`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when pages is out of range.
    pub fn validate_pages(&self) -> Result<(), crate::error::CliError> {
        validate_range(u64::from(self.pages), u64::from(MAX_PAGES), "--pages")
    }

    /// Validates that `--max-content-length` is within the range `[1, MAX_CONTENT_LENGTH_LIMIT]`.
    ///
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when the operation fails.
    pub fn validate_max_content_length(&self) -> Result<(), crate::error::CliError> {
        validate_range(
            self.max_content_length as u64,
            MAX_CONTENT_LENGTH_LIMIT as u64,
            "--max-content-length",
        )
    }

    /// v0.7.10 B3 fix: removed from `CliArgs` because the field is
    /// hoisted to `RootArgs`. The corresponding `validate_global_timeout`
    #[allow(clippy::empty_line_after_doc_comments)]
    /// Validates that `--proxy`, when provided, is a parseable URL with a supported scheme.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when the proxy URL is invalid.
    pub fn validate_proxy(&self) -> Result<(), crate::error::CliError> {
        let Some(url) = self.proxy.as_deref() else {
            return Ok(());
        };
        let parsed = url::Url::parse(url).map_err(|e| {
            crate::error::CliError::proxy_error(format!("invalid --proxy URL ({url:?}): {e}"))
        })?;
        match parsed.scheme() {
            "http" | "https" | "socks5" | "socks5h" => Ok(()),
            other => Err(crate::error::CliError::proxy_error(format!(
                "scheme {other:?} not supported in --proxy (use http/https/socks5)"
            ))),
        }
    }

    /// Validates that the number of retries is within the range `[0, MAX_RETRIES]`.
    ///
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when the operation fails.
    pub fn validate_retries(&self) -> Result<(), crate::error::CliError> {
        if self.retries > MAX_RETRIES {
            return Err(crate::error::CliError::invalid_config(format!(
                "--retries cannot exceed {} (got {})",
                MAX_RETRIES, self.retries
            )));
        }
        Ok(())
    }

    /// Validates that `--per-host-limit` is within the range `[1, MAX_PER_HOST_LIMIT]`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when per-host limit is out of range.
    pub fn validate_per_host_limit(&self) -> Result<(), crate::error::CliError> {
        validate_range(
            u64::from(self.per_host_limit),
            u64::from(MAX_PER_HOST_LIMIT),
            "--per-host-limit",
        )
    }

    /// Validates that `--timeout` is at least 1 second.
    ///
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when the operation fails.
    pub fn validate_timeout_seconds(&self) -> Result<(), crate::error::CliError> {
        if self.timeout_seconds == 0 {
            return Err(crate::error::CliError::invalid_config(format!(
                "--timeout must be at least 1 (got {})",
                self.timeout_seconds
            )));
        }
        Ok(())
    }
}
