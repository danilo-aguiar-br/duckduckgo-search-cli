// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests extracted from `src/output/project.rs`.

use super::*;
use crate::types::{HttpUrl, SearchMetadata};

fn sample_result(title: &str, url: &str, snippet: &str) -> SearchResult {
    SearchResult {
        position: 1,
        title: title.into(),
        url: HttpUrl::try_new(url).expect("url"),
        display_url: Some("example.com".into()),
        snippet: Some(snippet.into()),
        original_title: None,
        content: Some("LONG BODY".into()),
        content_size: Some(9),
        content_extraction_method: Some("readability".into()),
    }
}

fn sample_output(results: Vec<SearchResult>) -> SearchOutput {
    SearchOutput {
        query: "q".into(),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: crate::types::test_timestamp(),
        region: "br-pt".into(),
        result_count: results.len() as u32,
        results,
        pages_fetched: 1,
        error: None,
        message: None,
        metadata: SearchMetadata {
            selectors_hash: "abc".into(),
            user_agent: "test-ua".into(),
            ..SearchMetadata::default()
        },
        news: None,
        news_count: None,
    }
}

#[test]
fn parse_fields_accepts_en_aliases_and_dedupes() {
    let fs = FieldSet::parse("url, title, titulo, URL").expect("parse");
    assert_eq!(fs.keys(), &["url".to_string(), "titulo".to_string()]);
    assert!(!fs.requests_content());
}

#[test]
fn parse_fields_content_keys() {
    let fs = FieldSet::parse("conteudo,url").expect("parse");
    assert!(fs.requests_content());
}

#[test]
fn parse_fields_unknown_errors() {
    assert!(FieldSet::parse("url,nope").is_err());
    assert!(FieldSet::parse("  ,  ").is_err());
}

#[test]
fn filter_title_and_host() {
    let r = sample_result("Rust Book", "https://doc.rust-lang.org/book/", "learn");
    let f = ResultFilter::parse("titulo~rust").unwrap();
    assert!(f.matches_web(&r));
    let f2 = ResultFilter::parse("host:rust-lang.org").unwrap();
    assert!(f2.matches_web(&r));
    let f3 = ResultFilter::parse("host:example.com").unwrap();
    assert!(!f3.matches_web(&r));
}

/// GAP-E2E-V19-FILTER-SYNTAX-FOOTGUN: fail-fast on common typos.
#[test]
fn filter_rejects_tilde_equals_and_equality() {
    assert!(ResultFilter::parse("titulo~=Rust").is_err());
    assert!(ResultFilter::parse("posicao=1").is_err());
    assert!(ResultFilter::parse("title=foo").is_err());
    assert!(ResultFilter::parse("unknown~x").is_err());
    // Valid forms still work.
    assert!(ResultFilter::parse("titulo~Rust").is_ok());
    assert!(ResultFilter::parse("host:example.com").is_ok());
    assert!(ResultFilter::parse("bare-needle").is_ok());
}

/// GAP-E2E-V19-LIMIT-FLAG-MISSING: post-SERP slice.
#[test]
fn result_limit_truncates_web_and_news() {
    let mut out = sample_output(vec![
        sample_result("A", "https://a.example/", "a"),
        sample_result("B", "https://b.example/", "b"),
        sample_result("C", "https://c.example/", "c"),
    ]);
    apply_result_limit_search(&mut out, Some(2));
    assert_eq!(out.results.len(), 2);
    assert_eq!(out.result_count, 2);
}

#[test]
fn project_json_drops_unselected_result_keys() {
    let mut out = sample_output(vec![sample_result("T", "https://example.com/", "s")]);
    let fs = FieldSet::parse("url").unwrap();
    apply_to_search_output(&mut out, Some(&fs), None);
    let json = format_search_json_projected(&out, &fs).unwrap();
    assert!(json.contains("https://example.com/"));
    assert!(!json.contains("LONG BODY"));
    assert!(!json.contains("\"title\""));
    assert!(!json.contains("\"snippet\""));
    // envelope preserved (v2 EN wire)
    assert!(json.contains("\"query\""));
    assert!(json.contains("\"results\""));
}

/// v2.0.0: EN `--fields` project to EN serialize keys (ADR-0027).
#[test]
fn project_en_fields_serialize_en_wire_keys() {
    let mut out = sample_output(vec![sample_result(
        "Rust Book",
        "https://doc.rust-lang.org/",
        "learn",
    )]);
    let fs = FieldSet::parse("title,url").expect("EN fields");
    // FieldSet still canonicalizes to PT internal allowlist for matching.
    assert_eq!(fs.keys(), &["titulo".to_string(), "url".to_string()]);
    apply_to_search_output(&mut out, Some(&fs), None);
    let json = format_search_json_projected(&out, &fs).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).expect("json");
    assert!(
        v.get("results").is_some(),
        "envelope key is EN results: {json}"
    );
    assert!(
        v.get("resultados").is_none(),
        "must not serialize PT resultados"
    );
    let row = &v["results"][0];
    assert_eq!(row["title"], "Rust Book");
    assert!(row.get("titulo").is_none(), "must not serialize PT titulo");
    assert!(row["url"].as_str().unwrap().contains("rust-lang.org"));
    assert!(row.get("snippet").is_none() || row["snippet"].is_null());
}

#[test]
fn filter_reduces_result_count() {
    let mut out = sample_output(vec![
        sample_result("Keep Me", "https://keep.example/", "x"),
        sample_result("Drop", "https://drop.example/", "y"),
    ]);
    let f = ResultFilter::parse("url~keep").unwrap();
    apply_to_search_output(&mut out, None, Some(&f));
    assert_eq!(out.results.len(), 1);
    assert_eq!(out.result_count, 1);
}

fn sample_deep() -> DeepResearchOutput {
    use crate::deep_research::DeepResearchMetadata;
    DeepResearchOutput {
        kind: crate::types::DeepResearchKind::DeepResearch,
        query: "q".into(),
        metadata: DeepResearchMetadata {
            original_query: "q".into(),
            sub_queries: vec![],
            aggregation_strategy: "rrf".into(),
            unique_result_count: 2,
            unique_news_count: 1,
            total_elapsed_ms: 1,
            cascade_level: None,
            used_chrome: true,
            chrome_path_resolved: None,
            chrome_channel: None,
            sub_queries_total: 0,
            sub_queries_ok: 0,
            sub_queries_error: 0,
            partial: false,
            chrome_contention_advisory: false,
        },
        results: vec![
            AggregatedItem {
                url: HttpUrl::try_new("https://keep.example/a").unwrap(),
                title: "Keep Me".into(),
                display_url: Some("keep.example".into()),
                snippet: Some("body".into()),
                score: 0.9,
                position: 1,
                sources: vec!["sub1".into()],
            },
            AggregatedItem {
                url: HttpUrl::try_new("https://drop.example/b").unwrap(),
                title: "Drop".into(),
                display_url: None,
                snippet: Some("other".into()),
                score: 0.1,
                position: 2,
                sources: vec!["sub2".into()],
            },
        ],
        news: vec![AggregatedNewsItem {
            position: 1,
            title: "News Keep".into(),
            url: HttpUrl::try_new("https://keep.example/news").unwrap(),
            source: Some("Wire".into()),
            relative_date: Some("1h".into()),
            thumbnail: None,
            score: 0.5,
            occurrences: 2,
        }],
        news_count: 1,
        synth: Some(crate::synthesis::SynthesizedReport {
            format: crate::synthesis::SynthFormat::Markdown,
            body: "HUGE REPORT".into(),
            estimated_tokens: 3,
            reference_count: 0,
        }),
    }
}

#[test]
fn deep_filter_reduces_results_and_updates_counts() {
    let mut out = sample_deep();
    let f = ResultFilter::parse("url~keep").unwrap();
    apply_to_deep_output(&mut out, None, Some(&f));
    assert_eq!(out.results.len(), 1);
    assert_eq!(out.news.len(), 1);
    assert_eq!(out.metadata.unique_result_count, 1);
    assert_eq!(out.news_count, 1);
}

#[test]
fn deep_fields_project_drops_score_fontes_and_synth() {
    let mut out = sample_deep();
    let fs = FieldSet::parse("url,titulo").unwrap();
    apply_to_deep_output(&mut out, Some(&fs), None);
    assert!(out.synth.is_none(), "synth must drop under --fields");
    let json = format_deep_json_projected(&out, &fs).unwrap();
    assert!(json.contains("https://keep.example/a"));
    assert!(json.contains("Keep Me"));
    assert!(!json.contains("\"score\""));
    assert!(!json.contains("\"sources\"") && !json.contains("\"fontes\""));
    assert!(!json.contains("HUGE REPORT"));
    assert!(json.contains("\"kind\"") || json.contains("\"tipo\""));
    assert!(json.contains("\"metadata\"") || json.contains("\"metadados\""));
}

#[test]
fn deep_fields_parse_score_and_fontes_aliases() {
    let fs = FieldSet::parse("url,score,sources").unwrap();
    assert!(fs.contains("url"));
    assert!(fs.contains("score"));
    assert!(fs.contains("fontes"));
}

/// The one-pass escaper must agree with the chained-`replace` shape it replaced.
///
/// The rewrite exists to remove five full copies of every cell, not to change
/// what a cell looks like. This test keeps the old expression as the oracle so
/// the optimisation cannot quietly become a behaviour change.
#[test]
fn one_pass_tsv_escaper_matches_the_chained_replace_it_replaced() {
    fn oracle(raw: &str) -> String {
        raw.replace('\\', "\\\\")
            .replace('\t', "\\t")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    }
    let cases = [
        "",
        "sem nada a escapar",
        "\t",
        "\n",
        "\r",
        "\\",
        "a\tb\nc\rd\\e",
        "\\t literal e \t real",
        "acentuação e emoji 🦀 com \t tab",
        "\\\\\t\t\n\n\r\r",
        "termina com barra \\",
    ];
    for raw in cases {
        let mut got = String::new();
        super::push_tsv_escaped(&mut got, raw);
        assert_eq!(got, oracle(raw), "escaping diverged for {raw:?}");
    }
}

/// A cell that needs no escaping must reach the buffer without a detour.
///
/// `web_field_str` used to clone every field so the escaper could copy it four
/// more times. Borrowing is the point of the change, so it is asserted rather
/// than assumed.
#[test]
fn web_field_borrows_instead_of_cloning() {
    use std::borrow::Cow;
    let mut r = sample_result("Título", "https://example.com/a", "trecho");
    r.display_url = None;
    assert!(matches!(
        super::web_field_str(&r, "titulo"),
        Cow::Borrowed(_)
    ));
    assert!(matches!(
        super::web_field_str(&r, "conteudo"),
        Cow::Borrowed(_)
    ));
    assert!(matches!(super::web_field_str(&r, "url"), Cow::Borrowed(_)));
    // Absent optionals are the empty cell, borrowed from a literal.
    assert!(matches!(
        super::web_field_str(&r, "url_exibicao"),
        Cow::Borrowed("")
    ));
    // Only the numeric fields have to allocate.
    assert!(matches!(super::web_field_str(&r, "posicao"), Cow::Owned(_)));
    assert!(matches!(
        super::web_field_str(&r, "tamanho_conteudo"),
        Cow::Owned(_)
    ));
}
