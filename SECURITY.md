# Security Policy


## Supported Versions
- Only the latest minor and the previous minor receive security updates
- Version **1.0.0** is the current version (GAP-WS-TMP-PROFILE-ORPHAN-001 disk one-shot + `ddg-chrome-*` profiles; includes 0.9.8 agent-ready defaults, 0.9.9 e2e honesty, 0.9.6 process lifecycle, 0.9.7 Windows MSVC HANDLE fix)
- Older 0.9.x / 0.8.x lines are listed for historical context; prefer upgrading to **1.0.0+**
- Agent metadata fields `chrome_path_resolvido` and `chrome_canal` are a local JSON contract for integrators — **not** remote telemetry
- Content fetch is **ON by default** since v0.9.8 (opt-out `--no-fetch-content`); HTML from fetched pages is still untrusted input parsed locally with scraper/readability

| Version | Supported |
|---|---|
| 1.0.0 | **yes (current; GAP-WS-TMP-PROFILE-ORPHAN-001 process+disk one-shot, `ddg-chrome-*` only; ADR-0020)** |
| 0.9.10 | yes (previous crates.io line; runtime ≈ 0.9.9 — upgrade to 1.0.0 for disk hygiene) |
| 0.9.9 | yes (e2e news/timeout/probe/meta; default global timeout 180s; ADR-0019) |
| 0.9.8 | yes (GAP-WS-AGENT-READY-001 dual vertical + fetch default ON + Flatpak multi-canal; ADR-0018) |
| 0.9.7 | yes (0.9.6 lifecycle + Windows MSVC HANDLE null check) |
| 0.9.6 | yes (lifecycle GAP-WS-LIFECYCLE-001; **does not compile on Windows MSVC** — use 0.9.7+) |
| 0.9.5 | yes (previous; GAP-WS-113 + CI/release fix) |
| 0.9.4 | yes (GAP-WS-113 Chrome-only fail-closed, no auto-degradation, Lite fallback no-op) |
| 0.9.3 | yes (previous; GAP-WS-112 macOS/Windows headless=new) |
| 0.9.2 | yes (GAP-WS-108/109/110/111 chromiumoxide stealth hardening) |
| 0.9.1 | yes (GAP-WS-107 macOS/Windows headed native) |
| 0.9.0 | yes (GAP-WS-106 global flags; auto-degradation **superseded by 0.9.4**) |
| 0.8.9 | yes (GAP-WS-104 news vertical Chrome-only, ZeroCause `vertical-sem-resultados`, post-review fixes F1-F7) |
| 0.8.8 | yes (`has_native_display()`, Xvfb auto-install 22+ distros, 17 stealth signals, warm-up navigation, GAP-WS-060 through GAP-WS-103 closed) |
| 0.8.0 | yes (Chrome-primary transport, zero-cause classification, HTTP decompression) |
| 0.7.10 | yes (pre-flight scheduler, identity pin propagation) |
| 0.7.8 | yes (8 anti-bot detector gaps closed) |
| 0.7.7 | yes (GAP-WS-49 fixed TLS fingerprint regression) |
| 0.7.3 | partial (TLS stack fix — rustls replaced by BoringSSL) |
| < 0.7.3 | no |


## Reporting a Vulnerability
- Report security vulnerabilities via GitHub private advisory: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/security/advisories/new
- Include a clear description of the vulnerability and steps to reproduce
- Include the version affected and the potential impact
- DO NOT open a public GitHub issue for security vulnerabilities
- Expect an acknowledgment within 72 hours


## Disclosure Policy
- Período de embargo: 90 dias a partir do recebimento do relatório
- A vulnerabilidade NÃO será divulgada publicamente antes do término do período de embargo
- Correção e divulgação coordenada ocorrem ao final do período de embargo
- Se uma correção não puder ser entregue em 90 dias, a timeline será comunicada ao reporter


## Scope
- In scope: HTTP request construction flaws that could enable SSRF, header injection, or request smuggling
- In scope: HTML parsing weaknesses in the extraction pipeline triggered by hostile server responses
- In scope: Credential leakage through `--proxy user:pass@...` handling in logs, error messages, or output JSON
- In scope: Path traversal or symlink attacks against the output file path (`-o, --output`) or the XDG config directory
- In scope: Cookie jar tampering — the v0.7.3+ `cookies.json` file contains session cookies from DuckDuckGo and is written with 0o600 Unix permissions. Report any way to read this file as another local user, or any way the CLI sends those cookies to a non-DuckDuckGo origin.
- In scope: TLS misconfiguration that could enable MITM — since v0.8.6 the project uses `reqwest` + `rustls-tls` (pure Rust TLS, replacing BoringSSL/wreq from v0.7.3-v0.8.5). Report any fallback to unsafe cipher suites
- In scope: Supply chain issues in pinned transitive dependencies not yet documented in `deny.toml`


## Out of Scope
- Denial of service caused by the user passing pathological flags is expected behavior
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
- **v0.8.6+**: TLS is enforced via `rustls` (pure Rust, statically linked by `reqwest`). No plain HTTP connections to the search endpoint. v0.7.3-v0.8.5 used BoringSSL via `wreq`; v0.8.6 replaced it with `reqwest` + `rustls-tls` (ADR-0008). Cipher suite selection follows the `rustls` defaults.
- **v0.7.3+**: The CLI is no longer fully stateless. Cookie jar persistence adds state across invocations. This is a deliberate trade-off to reduce CAPTCHA rate on the DuckDuckGo server. The warm-up request (`GET https://duckduckgo.com/`) is idempotent and does not persist any user-identifying data beyond the cookies themselves.
- Since v0.8.0 the CLI executes JavaScript via Chrome for the search phase — the Chrome process is sandboxed and runs inside a private Xvfb virtual display (v0.8.5+)
- **v0.9.8+**: content fetch is **ON by default** for web + news (FETCH_CAP=10); opt out with `--no-fetch-content`. This increases the HTML parse surface (`scraper` / html5ever on untrusted page bodies) — still expected design; hostile pages remain in scope for parsing DoS reports
- **v0.9.8+ agent metadata is NOT telemetry**: `chrome_path_resolvido`, `chrome_canal`, and honest `usou_chrome` are local JSON contract fields only; no remote export


## Related Supply Chain Automation
- `cargo audit` runs against the RustSec advisory database on every push and pull request
- `cargo deny check advisories licenses bans sources` runs with policy declared in `deny.toml`
- Dependabot (weekly) opens pull requests for `cargo` and `github-actions` dependency updates
- Dependency updates: run `cargo update` / `cargo deny check` locally (no Dependabot/Actions on this repo)


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

- **GAP-WS-58 (CRITICAL, ghost-block)**: `detectar_interstitial` agora classifica
  body sub-4KB sem `result-page-signal` como `InterstitialKind::Cloudflare`. Threshold
  conservador evita falsos positivos em responses válidos de baixa densidade.
  Antes da fix, ghost-block puro (HTML vazio do Cloudflare) passava despercebido
  e a CLI retornava exit 0 com `quantidade_resultados: 0`, mascarando o bloqueio.
- **GAP-WS-59 (HIGH, markers 2026)**: 5 marcadores Cloudflare novos
  (`anomaly.js`, `botnet`, `cf-error-code`, `cf-ray`, `Performance & Security by Cloudflare`)
  + 1 marker DDG novo (`Unfortunately, bots` parcial). Detector cobre variantes
  2026 que passavam despercebidas.
- **GAP-WS-59 (HIGH, global flag)**: `--allow-lite-fallback` e `--pre-flight` hoisted
  para `RootArgs` com `global = true`. Fechou o caminho `unexpected argument` em
  subcomandos como `deep-research` que poderia expor attack surface em CI scripts.
- **GAP-WS-106 (HIGH, CLI ergonomics; historical v0.9.0–v0.9.3)**: nine flags hoisted to `global = true`. In those releases `deep-research` and `--vertical news|all` auto-degraded with a stderr warning instead of aborting with exit 2 when Chrome was unavailable. **Superseded by GAP-WS-113 / v0.9.4**: production is Chrome-only fail-closed (exit 2) — no auto `--no-news`, no Web downgrade.
- **Config.pre_flight**: adicionado com default `false` (opt-in). Sem mudança
  comportamental para usuários existentes.

## v1.0.0 Security Improvements

- **GAP-WS-TMP-PROFILE-ORPHAN-001 (HIGH, Chrome profile disk one-shot, ADR-0020)**: closes the residual where process reaping (0.9.6) left orphan `user-data-dir` trees under generic tempfile prefixes. Profiles use auditable prefix **`ddg-chrome-`** with Unix mode **`0o700`**; `force_reap` / `reap_all_registered` remove the directory after process kill; `ExitReapGuard` + panic hook + timeout/end-of-run reap cover cooperative exits.
- **Selective orphan sweep only**: next-run `sweep_orphan_profiles` removes stale **`ddg-chrome-*`** with no live owner process. **Hard policy (not optional):** never auto-`rm` generic `.tmp*` mass paths; never auto-`rm` `org.chromium.Chromium.*` global stubs — those are foreign or Chromium-owned and out of scope for bulk delete.
- **Ownership guards**: `is_cli_owned_profile_name` / `is_forbidden_bulk_delete_name` / `remove_user_data_dir` refuse foreign prefixes so a bug or hostile path cannot expand cleanup blast radius.
- **deep-research cancel inheritance**: inherits the main `CancellationToken` so SIGTERM cancels fan-out and disk reap can run (closes isolated-token residual).
- **Residual limit (documented, not a vulnerability)**: **SIGKILL**/OOM of the CLI itself is not interceptable; a later invocation may sweep only this CLI’s `ddg-chrome-*`. Historical pre-1.0.0 `.tmp*` profile dirs are **not** bulk-deleted by design.
- **No remote telemetry**: disk lifecycle and sweep emit local `tracing` only.

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

- **GAP-WS-113 (CRITICAL, Chrome-only universal transport, ADR-0016)**: production network path is exclusively chromiumoxide/CDP. Missing Chrome or `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` **fails closed with exit 2** on every network operation — no silent HTTP success, no auto Web/`--no-news` degradation. Removes a covert dual-transport channel that could surface empty results as legitimate zeros under anti-bot.
- **`--allow-lite-fallback` legacy no-op**: Lite is never a production success path; the flag remains for script BC only and does not force endpoint degradation.
- **Residual HTTP** only behind compile feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` (tests).

## v0.7.10 Security Improvements

- **GAP-WS-60 (CRITICAL, identity pin propagation)**: `--identity-profile` agora
  propaga o pino de identidade para TODOS os caminhos de output, incluindo
  `failure_output` (pipeline.rs) e `error_output` (parallel.rs). Antes da fix,
  o pino (`identidade_usada`) só aparecia no caminho de SUCESSO; em falha,
  era sempre `null`. Consumers agora podem correlacionar falhas a identidades
  específicas do pool de 12 para fins de auditoria e incident response.
  Helper novo: `identity_tag_for_cli_identity` em `src/identity.rs`.
- **B4 fix (CRITICAL, exit code honesty)**: `--probe-deep` standalone agora
  retorna exit 3 quando detecta captcha. Antes retornava exit 0 com
  `status: "captcha"` no JSON, permitindo bypass via `if [ $? -eq 0 ]`
  em shell scripts. Agora branching no exit code é confiável.
- **B1 fix (CRITICAL, JSON stream integrity)**: `--pre-flight` emitia dois
  objetos JSON concatenados no stdout via `print_line_stdout` early-return.
  Consumers com `| jaq '.resultados'` quebravam. Removido early print;
  `SearchOutput` carrega o contexto do pre-flight e o caller serializa
  exatamente uma vez.
- **B2 fix (CRITICAL, exit code honesty)**: `pre_flight_blocked` agora retorna
  exit 3 (RATE_LIMITED_OR_BLOCKED) em vez de exit 0 (SUCCESS). Tabela
  `EXIT CODES` do `--help` prometia exit 3 para "DuckDuckGo 202 block anomaly"
  mas o caminho caía no `Ok(output)` que retornava SUCCESS.
- **GAP-AUD-002 (CRITICAL, bench wiring)**: `cargo bench --bench pre_f_light_latency`
  agora roda Criterion corretamente após adicionar `[[bench]] harness = false`
  em `Cargo.toml`. Antes da fix, o harness default reportava `running 0 tests`
  em vez de executar os 5 cenários de benchmark, dando falsa impressão de
  "sem regressão" quando havia regressão real.
- **Pre-publish gate (regra 1264)**: `scripts/pre-publish-gate.sh` adiciona
  7 gates sequenciais antes de `cargo publish` real: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test --all-features --locked`,
  `cargo llvm-cov --fail-under-lines 80`, `rg -n v0.7.9 skill/` (sem version drift),
  `cargo publish --dry-run --allow-dirty --no-verify`, e `gh run list --branch main`
  (CI verde). Bloqueia publicação se qualquer gate falhar. Janela de yank: 72h.
- **Identity tag deterministic seeding**: o pino de identidade canônico
  usa seed determinístico por identidade (ex.: `chrome-linux-33333333cccc0003`),
  permitindo reprodução byte-a-byte de payloads JSON entre runs com a mesma
  seed. Sem randomness no pino.
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
- **Supply chain gate hardened**: `cargo audit --deny warnings` is now a
  blocking gate in `.github/workflows/ci.yml` and
  `.github/workflows/release.yml`. Any new RUSTSEC advisory above
  `MEDIUM` severity will fail the PR build. The previous
  `cargo audit` invocation only warned.
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


## Chrome Stealth Signals (v0.8.5)
- Chrome headed mode (inside private Xvfb virtual display since v0.8.5) injects 17 JavaScript stealth signals via CDP
- `navigator.webdriver` is set to `undefined` to avoid bot detection (real Chrome has `undefined`, not `false`)
- Canvas fingerprint spoofing prevents browser identification
- WebGL fingerprint spoofing via renderer and vendor overrides
- AudioContext fingerprint spoofing with noise injection
- `navigator.plugins` array populated with realistic entries
- `navigator.languages` set to match identity pool language
- `chrome` runtime object spoofed to appear as real Chrome
- `navigator.connection` set to realistic network type
- `navigator.maxTouchPoints` set to realistic touch values
- These signals are NOT used for malicious purposes
- Purpose: bypass Cloudflare anti-bot detection for legitimate search
- Chrome runs with `--no-sandbox` flag on Linux for compatibility
- `--no-sandbox` is required when running as root or in containers
- Cookie jar permissions remain `0o600` (owner read/write only)
- No user data is collected or transmitted by stealth scripts
