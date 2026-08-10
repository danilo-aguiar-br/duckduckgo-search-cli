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

    /// An agent-native reduction this envelope cannot express (v1.0.5).
    ///
    /// # Why this is not just `InvalidConfig`
    ///
    /// A refusal is read by two audiences with opposite needs. The agent
    /// parsing stdout needs a STABLE English sentence and a machine code it
    /// can branch on; the operator reading stderr needs their own language.
    /// `InvalidConfig` carries one string and therefore forced one audience to
    /// lose — which is how the refusal text ended up hardcoded English in
    /// `envelope_ops`, inside a binary that advertises `--ui-lang`.
    ///
    /// Carrying both renderings lets the emit boundary send each stream the
    /// text it needs, from one construction site.
    #[error("{english}")]
    AgentOpsRefused {
        /// Machine-readable refusal code for the `error` field on stdout.
        code: &'static str,
        /// English sentence for the stdout contract; never translated.
        english: String,
        /// Localized sentence for the human on stderr.
        localized: String,
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
    #[cfg(feature = "http-test-harness")]
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

/// `Erro: <translated body>` — for variants the product wrote entirely.
fn body(lang: crate::i18n::Language, msg: crate::i18n::Message) -> String {
    crate::i18n::generic_error_in(lang, msg.text(lang))
}

/// `Erro: <translated label>: <caller prose, verbatim>`.
///
/// The caller half is never touched. It routinely quotes an operating system,
/// a TLS stack or a remote service, and none of those speak the operator's
/// language on request.
fn labelled(lang: crate::i18n::Language, msg: crate::i18n::Message, detail: &str) -> String {
    crate::i18n::generic_error_in(lang, format!("{}: {detail}", msg.text(lang)))
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
    #[cfg(feature = "http-test-harness")]
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
            | Self::UnsupportedEncoding(_)
            | Self::PipelineInvariantViolation { .. } => exit_codes::GENERIC_ERROR,
            #[cfg(feature = "http-test-harness")]
            Self::HttpClient { .. } => exit_codes::GENERIC_ERROR,
            Self::InvalidConfig { .. }
            | Self::AgentOpsRefused { .. }
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

    /// The human sentence for stderr, in the operator's language.
    ///
    /// Splits the two audiences a failure has. [`std::fmt::Display`] stays the
    /// STABLE English text an agent parses out of stdout; this is what a person
    /// reads, prefixed per category and translated where a translation exists.
    ///
    /// # Two kinds of variant, two treatments
    ///
    /// A variant whose `Display` is a FIXED template gets a fully translated
    /// body: the product wrote every word, so the product may say them in
    /// another language. A variant that embeds `{message}` from its caller
    /// gets a translated LABEL and keeps the caller's prose verbatim —
    /// translating that half would mean inventing text nobody wrote, and the
    /// caller is often quoting an operating system or a remote service.
    ///
    /// [`Self::PathError`] gets neither: its `Display` is the bare caller
    /// message by deliberate design (GAP-WS-ERR-CHROME-PATH-001), so adding a
    /// label here would re-introduce the mislabelling that removed it.
    ///
    /// # Why the `match` is exhaustive
    ///
    /// The previous version ended in `other => generic_error(other)`, which
    /// answered every unlisted variant with the English `Display`. It was not
    /// wrong for any variant in particular; it was silent for all of them, and
    /// a new variant would inherit that silence without a single line of
    /// evidence. With no catch-all, adding a variant fails to compile until
    /// somebody decides which of the two treatments it takes.
    #[must_use]
    pub fn localized_detail(&self) -> String {
        self.localized_detail_in(crate::i18n::language())
    }

    /// [`Self::localized_detail`] with the language passed in.
    ///
    /// The process locale is a `OnceLock`, so a test that only had the global
    /// accessor could never compare both renderings of the same variant in one
    /// process — and the pt-BR half is exactly the half nobody was checking.
    ///
    /// [`Self::AgentOpsRefused`] is the one variant this cannot re-render: it
    /// carries an already-formatted `localized` string chosen when it was
    /// constructed, because its placeholders come from the failing invocation
    /// and no longer exist here.
    #[must_use]
    pub fn localized_detail_in(&self, lang: crate::i18n::Language) -> String {
        use crate::i18n::Message as M;
        match self {
            // ---- Product-owned bodies: fully translated. -------------------
            Self::RateLimited => body(lang, M::ErrorRateLimited),
            Self::Blocked => body(lang, M::ErrorBlocked),
            Self::NoResults => body(lang, M::ErrorNoResults),
            Self::Cancelled => body(lang, M::ErrorCancelled),
            Self::BrokenPipe => body(lang, M::ErrorBrokenPipe),
            Self::ChromeDisabledByEnv => body(lang, M::ErrorChromeDisabled),
            Self::InvalidUtf8(_) => body(lang, M::ErrorInvalidUtf8),
            Self::PayloadTooLarge { max, actual } => crate::i18n::generic_error_in(
                lang,
                M::ErrorPayloadTooLarge.format(
                    lang,
                    &[("max", &max.to_string()), ("actual", &actual.to_string())],
                ),
            ),
            Self::UnsupportedEncoding(encoding) => crate::i18n::generic_error_in(
                lang,
                M::ErrorUnsupportedEncoding.format(lang, &[("encoding", encoding)]),
            ),
            Self::DecompressionIo { source } => crate::i18n::generic_error_in(
                lang,
                M::ErrorDecompressionIo.format(lang, &[("error", &source.to_string())]),
            ),
            #[cfg(feature = "http-test-harness")]
            Self::HttpClient { .. } => body(lang, M::ErrorHttpClient),
            // Already carries its own prefix in both languages.
            Self::GlobalTimeout { seconds } => {
                crate::i18n::global_timeout_exceeded_in(lang, *seconds)
            }

            // ---- Caller-owned prose: translated label, verbatim body. ------
            Self::HttpError { message, .. } => labelled(lang, M::ErrorLabelHttp, message),
            Self::ProxyError { message } => labelled(lang, M::ErrorLabelProxy, message),
            Self::NetworkError { message } => labelled(lang, M::ErrorLabelNetwork, message),
            Self::PipelineInvariantViolation { message } => {
                labelled(lang, M::ErrorLabelPipelineInvariant, message)
            }
            Self::ChromeNotFound { message } => {
                labelled(lang, M::ErrorLabelChromeNotFound, message)
            }
            Self::ChromeUnavailable { message } => {
                labelled(lang, M::ErrorLabelChromeUnavailable, message)
            }

            // ---- Caller owns the whole sentence, label included. -----------
            Self::PathError { message } => crate::i18n::generic_error_in(lang, message),
            Self::InvalidConfig { message } => crate::i18n::configuration_error_in(lang, message),
            Self::AgentOpsRefused { localized, .. } => {
                crate::i18n::configuration_error_in(lang, localized)
            }
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
            Self::AgentOpsRefused { code, .. } => code,
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
            | Self::DecompressionIo { .. } => codes::HTTP_ERROR,
            #[cfg(feature = "http-test-harness")]
            Self::HttpClient { .. } => codes::HTTP_ERROR,
        }
    }

    /// Whether an external re-invocation of the CLI may succeed after a wait.
    ///
    /// Agents should prefer this over matching on `Display` strings. Permanent
    /// config / validation / cancel / empty-result failures return `false`.
    /// Soft anti-bot blocks return `false` (need a long cool-down, not a tight loop).
    ///
    /// GAP-E2E-V11-EXIT-TAXONOMY / V12: [`Self::ChromeUnavailable`] is retryable
    /// when the message classifies as a transient session fault (launch stream,
    /// WS reset, CDP timeout). [`Self::ChromeNotFound`] stays permanent.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited | Self::NetworkError { .. } | Self::GlobalTimeout { .. } => true,
            Self::HttpError { .. } => true,
            #[cfg(feature = "http-test-harness")]
            Self::HttpClient { .. } => true,
            Self::ChromeUnavailable { message } => {
                super::chrome_classify::is_chrome_session_transient_message(message)
            }
            Self::Blocked => false,
            Self::NoResults
            | Self::InvalidConfig { .. }
            | Self::AgentOpsRefused { .. }
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
        assert_eq!(CliError::path_error("test").error_code(), codes::PATH_ERROR);
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
        // V12: transient session faults are agent-retryable; permanent chrome miss is not.
        assert!(
            CliError::chrome_unavailable("chrome launch failed: unexpected end of stream")
                .is_retryable()
        );
        assert!(CliError::chrome_unavailable(
            "chrome timeout: exceeded for \"https://example.test/\""
        )
        .is_retryable());
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

    /// How a variant's stderr line is built, per the split in [`CliError::localized_detail_in`].
    #[derive(Debug, PartialEq, Eq)]
    enum Treatment {
        /// The product owns every word, so pt-BR must differ from English.
        Translated,
        /// The caller owns the whole sentence; only the prefix is translated.
        CallerOwned,
    }

    /// Exhaustive on purpose: a new variant fails to compile HERE too.
    ///
    /// `localized_detail_in` already forces a localization decision at compile
    /// time. This second exhaustive match forces the same variant to be given a
    /// sample below, so the decision is also measured and not merely made.
    fn treatment(err: &CliError) -> Treatment {
        match err {
            CliError::RateLimited
            | CliError::Blocked
            | CliError::NoResults
            | CliError::Cancelled
            | CliError::BrokenPipe
            | CliError::ChromeDisabledByEnv
            | CliError::InvalidUtf8(_)
            | CliError::PayloadTooLarge { .. }
            | CliError::UnsupportedEncoding(_)
            | CliError::DecompressionIo { .. }
            | CliError::GlobalTimeout { .. }
            | CliError::HttpError { .. }
            | CliError::ProxyError { .. }
            | CliError::NetworkError { .. }
            | CliError::PipelineInvariantViolation { .. }
            | CliError::ChromeNotFound { .. }
            | CliError::ChromeUnavailable { .. } => Treatment::Translated,
            #[cfg(feature = "http-test-harness")]
            CliError::HttpClient { .. } => Treatment::Translated,
            // `PathError` carries the caller's whole sentence by design
            // (GAP-WS-ERR-CHROME-PATH-001); `InvalidConfig` and
            // `AgentOpsRefused` carry caller prose after a translated prefix.
            CliError::PathError { .. }
            | CliError::InvalidConfig { .. }
            | CliError::AgentOpsRefused { .. } => Treatment::CallerOwned,
        }
    }

    /// One instance of every variant, so the treatment above is exercised.
    fn every_variant() -> Vec<CliError> {
        let mut all = vec![
            CliError::RateLimited,
            CliError::Blocked,
            CliError::NoResults,
            CliError::Cancelled,
            CliError::BrokenPipe,
            CliError::ChromeDisabledByEnv,
            CliError::PayloadTooLarge {
                max: 10,
                actual: 20,
            },
            CliError::UnsupportedEncoding("zstd".into()),
            CliError::decompression_io(std::io::Error::other("corrupt")),
            CliError::GlobalTimeout { seconds: 30 },
            CliError::http_msg("timeout"),
            CliError::proxy_error("bad proxy"),
            CliError::network_msg("reset"),
            CliError::PipelineInvariantViolation {
                message: "state".into(),
            },
            CliError::chrome_not_found("no binary"),
            CliError::chrome_unavailable("cdp failed"),
            CliError::path_error("bad path"),
            CliError::invalid_config("bad flag"),
            CliError::AgentOpsRefused {
                code: "agent_ops_unsupported",
                english: "not supported".into(),
                localized: "não suportado".into(),
            },
        ];
        all.push(CliError::InvalidUtf8(
            String::from_utf8(vec![0xff]).expect_err("invalid utf-8"),
        ));
        #[cfg(feature = "http-test-harness")]
        {
            // No public constructor for a `reqwest::Error`; covered by `treatment`
            // being exhaustive, which is what keeps a new variant from slipping by.
        }
        all
    }

    #[test]
    fn every_error_body_the_product_owns_is_translated() {
        use crate::i18n::Language;
        for err in every_variant() {
            let en = err.localized_detail_in(Language::En);
            let pt = err.localized_detail_in(Language::PtBr);
            assert!(!en.is_empty() && !pt.is_empty(), "empty line for {err:?}");
            match treatment(&err) {
                Treatment::Translated => assert_ne!(
                    en, pt,
                    "{err:?} renders identically in both languages — its body is \
                     still English under --ui-lang pt-BR"
                ),
                Treatment::CallerOwned => assert_ne!(
                    en, pt,
                    "{err:?} must still translate its prefix even though the \
                     body belongs to the caller"
                ),
            }
        }
    }

    #[test]
    fn caller_prose_survives_translation_verbatim() {
        use crate::i18n::Language;
        // The caller's half is often a quote from the OS or a remote service.
        // Translating it would mean inventing text nobody wrote.
        let err = CliError::network_msg("connection reset by peer");
        let pt = err.localized_detail_in(Language::PtBr);
        assert!(pt.starts_with("Erro: erro de rede: "), "{pt}");
        assert!(pt.ends_with("connection reset by peer"), "{pt}");
    }

    #[test]
    fn fixed_bodies_leave_no_english_behind() {
        use crate::i18n::Language;
        let pt = CliError::RateLimited.localized_detail_in(Language::PtBr);
        assert_eq!(pt, "Erro: limitação de taxa detectada pelo DuckDuckGo");
        let pt = CliError::NoResults.localized_detail_in(Language::PtBr);
        assert_eq!(pt, "Erro: zero resultados em todas as consultas");
    }

    #[test]
    fn invalid_config_no_longer_doubles_its_own_label() {
        use crate::i18n::Language;
        // `configuration_error(&err)` formatted the whole error, so the line read
        // `Configuration error: invalid configuration: bad flag`.
        let line = CliError::invalid_config("bad flag").localized_detail_in(Language::En);
        assert_eq!(line, "Configuration error: bad flag");
    }

    #[test]
    fn decompression_io_constructor_sets_source() {
        let err = CliError::decompression_io(std::io::Error::other("corrupt"));
        assert_eq!(err.error_code(), codes::HTTP_ERROR);
        assert!(StdError::source(&err).is_some());
    }
}
