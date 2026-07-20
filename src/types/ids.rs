// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (domain run correlation IDs)
//! Strongly typed run identifiers (UUID v7).
//!
//! Each search invocation gets a [`RunId`] for agent-side correlation across
//! multi-query batches and failure envelopes. Generated with [`Uuid::now_v7`]
//! so lexicographic order approximates wall-clock order (B-tree friendly if
//! persisted later). **Never** use [`Uuid::nil`].

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Correlation identifier for a single search run (UUID v7).
///
/// Wire JSON is the canonical hyphenated string (`8-4-4-4-12`). Field is private;
/// construct only via [`RunId::generate`] or [`RunId::try_parse`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct RunId(Uuid);

impl RunId {
    /// Generate a new monotonic UUID v7.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Parse a hyphenated (or simple) UUID string.
    ///
    /// # Errors
    ///
    /// Returns [`uuid::Error`] when the input is not a valid UUID.
    pub fn try_parse(raw: &str) -> Result<Self, uuid::Error> {
        Uuid::parse_str(raw.trim()).map(Self)
    }

    /// Borrow the inner UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }

    /// UUID version number (7 for production generators).
    #[must_use]
    pub fn version_num(self) -> usize {
        self.0.get_version_num()
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Default Display for Uuid is hyphenated lowercase.
        write!(f, "{}", self.0)
    }
}

impl AsRef<Uuid> for RunId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_is_v7() {
        let id = RunId::generate();
        assert_eq!(id.version_num(), 7);
        assert_ne!(id.as_uuid(), Uuid::nil());
    }

    #[test]
    fn display_is_hyphenated() {
        let id = RunId::generate();
        let s = id.to_string();
        assert_eq!(s.len(), 36);
        assert_eq!(s.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn serde_roundtrip_as_string() {
        let id = RunId::generate();
        let json = serde_json::to_string(&id).expect("ser");
        // Transparent Uuid serializes as a JSON string.
        assert!(json.starts_with('"') && json.ends_with('"'), "json={json}");
        let back: RunId = serde_json::from_str(&json).expect("de");
        assert_eq!(back, id);
    }

    #[test]
    fn try_parse_accepts_hyphenated() {
        let raw = "550e8400-e29b-41d4-a716-446655440000";
        let id = RunId::try_parse(raw).expect("parse");
        assert_eq!(id.to_string(), raw);
    }

    #[test]
    fn v4_feature_available_for_pure_random() {
        // Rules require the `v4` feature; keep a compile-time smoke that it links.
        let _ = Uuid::new_v4();
    }
}
