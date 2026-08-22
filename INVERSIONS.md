# Architectural Inversions
Read this in [Portuguese](INVERSIONS.pt-BR.md).

`duckduckgo-search-cli` deliberately inverts several common Rust ecosystem
defaults. This document explains each inversion, why it was made, and what
the trade-off is. Read this before proposing a "standard" alternative in
PRs — every inversion here has a recorded rationale that a "more idiomatic"
choice would silently break.

> **Current line: v1.0.6.** Inversions below keep the version where each
> decision landed; none of them was reverted through 1.0.5. Wire serialize
> default is **English** (**ADR-0027**); PT remains deserialize aliases +
> optional `--wire-keys pt`. Historical serialize-PT is **ADR-0023** (1.0.1;
> see Inversion 4).

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
- **What we did**: defined `enum CliError` (21 variants) in
  `src/error/cli_error.rs` via `thiserror`. Every error has a typed
  `error_code()` and `exit_code()`. No `anyhow` in `src/`.
- **Why**: machine-readable exit codes (`0..=6` for product outcomes, plus the
  signal and pipe codes **130**, **141** and **143**) and error codes
  (`http_error`, `rate_limited`, etc.) are part of the public contract.
  `anyhow` would erase these. AI agents and shell scripts branch on
  `error_code` to decide retry vs. fail.
- **Trade-off**: 21 variant match arms on every `?`. New error types
  require updating `exit_code()` and `error_code()`. Mitigation:
  the `#[non_exhaustive]` attribute on `CliError` in
  `src/error/cli_error.rs` allows downstream consumers to be
  forward-compatible.
- **No-go for revert**: removing typed errors would silently break every
  agent that matches on `error_code` for retry logic.

## Inversion 3 — `BTreeMap` for histogram in multi-query output (v0.8.0+)
- **Default expectation**: `HashMap` for aggregation.
- **What we did**: `MultiSearchOutput.zero_cause_histogram: BTreeMap<String, u32>`.
- **Why**: deterministic iteration order across runs is required for
  golden-file snapshot tests and for reproducible JSON output
  (`insta = "1"` snapshot tests). `HashMap` introduces random
  iteration order → flaky snapshot tests.
- **Trade-off**: slightly slower insert (O(log n) vs O(1)). Histogram
  has <100 entries in practice; cost is negligible.
- **No-go for revert**: a non-deterministic JSON output breaks the
  snapshot test contract.

## Inversion 4 — Portuguese Brazilian field names in JSON output (v0.2.0+; ADR-0023 as of v1.0.1; ADR-0027 as of v1.0.2)
- **Default expectation**: Rust ecosystem uses English identifiers.
- **What we did (history)**:
  - **v0.2.0+:** `SearchResult` fields serialized as `posicao`, `titulo`,
    `url`, `url_exibicao`, `snippet`, etc. (not `position`, `title`, `url`).
  - **v1.0.1 / ADR-0023:** English `serde` **deserialize** aliases for
    fixtures/tools; **serialize remained Portuguese** on the wire
    (schemas still documented PT names as primary).
  - **v1.0.2 / ADR-0027:** **serialize default is English**
    (`results`, `title`, `metadata`, `engine`, …). Portuguese remains as
    **deserialize aliases** plus optional **`--wire-keys pt`** (emit-boundary
    EN→PT remap) for legacy agent pipelines.
- **Why (original PT serialize):** README examples and `jaq` recipes in
  `docs/COOKBOOK.md` used Portuguese queries; English fields broke those
  pipelines (bug reported by user in v0.1.0 → fixed in v0.2.0). The PT-BR
  naming was a load-bearing part of the agent's mental model until ADR-0027.
- **Why (EN serialize default in 1.0.2):** agent-native interop and schema
  SSOT preferred English wire keys; PT opt-in via `--wire-keys pt` preserves
  the historical contract without making PT the default.
- **Trade-off**: pipelines that assumed PT serialize must migrate keys or
  pass `--wire-keys pt`. Mapping table: `docs/INTEGRATIONS.md` /
  `docs/MIGRATION.md`.
- **No-go for silent flip-back:** reverting default serialize to PT without
  a migration path would break every agent/skill/schema consumer on the
  v1.0.2 EN wire contract.

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

## Inversion 7 — Local `cargo` aliases instead of a CI pipeline (v0.7.10+)
- **Default expectation**: a hosted CI workflow runs every check on push.
- **What we did**: the project FORBIDS CI. There is no `bin/` directory and no
  workflow. Every gate is a numbered `cargo` alias declared in
  `.cargo/config.toml` and run by the operator on the host: `cargo check-all`
  (gate 1), `cargo lint` (gate 2), `cargo docs` (gate 4), `cargo test-all`
  (gate 5), the cross-target gates `check-windows`, `lint-windows`,
  `check-windows-msvc`, `check-macos`, `check-macos-intel`, `lint-macos` and
  `check-linux` (gate 3 family), the no-C-toolchain profile `check-nohttp`,
  `check-nohttp-all-targets`, `lint-nohttp` and `docs-nohttp` (gates 3b/4b),
  coverage `cov` and `cov-html` (gate 6), and release `publish-check` (gate 9)
  plus `pkg-list` (gate 10).
- **Why**: an alias IS the exact command, so what fails on the operator's
  machine is what would fail anywhere. No hosted runner, no queue, no secret,
  no remote build environment to trust.
- **Trade-off**: nothing forces the gates on a machine that skips them, so the
  discipline is the operator's. Mitigation: `.cargo/config.toml` is the single
  source of truth for the gate list and each alias carries its gate number as
  an inline comment.
- **No-go for revert**: adding a CI pipeline would move the gate off the
  operator's machine and reintroduce exactly the remote dependency this
  project rejects.

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
  in the envelope). No ADR covers this decision — `docs/decisions/` stops at
  ADR-0032.
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
  Local gate: `rg -n 'opentelemetry|OTLP|exporter|tracing::span' src/` returns
  exactly ONE match, `src/logging.rs:9`, which is the comment declaring the
  module is NOT telemetry. Any second match is a regression.
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
- **Chrome policy history**: v0.8.9 failed fast (exit 2) without Chrome and without `--no-news`; v0.9.0 / GAP-WS-106 briefly auto-applied `--no-news` with a stderr warning and proceeded web-only; **v0.9.4 / GAP-WS-113 restores hard fail-closed** — without usable Chrome every network op including `deep-research` and `--vertical news|all` **exits 2** (no auto `--no-news`, no Web downgrade). See ADR-0016.
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
- **What we did**: default `--vertical all` (web + news; opt-out `--vertical web` / deep `--no-news`); content fetch **ON** for web + news (FETCH_CAP=4 (v1.0.2; was 10 at v0.9.8); opt-out `--no-fetch-content`); multi-canal Chrome resolve (Flatpak export shell → deploy ELF `files/extra/chrome`; order `--chrome-path` → `CHROME_PATH` → host Chrome → host Chromium → Flatpak → Snap); transport flags `global = true` (including `--chrome-path` after `deep-research`); honest agent metadata `chrome_path_resolvido` / `chrome_canal` / `usou_chrome` (**not** telemetry); news may carry `conteudo`; no separate `--agent` flag.
- **Why**: AI agents need dual SERP + cleaned body text without inventing flags; Flatpak Chrome is common on Linux and was silently rejected when only the export shell was probed; clap rejected transport flags after the subcommand.
- **Trade-off**: longer default latency and larger JSON envelopes (bounded by FETCH_CAP=4 (v1.0.2; was 10 at v0.9.8)); anti-bot may still zero news (web>0, news empty → exit 0 honest degradation); hosts need a usable Chrome ELF (including Flatpak deploy path). Thin consumers opt out with `--vertical web --no-fetch-content`.
- **No-go for revert**: reintroducing web-only + fetch-off defaults breaks the agent-ready contract documented in skills, schemas, and ADR-0018.
- **Related**: `docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md` (ADR-0018); inventory `gaps.md`. Preserves Inversion 12 (one-shot) and Chrome-only production (0.9.4).

## Inversion 14 — Auditable Chrome profile prefix + disk one-shot (v1.0.0, GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020)
- **Default expectation**: process one-shot is enough; `tempfile::tempdir()` with generic `.tmp` is fine; reaping PIDs leaves the OS/tmp reaper to clean directories; bulk-deleting “all leftover temp dirs” is acceptable host hygiene.
- **What we did**: prefix **`ddg-chrome-`** via `tempfile::Builder` (Unix `0o700`); `force_reap` / `reap_all_registered` **`remove_dir_all` the profile**; `ExitReapGuard` + panic hook + timeout/end-of-run reap; next-run `sweep_orphan_profiles` **only** for owned `ddg-chrome-*` with no live owner; hard refusal to bulk-delete foreign `.tmp*` or `org.chromium.Chromium.*`; deep-research inherits main `CancellationToken`.
- **Why**: process reap (Inversion 12) still left orphan profile trees under generic `.tmp` after cancel/timeout/fan-out; mass-rm of `.tmp*` collides with other Rust apps; Chromium global stubs must not be treated as CLI-owned.
- **Trade-off**: SIGKILL/OOM of the CLI can still leave residual until the **next** invocation sweeps `ddg-chrome-*` only; historical pre-1.0.0 `.tmp*` profiles are **not** auto-mass-deleted (operators clean once if needed).
- **No-go for revert**: returning to generic `.tmp` or bulk-rm of foreign temp prefixes reintroduces unauditable residual and cross-app delete risk.
- **Related**: `docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md` (ADR-0020); extends Inversion 12 (process) with disk honesty; inventory `gaps.md`.

## Inversion 15 — A gate that reads the registry, not the tree (v1.0.6, GAP-REL-001 / ADR-0032)
- **Default expectation**: a green local test suite means users receive working code; `cargo publish --dry-run` is the last gate that matters; `cargo yank` is a safe, independent cleanup step.
- **What we did**: added `src/bin/verify_published.rs` behind `required-features = ["release-gate"]`, which queries crates.io, reads `max_stable_version`, and compares it against the tree; it sends an identifying User-Agent because the API answers **403** without one, and it NEVER reads that 403 as a missing crate.
- **Why**: v1.0.2 shipped an ungated `use` of a `cfg`-gated item and failed `E0432` on macOS and Windows. The tree was fixed and the registry was not, and **no gate in the project could tell those two states apart**. Yanking the fixed 1.0.5 then promoted the broken 1.0.2 back to `max_stable_version`, because that field is **derived** from yank state.
- **Trade-off**: the gate needs network and the optional `reqwest`/`rustls` stack, so it cannot run in the default C-free profile (ADR-0029); it is a maintainer tool and is deliberately absent from every binary a user installs.
- **No-go for revert**: removing it restores the exact blind spot that let a non-compiling version stay installable for two releases; publishing order (publish the sound version **before** yanking the broken ones) stops being enforced by anything.
- **Related**: `docs/decisions/0032-post-publish-verification-gate-v1-0-6.md` (ADR-0032); `gaps.md` GAP-REL-001 and GAP-REL-002.

## Inversion 16 — Disabling a privacy feature is not a privacy mitigation (v1.0.6, GAP-REL-003 / GAP-REL-004)
- **Default expectation**: passing `--disable-features=<Name>` for anything that sounds like tracking hardens the browser; repeating a Chromium switch appends to it.
- **What we did**: merged the two colliding `--disable-features` into one, REMOVED `WebRtcHideLocalIpsWithMdns` from the disable list, and added a launch-argv validator that rejects the same switch name carrying DIFFERENT values.
- **Why**: Chromium's `CommandLine` keeps **ONE value per switch name**, so the second `--disable-features` silently discarded the first and `AutomationControlled` never reached the `FeatureList`. Worse, `WebRtcHideLocalIpsWithMdns` is the feature that **HIDES** local IPs behind `.local` mDNS names in ICE candidates, so disabling it RE-EXPOSED the real local IP — the opposite of the comment above it. The two defects masked each other: fixing only the collision would have ACTIVATED the leak.
- **Trade-off**: the validator only fails on differing values, so a benign identical duplicate still passes; making it stricter blocked real launches when it was first written.
- **No-go for revert**: re-adding the switch reintroduces a local-IP disclosure that no test would catch, because it is a network-observable behavior, not a value the CLI prints.
- **Related**: `src/browser/session/flags.rs`; `gaps.md` GAP-REL-003 and GAP-REL-004.

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
