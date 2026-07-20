// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (field length / cardinality caps for external DTOs)
//! Memory-safe length and cardinality caps for inbound serde DTOs.
//!
//! Applied via `#[validate(length(...))]` / custom checks after deserialize.
//! File-level size gates remain in loaders (1 MiB TOML, [`crate::security::MAX_COOKIE_JAR_BYTES`]).

/// Maximum Unicode scalars in a cookie name.
pub const MAX_COOKIE_NAME_CHARS: u64 = 256;
/// Maximum Unicode scalars in a cookie value.
pub const MAX_COOKIE_VALUE_CHARS: u64 = 4096;
/// Maximum Unicode scalars in a cookie domain (DNS label bound).
pub const MAX_COOKIE_DOMAIN_CHARS: u64 = 253;

/// Maximum Unicode scalars in a CSS selector string from `selectors.toml`.
pub const MAX_CSS_SELECTOR_CHARS: u64 = 2048;
/// Maximum items in ad-filter list fields.
pub const MAX_SELECTOR_LIST_ITEMS: u64 = 256;

/// Maximum User-Agent string length from `user-agents.toml`.
pub const MAX_UA_CHARS: u64 = 512;
/// Maximum platform tag length (`linux`, `any`, …).
pub const MAX_UA_PLATFORM_CHARS: u64 = 32;

/// Minimum Unicode scalars per line kept by readability / Chrome `clean_text`.
///
/// Shared by HTTP readability and Chrome content extraction (GAP-SCRAPE-010).
pub const MIN_LINE_LENGTH: usize = 20;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_are_positive_and_ordered() {
        assert!(MAX_COOKIE_NAME_CHARS >= 1);
        assert!(MAX_COOKIE_VALUE_CHARS >= MAX_COOKIE_NAME_CHARS);
        assert!(MAX_CSS_SELECTOR_CHARS >= 64);
        assert!(MAX_UA_CHARS >= 32);
        assert!(MAX_SELECTOR_LIST_ITEMS >= 1);
        assert!(MIN_LINE_LENGTH >= 1);
    }
}
