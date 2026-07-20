// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (validated absolute HTTP(S) URLs)
//! Absolute `http`/`https` URL newtype for SERP result destinations.
//!
//! Parse at the extraction boundary; internal layers only see [`HttpUrl`].
//! Serializes as a JSON string (requires `url` crate feature `serde`).
//! **No** [`std::ops::Deref`] — preserves type safety (GraphRAG type-system rules).

use crate::error::CliError;
use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url;

/// Validated absolute HTTP or HTTPS URL with a host.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct HttpUrl(Url);

impl HttpUrl {
    /// Parse and validate an absolute `http`/`https` URL.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::InvalidConfig`] when the input is not an absolute
    /// HTTP(S) URL with a host (also rejects `javascript:`, `file:`, `data:`).
    pub fn try_new(raw: &str) -> Result<Self, CliError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(CliError::InvalidConfig {
                message: "URL is empty".into(),
            });
        }
        let parsed = Url::parse(trimmed).map_err(|e| CliError::InvalidConfig {
            message: format!("invalid URL: {e}"),
        })?;
        Self::from_url(parsed)
    }

    /// Validate an already-parsed [`Url`].
    ///
    /// # Errors
    ///
    /// Same as [`Self::try_new`].
    pub fn from_url(parsed: Url) -> Result<Self, CliError> {
        let scheme = parsed.scheme();
        if !matches!(scheme, "http" | "https") {
            return Err(CliError::InvalidConfig {
                message: format!(
                    "unsupported URL scheme '{scheme}' (allowed: http, https)"
                ),
            });
        }
        if parsed.host_str().is_none() {
            return Err(CliError::InvalidConfig {
                message: "URL missing host".into(),
            });
        }
        Ok(Self(parsed))
    }

    /// Infallible constructor for tests with known-valid URLs.
    #[cfg(test)]
    #[must_use]
    pub fn for_test(raw: &str) -> Self {
        Self::try_new(raw).expect("test HttpUrl must be valid")
    }

    /// Borrow the canonical string form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Borrow the inner [`Url`].
    #[must_use]
    pub fn as_url(&self) -> &Url {
        &self.0
    }

    /// Consume into the inner [`Url`].
    #[must_use]
    pub fn into_url(self) -> Url {
        self.0
    }
}

impl fmt::Display for HttpUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for HttpUrl {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PartialOrd for HttpUrl {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HttpUrl {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialEq<str> for HttpUrl {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for HttpUrl {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for HttpUrl {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https() {
        let u = HttpUrl::try_new("https://example.com/path?q=1").expect("ok");
        assert_eq!(u.as_url().scheme(), "https");
        assert_eq!(u.as_url().host_str(), Some("example.com"));
    }

    #[test]
    fn rejects_empty() {
        assert!(HttpUrl::try_new("").is_err());
        assert!(HttpUrl::try_new("   ").is_err());
    }

    #[test]
    fn rejects_file_and_javascript() {
        assert!(HttpUrl::try_new("file:///etc/passwd").is_err());
        assert!(HttpUrl::try_new("javascript:alert(1)").is_err());
        assert!(HttpUrl::try_new("data:text/plain,hi").is_err());
    }

    #[test]
    fn rejects_missing_host() {
        // Empty host / non-http schemes are rejected at the boundary.
        assert!(HttpUrl::try_new("https://").is_err());
        assert!(HttpUrl::try_new("/relative/path").is_err());
    }

    #[test]
    fn serde_roundtrip_as_string() {
        let u = HttpUrl::for_test("https://example.com/a");
        let json = serde_json::to_string(&u).expect("ser");
        assert_eq!(json, "\"https://example.com/a\"");
        let back: HttpUrl = serde_json::from_str(&json).expect("de");
        assert_eq!(back, u);
    }

    #[test]
    fn ord_lexicographic_on_str() {
        let a = HttpUrl::for_test("https://a.example/");
        let b = HttpUrl::for_test("https://b.example/");
        assert!(a < b);
    }
}
