// SPDX-License-Identifier: MIT OR Apache-2.0
//! v0.7.10 P19 — DDG class watcher for template rotation detection.
//!
//! DDG periodically rotates the CSS classes used in result pages
//! (`result__a`, `nrn-react-div`, etc.). When this happens,
//! `has_result_page_signal` may start returning false positives,
//! causing legitimate short results pages to be classified as
//! ghost-blocks.
//!
//! This module provides a watchdog that fetches a known search query
//! and extracts every `class="..."` attribute that appears in result
//! containers, surfacing new classes that DDG has introduced.
//!
//! The watchdog is **diagnostic-only** — it does NOT auto-update
//! `RESULT_PAGE_SELECTORS`. Operators are expected to review the
//! surfaced classes, validate they are real result-page signals (and
//! not noise from analytics/UI widgets), and bump the selectors list
//! in `probe_deep.rs` in a follow-up release.
//!
//! ## Status (v0.8.0)
//!
//! This module is **exposed but not yet wired to a CLI subcommand**.
//! The diagnostic is reachable as a library function via
//! `duckduckgo_search_cli::ddg_class_watch::watch_ddg_classes(html)` and
//! is covered by `tests/integration_wiremock.rs::ddg_class_watch_detects_new_template_class`.
//!
//! A future `--watch-ddg-classes` CLI subcommand (or `ddg-class-watch` binary
//! under `examples/`) is planned but NOT scheduled for v0.8.0 to keep the
//! release focused on the GAP-AUD-003 classifier. Tracked in `INVERSIONS.md` §3
//! and ADR-0004 (zero-cause classification).
//!
//! Until the CLI subcommand ships, this module serves as a **library primitive**
//! for downstream consumers (other Rust binaries, integration tests, ad-hoc
//! scripts via `cargo run --example ddg_class_watch_demo` once added).

use duckduckgo_search_cli::probe_deep::RESULT_PAGE_SELECTORS;
use std::collections::BTreeSet;

/// Result of a single watchdog run.
#[derive(Debug, Clone)]
pub struct DdgClassWatch {
    /// Known selectors from `RESULT_PAGE_SELECTORS` at watchdog time.
    pub known: BTreeSet<String>,
    /// Candidate classes that appeared in `<div class="...">` tags
    /// inside the live DDG response. May include false positives
    /// (analytics, ads, layout). Operators should validate each.
    pub candidates: BTreeSet<String>,
    /// Subset of `candidates` that are NOT in `known` — i.e., new
    /// classes DDG introduced since the last bump. This is the
    /// actionable delta.
    pub new_classes: BTreeSet<String>,
}

impl DdgClassWatch {
    /// Returns `true` when the watchdog surfaced at least one new
    /// class. Useful in integration tests.
    pub fn has_new_classes(&self) -> bool {
        !self.new_classes.is_empty()
    }
}

/// Extracts every `class="..."` attribute from the HTML body. Naive
/// regex — does not parse DOM — but sufficient for the watchdog's
/// diagnostic purpose. Filters empty classes and deduplicates.
pub fn extract_class_attributes(html: &str) -> BTreeSet<String> {
    let mut classes = BTreeSet::new();
    let bytes = html.as_bytes();
    let needle = b"class=\"";
    let mut i = 0;
    while let Some(pos) = find_subsequence(&bytes[i..], needle) {
        let start = i + pos + needle.len();
        if let Some(end_rel) = find_subsequence(&bytes[start..], b"\"") {
            let class_attr = &bytes[start..start + end_rel];
            if let Ok(s) = std::str::from_utf8(class_attr) {
                // Split by whitespace (HTML allows multiple classes).
                for token in s.split_whitespace() {
                    if !token.is_empty() {
                        classes.insert(token.to_string());
                    }
                }
            }
            i = start + end_rel + 1;
        } else {
            break;
        }
    }
    classes
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Run the watchdog against the given HTML body. Returns the watch
/// report. In production, the body would come from a `curl https://html.duckduckgo.com/?q=...`
/// response; in tests, fixtures are passed directly.
pub fn watch_ddg_classes(html_body: &str) -> DdgClassWatch {
    let known: BTreeSet<String> = RESULT_PAGE_SELECTORS
        .iter()
        .map(|s| s.to_string())
        .collect();
    let candidates = extract_class_attributes(html_body);
    let new_classes: BTreeSet<String> = candidates.difference(&known).cloned().collect();
    DdgClassWatch {
        known,
        candidates,
        new_classes,
    }
}

fn main() {
    // Sample HTML simulating a DDG result page with both known and new classes.
    let html = r#"<html><body>
        <div class="result__a">known selector</div>
        <div class="result__a nrn-react-div active">multi-class</div>
        <div class="result-2026-card">new template introduced by DDG</div>
        <div class="ad-banner">noise from analytics</div>
    </body></html>"#;

    let report = watch_ddg_classes(html);
    println!("=== DDG Class Watch Report ===");
    println!("Known selectors: {}", report.known.len());
    println!("Candidate classes: {:?}", report.candidates);
    println!("New classes (actionable delta): {:?}", report.new_classes);
    println!("Has new classes: {}", report.has_new_classes());

    if report.has_new_classes() {
        println!("\nACTION REQUIRED: review new classes and update RESULT_PAGE_SELECTORS in src/probe_deep.rs");
        std::process::exit(1);
    } else {
        println!("\nNo new classes — DDG template is stable.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_class_attributes_parses_single_class() {
        let html = r#"<div class="result__a">x</div>"#;
        let classes = extract_class_attributes(html);
        assert!(classes.contains("result__a"));
    }

    #[test]
    fn extract_class_attributes_parses_multiple_classes() {
        let html = r#"<div class="result__a nrn-react-div active">x</div>"#;
        let classes = extract_class_attributes(html);
        assert!(classes.contains("result__a"));
        assert!(classes.contains("nrn-react-div"));
        assert!(classes.contains("active"));
    }

    #[test]
    fn extract_class_attributes_dedupes() {
        let html = r#"<div class="foo"><span class="foo"></span></div>"#;
        let classes = extract_class_attributes(html);
        let count = classes.iter().filter(|c| *c == "foo").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn extract_class_attributes_skips_empty() {
        let html = r#"<div class=""><span class="   "></span></div>"#;
        let classes = extract_class_attributes(html);
        assert!(classes.is_empty());
    }

    #[test]
    fn watch_ddg_classes_returns_empty_when_no_new() {
        let html = r#"<div class="result__a">known</div>"#;
        let report = watch_ddg_classes(html);
        assert!(report.candidates.contains("result__a"));
        assert!(!report.has_new_classes());
    }

    #[test]
    fn watch_ddg_classes_detects_new_class() {
        // Simulate DDG introducing `result-2026-card` as a new template.
        let html = r#"
            <div class="result__a">legacy</div>
            <div class="result-2026-card">new template</div>
        "#;
        let report = watch_ddg_classes(html);
        assert!(report.has_new_classes());
        assert!(report.new_classes.contains("result-2026-card"));
    }
}
