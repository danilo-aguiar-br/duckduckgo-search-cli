// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reflective depth follow-up heuristics (GAP-E2E-48-008 / GAP-E2E-51-012).

use crate::aggregation::AggregatedItem;

/// Minimum alphanumeric tokens a depth follow-up query must contain.
const MIN_DEPTH_SUBQUERY_TOKENS: usize = 2;
/// Minimum non-stopword content tokens required (rejects junk glue like "rust your").
const MIN_DEPTH_CONTENT_TOKENS: usize = 2;
/// Mined gap-term length bounds (inclusive).
const MIN_GAP_TERM_LEN: usize = 4;
const MAX_GAP_TERM_LEN: usize = 32;

/// Stopwords / glue tokens excluded from depth reflection mining and quality checks.
///
fn depth_stopwords() -> std::collections::HashSet<&'static str> {
    [
        // EN function / glue
        "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "with", "from", "by", "as",
        "is", "are", "was", "were", "be", "been", "being", "this", "that", "these", "those", "it",
        "its", "at", "if", "but", "not", "nor", "so", "than", "then", "too", "very", "just", "only",
        "also", "into", "over", "under", "again", "further", "once", "here", "there", "all", "any",
        "both", "each", "few", "more", "most", "other", "some", "such", "no", "own", "same", "can",
        "will", "shall", "may", "might", "must", "could", "would", "should", "does", "did", "doing",
        "done", "have", "has", "had", "having", "do", "about", "above", "below", "between", "through",
        "during", "before", "after", "without", "within", "against", "across", "along", "among",
        "around", "because", "while", "until", "unless", "although", "though", "whether",
        "you", "your", "yours", "yourself", "yourselves", "we", "our", "ours", "ourselves", "they",
        "them", "their", "theirs", "themselves", "he", "him", "his", "she", "her", "hers", "me",
        "my", "mine", "myself", "who", "whom", "whose", "which", "what", "how", "why", "when", "where",
        "http", "https", "www", "com", "org", "net", "html", "php", "asp",
        // PT function / glue
        "de", "da", "do", "dos", "das", "em", "um", "uma", "uns", "umas", "os", "as", "ao", "aos",
        "à", "às", "para", "pra", "com", "por", "pelo", "pela", "pelos", "pelas", "não", "nao",
        "que", "se", "ser", "estar", "ter", "foi", "são", "sao", "era", "eram", "será", "sera",
        "seu", "sua", "seus", "suas", "meu", "minha", "meus", "minhas", "nosso", "nossa", "nossos",
        "nossas", "eles", "elas", "ele", "ela", "você", "voce", "vocês", "voces", "nós", "nos",
        "lhe", "lhes", "este", "esta", "estes", "estas", "esse", "essa", "esses", "essas", "isso",
        "isto", "aquele", "aquela", "aqueles", "aquelas", "aquilo", "mais", "menos", "muito",
        "muita", "muitos", "muitas", "pouco", "pouca", "poucos", "poucas", "sobre", "entre",
        "depois", "antes", "quando", "onde", "como", "porque", "pois", "mas", "ou", "já", "ja",
        "ainda", "também", "tambem", "só", "so", "sem", "até", "ate", "após", "apos", "desde",
        "durante", "através", "atraves", "contra", "segundo", "cada", "todo", "toda", "todos",
        "todas", "outro", "outra", "outros", "outras", "mesmo", "mesma", "mesmos", "mesmas",
    ]
    .into_iter()
    .collect()
}

/// Split a query string into lowercase alphanumeric tokens (keeps `-` / `_` inside tokens).
fn tokenize_depth_query(s: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(8);
    for tok in s.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_') {
        if tok.is_empty() {
            continue;
        }
        out.push(tok.to_ascii_lowercase());
    }
    out
}

/// Non-stopword content tokens (min length 3 to keep short technical terms like "io").
fn content_tokens(tokens: &[String], stop: &std::collections::HashSet<&str>) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    for t in tokens {
        if t.len() >= 3 && !stop.contains(t.as_str()) {
            out.push(t.clone());
        }
    }
    out
}

/// Quality gate for reflective depth sub-queries (GAP-E2E-51-012).
///
/// Rejects:
/// - fewer than [`MIN_DEPTH_SUBQUERY_TOKENS`] tokens
/// - stopword-only / junk glue (fewer than [`MIN_DEPTH_CONTENT_TOKENS`] content tokens)
/// - near-duplicates of the parent query (no new content token vs parent)
/// - exact parent after case/whitespace normalization
pub(crate) fn is_quality_depth_subquery(parent: &str, candidate: &str) -> bool {
    let parent = parent.trim();
    let candidate = candidate.trim();
    if candidate.is_empty() || candidate.len() > 200 {
        return false;
    }
    let stop = depth_stopwords();
    let parent_tokens = tokenize_depth_query(parent);
    let cand_tokens = tokenize_depth_query(candidate);
    if cand_tokens.len() < MIN_DEPTH_SUBQUERY_TOKENS {
        return false;
    }
    let parent_content = content_tokens(&parent_tokens, &stop);
    let cand_content = content_tokens(&cand_tokens, &stop);
    if cand_content.len() < MIN_DEPTH_CONTENT_TOKENS {
        return false;
    }
    // Exact / whitespace-normalized duplicate of parent.
    if parent_tokens == cand_tokens {
        return false;
    }
    // Near-duplicate: every content token already present in the parent.
    let parent_set: std::collections::HashSet<&str> =
        parent_content.iter().map(String::as_str).collect();
    let has_new_content = cand_content.iter().any(|t| !parent_set.contains(t.as_str()));
    if !has_new_content {
        return false;
    }
    true
}

/// Build heuristic follow-up queries from rare tokens in top aggregated titles/snippets.
///
/// Applies [`is_quality_depth_subquery`] so junk glues like `"rust your"` never fan out.
pub(crate) fn heuristic_depth_follow_ups(
    original: &str,
    aggregated: &[AggregatedItem],
    limit: usize,
    seen: &std::collections::HashSet<String>,
) -> Vec<String> {
    use std::collections::HashMap;
    let stop = depth_stopwords();
    let parent_content: std::collections::HashSet<String> = content_tokens(
        &tokenize_depth_query(original),
        &stop,
    )
    .into_iter()
    .collect();

    let mut freq: HashMap<String, usize> = HashMap::with_capacity(64);
    for item in aggregated.iter().take(12) {
        let blob = format!(
            "{} {}",
            item.title,
            item.snippet.as_deref().unwrap_or("")
        );
        for tok in blob.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_') {
            let t = tok.to_ascii_lowercase();
            if t.len() < MIN_GAP_TERM_LEN
                || t.len() > MAX_GAP_TERM_LEN
                || stop.contains(t.as_str())
                || parent_content.contains(&t)
            {
                continue;
            }
            *freq.entry(t).or_insert(0) += 1;
        }
    }
    // Prefer uncommon-but-present terms (appear 1..=3 times) as gap fillers.
    let mut candidates: Vec<(usize, String)> = Vec::with_capacity(freq.len());
    for (t, c) in freq {
        if (1..=3).contains(&c) {
            candidates.push((c, t));
        }
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));

    let base = original.trim();
    let mut out = Vec::with_capacity(limit);
    for (_, term) in candidates {
        if out.len() >= limit {
            break;
        }
        let q = format!("{base} {term}");
        let key = q.to_ascii_lowercase();
        if seen.contains(&key) || seen.contains(&term) {
            continue;
        }
        if !is_quality_depth_subquery(base, &q) {
            continue;
        }
        out.push(q);
    }
    // Fallback: pair original with a content keyword from the top title.
    if out.is_empty() {
        if let Some(first) = aggregated.first() {
            let title_tokens = tokenize_depth_query(&first.title);
            let word = content_tokens(&title_tokens, &stop)
                .into_iter()
                .find(|w| w.len() >= MIN_GAP_TERM_LEN && !parent_content.contains(w))
                .unwrap_or_else(|| "overview".to_string());
            let q = format!("{base} {word}");
            let key = q.to_ascii_lowercase();
            if !seen.contains(&key) && is_quality_depth_subquery(base, &q) {
                out.push(q);
            }
        }
    }
    out
}
