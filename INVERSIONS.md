# Architectural Inversions

`duckduckgo-search-cli` deliberately inverts several common Rust ecosystem
defaults. This document explains each inversion, why it was made, and what
the trade-off is. Read this before proposing a "standard" alternative in
PRs — every inversion here has a recorded rationale that a "more idiomatic"
choice would silently break.

## Inversion 1 — `wreq` instead of `reqwest` (v0.7.3–v0.8.5, REVERSED in v0.8.6)

> **Status: REVERSED in v0.8.6** — replaced by `reqwest` + `rustls-tls` (ADR-0008). Chrome headed (v0.8.0+) provides real browser TLS fingerprint, making BoringSSL emulation redundant. The BoringSSL build toolchain (NASM, CMake, Perl) blocked Windows users from `cargo install`.

- **Default expectation**: new Rust CLI projects use `reqwest` with `rustls-tls`.
- **What we did (v0.7.3)**: replaced `reqwest 0.12 + rustls` with `wreq 6.0.0-rc.29`
  (statically links BoringSSL).
- **Why**: `rustls` produces a canonical TLS fingerprint that Cloudflare Bot
  Management recognizes as non-browser, triggering CAPTCHA interstitials on
  DuckDuckGo. `wreq` + BoringSSL produces a fingerprint identical to Chrome
  and Safari, eliminating the CAPTCHA on macOS. See `docs/decisions/0001-tls-boring-via-wreq.md`.
- **Trade-off**: `wreq 6.0.0-rc` is a release candidate (not stable 1.0);
  compile time is ~40s longer due to BoringSSL; builds require `cmake`,
  `perl`, `pkg-config`, `libclang-dev` on Linux and NASM/CMake/MSVC/Perl on
  Windows. Every `cargo install` compiles BoringSSL from source.
- **Why reversed (v0.8.6)**: Chrome headed (primary transport since v0.8.0) generates a REAL browser TLS fingerprint, making wreq/BoringSSL emulation redundant. The BoringSSL build toolchain (NASM, CMake, Perl, MSVC) was a total barrier for Windows users (GAP-WS-066). See `docs/decisions/0008-reqwest-rustls-v0-8-6.md`.

## Inversion 2 — Thiserror for libs, no anyhow in library code (v0.5.0+)

- **Default expectation**: `anyhow::Result` is the de-facto standard for
  application-level Rust code.
- **What we did**: defined `enum CliError` (15 variants) in `src/error.rs`
  via `thiserror`. Every error has a typed `error_code()` and `exit_code()`.
  No `anyhow` in `src/`.
- **Why**: machine-readable exit codes (0..=6) and error codes
  (`http_error`, `rate_limited`, etc.) are part of the public contract.
  `anyhow` would erase these. AI agents and CI scripts branch on
  `error_code` to decide retry vs. fail.
- **Trade-off**: 15 variant match arms on every `?`. New error types
  require updating `exit_code()` and `error_code()`. Mitigation:
  the `error.rs` `#[non_exhaustive]` attribute on `CliError` allows
  downstream consumers to be forward-compatible.
- **No-go for revert**: removing typed errors would silently break every
  agent that matches on `error_code` for retry logic.

## Inversion 3 — `BTreeMap` for histogram in multi-query output (v0.8.0+)

- **Default expectation**: `HashMap` for aggregation.
- **What we did**: `MultiSearchOutput.causa_zero_histogram: BTreeMap<String, u32>`.
- **Why**: deterministic iteration order across runs is required for
  golden-file snapshot tests and for reproducible JSON output
  (`insta = "1"` snapshot tests). `HashMap` introduces random
  iteration order → flaky snapshot tests.
- **Trade-off**: slightly slower insert (O(log n) vs O(1)). Histogram
  has <100 entries in practice; cost is negligible.
- **No-go for revert**: a non-deterministic JSON output breaks the
  snapshot test contract.

## Inversion 4 — Portuguese Brazilian field names in JSON output (v0.2.0+)

- **Default expectation**: Rust ecosystem uses English identifiers.
- **What we did**: `SearchResult` fields serialize as `posicao`, `titulo`,
  `url`, `url_exibicao`, `snippet`, etc. (not `position`, `title`, `url`).
- **Why**: README examples and `jaq` recipes in `docs/COOKBOOK.md` use
  Portuguese queries; English fields broke those pipelines (bug reported
  by user in v0.1.0 → fixed in v0.2.0). The PT-BR naming is a
  load-bearing part of the agent's mental model.
- **Trade-off**: pipelines from other ecosystems (`n8n`, `zapier`,
  `make.com`) need to learn the Portuguese field names. The
  `docs/INTEGRATIONS.md` documents the full mapping table.
- **No-go for revert**: changing field names would silently break every
  CI pipeline built on the v0.2.0+ contract. The v0.1.0 → v0.2.0
  migration guide was a one-time event.

## Inversion 5 — `#[serde(skip_serializing_if = "Option::is_none")]` for ALL Option fields

- **Default expectation**: serialize `Option::None` as JSON `null`.
- **What we did**: every `Option<T>` field in `types.rs` carries
  `#[serde(skip_serializing_if = "Option::is_none")]`.
- **Why**: the JSON envelope should be minimal — consumers don't
  need to differentiate "field absent" from "field is null". Absent
  fields mean "not applicable for this query" (e.g., `causa_zero`
  is absent when results > 0, present when zero).
- **Trade-off**: pipelines can't distinguish "field missing" from
  "field was null at serialization". Mitigation: the SKILL.md documents
  the field semantics; `causa_zero` field is an additive diagnostic
  (BC opt-out preserves the field even when exit code is 5 legacy).
- **No-go for revert**: turning on `null` serialization would
  double the size of every JSON output and require every consumer
  to handle both `null` and missing.

## Inversion 6 — `--allow-lite-fallback` as OPT-IN (v0.7.8+; SUPERSEDED / NO-OP since v0.9.4)

> **Status: SUPERSEDED / NO-OP since v0.9.4 (GAP-WS-113 / ADR-0016)** — production is Chrome-only; Lite is never a success path. The flag remains for script BC only and does not force endpoint degradation.

- **Default expectation**: fallback to lite endpoint when html fails.
- **What we did (v0.7.8–v0.9.3)**: fallback required explicit `--allow-lite-fallback`
  flag. Without it, anti-bot detection returned exit 3 with
  `cascata_motivo` populated in JSON, NOT silent fallback.
- **What we do now (v0.9.4+)**: the flag is a **legacy no-op**. SERP stays HTML
  canonical under Chrome; install Chrome / `--chrome-path` / `--proxy` for remediation.
- **Why (original)**: silent fallback violates user intent. The user may want to
  know they're being blocked (for rate limit purposes) rather than
  receive truncated results from a degraded endpoint. v0.7.8 GAP-WS-52
  fixed the silent fallback behavior.
- **Why no-op now**: dual transport (HTTP/Lite under Chrome) produced zero hits
  misclassified as legitimate; GAP-WS-113 removes Lite as a production success path.
- **Trade-off**: scripts that still pass the flag are harmless (no-op) but must
  not treat it as active remediation.
- **No-go for revert**: reintroducing Lite as a silent success path would restore
  the covert dual-transport channel closed by ADR-0016.

## Inversion 7 — `bin/safety-contracts` binary for CI gates (v0.7.10+)

- **Default expectation**: a single CI workflow runs all checks.
- **What we did**: each CI gate is a discrete `bin/` script invoked
  individually by the workflow. Examples: `bin/check-fmt`, `bin/check-clippy`,
  `bin/check-tests`, `bin/check-audit`, `bin/check-coverage`, `bin/check-version-drift`.
- **Why**: discrete binaries let developers run the exact CI gate
  locally before pushing. A single `ci.yml` workflow with embedded
  bash was untestable in isolation.
- **Trade-off**: 9+ binaries to maintain. Mitigation: each binary
  is <50 lines and has a `README.md` per script.
- **No-go for revert**: monolithic CI is a known pain point for
  flake-debugging.

## Inversion 8 — `atomwrite` as the only file editing tool (v0.8.0+)

- **Default expectation**: `std::fs::write` or `tokio::fs::write` in
  Rust code, `sed -i`/`echo >` in scripts.
- **What we did**: every file modification goes through the
  `atomwrite` CLI tool with `--expect-checksum` (optimistic locking
  via BLAKE3) and atomic write (tempfile + fsync + rename).
- **Why**: a `c24-framework34.html` truncation incident (2026-06-15)
  in the upstream project lost ~127 lines of work. `atomwrite`
  provides 6 layers of defense (L1 telemetry, L2 `--require-backup`,
  L3 `--confirm`, L4 `--preview`, L5 `--auto-rotate`, L6 `risk_assessment`
  in the envelope). See ADR-0035.
- **Trade-off**: every script invocation has a `CS=$(atomwrite read --json ...)` ceremony. Mitigation: aliases in `.cargo/config.toml`
  (`cargo check-all`, `cargo lint`, etc.) reduce the boilerplate.
- **No-go for revert**: silent overwrites are exactly the failure mode
  that caused the 2026-06-15 incident.

## Inversion 9 — No telemetry, no analytics, no OTLP export (all versions)

- **Default expectation**: production CLIs emit usage telemetry
  to vendor-controlled endpoints.
- **What we did**: zero telemetry. `tracing` is used for local logs
  but never exported. `opentelemetry`, `OTLP`, `exporter`, and
  `analytics` patterns are explicitly absent from the codebase.
  CI gate `rg -n 'opentelemetry|OTLP|exporter|tracing::span' src/` returns 0.
- **Why**: privacy-first. The user is the sole owner of their search
  data. Anti-bot detection is harder when the client fingerprint
  doesn't include a telemetry agent signature.
- **Trade-off**: no observability into production usage. Mitigation:
  local `tracing` logs to stderr; `--verbose`/`-vv`/`-vvv` flags
  escalate verbosity; the user can grep their own logs.
- **No-go for revert**: the project README and SKILL.md explicitly
  state "no telemetry". Adding telemetry would require a new major
  version.

## Inversion 10 — Headed-inside-Xvfb instead of headless (v0.8.7, GAP-WS-072 to WS-078; macOS/Windows updated v0.9.3)

- **Default expectation**: browser automation uses plain headless for invisible execution.
- **What we did**: Chrome runs HEADED inside a private Xvfb virtual display on Linux. On macOS/Windows, **v0.9.3 (GAP-WS-112)** switched to **headless=new** (v0.9.1 headed native Quartz/DWM is superseded).
- **Why**: Cloudflare Bot Management 2026 detects classic headless signals; headed-in-Xvfb (Linux) and headless=new (macOS/Windows) produce fingerprints that pass anti-bot better than legacy headless. Xvfb provides an invisible X11 display so the user sees ZERO windows on Linux.
- **Trade-off**: Linux requires Xvfb (the CLI auto-installs it via `try_auto_install_xvfb()` for 22+ distros). macOS/Windows need no extra dependency. The warm-up navigation to duckduckgo.com adds ~800-1500ms latency per search.
- **No-go for revert**: dropping back to detectable legacy headless on Linux would restore Cloudflare detection.

## Inversion 11 — News vertical is Chrome-only and deep-research scans news by default (v0.8.9, GAP-WS-104/105; hardened fail-closed in v0.9.4 / ADR-0016)

- **Default expectation**: HTTP-first CLIs offer an HTTP fallback for every vertical, and new features ship opt-in.
- **What we did**: `--vertical news|all` routes EXCLUSIVELY through the Chrome transport (the news SERP requires JavaScript; there is NO HTTP fallback), and `deep-research` scans news by DEFAULT with the opt-out flag `--no-news`.
- **Chrome policy history**: v0.8.9 failed fast (exit 2) without Chrome and without `--no-news`; v0.9.0 / GAP-WS-106 briefly auto-applied `--no-news` with a stderr warning and proceeded web-only; **v0.9.4 / GAP-WS-113 restores hard fail-closed** — without usable Chrome (or with `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`) every network op including `deep-research` and `--vertical news|all` **exits 2** (no auto `--no-news`, no Web downgrade). See ADR-0016.
- **Why**: the news SERP is 100% JS-rendered (HTTP scraping returns an empty shell), and a deep-research blind to recent events produces stale syntheses — news-by-default guarantees freshness without an extra flag. Soft auto-degradation masked missing Chrome as empty/web-only success; fail-closed makes the dependency explicit.
- **Trade-off**: **hard Chrome dependency** for all production network ops since v0.9.4 (CI and hosts must provide Chrome/Chromium — and Xvfb on headless Linux when required); +2-4s per sub-query for news, overlapped in the fan-out. See `docs/decisions/0010-news-vertical-v0-8-9.md`, `docs/decisions/0011-deep-research-news-dual-v0-8-9.md`, and `docs/decisions/0016-chrome-only-universal-v0-9-4.md`.

## Inversion 12 — One-shot process ownership for Chromium/Xvfb (v0.9.6, GAP-WS-LIFECYCLE-001 / ADR-0017)

- **Default expectation**: browser automation trusts `kill_on_drop` / `Child::kill` on the root process and lets the OS reparent leftovers under `systemd --user` / `init`.
- **What we did**: full ownership of the session process tree in `src/process_lifecycle.rs` — process group (`setpgid`), Linux `PR_SET_PDEATHSIG`, `killpg`, tree walk, `user-data-dir` marker kill, Xvfb lock/socket cleanup, session registry + panic hook; `XvfbGuard` RAII; `ChromeBrowser` cooperative async shutdown with close/wait deadline and `force_reap_session` on `Drop`; `content_fetch` take + async shutdown; SIGTERM and SIGINT cancel the shared `CancellationToken`; `paths::atomic_write` for `--output`, `init-config`, and cookie jar.
- **Why**: chromiumoxide's root-only kill left orphan Chromium grandchildren and Xvfb under long-lived agent hosts (hundreds of browsers / GiB of RAM). A one-shot CLI must be NASCE → EXECUTA → MORRE for every external process it starts.
- **Trade-off**: **SIGKILL** of the CLI itself is not interceptable (OS limit); upgrading does not reap historical orphans from **pre-0.9.6** runs. Operators may need a one-time host cleanup after upgrade.
- **No-go for revert**: dropping back to root-only `kill_on_drop` reintroduces swarm accumulation on multi-day agent sessions.
- **Related**: `docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md` (ADR-0017).

## Inversion 13 — Agent-ready defaults: dual vertical + clean text + multi-canal Chrome (v0.9.8, GAP-WS-AGENT-READY-001 / ADR-0018)

- **Default expectation**: new capabilities ship opt-in; search stays web-only; content fetch is explicit; browser auto-detect only trusts host package-manager binaries; `--chrome-path` after `deep-research` is invalid; fetch never touches news.
- **What we did**: default `--vertical all` (web + news; opt-out `--vertical web` / deep `--no-news`); content fetch **ON** for web + news (FETCH_CAP=10; opt-out `--no-fetch-content`); multi-canal Chrome resolve (Flatpak export shell → deploy ELF `files/extra/chrome`; order `--chrome-path` → `CHROME_PATH` → host Chrome → host Chromium → Flatpak → Snap); transport flags `global = true` (including `--chrome-path` after `deep-research`); honest agent metadata `chrome_path_resolvido` / `chrome_canal` / `usou_chrome` (**not** telemetry); news may carry `conteudo`; no separate `--agent` flag.
- **Why**: AI agents need dual SERP + cleaned body text without inventing flags; Flatpak Chrome is common on Linux and was silently rejected when only the export shell was probed; clap rejected transport flags after the subcommand.
- **Trade-off**: longer default latency and larger JSON envelopes (bounded by FETCH_CAP=10); anti-bot may still zero news (web>0, news empty → exit 0 honest degradation); hosts need a usable Chrome ELF (including Flatpak deploy path). Thin consumers opt out with `--vertical web --no-fetch-content`.
- **No-go for revert**: reintroducing web-only + fetch-off defaults breaks the agent-ready contract documented in skills, schemas, and ADR-0018.
- **Related**: `docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md` (ADR-0018); inventory `docs/gaps.md`. Preserves Inversion 12 (one-shot) and Chrome-only production (0.9.4).

## How to Propose a New Inversion

1. Open an issue with the "Inversion Proposal" label.
2. Document: what default you're inverting, why the default fails in
   this project's context, what the trade-off is, and a no-go
   criterion (when this inversion should NOT be reverted).
3. Add a section to this file following the format of the existing
   inversions.
4. Update the `Cargo.toml` workspace `description` if the inversion
   affects the public contract.
5. Reference the inversion in the relevant ADR.
