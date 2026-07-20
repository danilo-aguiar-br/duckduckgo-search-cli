# ADR-0023 — Wire JSON Portuguese BC + English deserialize aliases

- Status: Accepted (2026-07-19)
- Related: GAP-E2E-51-008, GAP-WIRE-PT-001, `src/types/wire.rs`,
  `docs/schemas/*`, rules `rules_rust_codigo_ingles_internacionalizacao`
- Decisor: lead / TEAM-QUALITY

## Context

1. Agent stdout JSON has used **Portuguese keys** since early v0.x
   (`resultados`, `metadados`, `quantidade_resultados`, `regiao`, `noticias`,
   `causa_zero` kebab values such as `legitimo`, `filtro-silencioso`, …).
2. Rust **identifiers, logs, and user-facing messages** are English. Rules
   prefer English code; the wire surface is an intentional exception.
3. Renaming serialize keys to English would be a **MAJOR** breaking change
   for every agent/skill/schema consumer pinned on the PT contract.
4. Dual-writing both PT and EN keys on every emit would inflate payloads and
   still leave a dual mental model without a migration flag plan.

## Decision

1. **Keep Portuguese keys on serialize** for the entire v1.x line. The agent
   contract remains PT JSON. Schemas under `docs/schemas/` document PT keys.
2. **Do not silent-rename** output keys. No opt-in dual-write flag in this
   change set (full dual-write deferred; cost/benefit tracked under
   GAP-E2E-51-008 residual).
3. **Add English `serde(alias = "...")` on key wire fields and `ZeroCause`
   variants for DESERIALIZE only.** Serde aliases never affect serialize:
   stdout still emits PT names; fixtures/tools may feed EN spellings when
   parsing into the same structs.
4. Document the tension honestly: code EN + wire PT is **deliberate BC**, not
   an unfinished i18n bug. Rules EN applies to source identifiers; wire is a
   published contract.

## Consequences

### Positive

- Honesty vs pure-EN rules without breaking agents.
- EN fixtures/round-trips possible without inventing a second DTO layer.
- Serialize surface stays byte-stable for existing jq/skill consumers.

### Negative / accepted

- Dual mental model (code field `results` ↔ wire `resultados`) remains.
- Not every nested/deep-research key needs an alias immediately; residual
  deep-research PT-only keys are fine until a consumer asks.
- Full dual-write or MAJOR EN migration remains a future product decision.

## Verification

- Serialize a `SearchOutput` / `ZeroCause` → keys stay PT (`resultados`,
  `legitimo`, …).
- Deserialize JSON that uses English aliases (`results`, `legitimate`, …)
  into the same structs succeeds.
- `cargo test --lib` covers alias smoke + existing wire snapshots.

## Non-goals

- No product env flag for EN wire.
- No CI, no telemetry, no schema `$id` MAJOR bump solely for aliases.
