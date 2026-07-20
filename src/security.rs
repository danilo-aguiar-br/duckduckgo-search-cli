// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (boundary validation; no I/O fan-out)
//! Defensive security helpers and the **minimal threat model** for this CLI.
//!
//! # Threat model (review on architectural change)
//!
//! | Asset / surface | Adversary control | Mitigations in this crate |
//! |-----------------|-------------------|---------------------------|
//! | CLI args / env | Fully hostile | clap ranges, typed enums, [`ValidatedQuery`], kill switches |
//! | `--queries-file` / stdin | Fully hostile | size caps, line caps, control-char reject, NFC |
//! | Network HTML / redirects | Attacker-controlled | SSRF DNS gate (`content`), body caps (`decompress`), Chrome-only SERP |
//! | Cookie jar file | Malicious / oversized | size cap, `0o600`, atomic write, fail-open empty jar |
//! | Output path (`-o`, cookies) | Path traversal | [`crate::paths::validate_output_path`] |
//! | Chrome binary path | Wrong/malicious path | native-binary gate, reject `.bat`/`.cmd`/`.ps1`/`.sh` |
//! | Proxy URL | Credential leak | scheme allow-list, [`crate::http`] credential mask in logs |
//! | Terminal / Markdown output | Hostile SERP titles | control strip + ANSI strip + markdown escape |
//! | Dependencies | Supply chain | locked Cargo.lock; no prod secrets in binary |
//!
//! ## STRIDE (critical components)
//!
//! | Component | S | T | R | I | D | E | Control summary |
//! |-----------|---|---|---|---|---|---|-----------------|
//! | Query boundary | — | spoof query | — | inject ctrl/bidi | oversized list | — | [`ValidatedQuery`], caps |
//! | Path I/O | — | — | — | traversal | fill disk | — | validate + atomic write |
//! | HTTP content fetch | — | SSRF | — | header inject | body bomb | — | DNS gate, body caps, redirects |
//! | Cookie jar | session steal | — | — | jar rewrite | — | — | `0o600`, owner path only |
//! | Chrome spawn | — | — | — | path/args | process storm | — | binary gate, pool N, kill/reap |
//! | Proxy URL | cred leak | — | — | — | — | — | mask in logs/errors |
//!
//! **Accepted threats** (local one-shot CLI): multi-tenant authZ, server TLS
//! termination headers, container seccomp policies, SLSA provenance (NO CI).
//! Operator can pass pathological flags (self-DoS) by design.
//!
//! Rules sources: security development, defensive security, one-shot, memory
//! RAII, parallelism bounds.

use std::collections::HashSet;

use unicode_normalization::UnicodeNormalization;

use crate::error::CliError;

/// Maximum Unicode scalar values per single query string (post-NFC).
pub const MAX_QUERY_CHARS: usize = 2_048;

/// Maximum number of queries accepted after dedup (positional + file + stdin).
pub const MAX_QUERIES: usize = 500;

/// Hard cap on `--queries-file` size before read (DoS / memory).
pub const MAX_QUERIES_FILE_BYTES: u64 = 1_048_576;

/// Hard cap on cookie jar JSON on disk (session credentials, not unbounded).
pub const MAX_COOKIE_JAR_BYTES: u64 = 262_144;

/// Validated, NFC-normalized search query (parse-don't-validate newtype).
///
/// Construction is only possible through [`ValidatedQuery::try_new`] /
/// [`TryFrom`]. Invariants:
/// - non-empty after trim
/// - ≤ [`MAX_QUERY_CHARS`] Unicode scalars
/// - no C0/C1 controls (except horizontal tab), no bidi overrides / ZW
/// - stored form is Unicode NFC
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ValidatedQuery(String);

impl ValidatedQuery {
    /// Validate, NFC-normalize, and wrap a raw query string.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when empty, too long, or containing
    /// disallowed control / bidi characters.
    pub fn try_new(raw: &str) -> Result<Self, CliError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(CliError::InvalidConfig {
                message: "query is empty after trim".into(),
            });
        }
        // NFC before length / charset checks so equivalent forms share one
        // representation for dedup and comparison (rules-rust security).
        let normalized: String = trimmed.nfc().collect();
        let char_count = normalized.chars().count();
        if char_count > MAX_QUERY_CHARS {
            return Err(CliError::InvalidConfig {
                message: format!(
                    "query exceeds maximum length of {MAX_QUERY_CHARS} characters (got {char_count})"
                ),
            });
        }
        if contains_disallowed_chars(&normalized) {
            return Err(CliError::InvalidConfig {
                message:
                    "query contains disallowed control, zero-width, or bidi characters".into(),
            });
        }
        Ok(Self(normalized))
    }

    /// Borrow the validated NFC string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume into the inner `String` (still NFC + validated).
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for ValidatedQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ValidatedQuery {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<&str> for ValidatedQuery {
    type Error = CliError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<String> for ValidatedQuery {
    type Error = CliError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(&value)
    }
}

/// Returns `true` when `s` contains characters rejected at the trust boundary.
///
/// Rejects:
/// - C0/C1 control characters (except horizontal tab)
/// - Unicode bidi overrides / isolates (U+202A–U+202E, U+2066–U+2069)
/// - Common zero-width / bidi marks used for spoofing (U+200B–U+200F, U+FEFF)
#[must_use]
pub fn contains_disallowed_chars(s: &str) -> bool {
    s.chars().any(|c| {
        if c == '\t' {
            return false;
        }
        if c.is_control() {
            return true;
        }
        matches!(
            c,
            '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
    })
}

/// Validates one query string at the trust boundary (parse, don't trust).
///
/// Prefer [`ValidatedQuery::try_new`] when the caller needs the typed value.
///
/// # Errors
///
/// Returns [`CliError::InvalidConfig`] when empty, too long, or containing
/// disallowed control / bidi characters.
pub fn validate_query(query: &str) -> Result<(), CliError> {
    ValidatedQuery::try_new(query).map(|_| ())
}

/// Validates the full post-combine query list, NFC-normalizes, and re-dedups.
///
/// Returns cleaned [`ValidatedQuery`] values ready for [`crate::types::Config`] (GAP-SECDEV-009).
/// Order of first occurrence is preserved.
///
/// # Errors
///
/// Returns [`CliError::InvalidConfig`] when the list is empty after validation,
/// exceeds [`MAX_QUERIES`], or any entry fails [`ValidatedQuery::try_new`].
pub fn validate_query_list(queries: &[String]) -> Result<Vec<ValidatedQuery>, CliError> {
    if queries.is_empty() {
        return Err(CliError::InvalidConfig {
            message:
                "no query provided (positional arguments, --queries-file, and stdin are all empty)"
                    .into(),
        });
    }

    let mut out: Vec<ValidatedQuery> = Vec::with_capacity(queries.len().min(MAX_QUERIES));
    let mut seen: HashSet<String> = HashSet::with_capacity(queries.len().min(MAX_QUERIES));

    for (i, q) in queries.iter().enumerate() {
        let validated = ValidatedQuery::try_new(q).map_err(|e| match e {
            CliError::InvalidConfig { message } => CliError::InvalidConfig {
                message: format!("query #{}: {message}", i + 1),
            },
            other => other,
        })?;
        let key = validated.as_str().to_string();
        if seen.insert(key) {
            out.push(validated);
        }
    }

    if out.is_empty() {
        return Err(CliError::InvalidConfig {
            message:
                "no query provided (positional arguments, --queries-file, and stdin are all empty)"
                    .into(),
        });
    }
    if out.len() > MAX_QUERIES {
        return Err(CliError::InvalidConfig {
            message: format!(
                "too many queries: {} (maximum {MAX_QUERIES} after deduplication)",
                out.len()
            ),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn validated_query_is_transparent_string() {
        assert_eq!(
            std::mem::size_of::<super::ValidatedQuery>(),
            std::mem::size_of::<String>()
        );
    }

    use super::*;

    #[test]
    fn accepts_normal_query() {
        assert!(validate_query("rust async tokio").is_ok());
        assert!(validate_query("pesquisa com acentos çãõ").is_ok());
    }

    #[test]
    fn rejects_empty_and_whitespace() {
        assert!(validate_query("").is_err());
        assert!(validate_query("   ").is_err());
    }

    #[test]
    fn rejects_overlong_query() {
        let long: String = "a".repeat(MAX_QUERY_CHARS + 1);
        let err = validate_query(&long).unwrap_err();
        assert!(err.to_string().contains("maximum length"));
    }

    #[test]
    fn rejects_nul_and_control() {
        assert!(validate_query("a\0b").is_err());
        assert!(validate_query("a\nb").is_err());
        assert!(validate_query("a\x1b[31mb").is_err());
    }

    #[test]
    fn allows_tab_inside_query() {
        assert!(validate_query("rust\ttokio").is_ok());
    }

    #[test]
    fn rejects_bidi_override() {
        assert!(validate_query("safe\u{202E}evil").is_err());
        assert!(validate_query("x\u{200B}y").is_err());
    }

    #[test]
    fn query_list_caps_count() {
        let many: Vec<String> = (0..=MAX_QUERIES)
            .map(|i| format!("q{i}"))
            .collect();
        let err = validate_query_list(&many).unwrap_err();
        assert!(err.to_string().contains("too many queries"));
    }

    #[test]
    fn query_list_indexes_bad_entry() {
        let list = vec!["ok".into(), "bad\0".into()];
        let err = validate_query_list(&list).unwrap_err();
        assert!(err.to_string().contains("query #2"));
    }

    #[test]
    fn validated_query_try_from_and_display() {
        let v = ValidatedQuery::try_from("  hello  ").expect("ok");
        assert_eq!(v.as_str(), "hello");
        assert_eq!(v.to_string(), "hello");
    }

    #[test]
    fn nfc_normalizes_composed_forms() {
        // "é" as e + combining acute vs precomposed
        let decomposed = "cafe\u{0301}"; // café
        let composed = "café";
        let a = ValidatedQuery::try_new(decomposed).expect("decomp");
        let b = ValidatedQuery::try_new(composed).expect("comp");
        assert_eq!(a.as_str(), b.as_str());
        // List re-dedups after NFC.
        let list = validate_query_list(&vec![decomposed.into(), composed.into()]).expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].as_str(), a.as_str());
    }

    #[test]
    fn validate_query_list_returns_cleaned_strings() {
        let list = validate_query_list(&vec!["  rust  ".into(), "rust".into(), "tokio".into()])
            .expect("ok");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].as_str(), "rust");
        assert_eq!(list[1].as_str(), "tokio");
    }
}
