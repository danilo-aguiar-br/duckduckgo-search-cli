# Testing Guide

[Português (Brasil)](TESTING.pt-BR.md)

This guide covers test execution, categorization, and local multi-platform integration for
`duckduckgo-search-cli`.

## The gates — every alias in `.cargo/config.toml`

This project has no CI ([NO_CI.md](../NO_CI.md)). Every gate is a local cargo alias, so
this list IS the pipeline. `every_cargo_alias_is_documented_in_the_testing_guide` fails
the build when an alias is added here and not named below.

### Linux, full feature set
- `cargo check-all` — `check --all-targets --all-features --locked`
- `cargo lint` — `clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test-all` — `test --all-features --locked` (unit + integration + doctests)
- `cargo docs` — `doc --no-deps --all-features --locked`; run it as
  `RUSTDOCFLAGS='-D warnings' cargo docs`, because the alias cannot set the variable

### The shipped profile, with no C toolchain (ADR-0029)
- `cargo check-nohttp` — `check --no-default-features --features chrome --locked`
- `cargo check-nohttp-all-targets` — the same profile INCLUDING the test tree
  (v1.0.6, GAP-REL-002). `check-nohttp` omits `--all-targets` and `test-all` uses
  `--all-features`, where the harness is always on, so between the two nothing
  ever compiled the tests under the profile users install. Measured 2026-08-21:
  91 errors, three of them `E0432` — the same code as the defect that shipped in
  v1.0.2, hiding in the test tree.
- `cargo lint-nohttp` — same target under `clippy -- -D warnings`
- `cargo docs-nohttp` — `doc --no-deps --no-default-features --features chrome --locked`
- `--all-features` pulls `http-test-harness`, which pulls `reqwest` + `rustls` +
  `aws-lc-sys`; the last one is C. The `chrome` profile is what a user installs and is
  100% Rust.

### Cross-platform, without a linker
- `cargo check-windows` — target `x86_64-pc-windows-gnu`
- `cargo check-windows-msvc` — target `x86_64-pc-windows-msvc`
- `cargo lint-windows` — clippy on the gnu target with `-D warnings`
- `cargo check-macos` — target `aarch64-apple-darwin`
- `cargo check-macos-intel` — target `x86_64-apple-darwin`
- `cargo lint-macos` — clippy on the aarch64 target with `-D warnings`
- `cargo check-linux` — target `x86_64-unknown-linux-gnu` (v1.0.6)
- Prerequisite is additive and needs no root: `rustup target add <triple>`

### The blind spot depends on the HOST (v1.0.6)
- This list was written on a Linux host, where the uncovered targets were macOS
  and Windows.
- On a macOS host it INVERTS: `cargo check-macos` becomes native, and nothing
  compiles against Linux any more, leaving every `#[cfg(target_os = "linux")]`
  item without `rustc` coverage — `src/browser/xvfb.rs` included, which is where
  the original `E0432` came from.
- Whichever host you are on, add the target you are NOT, and run its alias.

### The honest limit of the cross-platform gates
- They are `cargo check` and `cargo clippy`, which do NOT link and do NOT run.
- They carry no `--all-targets`, so they cover the lib and the binary only.
- Tests, benches and examples are covered on Linux alone, by `check-all` and `test-all`.
- Runtime behaviour on macOS and Windows is therefore NOT validated by any gate.

### Coverage and packaging
- `cargo cov` — `llvm-cov --all-features --summary-only`
- `cargo cov-html` — `llvm-cov --all-features --html`
- `cargo pkg-list` — `package --list`, which shows exactly what the tarball ships
- `cargo publish-check` — `publish --dry-run --locked`
- `scripts/portability-lint.sh` — the checks a cargo alias cannot express

## The contract rulers

These six integration files do not test a feature. Each one MEASURES a rule
that the repository had only ever asserted in prose, and a rule guarded by
nothing is a rule that drifts. The first five measure a rule about the CODE;
the last one measures a rule about the DOCUMENTS.

### `tests/integration_stdout_boundary.rs`
- Guards the stdout boundary so that NO unreduced emission path appears silently
- v1.0.4 closed "flag accepted and ignored" by enumerating the six surfaces the
  plan happened to name, and `--probe` / `--probe-deep` emitted through their own
  helper, which never reached the projector
- Measured there: `--count-only`, `--limit 1`, `--fields status` and
  `--truncate-content 5` each returned 633 bytes against a 633-byte baseline, at
  exit 0
- The defect was the METHOD, because a list cannot report what is missing from itself
- This file sweeps EVERY stdout emission in `src/` and requires each one outside
  the output module to be declared with a reason
- A new bypass fails the build, and a STALE exemption fails the build too, so the
  allowlist cannot rot into decoration

### `tests/integration_hardcode_ssot.rs`
- Measures the hardcode prohibition instead of asserting it
- `src/endpoints.rs` declares itself the single source of truth for product
  network identity, and that sentence was guarded by nothing for years
- Five production call sites had drifted back to raw literals with no build failing
- `src/extraction/web/mod.rs` matched `"duckduckgo.com/y.js"` twice by hand and compared
  against a bare `"duckduckgo.com"`
- `types/selectors.rs` seeded the default ad filter with its own copy, and
  `zero_cause.rs` carried its own copy of the stealth-shell fragment
- The failure mode of a duplicated product fact is NOT a compile error, it is a
  silent behaviour split

### `tests/integration_golden_stdout.rs`
- Golden snapshots of the SHAPE of every offline stdout envelope
- A byte-for-byte golden of `doctor` would change on every host, because it
  carries absolute paths, a live count of chrome-like processes, a git SHA and a
  crate version
- Without normalisation noise becomes an alert and the snapshot is approved
  without being read, and a snapshot nobody reads is worse than none
- So the snapshot records every JSON path in the envelope with the type of its
  leaf, sorted, and values appear only where they are part of the contract
- This catches what `assert_conforms` cannot see: a key the schema never declared
  and the envelope stopped emitting, or an optional key that quietly disappeared
- Update with `cargo insta review`, or delete the `.snap` and re-run, then READ
  the diff

### `tests/integration_agent_ops_matrix.rs`
- Every agent-native operator, on every offline surface, must DO or REFUSE
- "Accepted and ignored" is invisible to a behavioural test, because the envelope
  is valid and the exit code is 0
- The only observable difference between honoured and silently dropped is the SIZE
  of what came back, which is why this measures bytes
- Every number comes from the same `Command::output()` call shape
- That shape avoids the v1.0.4 measurement trap, where a baseline read through a
  pipe kept the trailing newline and the variants read through `$(...)` ate it,
  faking a one-byte reduction on five surfaces

### `tests/integration_toolchain_boundary.rs`
- The shipped profile must contain NO C toolchain, measured from `cargo`
- `NO_CI.md`, `scripts/check-macos.sh` and the alias block in `.cargo/config.toml`
  all state that fact in prose, and three documents asserting a fact is not one
  measurement of it
- Adding one dependency that pulls `rustls` into the default feature set would
  restore the C requirement silently
- The first symptom would be a macOS or Windows gate dying with exit 101 on a host
  with no cross compiler
- It claims NOTHING about the test profile: `cargo test-all` runs `--all-features`
  and therefore DOES require a C toolchain, which is a real and declared limit

### `tests/integration_docs_version_ruler.rs`
- Guards the version label so that NO document declares a stale CURRENT version
- Three tests, measured green on 2026-08-21 with
  `cargo test --all-features --locked --test integration_docs_version_ruler`
- `no_document_declares_a_stale_current_version` walks every `.md` and `.txt`
  under the repository root, `docs/` and `skills/`, and fails when a document
  announces a current version different from `CARGO_PKG_VERSION`
- `bilingual_pairs_agree_on_the_current_version` fails when `X.md` and
  `X.pt-BR.md` declare different versions
- `the_version_scan_is_not_vacuous` stops the ruler from passing by measuring
  nothing, which is how a silent ruler survives forever

Why it exists, which is what stops someone deleting it:
- `tests/integration_docs_drift.rs` carries 20 tests and covers 22 of the 44
  documents in the repository
- MEASURED 2026-08-21: ELEVEN documents still announced `1.0.5` as the current
  line, and the list of liars was almost exactly the list of files that fall
  OUTSIDE that gate
- A document under a ruler cannot rot, because the build breaks when it lies
- A document outside one rots in SILENCE until a human happens to read it
- This is the SAME class as GAP-REL-001: there a configuration existed that no
  gate compiled, here a document existed that no test read, and both times
  everything was green while the user received something broken

What it deliberately does NOT do:
- It does NOT police historical references
- `since v0.9.8`, `ADR-0027 in v1.0.2` and a `## [1.0.4]` changelog heading are
  all CORRECT and must stay frozen
- Only a phrase that ANNOUNCES the current version is measured
- `CHANGELOG`, `docs/MIGRATION`, `gaps.md` and `docs/decisions/` are excluded by
  design, because recording the past IS their content

The measured trap:
- The first version of this ruler walked straight past `CONTRIBUTING.md:95`
- That line announced `v1.0.5` as still in force while the tree was at `1.0.6`
- The claim hid MID-SENTENCE instead of opening the line, so the scan never saw it
- The phrasing it used, `still current in`, was added to the marker list afterwards
- Note that this very bullet list had to be reworded to state that fact without
  tripping the ruler, which is the cheapest possible demonstration that it works
- A marker list is only as good as the phrasings someone thought to write down,
  so treat the list as incomplete by default

## v1.0.6 Test Notes

v1.0.6 closes ten release gaps. Each one shipped to crates.io before it was
found, so each entry below names the gate that now catches it.

- GAP-REL-001 — the PUBLISHED version did not compile outside Linux. No gate
  had ever asked what the registry actually serves. Closed by the post-publish
  gate documented below
- GAP-REL-002 — the DISTRIBUTED profile did not compile with `--all-targets`,
  measured at 91 errors, three of them `E0432`. Closed by the new alias
  `cargo check-nohttp-all-targets`, which is the shipped profile INCLUDING the
  test tree
- GAP-REL-003 — a duplicated `--disable-features` argument cancelled
  `AutomationControlled`, so the stealth flag was passed and had no effect
- GAP-REL-004 — the WebRTC mitigation was INVERTED, which re-exposed the
  local IP the mitigation existed to hide
- GAP-REL-005 — a golden snapshot encoded the operating system that generated
  it, so the snapshot failed on any other host
- GAP-REL-006 — `completions` panicked instead of exiting `141` on a closed
  pipe
- GAP-REL-007 — `config set A B --key C` wrote `C = B` and discarded `A`,
  silently storing the wrong key
- GAP-REL-008 — `--no-input` was declared and never read
- GAP-REL-009 — the CHANGELOG carried a SECOND `## [Unreleased]` heading
- GAP-REL-010 — cutting a SERP by BYTE index panicked on accented text

### The post-publish verification gate (ADR-0032)
- The binary is `src/bin/verify_published.rs`, behind
  `required-features = ["release-gate"]`
- Invoke it EXACTLY as
  `cargo run --bin verify_published --features release-gate`
- It compares what crates.io actually serves against this tree, which is the one
  question no other gate asks
- Every alias in `.cargo/config.toml` answers "does the TREE compile?" and
  `cargo publish --dry-run` answers "is the PACKAGE well-formed?"
- Neither answers the only question a user experiences, which is whether the
  version the registry SERVES compiles
- Run it after every `cargo publish` AND after every `cargo yank`
- ORDER IS MANDATORY: publish the sound version BEFORE yanking the broken
  ones. `max_stable_version` is DERIVED from yank state, so yanking first
  promotes an older broken version back into resolution
- That is exactly how v1.0.2 came back: the FIXED version was yanked and the
  broken v1.0.2 silently became `max_stable_version`, through no gate at all
- Exit codes reuse the crate taxonomy — `0` registry serves this version with no
  lower version live, `1` divergence, `2` unparseable response, `3` blocked by the
  registry (HTTP 403), `4` timeout
- Full rationale in
  [ADR-0032](decisions/0032-post-publish-verification-gate-v1-0-6.md)

## v1.0.2 Test Notes
- Wire JSON ENGLISH serialize default ([ADR-0027](decisions/0027-wire-en-default-v1-0-2.md)); assert `.results` / `.metadata` / `.chrome_path_resolved` / `.chrome_channel` / `.used_chrome` on default emit; legacy PT via `--wire-keys pt` still covered
- Budget underflow fail-fast → exit `2` (ADR-0024/0025); unit/integration coverage for `--print-budget`, `--allow-under-budget`, `--auto-contention-budget`, `budget_profile`
- `FETCH_CAP` default `4` (`--fetch-content-cap`); `DEFAULT_PAGES=1`
- Mute-audio always on (ADR-0026) — no unmute path; Chrome launch flags include mute + autoplay policy
- Agent ops flags unit tests: `--fields`/`--select`, `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only`, `--truncate-content`, `--max-output-bytes`
- `doctor --strict` / `doctor --probe-deep` (there is NO `doctor --probe`); root `--print-schema` and root `--probe` are separate entry points
- Deep-research defaults: `max-sub-queries=3` / `fetch-content-cap=4` (full-mode overrides with higher caps remain valid when explicit)
- v1.0.1 pipe-safe oneshot, v1.0.0 disk one-shot, v0.9.6 process lifecycle, v0.9.8 agent-ready, and v0.9.4 Chrome-only notes remain valid

## v1.0.1 Test Notes (Pass 52 / GAP-E2E-51-*)
- `cargo clippy --lib -- -D warnings` clean (and project gates toward zero warnings)
- Stream early close: `duckduckgo-search-cli -q --stream q1 q2 -n 10 | head -n 1` → CLI exit `141`; oneshot orphans `0` (`ensure_oneshot_cleanup` + SIG_IGN)
- Dual config parse/behaviour: `config get KEY` and `config get --key KEY`; `config set KEY VALUE` and `config set --key KEY --value VALUE`; `config effective` emits merged JSON
- `-f ndjson` accepted as multi-query stream alias (`--stream`)
- Wire: Portuguese keys on serialize; English deserialize aliases (ADR-0023) covered by lib tests — serialize default superseded by ADR-0027 / v1.0.2 EN
- News vertical: false anti-bot fixed; residual real DDG anti-bot may still exit 6 environmentally
- No remote telemetry; no product env knobs for lifecycle/config
- Lifecycle E2E remains TEST-ONLY gated env: `DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle -- --nocapture` (not a product env)
- v1.0.0 disk one-shot, v0.9.6 process lifecycle, v0.9.8 agent-ready, and v0.9.4 Chrome-only notes remain valid

## v0.9.8 Test Notes (GAP-WS-AGENT-READY-001 / ADR-0018)
- Assert default vertical is `all` (web + news envelope) unless `--vertical web`
- Assert content fetch ON by default; `--no-fetch-content` yields no `content` bodies (EN wire; legacy PT `conteudo` with `--wire-keys pt`)
- News rows may carry `content` / `content_size` / `content_extraction_method` when fetch is on (cap 4 (v1.0.2 default); PT names via `--wire-keys pt`)
- Agent metadata present on success/failure/deep paths: `chrome_path_resolved`, `chrome_channel`, honest `used_chrome` (not telemetry; EN wire default v1.0.2)
- Transport flags accepted after subcommands (e.g. `deep-research … --chrome-path …`)
- Flatpak multi-canal resolve covered by unit tests on path classification / wrapper→ELF mapping
- Optional gated E2E when Flatpak Chrome/Chromium is installed: `DUCKDUCKGO_FLATPAK_E2E=1 cargo test -- --nocapture` (host-dependent; skip when absent)
- Preserve-0.9.7 formula still green: `--vertical web --no-fetch-content`
- v1.0.1 pipe-safe oneshot, v1.0.0 disk one-shot, v0.9.6 process lifecycle, and v0.9.4 Chrome-only notes remain valid

## v1.0.0 Test Notes (GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020)
- Gap RESOLVED in v1.0.0 — disk one-shot + auditable Chrome profile prefix (see `gaps.md`, [ADR-0020](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md))
- Lifecycle E2E still gated: `DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle -- --nocapture`
- Integration asserts `ddg-chrome-` profile prefix (not generic `.tmp`) AND process reap (no orphan Chromium/Xvfb left by that run)
- Unit tests cover `force_reap` / `sweep_orphan_profiles` / prefix ownership guards (never bulk-delete foreign `.tmp*` or `org.chromium.Chromium.*`); cooperative path also uses `ExitReapGuard` + panic hook (operational, not a schema concern)
- SIGKILL residual is next-run sweep of owned `ddg-chrome-*` only — not mass hygiene of third-party temp dirs

## v0.9.6 Test Notes (GAP-WS-LIFECYCLE-001)
- Unit tests cover `process_lifecycle` (process group / marker reap paths) and `paths::atomic_write`
- Gated E2E: `DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle -- --nocapture`
- E2E requires Chrome/Chromium (and Xvfb on headless Linux) and asserts no orphan Chromium/Xvfb left by that run; SINCE v1.0.0 also asserts the `ddg-chrome-` profile prefix (not generic `.tmp`) and disk reap (GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020)
- Unit coverage for disk hygiene: `force_reap` / `sweep_orphan_profiles` / prefix ownership guards
- SIGTERM (and SIGINT) cancel the shared `CancellationToken` on the signals path (cooperative cancel for Docker/`timeout`)
- v0.9.4 fail-closed notes (GAP-WS-113) remain valid: Chrome-only production, `NO_CHROME` → exit 2, residual HTTP only under `http-test-harness`

## v0.8.9 Test Notes
- v0.8.9 adds `tests/integration_news_vertical.rs` covering the news vertical `--vertical <web|news|all>` (GAP-WS-104)
- v0.8.9 adds `tests/integration_deep_research_news.rs` covering the deep-research dual web+news fan-out (GAP-WS-105): one Chrome session per sub-query runs `--vertical all`, the `--no-news` opt-out, the aggregated envelope (`noticias[]`, `quantidade_noticias`, `metadados.total_noticias_unicas`), the news-only RRF (kept separate from the web RRF), the `news_indisponivel: true` mid-flight structured field, and the dual `--synthesize` ~70/30 budget split
- v0.9.4 note (GAP-WS-113): GAP-WS-106 auto-degrade (auto `--no-news` / web-only without Chrome) is SUPERSEDED. Production is fail-closed: builds without a usable Chrome exit `2` on network ops (historical product env `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` is REMOVED / not read). Residual HTTP lives only under feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` (wiremock/integration harness — TEST-ONLY). See ADR-0016.
- New HTML fixtures under `tests/fixtures/`:
  - `ddg_news_serp.html` — Strategy A SERP (semantic selectors from `selectors.toml`; 7 articles, 1 internal duckduckgo.com trap filtered out)
  - `ddg_news_serp_ofuscada.html` — obfuscated-classes SERP exercising the class-agnostic Strategy B fallback
  - `ddg_news_serp_vazia.html` — empty SERP producing `noticias: []` and `causa_zero: vertical-sem-resultados`
- Web-mode contract validated byte-identical to v0.8.8 (no `noticias`/`quantidade_noticias`/`vertical_usada` emitted in web mode)


## v0.8.8 Test Notes
- Test count: 528 tests (382 unit + 146 integration/doc), 0 failures
- v0.8.8 adds regression tests for 12 fixed gaps (GAP-WS-089 to GAP-WS-103)
- `--num` truncation tested in Chrome headed and batch paths (GAP-WS-090, GAP-WS-094)
- `fill_compat_fields()` coverage for metadata compat fields (GAP-WS-092, GAP-WS-093, GAP-WS-097)
- `ZeroResultsSuspeito` exit code 6 validated (GAP-WS-099)
- `tamanho_conteudo` reflects truncated text length (GAP-WS-100)
- Xvfb stale lock cleanup via `is_lock_stale()` PID checking (GAP-WS-089)


## v0.9.4 Test Notes (GAP-WS-113)
- Production path is Chrome-only; missing usable Chrome (or build without feature `chrome`) must yield exit `2` on search/probe/fetch/deep-research (fail-closed). Product env `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` is REMOVED / not read
- Wiremock / pure-HTTP SERP tests require `--features http-test-harness` and TEST-ONLY `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`
- `--allow-lite-fallback` is a no-op — tests must not assert Lite success from that flag
- Builds with `--no-default-features` are offline/unit only; they are not a production network path

## v0.8.7 Test Notes
- E2E tests require Google Chrome or Chromium installed
- Linux: Xvfb is auto-installed by the CLI at runtime via `try_auto_install_xvfb()`. To pre-install it by hand: `sudo apt-get install -y xvfb`
- macOS/Windows: no extra dependency — Chrome runs headless=new since v0.9.3 (Linux keeps Xvfb private)
- To test without Chrome (offline/unit only; not production): `cargo test --no-default-features`
- To test with forced headless: pass CLI `--chrome-headless` (product env `DUCKDUCKGO_CHROME_HEADLESS` REMOVED)
- Test count at v0.8.7 release: 548 tests (382 unit + integration + doc), 0 failures
- Deep-research JSON schema: `.results[].title` under the EN default wire (`.resultados[].titulo` only with `--wire-keys pt`), top-level `.query` field available


## v0.7.0 Test Additions

The v0.7.0 release added tests across the four new modules, all addressing previously open gaps:

- Doctests (12 tests) — added to `aggregation.rs`, `synthesis.rs`,
  `decomposition.rs`, and `deep_research.rs`. They serve as runnable
  documentation: each module exports at least one `no_run` example.
- Property-based tests (7 tests, `proptest`) — `aggregation::canonicalize_url`
  is checked for idempotence, fragment-strip, tracking-param-strip, and
  host-lower invariants. `synthesis::estimate_tokens` is checked for
  monotonicity, and `synthesis::trim_to_budget` is checked for both the
  ceiling and the idempotence invariant. The proptest regressions are
  written under `proptest-regressions/`, which is captured in
  `.gitignore`.
- Wiremock integration tests (17 tests, `tests/integration_deep_research.rs`)
  — pipeline smoke, query-param matching, HTTP 202 anomaly
  observability, HTTP 404 observability, and 13 surface-coverage tests
  that exercise the public API of every new module.
- Cancellation safety (1 test) — `decompose_respects_cancellation`
  validates that the heuristic decomposer returns early when its
  `CancellationToken` is cancelled.
- Manual file handling (3 tests) — blank-line and `#` comment
  skipping, file-with-only-comments rejection, and missing-path rejection.
- Total: 392 tests passing (279 lib + 12 doc + 101 integration). The
  v0.7.0 changes are purely additive. No tests removed, no test
  signatures changed, no test fixtures renamed.

### v0.7.0 gaps closed by these tests
- Latent UTF-8 panic in `synthesis::trim_to_budget` — was using
  byte indexing without a char-boundary check. The proptest caught the
  panic on a multi-byte input, the fix uses `floor_char_boundary`, and
  three regression tests now lock in the `is_char_boundary(out.len())`
  invariant.
- Empty / one-token / zero-max edge cases in `decomposition.rs`.
- `run_deep_research` cancellation safety — validates that the
  pipeline bails out before fanning out N sub-queries when the operator
  hits `Ctrl+C`.

## v0.6.5 Test Additions

The v0.6.5 release added 11 tests, all addressing previously open gaps:

- WS-11 (5 tests) — property-based invariants for the HTML parser in
  `extraction.rs`. Validates that empty inputs yield empty `Vec`, positions
  are dense and 1-based, URLs are normalized to absolute paths, the parser
  is deterministic, and malformed HTML does not panic. These tests would
  have caught the v0.6.3 → v0.6.4 migration regressions.
- WS-12 (4 tests) — per-host circuit breaker in `content_fetch.rs`.
  Validates the closed-state allows requests, the threshold opens the
  breaker, a single success resets the failure counter, and the half-open
  state is reachable after the cooldown window.
- WS-23 (1 test) — wiremock integration test for the `Retry-After`
  header on HTTP 429 responses. Validates the backoff delay is at least
  `Retry-After` seconds, with a 500ms slack for host scheduling variation.
- Existing 322 tests preserved — the v0.6.5 changes are purely additive.
  No tests removed, no test signatures changed, no test fixtures renamed.

### v0.6.5 gaps closed by these tests
- MP-26 (Windows HANDLE) — validated by `cargo test --all-features` run
  by hand on a Windows host. There is NO Windows job anywhere; see the
  cross-platform limit at the top of this guide.
- CI-01 (6 clippy errors) — `cargo clippy --all-targets --all-features -- -D warnings`
  now passes, which is itself a "test" that no lint regression exists.
- WS-12 (circuit breaker) — covered by 4 unit tests in
  `src/content_fetch.rs`. (HISTORICAL — the v0.6.5 layout. That file is a
  directory today, `src/content_fetch/`; the path is kept as the record of
  where the tests lived at v0.6.5.)
- WS-23 (Retry-After) — covered by 1 wiremock test in
  `tests/integration_wiremock.rs`.


## Why Categorized Tests

The test suite is split into four categories to balance speed, isolation,
and coverage:

| Category       | Speed      | Isolation   | Real I/O  | Count (v1.0.6) |
|----------------|------------|-------------|-----------|----------------|
| Unit           | < 1 s      | per-fn      | none      | 806            |
| Integration    | < 30 s     | per-test    | localhost | 304            |
| Doc            | < 5 s      | per-doc     | none      | 12             |
| Loom           | n/a        | n/a         | n/a       | 0 (gated)      |

This table is CURRENT, not historical, so it carries numbers measured on v1.0.6
rather than the v0.7.5 ones it used to carry. Measured 2026-08-21 by execution,
never by estimate:

- `cargo test --all-features --locked --lib -- --list` — `806` unit tests
- `cargo test --all-features --locked --tests -- --list` — `1110`, which
  includes the unittests target, so integration is `304`
- `cargo test --doc --all-features --locked -- --list` — `12` doctests, 0 benchmarks
- The `Doc` row previously read `0`, which contradicted the `cargo test-all` line
  at the top of this guide; `cargo test-all` (`test --all-features --locked`) runs them
- `806 + 304 + 12` totals `1122`, exactly what
  `cargo test --all-features --locked -- --list` returns
- Counts inside the per-version historical sections were NOT touched, because
  each one describes what was true at that version

## Test Categories

### Unit Tests
Located in `src//tests` modules (mod tests). Fast, in-process, no I/O.
Run with:

```bash
cargo test --lib
```

### Integration Tests
Located in `tests/*.rs` files. Use wiremock (no real HTTP), assert_cmd (no real
subprocess spawn), and tempfile (no real FS writes outside tmpdir).

```bash
# All integration tests
cargo test --tests

# Single integration test file
cargo test --test integration_wiremock
```

### Doc Tests
Located in `///` examples throughout `src/`. Compiled and executed by `cargo test --doc`.

```bash
cargo test --doc
```

### Loom Tests
Located in `tests/loom_atomics.rs`. Gated by `--cfg loom`. NOT compiled by
default — requires explicit opt-in.

```bash
RUSTFLAGS="--cfg loom" cargo test --test loom_atomics --release
```

> Known limitation: Loom conflicts with `hyper-util` and currently
> compiles but does not run cleanly. Issue tracked upstream.


## How to Run

### Local Development

```bash
# Quick feedback loop
timeout 300 cargo test --all-features --locked

# Specific category
cargo test --lib --locked
cargo test --tests --locked
cargo test --doc --locked
```

### With Coverage

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Run with HTML report
cargo llvm-cov --all-features --locked --html --open

# Run with text summary only
cargo llvm-cov --all-features --locked --summary-only
```

Minimum line coverage: `80%`. Local validation must fail below this threshold.

### Property-Based Tests (v0.6.5, WS-11)

5 invariants in `src/extraction.rs`. (HISTORICAL — the v0.6.5 layout. That
file is a directory today, `src/extraction/`; the path is kept as the record of
where the invariants lived at v0.6.5.)

```bash
cargo test ws11_
# Run all 5 property tests:
# - ws11_invariant_empty_inputs_yield_empty_results
# - ws11_invariant_positions_are_dense_and_one_based
# - ws11_invariant_urls_are_normalized_to_absolute
# - ws11_invariant_extraction_is_idempotent
# - ws11_invariant_malformed_html_does_not_panic
```

### WireMock Retry-After Test (v0.6.5, WS-23)

```bash
cargo test --test integration_wiremock test_retry_after_header_respected
```

### Circuit Breaker Tests (v0.6.5, WS-12)

```bash
cargo test ws12_
# Tests: ws12_breaker_allows_when_closed,
#        ws12_breaker_opens_after_threshold_failures,
#        ws12_breaker_resets_on_success,
#        ws12_breaker_half_opens_after_cooldown
```


## Environment Variables

| Variable                        | Effect                                                |
|---------------------------------|-------------------------------------------------------|
| `RUST_TEST_THREADS`             | Number of parallel test threads (default 1)            |
| `RUST_BACKTRACE`                | Set to `1` or `full` for detailed backtraces           |
| Product log filter              | CLI `-v`/`-q` + XDG `log_directive` only (not `RUST_LOG` product config) |
| `CARGO_TERM_COLOR`              | Force ANSI colors (`always`, `never`, `auto`)         |
| `LOOM_MAX_PREEMPTIONS`          | Max preemption bound for loom tests                    |
| `WIREMOCK_LOG`                  | WireMock request/response logging                      |
| `DUCKDUCKGO_LIFECYCLE_E2E`      | TEST-ONLY gated env (not product). Set to `1` to run browser lifecycle E2E (`tests/integration_browser_lifecycle.rs`; requires Chrome/Chromium, and Xvfb on headless Linux; v1.0.0+ asserts `ddg-chrome-` prefix + process/disk reap; v1.0.1 also stream`|head` → 141 + orphans 0 — ADR-0020 / Pass 52) |


## Local Validation Profiles

Three validation passes run the test suite. Each one is a command YOU run by
hand, on one host at a time. There is no matrix and no scheduler.

1. `validate` — `cargo test --all-features --locked`, run on whichever host
   you are sitting at. Covering Linux, macOS and Windows means running it once
   per host; nothing runs it for you, so an unvisited host is an uncovered host
2. `msrv` — `cargo check --all-targets --all-features --locked` on Rust 1.88 (MSRV since v0.7.2)
3. `coverage` — `cargo llvm-cov --all-features --locked --fail-under-lines 80` on Linux

Plus a manual `cargo nextest` profile available locally:

```toml
# .config/nextest.toml (not in repo, per project convention)
[profile.default]
retries = 2
test-threads = 1
```


## Troubleshooting

### `flaky::lazy_template` failures
Loom tests may be flaky. Re-run with:

```bash
RUSTFLAGS="--cfg loom" cargo test --test loom_atomics --release -- --test-threads=1
```

### `wiremock::MockServer` startup timeout
Increase the wait:

```bash
WIREMOCK_LOG=info cargo test --test integration_wiremock
```

### Coverage drops below 80%
Check the HTML report for uncovered lines:

```bash
cargo llvm-cov --html --open
```

The diff will show which lines are not exercised by the test suite. Add
unit or integration tests to cover the missing branches.

### Tests pass on one host but fail on another
- Check for environment-specific behavior (paths, timeouts, locale)
- Check for `Instant::now()` non-determinism in code under test
- Use `cargo nextest` with retries to detect flaky tests:

```bash
cargo nextest run --retries 3
```


## v0.7.3 Test Additions

The v0.7.3 release added 13 new tests across the three new modules:

- `session_warmup` (5 unit tests) — XDG path resolution on Linux, macOS, and Windows; missing-directory creation; path override via `DUCKDUCKGO_SEARCH_CLI_HOME` (HISTORICAL — see the note below); `default_cookies_filename` constant stability.
  - `DUCKDUCKGO_SEARCH_CLI_HOME` is NOT product configuration. Measured
    2026-08-21 with `rg -n -F "DUCKDUCKGO_SEARCH_CLI_HOME" src/ tests/`: the
    only hit in `src/` is the doc comment at `src/platform.rs:172`, which states
    the runtime path is NOT overridden by it, and there is ZERO hit under
    `tests/`. Nothing reads it — neither production nor the current test harness.
  - The supported override is the CLI flag `--config-home <PATH>`, as
    [AGENTS.md](AGENTS.md) already declares. NEVER set this variable expecting
    the product to honour it.
- `cookie_adapter` (3 unit tests, renamed from `wreq_cookie_adapter` in v0.8.6) — `PersistentJar::empty()` produces a valid `Arc<reqwest::cookie::Jar>`; `parse_json` roundtrip preserves cookies via `CookieStore::cookies()` header extraction; `save` and `load` roundtrip with `0o600` Unix permissions and atomic write semantics.
- `probe_deep` (5 unit tests) — `detect_interstitial` correctly identifies Cloudflare markers (`cf-chl-bypass`, `cf-challenge`, `challenge-platform`, `Attention Required`, `__cf_chl_jschl_tk__`); `detect_interstitial` correctly identifies DuckDuckGo `robot-detected` and `bots, we have detected` markers; `mitigation_suggestion` returns concrete next steps for each interstitial kind; `InterstitialKind::None` is the default for a normal HTML response; `execute_probe_deep` produces a valid JSON report.
- Total: 405 lib tests passing (was 279 in v0.7.2; current project total at v0.7.5). The v0.7.3 changes are purely additive. No tests removed, no test signatures changed, no test fixtures renamed.

### v0.7.3 gaps closed by these tests
- `probe_deep::detect_interstitial` — validates the marker strings are detected at all (the cost of a false negative is a CAPTCHA that goes undiagnosed). Five Cloudflare markers + two DuckDuckGo markers are unit-tested in isolation.
- `cookie_adapter::PersistentJar` — validates the JSON ↔ `reqwest::cookie::Jar` bridge does not lose cookies during roundtrip (rewritten in v0.8.6 to use `CookieStore::cookies()` header extraction). A regression here would silently strip session cookies, re-introducing a CAPTCHAd session.
- `session_warmup::default_cookies_path` — validates the XDG resolution is correct per platform. A regression here would put the cookie jar in the wrong directory or fail to set `0o600` permissions on Unix.


## v0.7.4 Test Additions

> HISTORICAL — removed in v0.8.6. The whole `build.rs` preflight stack
> described below stopped existing when `wreq` was replaced by `reqwest` +
> `rustls-tls`. The `DDG_SKIP_NASM_CHECK` escape hatch is NOT a current
> environment variable and setting it does NOTHING. Read this section as a
> record of what v0.7.4 shipped, NEVER as current configuration.

v0.7.4 adds build-time tests that validate the build.rs preflight for NASM assembler detection on Windows MSVC native builds.

- `build::preflight::nasm` — 4 unit tests validating:
  - `nasm_in_path` returns `true` when nasm.exe is on PATH
  - `nasm_in_path` returns `false` when nasm.exe is absent
  - `known_nasm_dir` returns `Some` for `C:\Program Files\NASM` and `C:\Program Files (x86)\NASM`
  - `known_nasm_dir` returns `None` for unknown paths
- GAP-WS-28 closed by these tests — the panic message, fix command, and DDG_SKIP_NASM_CHECK=1 escape hatch are all validated end-to-end in the build script.
- Test count: ~395 lib tests passing (was 292 in v0.7.3 = +3-5 new build preflight tests).

### v0.7.4 gaps closed by these tests
- `build::preflight::nasm_in_path` — validates the scan logic for nasm.exe in PATH. A regression here would cause the v0.7.4+ preflight to either false-positive (panic when NASM is installed) or false-negative (let the build proceed to the cryptic CMake error).
- `build::preflight::known_nasm_dir` — validates the heuristic for NASM-is-installed-but-PATH-is-stale detection. A regression would miss the actionable hint that the user just needs to refresh their PATH.

## v0.7.5 Test Additions

> HISTORICAL — removed in v0.8.6. The four preflights (NASM, CMake, MSVC,
> Strawberry Perl), the four `DDG_SKIP_*_CHECK=1` escape hatches, the
> `perl_in_path` detection and the `install-windows.ps1` helper are ALL gone
> from the current tree. None of those four variables is read today, and
> Strawberry Perl is NOT a dependency of the shipped profile — that profile is
> 100% Rust, measured by `tests/integration_toolchain_boundary.rs`. Read this
> section as a record, NEVER as current setup instructions.

v0.7.5 extends the build preflight to detect 4 tools (NASM, CMake 3.20+, MSVC C/C++, Strawberry Perl) and adds tests for the helper scripts.

- `build::preflight::cmake` — 3 unit tests validating cmake_in_path and known_cmake_dir heuristics.
- `build::preflight::msvc` — 2 unit tests validating cl_in_path and link_in_path detection.
- `build::preflight::perl` — 3 unit tests validating perl_in_path and known_perl_dir heuristics.
- `scripts::check_windows_toolchain` — 4 integration tests validating the JSON output schema and the all_present boolean for various tool combinations.
- `scripts::install_windows` — 1 integration test smoke-validating that the install-windows.ps1 --check-only mode emits a parseable report.
- GAP-WS-29/30/31 closed by these tests — each of the 4 preflight panic paths is unit-tested in isolation, and the 4 DDG_SKIP_*_CHECK=1 escape hatches are validated.
- Test count: 405 lib tests passing (was ~395 in v0.7.4 = +8-13 new build preflight + script tests). This is the current project total at v0.7.5.
- Cross-platform: run locally `cargo test --all-targets --all-features` (Windows/Linux/macOS). NO GitHub Actions / Windows host job.

### v0.7.5 gaps closed by these tests
- `build::preflight::cmake_in_path` — validates the scan for cmake.exe in PATH. A regression would let the v0.7.5+ build proceed to the cryptic failed to execute command: program not found panic from the cmake crate.
- `build::preflight::cl_in_path` and `link_in_path` — validates the MSVC compiler/linker detection. Both must be present; partial detection is treated as missing.
- `build::preflight::perl_in_path` — validates the Perl interpreter detection. Strawberry Perl is the de-facto Windows Perl; the test uses perl.exe filename pattern.
- `scripts::check_windows_toolchain::json_output` — validates that the diagnostic scripts JSON output is parseable and contains the 7 expected tool entries with found boolean and path string fields.
- `scripts::install_windows::check_only_mode` — validates that the --check-only flag produces a report without attempting to install anything, suitable for a local gate.


## v0.7.6 Test Additions

v0.7.6 closes GAP-WS-48 (same-day `cargo install` fix) and adds regression tests for the dependency conflict.

- `build::install::alloc_no_stdlib_pin` — 2 unit tests validating the `alloc-no-stdlib = "2.0.4"` pin is respected during `cargo install` and not silently upgraded to 3.0.0.
- `build::install::brotli_decompressor_pin` — 1 unit test validating the `brotli-decompressor = "5.0.1"` pin survives resolution on a clean toolchain.
- `integration::install_clean_toolchain` — 1 integration test that runs `cargo install --path . --offline` in a fresh `target/` and asserts exit 0.
- GAP-WS-48 closed by these tests — every dependency pin that the v0.7.6 fix relies on has a dedicated test.
- Test count: 408 lib tests passing (was 405 in v0.7.5 = +3 new install-pin tests). This is the project total at v0.7.6.
- local gate: the new install tests run in the `install-check` local validation job (historical; CI removed) alongside the v0.7.5 preflight tests.

### v0.7.6 gaps closed by these tests
- `build::install::alloc_no_stdlib_pin` — prevents the `2.0.4` vs `3.0.0` conflict from re-appearing silently. A regression would re-trigger the original `cargo install` panic.
- `build::install::brotli_decompressor_pin` — keeps BoringSSL brotli decoder pinned to a known-good version. A regression would break the Linux source build.
- `integration::install_clean_toolchain` — end-to-end install gate that catches any new dependency conflict before publishing.


## v0.7.7 Test Additions

> v0.8.6+: The `tls::emulation` tests below were REMOVED when `wreq` was replaced by `reqwest` + `rustls-tls`. See ADR-0008. The build preflight tests in v0.7.4–v0.7.5 (NASM, CMake, MSVC, Perl) were also removed as the preflights no longer exist in `build.rs`.

v0.7.7 closes GAP-WS-49 (TLS fingerprint regression) and adds regression tests for the `wreq` + `wreq-util` emulation stack. (Historical — tests removed in v0.8.6.)

- `tls::emulation::wreq_util_present` — 2 unit tests validating that `wreq-util 3.0.0-rc` with `features = ["emulation"]` is in the resolved dependency tree. (Removed in v0.8.6.)
- `tls::emulation::brotli_feature_enabled` — 1 unit test validating that the `brotli` feature on `wreq` is enabled (required for the emulation stack to compile). (Removed in v0.8.6.)
- `tls::probe_deep::captcha_classification` — 1 integration test that runs `--probe-deep` against a real DuckDuckGo endpoint and asserts the JSON envelope contains `status`, `cascade_reason`, and `mitigation_suggestion` fields.
- `tls::probe_deep::ok_envelope` — 1 integration test that asserts the success envelope matches the documented schema in `docs/HOW_TO_USE.md`.
- GAP-WS-49 closed by these tests — the emulation stack is locked in at the dependency level and validated end-to-end.
- Test count: 413 lib + integration tests passing (was 408 in v0.7.6 = +5 new TLS re-registration tests). This is the project total at v0.7.7.
- local gate: the TLS tests ran in the `tls-emulation` local validation job (historical; CI removed) in v0.7.7–v0.8.5. (Removed in v0.8.6 — wreq eliminated.)

### v0.7.7 gaps closed by these tests (historical — superseded by v0.8.6)
- `tls::emulation::wreq_util_present` — prevented another GAP-WS-48-style accidental removal of `wreq-util`. (Superseded: wreq-util removed in v0.8.6.)
- `tls::emulation::brotli_feature_enabled` — kept the `brotli` feature in the build graph. (Superseded: brotli removed in v0.8.6.)
- `tls::probe_deep::captcha_classification` — validates the local gate format for `--probe-deep`. A regression would let the gate return exit 0 on a captcha response.
- `tls::probe_deep::ok_envelope` — validates the success path JSON. A regression would break downstream agent consumers parsing the envelope.


## v0.7.8 Test Additions

v0.7.8 closes 8 gaps (GAP-WS-50 through GAP-WS-57) and adds regression tests for each. The detector overhaul is the biggest delta.

- `probe_deep::markers::cloudflare` — 4 unit tests validating the 4 new Cloudflare markers (`anomaly-modal`, `anomaly.js`, `botnet`, `Unfortunately, bots`) against real HTML fixtures under `tests/fixtures/`.
- `probe_deep::markers::ddg` — 1 unit test validating the new `anomaly-modal__title` DDG marker.
- `probe_deep::markers::legacy` — 3 unit tests validating that legacy markers (`cf-chl-bypass`, `cf-challenge`, `robot-detected`) still match.
- `cli::verbose::count_levels` — 1 unit test validating that `-v` (1), `-vv` (2), `-vvv` (3) parse correctly via `ArgAction::Count`.
- `cli::verbose::conflicts_with_quiet` — 1 unit test validating that `--verbose` and `--quiet` together fail clap validation.
- `search_retry::retries_honored` — 1 integration test in `tests/integration_search_retry.rs` validating that `--retries 5` produces `metadados.retentativas == 5` in the JSON.
- `search_retry::clamp_to_ten` — 1 integration test validating that `--retries 999` is clamped to 10 with a warning.
- `search::fallback_lite_opt_in` *(historical, v0.7.8–v0.9.3)* — 2 unit tests validating that `--allow-lite-fallback` does not trigger when the user did not pass the flag. Since v0.9.4 the flag is a no-op (GAP-WS-113).
- `search::fallback_lite_with_interstitial` *(historical, v0.7.8–v0.9.3)* — 2 unit tests validating that the fallback triggers when the detector classifies an interstitial and the flag is on. Lite is not a production success path since v0.9.4.
- Test count: 305 lib + 18 integration tests passing (was 292 lib + 13 integration in v0.7.7 = +10 new v0.7.8 tests). This is the project total at v0.7.8.
- local gate: the marker tests run in the `detector-markers` local validation job (historical; CI removed); the retry tests run in the `retry-pipeline` local validation job (historical; CI removed).

### v0.7.8 gaps closed by these tests
- `probe_deep::markers::cloudflare` and `ddg` — locks in the post-2026 marker list. A regression to the legacy-only detector would re-open GAP-WS-50.
- `cli::verbose::count_levels` — locks in the `ArgAction::Count` semantics. A regression to a single `verbose: bool` would re-open GAP-WS-53.
- `cli::verbose::conflicts_with_quiet` — prevents the contradictory flag combination. A regression would let operators shoot themselves in the foot.
- `search_retry::retries_honored` — locks in the `cfg.retries` propagation. A regression to the hard-coded `1` would re-open GAP-WS-57.
- `search_retry::clamp_to_ten` — locks in the `[1, 10]` clamp. A regression would let `--retries 999` trigger anti-bot detection.
- `search::fallback_lite_opt_in` *(historical)* — locked in the Lite opt-in contract for v0.7.8–v0.9.3. Superseded by v0.9.4 / GAP-WS-113: `--allow-lite-fallback` is a legacy no-op; tests must not assert Lite success from that flag.
- `search::fallback_lite_with_interstitial` *(historical)* — locked in the `detect_interstitial` predicate for the old Lite path. Superseded by Chrome-only production (ADR-0016).


## Chrome Stealth Tests (v0.8.0, updated v0.8.7)
- Chrome stealth tests require Xvfb on headless Linux (v0.8.7+ auto-installs on 22+ distros)
- Run with: `cargo test` (v0.8.7+ auto-spawns private Xvfb; manual fallback: `xvfb-run --auto-servernum cargo test`)
- `tests/integration_stealth_block_classification.rs` validates stealth signal
  injection and block classification. The name `tests/integration_chrome_stealth.rs`
  appeared here until v1.0.6 and never existed on disk; measured 2026-08-21 with
  `fd -e rs . tests/`
- `tests/integration_deep_research.rs` validates Chrome pipeline in deep-research
- Unit tests in `src/browser/tests.rs` validate `flags_stealth()` arguments
- HISTORICAL COUNT for this section (v0.8.0, updated v0.8.7): 378 tests passed
  with the Chrome feature enabled. It is NOT the current suite; the current
  numbers live in the table under `Why Categorized Tests`
- To skip Chrome tests: `cargo test --no-default-features`
