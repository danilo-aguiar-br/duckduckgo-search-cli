# `duckduckgo-search-cli` — Integration Guide for 16 AI Agents / LLMs

[Português (Brasil)](INTEGRATIONS.pt-BR.md)

- The definitive copy-paste playbook for plugging `duckduckgo-search-cli` into every major AI coding agent.
- Find your agent, copy the snippet, gain structured web search in under 30 seconds.

[![Crates.io](https://img.shields.io/crates/v/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![Docs.rs](https://img.shields.io/docsrs/duckduckgo-search-cli)](https://docs.rs/duckduckgo-search-cli)

## Agent Index / Índice de Agentes
| # | Agent | Shell mechanism | Jump |
|---|---|---|---|
| 1 | Claude Code (Anthropic) | Bash tool | [EN](#1-claude-code) · [PT](#1-claude-code-1) |
| 2 | OpenAI Codex | Shell / exec tool | [EN](#2-openai-codex) · [PT](#2-openai-codex-1) |
| 3 | Gemini CLI (Google) | Shell tool | [EN](#3-gemini-cli) · [PT](#3-gemini-cli-1) |
| 4 | Cursor | Terminal + chat | [EN](#4-cursor) · [PT](#4-cursor-1) |
| 5 | Windsurf (Codeium) | Cascade terminal | [EN](#5-windsurf) · [PT](#5-windsurf-1) |
| 6 | Aider | `/run` command | [EN](#6-aider) · [PT](#6-aider-1) |
| 7 | Continue.dev | Custom slash command | [EN](#7-continuedev) · [PT](#7-continuedev-1) |
| 8 | MiniMax Agent | Agent tool / API | [EN](#8-minimax-agent) · [PT](#8-minimax-agent-1) |
| 9 | OpenCode | Shell tool | [EN](#9-opencode) · [PT](#9-opencode-1) |
| 10 | Paperclip | Agent capability | [EN](#10-paperclip) · [PT](#10-paperclip-1) |
| 11 | OpenClaw | CLI tool binding | [EN](#11-openclaw) · [PT](#11-openclaw-1) |
| 12 | Google Antigravity | Agent shell | [EN](#12-google-antigravity) · [PT](#12-google-antigravity-1) |
| 13 | GitHub Copilot CLI | `gh copilot` | [EN](#13-github-copilot-cli) · [PT](#13-github-copilot-cli-1) |
| 14 | Devin (Cognition) | Cloud sandbox | [EN](#14-devin) · [PT](#14-devin-1) |
| 15 | Cline | VS Code terminal | [EN](#15-cline) · [PT](#15-cline-1) |
| 16 | Roo Code | VS Code terminal | [EN](#16-roo-code) · [PT](#16-roo-code-1) |


## Registry state — read this before you install
- v1.0.6 is the first release with the cfg-stripping defect closed (GAP-REL-001)
- MEASURED 2026-08-21: crates.io serves `1.0.2` as `max_stable_version`, and `1.0.2` does NOT compile on macOS or Windows
- Until `1.0.6` is published, `cargo install ... --version 1.0.6` fails immediately with `could not find duckduckgo-search-cli with version 1.0.6`
- That fast, named failure is DELIBERATE: installing without a version pin silently resolves to the broken `1.0.2` and dies minutes later with a cryptic `E0432`
- A fast failure that names what is missing beats a slow one that does not
- Check the live registry yourself before reporting an install bug: `cargo run --bin verify_published --features release-gate`
- That gate exits 0 only when the registry actually serves the version this tree carries


## Baseline Contract / Contrato Base
- Binary: `duckduckgo-search-cli`
- Install: `cargo install duckduckgo-search-cli --locked --version 1.0.6`
- ALWAYS pin the version and ALWAYS pass `--locked`, in every Setup block below: the published `1.0.2` and `1.0.1` do NOT compile on macOS or Windows, failing with `E0432: unresolved imports` at `src/browser/session/mod.rs:21` (GAP-REL-001); `--locked` additionally prevents the solver from re-resolving into the GAP-WS-48 broken transitive state
- Current: v1.0.6 (wire EN default ADR-0027). Migration: [MIGRATION.md](MIGRATION.md).
- Defaults: `--num 15`, `--pages 1` (raise with `--pages` 1..=5 when more SERP pages needed), `-f auto` (JSON in pipes, text in TTY)
- Key flags: `-q` (quiet), `-f json|text|markdown`, `-o FILE`, `--queries-file`, `--fetch-content` / `--no-fetch-content`, `--time-filter d|w|m|y`, `--proxy`, `--global-timeout` (default 180 since v0.9.9; pass lower only for thin SERP), `--parallel 5`, `--vertical web|news|all`, `--chrome-path`, `--wire-keys en|pt`
- Agent ops (v1.0.2): `--fields`/`--select`, `--filter`, `--limit`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`
- Discovery: `commands`, `schema`, `doctor`, `locale`, `man`
- v0.9.8 defaults: `--vertical all`, content fetch ON (top web + news, cap 4 (v1.0.2 default)); opt out with `--vertical web` / `--no-fetch-content` / deep `--no-news`; prefer `timeout 180` when fetch is on
- v0.6.4+ (preserved in v0.6.5 and v0.7.x) anti-bot flags: `--probe` (pre-flight health check), `--identity-profile` (pin a 12-identity pool profile), `--seed` (deterministic seed for UA + identity selection)
- v0.7.3+ session and probe-deep flags: `--no-warmup`, `--no-cookie-persistence`, `--cookies-path <PATH>`, `--probe-deep`, `--allow-lite-fallback` (NO-OP since v0.9.4 / GAP-WS-113)
- v0.9.8+: agent-ready multi-canal (GAP-WS-AGENT-READY-001 / ADR-0018) — Flatpak Chrome resolve, global transport flags, agent metadata (EN v1.0.2: `chrome_path_resolved` / `chrome_channel` / `used_chrome`; legacy PT via `--wire-keys pt`) (NOT telemetry)
- v0.9.6+: one-shot process ownership (GAP-WS-LIFECYCLE-001 / ADR-0017) — each CLI invocation fully reaps its Chromium/Xvfb process tree; prefer SIGTERM-first timeouts (GNU `timeout`)
- v1.0.0+: one-shot DISK + auditable profiles (GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020) — prefix `ddg-chrome-*`, remove profile on cooperative exit, next-run sweep only owned `ddg-chrome-*` (never bulk-rm foreign `.tmp*` or `org.chromium.Chromium.*`)
- v1.0.1+ (Pass 52): multi-query `--stream` / `-f ndjson` NDJSON SearchOutput lines; dual `config` get/set/unset + `config effective`; exit 141 broken pipe (SIG_IGN SIGPIPE + oneshot reap); product config CLI+XDG only
- v1.0.2 (ADR-0027): wire serialize ENGLISH by default (`.results`, `.title`, `.metadata`, …). Legacy PT: `--wire-keys pt` or `config set wire_keys pt`. No product env, no remote telemetry.
- v0.9.4+: production Chrome-only — missing Chrome / build without feature `chrome` → exit 2 fail-closed
- Exit codes: `0` success · `1` runtime · `2` config/no Chrome · `3` block · `4` timeout · `5` zero results · `6` suspected block (v0.8.0+, `zero_cause` != legitimo) · `141` broken pipe
- JSON schema (single query, v1.0.2 EN wire):
  ```json
  {
    "query": "...", "engine": "duckduckgo", "endpoint": "html",
    "timestamp": "2026-04-14T10:00:00Z", "region": "br-pt",
    "result_count": 15, "pages_fetched": 2,
    "results": [
      {"position": 1, "title": "...", "url": "...", "snippet": "...", "display_url": "..."}
    ],
    "metadata": {
      "execution_time_ms": 1234,
      "user_agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 ...",
      "identity_used": "chrome-linux-11111111aaaa0001",
      "chrome_path_resolved": "/usr/bin/google-chrome",
      "chrome_channel": "host",
      "used_chrome": true
    }
  }
  ```
- Legacy PT wire: add `--wire-keys pt` (paths become `.resultados`, `.metadados`, …). Full renames: [MIGRATION.md](MIGRATION.md).
- Pipe safety: Unix keeps SIGPIPE as SIG_IGN so broken-pipe writes return EPIPE → exit 141 and oneshot reap still runs. Always check `${PIPESTATUS[0]}` after piped invocations.
- Path safety (v0.5.0): `--output` validates paths BEFORE writing — rejects `..` components and system directories (`/etc`, `/usr`, `C:\Windows`). Agents skip manual path sanitization
- Credential safety (v0.5.0): proxy credentials in `--proxy` URLs NEVER appear in error messages — automatic masking to `http://us***@host` format
- Typed errors (v0.5.0): `ErroCliDdg` enum with 11 variants — deterministic `exit_code()` mapping for programmatic error handling
- Anti-blocking (v0.6.0): `BrowserProfile` injects per-family `Sec-Fetch-*`, Client Hints, and RFC 7231 `Accept-Language` — agents MUST NOT add duplicate headers; HTTP 202 anomaly detection and 5 KB silent-block detection run automatically
- Adaptive anti-bot (v0.6.4+ / WS-26, preserved in v0.6.5): 12-identity pool (4 browser families × 3 platforms) with 5-level cascade rotation. On HTTP 202/403/429, the pool rotates: same identity → same family/different platform → different family/same platform → different family+platform → random. Inspect `metadata.identity_used` (EN) for diagnostic visibility. Use `--probe` for pre-flight health checks on local gates / operator hosts.
- Multi-query schema (EN): `{query_count, timestamp, parallelism, searches: [<SingleSchema>]}`

## v0.9.8 Highlights for Integrations
- GAP-WS-AGENT-READY-001 / ADR-0018 — agent-ready defaults for real Linux hosts with multi-canal Chrome.
- Default `--vertical all` — plain search returns web + news; opt out with `--vertical web` (deep: `--no-news`).
- Content fetch ON by default — cleaned text for top web + news URLs (cap 4 (v1.0.2 default)); opt out with `--no-fetch-content`. Prefer outer `timeout 180`.
- News may include `content` (EN wire since v1.0.2; legacy PT `conteudo` under `--wire-keys pt`) — same readability pipeline as web (supersedes the older “fetch only `results[]`” rule).
- Multi-canal Chrome — Flatpak export/wrapper shells resolve to deploy ELF; order: CLI `--chrome-path` → XDG `config set chrome_path` → host Chrome → host Chromium → Flatpak → Snap (`CHROME_PATH` env is NOT read).
- Transport flags `global = true` — `--chrome-path`, `--proxy`, `--vertical`, fetch flags, identity, etc. accepted BEFORE or AFTER `deep-research`.
- Honest agent metadata (not telemetry) — EN v1.0.2: `chrome_path_resolved`, `chrome_channel`, `used_chrome` (legacy PT names via `--wire-keys pt`).
- Preserve thin 0.9.7 envelope: `timeout 60 duckduckgo-search-cli -q -f json --vertical web --no-fetch-content "query"`.
- Design: [`docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md`](decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md); inventory: `gaps.md`.

## v1.0.2 Highlights for Integrations
- ADR-0027 wire EN default — parse `.results[]`, `.metadata.*` (not `.resultados` / `.metadados`).
- Legacy agents — `--wire-keys pt` or `config set wire_keys pt`.
- Agent ops — `--fields`/`--filter`/`--sort`/`--limit`/`--count-only`/… without jq.
- Discovery — `commands`, `schema`, `doctor`, `locale`, `man`.
- See [MIGRATION.md](MIGRATION.md) and [`docs/decisions/0027-wire-en-default-v1-0-2.md`](decisions/0027-wire-en-default-v1-0-2.md).

## Command inventory v1.0.6

Full CLI surface for agent wiring (source of truth: `src/cli/mod.rs` + `config_args.rs`). Wire default EN (ADR-0027).

| Command | One-liner |
| ------- | --------- |
| Default search | `duckduckgo-search-cli -q -f json "rust async" \| jaq '.results[].url'` |
| `buscar` (hidden) | `duckduckgo-search-cli buscar "rust async" -q -f json` — same as default search; omitted from `--help` |
| `init-config` | `duckduckgo-search-cli init-config --force` (`--dry-run` available) |
| `completions <SHELL>` | `duckduckgo-search-cli completions bash` |
| `deep-research` | `duckduckgo-search-cli deep-research "query" -q -f json --print-budget` |
| `commands` | `duckduckgo-search-cli commands -q` |
| `schema` | `duckduckgo-search-cli schema --name search-output` |
| `doctor` | `duckduckgo-search-cli doctor -q` / `doctor --strict` / `doctor --probe-deep` |
| `locale` | `duckduckgo-search-cli locale -q` |
| `man` | `duckduckgo-search-cli man \| man -l -` (`--file PATH` optional) |
| `config path` | `duckduckgo-search-cli config path` |
| `config list` | `duckduckgo-search-cli config list` |
| `config get` | `duckduckgo-search-cli config get wire_keys` |
| `config set` | `duckduckgo-search-cli config set wire_keys en` |
| `config unset` | `duckduckgo-search-cli config unset proxy_url` |
| `config effective` | `duckduckgo-search-cli config effective` |
| `help` | `duckduckgo-search-cli help deep-research` |
| `--probe` (ROOT FLAG, not a subcommand) | `duckduckgo-search-cli --probe -q -f json` |
| `--probe-deep` (ROOT FLAG, not a subcommand) | `duckduckgo-search-cli --probe-deep -q -f json` |

Agent ops (global; search + deep-research): `--fields`/`--select`, `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only`, `--truncate-content`, `--max-output-bytes`, `--wire-keys en|pt`.

```bash
duckduckgo-search-cli "query" -q -f json \
  --fields url,title --filter 'url~github' --sort title --limit 5
duckduckgo-search-cli "query" -q -f json --count-only
duckduckgo-search-cli "query" -q -f json --wire-keys pt   # legacy PT serialize
```

## v0.9.6 Highlights for Integrations
- One-shot process contract (GAP-WS-LIFECYCLE-001, ADR-0017) — each CLI invocation fully reaps its Chromium/Xvfb process tree on exit. Agents may invoke the binary N times without leaking Chromium/Xvfb RAM across runs.
- Cooperative cancel on SIGTERM/SIGINT — supervisors that send SIGTERM first (e.g. `timeout`, Docker stop) cancel cooperatively so the lifecycle reap path runs.
- Prefer timeouts that send SIGTERM first — use GNU `timeout` (SIGTERM, then SIGKILL after grace) rather than hard-kill-only wrappers so process cleanup can complete.
- Upgrade note from <0.9.6 — historical orphans from pre-0.9.6 runs are NOT auto-cleaned; operators may need a one-time manual kill. New runs after upgrade do not leak.
- Residual limits — SIGKILL is not interceptable; if a supervisor kills with SIGKILL immediately, reap may not run.
- No telemetry — lifecycle hardening does not emit telemetry.
- No JSON schema break — output envelope, exit codes, and flags are unchanged; drop-in for existing integrations.
- Design details: [`docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md`](decisions/0017-browser-lifecycle-one-shot-v0-9-6.md) (ADR-0017 / GAP-WS-LIFECYCLE-001).

## 1. Claude Code
- Claude Code lacks native web search and fabricates links when training data is stale.
- `duckduckgo-search-cli` gives Claude Code a deterministic, JSON-structured web search in one Bash tool call.
- Schema is stable, PT-BR validated, and parsed cleanly with `jaq` — no raw HTML to process.
- Shell mechanism: native `Bash` tool; optional hooks via `~/.claude/settings.json`.

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6 --force
duckduckgo-search-cli --version   # expect 1.0.6+
```

### Snippet — Basic search (paste in chat)
- Paste the instruction below and Claude Code executes the search immediately. v1.0.2 uses ENGLISH wire keys (`.results`, `.title`, `.content`). Legacy PT: add `--wire-keys pt`.
> "Run `timeout 180 duckduckgo-search-cli "rust async tokio" -q -f json --num 15 | jaq '.results[] | {title, url, snippet, content: (.content // "")}'` and summarize the top 5 results for me. For a thin SERP-only call use `--vertical web --no-fetch-content` with `timeout 60`."

### Snippet — Multi-query research
- Use `--queries-file` to run up to 5 parallel searches in a single invocation.
> "Create `/tmp/queries.txt` with these 5 search queries (one per line), then run:
> `timeout 120 duckduckgo-search-cli --queries-file /tmp/queries.txt -q -f json --parallel 5 -o /tmp/research.json` and read the JSON file (EN: `.searches[]`)."

### Snippet — v0.6.4+ adaptive anti-bot (preserved in v0.6.5) (WS-26)
- v0.6.4+ (preserved in v0.6.5) adds an adaptive identity pool that rotates on detected blocks (HTTP 202/403/429). Use `--probe` for pre-flight health checks and inspect `metadata.identity_used` for diagnostic visibility.
> "Before launching real queries, run `timeout 15 duckduckgo-search-cli --probe` to confirm DDG is reachable. If exit is 0, proceed: `timeout 30 duckduckgo-search-cli "QUERY" -q -f json --num 15 | jaq '.results[] | {title, url}'`. If you receive exit 3 (anti-bot block), wait 300+ seconds before retry — the cascade has already rotated up to 5 identities internally."

### System prompt rule (add to `CLAUDE.md`)
- Add this rule so Claude Code uses the CLI automatically on every web-dependent task.
> When you need fresh web context, ALWAYS use `duckduckgo-search-cli QUERY -q --num 15` instead of fabricating links or relying on training data. Parse JSON output with `jaq`.
> ALWAYS run `duckduckgo-search-cli --probe` before launching real queries in long-running sessions to detect anti-bot blocks early (v0.6.5+).
> When exit code 3 is returned, treat it as anti-bot cascade exhausted — wait 300+ seconds before retrying.

### Caveats
- Sandbox may require `cargo install` approval on first run.
- Prefer outer GNU `timeout` (SIGTERM first) and leave the CLI default 180 for dual+fetch. Use `--global-timeout 60` only with thin SERP (`--vertical web --no-fetch-content`) or when the agent step budget is strictly under 180s (then thin the path too).
- The v0.6.4+ (preserved in v0.6.5) identity pool rotates automatically — agents MUST NOT pass `--identity-profile` on shared local runners unless reproducibility is required (use `--seed` instead for deterministic rotation).

## 2. OpenAI Codex
- Codex agents invent URLs and lose accuracy on libraries released after training cutoff.
- `duckduckgo-search-cli` replaces guess-based research with a single `shell` tool call and clean JSON.
- Works in Codex CLI and IDE extensions that support `shell` / `exec` tool execution.
- Shell mechanism: `shell` / `exec` tool with approval prompt (configurable to `auto` in trusted sandboxes).

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
# Optionally set approval to 'on-failure' for faster iteration
codex config set approval on-failure
```

### Snippet — Basic search
- Give Codex this instruction to trigger a structured web search.
> "Use the shell tool to execute:
> `duckduckgo-search-cli "postgres jsonb index performance" -q --num 15 -f json`
> then extract titles + urls with jaq."

### Snippet — Multi-query research
- Batch 5 searches in one command with `--queries-file` and `--parallel 5`.
> "Write queries to `./research.txt`, then run:
> `duckduckgo-search-cli --queries-file ./research.txt -q -f json --parallel 5 --global-timeout 90 -o ./out.json`
> and show me the first 3 results per query."

### System prompt rule
- Add this to your Codex system prompt to anchor the behavior globally.
> Always prefer `duckduckgo-search-cli` (installed globally) over inventing URLs. Default to `-q --num 15 -f json` and pipe through `jaq`.

### Caveats
- Codex CLI will prompt for command approval unless sandbox mode is `workspace-write`.
- If the agent step budget is under 180s: raise the budget OR pass thin flags (`--vertical web --no-fetch-content`) plus optional `--global-timeout 60`. Otherwise leave the CLI default 180 and wrap with outer GNU `timeout` (SIGTERM first) for dual+fetch.

## 3. Gemini CLI
- Gemini CLI needs explicit shell permission and falls back to fabricated answers without a web tool.
- `duckduckgo-search-cli` satisfies `run_shell_command` with a single binary call and structured JSON output.
- No API key required — the CLI uses DuckDuckGo's public HTML endpoint.
- Shell mechanism: `run_shell_command` tool, permission-gated per command prefix.

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
gemini   # launches REPL; allow `duckduckgo-search-cli` on first prompt
```

### Snippet — Basic search
- Paste this prompt into the Gemini CLI REPL for an instant structured result.
> "Run `duckduckgo-search-cli "wasm component model 2025" -q --num 15 | jaq '.results[:5]'` and give me a bullet list of the findings."

### Snippet — Multi-query research
- Cluster results by domain using `--parallel 5` and `jaq` post-processing.
> "Create `queries.txt`, then run `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 -o /tmp/gemini_out.json` — read the file and cluster duplicate domains."

### System prompt rule (`.gemini/GEMINI.md`)
- Place this rule in `.gemini/GEMINI.md` to anchor Gemini CLI web behavior globally.
> For web facts, use the shell tool to call `duckduckgo-search-cli QUERY -q --num 15 -f json`. Never fabricate URLs.

### Caveats
- First call requires per-session approval; "allow always for this prefix" speeds subsequent runs.
- Respect your project `.gemini/settings.json` `tool_permissions` allowlist.

## 4. Cursor
- Cursor's Composer agent runs commands autonomously but has no native web search capability.
- `duckduckgo-search-cli` injects live web context directly into Composer's edit-run loop.
- One command, structured JSON, no browser — Cursor stays in the terminal, you stay in flow.
- Shell mechanism: terminal commands embedded in chat, with agent mode auto-running in Composer.

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
# Verify from Cursor's integrated terminal:
duckduckgo-search-cli --version
```

### Snippet — Basic search (Composer agent mode)
- Paste this in Composer and it executes, parses, and writes results to a file automatically.
> "Run in terminal: `duckduckgo-search-cli "tauri v2 plugin api" -q --num 15 -f json | jaq '.results[] | {title, url}'` and paste the top 5 into a `RESEARCH.md` file."

### Snippet — Multi-query research
- Feed 5 questions at once — Composer handles parallel search and summarization.
> "Create `research_queries.txt` with my 5 questions, then execute:
> `duckduckgo-search-cli --queries-file research_queries.txt -q -f json --parallel 5 -o research.json`
> — summarize each query's top-3 results."

### System prompt rule (`.cursorrules`)
- Add this rule to `.cursorrules` so Composer defaults to CLI search before any fabrication.
> Prefer running `duckduckgo-search-cli QUERY -q --num 15` before searching the web mentally. Always pipe to `jaq` and cite URLs verbatim.

### Caveats
- In `auto-run` mode, Cursor executes without asking — prefer outer GNU `timeout` (SIGTERM first) and the CLI default 180 for dual+fetch; use `--global-timeout 60` only with thin SERP (`--vertical web --no-fetch-content`) or a step budget strictly under 180s (then thin the path too).
- Keep `-q` (quiet) to avoid cluttering the agent chat buffer.

## 5. Windsurf
- Windsurf's Cascade can execute terminal commands autonomously but has no built-in web search.
- `duckduckgo-search-cli` feeds Cascade live, structured web context with a single `run_command` call.
- Whitelisting the binary in Cascade auto-approval makes every research sprint instant.
- Shell mechanism: Cascade's `run_command` / terminal proposer (user approves or auto-approves).

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
# Confirm from Windsurf terminal:
which duckduckgo-search-cli
```

### Snippet — Basic search
- Instruct Cascade to run this and save structured results for downstream use.
> "Use the terminal to run: `duckduckgo-search-cli "axum tower middleware" -q --num 15 -f json`. Parse with `jaq '.results[:5] | map({title, url})'` and save to `ctx/search.json`."

### Snippet — Multi-query research
- Run 5 parallel searches and identify top domains in a single Cascade turn.
> "Write 5 search queries to `queries.txt`, then: `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 90 -o ctx/research.json`. Read `ctx/research.json` and identify the 3 most-cited domains."

### System prompt rule (Cascade system instructions)
- Add this to Cascade's system instructions to prevent URL fabrication globally.
> When the user asks for current / web-based information, run `duckduckgo-search-cli QUERY -q --num 15 -f json` via the terminal. Never hallucinate URLs.

### Caveats
- Cascade auto-approval can be scoped per-command; whitelist `duckduckgo-search-cli`.
- Disable `--stream` in Cascade — it expects batched JSON.

## 6. Aider
- Aider's `/run` command captures stdout into chat context — the fastest path to live web data.
- `duckduckgo-search-cli` pipes structured JSON directly into Aider's context with a one-liner.
- No config required — install the binary and start using `/run` immediately.
- Shell mechanism: `/run <cmd>` slash command (captures stdout into chat context).

### Setup
```bash
pipx install aider-chat
cargo install duckduckgo-search-cli --locked --version 1.0.6
aider
```

### Snippet — Basic search (inside aider REPL)
- Run this inside the Aider REPL to inject web results into the current chat context.
```
/run duckduckgo-search-cli "sqlx postgres migrations" -q --num 15 -f json | jaq '.results[:5] | map({title, url, snippet})'
```

### Snippet — Multi-query research
- Chain query file creation, parallel search, and `jaq` filtering in a single `/run` call.
```
/run echo "rust async tokio\nsqlx postgres\naxum middleware" > /tmp/q.txt && duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 -o /tmp/r.json && jaq '.searches[] | {query, top: .results[:3] | map(.url)}' /tmp/r.json
```

### System prompt rule (`.aider.conf.yml`)
- Configure Aider to read a rules file and enforce CLI-first web search.
```yaml
read: ["AIDER.md"]
```
- Add this to `AIDER.md` to trigger the behavior on every relevant request.
> Before suggesting code that depends on external libs, run `/run duckduckgo-search-cli "<lib> <question>" -q --num 10 -f json`.

### Caveats
- `/run` output is injected into the chat — prefer `-q` and JSON to minimize tokens.
- Aider truncates long outputs; use `--num 10` and `jaq` to pre-filter.

## 7. Continue.dev
- Continue.dev slash commands pipe shell output into the chat — perfect for structured search.
- `duckduckgo-search-cli` becomes a `/ddg` slash command with 8 lines of JSON config.
- Works in VS Code and JetBrains without plugins or API keys.
- Shell mechanism: custom commands of type `run` / custom tools (via MCP or `commands` array).

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
```

### Snippet — `~/.continue/config.json` slash command
- Add this block to your Continue config to gain `/ddg` as a native slash command.
```json
{
  "slashCommands": [
    {
      "name": "ddg",
      "description": "Search the web via DuckDuckGo",
      "run": "duckduckgo-search-cli \"{{{ input }}}\" -q --num 15 -f json | jaq '.results[:5] | map({title, url, snippet})'"
    }
  ]
}
```

### Snippet — Basic search (invoke in chat)
- Trigger a structured web search with a single slash command.
```
/ddg rust async tokio patterns 2026
```

### Snippet — Multi-query research slash command
- Add this second command to run semicolon-separated research sprints.
```json
{
  "name": "research",
  "description": "Multi-query DDG research",
  "run": "echo \"{{{ input }}}\" | tr ';' '\\n' > /tmp/q.txt && duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 -o /tmp/r.json && jaq '.searches[] | {query, urls: .results[:3] | map(.url)}' /tmp/r.json"
}
```

### System prompt rule
- Add this to Continue's `systemMessage` to anchor all web searches to the CLI.
> Use `/ddg` for any web search. Never hallucinate URLs.

### Caveats
- Continue v1+ expects slash commands in `~/.continue/config.yaml` — adapt accordingly.
- For team setups, commit the config to the repo as `.continue/config.json`.

## 8. MiniMax Agent
- MiniMax's function calling maps cleanly to a shell handler — no extra adapter layer needed.
- `duckduckgo-search-cli` becomes a `web_search` tool with a 10-line Python handler.
- The stable JSON schema means MiniMax can parse `.results` without prompt engineering.
- Shell mechanism: function calling that maps to a `shell_exec` tool you implement in the harness.

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
```

### Snippet — Tool definition (pass to MiniMax API)
- Pass this tool definition to the MiniMax API to register structured web search.
```json
{
  "name": "web_search",
  "description": "Search the web via duckduckgo-search-cli and return JSON",
  "parameters": {
    "type": "object",
    "properties": { "query": { "type": "string" } },
    "required": ["query"]
  }
}
```
- Implement the handler in your harness (harness-agnostic Python example):
```python
def web_search(query):
    return subprocess.check_output(
        ["duckduckgo-search-cli", query, "-q", "--num", "15", "-f", "json"],
        timeout=180  # match CLI default since v0.9.9; use 60 only for thin SERP
    )
```

### Snippet — Multi-query (batched function call)
- Instruct MiniMax to call `web_search` in parallel for multiple topics.
> "Call `web_search` 5 times in parallel (one per topic), then merge the `results` arrays (legacy PT `resultados` under `--wire-keys pt`)."
- Alternatively, run a single multi-query command from the harness:
```bash
duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 -o out.json
```

### System prompt rule
- Add this to the MiniMax system prompt to enforce CLI-first research.
> You have a `web_search` function. Use it whenever you need current information. Always inspect `results[].url` and `snippet` before answering.

### Caveats
- Enforce a harness-side timeout (prefer 180s for dual+fetch; MiniMax will happily wait forever). Use `timeout=60` only with thin SERP (`--vertical web --no-fetch-content`) or a strict sub-180s agent budget.
- Rate-limit: keep `--parallel` <= 5 to avoid DDG 429s.

## 9. OpenCode
- OpenCode's built-in shell tool executes binaries directly — zero configuration required.
- `duckduckgo-search-cli` integrates with a single whitelist entry and delivers JSON on first call.
- Works identically to Aider but with OpenCode's own config and approval model.
- Shell mechanism: built-in `shell` tool; configurable via `~/.config/opencode/config.toml`.

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
opencode --version
```

### Snippet — Basic search (in OpenCode REPL)
- Paste this instruction into the OpenCode chat for an immediate structured result.
> "Run `duckduckgo-search-cli "tokio select cancel-safety" -q --num 15 -f json | jaq '.results[:5]'` and synthesize a one-paragraph answer."

### Snippet — Multi-query research
- Run 5 parallel searches and read the aggregated JSON file directly.
> "Create `/tmp/queries.txt` with my 5 questions, then run:
> `duckduckgo-search-cli --queries-file /tmp/queries.txt -q -f json --parallel 5 -o /tmp/opencode_research.json` and read the file."

### System prompt rule (`~/.config/opencode/prompt.md`)
- Add this rule to the OpenCode prompt file to enforce CLI-first web research.
> For web queries, ALWAYS invoke `duckduckgo-search-cli QUERY -q --num 15 -f json`. Parse JSON with `jaq`. Cite URLs verbatim.

### Caveats
- OpenCode inherits shell approvals from config — whitelist the binary.
- Disable `--stream` (OpenCode buffers stdout).

## 10. Paperclip
- Paperclip supervises child processes and enforces timeouts — `duckduckgo-search-cli` is a natural fit.
- First-party integration target: the CLI was designed with Paperclip's YAML task schema in mind.
- Register once as a capability and call it from any agent task without extra glue code.
- Shell mechanism: `bash`/`cli` capability registered in the agent manifest.

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
# In Paperclip workspace:
paperclip capability add duckduckgo-search-cli
```

### Snippet — Basic search (agent task YAML)
- Add this task definition to your Paperclip agent manifest for single-query search.
```yaml
- name: web_search
  cli: duckduckgo-search-cli
  args: ["{{query}}", "-q", "--num", "15", "-f", "json"]
  parse: json
  # Supervisor budget ≥ CLI default 180 (v0.9.9+). For thin SERP only, lower both.
  timeout: 200
```

### Snippet — Multi-query research
- Add this task for 5-query parallel research sprints with automatic JSON output.
```yaml
- name: research_sprint
  cli: duckduckgo-search-cli
  args: ["--queries-file", "{{queries_path}}", "-q", "-f", "json",
         "--parallel", "5", "--global-timeout", "120", "-o", "{{out_path}}"]
  parse: json
  timeout: 150
```

### System prompt rule (Paperclip `SYSTEM.md`)
- Add this to Paperclip's `SYSTEM.md` to anchor all factual claims to the web tool.
> Use the `web_search` capability for every factual claim. Never synthesize URLs. Prefer `--num 15` + `jaq`-style filtering.

### Caveats
- Paperclip supervises child processes with its own `timeout` field — that is NOT the CLI product default. Leave CLI `--global-timeout` at 180 for dual+fetch; set Paperclip `timeout` ≥ 180. Use `--global-timeout 60` only with thin SERP (`--vertical web --no-fetch-content`) or a strict sub-180s budget (then thin the path too). Prefer SIGTERM-first supervisors (GNU `timeout` semantics).
- For reproducible runs, pin the CLI version: `cargo install duckduckgo-search-cli --version =1.0.6`.

## 11. OpenClaw
- OpenClaw's `tools.toml` binding model means zero harness code — declare the binary, use it.
- `duckduckgo-search-cli` binds with 5 lines of TOML and becomes available as `web` and `research` tools.
- Raw JSON is passed directly to the LLM — the stable schema eliminates prompt gymnastics.
- Shell mechanism: direct binary binding via `tools.toml`.

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
```

### Snippet — `tools.toml` binding
- Add this to `tools.toml` to register `duckduckgo-search-cli` as the `web` tool.
```toml
[[tool]]
name = "web"
bin  = "duckduckgo-search-cli"
args = ["{query}", "-q", "--num", "15", "-f", "json"]
# Harness outer budget; CLI default --global-timeout is 180 since v0.9.9.
# Use 60 only for intentional thin SERP / strict agent-budget override.
timeout_secs = 180
```

### Snippet — Multi-query research
- Add a second entry to enable the `research` tool for parallel multi-query sprints.
```toml
[[tool]]
name = "research"
bin  = "duckduckgo-search-cli"
args = ["--queries-file", "{path}", "-q", "-f", "json",
        "--parallel", "5", "--global-timeout", "120", "-o", "{out}"]
timeout_secs = 150
```

### System prompt rule
- Add this to the OpenClaw system prompt to bind tool usage to factual queries.
> Use tool `web` for single queries, tool `research` for multi-query sprints. Do not invent URLs.

### Caveats
- OpenClaw passes raw JSON to the LLM — no pre-parsing; rely on the model to read `.results`.
- Pair with `jaq` in a second tool call if output exceeds the context window.

## 12. Google Antigravity
- Google Antigravity mirrors Gemini CLI's shell mechanism in an IDE-first environment.
- `duckduckgo-search-cli` integrates with one approval click and delivers structured JSON via HTTPS.
- The CLI respects corporate proxy settings — no network reconfiguration needed.
- Shell mechanism: agent shell tool (mirrors Gemini CLI's `run_shell_command`).

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
# In Antigravity, open the agent panel and allow 'duckduckgo-search-cli' on first use.
```

### Snippet — Basic search
- Give Antigravity's agent this instruction to trigger a structured search.
> "Execute: `duckduckgo-search-cli "go generics 1.22 best practices" -q --num 15 -f json | jaq '.results[:5]'` and paste findings into `NOTES.md`."

### Snippet — Multi-query research
- Run 5 parallel queries and produce a markdown table summary in a single agent turn.
> "Build `queries.txt` with 5 lines, then run:
> `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 120 -o /tmp/antigravity_research.json`
> Summarize each query's top-3 in a markdown table."

### System prompt rule (Antigravity agent settings)
- Add this to Antigravity's agent settings to prevent URL fabrication globally.
> Prefer `duckduckgo-search-cli` for any web fact. Always `--num 15 -f json`. Cite URLs verbatim.

### Caveats
- Antigravity sandboxes network calls; the CLI itself uses HTTPS and is usually whitelisted by default.
- Use `--proxy` if your org mandates egress through a corporate proxy.

## 13. GitHub Copilot CLI
- Copilot CLI suggests commands but does not execute them — the CLI bridges suggestion to structured output.
- `duckduckgo-search-cli` becomes Copilot's recommended search tool with a single shell hint.
- A `ddg-research` wrapper script enables multi-query research in a single `gh copilot suggest` invocation.
- Shell mechanism: Copilot suggests commands; the user (or a wrapper script) executes them.

### Setup
```bash
gh extension install github/gh-copilot
cargo install duckduckgo-search-cli --locked --version 1.0.6
```

### Snippet — Basic search (suggest + run)
- Ask Copilot to suggest a search command and execute the result directly.
```bash
gh copilot suggest "search the web for 'rust axum middleware tower'" --target shell
# Copilot will propose:
duckduckgo-search-cli "rust axum middleware tower" -q --num 15 -f json | jaq '.results[:5]'
```

### Snippet — Multi-query wrapper
- Save this script as `~/.local/bin/ddg-research` to enable batch search from Copilot suggestions.
```bash
# Save as ~/.local/bin/ddg-research
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" > /tmp/q.txt
duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --global-timeout 120
```
- Then ask Copilot to use the wrapper:
```bash
gh copilot suggest "use ddg-research to compare axum vs actix vs rocket"
```

### System prompt rule
- Add this to your shell profile so Copilot learns your search preference.
```bash
export GH_COPILOT_HINTS="Always prefer 'duckduckgo-search-cli QUERY -q --num 15' over ad-hoc curl."
```

### Caveats
- `gh copilot` suggests but does not auto-execute — use `eval "$(gh copilot suggest ... | tail -1)"` at your own risk.
- Requires a GitHub Copilot subscription.

## 14. Devin
- Devin's cloud VM runs `cargo install` and persists the binary across sessions via snapshots.
- `duckduckgo-search-cli` gives Devin structured web access without per-task setup cost after the first snapshot.
- Devin can create query files, run parallel searches, and produce comparison tables autonomously.
- Shell mechanism: native terminal in the Devin VM; autonomous by default.

### Setup (in Devin session)
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
# Persist to Devin's machine snapshot so future sessions reuse it:
devin snapshot save "cargo-tools"
```

### Snippet — Basic search (Devin Slack / web prompt)
- Give Devin this prompt via Slack or the web interface for an immediate search task.
> "In the shell, run: `duckduckgo-search-cli "terraform aws eks 2026 best practices" -q --num 15 -f json | jaq '.results[:5]'` and append results to `research.md`."

### Snippet — Multi-query research
- Devin handles query file creation, parallel search, and structured output autonomously.
> "Create `queries.txt` (5 lines), then execute:
> `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 120 -o research.json`
> Open `research.json` and produce a comparison table."

### System prompt rule (Devin Knowledge)
- Add this to Devin Knowledge to anchor every factual claim to CLI search.
> For every web-dependent claim, use `duckduckgo-search-cli` — never fabricate URLs. Prefer `--num 15 -f json` and parse with `jaq`.

### Caveats
- First run triggers `cargo install` (2-4 min); save a snapshot to skip that in future sessions.
- Devin may hit DDG rate-limits under high parallelism — keep `--parallel 5`.

## 15. Cline
- Cline's `execute_command` tool runs any binary in the VS Code terminal — no extensions needed.
- `duckduckgo-search-cli` becomes a whitelisted auto-approve command in under 30 seconds of setup.
- Cline can create query files, run searches, and write markdown summaries in a single autonomous turn.
- Shell mechanism: VS Code integrated terminal; `execute_command` tool with per-command approval.

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
# From a VS Code terminal that Cline can see:
duckduckgo-search-cli --version
```

### Snippet — Basic search (Cline chat)
- Paste this instruction and Cline executes the search and saves structured results automatically.
> "Use execute_command to run:
> `duckduckgo-search-cli "rust cargo workspace inheritance" -q --num 15 -f json | jaq '.results[:5] | map({title, url})'`
> and save the JSON to `./research/ws.json`."

### Snippet — Multi-query research
- Cline creates the query file, runs parallel search, and writes the markdown summary in one turn.
> "Create `./research/queries.txt` with 5 queries, then execute:
> `duckduckgo-search-cli --queries-file ./research/queries.txt -q -f json --parallel 5 --global-timeout 120 -o ./research/out.json`
> Read `out.json` and write a markdown summary to `./research/SUMMARY.md`."

### System prompt rule (`.clinerules`)
- Add this rule to `.clinerules` so every web-dependent task uses the CLI automatically.
> For any web fact, use `duckduckgo-search-cli QUERY -q --num 15 -f json`. Never hallucinate URLs. Parse JSON with `jaq`.

### Caveats
- Auto-approval whitelists: add `duckduckgo-search-cli` to "Auto-approve execute_command" in Cline settings.
- Cline truncates stdout at ~10k tokens — use `-q` + `jaq` projections to stay under budget.

## 16. Roo Code
- Roo Code's custom modes let you create a dedicated `researcher` mode with auto-approved web search.
- `duckduckgo-search-cli` integrates with 12 lines of YAML and becomes the default tool in that mode.
- Roo's multi-agent orchestrator can fan out parallel research across multiple subagents safely.
- Shell mechanism: `execute_command` tool (inherited from Cline); mode-specific approval rules.

### Setup
```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
```

### Snippet — Basic search (Roo Code chat)
- Paste this in Roo Code chat for a structured 5-result search with immediate takeaway.
> "Execute: `duckduckgo-search-cli "rust leptos signals 2026" -q --num 15 -f json | jaq '.results[:5]'` — give me a 3-bullet takeaway."

### Snippet — Multi-query research (with custom Roo mode)
- Create a custom `researcher` mode in `.roo/modes.yaml` for auto-approved parallel searches.
```yaml
- slug: researcher
  name: Web Researcher
  whenToUse: "Invoke for any fact-heavy question"
  customInstructions: |
    Always run:
      duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --global-timeout 120 -o /tmp/r.json
    before answering. Cite .results[].url verbatim.
  autoApprove: ["execute_command"]
```
- Activate the mode with `/mode researcher` in chat.

### System prompt rule (`.roorules`)
- Add this to `.roorules` to enforce CLI search in all factual contexts.
> When in `researcher` mode (or whenever factual grounding is needed), use `duckduckgo-search-cli`. Always JSON + jaq.

### Caveats
- Per-mode auto-approval: scope `execute_command` tightly to the CLI prefix.
- Roo's multi-agent orchestrator may fan out — cap `--parallel 5` globally to respect DDG limits.

## Comparative Table / Tabela Comparativa
| # | Agent | Shell tool | Best for | Snippet complexity |
|---|---|---|---|---|
| 1 | Claude Code | Bash tool nativo | Terminal-first, hooks, CI/CD | one-liner |
| 2 | OpenAI Codex | shell/exec | Codebase refactors, tests | multi-step |
| 3 | Gemini CLI | run_shell_command | Google Cloud, Gemini power users | multi-step |
| 4 | Cursor | Terminal + Composer | IDE devs, fast edit/run loops | one-liner |
| 5 | Windsurf | Cascade run_command | Autonomous refactors | multi-step |
| 6 | Aider | `/run` | Git-native pair programming | one-liner |
| 7 | Continue.dev | Custom slash | Multi-editor teams | JSON config |
| 8 | MiniMax | Function calling | API-first apps | function handler |
| 9 | OpenCode | Shell | OSS terminal agents | multi-step |
| 10 | Paperclip | Agent capability | Paperclip workflows | YAML config |
| 11 | OpenClaw | tools.toml binding | Minimalist zero-config | TOML config |
| 12 | Google Antigravity | Agent shell | Experimental / preview users | multi-step |
| 13 | GitHub Copilot CLI | `gh copilot suggest` | Gh/Git-centric workflows | wrapper script |
| 14 | Devin | Cloud sandbox | Long-running autonomous tasks | multi-step |
| 15 | Cline | execute_command | VS Code autonomous agents | multi-step |
| 16 | Roo Code | execute_command + modes | Power users, multi-mode orchestration | YAML mode |

- Legend: one-liner = single command / trivial · multi-step = requires a few commands · JSON/YAML/TOML config = requires a config file · function handler = requires a harness function

## See also / Veja também
- Main README: [`../README.md`](../README.md)
- Changelog: [`../CHANGELOG.md`](../CHANGELOG.md)
- Issue tracker: [github.com/danilo-aguiar-br/duckduckgo-search-cli/issues](https://github.com/danilo-aguiar-br/duckduckgo-search-cli/issues)


## v0.7.0 — Deep Research for AI Agents

For agents that need multi-hop answers (Claude Code, OpenAI Codex, Gemini CLI, Cline, Roo Code, Devin, Paperclip, Antigravity), the `deep-research` subcommand fans out one user query into 1..=12 sub-queries, aggregates the results, and optionally emits a Markdown report ready for the context window.

```bash
timeout 120 duckduckgo-search-cli -q -f json deep-research "question" \
  --synthesize --synth-format markdown --budget-tokens 1500 \
  | jaq -r '.synthesis'
```

The new subcommand inherits every global flag (`-q -f json`, `--num`, `--lang`, `--country`, `--parallel`, `--endpoint`, `--proxy`, `--retries`, `--global-timeout`, `--fetch-content`, `--max-content-length`) and adds the deep-research-specific knobs above. The output schema is documented in `docs/AGENTS-GUIDE.md` and stable across the v0.7.x line.


## v0.7.3 — Session + Probe-Deep for AI Agents

For agents that hit `quantidade_resultados: 0` or HTTP 200 with empty body in v0.7.2 (the GAP-WS-27 macOS CAPTCHA), v0.7.3 ships:

- Cookie persistence + warm-up (session feature): each invocation now starts with a `GET https://duckduckgo.com/` warm-up that populates session cookies, persisted to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) with Unix permissions `0o600`. Opt out with `--no-warmup` or `--no-cookie-persistence`.
- CAPTCHA interstitial detection (probe-deep feature): `--probe-deep` runs a real search query and classifies the body as `ok` or `captcha` based on Cloudflare and DuckDuckGo markers. The probe report includes `status`, `cascade_reason`, `mitigation_suggestion`, `http_status`, and `latency_ms`. Use this flag on local macOS hosts before launching real queries to detect early signs of the CAPTCHA.
- `--allow-lite-fallback` (legacy no-op since v0.9.4): kept for argv compatibility; does NOT force Lite or remediate exit 3 (GAP-WS-113). Production is Chrome-only HTML SERP.

Recommended local gate for macOS runners:

```bash
# Step 1: health check (existing)
timeout 15 duckduckgo-search-cli --probe

# Step 2: deep CAPTCHA check (new in v0.7.3)
timeout 30 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'

# Step 3: real query
timeout 60 duckduckgo-search-cli "rust async tokio" -q -f json --num 10 | jaq '.results[].url'
```

If step 2 reports `status: "captcha"`, the operator should rotate proxy/identity and re-check with Chrome `--probe-deep`. Do NOT rely on `--allow-lite-fallback` (no-op since v0.9.4).


## v0.7.4 — Windows NASM preflight for AI agents

v0.7.4 adds a build.rs preflight that detects nasm.exe on PATH for Windows MSVC native builds. Without NASM, the build fails in seconds with the exact fix rather than after minutes of cryptic CMake errors. The DDG_SKIP_NASM_CHECK=1 env var is an escape hatch for custom build environments.

For AI agents running Windows build steps:

```bash
# Step 1: verify toolchain on Windows runner
pwsh scripts/check-windows-toolchain.ps1 --json
# Returns: {"all_present": true, "tools": [{"name": "nasm", "found": true, ...}]}
# Exit 0 if all 7 tools present, 1 otherwise.

# Step 2: install missing tools (run as Administrator)
pwsh scripts/install-windows.ps1
# Auto-installs NASM, CMake, Perl via winget (choco fallback).
# For MSVC, prints the exact Launch-VsDevShell.ps1 invocation.

# Step 3: re-run cargo install
cargo install duckduckgo-search-cli --version 0.7.4 --force
```

If the preflight aborts with a NASM assembler not found panic, do not retry. The fix is environment-level, not transient.

## v0.7.5 — 4 tools preflight + INSTALL-WINDOWS for AI agents

v0.7.5 extends the v0.7.4 preflight to all four tools the BoringSSL build needs on Windows MSVC: NASM, CMake 3.20+ (with the C++ CMake tools for Windows sub-component), MSVC C/C++ toolchain (cl.exe/link.exe), and Strawberry Perl. The preflight now fails fast for any of the four missing tools with actionable messages.

New helper scripts and docs:
- scripts/install-windows.ps1 — auto-installs what can be auto-installed (NASM, CMake, Perl); reports MSVC with Launch-VsDevShell.ps1 instruction.
- scripts/check-windows-toolchain.ps1 — standalone diagnostic; exit 0 = all 7 tools present, 1 = gap.
- docs/INSTALL-WINDOWS.md — 5 installation methods, troubleshooting for each GAP, all 4 escape hatches documented.

For AI agents:

```bash
# Step 1: comprehensive diagnostic
pwsh scripts/check-windows-toolchain.ps1 --json
# Reports cargo, rustc, cmake, nasm, cl.exe, link.exe, perl

# Step 2: if any of {cmake, nasm, cl.exe, link.exe, perl} missing,
# run the auto-installer
pwsh scripts/install-windows.ps1

# Step 3: open Developer PowerShell for VS 2022 (sets PATH, INCLUDE, LIB)
# Then re-run the install
cargo install duckduckgo-search-cli --version 0.7.5 --force
```

Escape hatches for non-standard installations:
- DDG_SKIP_NASM_CHECK=1 — skip NASM preflight
- DDG_SKIP_CMAKE_CHECK=1 — skip CMake preflight
- DDG_SKIP_MSVC_CHECK=1 — skip MSVC preflight
- DDG_SKIP_PERL_CHECK=1 — skip Perl preflight

Never use these as first-line workarounds. They exist for hermetic build environments where the tool is intentionally outside the scanned PATH.
## v0.7.6 — cargo install fix (GAP-WS-48)

`cargo install duckduckgo-search-cli --version 0.7.6` broke on 2026-06-14 due to a transitive `alloc-no-stdlib 2.0.4 vs 3.0.0` conflict in the freshly-resolved lock. Reproduction: 36 `E0277` errors at `enc/reader.rs`, `enc/writer.rs`, `enc/combined_alloc.rs` when DDG's request reached the brotli alloc trait boundary.

The CLI shipped with the fix already applied: dead `wreq-util` direct dep removed; `brotli` feature dropped from `wreq` because DDG never sends `Content-Encoding: br`. Validation: `cargo tree | rg 'brotli|alloc-no-stdlib|alloc-stdlib|wreq-util'` returns 0 matches; `cargo install --path . --offline` succeeds in 35.7s.

Impact for AI agents:

```bash
# CI / fresh machine workflow (no lock yet on target)
cargo install duckduckgo-search-cli --version 0.7.6 --locked
# --locked is mandatory: without it, the solver pulls the 2026-06-14
# broken versions and reproduces the original failure.
```

- 6 fewer crates in the dep graph (~-160KB binary surface)
- `cargo install` 5–10 seconds faster than v0.7.5
- Zero CLI changes, zero JSON schema changes
- gz/deflate/zstd still enabled; Accept-Encoding still advertises them

For agent pipelines: prefer the `cargo install … --locked` pattern whenever a `--version` pin is required to prevent the solver from re-introducing the broken transitive state.


## v0.7.7 — TLS fingerprint restoration (GAP-WS-49)

v0.7.6 closed `cargo install` but the published binary passed all smoke tests while real queries returned `resultados: 0` with `cascade_level: 0` — silent TLS fingerprint regression. Root cause: `wreq 6.0.0-rc.29` has no native `emulation` feature; the Chrome/Safari TLS fingerprint emulation lived only in `wreq-util 3.0.0-rc.12`. Without it, BoringSSL produces a JA3/JA4 fingerprint detectable by Cloudflare Bot Management; DDG serves `anomaly-modal` (45 occurrences in HTML body).

Cross-confirmation: direct `curl` with real browser headers (Chrome/120 UA, br Accept-Encoding, kl=br-pt cookie, Sec-Fetch-*) ALSO received `anomaly-modal` on 2026-06-14 09:25 UTC — the tightening is upstream and persistent. The 1-request `--probe-deep` does NOT trigger it because DDG fingerprints by volume/behavior, not single request.

For AI agents:

```bash
# Re-add the emulation deps the v0.7.6 cleanup deleted
cargo install duckduckgo-search-cli --version 0.7.7 --locked
# --locked is mandatory; v0.7.7's Cargo.lock pins
# brotli-decompressor =5.0.1, alloc-no-stdlib =2.0.4
```

- 8/8 local queries restored to 5+ results
- Binary +160KB (brotli 8.0.3 + brotli-decompressor 5.0.1 + wreq-util 3.0.0-rc.12)
- `cargo install` build time ~24s (faster than v0.7.6 because 5.0.1 is smaller than 5.0.2)
- 3 new crates in the supply chain (brotli family + wreq-util)


## v0.7.8 — Anti-bot detector overhaul (GAP-WS-50..57)

v0.7.8 closes 8 gaps clustered around the anti-bot detector chain. The headline change: `detect_interstitial` in `src/probe_deep.rs` now recognizes the `anomaly-modal` interstitial DDG rolled out on 2026-06-14 (was returning exit 5 with `resultados: 0` silently). New markers: `anomaly-modal`, `anomaly-modal__title`, `anomaly.js?cc=botnet`, `cf-turnstile`, `cf-mitigated`, `Unfortunately, bots use DuckDuckGo too.`; legacy markers preserved for compatibility with pre-2026 templates.

Related fixes:

- GAP-WS-51: probe-deep calibration query is now the 9-word `the quick brown fox jumps over the lazy dog` (constant `PROBE_CALIBRATION_QUERY` in `src/probe.rs`). The previous `q=rust` short query did not trigger upstream bot scoring and gave false-positive `ok` status.
- GAP-WS-52: `--allow-lite-fallback` historically consulted `detect_interstitial(&first_html) != InterstitialKind::None` instead of `accumulated_results.is_empty()`. v0.9.4 / GAP-WS-113: the flag is a legacy NO-OP (no Lite success path in production).
- GAP-WS-53: `-v` now accepts multiple occurrences via `ArgAction::Count`. Mapping: no flag → `info` (or XDG `log_directive`); `-v` → `debug`; `-vv`+ → `trace`; `-q` → `off`. Product log filter is CLI `-v`/`-q` + XDG `log_directive` (not `RUST_LOG` product config).
- GAP-WS-54: `scraper` bumped 0.20.0 → 0.27.0. Resolves transitive `fxhash 0.2.1` (RUSTSEC-2025-0057, unmaintained). `cargo audit --deny warnings` gate added to local gates.
- GAP-WS-55: stale comment about a non-existent `wreq 5.3.0` regression rewritten in `Cargo.toml:69-86`. New text documents the real pin strategy (6.0.0-rc.29 + 3 direct pins).
- GAP-WS-56: `buscar` subcommand now has `#[command(hide = true)]`. Top-level invocation path remains canonical; help output no longer duplicated.
- GAP-WS-57: `--retries N` flag is now honored in `src/parallel/mod.rs` (the module was a single `src/parallel.rs` file when the gap was closed). Previously the value was hard-coded to 1; now `cfg.retries` is propagated with clamp `[1, 10]` to prevent `--retries 999` from triggering anti-bot defenses.

For AI agents: zero breaking changes to the JSON schema or exit codes. 305 tests (292 lib + 13 integration) all passing. The detector update is the only behavioral change visible to operator-facing JSON: `metadados.cascade_reason` may now contain `interstitial_cloudflare` or `interstitial_ddg` on exit 3 responses.


## v0.9.1 — v0.9.3 — Stealth Hardening & macOS/Windows Headless
- v0.9.3 (GAP-WS-112): macOS/Windows switched to Chrome `headless=new` (`ChromeHeadMode::Headless`); Linux keeps a private Xvfb display. v0.9.1's headed-native Quartz/DWM approach was reverted because it produced a visible window
- v0.9.2 (GAP-WS-108): chromiumoxide `--enable-automation` removed via `.disable_default_args()` — eliminates the "managed by automated tests" banner
- v0.9.2 (GAP-WS-109): `Emulation.setUserAgentOverride` with coherent `UserAgentMetadata` — `navigator.userAgent`, `sec-ch-ua` header and `userAgentData.brands` now all report the real installed Chrome major version
- v0.9.2 (GAP-WS-110): WebRTC leak prevention — `--force-webrtc-ip-handling-policy=disable_non_proxied_udp` + `--disable-webrtc-hw-decoding`
- v0.9.2 (GAP-WS-111): `--disable-quic` — UDP no longer bypasses the proxy

## v1.0.1 — Pass 52 agent contract (stream / dual config / 141 / ADR-0023)

v1.0.1 (Pass 52) hardens the agent contract without a JSON schema break on PT wire keys:

- Multi-query `--stream` / `-f ndjson` IMPLEMENTED — one NDJSON `SearchOutput` line per completed query; single-query ignores with warning
- Dual `config` API: positional or `--key`/`--value` for get/set/unset; `config effective` dumps CLI > XDG > defaults
- Broken pipe → exit 141; SIG_IGN SIGPIPE so `ensure_oneshot_cleanup` runs
- Wire: Portuguese serialize BC + English deserialize aliases (ADR-0023)
- Product config CLI + XDG ONLY; Chrome-only CDP; no remote telemetry

## v1.0.0 — Disk one-shot + auditable profile prefix (GAP-WS-TMP-PROFILE-ORPHAN-001)

v1.0.0 (GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020) completes process one-shot with DISK honesty:

- Chrome `user-data-dir` uses prefix `ddg-chrome-*` (Unix `0o700`), not generic `.tmp*`
- `force_reap` removes the profile directory after process kill; `ExitReapGuard` + panic hook + timeout/end-of-run reap
- Next-run `sweep_orphan_profiles` cleans ONLY stale owned `ddg-chrome-*`
- Hard policy: never bulk-delete foreign `.tmp*` or `org.chromium.Chromium.*`
- deep-research inherits the main `CancellationToken` so SIGTERM cancels fan-out
- No telemetry and no JSON schema break vs 0.9.10/0.9.9
- Design: [`docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md`](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md)

## v0.9.6 — One-shot process ownership (GAP-WS-LIFECYCLE-001)

v0.9.6 (GAP-WS-LIFECYCLE-001 / ADR-0017) hardens one-shot process ownership for Chromium and Xvfb:

- Each CLI invocation fully reaps its Chromium/Xvfb process tree on exit (`process_lifecycle`: process group, tree walk, `user-data-dir` marker; Linux also uses `setpgid` + `PR_SET_PDEATHSIG`)
- `ChromeBrowser` shutdown + `Drop` force-reap; `XvfbGuard` RAII cleanup of Xvfb lock/socket
- SIGTERM cancels the cooperative `CancellationToken` (in addition to SIGINT) so supervisors like GNU `timeout` allow the reap path to run
- Atomic writes for output, config, and cookie jar
- Upgrade note: historical orphans from pre-0.9.6 runs are NOT auto-cleaned; operators may need a one-time manual kill. New runs after upgrade do not leak.
- Residual limit: SIGKILL is not interceptable — immediate hard-kill may skip reap
- No telemetry and no JSON schema break — envelope, exit codes, and flags are unchanged; still Chrome-only production (v0.9.4 / GAP-WS-113)
- Design details: [`docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md`](decisions/0017-browser-lifecycle-one-shot-v0-9-6.md)

## v0.9.4 — Chrome-only universal for AI agents (GAP-WS-113)

v0.9.4 (GAP-WS-113 / ADR-0016) makes production FAIL-CLOSED on Chrome:

- All production network ops (search, news, `deep-research`, `--probe`, `--probe-deep`, `--pre-flight`, `--fetch-content`) require chromiumoxide/CDP
- Missing Chrome or binary without feature `chrome` → exit 2 (no auto `--no-news`, no web HTTP success path; GAP-WS-106 auto-degrade superseded). Historical runtime `NO_CHROME` env kill-switch is NOT product config.
- `--allow-lite-fallback` is a legacy NO-OP
- Residual HTTP only under `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`
- Local validation hosts must install Chrome/Chromium (and Xvfb on headless Linux); this repo is CI-less (`NO_CI.md`)

## v0.9.0 — CLI ergonomics for AI agents (GAP-WS-106)

v0.9.0 (GAP-WS-106) improved the parser ergonomics with zero schema impact:

- Nine flags hoisted to `global = true`: `-n`, `-f`, `-o`, `-t`, `-l`, `-c`, `-p`, `-q`, `-v` — they may now appear BEFORE or AFTER the `deep-research` subcommand. Snippets like `duckduckgo-search-cli "query" -q -o out.json` parse cleanly.
- Actionable clap errors: when a known flag appears in the wrong position, the stderr message appends a PT-BR hint pointing to the correct placement (no more opaque `unexpected argument`).
- Historical note: v0.9.0 introduced auto-degradation without Chrome (auto `--no-news` / vertical→web). Superseded in v0.9.4 by fail-closed exit 2 (GAP-WS-113).
- No JSON schema changes; no new envelope fields; no new exit codes from GAP-WS-106 itself.

## v0.8.9 — News vertical (`--vertical`) for AI agents

v0.8.9 (GAP-WS-104) adds a news vertical via `--vertical <web|news|all>` (historical default `web`; v0.9.8 default is `all`). The `news` and `all` verticals are Chrome-only — there is NO HTTP fallback. Since GAP-WS-105 multi-query batches are accepted — one Chrome session per query — and `deep-research` scans news by DEFAULT (opt-out `--no-news`). Since v0.9.4 (GAP-WS-113) without usable Chrome the CLI fails closed with exit 2.

Envelope contract for agents:

- `.news[].{position,title,url}` — guaranteed non-null in every news item (legacy PT `{posicao,titulo,url}` under `--wire-keys pt`).
- `.news[].{source,relative_date,thumbnail}` — optional (`Option<String>`); always apply the `// ""` fallback in `jaq` (legacy PT `{fonte,data_relativa,thumbnail}` under `--wire-keys pt`).
- `.news_count` and `.metadata.vertical_used` — present ONLY when vertical != web; web mode output stays byte-identical to v0.8.8.
- New ZeroCause variant `vertical-no-results` — a news/all search with zero hits is legitimate and emits exit 5 (not exit 6). Legacy PT value `vertical-sem-resultados` is still accepted on deserialize.
- Exit-code accounting uses the total `results + news_count` (legacy PT `resultados + quantidade_noticias` under `--wire-keys pt`).
- v0.9.8 supersession: content fetch default ON applies to web + news (cap 4 (v1.0.2 default)); opt out with `--no-fetch-content`. Historical “fetch only `results[]`” is superseded.

Canonical formula:

```bash
timeout 90 duckduckgo-search-cli --vertical news "query" -q -f json | jaq '.news'
```

News RAG pipeline (guaranteed fields + optional fallbacks):

```bash
timeout 90 duckduckgo-search-cli --vertical news "rust 1.88 release" -q -f json \
  | jaq -r '.news[] | [.position, .title, .url, (.source // ""), (.relative_date // "")] | @tsv'
```

Combined web + news in a single Chrome pass (`--vertical all`):

```bash
timeout 90 duckduckgo-search-cli --vertical all "query" -q -f json \
  | jaq '{web: [.results[].url], news: [.news[].url]}'
```

For AI agents (v0.9.8): default vertical is already `all` with content fetch ON. Prefer `timeout 180` for default dual+fetch; use `--vertical web --no-fetch-content` for thin SERP-only multi-query sprints. Read `chrome_path_resolved` / `chrome_channel` as agent metadata, not telemetry (legacy PT `chrome_path_resolvido` / `chrome_canal` under `--wire-keys pt`).

- Maintainer: Danilo Aguiar ([@danilo-aguiar-br](https://github.com/danilo-aguiar-br)) · License: MIT OR Apache-2.0
