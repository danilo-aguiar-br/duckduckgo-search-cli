// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light (string measurement, no I/O).
//! Token-budget estimation and boundary-safe truncation for synthesis.

/// Approximate token count: 1 token ≈ 4 characters.
///
/// # Examples
///
/// ```
/// use duckduckgo_search_cli::synthesis::estimate_tokens;
///
/// assert_eq!(estimate_tokens(""), 0);
/// assert_eq!(estimate_tokens("abcd"), 1);
/// assert_eq!(estimate_tokens("abcde"), 2);
/// assert_eq!(estimate_tokens("a 16-character str!"), 5);
/// ```
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Truncates `text` to roughly `budget_tokens` tokens, preferring word
/// boundaries. Truncation is done at a valid UTF-8 char boundary so
/// multi-byte characters are never split.
///
/// # Examples
///
/// ```
/// use duckduckgo_search_cli::synthesis::trim_to_budget;
///
/// // Input shorter than budget: returned unchanged.
/// let s = "short text";
/// assert_eq!(trim_to_budget(s, 100), s);
///
/// // Truncation respects the nearest word boundary.
/// let long = "the quick brown fox jumps over the lazy dog";
/// let trimmed = trim_to_budget(long, 3);
/// assert!(trimmed.starts_with("the quick"));
/// assert!(trimmed.contains(" ..."));
///
/// // Multi-byte UTF-8 is never split mid-character.
/// let emoji_text = "🦀🦀🦀🦀 a b c d e f g h i j";
/// let out = trim_to_budget(emoji_text, 2);
/// assert!(out.is_char_boundary(out.len()));
/// ```
pub fn trim_to_budget(text: &str, budget_tokens: usize) -> String {
    let char_budget = budget_tokens.saturating_mul(4);
    if text.len() <= char_budget {
        return text.to_string();
    }
    // Snap the byte index to the nearest valid char boundary at or before
    // `char_budget`. This prevents panics on multi-byte UTF-8 inputs.
    let cut_byte = floor_char_boundary(text, char_budget);
    let mut cut = text[..cut_byte].to_string();
    if let Some(last_space) = cut.rfind(' ') {
        cut.truncate(last_space);
    }
    cut.push_str(" ...");
    cut
}

/// Returns the largest byte index `<= idx` that is a valid char boundary
/// in `s`. Returns 0 when `idx == 0`. Panics only on `idx > s.len()`.
fn floor_char_boundary(s: &str, idx: usize) -> usize {
    crate::text::floor_char_boundary(s, idx)
}
