# Inventário de gaps — duckduckgo-search-cli

## Metadados

| Campo | Valor |
|-------|--------|
| Versão | **0.9.9** |
| Data | 2026-07-14 |
| Caminho | `gaps.md` (raiz; excluído crates.io) + `docs/gaps.md` |
| ADR | `docs/decisions/0019-e2e-gaps-news-timeout-probe-meta-v0-9-9.md` |

## Gaps e2e — todos RESOLVIDOS em v0.9.9

| ID | Status |
|----|--------|
| GAP-WS-NEWS-LIVE-001 | **RESOLVIDO** v0.9.9 — denylist promo; wait selectors `news-vertical` (sem early `article`); empty se só chrome |
| GAP-WS-NEWS-FETCH-WASTE-001 | **RESOLVIDO** v0.9.9 — skip promo no fetch |
| GAP-WS-TIMEOUT-DEFAULT-001 | **RESOLVIDO** v0.9.9 — default global 180s |
| GAP-WS-EXIT4-JSON-001 | **RESOLVIDO** v0.9.9 — JSON no exit 4 |
| GAP-WS-PROBE-403-001 | **RESOLVIDO** v0.9.9 — calibração + healthy |
| GAP-WS-PREFLIGHT-META-001 | **RESOLVIDO** v0.9.9 — `pre_flight_executado` |
| GAP-WS-META-TIMING-001 | **RESOLVIDO** v0.9.9 — wall clock + fetch |
| GAP-WS-META-NO-CHROME-001 | **RESOLVIDO** v0.9.9 — meta chrome limpa |
| GAP-WS-ERR-CHROME-PATH-001 | **RESOLVIDO** v0.9.9 — PathError `{message}` |
| GAP-WS-QUIET-CONFIG-001 | **RESOLVIDO** v0.9.9 — quiet = off |
| GAP-WS-STREAM-NOOP-001 | **RESOLVIDO** v0.9.9 — help + meta honestos |
| GAP-WS-NEWS-FIXTURE-001 | **RESOLVIDO** v0.9.9 — testes promo-only |
| GAP-WS-DOCS-TIMEOUT-001 | **RESOLVIDO** v0.9.9 — default = docs 180 |
| GAP-WS-L04-REGRESSION-001 | **RESOLVIDO** v0.9.9 — residual L-04 fechado |
| GAP-WS-NEWS-FANOUT-001 | **RESOLVIDO** v0.9.9 — parallel/deep |
| GAP-WS-SELECTORS-XDG-001 | **RESOLVIDO** v0.9.9 — empty se só promo |
| GAP-WS-PROBE-SCHEMA-001 | **RESOLVIDO** v0.9.9 — status string |
| GAP-WS-STREAM-MULTI-001 | **RESOLVIDO** v0.9.9 — help multi NDJSON |

## Histórico resolvido (releases anteriores)

| ID | Versão |
|----|--------|
| GAP-WS-LIFECYCLE-001 | 0.9.6 / 0.9.7 |
| GAP-WS-113 | 0.9.4 |
| GAP-WS-AGENT-READY-001 | 0.9.8 (L-04 residual → 0.9.9) |

## Mandatos

- Produção Chrome-only CDP; **one-shot** por invocação; **sem telemetria**.
- Disco: atomwrite. Metadados agent ≠ telemetria.

## Evidência e2e v0.9.9 (host Fedora)

| Caso | Resultado |
|------|-----------|
| news OpenAI | `quantidade_noticias: 0`, `news_filtradas_promo: 5` |
| `--probe` | `healthy: true`, `status: "ok"` |
| NO_CHROME | `tentou_chrome: false`, path/canal null |
| chrome-path inválido | sem prefixo “invalid output path” |
| `--global-timeout 2` | JSON `erro: "timeout"` |

## Política

Status só muda com release + ADR. PT-BR com acentuação.
