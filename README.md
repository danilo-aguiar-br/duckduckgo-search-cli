# duckduckgo-search-cli

> **Language:** this file is the **English SSOT**. Brazilian Portuguese lives **only** in [`README.pt-BR.md`](README.pt-BR.md) — this English README does **not** embed a Portuguese monolith.

[![docs.rs](https://img.shields.io/docsrs/duckduckgo-search-cli)](https://docs.rs/duckduckgo-search-cli)
[![crates.io](https://img.shields.io/crates/v/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![License](https://img.shields.io/crates/l/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-orange)](https://github.com/danilo-aguiar-br/duckduckgo-search-cli)
[![Downloads](https://img.shields.io/crates/d/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-blue)](https://www.rust-lang.org)

> Web search at terminal speed — fresh, structured web context for your AI agent.

[Read in Portuguese](README.pt-BR.md)

<!--
SEO keywords: duckduckgo cli, search cli rust, llm web search tool, ai agent search,
claude code search, gemini cli search, codex search tool, headless web search,
json search results cli, parallel search rust, rust web search, cli web grounding,
aider search, cursor search tool, continue dev search, devin search, cline search,
retrieval augmented generation cli, rag cli, no api key search, ddg cli, tokio search cli,
rustls search cli, ndjson search stream, agent shell tool, mcp adjacent search cli
-->

## Quick Install
- Install with one command via cargo:

```bash
cargo install duckduckgo-search-cli --locked
```
## Why this exists

Every modern LLM carries a knowledge cutoff, and every autonomous agent eventually needs something its weights never saw: the latest library version, a 2026 incident post-mortem, a vendor's current pricing page. Bolting on a hosted search API costs money, leaks queries, and breaks when the vendor rate-limits you in the middle of a multi-step plan.

`duckduckgo-search-cli` is a single Rust binary that turns any shell into a first-class search tool. No API key. No tracking. Chrome-powered search that runs invisibly. A stable JSON schema, bounded concurrency, and predictable exit codes — exactly what an agent needs to ground itself in real-time web data without becoming a liability.

## Superpowers for every AI agent

Drop this binary into any agent that can run a shell command. That is nearly every serious agent shipping today.

| Agent                  | How it benefits                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------- |
| Claude Code            | `Bash` tool invokes `duckduckgo-search-cli "query" --num 15 -q \| jaq '.results'` for grounded research before edits. |
| OpenAI Codex           | Shell access feeds structured JSON into the context window for up-to-date library docs.           |
| Gemini CLI             | Pipe results into Gemini's JSON mode for synthesis of long-tail web facts.                        |
| Cursor                 | Declare the binary in `.cursorrules` and let the AI call it whenever it lacks context.            |
| Windsurf               | Cascade agents call the CLI as a deterministic tool for fresh web data.                           |
| Aider                  | Use `/run` to inject search results straight into the edit conversation.                          |
| Continue.dev           | Register as a custom slash-command to ground in-IDE completions.                                  |
| MiniMax                | Shell-tool integration gives MiniMax agents DuckDuckGo-backed global coverage.                    |
| OpenCode               | Model-agnostic shell integration — works with any provider OpenCode is pointed at.                |
| Paperclip              | Drop-in primitive for autonomous research pipelines.                                              |
| OpenClaw               | Open-source agent runtime — use it as the default search backend.                                 |
| Google Antigravity     | Headless CLI fallback for the surface-level browser agent when direct scraping is needed.         |
| GitHub Copilot CLI     | `gh copilot` workflows that require verified URLs and fresh references.                           |
| Devin                  | Autonomous engineer reads the JSON output to plan before touching code.                           |
| Cline                  | VS Code agent calls the binary as a terminal command for grounded answers.                        |
| Roo Code               | Roo Cline fork — same zero-config shell integration.                                              |

## Why it's perfect for AI agents

- **JSON-first by default (v1.0.2 EN wire).** Stable schema with `results[]` and `metadata` (English serialize — [ADR-0027](docs/decisions/0027-wire-en-default-v1-0-2.md)); legacy PT keys via `--wire-keys pt`. Ready for `jaq` and direct parsing into tool calls.
- **Zero API key, zero tracking.** Drives a real Chrome over CDP against DuckDuckGo (Chrome-only since GAP-WS-113; no Chrome means exit 2, never a silent HTTP downgrade). No authentication to rotate, no dashboard to babysit, no data leak surface.
- **Parallel by design.** `--parallel 1..=20` fans out multiple queries through a `tokio::JoinSet`, and `--per-host-limit` prevents burst abuse when `--fetch-content` is on.
- **15 results by default.** Generous context for LLMs without forcing you to spell out `--num`. Override per call when you need to.
- **Auto-pagination that just works.** Default `--pages` is **1**. When `--num` needs more results than a single DuckDuckGo page, the CLI may crawl additional pages up to `--pages` (max **5**) so you get the count you asked for — raise `--pages` explicitly when you need more.
- **Readable body extraction (default ON since v0.9.8).** Downloads top URLs (web + news, cap 4 in v1.0.2) and embeds cleaned text in JSON; opt out with `--no-fetch-content`.
- **Cross-platform single binary.** Linux (glibc, musl/Alpine), macOS Intel + Apple Silicon Universal, Windows MSVC — all from one `cargo install`.
- **Native Chrome transport (v0.8.0+ / ADR-0016 / ADR-0022 / ADR-0026).** Production SERP uses the **host Chrome TLS stack** so the CLI does **not** present a library TLS bot-class signature (`rustls` JA4) that Cloudflare blocks. **No** synthetic hardware fingerprint spoof (canvas/WebGL/audio). **Always muted** since **v1.0.2** (`--mute-audio` + autoplay policy — ADR-0026; no unmute) so deep-research never plays page sound on host speakers. Residual HTTP (harness) = rustls + `aws-lc-rs` (ADR-0021). v0.8.7+ Xvfb + automation-signal mitigation; v0.9.3+ headless=new on macOS/Windows.
- **NDJSON streaming.** `--stream` (or `-f ndjson` as multi-query stream alias) emits one line per result the moment it arrives, feeding reactive pipelines without buffering the whole response. Early consumer close → exit **141** with one-shot Chrome reap still running (v1.0.1).
- **Hardened exit codes.** Distinct codes for runtime errors, bad config, soft rate-limit, global timeout, zero-results, and broken pipe (**141**) — so agents can branch deterministically.
- **v0.5.0 security hardening.** Path traversal validation on `--output` rejects `..` and system directories; proxy credentials masked in error messages; typed errors via `CliError` (`src/error/cli_error.rs`) with 21 deterministic variants.
- **v0.6.0 anti-blocking.** Per-browser `Sec-Fetch-*` headers and Client Hints for Chrome/Edge; `Accept-Language` with RFC 7231 q-values; HTTP 202 anomaly detection; silent block detection with 5 KB threshold.
- **v0.9.6 / v1.0.0 / v1.0.1 one-shot ownership.** Agents can run N sequential invocations without accumulating orphan Chromium/Xvfb. Since **v1.0.0**, Chrome profiles use the auditable prefix `ddg-chrome-*` (not generic `.tmp*`) and are removed on cooperative exit; next-run sweep cleans only that prefix. **v1.0.1** hardens pipe-safe reap (`ensure_oneshot_cleanup`, SIG_IGN on SIGPIPE) so early `| head` / BrokenPipe still reaps `ddg-chrome-*`. Full tree+disk reap on success, error, timeout, SIGINT, SIGTERM, and broken pipe. See ADR-0017 + ADR-0020.
- **v1.0.2 agent ops.** Project and shape stdout without `jaq`: `--fields`/`--select`, `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only`, `--truncate-content`, `--max-output-bytes`, plus `--wire-keys en|pt`.

## Agent Skill — bundled, bilingual, auto-activating

Stop writing system prompts that remind your agent to search. This repo already ships a pre-built Claude Agent Skill, and Claude picks it up automatically the moment a user mentions research, verification, fresh docs or URL grounding — with no prompt engineering on your side.

- **Two production-grade skills live in this repo.** `skills/duckduckgo-search-cli-en/SKILL.md` and `skills/duckduckgo-search-cli-pt/SKILL.md` — English and Brazilian Portuguese, each with a unique `name` field so both can coexist in the same Claude install.
- **Auto-activation, straight out of the box.** The `description` field is front-loaded with the triggers users actually type ("search the web", "ground this", "verify this URL", "pesquise online", "traga resultados atualizados"). Claude matches on semantics — no slash command, no tool registration.
- **16 normative sections per skill** (counted as H2 + H3 headings in `skills/duckduckgo-search-cli-en/SKILL.md`). Mandatory `-q -f json` contract, `jaq` parsing, deterministic exit codes, batch mode, content extraction, endpoint fallback, retries, post-validation — the agent reads this once and stops inventing flags forever.
- **Token-efficient by design.** One ~1,000-word skill replaces a sprawling system prompt. Loaded once per session, referenced every time — the contract is not re-explained on every search turn.
- **Fail-closed by construction.** Every flag the agent might invoke is documented inside the skill with a frozen JSON contract; unknown flags are refused with exit 2 instead of being silently ignored, and a zero-result run exits 5/6 instead of returning invented rows.
- **Installs in one command.** Copy the folder into your Claude config and you are done — the skill lives on GitHub, not in the crates.io tarball, so always pull the freshest version from `main`.

```bash
# One-shot install (clone and copy whichever language you prefer).
git clone https://github.com/danilo-aguiar-br/duckduckgo-search-cli
cp -r duckduckgo-search-cli/skills/duckduckgo-search-cli-en ~/.claude/skills/
cp -r duckduckgo-search-cli/skills/duckduckgo-search-cli-pt ~/.claude/skills/

# Restart Claude Code (or reload the Agent SDK). That is the whole setup.
```

## Documentation

Three deep-dive guides ship with the crate. Read them once — they pay back forever.

| Guide | Why it matters |
|-------|---------------|
| [`docs/AGENT_RULES.md`](docs/AGENT_RULES.md) | 109 MUST/NEVER rule bullets for any LLM/agent invoking this CLI in production, counted as list items carrying MUST or NEVER. Bilingual EN+PT. |
| [`docs/COOKBOOK.md`](docs/COOKBOOK.md) | 39 English plus 19 Portuguese copy-paste recipe sections for research, ETL, monitoring, content extraction, counted as headings opening with `Recipe` or `Receita`. Bilingual EN+PT. |
| [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) | Drop-in snippets for 16 agents: Claude Code, Codex, Gemini CLI, Cursor, Windsurf, Aider, Continue.dev, MiniMax, OpenCode, Paperclip, OpenClaw, Antigravity, Copilot CLI, Devin, Cline, Roo Code. |

## Cargo features
| Feature | Default | Description |
|---------|---------|-------------|
| `chrome` | **yes** | Production network via real Chrome (`chromiumoxide`/CDP). Required for SERP, news, deep-research, probe, pre-flight, and content fetch. |
| `http-test-harness` | no | Residual HTTP SERP/probe for tests only (`DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`). Never a silent production SERP path. |
| `console` | no | `tokio-console` subscriber for runtime task debugging. |

Install defaults already enable `chrome`: `cargo install duckduckgo-search-cli --locked`. Docs.rs builds with `all-features = true` and multiplatform `targets` (see `Cargo.toml` `[package.metadata.docs.rs]`).

## Prerequisites (v0.8.7+)
- Google Chrome or Chromium (auto-detected via `detect_chrome()`)
- Linux: Xvfb auto-installed by the CLI via `try_auto_install_xvfb()` for 22+ distros (Fedora, Ubuntu, Debian, Arch, openSUSE, Alpine, Void, Gentoo, Amazon Linux, and derivatives)
- macOS/Windows: no extra dependency — Chrome runs in headless=new mode since v0.9.3
- Chrome is the ONLY production network transport since v0.9.4 (GAP-WS-113) — chromiumoxide/CDP for search, news, deep-research, `--probe`, `--probe-deep`, `--pre-flight`, and `--fetch-content`
- Residual HTTP/reqwest lives only under feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` (plus cookie/UA helpers) — never a silent production SERP path
- Without usable Chrome (or a binary built without feature `chrome`) → **exit 2 fail-closed** (no auto `--no-news`, no Web downgrade, no silent HTTP). Product env `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` is **removed** / not read — use CLI / rebuild without `chrome` only for non-production.
- Feature `chrome` is default; `--allow-lite-fallback` is a **legacy no-op** (SERP stays HTML Chrome)
- v0.8.7: `has_native_display()` detects native display per platform before deciding headed vs headless
- v0.8.7+: Linux runs Chrome HEADED inside a private Xvfb display (ZERO visible windows); v0.9.3 switched macOS/Windows to headless=new
- v0.8.7: warm-up navigation to duckduckgo.com before search URL (Cloudflare cookie pre-load)
- v0.8.7: UA↔Chrome process coherence — only Chrome UA when browser is Chromium (`chrome_only_ua_for_platform()`)
- v0.8.7 / ADR-0022: CDP automation-signal mitigation only (`webdriver`, plugins, `window.chrome`, outer size, Permissions, CDP leak prevention) — **no** canvas/WebGL/audio hardware fingerprint spoof
- Fallback cascade: Xvfb private → auto-install Xvfb → native headed → headless (last resort with warning)
- Chrome head mode: CLI flags `--chrome-visible` (debug) / `--chrome-headless` (force headless). Product envs `DUCKDUCKGO_CHROME_*` are **removed** — use CLI flags only.
- **One-shot process + disk contract (v0.9.6 process / v1.0.0 disk / v1.0.1 pipe-safe):** each invocation owns its Chromium tree, private Xvfb (Linux), and profile under **`ddg-chrome-*`** (Unix `0o700`). On success, error, timeout, SIGINT, SIGTERM, or **BrokenPipe (exit 141)** the CLI reaps the full tree via `ensure_oneshot_cleanup` (process group + PID tree + unique `user-data-dir` marker) and **`remove_dir_all`s the profile**. SIGPIPE stays **SIG_IGN** so Drop/reap still run when `| head` closes early. No automation browser or Xvfb from **this** run may survive cooperative exit. Next run sweeps stale **`ddg-chrome-*` only** — never bulk-deletes foreign `.tmp*` or `org.chromium.Chromium.*`. No remote telemetry. Residual: SIGKILL/OOM of the CLI is not interceptable (Xvfb may still die via `PR_SET_PDEATHSIG` on Linux); pre-0.9.6 process orphans and pre-1.0.0 generic `.tmp*` profiles are not mass-auto-cleaned. See ADR-0017 + ADR-0020.

## What's new in v1.0.6 (2026-08-21)
- **The release gate now ends at the registry, not at the package.** Every gate answered "does the tree compile?"; none answered "does the version the registry *serves* compile?". v1.0.5 was published and then yanked, and because `max_stable_version` is derived from yank state, that promoted the broken v1.0.2 back to being the default — eleven days after the fix had shipped, with no signal anywhere. New `verify_published` binary (ADR-0032) runs after `cargo publish` **and after every `cargo yank`**.
- **The shipped profile did not compile with its tests.** `cargo check --no-default-features --features chrome --all-targets` exited 101 with 91 errors, three of them `E0432` — the same code as the defect that shipped in v1.0.2, hiding in the test tree. Nothing caught it because `cargo test-all` uses `--all-features`, where the harness is always on.
- **Two stealth mitigations were silently inert.** `--disable-features` was passed twice with different values, and Chromium keeps only one — so `AutomationControlled` never reached the browser. The WebRTC entry was inverted and would have re-exposed the local IP the moment the first bug was fixed; both are corrected together.
- **A panic reachable from any accented SERP.** The body was truncated at a raw byte index; slicing `str` mid code point panics, and every accented pt-BR character is two bytes.
- **`config set A B --key C` silently discarded an operand**, writing to a key the caller never named. Now fails closed.
- **`--no-input` was declared and never read.** It now refuses stdin, as its name says.
- **RUSTSEC-2026-0258** (`h2` unbounded empty DATA frames) reached the default profile through `chromiumoxide`'s own `reqwest`, not through the optional harness. Updated to `h2 0.4.18`.
- Componentised `decomposition` (640 → 44 lines), `synthesis` (570 → 105), `extraction::web` (462 → 143), and split the search validators out of `buscar_args`, deduplicating four identical range checks into one.

## What's new in v1.0.5 (2026-08-10)
- Close the class by ruler, not by list — `--probe` and `--probe-deep` still ignored every agent-native operator after v1.0.4.
- Measure the defect: on v1.0.4 `--count-only`, `--limit 1`, `--fields status` and `--truncate-content 5` each returned 633 bytes against a 633-byte baseline, at exit 0.
- Route the probe through the projector and return the exit code from `emit_probe`, so a refusal reaches the caller instead of being swallowed by `let _ = …`.
- Treat a string the agent hands back to a program as identity, not content, so `--truncate-content` no longer mutilates config keys, locale tags, schema ids, paths or probe error codes.
- Refuse `--truncate-content` with exit 2 on `config list`, `config path`, `config get/set/unset`, `config effective` and `locale`, where every string is an identifier.
- Unify refusal on `output::emit_envelope_or_refuse` — `{"error", "message"}` on stdout, localized prose on stderr, exit code unchanged.
- Rename the published matrix field to `agent_ops[].discriminator_key`, because every envelope carries the key `type` and the old field carried the value.
- Add `tests/integration_stdout_boundary.rs` and `tests/integration_agent_ops_matrix.rs` as rulers that fail the build on a new bypass or a stale exemption.
- Promote four probe ceilings to XDG keys `probe_launch_timeout_seconds`, `probe_extract_timeout_seconds`, `probe_deep_launch_timeout_seconds` and `probe_deep_extract_timeout_seconds`.
- Translate the eight refusal sentences into `en` and `pt_br`, while the stdout `message` stays English on purpose.
- Declare the agent-native flags once via `#[command(flatten)] AgentOpsArgs`, and honour `--fields` on a search that timed out.
- Remove `docs/generated/flag-desc-{en,pt}.json`, the last orphans of the Python generator deleted in v1.0.4.
- Implement `--truncate-content` on `deep-research`, where the function had an empty body while its three siblings cut.
- Sweep orphaned Chrome by the `ddg-chrome-*` marker on macOS and Windows, replacing an empty `cfg` block and a no-op.
- Pin `html_root_url` to the shipped version with a ruler, instead of leaving it frozen three releases back.
- Stop reading the Cargo test variable `CARGO_BIN_EXE_timeout` in production logging.
- Measure Portuguese on four axes — comments, assertion and `tracing` prose, Rust identifiers and EN/PT hybrids — from one SSOT in `tests/common/language.rs`.
- Suffix `--version` with `-dirty` when the working tree is not clean, so two different binaries stop reporting one identity.
- Add `cargo docs-nohttp`, which runs rustdoc on the DEFAULT feature set and caught six intra-doc links `cargo docs` could not see.
- Pin `rust-toolchain.toml` to the declared MSRV with a ruler, so a drifting channel cannot keep the gates green.
- Localize the error BODY under `--ui-lang pt-BR`, by making `localized_detail` exhaustive and routing the nine `run.rs` emission sites through it.
- Extend the flag ruler to `llms-full.txt` and to subcommand-exclusive flags, closing a 27-flag drift and a wrong `--global-timeout` default.
- Consume aggregation input by value and escape TSV in a single pass, removing 26 clones per loop and five copies per cell.

## What's new in v1.0.4 (2026-08-09)
- Abolish "flag accepted and ignored" — every agent-native operation now either acts or refuses by name, with no third outcome.
- Apply `--fields` and `--truncate-content` on any JSON object, and refuse the five row operators with exit 2 where the surface declares no row array.
- Declare the row array per surface instead of inferring it, because `doctor` carries `checks` and `failed_checks` and `config effective` carries `allowed_keys` and `precedence`.
- Publish the capability matrix in `commands` under `agent_ops`, so a caller learns the contract instead of collecting exit codes.
- Measure the result: `commands` 6421 → 47 bytes with `--fields version`, `doctor` 2524 → 38 with `--fields type,status`, `schema` 4726 → 1107 with `--fields schemas.id`.
- Fix three fields that emitted Portuguese keys under the English default wire — `AggregatedItem.display_url`, `AggregatedNewsItem.source` and `AggregatedNewsItem.relative_date`.
- Give the five `config` shapes, `locale` and `init-config` a real `type` discriminator, so every published schema either routes or declares why it does not.
- Delete `scripts/regen_cli_flags_readme.py` and replace the generator with `tests/integration_docs_drift.rs`, a ruler that fails on divergence instead of rewriting on demand.
- Propagate the `--fields` / `--filter` parse error on the multi-query stream path, which previously dropped it with `.ok()`.

## What's new in v1.0.3 (2026-08-07)
- Fix the cross-platform hotfix — the crate did not build on macOS or Windows in v1.0.2.
- Split the ungated `use` in `src/browser/session/mod.rs`, because Rust strips `cfg`-disabled items before name resolution (`E0432`).
- Fix `E0308` twice in `src/browser/detect.rs`, which matched `std::env::var_os` with `if let Ok(..)` on a Windows-only path no gate had ever compiled.
- Gate the 13 off-Linux warnings that `-D warnings` rejected.
- Rebuild `tests/integration_content_fetch.rs` on `common::lean_config`; it had stopped compiling on every platform (17 errors).
- Add `cargo check-windows` / `cargo lint-windows`, `scripts/check-macos.sh` and `scripts/portability-lint.sh` as local gates required before tag and `cargo publish`.
- Record in [`ADR-0028`](docs/decisions/0028-local-cross-platform-gate-v1-0-3.md) why forbidding remote CI moves cross-platform verification onto the host instead of removing it.

## What's new in v1.0.2 (2026-07)
- **Wire JSON English by default (ADR-0027)** — serialize keys are English: `results`, `title`, `metadata`, `result_count`, `used_chrome`, `chrome_channel`, `chrome_path_resolved`, `execution_time_ms`, `news`, `searches`, … Deserialize still accepts Portuguese aliases. **Migration guide:** [`docs/MIGRATION.md`](docs/MIGRATION.md) · [PT-BR](docs/MIGRATION.pt-BR.md).
- **`--wire-keys en|pt`** + XDG `wire_keys` — opt-in legacy Portuguese keys at the emit boundary (`--wire-keys pt` or `config set wire_keys pt`).
- **Agent ops (no jq required)** — `--fields`/`--select`, `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only`, `--truncate-content`, `--max-output-bytes` (plus XDG defaults).
- **RuntimeConfig SSOT** — `src/runtime/` precedence **CLI > XDG > FACTORY**; `config` is CRUD-only; `config effective` shows the merge.
- **`budget_profile`** — `lab` | `desktop_contended` | `thin` via `config set budget_profile …`.
- **`chrome_session_retries`** — CLI + XDG process-wide SERP launch retries.
- **no-warmup fail-closed** — `--no-warmup` requires hidden `--allow-no-warmup` or XDG `allow_no_warmup=true` (lab only).
- **Linux cgroup opt-in** — XDG `linux_cgroup_enabled` + `linux_cgroup_memory_max_mb` (doctor reports `linux_cgroup`; n/a non-Linux).
- **Chrome always muted (ADR-0026)** — every launch path passes effective `--mute-audio` + autoplay policy (MUTE-002 fixed quad-dash argv). No unmute.
- **Deep-research budget dual/contention (ADR-0025)** — wall-clock estimate models dual multiproc vs sequential dual and host Chrome contention; fail-fast exit 2 on `budget_underflow` (escape: `--allow-under-budget`).
- **`--print-budget`** — dry estimate JSON without Chrome (QUERY optional); emits `suggested_global_timeout`, `shell_timeout_hint`, `runtime_dual_multiproc`, `chrome_n`.
- **`--auto-contention-budget`** (default ON) raises effective global timeout; `--no-auto-contention-budget` for strict fail-fast.
- **`doctor` dual readiness** — reports `ready_for_dual_deep_research`, `recommended_global_timeout`, dual-preserving remediations.
- **Defaults agent-happy under `timeout 180`** — `--max-sub-queries` default **3**, `--fetch-content-cap` default **4**; grace timeout deep **20s**.
- **Partial agent fields** — timeout/deep envelopes carry `partial` / `sub_queries_*` counters (agent contract, no phone-home).
- Residual honesty: deep-research schema metadata may lag live envelope fields (`partial` / `sub_queries_*`); treat schemas as best-effort.

## What's new in v1.0.1 (2026-07-19)
- **Pass 48 DR contract + Pass 52 oneshot/stream/config** — deep-research honors `-o`, agent-stable timeout JSON, heuristic `--depth`, `-f tsv`, XDG `config`/`man` subcommands; no product env knobs; **no remote telemetry**.
- **Pipe-safe one-shot (Pass 52 / GAP-E2E-51-001/007)** — `ensure_oneshot_cleanup` on all exits including early pipe close; Unix **SIG_IGN** for SIGPIPE (not SIG_DFL) so Chrome Drop/reap still runs; stream `BrokenPipe` → exit **141**.
- **`-f ndjson`** accepted as alias for multi-query stream mode (`--stream`).
- **`config` dual API** — `config get KEY` **or** `config get --key KEY`; `config set KEY VALUE` **or** `config set --key KEY --value VALUE`; also `config unset` dual form and **`config effective`** (merged CLI+XDG+defaults JSON).
- **Wire JSON (ADR-0023, superseded serialize default by v1.0.2)** — Portuguese field names on **serialize** in 1.x (BC for agents); English `serde` aliases on **deserialize**. **v1.0.2** flips serialize default to EN — see above.
- **News vertical** — fixed false anti-bot classification on isolated news; residual real DDG anti-bot may still yield exit 6 environmentally (honest empty `news`, never synthetic).
- Product config via **CLI + XDG only** — do not teach product env vars such as `DUCKDUCKGO_ZERO_CAUSE_STRICT` or `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` as live knobs (historical migration notes mark them **removed**).

## What's new in v1.0.0 (2026-07-15)
- **GAP-WS-TMP-PROFILE-ORPHAN-001 RESOLVED (ADR-0020)** — honest one-shot on **disk** as well as process: Chrome `user-data-dir` uses prefix **`ddg-chrome-`** (not default `.tmp`); `force_reap` removes the profile directory; `ExitReapGuard` + panic hook + timeout/end-of-run reap.
- **Hard disk hygiene policy** — (1) never auto-rm generic `.tmp*`; (2) never auto-rm `org.chromium.Chromium.*`; (3) SIGKILL/OOM residual is cleaned on the **next** run via `sweep_orphan_profiles` **only** for `ddg-chrome-*`.
- **deep-research** inherits the main `CancellationToken` so SIGTERM cancels fan-out and disk reap can complete; `Config::default().global_timeout_seconds` aligned to 180.
- **Stable 1.0.0 contract** — Chrome-only CDP SERP, agent-ready defaults (0.9.8), e2e honesty (0.9.9), process+disk one-shot, atomwrite, **no remote telemetry**. No JSON schema break vs 0.9.10/0.9.9.
- Inventory: `gaps.md`; ADR: `docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md`.

## What's new in v0.9.8 (2026-07-14)
- **GAP-WS-AGENT-READY-001 RESOLVED (L-01…L-08)** — agent-ready defaults, multi-canal Chrome, dual web+news, clean text. ADR-0018; inventory `gaps.md`.
- **L-01/L-02 Multi-canal Chrome** — Flatpak export shell rejected → resolve real ELF under `…/files/extra/chrome`; Fedora Chromium wrappers resolved to lib64 ELF. Order: `--chrome-path` → `CHROME_PATH` → host Chrome → host Chromium → Flatpak → Snap. `needs_no_sandbox` for Flatpak deploy paths.
- **L-03 Dual default** — search default `--vertical` is **`all`** (web + news). Opt out with `--vertical web`. Deep-research already dual unless `--no-news`.
- **L-04 News SERP** — multi-selector poll; honest Chrome-usage flag on news-only / multi-query / deep / failure envelopes (v1.0.2 EN: `used_chrome`; legacy PT `usou_chrome` only with `--wire-keys pt`).
- **L-05 Clean text DEFAULT ON** — content fetch ON for **web + news** (FETCH_CAP=4 (v1.0.2; was 10 at v0.9.8)); opt out with **`--no-fetch-content`**. News may carry body fields when fetch is on (v1.0.2 EN: `content` / `content_size` / `content_extraction_method`; legacy PT with `--wire-keys pt`).
- **L-06 Transport flags `global = true`** — `--chrome-path`, `--proxy`, `--vertical`, fetch flags, identity, etc. work **after** `deep-research` (and before).
- **L-07 UA fan-out** — shared `coerce_chrome_user_agent`; one-shot lifecycle retained (0.9.6 / ADR-0017); Chrome-only production (0.9.4 / ADR-0016); atomwrite; **no remote telemetry**.
- **L-08 Docs/schemas/skills** — ADR-0018, versioned inventory, skills EN/PT, CHANGELOG.
- **Agent metadata (NOT telemetry)** — v1.0.2 EN wire: `chrome_path_resolved`, `chrome_channel`, honest `used_chrome` (historical PT `chrome_path_resolvido` / `chrome_canal` / `usou_chrome` only with `--wire-keys pt`).
- **Residuals** — anti-bot may still zero news (exit 6 + `zero_cause: anti-bot` after session prime + retries; never fake-success); SIGKILL OS limit; no separate `--agent` flag (defaults are agent-ready).

## What's new in v0.9.6 (2026-07-12)
- **GAP-WS-LIFECYCLE-001 closed** — true one-shot ownership of external process trees (Chromium multi-process + Xvfb + `TempDir`)
- **`src/process_lifecycle.rs`** — process-group spawn (`setpgid` + Linux `PR_SET_PDEATHSIG`), `killpg`, process-tree walk, cmdline marker kill by unique `user-data-dir`, Xvfb lock/socket cleanup, session registry + panic-hook best-effort reap
- **`ChromeBrowser`** — `XvfbGuard` always kills Xvfb on drop (including failed Chrome launch); async `shutdown` with cooperative `close`/`wait` deadline then forced kill; `force_reap_session` on `Drop`
- **`content_fetch`** — `take()` + async `shutdown` after JoinSet drain (no bare `drop(Arc)`)
- **Signals** — Unix SIGTERM (and SIGINT) cancel the `CancellationToken` so Docker/`timeout`/supervisors trigger cooperative cancel
- **Atomwrite** — `paths::atomic_write` for `--output`, `init-config`, and cookie jar (tempfile same-dir + `sync_data` + persist)
- **Tests** — unit tests for process group/marker/atomwrite; gated E2E `DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle`
- **Docs** — ADR-0017, `gaps.md` RESOLVIDO, this contract. No JSON schema break vs 0.9.5. No remote telemetry

### v0.9.1 → v0.9.3 migration (stealth hardening)
- v0.9.1 (GAP-WS-107): macOS/Windows switched to headed native Quartz/DWM + UA platform coercion
- v0.9.2 (GAP-WS-108/109/110/111): chromiumoxide `--enable-automation` removed via `.disable_default_args()`; UA Chrome aligned to real installed version via `detect_chrome_major_version()` + `Emulation.setUserAgentOverride`; `--force-webrtc-ip-handling-policy=disable_non_proxied_udp`, `--disable-webrtc-hw-decoding`, `--disable-quic` added to flags_stealth
- v0.9.3 (GAP-WS-112): macOS/Windows switched to headless=new (Quartz/DWM clamped `--window-position`); Linux keeps Xvfb private; debug escape hatch is CLI `--chrome-visible` (product env `DUCKDUCKGO_CHROME_VISIBLE` **removed**)

## Quick Start

```bash
cargo install duckduckgo-search-cli --locked
duckduckgo-search-cli "rust async runtime"
# 15 fresh JSON results on your desk.

# For LLMs and agents (v1.0.2 EN wire):
duckduckgo-search-cli "tokio JoinSet examples" --num 15 -q | jaq '.results'

# Agent ops without jaq:
duckduckgo-search-cli "rust async" -q -f json --fields url,title --filter 'title~async' --sort title --count-only
duckduckgo-search-cli "rust async" -q -f json --wire-keys pt   # legacy PT keys
```

## Deep Research (v0.7.0)

For multi-hop research questions — "compare the four major Rust HTTP clients in 2026", "what changed in Tokio 1.40", "summarise the history of DuckDuckGo's HTML endpoint" — `duckduckgo-search-cli` ships a query fan-out pipeline that decomposes the original question into 1..=12 sub-queries, fans them out in parallel, aggregates the results, and optionally synthesises a numbered-reference report.

Since v0.8.9 (GAP-WS-105) `deep-research` also scans the DuckDuckGo news vertical by DEFAULT: every sub-query runs as `--vertical all`, so the SAME Chrome session navigates the web SERP and then the news SERP. The envelope always carries an aggregated `news[]` list (empty when zero; v1.0.2 EN wire — legacy PT `noticias[]` with `--wire-keys pt`). Pass `--no-news` to opt out. **v0.9.4 GAP-WS-113:** without Chrome the CLI **fails closed (exit 2)** — no auto `--no-news` degradation.

```bash
# Default heuristic decomposition (3 sub-queries in v1.0.2, RRF aggregation, no synthesis).
duckduckgo-search-cli deep-research "best rust http client 2026" -f json -q \
  | jaq '.results[] | {title, url, score}'

# Markdown report with explicit token budget and full content extraction.
duckduckgo-search-cli deep-research "tokio vs async-std production 2026" \
  --synthesize --budget-tokens 1500 --synth-format markdown \
  --fetch-content --max-content-length 6000 -f json -q

# Manual sub-queries from a file (comments `#` and blanks ignored).
cat > /tmp/qs.txt <<EOF
# Overview
what is tokio runtime 2026
# Comparison
tokio vs async-std vs smol
# Adoption
tokio production users 2026
EOF
duckduckgo-search-cli deep-research "tokio runtime 2026" \
  --sub-queries-file /tmp/qs.txt --aggregate dedupe-by-url -f json -q
```

### Deep Research flags

| Flag                       | Default        | Description                                                                 |
| -------------------------- | -------------- | --------------------------------------------------------------------------- |
| `--max-sub-queries N`      | `3` (v1.0.2)   | Maximum sub-queries produced (1..=12).                                      |
| `--sub-query-strategy`     | `heuristic`    | `heuristic` (five canonical templates) or `manual` (from `--sub-queries-file`). |
| `--sub-queries-file PATH`  | (none)         | Path to explicit sub-queries (one per line; `#` comments skipped).          |
| `--aggregate`              | `rrf`          | `rrf` (Reciprocal Rank Fusion, K=60) or `dedupe-by-url` (canonical URL).    |
| `--depth`                  | `0`            | Reflection rounds planned but not executed in v0.7.0.                      |
| `--fetch-content` / `--no-fetch-content` | **on** (v0.9.8) | Extract cleaned page body for top-K web + news (cap 4 in v1.0.2); opt out with `--no-fetch-content`. |
| `--synthesize`             | off            | Produce a final Markdown / PlainText / JSON report.                          |
| `--budget-tokens N`        | `1200`         | Token budget for the synthesised report (1 token ≈ 4 chars).               |
| `--synth-format`           | `markdown`     | Output format for synthesis: `markdown`, `plain-text`, `json`.              |
| `--no-news`                | off            | Skip the news vertical scan (v0.8.9, GAP-WS-105). Default runs `--vertical all` per sub-query via Chrome. **v0.9.4 GAP-WS-113:** without usable Chrome the CLI **fails closed exit 2** (no auto `--no-news`; v0.9.0–0.9.3 auto-degrade was superseded). |

### Deep Research output schema (v1.0.2 EN wire)

```jsonc
{
  "query": "best rust http client 2026",
  "kind": "deep_research",
  "metadata": {
    "original_query": "best rust http client 2026",
    "sub_queries": [
      { "text": "...", "strategy": "heuristic", "status": "ok", "elapsed_ms": 420 }
    ],
    "unique_result_count": 27,
    "unique_news_count": 9,
    "total_time_ms": 1850,
    "cascade_level": 0
  },
  "results": [
    { "title": "...", "url": "...", "score": 0.041, "sources": ["..."] }
  ],
  "news": [
    { "position": 1, "title": "...", "url": "...", "source": "...", "relative_date": "2 hours ago", "score": 0.032, "occurrences": 2 }
  ],
  "news_count": 9,
  "synthesis": {
    "format": "markdown",
    "body": "# Research Report\n\n...\n\n[1] Title — url",
    "estimated_tokens": 1200,
    "reference_count": 5
  }
}
```

> Prefer `jaq '.results'` / `.metadata` on **v1.0.2+**. Legacy PT keys: `--wire-keys pt`. Full rename table: [`docs/MIGRATION.md`](docs/MIGRATION.md).

The subcommand inherits the global flags (`--num`, `--lang`, `--country`, `--parallel`, `--endpoint`, `--proxy`, `--retries`, `--global-timeout`, and since v0.9.8 transport flags with `global = true` including `--chrome-path`, `--vertical`, fetch flags, identity) — these work **before or after** `deep-research`. All cancellation, retry, anti-bot, and circuit-breaker behaviour from the search path applies unchanged.

## Real-world recipes

```bash
# 1. Extract only URLs for a downstream fetcher.
duckduckgo-search-cli "site:example.com changelog 2025" --num 15 -f json \
  | jaq -r '.results[].url'

# 2. Feed cleaned page bodies into a summarizer.
duckduckgo-search-cli "tokio runtime internals" --num 15 \
  --fetch-content --max-content-length 4000 -f json \
  | jaq -r '.results[] | "# \(.title)\n\(.content)\n"' > corpus.md

# 3. Fan-out multiple queries in one shot.
duckduckgo-search-cli "rust rayon" "rust tokio" "rust crossbeam" \
  --num 15 --parallel 3 -f json

# 4. NDJSON streaming for reactive pipelines.
duckduckgo-search-cli "wasm runtimes" --num 15 --stream \
  | jaq -r 'select(.url) | .url' \
  | xargs -I{} my-downloader {}

# 5. Route through a corporate proxy (CLI --proxy or XDG proxy_url — env not read).
duckduckgo-search-cli "vendor status page 2026" --num 15 \
  --proxy http://user:pass@proxy.internal:8080 -f json

# 6. Agent ops: project + filter + sort without jaq.
duckduckgo-search-cli "rust async" -q -f json \
  --fields url,title --filter 'title~async' --sort title --limit 5

# 7. Budget dry-run (deep-research, no Chrome).
duckduckgo-search-cli deep-research --print-budget -q

# 8. Offline smoke test (no real network).
cargo test --test integration_wiremock
```

## Configuration

```bash
# Write default selectors.toml and user-agents.toml to the XDG dir.
duckduckgo-search-cli init-config

# Dry-run first to see what would be written.
duckduckgo-search-cli init-config --dry-run

# Overwrite existing files explicitly.
duckduckgo-search-cli init-config --force

# Persist XDG knobs (RuntimeConfig SSOT — CLI > XDG > FACTORY).
duckduckgo-search-cli config set wire_keys en
duckduckgo-search-cli config set budget_profile desktop_contended
duckduckgo-search-cli config set chrome_session_retries 2
duckduckgo-search-cli config effective
```

Since v1.0.6, `config get` / `config set` / `config unset` fail closed when you MIX the positional form with `--key`: `config set wire_keys en --key wire_keys` prints the `error-response` envelope with `category: "usage"` on stdout and exits **2**. Pick one form per invocation.

## Commands

Every subcommand with a one-liner (v1.0.6). Hidden `buscar` is equivalent to default search mode.

| Command | Example |
| ------- | ------- |
| **Default search** | `duckduckgo-search-cli -q -f json "rust async" \| jaq '.results[].url'` |
| `buscar` (hidden) | `duckduckgo-search-cli buscar -q -f json "query"` (equivalent to default search; omitted from `--help`) |
| `init-config` | `duckduckgo-search-cli init-config` / `init-config --force` / `init-config --dry-run` |
| `completions` | `duckduckgo-search-cli completions bash > ~/.local/share/bash-completion/completions/duckduckgo-search-cli` (since v1.0.6 a closed consumer pipe exits **141** instead of panicking) |
| `deep-research` | `duckduckgo-search-cli deep-research "tokio vs async-std" -q -f json --print-budget` |
| `commands` | `duckduckgo-search-cli commands -q` |
| `schema` | `duckduckgo-search-cli schema --name search-output` |
| root `--print-schema` | `duckduckgo-search-cli --print-schema` (same catalog as `schema` without `--name`) |
| root `--probe` | `duckduckgo-search-cli --probe -q -f json` (separate from `doctor`) |
| `doctor` | `duckduckgo-search-cli doctor -q` / `doctor --strict` / `doctor --probe-deep` (no `doctor --probe`) |
| `locale` | `duckduckgo-search-cli locale -q` |
| `man` | `duckduckgo-search-cli man \| man -l -` |
| `config path` | `duckduckgo-search-cli config path` |
| `config list` | `duckduckgo-search-cli config list` |
| `config get` | `duckduckgo-search-cli config get wire_keys` |
| `config set` | `duckduckgo-search-cli config set budget_profile lab` (mixing the positional key with `--key` exits **2**) |
| `config unset` | `duckduckgo-search-cli config unset proxy_url` |
| `config effective` | `duckduckgo-search-cli config effective` |
| `help` | `duckduckgo-search-cli help deep-research` |

**Agent-native flags (search + deep-research):**

```bash
duckduckgo-search-cli "query" -q -f json --fields url,title --filter 'url~github' --sort title --limit 5
duckduckgo-search-cli "query" -q -f json --count-only
duckduckgo-search-cli "query" -q -f json --wire-keys pt   # legacy PT serialize
```

### Agent-native capability matrix per surface

The table above lists the subcommands; it does not tell you which agent-native operator each one accepts. Since v1.0.4 that matrix is published data, and since v1.0.5 every surface either acts or refuses by name.

- Read the live matrix with `duckduckgo-search-cli commands`, under `agent_ops`, instead of trusting this static summary.
- Apply `--fields` / `--select` and `--max-output-bytes` on every surface, because they have meaning on any JSON object.
- Apply the five row operators — `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only` — only where the surface declares a row array: `doctor` (`checks`), `schema` (`schemas`), `locale` (`available`), `config list` and `config effective` (`allowed_keys`), `init-config` (`files`).
- Expect the rowless surfaces — `commands`, `config path`, `config get`, `config set`, `config unset`, root `--probe` and root `--probe-deep` — to REFUSE those five operators with exit 2.
- Expect `locale`, `config list` and `config effective` to REFUSE `--truncate-content` with exit 2, because on those surfaces every string is an identifier the caller hands back to a program.
- Expect `config path`, `config get`, `config set` and `config unset` to support only `--fields` and `--max-output-bytes`.
- Expect `doctor`, `schema`, `init-config`, `commands`, root `--probe` and root `--probe-deep` to support `--truncate-content`, because those envelopes carry real prose.
- Read every refusal the same way: `{"error", "message"}` on stdout in the published `error-response` shape, localized prose on stderr, exit 2.
- Read `agent_ops[].discriminator_key` as the KEY that routes the envelope (`type`), never as the value it carries.

```bash
duckduckgo-search-cli commands -q -f json | jaq '.agent_ops'
duckduckgo-search-cli doctor -q -f json --fields type,status
duckduckgo-search-cli commands -q -f json --count-only   # exit 2: rowless surface
```

## Flags

> **SSOT:** generated from `duckduckgo-search-cli --help` and subcommand `--help` on binary **v1.0.6** (70 root flags + deep/doctor/init/schema/man exclusives). Prefer `commands` / `schema` for low-token agent discovery. Portuguese prose lives **only** in [`README.pt-BR.md`](README.pt-BR.md) — this file is English-only.

### Root / default search (complete inventory from `--help`)

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `-n`, `--num` | `15` | Max results per query (default 15; pages controlled by `--pages`, default 1). |
| `-l`, `--lang` | `pt` | DuckDuckGo `kl` language code (SERP). Default `pt`. |
| `-c`, `--country` | `br` | DuckDuckGo `kl` country code. `--region` is a legacy alias. Default `br`. |
| `--queries-file` | (none) | File with additional queries (one per line). Empty lines ignored. |
| `--endpoint` | `html` | `html` (default production) or `lite` (legacy value only; not a production success path under GAP-WS-113). |
| `--vertical` | **`all`** (v0.9.8) | `web`, `news`, or `all` (**default `all`** since v0.9.8). News is Chrome-only. |
| `--time-filter` | (none) | Time filter: `d` / `w` / `m` / `y`. Default: none. |
| `--safe-search` | `moderate` | Safe-search: `off`, `moderate` (default), or `on`. |
| `--identity-profile` | `auto` | Pin a 12-identity pool profile (`chrome-win`, `safari-mac`, …). Default `auto` rotates on block. |
| `--cookies-path` | (none) | Override cookie jar path (default: XDG config dir + `cookies.json`). |
| `--seed` | (none) | Deterministic seed for UA + identity selection (debug reproducibility). |
| `--config` | (none) | Path to configuration directory (overrides default OS config path). |
| `-f`, `--format` | `auto` | Output format: `json`, `text`, `markdown`/`md`, `tsv`, `ndjson`, or `auto` (TTY-aware). |
| `-o`, `--output` | (none) | Write to file instead of stdout (creates parents; Unix 0o644). |
| `--stream` | off | Multi-query: emit NDJSON as each search completes. Alias: `-f ndjson`. Early close → exit **141**. |
| `--fields` | (none) | Project each result row to listed wire fields (comma-separated; EN or PT tokens). |
| `--select` | (none) | Alias of `--fields` (agent-native / ETL-familiar name). |
| `--filter` | (none) | Filter result rows after SERP extract (agent-native; no jq). |
| `--limit` | (none) | Cap result rows **after** extract + `--filter` (agent-native; no jq). |
| `--sort` | (none) | Sort result rows after filter (agent-native; no jq). |
| `--dedupe-by` | (none) | Deduplicate rows by canonical URL after sort (agent-native; no jq). |
| `--count-only` | off | Emit only compact EN counts — no result rows. |
| `--truncate-content` | (none) | Truncate each row `content` to N Unicode scalars (anti-token). |
| `--max-output-bytes` | (none) | Fail-closed if formatted stdout payload exceeds N bytes. |
| `--pretty` | off | Indented JSON (`to_string_pretty`). Default is **compact** JSON for agent token budgets. |
| `--no-color` | off | Disable colored output (respects `NO_COLOR` / no-color.org). |
| `--print-schema` | off | Print JSON Schema catalog on stdout (same as `schema` without `--name`). |
| `--ui-lang` | (none) | UI language for human stderr (`en`|`pt-BR`). **Not** SERP `-l`/`--lang`. |
| `--config-home` | (none) | Override XDG/platform config directory (selectors, cookies, ui-lang). |
| `--wire-keys` | **`en`** (v1.0.2) | Serialize JSON keys as English (`en`, **v1.0.2 default**) or legacy Portuguese (`pt`). Also `config set wire_keys`. |
| `-t`, `--timeout` | `15` | Per-query timeout in seconds (default 15). |
| `-p`, `--parallel` | `5` | Concurrent requests (`1..=20`, default 5). Alias: `--max-concurrency`. |
| `--shared-session-verticals` | off | Force one shared Chrome session for web+news (`--vertical all`) instead of dual multi-process Chromes. |
| `--pages` | `1` | Pages per query (`1..=5`, default **1**; may auto-raise with `--num`). |
| `--retries` | `2` | Extra retries on transient HTTP/network failures (`0..=10`, default 2). |
| `--disable-retry` | off | Force zero retries (incident kill switch). Equivalent to `--retries 0`. |
| `--base-url-html` | (none) | Override HTML SERP base URL (wiremock/tests). |
| `--base-url-lite` | (none) | Override Lite base URL. |
| `--base-url-serp` | (none) | Override SERP / warm-up base URL. |
| `--proxy` | (none) | HTTP/HTTPS/SOCKS5 proxy via CLI (product does **not** inherit `HTTP(S)_PROXY`). |
| `--no-proxy` | off | Disable every proxy source (explicit no-proxy). |
| `--allow-lite-fallback` | off | **LEGACY NO-OP (GAP-WS-113)** — does not force Lite; SERP stays HTML Chrome. Kept so scripts do not exit 2 on unknown flag. |
| `--global-timeout` | `180` | Whole-pipeline timeout (`1..=3600` s, default 180). Distinct from per-request `--timeout`. Global on subcommands. |
| `--cancel-grace-secs` | `5` | Cooperative cancel grace before hard exit (`1..=60` s, default 5). |
| `--probe` | off | Chrome health probe via CDP: minimal reachability + latency as JSON. |
| `-v`, `--verbose` | off | `-v` = DEBUG, `-vv`+ = TRACE on stderr. Product log = CLI `-v`/`-q` + XDG `log_directive` (not `RUST_LOG`). |
| `-q`, `--quiet` | off | Silence **all** tracing on stderr (including ERROR). |
| `-V`, `--version` | — | Print `NAME VERSION (git:SHA)`. The SHA carries `-dirty` when the working tree is not clean (v1.0.6). |
| `-h`, `--help` | — | Print help for the root command or for any subcommand. |
| `--no-input` | off | Agent contract: never prompt / never read interactive TTY. |
| `--probe-deep` | off | Deep Chrome/CDP health check including interstitial (CAPTCHA) detection; JSON report. |
| `--require-results` | off | Fail with exit 5 when search returns zero results (agent gate). Deep-research may use exit 70 in require mode. |
| `--pre-flight` | off | Pre-flight ghost-block / interstitial calibration on shared Chrome SERP. Does **not** unlock pure-HTTP or Lite (GAP-WS-113). Web vertical only. |
| `--no-zero-cause-strict` | off | Disable strict zero-cause mapping (legacy exit 5 for all zeros). Default strict ON → exit 6 for non-legitimate zeros. |
| `--fetch-content` | **on** (v0.9.8) | Affirms content extraction (**default ON** since v0.9.8 for web+news). Prefer omit or use `--no-fetch-content`. |
| `--no-fetch-content` | off | Disable page content extraction (opt-out of v0.9.8 agent-ready default). |
| `--fetch-content-cap` | **`4`** (v1.0.2) | Max URLs to enrich per vertical (`1..=50`, default **4** in v1.0.2; was 10 at v0.9.8). |
| `--max-content-length` | `10000` | Max characters of extracted body per page (`1..=100_000`, default 10000). |
| `--per-host-limit` | `2` | Concurrent fetches per host under content fetch (`1..=10`, default 2). |
| `--match-platform-ua` | off | Filter UA pool to current OS when external `user-agents.toml` is present. |
| `--chrome-path` | (none) | Manual Chrome/Chromium executable for all production network ops. Multi-channel (v0.9.8): Flatpak export→ELF. Global after `deep-research`. |
| `--chrome-visible` | off | Force headed Chrome (visible window). Debug override. |
| `--chrome-headless` | off | Force headless Chrome (`--headless=new`). Overrides Xvfb auto path. |
| `--chrome-xvfb` | off | Request private Xvfb headed mode on Linux (invisible headed anti-bot). |
| `--chrome-session-retries` | `2` | Additional Chrome session launch attempts after the first (transient CDP; default 2). |
| `--dump-news-html` | (none) | Write news SERP HTML after Chrome extract to this path (local debug only). |
| `--no-warmup` | off | Skip warm-up `GET https://duckduckgo.com/` that populates session cookies. |
| `--no-cookie-persistence` | off | Keep cookies in memory only; never write the jar to disk. |

### `deep-research` only (in addition to global root flags)

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `--max-sub-queries` | **`3`** (v1.0.2) | Max sub-queries from decomposition (`1..=12`, default **3** in v1.0.2). |
| `--sub-query-strategy` | `heuristic` | Decomposition strategy (e.g. `heuristic`). |
| `--sub-queries-file` | (none) | File with explicit sub-queries for deep-research (one per line). |
| `--aggregate` | `rrf` | Aggregation mode for deep-research results. |
| `--depth` | `0` | Research depth / fan-out intensity. |
| `--budget-tokens` | `4000` | Token budget bound for deep-research. |
| `--synth-format` | `markdown` | Synthesis output format for deep-research report. |
| `--synthesize` | off | Enable LLM/report synthesis step in deep-research. |
| `--allow-under-budget` | off | Allow proceeding when estimated budget is under the requested profile. |
| `--print-budget` | off | Dry-run budget estimate without Chrome / without inventing a placeholder query. |
| `--auto-contention-budget` | off | Enable contention-aware budget adjustment (default policy). |
| `--no-auto-contention-budget` | off | Disable contention-aware budget auto-adjustment. |
| `--require-all-sub-queries` | off | Fail if any sub-query does not complete successfully. |
| `--no-news` | off | Opt out of the news vertical in deep-research (news is default on). |

### `doctor` only

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `--strict` | off | Doctor: fail closed on non-OK checks. |

### `init-config` only

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `--force` | off | init-config: overwrite existing config files. |
| `--dry-run` | off | init-config: simulate without writing files. |

### `schema` only

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `--name` | (none) | schema: emit a named schema body instead of the catalog. |

### `man` only

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `--file` | (none) | man: write roff to this path (atomic). Uses `--file`, not `-o`. |

## News Vertical (v0.8.9; defaults superseded by v0.9.8; wire EN since v1.0.2)

- **v0.9.8:** default `--vertical` is **`all`** (web + news). Opt out with `--vertical web`. (Historical v0.8.9 default was `web` — **superseded by v0.9.8**.)
- `--vertical news` returns news only (`results: []`); `--vertical all` returns web AND news in the SAME Chrome session
- The news vertical is routed EXCLUSIVELY through Chrome (the SERP requires JavaScript) — NO HTTP fallback
- Multi-query batches accepted since GAP-WS-105 (`--queries-file` and multiple positional queries) — each query runs its own Chrome session; in `deep-research` the news vertical is the DEFAULT (opt-out `--no-news`)
- **v1.0.2 EN wire (default)** with `--vertical news|all`: `news[].{position,title,url,source,relative_date,thumbnail}`, `news_count`, and `metadata.vertical_used`; news may also carry `content` / `content_size` / `content_extraction_method` when fetch is on. Legacy PT keys (`noticias`, `quantidade_noticias`, `metadados.vertical_usada`, …) only with `--wire-keys pt`
- Opt-out path for web-only SERP: `--vertical web` (and `--no-fetch-content` if you need the pre-0.9.8 thin envelope)
- A legitimate zero of news classifies as `zero_cause: vertical-no-results` (exit 5, NOT 6); legacy PT cause string with `--wire-keys pt`
- **v0.9.8 content fetch:** default ON for **web + news** (FETCH_CAP=4 (v1.0.2; was 10 at v0.9.8)); opt out with `--no-fetch-content`. (Historical claim “fetch ONLY on web `results[]`” is **superseded by v0.9.8**.)
- Agent metadata may include `chrome_path_resolved` and `chrome_channel` (local contract — **not** telemetry; legacy PT names with `--wire-keys pt`)

```bash
timeout 90 duckduckgo-search-cli --vertical news "noticias brasil" -q -f json | jaq '.news'
timeout 90 duckduckgo-search-cli --vertical all "rust release" -q -f json | jaq '{web: .result_count, news: .news_count}'
```

## Schema JSON (v0.8.9 → v1.0.2)

### Consumer migration guide

When parsing the JSON envelope, consumers MUST handle these schema
changes. Paths below are **historical PT names as introduced**; **v1.0.2
default serialize is English** (ADR-0027) — map via the EN column or see
[`docs/MIGRATION.md`](docs/MIGRATION.md). Legacy PT emit: `--wire-keys pt`.

| Version | Historical PT path (as shipped) | v1.0.2 EN path (default) | Type | Default | BC |
|---|---|---|---|---|---|
| v0.7.10 | `metadados.pre_flight_disparado` | `metadata.pre_flight_fired` | bool | `false` | Additive |
| v0.8.9 | `noticias[]` | `news[]` | array | (absent) | Additive — only with `--vertical news\|all` |
| v0.8.9 | `quantidade_noticias` | `news_count` | u32 | (absent) | Additive — only with `--vertical news\|all` |
| v0.8.9 | `metadados.vertical_usada` | `metadata.vertical_used` | string | (absent) | Additive — only with `--vertical news\|all` |
| v0.8.9 | `noticias[]` (deep-research) | `news[]` | array | `[]` | Additive — ALWAYS present in the deep-research envelope (GAP-WS-105) |
| v0.8.9 | `quantidade_noticias` (deep-research) | `news_count` | number | `0` | Additive — ALWAYS present in the deep-research envelope (GAP-WS-105) |
| v0.8.9 | `metadados.total_noticias_unicas` (deep-research) | `metadata.unique_news_count` | number | `0` | Additive (GAP-WS-105) |
| v0.8.9 | `metadados.sub_queries[].quantidade_noticias` / `.news_indisponivel` | `metadata.sub_queries[].news_count` / `.news_unavailable` | number / bool | (absent) | Additive — optional (GAP-WS-105) |
| v0.9.8 | `metadados.chrome_path_resolvido` | `metadata.chrome_path_resolved` | string | (absent) | Additive — resolved Chrome ELF path (agent metadata, **not** telemetry) |
| v0.9.8 | `metadados.chrome_canal` | `metadata.chrome_channel` | string | (absent) | Additive — channel (`host-chrome`, `flatpak`, `snap`, …) |
| v0.9.8 | `metadados.usou_chrome` | `metadata.used_chrome` | bool | (honest) | Honest on news-only, multi-query, deep, failure envelopes |
| v0.9.8 | `noticias[].conteudo` / `.tamanho_conteudo` / `.metodo_extracao_conteudo` | `news[].content` / `.content_size` / `.content_extraction_method` | string / number / string | (absent) | Additive when content fetch ON (default) |

No fields removed. No fields deprecated. The `--require-results`
flag in `deep-research` is local to that subcommand and emits exit
code `70` (EX_SOFTWARE) on zero aggregated results instead of `0`
(silent zero-result).

When the probe-deep endpoint detects a CAPTCHA, the JSON envelope
now includes the specific marker matched:

```json
{
  "status": "captcha",
  "cascade_reason": "cloudflare",
  "mitigation_suggestion": "Cloudflare challenge detected (marker: cf-turnstile). Re-run with --pre-flight..."
}
```

Consumers should treat sentinels starting with `<` (e.g.
`<ghost-block-no-marker>`, `<empty-body>`, `<no-marker>`) as
non-literal markers and omit them from user-facing lists.

## Environment variables

Product configuration is **CLI + XDG only** (no product env knobs). Historical env names below are **removed / not read** and must not be taught as live config.

| Variable / knob | Status | Use instead |
| -------------- | ------ | ----------- |
| Product log filter | CLI + XDG | `-v` / `-vv` / `-q`, or `config set log_directive duckduckgo_search_cli=debug` (precedence: `-q` > `-v` > XDG > `info`) |
| Proxy | CLI + XDG | `--proxy URL` / `--no-proxy`, or `config set proxy_url …` (does **not** inherit `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY`) |
| Chrome path | CLI + XDG | `--chrome-path PATH` or `config set chrome_path …` (`CHROME_PATH` is not product config) |
| `RUST_LOG` | **Not product config** | Use CLI `-v`/`-q` or XDG `log_directive` |
| `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` | **Not read** | `--proxy` / XDG `proxy_url` |
| `DUCKDUCKGO_CHROME_VISIBLE` | **Removed** | `--chrome-visible` |
| `DUCKDUCKGO_CHROME_HEADLESS` | **Removed** | `--chrome-headless` |
| `DUCKDUCKGO_CHROME_XVFB` | **Removed** | private Xvfb is automatic on Linux |
| `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` | **Removed** (not read) | Chrome required via feature `chrome`; missing Chrome → exit 2 |
| `DUCKDUCKGO_ZERO_CAUSE_STRICT` | **Removed** | `--no-zero-cause-strict` for legacy exit 5 |

## Output formats

- `json` (default for pipes): canonical schema with `results[]` and `metadata` (v1.0.2 EN wire; legacy PT via `--wire-keys pt`), stable field order. Each result may include optional original-title fields when the "Official site" heuristic rewrites the title.
- `text`: human-readable block `NN. Title\n   URL\n   snippet`.
- `markdown`: `- [Title](URL)\n  > snippet`.
- Stream (`--stream` or `-f ndjson`): multi-query NDJSON — one compact result/envelope line per LF as they arrive. Consumer closes early → exit **141** (v1.0.1); one-shot Chrome reap still runs.

## Exit codes

| Code | Meaning                                                        |
| ---- | -------------------------------------------------------------- |
| 0    | Success.                                                       |
| 1    | Runtime error (network, parse, I/O).                           |
| 2    | Invalid configuration (CLI flag out of range, bad proxy URL).  |
| 3    | DuckDuckGo 202 block anomaly (soft-rate-limit).                |
| 4    | Global timeout exceeded.                                       |
| 5    | Zero results across all queries.                               |
| 6    | Suspected block (zero results with non-legitimate cause, v0.8.0+). |
| 130  | Cancelled by SIGINT (Ctrl+C). Cooperative cancel, not a failure. |
| 141  | Broken pipe (stdout consumer closed early; v1.0.1 stream-safe). |
| 143  | Cancelled by SIGTERM — what `timeout` sends first. Not a failure. |

## Troubleshooting

1. **HTTP 202 / block anomaly (exit 3)** — back off, raise `--retries`, rotate UA via `init-config` and tweak `user-agents.toml`.
2. **Rate limited (HTTP 429)** — lower `--per-host-limit`, enable `--match-platform-ua`, or add `--proxy`.
3. **Zero results (exit 5)** — check `--lang` and `--country`, verify `--time-filter`, ensure Chrome is usable (v0.9.4 is Chrome-only; Lite is not a remediation path).
4. **Chrome not found (exit 2 since GAP-WS-113)** — install Chromium via your package manager, or pass `--chrome-path /path/to/chrome` (`cargo install duckduckgo-search-cli --locked --force` includes feature `chrome` by default).
5. **UTF-8 issues on Windows** — the binary auto-switches cmd.exe to code page 65001; if you still see mojibake, run `chcp 65001` before the command.
6. **How do I integrate with Claude Code, Cursor, Aider, or another agent?** — expose the binary as a shell tool. Most agents accept a command template such as `duckduckgo-search-cli "{query}" --num 15 -q -f json`. The stable schema keeps the tool contract stable across releases.
7. **Pipe to jaq/jq returns empty** — check `echo ${PIPESTATUS[*]}` after the pipe. If the first number is non-zero, the CLI errored before producing output. Common causes: DuckDuckGo rate-limiting (exit 5), global timeout (exit 4), or missing query. Always pass `-q -f json` when piping.
8. **`--output` rejects my path (exit 2)** — v0.5.0 validates output paths before writing. Paths containing `..` are rejected to prevent directory traversal. Paths targeting system directories (`/etc`, `/usr`, `/bin`, `C:\Windows`) are blocked. Use paths under your home directory, `/tmp`, or the current working directory.
9. **Getting exit 5 (zero results) frequently** — this is usually temporary rate-limiting from DuckDuckGo, not a permanent block. Wait 60 seconds and retry. If the problem persists, add `--proxy socks5://127.0.0.1:9050` to rotate your outbound IP, confirm Chrome is healthy (`--probe` / `--probe-deep`), or adjust `--chrome-path`. Do **not** use `--allow-lite-fallback` (no-op since v0.9.4 / GAP-WS-113).
10. **CAPTCHA interstitial suspected (v0.7.3+)** — run `duckduckgo-search-cli --probe-deep -q -f json` to classify the response body. If `status` is `captcha`, the response is blocked. The probe also reports `mitigation_suggestion` (v1.0.2 EN; legacy PT `sugestao_mitigacao` with `--wire-keys pt`) with concrete next steps (rotate proxy, switch endpoint, back off). Treat the cookie jar as credential: the file `cookies.json` is written with 0o600 permissions and contains session cookies from DuckDuckGo.
11. **Orphan Chromium / Xvfb / temp profiles after many agent runs** — upgrade to **1.0.6** (`cargo install duckduckgo-search-cli --locked --force`) for pipe-safe reap (**SIG_IGN** on SIGPIPE + `ensure_oneshot_cleanup` on all exits, including early `| head` / BrokenPipe → exit **141**) plus EN wire + agent ops. Process one-shot landed in **0.9.6** (ADR-0017); **disk** one-shot + auditable `ddg-chrome-*` profiles landed in **1.0.0** (ADR-0020); **1.0.1** closes the early-pipe orphan hole (Pass 52). New invocations reap their tree and remove their profile; the next run sweeps only stale `ddg-chrome-*` (never bulk-deletes foreign `.tmp*` or `org.chromium.Chromium.*`). Historical process orphans (pre-0.9.6) or generic `.tmp*` profile dirs (pre-1.0.0) are **not** mass-auto-killed: identify automation Chrome by cmdline `user-data-dir` and stop those PIDs / remove those dirs once if needed. Prefer supervisors that send **SIGTERM** first (GNU `/usr/bin/timeout`); bare **SIGKILL/OOM** remains an OS residual limit. **Breaking wire:** if scripts still parse `resultados`/`metadados`, either update to EN keys or pass `--wire-keys pt` — see [`docs/MIGRATION.md`](docs/MIGRATION.md).
12. **A Rust `timeout` wrapper shadows GNU coreutils** — the binary `~/.cargo/bin/timeout` (Rust crate `timeout-cli` v0.1.0) shadows GNU coreutils on `PATH` and re-parses the *subprocess* arguments as its own. Running `timeout 60 duckduckgo-search-cli -vv -q -f json "query"` makes the Rust `timeout` consume `-v` and `-q`, so the CLI never sees them. Symptom: exit **2** with `the argument '--verbose' cannot be used multiple times`. **Workaround:** call GNU coreutils explicitly — `/usr/bin/timeout 60 duckduckgo-search-cli -vv -q -f json "query"`. To find out which `timeout` is first on `PATH`, run `command -v timeout` and `file $(command -v timeout)`. The helper script [`scripts/detect-timeout-wrapper.sh`](scripts/detect-timeout-wrapper.sh) automates that detection. The Rust wrapper also rejects GNU-style suffixes such as `5m`, so convert to whole seconds first.

## Migration notes before v1.0.0 — consolidated

Release history is not inlined here. Every note for v0.3.x through v0.9.x lives in [`CHANGELOG.md`](CHANGELOG.md); only what is still actionable on a 1.0.x install is kept below.

- Read [`CHANGELOG.md`](CHANGELOG.md) for the full v0.3.x → v0.9.x history, including the v0.7.x TLS/build-environment migration and the Windows toolchain notes.
- Read [`docs/INSTALL-WINDOWS.md`](docs/INSTALL-WINDOWS.md) when a Windows source build fails on a missing toolchain component.
- Read [`docs/MIGRATION.md`](docs/MIGRATION.md) for the only breaking wire change in the 1.x line — Portuguese keys became English on serialize in v1.0.2; pass `--wire-keys pt` to keep the legacy emit.
- Expect `--num` to default to 15 and to auto-raise `--pages` up to 5 when a single DuckDuckGo page cannot satisfy the requested count (v0.4.0).
- Expect `--identity-profile auto` to rotate a 12-identity pool through a 5-level cascade when a block is detected (v0.6.4).
- Expect a per-host circuit breaker to open after 3 consecutive failures and to cool down for 30 seconds under `--fetch-content --parallel` (v0.6.5).
- Expect `deep-research` to exit 0 when web OR news produced results, and 5 only when BOTH are empty (v0.8.9).
- Expect `--synthesize` to give recent news roughly 30% of `--budget-tokens` and web roughly 70%, with the format unchanged under `--no-news` or zero news (v0.8.9).
- Expect `--allow-lite-fallback` to be a legacy no-op since v0.9.4 — it is kept only so old scripts do not exit 2 on an unknown flag.
- Expect the cookie jar to be written with Unix `0o600` under the XDG config directory; opt out with `--no-cookie-persistence`.

See the [CHANGELOG](CHANGELOG.md) for release history.

License: MIT OR Apache-2.0.
