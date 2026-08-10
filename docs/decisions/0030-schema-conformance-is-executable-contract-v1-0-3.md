# ADR-0030 — A published schema is a contract only when an envelope is validated against it (v1.0.3)

- Status: Accepted (2026-08-08)
- Related: ADR-0027 (English wire default), GAP-WIRE-PROBE-001, GAP-SCHEMA-STALE-PT-001, GAP-SCHEMA-UNDECLARED-001, GAP-WIRE-HISTOGRAM-001
- Decisor: lead
- Context: second-pass audit of v1.0.3 — every gate was green and the published contract was still broken

## Context

`docs/schemas/*.json` is what agents code against. Until v1.0.3 nothing fed a real
envelope through a validator, so the eleven schemas were prose that happened to be
versioned alongside the code.

The first v1.0.3 pass added `tests/integration_schema_conformance.rs`, but covered
two schemas out of the six the plan required. The second-pass audit measured what
lived in the four blind spots:

- **Eleven emit sites bypassed the wire mapper.** `run.rs` and `commands::deep_research`
  built thin error envelopes with `serde_json::json!` and wrote them via
  `print_line_stdout(&payload.to_string())`. `--wire-keys pt` produced bytes identical
  to the English default. This is the exact mirror of the `--probe` defect that leaked
  Portuguese into English output — same root cause, opposite direction.
- **Seven schema properties still carried the pre-ADR-0027 Portuguese spelling.**
  `retentativas`, `news_filtradas_promo`, `cascata_nivel_observado`,
  `endpoint_used_compat`, `paralelismo` (also listed in `required`), and a second
  `retentativas` in `error-response`.
- **Five emitted fields were never declared.** `retries_configured`, `flags_ignored`,
  `result_count`, `results`, `next_action_suggestion`.
- **One Portuguese key was in the code, not just the schema.**
  `MultiSearchOutput` renamed its histogram to `causa_zero_histogram` on the wire, while
  `SKILL.md` documented `zero_cause_histogram`. Documentation and binary disagreed.

`search-metadata`, `multi-search-output` and `error-response` all set
`additionalProperties: false`. That is not a per-field warning: one undeclared key
rejects the **whole document**. So every successful search this CLI emitted failed
validation against the CLI's own published contract.

## Decision

A schema is only considered part of the contract when a real envelope — produced by the
Rust types through the actual wire path — is validated against it in CI-equivalent local
gates.

Three concrete rules follow.

1. **Wire fields go through one serializer.** `output::emit_wire_line` is the single
   entry point for anything carrying wire keys. `print_line_stdout` with a hand-built
   `json!` is reserved for introspection surfaces (`config`, `schema`, `commands`,
   `locale`, `init-config`, `doctor`), which stay English on purpose because their keys
   are configuration and command identifiers, not search-result fields. That exemption is
   now written in the doc comment instead of being an accident of call-site history.

2. **Fixtures are maximal, not representative.** A sparse fixture cannot catch an
   undeclared property, because the property is only emitted when something sets it.
   `flags_ignored` and `retries_configured` stayed invisible until the fixture forced
   every optional field to `Some` simultaneously.

3. **Coverage is itself asserted.** `every_published_schema_is_covered_or_explicitly_excluded`
   fails when a schema in `docs/schemas/` is neither validated nor accompanied by a written
   exemption. This is the countermeasure for the meta-gap: the reason nothing went red when
   the plan's Fase 2 shipped at one third of its required scope is that an unwritten test
   emits no signal.

## Consequences

Positive:

- Envelope coverage went from 2 to 10 of the 11 published schemas; the conformance suite
  went from 10 to 20 cases.
- The English default and `--wire-keys pt` are now both asserted per envelope, so a rename
  on one side without the other is a compile-or-test failure rather than a silent break.
- Schema rot has a failing test attached to it.

Negative, and accepted:

- `zero_cause_histogram` is a wire rename. The serde `alias` keeps pre-1.0.3 documents
  deserializable and `--wire-keys pt` still emits `causa_zero_histogram`, so a PT consumer
  sees no change; an English consumer that hard-coded the Portuguese key must update. The
  alternative — freezing a Portuguese key in the English wire — would contradict ADR-0027
  and keep the binary at odds with its own published skill.
- ~~`config.schema.json` remains uncovered. It describes the `selectors.toml` /
  `user-agents.toml` pair written by `init-config`, and no single emitted artifact has the
  combined `{selectors, user_agents}` shape. Synthesising one would test the fixture rather
  than the contract, so it is an explicit exemption with that reason recorded in the test.~~

> **Revoked 2026-08-08 (third-pass audit).** The premise was true and the conclusion was
> wrong. No single *file* has the combined shape, but two files exist and each has a
> checkable one — so the honest move was to describe them, not to skip them. Going to look
> found what the exemption had been covering: **`config.schema.json` was fiction.** It
> declared `user_agents` as an array of strings; the file `init-config` actually writes is a
> table whose root key is `agents`, holding `{ua, platform}` rows. It also implied
> `selectors.toml` was wrapped in a `selectors` key, when its root tables are
> `html_endpoint`, `lite_endpoint`, `pagination`, `related_searches` and `news`. No version
> of this product ever wrote the declared document, so no consumer can have been validating
> against it successfully and the rewrite breaks nobody.
>
> The schema now carries a `$defs` entry per real file and keeps the two-key aggregate as
> the published root. `init_config_artifacts_conform_to_published_schema` runs the real
> binary with `--config-home` into a temp dir, parses what landed on disk and validates it;
> `user_agents_file_is_a_table_of_rows_not_an_array_of_strings` pins the shape and asserts
> the old declaration is now *rejected*, so the fiction cannot come back.
>
> Envelope coverage is therefore **11 of 11**, and `EXCLUDED` in the coverage ledger is
> empty. That emptiness is the point: an exemption is a place defects hide, and this one hid
> a broken contract for three releases. The rule going forward is that a schema is exempt
> only when there is no artifact at all — not when assembling the artifact looks awkward.

## Verification

- `cargo test --all-features --locked --test integration_schema_conformance` — 20 passed.
- Eleven `NO_CI.md` gates re-run from scratch after the change, all exit 0.
- Live proof beyond fixtures: the installed binary's real search envelope was compared
  key-by-key against `search-output`, `search-metadata` and `search-result`; no undeclared
  key at any of the three levels.
- `--wire-keys pt` on the thin error path emits `erro`, `mensagem`,
  `quantidade_resultados`, `resultados`, where it previously emitted the English set.
