// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (domain newtypes — parse don't validate)
//! Bounded domain newtypes for configuration scalars and SERP codes.
//!
//! Invariants live in the type (private field + [`try_new`]), not in comments
//! at call sites. All wrappers are `#[repr(transparent)]` zero-cost abstractions.
//!
//! **Never** implement [`std::ops::Deref`] on these types (type-safety rules).

use crate::error::CliError;
use std::num::NonZeroU64;

// ---------------------------------------------------------------------------
// Bounds (SSOT — keep aligned with clap ranges in `cli.rs`)
// ---------------------------------------------------------------------------

/// Maximum per-request HTTP timeout (seconds).
pub const MAX_TIMEOUT_SECONDS: u64 = 3600;
/// Maximum global execution timeout (seconds).
pub const MAX_GLOBAL_TIMEOUT_SECONDS: u64 = 3600;
/// Default global timeout (seconds).
pub const DEFAULT_GLOBAL_TIMEOUT_SECONDS: u64 = 180;
/// Default per-query HTTP timeout (seconds).
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 15;
/// Default retry budget for residual HTTP paths.
pub const DEFAULT_RETRIES: u32 = 2;
/// Default SERP pages per query.
pub const DEFAULT_PAGES: u32 = 1;
/// Default deep-research synthesis token budget.
pub const DEFAULT_BUDGET_TOKENS: u32 = 4000;
/// Default cancel-grace seconds after first SIGINT.
pub const DEFAULT_CANCEL_GRACE_SECS: u64 = 5;
/// Maximum SERP pages per query.
pub const MAX_PAGES: u32 = 5;
/// Maximum retry attempts.
pub const MAX_RETRIES: u32 = 10;
/// Maximum parallelism degree.
pub const MAX_PARALLELISM: u32 = 20;
/// Default parallelism degree.
pub const DEFAULT_PARALLELISM: u32 = 5;
/// Maximum `--num` result count.
pub const MAX_RESULT_COUNT: u32 = 500;
/// Default `--num` when omitted.
pub const DEFAULT_RESULT_COUNT: u32 = 15;
/// Maximum extracted content length (characters).
pub const MAX_CONTENT_LENGTH: usize = 100_000;
/// Default max content length.
pub const DEFAULT_CONTENT_LENGTH: usize = 10_000;
/// Maximum per-host concurrent fetches.
pub const MAX_PER_HOST_LIMIT: u32 = 10;
/// Default per-host concurrent fetches.
pub const DEFAULT_PER_HOST_LIMIT: u32 = 2;
/// Default SERP language code for `-l` / `--lang` (DuckDuckGo `kl`).
///
/// Named SSOT for clap `default_value` and XDG `default_lang` apply
/// (GAP-E2E-51-013). OS-locale SERP inference is not implemented; keep this
/// product default stable.
pub const DEFAULT_SERP_LANG: &str = "pt";
/// Default SERP country / region code for `-c` / `--country` (DuckDuckGo `kl`).
///
/// Named SSOT for clap `default_value` and XDG `default_country` apply
/// (GAP-E2E-51-013).
pub const DEFAULT_SERP_COUNTRY: &str = "br";
/// Maximum length of SERP language / country codes.
pub const MAX_SERP_CODE_LEN: usize = 16;
/// Maximum length of a User-Agent string stored on [`Config`](crate::types::Config).
pub const MAX_USER_AGENT_LEN: usize = 512;

/// Conservative wall-clock estimate (seconds) for one Chrome SERP sub-query.
/// Used only for deep-research timeout×workload warnings (GAP-E2E-48 budget).
pub const BUDGET_SERP_SECONDS_ESTIMATE: u64 = 8;
/// Conservative wall-clock estimate (seconds) for one serial nested content fetch.
pub const BUDGET_FETCH_SECONDS_ESTIMATE: u64 = 5;
/// Grace period (seconds) after global timeout cancel to harvest partial DR results.
pub const DEEP_RESEARCH_TIMEOUT_GRACE_SECONDS: u64 = 5;

/// Lower-bound wall-clock estimate for a deep-research run (seconds).
///
/// `n_sub * (serp + fetch_cap * fetch_s * verts)` when fetch is on; SERP-only
/// when fetch is off. Used for stderr budget warnings — not a hard cap.
#[must_use]
pub fn estimate_deep_research_seconds(
    max_sub_queries: usize,
    fetch_content: bool,
    fetch_content_cap: usize,
    dual_vertical: bool,
) -> u64 {
    let n = max_sub_queries.max(1) as u64;
    let verts = if dual_vertical { 2_u64 } else { 1_u64 };
    let serp = BUDGET_SERP_SECONDS_ESTIMATE;
    let fetch = if fetch_content {
        (fetch_content_cap as u64).max(1) * BUDGET_FETCH_SECONDS_ESTIMATE * verts
    } else {
        0
    };
    n.saturating_mul(serp.saturating_add(fetch))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn invalid(message: impl Into<String>) -> CliError {
    CliError::InvalidConfig {
        message: message.into(),
    }
}

// ---------------------------------------------------------------------------
// TimeoutSeconds
// ---------------------------------------------------------------------------

/// Per-request HTTP timeout in seconds (`1..=MAX_TIMEOUT_SECONDS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct TimeoutSeconds(NonZeroU64);

impl TimeoutSeconds {
    /// Parse a raw second count into a validated timeout.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when `raw` is 0 or exceeds the max.
    pub fn try_new(raw: u64) -> Result<Self, CliError> {
        if raw == 0 {
            return Err(invalid(format!(
                "timeout_seconds must be >= 1 (got {raw})"
            )));
        }
        if raw > MAX_TIMEOUT_SECONDS {
            return Err(invalid(format!(
                "timeout_seconds exceeds maximum of {MAX_TIMEOUT_SECONDS} (got {raw})"
            )));
        }
        NonZeroU64::new(raw)
            .map(Self)
            .ok_or_else(|| invalid("timeout_seconds must be non-zero"))
    }

    /// Borrow the validated second count.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

// ---------------------------------------------------------------------------
// GlobalTimeoutSeconds
// ---------------------------------------------------------------------------

/// Whole-run global timeout in seconds (`1..=MAX_GLOBAL_TIMEOUT_SECONDS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct GlobalTimeoutSeconds(NonZeroU64);

impl GlobalTimeoutSeconds {
    /// Parse a raw second count into a validated global timeout.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when out of range.
    pub fn try_new(raw: u64) -> Result<Self, CliError> {
        if raw == 0 {
            return Err(invalid(format!(
                "global_timeout_seconds must be >= 1 (got {raw})"
            )));
        }
        if raw > MAX_GLOBAL_TIMEOUT_SECONDS {
            return Err(invalid(format!(
                "global_timeout_seconds exceeds maximum of {MAX_GLOBAL_TIMEOUT_SECONDS} (got {raw})"
            )));
        }
        NonZeroU64::new(raw)
            .map(Self)
            .ok_or_else(|| invalid("global_timeout_seconds must be non-zero"))
    }

    /// Borrow the validated second count.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

// ---------------------------------------------------------------------------
// PageCount
// ---------------------------------------------------------------------------

/// Number of SERP pages to fetch (`1..=MAX_PAGES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct PageCount(u32);

impl PageCount {
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when out of range.
    pub fn try_new(raw: u32) -> Result<Self, CliError> {
        if raw == 0 || raw > MAX_PAGES {
            return Err(invalid(format!(
                "pages must be in 1..={MAX_PAGES} (got {raw})"
            )));
        }
        Ok(Self(raw))
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// RetryBudget
// ---------------------------------------------------------------------------

/// Retry attempts (`0..=MAX_RETRIES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RetryBudget(u32);

impl RetryBudget {
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when `raw > MAX_RETRIES`.
    pub fn try_new(raw: u32) -> Result<Self, CliError> {
        if raw > MAX_RETRIES {
            return Err(invalid(format!(
                "retries must be in 0..={MAX_RETRIES} (got {raw})"
            )));
        }
        Ok(Self(raw))
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// ParallelismDegree
// ---------------------------------------------------------------------------

/// Fan-out parallelism (`1..=MAX_PARALLELISM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ParallelismDegree(u32);

impl ParallelismDegree {
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when out of range.
    pub fn try_new(raw: u32) -> Result<Self, CliError> {
        if raw == 0 || raw > MAX_PARALLELISM {
            return Err(invalid(format!(
                "parallelism must be in 1..={MAX_PARALLELISM} (got {raw})"
            )));
        }
        Ok(Self(raw))
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// ResultCount
// ---------------------------------------------------------------------------

/// Desired organic result count (`1..=MAX_RESULT_COUNT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ResultCount(u32);

impl ResultCount {
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when out of range.
    pub fn try_new(raw: u32) -> Result<Self, CliError> {
        if raw == 0 || raw > MAX_RESULT_COUNT {
            return Err(invalid(format!(
                "num_results must be in 1..={MAX_RESULT_COUNT} (got {raw})"
            )));
        }
        Ok(Self(raw))
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// ContentLengthLimit
// ---------------------------------------------------------------------------

/// Max extracted content length in characters (`1..=MAX_CONTENT_LENGTH`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ContentLengthLimit(usize);

impl ContentLengthLimit {
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when out of range.
    pub fn try_new(raw: usize) -> Result<Self, CliError> {
        if raw == 0 || raw > MAX_CONTENT_LENGTH {
            return Err(invalid(format!(
                "max_content_length must be in 1..={MAX_CONTENT_LENGTH} (got {raw})"
            )));
        }
        Ok(Self(raw))
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

// ---------------------------------------------------------------------------
// PerHostLimit
// ---------------------------------------------------------------------------

/// Per-host concurrent content fetches (`1..=MAX_PER_HOST_LIMIT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct PerHostLimit(u32);

impl PerHostLimit {
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when out of range.
    pub fn try_new(raw: u32) -> Result<Self, CliError> {
        if raw == 0 || raw > MAX_PER_HOST_LIMIT {
            return Err(invalid(format!(
                "per_host_limit must be in 1..={MAX_PER_HOST_LIMIT} (got {raw})"
            )));
        }
        Ok(Self(raw))
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the inner value as `usize`.
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

// ---------------------------------------------------------------------------
// SerpLanguage / SerpCountry
// ---------------------------------------------------------------------------

fn parse_serp_code(raw: &str, field: &str) -> Result<String, CliError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(invalid(format!("{field} is empty after trim")));
    }
    if trimmed.len() > MAX_SERP_CODE_LEN {
        return Err(invalid(format!(
            "{field} exceeds maximum length of {MAX_SERP_CODE_LEN} (got {})",
            trimmed.len()
        )));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(invalid(format!(
            "{field} contains invalid characters (allowed: A-Z a-z 0-9 _ -)"
        )));
    }
    Ok(trimmed.to_string())
}

/// DuckDuckGo SERP language code for the `kl` parameter (e.g. `pt`, `en`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SerpLanguage(String);

impl SerpLanguage {
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when empty, too long, or bad charset.
    pub fn try_new(raw: &str) -> Result<Self, CliError> {
        parse_serp_code(raw, "language").map(Self)
    }

    /// Borrow the validated language code.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SerpLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for SerpLanguage {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// DuckDuckGo SERP country / region code (e.g. `br`, `us`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SerpCountry(String);

impl SerpCountry {
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when empty, too long, or bad charset.
    pub fn try_new(raw: &str) -> Result<Self, CliError> {
        parse_serp_code(raw, "country").map(Self)
    }

    /// Borrow the validated country code.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SerpCountry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for SerpCountry {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

// ---------------------------------------------------------------------------
// UserAgentString
// ---------------------------------------------------------------------------

/// Non-empty User-Agent string selected for the session.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct UserAgentString(String);

impl UserAgentString {
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when empty or too long after trim.
    pub fn try_new(raw: &str) -> Result<Self, CliError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(invalid("user_agent is empty after trim"));
        }
        if trimmed.len() > MAX_USER_AGENT_LEN {
            return Err(invalid(format!(
                "user_agent exceeds maximum length of {MAX_USER_AGENT_LEN} (got {})",
                trimmed.len()
            )));
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Borrow the validated User-Agent string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume into the inner `String`.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for UserAgentString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for UserAgentString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_rejects_zero_and_over_max() {
        assert!(TimeoutSeconds::try_new(0).is_err());
        assert!(TimeoutSeconds::try_new(MAX_TIMEOUT_SECONDS + 1).is_err());
        assert_eq!(TimeoutSeconds::try_new(30).unwrap().get(), 30);
    }

    #[test]
    fn page_count_bounds() {
        assert!(PageCount::try_new(0).is_err());
        assert!(PageCount::try_new(6).is_err());
        assert_eq!(PageCount::try_new(3).unwrap().get(), 3);
    }

    #[test]
    fn retry_budget_allows_zero() {
        assert_eq!(RetryBudget::try_new(0).unwrap().get(), 0);
        assert!(RetryBudget::try_new(MAX_RETRIES + 1).is_err());
    }

    #[test]
    fn serp_language_charset() {
        assert!(SerpLanguage::try_new("").is_err());
        assert!(SerpLanguage::try_new("pt br").is_err());
        assert_eq!(SerpLanguage::try_new("pt-br").unwrap().as_str(), "pt-br");
        assert_eq!(SerpLanguage::try_new("  en  ").unwrap().as_str(), "en");
    }

    #[test]
    fn user_agent_non_empty() {
        assert!(UserAgentString::try_new("").is_err());
        assert!(UserAgentString::try_new("   ").is_err());
        let ua = UserAgentString::try_new("Mozilla/5.0 Test").unwrap();
        assert_eq!(ua.as_str(), "Mozilla/5.0 Test");
    }

    #[test]
    fn transparent_layout_timeout() {
        assert_eq!(
            std::mem::size_of::<TimeoutSeconds>(),
            std::mem::size_of::<NonZeroU64>()
        );
        assert_eq!(
            std::mem::size_of::<PageCount>(),
            std::mem::size_of::<u32>()
        );
    }
}
