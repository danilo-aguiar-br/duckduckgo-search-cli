// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (typed domain error enum via thiserror)
//! Typed [`CliError`] enum — public error contract of the library/CLI.

use super::{codes, exit_codes};

/// Typed error enum for the CLI domain.
///
/// Each variant maps to a specific exit code and JSON error code.
/// Display messages follow the Rust ecosystem convention: concise, **lowercase**,
/// no trailing period (rules-rust tratamento de erros / thiserror style).
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum CliError {
    /// HTTP-level failure with optional source chain.
    #[error("http error: {message}")]
    HttpError {
        /// Human-readable description of the HTTP failure (without duplicating `cause`).
        message: String,
        /// Underlying cause, when available.
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Persistent rate limiting after exhausting retries (HTTP 429).
    #[error("rate limiting detected by duckduckgo")]
    RateLimited,

    /// Anti-bot blocking detected (HTTP 202 anomaly or persistent 403).
    #[error("anti-bot blocking detected (http 202 anomaly)")]
    Blocked,

    /// Zero organic results across all queries.
    #[error("zero results across all queries")]
    NoResults,

    /// Invalid CLI configuration (incompatible arguments, bad values).
    #[error("invalid configuration: {message}")]
    InvalidConfig {
        /// Description of the configuration problem.
        message: String,
    },

    /// Global timeout exceeded.
    #[error("global timeout exceeded ({seconds}s)")]
    GlobalTimeout {
        /// Configured timeout in seconds.
        seconds: u64,
    },

    /// Cooperative cancel via SIGINT (Ctrl-C) or SIGTERM (timeout/supervisor).
    ///
    /// [`CliError::exit_code`] returns **130** (SIGINT convention). Callers that
    /// must distinguish SIGTERM should use [`crate::signals::exit_code_for_error`]
    /// (returns **143** when the signal handler recorded SIGTERM).
    #[error("operation cancelled via sigint/sigterm")]
    Cancelled,

    /// Proxy configuration or connection failure.
    #[error("proxy error: {message}")]
    ProxyError {
        /// Description of the proxy problem.
        message: String,
    },

    /// Low-level network error (DNS, TLS, connection reset).
    #[error("network error: {message}")]
    NetworkError {
        /// Description of the network failure.
        message: String,
    },

    /// Consumer closed the pipe (SIGPIPE / broken pipe).
    #[error("pipe closed by consumer (broken pipe)")]
    BrokenPipe,

    /// Pipeline invariant violation — internal state reached an impossible branch.
    ///
    /// Used to replace panics in production code paths where the compiler cannot
    /// prove that all enum variants are exhausted. Propagated as a structured
    /// error so cleanup paths still run.
    #[error("pipeline invariant violation: {message}")]
    PipelineInvariantViolation {
        /// Description of the invariant that was violated.
        message: String,
    },

    /// Path-related failure (output path traversal **or** invalid chrome path, etc.).
    ///
    /// The full message is caller-supplied — do not prefix with "invalid output path"
    /// (GAP-WS-ERR-CHROME-PATH-001): chrome detection errors were mislabeled.
    #[error("{message}")]
    PathError {
        /// Description of why the path was rejected.
        message: String,
    },

    /// Chrome/Chromium binary not found on the host (or `--chrome-path` missing).
    #[error("chrome not found: {message}")]
    ChromeNotFound {
        /// Actionable remediation text for the operator/agent.
        message: String,
    },

    /// Chrome transport failed after detection (launch/CDP/session).
    #[error("chrome unavailable: {message}")]
    ChromeUnavailable {
        /// Actionable remediation text for the operator/agent.
        message: String,
    },

    /// Legacy variant: product env kill-switch removed (GAP-SCRAPE-R2-013).
    /// Kept for stable `error_code` wire strings; not produced in production.
    #[error(
        "chrome transport unavailable (rebuild with --features chrome; product env kill-switch removed)"
    )]
    ChromeDisabledByEnv,

    /// Wire or decompressed payload exceeded a configured safety cap.
    #[error("payload exceeds {max} bytes (got {actual})")]
    PayloadTooLarge {
        /// Configured cap in bytes that was exceeded.
        max: usize,
        /// Number of bytes observed before aborting.
        actual: usize,
    },

    /// HTTP `Content-Encoding` header is not supported by the decompressor.
    #[error("unsupported content-encoding: {0}")]
    UnsupportedEncoding(String),

    /// Response body is not valid UTF-8 after decompression.
    #[error("response body is not valid utf-8")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),

    /// Underlying HTTP client error during response decoding (http-test-harness / residual HTTP).
    ///
    /// Construct via [`CliError::http_client`] — **no** blanket `From<reqwest::Error>`
    /// so unrelated `?` sites cannot silently reclassify as this variant.
    #[error("http client error")]
    HttpClient {
        /// Source reqwest error (kept for diagnostics; not serialized to agent JSON).
        #[source]
        source: reqwest::Error,
    },

    /// Underlying I/O error during gzip/deflate decompression (or buffer reserve).
    ///
    /// Construct via [`CliError::decompression_io`] — **no** blanket `From<std::io::Error>`.
    #[error("decompression i/o error")]
    DecompressionIo {
        /// Source I/O error from the decompressor or buffer path.
        #[source]
        source: std::io::Error,
    },
}

impl CliError {
    /// Builds [`CliError::HttpError`] with a short message and **no** source chain.
    #[must_use]
    pub fn http_msg(message: impl Into<String>) -> Self {
        Self::HttpError {
            message: message.into(),
            cause: None,
        }
    }

    /// Builds [`CliError::HttpError`] with a short message and a typed source.
    ///
    /// Do **not** embed `{source}` text in `message` — leave the chain to `Error::source`.
    #[must_use]
    pub fn http_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::HttpError {
            message: message.into(),
            cause: Some(Box::new(source)),
        }
    }

    /// Builds [`CliError::NetworkError`] with a short message.
    #[must_use]
    pub fn network_msg(message: impl Into<String>) -> Self {
        Self::NetworkError {
            message: message.into(),
        }
    }

    /// Builds [`CliError::InvalidConfig`].
    #[must_use]
    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::InvalidConfig {
            message: message.into(),
        }
    }

    /// Builds [`CliError::ProxyError`].
    #[must_use]
    pub fn proxy_error(message: impl Into<String>) -> Self {
        Self::ProxyError {
            message: message.into(),
        }
    }

    /// Builds [`CliError::PathError`].
    #[must_use]
    pub fn path_error(message: impl Into<String>) -> Self {
        Self::PathError {
            message: message.into(),
        }
    }

    /// Builds [`CliError::ChromeNotFound`].
    #[must_use]
    pub fn chrome_not_found(message: impl Into<String>) -> Self {
        Self::ChromeNotFound {
            message: message.into(),
        }
    }

    /// Builds [`CliError::ChromeUnavailable`].
    #[must_use]
    pub fn chrome_unavailable(message: impl Into<String>) -> Self {
        Self::ChromeUnavailable {
            message: message.into(),
        }
    }

    /// Wraps a `reqwest::Error` as [`CliError::HttpClient`] (explicit, no blanket `From`).
    #[must_use]
    pub fn http_client(source: reqwest::Error) -> Self {
        Self::HttpClient { source }
    }

    /// Wraps an I/O error as [`CliError::DecompressionIo`] (explicit, no blanket `From`).
    #[must_use]
    pub fn decompression_io(source: std::io::Error) -> Self {
        Self::DecompressionIo { source }
    }

    /// Returns the exit code corresponding to this error variant.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::HttpError { .. }
            | Self::NetworkError { .. }
            | Self::PayloadTooLarge { .. }
            | Self::InvalidUtf8(_)
            | Self::DecompressionIo { .. }
            | Self::HttpClient { .. }
            | Self::UnsupportedEncoding(_)
            | Self::PipelineInvariantViolation { .. } => exit_codes::GENERIC_ERROR,
            Self::InvalidConfig { .. }
            | Self::ProxyError { .. }
            | Self::PathError { .. }
            | Self::ChromeNotFound { .. }
            | Self::ChromeUnavailable { .. }
            | Self::ChromeDisabledByEnv => exit_codes::INVALID_CONFIG,
            Self::RateLimited | Self::Blocked => exit_codes::RATE_LIMITED_OR_BLOCKED,
            Self::GlobalTimeout { .. } => exit_codes::GLOBAL_TIMEOUT,
            Self::NoResults => exit_codes::ZERO_RESULTS,
            Self::Cancelled => exit_codes::CANCELLED,
            Self::BrokenPipe => exit_codes::BROKEN_PIPE,
        }
    }

    /// Returns the string error code for use in the `error` field of the JSON output.
    #[must_use]
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::HttpError { .. } => codes::HTTP_ERROR,
            Self::RateLimited => codes::RATE_LIMITED,
            Self::Blocked => codes::BLOCKED,
            Self::NoResults => codes::NO_RESULTS_FOUND,
            Self::InvalidConfig { .. } => codes::INVALID_CONFIG,
            Self::GlobalTimeout { .. } => codes::TIMEOUT,
            Self::Cancelled => codes::CANCELLED,
            Self::ProxyError { .. } => codes::PROXY_ERROR,
            Self::NetworkError { .. } => codes::NETWORK_ERROR,
            Self::BrokenPipe => codes::BROKEN_PIPE,
            Self::PathError { .. } => codes::PATH_ERROR,
            Self::PipelineInvariantViolation { .. } => codes::PIPELINE_INVARIANT_VIOLATION,
            Self::ChromeNotFound { .. } => codes::CHROME_NOT_FOUND,
            Self::ChromeUnavailable { .. } => codes::CHROME_UNAVAILABLE,
            Self::ChromeDisabledByEnv => codes::CHROME_DISABLED_BY_ENV,
            // Decompression-layer errors share http_error because they originate
            // from the HTTP response pipeline; consumers can match the variant.
            Self::PayloadTooLarge { .. }
            | Self::UnsupportedEncoding(_)
            | Self::InvalidUtf8(_)
            | Self::DecompressionIo { .. }
            | Self::HttpClient { .. } => codes::HTTP_ERROR,
        }
    }

    /// Whether an external re-invocation of the CLI may succeed after a wait.
    ///
    /// Agents should prefer this over matching on `Display` strings. Permanent
    /// config / validation / cancel / empty-result failures return `false`.
    /// Soft anti-bot blocks return `false` (need a long cool-down, not a tight loop).
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited | Self::NetworkError { .. } | Self::GlobalTimeout { .. } => true,
            Self::HttpError { .. } | Self::HttpClient { .. } => true,
            Self::Blocked => false,
            Self::NoResults
            | Self::InvalidConfig { .. }
            | Self::ProxyError { .. }
            | Self::PathError { .. }
            | Self::Cancelled
            | Self::BrokenPipe
            | Self::PipelineInvariantViolation { .. }
            | Self::PayloadTooLarge { .. }
            | Self::UnsupportedEncoding(_)
            | Self::InvalidUtf8(_)
            | Self::DecompressionIo { .. }
            | Self::ChromeNotFound { .. }
            | Self::ChromeUnavailable { .. }
            | Self::ChromeDisabledByEnv => false,
        }
    }

    /// Explicit complement of [`Self::is_retryable`] for permanent failures.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        !self.is_retryable() && !matches!(self, Self::Cancelled | Self::BrokenPipe)
    }

    /// True when this error is a Chrome-transport policy / availability failure.
    #[must_use]
    pub fn is_chrome_transport_error(&self) -> bool {
        matches!(
            self,
            Self::ChromeNotFound { .. }
                | Self::ChromeUnavailable { .. }
                | Self::ChromeDisabledByEnv
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as StdError;

    #[test]
    fn error_codes_are_non_empty_strings() {
        assert!(!codes::HTTP_ERROR.is_empty());
        assert!(!codes::BLOCKED.is_empty());
        assert!(!codes::NO_RESULTS_FOUND.is_empty());
        assert!(!codes::CHROME_NOT_FOUND.is_empty());
        assert!(!codes::CHROME_UNAVAILABLE.is_empty());
        assert!(!codes::CHROME_DISABLED_BY_ENV.is_empty());
    }

    #[test]
    fn exit_codes_have_correct_values() {
        assert_eq!(exit_codes::SUCCESS, 0);
        assert_eq!(exit_codes::GENERIC_ERROR, 1);
        assert_eq!(exit_codes::INVALID_CONFIG, 2);
        assert_eq!(exit_codes::RATE_LIMITED_OR_BLOCKED, 3);
        assert_eq!(exit_codes::GLOBAL_TIMEOUT, 4);
        assert_eq!(exit_codes::ZERO_RESULTS, 5);
        assert_eq!(exit_codes::SUSPECTED_BLOCK, 6);
        assert_eq!(exit_codes::CANCELLED, 130);
        assert_eq!(exit_codes::CANCELLED_SIGTERM, 143);
        assert_eq!(exit_codes::BROKEN_PIPE, 141);
    }

    #[test]
    fn cli_error_exit_codes_are_correct() {
        assert_eq!(
            CliError::RateLimited.exit_code(),
            exit_codes::RATE_LIMITED_OR_BLOCKED
        );
        assert_eq!(
            CliError::Blocked.exit_code(),
            exit_codes::RATE_LIMITED_OR_BLOCKED
        );
        assert_eq!(CliError::NoResults.exit_code(), exit_codes::ZERO_RESULTS);
        assert_eq!(
            CliError::GlobalTimeout { seconds: 60 }.exit_code(),
            exit_codes::GLOBAL_TIMEOUT
        );
        assert_eq!(
            CliError::invalid_config("test").exit_code(),
            exit_codes::INVALID_CONFIG
        );
        assert_eq!(CliError::BrokenPipe.exit_code(), exit_codes::BROKEN_PIPE);
        assert_eq!(CliError::BrokenPipe.exit_code(), 141);
        assert_eq!(CliError::Cancelled.exit_code(), exit_codes::CANCELLED);
        assert_eq!(CliError::Cancelled.exit_code(), 130);
        assert_eq!(
            CliError::chrome_not_found("missing").exit_code(),
            exit_codes::INVALID_CONFIG
        );
        assert_eq!(
            CliError::ChromeDisabledByEnv.exit_code(),
            exit_codes::INVALID_CONFIG
        );
    }

    #[test]
    fn cli_error_display_is_lowercase_and_non_empty() {
        let variants: Vec<CliError> = vec![
            CliError::http_msg("timeout"),
            CliError::RateLimited,
            CliError::Blocked,
            CliError::NoResults,
            CliError::invalid_config("bad flag"),
            CliError::GlobalTimeout { seconds: 30 },
            CliError::Cancelled,
            CliError::proxy_error("bad proxy"),
            CliError::network_msg("reset"),
            CliError::BrokenPipe,
            CliError::PipelineInvariantViolation {
                message: "state".into(),
            },
            CliError::path_error("bad path"),
            CliError::chrome_not_found("no binary"),
            CliError::chrome_unavailable("cdp failed"),
            CliError::ChromeDisabledByEnv,
            CliError::PayloadTooLarge {
                max: 10,
                actual: 20,
            },
            CliError::UnsupportedEncoding("zstd".into()),
        ];
        for err in variants {
            let text = format!("{err}");
            assert!(!text.is_empty(), "empty display for {err:?}");
            let first = text.chars().next().expect("non-empty");
            assert!(
                first.is_lowercase() || first.is_ascii_digit(),
                "display must start lowercase: {text:?}"
            );
            assert!(
                !text.ends_with('.'),
                "display must not end with period: {text:?}"
            );
        }
    }

    #[test]
    fn cli_error_codes_are_correct_strings() {
        assert_eq!(CliError::RateLimited.error_code(), "rate_limited");
        assert_eq!(CliError::Blocked.error_code(), "blocked");
        assert_eq!(CliError::NoResults.error_code(), "no_results_found");
        assert_eq!(CliError::Cancelled.error_code(), "cancelled");
        assert_eq!(CliError::BrokenPipe.error_code(), "broken_pipe");
        assert_eq!(
            CliError::path_error("test").error_code(),
            codes::PATH_ERROR
        );
        assert_eq!(
            CliError::invalid_config("test").error_code(),
            codes::INVALID_CONFIG
        );
        assert_eq!(
            CliError::chrome_not_found("x").error_code(),
            codes::CHROME_NOT_FOUND
        );
        assert_eq!(
            CliError::chrome_unavailable("x").error_code(),
            codes::CHROME_UNAVAILABLE
        );
        assert_eq!(
            CliError::ChromeDisabledByEnv.error_code(),
            codes::CHROME_DISABLED_BY_ENV
        );
    }

    #[test]
    fn cli_error_is_retryable_classification() {
        assert!(CliError::RateLimited.is_retryable());
        assert!(CliError::network_msg("reset").is_retryable());
        assert!(CliError::GlobalTimeout { seconds: 30 }.is_retryable());
        assert!(!CliError::Blocked.is_retryable());
        assert!(!CliError::NoResults.is_retryable());
        assert!(!CliError::invalid_config("x").is_retryable());
        assert!(!CliError::Cancelled.is_retryable());
        assert!(!CliError::ChromeDisabledByEnv.is_retryable());
        assert!(!CliError::chrome_not_found("x").is_retryable());
        assert!(CliError::invalid_config("x").is_permanent());
        assert!(!CliError::Cancelled.is_permanent());
        assert!(CliError::ChromeDisabledByEnv.is_chrome_transport_error());
    }

    #[test]
    fn http_with_source_preserves_chain_without_duplicating_in_display() {
        let io = std::io::Error::other("root cause detail");
        let err = CliError::http_with_source("request failed", io);
        let display = format!("{err}");
        assert!(display.contains("request failed"));
        assert!(
            !display.contains("root cause detail"),
            "display must not embed source text: {display}"
        );
        let source = StdError::source(&err).expect("source present");
        assert!(source.to_string().contains("root cause detail"));
    }

    #[test]
    fn decompression_io_constructor_sets_source() {
        let err = CliError::decompression_io(std::io::Error::other("corrupt"));
        assert_eq!(err.error_code(), codes::HTTP_ERROR);
        assert!(StdError::source(&err).is_some());
    }
}
