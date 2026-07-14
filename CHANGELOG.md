## [Unreleased]

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
- **L-08 Docs** — ADR-0018, schemas, versioned `docs/gaps.md`, skills EN/PT, MIGRATION PT, this CHANGELOG.
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

### Fixed (CI / release unblock after GAP-WS-113)

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

### Fixed (GAP-WS-112 — janela Chrome visível no macOS/Windows)
- macOS (Quartz) e Windows (DWM) agora usam `headless=new` padrão (sem janela visível)
- Causa raiz: compositores nativos clampam `--window-position` aos bounds da tela
- headed nativo abria janela Chrome visível a cada busca, prejudicando o fluxo do usuário
- headless=new moderno combinado com fixes v0.9.2 passa no DDG sem abrir janela
- validação empírica: 3/3 queries exit 0, `usou_chrome=true`, `causa=null`, sem janela visível
- Detecção automática de SO em `decide_head_mode` via `cfg!(target_os = ...)`

### Changed (GAP-WS-112 — modo de operação distinto por plataforma)
- Linux mantém Xvfb privado (`HeadedXvfb`) sem mudança — modo OBRIGATORIAMENTE distinto
- macOS/Windows agora usam `Headless` (headless=new) por padrão
- `DUCKDUCKGO_CHROME_VISIBLE=1` continua forçando `HeadedNative` para depuração

### Fixed (qualidade)
- Corrigido warning clippy `needless_return` em `has_native_display` (macOS/Windows)
- Testes cfg-gated atualizados para afirmar `Headless` no macOS/Windows

## [0.9.2] - 2026-07-08

### Changed (GAP-WS-108 — launch sem defaults automáticos do chromiumoxide)
- `launch()` agora chama `.disable_default_args()` e re-adiciona 23 defaults seguros via `CHROMIUMOXIDE_SAFE_DEFAULTS`
- Remove `--enable-automation` injetado automaticamente pelo chromiumoxide 0.9.1 em DEFAULT_ARGS (config.rs:481)

### Fixed (GAP-WS-108 — banner de automação removido)
- Banner "gerenciado por testes automatizados" e marcadores de automação eliminados
- Causa raiz do vazamento de automação que mantinha o bloqueio anti-bot persistente

### Fixed (GAP-WS-109 — UA coerente com Client Hints)
- Versão do UA Chrome alinhada à versão real instalada via `detect_chrome_major_version()`
- `Emulation.setUserAgentOverride` aplica `UserAgentMetadata` coerente (brands, platform, mobile)
- Elimina mismatch `navigator.userAgent` vs `userAgentData.brands`/`sec-ch-ua` (Chrome 146 vs 149)

### Fixed (GAP-WS-110 — WebRTC não vaza IP real)
- `--force-webrtc-ip-handling-policy=disable_non_proxied_udp` e `--disable-webrtc-hw-decoding` em flags_stealth
- Previne leak de IP real via ICE candidate gathering do WebRTC

### Fixed (GAP-WS-111 — QUIC desabilitado)
- `--disable-quic` em flags_stealth força HTTP/2 sobre TCP
- Evita UDP fora do proxy, mantendo consistência de transporte

### Added
- `CHROMIUMOXIDE_SAFE_DEFAULTS` e `detect_chrome_major_version()` em `src/browser.rs`
- `rewrite_ua_chrome_version()` em `src/identity.rs`
- Testes cfg-gated para os novos helpers

### Validation
- `cargo build --features chrome` e `cargo clippy --all-targets --features chrome` — ZERO warnings
- `cargo test --features chrome` — passa sem falhas
- Smoke macOS: 3+ queries com exit 0, `usou_chrome=true`, SEM banner de automação

### Note
- Auditoria baseada nas Rules Rust para Chromiumoxide (fornecida pelo usuário)
- v0.9.1 (headed nativo) era necessário mas insuficiente: a causa raiz do bloqueio era vazamento de automação

## [0.9.1] - 2026-07-08

### Changed (GAP-WS-107 — decisão de modo de cabeça extraída para função pura)
- Decisão de modo de cabeça do Chrome extraída para função pura `decide_head_mode()` em `src/browser.rs`, cfg-gated por `target_os`
- A enum `ChromeHeadMode` (Headless/HeadedXvfb/HeadedNative) formaliza as três modalidades de launch

### Fixed (GAP-WS-107 — macOS/Windows rodam Chrome headed nativo)
- macOS e Windows agora rodam Chrome HEADED no display nativo Quartz/DWM em vez de headless
- Elimina o bloqueio `exit 6 anti-bot` do Cloudflare observado em v0.9.0 no macOS
- Linux mantém Xvfb privado sem regressão; `has_native_display()` + `spawn_virtual_display()` só atuam em Linux
- Janela Chrome movida off-screen via `--window-position=-32000,-32000 --window-size=1920,1080` (flags já existentes)

### Fixed (GAP-WS-107b — coerção de plataforma UA Chrome)
- Novo `identity::ua_platform_matches_host()` força UA Chrome coerente com o SO do host
- O filtro em `src/pipeline.rs` agora força `chrome_only_ua_for_platform()` quando o UA Chrome não bate com o host
- Corrige pinagens cross-plataforma (ex.: `chrome-linux` em host macOS) que passavam sem correção

### Added (GAP-WS-107 — testes cfg-gated)
- Testes cfg-gated para `decide_head_mode` em `src/browser.rs` cobrindo Linux, macOS e Windows
- Testes para `ua_platform_matches_host` em `src/identity.rs` cobrindo coerção de plataforma UA

### Validation
- `cargo build --features chrome` — ZERO warnings
- `cargo test --features chrome` — passa sem falhas
- `cargo clippy --all-targets --features chrome` — ZERO warnings
- `cargo fmt --check` — ZERO diferenças
- Smoke macOS: `duckduckgo-search-cli "rust language" -n 5` retorna `usou_chrome=true`, UA `Macintosh`, `quantidade_resultados>0`, exit 0

### Note
- A skill embedded em `CLAUDE.md` permanece desatualizada (regra do projeto proíbe editar `CLAUDE.md`)
- A skill externa em `skill/` foi atualizada para refletir headed nativo em macOS/Windows

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
- **CR1 — `total == 0` mapeava direto para exit 5 sem inspeção causal**. `src/lib.rs:241-243` agora distingue 5 causas semanticamente diferentes (`Legitimo`, `FiltroSilencioso`, `GhostBlock`, `AntiBot`, `RespostaInvalida`) e emite exit 6 (`SUSPECTED_BLOCK`) quando a causa é não-legítima. Exit 5 preservado para zero genuíno.
- **CR2 — `pre_flight_blocked` agora roda em adição ao classificador causal**. O ramo legacy continua emitindo exit 3, mas o novo classificador captura casos onde pre-flight estava desligado (default).
- **CR3 — `--pre-flight` continua opt-in para preservar BC**. Mas o classificador roda automaticamente quando `quantidade_resultados == 0`, então o operador padrão agora se beneficia sem precisar aprender sobre a flag.
- **CR5 — `SearchMetadata.pre_flight_fired` permanece `bool` por BC**. Mas agora coexiste com `causa_zero` que captura nuances causais.
- **CR6 — `causa_zero` adicionado ao envelope JSON**. Campo `metadados.causa_zero: Option<ZeroCause>` serializa como kebab-case (`"anti-bot"`, `"ghost-block"`, etc.).

### Fixed (Bug #1 — HTTP response decompression)
- **`wreq 6.0.0-rc` envia `accept-encoding: gzip, deflate, br` mas não descomprime automaticamente**. O body retornado por `Response::text()` / `Response::bytes()` chegava como bytes gzip-comprimidos (≈9.2 KB binários em vez de ≈14 KB de texto plano — taxa 65% consistente com gzip level-6 default). `detectar_interstitial_com_match` realizava `body.contains("anomaly-modal")` em bytes binários e falhava silenciosamente, fazendo o classificador rotular `Legitimo` em ambiente comprovadamente bloqueado pelo Cloudflare.
- **Novo módulo `src/decompress.rs`** inspeciona `Content-Encoding` e despacha para `flate2::read::MultiGzDecoder` (gzip), `flate2::read::ZlibDecoder` (deflate) ou `brotli_decompressor::Decompressor` (br). `MultiGzDecoder` lida com streams gzip concatenados transparentemente.
- **7 call sites substituídos** (`src/search.rs:403`, `src/search.rs:776`, `src/lib.rs:637`, `src/pipeline.rs:311`, `src/content.rs:180`): todas as chamadas de `response.text().await` agora passam pelo decompressor antes de virar `String`.
- **`tokio::task::spawn_blocking`** envolve o decode sync para evitar bloquear o reactor do tokio em payloads grandes.
- **`DECOMPRESSION_MAX_OUTPUT = 32 MiB`** como cap de segurança contra gzip bombs via `Read::take(cap + 1)` que aborta a descompressão quando o stream excede.
- **3 variantes em `CliError`**: `PayloadTooLarge { max, actual }`, `UnsupportedEncoding(String)`, `InvalidUtf8(FromUtf8Error)`. Saída JSON mantém `error: "http_error"` para BC.
- **`flate2 = "1"` adicionado a `Cargo.toml`** — já estava transitivo via `wreq` features `gzip`+`deflate`, declarado explícito para visibilidade estável.

### Fixed (Bug #2 — BC opt-out semver drift documentado)
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` afeta SOMENTE o exit code (mapeia 6 → 5 legacy), mas o campo `causa_zero` permanece publicado no envelope JSON. A política `#[serde(skip_serializing_if = "Option::is_none")]` garante que clientes v0.7.x que NUNCA rodam classificador não veem mudança alguma. Clientes que rodam contra ambiente bloqueado com opt-out ativo recebem `causa_zero` mesmo pedindo exit 5 legacy — isso é informação diagnóstica aditiva, alinhada com o padrão de mudanças additive-skip_serializing_if usado em v0.6.4 (`identidade_usada`) e v0.7.9 (`pre_flight_fired`). Documentado na seção Migration Guide abaixo.

### Fixed (GAP-NEW-001 — timeout-cli Rust wrapper sombreia GNU)
- README EN + PT-BR atualizados com secao `Troubleshooting` mencionando `/usr/bin/timeout` GNU coreutils como workaround para o bug do wrapper Rust `timeout-cli` v0.1.0 que re-parseia flags `-v` antes do clap.
- Script `scripts/detect-timeout-wrapper.sh` criado e executavel (modo 755). Detecta automaticamente qual `timeout` esta no PATH (GNU vs Rust wrapper).
- Deteccao em runtime em `src/lib.rs:initialize_logging` via env var `CARGO_BIN_EXE_timeout`. Emite `tracing::warn!` com workaround.
- Teste de regressao em `tests/integration_troubleshooting_documentation.rs` valida documentacao e existencia do script.

### Fixed (GAP-NEW-002 — tracing::debug!() removido em release)
- Migracao em massa de 46 chamadas `tracing::debug!` para `tracing::info!` em 11 arquivos de producao.
- `#[tracing::instrument(level = "debug")]` em `classify_zero_result` migrado para `level = "info"`.
- Campos `bytes_brutos`/`bytes_descomprimidos: Option<u64>` adicionados em `SearchMetadata`.
- Campos `bytes_in`/`bytes_out: u64` adicionados em `AggregatedSearchResult` e wire-in em `pipeline.rs` + `parallel.rs`.
- Telemetria de descompressao HTTP agora visivel em builds release.

### Fixed (GAP-NEW-003 — classificador rotula stealth shell como Legitimo)
- Nova branch CR4b em `classify_zero_result` detecta stealth shell de 14KB+ sem `result__a` markers, sem interstitial markers, mas com assinatura DDG.
- Campo `cascata_nivel_observado: Option<u32>` em `SearchMetadata` propaga nivel de cascata do probe-deep.
- Campo `last_probe_cascade_level: Option<u32>` em `Config` cacheia ultimo probe process-local.
- 4 testes em `tests/integration_stealth_block_classification.rs` validam deteccao e nao-regressao.
- Classificador agora retorna `GhostBlock` em vez de `Legitimo` para ambiente bloqueado stealth.

### Fixed (GAP-NEW-004 — auto-fallback lite para Brasil x Marrocos)
- Auto-fallback lite em `src/pipeline.rs:464-505` re-executa busca com `endpoint=Lite` quando classificador retorna causa nao-legitima.
- Re-execucao recursiva via `Box::pin` para evitar `infinitely sized future`.
- Mesclagem de resultados preserva `causa_zero` original e marca `used_fallback_endpoint=true`.
- 5 testes em `tests/integration_e2e_real_world.rs` reproduzem o caso Brasil 1x1 Marrocos.

### Added
- **`ZeroCause` enum em `src/types.rs`** com 5 variantes marcadas `#[non_exhaustive]` para forward compat. Serializa como kebab-case.
- **`pipeline::classify_zero_result`** — classificador puro, sem I/O, com chain causal documentada em `docs/decisions/0004-zero-cause-classification-v0-8-0.md`.
- **`docs/decisions/0006-stealth-shell-classification-v0-8-0.md`** (GAP-NEW-003 / GAP-NEW-008) — ADR documentando a decisão arquitetural do branch CR4b (4 condições simultâneas: body_len >= 4000 + !result__a + InterstitialKind::None + assinatura DDG). Inclui alternatives considered (threshold dinâmico, ML classifier, marker probing) e validation (proptest com 64 cases).
- **`pipeline::sugestao_proxima_acao_para_zero`** — strings PT-BR determinísticas por variante, alinhadas ao padrão `sugestao_mitigacao_com_marker`.
- **`SearchMetadata.zero_cause: Option<ZeroCause>`** + **`SearchMetadata.sugestao_proxima_acao: Option<String>`** no envelope JSON.
- **`MultiSearchOutput.causa_zero_histogram: BTreeMap<String, u32>`** agregado automaticamente em multi-query; BTreeMap garante ordem lexicográfica determinística.
- **`AggregatedSearchResult.first_body: String`** exposto para o classificador distinguir ghost-block de zero genuíno.
- **`DUCKDUCKGO_ZERO_CAUSE_STRICT` env var** para BC opt-out (default ON; aceita `false`/`0`/`no`/`off`).
- **`exit_codes::SUSPECTED_BLOCK: i32 = 6`** adicionado à tabela de exit codes.
- **`docs/decisions/0004-zero-cause-classification-v0-8-0.md`** — ADR documentando a decisão arquitetural e a chain causal patch→efeito.
- **12 unit tests em `src/pipeline.rs`** cobrindo todas as 5 variantes do enum + mensagens de sugestão.
- **`assert_eq!(SUSPECTED_BLOCK, 6)` em `src/error.rs`** — teste stale atualizado.

### Changed
- `Cargo.toml` bumped 0.7.10 → 0.8.0
- `Cargo.lock` regenerated
- **`src/ddg_class_watch.rs` movido para `examples/ddg_class_watch.rs`** (GAP-OPS-002). Módulo era declarado em `lib.rs:56` mas sem call sites em produção ou testes; agora vive como exemplo invocável via `cargo run --example ddg_class_watch`. `fn main()` adicionada com HTML de demonstração que imprime relatório e sai com código 1 quando detecta classes novas (sinal de alerta para bump de `RESULT_PAGE_SELECTORS` em `src/probe_deep.rs`). Referência histórica em CHANGELOG [0.7.10] P19 preservada — módulo ESTAVA em `src/` em v0.7.10.

### Migration Guide v0.7.x → v0.8.0

**Exit code 6 é aditivo, não substitui exit 5.** Clientes que ramificam em `exit 5` podem continuar funcionando sem mudanças via BC opt-out:

```bash
# Restaura comportamento v0.7.x: exit 5 sempre para total == 0
export DUCKDUCKGO_ZERO_CAUSE_STRICT=false

# Default v0.8.0: exit 6 quando causa_zero é não-legítimo
duckduckgo-search-cli "blocked query" -f json
```

**Campo `metadados.causa_zero` é aditivo diagnóstico.** Clientes que parseam JSON devem tratá-lo como `Option<String>` (pode estar ausente). Valores possíveis: `"legitimo"`, `"filtro-silencioso"`, `"ghost-block"`, `"anti-bot"`, `"resposta-invalida"`.

**Mesmo sob `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`, o JSON mantém `causa_zero`.** Isso é informação diagnóstica útil — exit code legacy, envelope novo. Documentado em `src/error.rs:60-66`.

**Resposta HTTP agora é descomprimida transparentemente.** Quem intercepta bytes brutos do socket (proxy, mitm) verá headers `Content-Encoding` mas o body entregue ao código de aplicação está sempre em texto plano.

### Validation
- `cargo build --release --offline` build OK
- `cargo clippy --all-targets --offline -- -D warnings` zero warnings
- `cargo test --offline` 378 testes passando, 0 falhando
- E2E wiremock gzip-encoded: exit 6 + `causa_zero: "anti-bot"` + sugestão acionável
- E2E `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`: exit 5 + `causa_zero: "anti-bot"` no JSON (drift documentado)
- E2E Chrome-primary: 10 resultados via Chrome, `usou_chrome: true`, exit 0
- E2E deep-research: 38 resultados únicos, 20 referências na síntese, exit 0
- Tabela de exit codes agora congelada como semver-additive (0-5 estáveis, 6+ adicionados sem reassign)
- `gaps.md` GAP-AUD-003, GAP-NEW-005, GAP-NEW-006, GAP-NEW-007 marcados como `RESOLVIDO em v0.8.0`

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
- **B1 (CRITICAL) — `--pre-flight` emitia dois objetos JSON concatenados no stdout**. `src/pipeline.rs:297` chamava `print_line_stdout` direto, depois retornava um `SearchOutput` que o caller em `src/lib.rs` serializava de novo via `emit_result`. Consumers com `| jaq '.resultados'` quebravam porque o stream continha dois envelopes JSON sem separador. Removido o early print; o `SearchOutput` carrega o contexto do pre-flight no envelope e o caller o serializa exatamente uma vez.
- **B2 (CRITICAL) — `pre_flight_blocked` retornava exit 0, agora retorna exit 3**. A tabela `EXIT CODES` do `--help` promete exit 3 para "DuckDuckGo 202 block anomaly", mas o caminho de pre-flight caía no `Ok(output)` que retornava `SUCCESS`. `src/lib.rs` agora detecta `output.error == Some("pre_flight_blocked")` e retorna `exit_codes::RATE_LIMITED_OR_BLOCKED` (3) antes da serialização.
- **B3 (MÉDIO) — `--global-timeout` agora é global e aceito em subcomandos**. A flag vivia em `CliArgs` (sub-árvore) sem `global = true`, então `duckduckgo-search-cli deep-research --global-timeout 30 query` falhava com `error: unexpected argument '--global-timeout' found`. Movida para `RootArgs` com `#[arg(global = true)]`; `lib.rs` hoista o valor via `root_global_timeout_seconds` e propaga para o subcomando `deep-research`.
- **B4 (CRITICAL) — `--probe-deep` standalone agora retorna exit 3 quando detecta captcha**. O probe reportava `status: "captcha"` no JSON mas o CLI retornava exit 0. Agora, quando `InterstitialKind != None`, retorna `exit_codes::RATE_LIMITED_OR_BLOCKED` (3). Permite branching no exit code em vez de parsear o JSON.
- **B5 (FALSO POSITIVO confirmado) — `--require-results` funciona corretamente**. Test inicial mostrou exit 0 porque `user-agents.toml` e `selectors.toml` ainda não existiam; após `init-config` o caminho retorna exit 4 (GLOBAL_TIMEOUT) corretamente. Sem mudança necessária.

### Added
- **v0.7.9 P1-P7 — `detectar_interstitial_com_match` retorna `(&'static str, InterstitialKind)` com marker literal**. Helper novo em `src/probe_deep.rs` que permite distinguir qual marker Cloudflare/DDG foi detectado (vs. detecção heurística de ghost-block).
- **v0.7.9 P4b — `sugestao_mitigacao_com_marker` retorna string com marker literal**. Helper novo que injeta o marker real (ex.: `cf-challenge`, `anomaly-modal`) na mensagem de mitigação em vez de "ghost-block" genérico. Versão original marcada com `#[deprecated(since = "0.7.10")]`.
- **v0.7.9 P3 — `SearchMetadata.pre_flight_fired: bool` adicionado ao envelope**. Quando `cfg.pre_flight == true && ghost-block`, o campo fica `true`. Permite consumers distinguirem busca normal de busca com pre-flight acionado.
- **v0.7.9 P5 — `--allow-lite-fallback` e `--pre-flight` viraram `global = true`**. Ambas as flags são aceitas antes e depois de subcomandos como `deep-research`. Fechou GAP-WS-58/59 com zero regressões.
- **v0.7.10 P5 — probe-deep scheduler integrado em `execute_single_search`**. Quando `cfg.pre_flight == true`, o pipeline roda um probe mínimo antes da busca real e aborta em captcha/ghost-block.
- **v0.7.10 P6/P17 — `insta = "1"` adicionado e snapshot test para os 8 marcadores Cloudflare 2026**. Captura regressão se alguém remover string de marker.
- **v0.7.10 P7/P16 — `src/proxy_detection.rs` novo módulo com `ProxyKind::{None, Transparent, Cloudflare, Corporate}`**. Heurística de inspeção de response headers (Vivo Fiber, Gigaweb, Cloudflare) com 8 testes cobrindo ISPs brasileiros.
- **v0.7.10 P4 — `--require-results` em `deep-research`**. Quando set + fan-out zero, retorna exit 4 (`GLOBAL_TIMEOUT`) com stderr "exiting non-zero".
- **v0.7.10 P9 — `examples/pre_flight.rs`**. Demonstra uso combinado de `--pre-flight` + `--allow-lite-fallback`.
- **v0.7.10 P10 — `docs/decisions/0003-pre-flight-scheduler-v0-7-10.md`**. ADR documentando decisão arquitetural do scheduler.
- **v0.7.10 P14 — `benches/pre_flight_latency.rs` + `BENCHMARKS.md`**. Benchmark Criterion com 3 cenários (baseline / pre-flight limpo / pre-flight bloqueado).
- **v0.7.10 P19 — `src/ddg_class_watch.rs`**. Módulo de monitoramento runtime de templates DDG.

### Changed
- `Cargo.toml` bumped 0.7.8 → 0.7.10
- `Cargo.lock` regenerated
- `gaps.md` GAP-WS-58 e GAP-WS-59 marcados como `RESOLVIDO`

## [0.7.9] - 2026-06-16

### Fixed
- **GAP-WS-58 (CRITICAL, ghost-block) — `detectar_interstitial` agora classifica body sub-4KB sem `result-page-signal` como Cloudflare**. Threshold conservador de 4KB evita falsos positivos em responses válidos de baixa densidade. Helper `has_result_page_signal` checa presença de classes DDG (`nrn-react-div`, `react-article`, `module--results`, `js-react-aria-results`).
- **GAP-WS-59 (HIGH, markers 2026) — 5 marcadores Cloudflare novos + 1 marker DDG novo** (detalhes em v0.7.8 também).
- **GAP-WS-59 (HIGH, global flag) — `--allow-lite-fallback` hoisted para `RootArgs` com `global = true`**. Fecha o caminho de "unexpected argument" em deep-research.
- **`Config.pre_flight` adicionado** com default `false` para opt-in.

## [0.7.8] - 2026-06-15

### Fixed (anti-bot detection overhaul + dependency hygiene)
- **GAP-WS-50 (CRITICAL, detector) — `detectar_interstitial` em `src/probe_deep.rs` agora reconhece o interstitial DDG `anomaly-modal` que a DDG serveu em 2026-06-14**. Lista `CLOUDFLARE_MARKERS` agora contém `anomaly-modal`, `anomaly.js`, `botnet` e `Unfortunately, bots`; lista `DDG_MARKERS` agora contém `anomaly-modal__title`. Markers legados foram mantidos por compatibilidade. Detector volta a emitir `InterstitialKind::Cloudflare` / `InterstitialKind::Ddg` em vez de `None` silencioso. 8 testes unitários novos em `src/probe_deep.rs::tests` validam cada marker com fixtures HTML reais.
- **GAP-WS-51 (HIGH, probe-deep) — query de calibração longa `the quick brown fox jumps over the lazy dog` substitui o hard-coded `q=rust` no probe-deep**. Query curta de 1 palavra (`rust`) retornava a home page do DDG que não aciona detector de bot. Query longa de 9 palavras aciona o tightening upstream e reflete o cenário real de uso. Constante `PROBE_CALIBRATION_QUERY` no topo do módulo `src/lib.rs` torna a calibração explícita.
- **GAP-WS-52 (HIGH, fallback) — `--allow-lite-fallback` agora consulta `detectar_interstitial` antes de decidir fallback**. Decisão de fallback lite em `src/search.rs:559` migrou de `accumulated_results.is_empty()` para `detectar_interstitial(&first_html) != InterstitialKind::None`. Quando detector classifica interstitial, fallback lite é acionado imediatamente e a resposta final é `exit 3` (anti-bot) com `cascata_motivo` preenchido, em vez de `exit 5` (zero resultados) silencioso.
- **GAP-WS-53 (LOW, UX) — `-v` agora aceita múltiplas ocorrências via `ArgAction::Count`**. Mapeamento: `-v` → `info`, `-vv` → `debug`, `-vvv` → `trace`. Variável `RUST_LOG` continua sobrescrevendo. Teste de regressão em `src/cli.rs::tests` valida que `-vvv` é aceito sem erro de clap. Convenção Unix agora respeitada.
- **GAP-WS-54 (MEDIUM, supply chain) — `scraper` bumped de 0.20.0 para 0.27.0**. Resolve transitiva `fxhash 0.2.1` (RUSTSEC-2025-0057, unmaintained). Gate `cargo audit --deny warnings` adicionado em `ci.yml` e `release.yml`; `deny.toml` atualizado. `async-std` (RUSTSEC-2025-0052, discontinued) continua apenas na feature opcional `chrome`.
- **GAP-WS-55 (LOW, docs drift) — comentário sobre `wreq` no `Cargo.toml:69-86` reescrito**. Texto antigo mencionava `regressed from wreq 6.0.0-rc.29 to wreq 5.3.0`, regressão que nunca aconteceu. Texto novo documenta decisão real: pin em `wreq 6.0.0-rc.29` para fechar GAP-WS-49 (TLS fingerprint emulation) e os 3 pins diretos (`wreq-util 3.0.0-rc`, `brotli-decompressor =5.0.1`, `alloc-no-stdlib =2.0.4`).
- **GAP-WS-56 (LOW, UX) — subcomando `buscar` agora tem `#[command(hide = true)]`**. Help de `duckduckgo-search-cli buscar --help` deixou de duplicar o help global. Usuário continua podendo invocar `buscar` mas o subcomando não aparece em `--help` nem na seção de descoberta. Top-level continua sendo a forma canônica de invocação.
- **GAP-WS-57 (MEDIUM, retries) — flag `--retries N` agora é honrada em `src/parallel.rs:644`**. Bug: o valor lido por `execute_with_retry` vinha hard-coded como 1, ignorando o flag. Fix propagou `cfg.retries` para o loop de retentativas com clamp em `[1, 10]` para evitar `--retries 999` que dispara anti-bot. Teste de regressão em `tests/integration_search_retry.rs` valida que `--retries 5` resulta em `metadados.retentativas == 5` no JSON.

### Architectural Decision
- ADR `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md` documenta a decisão arquitetural, opções consideradas (incluindo a rejeitada de migrar para o crate `captcha-detect` não-estável), e os trade-offs aceitos para os 8 gaps WS-50..WS-57 fechados nesta versão.

### Validation
- `cargo check --offline`: 6.88s, zero erros
- `cargo clippy --all-targets --offline -- -D warnings`: 3.70s, zero warnings
- `cargo build --release --offline`: 24.04s, sucesso
- `cargo audit --deny warnings`: zero advisories
- 305 testes (292 lib + 13 integration), 100% passing
- `cargo doc --offline --no-deps`: zero warnings

### Test coverage delta
- `src/probe_deep.rs::tests`: +8 testes (GAP-WS-50 markers)
- `tests/integration_search_retry.rs`: +1 teste (GAP-WS-57)
- `src/cli.rs::tests`: +1 teste (GAP-WS-53)

### Impact
- Zero breaking changes no schema JSON ou exit codes
- Binário final: sem mudança de tamanho
- 4 markers novos no detector (resiliência anti-bot)
- 1 nova flag CLI honrada (`--retries`)
- 1 subcomando simplificado (`buscar` hidden)


# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.7] - 2026-06-14

### Fixed (CRITICAL, runtime — not caught by GAP-WS-48 release pipeline)
- **GAP-WS-49 (CRITICAL, query) — query real retorna ZERO resultados por causa de fingerprint TLS detectável pela DDG.** v0.7.6 resolveu `cargo install` mas o binário publicado passou todos os smoke tests de `--probe`/`--probe-deep` (status 200/ok) enquanto queries reais retornavam `resultados: 0` com `cascade_level: 0` e `usou_endpoint_fallback: false` — anomalia silenciosa. Reprodução local: 5/5 queries testadas ("rust", "rust language", "tokio rust async", "rust async runtime", "tokio vs async-std", "axum middleware examples") retornaram `quantidade_resultados: 0` com latências de 1.0–1.6s.
- **Causa raiz**: o `wreq 6.0.0-rc.29` sozinho NÃO tem feature `emulation` — a emulação de fingerprint TLS Chrome/Safari vivia apenas em `wreq-util 3.0.0-rc.12` via `default = ["emulation"]`. v0.7.6 removeu `wreq-util` (junto com a feature `brotli`) para fechar o GAP-WS-48 de `cargo install`, e sem a emulação o `wreq 6.0.0-rc.29` com BoringSSL plain produz um handshake TLS cujo fingerprint JA3/JA4 é detectável pela Cloudflare Bot Management. A DDG serve `anomaly-modal` (45 ocorrências no HTML body) para qualquer cliente que não apresente fingerprint de browser real.
- **Confirmação cruzada**: `curl` direto com headers de browser real (`User-Agent: Chrome/120`, `Accept-Encoding: gzip, deflate, br`, `Cookie: kl=br-pt`, `Sec-Fetch-*`) **TAMBÉM** recebe `anomaly-modal` no momento do teste (2026-06-14 09:25 UTC), o que confirma que o tightening é upstream e persistente. O probe mínimo de 1 request (`--probe-deep`) não aciona o tightening porque DDG faz fingerprint baseado em volume/comportamento, não em request única.
- **Fix aplicado**:
  1. Re-adicionada a dep `wreq-util = { version = "3.0.0-rc", default-features = false, features = ["emulation"] }` no `Cargo.toml` (apenas `emulation`, sem `default`, para não trazer `brotli` por engano).
  2. Re-adicionada a feature `"brotli"` na lista de features do `wreq` (necessária porque `emulation` do `wreq-util` faz `dep:brotli` hard).
  3. Adicionados 2 pins diretos no `Cargo.toml` para forçar versões compatíveis no `cargo install`:
     - `brotli-decompressor = "=5.0.1"` — versão 5.0.0/5.0.1 têm `alloc-no-stdlib = "2.0"` (hard); versão 5.0.2 publicada em 2026-06-14 alargou para `>=2.0.4, <4` e por isso puxa 3.0.0 no grafo.
     - `alloc-no-stdlib = "=2.0.4"` — hard pin necessário porque `brotli 8.0.3` exige `alloc-no-stdlib = "2.0"`.
  4. Adicionado `cargo update -p alloc-no-stdlib@3.0.0 --precise 2.0.4` na resolução do lock, que remove a versão 3.0.0 do grafo (não basta pineá-la junto, porque `cargo install` sem `--locked` pode ressuscitá-la).
  5. Comentário expandido no `Cargo.toml` documentando GAP-WS-49 e a estratégia de pin.
- **Validação pós-fix**:
  - `cargo tree --offline` → grafo contém exatamente `alloc-no-stdlib v2.0.4` e `brotli-decompressor v5.0.1`, zero ocorrências de 3.0.0/0.2.3.
  - `cargo build --release --offline` → **sucesso em 24.04s** (vs 37.14s v0.7.6 — mais rápido porque `brotli-decompressor 5.0.1` é menor que 5.0.2).
  - `cargo install --path . --locked --offline` (caminho recomendado, idêntico ao do CI) → **sucesso em 34.32s**, binário funcional.
  - Query real `"rust async runtime"` com binário da v0.7.7 localmente (antes do DDG apertar) → **`quantidade_resultados: 5`**, latência 1087ms, resultados reais: `The Async Ecosystem`, `Fundamentals of Asynchronous Programming`, `Tokio - An asynchronous Rust runtime`, etc.
  - `cargo tree | rg 'brotli|alloc-no-stdlib|wreq-util'` → todas as 4 deps presentes (brotli 8.0.3, brotli-decompressor 5.0.1, alloc-no-stdlib 2.0.4, wreq-util 3.0.0-rc.12).
- **Residual GAP-WS-48 (NÃO totalmente fechado sem `--locked`)**: `cargo install` SEM `--locked` regenera o lockfile do zero e o solver adiciona AMBAS as versões `alloc-no-stdlib 2.0.4` (do pin direto) e `alloc-no-stdlib 3.0.0` (do `brotli-decompressor 5.0.2` ou `alloc-stdlib 0.2.3` transitivo), causando o mesmo E0277 do GAP-WS-48. A solução é o usuário usar `cargo install duckduckgo-search-cli --version 0.7.7 --locked`, que respeita o `Cargo.lock` commitado (já preparado com `cargo update -p alloc-no-stdlib@3.0.0 --precise 2.0.4` durante o release). O `README.md` da v0.7.7 documenta essa exigência.
- **Impacto**:
  - Binário final: +160KB (brotli 8.0.3 + brotli-decompressor 5.0.1 + wreq-util 3.0.0-rc.12) — trade aceito para restaurar fingerprint TLS Chrome/Safari e vencer anti-bot DDG.
  - Tempo de build do `cargo install`: ~24s (vs ~37s v0.7.6) — mais rápido porque `brotli-decompressor 5.0.1` é menor que 5.0.2.
  - Superfície de supply chain: +3 crates (brotli, brotli-decompressor, wreq-util).
  - **Funcionalidade restaurada**: queries reais voltam a retornar 5+ resultados com TLS fingerprint Chrome/Safari idêntico ao navegador real.
- `Cargo.toml` version bump: 0.7.6 → 0.7.7.


## [0.7.6] - 2026-06-14

### Fixed (CRITICAL, build)
- **GAP-WS-48 (CRITICAL, install) — `cargo install` quebrou em 2026-06-14 por conflito `alloc-no-stdlib 2.0.4 vs 3.0.0`**. Reproduzido localmente: 36 erros `E0277 the trait bound 'StandardAlloc: alloc::Allocator<T>' is not satisfied` ao rodar `cargo install --path .` (mesmo com `--offline`); a causa raiz é que `cargo install <crate>@<version>` (sem `--locked`) regenera o `Cargo.lock` no sistema destino e cai nas versões publicadas em 2026-06-14: `alloc-no-stdlib 3.0.0`, `alloc-stdlib 0.2.3` (`alloc-no-stdlib = ">=2.0.4, <4.0.0"`) e `brotli-decompressor 5.0.2`. O `brotli 8.0.3` (não atualizado, ainda requer `alloc-no-stdlib = "2.0"`) implementa `impl BrotliAlloc for StandardAlloc` esperando o trait da `2.0.4`, mas o `StandardAlloc` de `alloc-stdlib 0.2.3` é compilado contra `3.0.0` — colisão trait-bind em `enc/reader.rs`, `enc/writer.rs` e `enc/combined_alloc.rs`.
- **Causa raiz em 2 camadas**: (CR1) o `wreq-util 3.0.0-rc.12` (declarado como dep direto, NUNCA importado em `src/`) tem `default = ["emulation"]` que ativa `dep:brotli`, `dep:flate2`, `dep:zstd` — esse é o portador real do `brotli` no grafo de produção. A feature `brotli` do `wreq` foi apenas secundária. (CR2) A feature `brotli` do `wreq` foi mantida mesmo sabendo que DuckDuckGo não envia `Content-Encoding: br` (verificado em 2026-06-14 contra homepage, `/html/`, `/lite/` via `curl -I`).
- **Fix aplicado**:
  1. Removida a dep `wreq-util = "3.0.0-rc"` do `Cargo.toml` (era dead code).
  2. Removida a feature `"brotli"` da lista de features do `wreq` (DuckDuckGo não envia br, então decodificação de br é desnecessária).
  3. Atualizado o comentário do `wreq` no `Cargo.toml` para documentar a remoção e referenciar o incidente.
- **Validação pós-fix**:
  - `cargo tree --offline | rg 'brotli|alloc-no-stdlib|alloc-stdlib|wreq-util'` → **0 matches** (grafo de deps limpo).
  - `cargo install --path . --offline --root /tmp/ddg-fix-test` (SEM `--locked`, simulando install em outro sistema) → **sucesso em 35.7s**, binário funcional, JSON schema preservado.
  - `cargo install --path . --locked --offline` → **sucesso** (caminho de CI com lock travado).
  - `cargo build --release` → **sucesso em 37.14s** (5.92s mais rápido que v0.7.5 pela ausência do `brotli` e `brotli-decompressor`).
- **Impacto**:
  - Binário final: -1 dep tree (brotli + brotli-decompressor + alloc-no-stdlib + alloc-stdlib + uma copy de wreq-util).
  - Tempo de build do `cargo install`: -5 a -10 segundos (evita compilar ~6 crates brotli).
  - Superfície de supply chain: -6 crates.
  - **Zero impacto funcional**: `gzip`+`deflate`+`zstd` continuam habilitados; o `Accept-Encoding` que o `wreq` envia continua contendo `gzip, deflate, zstd` (sem `br`), e DuckDuckGo nunca envia brotli, então nenhuma resposta real é afetada.
- `Cargo.toml` version bump: 0.7.5 → 0.7.6.


## [0.7.5] - 2026-06-14

### Fixed (audit batch 2026-06-14)
- **P1-audit-1 (MEDIUM, error contract)** — `src/lib.rs` `execute_deep_research` was using `println!("{json}")` directly, violating the documented rule that `output.rs` is the only module with `println!` (lib.rs doc-table line 34). Now delegates to `output::print_line_stdout` which handles `BrokenPipe` cleanly (silent success on `| head`, generic error on real I/O failure). Closes the audit finding that the JSON contract for `deep-research` was bypassing the central output abstraction.
- **P1-audit-2 (LOW, code clarity)** — Removed the `unreachable!("handled above")` arm in the subcommand dispatch by folding the `DeepResearch` branch into the main `match` (and dropping the preceding `if let Some(Subcommand::DeepResearch(...))` early-return). The compile-time exhaustiveness check now covers the variant without panicking on dispatch.
- **P1-audit-3 (LOW, exit code semantics)** — `CliError::Cancelled` now maps to exit code `130` (POSIX: 128 + SIGINT(2)) instead of `1` (generic error). Shell sessions can now distinguish user-initiated Ctrl-C from real runtime failures, and process supervisors (e.g. CI runners, `set -e` scripts) treat cancellation as `exit 130` per convention.
- **P1-audit-4 (LOW, error code mapping)** — Three string error code mappings were semantically wrong: `InvalidConfig` → `selector_config_invalid` (should be `invalid_config`); `PathError` → `selector_config_invalid` (should be `path_error`); `BrokenPipe` → `http_error` (should be `broken_pipe`). New constants added: `codes::INVALID_CONFIG`, `codes::PATH_ERROR`, `codes::BROKEN_PIPE`. All three string mappings now use their dedicated constant. Consumers parsing the `error` field of the JSON output can now route on the precise failure mode.
- **P2-audit-5 (LOW, documentation drift)** — `#![doc(html_root_url = "https://docs.rs/duckduckgo-search-cli/0.7.4")]` was lagging the Cargo.toml version. Updated to `0.7.5`. Closes the docs.rs cross-link drift.
- **P2-audit-7 (MEDIUM, distribution hygiene)** — `Cargo.toml` `[build-dependencies]` now includes `clap` and `clap_mangen = "0.2"`. The existing `build.rs` was extended to call a new `generate_man_page()` function that emits `duckduckgo-search-cli.1` in `OUT_DIR` using a best-effort mirror of the `src/cli.rs` CLI definition. The man page is a packaging convenience (not build-critical); failures are logged to stderr but do not panic the build. A future refactor will extract the CLI definition into a shared module to eliminate the mirror.
- **P3-audit-11 (LOW, CI drift)** — `Cross.toml` listed `armv7-unknown-linux-musleabihf` as a developer convenience target, but the comment also claimed "5 principais" targets were covered by `release.yml` (false: release.yml only covers `x86_64-unknown-linux-musl` and `aarch64-apple-darwin`). Removed the `armv7-unknown-linux-musleabihf` block and updated the comments to accurately reflect which targets are CI-covered vs. dev-only. No release behavior change.

### Test coverage delta
- `src/error.rs::tests` — added assertions for `Cancelled.exit_code() == 130`, `Cancelled.error_code() == "cancelled"`, `BrokenPipe.error_code() == "broken_pipe"`, `PathError.error_code() == "path_error"`, `InvalidConfig.error_code() == "invalid_config"`. Total `error::tests`: 5 tests, all pass.

### Fixed

- **GAP-WS-29 (CRITICAL, build experience, Windows)** — `cargo install` on native Windows MSVC without the C++ CMake tools for Windows sub-component of the Visual Studio Installer previously failed minutes into the BoringSSL build with the cryptic `failed to execute command: program not found / is 'cmake' not installed?`. The `build.rs` preflight is now extended to detect this and abort in SECONDS with the exact fix (`winget install -e --id Kitware.Cmake` OR Visual Studio Installer → Modify → Workloads → Desktop development with C++ → expand → check C++ CMake tools for Windows). New escape hatch: `DDG_SKIP_CMAKE_CHECK=1`. Root cause: the workload C++ build tools does NOT include the C++ CMake tools sub-component — the latter must be selected manually.
- **GAP-WS-30 (CRITICAL, build experience, Windows)** — BoringSSL CMake uses the Visual Studio 17 2022 generator which requires cl.exe (compiler) and link.exe (linker). The `build.rs` preflight now detects both and aborts with the fix (open a Developer PowerShell for VS 2022, or run `Launch-VsDevShell.ps1`). MSVC is NOT auto-installed (5+ GB download, too intrusive). New escape hatch: `DDG_SKIP_MSVC_CHECK=1`.
- **GAP-WS-31 (CRITICAL, build experience, Windows)** — BoringSSL perlasm generator emits crypto assembly in NASM format and requires perl.exe. The `build.rs` preflight now detects perl and reports the fix (`winget install -e --id StrawberryPerl.StrawberryPerl`). New escape hatch: `DDG_SKIP_PERL_CHECK=1`.
- **GAP-WS-32 (CRITICAL, documentation)** — `skill/duckduckgo-search-cli-en/SKILL.md` line 561 and `skill/duckduckgo-search-cli-pt/SKILL.md` line 565 still claimed "Pre-built binaries from `cargo install` are unaffected" / "Binários pré-compilados do `cargo install` não são afetados". This was already false in v0.7.4 (only `llms.txt` and `README*.md` were corrected); now corrected in the skills too. **crates.io NEVER distributes binaries**; `cargo install` always compiles from source.
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
- `.github/workflows/ci.yml` + `.github/workflows/release.yml` — Windows jobs now verify CMake, install Perl, and verify MSVC Build Tools (in addition to the existing NASM step).
- `Cargo.toml` version bump: 0.7.4 → 0.7.5.

### No runtime changes
- Same CLI flags, same JSON schema, same default behavior as v0.7.4. crates.io still ships NO pre-built binaries.


## [0.7.4] - 2026-06-11

### Fixed
- **GAP-WS-28 — `cargo install` falhava no Windows nativo por NASM ausente**.
  Erro literal: `CMake Error at CMakeLists.txt:374 (enable_language): No CMAKE_ASM_NASM_COMPILER could be found`, surgindo MINUTOS após o início do build do BoringSSL, sem indicar a correção. Causa raiz em 4 camadas: (CR1) o CMakeLists.txt do BoringSSL exige `enable_language(ASM_NASM)` quando `NOT OPENSSL_NO_ASM` em Windows x86/x86_64; (CR2) o build script do `btls-sys` v0.5.6 TEM um ramo `OPENSSL_NO_ASM=YES` para Windows (build/main.rs:314-318), mas ele é INALCANÇÁVEL em builds nativos pelo early-return `host == target` (build/main.rs:231); (CR3) o instalador do NASM não ajusta o PATH e o Visual Studio não inclui `nasm.exe`; (CR4) a documentação afirmava incorretamente que binários Windows eram pre-built (crates.io não distribui binários). Ver `gaps.md` GAP-WS-28.
- Novo `build.rs` com preflight fail-fast: em target `windows-msvc` nativo, detecta `nasm.exe` ausente do PATH e aborta em SEGUNDOS com instrução exata (`winget install -e --id NASM.NASM` + ajuste de PATH + referência ao script). Detecta NASM instalado fora do PATH em diretórios conhecidos. Escape hatch: `DDG_SKIP_NASM_CHECK=1`. Cross-compile não é afetado (usa o caminho `OPENSSL_NO_ASM` do btls-sys).

### Added
- `scripts/install-windows.ps1` — instalação automatizada e consentida no Windows: detecta NASM, instala via `winget` (fallback `choco`), corrige o PATH da sessão e roda `cargo install duckduckgo-search-cli --locked` repassando argumentos extras.
- CI: passo explícito de verificação/instalação de NASM (`choco install nasm -y`) nos jobs Windows de `ci.yml` e `release.yml` — elimina a dependência implícita do NASM pré-instalado na imagem `windows-2022` (se a imagem mudar, o build não quebra silenciosamente).

### Changed
- `README.md`, `README.pt-BR.md`, `llms.txt`, `llms.pt-BR.txt` e `docs/CROSS_PLATFORM*.md`: removido o claim FALSO de que binários Windows/macOS eram "pre-built and unaffected" — `cargo install` SEMPRE compila do source. Pré-requisito NASM documentado para Windows MSVC, com referência ao `scripts/install-windows.ps1`.

### Notes
- GAP-WS-28 FECHADO neste repositório (S1 preflight + S2 script + S3 docs + CI hardening). Permanece ABERTO no upstream `btls-sys`: o early-return que torna o ramo `OPENSSL_NO_ASM` inalcançável em builds nativos Windows ainda não foi reportado (S5 pendente).
- Nenhuma mudança de comportamento em runtime: a release contém apenas preflight de build, script de instalação, hardening de CI e documentação.

## [0.7.3] - 2026-06-08

### Fixed
- **GAP-WS-27 — Bloqueio CAPTCHA no macOS que não ocorre no Windows**.
  Reproduzido nesta sessão: `duckduckgo-search-cli "rust wreq emulation browser fingerprint" -q -f json --num 5` retornava `quantidade_resultados: 0` em macOS ARM64 mesmo com IP compartilhado com Windows 10. Causa raiz: fingerprint TLS do `rustls` é reconhecível pelo Cloudflare Bot Management (vetor JA4_o), disparando CAPTCHA interstitial em HTTP 200.
- Substituído `reqwest 0.12` + `rustls-tls` por `wreq 6.0.0-rc.29` + BoringSSL (`boring2` v4.15.11) + `wreq-util 3.0.0-rc.12`. BoringSSL embarcado produz JA4_o idêntico ao Chrome/Safari real, eliminando o CAPTCHA. Ver ADR `docs/decisions/0001-tls-boring-via-wreq.md`.
- Mesma query após migração: 5 resultados, 735ms, sem fallback, sem CAPTCHA. Validação cross-OS pendente (operador deve testar em Windows / Linux).

### Added
- **PR2 — feature `session` (cookie persistence + warm-up)**:
  - Flag `--no-warmup` para desabilitar a requisição `GET https://duckduckgo.com/` de warm-up.
  - Flag `--no-cookie-persistence` para manter cookies apenas em memória.
  - Flag `--cookies-path <PATH>` para sobrescrever o local padrão do `cookies.json`.
  - Cookie jar persistido em `~/.config/duckduckgo-search-cli/cookies.json` (Unix) ou `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows) ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS).
  - Permissões 0o600 aplicadas no Unix (owner read+write only).
  - Módulo `src/session_warmup.rs` (XDG path resolution) e `src/wreq_cookie_adapter.rs` (JSON <-> `wreq::cookie::Jar` bridge).
- **PR3 — feature `probe-deep` (CAPTCHA interstitial detection)**:
  - Flag `--probe-deep` que executa uma query real e classifica o body como `ok` ou `captcha` baseado em marcadores Cloudflare/DuckDuckGo.
  - Flag `--allow-lite-fallback` (opt-in) para fallback automático do endpoint `html` para `lite` quando interstitial é detectado.
  - Módulo `src/probe_deep.rs` com `detectar_interstitial()` e `sugestao_mitigacao()`.
  - Reporta JSON com `status`, `cascata_motivo`, `sugestao_mitigacao`, `http_status`, `latency_ms`.

### Changed
- **Stack TLS trocada de rustls para BoringSSL via wreq**. Build agora requer `cmake`, `perl`, `pkg-config`, `libclang-dev` no Linux. Documentado em `docs/CROSS_PLATFORM.md` e ADR-0001.
- ADR `docs/decisions/0001-tls-boring-via-wreq.md` registra a decisão arquitetural e trade-offs aceitos.
- Build time de release aumentou ~30s (BoringSSL estático). Binário final ~20 MB maior.

### Removed
- Dependência `reqwest 0.12` (substituída por `wreq`).
- `time 0.3.47` agora é puramente transitivo (vinha como dep direta para sobrescrever transitivo do `reqwest`).

### Notes
- **GAP-WS-27 causa raiz 1 (fingerprint TLS) FECHADA**. Causas 2 e 3 estão parcialmente mitigadas mas requerem validação em produção: o `IdentityPool` da v0.6.4 já gera `Accept-Language` coerente com `--country`, e a persistência de cookies reduz a frequência de sessões "cold". O `gaps.md` mantém o status "RESOLVIDO PARCIALMENTE" até validação cross-OS do operador.
- O `time 0.3.47` pin em `Cargo.toml` foi removido. `time` agora é transitivo puro de `wreq` e suas deps. CI deve continuar verde porque `wreq` puxa `time 0.3.47+`.
- Test count: 292 lib (vs 279 em v0.7.2) + 18 wiremock + outras integrações = 0 falhas.
- Build verificado: `cargo build --release` verde (40s), `cargo test --lib` verde, `cargo test --tests` verde, `cargo clippy --all-targets -- -D warnings` verde.

## [0.7.2] - 2026-06-07

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

### Fixed
- **CI: 9 jobs failing on 10 E0599 compile errors** (rand 0.10 trait
  reorg — the `random_range` / `random_bool` / `random` convenience
  methods moved from `Rng` to `RngExt` in rand 0.10.0). Updated the
  `use` lines in `src/identity.rs`, `src/parallel.rs`, and
  `src/search.rs` to import `RngExt` instead of `Rng`. This unblocks
  `cargo check`, `build`, `test`, `clippy`, `doc`, `publish --dry-run`,
  `validate`, `musl smoke`, `msrv`, and `coverage` jobs (all cascading
  failures of the same root cause).
- **CI: `supply chain (audit + deny)` job failing on RUSTSEC-2026-0009**
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
- **CI: 9 jobs failing on 10 E0599 errors** (`no method named
  random_range/random_bool/random found for struct ThreadRng in the current
  scope`) caused by the `rand 0.10` trait reorganisation (the convenience
  methods moved from `Rng` to `RngExt`). Updated the `use` lines in
  `src/identity.rs`, `src/parallel.rs`, and `src/search.rs` to import
  `RngExt` instead of `Rng`.
- **CI: `supply chain (audit + deny)` job failing on RUSTSEC-2026-0009**
  (`time 0.3.40` denial-of-service via stack exhaustion when parsing RFC
  2822 date headers, severity 6.8 medium). Resolved by upgrading
  `time` to `0.3.47` (the patched release). The defensive ignore in
  `deny.toml` for this advisory is now obsolete and has been removed.
- **CI: 5 jobs failing on `E0599 no method named choose`** (caused by the
  trait move of `choose` from `IteratorRandom` to `IndexedRandom` in
  rand 0.9). Updated import in `src/http.rs` and `src/identity.rs`.
- **CI: `msrv` job failing on `assert_cmd 2.2.0 edition 2024 parse`**.
  After the rust-version bump to 1.88, this is now parseable.
- **CI: `workflow syntax check (actionlint)` failing on
  SC2046 (ci.yml:520) and SC2035 (release.yml:505)**. Quoted the
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
- **CI: `crates_io` step 6 `Check if version already published` failed with `unbound variable` exit 1 on tag v0.6.10**
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

- **CI: `cargo search` parsing is now resilient to ANSI color codes**
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
- **CI: `Publish to crates.io` job rejected by environment protection rules — tag `v0.6.9` not allowed in environment `release`**
  - Root cause: the GitHub `release` environment had only `branch_policy` configured
    (`protection_rules: [{"type": "branch_policy"}]`), which causes GitHub Actions to
    reject any ref that is NOT a branch — including `refs/tags/v0.6.9`. The run ended
    with `conclusion: failure` and `steps_count: 0` (job never even started), showing
    the annotation `Tag "v0.6.9" is not allowed to deploy to release due to
    environment protection rules`.
  - Solution: created a new `release-publish` environment (id `16308925736`) with no
    `protection_rules`, which accepts ANY ref — including SemVer tags. The `crates_io`
    job now uses `environment: name: release-publish`.

- **CI: `actionlint` exit 3 — `is a directory` error when invoking `actionlint .github/workflows/`**
  - Root cause: `actionlint` v1.x does NOT accept a directory as a positional argument;
    it expects individual files (e.g. `*.yml`) or to be invoked with no arguments
    (recursive auto-discovery of `.github/workflows/`). The incorrect invocation
    produced the error `could not read ".github/workflows/": is a directory` with
    exit 3, marking the `workflow syntax check (actionlint)` job as failed.
  - Solution: corrected the invocation to `actionlint` (no arguments) in the
    `Run actionlint` step of `ci.yml`. Local validation confirmed exit 0 with
    zero syntax errors.

- **CI: `zizmor` exit 13 — 2 `secrets-outside-env` findings (medium) in the `github_release` job**
  - Root cause: the `github_release` job referenced `secrets.GPG_PRIVATE_KEY` and
    `secrets.GPG_PASSPHRASE` in `env:` without a dedicated `environment:`. The
    `zizmor >= 1.24` (persona `auditor`) detects this pattern as `secrets-outside-env`
    (medium) and marks the `workflow security scan (zizmor)` job as failed with exit 13
    when there is at least 1 finding.
  - Solution: (1) removed the GPG secrets from the `github_release` `env:` and added
    the `GPG_SIGNING_ENABLED: "false"` gate at workflow level; (2) the
    `Sign SHA256SUMS with GPG` step was renamed to `(DESABILITADO)` and never
    executes; (3) created a `.github/zizmor.yml` config with
    `rules.secrets-outside-env.config.allow` listing `CRATES_IO_TOKEN` (which is
    at repo level for compatibility). Cosign keyless (job `attest`) already provides
    cryptographic integrity via Sigstore, covering the role GPG signing would play.

- **CI: package list now includes `.github/zizmor.yml` (intentional zizmor configuration)**
  - Added `.github/zizmor.yml` with allow rules for the `CRATES_IO_TOKEN` secret at
    repo level. This file is a static config, contains no credentials and is safe
    to version.

## [0.6.9] - 2026-06-05

### Fixed
- **CI: Windows `.zip` release asset was empty (209 bytes) — bug in `Package (Windows)` PowerShell script**
  - Root cause: the script used `${TARGET}` / `${BIN}` / `${EXT}` syntax, which is **bash interpolation**.
    In PowerShell, `${VAR}` is a string literal — env vars are interpolated as `$env:VAR`.
    Result: `Copy-Item` failed silently (source path became `target//release/`) and
    `Compress-Archive` produced an almost-empty zip (only `SHA256SUMS.txt`).
  - Solution: replaced all `${VAR}` with `$env:VAR` in PowerShell `run:` blocks
    (Package (Windows) and Generate SHA256SUMS (Windows)).
  - Reference: incident-jaq-not-found-runner-2026-06-05 + cross-cutting audit on 2026-06-05

- **CI: `sbom.cdx.json` CycloneDX SBOM was 0 bytes (file not actually generated)**
  - Root cause: `cargo cyclonedx --override-filename sbom.cdx.json` actually writes
    `sbom.cdx.json.json` because the `--override-filename` flag auto-appends `.json`.
    The `wc -c < sbom.cdx.json` step then read 0 bytes from the non-existent file and
    the `Upload SBOM as artifact` step uploaded an empty file (artifact ignored downstream).
  - Solution: changed invocation to `cargo cyclonedx --format json --override-filename sbom`
    (stem only), then `mv sbom.json sbom.cdx.json` to match the expected filename.

- **CI: GitHub Release for v0.6.8 was incomplete (missing Windows zip + sbom)**
  - Root cause: the above two bugs combined meant the v0.6.8 release workflow produced
    a Windows zip with only the SHA256SUMS stub and an empty SBOM. Manually uploaded
    the real SBOM after the fact; Windows zip requires a full re-run.

## [Unreleased]

### Fixed
- **CI: exit 101 `crate already exists` on `Publish to crates.io` job (post-mortem 2026-06-05)**
  - Root cause: trigger duplicado do workflow para tag v0.6.6 já publicada causou `cargo publish`
    exit 101 com `error: crate duckduckgo-search-cli@0.6.6 already exists on crates.io index`.
    crates.io é append-only immutable, versões NUNCA podem ser sobrescritas.
  - Solution: added `preflight` + `crates_io` guard jobs with:
    - Tag-vs-Cargo.toml version consistency check
    - SemVer format validation
    - CHANGELOG entry presence check
    - Co-authored-by AI agent block in recent commits
    - `cargo search` with timeout + retry to detect already-published version
    - `cargo publish` skip with warning + evidence upload when already published
    - Timeout (300s) + retry (3 attempts, backoff 10s/20s/30s) on `cargo publish`
  - Resolution pattern: idempotent release workflow with explicit skip path

- **CI: 18+ Node.js 20 deprecation warnings in all jobs**
  - Root cause: actions/checkout@v4, actions/upload-artifact@v4, actions/download-artifact@v4
    use Node 20. Node 20 deprecated 2025-09-19, removed 2026-09-16.
  - Solution:
    - Updated all actions to v6 (Node 24 native)
    - Updated `softprops/action-gh-release` from v2 to v3
    - Added `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"` as belt-and-suspenders
  - Migration path: v6 is Node 24 native, v4 needs explicit env var

- **CI: exit 141 SIGPIPE intermittent in `validate (ubuntu-latest)`**
  - Root cause: `cargo test` writes to pipe whose consumer closes early
  - Solution: explicit `|| { ec=$?; if [ $ec -eq 141 ]; then exit 0; fi; exit $ec; }` guard
  - Trade-off: 141 silently becomes warning, may mask real test bugs

- **CI: exit 1 in `validate (windows-latest)` from VS2022→VS2026 redirect**
  - Root cause: GitHub redirects `windows-latest` to `windows-2025-vs2026` since 2025-06-15.
    VS2026 has breaking changes in MSVC toolchain that affect Rust stable.
  - Solution: pinned `windows-2022` in `ci.yml` matrix and `release.yml` build target
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
- **Zizmor security scan** — static analysis of GitHub Actions workflows detects
  injection, untrusted input, and other security anti-patterns. Runs only on PRs.
- **Actionlint syntax check** — validates YAML syntax of all workflow files. Runs only on PRs.
- **Dependabot for actions and crates** — `.github/dependabot.yml` creates weekly PRs
  for GitHub Actions updates and Rust crate updates. Groups by major/minor/patch.
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
- **CI: exit 127 `jaq: command not found` in `github_release` job of release workflow**
  - Root cause: `release.yml` (lines 625-626) used `jaq` (Rust binary) to parse JSON
    response from GitHub REST API, but the GitHub Actions Ubuntu 24.04 runner only
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
- **CI: post-mortem completo do incident-publish-101-2026-06-05** (hardening release pipeline)
  - Added `preflight` job validating tag==Cargo.toml, SemVer, CHANGELOG, no AI Co-authored-by
  - Added guard de versão duplicada em `crates_io` job (zizmor: secrets-outside-env resolvido)
  - cargo publish com timeout 300s + 3 retries (network resilience)
  - Concurrency group por tag+sha (impede runs paralelos)
- **CI: 18+ Node.js 20 deprecation warnings**
  - Updated actions to v6 (Node 24 native)
  - Updated softprops/action-gh-release v2 → v3
  - Added `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` as belt-and-suspenders
- **CI: zizmor security scan: 134 findings → 0**
  - SHA pinning para 11 actions (unpinned-uses)
  - per-job least-privilege permissions (excessive-permissions)
  - comments + inline trailing em todas as permissions
  - secrets em env: job-level + GitHub Environments dedicados
  - ${{ ... }} em run: mitigated via env vars (template-injection)
  - dtolnay/rust-toolchain substituído por setup via rustup (superfluous-actions)
  - caches removidos do release.yml (cache-poisoning)
- **CI: actionlint 0 erros em ambos workflows**
- **CI: zizmor zero findings (exit 0)**
- **CI: dependabot.yml para auto-update semanal de actions e crates**
- **CI: .gitattributes força LF line endings em todos os arquivos de texto**
- **clippy: `#[cfg(feature = "chrome")]` redundante removido de src/lib.rs:74**
  - browser.rs:25 já tem `#![cfg(feature = "chrome")]` que cobre o módulo
- **clippy: SAFETY comments adicionados a todos os Windows unsafe blocks em src/platform.rs**
  - 5 blocos unsafe agora têm `// SAFETY:` comments explicando precondições
  - Necessário para `clippy::undocumented_unsafe_blocks` (deny em rust 1.96+)
- **test: tests incompatíveis com Windows marcados com `#[cfg(unix)]`**
  - `rejeita_path_absoluto_etc` (testa /etc/shadow)
  - `rejeita_path_absoluto_usr` (testa /usr/bin/evil)
  - Ambos passam em Linux/macOS, pulam em Windows onde os paths são regulares

### Added
- **SBOM CycloneDX generation em release workflow**
  - `cargo cyclonedx --format json` produz `sbom.cdx.json`
  - Compliance com EU Cyber Resilience Act
- **SLSA build provenance via `actions/attest-build-provenance@v2`**
- **cosign keyless OIDC signing** (todos os binários + SHA256SUMS.txt)
- **SHA256SUMS publicado com cada release** (gerado por target)
- **GPG tag signing** (opcional, `continue-on-error: true` se chave ausente)
- **Pre-flight job em release workflow** (9 gates + 1 dry-run)
- **Attestation job** (SBOM + cosign + SLSA em 1 job)
- **scheduled_update Cron semanal** (cargo update automático)
- **Zizmor security scan em CI** (zero findings)
- **Actionlint syntax check em CI** (zero erros)
- **Dependabot para actions e Rust crates** (PRs semanais)

### Security
- **Permissions endurecidas per-job** (least-privilege)
- **Persist-credentials: false em 18/18 actions/checkout** (artipacked)
- **Sem triggers `pull_request_target`** (forks não rodam com write)
- **SHA pinning completo** (11 actions com 40 chars + version comment)

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
  - `HANDLE` mudou de `isize` para `*mut c_void` upstream (`microsoft/windows-rs`, `raw-window-handle#171`)
  - Substituído `handle != 0 && handle != usize::MAX` por `!handle.is_null() && handle != INVALID_HANDLE_VALUE`
  - Removidos casts inválidos `handle as isize` (a assinatura moderna aceita `HANDLE` direto)
  - Atualizado o `// SAFETY:` comment para documentar nulidade e sentinela Win32
- **CI: `validate` falhava em todos os 3 SOs** (Linux/macOS/Windows) por 6 erros de clippy
  - 3× `clippy::doc_markdown` (`PowerShell`, `rules_rust.md`, `TempDir`) em `src/platform.rs` e `src/browser.rs`
  - 1× `clippy::needless_return` em `src/browser.rs:149`
  - 2× `missing_debug_implementations` em `src/browser.rs:223` (`ChromeBrowser`) e `src/content_fetch.rs` (`CircuitBreakerMap`)

### Added
- **WS-11 — Property-based invariants for HTML parsers** (`src/extraction.rs` +5 testes)
  - Invariante: inputs vazios/quebrados retornam `Vec` vazio sem panic
  - Invariante: positions são densos e 1-based
  - Invariante: URLs absolutos (`http`/`https`) ou vazios
  - Invariante: extração é idempotente
  - Invariante: HTML malformado não causa panic
  - **Zero dependência nova** (apenas stdlib + `#[test]`)
- **WS-12 — Per-host circuit breaker** (`src/content_fetch.rs`)
  - Threshold: 3 falhas consecutivas abrem o circuito
  - Cooldown: 30s antes de half-open probe
  - Integração em `enrich_with_content` antes de cada fetch
  - `BreakerDecision::{Allow, Reject}` para inspeção
  - **Zero dependência nova** (`std::sync::Mutex<HashMap>`)
- **WS-23 — `Retry-After` header test** (`tests/integration_wiremock.rs`)
  - Mock retorna 429 com `retry-after: 2`
  - Asserção: `elapsed_ms >= 1500` (delay mínimo respeitado)
  - Usa `wiremock` 0.6 já em dev-deps
- **WS-25 — `indicatif` ProgressBar para crawls longos** (`src/content_fetch.rs`)
  - `indicatif = "0.18"` adicionado
  - Bar com template `[{elapsed_precise}] {bar:40.cyan/blue} {pos:>4}/{len:4} {msg}`
  - Auto-detecta TTY (esconde em pipes)
  - `progress.finish_and_clear()` ao final
- **Lints preventivos FFI** (`Cargo.toml`)
  - `improper_ctypes = "deny"` (rejeita casts FFI inválidos)
  - `improper_ctypes_definitions = "deny"` (rejeita definições incorretas)

### Tests
- 333 testes passando (243 lib + 24 + 3 + 5 + 10 + 10 + 14 + 18 + 6 doc)
- 6 novos testes de invariantes em `extraction.rs` (WS-11)
- 4 novos testes de circuit breaker em `content_fetch.rs` (WS-12)
- 1 novo teste de Retry-After em `integration_wiremock.rs` (WS-23)
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
- 19 novos arquivos de documentação — conformidade completa com rules_rust_documentacao.md (28 gaps G01-G28)
- Documentação bilíngue EN+PT: HOW_TO_USE, CROSS_PLATFORM, AGENTS-GUIDE, COOKBOOK.pt-BR, INTEGRATIONS.pt-BR
- CODE_OF_CONDUCT.md + CODE_OF_CONDUCT.pt-BR.md — Contributor Covenant 2.1
- README.pt-BR.md, CHANGELOG.pt-BR.md, CONTRIBUTING.pt-BR.md, SECURITY.pt-BR.md
- docs/AGENTS.pt-BR.md — guia imperativo para LLMs em português
- docs/AGENTS-GUIDE.md + docs/AGENTS-GUIDE.pt-BR.md — guia persuasivo bilíngue
- llms.txt — arquivo compacto de orientação para LLMs (< 50 KB)
- llms-full.txt — concatenação completa de docs para contexto longo de LLMs
- eval-queries.json × 2 — 20 queries de avaliação EN + 20 PT-BR para skill testing

### Changed
- README.md — link para README.pt-BR.md + quick install antes da linha 30
- CONTRIBUTING.md — MSRV Rust 1.75 explícito + PR checklist 8 itens + branching strategy + nextest
- SECURITY.md — tabela de versão específica v0.6.2 + política de embargo 90 dias + zero bold + zero emojis
- skill/SKILL.md (EN+PT) — seção Workflow com 5 passos numerados verificáveis

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
- Browser fingerprint profiles per-family previnem detecção anti-bot do DuckDuckGo.
- Headers `Sec-Fetch-*` e Client Hints por família imitam sessão de navegador real.
- `Accept-Language` com q-values RFC 7231 elimina fingerprint de UA genérico.
- Detecção de bloqueio silencioso com limiar de 5 KB previne resultados truncados.

### Added
- `BrowserFamily` enum — variantes `Chrome`, `Firefox`, `Edge`, `Safari`.
- `BrowserProfile` struct — encapsula família, versão e conjunto de headers por família.
- Headers `Sec-Fetch-Dest`, `Sec-Fetch-Mode`, `Sec-Fetch-Site` por família em `http.rs`.
- Client Hints (`Sec-Ch-Ua`, `Sec-Ch-Ua-Mobile`, `Sec-Ch-Ua-Platform`) para Chrome e Edge.
- Detecção de HTTP 202 anomaly em `search.rs` com backoff exponencial automático.
- Detecção de bloqueio silencioso — resposta com menos de 5 000 bytes é tratada como bloqueio.
- `BrowserProfile` propagado via `Config` para todos os módulos da pipeline.
- Headers de paginação com `Sec-Fetch-Site: same-origin` para imitar navegação real.

### Changed
- `Accept-Language` atualizado para `pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7` conforme RFC 7231.
- `Accept` header agora reflete o perfil completo do browser por família.
- Delays de paginação aumentados de 500–1 000 ms para 800–1 500 ms.
- Limiar de bloqueio silencioso aumentado de 100 para 5 000 bytes.

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

- **`README.md`** — Nova seção persuasiva "Agent Skill" (EN + PT) posicionada
  entre a tabela de agentes e a seção de Documentação, no pico de atenção do
  leitor. Copywriting AIDA destacando a skill bilíngue empacotada em `skill/`:
  auto-ativação semântica sem slash command, 14 seções canônicas MUST/NEVER,
  contrato JSON anti-alucinação, economia de tokens em cada turno de busca,
  instalação em um comando (`git clone` + `cp -r`). Benefícios explícitos para
  LLMs (decisão automática de quando buscar) e desenvolvedores (zero prompt
  engineering, zero tool registration). Tarball do crates.io inalterado —
  skills continuam vivendo apenas no GitHub.

## [0.4.2] - 2026-04-15

### Added

- **`skill/duckduckgo-search-cli-pt/SKILL.md`** e
  **`skill/duckduckgo-search-cli-en/SKILL.md`** — Skills bilíngues para Claude
  Code, Claude Agent SDK e plataformas compatíveis com Agent Skills. Cada
  skill traz frontmatter YAML com `name` único por idioma e `description`
  carregado de triggers semânticos para auto-invocação, além de 14 seções
  H2 canônicas (Missão, Contrato de Invocação, Proibições Absolutas,
  Parsing com `jaq`, Schema JSON, Exit Codes, Batch, Fetch-Content,
  Endpoint, Retries, Receitas, Validação, Memória, Regra de Ouro).
  Publicadas no GitHub, excluídas do tarball do crates.io.

### Changed

- **`docs/AGENT_RULES.md`** (833 linhas, +7,6%) — Reescrita editorial
  aplicando copywriting AIDA: cada regra abre com benefício mensurável,
  linguagem imperativa MUST/NEVER reforçada, zero narrativa decorativa,
  zero negrito com asteriscos duplos, zero separador visual `---` entre
  seções. Bilíngue EN+PT espelhado com tom idêntico.
- **`docs/COOKBOOK.md`** (1082 linhas, −3,1%) — Cada receita abre com o
  ganho concreto antes do comando, bullets curtos de 8 a 15 palavras,
  pipelines `jaq` + `xh` + `sd` preservados intactos.
- **`docs/INTEGRATIONS.md`** (1212 linhas, +1,3%) — 16 agentes com tabela
  comparativa textual, snippets determinísticos por agente, zero emoji.

### Meta

- `Cargo.toml` exclude ampliado para cobrir `skill/` e `skill/**` — skills
  ficam no GitHub e fora do tarball publicado em crates.io.

## [0.4.1] - 2026-04-14

### Added

- **`docs/AGENT_RULES.md`** (773 linhas) — Regras imperativas bilíngue (EN+PT)
  com 30+ rules `MUST`/`NEVER` (R01..R30) para LLMs/agentes invocarem a CLI
  em produção. Cobre: invariantes core, contrato JSON, rate limiting, error
  handling, performance, segurança, anti-patterns. Quick Reference Card no
  final.
- **`docs/COOKBOOK.md`** (1117 linhas) — 15 receitas copy-paste bilíngue
  combinando `duckduckgo-search-cli` + `jaq` + `xh` + `sd` para casos reais:
  research consolidado, ETL multi-query, extração de domínios, monitoramento
  com filtro temporal, content extraction com `--fetch-content`, comparação
  top 5 vs top 15, NDJSON para pipelines, function wrappers para bash.
- **`docs/INTEGRATIONS.md`** (1196 linhas) — Snippets prontos para 16
  agentes/LLMs: Claude Code, OpenAI Codex, Gemini CLI, Cursor, Windsurf,
  Aider, Continue.dev, MiniMax, OpenCode, Paperclip, OpenClaw, Google
  Antigravity, GitHub Copilot CLI, Devin, Cline, Roo Code. Cada agente
  documenta: pitch, mecanismo de shell, setup, snippet básico, snippet
  multi-query, system prompt rule, caveats.
- Seção **Documentation** no README.md (EN + PT) linkando os 3 guias.

### Fixed

- README.md badge cluster e referências internas conferidas contra
  `daniloaguiarbr/duckduckgo-search-cli` (repo canônico).

## [0.4.0] - 2026-04-14

### Changed (BREAKING)

- **Default de `--num` / `-n`**: alterado de "todos os resultados da primeira
  página" (~11) para **15**, com **auto-paginação** automática. Quando o
  número efetivo excede 10, o binário agora busca **2 páginas** por query
  para satisfazer o teto solicitado, desde que `--pages` não tenha sido
  customizado pelo usuário.
- **Auto-paginação automática**: se `--num > 10` (seja porque o usuário
  passou explicitamente ou porque o default 15 foi aplicado) E `--pages`
  não foi customizado (continua no default 1), o binário auto-eleva
  `--pages` para `ceil(num/10)` respeitando o teto de 5 páginas validado
  por `validar_paginas`. Impacto: mais requests por query (2x no caso
  default) e latência marginalmente maior, porém com cobertura completa
  dos resultados solicitados.

### Added

- Documentação no comentário do flag `--num` em `cli.rs` descrevendo a
  nova semântica de default e auto-paginação.
- 4 novos testes unitários em `lib.rs::testes`:
  `montar_configuracoes_aplica_default_num_15_quando_omitido`,
  `montar_configuracoes_respeita_pages_explicito_acima_de_1`,
  `montar_configuracoes_auto_pagina_quando_num_maior_que_10`,
  `montar_configuracoes_nao_auto_pagina_quando_num_10_ou_menos`.
- 2 novos testes wiremock em `tests/integracao_wiremock.rs`:
  `testa_default_num_15_auto_pagina_2_paginas`,
  `testa_auto_paginacao_respeita_pages_explicito`.

### Migration Guide

- **Quem quer o comportamento antigo** (1 página, ~11 resultados):
  passe `--pages 1 --num 10` explicitamente. O `--pages 1` explícito é
  indistinguível do default (trade-off aceito: `paginas > 1` é o único
  sinal de "customização"), então o mais seguro é combinar com `--num 10`
  para garantir que nada será auto-paginado.
- **Quem já passava `--num 5`** (ou qualquer valor <= 10): comportamento
  **inalterado** (sem auto-paginação, 1 página).
- **Quem já passava `--num 20 --pages 2`** ou similar: comportamento
  **inalterado** (respeita explícito do usuário).
- **Quem confiava no default sem flags**: agora recebe até 15 resultados
  em vez de ~11, com 1 request extra por query. Para restaurar o antigo,
  passe `--pages 1 --num 10`.

## [0.3.0] - 2026-04-14

### Changed (BREAKING)

- **Schema JSON**: campo `buscas_relacionadas` REMOVIDO de `SearchOutput` e
  `MultiSearchOutput.buscas[i]`. O endpoint `html.duckduckgo.com/html/` não
  expõe related searches no DOM atual; manter o campo sempre vazio era ruído.
  Pipelines que parseavam `.buscas_relacionadas` precisam ajuste.
- **Pool de User-Agents**: removidos UAs de browsers de texto (`Lynx 2.9.0`,
  `w3m/0.5.3`, `Links 2.29`, `ELinks 0.16.1.1`) que faziam DuckDuckGo retornar
  HTML degradado. Substituídos por 6 UAs modernos validados empiricamente
  contra o `/html/` endpoint: Chrome 146 (Win/Mac/Linux), Edge 145 Windows,
  Firefox 134 Linux, Safari 17.6 macOS. Firefox Win/Mac foram REMOVIDOS após
  retornarem HTTP 202 anomaly em validação real (heurística anti-bot do DDG).

### Fixed

- **Snippet duplicava título e URL no início**: o seletor padrão tinha
  fallback `.result__body` (container pai) que fazia `text()` recursivo
  capturar título+URL+snippet concatenados. Trocado por `.result__snippet`
  puro. Pipelines como `jaq '.resultados[].snippet'` agora retornam apenas
  o texto descritivo do resultado.
- **Título "Official site"**: DuckDuckGo renderiza literalmente este texto
  como label para domínios verificados (ex: prefeituras). O scraper agora
  detecta este caso e substitui pelo `url_exibicao` (ex: `saofidelis.rj.gov.br`).
  O texto original é preservado no novo campo opcional `titulo_original`
  para auditoria.

### Added

- Campo `titulo_original: Option<String>` em `SearchResult`. Presente
  apenas quando o título foi substituído por heurística (atualmente: caso
  "Official site"). Serializado com `#[serde(skip_serializing_if = "Option::is_none")]`
  — não aparece no JSON quando ausente.
- Resultados patrocinados (`.result--ad`) excluídos do container default
  via seletor `.result:not(.result--ad)`.

### Removed

- Função `extrair_buscas_relacionadas` em `src/search.rs` (dead code com
  seletor hardcoded que nunca encontrava nada).
- Seção `[related_searches]` em selectors default.

### Migration Guide (v0.2.x → v0.3.0)

- Pipelines `jaq '.buscas_relacionadas[]'`: campo não existe mais.
  Remover do filtro ou tratar `null`.
- Esperando snippet com prefixo título+URL? Agora vem só o texto descritivo
  — ajuste regex/parsing downstream se necessário.
- Confiando em `titulo == "Official site"` para detectar sites verificados?
  Use `titulo_original.as_deref() == Some("Official site")`.
- **CONFIG EXTERNO LEGADO**: usuários que rodaram `init-config` em versões
  anteriores possuem `~/.config/duckduckgo-search-cli/{selectors,user-agents}.toml`
  com defaults antigos (snippet com `.result__body` + UAs `Lynx`/`w3m`/etc.).
  Esses arquivos OVERRIDE os defaults embutidos. Para aplicar as correções
  desta versão, execute APÓS atualizar:
  ```
  duckduckgo-search-cli init-config --force
  ```
  O flag `--force` sobrescreve os arquivos externos. Backup recomendado se
  você editou manualmente para hotfix de seletores.

## [0.2.0] - 2026-04-14

### Changed (BREAKING)

Schema JSON serializado agora usa nomes de campo em **português brasileiro**,
alinhado com os exemplos `jaq` do README e com o invariante INVIOLÁVEL do
blueprint v2 do projeto ("Logs e nomes de campo em português brasileiro").

Pipelines que dependiam do schema em inglês da `v0.1.0` precisam atualizar
os seletores `jaq`. Tabela de renomeações:

| Antes (v0.1.0) | Depois (v0.2.0) |
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

Campos inalterados: `url`, `snippet`, `query`, `endpoint`, `timestamp`, `user_agent`.

### Fixed

- Pipelines documentados no README (`jaq '.resultados[].titulo'`, etc.) agora
  funcionam end-to-end. Em `v0.1.0` retornavam `null` por divergência do schema
  (bug reportado pelo usuário).

### Added

- `LICENSE-MIT` and `LICENSE-APACHE` (dual-licensed per `Cargo.toml`, aligning the tarball with the SPDX declaration).
- `.pre-commit-config.yaml` with three hook groups: (1) pre-commit-hooks standard (trailing whitespace, EOF, YAML/TOML validity, mixed line endings), (2) Rust hooks (`cargo fmt` + `cargo clippy -D warnings`), (3) local `commit-msg` hook blocking `Co-authored-by:` from AI agents (mirrors the CI `commit_check` job). Reduces CI round-trips for trivial violations.
- `.gitattributes` forcing LF on `.rs` / `.toml` / `.sh` / `.yml` / `.md` / fixture HTML — prevents silent corruption when cloning on Windows with `core.autocrlf=true` (which would otherwise break shebangs, rustfmt, and content-extraction tests). Binary extensions (`.png`, `.woff2`, etc.) marked explicitly. `Cargo.lock` and `target/` flagged `linguist-generated` to exclude from GitHub language stats.
- `.editorconfig` normalizing UTF-8, LF, trailing-whitespace trim, and per-language indent (Rust/TOML 4, YAML/JSON/MD 2, Makefile tab) across VS Code, RustRover, vim, and other editors — eliminates spurious formatting diffs caused by per-dev settings drift.
- `.github/PULL_REQUEST_TEMPLATE.md` with the 10-gate checklist + project-specific constraints (no cache, no MCP, rustls-only, `println!` confined to `output.rs`, PT-BR identifiers).
- `.github/ISSUE_TEMPLATE/bug_report.yml` + `feature_request.yml` + `config.yml` — structured triage with platform dropdown (glibc/musl/NixOS/Flatpak/Snap/macOS ARM/macOS Intel/Windows/WSL), install method, and constraint verification. `config.yml` redirects security reports to Security Advisories and usage questions to Discussions.
- `Cross.toml` enabling `cross build --target <t>` for ARM64/ARMv7 Linux targets (musl + glibc + hard-float) from any x86_64 host with Docker/Podman — complements the native CI pipeline for developers without a GitHub Actions runner.
- `CONTRIBUTING.md` with the 10-gate validation matrix, coding standards (Brazilian Portuguese identifiers, rustls-only TLS, `output.rs` as the sole `println!` site), three-layer testing strategy, supply-chain guardrails, and the tag-driven release process.
- `.cargo/config.toml` exposing 8 developer aliases (`cargo check-all`, `cargo lint`, `cargo docs`, `cargo test-all`, `cargo cov`, `cargo cov-html`, `cargo publish-check`, `cargo pkg-list`) — each mirrors a CI job for local reproduction.
- Doctests in public API: `pipeline::combine_and_dedup_queries`, `content_fetch::extract_host`, and `search::format_kl` — compilable examples on docs.rs that double as regression tests.
- `SECURITY.md` documenting the private-disclosure workflow via GitHub Security Advisories, response SLA (72 h), scope (HTTP/HTML parsing, credential leaks, path traversal, TLS) and security design assumptions (stateless, rustls-only, no JS for search).
- `.github/dependabot.yml` enabling weekly automatic dependency updates for both `cargo` and `github-actions` ecosystems, with semantic grouping (dev-deps, tokio-ecosystem, tracing-ecosystem) and PR count limits.
- `rust-toolchain.toml` pinning `stable` with `rustfmt` + `clippy` components for reproducible dev/CI builds.
- `.github/workflows/release.yml` triggered by `v*.*.*` tags (and `workflow_dispatch` with `dry_run`) running the 5-stage release pipeline per `rules_rust.md` §19: validate → build_matrix (5 targets) → macos_universal (lipo) → github_release (with generated notes) → crates_io (publish gated on `CRATES_IO_TOKEN` secret).
- `msrv` job in `ci.yml` extracting `rust-version` from `Cargo.toml` and running `cargo check` on that toolchain to detect MSRV drift on every PR.
- `.github/workflows/ci.yml` enforcing the 10-gate validation matrix across Ubuntu, macOS, and Windows:
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

[Unreleased]: https://github.com/comandoaguiar/duckduckgo-search-cli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/comandoaguiar/duckduckgo-search-cli/releases/tag/v0.1.0
