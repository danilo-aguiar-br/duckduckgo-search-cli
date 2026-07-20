// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (UTC clock SSOT for domain timestamps)
//! UTC timestamps for the agent wire contract (RFC 3339).
//!
//! Production paths must use [`utc_now`] rather than ad-hoc `Utc::now().to_rfc3339()`
//! so the Rust type stays [`DateTime<Utc>`] end-to-end (serde emits RFC 3339 strings).

use chrono::{DateTime, Utc};

/// Current wall-clock time in UTC (domain SSOT).
///
/// Never use [`chrono::Local::now`] in this CLI — host/container TZ is not part of
/// the agent contract.
#[inline]
#[must_use]
pub fn utc_now() -> DateTime<Utc> {
    Utc::now()
}

/// Fixed RFC 3339 fixture for unit tests (avoids stringly `"t"` timestamps).
#[cfg(test)]
#[must_use]
pub fn test_timestamp() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-04-14T00:00:00Z")
        .expect("fixture RFC 3339")
        .with_timezone(&Utc)
}

/// Alternate fixture used by some output-format tests (`+00:00` offset form).
#[cfg(test)]
#[must_use]
pub fn test_timestamp_offset() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-04-14T00:00:00+00:00")
        .expect("fixture RFC 3339 offset")
        .with_timezone(&Utc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_now_is_utc() {
        let now = utc_now();
        assert_eq!(now.timezone(), Utc);
    }

    #[test]
    fn test_timestamp_roundtrips_rfc3339() {
        let ts = test_timestamp();
        let s = ts.to_rfc3339();
        let parsed = DateTime::parse_from_rfc3339(&s)
            .expect("roundtrip")
            .with_timezone(&Utc);
        assert_eq!(parsed, ts);
    }
}
