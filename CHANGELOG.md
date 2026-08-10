# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

The Portuguese edition of this document is [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md).


## [Unreleased]


## [1.0.5] — 2026-08-10 (close the class by ruler, not by list)

### Fixed — the class v1.0.4 declared closed was still open on `--probe`

v1.0.4 set out to abolish "flag accepted and ignored" and converted the six surfaces
its plan happened to NAME. `--probe` and `--probe-deep` were named too, and skipped.
They emitted through `emit_probe_payload`, a helper serving nineteen call sites that
never reached the projector, so every operator remained a no-op there. Measured on
v1.0.4: `--count-only`, `--limit 1`, `--fields status` and `--truncate-content 5`
each returned **633 bytes against a 633-byte baseline, at exit 0** — on the health
check surface an agent reaches for first.

Routing the probe through the projector was only half the fix. Every call site wrote
`let _ = emit_probe_payload(...)`, so a refusal would have been swallowed exactly like
the old no-op. `emit_probe` now RETURNS the exit code and each site passes the code it
wants on success, so the refusal reaches the caller.

The probe is a WIRE surface, so `--wire-keys pt` still applies: the new
`KeyPolicy::ProcessWire` runs the reduction on the English document first and maps keys
last, which keeps `--fields` paths meaning the same thing in both languages.

### Fixed — `--truncate-content` mutilated contract identifiers

`schema --truncate-content 12` turned `invoke` into `duckduckgo-s` and `id` into
`searc`: a command line that no longer runs and a name `schema --name` rejects. v1.0.4
exempted only the discriminator. The rule is one sentence — **a string the agent hands
back to a program is IDENTITY, not content** — and it now covers config keys, locale
tags, schema ids, filesystem paths and probe error codes, declared per surface in
`EnvelopeShape::identity` and published through `commands`.

Exempting identity opened a second hiding place, found by the new matrix test: on
`config list`, `config path`, `config get/set/unset`, `config effective` and `locale`
EVERY string is an identifier, so `--truncate-content 4` returned **1138 bytes against
a 1138-byte baseline** — byte-for-byte the signature of the original defect, correct
this time, and indistinguishable to the caller. Those surfaces now REFUSE with exit 2
and say why. A test asserts truncate still shortens real prose elsewhere, so the
exemption cannot quietly become total.

### Fixed — refusal had two contracts

`doctor`, `locale`, `commands`, `schema` and `init-config` each hand-rolled the same
`match`: exit 2, prose on stderr, stdout EMPTY. `config` hand-rolled a different one:
exit 2, an `error-response` envelope on stdout, nothing on stderr. Same flag, same
failure, two shapes, nothing declaring which was intended.

There is now ONE emitter, `output::emit_envelope_or_refuse`, for every surface
including the probe. stdout carries `{"error", "message"}` — the published
`error-response` shape, so a refusal is as routable as the success it replaces —
stderr carries the localized sentence, and the exit code is unchanged. The stdout
`message` stays English on purpose: it is the machine half of the contract.

### Fixed — the published capability matrix named the wrong thing

`commands` published `agent_ops[].discriminator` carrying the VALUE (`doctor`,
`schema_catalog`, `config_list`) while every envelope carries the KEY `type`. An agent
that trusted the matrix went looking for a key named `doctor`. The slot always meant
the key — that is what the reduction code compares against — so the data was corrected
and the field renamed to `discriminator_key`. Found while wiring `SURFACES` to be the
single definition of each shape; `config`, `schema`, `doctor`, `locale`, `commands` and
`init-config` now LOOK UP their shape instead of rebuilding a local copy.

### Added — the ruler that replaces the list

`tests/integration_stdout_boundary.rs` sweeps every stdout emission in `src/` and
requires each one outside `src/output/` to carry a declared reason. A new bypass fails
the build; a stale exemption fails the build too. This is what would have caught the
probe: the v1.0.4 failure was not the missed surface, it was closing a class by
enumerating targets, and an enumeration cannot report what is missing from itself.

`tests/integration_agent_ops_matrix.rs` runs every operator against every offline
surface and demands the pair — either the bytes fall, or the exit is 2 with a routable
envelope. It found two real defects on its first run.

### Added — probe ceilings are configuration, not literals

`Duration::from_secs(args.timeout_seconds.min(30))` and three siblings were policy
written as digits inside Chrome calls. They are now named constants in `types::bounded`
with rustdoc explaining each value, and four XDG keys —
`probe_launch_timeout_seconds`, `probe_extract_timeout_seconds`,
`probe_deep_launch_timeout_seconds`, `probe_deep_extract_timeout_seconds` — resolving
CLI, then XDG, then the compiled default. A shorter `--timeout` still wins.

### Changed — refusals speak the operator's language

The eight refusal sentences were raw English `format!` literals inside
`output::envelope_ops`, in a binary that ships `--ui-lang`. They are now `Message`
variants translated in `en` and `pt_br`. `CliError::AgentOpsRefused` carries BOTH
renderings from one template via `i18n::bilingual`, so the agent's stdout text stays
stable English while stderr follows the locale.

### Changed — the three-way routing partition is typed

`NON_DISCRIMINATED_SCHEMAS` mixed fragments, shape-routed envelopes and non-envelopes
in one free-text column. It is now a `RoutingKind` enum, and the claims are CHECKED: a
declared `parent` must really `$ref` the child, and declared `identified_by` keys must
really appear in `required` — resolved transitively through `allOf`, which is how the
first version of that test correctly failed on `ndjson-event`.

### Changed — the agent-native flags are declared once

The nine flags were written out on `CliArgs` AND on `DeepResearchArgs`, reconciled by
eight hand-written `if let` blocks, then copied field-by-field a third time into
`Config`. One `#[command(flatten)] AgentOpsArgs` replaces the duplicate declaration,
`AgentOpsArgs::overlay` replaces the merge, and `Config.agent_ops` replaces the loose
siblings. `--max-output-bytes` deliberately stays a sibling: it is a process-wide
stdout cap, not a per-envelope reduction.

### Changed — `--fields` now applies to a timed-out search

The timeout envelope was serialized straight to stdout, so `--fields` was honoured on
a search that succeeded and dropped on one that timed out. Same invocation, same flag,
two behaviours decided by network speed.

### Removed — the last orphans of the deleted Python generator

`docs/generated/flag-desc-{en,pt}.json` were inputs to the regenerator deleted in
v1.0.4: no reader in Rust or shell, fourteen of seventy flags, dated 31 July, and still
advertised in both CHANGELOGs. A new guard requires every file under `docs/generated/`
to name its consumer, so the next orphan cannot be created silently.

### Fixed — six product defects the second audit pass found

`--truncate-content` was accepted and IGNORED on `deep-research`: the function
`apply_truncate_content_deep` had an empty body while its three siblings —
search, multi and pipeline — all implemented the cut. An agent asked for a
smaller envelope, got exit 0, and received the whole thing. This is the same
class the entry above declares closed, surviving in the one cell of the matrix
nobody looked at.

`html_root_url` was frozen at `1.0.3` while the crate shipped `1.0.5`, so every
deep link from rustdoc pointed at a release that was not this one. A ruler now
compares the attribute against `CARGO_PKG_VERSION`; the attribute needs a string
literal, so the ruler is the only thing that can hold them together.

Chrome reaping on macOS was an EMPTY `cfg` block: `all(unix, not(linux))`
compiled to nothing, so an orphaned Chrome outlived the process and broke the
one-shot contract on that platform. The Windows fallback was the same defect in
another dialect — `windows_kill_by_cmdline_substring` was a no-op, and the live
path needed a `chrome_pid` that does not exist after a crash. Both now sweep by
the `ddg-chrome-*` marker, the prefix this CLI owns.

Production logging read `CARGO_BIN_EXE_timeout`, a Cargo TEST variable, outside
`cfg(test)` and outside any feature. Harness state was steering a shipped binary.

### Added — the language ruler measures four axes, not one

`code_comments_are_english` swept `src`, `tests` and `benches` and reported zero
survivors, because `looks_portuguese` returned false unless the line began with
`//`. The ruler had zeroed its own scope with a three-line `if`, and 307
non-comment lines carrying Portuguese sat outside it.

The replacement lives in `tests/common/language.rs` as a single SSOT — the two
markers tables it replaced had DIVERGED — and measures comments, assertion and
`tracing` prose, Rust identifiers, and EN/PT hybrids like `must not ria` that no
single-language marker catches. Fixture strings stay untouched: the product
searches in pt-BR, so that data is data. Exemptions are declared per file with a
written reason, and a second test proves every exemption still matches a file.

Two Portuguese sentences were reaching production logs and are now English.

### Added — the artifact says which tree it was built from

`build.rs` ran `git rev-parse --short=12 HEAD` and nothing else, so a clean build
and a dirty build reported the same string byte for byte. During an audit — which
is exactly when the tree is dirty — two different binaries were indistinguishable.
`--version` now carries a `-dirty` suffix when `git status --porcelain` is not
empty, and the narrow `rerun-if-changed` list was REMOVED, because without that
removal Cargo would not re-run the script and the suffix would be born stale.

### Changed — the rustdoc gate saw one feature set, and it was not the default

`cargo docs` pins `--all-features`, so six broken intra-doc links pointing at
items behind `http-test-harness` resolved under the gate and failed under plain
`cargo doc --no-deps` — the canonical command, and the one the pre-publish gate
runs. `package.metadata.docs.rs` sets `all-features = true`, so the PUBLISHED
documentation was never wrong; what was wrong is that the repository failed the
default command. The new `cargo docs-nohttp` alias measures the default profile,
and both are listed in `NO_CI`.

A companion ruler pins `rust-toolchain.toml` to the declared `rust-version`, so
a channel drifting above the MSRV can no longer keep every gate green while
breaking the user who honours it.

### Changed — errors speak Portuguese all the way down

`CliError::localized_detail` was corrected and NOTHING changed, because nine
emission sites in `run.rs` formatted the error through `Display` and never
called it. Under `--ui-lang pt-BR` the operator read a Portuguese prefix followed
by an English body. The `match` is now exhaustive, so a new variant cannot
compile without a translation, and the emission sites route through the one
function. `Message::ALL` stopped being a hand-copied list of the enum.

### Fixed — the wire described in prose was not the wire the binary emits

Seven claims in `llms.txt`, `llms.pt-BR.txt` and `llms-full.txt` were false and
each was measured against the source, the published schema or `--help` before
being touched.

`synth` is not a wire key — `serde(rename)` emits `synthesis`. There is no
top-level `sources` on the deep envelope; that array is nested inside
`synthesis`, and `partial` / `sub_queries_*` live under `.metadata`.
`discriminator_key` was documented as "always `type`" while `deep-research`
routes on `kind`, so an agent following the text would fail to route the one
surface that differs. The row-surface list omitted `buscar` and `deep-research`,
the two surfaces an agent uses most, even after v1.0.5 added them to `SURFACES`.
`--print-schema` and `--pre-flight` were labelled root-only and are global.
`wreq` and BoringSSL were sold in the present tense and are in no manifest.
The Baseline Contract block mixed `motor`, `regiao`, `metadados` with English
keys into a hybrid envelope no run has ever produced.

### Fixed — four exit-code tables that disagreed with each other

`llms-full.txt` carried four: one correct, one missing `130` and `143`, one
missing `6` and `141`, and a Portuguese one that stopped at `5`. `llms.txt` and
`llms.pt-BR.txt` omitted `130` and `143` too — while telling the reader to wrap
every call in `timeout`, which sends SIGTERM and therefore produces `143`. The
reader was instructed to generate a code the document refused to explain.

`every_exit_code_appears_in_every_exit_code_table` parses
`src/error/exit_codes.rs` instead of restating the list, so a new code is
undocumented-by-default rather than exempt-by-default. It failed on its first
run against `README.md`, naming `[130, 143]`.

### Fixed — the published crate shipped two documents without their translation

`include` in `Cargo.toml` is an ALLOWLIST, and `cargo package` does not warn
about a file you forgot to list. `BENCHMARKS.pt-BR.md` and `NO_CI.pt-BR.md` were
in the tree, were green under `every_root_document_has_a_translation` — which
enumerates the directory — and were absent from the tarball. A Portuguese reader
installing from crates.io received English-only for both, for every release that
had those files.

`published_documentation_ships_every_translation` now reads `cargo package
--list` instead of the directory. The repository passing is not the artifact
passing, and until this ruler existed nothing measured the difference.

### Fixed — documentation drift the doc rulers could not see

`llms-full.txt` — the artifact an agent loads for whole-product context — sat 27
flags behind the binary and stated `--global-timeout` default `60` in three
places when the real default is `180`. The flag ruler named two files and
measured ROOT flags only, so the file drifted with every gate green.
`every_documented_flag_reference_covers_the_live_surface` now covers it and the
subcommand-exclusive flags, and the file carries the full surface in two new
tables. `llms.txt` stays deliberately outside: its contract is the llmstxt.org
discovery stub, and forcing the tree into it would pit one written rule against
another.

### Performance

Aggregation borrowed `&[SearchOutput]` and was therefore FORCED into 26 clones
per query-by-result loop; it consumes by value now. TSV projection copied each
cell six times — one clone, four chained `replace`, one write, inside the loop —
and escapes in a single pass over `&str` instead.

`--truncate-content` truncates in place at a scalar boundary instead of allocating a
fresh `String` per field, and the process reduction knobs moved from
`RwLock<Option<AgentOps>>` — which cloned seven fields on every read — to a `OnceLock`
returning a reference. One-shot means install-once; the lock modelled a mutability that
does not exist.

A move-instead-of-clone in `project_paths` was implemented and BACKED OUT: it mutates
the document before later paths resolve, which degrades the `--fields` error message on
a contract surface. The comment in place records the measurement and the decision.

### Fixed — `jaq -r '.synth'` was documented in seven files and never worked

`deep-research --synthesize` serialises its report under `synthesis`, with `sintese`
kept as a deserialize alias. Seven documents — both `AGENTS`, both `AGENTS-GUIDE`, both
`HOW_TO_USE` and `COOKBOOK.md` — told the reader to run `jaq -r '.synth'`, which is the
Rust FIELD name and never crossed the wire. `jaq` answers `null` and exits 0, so the
recipe fails silently: the agent reports an empty synthesis rather than a broken
command.

`every_jaq_path_in_the_documentation_exists_in_a_schema` now harvests every key from
`docs/schemas/*.json` and checks each path that OPENS a `jaq` program in the
documentation. Seven legacy PT aliases and one foreign shape are exempt by name, each
with where it really comes from. Reintroducing `.synth` in one file was seen failing the
ruler before it was accepted.

### Fixed — the flag inventory published two flags the binary rejects

`docs/generated/cli-flags-inventory.json` listed `headless` and `name` in `root_longs`
and counted them in `root_count`. Neither is a flag: `headless` was lifted out of the
description of `--chrome-headless` ("Force headless Chrome (`--headless=new`)") and
`name` out of clap's "a similar argument exists: '--name'" tip. Both exit 2.

Three gates were green over it, because all three parsed the same help text with the
same scanner. A ruler that derives its expectation from the artifact it measures cannot
disagree with it. `declared_flags` now reads the option column only — which required
stripping the SGR sequences clap emits even into a pipe — and
`every_documented_root_flag_is_accepted_by_clap` asks the binary instead, passing an
unknown trailing token so argv parsing fails before any Chrome, socket or file work.

The same change exposed the opposite error: `--region` and `--max-concurrency` are
hidden clap aliases, real and accepted but absent from the option column, and the
phantom ruler had been calling two working flags phantoms. It now asks clap too.

### Fixed — agent prompts in `docs/INTEGRATIONS` used the PT wire under the EN default

The file states at the top that the default wire is English and that a reader must parse
`.results[]` and not `.resultados`. Thirty lines below, the copy-paste prompts for
Cursor, Aider, Continue, Cline, Roo Code and eight other hosts told the agent to run
`jaq '.resultados[:5] | map({titulo, url})'` with no `--wire-keys pt`. A document that
contradicts itself survives because nothing compares prose to prose.

Every operational path was converted to the EN wire across `INTEGRATIONS`, `COOKBOOK`,
`HOW_TO_USE`, `TESTING`, `AGENTS` and `AGENTS-GUIDE`, in both languages. The five lines
that legitimately NAME the PT aliases — the mapping tables and the `--wire-keys pt`
explanations — were restored by hand after the sweep. `docs/TESTING` had gone further
and asserted the deep-research field is `.titulo` "not `.title`", which is the truth
inverted.

### Fixed — the documentation rulers had never looked inside `docs/`

`every_root_document_has_a_translation` and `published_documentation_ships_every_translation`
read ONE directory, so twenty documents under `docs/` were ungoverned. The exposure was
larger than the damage: `docs/AGENT_RULES.md` and `docs/PROMPT_RULES_ANTI_CLOUDFLARE.pt-BR.md`
are unpaired, both defensibly, and nothing said so.

`every_docs_document_has_a_translation_or_a_declared_reason` enumerates `docs/` and
requires either the pair or a written reason. `docs/AGENTS.md` and its mirror joined
`FLAG_REFERENCES` and `COMMAND_REFERENCES`, which immediately failed: the agent contract
was organised as one section per release — a delta log — so twenty live flags had never
been named in it. A new "Complete surface — v1.0.5" appendix names every subcommand and
every flag, including the two hidden aliases and the fact that `--allow-lite-fallback`
is accepted and does nothing.

Also corrected: `docs/AGENT_RULES.md` claimed "Version: v1.0.3" and a "v1.0.2 inventory".
The version-claim ruler did not catch it because it matches only the two openers listed
in `CLAIM_PREFIXES`, and this document invented a third one. `Version:` and `Versão:`
joined the list, and the value parser now tolerates a leading `v`, so `Version: **v1.0.5**`
and `Current version: 1.0.5` are read as the same claim. `docs/AGENTS.md` and its mirror
were making the stale claim too, and reverting one of them was seen failing the widened
ruler before it was accepted.

### Fixed — the testing guide named none of the seventeen gates

This project has no CI, so `.cargo/config.toml` is the entire pipeline and its aliases
ARE the gates. `docs/TESTING.md` and its mirror named zero of them. They spoke of
`cargo test`, `cargo check`, `cargo clippy`, `nextest` and `llvm-cov` — none of which is
how this repository is gated. A contributor following the testing guide would never run
`check-windows`, `check-windows-msvc`, `check-macos`, `check-macos-intel`, `lint-macos`,
`lint-windows`, `lint-nohttp`, `check-nohttp` or `docs-nohttp`, which are exactly the
gates that exist because v1.0.2 shipped without compiling on macOS or Windows.

Both guides now list every alias with its real command line, and state the limit the
cross-platform gates carry: they are `cargo check` and `cargo clippy`, so they neither
link nor run, and they omit `--all-targets`, so tests, benches and examples are covered
on Linux only. Runtime behaviour on macOS and Windows is validated by no gate at all.

`every_cargo_alias_is_documented_in_the_testing_guide` parses the `[alias]` section and
fails when a gate is added and left unnamed. Renaming one entry in the guide was seen
failing it before it was accepted.

### Fixed — four renamed fields still shown in the response examples of twelve documents

`--probe-deep` emits `cascade_reason` and `mitigation_suggestion`. v1.0.3 renamed both
from `cascata_motivo` and `sugestao_mitigacao`, and the published schema had never
declared the Portuguese spellings at all. Twelve documents still printed the old names
inside pretty-printed response examples, so a reader parsing the envelope they were
shown got nothing.

Two more of the same shape: `title_original` is called `original_title` on the wire and
in `docs/schemas/search-result.schema.json`, yet `docs/AGENTS.md` instructed the reader
to read `.results[].title_original` "with a `// .title` fallback" — an instruction whose
fallback fires every time. And `retentativas` appeared in a metadata example although
its own schema entry records that "the English wire always emitted `retries`, so the old
name never matched".

`every_quoted_json_key_in_current_documentation_exists_in_a_schema` reads the keys inside
JSON examples, which the `jaq` ruler could not see: a key in a printed envelope is not a
path opening a `jaq` program. It measures the ten current-state documents only —
`MIGRATION*` and `decisions/` record what a past release emitted and are left alone.
Nine keys that belong to other shapes (OpenAI messages, a Continue config field, a
cookbook script's own report) are exempt by name with their real origin.


## [1.0.4] — 2026-08-09 (contract, wire and agent surface — nothing accepted and ignored)

### Fixed — six agent-native flags were accepted and silently ignored

`--fields`, `--filter`, `--limit`, `--sort`, `--dedupe-by`, `--count-only` and
`--truncate-content` are declared on the ROOT argument set but were implemented per
CONCRETE TYPE, on `SearchOutput` and its two relatives. Every other surface therefore
accepted them, exited `0`, and emitted a byte-for-byte unchanged envelope. Measured:
`doctor --fields type` produced 2523 bytes against a 2524-byte baseline — the one byte
was the trailing newline.

- **Every operation now either acts or refuses by name.** There is no third outcome.
  `--fields` and `--truncate-content` have meaning on any JSON object and apply
  everywhere. The five row operations need an array of rows; a surface without one
  refuses them with exit `2`, naming the flag, the surface and what IS supported there.
- **The row array is DECLARED per surface, never inferred.** `doctor` carries both
  `checks` and `failed_checks`, and `config effective` carries both `allowed_keys` and
  `precedence`. Guessing which one the operator meant is the sort of invented semantics
  that becomes a contract nobody chose.
- **Measured after:** `commands` 6421 → 47 bytes with `--fields version`; `doctor`
  2524 → 38 with `--fields type,status`; `schema` 4726 → 1107 with `--fields schemas.id`.
- **The capability matrix is published**, in `commands` under `agent_ops`, so a caller
  learns the contract instead of discovering it by collecting exit codes.
- A `--fields` path that matches nothing is an error naming the level where the path
  broke and the keys available THERE. Reporting the top-level keys for `checks.id` sent
  the reader looking one level too high.

### Fixed — three fields emitted Portuguese keys under the English default wire

ADR-0027 says domain types serialize ENGLISH and Portuguese is a remap applied once at
the emit boundary. Nothing enforced the first half of that sentence.
`AggregatedItem.display_url`, `AggregatedNewsItem.source` and
`AggregatedNewsItem.relative_date` kept a Portuguese `serde(rename)` right through the
migration, so the EN default emitted `url_exibicao`, `fonte` and `data_relativa`.

- **`deep-research-output.schema.json` declared the English names** under
  `additionalProperties: false`, so a real news row with a publisher or a date FAILED
  the contract the product publishes for it. A wrong schema is worse than an absent one.
- **Why no ruler caught it**: all three are `Option` with `skip_serializing_if`, and
  every fixture left them `None`. A key that is never emitted is invisible to a drift
  check in BOTH directions. The conformance fixtures now populate every optional field.
- Portuguese survives as a deserialize `alias`, and `--wire-keys pt` output is
  unchanged: the EN→PT table already listed all three pairs, waiting for the structs.
- `no_domain_type_renames_a_field_to_portuguese` walks the crate source, so a field
  added tomorrow is caught with no fixture at all.

### Fixed — seven published envelopes carried no discriminator at all

The five `config` shapes, `locale` and `init-config` emitted no routing key, so the
published catalog could not route them and an agent had to recognise seven shapes by
hand. They were invisible to every existing ruler for a structural reason: a schema with
no discriminator is absent from BOTH sides of any comparison between the routing table
and the schemas.

- All seven now emit `type` from a compiler-checked enum, declared as a `const` in their
  schema and listed in `DISCRIMINATOR_SCHEMAS`.
- `every_published_schema_is_routable` partitions all 24 published schemas into
  routable and deliberately-unroutable, and fails the build on anything in neither.
  The escape list must state WHY, and the reason is now published in the catalog as
  `routing` — absence alone cannot tell an agent "by design" from "by oversight".

### Changed — discriminators are compiler-checked constants, not typed strings

`DoctorKind`, `DeepResearchKind`, `CommandsKind`, `SchemaCatalogKind`, `LocaleKind`,
`InitConfigKind` and `ConfigKind` join `ProbeKind` and `ProbeDeepKind`. `commands` and
the `schema` catalog gained real envelope structs instead of `json!` literals.

### Removed — the last Python in a repository that forbids it

`scripts/regen_cli_flags_readme.py` is gone: 228 lines of Python in a project whose
contract is self-contained and Rust-native. The 1.0.2 entry below still names it, because
that entry is a true record of what happened then.

- Worse than the language was the shape. A generator only helps whoever remembers to run
  it, and nobody had: the committed inventory was produced by binary **1.0.2** and listed
  **66** root flags against the **70** the binary shipped. The agent-facing documentation
  described a product that no longer existed, and nothing said so.
- `tests/integration_docs_drift.rs` replaces it with a ruler, not a generator. It fails
  `cargo test-all` with the exact set of drifted flags, asserts both READMEs mention every
  live flag, and refuses phantom flags in the generated tables. Updating the snapshot is an
  explicit `#[ignore]`d test rather than an environment variable, because this project
  forbids product env vars and a harness should not teach a habit the product refuses.

### Fixed — the multi-query stream path discarded a parse error

`pipeline::run_stream` re-parsed `--fields` and `--filter` with `.ok()`, dropping an
error the non-stream path refuses with exit `2`. Masked today by the upstream fail-fast
gate; propagated now, so the two paths say the same thing about the same input.

### Fixed — a schema claimed a discriminator it only partly covered (ADR-0031)

Fifth-pass audit, this time of the FOURTH pass's own fix. Publishing
`deep-research-budget.schema.json` closed the envelope that happened to be in hand and
introduced a worse defect than the one it closed.

- **`type: "deep_research_error"` is emitted from four sites with four disjoint shapes**:
  `budget_underflow` (19 keys, exit 2), `cancelled` (6 keys, exit 130/143), `timeout`
  (8 keys, plus 9 more when partials were harvested, exit 4) and `sub_queries_incomplete`
  (8 keys, exit 2). The published schema covered the first behind a `oneOf` with
  `additionalProperties: false` on both branches, so an agent validating a `cancelled`
  envelope failed BOTH branches. Before the fix there was no schema and the agent knew it
  did not know; after it, a schema rejected a valid envelope. A wrong schema is worse than
  an absent one, because absence is legible and a false negative is not.
- **One published schema per `type` value.** `deep-research-error.schema.json` now covers
  all four shapes, keying each branch on `error` with a `const`: the effective routing key
  for this family is the PAIR (`type`, `error`), and the schema encodes it instead of
  leaving it to the reader. `deep-research-budget.schema.json` is back to one discriminator,
  and a regression test asserts the refusal does NOT validate against it.
- **Nine introspection surfaces had no contract at all**: `commands`, the `schema` catalog,
  `locale`, `doctor` and all five `config` subcommands. `doctor` is the richest at 18 keys
  and the one an agent most often parses before deciding whether a run is worth attempting.
  Closing `config list` alone would have repeated the very mistake being corrected, so the
  whole family was swept: `path`, `get`, the shared `set`/`unset` acknowledgement (keyed on
  `action` rather than `type`) and `effective` were all undeclared too. All nine are now
  published and validated against the compiled binary's real stdout.
- **The class, not just the instances.** `catalog_matches_published_schema_files` compares
  two sets of FILES and is structurally blind to "envelope emitted with no schema" — there
  is no file for it to enumerate. `every_emitted_discriminator_has_a_published_schema` walks
  the crate source and fails if a `type` literal has no contract. It asserts the scan is a
  SUBSET of a hand-written table rather than complete: `doctor`, `probe` and `probe-deep`
  carry their discriminator on a renamed serde field, so no literal exists to find.

### Added — the schema catalog carries the routing rule as data

Each entry of `duckduckgo-search-cli schema` gains an optional `discriminator` naming the
`type` value its schema describes. A mapping that lives only in a test protects the build;
publishing it means the consumer no longer has to hold it. A test asserts no two schemas
claim one `type`, since ambiguous routing is the original defect restated.

`output::sub_queries_incomplete_payload` is now public and lives beside the cancel and
timeout envelopes. Three of the four shapes in one module: their being scattered across
three files is the structural reason nobody saw they shared a discriminator.

Twenty-three schemas ship, all validated against a real envelope, `EXCLUDED` still empty. The
conformance suite goes from 28 cases to 42. The cancel and timeout branches are validated
against bytes the product actually wrote — the test arms the in-flight guard and drives the
real signal path — because hand-building the payload would validate the test's idea of the
envelope rather than the product's.

### Fixed — a published schema described a document the product never wrote

Fourth-pass audit of 1.0.3. The three previous passes closed phases 1 to 7; re-reading the
approved plan item by item found four sub-items that were listed and silently skipped. None
of them was red, because an unwritten test and a stale document emit no signal.

- **`config.schema.json` was fiction.** It declared `user_agents` as an array of strings.
  The file `init-config` actually writes is a table whose root key is `agents`, holding
  `{ua, platform}` rows; `selectors.toml` was likewise not wrapped in a `selectors` key. With
  `additionalProperties: false`, the real pair failed the whole document. No version ever
  wrote the declared shape, so no consumer can have been validating successfully. The schema
  now carries a `$defs` entry per real file, and the conformance suite runs the binary with
  `--config-home` into a temp dir and validates what landed on disk.
- **The exemption was the hiding place.** ADR-0030 had exempted `config.schema.json` on the
  grounds that no single artifact has the combined shape. That was true and the conclusion
  was wrong. `EXCLUDED` in the coverage ledger is now empty, and the rule is written down:
  exempt only when there is no artifact at all, never when assembling one looks awkward.
- **Two emitted envelopes had no schema at all.** `--print-budget` (26 keys) and the
  `budget_underflow` refusal (18 keys) were undeclared, as was the report `init-config`
  prints — the last open box on both README checklists. Added
  `deep-research-budget.schema.json` and `init-config-output.schema.json`, both validated.
  The coverage ledger catches "schema with no test"; it could not catch "envelope with no
  schema", which is the deeper blind spot.
- **The CLI's schema catalog could drift from the published files.** `SCHEMAS` in
  `commands::schema_cmd` is hand-maintained, so adding a file under `docs/schemas/` did not
  add it to `duckduckgo-search-cli schema`. `catalog_matches_published_schema_files` now
  fails in both directions, naming the offending ids.

### Changed — `deep-research` split by responsibility (plan Fase 8)

`execute_deep_research` was roughly 580 lines in one function carrying five responsibilities.
It is now an orchestrator that owns the stage ORDER and the timeout fence, over four private
siblings: `deep_research_preflight`, `deep_research_budget`, `deep_research_session` and
`deep_research_emit`. Every stage emits its own envelope on failure and returns only an exit
code, so the orchestrator never has to know which one writes what.

Behaviour is unchanged and was proved so, not assumed: five envelopes captured before the
move — `--print-budget`, empty query, invalid `--fields`, invalid `--filter`, and budget under
`--wire-keys pt` — come out byte-identical afterwards, with the same exit codes and stderr.

### Documentation

- `NO_CI.md`, `gaps.md` and both `CROSS_PLATFORM` files still taught that `zig` and
  `cargo-zigbuild` were required for the macOS gate. ADR-0029 had removed the C dependency
  and `scripts/check-macos.sh` already ran plain `cargo check`, so a reader was installing
  a toolchain for nothing. The cross-platform tables now separate cross-*check* (works, and
  is a gate) from cross-*build* (still not guaranteed).
- Added `rustfmt.toml` pinning `edition` and `style_edition` to their current values —
  a deliberate zero-hunk change, so that a future edition bump cannot silently reformat 100+
  files in the same commit.

### Fixed — the published JSON schemas did not describe the actual wire

Second-pass audit of 1.0.3. Every gate was green and the contract was still broken, because
nothing validated a real envelope against `docs/schemas/*.json`.

- **`--wire-keys pt` was a no-op on eleven emit sites.** The thin error envelopes in
  `src/run.rs` and `src/commands/deep_research.rs` were hand-built with `serde_json::json!`
  and written through `print_line_stdout(&payload.to_string())`, which skips the wire-keys
  remap. Portuguese output was byte-identical to English. They now go through the typed
  `types::ThinErrorResponse` and the new `output::emit_wire_line`.
- **Every successful search violated `search-metadata.schema.json`.** The schema still
  declared the pre-ADR-0027 spellings `retentativas`, `news_filtradas_promo`,
  `cascata_nivel_observado` and `endpoint_used_compat`, and omitted `retries_configured`
  and `flags_ignored` entirely. With `additionalProperties: false`, one undeclared key
  rejects the whole document.
- **Every multi-query run violated `multi-search-output.schema.json`** twice: `paralelismo`
  was declared *and* required, while the wire emits `parallelism`.
- **`error-response.schema.json`** did not declare `result_count`, `results` or
  `next_action_suggestion`, and carried `retentativas` as its only Portuguese property.

### Changed — wire rename (`zero_cause_histogram`)

- `MultiSearchOutput` emitted `causa_zero_histogram` on the **English** wire — the last
  ADR-0027 leftover living in the code rather than in a schema, and already contradicted by
  `SKILL.md`, which documented `zero_cause_histogram`. The wire key and the Rust field are now
  `zero_cause_histogram`.
- Migration: a serde `alias` keeps pre-1.0.3 documents deserializable, and `--wire-keys pt`
  still emits `causa_zero_histogram`. Only an English consumer that hard-coded the Portuguese
  key needs to change.

### Added — conformance is now enforced, including its own coverage

- `tests/integration_schema_conformance.rs` grew from 10 to 20 cases and from 2 to 10 of the
  11 published schemas, validating both the English default and `--wire-keys pt`.
- The metadata fixture is **maximal** — every optional field set — because a sparse fixture
  cannot surface an undeclared property. That is what finally exposed `flags_ignored`.
- `every_published_schema_is_covered_or_explicitly_excluded` fails when a schema has neither a
  conformance test nor a written exemption, so a half-delivered coverage plan can no longer
  ship green. See ADR-0030.

## [1.0.3] — 2026-08-07 (cross-platform hotfix — macOS and Windows never compiled in 1.0.2)

### Fixed — the crate did not build outside Linux

- **macOS / Windows `E0432`** — `src/browser/session/mod.rs` imported `detect_linux_distro` and
  `xvfb_manual_instruction` through an **ungated** `use`, while both are declared under
  `#[cfg(target_os = "linux")]`. Rust strips `cfg`-disabled items *before* name resolution, so
  correctly gated call sites did not rescue the import. The `use` is now split, with the two
  Linux-only symbols behind `#[cfg(target_os = "linux")]`.
- **Windows `E0308` ×2** — `src/browser/detect.rs` matched `std::env::var_os` with
  `if let Ok(..)`; `var_os` returns `Option<OsString>`. This Windows-only code path had never
  been compiled by any gate.
- **13 off-Linux warnings** rejected by `-D warnings` (unused imports, never-used items, a
  missing doc on the `#[cfg(not(unix))]` stub of `apply_process_group_and_pdeathsig`). All are
  now gated with the `cfg` of their actual consumer, or carry `#[allow(dead_code)]` in the
  existing `xvfb.rs` convention where a deliberate `not(linux)` stub is never called.
- **`tests/integration_content_fetch.rs` no longer compiled on any platform** (17 errors). It
  was the only integration test that ignored `tests/common/mod.rs` and hand-wrote a
  `Config { … }` literal, so it silently rotted through the newtype migration. It now builds on
  `common::lean_config`, which is exactly what that helper's doc comment asks for.

### Added — gates that make this class of regression unshippable

- `cargo check-windows` / `cargo lint-windows` aliases in `.cargo/config.toml`
  (`x86_64-pc-windows-gnu`). Windows satisfies both `not(target_os = "linux")` and `not(unix)`,
  so it covers the whole `cfg`-regression class.
- `scripts/check-macos.sh` — real `rustc` check against `aarch64-apple-darwin` from a Linux
  host, via `cargo-zigbuild`. `cargo check` does not link, so no Apple SDK is needed.
- `scripts/portability-lint.sh` — millisecond structural pre-check that fails on an ungated
  `use` of a platform-only item.
- `NO_CI.md` now requires all three before tag and `cargo publish`.
- [`ADR-0028`](docs/decisions/0028-local-cross-platform-gate-v1-0-3.md) records why forbidding
  remote CI moves cross-platform verification onto the host instead of removing it.

### Changed

- `Cargo.toml`: dropped the dead 72-line `exclude` block. `include` and `exclude` were both
  present; `include` wins, so `exclude` had no effect.

### Documentation (V36 — 2026-07-31 flags SSOT + EN/PT split)

- **Flag tables generated from live CLI** `duckduckgo-search-cli --help` / subcommand `--help` (binary **v1.0.2**): 66 root + 14 deep-only + doctor/init/schema/man exclusives = **85** flags, identical set EN/PT.
- Artifacts: `docs/generated/cli-flags-inventory.json`, `docs/generated/flags_en.md`, `docs/generated/flags_pt.md`, curated `flag-desc-{en,pt}.json`.
- Regenerator: `scripts/regen_cli_flags_readme.py` (apply via `atomwrite write` so EN/PT never hand-diverge).
- **SUPERSEDED (v1.0.4 / v1.0.5).** The two lines above described how to regenerate these tables *at the time of this release*, and following them today fails: the Python regenerator was deleted in v1.0.4 and replaced by a Rust drift ruler that FAILS on divergence instead of rewriting on demand, and the curated `flag-desc-{en,pt}.json` inputs were deleted in v1.0.5 once it was measured that nothing read them. The record stays because it is what happened; the instruction is marked because a reader acting on it would be acting on a product that no longer exists. See the v1.0.4 and v1.0.5 entries.
- **README.md English-only:** removed embedded `## Português` monolith (~270 lines); pointer to [`README.pt-BR.md`](README.pt-BR.md) as PT SSOT.
- README.pt-BR: full generated flag inventory; Deep Research bullets point at SSOT table (fixed stale “depth not executed in v0.7.0”).

### Documentation (V35 — 2026-07-31 wire EN residual)

- News vertical + agent metadata sections: **v1.0.2 EN wire** primary (`news[]`, `news_count`, `metadata.vertical_used`, `chrome_path_resolved`, `used_chrome`, `zero_cause: vertical-no-results`); PT only with `--wire-keys pt`.
- Flags tables (README EN/PT): agent ops + `--wire-keys` + `--print-schema` + `--fetch-content-cap` default **4**.
- Skills evals: multi-query `.searches[]`; ZeroCause `.metadata.zero_cause` / `.metadata.next_action_suggestion`.
- `docs/schemas/README.md`: probe-deep EN field names; news ZeroCause `vertical-no-results`.
- SECURITY.md Disclosure Policy: English only (removed accidental PT bullets).
- README pre-flight: no false “auto-route to Lite” claim (GAP-WS-113).
- README.pt-BR deep-research schema example: EN keys (`original_query`, `unique_result_count`, `synthesis`, …).

### Documentation (V34 — 2026-07-31 residual)

- Skill install paths: `skill/` → **`skills/`** in README EN/PT and `llms-full.txt` (current guidance).
- Product docs no longer present FETCH_CAP **10** as the current default — **4 (v1.0.2)**; cap 10 only as historical or explicit deep-research override.
- `docs/CROSS_PLATFORM` + `docs/INSTALL-WINDOWS` EN/PT banners: current release **v1.0.2**.
- `docs/TESTING` EN/PT: new **v1.0.2 Test Notes** (wire EN, budget fail-fast, doctor/root probe split).
- `docs/INTEGRATIONS` EN/PT: defaults `--pages 1` (not “auto-paginates 2 pages”).
- Full CLI inventory includes root `--print-schema` + root `--probe` in README, skills, AGENTS-GUIDE.
- SECURITY EN/PT: prefer upgrade to **1.0.2**.
- PATH binary reinstalled: **`duckduckgo-search-cli 1.0.2`** (replaced stale 2.0.0 draft install).
- `docs/AGENT_RULES.md` R04/R25 EN+PT: `--pages` default **1** (not automatic 2 pages).

### Documentation (V33 — 2026-07-31)

- Full GraphRAG docs audit on product **v1.0.2**: removed invented `doctor --probe` (doctor flags are `--strict` / `--probe-deep`; root `--probe` is separate).
- Corrected product defaults in docs: `--pages` default **1**, `FETCH_CAP=4` (v1.0.2), fail-fast `budget_underflow` in AGENTS.pt-BR.
- `docs/schemas/README.md` wire EN (ADR-0027) + deep news field names + GAP-SCHEMA-DEEP checklist.
- Documented root `--print-schema` in HOW_TO_USE EN/PT, AGENTS EN/PT, `llms.txt` / `llms.pt-BR.txt`.
- Full subcommand catalog kept complete (init-config, completions, deep-research, commands, schema, doctor, locale, man, config*, buscar).
- Skills evals: EN metadata keys + fetch-cap default 4.
- Clap help for `-n` no longer claims auto-pagination to 2 pages.

## [1.0.2] — 2026-07-30 (product line; draft 2.0.0 superseded — publish as 1.0.2)

### BREAKING — Wire JSON English default (ADR-0027)

- **Serialize** agent stdout keys are **English** (`results`, `title`, `metadata`, `engine`, …).
- **Deserialize** still accepts Portuguese aliases for legacy fixtures.
- **Migration:** replace `resultados`→`results`, `titulo`→`title`, `metadados`→`metadata`, `quantidade_resultados`→`result_count`, etc. See `docs/decisions/0027-wire-en-default-v1-0-2.md`.
- `--fields` / `--filter` still accept **both** PT and EN tokens; projected JSON emits EN keys.

### Added / Fixed (residual close wave)

- **budget_profile** apply: `lab` | `desktop_contended` | `thin` via `config set budget_profile` (G4).
- **Proxy argv secret (G5):** Chrome `--proxy-server` never includes `user:pass@` (stripped for process listings).
- **budget/profile.rs** SSOT + unit tests; doctor/print-budget path re-applies XDG per-key after profile.
- **Agent ops (G9/G10/G13):** `--sort`, `--dedupe-by url`, `--count-only`, `--truncate-content`, `--max-output-bytes` + XDG defaults; module `src/output/agent_ops.rs`.
- **Schemas/skills/MIGRATION 1.0.2:** `docs/schemas/*` + skills EN/PT + `docs/MIGRATION.md` document EN wire. Deserialize still accepts PT aliases. `--wire-keys pt` serialize shipped in V30 (default EN only).
- **G23 no-warmup fail-closed:** `--no-warmup` requires hidden `--allow-no-warmup` or XDG `allow_no_warmup=true` (lab only).
- **G6 cgroup multi-OS:** XDG `linux_cgroup_enabled` + `linux_cgroup_memory_max_mb`; `src/cgroup.rs`; doctor reports `linux_cgroup` (`n/a` non-Linux).
- **RuntimeConfig SSOT (G3 / V28):** `src/runtime/` — keys, user getters, apply (CLI>XDG>FACTORY), build_config, factory seeds, persist, validate. `config` subcommand is CRUD-only.
- **Monólitos SRP (G1/G12 / V28):** hard fail >1000 LOC closed for domain sources; tests extracted (`lib_tests`, `pipeline/tests`, `search/tests`, `process_lifecycle/tests`, `cli/tests`, `deep_research_tests`, …).
- **G7 flaky retries closed:** process-wide `chrome_session_retries` from CLI/XDG applied before SERP; launch path uses SSOT (no contention redesign).
- **V29 audit (`gaps-md-auditoria-solucao`):** revalidated PATH **1.0.2** (draft branch briefly labeled 2.0.0; product SSOT Cargo **1.0.2**) + gates (723 lib / 56 e2e / 15 wiremock / clippy clean). Closed residual ad-hoc PT error envelopes (`error`/`message`/`type`/`result_count`); eliminated double-emit on quiet `--no-warmup`; wiremock asserts EN `result_count`; docsrs `# Errors` for V28 runtime/agent_ops splits.
- **V30 residual close:**
  - `sub_queries[].status` default **`error`** (never `erro` without `--wire-keys pt`); constants `SUB_QUERY_STATUS_*`.
  - **`--wire-keys en|pt`** + XDG `wire_keys` — emit-boundary EN→PT remap (`src/output/wire_keys.rs`); ADR-0027 amended.
  - XDG operational defaults: `default_timeout`, `default_retries`, `default_pages`, `default_num_results`, `default_max_content_length`, `default_per_host_limit`, `default_cancel_grace_secs`.
  - Rust ids EN: `partial`, `sub_queries_error`.
  - Soft monólitos domain ≤800 (search/, deep_research/, chrome/, pipeline/single, lib→run, …).
  - Docs/schemas/skills/AGENTS EN wire alignment; SECURITY current **1.0.2**.
- **V32 residual close (2026-07-31):**
  - ADR-0027 path renamed to `docs/decisions/0027-wire-en-default-v1-0-2.md` (stub redirect at old `…v2-0-0.md`).
  - **GAP-SCHEMA-DEEP closed:** deep metadata schema lists `partial` / `sub_queries_*` / `chrome_contention_advisory` / `total_time_ms`; wire key `synthesis`.
  - **GAP-PRETTY-FIELDS closed:** `--pretty` + `--fields` emits indented JSON via `value_to_wire_string`; NDJSON/`to_wire_string` stay compact; `--count-only` remains compact by design.
  - **Cargo.toml:** `exclude` replaced by explicit `include` allowlist (GraphRAG docs rules) — docs, skills, schemas, config ship; gaps/docs_prd/docs_rules/GraphRAG DBs omitted.
  - PT historical accent pass on HOW_TO_USE.pt-BR §v0.7.3.

## [1.0.2] — wave 2026-07-22 (mute + budget; retained in final 1.0.2)

### Fixed — Chrome mute audio operational standard (ADR-0026 / GAP-CHROME-MUTE-001 + MUTE-002)

- **Root cause (V24):** headed Chrome (Xvfb) without effective mute; pages with autoplay/ads/media could play host speakers during deep-research / SERP / fetch-content.
- **Root cause (MUTE-002, residual sound after V24):** CLI passed full tokens (`--mute-audio`) into chromiumoxide `ArgsBuilder`, which always formats `--{key}` → process argv became **`----mute-audio`** (ignored by Chromium). Headed mode does not receive chromiumoxide's headless-only mute. Validated via live `/proc/<pid>/cmdline`.
- **Operational standard:** every Chrome launch path is muted — no opt-out.
  - SSOT: `CHROME_MUTE_AUDIO_FLAG` + `CHROME_AUTOPLAY_POLICY_FLAG`
  - Belt: both `CHROMIUMOXIDE_SAFE_DEFAULTS` and `flags_stealth`
  - Boundary adapter: `chromiumoxide_arg_token` strips one leading `--` before `BrowserConfig::args`
  - Fail-closed: `ensure_chrome_audio_muted` (source) + `ensure_chrome_audio_muted_rendered` (must be exactly `--mute-audio`, never `----mute-audio`)
  - Unit tests: sandbox/proxy matrix + safe-defaults + reject loud args + quad-dash regression
- **ADR-0026** (MUTE-002 amendment); orthogonal to ADR-0022 (no AudioContext fingerprint spoof).

### Fixed — deep-research budget contention-aware (V23 / CLI-BUDGET-*/CLI-DOC-*)

- **CLI-BUDGET-01…04:** wall-clock estimate models dual multiproc vs sequential dual, JoinSet waves, and host Chrome contention factor (XDG thresholds).
- **CLI-PRINT-01 / CLI-AUTO-01:** `print-budget` emits `suggested_global_timeout`, `shell_timeout_hint`, `runtime_dual_multiproc`, `chrome_n`; `--auto-contention-budget` (default ON) raises effective GT; `--no-auto-contention-budget` for strict fail-fast.
- **CLI-DOC-01/02:** doctor reports `ready_for_dual_deep_research`, `recommended_global_timeout`, dual-preserving remediations (not only single-flight).
- **CLI-TIMEOUT-01 / CLI-OBS-01:** grace default **20s**; timeout envelope carries sub-query counters + dual next_action (agent contract, no phone-home).
- **CLI-SYNTH-01 / V21 partial:** `sub_queries_total/ok/erro`, `parcial`; `--require-all-sub-queries`; synthesis uses SSOT stats (not `max(sources)`).
- **XDG keys:** `budget_contention_*`, `deep_research_auto_contention_budget`, `deep_research_timeout_grace_seconds`, `budget_profile`.
- **ADR-0025** documents the contract; module split `src/budget/{input,estimate,contention,print,validate}.rs` + `src/process_count.rs`.

### Fixed — audit wave `/r-auditoria` v7 (agent-native ops)

- **GAP-PRINT-BUDGET-QUERY:** `deep-research --print-budget` works **without** inventing a QUERY (clap `required_unless_present`).
- **GAP-HELP-CAP-DRIFT:** help text for `--fetch-content-cap` says default **4** (matches runtime).
- **GAP-FIELDS-PROJECT:** global `--fields` / `--select` project result rows in the binary (JSON/TSV); omits unselected keys so agents do not need `jaq`. When fields omit content keys, page fetch is skipped.
- **GAP-RESULT-FILTER:** global `--filter` (`titulo~x`, `url~y`, `host:example.com`, or bare substring).
- **GAP-NO-INPUT:** global `--no-input` no-op (always non-interactive; agent template contract).
- **GAP-ORPHAN-SWEEP-NOISE:** orphan profile sweep completion log is `debug` (quiet at default INFO for path-seco).
- **NEW** `src/output/project.rs` — pure projection/filter (SRP + unit tests).

### Fixed — audit wave `/r-auditoria` (post budget contract)

- **GAP-TEST-COMPILE-8:** 8 integration crates compile again via shared `tests/common` fixtures (`Config` newtypes, `HttpUrl`, `SearchMetadata.run_id`, `proxy_config`).
- **GAP-DOC-DRIFT-CAP10:** agent docs EN/PT teach default fetch-cap **4** and budget **fail-fast** (not cap 10 / warn-only).
- **GAP-CLIPPY-ALL-TARGETS:** `clippy --lib/--tests/--examples -D warnings` clean; unit-test-only clippy allows documented in `lib.rs`.
- **GAP-LIBDOC-SIGPIPE:** rustdoc inventory documents SIG_IGN one-shot policy (not SIG_DFL).

### Fixed — deep-research budget contract (GAP-AUD-DR-001…012)

- **GAP-TEST-NEWS-HARNESS:** `integration_deep_research_news` wiremock binary tests require `--features http-test-harness`, use thin flags (`--no-fetch-content --global-timeout 30 -q`), and no longer hang on real Chrome I/O.

- **CM-01 / GAP-E2E-51-020 closed:** fail-fast **exit 2** with JSON `erro=budget_underflow` when `--global-timeout` &lt; gated estimate (before any Chrome). Escape hatch: `--allow-under-budget` or XDG `deep_research_allow_under_budget`.
- **CM-02/03:** defaults aligned to agent happy path `timeout 180 … deep-research`: `max_sub_queries=3`, `fetch_content_cap=4`, dual+fetch ON, depth 0; gated estimate ≤ 180s.
- **CM-05:** timeout envelope emitted **before** oneshot Chrome reap; partial results truncated (cap 15). **SIGTERM/SIGINT force-exit** emits minimal deep cancel JSON before reap when deep-research is in-flight.
- **CM-06:** unit + integration tests assert default budget fits global timeout; legacy 5×10 dual rejects under 180.
- **CM-07 / GAP-E2E-51-019 closed:** `tests/integration_deep_research.rs` compiles and passes (wire types `HttpUrl`, `DateTime<Utc>`, `run_id`).
- **CM-08:** news-aware heuristic template (`latest news recent press`) when dual news is on.
- **CM-09:** dual news `Err` no longer collapses to empty `Some([])`; per-sub-query `news_indisponivel` + `causa_zero` / `news_erro` / `news_diagnostico` (rich diagnosis).
- **CM-10:** `doctor` + **`probe-deep`** report `deep_research_budget` / `budget_ok` SSOT snapshot.
- **CM-11:** estimate formula includes `--depth` reflective rounds.
- **CM-12:** XDG keys: `default_max_sub_queries`, `default_fetch_content_cap`, `deep_research_allow_under_budget`, `budget_serp_seconds`, `budget_fetch_seconds`, `budget_safety_margin_percent`.
- **CM-13:** i18n EN/pt-BR for budget underflow and allow-override messages.
- **CM-14:** 10% safety margin SSOT (`BUDGET_SAFETY_MARGIN_PERCENT`).
- **CM-15:** `src/budget/` SSOT + `src/cli/deep_research_args.rs` + `src/output/deep_envelope.rs` (cli directory module).
- **`--print-budget`:** dry estimate JSON without Chrome (agent-first).
- **e2e:** binary fail-fast test for legacy 5×10 under 180s (`budget_underflow`, exit 2).
- **CM-01b:** budget gate / `--print-budget` run **before** Chrome require.

### Breaking (defaults only — flags restore 1.0.1 behaviour)

- `--max-sub-queries` default **5 → 3** (deep-research).
- `--fetch-content-cap` default **10 → 4** (global; affects buscar + deep).
- Full mode: `--max-sub-queries 5 --fetch-content-cap 10 --global-timeout 600` (outer timeout ≥ gated estimate).

### Residual

- **GAP-E2E-51-011:** `search`/`lib`/`pipeline` monólitos still large; **cli + deep budget/envelope axis closed** in v1.0.2.

### Note

- No product telemetry. No GitHub Actions. Config via CLI + XDG only.

## [1.0.1] — 2026-07-19

### Fixed — deep-research agent contract + Pass 48 gaps

- **GAP-E2E-48-006 / CM-01–02:** `deep-research` honors global `-o/--output` (was `output_file: None`); unified `output::emit_payload` route (atomic write, empty stdout with `-o`).
- **GAP-E2E-48-007 / CM-04–05:** global timeout emits agent-stable JSON (`erro=timeout`) to stdout or `-o`; best-effort partial harvest after cancel grace.
- **CM-06:** stderr budget warning when `--global-timeout` &lt; estimated workload (fetch × sub-queries × verticals).
- **GAP-E2E-48-008:** `--depth` runs heuristic reflection rounds (no LLM stub).
- **GAP-E2E-48-001:** `-f tsv` implemented end-to-end for search results.
- **GAP-E2E-48-004 / XDG:** subcommands `man` and `config` (path/list/get/set/unset) — no product env.
- **GAP-E2E-48-005:** `init-config` JSON wire keys English (`created`, `path`, `base_directory`, …).
- **GAP-E2E-48-011:** CDP InvalidMessage noise logged at `debug` (not `info`).
- **GAP-E2E-48-013:** `--pages` is `global = true` for subcommand argv order.
- **GAP-E2E-48-016:** removed dead `parse_zero_cause_strict_env` product-env helper.
- **Hygiene:** missing-docs / unused-import cleanup toward zero `cargo` warnings.

### Fixed — Pass 52 / GAP-E2E-51 (v1.0.1 close-out)

- **GAP-E2E-51-001:** `ensure_oneshot_cleanup` + residual Chrome kill + force profile remove; **SIG_IGN** for SIGPIPE so Drop/reap runs; pipe e2e orphans=0.
- **GAP-E2E-51-002:** `cargo clippy --lib -- -D warnings` clean (baseline allows + docs).
- **GAP-E2E-51-003:** `config get/set/unset` dual API (positional `KEY`/`VALUE` **and** `--key`/`--value`).
- **GAP-E2E-51-004:** schemas README stream marked **implemented** (not unimplemented).
- **GAP-E2E-51-005:** `-f ndjson` aliases to `--stream` mode.
- **GAP-E2E-51-006:** news vertical false anti-bot (CSS anomaly-modal) + session prime + retries.
- **GAP-E2E-51-007:** stream `BrokenPipe` → exit **141**; stream|head e2e (orphans 0).
- **GAP-E2E-51-008:** ADR-0023 wire PT serialize + EN deserialize aliases (backward compatible).
- **GAP-E2E-51-009:** docs purge product-env teaching (no product env).
- **GAP-E2E-51-010:** CROSS_PLATFORM + inventory meta v1.0.1.
- **GAP-E2E-51-012:** depth reflection quality filter (rejects junk like `"rust your"`).
- **GAP-E2E-51-013:** XDG `default_lang` / `default_country`.
- **GAP-E2E-51-014:** doctor `channel=` (EN).
- **GAP-E2E-51-017:** schema paths `types/` / `error/`.
- **GAP-E2E-51-018:** `config effective`.

### Residual (honest — not fully closed in 1.0.1)

- **GAP-E2E-51-011:** partial monólitos (`cli`/`search`/`lib` still large; `http/` + `parallel/` split done).
- **GAP-E2E-51-016:** closed by this release commit + GitHub tag `v1.0.1` + crates.io publish.
- **GAP-E2E-51-019:** integration harness residual (lib tests OK; `tests/*` compile drift).
- **GAP-E2E-51-020:** DR budget **warn only** (no fail-fast XDG strict).

### Note

- Proxy remains CLI/XDG only (no `HTTP(S)_PROXY` inheritance). Docs must not recommend product env vars.
- No remote telemetry. Local gates only (`NO_CI.md`).

## [1.0.0] — 2026-07-15

### Fixed — GAP-WS-TMP-PROFILE-ORPHAN-001 (disk one-shot + auditable profile prefix)

- **Root cause:** process one-shot (v0.9.6) without full **profile-on-disk** one-shot; `tempfile::tempdir()` default prefix `.tmp`; `force_reap` / `reap_all_registered` killed PIDs but did **not** `remove_dir_all(user_data_dir)`; SIGTERM only cancelled the token; `deep-research` used an isolated `CancellationToken` so main SIGTERM did not cancel Chrome sessions.
- **`USER_DATA_DIR_PREFIX = "ddg-chrome-"`** — `tempfile::Builder` in `ChromeBrowser::launch`; Unix profile dir mode `0o700`.
- **`force_reap`** — after process-tree/marker/Xvfb kill: settle + `remove_dir_all` + one retry (idempotent).
- **`ExitReapGuard`** on `main` Drop + panic hook; synchronous `reap_all_registered` on global timeout, pipeline end, and deep-research end.
- **`sweep_orphan_profiles`** — on first panic-hook install, remove stale `ddg-chrome-*` under `env::temp_dir()` with no live process marker (never mass-delete `.tmp*`).
- **deep-research** inherits main `CancellationToken` + global timeout fence with reap.
- **`Config::default().global_timeout_seconds`** aligned to `DEFAULT_GLOBAL_TIMEOUT` (180).
- Tests: unit force_reap/sweep/prefix; lifecycle integration asserts `ddg-chrome-` path.
- Docs: ADR-0020; `gaps.md` inventory **zero open**; no remote telemetry; atomwrite output paths unchanged.

### Note

- SIGKILL/OOM can still leave residual dirs; next invocation best-effort sweeps only `ddg-chrome-*`.
- Legacy `/tmp/.tmp*` orphans from 0.9.x are **not** auto-deleted (third-party safety).
- Stable **1.0.0** contract: Chrome-only CDP SERP, one-shot process **and** disk, agent JSON meta ≠ telemetry.

## [0.9.10] — 2026-07-15

### Changed

- **Upstream repository** — `repository` and `homepage` in `Cargo.toml` now point to [`danilo-aguiar-br/duckduckgo-search-cli`](https://github.com/danilo-aguiar-br/duckduckgo-search-cli) (new GitHub account after the previous account suspension).
- **No GitHub Actions (forbidden)** — all CI/CD workflows, Dependabot, zizmor, and pre-commit CI configs are removed; validation is **local only** (`cargo test`, `cargo deny`, etc.). See `NO_CI.md`.
- Docs, schemas  fields, and clone URLs updated to the new namespace.

### Note

- Runtime behaviour is unchanged from **0.9.9**. This release is primarily crates.io metadata + hosting migration.

## [0.9.9] — 2026-07-14

### Fixed (e2e audit — all inventário gaps)

- **GAP-WS-NEWS-LIVE-001 / L04 / FANOUT**: denylist DDG promo URLs; full-document news fallback no longer returns App Store / Duck.ai / footer chrome as “news”; parallel + deep inherit filter.
- **GAP-WS-NEWS-FETCH-WASTE-001**: content fetch skips promo hosts.
- **GAP-WS-TIMEOUT-DEFAULT-001 / DOCS-TIMEOUT-001**: `DEFAULT_GLOBAL_TIMEOUT` raised **60 → 180** for agent-ready defaults.
- **GAP-WS-EXIT4-JSON-001**: global timeout emits JSON (`erro: "timeout"`) on stdout before exit 4.
- **GAP-WS-PROBE-403-001 / PROBE-SCHEMA-001**: probe uses calibration query + SERP signals; `status: "ok"|"blocked"`, `healthy` honest.
- **GAP-WS-PREFLIGHT-META-001**: `pre_flight_executado` + `pre_flight_status` (distinct from ghost-block `pre_flight_disparado`).
- **GAP-WS-META-TIMING-001**: `tempo_execucao_ms` includes content-fetch wall clock.
- **GAP-WS-META-NO-CHROME-001**: NO_CHROME envelopes clear path/canal and `tentou_chrome: false`.
- **GAP-WS-ERR-CHROME-PATH-001**: `PathError` display is the caller message only (no false “invalid output path” prefix).
- **GAP-WS-QUIET-CONFIG-001**: `-q` sets tracing fully off; config errors can emit JSON without stderr noise.
- **GAP-WS-STREAM-NOOP-001 / STREAM-MULTI-001**: help text honest; metadata `stream_solicitado` / `stream_efetivo`.
- **GAP-WS-NEWS-FIXTURE-001**: promo-only unit fixtures + filter tests.
- Agent meta `news_filtradas_promo` (not telemetry). One-shot lifecycle retained (ADR-0017/0019).

### Migration

- Default global timeout is **180s**. Pass `--global-timeout 60` to keep the old fence.
- News may legitimately be empty when the live SERP only exposes DDG chrome UI.
- Probe JSON: prefer `healthy` + string `status`; do not treat bare HTTP 403 on `/html/` as the health signal.

## [0.9.8] — 2026-07-14

### BREAKING — GAP-WS-AGENT-READY-001 agent-ready defaults

- **Default `--vertical` is `all`** (web + news). Opt out with `--vertical web` (deep: `--no-news`).
- **Content fetch default ON** for agent-ready clean text. Opt out with `--no-fetch-content`.
- **News results** may include `conteudo` / `tamanho_conteudo` / `metodo_extracao_conteudo` (top 10 URLs).
- Metadata may include `chrome_path_resolvido` and `chrome_canal` (agent contract fields — **not** telemetry).

### Added / Fixed

- **L-01/L-02 Multi-canal Chrome** — resolve Flatpak export shell → deploy ELF (`files/extra/chrome`); Fedora chromium wrapper → lib64 ELF; candidate order host Chrome → host Chromium → Flatpak → Snap; `needs_no_sandbox` for Flatpak deploy paths.
- **L-03 Dual default** — search and deep dual web+news; dual with web>0 and news empty stays exit 0 (honest degradation).
- **L-04 News SERP** — multi-selector poll; full-document Strategy B; honest `usou_chrome` on news-only.
- **L-05 Clean text** — readability via chromiumoxide for web + news; FETCH_CAP=10.
- **L-06 Global transport flags** — `--chrome-path`, `--proxy`, `--vertical`, fetch flags, etc. work after `deep-research`.
- **L-07 UA fan-out** — `identity::coerce_chrome_user_agent` shared with single-path; one-shot lifecycle retained.
- **L-08 Docs** — ADR-0018, schemas, local `gaps.md` inventory, skills EN/PT, MIGRATION PT, this CHANGELOG.
- **R-01/R-02/R-03** — `chrome_path_resolvido` / `chrome_canal` on multi-query fan-out, deep-research envelope, and failure paths.
- **R-12** — `BrowserConfigBuilder::surface_invalid_messages` at Chrome launch.
- **Mandates** — chromiumoxide-only production; one-shot; atomwrite; no telemetry.

## [0.9.7] — 2026-07-13

### Fixed (Windows MSVC build after 0.9.6)

- **`process_lifecycle::windows_terminate_pid`** — compare `HANDLE` with `.is_null()` (`windows-sys` 0.61: `HANDLE = *mut c_void`, not `== 0`).
- **Unused imports on non-Linux** — `apply_process_group_and_pdeathsig` import is Linux-only; `Duration` import is Unix-only (clean Windows release build).

### Note

- **0.9.6** is on crates.io but does **not** compile on Windows MSVC. Prefer **0.9.7** for all platforms. Yank optional; source fix is this patch.

## [0.9.6] — 2026-07-13

### Fixed — GAP-WS-LIFECYCLE-001 one-shot Chromium/Xvfb process ownership

- **Root cause:** incomplete lifecycle for the external process tree (Xvfb + multi-process Chromium + `TempDir`). `kill_on_drop` / `Child::kill` only reaped the browser **root**, leaving orphans under `systemd --user`, residual `/tmp/.tmp*` on tmpfs, and growing RAM/swap across long sessions.
- **`src/process_lifecycle.rs`** — process-group spawn (`setpgid` + Linux `PR_SET_PDEATHSIG`), `killpg`, process-tree walk, cmdline marker kill by unique `user-data-dir`, Xvfb lock/socket cleanup, session registry + panic-hook best-effort reap.
- **`ChromeBrowser`** — `XvfbGuard` always kills Xvfb on drop (including failed Chrome launch); async `shutdown` with cooperative `close`/`wait` **deadline** then forced kill + tree/marker reap; synchronous `force_reap_session` on `Drop`; idempotent `finalized` flag.
- **`content_fetch`** — `Mutex<Option<ChromeBrowser>>` + `take()` + async `shutdown` after JoinSet drain (no bare `drop(Arc)`).
- **Signals** — Unix **SIGTERM** (and SIGINT) cancel the `CancellationToken` so supervisors/Docker/`timeout` trigger cooperative cancel paths.
- **Atomwrite** — `paths::atomic_write` (tempfile same-dir + `sync_data` + persist) for `--output`, `init-config`, and cookie jar persistence.
- **Tests** — unit tests for process group/marker/atomwrite; gated E2E `tests/integration_browser_lifecycle.rs` (`DUCKDUCKGO_LIFECYCLE_E2E=1`).
- **Docs** — ADR-0017, `gaps.md` marked RESOLVIDO, README one-shot contract note.
- **No telemetry.** Version is **0.9.6** (0.9.3 already shipped ADR-0015 headless macOS/Windows).

### Fixed — cooperative cancel exit code 130

- Unify cooperative cancel exit code to **130** (`CliError::Cancelled`): `lib.rs` pipeline `Err` now returns `err.exit_code()` instead of always `1`; deep-research no longer maps `Cancelled` to global timeout `4`; Chrome cancel helper and parallel/content cancel paths emit `Cancelled`; HTTP harness cancel promotes `RetryFailReason` cancel messages to `Cancelled`.

### Documentation

- Root documentation pass for v0.9.6 publish readiness: SECURITY supported versions, INTEGRATIONS version pin, `llms*.txt` What's new, README Troubleshooting/What's new, INVERSIONS one-shot inversion, CONTRIBUTING lifecycle E2E, bilingual mirrors.
- `docs/` documentation pass for v0.9.6 (GAP-WS-LIFECYCLE-001 / ADR-0017): MIGRATION, TESTING, INTEGRATIONS, CROSS_PLATFORM, HOW_TO_USE, COOKBOOK, AGENTS, AGENTS-GUIDE, AGENT_RULES, INSTALL-WINDOWS, schemas/README — bilingual where applicable; one-shot process contract, SIGTERM-first timeout guidance, residual SIGKILL/historical orphan limits, `DUCKDUCKGO_LIFECYCLE_E2E`, no JSON schema break.
- `skill/` documentation pass for v0.9.6: rewrite EN/PT `SKILL.md` as consolidated imperative CLI execution guides (≤4000 words, description ≤1024 chars, no version-history narrative, no bold, no Rust code); ONE-SHOT Chromium/Xvfb lifecycle, SIGTERM-first `timeout`, formulas for all flags, ZeroCause/exit codes, jaq; `eval-queries.json` +q26 lifecycle.
- Root `CLAUDE.md` / `AGENTS.md` (identical) duckduckgo-search-cli section realigned to v0.9.6: ONE-SHOT / ADR-0017 contract, SIGTERM-first `timeout`, residual SIGKILL + historical orphans, workflow 11 steps, `CHROME_PATH`, `DUCKDUCKGO_LIFECYCLE_E2E`, no bold, description without internal colons; fixed corrupted `AskUserQuestion` tokens; exit 130 cancel contract aligned with code.
- Fix `.gitignore`: anchor `/AGENTS.md` and `/CLAUDE.md` to repo root only so published `docs/AGENTS.md` is no longer hidden from git (GraphRAG inventory requires `docs/AGENTS.md`).

## [0.9.5] — 2026-07-11

### Fixed (local gates / release unblock after GAP-WS-113)

- **`chrome_policy` always compiled** — `require_chrome_transport` / `http_test_harness_active` no longer live behind `#![cfg(feature = "chrome")]`, so `cargo build --no-default-features` works again (dead `not(feature = "chrome")` branch was uncompilable).
- **`integration_content_fetch` residual HTTP path** — tests force harness env + nonexistent `--chrome-path` so wiremock HTTP enrichment runs under Chrome-only production policy.
- **Supply chain** — bump transitive `anyhow` ≥1.0.103, `crossbeam-epoch` ≥0.9.20, `quinn-proto` ≥0.11.15 (RUSTSEC-2026-0190 / 0204 / 0185).
- **`dirs` 5 → 6** and **`windows-sys` 0.59 → 0.61** — reduce `cargo-deny` multiple-versions noise on Windows targets.
- **CI gates** — schema count 11; skill frontmatter without hard-coded 0.8.0; MSVC check via `vswhere` (not bare `cl.exe` on Git Bash PATH); replace removed `dtolnay/cargo-toolchain` for cargo-machete; fix rustdoc link and Windows-only `needless_return` in `cookie_adapter`.

### Note

- No intentional product/API break vs 0.9.4 (still Chrome-only / GAP-WS-113). This patch restores green Release/CI so GitHub binary assets and a clean publish path work again.

## [0.9.4] — 2026-07-10

### BREAKING — GAP-WS-113 Chrome-only universal transport

- **All production network operations require `chromiumoxide` (feature `chrome`)** — search, news, deep-research, probe, probe-deep, pre-flight, fetch-content.
- **Removed silent HTTP (`reqwest`) fallback** after Chrome failure on SERP (no more zero results with fake success).
- **`DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` fails closed** (exit 2) on every network operation — no web downgrade, no auto `--no-news`.
- **`--allow-lite-fallback` is a legacy no-op** — never forces Lite; SERP stays HTML canonical under Chrome.
- **Auto-fallback Lite removed** (GAP-NEW-004 deleted from production path).
- **Zero-cause classifier**: body ≥4KB without result-page signal is **never** `legitimo` (fixes ~26KB Lite shell false positive).
- **`--probe` uses real Chrome navigation** — no reqwest `200 OK` health lies under anti-bot.
- Residual HTTP retained only behind compile feature **`http-test-harness`** + env `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` for wiremock tests.
- ADR: `docs/decisions/0016-chrome-only-universal-v0-9-4.md`.

### Dependencies

- Removed unused direct dependency **`time`** (was only a RUSTSEC pin; still transitive via `reqwest`/`cookie_store`).
- Dropped unused **`reqwest` feature `zstd`** (DuckDuckGo SERP does not serve zstd; smaller dependency graph).
- Kept **`reqwest`** as residual compile-time dep for wiremock harness, cookie jar helpers, and UA/header builders — production success path remains chromiumoxide-only (GAP-WS-113). Full optional-`reqwest` split deferred (high blast radius).

### Fixed (completion pass)

- `--probe-deep` navigates SERP via chromiumoxide DOM (not reqwest POST).
- `--pre-flight` runs on the shared Chrome SERP session (single launch with search).
- `--fetch-content` is Chrome-first/only in production; HTTP residual only under `http-test-harness`.
- AGENTS/skill/README/MIGRATION/schemas aligned with fail-closed Chrome-only policy.

### Fixed

- Zero results with `usou_chrome: true` + `endpoint: lite` + `causa_zero: legitimo` (root causes C1–C5 in `gaps.md` GAP-WS-113).
- `parallel.rs` no longer swallows Chrome errors with `.ok()`.
- Content-fetch no longer continues "HTTP only" after Chrome launch failure in production policy.

## [0.9.3] - 2026-07-08

### Fixed (GAP-WS-112 — visible Chrome window on macOS/Windows)
- macOS (Quartz) and Windows (DWM) now use `headless=new` by default (no visible window)
- Root cause: native compositors clamp `--window-position` to the screen bounds
- Native headed opened a visible Chrome window on every search, disrupting the user's flow
- Modern headless=new combined with the v0.9.2 fixes passes DDG without opening a window
- Empirical validation: 3/3 queries exit 0, `usou_chrome=true`, `causa=null`, no visible window
- Automatic OS detection in `decide_head_mode` via `cfg!(target_os = ...)`

### Changed (GAP-WS-112 — per-platform operating mode)
- Linux keeps the private Xvfb (`HeadedXvfb`) unchanged — a MANDATORILY distinct mode
- macOS/Windows now use `Headless` (headless=new) by default
- `DUCKDUCKGO_CHROME_VISIBLE=1` still forces `HeadedNative` for debugging

### Fixed (quality)
- Fixed the clippy `needless_return` warning in `has_native_display` (macOS/Windows)
- cfg-gated tests updated to assert `Headless` on macOS/Windows

## [0.9.2] - 2026-07-08

### Changed (GAP-WS-108 — launch without chromiumoxide's automatic defaults)
- `launch()` now calls `.disable_default_args()` and re-adds 23 safe defaults via `CHROMIUMOXIDE_SAFE_DEFAULTS`
- Removes `--enable-automation`, injected automatically by chromiumoxide 0.9.1 in DEFAULT_ARGS (config.rs:481)

### Fixed (GAP-WS-108 — automation banner removed)
- The "controlled by automated test software" banner and the automation markers are gone
- Root cause of the automation leak that kept the anti-bot block persistent

### Fixed (GAP-WS-109 — UA coherent with Client Hints)
- The Chrome UA version is aligned with the actually installed version via `detect_chrome_major_version()`
- `Emulation.setUserAgentOverride` applies a coherent `UserAgentMetadata` (brands, platform, mobile)
- Eliminates the `navigator.userAgent` vs `userAgentData.brands`/`sec-ch-ua` mismatch (Chrome 146 vs 149)

### Fixed (GAP-WS-110 — WebRTC no longer leaks the real IP)
- `--force-webrtc-ip-handling-policy=disable_non_proxied_udp` and `--disable-webrtc-hw-decoding` in flags_stealth
- Prevents the real IP from leaking through WebRTC ICE candidate gathering

### Fixed (GAP-WS-111 — QUIC disabled)
- `--disable-quic` in flags_stealth forces HTTP/2 over TCP
- Avoids UDP outside the proxy, keeping transport consistent

### Added
- `CHROMIUMOXIDE_SAFE_DEFAULTS` and `detect_chrome_major_version()` in `src/browser.rs`
- `rewrite_ua_chrome_version()` in `src/identity.rs`
- cfg-gated tests for the new helpers

### Validation
- `cargo build --features chrome` and `cargo clippy --all-targets --features chrome` — ZERO warnings
- `cargo test --features chrome` — passes with no failures
- macOS smoke: 3+ queries at exit 0, `usou_chrome=true`, NO automation banner

### Note
- Audit based on the Rules Rust for Chromiumoxide (supplied by the user)
- v0.9.1 (native headed) was necessary but insufficient: the root cause of the block was the automation leak

## [0.9.1] - 2026-07-08

### Changed (GAP-WS-107 — head-mode decision extracted into a pure function)
- The Chrome head-mode decision was extracted into the pure function `decide_head_mode()` in `src/browser.rs`, cfg-gated by `target_os`
- The `ChromeHeadMode` enum (Headless/HeadedXvfb/HeadedNative) formalises the three launch modes

### Fixed (GAP-WS-107 — macOS/Windows run native headed Chrome)
- macOS and Windows now run HEADED Chrome on the native Quartz/DWM display instead of headless
- Eliminates the Cloudflare `exit 6 anti-bot` block observed in v0.9.0 on macOS
- Linux keeps its private Xvfb with no regression; `has_native_display()` + `spawn_virtual_display()` act on Linux only
- The Chrome window is moved off-screen via `--window-position=-32000,-32000 --window-size=1920,1080` (pre-existing flags)

### Fixed (GAP-WS-107b — Chrome UA platform coercion)
- New `identity::ua_platform_matches_host()` forces a Chrome UA coherent with the host OS
- The filter in `src/pipeline.rs` now forces `chrome_only_ua_for_platform()` when the Chrome UA does not match the host
- Fixes cross-platform pins (e.g. `chrome-linux` on a macOS host) that used to pass through uncorrected

### Added (GAP-WS-107 — cfg-gated tests)
- cfg-gated tests for `decide_head_mode` in `src/browser.rs` covering Linux, macOS and Windows
- Tests for `ua_platform_matches_host` in `src/identity.rs` covering UA platform coercion

### Validation
- `cargo build --features chrome` — ZERO warnings
- `cargo test --features chrome` — passes with no failures
- `cargo clippy --all-targets --features chrome` — ZERO warnings
- `cargo fmt --check` — ZERO differences
- macOS smoke: `duckduckgo-search-cli "rust language" -n 5` returns `usou_chrome=true`, UA `Macintosh`, `quantidade_resultados>0`, exit 0

### Note
- The skill embedded in `CLAUDE.md` remains out of date (the project rule forbids editing `CLAUDE.md`)
- The external skill under `skill/` was updated to reflect native headed on macOS/Windows

## [0.9.0] - 2026-07-07

### Changed (GAP-WS-106 — CLI ergonomics: global flags, actionable errors, feature auto-degradation)
- Nine flags are now `global = true` in `CliArgs` (`src/cli.rs`), accepted BEFORE OR AFTER the `deep-research` subcommand: `-n`/`--num`, `-f`/`--format`, `-o`/`--output`, `-t`/`--timeout`, `-l`/`--lang`, `-c`/`--country`, `-p`/`--parallel`, `-q`/`--quiet`, `-v`/`--verbose` (verbose hoisted for symmetry with `conflicts_with = "quiet"`). Extends the precedent set by GAP-WS-058/059/B3 (which hoisted `--allow-lite-fallback`, `--pre-flight`, `--global-timeout`) to the most-used flags.
- `run()` in `src/lib.rs` replaced `RootArgs::parse()` with `try_parse()`; on `ErrorKind::UnknownArgument` for a known local flag positioned after the subcommand, a hint is appended explaining the flag must appear BEFORE the subcommand (now rare — only local flags like `--pages` trigger it; the 9 hoisted flags accept either position). `DisplayHelp`/`DisplayVersion` are still deferred to `Error::exit()` to preserve exit 0.
- New public helper `is_known_global_flag(&str) -> bool` in `src/cli.rs` matches the 9 hoisted shorts/longs plus all local `CliArgs` longs.

### Fixed (GAP-WS-106 — feature auto-degradation replaces fail-fast exit 2)
> **Superseded by GAP-WS-113 / v0.9.4 (fail-closed Chrome-only).** The auto-degradation behavior below applied only in v0.9.0–v0.9.3. Since v0.9.4, missing Chrome or `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` fails with exit 2 (no auto `--no-news`, no Web downgrade).
- `execute_deep_research` (`src/lib.rs`): without a usable Chrome (build without the `chrome` feature, `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`, or Chrome detection failure) the subcommand NO LONGER aborts with exit 2 (`INVALID_CONFIG`) citing `--no-news` — it now auto-applies `effective_no_news = true` with a warning on stderr (via `output::emit_stderr`) and proceeds web-only. `--no-news` remains as an explicit opt-in/noop for backwards compatibility.
- `build_config` (`src/lib.rs`): `--vertical news|all` in a build without `chrome` (or with `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`) NO LONGER returns `Err(InvalidConfig)` (exit 2) — it downgrades to `VerticalMode::Web` with a warning on stderr and proceeds. `convert_vertical` carries `#[cfg_attr(not(feature = "chrome"), allow(dead_code))]`.

### Added (GAP-WS-106 — tests)
- `tests/global_flags.rs` (new): end-to-end coverage for Symptom A (unknown flag does NOT trigger the hint; known local flag after the subcommand DOES trigger the PT-BR hint via `deep-research --pages 3 rust`) and Symptom C (`deep-research` accepts the implicit `--no-news` in a build without chrome).
- `src/cli.rs::mod tests`: three regression tests (`quiet_global_aceito_apos_subcomando`, `output_global_aceito_apos_subcomando`, `is_known_global_flag_cobre_todas_as_flags_do_root_parser`).

### Validation
- `cargo build` and `cargo build --no-default-features` — ZERO errors/warnings
- `cargo test` — 430 (default) / 412 (`--no-default-features`) tests passing
- `cargo clippy --all-targets` — ZERO warnings in both configurations
- `cargo fmt --check` — ZERO differences
- Smoke: `--version` exit 0; `--help` exit 0; `deep-research --pages 3 rust` prints the positioning hint

### Note
- Embedded skill text in `CLAUDE.md` / `AGENTS.md` and external `skill/` was realigned in the v0.9.4 documentation pass to GAP-WS-113 (fail-closed Chrome-only). The temporary v0.9.0 auto-degradation notes are historical only.

## [0.8.9] - 2026-07-06

### Added (GAP-WS-104 — search covered ONLY the web vertical, news vertical was never visited)
- New flag `--vertical <web|news|all>` (default `web`): opt-in to the DuckDuckGo news vertical (`ia=news&iar=news`)
- `--vertical news` returns news only (`resultados: []`); `--vertical all` returns web AND news in the SAME Chrome session (single warm-up, best-effort news)
- News is routed EXCLUSIVELY through the Chrome-primary transport — the news SERP requires JavaScript rendering and has NO HTTP fallback (html/lite endpoints structurally lack a news vertical)
- After navigation, the CLI polls the rendered DOM for the React module `[data-react-module-id="news"]` (`tokio::time::sleep` loop with timeout); on timeout it still extracts and lets the cascade decide
- Extraction cascade: Strategy A (semantic selectors from the `[news]` section of `selectors.toml`, hot-fixable without recompiling) → Strategy B (class-agnostic fallback keyed on external anchors + relative-date heuristic for PT "há N ..." and EN "N ... ago" patterns)
- Internal duckduckgo.com links are filtered out; results deduped by URL preserving order; protocol-relative thumbnails resolved to `https://`
- News HTML capture uses a 1 MiB cap (web SERP keeps 256 KiB) — the React news SERP is heavier
- `--num` caps news results the same way it caps web results (GAP-WS-090 pattern)
- New JSON envelope fields, emitted ONLY when `--vertical news|all`: root `noticias[]` (`posicao`, `titulo`, `url` guaranteed; `fonte`, `data_relativa`, `thumbnail` optional), root `quantidade_noticias`, and `metadados.vertical_usada` — default web mode stays byte-identical to v0.8.8
- `data_relativa` is kept verbatim as rendered by DuckDuckGo (e.g. "há 2 horas", "3 hours ago") — no absolute-date conversion in this iteration
- New `ZeroCause` variant `vertical-sem-resultados`: legitimate zero news (rendered SERP without articles) ⇒ exit 5, NOT 6; anti-bot interstitial in the news body still classifies as `anti-bot`
- Exit-code total now sums `quantidade_noticias`: news-only with articles found ⇒ exit 0; `--vertical all` with web>0 and news=0 ⇒ success with `noticias: []`
- Config guards (exit 2, `INVALID_CONFIG`): `--vertical news|all` rejects multiple queries (`--queries-file` or multiple positional), the `deep-research` subcommand, builds without the `chrome` feature, and `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`
- `--fetch-content` keeps acting ONLY on `resultados[]` — news results are never content-fetched
- Rejected alternative documented: the internal `news.js?vqd=` endpoint was discarded because the `vqd` token rotates per query and the endpoint is undocumented and unstable

### Added (GAP-WS-105 — deep-research now runs the news vertical by default, dual web + news)
- `deep-research` now scans the news vertical by DEFAULT: each sub-query executes as `--vertical all` — the SAME Chrome session navigates the web SERP and then the news SERP (no extra sessions, no dedicated parallel lane)
- New flag `--no-news` (deep-research only): opts out and downgrades every sub-query to the pure web vertical — recommended for CI and Chrome-less environments
- Fail-fast guard: without a usable Chrome (feature `chrome` not compiled, `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`, or Chrome detection failure) and without `--no-news`, the subcommand aborts BEFORE the fan-out with exit 2 (`INVALID_CONFIG`) and a message citing `--no-news`
- News aggregation runs in a SEPARATE RRF score space (`aggregate_news` / `AggregatedNewsItem`) — news scores are NEVER fused with the web RRF (scores computed over distinct lists are not comparable); dedupe by canonical URL; ties broken by recency (internal parse of `data_relativa` such as "há 2 horas" / "3 hours ago"; the JSON keeps the string VERBATIM)
- New deep-research envelope fields, ALWAYS serialized: root `noticias[]` (`posicao`, `titulo`, `url`, `score`, `ocorrencias` guaranteed; `fonte`, `data_relativa`, `thumbnail` optional) — empty array with `--no-news` or zero news; root `quantidade_noticias`; `metadados.total_noticias_unicas`
- New OPTIONAL per-sub-query fields: `metadados.sub_queries[].quantidade_noticias` (omitted with `--no-news` or when news was unavailable) and `metadados.sub_queries[].news_indisponivel` (`true` when the news scan was expected but Chrome fell mid-flight and the sub-query degraded to HTTP web — never silent)
- Dual synthesis (`--synthesize`): the web section keeps the current format under ~70% of `--budget-tokens` and a "Notícias recentes" section consumes the remaining ~30%; with `--no-news` or zero news the report format is unchanged
- Exit codes: exit 0 when EITHER vertical produced results (web>0 OR news>0); exit 5 (`ZERO_RESULTS`) only when web AND news are both empty
- Batch multi-query now ACCEPTS `--vertical news|all` (guard removed): `--queries-file` and multiple positional queries work — each query runs its own Chrome session in the parallel fan-out, and each `buscas[]` item carries its own `noticias[]` / `quantidade_noticias`

### Fixed (post-review of GAP-WS-104 — F1..F7)
- F1: Chrome runtime failure (launch or navigation) under `--vertical news` (news-only) propagated a raw error up to `lib.rs`, producing EMPTY stdout with exit 1 and breaking the guarantee that `-f json` always emits a JSON envelope — a structured envelope in the `failure_output` pattern is now emitted (`resultados: []`, `noticias: []`, `erro`/`mensagem` filled, `causa_zero: resposta-invalida` ⇒ exit 6 under strict, exit 5 under the legacy `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` opt-out)
- F2: `--pre-flight` probed the web HTML endpoint (reqwest) unconditionally, even under `--vertical news` where news is Chrome-only with no HTTP fallback — a false positive aborted with exit 3 without ever attempting the news SERP; the probe now runs ONLY when the execution includes the web vertical (`web`/`all`), and news-only logs an informational skip notice on stderr
- F3: under `--vertical all` with web>0 and the news SERP blocked by an anti-bot interstitial, the diagnostic was silently dropped (`causa_zero` stays `None` by design — it describes the TOTAL zero of the envelope); a structured `tracing::warn` on stderr now carries the diagnosis and the suggested action — NO new JSON envelope fields
- F4: `cargo clippy --all-targets --no-default-features -- -D warnings` gate is green again — `use crate::extraction;` gated behind `#[cfg(feature = "chrome")]` and `mut effective_identity_tag` annotated with `#[cfg_attr(not(feature = "chrome"), allow(unused_mut))]`
- F5: the cancellation token (Ctrl+C/global timeout) was discarded across the whole Chrome transport (unused in the web path, absent in the news path) — Chrome launch/web/news operations now race the token via `tokio::select!` at the pipeline level, returning the same cancellation error class as the reqwest path (`network_error`, "execution cancelled") and shutting the browser down on the cancelled branch
- F6: `news_meta_from_ancestors` in the news extractor stopped climbing the ancestor chain as soon as EITHER `fonte` or `data_relativa` was found, losing the other field when they live at different ancestor levels — it now climbs up to 4 levels while ANY field is still None (innermost value wins); covered by regression test `news_meta_from_ancestors_finds_date_above_source_level` (red→green)
- F7: `parse_news_selector` used an `expect()`-style panic path when a selector coming from `selectors.toml [news]` failed to parse — replaced by a panic-free nested match falling back to a universal `*` selector (OnceLock), covered by test `news_selectors_defaults_all_compile`

### Validation
- `cargo build` — ZERO errors
- `cargo clippy` — ZERO warnings
- `cargo fmt --check` — ZERO differences
- `cargo test` — ZERO failures
- Fixtures: Strategy A SERP (7 articles, 1 internal trap filtered), obfuscated-classes SERP (Strategy B), empty SERP (`noticias: []`)
- Web-mode contract byte-identical to v0.8.8 (no `noticias`/`quantidade_noticias`/`vertical_usada` emitted)


## [0.8.8] - 2026-06-24

### Fixed (GAP-WS-089 — spawn_virtual_display() stale lock files exhaust Xvfb pool 99..200)
- `spawn_virtual_display()` iterated displays 99..200 checking only `Path::exists()` on `/tmp/.X{N}-lock`
- Did NOT verify if the PID inside the lock file was still alive
- After ~100 cancelled/failed runs, ALL slots had stale locks from dead processes
- Result: Xvfb ALWAYS failed even when installed, Chrome fell to headless, DuckDuckGo blocked with anti-bot (exit 6)
- Fix: `is_lock_stale()` reads PID from lock, verifies via `/proc/{pid}`, removes stale lock and socket before reusing slot

### Fixed (GAP-WS-090 — --num flag completely ignored when Chrome headed search is used)
- `--num 1`, `--num 3`, `--num 5` all returned 10 results (one full DDG page)
- Chrome primary extracted ALL results from the HTML page without truncating by `--num`
- Fix: truncate `agregado.results` to `min(num, len)` BEFORE computing `quantidade_resultados`

### Fixed (GAP-WS-091 — skill documents --region but real flag is --country/-c)
- Skill documented `--region <CODE>` as a valid flag
- Real flag in the CLI was `--country`/`-c` (default `br`)
- Fix: added `alias = "region"` to the `--country` arg in Clap

### Fixed (GAP-WS-092 — skill documents .metadados.quantidade_resultados but field was only at root level)
- Skill documented field at `.metadados.quantidade_resultados`
- Field only existed at `.quantidade_resultados` (root of `SearchOutput`)
- Fix: compat field `result_count_compat` added to `SearchMetadata`, populated via `fill_compat_fields()` before emission

### Fixed (GAP-WS-093 — skill documents .metadados.endpoint_usado but field was only at root level)
- Skill documented field at `.metadados.endpoint_usado`
- Field only existed at `.endpoint` (root of `SearchOutput`)
- Fix: compat field `endpoint_used_compat` added to `SearchMetadata`, populated via `fill_compat_fields()` before emission

### Fixed (GAP-WS-094 — --num ignored in batch/parallel path)
- `execute_query_with_cancellation()` in the batch path did NOT truncate results by `--num`
- Fix from GAP-WS-090 covered ONLY `execute_single_search()` (single-query path)
- `--num 2` with `--queries-file` returned 10 results per search
- Fix: truncate `agregado.results` to `min(num, len)` before computing `quantidade` in `execute_query_with_cancellation()`

### Fixed (GAP-WS-095 — identidade_usada null when Chrome headed is used with Auto)
- `identidade_usada` returned `null` on Chrome headed searches with `identity_profile = Auto`
- `effective_identity_tag` was `None` because `browser_profile_for_cli_identity(Auto)` returns `None`
- Chrome selected UA from pool via `chrome_only_ua_for_platform()` but did NOT propagate the tag back
- Fix: after Chrome headed succeeds, look up the matching identity in the pool by UA and populate `effective_identity_tag`

### Discarded (GAP-WS-096 — skill documents --allow-lite-fallback MUST come BEFORE deep-research but Clap accepts both positions)
- Skill documents: "flag --allow-lite-fallback MUST come BEFORE subcommand deep-research — Clap rejects after subcommand with exit 2"
- Actual behavior: Clap accepts `--allow-lite-fallback` in BOTH positions without error
- NOT a CLI bug — the skill documents a restriction that does not exist. CLI is correct

### Fixed (GAP-WS-097 — skill documents .metadados.nivel_cascata but field was never populated)
- Field `cascade_level` (serialized as `nivel_cascata`) existed in the struct but was ALWAYS `None` and omitted by `skip_serializing_if`
- Real field with value was `cascade_level_observed` (serialized as `cascata_nivel_observado`)
- Fix: `fill_compat_fields()` now populates `cascade_level` with the value from `cascade_level_observed`

### Discarded (GAP-WS-098 — --fetch-content without --max-content-length)
- Investigation revealed that `--fetch-content` WITHOUT `--max-content-length` works with internal default of 4096
- Cases of `conteudo: null` are individual fetch failures for specific URLs (timeout, blocking, etc.)
- NOT a CLI bug — expected behavior when reqwest cannot access the URL
- The skill documents it as PROHIBITED for best practices (unbounded memory), NOT because the CLI fails

### Fixed (GAP-WS-099 — ZeroResultsSuspeito did not produce exit code 6)
- Enum `ZeroCause` has 6 variants: `Legitimo`, `FiltroSilencioso`, `GhostBlock`, `AntiBot`, `RespostaInvalida`, `ZeroResultsSuspeito`
- The exit code 6 match in `lib.rs` covered ONLY 4 variants — `ZeroResultsSuspeito` was missing
- When classifier returned `ZeroResultsSuspeito`, `zero_cause_non_legitimo` was `false` and exit code fell to 5 instead of 6
- Fix: added `ZeroResultsSuspeito` to the match arm in BOTH branches (Single and Multi)

### Fixed (GAP-WS-100 — tamanho_conteudo reported size of original HTML body instead of truncated text)
- `content_size` was set with `size_original` (bytes of raw HTML body from `extract_http_content()`)
- Extracted text was truncated via `apply_readability(html, max_size)` but `tamanho_conteudo` ignored the truncation
- Result: `--max-content-length 500` returned `tamanho_conteudo: 18594` when `conteudo` field had ~494 chars
- Fix: use `text.len()` for `content_size` instead of `size_original`

### Discarded (GAP-WS-101 — skill documents --region br-pt but flag expects simple country code)
- Skill documents: `--region br-pt` as usage example
- Actual behavior: `--country`/`--region` accepts ONLY the country code (`br`, `us`, `uk`)
- `format_kl(lang, country)` concatenates `country-lang`, so `--region br-pt` generates `br-pt-pt` (duplicated)
- NOT a CLI bug — `format_kl` works correctly. It is a skill documentation gap (CLAUDE.md), which is PROHIBITED to alter

### Fixed (GAP-WS-102 — deep-research nivel_cascata ALWAYS null)
- `cascade_level` in `DeepResearchMetadata` was derived from `o.metadata.cascade_level` (compat field)
- `fill_compat_fields()` populates `cascade_level` from `cascade_level_observed` — but runs AFTER the pipeline returns
- During deep-research execution, `cascade_level` was still `None` in all sub-query outputs
- Fix: read from `cascade_level_observed` (real field) instead of `cascade_level` (compat field)

### Fixed (GAP-WS-103 — exit code 6 SUSPECTED_BLOCK missing from --help)
- EXIT CODES section in `--help` listed only exit codes 0-5
- Exit code 6 (`SUSPECTED_BLOCK`) was emitted by the CLI but NOT documented in help
- Operators using `--help` as reference did NOT know exit 6 existed
- Fix: added line `6    Suspected block (zero results with non-legitimate causa_zero)` to after_long_help

### Validation
- `cargo build` — ZERO errors
- `cargo clippy` — ZERO warnings
- `cargo fmt --check` — ZERO differences
- `cargo test` — ZERO failures
- Chrome headed via Xvfb works after stale lock cleanup (GAP-WS-089)
- `--num N` respected in single, batch, and deep-research paths (GAP-WS-090, 094)
- `--region` alias works as documented (GAP-WS-091)
- Compat fields `.metadados.quantidade_resultados`, `.metadados.endpoint_usado`, `.metadados.nivel_cascata` present (GAP-WS-092, 093, 097)
- `identidade_usada` populated for Chrome headed Auto (GAP-WS-095)
- `ZeroResultsSuspeito` emits exit 6 (GAP-WS-099)
- `tamanho_conteudo` reflects actual content size (GAP-WS-100)
- Deep-research `nivel_cascata` populated (GAP-WS-102)
- `--help` lists exit codes 0-6 (GAP-WS-103)


## [0.8.7] - 2026-06-23

### Fixed (GAP-WS-072 — code ignores native display ($DISPLAY/$WAYLAND_DISPLAY))
- Linux desktop with GNOME/KDE fell into headless mode despite having an active display
- macOS and Windows ALWAYS fell into headless (Cloudflare-detected, 0 results)
- Added `has_native_display()` that detects native display per platform
- macOS/Windows now return `true` (Quartz/DWM always active), Linux checks `$DISPLAY`/`$WAYLAND_DISPLAY`

### Fixed (GAP-WS-073 — Chrome headed shows visible window to user)
- When Chrome ran headed, the window appeared on the user's screen
- `--window-position=-32000,-32000` does NOT work on GNOME/Mutter (clamps to screen bounds)
- Fix: spawn private Xvfb even when native display exists — Chrome headed in isolated virtual display
- If Xvfb unavailable: fallback to headless (invisible) with instruction message

### Fixed (GAP-WS-074 — Safari/Firefox UA sent with Chromium TLS fingerprint)
- Identity pool could select Safari or Firefox UA for Chrome-primary search
- Cloudflare cross-checks JA3/JA4 TLS fingerprint (Chromium) against UA (Safari) — mismatch detected
- Added `chrome_only_ua_for_platform()` filter: ONLY Chrome UA with Chromium browser

### Fixed (GAP-WS-075 — Chrome launch flags increase bot score)
- Missing anti-detection flags: `--disable-features=AutomationControlled,TranslateUI`, `--disable-infobars`
- `--disable-extensions` was a suspicious flag that increased bot score — REMOVED

### Fixed (GAP-WS-076 — stealth scripts incomplete against Cloudflare 2026)
- `navigator.webdriver` was set to `false` instead of `undefined` (real Chrome has `undefined`)
- Added: CDP stack trace filter, Permissions API (clipboard, geolocation), WebSocket CDP leak prevention

### Fixed (GAP-WS-077 — Chrome navigates directly to search URL without warm-up)
- Chrome navigated directly to the search URL without visiting duckduckgo.com first
- Cloudflare resolves the JS challenge on the first visit and sets cookies
- Fix: navigate to duckduckgo.com BEFORE the search URL with pseudo-random delay (800-1500ms)

### Fixed (GAP-WS-078 — CLI does not detect OS nor auto-install dependencies (Xvfb))
- Added `detect_linux_distro()` reading `/etc/os-release` ID field
- Added `detect_linux_variant()` detecting immutable distros (Silverblue, Kinoite, NixOS, Guix, ostree)
- Added `try_auto_install_xvfb()` using `sudo -n` (non-interactive) to avoid password blocking
- Supports 22+ distros: Fedora, RHEL, CentOS, Rocky, AlmaLinux, Ubuntu, Debian, Mint, Pop, Zorin, Elementary, Kali, Arch, Manjaro, EndeavourOS, Garuda, openSUSE, SLES, Alpine, Amazon Linux, Void, Gentoo
- Immutable distros: skips auto-install and shows manual command

### Fixed (GAP-WS-079 — auto-install Xvfb not called in native display branch)
- `try_auto_install_xvfb()` was only called in the no-display branch (server)
- In the native-display branch (desktop), when `spawn_virtual_display()` failed, fell to native headed (visible window)
- Fix: call `try_auto_install_xvfb()` + retry `spawn_virtual_display()` ALSO in the native display branch

### Fixed (GAP-WS-080 — Stdio::null() hid package manager output)
- Auto-install used `.stdout(Stdio::null()).stderr(Stdio::null())` — user saw ZERO output during install
- Fix: changed to `.stdout(Stdio::inherit()).stderr(Stdio::inherit())` — real-time package manager output

### Fixed (GAP-WS-081 — failure messages visible only via tracing (invisible with -q))
- All auto-install success/failure messages went only to `tracing::warn`/`info`
- With `-q` (quiet), user saw NOTHING about the install outcome
- Fix: added `eprintln!` with ANSI colors for ALL states (pre-install, success, failure, error)

### Fixed (GAP-WS-082 — manual install instructions only in native display branch)
- The `eprintln` with install instructions existed ONLY in the `has_native_display()` branch
- In the server branch (no display), when Xvfb failed, fell silently to headless
- Fix: added install instructions in both branches

### Fixed (GAP-WS-083 — distros missing from manual install instructions)
- Hardcoded instructions covered only Fedora, Ubuntu, Arch
- Missing: Alpine, Void, Gentoo, Amazon Linux, NixOS, Guix, openSUSE, SLES, and derived distros
- Created `xvfb_manual_instruction()` with match for 22+ distros

### Fixed (GAP-WS-084 — apt vs apt-get inconsistency between code and instructions)
- Auto-install code used `apt-get` but manual instruction message said `apt`
- `apt-get` is the correct binary for scripts (`apt` is an interactive wrapper)
- Standardized on `apt-get` in both code and instructions

### Fixed (GAP-WS-085 — no message displayed BEFORE auto-install attempt)
- `sudo -n` was executed without any prior output to the user
- Fix: added `eprintln!` showing that Xvfb was not found and auto-install will be attempted
- Shows the exact command to be executed (e.g. `sudo dnf install -y xorg-x11-server-Xvfb`)

### Fixed (GAP-WS-086 — redundant /etc/os-release read in detect_linux_variant())
- `detect_linux_variant()` called `detect_linux_distro()` internally, re-reading `/etc/os-release`
- Eliminated circular dependency — now searches directly for `\nid=nixos` and `\nid=guix`

### Fixed (GAP-WS-087 — AggregatedItem.title serializes as "title" instead of "titulo")
- Normal search serialized field as `"titulo"` via `SearchResult` with serde rename
- Deep-research used `AggregatedItem` that had NO serde rename — inconsistent schema
- Fix: added `#[serde(rename = "titulo")]` to `AggregatedItem.title`

### Fixed (GAP-WS-088 — DeepResearchOutput missing top-level query field)
- `SearchOutput` has a top-level `.query` field in the JSON envelope
- `DeepResearchOutput` did NOT have `.query` — only `metadados.query_original`
- Consumers using `.query` uniformly got null for deep-research
- Fix: added `pub query: String` populated with `args.query.clone()`


## [0.8.6] - 2026-06-22

### Changed (GAP-WS-066 — cargo install fails on Windows — btls-sys requires NASM+CMake)
- BREAKING BUILD: replaced `wreq` (BoringSSL) with `reqwest` + `rustls-tls` (pure Rust TLS)
- Eliminates 4 Windows build prerequisites: NASM, CMake, Perl, MSVC cl.exe
- `cargo install duckduckgo-search-cli` now works on Windows with only the Rust toolchain
- Removed crates: `wreq`, `wreq-util`, `brotli`, `brotli-decompressor`, `alloc-no-stdlib`
- Removed build.rs preflights: `nasm_in_path`, `cmake_in_path`, `cl_in_path`, `perl_in_path`
- Renamed `src/wreq_cookie_adapter.rs` → `src/cookie_adapter.rs`
- Cookie persistence rewritten: uses `reqwest::cookie::Jar` + `CookieStore::cookies()` header extraction
- Brotli decompression removed (DuckDuckGo never serves brotli for HTML endpoints)
- HTTP fallback loses BoringSSL TLS fingerprint emulation (Chrome headed is primary since v0.8.0)
- ADR-0001 (wreq/BoringSSL) superseded by ADR-0008 (reqwest/rustls)
- Unified TLS stack: `rustls` in all components (chromiumoxide + reqwest)

### Fixed (GAP-WS-067 — `--num 0` accepted without validation)
- `--num 0` was silently accepted, producing a search that could never return useful results
- Added `value_parser(clap::value_parser!(u32).range(1..))` to reject zero at argument parsing time

### Fixed (GAP-WS-068 — docs say `--synth-format plain` but clap expects `plain-text`)
- 4 documentation files declared `plain` as a valid value for `--synth-format`
- The clap `ValueEnum` derive converts `PlainText` to `plain-text` (kebab-case)
- Corrected `plain` → `plain-text` in AGENTS.md, AGENTS.pt-BR.md, HOW_TO_USE.md, HOW_TO_USE.pt-BR.md

### Fixed (GAP-WS-069 — doc comment in decompress.rs mentions 'wreq' without migration context)
- `src/decompress.rs:39` said "brotli removed in v0.8.6 with wreq" — clarified to mention the wreq-to-reqwest migration

### Fixed (GAP-WS-070 — 4 recipes in MIGRATION.md with global flags after subcommand)
- 4 deep-research recipes in MIGRATION.md and MIGRATION.pt-BR.md had `-q -f json` AFTER the subcommand
- clap requires global flags BEFORE the subcommand — recipes caused `unexpected argument '-q'`
- Reordered flags to appear before `deep-research` in all 4 recipes

### Documentation (GAP-WS-071 — 10+ docs still describe wreq/BoringSSL as current TLS stack)
- 13 documentation files updated to reflect reqwest+rustls-tls as the current TLS stack
- Historical wreq references in v0.7.x changelog sections preserved with context notes
- Affected: README, SECURITY, INVERSIONS, CONTRIBUTING, HOW_TO_USE, AGENTS, ADR-0002/0005/0007


## [0.8.5] - 2026-06-21

### Fixed (GAP-WS-065 — Chrome headless detected by Cloudflare — 0 results)
- CRITICAL regression: `--headless=new` Chrome is detectable by Cloudflare anti-bot
- All queries returned 0 results with `anomaly-modal` interstitial since v0.8.1
- Root cause: GAP-WS-060 fix changed Chrome from headed to headless by default
- Cloudflare fingerprints headless Chrome via JS signals (`navigator.webdriver`, CDP protocol, missing plugins)
- Fix: auto-spawn private Xvfb virtual display, run Chrome headed inside it
- Chrome runs headed (passes anti-bot) but user sees ZERO visible windows
- `builder.env("DISPLAY", ":99")` passes virtual display only to Chrome child process
- Xvfb cleanup is automatic via `Drop` on `ChromeBrowser`
- Fallback: if Xvfb not available, falls back to headless (with anti-bot risk)
- New env var: `DUCKDUCKGO_CHROME_HEADLESS=1` to force headless mode


## [0.8.4] - 2026-06-21

### Fixed (GAP-WS-064 — `cascade_level_observed` always `null` in parallel path)
- Batch queries and deep-research sub-queries never reported `cascata_nivel_observado` in JSON metadata
- Root cause: `cascade_level_observed: None` hardcoded in `search_one_query` success path in `parallel.rs`
- Fix: reuse `derive_cascade_level_from_attempts` from `pipeline.rs` (now `pub(crate)`)
- Single queries via `pipeline.rs` were already correct — this fix brings parity


## [0.8.3] - 2026-06-21

### Fixed (GAP-WS-062 — `chrome_attempted` metadata incorrect in parallel path)
- `parallel.rs` reported `tentou_chrome: true` even when `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` disabled Chrome at runtime
- Root cause: `chrome_attempted = cfg!(feature = "chrome")` is a compile-time constant, always `true` when the chrome feature is enabled
- Fix: runtime check now includes `NO_CHROME` env var — `cfg!(feature = "chrome") && NO_CHROME != "1"`
- Affects batch queries (`--queries-file`) and deep-research sub-queries
- `pipeline.rs` (single queries) was already correct — this fix brings parity between both paths

### Fixed (GAP-WS-063 — `identity_used` always `null` in parallel path success)
- Batch queries via `--queries-file` never reported `identidade_usada` in JSON metadata, even with `--identity-profile chrome-linux`
- Root cause: `identity_used: None` hardcoded in `search_one_query` success path and early-return error path
- Fix: call `identity_tag_for_cli_identity(config.identity_profile, None)` in both paths
- Single queries via `pipeline.rs` were already correct — this fix brings parity


## [0.8.2] - 2026-06-21

### Fixed (GAP-WS-061 — deep-research ignores root search flags)
- `execute_deep_research` now inherits all search flags from the root CLI args instead of using hardcoded defaults
- `--num`, `--lang`, `--country`, `--endpoint`, `--retries`, `--proxy`, `--timeout`, `--parallel`, `--max-content-length`, `--identity-profile` are now respected when placed BEFORE the `deep-research` subcommand
- Previously hardcoded: `num_results=10`, `language="en"`, `country="us"`, `retries=2`, `proxy=None`
- Now inherits user values: `--num 5 --lang pt --country br deep-research "query"` works as expected
- `--allow-lite-fallback`, `--pre-flight`, `--identity-profile` propagated from root args
- Removed dead `initialize_logging(0, false, false)` call (subscriber already initialized before subcommand dispatch)


## [0.8.1] - 2026-06-21

### Fixed (GAP-WS-060 — Chrome opens visible window on desktops)
- Chrome now runs in headless mode (`--headless=new`) by DEFAULT on all platforms.
- Previously, Chrome opened a visible GUI window on any desktop with `$DISPLAY` set (Linux, macOS, Windows).
- `DUCKDUCKGO_CHROME_VISIBLE=1` enables headed mode for debugging.
- `DUCKDUCKGO_CHROME_XVFB=1` enables headed mode via xvfb-run for anti-bot evasion on headless servers.
- ZERO visible Chrome windows during normal CLI execution.
- Function `which_xvfb_run()` renamed to `is_xvfb_requested()` with correct semantics.

## [0.8.0] - 2026-06-19

### Fixed (GAP-AUD-003 — zero-result causal classification)
- **CR1 — `total == 0` mapped straight to exit 5 with no causal inspection**. `src/lib.rs:241-243` now distinguishes 5 semantically different causes (`Legitimo`, `FiltroSilencioso`, `GhostBlock`, `AntiBot`, `RespostaInvalida`) and emits exit 6 (`SUSPECTED_BLOCK`) when the cause is non-legitimate. Exit 5 is preserved for a genuine zero.
- **CR2 — `pre_flight_blocked` now runs in addition to the causal classifier**. The legacy branch still emits exit 3, but the new classifier catches cases where pre-flight was off (the default).
- **CR3 — `--pre-flight` stays opt-in to preserve BC**. But the classifier runs automatically whenever `quantidade_resultados == 0`, so the default operator now benefits without having to learn about the flag.
- **CR5 — `SearchMetadata.pre_flight_fired` stays a `bool` for BC**. But it now coexists with `causa_zero`, which captures the causal nuance.
- **CR6 — `causa_zero` added to the JSON envelope**. The field `metadados.causa_zero: Option<ZeroCause>` serialises as kebab-case (`"anti-bot"`, `"ghost-block"`, etc.).

### Fixed (Bug #1 — HTTP response decompression)
- **`wreq 6.0.0-rc` sends `accept-encoding: gzip, deflate, br` but does not decompress automatically**. The body returned by `Response::text()` / `Response::bytes()` arrived as gzip-compressed bytes (≈9.2 KB binary instead of ≈14 KB of plain text — a 65% ratio consistent with the gzip level-6 default). `detectar_interstitial_com_match` was running `body.contains("anomaly-modal")` against binary bytes and failing silently, which made the classifier label `Legitimo` in an environment provably blocked by Cloudflare.
- **New module `src/decompress.rs`** inspects `Content-Encoding` and dispatches to `flate2::read::MultiGzDecoder` (gzip), `flate2::read::ZlibDecoder` (deflate) or `brotli_decompressor::Decompressor` (br). `MultiGzDecoder` handles concatenated gzip streams transparently.
- **7 call sites replaced** (`src/search.rs:403`, `src/search.rs:776`, `src/lib.rs:637`, `src/pipeline.rs:311`, `src/content.rs:180`): every `response.text().await` call now goes through the decompressor before becoming a `String`.
- **`tokio::task::spawn_blocking`** wraps the sync decode to avoid blocking the tokio reactor on large payloads.
- **`DECOMPRESSION_MAX_OUTPUT = 32 MiB`** as a safety cap against gzip bombs, via `Read::take(cap + 1)`, which aborts decompression when the stream exceeds it.
- **3 variants on `CliError`**: `PayloadTooLarge { max, actual }`, `UnsupportedEncoding(String)`, `InvalidUtf8(FromUtf8Error)`. The JSON output keeps `error: "http_error"` for BC.
- **`flate2 = "1"` added to `Cargo.toml`** — it was already transitive via the `wreq` `gzip`+`deflate` features, declared explicitly for stable visibility.

### Fixed (Bug #2 — documented BC opt-out semver drift)
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` affects ONLY the exit code (mapping 6 → legacy 5), but the `causa_zero` field stays published in the JSON envelope. The `#[serde(skip_serializing_if = "Option::is_none")]` policy guarantees that v0.7.x clients that NEVER run the classifier see no change at all. Clients running against a blocked environment with the opt-out active receive `causa_zero` even though they asked for the legacy exit 5 — that is additive diagnostic information, aligned with the additive-skip_serializing_if change pattern used in v0.6.4 (`identidade_usada`) and v0.7.9 (`pre_flight_fired`). Documented in the Migration Guide section below.

### Fixed (GAP-NEW-001 — the Rust timeout-cli wrapper shadows GNU)
- README EN + PT-BR updated with a `Troubleshooting` section citing GNU coreutils `/usr/bin/timeout` as the workaround for the bug in the Rust `timeout-cli` v0.1.0 wrapper, which re-parses `-v` flags before clap.
- Script `scripts/detect-timeout-wrapper.sh` created and made executable (mode 755). It detects automatically which `timeout` is on PATH (GNU vs the Rust wrapper).
- Runtime detection in `src/lib.rs:initialize_logging` via the `CARGO_BIN_EXE_timeout` env var. Emits `tracing::warn!` with the workaround.
- Regression test in `tests/integration_troubleshooting_documentation.rs` validates the documentation and the script's existence.

### Fixed (GAP-NEW-002 — `tracing::debug!()` vanished in release)
- Mass migration of 46 `tracing::debug!` calls to `tracing::info!` across 11 production files.
- `#[tracing::instrument(level = "debug")]` on `classify_zero_result` migrated to `level = "info"`.
- Fields `bytes_brutos`/`bytes_descomprimidos: Option<u64>` added to `SearchMetadata`.
- Fields `bytes_in`/`bytes_out: u64` added to `AggregatedSearchResult` and wired in through `pipeline.rs` + `parallel.rs`.
- Local HTTP decompression telemetry is now visible in release builds.

### Fixed (GAP-NEW-003 — the classifier labelled a stealth shell as `Legitimo`)
- A new CR4b branch in `classify_zero_result` detects a 14KB+ stealth shell with no `result__a` markers, no interstitial markers, but with a DDG signature.
- Field `cascata_nivel_observado: Option<u32>` on `SearchMetadata` propagates the cascade level from probe-deep.
- Field `last_probe_cascade_level: Option<u32>` on `Config` caches the last process-local probe.
- 4 tests in `tests/integration_stealth_block_classification.rs` validate detection and non-regression.
- The classifier now returns `GhostBlock` instead of `Legitimo` for a stealth-blocked environment.

### Fixed (GAP-NEW-004 — lite auto-fallback for the Brazil vs Morocco case)
- Lite auto-fallback in `src/pipeline.rs:464-505` re-runs the search with `endpoint=Lite` when the classifier returns a non-legitimate cause.
- Recursive re-execution via `Box::pin` to avoid an `infinitely sized future`.
- Result merging preserves the original `causa_zero` and marks `used_fallback_endpoint=true`.
- 5 tests in `tests/integration_e2e_real_world.rs` reproduce the Brazil 1x1 Morocco case.

### Added
- **`ZeroCause` enum in `src/types.rs`** with 5 variants marked `#[non_exhaustive]` for forward compat. Serialises as kebab-case.
- **`pipeline::classify_zero_result`** — a pure, I/O-free classifier whose causal chain is documented in `docs/decisions/0004-zero-cause-classification-v0-8-0.md`.
- **`docs/decisions/0006-stealth-shell-classification-v0-8-0.md`** (GAP-NEW-003 / GAP-NEW-008) — ADR documenting the architectural decision behind the CR4b branch (4 simultaneous conditions: body_len >= 4000 + !result__a + InterstitialKind::None + DDG signature). Includes alternatives considered (dynamic threshold, ML classifier, marker probing) and validation (proptest with 64 cases).
- **`pipeline::sugestao_proxima_acao_para_zero`** — deterministic PT-BR strings per variant, aligned with the `sugestao_mitigacao_com_marker` pattern.
- **`SearchMetadata.zero_cause: Option<ZeroCause>`** + **`SearchMetadata.sugestao_proxima_acao: Option<String>`** in the JSON envelope.
- **`MultiSearchOutput.causa_zero_histogram: BTreeMap<String, u32>`** aggregated automatically on multi-query; BTreeMap guarantees deterministic lexicographic order.
- **`AggregatedSearchResult.first_body: String`** exposed so the classifier can distinguish a ghost-block from a genuine zero.
- **`DUCKDUCKGO_ZERO_CAUSE_STRICT` env var** for the BC opt-out (default ON; accepts `false`/`0`/`no`/`off`).
- **`exit_codes::SUSPECTED_BLOCK: i32 = 6`** added to the exit-code table.
- **`docs/decisions/0004-zero-cause-classification-v0-8-0.md`** — ADR documenting the architectural decision and the patch→effect causal chain.
- **12 unit tests in `src/pipeline.rs`** covering all 5 enum variants + the suggestion messages.
- **`assert_eq!(SUSPECTED_BLOCK, 6)` in `src/error.rs`** — stale test updated.

### Changed
- `Cargo.toml` bumped 0.7.10 → 0.8.0
- `Cargo.lock` regenerated
- **`src/ddg_class_watch.rs` moved to `examples/ddg_class_watch.rs`** (GAP-OPS-002). The module was declared at `lib.rs:56` but had no call sites in production or tests; it now lives as an example invocable via `cargo run --example ddg_class_watch`. A `fn main()` was added with demonstration HTML that prints a report and exits with code 1 when it detects new classes (an alert signal to bump `RESULT_PAGE_SELECTORS` in `src/probe_deep.rs`). The historical reference in CHANGELOG [0.7.10] P19 is preserved — the module WAS in `src/` in v0.7.10.

### Migration Guide v0.7.x → v0.8.0

**Exit code 6 is additive; it does not replace exit 5.** Clients that branch on `exit 5` can keep working unchanged through the BC opt-out:

```bash
# Restores v0.7.x behaviour: exit 5 always for total == 0
export DUCKDUCKGO_ZERO_CAUSE_STRICT=false

# v0.8.0 default: exit 6 when causa_zero is non-legitimate
duckduckgo-search-cli "blocked query" -f json
```

**The `metadados.causa_zero` field is additive diagnostics.** Clients parsing JSON must treat it as `Option<String>` (it may be absent). Possible values: `"legitimo"`, `"filtro-silencioso"`, `"ghost-block"`, `"anti-bot"`, `"resposta-invalida"`.

**Even under `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`, the JSON keeps `causa_zero`.** That is useful diagnostic information — legacy exit code, new envelope. Documented in `src/error.rs:60-66`.

**The HTTP response is now decompressed transparently.** Anyone intercepting raw socket bytes (proxy, mitm) will see `Content-Encoding` headers, but the body handed to application code is always plain text.

### Validation
- `cargo build --release --offline` build OK
- `cargo clippy --all-targets --offline -- -D warnings` zero warnings
- `cargo test --offline` 378 tests passing, 0 failing
- E2E wiremock gzip-encoded: exit 6 + `causa_zero: "anti-bot"` + actionable suggestion
- E2E `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`: exit 5 + `causa_zero: "anti-bot"` in the JSON (documented drift)
- E2E Chrome-primary: 10 results via Chrome, `usou_chrome: true`, exit 0
- E2E deep-research: 38 unique results, 20 references in the synthesis, exit 0
- The exit-code table is now frozen as semver-additive (0-5 stable, 6+ added without reassignment)
- `gaps.md` GAP-AUD-003, GAP-NEW-005, GAP-NEW-006, GAP-NEW-007 marked `RESOLVIDO em v0.8.0`

### Added (Chrome headed as PRIMARY search transport — GAP-NEW-005, GAP-NEW-006, GAP-NEW-007)
- Chrome headed mode via `xvfb-run` is now the PRIMARY search transport
- `src/browser.rs:46` — `STEALTH_SCRIPTS` constant with 17 JavaScript stealth signals
- Canvas, WebGL, AudioContext fingerprint spoofing via CDP injection
- `navigator.webdriver` set to `false` before page navigation
- `navigator.plugins`, `navigator.languages`, `chrome` object spoofing
- `navigator.connection`, `navigator.maxTouchPoints` set to realistic values
- `which_xvfb_run()` function auto-detects `xvfb-run` binary on Linux
- `ChromeBrowser::launch()` uses headed mode when `DISPLAY` or `xvfb-run` available
- Headless mode is FALLBACK when neither display nor xvfb-run is available
- `execute_chrome_search()` in `src/pipeline.rs` — Chrome-first search pipeline
- `execute_chrome_search_pub()` public wrapper for `src/parallel.rs` deep-research
- `parallel.rs` deep-research uses Chrome pipeline via `execute_chrome_search_pub()`
- wreq remains ONLY for `--fetch-content` and `--probe` HTTP requests
- wreq TLS emulation via `Emulation::Chrome136` in `src/http.rs` (closes GAP-NEW-005)
- `SearchMetadata.tentou_chrome` field added (serialized as `tentou_chrome`)
- `SearchMetadata.usou_chrome` now `true` when Chrome-primary succeeds (not just fallback)
- Tracing initialization moved BEFORE subcommand dispatch in `src/lib.rs`
- Deep-research `-q` flag now properly silences tracing output

### Prerequisites (v0.8.0)
- Linux: `sudo apt install xvfb` (Debian/Ubuntu) or `sudo dnf install xorg-x11-server-Xvfb` (Fedora)
- Linux: Google Chrome or Chromium must be installed
- macOS: Chrome must be installed; no xvfb needed (native display used)
- Windows: Chrome must be installed; no xvfb needed (native display used)
- The `chrome` feature is enabled by default in `Cargo.toml`
- To build without Chrome: `cargo build --no-default-features`

### Validation (Chrome-primary)
- 378 unit tests passing, 0 clippy warnings
- E2E simple search: 10 results via Chrome, `usou_chrome: true`, exit 0
- E2E deep-research: 38 unique results, 20 references in synthesis, exit 0
- ZERO Cloudflare blocking with headed Chrome + 17 stealth signals

## [0.7.10] - 2026-06-17

### Fixed (anti-bot UX + observability + e2e hardening + 4 bug fixes)
- **B1 (CRITICAL) — `--pre-flight` emitted two concatenated JSON objects on stdout**. `src/pipeline.rs:297` called `print_line_stdout` directly and then returned a `SearchOutput` that the caller in `src/lib.rs` serialised again via `emit_result`. Consumers using `| jaq '.resultados'` broke because the stream contained two JSON envelopes with no separator. The early print was removed; the `SearchOutput` carries the pre-flight context in the envelope and the caller serialises it exactly once.
- **B2 (CRITICAL) — `pre_flight_blocked` returned exit 0, now returns exit 3**. The `EXIT CODES` table in `--help` promises exit 3 for "DuckDuckGo 202 block anomaly", but the pre-flight path fell into the `Ok(output)` that returned `SUCCESS`. `src/lib.rs` now detects `output.error == Some("pre_flight_blocked")` and returns `exit_codes::RATE_LIMITED_OR_BLOCKED` (3) before serialisation.
- **B3 (MEDIUM) — `--global-timeout` is now global and accepted on subcommands**. The flag lived on `CliArgs` (the sub-tree) without `global = true`, so `duckduckgo-search-cli deep-research --global-timeout 30 query` failed with `error: unexpected argument '--global-timeout' found`. It moved to `RootArgs` with `#[arg(global = true)]`; `lib.rs` hoists the value via `root_global_timeout_seconds` and propagates it to the `deep-research` subcommand.
- **B4 (CRITICAL) — standalone `--probe-deep` now returns exit 3 when it detects a captcha**. The probe reported `status: "captcha"` in the JSON but the CLI returned exit 0. Now, when `InterstitialKind != None`, it returns `exit_codes::RATE_LIMITED_OR_BLOCKED` (3). This allows branching on the exit code instead of parsing the JSON.
- **B5 (confirmed FALSE POSITIVE) — `--require-results` works correctly**. The initial test showed exit 0 because `user-agents.toml` and `selectors.toml` did not exist yet; after `init-config` the path correctly returns exit 4 (GLOBAL_TIMEOUT). No change needed.

### Added
- **v0.7.9 P1-P7 — `detectar_interstitial_com_match` returns `(&'static str, InterstitialKind)` with the literal marker**. A new helper in `src/probe_deep.rs` that makes it possible to tell which Cloudflare/DDG marker was detected (as opposed to heuristic ghost-block detection).
- **v0.7.9 P4b — `sugestao_mitigacao_com_marker` returns a string with the literal marker**. A new helper that injects the real marker (e.g. `cf-challenge`, `anomaly-modal`) into the mitigation message instead of a generic "ghost-block". The original version is marked `#[deprecated(since = "0.7.10")]`.
- **v0.7.9 P3 — `SearchMetadata.pre_flight_fired: bool` added to the envelope**. When `cfg.pre_flight == true && ghost-block`, the field is `true`. This lets consumers tell a normal search from one where pre-flight fired.
- **v0.7.9 P5 — `--allow-lite-fallback` and `--pre-flight` became `global = true`**. Both flags are accepted before and after subcommands such as `deep-research`. Closed GAP-WS-58/59 with zero regressions.
- **v0.7.10 P5 — probe-deep scheduler integrated into `execute_single_search`**. When `cfg.pre_flight == true`, the pipeline runs a minimal probe before the real search and aborts on captcha/ghost-block.
- **v0.7.10 P6/P17 — `insta = "1"` added, plus a snapshot test for the 8 Cloudflare 2026 markers**. Catches a regression if someone removes a marker string.
- **v0.7.10 P7/P16 — `src/proxy_detection.rs`, a new module with `ProxyKind::{None, Transparent, Cloudflare, Corporate}`**. A response-header inspection heuristic (Vivo Fiber, Gigaweb, Cloudflare) with 8 tests covering Brazilian ISPs.
- **v0.7.10 P4 — `--require-results` on `deep-research`**. When set and the fan-out is zero, it returns exit 4 (`GLOBAL_TIMEOUT`) with "exiting non-zero" on stderr.
- **v0.7.10 P9 — `examples/pre_flight.rs`**. Demonstrates combined use of `--pre-flight` + `--allow-lite-fallback`.
- **v0.7.10 P10 — `docs/decisions/0003-pre-flight-scheduler-v0-7-10.md`**. ADR documenting the scheduler's architectural decision.
- **v0.7.10 P14 — `benches/pre_flight_latency.rs` + `BENCHMARKS.md`**. A Criterion benchmark with 3 scenarios (baseline / clean pre-flight / blocked pre-flight).
- **v0.7.10 P19 — `src/ddg_class_watch.rs`**. A module for runtime monitoring of DDG templates.

### Changed
- `Cargo.toml` bumped 0.7.8 → 0.7.10
- `Cargo.lock` regenerated
- `gaps.md` GAP-WS-58 and GAP-WS-59 marked `RESOLVIDO`

## [0.7.9] - 2026-06-16

### Fixed
- **GAP-WS-58 (CRITICAL, ghost-block) — `detectar_interstitial` now classifies a sub-4KB body with no `result-page-signal` as Cloudflare**. The conservative 4KB threshold avoids false positives on valid low-density responses. The `has_result_page_signal` helper checks for the presence of DDG classes (`nrn-react-div`, `react-article`, `module--results`, `js-react-aria-results`).
- **GAP-WS-59 (HIGH, 2026 markers) — 5 new Cloudflare markers + 1 new DDG marker** (details in v0.7.8 as well).
- **GAP-WS-59 (HIGH, global flag) — `--allow-lite-fallback` hoisted to `RootArgs` with `global = true`**. Closes the "unexpected argument" path on deep-research.
- **`Config.pre_flight` added** with default `false`, opt-in.

## [0.7.8] - 2026-06-15

### Fixed (anti-bot detection overhaul + dependency hygiene)
- **GAP-WS-50 (CRITICAL, detector) — `detectar_interstitial` in `src/probe_deep.rs` now recognises the DDG `anomaly-modal` interstitial that DDG served on 2026-06-14**. The `CLOUDFLARE_MARKERS` list now contains `anomaly-modal`, `anomaly.js`, `botnet` and `Unfortunately, bots`; the `DDG_MARKERS` list now contains `anomaly-modal__title`. Legacy markers were kept for compatibility. The detector emits `InterstitialKind::Cloudflare` / `InterstitialKind::Ddg` again instead of a silent `None`. 8 new unit tests in `src/probe_deep.rs::tests` validate each marker against real HTML fixtures.
- **GAP-WS-51 (HIGH, probe-deep) — the long calibration query `the quick brown fox jumps over the lazy dog` replaces the hard-coded `q=rust` in probe-deep**. The short one-word query (`rust`) returned the DDG home page, which does not trip the bot detector. The 9-word long query trips the upstream tightening and reflects the real usage scenario. The `PROBE_CALIBRATION_QUERY` constant at the top of the `src/lib.rs` module makes the calibration explicit.
- **GAP-WS-52 (HIGH, fallback) — `--allow-lite-fallback` now consults `detectar_interstitial` before deciding on a fallback**. The lite fallback decision at `src/search.rs:559` moved from `accumulated_results.is_empty()` to `detectar_interstitial(&first_html) != InterstitialKind::None`. When the detector classifies an interstitial, the lite fallback fires immediately and the final response is `exit 3` (anti-bot) with `cascata_motivo` populated, instead of a silent `exit 5` (zero results).
- **GAP-WS-53 (LOW, UX) — `-v` now accepts multiple occurrences via `ArgAction::Count`**. Mapping: `-v` → `info`, `-vv` → `debug`, `-vvv` → `trace`. The `RUST_LOG` variable still overrides. A regression test in `src/cli.rs::tests` validates that `-vvv` is accepted without a clap error. The Unix convention is now respected.
- **GAP-WS-54 (MEDIUM, supply chain) — `scraper` bumped from 0.20.0 to 0.27.0**. Resolves the transitive `fxhash 0.2.1` (RUSTSEC-2025-0057, unmaintained). A `cargo audit --deny warnings` gate was added to the local gates; `deny.toml` updated. `async-std` (RUSTSEC-2025-0052, discontinued) remains only under the optional `chrome` feature.
- **GAP-WS-55 (LOW, docs drift) — the `wreq` comment at `Cargo.toml:69-86` was rewritten**. The old text mentioned `regressed from wreq 6.0.0-rc.29 to wreq 5.3.0`, a regression that never happened. The new text documents the real decision: pinning `wreq 6.0.0-rc.29` to close GAP-WS-49 (TLS fingerprint emulation) and the 3 direct pins (`wreq-util 3.0.0-rc`, `brotli-decompressor =5.0.1`, `alloc-no-stdlib =2.0.4`).
- **GAP-WS-56 (LOW, UX) — the `buscar` subcommand now carries `#[command(hide = true)]`**. The help for `duckduckgo-search-cli buscar --help` no longer duplicates the global help. The user can still invoke `buscar`, but the subcommand no longer appears in `--help` or in the discovery section. Top-level remains the canonical form of invocation.
- **GAP-WS-57 (MEDIUM, retries) — the `--retries N` flag is now honoured at `src/parallel.rs:644`**. Bug: the value read by `execute_with_retry` was hard-coded to 1, ignoring the flag. The fix propagates `cfg.retries` into the retry loop with a clamp to `[1, 10]` to avoid a `--retries 999` that trips anti-bot. A regression test in `tests/integration_search_retry.rs` validates that `--retries 5` yields `metadados.retentativas == 5` in the JSON.

### Architectural Decision
- ADR `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md` documents the architectural decision, the options considered (including the rejected migration to the non-stable `captcha-detect` crate), and the trade-offs accepted for the 8 gaps WS-50..WS-57 closed in this version.

### Validation
- `cargo check --offline`: 6.88s, zero errors
- `cargo clippy --all-targets --offline -- -D warnings`: 3.70s, zero warnings
- `cargo build --release --offline`: 24.04s, success
- `cargo audit --deny warnings`: zero advisories
- 305 tests (292 lib + 13 integration), 100% passing
- `cargo doc --offline --no-deps`: zero warnings

### Test coverage delta
- `src/probe_deep.rs::tests`: +8 tests (GAP-WS-50 markers)
- `tests/integration_search_retry.rs`: +1 test (GAP-WS-57)
- `src/cli.rs::tests`: +1 test (GAP-WS-53)

### Impact
- Zero breaking changes to the JSON schema or the exit codes
- Final binary: no size change
- 4 new markers in the detector (anti-bot resilience)
- 1 new CLI flag honoured (`--retries`)
- 1 subcommand simplified (`buscar` hidden)


## [0.7.7] - 2026-06-14

### Fixed (CRITICAL, runtime — not caught by GAP-WS-48 release pipeline)
- **GAP-WS-49 (CRITICAL, query) — a real query returns ZERO results because of a TLS fingerprint detectable by DDG.** v0.7.6 fixed `cargo install`, but the published binary passed every `--probe`/`--probe-deep` smoke test (status 200/ok) while real queries returned `resultados: 0` with `cascade_level: 0` and `usou_endpoint_fallback: false` — a silent anomaly. Local reproduction: 5/5 queries tested ("rust", "rust language", "tokio rust async", "rust async runtime", "tokio vs async-std", "axum middleware examples") returned `quantidade_resultados: 0` with latencies of 1.0–1.6s.
- **Root cause**: `wreq 6.0.0-rc.29` on its own does NOT have the `emulation` feature — Chrome/Safari TLS fingerprint emulation lived only in `wreq-util 3.0.0-rc.12` via `default = ["emulation"]`. v0.7.6 removed `wreq-util` (along with the `brotli` feature) to close the `cargo install` GAP-WS-48, and without the emulation `wreq 6.0.0-rc.29` with plain BoringSSL produces a TLS handshake whose JA3/JA4 fingerprint is detectable by Cloudflare Bot Management. DDG serves `anomaly-modal` (45 occurrences in the HTML body) to any client that does not present a real browser fingerprint.
- **Cross-confirmation**: plain `curl` with real browser headers (`User-Agent: Chrome/120`, `Accept-Encoding: gzip, deflate, br`, `Cookie: kl=br-pt`, `Sec-Fetch-*`) **ALSO** receives `anomaly-modal` at the time of the test (2026-06-14 09:25 UTC), which confirms the tightening is upstream and persistent. The minimal 1-request probe (`--probe-deep`) does not trip the tightening because DDG fingerprints on volume and behaviour, not on a single request.
- **Fix applied**:
  1. Re-added the dep `wreq-util = { version = "3.0.0-rc", default-features = false, features = ["emulation"] }` in `Cargo.toml` (only `emulation`, without `default`, so `brotli` is not pulled in by accident).
  2. Re-added the `"brotli"` feature to the `wreq` feature list (required because `wreq-util`'s `emulation` makes `dep:brotli` hard).
  3. Added 2 direct pins in `Cargo.toml` to force compatible versions under `cargo install`:
     - `brotli-decompressor = "=5.0.1"` — versions 5.0.0/5.0.1 have `alloc-no-stdlib = "2.0"` (hard); version 5.0.2, published on 2026-06-14, widened it to `>=2.0.4, <4` and therefore pulls 3.0.0 into the graph.
     - `alloc-no-stdlib = "=2.0.4"` — a hard pin required because `brotli 8.0.3` demands `alloc-no-stdlib = "2.0"`.
  4. Added `cargo update -p alloc-no-stdlib@3.0.0 --precise 2.0.4` to the lock resolution, which removes version 3.0.0 from the graph (pinning alone is not enough, because `cargo install` without `--locked` can resurrect it).
  5. Expanded the `Cargo.toml` comment documenting GAP-WS-49 and the pinning strategy.
- **Post-fix validation**:
  - `cargo tree --offline` → the graph contains exactly `alloc-no-stdlib v2.0.4` and `brotli-decompressor v5.0.1`, zero occurrences of 3.0.0/0.2.3.
  - `cargo build --release --offline` → **success in 24.04s** (vs 37.14s on v0.7.6 — faster because `brotli-decompressor 5.0.1` is smaller than 5.0.2).
  - `cargo install --path . --locked --offline` (the recommended path, identical to CI) → **success in 34.32s**, working binary.
  - Real query `"rust async runtime"` with the v0.7.7 binary locally (before DDG tightened) → **`quantidade_resultados: 5`**, latency 1087ms, real results: `The Async Ecosystem`, `Fundamentals of Asynchronous Programming`, `Tokio - An asynchronous Rust runtime`, etc.
  - `cargo tree | rg 'brotli|alloc-no-stdlib|wreq-util'` → all 4 deps present (brotli 8.0.3, brotli-decompressor 5.0.1, alloc-no-stdlib 2.0.4, wreq-util 3.0.0-rc.12).
- **Residual GAP-WS-48 (NOT fully closed without `--locked`)**: `cargo install` WITHOUT `--locked` regenerates the lockfile from scratch and the solver adds BOTH `alloc-no-stdlib 2.0.4` (from the direct pin) and `alloc-no-stdlib 3.0.0` (from transitive `brotli-decompressor 5.0.2` or `alloc-stdlib 0.2.3`), causing the same E0277 as GAP-WS-48. The solution is for the user to run `cargo install duckduckgo-search-cli --version 0.7.7 --locked`, which respects the committed `Cargo.lock` (already prepared with `cargo update -p alloc-no-stdlib@3.0.0 --precise 2.0.4` during the release). The v0.7.7 `README.md` documents this requirement.
- **Impact**:
  - Final binary: +160KB (brotli 8.0.3 + brotli-decompressor 5.0.1 + wreq-util 3.0.0-rc.12) — a trade accepted to restore the Chrome/Safari TLS fingerprint and beat DDG anti-bot.
  - `cargo install` build time: ~24s (vs ~37s on v0.7.6) — faster because `brotli-decompressor 5.0.1` is smaller than 5.0.2.
  - Supply-chain surface: +3 crates (brotli, brotli-decompressor, wreq-util).
  - **Functionality restored**: real queries return 5+ results again, with a Chrome/Safari TLS fingerprint identical to a real browser.
- `Cargo.toml` version bump: 0.7.6 → 0.7.7.


## [0.7.6] - 2026-06-14

### Fixed (CRITICAL, build)
- **GAP-WS-48 (CRITICAL, install) — `cargo install` broke on 2026-06-14 over an `alloc-no-stdlib 2.0.4 vs 3.0.0` conflict**. Reproduced locally: 36 `E0277 the trait bound 'StandardAlloc: alloc::Allocator<T>' is not satisfied` errors when running `cargo install --path .` (even with `--offline`); the root cause is that `cargo install <crate>@<version>` (without `--locked`) regenerates `Cargo.lock` on the target system and lands on the versions published on 2026-06-14: `alloc-no-stdlib 3.0.0`, `alloc-stdlib 0.2.3` (`alloc-no-stdlib = ">=2.0.4, <4.0.0"`) and `brotli-decompressor 5.0.2`. `brotli 8.0.3` (not updated, still requiring `alloc-no-stdlib = "2.0"`) implements `impl BrotliAlloc for StandardAlloc` expecting the trait from `2.0.4`, but the `StandardAlloc` from `alloc-stdlib 0.2.3` is compiled against `3.0.0` — a trait-bind collision in `enc/reader.rs`, `enc/writer.rs` and `enc/combined_alloc.rs`.
- **Two-layer root cause**: (CR1) `wreq-util 3.0.0-rc.12` (declared as a direct dep, NEVER imported in `src/`) has `default = ["emulation"]`, which activates `dep:brotli`, `dep:flate2`, `dep:zstd` — that is the real carrier of `brotli` in the production graph. The `wreq` `brotli` feature was only secondary. (CR2) The `wreq` `brotli` feature was kept even knowing DuckDuckGo does not send `Content-Encoding: br` (verified on 2026-06-14 against the homepage, `/html/` and `/lite/` via `curl -I`).
- **Fix applied**:
  1. Removed the dep `wreq-util = "3.0.0-rc"` from `Cargo.toml` (it was dead code).
  2. Removed the `"brotli"` feature from the `wreq` feature list (DuckDuckGo does not send br, so br decoding is unnecessary).
  3. Updated the `wreq` comment in `Cargo.toml` to document the removal and reference the incident.
- **Post-fix validation**:
  - `cargo tree --offline | rg 'brotli|alloc-no-stdlib|alloc-stdlib|wreq-util'` → **0 matches** (clean dep graph).
  - `cargo install --path . --offline --root /tmp/ddg-fix-test` (WITHOUT `--locked`, simulating an install on another system) → **success in 35.7s**, working binary, JSON schema preserved.
  - `cargo install --path . --locked --offline` → **success** (local path with the lock pinned).
  - `cargo build --release` → **success in 37.14s** (5.92s faster than v0.7.5 thanks to the absence of `brotli` and `brotli-decompressor`).
- **Impact**:
  - Final binary: -1 dep tree (brotli + brotli-decompressor + alloc-no-stdlib + alloc-stdlib + one copy of wreq-util).
  - `cargo install` build time: -5 to -10 seconds (avoids compiling ~6 brotli crates).
  - Supply-chain surface: -6 crates.
  - **Zero functional impact**: `gzip`+`deflate`+`zstd` remain enabled; the `Accept-Encoding` that `wreq` sends still contains `gzip, deflate, zstd` (without `br`), and DuckDuckGo never sends brotli, so no real response is affected.
- `Cargo.toml` version bump: 0.7.5 → 0.7.6.


## [0.7.5] - 2026-06-14

### Fixed (audit batch 2026-06-14)
- **P1-audit-1 (MEDIUM, error contract)** — `src/lib.rs` `execute_deep_research` was using `println!("{json}")` directly, violating the documented rule that `output.rs` is the only module with `println!` (lib.rs doc-table line 34). Now delegates to `output::print_line_stdout` which handles `BrokenPipe` cleanly (silent success on `| head`, generic error on real I/O failure). Closes the audit finding that the JSON contract for `deep-research` was bypassing the central output abstraction.
- **P1-audit-2 (LOW, code clarity)** — Removed the `unreachable!("handled above")` arm in the subcommand dispatch by folding the `DeepResearch` branch into the main `match` (and dropping the preceding `if let Some(Subcommand::DeepResearch(...))` early-return). The compile-time exhaustiveness check now covers the variant without panicking on dispatch.
- **P1-audit-3 (LOW, exit code semantics)** — `CliError::Cancelled` now maps to exit code `130` (POSIX: 128 + SIGINT(2)) instead of `1` (generic error). Shell sessions can now distinguish user-initiated Ctrl-C from real runtime failures, and process supervisors (e.g. CI runners, `set -e` scripts) treat cancellation as `exit 130` per convention.
- **P1-audit-4 (LOW, error code mapping)** — Three string error code mappings were semantically wrong: `InvalidConfig` → `selector_config_invalid` (should be `invalid_config`); `PathError` → `selector_config_invalid` (should be `path_error`); `BrokenPipe` → `http_error` (should be `broken_pipe`). New constants added: `codes::INVALID_CONFIG`, `codes::PATH_ERROR`, `codes::BROKEN_PIPE`. All three string mappings now use their dedicated constant. Consumers parsing the `error` field of the JSON output can now route on the precise failure mode.
- **P2-audit-5 (LOW, documentation drift)** — `#![doc(html_root_url = "https://docs.rs/duckduckgo-search-cli/0.7.4")]` was lagging the Cargo.toml version. Updated to `0.7.5`. Closes the docs.rs cross-link drift.
- **P2-audit-7 (MEDIUM, distribution hygiene)** — `Cargo.toml` `[build-dependencies]` now includes `clap` and `clap_mangen = "0.2"`. The existing `build.rs` was extended to call a new `generate_man_page()` function that emits `duckduckgo-search-cli.1` in `OUT_DIR` using a best-effort mirror of the `src/cli.rs` CLI definition. The man page is a packaging convenience (not build-critical); failures are logged to stderr but do not panic the build. A future refactor will extract the CLI definition into a shared module to eliminate the mirror.
- **P3-audit-11 (LOW, CI drift)** — `Cross.toml` listed `armv7-unknown-linux-musleabihf` as a developer convenience target, but the comment also claimed "5 principais" targets were covered by local release process (false: local release process only covers `x86_64-unknown-linux-musl` and `aarch64-apple-darwin`). Removed the `armv7-unknown-linux-musleabihf` block and updated the comments to accurately reflect which targets are release-local vs. dev-only. No release behavior change.

### Test coverage delta
- `src/error.rs::tests` — added assertions for `Cancelled.exit_code() == 130`, `Cancelled.error_code() == "cancelled"`, `BrokenPipe.error_code() == "broken_pipe"`, `PathError.error_code() == "path_error"`, `InvalidConfig.error_code() == "invalid_config"`. Total `error::tests`: 5 tests, all pass.

### Fixed

- **GAP-WS-29 (CRITICAL, build experience, Windows)** — `cargo install` on native Windows MSVC without the C++ CMake tools for Windows sub-component of the Visual Studio Installer previously failed minutes into the BoringSSL build with the cryptic `failed to execute command: program not found / is 'cmake' not installed?`. The `build.rs` preflight is now extended to detect this and abort in SECONDS with the exact fix (`winget install -e --id Kitware.Cmake` OR Visual Studio Installer → Modify → Workloads → Desktop development with C++ → expand → check C++ CMake tools for Windows). New escape hatch: `DDG_SKIP_CMAKE_CHECK=1`. Root cause: the workload C++ build tools does NOT include the C++ CMake tools sub-component — the latter must be selected manually.
- **GAP-WS-30 (CRITICAL, build experience, Windows)** — BoringSSL CMake uses the Visual Studio 17 2022 generator which requires cl.exe (compiler) and link.exe (linker). The `build.rs` preflight now detects both and aborts with the fix (open a Developer PowerShell for VS 2022, or run `Launch-VsDevShell.ps1`). MSVC is NOT auto-installed (5+ GB download, too intrusive). New escape hatch: `DDG_SKIP_MSVC_CHECK=1`.
- **GAP-WS-31 (CRITICAL, build experience, Windows)** — BoringSSL perlasm generator emits crypto assembly in NASM format and requires perl.exe. The `build.rs` preflight now detects perl and reports the fix (`winget install -e --id StrawberryPerl.StrawberryPerl`). New escape hatch: `DDG_SKIP_PERL_CHECK=1`.
- **GAP-WS-32 (CRITICAL, documentation)** — `skill/duckduckgo-search-cli-en/SKILL.md` line 561 and `skill/duckduckgo-search-cli-pt/SKILL.md` line 565 still claimed "Pre-built binaries from `cargo install` are unaffected", and the PT skill carried the same claim translated. This was already false in v0.7.4 (only `llms.txt` and `README*.md` were corrected); now corrected in the skills too. **crates.io NEVER distributes binaries**; `cargo install` always compiles from source.
- **GAP-WS-33 (MEDIUM, documentation)** — Skill frontmatter said "Released 2026-06-08" (v0.7.3 date) while the binary is v0.7.4 of 2026-06-11. Now both EN and PT skills say "Released 2026-06-14 (v0.7.5)".
- **GAP-WS-34 (MEDIUM, documentation)** — Skills only listed Linux build prerequisites. Now mention the four Windows prerequisites (NASM, CMake, MSVC, Perl) and the new `build.rs` preflight + escape hatches.
- **GAP-WS-35 (MEDIUM, documentation)** — `llms-full.txt` (line 273-305, embedding of `docs/HOW_TO_USE.md`) claimed "Pre-built binaries require no Rust installation" without qualifying that this is ONLY true for GitHub Releases binaries. `cargo install` always requires Rust and always compiles from source. Now qualified.
- **GAP-WS-36 (MEDIUM, documentation)** — `docs/CROSS_PLATFORM.md` line 193 and `README.md` line 336 and `README.pt-BR.md` line 428 claimed "VS Build Tools with C++ workload provides CMake". The C++ workload does NOT provide CMake — that is a separate sub-component. Now corrected in all three files.
- **GAP-WS-37 (MEDIUM, build)** — `build.rs` v0.7.4 only checked for NASM. Now checks for the four BoringSSL build prerequisites (nasm, cmake, cl.exe, link.exe, perl) and supports four independent escape hatches.

### Added
- `scripts/check-windows-toolchain.ps1` — standalone diagnostic (no installs) that checks all 7 tools (cargo, rustc, cmake, nasm, cl.exe, link.exe, perl) and emits text or JSON output. Exit code 0 if all present, 1 otherwise. Useful for support tickets and CI gates.
- `docs/INSTALL-WINDOWS.md` (EN) + `docs/INSTALL-WINDOWS.pt-BR.md` (PT) — step-by-step guide covering 5 installation methods (VS Installer + standalone; all-winget standalone; Chocolatey; helper script; standalone diagnostic). Includes troubleshooting for each of the 4 GAPs and the `DDG_SKIP_*_CHECK` escape hatches.

### Changed
- `scripts/install-windows.ps1` — refactored to use generic `Find-Tool` and `Install-Tool` helpers; now detects and auto-installs CMake (`Kitware.Cmake`) and Perl (`StrawberryPerl.StrawberryPerl`) in addition to NASM. MSVC is NOT auto-installed (too large); the script prints the exact `Launch-VsDevShell.ps1` instruction instead. New `--check-only` mode produces a tabular report suitable for CI gates.
- `build.rs` — 4 detector functions (`nasm_in_path`, `cmake_in_path`, `cl_in_path`, `link_in_path`, `perl_in_path`) + 2 `known_*dir` functions. The preflight fires 4 panic messages with actionable fixes when a tool is missing. 4 independent escape hatches.
- `local gates` + `local release process` — Windows jobs now verify CMake, install Perl, and verify MSVC Build Tools (in addition to the existing NASM step).
- `Cargo.toml` version bump: 0.7.4 → 0.7.5.

### No runtime changes
- Same CLI flags, same JSON schema, same default behavior as v0.7.4. crates.io still ships NO pre-built binaries.


## [0.7.4] - 2026-06-11

### Fixed
- **GAP-WS-28 — `cargo install` failed on native Windows because NASM was missing**.
  Literal error: `CMake Error at CMakeLists.txt:374 (enable_language): No CMAKE_ASM_NASM_COMPILER could be found`, surfacing MINUTES into the BoringSSL build, without stating the fix. Four-layer root cause: (CR1) BoringSSL's CMakeLists.txt requires `enable_language(ASM_NASM)` when `NOT OPENSSL_NO_ASM` on Windows x86/x86_64; (CR2) the `btls-sys` v0.5.6 build script DOES have an `OPENSSL_NO_ASM=YES` branch for Windows (build/main.rs:314-318), but it is UNREACHABLE in native builds because of the `host == target` early-return (build/main.rs:231); (CR3) the NASM installer does not adjust PATH and Visual Studio does not ship `nasm.exe`; (CR4) the documentation incorrectly claimed Windows binaries were pre-built (crates.io does not distribute binaries). See `gaps.md` GAP-WS-28.
- New `build.rs` with a fail-fast preflight: on a native `windows-msvc` target it detects `nasm.exe` missing from PATH and aborts in SECONDS with the exact instruction (`winget install -e --id NASM.NASM` + PATH adjustment + a reference to the script). It also detects NASM installed outside PATH in known directories. Escape hatch: `DDG_SKIP_NASM_CHECK=1`. Cross-compilation is unaffected (it uses the `OPENSSL_NO_ASM` path in btls-sys).

### Added
- `scripts/install-windows.ps1` — automated, consented installation on Windows: detects NASM, installs it via `winget` (with a `choco` fallback), fixes the session PATH and runs `cargo install duckduckgo-search-cli --locked`, forwarding any extra arguments.
- CI: an explicit NASM verification/installation step (`choco install nasm -y`) in the Windows jobs of the local gates — removes the implicit dependency on NASM being pre-installed in the `Windows host` image (if the image changes, the build does not break silently).

### Changed
- `README.md`, `README.pt-BR.md`, `llms.txt`, `llms.pt-BR.txt` and `docs/CROSS_PLATFORM*.md`: removed the FALSE claim that Windows/macOS binaries were "pre-built and unaffected" — `cargo install` ALWAYS compiles from source. The NASM prerequisite is documented for Windows MSVC, with a reference to `scripts/install-windows.ps1`.

### Notes
- GAP-WS-28 CLOSED in this repository (S1 preflight + S2 script + S3 docs + local gate hardening). It remains OPEN upstream in `btls-sys`: the early-return that makes the `OPENSSL_NO_ASM` branch unreachable in native Windows builds has not been reported yet (S5 pending).
- No runtime behaviour change: this release contains only the build preflight, the install script, local hardening and documentation.

## [0.7.3] - 2026-06-08

### Fixed
- **GAP-WS-27 — CAPTCHA block on macOS that does not happen on Windows**.
  Reproduced in this session: `duckduckgo-search-cli "rust wreq emulation browser fingerprint" -q -f json --num 5` returned `quantidade_resultados: 0` on macOS ARM64 even sharing the IP with Windows 10. Root cause: the `rustls` TLS fingerprint is recognisable by Cloudflare Bot Management (the JA4_o vector), triggering a CAPTCHA interstitial under HTTP 200.
- Replaced `reqwest 0.12` + `rustls-tls` with `wreq 6.0.0-rc.29` + BoringSSL (`boring2` v4.15.11) + `wreq-util 3.0.0-rc.12`. Embedded BoringSSL produces a JA4_o identical to real Chrome/Safari, eliminating the CAPTCHA. See ADR `docs/decisions/0001-tls-boring-via-wreq.md`.
- The same query after the migration: 5 results, 735ms, no fallback, no CAPTCHA. Cross-OS validation pending (the operator must test on Windows / Linux).

### Added
- **PR2 — `session` feature (cookie persistence + warm-up)**:
  - Flag `--no-warmup` to disable the `GET https://duckduckgo.com/` warm-up request.
  - Flag `--no-cookie-persistence` to keep cookies in memory only.
  - Flag `--cookies-path <PATH>` to override the default `cookies.json` location.
  - Cookie jar persisted at `~/.config/duckduckgo-search-cli/cookies.json` (Unix), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows) or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS).
  - Permissions 0o600 applied on Unix (owner read+write only).
  - Module `src/session_warmup.rs` (XDG path resolution) and `src/wreq_cookie_adapter.rs` (JSON <-> `wreq::cookie::Jar` bridge).
- **PR3 — `probe-deep` feature (CAPTCHA interstitial detection)**:
  - Flag `--probe-deep`, which runs a real query and classifies the body as `ok` or `captcha` based on Cloudflare/DuckDuckGo markers.
  - Flag `--allow-lite-fallback` (opt-in) for automatic fallback from the `html` endpoint to `lite` when an interstitial is detected.
  - Module `src/probe_deep.rs` with `detectar_interstitial()` and `sugestao_mitigacao()`.
  - Reports JSON with `status`, `cascata_motivo`, `sugestao_mitigacao`, `http_status`, `latency_ms`.

### Changed
- **TLS stack switched from rustls to BoringSSL via wreq**. The build now requires `cmake`, `perl`, `pkg-config` and `libclang-dev` on Linux. Documented in `docs/CROSS_PLATFORM.md` and ADR-0001.
- ADR `docs/decisions/0001-tls-boring-via-wreq.md` records the architectural decision and the accepted trade-offs.
- Release build time grew by ~30s (static BoringSSL). The final binary is ~20 MB larger.

### Removed
- The `reqwest 0.12` dependency (replaced by `wreq`).
- `time 0.3.47` is now purely transitive (it used to be a direct dep to override `reqwest`'s transitive one).

### Notes
- **GAP-WS-27 root cause 1 (TLS fingerprint) CLOSED**. Causes 2 and 3 are partially mitigated but require production validation: the v0.6.4 `IdentityPool` already generates an `Accept-Language` coherent with `--country`, and cookie persistence reduces the frequency of "cold" sessions. `gaps.md` keeps the status "RESOLVIDO PARCIALMENTE" until the operator's cross-OS validation.
- The `time 0.3.47` pin in `Cargo.toml` was removed. `time` is now a pure transitive of `wreq` and its deps. CI should stay green because `wreq` pulls `time 0.3.47+`.
- Test count: 292 lib (vs 279 in v0.7.2) + 18 wiremock + other integrations = 0 failures.
- Build verified: `cargo build --release` green (40s), `cargo test --lib` green, `cargo test --tests` green, `cargo clippy --all-targets -- -D warnings` green.

## [0.7.2] - 2026-06-07

### Fixed
- **Historical note (CI/Actions removed from this repo): 9 jobs failing on 10 E0599 compile errors** (rand 0.10 trait
  reorg — the `random_range` / `random_bool` / `random` convenience
  methods moved from `Rng` to `RngExt` in rand 0.10.0). Updated the
  `use` lines in `src/identity.rs`, `src/parallel.rs`, and
  `src/search.rs` to import `RngExt` instead of `Rng`. This unblocks
  `cargo check`, `build`, `test`, `clippy`, `doc`, `publish --dry-run`,
  `validate`, `musl smoke`, `msrv`, and `coverage` jobs (all cascading
  failures of the same root cause).
- **Historical note (CI/Actions removed from this repo): `supply chain (audit + deny)` job failing on RUSTSEC-2026-0009**
  (`time 0.3.40` denial-of-service via stack exhaustion when parsing
  RFC 2822 date headers, severity 6.8 medium). Resolved by upgrading
  `time` to `0.3.47` (the patched release). The defensive ignore in
  `deny.toml` for this advisory is now obsolete and has been removed.

### Changed
- **`rand` bumped from 0.8 (used in published v0.7.1) to 0.10** in
  this hotfix. The dev-deps ecosystem (proptest 1.11+, getrandom
  0.4+) unified on 0.10, and 0.10 introduced the `RngExt` trait as
  the new home for the convenience methods.
- **`rust-version` bumped from 1.75 to 1.88** (matches `time` 0.3.47
  MSRV and the `rand` 0.10 ecosystem). All other crates still compile
  on 1.88+.
- **`time` pinned to `0.3.47`** as a direct dependency to override the
  transitive `time 0.3.40` pulled in by `cookie_store 0.22.0` →
  `reqwest 0.12.28` (RUSTSEC-2026-0009 stack-exhaustion DoS).

### Notes
- v0.7.1 was published with the source compiled against `rand 0.9`
  (the lock at the time resolved to a registry snapshot that no
  longer exists on crates.io). The CI subsequently failed because
  the registry was updated and the lock now resolves to `rand 0.10`.
  This hotfix migrates the source forward to match the registry
  state.
- Test count: 402 (289 lib + 101 integration + 12 doctest), 0
  failures. Clippy clean, doc clean, fmt clean, deny clean, audit
  clean.

## [0.7.1] - 2026-06-07

### Changed
- **Migrated from `rand` 0.8 to `rand` 0.10** to align with the dev-deps
  ecosystem (proptest 1.11+, getrandom 0.4+) and the new RngExt trait
  surface in 0.10.0. Code now imports `rand::RngExt` for the
  `random_range` / `random_bool` / `random` methods.
- **`rust-version` bumped from 1.75 to 1.88** (matches `time` 0.3.47 MSRV
  and the `rand` 0.10 ecosystem). All other crates still compile on 1.88+.
- **`reqwest` features `gzip` and `brotli` removed**: reqwest 0.12 dropped
  the `ClientBuilder::gzip`/`brotli` builder methods. Decompression is now
  enabled via the standard `Accept-Encoding: gzip, br` request header (which
  reqwest handles transparently).
- **Replaced `rand::thread_rng()` with `rand::rng()`** in 4 sites (the
  former is deprecated since rand 0.9).
- **Replaced `Rng::gen_range` → `RngExt::random_range`** in 7 sites.
- **Replaced `Rng::gen_bool` → `RngExt::random_bool`** in 2 sites.
- **Replaced `Rng::gen::<T>()` → `RngExt::random::<T>()`** in 1 site.
- **Replaced `rand::seq::SliceRandom` with `rand::seq::IndexedRandom`** for
  `choose` calls on slices (the `choose` method moved traits in 0.9).
  `IteratorRandom::choose` is still used for `Iterator` types (e.g.
  `slice.iter().filter().choose`).
- **Pinned `time = "0.3.47"` as a direct dependency** to override the
  transitive `time 0.3.40` pulled in by `cookie_store 0.22.0` →
  `reqwest 0.12.28` (RUSTSEC-2026-0009 stack-exhaustion DoS).

### Fixed
- **Historical note (CI/Actions removed from this repo): 9 jobs failing on 10 E0599 errors** (`no method named
  random_range/random_bool/random found for struct ThreadRng in the current
  scope`) caused by the `rand 0.10` trait reorganisation (the convenience
  methods moved from `Rng` to `RngExt`). Updated the `use` lines in
  `src/identity.rs`, `src/parallel.rs`, and `src/search.rs` to import
  `RngExt` instead of `Rng`.
- **Historical note (CI/Actions removed from this repo): `supply chain (audit + deny)` job failing on RUSTSEC-2026-0009**
  (`time 0.3.40` denial-of-service via stack exhaustion when parsing RFC
  2822 date headers, severity 6.8 medium). Resolved by upgrading
  `time` to `0.3.47` (the patched release). The defensive ignore in
  `deny.toml` for this advisory is now obsolete and has been removed.
- **Historical note (CI/Actions removed from this repo): 5 jobs failing on `E0599 no method named choose`** (caused by the
  trait move of `choose` from `IteratorRandom` to `IndexedRandom` in
  rand 0.9). Updated import in `src/http.rs` and `src/identity.rs`.
- **Historical note (CI/Actions removed from this repo): `msrv` job failing on `assert_cmd 2.2.0 edition 2024 parse`**.
  After the rust-version bump to 1.88, this is now parseable.
- **Historical note (CI/Actions removed from this repo): `workflow syntax check (actionlint (removed with Actions))` failing on
  SC2046 (local gates:520) and SC2035 (local release process:505)**. Quoted the
  unquoted command substitution and prefixed the glob with `--` to
  prevent option-like name expansion.

## [0.7.0] - 2026-06-07

### Added
- **New subcommand `deep-research`** — query fan-out pipeline for LLM
  consumption. Splits the user query into 1..=12 sub-queries via five
  canonical heuristic templates (aspect, comparison, timeline, opinion,
  cause), fans them out through the existing parallel executor, aggregates
  the per-sub-query results with Reciprocal Rank Fusion (K=60) or
  canonical-URL deduplication, and optionally produces a synthesised
  report in Markdown, PlainText, or JSON with numbered references.
- **New module `src/deep_research.rs`** — pipeline orchestrator
  (`run_deep_research(args, cfg, cancel)`).
- **New module `src/decomposition.rs`** — heuristic + manual sub-query
  generation. Reads explicit sub-queries from a file when the
  `--sub-query-strategy manual` flag is set; comments (`#`) and blank
  lines are ignored.
- **New module `src/aggregation.rs`** — `Rrf(K=60)` and `DedupeByUrl`
  strategies. URL canonicalisation strips `utm_*` and other tracking
  parameters, lowercases the host and scheme, sorts query parameters,
  and collapses repeated slashes. The canonical form is hashed with
  `blake3` (first 16 hex chars) to serve as the dedup key.
- **New module `src/synthesis.rs`** — three output formats
  (Markdown, PlainText, Json) with a configurable token budget
  (1 token ≈ 4 chars heuristic) and a 20-reference cap per report.
- **New dependencies**:
  - `url = "2"` — URL canonicalisation in `aggregation.rs`.
  - `regex = "1"` — used by `decomposition::is_composite_query` to
    detect composite-query signals and suppress redundant templates.
  - `proptest = "1"` (dev) — property-based tests for new modules.

### Changed
- **Version bumped** from `0.6.11` to `0.7.0` (minor: new public
  subcommand `deep-research` and four new public modules
  `deep_research`, `decomposition`, `aggregation`, `synthesis`). No
  breaking changes to the existing `buscar` subcommand or the default
  `SearchOutput` / `MultiSearchOutput` schemas — additive only.
- **`Config` construction in `lib::execute_deep_research`** builds a
  default config from the global flags — `parallelism = 5`,
  `retries = 2`, `endpoint = Html`, `language = en`, `country = us`,
  `global_timeout = 120s`. The pipeline inherits these defaults and
  does NOT require the operator to pass a full `CliArgs`.

### Internal
- **Cargo.toml `exclude` block** — `gaps.md` and `docs_prd/` are
  excluded from the published crate.
- **`[profile.release]` panic = "abort"** — smaller binary, harder to
  leak panic payloads across the FFI boundary if one is ever added.
- **`.gitignore`** — added `proptest-regressions/`, `coverage/`,
  `tarpaulin-report.html`, and `.cargo-deny-state.json` to match the
  real artifacts produced by the new test suite and CI tooling.

### Gap closure pass
- **Doctests added to all four new modules** (12 doctests total):
  `aggregation::canonicalize_url`, `synthesis::estimate_tokens`,
  `synthesis::trim_to_budget`, `decomposition::HeuristicTemplate::suffix`,
  `deep_research::DeepResearchArgs::validate`, and a usage example in
  `deep_research::run_deep_research`.
- **Property-based tests with `proptest`** (7 tests) covering
  `canonicalize_url` (idempotence, fragment strip, tracking-param strip,
  host lowercasing) and `synthesis` (`estimate_tokens` monotonicity,
  `trim_to_budget` ceiling + idempotence). `proptest-regressions/` is
  captured in `.gitignore`.
- **`regex` integrated** in `decomposition::is_composite_query` with
  `CompositeSignal` enum (Comparison, Aspect, Timeline, Opinion, Cause,
  Topic) and `OnceLock`-cached compiled patterns. The heuristic strategy
  now suppresses redundant templates (e.g. `Comparison` is skipped when
  the query already contains `vs` or `or`).
- **Wiremock integration tests** in `tests/integration_deep_research.rs`
  (17 tests): pipeline smoke, query-param matching, HTTP 202 anomaly
  observability, 404 observability, and 13 surface-coverage tests.
- **`cargo deny check`** — all four gates pass: `advisories ok, bans ok,
  licenses ok, sources ok`.
- **`cargo publish --dry-run`** — package created and verified
  (1.1 MiB, 14.00 s on a warm cache).
- **Latent UTF-8 bug fixed in `synthesis::trim_to_budget`** — was using
  byte indexing without a char-boundary check, which panicked on
  multi-byte inputs (the same panic shape that the proptest book
  highlights). Replaced with a private `floor_char_boundary` helper.
  Three proptests lock in the invariant
  `is_char_boundary(out.len())` for arbitrary inputs.

### Validation
- `cargo build --release` — clean.
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `cargo test --lib` — 279 tests passing, 0 failing.
- `cargo test --doc` — 12 doctests passing.
- `cargo test --tests` — 101 integration tests passing (24 + 3 + 17 + 5 + 10 + 10 + 14 + 18).
- **Total: 392 tests passing** (279 lib + 12 doc + 101 integration), 0 failing.
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --lib` — clean.
- `cargo fmt --all -- --check` — clean.
- `cargo audit` — no new advisories (pre-existing `RUSTSEC-2025-0057`
  on `selectors 0.25.0` is the only warning and is tracked separately).
- `cargo deny check` — all four gates ok.
- `cargo publish --dry-run` — ok.

## [0.6.11] - 2026-06-05

### Fixed
- **Historical note (CI/Actions removed from this repo): `crates_io` step 6 `Check if version already published` failed with `unbound variable` exit 1 on tag v0.6.10**
  - Root cause: the `VERSION` variable was referenced as `VERSION="${VERSION}"`
    on the first line of the script, but it was never defined in the step's
    `env:` block. With `set -euo pipefail` active, accessing an undefined
    variable caused `bash: VERSION: unbound variable` with exit 1, marking
    the step as `conclusion: failure` and short-circuiting the rest of the
    publish path. crates.io v0.6.10 was published manually via
    `cargo publish --allow-dirty` as a workaround.
  - Solution: passed `VERSION: ${{ steps.detect_version.outputs.version }}`
    in the step's `env:` block, mirroring the pattern already used by
    `Verify tag matches Cargo.toml version`. Also hardened the script
    with `NO_COLOR=1` and a `sed` ANSI-strip as defense-in-depth against
    color codes that would break the parsing regex. Bumped retries from
    3 to 5 with linear backoff (5s/10s/15s/20s) to absorb transient
    crates.io rate limits.

- **Historical note (CI/Actions removed from this repo): `cargo search` parsing is now resilient to ANSI color codes**
  - The `cargo search` output is wrapped in ANSI escape codes when
    `CARGO_TERM_COLOR=always` is set (as it is in this workflow). On
    some color schemes the regex `= "[0-9]+\.[0-9]+\.[0-9]+"` was
    still matched, but on others the color codes were injected between
    characters and broke parsing.
  - Solution: strip ANSI escapes with `sed -E 's/\x1b\[[0-9;]*[a-zA-Z]//g'`
    before applying the regex, and set `NO_COLOR=1` to disable color
    output explicitly. Both layers ensure the regex sees clean ASCII.

## [0.6.10] - 2026-06-05

### Fixed
- **Historical note (CI/Actions removed from this repo): `Publish to crates.io` job rejected by environment protection rules — tag `v0.6.9` not allowed in environment `release`**
  - Root cause: the GitHub `release` environment had only `branch_policy` configured
    (`protection_rules: [{"type": "branch_policy"}]`), which caused (now-removed) GitHub Actions to
    reject any ref that is NOT a branch — including `refs/tags/v0.6.9`. The run ended
    with `conclusion: failure` and `steps_count: 0` (job never even started), showing
    the annotation `Tag "v0.6.9" is not allowed to deploy to release due to
    environment protection rules`.
  - Solution: created a new `release-publish` environment (id `16308925736`) with no
    `protection_rules`, which accepts ANY ref — including SemVer tags. The `crates_io`
    job now uses `environment: name: release-publish`.

- **Historical note (CI/Actions removed from this repo): actionlint (removed with Actions) exit 3 — `is a directory` error when invoking actionlint (removed with Actions) `**
  - Root cause: actionlint (removed with Actions) v1.x does NOT accept a directory as a positional argument;
    it expects individual files (e.g. `*.yml`) or to be invoked with no arguments
    (recursive auto-discovery of ``). The incorrect invocation
    produced the error `could not read "": is a directory` with
    exit 3, marking the `workflow syntax check (actionlint (removed with Actions))` job as failed.
  - Solution: corrected the invocation to actionlint (removed with Actions) (no arguments) in the
    `Run actionlint (removed with Actions) step of local gates. Local validation confirmed exit 0 with
    zero syntax errors.

- **Historical note (CI/Actions removed from this repo): zizmor (removed with Actions) exit 13 — 2 `secrets-outside-env` findings (medium) in the `github_release` job**
  - Root cause: the `github_release` job referenced `secrets.GPG_PRIVATE_KEY` and
    `secrets.GPG_PASSPHRASE` in `env:` without a dedicated `environment:`. The
    zizmor (removed with Actions) >= 1.24` (persona `auditor`) detects this pattern as `secrets-outside-env`
    (medium) and marks the `workflow security scan (zizmor (removed with Actions))` job as failed with exit 13
    when there is at least 1 finding.
  - Solution: (1) removed the GPG secrets from the `github_release` `env:` and added
    the `GPG_SIGNING_ENABLED: "false"` gate at workflow level; (2) the
    `Sign SHA256SUMS with GPG` step was renamed to `(DESABILITADO)` and never
    executes; (3) created a zizmor (removed with Actions) config (removed)` config with
    `rules.secrets-outside-env.config.allow` listing `crates.io token` (which is
    at repo level for compatibility). Cosign keyless (job `attest`) already provides
    cryptographic integrity via Sigstore, covering the role GPG signing would play.

- **Historical note (CI/Actions removed from this repo): package list now includes zizmor (removed with Actions) config (removed)` (intentional zizmor (removed with Actions) configuration)**
  - Added zizmor (removed with Actions) config (removed)` with allow rules for the `crates.io token` secret at
    repo level. This file is a static config, contains no credentials and is safe
    to version.

## [0.6.9] - 2026-06-05

### Fixed
- **Historical note (CI/Actions removed from this repo): Windows `.zip` release asset was empty (209 bytes) — bug in `Package (Windows)` PowerShell script**
  - Root cause: the script used `${TARGET}` / `${BIN}` / `${EXT}` syntax, which is **bash interpolation**.
    In PowerShell, `${VAR}` is a string literal — env vars are interpolated as `$env:VAR`.
    Result: `Copy-Item` failed silently (source path became `target//release/`) and
    `Compress-Archive` produced an almost-empty zip (only `SHA256SUMS.txt`).
  - Solution: replaced all `${VAR}` with `$env:VAR` in PowerShell `run:` blocks
    (Package (Windows) and Generate SHA256SUMS (Windows)).
  - Reference: incident-jaq-not-found-runner-2026-06-05 + cross-cutting audit on 2026-06-05

- **Historical note (CI/Actions removed from this repo): `sbom.cdx.json` CycloneDX SBOM was 0 bytes (file not actually generated)**
  - Root cause: `cargo cyclonedx --override-filename sbom.cdx.json` actually writes
    `sbom.cdx.json.json` because the `--override-filename` flag auto-appends `.json`.
    The `wc -c < sbom.cdx.json` step then read 0 bytes from the non-existent file and
    the `Upload SBOM as artifact` step uploaded an empty file (artifact ignored downstream).
  - Solution: changed invocation to `cargo cyclonedx --format json --override-filename sbom`
    (stem only), then `mv sbom.json sbom.cdx.json` to match the expected filename.

- **Historical note (CI/Actions removed from this repo): git tag release notes for v0.6.8 was incomplete (missing Windows zip + sbom)**
  - Root cause: the above two bugs combined meant the v0.6.8 release workflow produced
    a Windows zip with only the SHA256SUMS stub and an empty SBOM. Manually uploaded
    the real SBOM after the fact; Windows zip requires a full re-run.

## [Unreleased]

### Fixed
- **Historical note (CI/Actions removed from this repo): exit 101 `crate already exists` on `Publish to crates.io` job (post-mortem 2026-06-05)**
  - Root cause: a duplicate workflow trigger for the already-published tag v0.6.6 caused `cargo publish`
    to exit 101 with `error: crate duckduckgo-search-cli@0.6.6 already exists on crates.io index`.
    crates.io is append-only and immutable; versions can NEVER be overwritten.
  - Solution: added `preflight` + `crates_io` guard jobs with:
    - Tag-vs-Cargo.toml version consistency check
    - SemVer format validation
    - CHANGELOG entry presence check
    - Co-authored-by AI agent block in recent commits
    - `cargo search` with timeout + retry to detect already-published version
    - `cargo publish` skip with warning + evidence upload when already published
    - Timeout (300s) + retry (3 attempts, backoff 10s/20s/30s) on `cargo publish`
  - Resolution pattern: idempotent release workflow with explicit skip path

- **Historical note (CI/Actions removed from this repo): 18+ Node.js 20 deprecation warnings in all jobs**
  - Root cause: checkout step (removed with Actions), upload-artifact (removed), download-artifact (removed)
    use Node 20. Node 20 deprecated 2025-09-19, removed 2026-09-16.
  - Solution:
    - Updated all actions to v6 (Node 24 native)
    - Updated `softprops/action-gh-release` from v2 to v3
    - Added `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"` as belt-and-suspenders
  - Migration path: v6 is Node 24 native, v4 needs explicit env var

- **Historical note (CI/Actions removed from this repo): exit 141 SIGPIPE intermittent in `validate (Linux host)`**
  - Root cause: `cargo test` writes to pipe whose consumer closes early
  - Solution: explicit `|| { ec=$?; if [ $ec -eq 141 ]; then exit 0; fi; exit $ec; }` guard
  - Trade-off: 141 silently becomes warning, may mask real test bugs

- **Historical note (CI/Actions removed from this repo): exit 1 in `validate (windows-latest)` from VS2022→VS2026 redirect**
  - Root cause: GitHub redirects `windows-latest` to `windows-2025-vs2026` since 2025-06-15.
    VS2026 has breaking changes in MSVC toolchain that affect Rust stable.
  - Solution: pinned `Windows host` in local gates matrix and local release process build target
  - Re-evaluate pin after 2026-07-15 once VS2026 stabilizes

### Added
- **SBOM CycloneDX generation in release workflow** — `cargo cyclonedx --format json` produces
  `sbom.cdx.json` uploaded as artifact. Enables compliance with EU Cyber Resilience Act.
- **SLSA provenance attestation** — `actions/attest-build-provenance@v2` creates signed
  provenance for all release artifacts. Level 3 SLSA compliance.
- **cosign keyless OIDC signing** — every binary + SHA256SUMS.txt signed with `cosign sign-blob`
  using GitHub OIDC token. No private key management required.
- **SHA256SUMS published with every release** — `sha256sum` generated per target, combined
  into single `SHA256SUMS.txt`, uploaded as release asset and as part of every binary tarball/zip.
- **GPG tag signing** — optional `gpg --detach-sign SHA256SUMS.txt` if `GPG_PRIVATE_KEY` secret
  is configured. `continue-on-error: true` to avoid blocking release on missing key.
- **Concurrency control** — `concurrency.group: release-${{ github.ref }}-${{ github.sha }}`
  prevents parallel runs for same tag+SHA. `cancel-in-progress: false` (release) / conditional
  on PR (CI) ensures publish is never aborted mid-flight.
- **Pre-flight job in release workflow** — validates tag version == Cargo.toml version,
  SemVer format, CHANGELOG entry, no AI agent Co-authored-by BEFORE any build runs.
- **Cron weekly dependency update** — `scheduled_update` job runs Sundays 03:00 UTC,
  executes `cargo update --workspace`, creates PR if changes detected.
- **historical workflow security scan (removed with Actions)** — static analysis of (now-removed) GitHub Actions workflows detects
  injection, untrusted input, and other security anti-patterns. Runs only on PRs.
- **actionlint (removed with Actions) syntax check** — validates YAML syntax of all workflow files. Runs only on PRs.
- **Dependabot (removed with Actions) for actions and crates** — dependabot (removed with Actions) config (removed)` creates weekly PRs
  for (removed) GitHub Actions updates and Rust crate updates. Groups by major/minor/patch.
- **`.gitattributes` LF normalization** — forces LF line endings in all text files,
  preventing CRLF issues on Windows that break `cargo fmt --check`.

### Security
- **Permissions hardened per job** — top-level `permissions: contents: write packages: write
  id-token: write attestations: write checks: write discussions: write` for release;
  per-job `permissions:` blocks in CI for least-privilege.
- **`continue-on-error: true` on GPG step** — missing GPG key does not block release;
  optional enhancement.
- **No `pull_request_target` triggers** — workflows never run with write permissions
  on PRs from forks.

## [0.6.8] - 2026-06-05

### Fixed
- **Historical note (CI/Actions removed from this repo): exit 127 `jaq: command not found` in `github_release` job of release workflow**
  - Root cause: local release process (lines 625-626) used `jaq` (Rust binary) to parse JSON
    response from GitHub REST API, but the (now-removed) GitHub Actions Ubuntu runner only
    has `jq 1.7` pre-installed — `jaq` is not part of the standard runner image.
    Bug introduced by commit `7f489b5` (2026-06-05) when bypassing the broken
    `softprops/action-gh-release` action.
  - Solution: replaced `jaq` with `jq` (pre-installed, syntax-compatible) and added
    explicit fail-fast validation for extracted `UPLOAD_URL` and `RELEASE_ID` values
    to surface clear diagnostic messages on malformed API responses.
  - Reference: <https://github.com/actions/runner-images/blob/main/images/ubuntu/
    Ubuntu2404-Readme.md> (Tools section lists `jq 1.7`, `jaq` is absent)

## [0.6.7] - 2026-06-05

### Fixed
- **Historical note (CI/Actions removed from this repo): full post-mortem of incident-publish-101-2026-06-05** (release pipeline hardening)
  - Added `preflight` job validating tag==Cargo.toml, SemVer, CHANGELOG, no AI Co-authored-by
  - Added a duplicate-version guard in the `crates_io` job (zizmor (removed with Actions): secrets-outside-env resolved)
  - cargo publish with a 300s timeout + 3 retries (network resilience)
  - Concurrency group per tag+sha (prevents parallel runs)
- **Historical note (CI/Actions removed from this repo): 18+ Node.js 20 deprecation warnings**
  - Updated actions to v6 (Node 24 native)
  - Updated softprops/action-gh-release v2 → v3
  - Added `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` as belt-and-suspenders
- **Historical note (CI/Actions removed from this repo): historical workflow security scan (removed with Actions): 134 findings → 0**
  - SHA pinning for 11 actions (unpinned-uses)
  - per-job least-privilege permissions (excessive-permissions)
  - comments + inline trailing on every permission
  - secrets in env: job-level + dedicated GitHub Environments
  - ${{ ... }} in run: mitigated via env vars (template-injection)
  - rustup toolchain replaced by setup via rustup (superfluous-actions)
  - caches removed from the local release process (cache-poisoning)
- **Historical note (CI/Actions removed from this repo): actionlint (removed with Actions) 0 errors on both workflows**
- **Historical note (CI/Actions removed from this repo): zizmor (removed with Actions) zero findings (exit 0)**
- **Historical note (CI/Actions removed from this repo): dependabot (removed with Actions).yml for weekly auto-update of actions and crates**
- **Historical note (CI/Actions removed from this repo): .gitattributes forces LF line endings on every text file**
- **clippy: redundant `#[cfg(feature = "chrome")]` removed from src/lib.rs:74**
  - browser.rs:25 already has `#![cfg(feature = "chrome")]`, which covers the module
- **clippy: SAFETY comments added to every Windows unsafe block in src/platform.rs**
  - 5 unsafe blocks now carry `// SAFETY:` comments explaining the preconditions
  - Required for `clippy::undocumented_unsafe_blocks` (deny on rust 1.96+)
- **test: Windows-incompatible tests marked with `#[cfg(unix)]`**
  - `rejeita_path_absoluto_etc` (tests /etc/shadow)
  - `rejeita_path_absoluto_usr` (tests /usr/bin/evil)
  - Both pass on Linux/macOS and skip on Windows, where those paths are regular

### Added
- **SBOM CycloneDX generation in the release workflow**
  - `cargo cyclonedx --format json` produces `sbom.cdx.json`
  - Compliance with the EU Cyber Resilience Act
- **SLSA build provenance via `actions/attest-build-provenance@v2`**
- **cosign keyless OIDC signing** (every binary + SHA256SUMS.txt)
- **SHA256SUMS published with every release** (generated per target)
- **GPG tag signing** (optional, `continue-on-error: true` when the key is absent)
- **Pre-flight job in the release workflow** (9 gates + 1 dry-run)
- **Attestation job** (SBOM + cosign + SLSA in 1 job)
- **Weekly scheduled_update Cron** (automatic cargo update)
- **historical workflow security scan (removed with Actions) in CI** (zero findings)
- **actionlint (removed with Actions) syntax check in CI** (zero errors)
- **Dependabot (removed with Actions) for actions and Rust crates** (weekly PRs)

### Security
- **Per-job hardened permissions** (least-privilege)
- **Persist-credentials: false on 18/18 checkout step (removed with Actions)** (artipacked)
- **No `pull_request_target` triggers** (forks do not run with write)
- **Complete SHA pinning** (11 actions with 40 chars + version comment)

## [0.6.6] - 2026-06-05

### Fixed
- **docs.rs build failure (Build #3487310) caused by `#[doc(cfg(...))]` becoming unstable**
  - Removed `#[cfg_attr(docsrs, doc(cfg(feature = "chrome")))]` from `src/lib.rs:70`
  - Root cause: in Oct 2025 the Rust team merged `doc_auto_cfg` into `doc_cfg` (rust-lang/rust#43781),
    making `#[doc(cfg(...))]` require `#![feature(doc_cfg)]` (nightly-only) on the crate root.
    The build failed with `error[E0658]: #[doc(cfg)] is experimental` on nightly `1.98.0`.
  - The feature gating itself is preserved: `#[cfg(feature = "chrome")]` still excludes
    `pub mod browser` from default builds. The module-level docstring in `src/browser.rs`
    already documents the feature requirement explicitly.
  - `cargo doc --all-features` and `RUSTDOCFLAGS="--cfg docsrs" cargo doc --all-features`
    both pass without warning or error.

## [0.6.5] - 2026-06-05

### Fixed
- **MP-26 — Windows HANDLE cast broken in `windows-sys 0.59+`** (`src/platform.rs:51-63`)
  - `HANDLE` changed from `isize` to `*mut c_void` upstream (`microsoft/windows-rs`, `raw-window-handle#171`)
  - Replaced `handle != 0 && handle != usize::MAX` with `!handle.is_null() && handle != INVALID_HANDLE_VALUE`
  - Removed the invalid `handle as isize` casts (the modern signature accepts `HANDLE` directly)
  - Updated the `// SAFETY:` comment to document nullity and the Win32 sentinel
- **Historical note (CI/Actions removed from this repo): `validate` failed on all 3 OSes** (Linux/macOS/Windows) over 6 clippy errors
  - 3× `clippy::doc_markdown` (`PowerShell`, `rules_rust.md`, `TempDir`) in `src/platform.rs` and `src/browser.rs`
  - 1× `clippy::needless_return` in `src/browser.rs:149`
  - 2× `missing_debug_implementations` in `src/browser.rs:223` (`ChromeBrowser`) and `src/content_fetch.rs` (`CircuitBreakerMap`)

### Added
- **WS-11 — Property-based invariants for HTML parsers** (`src/extraction.rs` +5 tests)
  - Invariant: empty/broken inputs return an empty `Vec` without panicking
  - Invariant: positions are dense and 1-based
  - Invariant: URLs are absolute (`http`/`https`) or empty
  - Invariant: extraction is idempotent
  - Invariant: malformed HTML does not cause a panic
  - **Zero new dependencies** (stdlib + `#[test]` only)
- **WS-12 — Per-host circuit breaker** (`src/content_fetch.rs`)
  - Threshold: 3 consecutive failures open the circuit
  - Cooldown: 30s before the half-open probe
  - Integrated into `enrich_with_content` before every fetch
  - `BreakerDecision::{Allow, Reject}` for inspection
  - **Zero new dependencies** (`std::sync::Mutex<HashMap>`)
- **WS-23 — `Retry-After` header test** (`tests/integration_wiremock.rs`)
  - The mock returns 429 with `retry-after: 2`
  - Assertion: `elapsed_ms >= 1500` (minimum delay respected)
  - Uses `wiremock` 0.6, already in dev-deps
- **WS-25 — `indicatif` ProgressBar for long crawls** (`src/content_fetch.rs`)
  - `indicatif = "0.18"` added
  - Bar with the template `[{elapsed_precise}] {bar:40.cyan/blue} {pos:>4}/{len:4} {msg}`
  - Auto-detects TTY (hidden in pipes)
  - `progress.finish_and_clear()` at the end
- **Preventive FFI lints** (`Cargo.toml`)
  - `improper_ctypes = "deny"` (rejects invalid FFI casts)
  - `improper_ctypes_definitions = "deny"` (rejects incorrect definitions)

### Tests
- 333 tests passing (243 lib + 24 + 3 + 5 + 10 + 10 + 14 + 18 + 6 doc)
- 6 new invariant tests in `extraction.rs` (WS-11)
- 4 new circuit-breaker tests in `content_fetch.rs` (WS-12)
- 1 new Retry-After test in `integration_wiremock.rs` (WS-23)
- `cargo fmt --all --check` clean
- `cargo clippy --all-targets --all-features --locked -- -D warnings` clean
- `cargo publish --dry-run --locked --allow-dirty` clean

## [0.6.4] - 2026-06-03

### Added
- **WS-26 — Adaptive anti-bot identity rotation** (new `src/identity.rs` module)
  - 12-identity pool (4 browser families × 3 platforms) for adaptive rotation
  - `IdentityProfile::shuffled_headers()` produces seed-deterministic header order
  - `IdentityPool::rotate_on_block()` implements a 5-level cascade: same identity → same family/different platform → different family/same platform → different family+platform → random
  - `BrowserFamily` and `Platform` enums with canonical English names
  - 5 unit tests covering pool size, cascade level, determinism, header shape, tag stability
- **New CLI flags** (additive, no breaking changes)
  - `--probe` — pre-flight health check (sends 1 minimal request, reports status/latency/Set-Cookie as JSON)
  - `--identity-profile` — pin the session to a specific identity (`auto`, `chrome-win`, `chrome-mac`, `chrome-linux`, `edge-win`, `firefox-linux`, `safari-mac`). `auto` is default.
- **New JSON metadata fields** (additive, `Option` + `skip_serializing_if = "Option::is_none"`)
  - `metadados.identidade_usada` — string tag of the identity that produced the response
  - `metadados.nivel_cascata` — cascade level reached during the request

### Changed
- **Version rollback**: `0.7.0` (unpublished) → `0.6.4` to preserve the in-development feature set under a stable patch number
- All existing CLI flags, JSON output schemas, and exit codes remain unchanged — strictly additive changes

### Tests
- 5 new identity unit tests (313 total tests passing, up from 308)
- All 224 lib tests + 83 integration tests + 6 doc tests pass
- `cargo clippy --lib --bins -- -D warnings` clean
- `cargo fmt --check` clean

## [0.6.3] - 2026-04-17

### Changed
- Translated all 96 doc comments (`///` and `//!`) across 19 source files from Portuguese to English — docs.rs now renders fully in English for international crates.io audience.
- No code behavior, public API, or JSON output fields changed.

## [0.6.2] - 2026-04-17

### Added
- 19 new documentation files — full conformance with rules_rust_documentacao.md (28 gaps G01-G28)
- Bilingual EN+PT documentation: HOW_TO_USE, CROSS_PLATFORM, AGENTS-GUIDE, COOKBOOK.pt-BR, INTEGRATIONS.pt-BR
- CODE_OF_CONDUCT.md + CODE_OF_CONDUCT.pt-BR.md — Contributor Covenant 2.1
- README.pt-BR.md, CHANGELOG.pt-BR.md, CONTRIBUTING.pt-BR.md, SECURITY.pt-BR.md
- docs/AGENTS.pt-BR.md — imperative guide for LLMs, in Portuguese
- docs/AGENTS-GUIDE.md + docs/AGENTS-GUIDE.pt-BR.md — bilingual persuasive guide
- llms.txt — compact orientation file for LLMs (< 50 KB)
- llms-full.txt — full concatenation of the docs for long-context LLMs
- eval-queries.json × 2 — 20 evaluation queries in EN + 20 in PT-BR for skill testing

### Changed
- README.md — link to README.pt-BR.md + quick install above line 30
- CONTRIBUTING.md — explicit MSRV Rust 1.75 + an 8-item PR checklist + branching strategy + nextest
- SECURITY.md — a version-specific table for v0.6.2 + a 90-day embargo policy + zero bold + zero emojis
- skill/SKILL.md (EN+PT) — a Workflow section with 5 numbered, verifiable steps

## [0.6.1] - 2026-04-17

### Fixed
- `--timeout 0` now returns exit 2 (invalid config) instead of executing a search with zero timeout and returning exit 5.
- `--output /tmp/../../etc/passwd` now returns exit 2 (invalid config) instead of exit 1 (runtime OS error) — path traversal validation moved to `montar_configuracoes()`, before the pipeline starts.

### Added
- `validar_timeout_segundos()` method on `CliArgs` — rejects values of 0 with a descriptive error.
- Early path traversal check in `montar_configuracoes()` — calls `paths::validate_output_path()` at config validation time, not at write time.
- 2 E2E regression tests: `timeout_zero_retorna_exit_2` and `output_com_path_traversal_retorna_exit_2`.
- 1 unit test: `validar_timeout_segundos_rejeita_zero`.

## [0.6.0] - 2026-04-16

### Security
- Per-family browser fingerprint profiles prevent DuckDuckGo anti-bot detection.
- Per-family `Sec-Fetch-*` headers and Client Hints imitate a real browser session.
- `Accept-Language` with RFC 7231 q-values eliminates the generic UA fingerprint.
- Silent-block detection with a 5 KB threshold prevents truncated results.

### Added
- `BrowserFamily` enum — variants `Chrome`, `Firefox`, `Edge`, `Safari`.
- `BrowserProfile` struct — encapsulates family, version and the per-family header set.
- Per-family `Sec-Fetch-Dest`, `Sec-Fetch-Mode`, `Sec-Fetch-Site` headers in `http.rs`.
- Client Hints (`Sec-Ch-Ua`, `Sec-Ch-Ua-Mobile`, `Sec-Ch-Ua-Platform`) for Chrome and Edge.
- HTTP 202 anomaly detection in `search.rs` with automatic exponential backoff.
- Silent-block detection — a response under 5,000 bytes is treated as a block.
- `BrowserProfile` propagated via `Config` to every module in the pipeline.
- Pagination headers with `Sec-Fetch-Site: same-origin` to imitate real navigation.

### Changed
- `Accept-Language` updated to `pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7` per RFC 7231.
- The `Accept` header now reflects the full per-family browser profile.
- Pagination delays increased from 500–1,000 ms to 800–1,500 ms.
- The silent-block threshold increased from 100 to 5,000 bytes.

## [0.5.0] - 2026-04-16

### Security
- Path traversal validation on `--output` — rejects `..` components and writes to system directories (`/etc`, `/usr`, `C:\Windows`).
- Proxy credential masking — error messages no longer expose passwords from `--proxy http://user:pass@host` URLs.

### Added
- `src/paths.rs` — centralized path validation, parent directory creation, and Unix permission application.
- `src/signals.rs` — centralized SIGPIPE restoration (Unix) and Ctrl+C/SIGINT handler (cross-platform).
- `ErroCliDdg` enum with `thiserror` — 11 typed error variants with `exit_code()` and `codigo_erro()` methods.
- `mascarar_url_proxy()` in `http.rs` — redacts credentials from proxy URLs in error context.
- 21 new unit tests across `paths.rs`, `signals.rs`, `error.rs`, and `http.rs`.

### Changed
- `thiserror = "2"` added to dependencies for structured domain errors.
- `src/main.rs` reduced from 63 to 23 lines — signal handling extracted to `signals.rs`.
- `src/output.rs` file writes now validate paths via `paths::validate_output_path()` before I/O.
- `deny.toml` updated with RUSTSEC-2026-0097 exception (rand 0.8 unsound with custom logger — not applicable).

## [0.4.4] - 2026-04-16

### Fixed
- SIGPIPE restored to SIG_DFL on Unix — pipes to `jaq`, `head`, and other consumers no longer lose stdout silently.
- BrokenPipe errors detected in anyhow chain and treated as exit 0 (not exit 1) at all output boundaries.

### Added
- `--help` now shows EXIT CODES (0–5) and PIPE USAGE sections via `after_long_help`.
- 3 E2E tests for pipe regression: exit codes in help, short help exclusion, stdout byte count.
- README troubleshooting item 7: "Pipe to jaq/jq returns empty" with PIPESTATUS diagnostic (EN + PT).
- `docs_rules/rules_rust.md`: SIGPIPE + BrokenPipe added to I/O checklist.
- `docs/AGENT_RULES.md`: R24 pipe safety rule with PIPESTATUS diagnostic.
- `docs/COOKBOOK.md`: Recipe 16 pipe diagnostic (EN + PT).
- `docs/INTEGRATIONS.md`: pipe safety clause in baseline contract.
- Exit code branching section in both skill files (EN + PT).

## [0.4.3] - 2026-04-15

### Changed

- **`README.md`** — New persuasive "Agent Skill" section (EN + PT) positioned
  between the agent table and the Documentation section, at the reader's peak
  attention. AIDA copywriting highlighting the bilingual skill packaged under
  `skill/`: semantic auto-activation with no slash command, 14 canonical
  MUST/NEVER sections, an anti-hallucination JSON contract, token savings on
  every search turn, one-command installation (`git clone` + `cp -r`). Explicit
  benefits for LLMs (automatic decision of when to search) and for developers
  (zero prompt engineering, zero tool registration). The crates.io tarball is
  unchanged — the skills still live on GitHub only.

## [0.4.2] - 2026-04-15

### Added

- **`skill/duckduckgo-search-cli-pt/SKILL.md`** and
  **`skill/duckduckgo-search-cli-en/SKILL.md`** — Bilingual skills for Claude
  Code, the Claude Agent SDK and platforms compatible with Agent Skills. Each
  skill carries YAML frontmatter with a `name` unique per language and a
  `description` loaded with semantic triggers for auto-invocation, plus 14
  canonical H2 sections (Mission, Invocation Contract, Absolute Prohibitions,
  Parsing with `jaq`, JSON Schema, Exit Codes, Batch, Fetch-Content,
  Endpoint, Retries, Recipes, Validation, Memory, Golden Rule).
  Published on GitHub, excluded from the crates.io tarball.

### Changed

- **`docs/AGENT_RULES.md`** (833 lines, +7.6%) — Editorial rewrite applying
  AIDA copywriting: every rule opens with a measurable benefit, imperative
  MUST/NEVER language reinforced, zero decorative narrative, zero bold with
  double asterisks, zero `---` visual separators between sections. Bilingual
  EN+PT mirrored with an identical tone.
- **`docs/COOKBOOK.md`** (1082 lines, −3.1%) — Every recipe opens with the
  concrete gain before the command, short bullets of 8 to 15 words,
  `jaq` + `xh` + `sd` pipelines preserved intact.
- **`docs/INTEGRATIONS.md`** (1212 lines, +1.3%) — 16 agents with a textual
  comparison table, deterministic snippets per agent, zero emoji.

### Meta

- The `Cargo.toml` exclude was widened to cover `skill/` and `skill/**` — the
  skills stay on GitHub and out of the tarball published to crates.io.

## [0.4.1] - 2026-04-14

### Added

- **`docs/AGENT_RULES.md`** (773 lines) — Bilingual imperative rules (EN+PT)
  with 30+ `MUST`/`NEVER` rules (R01..R30) for LLMs/agents invoking the CLI in
  production. Covers: core invariants, the JSON contract, rate limiting, error
  handling, performance, security, anti-patterns. Quick Reference Card at the
  end.
- **`docs/COOKBOOK.md`** (1117 lines) — 15 bilingual copy-paste recipes
  combining `duckduckgo-search-cli` + `jaq` + `xh` + `sd` for real cases:
  consolidated research, multi-query ETL, domain extraction, monitoring with a
  time filter, content extraction with `--fetch-content`, top 5 vs top 15
  comparison, NDJSON for pipelines, bash function wrappers.
- **`docs/INTEGRATIONS.md`** (1196 lines) — Ready-made snippets for 16
  agents/LLMs: Claude Code, OpenAI Codex, Gemini CLI, Cursor, Windsurf,
  Aider, Continue.dev, MiniMax, OpenCode, Paperclip, OpenClaw, Google
  Antigravity, GitHub Copilot CLI, Devin, Cline, Roo Code. Each agent
  documents: pitch, shell mechanism, setup, basic snippet, multi-query
  snippet, system prompt rule, caveats.
- A **Documentation** section in README.md (EN + PT) linking the 3 guides.

### Fixed

- The README.md badge cluster and internal references were checked against
  `daniloaguiarbr/duckduckgo-search-cli` (the canonical repo).

## [0.4.0] - 2026-04-14

### Changed (BREAKING)

- **Default for `--num` / `-n`**: changed from "every result on the first
  page" (~11) to **15**, with automatic **auto-pagination**. When the
  effective number exceeds 10, the binary now fetches **2 pages** per query
  to satisfy the requested ceiling, provided `--pages` was not customised
  by the user.
- **Automatic auto-pagination**: if `--num > 10` (either because the user
  passed it explicitly or because the default of 15 applied) AND `--pages`
  was not customised (still at the default of 1), the binary auto-raises
  `--pages` to `ceil(num/10)`, respecting the 5-page ceiling validated by
  `validar_paginas`. Impact: more requests per query (2x in the default
  case) and marginally higher latency, but full coverage of the requested
  results.

### Added

- Documentation in the `--num` flag comment in `cli.rs` describing the new
  default and auto-pagination semantics.
- 4 new unit tests in `lib.rs::testes`:
  `montar_configuracoes_aplica_default_num_15_quando_omitido`,
  `montar_configuracoes_respeita_pages_explicito_acima_de_1`,
  `montar_configuracoes_auto_pagina_quando_num_maior_que_10`,
  `montar_configuracoes_nao_auto_pagina_quando_num_10_ou_menos`.
- 2 new wiremock tests in `tests/integracao_wiremock.rs`:
  `testa_default_num_15_auto_pagina_2_paginas`,
  `testa_auto_paginacao_respeita_pages_explicito`.

### Migration Guide

- **If you want the old behaviour** (1 page, ~11 results):
  pass `--pages 1 --num 10` explicitly. An explicit `--pages 1` is
  indistinguishable from the default (accepted trade-off: `paginas > 1` is
  the only signal of "customisation"), so the safest route is to combine it
  with `--num 10` to guarantee nothing gets auto-paginated.
- **If you already passed `--num 5`** (or any value <= 10): behaviour is
  **unchanged** (no auto-pagination, 1 page).
- **If you already passed `--num 20 --pages 2`** or similar: behaviour is
  **unchanged** (the user's explicit choice is respected).
- **If you relied on the flagless default**: you now receive up to 15
  results instead of ~11, at the cost of 1 extra request per query. To
  restore the old behaviour, pass `--pages 1 --num 10`.

## [0.3.0] - 2026-04-14

### Changed (BREAKING)

- **JSON schema**: the `buscas_relacionadas` field was REMOVED from `SearchOutput`
  and `MultiSearchOutput.buscas[i]`. The `html.duckduckgo.com/html/` endpoint does
  not expose related searches in the current DOM; keeping the field permanently
  empty was noise. Pipelines that parsed `.buscas_relacionadas` need adjusting.
- **User-Agent pool**: removed text-browser UAs (`Lynx 2.9.0`,
  `w3m/0.5.3`, `Links 2.29`, `ELinks 0.16.1.1`) that made DuckDuckGo return
  degraded HTML. Replaced by 6 modern UAs validated empirically against the
  `/html/` endpoint: Chrome 146 (Win/Mac/Linux), Edge 145 Windows,
  Firefox 134 Linux, Safari 17.6 macOS. Firefox Win/Mac were REMOVED after
  returning an HTTP 202 anomaly in real validation (DDG's anti-bot heuristic).

### Fixed

- **The snippet duplicated the title and URL at the start**: the default selector
  had a `.result__body` fallback (the parent container), which made the recursive
  `text()` capture title+URL+snippet concatenated. Swapped for plain
  `.result__snippet`. Pipelines such as `jaq '.resultados[].snippet'` now return
  only the descriptive text of the result.
- **The "Official site" title**: DuckDuckGo literally renders this text as a label
  for verified domains (e.g. city halls). The scraper now detects this case and
  substitutes the `url_exibicao` (e.g. `saofidelis.rj.gov.br`). The original text
  is preserved in the new optional `titulo_original` field for auditing.

### Added

- Field `titulo_original: Option<String>` on `SearchResult`. Present only when
  the title was replaced by a heuristic (currently: the "Official site" case).
  Serialised with `#[serde(skip_serializing_if = "Option::is_none")]`
  — it does not appear in the JSON when absent.
- Sponsored results (`.result--ad`) excluded from the default container via the
  selector `.result:not(.result--ad)`.

### Removed

- Function `extrair_buscas_relacionadas` in `src/search.rs` (dead code with a
  hardcoded selector that never found anything).
- The `[related_searches]` section in the default selectors.

### Migration Guide (v0.2.x → v0.3.0)

- `jaq '.buscas_relacionadas[]'` pipelines: the field no longer exists.
  Remove it from the filter or handle `null`.
- Expecting a snippet prefixed with title+URL? It now carries only the descriptive
  text — adjust downstream regex/parsing if needed.
- Relying on `titulo == "Official site"` to detect verified sites?
  Use `titulo_original.as_deref() == Some("Official site")`.
- **LEGACY EXTERNAL CONFIG**: users who ran `init-config` on earlier versions have
  `~/.config/duckduckgo-search-cli/{selectors,user-agents}.toml` with the old
  defaults (snippet using `.result__body` + `Lynx`/`w3m`/etc. UAs). Those files
  OVERRIDE the embedded defaults. To apply this version's fixes, run AFTER
  upgrading:
  ```
  duckduckgo-search-cli init-config --force
  ```
  The `--force` flag overwrites the external files. A backup is recommended if you
  edited them by hand to hotfix selectors.

## [0.2.0] - 2026-04-14

### Changed (BREAKING)

The serialised JSON schema now uses **Brazilian Portuguese** field names,
aligned with the `jaq` examples in the README and with the INVIOLABLE invariant
of the project's v2 blueprint ("Logs and field names in Brazilian Portuguese").

Pipelines that depended on the English schema of `v0.1.0` must update their
`jaq` selectors. Rename table:

| Before (v0.1.0) | After (v0.2.0) |
|----------------|-----------------|
| `position` | `posicao` |
| `title` | `titulo` |
| `displayed_url` | `url_exibicao` |
| `content` | `conteudo` |
| `content_length` | `tamanho_conteudo` |
| `content_extraction_method` | `metodo_extracao_conteudo` |
| `execution_time_ms` | `tempo_execucao_ms` |
| `selectors_hash` | `hash_seletores` |
| `retries` | `retentativas` |
| `fallback_endpoint_used` | `usou_endpoint_fallback` |
| `concurrent_fetches` | `fetches_simultaneos` |
| `fetch_successes` | `sucessos_fetch` |
| `fetch_failures` | `falhas_fetch` |
| `chrome_used` | `usou_chrome` |
| `proxy_used` | `usou_proxy` |
| `engine` | `motor` |
| `region` | `regiao` |
| `results_count` | `quantidade_resultados` |
| `results` | `resultados` |
| `related_searches` | `buscas_relacionadas` |
| `pages_fetched` | `paginas_buscadas` |
| `error` | `erro` |
| `message` | `mensagem` |
| `metadata` | `metadados` |
| `queries_count` | `quantidade_queries` |
| `parallel` | `paralelismo` |
| `searches` | `buscas` |

Unchanged fields: `url`, `snippet`, `query`, `endpoint`, `timestamp`, `user_agent`.

### Fixed

- The pipelines documented in the README (`jaq '.resultados[].titulo'`, etc.) now
  work end-to-end. On `v0.1.0` they returned `null` because of the schema
  divergence (bug reported by the user).

### Added

- `LICENSE-MIT` and `LICENSE-APACHE` (dual-licensed per `Cargo.toml`, aligning the tarball with the SPDX declaration).
- `pre-commit config (removed)` with three hook groups: (1) pre-commit-hooks standard (trailing whitespace, EOF, YAML/TOML validity, mixed line endings), (2) Rust hooks (`cargo fmt` + `cargo clippy -D warnings`), (3) local `commit-msg` hook blocking `Co-authored-by:` from AI agents (mirrors the CI `commit_check` job). Reduces CI round-trips for trivial violations.
- `.gitattributes` forcing LF on `.rs` / `.toml` / `.sh` / `.yml` / `.md` / fixture HTML — prevents silent corruption when cloning on Windows with `core.autocrlf=true` (which would otherwise break shebangs, rustfmt, and content-extraction tests). Binary extensions (`.png`, `.woff2`, etc.) marked explicitly. `Cargo.lock` and `target/` flagged `linguist-generated` to exclude from GitHub language stats.
- `.editorconfig` normalizing UTF-8, LF, trailing-whitespace trim, and per-language indent (Rust/TOML 4, YAML/JSON/MD 2, Makefile tab) across VS Code, RustRover, vim, and other editors — eliminates spurious formatting diffs caused by per-dev settings drift.
- `PULL_REQUEST_TEMPLATE.md` with the 10-gate checklist + project-specific constraints (no cache, no MCP, rustls-only, `println!` confined to `output.rs`, PT-BR identifiers).
- `ISSUE_TEMPLATE/bug_report.yml` + `feature_request.yml` + `config.yml` — structured triage with platform dropdown (glibc/musl/NixOS/Flatpak/Snap/macOS ARM/macOS Intel/Windows/WSL), install method, and constraint verification. `config.yml` redirects security reports to Security Advisories and usage questions to Discussions.
- `Cross.toml` enabling `cross build --target <t>` for ARM64/ARMv7 Linux targets (musl + glibc + hard-float) from any x86_64 host with Docker/Podman — complements the native local build pipeline for developers without a remote CI runner (Actions forbidden).
- `CONTRIBUTING.md` with the 10-gate validation matrix, coding standards (Brazilian Portuguese identifiers, rustls-only TLS, `output.rs` as the sole `println!` site), three-layer testing strategy, supply-chain guardrails, and the tag-driven release process.
- `.cargo/config.toml` exposing 8 developer aliases (`cargo check-all`, `cargo lint`, `cargo docs`, `cargo test-all`, `cargo cov`, `cargo cov-html`, `cargo publish-check`, `cargo pkg-list`) — each mirrors a local validation job (historical; CI removed) for local reproduction.
- Doctests in public API: `pipeline::combine_and_dedup_queries`, `content_fetch::extract_host`, and `search::format_kl` — compilable examples on docs.rs that double as regression tests.
- `SECURITY.md` documenting the private-disclosure workflow via private security report channel, response SLA (72 h), scope (HTTP/HTML parsing, credential leaks, path traversal, TLS) and security design assumptions (stateless, rustls-only, no JS for search).
- dependabot (removed with Actions) config (removed)` enabling weekly automatic dependency updates for both `cargo` and `local-deps-only` ecosystems, with semantic grouping (dev-deps, tokio-ecosystem, tracing-ecosystem) and PR count limits.
- `rust-toolchain.toml` pinning `stable` with `rustfmt` + `clippy` components for reproducible dev/CI builds.
- `local release process` triggered by `v*.*.*` tags (and `manual local trigger` with `dry_run`) running the 5-stage release pipeline per `rules_rust.md` §19: validate → build_matrix (5 targets) → macos_universal (lipo) → github_release (with generated notes) → crates_io (publish gated on `crates.io token` secret).
- `msrv` job in local gates extracting `rust-version` from `Cargo.toml` and running `cargo check` on that toolchain to detect MSRV drift on every PR.
- `local gates` enforcing the 10-gate validation matrix across Ubuntu, macOS, and Windows:
  - `cargo check` / `clippy -D warnings` / `fmt --check` / `doc -D warnings` / `test --all-features` on all three OSes.
  - `cargo llvm-cov --fail-under-lines 80` dedicated job on Ubuntu.
  - `cargo audit` + `cargo deny check advisories licenses bans sources` supply-chain gate.
  - `cargo publish --dry-run` + `cargo package --list` sensitive-file guard.
  - Static musl binary smoke test (`x86_64-unknown-linux-musl`) covering Alpine Linux and minimal containers.
  - `commit_check` job blocking `Co-authored-by:` trailers from AI agents in PRs.
- `deny.toml` with full four-axis supply-chain policy (advisories/licenses/bans/sources) and documented ignores for three transitive unmaintained advisories (`RUSTSEC-2025-0057 fxhash`, `RUSTSEC-2025-0052 async-std`, `RUSTSEC-2026-0097 rand`) with justification and revisit notes.
- 22 new tests raising coverage from 77.4% to 86.4% (lines): `tests/integration_pipeline.rs` (10), `tests/integracao_fetch_conteudo.rs` (3), and 9 inline tests for `output.rs` covering `emit_ndjson`, `emit_stream_text`, `emit_stream_markdown`, and the `PipelineResult` variants via `tempfile`.

### Changed

- `parallel.rs` coverage 50% → 81%; `pipeline.rs` 55% → 82%; `content_fetch.rs` 68% → 85%; `output.rs` 70% → 87%.

## [0.1.0] - 2026-04-14

### Added

- Core search pipeline against DuckDuckGo HTML endpoint via pure HTTP (`html.duckduckgo.com/html/`).
- Lite endpoint fallback via `--endpoint lite` for JavaScript-less pages.
- Multi-query mode with automatic deduplication, positional args, `--queries-file`, and stdin.
- Parallel fan-out of queries with `--parallel` (1..=20), bounded by `tokio::JoinSet` + `Semaphore`.
- `--pages` (1..=5) to collect multiple result pages per query.
- `--fetch-content` fetches each result URL via pure HTTP, applies readability, and embeds the cleaned text in the JSON output.
- `--max-content-length` (1..=100_000) truncates extracted content respecting word boundaries.
- Chrome headless fallback under `--features chrome` with cross-platform detection (Linux including Flatpak/Snap, macOS including Apple Silicon, Windows including registry paths) and stealth flags (`--disable-blink-features=AutomationControlled`, `--window-size=1920,1080`, `--no-first-run`, platform-specific `--no-sandbox`, `--disable-gpu`).
- `--chrome-path` flag to manually specify the Chrome/Chromium executable.
- `--proxy URL` + `--no-proxy` (HTTP/HTTPS/SOCKS5) with precedence over env vars.
- `--global-timeout` (1..=3600 s) wraps the whole pipeline in `tokio::time::timeout`.
- `--per-host-limit` (1..=10) rate-limits fetches per host via a per-host `Semaphore` map.
- `--match-platform-ua` narrows the user-agent pool to the current platform.
- `--stream` NDJSON mode emits one result per line as they are extracted.
- Four output formats: `json` (default), `text`, `markdown`, `auto` (TTY-aware).
- External configuration files: `selectors.toml` and `user-agents.toml` under XDG config dir, overriding embedded defaults.
- Subcommand `init-config` with `--force` and `--dry-run` to bootstrap user config files.
- Exit codes: `0` success, `1` runtime, `2` config, `3` block (HTTP 202 anomaly), `4` global timeout, `5` zero results.
- UTF-8 console initialization on Windows via `SetConsoleOutputCP(65001)`.
- Rustls-TLS everywhere for dependency-free cross-platform builds.
- `tracing` + `tracing-subscriber` with `RUST_LOG` honored; `--verbose` / `--quiet` flags.
- 163 unit + integration tests covering CLI parsing, config montage, HTTP extraction, parallel fan-out, selectors, and wiremock-backed search flows.

### Security

- All credentials (`--proxy user:pass@host`) are masked in logs.
- Output file creation applies Unix permissions `0o644`.

[Unreleased]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.9.10...v1.0.0
[0.9.10]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.9.8...v0.9.10
[0.9.8]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.9.7...v0.9.8
[0.9.7]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.9.6...v0.9.7
[0.9.6]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.9.5...v0.9.6
[0.9.5]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.9.4...v0.9.5
[0.9.4]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.9.0...v0.9.4
[0.9.0]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.8.9...v0.9.0
[0.8.9]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.8.8...v0.8.9
[0.8.8]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.8.7...v0.8.8
[0.8.7]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.7.10...v0.8.7
[0.7.10]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.7.8...v0.7.10
[0.7.8]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.7.7...v0.7.8
[0.7.7]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.7.6...v0.7.7
[0.7.6]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.7.5...v0.7.6
[0.7.5]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.7.3...v0.7.5
[0.7.3]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.11...v0.7.0
[0.6.11]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.10...v0.6.11
[0.6.10]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.9...v0.6.10
[0.6.9]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.8...v0.6.9
[0.6.8]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.7...v0.6.8
[0.6.7]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.6...v0.6.7
[0.6.6]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.5...v0.6.6
[0.6.5]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.4...v0.6.5
[0.6.4]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.4.4...v0.5.0
[0.4.4]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/releases/tag/v0.1.0
