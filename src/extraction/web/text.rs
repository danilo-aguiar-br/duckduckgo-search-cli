// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (text normalisation)
//! Text extraction, normalisation and ad-class helpers plus payload limits.

use scraper::ElementRef;

/// Bounded limits to prevent absurdly large payloads (section 5.4 — rule 4).
pub(super) const TITLE_LIMIT: usize = 200;
pub(super) const URL_LIMIT: usize = 2000;
pub(super) const SNIPPET_LIMIT: usize = 500;

pub(crate) fn join_text(el: &ElementRef<'_>) -> String {
    let mut out = String::with_capacity(128);
    let mut need_space = false;
    for frag in el.text() {
        for word in frag.split_whitespace() {
            if need_space {
                out.push(' ');
            }
            out.push_str(word);
            need_space = true;
        }
    }
    out
}

/// Dynamic version: accepts the list of ad classes configured in the TOML file.
pub(super) fn contains_dynamic_ad_class(element: &ElementRef<'_>, raw_classes: &[String]) -> bool {
    element
        .value()
        .classes()
        .any(|class| raw_classes.iter().any(|c| c == class))
}

/// Normalises extracted text: collapses whitespace, trims and truncates at `limit` characters
/// respecting UTF-8 character boundaries.
pub(crate) fn normalize_text(raw: &str, limit: usize) -> String {
    let mut result_buf = String::with_capacity(raw.len().min(limit + 64));
    let mut needs_space = false;
    let mut chars_written: usize = 0;

    for word in raw.split_whitespace() {
        let separator = usize::from(needs_space);
        let word_len = word.chars().count();

        if chars_written + separator + word_len > limit {
            let remaining = limit.saturating_sub(chars_written + separator);
            if remaining > 0 {
                if needs_space {
                    result_buf.push(' ');
                }
                for ch in word.chars().take(remaining) {
                    result_buf.push(ch);
                }
            }
            break;
        }

        if needs_space {
            result_buf.push(' ');
            chars_written += 1;
        }
        result_buf.push_str(word);
        chars_written += word_len;
        needs_space = true;
    }

    result_buf
}
