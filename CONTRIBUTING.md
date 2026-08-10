# Contributing to duckduckgo-search-cli
Read this in [Portuguese](CONTRIBUTING.pt-BR.md).


## Welcome
- Thank you for your interest in contributing to duckduckgo-search-cli
- Every contribution improves a tool used by developers and AI agents worldwide
- This guide covers the minimum you need to land a change successfully
- Current line documented here: **v1.0.5** (stdout boundary ruler, agent-ops matrix, identity never truncated, single refusal envelope)
- Wire break introduced in v1.0.2: see [docs/MIGRATION.md](docs/MIGRATION.md)


## Quick Start
Clone the repository and run the fast gates in five commands:

```bash
git clone https://github.com/danilo-aguiar-br/duckduckgo-search-cli
cd duckduckgo-search-cli
cargo check-all    # gate 1 — compile
cargo lint         # gate 2 — clippy -D warnings
cargo fmt --check  # gate 3 — format
cargo test-all     # gate 5 — unit + integration + doctest
```

Aliases live in [`.cargo/config.toml`](.cargo/config.toml) (`check-all`, `lint`,
`docs`, `test-all`, `check-windows`, `check-windows-msvc`, `lint-windows`,
`check-nohttp`, `lint-nohttp`, `cov`, `cov-html`, `publish-check`, `pkg-list`).
Prefer them over expanding every flag by hand. Full local pipeline:

```bash
cargo check-all && cargo lint && cargo fmt --check && \
  RUSTDOCFLAGS="-D warnings" cargo docs && cargo test-all
```


## CLI Surface (v1.0.5)
Public subcommands live in the clap tree at `src/cli/mod.rs`. Wire serialization defaults to **EN** (ADR-0027).

| Surface | Notes |
| ------- | ----- |
| Default search | `duckduckgo-search-cli [OPTIONS] [QUERY]...` (no subcommand) |
| `buscar` | Hidden alias of default search |
| `init-config` | `--force`, `--dry-run` |
| `completions <SHELL>` | bash / zsh / fish / powershell / elvish |
| `deep-research` | fan-out + aggregate; `--print-budget`, budget flags |
| `commands` | JSON command tree (agent discovery) |
| `schema` | catalog or `--name NAME` |
| `doctor` | `--strict`, `--probe-deep` (root `--probe` is separate) |
| `locale` | resolved UI locale JSON |
| `man` | roff man page; optional `--file PATH` |
| `config path` | print XDG config dir as JSON |
| `config list` | list keys in `config.toml` |
| `config get` | get one key (positional or `--key`) |
| `config set` | set one key (positional or `--key`/`--value`) |
| `config unset` | remove one key |
| `config effective` | merged CLI > XDG > FACTORY JSON |
| `help` | clap help |

Agent ops (global): `--fields`/`--select`, `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only`, `--truncate-content`, `--max-output-bytes`, `--wire-keys en|pt`.

Since v1.0.4 every agent op either acts or refuses by name with exit code 2 — there is no third outcome. Since v1.0.5 identity strings (config keys, locale tags, schema ids, paths, probe error codes) are exempt from `--truncate-content`, and surfaces made entirely of identifiers refuse the flag instead of returning an unchanged envelope.

Full one-liners: root [`INTEGRATIONS.md`](INTEGRATIONS.md), catalog [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md), tables in [`README.md`](README.md).


## Code of Conduct
- This project adopts the [Contributor Covenant 2.1](CODE_OF_CONDUCT.md)
- Read it in full before opening any issue or pull request
- Report violations through the channel described in `CODE_OF_CONDUCT.md`


## Development Setup
### Prerequisites
- MSRV (Minimum Supported Rust Version): Rust 1.88 — declared in `Cargo.toml` (`rust-version`) and pinned in `rust-toolchain.toml`
- Run `rustup update stable` to match the toolchain
- Install llvm-cov: `cargo install cargo-llvm-cov`
- Install cargo-audit: `cargo install cargo-audit`
- Install cargo-deny: `cargo install cargo-deny`
- Add the cross-check targets once per host, with no root and no system package: `rustup target add x86_64-pc-windows-gnu x86_64-pc-windows-msvc aarch64-apple-darwin x86_64-apple-darwin`
- This project does **not** use `cargo-nextest` — the suite runs via plain `cargo test` / `cargo test-all`


## Chrome Development Prerequisites
- Install Google Chrome or Chromium for E2E tests
- Linux: Xvfb is auto-installed by the CLI at runtime via `try_auto_install_xvfb()` for 22+ distros
- For development, install it manually: `sudo dnf install xorg-x11-server-Xvfb` (Fedora) or `sudo apt-get install xvfb` (Debian/Ubuntu)
- macOS/Windows: no extra dependency — Chrome runs in **headless=new** since v0.9.3 (not headed native Quartz/DWM; that path was v0.9.1 only and is superseded)
- Run E2E tests: `cargo test-all` (or `cargo test --all-features --locked`; the CLI auto-spawns Xvfb if needed)
- Run tests without Chrome: `cargo test --no-default-features`
- Product headless toggle is the CLI flag **`--chrome-headless`** (not a product env). Verbosity is **`-v`/`-vv`/`-q`** or XDG `log_directive` — the product does **not** use `RUST_LOG` (GAP-LOG-ENV-001). **No product env** for runtime knobs — CLI flags + XDG only.
- The `chrome` feature is enabled by default in `Cargo.toml`
- Chrome stealth tests are in `tests/integration_stealth_block_classification.rs`
- Deep-research Chrome tests are in `tests/integration_deep_research.rs`
- **Wire JSON (v1.0.2, ADR-0027)** serializes **English** keys by default (`.results`, `.metadata`, …). Tests and fixtures may still deserialize PT aliases. Legacy agents: `--wire-keys pt` or `config set wire_keys pt`. See [docs/MIGRATION.md](docs/MIGRATION.md).
- **Agent-ready defaults (v0.9.8, still current in v1.0.5)** affect E2E latency: content fetch is **ON** and the default vertical is **`all`** (dual web+news). Prefer longer timeouts, or use `--vertical web --no-fetch-content` when a thin and fast smoke is enough.
- **Test-harness-only env vars** (not product config — never document them as runtime knobs for end users):
  - `DUCKDUCKGO_FLATPAK_E2E=1` — **test harness only, not product config**
  - `DUCKDUCKGO_LIFECYCLE_E2E=1` — **test harness only, not product config**
  - `DUCKDUCKGO_CHROME_HEADLESS=1` — **test harness only, not product config** (the product uses CLI `--chrome-headless`)
- **Flatpak multi-canal E2E (v0.9.8+)** — gated behind `DUCKDUCKGO_FLATPAK_E2E=1` (**test harness only, not product config**):

  ```bash
  DUCKDUCKGO_FLATPAK_E2E=1 cargo test --test integration_flatpak_chrome -- --nocapture
  ```

  Covers Flatpak export→ELF resolve (`files/extra/chrome`) when a Flatpak Chrome deploy is present.
- **Lifecycle E2E (v1.0.0 contract, current line v1.0.5; GAP-WS-TMP-PROFILE-ORPHAN-001 + process GAP-WS-LIFECYCLE-001)** — gated behind `DUCKDUCKGO_LIFECYCLE_E2E=1` (**test harness only, not product config**):

  ```bash
  DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle
  ```

  Requires Chrome; asserts no residual chrome process remains with this run's `user-data-dir` after exit; profile path prefix **`ddg-chrome-`**. Unit tests cover `force_reap` / `sweep_orphan_profiles` / ownership guards (never `.tmp*` bulk delete) without the E2E env var. See **ADR-0020** (disk one-shot) and **ADR-0017** (process one-shot).


## Branching Strategy
- Main branch: `main`
- Feature branches: `feature/descriptive-name`, cut from main
- Fix branches: `fix/bug-name`, cut from main
- Open the PR back into main
- Squash and Merge is the default merge method


## Commit Convention
- Use conventional prefixes: `feat:`, `fix:`, `deps:`, `docs:`, `test:`, `refactor:` (never `ci:` — CI/CD is forbidden)
- Never add `Co-authored-by:` trailers from AI agents such as dependabot, renovate, Claude, GPT, Copilot, Cursor, or Gemini
- Use squash and merge for PRs carrying multiple commits
- Write the subject in terms of the problem solved, not the file touched


## Coding Standards
### Mandatory conventions
- Code comments, log messages, and struct field names are in Brazilian Portuguese, per `CLAUDE.md`
- Public API identifiers may be English when they follow conventional Rust style, such as `from` and `into`
- Never use `.unwrap()` or `.expect()` in production code
- Propagate errors with `?` and the typed variant defined in `src/error.rs` (enum `CliError` via `thiserror`)
- The project uses plain `thiserror 2` — `anyhow` is **not** a dependency
### Centralized I/O
- `src/output/` is the ONLY place allowed to call `println!` or `print!`
- Every other module logs through `tracing`
- `tests/integration_stdout_boundary.rs` sweeps every stdout emission and fails the build on an undeclared bypass
### TLS and anti-fingerprint (dual-plane — ADR-0021 / ADR-0022)
- **Production SERP/probe/fetch:** **native Chrome** transport (the browser TLS stack on the host; ADR-0016). Goal: do **not** expose a library TLS signature (`rustls` JA4 bot-class) that Cloudflare blocks (GAP-WS-27). It is **not** a "fingerprint feature".
- **Forbidden (ADR-0022):** synthetic hardware fingerprint spoofing (forced canvas/WebGL/Audio/hwConcurrency) — it becomes a shared automation signature.
- **Allowed stealth:** CDP automation signals only (`webdriver`, plugins, `window.chrome`, DevTools leak) — see `src/browser/stealth.rs`.
- **Residual HTTP** (harness): `reqwest` + rustls + CryptoProvider **`aws-lc-rs`** (`tls_bootstrap`). Feature `rustls-tls-webpki-roots-no-provider` (no `ring`).
- Never re-enable `native-tls` / OpenSSL. Never enable the chromiumoxide fetcher features.
- Residual proxy: `--proxy` or XDG config only — no inheritance from `HTTP_PROXY`.
### Design constraints
- No cache, no MCP, no paid API — non-negotiable constraints from the v2 blueprint


## Testing
### Three test layers
- Inline unit tests with `#[cfg(test)] mod testes` for pure functions
- Integration tests in `tests/` using `wiremock` — ZERO real HTTP
- Doctests inside `///` blocks on public APIs — they double as examples on docs.rs
### Running tests
- Run the suite with `cargo test-all` (or `cargo test --all-features --locked`)
- Run coverage with `cargo cov` — 80% minimum is mandatory
- Reject any PR that pushes coverage below the threshold during local review
### News vertical (v0.8.9)
- Fixtures in `tests/fixtures/`: `ddg_news_serp.html` (Strategy A, 7 articles + 1 filtered internal trap), `ddg_news_serp_ofuscada.html` (Strategy B fallback), `ddg_news_serp_vazia.html` (empty SERP → `zero_cause: vertical-no-results`)
- Integration tests: `tests/integration_news_vertical.rs`, `tests/integration_deep_research_news.rs` — run them with `cargo test --features chrome --test integration_news_vertical --test integration_deep_research_news`
- Hot-fix without recompiling: a DDG-side selector break is fixable through `config/selectors.toml` section `[news]` (Strategy A); Strategy B is the class-agnostic safety net
- See `docs/TESTING.md` for the full news vertical test matrix
### Contract rulers (v1.0.4 / v1.0.5)
- `tests/integration_schema_conformance.rs` validates real envelopes against `docs/schemas/*.json` in both wire languages
- `tests/integration_agent_ops_matrix.rs` runs every agent op against every offline surface and demands either fewer bytes or exit 2
- `tests/integration_stdout_boundary.rs` fails the build on a new stdout bypass and on a stale exemption
- `tests/integration_docs_drift.rs` fails with the exact set of flags that drifted between the binary and both READMEs


## 10-Gate Validation Matrix
### Required gates
Every PR must pass all 10 gates **locally**. CI/CD and GitHub Actions are **forbidden** in this repo.
Prefer the aliases from [`.cargo/config.toml`](.cargo/config.toml) where listed:

| # | Gate | Local command |
|---|------|---------------|
| 1 | Compilation | `cargo check-all` |
| 2 | Clippy | `cargo lint` |
| 3 | Format | `cargo fmt --all -- --check` |
| 4 | Docs | `RUSTDOCFLAGS="-D warnings" cargo docs` |
| 5 | Tests | `cargo test-all` |
| 6 | Coverage >= 80% | `cargo cov` |
| 7 | Vuln audit | `cargo audit --deny warnings` |
| 8 | Supply chain | `cargo deny check advisories licenses bans sources` |
| 9 | Publish dry-run | `cargo publish-check` |
| 10 | Package content | `cargo pkg-list` |

### Cross-platform gates
Run these before every tag as well. Forbidding remote CI does not remove the need to verify other platforms — it moves that verification onto the maintainer's host, and v1.0.2 shipped to crates.io unable to compile on macOS or Windows because no gate passed `--target`. The full rationale is in [NO_CI.md](NO_CI.md).

| # | Gate | Local command |
|---|------|---------------|
| A | Ungated `use` of a gated item | `./scripts/portability-lint.sh` |
| B | Windows GNU ABI | `cargo check-windows` |
| C | Windows MSVC ABI | `cargo check-windows-msvc` |
| D | Windows clippy | `cargo lint-windows` |
| E | macOS ARM | `./scripts/check-macos.sh` |
| F | macOS Intel | `./scripts/check-macos.sh x86_64-apple-darwin` |
| G | Host without the HTTP harness | `cargo check-nohttp` and `cargo lint-nohttp` |


## Pull Request Process
### Before opening the PR
- `cargo fmt --all -- --check` returns ZERO differences
- `cargo lint` returns ZERO warnings
- `cargo test-all` returns ZERO failures
- `RUSTDOCFLAGS="-D warnings" cargo docs` returns no warnings
- `cargo audit --deny warnings` reports no known vulnerability
- The cross-platform gates above pass when the change touches `cfg`, process, or browser code
- `CHANGELOG.md` and `CHANGELOG.pt-BR.md` are updated with the change
- The PR title describes the problem solved in user terms
### Review
- Open the PR against `main` and keep the diff scoped to one problem
- Answer review comments in the PR thread instead of force-pushing silently
- Squash and merge once every gate above is green on the maintainer's host


## Documentation
- Update both language versions of any document you touch — EN and pt-BR must stay technically identical
- Never translate commands, flags, exit codes, or file names; translate titles and prose
- Document a new flag in `README.md`, `README.pt-BR.md`, and the relevant `docs/` page in the same PR
- Add a `docs/decisions/` ADR when the change inverts a default or breaks a published contract
- Add or update a JSON Schema under `docs/schemas/` whenever an emitted envelope changes shape
- Every file under `docs/generated/` must name its consumer, or the guard fails the build


## Supply Chain
- Every new dependency must pass `cargo deny check`
- If a candidate brings a license outside the allowlist or a transitive advisory, find an alternative or document the ignore in `deny.toml`
- Document each ignore with `# Why:` and `# How to apply:` lines in `deny.toml`
- Prefer crates with `trustScore >= 7` in `context7-cli` (see `CLAUDE.md`)


## How to Report Bugs
### Bug report template
- Open an issue titled `[bug] concise description of the problem`
- Include the CLI version: `duckduckgo-search-cli --version`
- Include the operating system and the Rust version: `rustc --version`
- Include the exact command that reproduces the problem
- Include the complete output, stderr included


## How to Request Features
### Feature request template
- Open an issue titled `[feature] concise description`
- Describe the problem the feature would solve
- Describe the expected behavior
- Include usage examples or real cases


## Reporting Security Issues
- See [SECURITY.md](SECURITY.md) for the full process
- Never open a public issue for a vulnerability
- Use the private GitHub advisory channel for responsible disclosure


## Release Process
### Maintainer flow
- Bump the `version` field in `Cargo.toml`
- Move the `[Unreleased]` content in `CHANGELOG.md` under a new version header with a date
- Mirror the same entry in `CHANGELOG.pt-BR.md`
- Run the 10 validation gates **locally**, plus the cross-platform gates (no Actions)
- Create an annotated tag: `git tag -a v1.0.X -m "description"`
- Push: `git push origin main && git push origin v1.0.X` (tag only; **no** release workflow)
- Publish to crates.io **manually**: `cargo publish --locked`, after the dry-run and explicit authorization
- There is **no** GitHub Actions matrix, no Dependabot, no zizmor, no pre-commit hooks, and no GitHub Actions secrets — all forbidden


## Pre-Publish (local only)
- CI/CD and GitHub Actions are **forbidden** in this repository (there is no `.github/workflows` directory)
- Before publishing: run the 10 local gates, the cross-platform gates, and `cargo publish --dry-run --locked`
- Maintainers publish manually with `cargo publish --locked` after explicit authorization
- Yank window for a broken release: 72 hours


## Recognition
- Every merged contribution is credited in `CHANGELOG.md` and `CHANGELOG.pt-BR.md` under the version that ships it
- Security reporters are credited in the Hall of Fame of [SECURITY.md](SECURITY.md), unless they ask to stay anonymous
- Contributors who close a documented gap are named alongside the gap id in the ADR that records the decision
- Ask to be credited under a different name, or not at all, and that request is honored


## Questions
- Open a GitHub issue titled `[question] concise subject` for anything this guide does not answer
- Read [docs/HOW_TO_USE.md](docs/HOW_TO_USE.md) and [docs/COOKBOOK.md](docs/COOKBOOK.md) before asking about usage
- Run `duckduckgo-search-cli commands` and `duckduckgo-search-cli schema` to discover the live surface instead of guessing
- Never use an issue to report a vulnerability — follow [SECURITY.md](SECURITY.md) instead


## Related Documentation
- [NO_CI.md](NO_CI.md) — **policy: CI/CD and GitHub Actions are forbidden** (local gates only)
- [CHANGELOG.md](CHANGELOG.md) and [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md) — synchronized bilingual history
- [SECURITY.md](SECURITY.md) — responsible disclosure policy and supported versions
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — Contributor Covenant 2.1
- [INVERSIONS.md](INVERSIONS.md) — architectural inversions and their no-go criteria
- [docs/INSTALL-WINDOWS.md](docs/INSTALL-WINDOWS.md) — Windows setup (NOTE: since v0.8.6, `reqwest`+`rustls-tls` replaced BoringSSL/wreq, so the native build prerequisites are no longer required)
- [INTEGRATIONS.md](INTEGRATIONS.md) — catalog of integrations with 16+ AI agents
- [docs/INTEGRATIONS.md](docs/INTEGRATIONS.md) — full integration guide
- [docs/decisions/](docs/decisions/) — Architecture Decision Records (ADRs)
- [docs/CROSS_PLATFORM.md](docs/CROSS_PLATFORM.md) — per-platform behavior


## Agent Teams Workflow
- Releases from v0.7.8 onward used the 8-phase Agent Teams flow
- Each teammate receives a self-contained prompt with Rule Zero, identity, context, and tools
- The lead coordinates, delegates, and verifies — it does not implement directly
- See `CLAUDE.md` at the repository root for the full protocol
- ADRs in `docs/decisions/` record the decisions taken in each release
- Since v0.7.10, releases use `atomwrite` plus `TaskCreate` instead of Agent Teams, because of the known `Team does not exist` state bug recorded in graphrag `mem 1244`
- Each patch runs `atomwrite read` (checksum) → `atomwrite write` → `cargo check --offline` → `cargo test --lib --offline`


## Release Notes v0.7.8
### Eight gaps closed (anti-bot detector overhaul)
- GAP-WS-50 — expanded lists in `src/probe_deep.rs` (8 Cloudflare markers + 1 DDG)
- GAP-WS-51 — constant `PROBE_CALIBRATION_QUERY` in `src/lib.rs` for the canonical probe query
- GAP-WS-52 — conditional fallback predicate in `src/search.rs` honors the real detector
- GAP-WS-53 — `-vv` and `-vvv` levels added in `src/cli.rs` with `ArgAction::Count`
- GAP-WS-54 — `scraper` bumped to 0.27, resolving transitive RUSTSEC-2025-0057
- GAP-WS-55 — wreq block rewritten in `Cargo.toml` with an exact pin on 6.0.0-rc.29
- GAP-WS-56 — `Buscar` subcommand marked `#[command(hide = true)]`
- GAP-WS-57 — `retries` now honored in `src/parallel.rs` inside the error_output loop
- Full ADR in `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`


## Release Notes v0.7.9
### Ghost-block + 2026 markers (eight gaps closed)
- GAP-WS-58 (CRITICAL) — `detectar_interstitial` classifies a sub-4KB body without `result-page-signal` as `InterstitialKind::Cloudflare`
- GAP-WS-59 (HIGH) — 5 new Cloudflare markers + 1 new DDG marker
- GAP-WS-59 (HIGH) — `--allow-lite-fallback` and `--pre-flight` became `global = true`
- v0.7.9 P1 — `detectar_interstitial_com_match` returns `(&'static str, InterstitialKind)` with the literal marker
- v0.7.9 P3 — `SearchMetadata.pre_flight_fired: bool` added to the envelope
- v0.7.9 P4b — `sugestao_mitigacao_com_marker` injects the real marker (for example `cf-challenge`)
- `Config.pre_flight` added with default `false`


## Release Notes v0.7.10
### Identity pin + bench wiring + pre-publish gate (seven gaps closed)
- GAP-WS-60 (CRITICAL) — `--identity-profile` propagates to `failure_output` and `error_output` through `identity_tag_for_cli_identity` in `src/identity.rs`
- GAP-AUD-001 (local audit) — the `identidade_usada` pin is now present on failure paths (it was `null`)
- GAP-AUD-002 (local audit) — `[[bench]] harness = false` in `Cargo.toml` fixes `cargo bench`, which was running the test harness
- B1 (CRITICAL) — `--pre-flight` no longer emits two concatenated JSON objects on stdout
- B2 (CRITICAL) — `pre_flight_blocked` now returns exit 3 (it was 0)
- B3 (MEDIUM) — `--global-timeout` became global, accepted on subcommands
- B4 (CRITICAL) — standalone `--probe-deep` returns exit 3 when it detects a captcha
- v0.7.10 P4 — `--require-results` on `deep-research`, exit 4 when fan-out is zero
- v0.7.10 P5 — probe-deep scheduler integrated into `execute_single_search`
- v0.7.10 P6 — snapshot test `cloudflare_markers_snapshot_v0_7_10` via `insta = "1"`
- v0.7.10 P7 — `src/proxy_detection.rs` new module (Vivo Fiber, Gigaweb, Cloudflare)
- v0.7.10 P16 — `src/ddg_class_watch.rs` runtime watchdog
- v0.7.10 P19 — pre-publish checklist is local only (script removed; gates 1–10 are manual)
- v0.7.10 P19 — `skills/duckduckgo-search-cli-{en,pt}/evals/queries.json` +4 queries (q47-q50)
