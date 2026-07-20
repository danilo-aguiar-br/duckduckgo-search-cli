//! Extraction latency microbench (pure CPU). Prefer median over mean —
//! see `benches/latency_config.rs` and `BENCHMARKS.md`.

#[path = "latency_config.rs"]
mod latency_config;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use duckduckgo_search_cli::extraction;

const SAMPLE_HTML: &str = include_str!("../tests/fixtures/ddg_html_pagina_1.html");
const SAMPLE_LITE: &str = include_str!("../tests/fixtures/ddg_lite_pagina_1.html");

fn bench_extract_results(c: &mut Criterion) {
    c.bench_function("extract_results", |b| {
        b.iter(|| extraction::extract_results(black_box(SAMPLE_HTML)))
    });
}

fn bench_extract_results_with_strategies(c: &mut Criterion) {
    c.bench_function("extract_results_with_strategies", |b| {
        b.iter(|| extraction::extract_results_with_strategies(black_box(SAMPLE_HTML)))
    });
}

fn bench_extract_results_lite(c: &mut Criterion) {
    c.bench_function("extract_results_lite", |b| {
        b.iter(|| extraction::extract_results_lite(black_box(SAMPLE_LITE)))
    });
}

criterion_group!(
    name = benches;
    config = latency_config::latency_criterion();
    targets = bench_extract_results,
              bench_extract_results_with_strategies,
              bench_extract_results_lite,
);
criterion_main!(benches);
