# Testing Guide

This guide covers test execution, categorization, and CI integration for
`duckduckgo-search-cli`.

## v0.8.9 Test Notes

- v0.8.9 adds `tests/integration_news_vertical.rs` covering the news vertical `--vertical <web|news|all>` (GAP-WS-104)
- v0.8.9 adds `tests/integration_deep_research_news.rs` covering the deep-research dual web+news fan-out (GAP-WS-105): one Chrome session per sub-query runs `--vertical all`, the `--no-news` opt-out, the aggregated envelope (`noticias[]`, `quantidade_noticias`, `metadados.total_noticias_unicas`), the news-only RRF (kept separate from the web RRF), the `news_indisponivel: true` mid-flight structured field, and the dual `--synthesize` ~70/30 budget split
- **v0.9.4 note (GAP-WS-113):** GAP-WS-106 auto-degrade (auto `--no-news` / web-only without Chrome) is **superseded**. Production is fail-closed: `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` and builds without a usable Chrome exit **2** on network ops. Residual HTTP lives only under feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` (wiremock/integration harness). See ADR-0016.
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

- Production path is Chrome-only; `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` must yield **exit 2** on search/probe/fetch/deep-research (fail-closed)
- Wiremock / pure-HTTP SERP tests require `--features http-test-harness` and `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`
- `--allow-lite-fallback` is a no-op — tests must not assert Lite success from that flag
- Builds with `--no-default-features` are offline/unit only; they are not a production network path

## v0.8.7 Test Notes

- E2E tests require Google Chrome or Chromium installed
- Linux: Xvfb is auto-installed by the CLI at runtime via `try_auto_install_xvfb()`. For CI, pre-install: `sudo apt-get install -y xvfb`
- macOS/Windows: no extra dependency — Chrome runs headless=new since v0.9.3 (Linux keeps Xvfb private)
- To test without Chrome (offline/unit only; not production): `cargo test --no-default-features`
- To test with forced headless: `DUCKDUCKGO_CHROME_HEADLESS=1 cargo test`
- Test count at v0.8.7 release: 548 tests (382 unit + integration + doc), 0 failures
- Deep-research JSON schema: `.resultados[].titulo` (not `.title`), top-level `.query` field available


## v0.7.0 Test Additions

The v0.7.0 release added tests across the four new modules, all addressing previously open gaps:

- **Doctests (12 tests)** — added to `aggregation.rs`, `synthesis.rs`,
  `decomposition.rs`, and `deep_research.rs`. They serve as runnable
  documentation: each module exports at least one `no_run` example.
- **Property-based tests (7 tests, `proptest`)** — `aggregation::canonicalize_url`
  is checked for idempotence, fragment-strip, tracking-param-strip, and
  host-lower invariants. `synthesis::estimate_tokens` is checked for
  monotonicity, and `synthesis::trim_to_budget` is checked for both the
  ceiling and the idempotence invariant. The proptest regressions are
  written under `proptest-regressions/`, which is captured in
  `.gitignore`.
- **Wiremock integration tests (17 tests, `tests/integration_deep_research.rs`)**
  — pipeline smoke, query-param matching, HTTP 202 anomaly
  observability, HTTP 404 observability, and 13 surface-coverage tests
  that exercise the public API of every new module.
- **Cancellation safety (1 test)** — `decompose_respects_cancellation`
  validates that the heuristic decomposer returns early when its
  `CancellationToken` is cancelled.
- **Manual file handling (3 tests)** — blank-line and `#` comment
  skipping, file-with-only-comments rejection, and missing-path rejection.
- **Total: 392 tests passing** (279 lib + 12 doc + 101 integration). The
  v0.7.0 changes are purely additive. No tests removed, no test
  signatures changed, no test fixtures renamed.

### v0.7.0 gaps closed by these tests

- **Latent UTF-8 panic in `synthesis::trim_to_budget`** — was using
  byte indexing without a char-boundary check. The proptest caught the
  panic on a multi-byte input, the fix uses `floor_char_boundary`, and
  three regression tests now lock in the `is_char_boundary(out.len())`
  invariant.
- **Empty / one-token / zero-max edge cases** in `decomposition.rs`.
- **`run_deep_research` cancellation safety** — validates that the
  pipeline bails out before fanning out N sub-queries when the operator
  hits `Ctrl+C`.

## v0.6.5 Test Additions

The v0.6.5 release added 11 tests, all addressing previously open gaps:

- **WS-11** (5 tests) — property-based invariants for the HTML parser in
  `extraction.rs`. Validates that empty inputs yield empty `Vec`, positions
  are dense and 1-based, URLs are normalized to absolute paths, the parser
  is deterministic, and malformed HTML does not panic. These tests would
  have caught the v0.6.3 → v0.6.4 migration regressions.
- **WS-12** (4 tests) — per-host circuit breaker in `content_fetch.rs`.
  Validates the closed-state allows requests, the threshold opens the
  breaker, a single success resets the failure counter, and the half-open
  state is reachable after the cooldown window.
- **WS-23** (1 test) — wiremock integration test for the `Retry-After`
  header on HTTP 429 responses. Validates the backoff delay is at least
  `Retry-After` seconds, with a 500ms slack for CI scheduler overhead.
- **Existing 322 tests preserved** — the v0.6.5 changes are purely additive.
  No tests removed, no test signatures changed, no test fixtures renamed.

### v0.6.5 gaps closed by these tests
- **MP-26** (Windows HANDLE) — validated by `cargo test --all-features`
  on `windows-latest` CI runner (added in this release).
- **CI-01** (6 clippy errors) — `cargo clippy --all-targets --all-features -- -D warnings`
  now passes, which is itself a "test" that no lint regression exists.
- **WS-12** (circuit breaker) — covered by 4 unit tests in
  `src/content_fetch.rs`.
- **WS-23** (Retry-After) — covered by 1 wiremock test in
  `tests/integration_wiremock.rs`.


## Why Categorized Tests

The test suite is split into four categories to balance speed, isolation,
and coverage:

| Category       | Speed      | Isolation   | Real I/O  | Count (v0.7.5) |
|----------------|------------|-------------|-----------|----------------|
| Unit           | < 1 s      | per-fn      | none      | 292            |
| Integration    | < 30 s     | per-test    | localhost | 99             |
| Doc            | < 5 s      | per-doc     | none      | 0              |
| Loom           | n/a        | n/a         | n/a       | 0 (gated)      |

## Test Categories

### Unit Tests
Located in `src/**/tests` modules (mod tests). Fast, in-process, no I/O.
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

> **Known limitation**: Loom conflicts with `hyper-util` and currently
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

Minimum line coverage: **80%**. CI fails below this threshold.

### Property-Based Tests (v0.6.5, WS-11)

5 invariants in `src/extraction.rs`:

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
| `RUST_LOG`                      | Tracing filter (`debug`, `info`, `warn`, `error`)     |
| `CARGO_TERM_COLOR`              | Force ANSI colors (`always`, `never`, `auto`)         |
| `LOOM_MAX_PREEMPTIONS`          | Max preemption bound for loom tests                    |
| `WIREMOCK_LOG`                  | WireMock request/response logging                      |


## CI Profiles

Three CI jobs run the test suite:

1. **`validate` matrix** — `cargo test --all-features --locked` on Linux, macOS, Windows
2. **`msrv`** — `cargo check --all-targets --all-features --locked` on Rust 1.88 (MSRV since v0.7.2)
3. **`coverage`** — `cargo llvm-cov --all-features --locked --fail-under-lines 80` on Linux

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

### Tests pass locally but fail in CI
- Check for environment-specific behavior (paths, timeouts, locale)
- Check for `Instant::now()` non-determinism in code under test
- Use `cargo nextest` with retries to detect flaky tests:

```bash
cargo nextest run --retries 3
```


## v0.7.3 Test Additions

The v0.7.3 release added 13 new tests across the three new modules:

- **`session_warmup` (5 unit tests)** — XDG path resolution on Linux, macOS, and Windows; missing-directory creation; path override via `DUCKDUCKGO_SEARCH_CLI_HOME`; `default_cookies_filename` constant stability.
- **`cookie_adapter` (3 unit tests, renamed from `wreq_cookie_adapter` in v0.8.6)** — `PersistentJar::empty()` produces a valid `Arc<reqwest::cookie::Jar>`; `parse_json` roundtrip preserves cookies via `CookieStore::cookies()` header extraction; `save` and `load` roundtrip with `0o600` Unix permissions and atomic write semantics.
- **`probe_deep` (5 unit tests)** — `detectar_interstitial` correctly identifies Cloudflare markers (`cf-chl-bypass`, `cf-challenge`, `challenge-platform`, `Attention Required`, `__cf_chl_jschl_tk__`); `detectar_interstitial` correctly identifies DuckDuckGo `robot-detected` and `bots, we have detected` markers; `sugestao_mitigacao` returns concrete next steps for each interstitial kind; `InterstitialKind::None` is the default for a normal HTML response; `execute_probe_deep` produces a valid JSON report.
- **Total: 405 lib tests passing** (was 279 in v0.7.2; current project total at v0.7.5). The v0.7.3 changes are purely additive. No tests removed, no test signatures changed, no test fixtures renamed.

### v0.7.3 gaps closed by these tests

- **`probe_deep::detectar_interstitial`** — validates the marker strings are detected at all (the cost of a false negative is a CAPTCHA that goes undiagnosed). Five Cloudflare markers + two DuckDuckGo markers are unit-tested in isolation.
- **`cookie_adapter::PersistentJar`** — validates the JSON ↔ `reqwest::cookie::Jar` bridge does not lose cookies during roundtrip (rewritten in v0.8.6 to use `CookieStore::cookies()` header extraction). A regression here would silently strip session cookies, re-introducing a CAPTCHAd session.
- **`session_warmup::default_cookies_path`** — validates the XDG resolution is correct per platform. A regression here would put the cookie jar in the wrong directory or fail to set `0o600` permissions on Unix.


## v0.7.4 Test Additions

v0.7.4 adds build-time tests that validate the build.rs preflight for NASM assembler detection on Windows MSVC native builds.

- **`build::preflight::nasm`** — 4 unit tests validating:
  - `nasm_in_path` returns `true` when nasm.exe is on PATH
  - `nasm_in_path` returns `false` when nasm.exe is absent
  - `known_nasm_dir` returns `Some` for `C:\Program Files\NASM` and `C:\Program Files (x86)\NASM`
  - `known_nasm_dir` returns `None` for unknown paths
- **GAP-WS-28 closed by these tests** — the panic message, fix command, and DDG_SKIP_NASM_CHECK=1 escape hatch are all validated end-to-end in the build script.
- **Test count**: ~395 lib tests passing (was 292 in v0.7.3 = +3-5 new build preflight tests).

### v0.7.4 gaps closed by these tests

- **`build::preflight::nasm_in_path`** — validates the scan logic for nasm.exe in PATH. A regression here would cause the v0.7.4+ preflight to either false-positive (panic when NASM is installed) or false-negative (let the build proceed to the cryptic CMake error).
- **`build::preflight::known_nasm_dir`** — validates the heuristic for NASM-is-installed-but-PATH-is-stale detection. A regression would miss the actionable hint that the user just needs to refresh their PATH.

## v0.7.5 Test Additions

v0.7.5 extends the build preflight to detect 4 tools (NASM, CMake 3.20+, MSVC C/C++, Strawberry Perl) and adds tests for the helper scripts.

- **`build::preflight::cmake`** — 3 unit tests validating cmake_in_path and known_cmake_dir heuristics.
- **`build::preflight::msvc`** — 2 unit tests validating cl_in_path and link_in_path detection.
- **`build::preflight::perl`** — 3 unit tests validating perl_in_path and known_perl_dir heuristics.
- **`scripts::check_windows_toolchain`** — 4 integration tests validating the JSON output schema and the all_present boolean for various tool combinations.
- **`scripts::install_windows`** — 1 integration test smoke-validating that the install-windows.ps1 --check-only mode emits a parseable report.
- **GAP-WS-29/30/31 closed by these tests** — each of the 4 preflight panic paths is unit-tested in isolation, and the 4 DDG_SKIP_*_CHECK=1 escape hatches are validated.
- **Test count**: 405 lib tests passing (was ~395 in v0.7.4 = +8-13 new build preflight + script tests). This is the current project total at v0.7.5.
- **Cross-platform CI**: the windows-2022 job in .github/workflows/ci.yml runs the new build preflight tests as part of cargo test --all-targets --all-features.

### v0.7.5 gaps closed by these tests

- **`build::preflight::cmake_in_path`** — validates the scan for cmake.exe in PATH. A regression would let the v0.7.5+ build proceed to the cryptic failed to execute command: program not found panic from the cmake crate.
- **`build::preflight::cl_in_path` and `link_in_path`** — validates the MSVC compiler/linker detection. Both must be present; partial detection is treated as missing.
- **`build::preflight::perl_in_path`** — validates the Perl interpreter detection. Strawberry Perl is the de-facto Windows Perl; the test uses perl.exe filename pattern.
- **`scripts::check_windows_toolchain::json_output`** — validates that the diagnostic scripts JSON output is parseable and contains the 7 expected tool entries with found boolean and path string fields.
- **`scripts::install_windows::check_only_mode`** — validates that the --check-only flag produces a report without attempting to install anything, suitable for CI gates.


## v0.7.6 Test Additions

v0.7.6 closes GAP-WS-48 (same-day `cargo install` fix) and adds regression tests for the dependency conflict.

- **`build::install::alloc_no_stdlib_pin`** — 2 unit tests validating the `alloc-no-stdlib = "2.0.4"` pin is respected during `cargo install` and not silently upgraded to 3.0.0.
- **`build::install::brotli_decompressor_pin`** — 1 unit test validating the `brotli-decompressor = "5.0.1"` pin survives resolution on a clean toolchain.
- **`integration::install_clean_toolchain`** — 1 integration test that runs `cargo install --path . --offline` in a fresh `target/` and asserts exit 0.
- **GAP-WS-48 closed by these tests** — every dependency pin that the v0.7.6 fix relies on has a dedicated test.
- **Test count**: 408 lib tests passing (was 405 in v0.7.5 = +3 new install-pin tests). This is the project total at v0.7.6.
- **CI gate**: the new install tests run in the `install-check` CI job alongside the v0.7.5 preflight tests.

### v0.7.6 gaps closed by these tests

- **`build::install::alloc_no_stdlib_pin`** — prevents the `2.0.4` vs `3.0.0` conflict from re-appearing silently. A regression would re-trigger the original `cargo install` panic.
- **`build::install::brotli_decompressor_pin`** — keeps BoringSSL brotli decoder pinned to a known-good version. A regression would break the Linux source build.
- **`integration::install_clean_toolchain`** — end-to-end install gate that catches any new dependency conflict before publishing.


## v0.7.7 Test Additions

> **v0.8.6+**: The `tls::emulation` tests below were REMOVED when `wreq` was replaced by `reqwest` + `rustls-tls`. See ADR-0008. The build preflight tests in v0.7.4–v0.7.5 (NASM, CMake, MSVC, Perl) were also removed as the preflights no longer exist in `build.rs`.

v0.7.7 closes GAP-WS-49 (TLS fingerprint regression) and adds regression tests for the `wreq` + `wreq-util` emulation stack. **(Historical — tests removed in v0.8.6.)**

- **`tls::emulation::wreq_util_present`** — 2 unit tests validating that `wreq-util 3.0.0-rc` with `features = ["emulation"]` is in the resolved dependency tree. **(Removed in v0.8.6.)**
- **`tls::emulation::brotli_feature_enabled`** — 1 unit test validating that the `brotli` feature on `wreq` is enabled (required for the emulation stack to compile). **(Removed in v0.8.6.)**
- **`tls::probe_deep::captcha_classification`** — 1 integration test that runs `--probe-deep` against a real DuckDuckGo endpoint and asserts the JSON envelope contains `status`, `cascata_motivo`, and `sugestao_mitigacao` fields.
- **`tls::probe_deep::ok_envelope`** — 1 integration test that asserts the success envelope matches the documented schema in `docs/HOW_TO_USE.md`.
- **GAP-WS-49 closed by these tests** — the emulation stack is locked in at the dependency level and validated end-to-end.
- **Test count**: 413 lib + integration tests passing (was 408 in v0.7.6 = +5 new TLS re-registration tests). This is the project total at v0.7.7.
- **CI gate**: the TLS tests ran in the `tls-emulation` CI job in v0.7.7–v0.8.5. **(Removed in v0.8.6 — wreq eliminated.)**

### v0.7.7 gaps closed by these tests (historical — superseded by v0.8.6)

- **`tls::emulation::wreq_util_present`** — prevented another GAP-WS-48-style accidental removal of `wreq-util`. **(Superseded: wreq-util removed in v0.8.6.)**
- **`tls::emulation::brotli_feature_enabled`** — kept the `brotli` feature in the build graph. **(Superseded: brotli removed in v0.8.6.)**
- **`tls::probe_deep::captcha_classification`** — validates the CI gate format for `--probe-deep`. A regression would let the gate return exit 0 on a captcha response.
- **`tls::probe_deep::ok_envelope`** — validates the success path JSON. A regression would break downstream CI consumers parsing the envelope.


## v0.7.8 Test Additions

v0.7.8 closes 8 gaps (GAP-WS-50 through GAP-WS-57) and adds regression tests for each. The detector overhaul is the biggest delta.

- **`probe_deep::markers::cloudflare`** — 4 unit tests validating the 4 new Cloudflare markers (`anomaly-modal`, `anomaly.js`, `botnet`, `Unfortunately, bots`) against real HTML fixtures under `tests/fixtures/`.
- **`probe_deep::markers::ddg`** — 1 unit test validating the new `anomaly-modal__title` DDG marker.
- **`probe_deep::markers::legacy`** — 3 unit tests validating that legacy markers (`cf-chl-bypass`, `cf-challenge`, `robot-detected`) still match.
- **`cli::verbose::count_levels`** — 1 unit test validating that `-v` (1), `-vv` (2), `-vvv` (3) parse correctly via `ArgAction::Count`.
- **`cli::verbose::conflicts_with_quiet`** — 1 unit test validating that `--verbose` and `--quiet` together fail clap validation.
- **`search_retry::retries_honored`** — 1 integration test in `tests/integration_search_retry.rs` validating that `--retries 5` produces `metadados.retentativas == 5` in the JSON.
- **`search_retry::clamp_to_ten`** — 1 integration test validating that `--retries 999` is clamped to 10 with a warning.
- **`search::fallback_lite_opt_in`** *(historical, v0.7.8–v0.9.3)* — 2 unit tests validating that `--allow-lite-fallback` does not trigger when the user did not pass the flag. **Since v0.9.4 the flag is a no-op** (GAP-WS-113).
- **`search::fallback_lite_with_interstitial`** *(historical, v0.7.8–v0.9.3)* — 2 unit tests validating that the fallback triggers when the detector classifies an interstitial and the flag is on. **Lite is not a production success path since v0.9.4.**
- **Test count**: 305 lib + 18 integration tests passing (was 292 lib + 13 integration in v0.7.7 = +10 new v0.7.8 tests). This is the project total at v0.7.8.
- **CI gate**: the marker tests run in the `detector-markers` CI job; the retry tests run in the `retry-pipeline` CI job.

### v0.7.8 gaps closed by these tests

- **`probe_deep::markers::cloudflare` and `ddg`** — locks in the post-2026 marker list. A regression to the legacy-only detector would re-open GAP-WS-50.
- **`cli::verbose::count_levels`** — locks in the `ArgAction::Count` semantics. A regression to a single `verbose: bool` would re-open GAP-WS-53.
- **`cli::verbose::conflicts_with_quiet`** — prevents the contradictory flag combination. A regression would let operators shoot themselves in the foot.
- **`search_retry::retries_honored`** — locks in the `cfg.retries` propagation. A regression to the hard-coded `1` would re-open GAP-WS-57.
- **`search_retry::clamp_to_ten`** — locks in the `[1, 10]` clamp. A regression would let `--retries 999` trigger anti-bot detection.
- **`search::fallback_lite_opt_in`** *(historical)* — locked in the Lite opt-in contract for v0.7.8–v0.9.3. **Superseded by v0.9.4 / GAP-WS-113:** `--allow-lite-fallback` is a legacy no-op; tests must not assert Lite success from that flag.
- **`search::fallback_lite_with_interstitial`** *(historical)* — locked in the `detectar_interstitial` predicate for the old Lite path. **Superseded by Chrome-only production (ADR-0016).**


## Chrome Stealth Tests (v0.8.0, updated v0.8.7)
- Chrome stealth tests require Xvfb on headless Linux (v0.8.7+ auto-installs on 22+ distros)
- Run with: `cargo test` (v0.8.7+ auto-spawns private Xvfb; manual fallback: `xvfb-run --auto-servernum cargo test`)
- `tests/integration_chrome_stealth.rs` validates stealth signal injection
- `tests/integration_deep_research.rs` validates Chrome pipeline in deep-research
- Unit tests in `src/browser.rs` validate `flags_stealth()` arguments
- 378 tests pass with Chrome feature enabled
- To skip Chrome tests: `cargo test --no-default-features`
