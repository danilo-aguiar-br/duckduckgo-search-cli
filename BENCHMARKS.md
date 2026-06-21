# Benchmarks — `duckduckgo-search-cli`

Resultados de regressão de latência para v0.7.10.

## Pre-flight detector (`benches/pre_flight_latency.rs`)

Cenários puros (sem I/O) medindo CPU cost do detector de intersticial.

| Cenário | Latência média | Notas |
|---|---|---|
| `baseline_1kb_lorem` | ~150 ns | 1KB lorem ipsum, sem marker → `InterstitialKind::None` |
| `baseline_with_cloudflare_marker` | ~80 ns | 1KB + `cf-challenge` → early-return no primeiro marker |
| `ghost_block_short_2kb` | ~250 ns | 2KB lorem → ghost-block branch + sentinel |
| `legit_short_with_selector` | ~200 ns | 500B com `result__a` → BC-safe via `has_result_page_signal` |
| `has_result_page_signal_false` | ~150 ns | 2KB sem selectors → `false` |
| `has_result_page_signal_true` | ~80 ns | 500B com selector → early-return |

**Tabela atualizada por `cargo bench --bench pre_flight_latency`.**

### Interpretação

- O detector é **O(n)** onde `n` é o tamanho do body. Operações dominantes:
  1. Loop sobre `CLOUDFLARE_MARKERS` (19 strings).
  2. Loop sobre `DDG_MARKERS` (5 strings).
  3. Para ghost-block: chamada a `has_result_page_signal` (11 selectors).
- O overhead do pre-flight gate (`+200-300ms` documentado no ADR-0003) **NÃO** vem deste detector — vem do **probe-deep request HTTP** extra que seria adicionado se P5 (probe-deep scheduler) for implementado.
- v0.7.10 introduz `detectar_interstitial_com_match` que retorna tupla `(marker, kind)`. O overhead sobre `detectar_interstitial` é zero — ambas compartilham o mesmo loop, apenas a função nova também retorna o marker que foi encontrado.

### Regressão

Para detectar regressão de performance, o CI deve rodar:

```bash
cargo bench --bench pre_flight_latency -- --save-baseline baseline-v0.7.10
cargo bench --bench pre_flight_latency -- --baseline baseline-v0.7.10
```

Criterion reporta regressões > 5% automaticamente.

## Extraction (`benches/extraction_bench.rs`)

Pré-existente — não modificado por v0.7.10. Mede `extraction::extract_results` em fixtures HTML reais.