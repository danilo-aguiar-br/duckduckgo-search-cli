# Benchmarks — `duckduckgo-search-cli`

Latency regression baselines (historically v0.7.10; re-run after hot-path changes).
Methodology below remains valid for the current line **v1.0.1** (pure-CPU
helpers; product wall-clock is still Chrome + RTT).

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
   has **no** ns-level budget: Chrome cold start + RTT dominate (`tempo_execucao_ms` is a
   single wall-clock sample per one-shot invocation — not a multi-sample histogram).

### Reading percentiles from Criterion (local)

```bash
cargo bench --bench pre_flight_latency
# Median ≈ P50 (preferred); mean is secondary:
jq '{median: .median.point_estimate, mean: .mean.point_estimate,
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

| Scenario | Median (≈P50) | Notes |
|---|---|---|
| `baseline_1kb_lorem` | ~150 ns | 1KB lorem, no marker → `InterstitialKind::None` |
| `baseline_with_cloudflare_marker` | ~80 ns | 1KB + `cf-challenge` → early-return |
| `ghost_block_short_2kb` | ~250 ns | 2KB lorem → ghost-block branch + sentinel |
| `legit_short_with_selector` | ~200 ns | 500B with `result__a` → BC-safe signal |
| `has_result_page_signal_false` | ~150 ns | 2KB without selectors → `false` |
| `has_result_page_signal_true` | ~80 ns | 500B with selector → early-return |

**Tabela atualizada por `cargo bench --bench pre_flight_latency`.**

### Interpretação

- O detector é **O(n)** onde `n` é o tamanho do body. Operações dominantes:
  1. Loop sobre `CLOUDFLARE_MARKERS` (19 strings).
  2. Loop sobre `DDG_MARKERS` (5 strings).
  3. Para ghost-block: chamada a `has_result_page_signal` (11 selectors).
- O overhead do pre-flight gate (`+200-300ms` documentado no ADR-0003) **NÃO** vem deste detector — vem do **probe-deep request HTTP** extra que seria adicionado se P5 (probe-deep scheduler) for implementado.
- v0.7.10 introduz `detectar_interstitial_com_match` que retorna tupla `(marker, kind)`. O overhead sobre `detectar_interstitial` é zero — ambas compartilham o mesmo loop, apenas a função nova também retorna o marker que foi encontrado.

### Regressão (local — **sem CI/GitHub Actions**)

Este repositório **proíbe** pipelines CI (`NO_CI.md`). Rode os benches
**localmente** antes de release ou de merge que toque hot paths:

```bash
cargo bench --bench pre_flight_latency -- --save-baseline baseline-v0.7.10
cargo bench --bench pre_flight_latency -- --baseline baseline-v0.7.10
# Opcional: demais benches do crate
cargo bench --bench extraction_bench
cargo bench --bench decompress_bench
cargo bench --bench zero_cause_bench
cargo bench --bench tracing_overhead_bench
# RSS de smoke (binário release):
/usr/bin/time -v ./target/release/duckduckgo-search-cli --version
```

Criterion reporta regressões > 5% automaticamente no comparativo de baseline.

## Extraction (`benches/extraction_bench.rs`)

Pré-existente — não modificado por v0.7.10. Mede `extraction::extract_results` em fixtures HTML reais.