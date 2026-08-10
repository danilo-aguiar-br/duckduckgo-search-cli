// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests based on REAL HTML fixtures captured from `DuckDuckGo`.
//!
//! The fixtures under `tests/fixtures/` were captured on 2026-04-14 via:
//!   xh "<https://html.duckduckgo.com/html/?q=rust+programming>"
//!   xh "<https://lite.duckduckgo.com/lite/?q=rust+programming>"
//!
//! IMPORTANT: the User-Agent `xh` sends by default (`xh/0.25.3`) is NOT
//! flagged as a bot by `DuckDuckGo`, unlike "complete" Chrome/Firefox UAs,
//! which return HTTP 202 with a challenge anomaly. That difference was the
//! root cause of the "0 results" reported in iteration 3 and is documented
//! in PHASE A of this iteration's diagnosis.
//!
//! These tests guard against selector regressions: if DDG changes the DOM,
//! they fail (and the fixture must be re-captured).

use duckduckgo_search_cli::extraction::{
    extract_results, extract_results_lite, extract_results_with_strategies,
};
use std::fs;
use std::path::PathBuf;

fn load_fixture(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fixtures");
    path.push(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

#[test]
fn real_html_page_1_extracts_at_least_ten_results() {
    let html = load_fixture("ddg_html_pagina_1.html");
    let results = extract_results(&html);
    assert!(
        results.len() >= 10,
        "expected >= 10 results, got {}",
        results.len()
    );

    // All results must have non-empty title and URL.
    for r in &results {
        assert!(
            !r.title.is_empty(),
            "empty title at position {}",
            r.position
        );
        assert!(
            !r.url.as_str().is_empty(),
            "empty URL at position {}",
            r.position
        );
        assert!(
            r.url.as_str().starts_with("https://") || r.url.as_str().starts_with("http://"),
            "URL is not absolute at position {}: {}",
            r.position,
            r.url
        );
        // No URL may remain an internal redirect.
        assert!(
            !r.url.as_str().contains("duckduckgo.com/l/?uddg="),
            "URL was not unwrapped: {}",
            r.url
        );
    }

    // Most results must have a snippet.
    let with_snippet = results
        .iter()
        .filter(|r| r.snippet.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
        .count();
    assert!(
        with_snippet >= 8,
        "expected at least 8 results with a snippet, got {with_snippet}"
    );

    // Positions must be sequential starting at 1.
    for (i, r) in results.iter().enumerate() {
        assert_eq!(
            r.position,
            (i + 1) as u32,
            "positions must be sequential and 1-indexed"
        );
    }
}

#[test]
fn real_html_page_1_extracts_display_url_when_present() {
    let html = load_fixture("ddg_html_pagina_1.html");
    let results = extract_results(&html);
    let with_display_url = results
        .iter()
        .filter(|r| {
            r.display_url
                .as_ref()
                .map(|u| !u.is_empty())
                .unwrap_or(false)
        })
        .count();
    assert!(
        with_display_url >= 8,
        "expected >= 8 results with display_url, got {with_display_url}"
    );
}

#[test]
fn real_lite_page_1_extracts_at_least_ten_results() {
    let html = load_fixture("ddg_lite_pagina_1.html");
    let results = extract_results_lite(&html);
    assert!(
        results.len() >= 10,
        "expected >= 10 Lite results, got {}",
        results.len()
    );

    for r in &results {
        assert!(
            !r.title.is_empty(),
            "empty Lite title at position {}",
            r.position
        );
        assert!(
            !r.url.as_str().is_empty(),
            "empty Lite URL at position {}",
            r.position
        );
        assert!(
            r.url.as_str().starts_with("https://") || r.url.as_str().starts_with("http://"),
            "Lite URL is not absolute: {}",
            r.url
        );
        assert!(
            !r.url.as_str().contains("duckduckgo.com/l/?uddg="),
            "Lite URL was not unwrapped: {}",
            r.url
        );
    }

    // Most Lite results must have a snippet (it comes in a separate <tr>).
    let with_snippet = results
        .iter()
        .filter(|r| r.snippet.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
        .count();
    assert!(
        with_snippet >= 8,
        "expected >= 8 Lite results with a snippet, got {with_snippet}"
    );
}

#[test]
fn real_html_page_1_filters_duckduckgo_internal_links() {
    let html = load_fixture("ddg_html_pagina_1.html");
    let results = extract_results(&html);
    for r in &results {
        assert!(
            !r.url.as_str().contains("html.duckduckgo.com")
                && !r.url.as_str().contains("lite.duckduckgo.com")
                && !r.url.as_str().contains("duckduckgo.com/y.js"),
            "result contains an internal DDG URL: {}",
            r.url
        );
    }
}

#[test]
fn strategies_extraction_combines_and_works_on_real_html() {
    let html = load_fixture("ddg_html_pagina_1.html");
    let simple_results = extract_results(&html);
    let strategy_results = extract_results_with_strategies(&html);

    // In valid HTML, Strategy 1 wins — the fallback function must return
    // the same count.
    assert_eq!(
        simple_results.len(),
        strategy_results.len(),
        "Strategy 1 returned results — Strategy 2 must not overwrite them"
    );
}
