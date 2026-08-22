# ADR-0031 — One published schema per `type` discriminator, and the emitted set is what must be covered (v1.0.3)

- Status: Accepted (2026-08-08)
- Related: ADR-0030 (a schema is a contract only when validated), GAP-SCHEMA-OVERLOADED-DISCRIMINATOR-001, GAP-SCHEMA-INTROSPECTION-UNDECLARED-001
- Decisor: lead
- Context: third-pass audit of v1.0.3 — auditing the FIX from ADR-0030 found the fix had introduced a worse defect than the one it closed


## Context

ADR-0030 established that a published schema is a contract only when a real
envelope is validated against it, and shipped `deep-research-budget.schema.json`
to close the last undeclared envelope the audit had in hand.

Auditing that work found two things.

### A schema that claims a discriminator it only partly covers

`type: "deep_research_error"` is emitted from FOUR sites with four disjoint
payload shapes:

- `src/budget/print.rs` — `budget_underflow`, nineteen keys, exit 2
- `src/output/deep_envelope.rs` — `cancelled`, six keys, exit 130 or 143
- `src/output/deep_envelope.rs` — `timeout`, eight keys plus nine more when
  partials were harvested, exit 4
- `src/commands/deep_research_emit.rs` — `sub_queries_incomplete`, eight keys, exit 2

`deep-research-budget.schema.json` covered the first behind a `oneOf` with
`additionalProperties: false` on both branches. An agent validating a `cancelled`
envelope against the schema that claims its discriminator therefore failed BOTH
branches.

Before the fix there was no schema and the agent knew it did not know. After the
fix there was a schema that rejected a valid envelope. **A wrong schema is worse
than an absent one**, because absence is legible and a false negative is not.

The root cause is that `type` was never a sufficient discriminator for this
family, and publishing a schema for the one shape that happened to be in hand
encoded that mistake into the contract.

### Nine introspection surfaces with no contract at all

`commands`, `schema` (catalog), `locale`, `doctor` and `config list` all write
structured JSON and none had a published schema. `doctor` is the richest, at
eighteen keys, and the one an agent is most likely to parse before deciding
whether a run is worth attempting.

Closing `config list` alone would have repeated the mistake this ADR exists to
correct — fixing the instance in hand and leaving the class. Sweeping the whole
`config` family found four more undeclared envelopes:

- `config path` — `{config_directory, config_file}`
- `config get` — `{key, present, value}`, where `present` distinguishes STORED
  from the compiled default, a difference the exit code does not carry
- `config set` and `config unset` — discriminated from each other by `action`,
  not by `type`, so their shared schema keys its `oneOf` on that instead
- `config effective` — the `cli`/`xdg`/`default` layers per key, with `source`
  naming the winner

### Why no gate caught either

`catalog_matches_published_schema_files` compares two sets of FILES: what sits
under `docs/schemas/` against what `SCHEMAS` compiles in. It is real coverage and
it is structurally blind to the opposite direction. "Envelope emitted with no
schema" has no file for a file-to-file comparison to enumerate, so the absence is
invisible BY CONSTRUCTION, not by oversight.

That blind spot had already been written down in the ADR-0030 work. It was
documented and then not swept. Fixing the instance did not fix the class.


## Decision

One published schema per `type` value. The routing rule an agent follows is
mechanical: read `type` off the envelope, find the schema that declares it, fetch
that schema. Splitting a discriminator across files, or serving two discriminators
from one file, both break that rule.

- `deep-research-budget.schema.json` describes ONLY `deep_research_budget`.
- `deep-research-error.schema.json` describes ALL FOUR shapes of
  `deep_research_error`, keying each `oneOf` branch on `error` with a `const`, so
  exactly one branch matches. The effective routing key for this family is the
  PAIR (`type`, `error`), and the schema encodes that instead of leaving it to
  the reader.
- The nine introspection surfaces get schemas. Six of them — `locale` and the
  five `config` subcommands — carry no `type` at all; that asymmetry is recorded
  in their descriptions rather than fixed, because adding `type` now would be a
  wire change to surfaces reachable only by asking for them explicitly. They are
  identified by the subcommand the caller invoked, which it always knows.
- Where a family has a discriminator under a different NAME, the same pattern
  applies: `config set` and `config unset` share one file keyed on `action`. The
  rule is one file per discriminator VALUE, not one file per field called `type`.

The catalog carries the routing rule as data. Each entry in the `schema`
catalog gains an optional `discriminator` field naming the `type` value its
schema describes. A table that lives only in a test protects the build; publishing
it means the consumer no longer has to hold the mapping.

Coverage is measured against the EMITTED set, not the published set.
`every_emitted_discriminator_has_a_published_schema` walks the crate source,
harvests every `"type": "…"` literal, and fails if one is not declared. It
asserts the scan is a SUBSET of a hand-written table rather than claiming the
scan is complete: three envelopes — `doctor`, `probe`, `probe-deep` — carry their
discriminator on a `#[serde(rename = "type")]` struct field, so no literal exists
to find. Naming the scan's limit is what keeps it from becoming the product's.


## Consequences
- Twenty-three schemas ship, every one validated against a real envelope, no
  exemptions. The conformance suite goes from 28 cases to 42.
- The cancel and timeout branches are validated against **bytes the product
  actually wrote**: the test arms the in-flight guard, drives the real signal
  path, and reads the file back. Hand-building the payload in the test would have
  validated the test's idea of the envelope rather than the product's — the same
  substitution that produced the defect this ADR closes.
- `sub_queries_incomplete_payload` moved from a private module to
  `output::deep_envelope` and became public. Three of the four shapes now live in
  one module. Their being scattered across three files is the structural reason
  nobody saw they shared a discriminator: no single place showed the set.
- A regression test asserts the refusal does NOT validate against the budget
  schema, so merging the two files back fails loudly instead of quietly
  re-teaching agents the wrong routing rule.
- Adding an envelope without a contract now breaks the build.
- One asymmetry is documented rather than removed. The four error shapes do
  not share a wire policy: `budget_underflow` bypasses the mapper and stays
  English, while `cancelled` and `timeout` go through it, so `--wire-keys pt`
  renames `type` to `tipo` for those two. Under pt an agent routing by `type`
  therefore finds it in only two of the four. Unifying was rejected for a
  concrete reason: the timeout envelope embeds `partial_results`, which IS search
  data, and bypassing the mapper would leave that portion English too — a
  behaviour change to the data, not just the diagnostics. The asymmetry is
  described in the schemas and asserted by
  `error_envelope_wire_policy_differs_between_budget_and_signal_paths`, so it
  cannot drift silently, and whoever unifies it must first decide what the
  embedded payload should do.


## Alternatives rejected
- One file per error variant (`deep-research-cancelled.schema.json`, …).
  Breaks routing: the agent reads `type` BEFORE it knows which file to open, so
  four files for one `type` leaves it guessing which to try.
- Widening the budget schema to accept all four shapes. Would have kept one
  file for two discriminators, so `--print-budget` output and a refusal would
  validate against the same contract and `type` would stop distinguishing them.
- Silencing the unused-table warning with `#[cfg(test)]`. Would have hidden
  that the mapping served only the build. Publishing it in the catalog gave the
  constant a real consumer and removed the warning as a side effect.
- Pinning `partial_results` to `deep-research-output.schema.json` by `$ref`.
  The payload is truncated to fifteen rows and, under `--fields`, projected, so
  required keys of the full contract may legitimately be absent. Pinning it would
  reject a valid envelope — precisely the defect being fixed.


## Verification
- `cargo test --all-features` — 42 conformance cases, all green, `EXCLUDED` empty.
- `every_emitted_discriminator_has_a_published_schema` — green, and proven to
  bite: its first run failed on `"…"` harvested from a doc comment, which is why
  it now skips comment lines.
- `schema_catalog_routes_every_discriminator_unambiguously` — asserts no two
  schemas claim one `type`.
- `budget_schema_no_longer_claims_the_error_discriminator` — asserts the split.
- The eleven local gates of `NO_CI.md`, all exit 0.
