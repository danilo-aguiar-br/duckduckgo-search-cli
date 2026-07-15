# JSON Schemas Index

> Bilingual document (EN + [Português Brasileiro](#português-brasileiro) sections below).

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

> **Status (v1.0.0)**: Present on disk and hand-maintained in sync with `src/types.rs` under Chrome-only production (**GAP-WS-113**) and agent-ready defaults (**GAP-WS-AGENT-READY-001 / ADR-0018**). **No JSON schema break for lifecycle** in 1.0.0 — schemas are unchanged; the process+disk one-shot contract (**GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020**, extending process-only GAP-WS-LIFECYCLE-001 / ADR-0017) is **operational only** (profile prefix `ddg-chrome-*`, cooperative `force_reap` / `ExitReapGuard` / `remove_dir_all`, next-run `sweep_orphan_profiles` of owned `ddg-chrome-*` only; **hard policy:** never bulk-rm foreign `.tmp*` or `org.chromium.Chromium.*`). Schemas still do **not** encode profile path or disk ownership — document honesty: lifecycle is a process+disk runtime contract, not a schema-breaking change. Additive agent-ready fields from 0.9.8 remain current defaults: `metadados.chrome_path_resolvido`, `metadados.chrome_canal`, honest `usou_chrome`, news/web `conteudo*` when content fetch is on (default ON; opt-out `--no-fetch-content`; FETCH_CAP=10 for web+news). Default vertical is **`all`**. **Multi-search** (`multi-search-output.schema.json`): each `buscas[]` item `$ref`s `search-output.schema.json`, so chrome agent metadata is inherited per query via `metadados` (not telemetry). **Error path**: many failures emit a full `SearchOutput` via `failure_output`/`error_output` (full chrome contract); the thin `error-response.schema.json` may still carry best-effort `metadados.usou_chrome` / `chrome_path_resolvido` / `chrome_canal` on residual thin error envelopes. Schemas cover: `search-output`, `search-metadata`, `search-result`, `news-result`, `deep-research-output`, `probe-output`, `probe-deep-output`, `multi-search-output`, `config`, `error-response`. `ndjson-event` is a **placeholder** for the unimplemented `--stream` feature. Rust types remain the source of truth.


## News Vertical Fields (v0.8.9, GAP-WS-104; defaults v0.9.8)

The `--vertical <web|news|all>` flag (**default `all` since v0.9.8**; historical default was `web`) emits news fields when vertical is `news` or `all`. Multi-query batches accept `--vertical news|all` since GAP-WS-105; each `buscas[]` item of `multi-search-output.schema.json` may carry them:

- Root `noticias[]` — array of `news-result.schema.json` objects. Guaranteed
  per item: `posicao` (integer, 1-indexed), `titulo` (string), `url` (string).
  Optional per item: `fonte`, `data_relativa`, `thumbnail`, and (v0.9.8) `conteudo` /
  `tamanho_conteudo` / `metodo_extracao_conteudo` when content fetch is on.
- Root `quantidade_noticias` — integer count after dedupe/cap. The process
  exit code sums `quantidade_resultados + quantidade_noticias`.
- `metadados.vertical_usada` — `"news"` or `"all"`.
- `metadados.chrome_path_resolvido` / `metadados.chrome_canal` — agent metadata
  (v0.9.8; **not** telemetry).

With explicit `--vertical web` (and optionally `--no-fetch-content`) news fields
are ABSENT, preserving a thin web-only envelope. Validators must treat news and
content fields as optional (`required` lists do not force them).

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
(**GAP-WS-113** / ADR-0016) and agent-ready defaults (**GAP-WS-AGENT-READY-001 /
ADR-0018**): default `--vertical all`, content fetch ON (opt-out
`--no-fetch-content`, FETCH_CAP=10 for web+news). Current release status is
**v1.0.0**: lifecycle is process+disk (**GAP-WS-TMP-PROFILE-ORPHAN-001 /
ADR-0020**); that contract is operational only (`ddg-chrome-*`, `force_reap` /
`ExitReapGuard`, never bulk-rm foreign `.tmp*` / `org.chromium.Chromium.*`;
schemas do not encode profile path; no JSON schema break vs 0.9.x agent-ready
fields).

**Multi-search inheritance**: `multi-search-output.schema.json` `buscas[]` items
`$ref` `search-output.schema.json`, so each query envelope inherits
`metadados.chrome_path_resolvido`, `metadados.chrome_canal`, and honest
`usou_chrome` from `search-metadata.schema.json` (agent metadata, **not**
telemetry).

**Failure envelopes**: many failures emit a full `SearchOutput` via
`failure_output`/`error_output` (complete chrome agent contract on
`metadados`). The thin `error-response.schema.json` may also expose
best-effort `metadados.usou_chrome` / `chrome_path_resolvido` / `chrome_canal`
on residual thin error paths.

## Português Brasileiro

Este arquivo documenta o inventário de schemas JSON para `duckduckgo-search-cli`.
Os schemas são contratos legíveis por máquina que permitem a agentes, IDEs e
clientes type-safe validar a saída da CLI sem executar o binário.

### Status (v1.0.0)

Presentes em disco e mantidos à mão em sincronia com `src/types.rs` sob produção
Chrome-only (**GAP-WS-113**) e defaults agent-ready (**GAP-WS-AGENT-READY-001 /
ADR-0018**). **Sem quebra de schema JSON no lifecycle** na 1.0.0 — os schemas
permanecem inalterados; o contrato one-shot processo+disco
(**GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020**, estendendo o one-shot de processo
GAP-WS-LIFECYCLE-001 / ADR-0017) é **apenas operacional** (prefixo de perfil
`ddg-chrome-*`, `force_reap` / `ExitReapGuard` / `remove_dir_all` cooperativo,
`sweep_orphan_profiles` da próxima run só em `ddg-chrome-*` de propriedade;
**política rígida:** nunca bulk-rm de `.tmp*` estrangeiro nem
`org.chromium.Chromium.*`). Os schemas **não** codificam path de perfil nem
posse em disco — honestidade documental: o lifecycle é contrato de runtime
processo+disco, não mudança que quebra schema. Campos aditivos agent-ready da
0.9.8 continuam como defaults vigentes: `metadados.chrome_path_resolvido`,
`metadados.chrome_canal`, `usou_chrome` honesto (incluindo deep-research e
envelopes de falha); `conteudo` em web/news com fetch de conteúdo (**LIGADO por
padrão**; opt-out `--no-fetch-content`; FETCH_CAP=10). Vertical padrão da search
é **`all`**.

**Multi-search**: cada item de `buscas[]` em `multi-search-output.schema.json`
usa `$ref` de `search-output.schema.json`, herdando metadados chrome de agente
por query via `metadados` (não é telemetria).

**Falhas**: muitas falhas emitem `SearchOutput` completo via
`failure_output`/`error_output` (contrato chrome completo em `metadados`); o
schema fino `error-response.schema.json` pode ainda carregar
`metadados.usou_chrome` / `chrome_path_resolvido` / `chrome_canal` best-effort
no caminho residual de erro fino.

Schemas cobertos: `search-output`, `search-metadata`, `search-result`,
`news-result`, `deep-research-output`, `probe-output`, `probe-deep-output`,
`multi-search-output`, `config`, `error-response`. O schema `ndjson-event` é
apenas um **placeholder** para a feature `--stream` (ainda **não implementada**).
As definições de tipo Rust permanecem a fonte da verdade.

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
