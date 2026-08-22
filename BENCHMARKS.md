# Benchmarks — `duckduckgo-search-cli`

Read this in [Portuguese](BENCHMARKS.pt-BR.md).

Latency regression baselines (re-run after hot-path changes).
Methodology below remains valid for the current line **v1.0.6** (pure-CPU
helpers; product wall-clock is still Chrome + RTT).

> **Provenance of every number on this page.** All figures were measured on the
> **v0.7.10** binary and have **not** been re-measured since. No measurement
> date and no host, CPU model or architecture were recorded at the time, so the
> host is **unknown** and the numbers are **not** comparable across machines.
> Treat them as the historical shape of the curve, never as an acceptance
> threshold for v1.0.6. Re-run the benches locally and record date, binary
> version and host before quoting any of them as current.

## Methodology (efficiency / performance / **latency** rules)

1. **Measure before changing** — capture Criterion baseline or `/usr/bin/time -v` first.
2. **Profile before micro-opts** — `cargo flamegraph` / `samply` / `perf` on a representative
   Chrome SERP run; do not optimize cold startup or intuition-only paths.
3. **Release only** — benches use `[profile.bench]` (inherits fat LTO release). Never compare
   debug vs release.
4. **One change, re-measure** — accept only gains beyond Criterion noise (~5% auto-flag).
5. **No CI thresholds** — this repo forbids GitHub Actions (`NO_CI.md`). Baselines live here;
   run locally before release when touching extraction, decompress, probe_deep, parallel,
   content_fetch, or aggregation.
6. **Product constraint** — wall time is dominated by Chrome + network RTT, not Rust parse
   loops. Prefer algorithmic caps and I/O concurrency over SIMD/ahash without evidence.
7. **Tail latency, not averages** — primary pure-CPU metric is **median (≈P50)** from
   Criterion `estimates.json`, then inspect std-dev / outliers. Do **not** treat Criterion’s
   printed mean as the sole success metric. Shared config: `benches/latency_config.rs`
   (`sample_size=200`, `noise_threshold=0.05`, warm-up 500ms, measure 3s).
8. **Latency budgets (pure CPU, local x86_64 release LTO)** — detector / classifier helpers
   target **P99 ≪ 5 µs**; gzip decode of ~14 KB target **P99 ≪ 200 µs**. End-to-end search
   has **no** ns-level budget: Chrome cold start + RTT dominate (`execution_time_ms`, the
   EN wire key since v1.0.2, is a
   single wall-clock sample per one-shot invocation — not a multi-sample histogram).

### Reading percentiles from Criterion (local)

```bash
cargo bench --bench pre_flight_latency
# Median ≈ P50 (preferred); mean is secondary:
jaq '{median: .median.point_estimate, mean: .mean.point_estimate,
     std_dev: .std_dev.point_estimate}' \
  target/criterion/baseline_1kb_lorem/new/estimates.json
```

Optional local profile (not required for every commit):

```bash
# After: cargo install flamegraph  (or use samply)
cargo flamegraph --bin duckduckgo-search-cli -- --help
/usr/bin/time -v ./target/release/duckduckgo-search-cli --version
```

## Pre-flight detector (`benches/pre_flight_latency.rs`)

Pure scenarios (no I/O) measuring CPU cost of the interstitial detector.

Measured on the **v0.7.10** binary; measurement date and host are **unrecorded**, and the table has **not** been re-measured on v1.0.6.

| Scenario | Median (≈P50) | Notes |
|---|---|---|
| `baseline_1kb_lorem` | ~150 ns | 1KB lorem, no marker → `InterstitialKind::None` |
| `baseline_with_cloudflare_marker` | ~80 ns | 1KB + `cf-challenge` → early-return |
| `ghost_block_short_2kb` | ~250 ns | 2KB lorem → ghost-block branch + sentinel |
| `legit_short_with_selector` | ~200 ns | 500B with `result__a` → BC-safe signal |
| `has_result_page_signal_false` | ~150 ns | 2KB without selectors → `false` |
| `has_result_page_signal_true` | ~80 ns | 500B with selector → early-return |

**Table refreshed by `cargo bench --bench pre_flight_latency`.**

### Interpretation

- The detector is **O(n)** where `n` is the body size. Dominant operations:
  1. Loop over `CLOUDFLARE_MARKERS` (18 strings).
  2. Loop over `DDG_MARKERS` (4 strings).
  3. For ghost-block: call to `has_result_page_signal` (15 selectors in `RESULT_PAGE_SELECTORS`).
- The pre-flight gate overhead (`+200-300ms` documented in ADR-0003) does **NOT** come from this detector — it comes from the extra **probe-deep HTTP request** that would be added if P5 (probe-deep scheduler) were implemented.
- v0.7.10 introduced `detect_interstitial_with_match` (`src/probe_deep.rs:163`), which returns a `(marker, kind)` tuple. Its overhead over `detect_interstitial` is zero — both share the same loop; the newer function merely also returns the marker that matched.

### Regression (local — **no CI/GitHub Actions**)

This repository **forbids** CI pipelines (`NO_CI.md`). Run the benches
**locally** before a release, or before merging anything that touches hot paths:

```bash
# Save the baseline under the line you are actually on, not the historical one.
cargo bench --bench pre_flight_latency -- --save-baseline baseline-v1.0.6
cargo bench --bench pre_flight_latency -- --baseline baseline-v1.0.6
# Optional: the remaining benches of the crate
cargo bench --bench extraction_bench
cargo bench --bench decompress_bench
cargo bench --bench zero_cause_bench
cargo bench --bench tracing_overhead_bench
# Smoke RSS (release binary):
/usr/bin/time -v ./target/release/duckduckgo-search-cli --version
```

Criterion flags regressions > 5% automatically in the baseline comparison.

## Extraction (`benches/extraction_bench.rs`)

Pre-existing — not modified by v0.7.10. Measures `extraction::extract_results` against real HTML fixtures.