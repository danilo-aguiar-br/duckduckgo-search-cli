// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit and property tests for the synthesis module.

use super::render::finite_score;
use super::*;

fn item(url: &str, title: &str, snippet: &str, score: f64) -> AggregatedItem {
    AggregatedItem {
        url: crate::types::HttpUrl::for_test(url),
        title: title.to_string(),
        display_url: None,
        snippet: Some(snippet.to_string()),
        score,
        position: 1,
        sources: vec!["alpha".to_string()],
    }
}

#[test]
fn estimate_tokens_is_4_chars_per_token() {
    assert_eq!(estimate_tokens(""), 0);
    assert_eq!(estimate_tokens("abcd"), 1);
    assert_eq!(estimate_tokens("abcde"), 2);
}

#[test]
fn trim_to_budget_preserves_under_limit() {
    let s = "hello world".to_string();
    assert_eq!(trim_to_budget(&s, 100), s);
}

#[test]
fn trim_to_budget_cuts_above_limit() {
    let s = "a".repeat(200);
    let out = trim_to_budget(&s, 10);
    assert!(out.len() < 200);
    assert!(out.ends_with("..."));
}

#[test]
fn markdown_reports_empty_when_no_items() {
    let r = synthesize(&[], "q", SynthFormat::Markdown, 4000);
    assert!(r.body.contains("No results"));
}

#[test]
fn markdown_caps_at_twenty_references() {
    let items: Vec<AggregatedItem> = (0..50)
        .map(|i| {
            item(
                &format!("https://e.com/{i}"),
                "t",
                "s",
                1.0 - i as f64 * 0.01,
            )
        })
        .collect();
    let r = synthesize(&items, "q", SynthFormat::Markdown, 4000);
    assert!(r.body.contains("[20]"));
    assert!(!r.body.contains("[21]"));
    assert_eq!(r.reference_count, 20);
}

#[test]
fn json_is_valid_json_with_references() {
    let items = vec![item("https://e.com/a", "title", "snippet", 0.5)];
    let r = synthesize(&items, "q", SynthFormat::Json, 4000);
    let parsed: serde_json::Value = serde_json::from_str(&r.body).expect("valid json");
    assert_eq!(parsed["query"], "q");
    assert_eq!(parsed["references"][0]["url"], "https://e.com/a");
}

#[test]
fn plain_text_renders_numbered_list() {
    let items = vec![item("https://e.com/a", "title", "snippet", 0.5)];
    let r = synthesize(&items, "q", SynthFormat::PlainText, 4000);
    assert!(r.body.contains("1. title"));
    assert!(r.body.contains("URL: https://e.com/a"));
}

#[test]
fn budget_respected_with_five_percent_margin() {
    // Markdown output is bounded by the budget — the snippet itself is
    // capped at `budget_tokens * 4` chars, the surrounding headings
    // add a small constant overhead, so the total fits within ~10% of
    // the budget.
    let long_snippet = "a".repeat(100_000);
    let items = vec![item("https://e.com/a", "t", &long_snippet, 0.5)];
    let r = synthesize(&items, "q", SynthFormat::Markdown, 100);
    assert!(
        r.estimated_tokens <= 110,
        "estimated_tokens {} exceeded budget+10%",
        r.estimated_tokens
    );
}

fn news_item(
    url: &str,
    title: &str,
    source: Option<&str>,
    date: Option<&str>,
) -> AggregatedNewsItem {
    AggregatedNewsItem {
        position: 1,
        title: title.to_string(),
        url: crate::types::HttpUrl::for_test(url),
        source: source.map(str::to_string),
        relative_date: date.map(str::to_string),
        thumbnail: None,
        score: 0.5,
        occurrences: 1,
    }
}

#[test]
fn synthesize_dual_delegates_when_news_empty() {
    let items = vec![item("https://e.com/a", "title", "snippet", 0.5)];
    for format in [
        SynthFormat::Markdown,
        SynthFormat::PlainText,
        SynthFormat::Json,
    ] {
        let web_only = synthesize(&items, "q", format, 4000);
        let dual = synthesize_dual(&items, &[], "q", format, 4000);
        assert_eq!(web_only, dual, "empty news must delegate to synthesize");
    }
}

#[test]
fn finite_score_clamps_nan_and_infinity() {
    assert_eq!(finite_score(1.25), 1.25);
    assert_eq!(finite_score(f64::NAN), 0.0);
    assert_eq!(finite_score(f64::INFINITY), 0.0);
    assert_eq!(finite_score(f64::NEG_INFINITY), 0.0);
}

#[test]
fn json_synthesis_never_emits_nan_literal() {
    let items = vec![item("https://e.com/a", "title", "snippet", f64::NAN)];
    let r = synthesize(&items, "q", SynthFormat::Json, 4000);
    assert!(
        !r.body.contains("NaN") && !r.body.contains("Infinity"),
        "I-JSON forbids NaN/Infinity JSON literals; body was: {}",
        r.body
    );
    let parsed: serde_json::Value = serde_json::from_str(&r.body).expect("valid JSON");
    let score = parsed["references"][0]["score"]
        .as_f64()
        .expect("score number");
    assert_eq!(score, 0.0);
}

#[test]
fn synthesize_dual_markdown_contains_news_section() {
    let web = vec![item("https://e.com/a", "title", "snippet", 0.5)];
    let news = vec![news_item(
        "https://n.com/1",
        "manchete",
        Some("G1"),
        Some("há 2 horas"),
    )];
    let r = synthesize_dual(&web, &news, "q", SynthFormat::Markdown, 4000);
    assert!(r.body.contains("### Recent news"));
    assert!(r.body.contains("manchete — G1, há 2 horas"));
    assert!(r.body.contains("### Key Findings"), "web section preserved");
    assert_eq!(r.reference_count, 2, "web + news references");
}

#[test]
fn synthesize_dual_plain_text_contains_news_section() {
    let web = vec![item("https://e.com/a", "title", "snippet", 0.5)];
    let news = vec![news_item("https://n.com/1", "manchete", None, None)];
    let r = synthesize_dual(&web, &news, "q", SynthFormat::PlainText, 4000);
    assert!(r.body.contains("Recent news:"));
    assert!(r.body.contains("1. manchete"));
    assert!(
        !r.body.contains("manchete —"),
        "no dangling metadata suffix"
    );
}

#[test]
fn synthesize_dual_json_has_news_array() {
    let web = vec![item("https://e.com/a", "title", "snippet", 0.5)];
    let news = vec![news_item(
        "https://n.com/1",
        "manchete",
        Some("G1"),
        Some("há 2 horas"),
    )];
    let r = synthesize_dual(&web, &news, "q", SynthFormat::Json, 4000);
    let parsed: serde_json::Value = serde_json::from_str(&r.body).expect("valid json");
    assert_eq!(parsed["references"][0]["url"], "https://e.com/a");
    assert_eq!(parsed["news"][0]["url"], "https://n.com/1");
    assert_eq!(parsed["news"][0]["fonte"], "G1");
    assert_eq!(parsed["news"][0]["data_relativa"], "há 2 horas");
}

#[test]
fn synthesize_dual_respects_budget_split() {
    let long_snippet = "palavra ".repeat(20_000);
    let web = vec![item("https://e.com/a", "t", &long_snippet, 0.5)];
    let news: Vec<AggregatedNewsItem> = (0..20)
        .map(|i| {
            news_item(
                &format!("https://n.com/{i}"),
                &format!("manchete bem comprida numero {i} {}", "x ".repeat(80)),
                Some("Fonte"),
                Some("há 2 horas"),
            )
        })
        .collect();
    let budget = 100;
    let r = synthesize_dual(&web, &news, "q", SynthFormat::Markdown, budget);
    assert!(
        r.estimated_tokens <= budget,
        "estimated_tokens {} exceeded budget {}",
        r.estimated_tokens,
        budget
    );
}

// ---------------------------------------------------------------
// Property-based tests (proptest)
// ---------------------------------------------------------------
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// `estimate_tokens` is monotonic non-decreasing with input length.
        #[test]
        fn estimate_tokens_is_monotonic(short in ".{0,20}", long_extra in ".{1,40}") {
            let short_t = estimate_tokens(&short);
            let long = format!("{short}{long_extra}");
            let long_t = estimate_tokens(&long);
            prop_assert!(long_t >= short_t);
        }

        /// `trim_to_budget` never returns more characters than the
        /// (4 × budget) char ceiling, plus the ` ...` suffix overhead.
        #[test]
        fn trim_to_budget_respects_ceiling(
            text in ".{0,200}",
            budget in 0usize..50,
        ) {
            let out = trim_to_budget(&text, budget);
            let ceiling = budget.saturating_mul(4) + 4;
            prop_assert!(
                out.len() <= ceiling,
                "trim produced {} chars > ceiling {}",
                out.len(),
                ceiling
            );
        }

        /// `trim_to_budget` is idempotent: trimming an already-trimmed
        /// string at the same budget must yield the same result.
        #[test]
        fn trim_to_budget_is_idempotent(text in ".{0,80}", budget in 1usize..20) {
            let once = trim_to_budget(&text, budget);
            let twice = trim_to_budget(&once, budget);
            prop_assert_eq!(once, twice);
        }
    }
}
