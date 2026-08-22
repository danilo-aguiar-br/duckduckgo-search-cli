# JSON Schemas Index

> v1.0.2 / ADR-0027: stdout wire keys are English (`results`, `title`, `metadata`, …).
> Portuguese names remain deserialize aliases only. Prefer `--fields url,title` and agent ops
> `--sort` / `--dedupe-by` / `--count-only` / `--limit` / `--filter` so agents never need `jq`.


> Bilingual document (EN + [Português Brasileiro](#português-brasileiro) sections below).

This directory contains machine-readable JSON schemas for the public output
contracts of `duckduckgo-search-cli`. Each schema is versioned and synchronized
with the Rust type definitions in `src/types/`.

## Available Schemas

The following output contracts are exposed by the CLI:

| Schema | Source Type | Output |
|--------|-------------|--------|
| `search-output.schema.json` | `SearchOutput` | Single-query JSON root `{ query, results, metadata }` |
| `multi-search-output.schema.json` | `MultiSearchOutput` | Multi-query JSON root `{ query_count, searches[] }` |
| `search-result.schema.json` | `SearchResult` | Individual result row |
| `news-result.schema.json` (v0.8.9+) | `NewsResult` | Individual news-vertical result row (`--vertical news\|all`) |
| `search-metadata.schema.json` | `SearchMetadata` | Latency, identity, cascade level |
| `probe-output.schema.json` | `ProbeReport` | `--probe` JSON response |
| `probe-deep-output.schema.json` (v0.7.3+) | `ProbeDeepReport` | `--probe-deep` JSON response with `status`, `cascade_reason`, `mitigation_suggestion`, `http_status`, `latency_ms`, `endpoint` (v1.0.2 EN wire; legacy PT names only with `--wire-keys pt`) |
| `deep-research-output.schema.json` (v0.8.9+) | `DeepResearchOutput` | `deep-research` JSON root `{ kind, query, metadata, results[], news[], news_count, synthesis? }` |
| `config.schema.json` | (config TOML) | Content of the two TOML files `init-config` writes: `selectors` + `user_agents` |
| `init-config-output.schema.json` (v1.0.3+) | `InitConfigReport` | Report `init-config` prints: which files were created / skipped / overwritten |
| `deep-research-budget.schema.json` (v1.0.3+) | `budget::print` | `--print-budget` payload (`type: deep_research_budget`) — one discriminator per file |
| `deep-research-error.schema.json` (v1.0.3+) | `budget::print`, `output::deep_envelope` | All four shapes of `type: deep_research_error`, routed by the PAIR (`type`, `error`): `budget_underflow`, `cancelled`, `timeout`, `sub_queries_incomplete` |
| `commands-output.schema.json` (v1.0.3+) | `commands::commands_tree` | `commands` tree (`type: commands`) — the argv-discovery surface |
| `schema-catalog.schema.json` (v1.0.3+) | `commands::schema_cmd` | `schema` with no `--name` (`type: schema_catalog`), including the `discriminator` routing hint |
| `locale-output.schema.json` (v1.0.3+) | `commands::locale` | `locale` UI-language diagnostic — carries `type: locale` since v1.0.4 |
| `doctor-output.schema.json` (v1.0.3+) | `DoctorReport` | `doctor` host readiness (`type: doctor`) — read `status`, not the legacy `ok` |
| `config-list-output.schema.json` (v1.0.3+) | `commands::config` | `config list` — what is STORED in XDG; carries `type: config_list` since v1.0.4 |
| `config-path-output.schema.json` (v1.0.3+) | `commands::config` | `config path` — `{config_directory, config_file}` |
| `config-get-output.schema.json` (v1.0.3+) | `commands::config` | `config get <KEY>` — `{key, present, value}`; `present` false still falls back to a default |
| `config-mutation-output.schema.json` (v1.0.3+) | `commands::config` | `config set` and `config unset`, discriminated from each other by `action` rather than `type` |
| `config-effective-output.schema.json` (v1.0.3+) | `commands::config` | `config effective` — what would APPLY, with the `cli`/`xdg`/`default` layers and which one won |
| `error-response.schema.json` | `CliError` | Structured error envelope (stderr / exit 2 path). Routed by SHAPE, not by discriminator: it carries no `type`. |
| `classified-error-output.schema.json` (v1.0.4+) | `run`, `commands::schema_cmd` | The `type: "error"` envelope, where `error` is an OBJECT with `category` / `code` / `message`. Until v1.0.4 the catalog routed `type: "error"` to `error-response`, a different shape where `error` is a string. |
| `ndjson-event.schema.json` | `SearchOutput` per line | Multi-query `--stream` NDJSON: one compact `SearchOutput` object per LF line (not event envelopes) |

Status (v1.0.6):
- Present on disk and hand-maintained in sync with `src/types/` under Chrome-only production (GAP-WS-113) and agent-ready defaults (GAP-WS-AGENT-READY-001 / ADR-0018)
- NO JSON schema break for lifecycle in 1.0.0 — the schemas are unchanged
- The process+disk one-shot contract (GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020, extending process-only GAP-WS-LIFECYCLE-001 / ADR-0017) is OPERATIONAL ONLY
- That contract covers the profile prefix `ddg-chrome-*`, cooperative `force_reap` / `ExitReapGuard` / `remove_dir_all`, and next-run `sweep_orphan_profiles` of owned `ddg-chrome-*` only
- HARD POLICY: never bulk-rm foreign `.tmp*` or `org.chromium.Chromium.*`
- Schemas still do NOT encode profile path or disk ownership
- Document honesty: lifecycle is a process+disk runtime contract, not a schema-breaking change
- Additive agent-ready fields from 0.9.8 remain the defaults in force: `metadata.chrome_path_resolved`, `metadata.chrome_channel`, honest `used_chrome`, and news/web `content*` when content fetch is on
- Content fetch is ON by default; the opt-out is `--no-fetch-content`; FETCH_CAP=4 for web+news (v1.0.2)
- The default vertical is `all`
- Multi-search (`multi-search-output.schema.json`): each `searches[]` item `$ref`s `search-output.schema.json`, so chrome agent metadata is inherited per query via `metadata` (not telemetry)
- Error path: many failures emit a full `SearchOutput` via `failure_output`/`error_output` (full chrome contract)
- The thin `error-response.schema.json` may still carry best-effort `metadata.used_chrome` / `chrome_path_resolved` / `chrome_channel` on residual thin error envelopes (PT keys only with `--wire-keys pt`)
- Schemas cover `search-output`, `search-metadata`, `search-result`, `news-result`, `deep-research-output`, `probe-output`, `probe-deep-output`, `multi-search-output`, `config`, `error-response` and `ndjson-event` (stream line = SearchOutput)
- Since v1.0.3 they also cover `init-config-output`, `deep-research-budget`, `deep-research-error`, `commands-output`, `schema-catalog`, `locale-output`, `doctor-output`, `config-list-output`, `config-path-output`, `config-get-output`, `config-mutation-output` and `config-effective-output`
- Since v1.0.4 they also cover `classified-error-output`
- All 24 are validated against a real envelope by `tests/integration_schema_conformance.rs`, with no exemptions
- `commands::schema_cmd` is asserted to expose exactly this set
- Since v1.0.4 that coverage claim is MEASURED rather than declared
- The ledger harvests the `assert_conforms` call sites across `tests/`, so deleting a test turns the schema red instead of leaving a stale name in a hand-written list
- v1.0.4 also gave the five `config-*` envelopes, `locale-output` and `init-config-output` a `type` discriminator
- All seven were unroutable from the published catalog before that
- A schema with no discriminator is invisible to any comparison between the routing table and the schemas

Routing rule (v1.0.3, ADR-0031):
- One published schema per `type` value
- An agent reads `type` off an envelope, looks it up in the `discriminator` field of the `schema` catalog, and fetches that schema — no hardcoded mapping
- `deep_research_error` is the one discriminator with four disjoint payloads, so its schema keys each `oneOf` branch on `error` as well
- The effective routing key there is the PAIR (`type`, `error`)
- Since v1.0.4 `locale` carries `type: locale` and `config list` carries `type: config_list`, so both route by discriminator like every other surface
- MEASURED on 2026-08-21: `locale` emitted `"type":"locale"` and `config list` emitted `"type":"config_list"` on binary v1.0.6
- `every_emitted_discriminator_has_a_published_schema` fails the build if a new envelope ships without a contract
- The drift test alone could never catch that, because it compares files to files and an undeclared envelope has no file
- `--stream` (multi-query) emits NDJSON — one compact `SearchOutput` per LF line (`output::emit_ndjson`), not begin/match/end event envelopes
- Since v1.0.1 the CLI `-f ndjson` is an alias that enables the same multi-query stream mode as `--stream` (domain format stays JSON; single-query ignores stream with a warning)
- Wire names (ADR-0027 supersedes the ADR-0023 default): schemas document ENGLISH keys as primary on the wire (`results`, `metadata`, …)
- English `serde` deserialize aliases exist for input and fixtures only, and do NOT change serialize output
- Rust types remain the source of truth
- The 24 schemas are hand-maintained on disk — there is no generation step and no schema-generation dependency
- Gates are LOCAL only (`NO_CI.md`)


## News Vertical Fields (v0.8.9, GAP-WS-104; defaults v0.9.8)

The `--vertical <web|news|all>` flag (default `all` since v0.9.8; historical default was `web`) emits news fields when vertical is `news` or `all`. Multi-query batches accept `--vertical news|all` since GAP-WS-105; each `searches[]` item of `multi-search-output.schema.json` may carry them:

- Root `news[]` — array of `news-result.schema.json` objects. Guaranteed
  per item: `position` (integer, 1-indexed), `title` (string), `url` (string).
  Optional per item: `source`, `relative_date`, `thumbnail`, and (v0.9.8) `content` /
  `content_size` / `content_extraction_method` when content fetch is on.
- Root `news_count` — integer count after dedupe/cap. The process
  exit code sums `result_count + news_count`.
- `metadata.vertical_used` — `"news"` or `"all"`.
- `metadata.chrome_path_resolved` / `metadata.chrome_channel` — agent metadata
  (v0.9.8; not telemetry).

With explicit `--vertical web` (and optionally `--no-fetch-content`) news fields
are ABSENT, preserving a thin web-only envelope. Validators must treat news and
content fields as optional (`required` lists do not force them).

The `zero_cause` enum (on `metadata`; legacy PT `causa_zero` only with `--wire-keys pt`)
gains the variant `vertical-no-results` (legacy PT `vertical-sem-resultados`):
legitimate zero from the news vertical (rendered SERP without articles), exit 5 —
an anti-bot interstitial in the news body still classifies as `anti_bot` / `anti-bot`.


## Deep-Research News Fields (v0.8.9, GAP-WS-105)

`deep-research` scans the news vertical by DEFAULT (opt-out `--no-news`) and
its envelope (`deep-research-output.schema.json`) gains:

- Root `news[]` — aggregated news items. Guaranteed per item: `position`,
  `title`, `url`, `score` (news-only RRF, NOT comparable with
  `results[].score`), `occurrences` (number of sub-queries the item
  appeared in). Optional: `source`, `relative_date` (verbatim string),
  `thumbnail`. Legacy PT keys only with `--wire-keys pt`.
- Root `news_count` — ALWAYS present (0 with `--no-news` or zero news).
- `metadata.unique_news_count` — ALWAYS present (EN wire v1.0.2 / ADR-0027).
- `metadata.sub_queries[].news_count` and
  `metadata.sub_queries[].news_unavailable` — OPTIONAL (omitted with
  `--no-news`; `news_unavailable: true` when the news scan failed mid-flight
  as a structured field — not a production HTTP transport degrade).


## Maintenance Strategy

All 24 schemas already exist on disk and are hand-maintained. No code generation step is planned, and no schema-generation dependency is adopted. The rules below govern how they stay true.
- Rust types in `src/types/` remain the SOURCE OF TRUTH; a schema is edited only after the type changes
- Every new emitted envelope MUST ship a schema in the same change, because `every_emitted_discriminator_has_a_published_schema` fails the build otherwise
- Every schema MUST be validated against a REAL envelope in `tests/integration_schema_conformance.rs`, never against a hand-built fixture where the product can write the bytes
- The coverage claim is MEASURED, not declared: the ledger harvests the `assert_conforms` call sites across `tests/`, so deleting a test turns the schema red instead of leaving a stale name in a hand-written list
- Every schema carrying a `type` MUST appear in the `discriminator` routing table emitted by `schema` with no `--name`
- Gates are local only (`NO_CI.md` — no remote CI/Actions); run them before release, never "on every push"


## Schema Coverage Checklist

Files on disk vs. still missing:

- [x] `search-output.schema.json`
- [x] `multi-search-output.schema.json`
- [x] `search-result.schema.json`
- [x] `search-metadata.schema.json`
- [x] `probe-output.schema.json`
- [x] `probe-deep-output.schema.json` (v0.7.3+ — for `--probe-deep` flag)
- [x] `news-result.schema.json` (v0.8.9+)
- [x] `deep-research-output.schema.json` (v0.7.0+ — `deep-research` subcommand; v0.8.7 adds `.query`; v0.8.9 GAP-WS-105 adds `news[]`, `news_count`, `metadata.unique_news_count` and per-sub-query news fields; v1.0.2 GAP-SCHEMA-DEEP adds `partial` / `sub_queries_*` / `chrome_contention_advisory` / `total_time_ms`; wire EN ADR-0027)
- [x] `config.schema.json` (for `init-config` / config TOML shape)
- [x] `error-response.schema.json` (structured error envelope; routed by shape — it has no `type`)
- [x] `classified-error-output.schema.json` (v1.0.4 — the `type: "error"` envelope, found when the discriminator table was first checked against the schemas)
- [x] `ndjson-event.schema.json` (implemented — multi-query `--stream` emits NDJSON `SearchOutput` lines)
- [x] `init-config-output.schema.json` (v1.0.3 — was the last open box on this list; the report is now validated in four action variants)
- [x] `deep-research-budget.schema.json` (v1.0.3 — `--print-budget` only; a test asserts the refusal does NOT validate here, so the split cannot regress)
- [x] `deep-research-error.schema.json` (v1.0.3 — all four `deep_research_error` shapes; the cancel and timeout branches are validated against bytes the product really wrote, not hand-built fixtures)
- [x] `commands-output.schema.json` (v1.0.3)
- [x] `schema-catalog.schema.json` (v1.0.3 — the index was the one page missing from the index)
- [x] `locale-output.schema.json` (v1.0.3)
- [x] `doctor-output.schema.json` (v1.0.3)
- [x] `config-list-output.schema.json` (v1.0.3)
- [x] `config-path-output.schema.json` (v1.0.3)
- [x] `config-get-output.schema.json` (v1.0.3)
- [x] `config-mutation-output.schema.json` (v1.0.3 — `set` and `unset`, keyed on `action`)
- [x] `config-effective-output.schema.json` (v1.0.3)

> Closing `config list` alone would have repeated the very mistake this round
> corrects. Sweeping the whole `config` family found four more undeclared
> envelopes: `path`, `get`, the shared `set`/`unset` acknowledgement, and
> `effective`. Fixing the instance is not fixing the class.


## Validation

The binary is the SOURCE of the schemas: the `schema` subcommand emits the catalog with no `--name`, and one schema body with `--name <ID>`. No Python validator is used anywhere in this project.

```bash
# List every published schema id and the discriminator routing table
timeout 30 duckduckgo-search-cli schema -q -f json | jaq -c '.schemas[].id'

# Fetch one schema body straight from the binary (no file path needed)
timeout 30 duckduckgo-search-cli schema --name search-output -q -f json > /tmp/search-output.schema.json

# Capture real output and route it by its own discriminator
timeout 30 duckduckgo-search-cli -q -f json "rust" > /tmp/out.json
jaq -r '.type // "no-discriminator (routed by shape)"' /tmp/out.json

# Fetch the probe-deep contract the same way (v0.7.3+)
timeout 30 duckduckgo-search-cli schema --name probe-deep-output -q -f json > /tmp/probe-deep.schema.json
timeout 15 duckduckgo-search-cli --probe-deep -q -f json > /tmp/probe.json
```

Conformance of a real envelope against its schema is asserted by the LOCAL Rust harness, `tests/integration_schema_conformance.rs`, run with `cargo test --test integration_schema_conformance`. That harness is the only sanctioned validator, and it covers all 24 schemas with no exemptions.


## English

This file documents the JSON schema inventory for `duckduckgo-search-cli`.
The schemas are machine-readable contracts that allow agents, IDEs, and
type-safe clients to validate CLI output without running the binary.
Production output contracts assume Chrome-only network transport
(GAP-WS-113 / ADR-0016) and agent-ready defaults (GAP-WS-AGENT-READY-001 /
ADR-0018): default `--vertical all`, content fetch ON (opt-out
`--no-fetch-content`, FETCH_CAP=4 for web+news (v1.0.2)). Current release status is
v1.0.6: lifecycle is process+disk (GAP-WS-TMP-PROFILE-ORPHAN-001 /
ADR-0020); that contract is operational only (`ddg-chrome-*`, `force_reap` /
`ExitReapGuard`, never bulk-rm foreign `.tmp*` / `org.chromium.Chromium.*`;
schemas do not encode profile path; no JSON schema break vs 0.9.x agent-ready
fields).

Multi-search inheritance: `multi-search-output.schema.json` `searches[]` items
`$ref` `search-output.schema.json`, so each query envelope inherits
`metadata.chrome_path_resolved`, `metadata.chrome_channel`, and honest
`used_chrome` from `search-metadata.schema.json` (agent metadata, not
telemetry).

Failure envelopes: many failures emit a full `SearchOutput` via
`failure_output`/`error_output` (complete chrome agent contract on
`metadata`). The thin `error-response.schema.json` may also expose
best-effort `metadata.used_chrome` / `chrome_path_resolved` / `chrome_channel`
on residual thin error paths (PT keys only with `--wire-keys pt`).

Stream / NDJSON (v1.0.1): multi-query `--stream` emits one compact
`SearchOutput` per LF line (`ndjson-event.schema.json`). CLI `-f ndjson`
is an alias for that stream mode since 1.0.1 (not a separate non-stream format).

Wire field names (ADR-0027 / v1.0.2): English keys are primary on serialize
and in these schemas (`results`, `metadata`, `used_chrome`, …). Portuguese deserialize
aliases remain accepted; legacy PT emit only via `--wire-keys pt` or XDG `wire_keys=pt`.

## Português Brasileiro

Este arquivo documenta o inventário de schemas JSON para `duckduckgo-search-cli`.
Os schemas são contratos legíveis por máquina que permitem a agentes, IDEs e
clientes type-safe validar a saída da CLI sem executar o binário.

### Status (v1.0.6)

Presentes em disco e mantidos à mão em sincronia com `src/types/` sob produção
Chrome-only (GAP-WS-113) e defaults agent-ready (GAP-WS-AGENT-READY-001 /
ADR-0018). Sem quebra de schema JSON no lifecycle na 1.0.0 — os schemas
permanecem inalterados; o contrato one-shot processo+disco
(GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020, estendendo o one-shot de processo
GAP-WS-LIFECYCLE-001 / ADR-0017) é apenas operacional (prefixo de perfil
`ddg-chrome-*`, `force_reap` / `ExitReapGuard` / `remove_dir_all` cooperativo,
`sweep_orphan_profiles` da próxima run só em `ddg-chrome-*` de propriedade;
política rígida: nunca bulk-rm de `.tmp*` estrangeiro nem
`org.chromium.Chromium.*`). Os schemas não codificam path de perfil nem
posse em disco — honestidade documental: o lifecycle é contrato de runtime
processo+disco, não mudança que quebra schema. Campos aditivos agent-ready da
0.9.8 continuam como defaults vigentes: `metadata.chrome_path_resolved`,
`metadata.chrome_channel`, `used_chrome` honesto (incluindo deep-research e
envelopes de falha); `content` em web/news com fetch de conteúdo (LIGADO por
padrão; opt-out `--no-fetch-content`; FETCH_CAP=4 v1.0.2). Vertical padrão da search
é `all`.

Multi-search: cada item de `searches[]` em `multi-search-output.schema.json`
usa `$ref` de `search-output.schema.json`, herdando metadados chrome de agente
por query via `metadata` (não é telemetria; wire EN padrão ADR-0027).

Falhas: muitas falhas emitem `SearchOutput` completo via
`failure_output`/`error_output` (contrato chrome completo em `metadata`); o
schema fino `error-response.schema.json` pode ainda carregar
`metadata.used_chrome` / `chrome_path_resolved` / `chrome_channel` best-effort
no caminho residual de erro fino (chaves PT só com `--wire-keys pt`).

Schemas cobertos, os 24 presentes em disco: `search-output`,
`search-metadata`, `search-result`, `news-result`, `deep-research-output`,
`probe-output`, `probe-deep-output`, `multi-search-output`, `config`,
`error-response`, `ndjson-event`; desde a v1.0.3 `init-config-output`,
`deep-research-budget`, `deep-research-error`, `commands-output`,
`schema-catalog`, `locale-output`, `doctor-output`, `config-list-output`,
`config-path-output`, `config-get-output`, `config-mutation-output` e
`config-effective-output`; e desde a v1.0.4 `classified-error-output`. Todos os
24 são validados contra um envelope real por
`tests/integration_schema_conformance.rs`, sem isenções, e `commands::schema_cmd`
é asseverado a expor exatamente esse conjunto. O schema `ndjson-event`
documenta o wire implementado de multi-query `--stream`: cada linha NDJSON
é um `SearchOutput` compacto (runtime emite via `output::emit_ndjson`).
Desde a v1.0.1, a flag CLI `-f ndjson` é alias do modo stream multi-query
(igual a `--stream`; formato de domínio permanece JSON; single-query ignora
stream com aviso).

Nomes no wire (ADR-0027 / v1.0.2): as chaves em inglês são primárias na
serialização e nestes schemas (`results`, `metadata`, …); aliases ingleses
de `serde` valem só na deserialização (fixtures/ferramentas) e não alteram
o stdout. As definições de tipo Rust permanecem a fonte da verdade.

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
- [x] `classified-error-output.schema.json` (v1.0.4 — o envelope `type: "error"` onde `error` é OBJETO com `category` / `code` / `message`)
- [x] `ndjson-event.schema.json` (implementado — multi-query `--stream` emite linhas NDJSON `SearchOutput`)
- [x] `init-config-output.schema.json` (v1.0.3 — era a última caixa aberta desta lista)
- [x] `deep-research-budget.schema.json` (v1.0.3 — SOMENTE `--print-budget`; um teste garante que a recusa NÃO valida aqui)
- [x] `deep-research-error.schema.json` (v1.0.3 — as quatro formas de `deep_research_error`)
- [x] `commands-output.schema.json` (v1.0.3)
- [x] `schema-catalog.schema.json` (v1.0.3)
- [x] `locale-output.schema.json` (v1.0.3)
- [x] `doctor-output.schema.json` (v1.0.3)
- [x] `config-list-output.schema.json` (v1.0.3)
- [x] `config-path-output.schema.json` (v1.0.3)
- [x] `config-get-output.schema.json` (v1.0.3)
- [x] `config-mutation-output.schema.json` (v1.0.3 — `set` e `unset`, chaveados por `action`)
- [x] `config-effective-output.schema.json` (v1.0.3)

### Regra de roteamento (v1.0.3, ADR-0031)

Um schema publicado por valor de `type`. O agente lê `type` do envelope, procura
esse valor no campo `discriminator` do catálogo `schema` e busca aquele schema —
sem mapeamento hardcoded. `deep_research_error` é o único discriminador com
quatro payloads disjuntos, então seu schema fixa `error` em cada ramo `oneOf`: a
chave de roteamento efetiva ali é o PAR (`type`, `error`). Desde a v1.0.4 as
duas superfícies `locale` e `config list` carregam discriminador próprio:
`locale` emite `type: locale` e `config list` emite `type: config_list`, então
ambas roteiam por discriminador como todas as demais. MEDIDO em 2026-08-21 no
binário v1.0.6: `locale` devolveu `"type":"locale"` e `config list` devolveu
`"type":"config_list"`.

A primeira tentativa desse contrato publicou só a forma `budget_underflow` sob
`deep-research-budget.schema.json`. Isso fez o arquivo reivindicar o
discriminador `deep_research_error` cobrindo um de seus quatro formatos, então
um agente validando um envelope `cancelled` reprovava em TODOS os ramos. Schema
errado é pior que schema ausente: ausente, o agente sabe que não sabe.
