// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure CPU, sub-microsecond per call. No fan-out — justified.
//! Shared UTF-8 truncation primitives.
//!
//! # Why one module instead of seven private copies
//!
//! Before v1.0.5 this logic existed seven times across `content::readability`,
//! `browser::extract` (twice), `synthesis` (twice), `deep_research::news_diag`
//! and `output::agent_ops`. One of those copies carried a comment admitting it
//! mirrored another, and it was the slower of the two. Copies of a boundary
//! calculation are the kind that stay correct individually and drift as a set.
//!
//! # The two families are NOT merged, deliberately
//!
//! Reading the seven copies showed two different contracts wearing the same
//! name, and collapsing them would have replaced duplication with ambiguity:
//!
//! - **Characters.** What `--truncate-content` counts, per its published
//!   contract: `N` is a number of characters, so a cap of 40 means 40
//!   characters whether they are ASCII or emoji.
//! - **Bytes.** What a memory cap means: a 1 MiB ceiling on a SERP body is
//!   about allocation, and rounding it down to a character boundary only
//!   prevents slicing a multi-byte sequence in half.
//!
//! A single `truncate(s, n)` would have to pick one, and every caller of the
//! other would silently get the wrong ceiling.

/// Largest byte index `<= idx` that lies on a UTF-8 character boundary.
///
/// Returns `s.len()` when `idx` is at or past the end, so it is safe to call
/// with a cap larger than the string.
#[must_use]
pub fn floor_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Truncates to at most `max_bytes`, rounding down to a character boundary.
///
/// This is the **byte** family: use it for memory ceilings, never for a
/// user-facing count. Borrows when nothing is cut.
#[must_use]
pub fn truncate_to_bytes(s: &str, max_bytes: usize) -> &str {
    &s[..floor_char_boundary(s, max_bytes)]
}

/// Truncates to at most `max_chars` characters, borrowing when nothing is cut.
///
/// This is the **character** family, and it is what a user-facing cap means.
#[must_use]
pub fn truncate_to_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((cut, _)) => &s[..cut],
        None => s,
    }
}

/// True when `s` is longer than `max_chars` characters.
///
/// Cheaper than `s.chars().count() > max_chars` on long strings: it stops
/// counting as soon as the answer is known, instead of walking to the end.
#[must_use]
pub fn exceeds_chars(s: &str, max_chars: usize) -> bool {
    s.char_indices().nth(max_chars).is_some()
}

/// Truncates to at most `max_chars`, backing up to the last whitespace.
///
/// Falls back to a hard cut when the prefix has no whitespace at all, which is
/// what a single very long token requires. A `max_chars` of zero yields the
/// empty string rather than panicking.
#[must_use]
pub fn truncate_at_word(text: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return "";
    }
    let prefix = truncate_to_chars(text, max_chars);
    if prefix.len() == text.len() {
        return text;
    }
    match prefix.rfind(char::is_whitespace) {
        Some(pos) => prefix[..pos].trim_end(),
        None => prefix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_boundary_never_splits_a_multibyte_sequence() {
        let s = "áé"; // two 2-byte characters
        assert_eq!(floor_char_boundary(s, 0), 0);
        assert_eq!(floor_char_boundary(s, 1), 0);
        assert_eq!(floor_char_boundary(s, 2), 2);
        assert_eq!(floor_char_boundary(s, 3), 2);
        assert_eq!(floor_char_boundary(s, 99), s.len());
        // The point of the function: every result is sliceable.
        for i in 0..=s.len() + 3 {
            let _ = &s[..floor_char_boundary(s, i)];
        }
    }

    #[test]
    fn byte_and_char_families_disagree_on_purpose() {
        let s = "🦀🦀🦀"; // three 4-byte characters
        assert_eq!(truncate_to_bytes(s, 4), "🦀");
        assert_eq!(truncate_to_chars(s, 4), s);
        assert_eq!(truncate_to_chars(s, 1), "🦀");
        // 4 bytes is one crab; 4 characters is all of them. Merging the two
        // helpers would force one of these callers to be wrong.
    }

    #[test]
    fn truncate_to_chars_borrows_when_nothing_is_cut() {
        let s = "curto";
        assert!(std::ptr::eq(truncate_to_chars(s, 99), s));
        assert!(std::ptr::eq(truncate_to_chars(s, 5), s));
    }

    #[test]
    fn exceeds_chars_agrees_with_a_full_count() {
        for s in ["", "a", "áé", "🦀🦀🦀", "uma frase com espaços"] {
            let total = s.chars().count();
            for n in 0..total + 3 {
                assert_eq!(
                    exceeds_chars(s, n),
                    total > n,
                    "exceeds_chars({s:?}, {n}) disagreed with the full count"
                );
            }
        }
    }

    #[test]
    fn truncate_at_word_backs_up_to_whitespace() {
        let text = "uma frase qualquer com várias palavras";
        let t = truncate_at_word(text, 10);
        assert!(t.chars().count() <= 10);
        assert!(!t.ends_with(' '));
        assert!(text.starts_with(t), "must be a prefix: {t:?}");
    }

    #[test]
    fn truncate_at_word_edge_cases() {
        assert_eq!(truncate_at_word("oi", 100), "oi");
        assert_eq!(truncate_at_word("", 100), "");
        assert_eq!(truncate_at_word("qualquer", 0), "");
        // A single long token has no whitespace to back up to, so it is cut hard.
        assert_eq!(
            truncate_at_word("palavraSemEspacoNenhum", 10)
                .chars()
                .count(),
            10
        );
    }

    /// The two implementations this replaced must agree with it, character for
    /// character, or the consolidation changed behaviour instead of removing a
    /// copy.
    #[test]
    fn matches_both_implementations_it_replaced() {
        fn browser_copy(text: &str, max_size: usize) -> String {
            if max_size == 0 {
                return String::new();
            }
            let total: usize = text.chars().count();
            if total <= max_size {
                return text.to_string();
            }
            let prefix: String = text.chars().take(max_size).collect();
            if let Some(pos) = prefix.rfind(char::is_whitespace) {
                return prefix[..pos].trim_end().to_string();
            }
            prefix
        }
        fn readability_copy(text: &str, max_size: usize) -> String {
            if max_size == 0 {
                return String::new();
            }
            let Some(cut) = text.char_indices().nth(max_size).map(|(i, _)| i) else {
                return text.to_string();
            };
            let prefix = &text[..cut];
            if let Some(pos) = prefix.rfind(char::is_whitespace) {
                return prefix[..pos].trim_end().to_string();
            }
            prefix.to_string()
        }
        let samples = [
            "",
            "oi",
            "uma frase com acentuação e espaços",
            "palavraSemEspacoNenhum",
            "🦀 emoji 🦀 misturado 🦀",
            "   espaços   à   frente   ",
        ];
        for s in samples {
            for n in 0..s.chars().count() + 3 {
                assert_eq!(truncate_at_word(s, n), browser_copy(s, n), "{s:?} / {n}");
                assert_eq!(
                    truncate_at_word(s, n),
                    readability_copy(s, n),
                    "{s:?} / {n}"
                );
            }
        }
    }
}
