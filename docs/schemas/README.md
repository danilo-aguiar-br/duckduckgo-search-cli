# JSON Schemas Index

This directory contains machine-readable JSON schemas for the public output
contracts of `duckduckgo-search-cli`. Each schema is versioned and synchronized
with the Rust type definitions in `src/types.rs`.

## Available Schemas

The following output contracts are exposed by the CLI:

| Schema | Source Type | Output |
|--------|-------------|--------|
| `search-output.schema.json` | `SearchOutput` | Single-query JSON root `{ query, resultados, metadados }` |
| `multi-search-output.schema.json` | `MultiSearchOutput` | Multi-query JSON root `{ quantidade_queries, buscas[] }` |
| `search-result.schema.json` | `SearchResult` | Individual result row |
| `news-result.schema.json` (v0.8.9+) | `NewsResult` | Individual news-vertical result row (`--vertical news\|all`) |
| `search-metadata.schema.json` | `SearchMetadata` | Latency, identity, cascade level |
| `probe-output.schema.json` | `ProbeReport` | `--probe` JSON response |
| `probe-deep-output.schema.json` (v0.7.3+) | `ProbeDeepReport` | `--probe-deep` JSON response with `status`, `cascata_motivo`, `sugestao_mitigacao`, `http_status`, `latency_ms`, `endpoint` |
| `deep-research-output.schema.json` (v0.8.9+) | `DeepResearchOutput` | `deep-research` JSON root `{ tipo, query, metadados, resultados[], noticias[], quantidade_noticias, sintese? }` |
| `config.schema.json` | (config TOML) | Configuration file / `init-config` shape |
| `error-response.schema.json` | `CliError` | Structured error envelope (stderr / exit 2 path) |
| `ndjson-event.schema.json` | (planned / unimplemented) | Placeholder for `--stream` NDJSON events — **not implemented** |

> **Status (v0.9.6)**: Present on disk and hand-maintained in sync with `src/types.rs` under Chrome-only production (**GAP-WS-113**). **No schema break** in 0.9.6 — one-shot browser lifecycle (GAP-WS-LIFECYCLE-001 / ADR-0017) is a **process-only** change. Schemas still cover: `search-output`, `search-metadata`, `search-result`, `news-result`, `deep-research-output`, `probe-output`, `probe-deep-output`, `multi-search-output`, `config`, `error-response`. `ndjson-event` is a **placeholder** for the unimplemented `--stream` feature. The Rust type definitions remain the source of truth. Automated generation via `schemars` is planned for a future version.


## News Vertical Fields (v0.8.9, GAP-WS-104)

The `--vertical <web|news|all>` flag (default `web`) adds three OPTIONAL fields
to the search envelope, emitted ONLY when `--vertical news|all`. Since
GAP-WS-105 (same release) multi-query batches accept `--vertical news|all`,
so each `buscas[]` item of `multi-search-output.schema.json` may carry them:

- Root `noticias[]` — array of `news-result.schema.json` objects. Guaranteed
  per item: `posicao` (integer, 1-indexed), `titulo` (string), `url` (string).
  Optional per item: `fonte`, `data_relativa`, `thumbnail` (may be absent
  depending on which selector cascade strategy matched).
- Root `quantidade_noticias` — integer count after dedupe/cap. The process
  exit code sums `quantidade_resultados + quantidade_noticias`.
- `metadados.vertical_usada` — `"news"` or `"all"`.

In the default `web` mode these fields are ABSENT (Rust
`#[serde(skip_serializing_if = "Option::is_none")]`), keeping the JSON
contract byte-identical to v0.8.8. For this reason NONE of the new fields is
listed in `required` — validators must treat them as optional.

The `causa_zero` enum (root and `metadados`) gains the variant
`vertical-sem-resultados`: legitimate zero from the news vertical (rendered
SERP without articles), exit 5 — an anti-bot interstitial in the news body
still classifies as `anti-bot`.


## Deep-Research News Fields (v0.8.9, GAP-WS-105)

`deep-research` scans the news vertical by DEFAULT (opt-out `--no-news`) and
its envelope (`deep-research-output.schema.json`) gains:

- Root `noticias[]` — aggregated news items. Guaranteed per item: `posicao`,
  `titulo`, `url`, `score` (news-only RRF, NOT comparable with
  `resultados[].score`), `ocorrencias` (number of sub-queries the item
  appeared in). Optional: `fonte`, `data_relativa` (verbatim string),
  `thumbnail`.
- Root `quantidade_noticias` — ALWAYS present (0 with `--no-news` or zero news).
- `metadados.total_noticias_unicas` — ALWAYS present.
- `metadados.sub_queries[].quantidade_noticias` and
  `metadados.sub_queries[].news_indisponivel` — OPTIONAL (omitted with
  `--no-news`; `news_indisponivel: true` when the news scan failed mid-flight
  as a structured field — not a production HTTP transport degrade).


## Generation Strategy

When schemas are added, the plan is:

1. Add `schemars = "0.8"` as a dev-dependency
2. Derive `JsonSchema` on each public type in `src/types.rs`
3. Generate schemas via `cargo run --bin dump-schemas -- output/schemas/`
4. Add a CI step that fails if any `*.schema.json` is out of sync with the Rust types
5. Validate every example in `docs/COOKBOOK.md` against the schemas on every push


## Schema Coverage Checklist

Files on disk vs. still missing:

- [x] `search-output.schema.json`
- [x] `multi-search-output.schema.json`
- [x] `search-result.schema.json`
- [x] `search-metadata.schema.json`
- [x] `probe-output.schema.json`
- [x] `probe-deep-output.schema.json` (v0.7.3+ — for `--probe-deep` flag)
- [x] `news-result.schema.json` (v0.8.9+)
- [x] `deep-research-output.schema.json` (v0.7.0+ — `deep-research` subcommand; v0.8.7 adds `.query` field and `.resultados[].titulo` rename; v0.8.9 GAP-WS-105 adds `noticias[]`, `quantidade_noticias`, `metadados.total_noticias_unicas` and per-sub-query news fields)
- [x] `config.schema.json` (for `init-config` / config TOML shape)
- [x] `error-response.schema.json` (structured error envelope)
- [x] `ndjson-event.schema.json` (placeholder only — `--stream` **unimplemented**)
- [ ] `init-config-output.schema.json` (dedicated `init-config` command output — **absent**)


## Validation

Once generated, schemas can be validated with any JSON Schema validator
against real CLI output:

```bash
# Capture real output
timeout 30 duckduckgo-search-cli -q -f json "rust" > /tmp/out.json

# Validate against schema
jaq . /tmp/out.json | jsonschema -i /dev/stdin schemas/search-output.schema.json

# Validate probe-deep output (v0.7.3+)
timeout 15 duckduckgo-search-cli --probe-deep -q -f json > /tmp/probe.json
jaq . /tmp/probe.json | jsonschema -i /dev/stdin schemas/probe-deep-output.schema.json
```


## English

This file documents the JSON schema inventory for `duckduckgo-search-cli`.
The schemas are machine-readable contracts that allow agents, IDEs, and
type-safe clients to validate CLI output without running the binary.
Production output contracts assume Chrome-only network transport
(**GAP-WS-113** / ADR-0016).

## Português Brasileiro

Este arquivo documenta o inventário de schemas JSON para `duckduckgo-search-cli`.
Os schemas são contratos legíveis por máquina que permitem a agentes, IDEs e
clientes type-safe validar a saída da CLI sem executar o binário.

### Status (v0.9.6)

Presentes em disco e mantidos à mão em sincronia com `src/types.rs` sob produção
Chrome-only (**GAP-WS-113**). **Sem quebra de schema** na 0.9.6 — o lifecycle
one-shot do browser (GAP-WS-LIFECYCLE-001 / ADR-0017) é mudança **somente de
processos**. Schemas cobertos: `search-output`, `search-metadata`, `search-result`,
`news-result`, `deep-research-output`, `probe-output`, `probe-deep-output`,
`multi-search-output`, `config`, `error-response`. O schema `ndjson-event` é
apenas um **placeholder** para a feature `--stream` (ainda **não implementada**).
As definições de tipo Rust permanecem a fonte da verdade. Geração automatizada
via `schemars` está planejada para uma versão futura.

### Checklist de cobertura

- [x] `search-output.schema.json`
- [x] `multi-search-output.schema.json`
- [x] `search-result.schema.json`
- [x] `search-metadata.schema.json`
- [x] `probe-output.schema.json`
- [x] `probe-deep-output.schema.json`
- [x] `news-result.schema.json`
- [x] `deep-research-output.schema.json`
- [x] `config.schema.json`
- [x] `error-response.schema.json`
- [x] `ndjson-event.schema.json` (placeholder — `--stream` não implementado)
- [ ] `init-config-output.schema.json` (ausente)
