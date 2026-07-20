// SPDX-License-Identifier: MIT OR Apache-2.0
//! v0.7.10 P14 — Pre-flight latency benchmark.
//!
//! Measures three scenarios to validate the +200-300ms cost documented
//! in the ADR-0003 decision table:
//!
//! 1. **baseline**: detection on a 1.5KB body without result-page
//!    selector (typical captcha response). No I/O — pure classification.
//! 2. **`ghost_block_short`**: detection on a 2KB lorem-ipsum body
//!    (size-heuristic path triggered). Measures the ghost-block branch.
//! 3. **`legit_short_with_selector`**: detection on a 500B body with
//!    `result__a` (false-positive guard). Measures the BC-safe path.
//!
//! All benches are pure (no HTTP) so the reported numbers represent
//! CPU cost only, isolating the detector overhead from network jitter.
//!
//! Run with:
//!   `cargo bench --bench pre_flight_latency`
//!
//! Expected (baseline on Linux `x86_64`, single core, release — **median ≈ P50**):
//!   `baseline`: ~150ns
//!   `ghost_block_short`: ~250ns
//!   `legit_short_with_selector`: ~200ns
//! Primary metric: median from `target/criterion/.../estimates.json`, not mean.
//! Latency budget (pure CPU): P99 of detector ≪ 5 µs on modern x86_64 (<< RTT).

#[path = "latency_config.rs"]
mod latency_config;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use duckduckgo_search_cli::probe_deep::{
    detect_interstitial, detect_interstitial_with_match, has_result_page_signal,
};

fn make_lorem_ipsum_kb(kb: usize) -> String {
    let mut html = String::with_capacity(kb * 1024);
    while html.len() < kb * 1024 {
        html.push_str("lorem ipsum dolor sit amet consectetur adipiscing elit ");
    }
    html
}

fn make_legit_short() -> String {
    r#"<html><body>
        <a class="result__a" href="https://example.com">link</a>
        <a class="result__snippet">a short snippet of a real result</a>
        </body></html>"#
        .to_string()
}

fn bench_baseline(c: &mut Criterion) {
    let body = make_lorem_ipsum_kb(1);
    c.bench_function("baseline_1kb_lorem", |b| {
        b.iter(|| detect_interstitial(black_box(&body)))
    });
}

fn bench_baseline_with_marker(c: &mut Criterion) {
    let body = format!("{} cf-challenge", make_lorem_ipsum_kb(1));
    c.bench_function("baseline_with_cloudflare_marker", |b| {
        b.iter(|| detect_interstitial(black_box(&body)))
    });
}

fn bench_ghost_block_short(c: &mut Criterion) {
    let body = make_lorem_ipsum_kb(2);
    c.bench_function("ghost_block_short_2kb", |b| {
        b.iter(|| {
            let (marker, kind) = detect_interstitial_with_match(black_box(&body));
            (marker, kind)
        })
    });
}

fn bench_legit_short_with_selector(c: &mut Criterion) {
    let body = make_legit_short();
    c.bench_function("legit_short_with_selector", |b| {
        b.iter(|| {
            let (marker, kind) = detect_interstitial_with_match(black_box(&body));
            let _ = marker;
            kind
        })
    });
}

fn bench_has_result_page_signal(c: &mut Criterion) {
    let body = make_lorem_ipsum_kb(2);
    c.bench_function("has_result_page_signal_false", |b| {
        b.iter(|| has_result_page_signal(black_box(&body)))
    });

    let body = make_legit_short();
    c.bench_function("has_result_page_signal_true", |b| {
        b.iter(|| has_result_page_signal(black_box(&body)))
    });
}

criterion_group!(
    name = pre_flight;
    config = latency_config::latency_criterion();
    targets = bench_baseline,
              bench_baseline_with_marker,
              bench_ghost_block_short,
              bench_legit_short_with_selector,
              bench_has_result_page_signal,
);
criterion_main!(pre_flight);
