# Security Policy
Read this in [Portuguese](SECURITY.pt-BR.md).


## Supported Versions
- Only the current release receives routine security fixes
- Version **1.0.5** is the current version (stdout boundary ruler, agent-ops matrix, identity strings never truncated, one refusal envelope; no product env, no remote telemetry)
- Versions **1.0.4** and **1.0.3** receive fixes for **Critical** findings only, until the next minor ships
- Every line below **1.0.3** is unsupported — **1.0.2 and earlier do not compile on macOS or Windows**
- Older lines stay in the table for historical context only; upgrade to **1.0.5**
- Agent metadata fields `chrome_path_resolved` and `chrome_channel` (EN wire default v1.0.2; legacy PT `chrome_path_resolvido` / `chrome_canal` only with `--wire-keys pt`) are a local JSON contract for integrators — **not** remote telemetry
- Content fetch is **ON by default** since v0.9.8 (opt-out `--no-fetch-content`); HTML from fetched pages is still untrusted input parsed locally with scraper/readability
- Pass 52 does **not** invent CVEs; lifecycle and stream-pipe hardening are operational correctness, not security advisories

| Version | Supported |
|---|---|
| 1.0.5 | **yes (current; stdout boundary ruler, agent-ops matrix, identity exempt from truncation, single refusal envelope)** |
| 1.0.4 | critical-only (agent-native flags act or refuse by name; seven envelopes gained a discriminator; EN wire enforced on domain types) |
| 1.0.3 | critical-only (cross-platform hotfix — restores macOS and Windows compilation) |
| 1.0.2 | no (**does not compile on macOS or Windows** — use 1.0.3+; wire EN default ADR-0027, RuntimeConfig SSOT, agent ops, budget contention, mute-audio standard) |
| 1.0.1 | no (historical; Pass 52 SIG_IGN+oneshot, BrokenPipe→141) |
| 1.0.0 | no (historical; GAP-WS-TMP-PROFILE-ORPHAN-001 process+disk one-shot, `ddg-chrome-*` only; ADR-0020) |
| 0.9.10 | no (historical crates.io line; runtime ≈ 0.9.9) |
| 0.9.9 | no (historical; e2e news/timeout/probe/meta; default global timeout 180s; ADR-0019) |
| 0.9.8 | no (historical; GAP-WS-AGENT-READY-001 dual vertical + fetch default ON + Flatpak multi-canal; ADR-0018) |
| 0.9.7 | no (historical; 0.9.6 lifecycle + Windows MSVC HANDLE null check) |
| 0.9.6 | no (historical; lifecycle GAP-WS-LIFECYCLE-001; **does not compile on Windows MSVC**) |
| 0.9.5 | no (historical; GAP-WS-113 + release fix) |
| 0.9.4 | no (historical; GAP-WS-113 Chrome-only fail-closed, no auto-degradation, Lite fallback no-op) |
| 0.9.3 | no (historical; GAP-WS-112 macOS/Windows headless=new) |
| 0.9.2 | no (historical; GAP-WS-108/109/110/111 chromiumoxide stealth hardening) |
| 0.9.1 | no (historical; GAP-WS-107 macOS/Windows headed native) |
| 0.9.0 | no (historical; GAP-WS-106 global flags; auto-degradation **superseded by 0.9.4**) |
| 0.8.9 | no (historical; GAP-WS-104 news vertical Chrome-only, ZeroCause `vertical-sem-resultados`, post-review fixes F1-F7) |
| 0.8.8 | no (historical; `has_native_display()`, Xvfb auto-install 22+ distros, 17 stealth signals, warm-up navigation, GAP-WS-060 through GAP-WS-103 closed) |
| 0.8.0 | no (historical; Chrome-primary transport, zero-cause classification, HTTP decompression) |
| 0.7.10 | no (historical; pre-flight scheduler, identity pin propagation) |
| 0.7.8 | no (historical; 8 anti-bot detector gaps closed) |
| 0.7.7 | no (historical; GAP-WS-49 fixed TLS fingerprint regression) |
| 0.7.3 | no (historical; TLS stack fix — rustls replaced by BoringSSL) |
| < 0.7.3 | no |


## Reporting a Vulnerability
- Report security vulnerabilities via GitHub private advisory: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/security/advisories/new
- Include a clear description of the vulnerability and steps to reproduce
- Include the version affected and the potential impact
- Include any mitigation you already identified
- DO NOT open a public GitHub issue for security vulnerabilities
- Expect an acknowledgment within 72 hours


## Response and Fix SLA
- Acknowledge every report within **72 hours** of receipt
- Confirm or reject the finding within **7 days** of the acknowledgment
- Fix **Critical** severity (CVSS 9.0–10.0) within **7 days** of confirmation
- Fix **High** severity (CVSS 7.0–8.9) within **30 days** of confirmation
- Fix **Medium** severity (CVSS 4.0–6.9) within **90 days** of confirmation
- Fix **Low** severity (CVSS 0.1–3.9) in the next scheduled release
- Communicate a revised timeline to the reporter whenever a deadline above cannot be met


## Disclosure Policy
- Embargo period: 90 days from receipt of the report
- The vulnerability will NOT be disclosed publicly before the embargo ends
- Coordinated fix and disclosure happen at the end of the embargo period
- If a fix cannot ship within 90 days, the timeline is communicated to the reporter


## Scope
- In scope: HTTP request construction flaws that could enable SSRF, header injection, or request smuggling against DuckDuckGo or fetched URLs
- In scope: HTML parsing weaknesses in the extraction pipeline triggered by hostile server responses (DoS via crafted DOM, XXE despite the HTML context, CPU-bomb selectors)
- In scope: Credential leakage through `--proxy user:pass@...` handling in logs, error messages, or output JSON — masking must prevent this, so report any leak
- In scope: Path traversal or symlink attacks against the output file path (`-o, --output`) or the XDG config directory
- In scope: Cookie jar tampering — the v0.7.3+ `cookies.json` file contains session cookies from DuckDuckGo and is written with 0o600 Unix permissions. Report any way to read this file as another local user, or any way the CLI sends those cookies to a non-DuckDuckGo origin.
- In scope: TLS misconfiguration that could enable MITM — residual Rust HTTP uses `reqwest` + **rustls** with sole CryptoProvider **`aws-lc-rs`** (ADR-0021; no `native-tls`/OpenSSL). Production SERP TLS is the Chrome process (ADR-0016). Report any fallback to unsafe cipher suites or reintroduction of `native-tls`
- In scope: Supply chain issues in pinned transitive dependencies not yet documented in `deny.toml`


## Out of Scope
- Denial of service caused by the user passing pathological flags is expected behavior (`--parallel 20 --pages 5 --fetch-content` over thousands of queries is expected to consume significant resources)
- Vulnerabilities in DuckDuckGo itself — report those to DuckDuckGo
- Vulnerabilities in Chrome/Chromium used under `--features chrome` — report those to the Chromium project
- Issues requiring a compromised local user account or write access to `$XDG_CONFIG_HOME`
- Residual orphan Chromium/Xvfb processes from pre-0.9.6 runs, leftover profile dirs from pre-1.0.0 (generic `.tmp*`) binaries, or residual after an external **SIGKILL**/OOM of the CLI itself, are operational host hygiene limits (OS cannot deliver handlers on SIGKILL) — not a CVE unless they enable privilege escalation or cross-user access. Since v1.0.0 the CLI never bulk-deletes foreign `/tmp/.tmp*` or `org.chromium.Chromium.*`; next-run sweep only targets owned `ddg-chrome-*`


## Security Design Assumptions
- This CLI is a read-only HTTP client — it performs no writes to remote systems
- All external inputs (query strings, output paths) are validated before use
- Path traversal attacks are blocked: output paths with `..` components are rejected with exit code 2
- Proxy URLs are masked in logs: credentials are replaced with `[...]` before any output
- **v0.7.3+**: A cookie jar is persisted to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). The file is written with Unix permissions `0o600` (owner read+write only). On Windows, the directory inherits the user's profile ACL. The cookies are session cookies issued by `duckduckgo.com` and `html.duckduckgo.com`. **Treat this file as you would treat any credential.** Use `--no-cookie-persistence` to keep cookies in memory only. Use `--cookies-path <PATH>` to relocate the file to an encrypted volume (e.g., a LUKS-mounted directory or a tmpfs restricted to your UID).
- **v0.7.8+**: Verbose flag surface expanded. `-v` is info, `-vv` is debug, `-vvv` is trace (GAP-WS-53). Operators investigating anomalies can escalate log detail without recompiling. The flag `conflicts_with = "quiet"` prevents contradictory intent. Use this when reporting a suspected vulnerability — `-vvv` output is the most useful diagnostic the maintainers can receive.
- The binary does not execute subprocesses or shell commands based on search results
- **v0.8.6+ / Pass 40 (ADR-0021)**: Residual HTTP TLS is **rustls** + process provider **`aws-lc-rs`** (`tls_bootstrap` in binary `main`). Feature `rustls-tls-webpki-roots-no-provider` (Mozilla CA bundle; no bundled `ring`). Production SERP uses Chrome TLS (ADR-0016). DDG endpoints are `https://` only.
- **v0.7.3+**: The CLI is no longer fully stateless. Cookie jar persistence adds state across invocations. This is a deliberate trade-off to reduce CAPTCHA rate on the DuckDuckGo server. The warm-up request (`GET https://duckduckgo.com/`) is idempotent and does not persist any user-identifying data beyond the cookies themselves.
- Since v0.8.0 the CLI executes JavaScript via Chrome for the search phase — the Chrome process is sandboxed and runs inside a private Xvfb virtual display (v0.8.5+)
- **v0.9.8+**: content fetch is **ON by default** for web + news (FETCH_CAP=4 (v1.0.2; was 10 at v0.9.8)); opt out with `--no-fetch-content`. This increases the HTML parse surface (`scraper` / html5ever on untrusted page bodies) — still expected design; hostile pages remain in scope for parsing DoS reports
- **v0.9.8+ / v1.0.2 agent metadata is NOT telemetry**: default EN wire uses `chrome_path_resolved`, `chrome_channel`, and honest `used_chrome` (legacy PT names `chrome_path_resolvido` / `chrome_canal` / `usou_chrome` only with `--wire-keys pt`); local JSON contract fields only; no remote export
- When `--fetch-content` is active, fetched pages are parsed with `scraper` (which uses `html5ever`) — untrusted HTML is the expected input
- Output files are created with Unix permission `0o644` (owner writes, world reads)
- Nothing is written outside the path the user passed


## Related Supply Chain Automation
- Run **locally** (CI/CD and GitHub Actions are **forbidden** in this repo):
- `cargo audit --deny warnings` against the RustSec advisory database
- `cargo deny check advisories licenses bans sources` with policy in `deny.toml`
- Dependency updates: `cargo update` / `cargo deny check` locally — **no** Dependabot, **no** Actions


## Security Update Policy
- Ship every security fix as a patch release on crates.io — there is no CI pipeline, so releases are cut manually (see [NO_CI.md](NO_CI.md))
- Run the 10 local validation gates before publishing any security release (see [CONTRIBUTING.md](CONTRIBUTING.md))
- Record the fix in `CHANGELOG.md` and `CHANGELOG.pt-BR.md` under the released version
- Yank a broken release from crates.io within **72 hours** when a fix cannot ship in that window
- Never backport a fix silently — the changelog entry names the version that carries it
- Announce nothing before the embargo of the Disclosure Policy above has ended


## Hall of Fame
- Credit every reporter who follows this policy, unless the reporter asks to stay anonymous
- Add the reporter name and the fixed version to this list at disclosure time
- No researcher has reported a confirmed vulnerability yet — this list is empty on purpose


## Best Practices for Users
- Install from crates.io with `cargo install duckduckgo-search-cli --locked` and keep the binary on the current release
- Treat `cookies.json` as a credential: it is mode `0o600` on Unix, and `--no-cookie-persistence` keeps the session in memory only
- Relocate the cookie jar to an encrypted volume with `--cookies-path <PATH>` on shared hosts
- Never pass proxy credentials on a shared shell history — prefer the XDG key `proxy_url` over `--proxy user:pass@...`
- Keep `--output` inside a directory you own; paths containing `..` are rejected with exit code 2
- Wrap agent invocations in an external `timeout` so a hung Chrome process cannot outlive the run
- Read `.metadata.zero_cause` before retrying a zero-result run instead of looping blindly
- Report anything that looks like a leak through the private advisory channel above, never through a public issue


## v0.6.5 Security Improvements
- **MP-26 (HANDLE type-safety)**: `src/platform.rs:51-69` uses `is_null()` and
  `INVALID_HANDLE_VALUE` instead of `handle != 0` and `handle as isize`. The
  Win32 API now receives a properly-typed `HANDLE` (`*mut c_void`) per the
  `windows-sys 0.59+` ABI. Eliminates UB latent in v0.6.4.
- **CI-01 (clippy lints)**: `improper_ctypes` and `improper_ctypes_definitions`
  are now `deny` in `Cargo.toml`, preventing future FFI type drift. Missing
  `Debug` impls and `clippy::needless_return` regressions are now caught
  at `cargo clippy --all-targets --all-features -- -D warnings`.
- **Lints promoted to deny**: `missing_safety_doc` and `unsafe_op_in_unsafe_fn`
  prevent underspecified `unsafe` API surface.

For vulnerabilities in v0.6.4 specifically, the Windows HANDLE cast issue
was the most prominent: a build failure on Windows that could be triggered
by `cargo install duckduckgo-search-cli`. v0.6.5 ships the type-safe fix.


## v0.7.3 Security Improvements
> **Note (v0.8.6)**: The BoringSSL/wreq stack described below was replaced by `reqwest` + `rustls-tls` in v0.8.6 (ADR-0008). This section is historical.

- **GAP-WS-27 (TLS fingerprint)**: The Cloudflare Bot Management CAPTCHA
  interstitial that affected macOS users in v0.7.2 (HTTP 200 with
  `quantidade_resultados: 0`) is fixed. The TLS stack changed from `rustls`
  to BoringSSL (statically linked by `wreq 6.0.0-rc.29`).
- **BoringSSL pinned via `wreq 6.0.0-rc`**: BoringSSL is the same TLS
  library that Chrome and Android use in production. CVEs against
  BoringSSL are tracked by Chromium and addressed in upstream commits
  that `wreq` consumes on each release.
- **Cookie jar hardening (0o600)**: The `cookies.json` file written by
  the v0.7.3+ `session` feature is created with Unix permissions `0o600`
  (owner read+write only). On Windows, the file inherits the user's
  profile directory ACL.
- **Cookie jar location is XDG-aware**: Linux follows `XDG_CONFIG_HOME`
  (defaults to `~/.config`). Windows uses `%APPDATA%`. macOS uses
  `~/Library/Application Support`. The path is overridable via
  `--cookies-path <PATH>` to point at an encrypted volume.
- **Build-time supply chain**: Compiling from source now requires
  `cmake`, `perl`, `pkg-config`, and `libclang-dev` on Linux. These are
  C toolchain components that compile the BoringSSL static library.
  **`cargo install` always compiles from source** — crates.io does not
  distribute pre-built binaries for any platform. Every Windows user must
  satisfy the four BoringSSL build prerequisites (NASM, CMake, MSVC, Perl)
  themselves. See `gaps.md` GAP-WS-28/29/30/31 and `docs/INSTALL-WINDOWS.md`
  for the full prerequisite list and step-by-step setup.
- **MSRV unchanged from v0.7.2**: `rust-version = "1.88"`.

## v0.7.9 Security Improvements
- **GAP-WS-58 (CRITICAL, ghost-block)**: `detectar_interstitial` now classifies a
  sub-4KB body without `result-page-signal` as `InterstitialKind::Cloudflare`. The
  conservative threshold avoids false positives on valid low-density responses.
  Before the fix, a pure ghost-block (empty Cloudflare HTML) went unnoticed and
  the CLI returned exit 0 with `quantidade_resultados: 0`, masking the block.
- **GAP-WS-59 (HIGH, markers 2026)**: 5 new Cloudflare markers
  (`anomaly.js`, `botnet`, `cf-error-code`, `cf-ray`, `Performance & Security by Cloudflare`)
  plus 1 new DDG marker (partial `Unfortunately, bots`). The detector covers the
  2026 variants that were slipping through.
- **GAP-WS-59 (HIGH, global flag)**: `--allow-lite-fallback` and `--pre-flight` were
  hoisted to `RootArgs` with `global = true`. That closed the `unexpected argument`
  path on subcommands such as `deep-research`, which could expose attack surface in scripts.
- **GAP-WS-106 (HIGH, CLI ergonomics; historical v0.9.0–v0.9.3)**: nine flags hoisted to `global = true`. In those releases `deep-research` and `--vertical news|all` auto-degraded with a stderr warning instead of aborting with exit 2 when Chrome was unavailable. **Superseded by GAP-WS-113 / v0.9.4**: production is Chrome-only fail-closed (exit 2) — no auto `--no-news`, no Web downgrade.
- **Config.pre_flight**: added with default `false` (opt-in). No behavioral change
  for existing users.

## v1.0.0 Security Improvements
- **GAP-WS-TMP-PROFILE-ORPHAN-001 (HIGH, Chrome profile disk one-shot, ADR-0020)**: closes the residual where process reaping (0.9.6) left orphan `user-data-dir` trees under generic tempfile prefixes. Profiles use auditable prefix **`ddg-chrome-`** with Unix mode **`0o700`**; `force_reap` / `reap_all_registered` remove the directory after process kill; `ExitReapGuard` + panic hook + timeout/end-of-run reap cover cooperative exits.
- **Selective orphan sweep only**: next-run `sweep_orphan_profiles` removes stale **`ddg-chrome-*`** with no live owner process. **Hard policy (not optional):** never auto-`rm` generic `.tmp*` mass paths; never auto-`rm` `org.chromium.Chromium.*` global stubs — those are foreign or Chromium-owned and out of scope for bulk delete.
- **Ownership guards**: `is_cli_owned_profile_name` / `is_forbidden_bulk_delete_name` / `remove_user_data_dir` refuse foreign prefixes so a bug or hostile path cannot expand cleanup blast radius.
- **deep-research cancel inheritance**: inherits the main `CancellationToken` so SIGTERM cancels fan-out and disk reap can run (closes isolated-token residual).
- **Residual limit (documented, not a vulnerability)**: **SIGKILL**/OOM of the CLI itself is not interceptable; a later invocation may sweep only this CLI’s `ddg-chrome-*`. Historical pre-1.0.0 `.tmp*` profile dirs are **not** bulk-deleted by design.
- **No remote telemetry**: disk lifecycle and sweep emit local `tracing` only.

## v1.0.2 Security Improvements
- **ADR-0027 (wire EN default)**: stdout JSON serialization uses **English** keys (`results`, `title`, `metadata`, `result_count`, `chrome_channel`, `chrome_path_resolved`, `used_chrome`, …). Deserialization still accepts PT aliases. Legacy remap: `--wire-keys pt` or `config set wire_keys pt`.
- **RuntimeConfig SSOT** (`src/runtime/`): precedence CLI > XDG > FACTORY; no product env for runtime knobs.
- **Agent ops** (no jq): `--fields`/`--select`, `--filter`, `--limit`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`.
- **Mute-audio standard (ADR-0026)**: Chrome launches muted by policy (no product unmute).
- **No remote telemetry**: agent metadata and lifecycle remain local-only.

## v0.9.8 Security Improvements
- **GAP-WS-AGENT-READY-001 (HIGH, agent-ready defaults, ADR-0018)**: default dual vertical + content fetch ON increases local HTML parse surface (still expected). Agent metadata (`chrome_path_resolvido`, `chrome_canal`, honest `usou_chrome`) is **not** telemetry and is not exported remotely.
- **Multi-canal Chrome resolve**: Flatpak export shells are not executed as the browser; the CLI resolves a real ELF under `files/extra/chrome` (and similar). Prefer `--chrome-path` when the operator wants an explicit binary.
- **Transport flags `global = true`**: reduces including `--chrome-path` after `deep-research` no longer fail clap parse (exit 2) — reduces accepted before or after the subcommand.
- **No remote telemetry**: one-shot lifecycle, atomwrite, and agent metadata remain local-only.

## v0.9.6 Security Improvements
- **GAP-WS-LIFECYCLE-001 (HIGH, one-shot Chromium/Xvfb ownership, ADR-0017)**: the CLI is NASCE → EXECUTA → MORRE. `src/process_lifecycle.rs` owns the full process tree (process group via `setpgid`, Linux `PR_SET_PDEATHSIG`, `killpg`, tree walk, `user-data-dir` marker kill, Xvfb lock/socket cleanup, session registry + panic hook). `ChromeBrowser` uses `XvfbGuard`, cooperative async shutdown with close/wait deadline, and `force_reap_session` on `Drop`. `content_fetch` takes ownership and runs async shutdown. A normal or cooperatively cancelled invocation must not leave orphan Chromium/Xvfb from **this** run.
- **Atomic writes (`paths::atomic_write`)**: `--output`, `init-config`, and the cookie jar write via tempfile + fsync + rename, reducing partial/corrupt config, cookie, or output files on crash mid-write.
- **SIGTERM + SIGINT cooperative cancel**: both signals cancel the shared `CancellationToken` so shutdown paths run instead of abandoning the browser tree.
- **Residual limit (documented, not a vulnerability)**: **SIGKILL** of the CLI process itself is not interceptable at the OS level; historical orphans from **pre-0.9.6** runs are not cleaned by a later upgrade. Operators may need a one-time host cleanup after upgrading from older versions.
- **No remote telemetry**: lifecycle/reap paths emit local `tracing` only; nothing is exported.

## v0.9.4 Security Improvements
- **GAP-WS-113 (CRITICAL, Chrome-only universal transport, ADR-0016)**: production network path is exclusively chromiumoxide/CDP. Missing Chrome (or a binary built without feature `chrome`) **fails closed with exit 2** on every network operation — no silent HTTP success, no auto Web/`--no-news` degradation. Product env `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` is **removed** / not read. Removes a covert dual-transport channel that could surface empty results as legitimate zeros under anti-bot.
- **`--allow-lite-fallback` legacy no-op**: Lite is never a production success path; the flag remains for script BC only and does not force endpoint degradation.
- **Residual HTTP** only behind compile feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` (tests).

## v0.7.10 Security Improvements
- **GAP-WS-60 (CRITICAL, identity pin propagation)**: `--identity-profile` now
  propagates the identity pin to EVERY output path, including
  `failure_output` (pipeline.rs) and `error_output` (parallel.rs). Before the fix,
  the pin (`identidade_usada`) appeared only on the SUCCESS path; on failure it
  was always `null`. Consumers can now correlate failures with specific
  identities from the pool of 12, for audit and incident response.
  New helper: `identity_tag_for_cli_identity` in `src/identity.rs`.
- **B4 fix (CRITICAL, exit code honesty)**: standalone `--probe-deep` now
  returns exit 3 when it detects a captcha. It previously returned exit 0 with
  `status: "captcha"` in the JSON, allowing a bypass through `if [ $? -eq 0 ]`
  in shell scripts. Branching on the exit code is now reliable.
- **B1 fix (CRITICAL, JSON stream integrity)**: `--pre-flight` emitted two
  concatenated JSON objects on stdout through a `print_line_stdout` early return.
  Consumers piping to `jaq '.resultados'` broke. The early print was removed;
  `SearchOutput` carries the pre-flight context and the caller serializes
  exactly once.
- **B2 fix (CRITICAL, exit code honesty)**: `pre_flight_blocked` now returns
  exit 3 (RATE_LIMITED_OR_BLOCKED) instead of exit 0 (SUCCESS). The
  `EXIT CODES` table in `--help` promised exit 3 for the DuckDuckGo 202 block
  anomaly, but the path fell through to `Ok(output)`, which returned SUCCESS.
- **GAP-AUD-002 (CRITICAL, bench wiring)**: `cargo bench --bench pre_flight_latency`
  now runs Criterion correctly, after adding `[[bench]] harness = false`
  in `Cargo.toml`. Before the fix, the default harness reported `running 0 tests`
  instead of executing the 5 benchmark scenarios, giving a false impression of
  no regression while a real regression was present.
- **Pre-publish (rule 1264, local only)**: manual gates before `cargo publish`
  (`fmt`, `clippy -D warnings`, `test --locked`, `llvm-cov`, dry-run). GitHub
  Actions were removed. Pre-publish is a manual local checklist only (see `NO_CI.md`).
  Yank window: 72h.
- **Identity tag deterministic seeding**: the canonical identity pin uses a
  deterministic per-identity seed (for example `chrome-linux-33333333cccc0003`),
  allowing byte-for-byte reproduction of JSON payloads across runs with the same
  seed. There is no randomness in the pin.
- **MSRV unchanged from v0.7.2**: `rust-version = "1.88"`.


## v0.7.8 Security Improvements
- **RUSTSEC-2025-0057 (fxhash unmaintained) RESOLVED**: The transitive
  dependency `fxhash 0.2.1` (RUSTSEC-2025-0057, marked unmaintained by the
  RustSec advisory database) is gone in v0.7.8. The bump from `scraper
  0.20.0` to `scraper 0.27.0` removed the transitive path through
  `fxhash`. The `cargo audit --deny warnings` gate now runs clean for this
  advisory. `deny.toml` no longer needs the `RUSTSEC-2025-0057` ignore
  exception. Only the `async-std` (RUSTSEC-2025-0052) ignore remains,
  scoped to the optional `chrome` feature.
- **Supply chain gate hardened**: `cargo audit --deny warnings` must pass
  **locally** before publish/PR merge. CI/CD and GitHub Actions are forbidden;
  any new RUSTSEC advisory above `MEDIUM` must fail the local gate.
- **Anti-bot detector rebalance (GAP-WS-52; historical through v0.9.3)**: The
  fallback predicate read the real detector result instead of a fixed
  assumption. When `--allow-lite-fallback` was off but the detector flagged a
  CAPTCHA interstitial, the CLI emitted a structured `tracing::warn!` and
  continued to exit with the appropriate code — it did NOT silently fall back.
  **Since v0.9.4 / GAP-WS-113 the flag is a legacy no-op** (Chrome-only; Lite is
  not a production success path).
- **Verbose level surface (GAP-WS-53)**: `-vv` and `-vvv` flags added
  to `src/cli.rs` via `ArgAction::Count`. Operators can now escalate
  log verbosity without recompiling. The flag `conflicts_with = "quiet"`
  prevents contradictory intent.
- **`Buscar` subcommand hidden (GAP-WS-56)**: The legacy `Buscar`
  subcommand is marked `#[command(hide = true)]`. It remains callable
  for backward compatibility but disappears from `--help`. Reduces
  surface area for confused-deputy attacks against CI scripts that
  parse `--help` output.
- **`--retries` honored end-to-end (GAP-WS-57)**: The retry counter
  in `src/parallel.rs:644` now reads `config.retries` instead of a
  hard-coded constant. The previous behavior silently dropped the
  user-supplied `--retries` value in the `error_output` path.
- **Pinned `wreq 6.0.0-rc.29` (GAP-WS-55)**: The `wreq` block in
  `Cargo.toml` was rewritten. The previous release claimed
  `wreq 5.3.0` but the actual pin in use is `6.0.0-rc.29` with three
  direct pins (`wreq-util`, `brotli-decompressor =5.0.1`,
  `alloc-no-stdlib =2.0.4`). **(Historical: wreq and all its pins were removed in v0.8.6 — ADR-0008.)**
- **MSRV unchanged from v0.7.7**: `rust-version = "1.88"`.

For vulnerabilities introduced or surfaced by v0.7.7 specifically, the
TLS fingerprint regression (GAP-WS-49) was the most prominent: a
`wreq-util` resolution failure that broke BoringSSL emulation on certain
Linux distributions. v0.7.7 ships the pinned-`wreq-util` fix and
restored normal operation.


## Chrome automation-signal mitigation (v0.8.5+ / ADR-0022)
- Production SERP uses **native Chrome TLS** (ADR-0016). Goal: avoid **library** TLS bot-class signatures (`rustls` JA4) that Cloudflare Bot Management blocks (GAP-WS-27) — **not** a marketed “fingerprint feature”
- **Forbidden (ADR-0022):** synthetic hardware fingerprint spoofing (canvas noise, WebGL GPU lies, AudioContext noise, forced `hardwareConcurrency` / `deviceMemory` / `colorDepth` / fixed `languages` / fixed `connection`). Static spoofs become a shared automation signature
- **Allowed:** CDP automation-signal mitigation only — `navigator.webdriver` → `undefined`, realistic `plugins` / `mimeTypes`, `window.chrome` stubs, outer window size, Permissions quirks, DevTools WebSocket leak block (GAP-WS-076)
- Purpose: legitimate search against DuckDuckGo without presenting a bot-class client profile
- Chrome may use `--no-sandbox` on Linux when required (root/containers)
- Cookie jar permissions remain `0o600` (owner read/write only)
- No user data is collected or transmitted by stealth scripts; no product telemetry
