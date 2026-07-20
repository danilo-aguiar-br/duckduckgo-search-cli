// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (charset decode + optional meta sniff via scraper)
//! HTML body charset detection and UTF-8 conversion (GAP-SCRAPE-002…007).
//!
//! Chain (WHATWG-inspired, docsrs `encoding_rs`):
//! 1. BOM (`Encoding::for_bom`)
//! 2. `Content-Type` charset parameter (ASCII-case-insensitive)
//! 3. `<meta charset>` / `http-equiv=content-type` via **scraper** (no HTML regex)
//! 4. Fallback [`encoding_rs::WINDOWS_1252`] for HTML without a declared charset
//!
//! `had_errors` from `decode` is logged at debug (hostile bodies; never panic).

use scraper::{Html, Selector};
use std::sync::LazyLock;

/// Bytes of the body inspected when sniffing `<meta charset>` (memory bound).
pub(crate) const META_CHARSET_SNIFF_BYTES: usize = 4096;

/// Returns `true` when `Content-Type` is HTML (`text/html` or XHTML).
pub(crate) fn is_html_content_type(content_type: &str) -> bool {
    let ct = content_type.trim();
    let b = ct.as_bytes();
    // Skip optional parameters when matching type/subtype prefix.
    let type_end = b.iter().position(|&c| c == b';').unwrap_or(b.len());
    let main = &b[..type_end];
    let main = trim_ascii(main);
    main.eq_ignore_ascii_case(b"text/html") || main.eq_ignore_ascii_case(b"application/xhtml+xml")
}

/// Content-Types that may still carry HTML when the server lies or omits type.
fn is_generic_or_empty_content_type(content_type: &str) -> bool {
    let ct = content_type.trim();
    if ct.is_empty() {
        return true;
    }
    let type_end = ct.as_bytes().iter().position(|&c| c == b';').unwrap_or(ct.len());
    let main = ct[..type_end].trim();
    main.eq_ignore_ascii_case("application/octet-stream")
        || main.eq_ignore_ascii_case("text/plain")
        || main.eq_ignore_ascii_case("binary/octet-stream")
}

/// Heuristic HTML magic on the first non-whitespace bytes (case-insensitive).
pub(crate) fn looks_like_html(bytes: &[u8]) -> bool {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(0);
    let slice = &bytes[start..];
    if slice.len() >= 9 && slice[..9].eq_ignore_ascii_case(b"<!doctype") {
        return true;
    }
    if slice.len() >= 5 && slice[..5].eq_ignore_ascii_case(b"<html") {
        return true;
    }
    // Partial HTML fragments sometimes start with <head or <body.
    if slice.len() >= 5 && slice[..5].eq_ignore_ascii_case(b"<head") {
        return true;
    }
    if slice.len() >= 5 && slice[..5].eq_ignore_ascii_case(b"<body") {
        return true;
    }
    false
}

/// Accept HTML when Content-Type says so, or when type is generic and body sniffs as HTML.
pub(crate) fn accept_as_html(content_type: &str, body: &[u8]) -> bool {
    if is_html_content_type(content_type) {
        return true;
    }
    if is_generic_or_empty_content_type(content_type) {
        return looks_like_html(body);
    }
    false
}

/// Extract `charset=` from a Content-Type header (RFC 9110: param names case-insensitive).
pub(crate) fn extract_charset(content_type: &str) -> Option<String> {
    for part in content_type.split(';').skip(1) {
        let trimmed = part.trim();
        let lower = trimmed.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("charset=") {
            // Value was lowercased with the key; re-slice original length from trimmed.
            let raw = trimmed.get("charset=".len()..).unwrap_or(value);
            let clean = raw.trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
            if !clean.is_empty() {
                return Some(clean.to_ascii_lowercase());
            }
        }
    }
    // Also accept `charset=` on a single-token string without type prefix.
    let trimmed = content_type.trim();
    let lower = trimmed.to_ascii_lowercase();
    if let Some(value) = lower.strip_prefix("charset=") {
        let raw = trimmed.get("charset=".len()..).unwrap_or(value);
        let clean = raw.trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
        if !clean.is_empty() {
            return Some(clean.to_ascii_lowercase());
        }
    }
    None
}

/// Decode wire/HTML bytes to UTF-8 using BOM → header charset → meta → WINDOWS_1252.
///
/// `header_charset` is the optional label from `Content-Type` (already lowercased preferred).
pub fn decode_to_utf8(bytes: &[u8], header_charset: Option<&str>) -> String {
    // 1. BOM takes precedence (Encoding Standard).
    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(bytes) {
        let rest = &bytes[bom_len..];
        let (cow, _used, had_errors) = enc.decode(rest);
        if had_errors {
            tracing::debug!(
                encoding = enc.name(),
                "charset decode reported had_errors after BOM"
            );
        }
        return cow.into_owned();
    }

    // 2. Header Content-Type charset.
    if let Some(label) = header_charset.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(text) = decode_with_label(bytes, label) {
            return text;
        }
    }

    // 3. Meta charset sniff (scraper on a provisional UTF-8/1252 view of a prefix).
    if let Some(label) = sniff_meta_charset(bytes) {
        if let Some(text) = decode_with_label(bytes, &label) {
            return text;
        }
    }

    // 4. HTML default fallback: WINDOWS_1252 (Encoding Standard / docsrs guidance).
    let (cow, _used, had_errors) = encoding_rs::WINDOWS_1252.decode(bytes);
    if had_errors {
        tracing::debug!("WINDOWS_1252 fallback decode reported had_errors");
    }
    cow.into_owned()
}

fn decode_with_label(bytes: &[u8], label: &str) -> Option<String> {
    let label = label.trim();
    if label.is_empty() {
        return None;
    }
    // Fast path: declared UTF-8.
    if label.eq_ignore_ascii_case("utf-8") || label.eq_ignore_ascii_case("utf8") {
        return Some(match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => {
                let (cow, _, had_errors) = encoding_rs::UTF_8.decode(bytes);
                if had_errors {
                    tracing::debug!("UTF-8 decode reported had_errors");
                }
                cow.into_owned()
            }
        });
    }
    match encoding_rs::Encoding::for_label(label.as_bytes()) {
        Some(enc) => {
            let (cow, _used, had_errors) = enc.decode(bytes);
            if had_errors {
                tracing::debug!(encoding = enc.name(), "charset decode reported had_errors");
            }
            Some(cow.into_owned())
        }
        None => {
            tracing::debug!(
                charset = label,
                "unknown charset label — trying WINDOWS_1252"
            );
            None
        }
    }
}

/// Sniff `<meta charset>` / `http-equiv=content-type` from a body prefix via scraper.
fn sniff_meta_charset(bytes: &[u8]) -> Option<String> {
    let prefix = if bytes.len() > META_CHARSET_SNIFF_BYTES {
        &bytes[..META_CHARSET_SNIFF_BYTES]
    } else {
        bytes
    };
    // Provisional decode for meta parse only (UTF-8 lossy is enough to see ASCII meta).
    let provisional = String::from_utf8_lossy(prefix);
    let document = Html::parse_document(&provisional);

    static SEL_META_CHARSET: LazyLock<Selector> = LazyLock::new(|| {
        Selector::parse("meta[charset]").expect("static selector meta[charset]")
    });
    static SEL_META_HTTP_EQUIV: LazyLock<Selector> = LazyLock::new(|| {
        Selector::parse("meta[http-equiv]").expect("static selector meta[http-equiv]")
    });

    if let Some(el) = document.select(&SEL_META_CHARSET).next() {
        if let Some(cs) = el.value().attr("charset") {
            let clean = cs.trim().trim_matches(|c: char| c == '"' || c == '\'');
            if !clean.is_empty() {
                return Some(clean.to_ascii_lowercase());
            }
        }
    }

    for el in document.select(&SEL_META_HTTP_EQUIV) {
        let equiv = el.value().attr("http-equiv").unwrap_or("");
        if !equiv.eq_ignore_ascii_case("content-type") {
            continue;
        }
        if let Some(content) = el.value().attr("content") {
            if let Some(cs) = extract_charset(content) {
                return Some(cs);
            }
            // content may be only "text/html; charset=..." — extract_charset handles it.
            if let Some(cs) = extract_charset(&format!("text/html; {content}")) {
                return Some(cs);
            }
        }
    }
    None
}

fn trim_ascii(b: &[u8]) -> &[u8] {
    let start = b.iter().position(|c| !c.is_ascii_whitespace()).unwrap_or(b.len());
    let end = b
        .iter()
        .rposition(|c| !c.is_ascii_whitespace())
        .map(|i| i + 1)
        .unwrap_or(start);
    &b[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_html_accepts_text_html_and_variants() {
        assert!(is_html_content_type("text/html"));
        assert!(is_html_content_type("text/html; charset=utf-8"));
        assert!(is_html_content_type("application/xhtml+xml"));
        assert!(is_html_content_type("TEXT/HTML"));
        assert!(is_html_content_type("  text/html ; charset=utf-8"));
    }

    #[test]
    fn is_html_rejects_non_html() {
        assert!(!is_html_content_type("application/pdf"));
        assert!(!is_html_content_type("image/png"));
        assert!(!is_html_content_type("application/json"));
        assert!(!is_html_content_type(""));
    }

    #[test]
    fn extract_charset_identifies_utf8() {
        assert_eq!(
            extract_charset("text/html; charset=UTF-8"),
            Some("utf-8".to_string())
        );
        assert_eq!(
            extract_charset("text/html; charset=\"iso-8859-1\""),
            Some("iso-8859-1".to_string())
        );
    }

    #[test]
    fn extract_charset_case_insensitive_param() {
        assert_eq!(
            extract_charset("text/html; Charset=UTF-8"),
            Some("utf-8".to_string())
        );
        assert_eq!(
            extract_charset("text/html; CHARSET=windows-1252"),
            Some("windows-1252".to_string())
        );
    }

    #[test]
    fn extract_charset_absent_returns_none() {
        assert_eq!(extract_charset("text/html"), None);
        assert_eq!(extract_charset(""), None);
    }

    #[test]
    fn looks_like_html_detects_doctype_and_html() {
        assert!(looks_like_html(b"<!DOCTYPE html><html></html>"));
        assert!(looks_like_html(b"  <HTML lang=en>"));
        assert!(looks_like_html(b"<head></head>"));
        assert!(!looks_like_html(b"%PDF-1.4"));
        assert!(!looks_like_html(b"{\"a\":1}"));
    }

    #[test]
    fn accept_as_html_sniffs_generic_types() {
        let html = b"<!DOCTYPE html><html><body>hi</body></html>";
        assert!(accept_as_html("application/octet-stream", html));
        assert!(accept_as_html("", html));
        assert!(accept_as_html("text/plain", html));
        assert!(!accept_as_html("application/pdf", html));
        assert!(!accept_as_html("application/octet-stream", b"%PDF"));
    }

    #[test]
    fn decode_utf8_pure() {
        let bytes = "hello world".as_bytes();
        let s = decode_to_utf8(bytes, Some("utf-8"));
        assert_eq!(s, "hello world");
    }

    #[test]
    fn decode_latin1_to_utf8() {
        let bytes: &[u8] = &[0xE1, 0x6C, 0x6F];
        let s = decode_to_utf8(bytes, Some("iso-8859-1"));
        assert_eq!(s, "álo");
    }

    #[test]
    fn decode_windows1252_to_utf8() {
        let bytes: &[u8] = &[0xE7];
        let s = decode_to_utf8(bytes, Some("windows-1252"));
        assert_eq!(s, "ç");
    }

    #[test]
    fn decode_unknown_charset_falls_back_to_windows1252() {
        // ASCII-only body is identical under 1252.
        let bytes = b"teste";
        let s = decode_to_utf8(bytes, Some("charset-that-does-not-exist"));
        assert_eq!(s, "teste");
    }

    #[test]
    fn decode_bom_utf8() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"bom-body");
        let s = decode_to_utf8(&bytes, None);
        assert_eq!(s, "bom-body");
    }

    #[test]
    fn decode_meta_charset_windows1252() {
        // Meta declares windows-1252; body has 0xE7 (ç).
        let mut html = b"<!DOCTYPE html><html><head><meta charset=\"windows-1252\"></head><body>"
            .to_vec();
        html.push(0xE7);
        html.extend_from_slice(b"</body></html>");
        let s = decode_to_utf8(&html, None);
        assert!(s.contains('ç'), "got: {s:?}");
    }

    #[test]
    fn decode_absent_charset_uses_windows1252_for_legacy_byte() {
        // No header, no meta, byte 0xE7 → ç under WINDOWS_1252 fallback.
        let bytes: &[u8] = &[0xE7];
        let s = decode_to_utf8(bytes, None);
        assert_eq!(s, "ç");
    }
}
