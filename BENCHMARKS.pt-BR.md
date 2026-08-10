# Benchmarks — `duckduckgo-search-cli`

Leia em [English](BENCHMARKS.md).

Baselines de regressão de latência (historicamente v0.7.10; re-execute após mudar hot
path). A metodologia abaixo continua válida para a linha atual **v1.0.5** (helpers
puramente de CPU; o wall-clock do produto ainda é Chrome + RTT).

## Metodologia (regras de eficiência / performance / **latência**)

1. **Meça antes de mudar** — capture primeiro o baseline do Criterion ou o `/usr/bin/time -v`.
2. **Perfile antes de micro-otimizar** — `cargo flamegraph` / `samply` / `perf` sobre uma
   execução representativa de SERP no Chrome; não otimize startup frio nem caminhos guiados
   só por intuição.
3. **Somente release** — os benches usam `[profile.bench]` (herda o release com fat LTO).
   Nunca compare debug contra release.
4. **Uma mudança, re-meça** — aceite apenas ganhos acima do ruído do Criterion (~5% sinalizado
   automaticamente).
5. **Sem thresholds de CI** — este repositório proíbe GitHub Actions (`NO_CI.pt-BR.md`). Os
   baselines vivem aqui; rode localmente antes do release ao tocar extraction, decompress,
   probe_deep, parallel, content_fetch ou aggregation.
6. **Restrição do produto** — o tempo de parede é dominado por Chrome + RTT de rede, não pelos
   loops de parse em Rust. Prefira limites algorítmicos e concorrência de I/O a SIMD/ahash sem
   evidência.
7. **Latência de cauda, não médias** — a métrica primária pura de CPU é a **mediana (≈P50)** do
   `estimates.json` do Criterion, e depois o desvio-padrão / outliers. **Não** trate a média
   impressa pelo Criterion como única métrica de sucesso. Config compartilhada:
   `benches/latency_config.rs` (`sample_size=200`, `noise_threshold=0.05`, warm-up 500ms,
   medição 3s).
8. **Orçamentos de latência (CPU pura, release LTO local x86_64)** — helpers de detector /
   classifier miram **P99 ≪ 5 µs**; decode gzip de ~14 KB mira **P99 ≪ 200 µs**. A busca
   end-to-end **não** tem orçamento em nanossegundos: cold start do Chrome + RTT dominam
   (`tempo_execucao_ms` é uma única amostra de wall-clock por invocação one-shot — não um
   histograma multi-amostra).

### Lendo percentis do Criterion (local)

```bash
cargo bench --bench pre_flight_latency
# Median ≈ P50 (preferred); mean is secondary:
jq '{median: .median.point_estimate, mean: .mean.point_estimate,
     std_dev: .std_dev.point_estimate}' \
  target/criterion/baseline_1kb_lorem/new/estimates.json
```

Perfilamento local opcional (não exigido a cada commit):

```bash
# After: cargo install flamegraph  (or use samply)
cargo flamegraph --bin duckduckgo-search-cli -- --help
/usr/bin/time -v ./target/release/duckduckgo-search-cli --version
```

## Detector de pre-flight (`benches/pre_flight_latency.rs`)

Cenários puros (sem I/O) que medem o custo de CPU do detector de interstitial.

| Cenário | Mediana (≈P50) | Notas |
|---|---|---|
| `baseline_1kb_lorem` | ~150 ns | 1KB de lorem, sem marker → `InterstitialKind::None` |
| `baseline_with_cloudflare_marker` | ~80 ns | 1KB + `cf-challenge` → early-return |
| `ghost_block_short_2kb` | ~250 ns | 2KB de lorem → branch de ghost-block + sentinela |
| `legit_short_with_selector` | ~200 ns | 500B com `result__a` → sinal BC-safe |
| `has_result_page_signal_false` | ~150 ns | 2KB sem selectors → `false` |
| `has_result_page_signal_true` | ~80 ns | 500B com selector → early-return |

**Tabela atualizada por `cargo bench --bench pre_flight_latency`.**

### Interpretação

- O detector é **O(n)** onde `n` é o tamanho do body. Operações dominantes:
  1. Loop sobre `CLOUDFLARE_MARKERS` (18 strings).
  2. Loop sobre `DDG_MARKERS` (4 strings).
  3. Para ghost-block: chamada a `has_result_page_signal` (15 selectors em `RESULT_PAGE_SELECTORS`).
- O overhead do pre-flight gate (`+200-300ms` documentado no ADR-0003) **NÃO** vem deste detector — vem do **probe-deep request HTTP** extra que seria adicionado se P5 (probe-deep scheduler) for implementado.
- v0.7.10 introduz `detect_interstitial_with_match` que retorna tupla `(marker, kind)`. O overhead sobre `detect_interstitial` é zero — ambas compartilham o mesmo loop, apenas a função nova também retorna o marker que foi encontrado.

### Regressão (local — **sem CI/GitHub Actions**)

Este repositório **proíbe** pipelines CI (`NO_CI.pt-BR.md`). Rode os benches
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

Pré-existente — não modificado pela v0.7.10. Mede `extraction::extract_results` em fixtures HTML reais.
