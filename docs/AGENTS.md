# duckduckgo-search-cli — Agent Instructions

[Português (Brasil)](AGENTS.pt-BR.md)

## Rule Zero
- Read this document COMPLETELY before invoking `duckduckgo-search-cli`.
- ALL your invocations MUST be in TOTAL conformance with the rules here.
- Violations result in execution errors, blocked pipelines, and lost results.
- Rule Zero applies to every call, every script, every pipeline, without exception.


## Overview
- `duckduckgo-search-cli` is a Rust CLI for DuckDuckGo search via Chrome/CDP (production Chrome-only since v0.9.4, GAP-WS-113).
- Designed for consumption by LLMs and AI agents in automated pipelines.
- Structured output in JSON, Markdown, plain text, or TSV.
- Exit codes are semantically defined for precise error handling.
- Version: **v0.9.8** (GAP-WS-AGENT-READY-001 / ADR-0018 agent-ready defaults; GAP-WS-LIFECYCLE-001 one-shot; GAP-WS-113 Chrome-only fail-closed) — MSRV: Rust 1.88.


## Installation
- Install via Cargo: `cargo install duckduckgo-search-cli`
- Verify installation: `duckduckgo-search-cli --version`
- Update to latest: `cargo install duckduckgo-search-cli --force`


## Quick Start
- MANDATORY: ALWAYS wrap with `timeout` and pass `-q -f json`:

```bash
timeout 30 duckduckgo-search-cli -q -f json --num 15 "rust async runtime"
```

- Parse output with `jaq` — NEVER with `jq` or text tools:

```bash
timeout 30 duckduckgo-search-cli -q -f json --num 10 "query" | jaq -r '.resultados[].url'
```

- Check exit code BEFORE parsing:

```bash
timeout 60 duckduckgo-search-cli -q -f json --num 15 "query" > /tmp/out.json
case $? in
  0) jaq '.resultados[].url' /tmp/out.json ;;
  3) echo "blocked, backing off 300s"; sleep 300 ;;
  4) echo "global timeout, increase --global-timeout" ;;
  5) echo "zero results, rephrase query" ;;
  6) echo "suspected block, inspect .metadados.causa_zero" ;;
  *) echo "unexpected error" ;;
esac
```


## Flags Reference
- `-q, --quiet` — silence tracing logs; stdout carries only the payload
- `-f, --format <FORMAT>` — output format: `json` (MANDATORY in scripts), `markdown`, `text`, `tsv`
- `-n, --num <N>` — number of results per page (default 15, max 30)
- `--pages <N>` — number of pages to fetch (default 2, auto-paginates)
- `--parallel <N>` — concurrent requests for multi-query (MUST be ≤ 5)
- `--queries-file <FILE>` — file with one query per line for batch mode
- `--fetch-content` / `--no-fetch-content` — content fetch is **ON by default** since v0.9.8 (top web + news URLs, cap 10; N× latency). Opt out with `--no-fetch-content`; `--fetch-content` remains valid as an explicit opt-in
- `--max-content-length <N>` — cap bytes fetched per page (recommended whenever fetch is on)
- `--output <FILE>` — write JSON to file with path safety validation
- `--endpoint <html|lite>` — search endpoint (default `html`; production SERP is Chrome HTML only — do **not** use `lite` as exit-3 remediation, GAP-WS-113)
- `--vertical <web|news|all>` — search vertical (**default `all` since v0.9.8**). Opt out with `--vertical web`. `news`/`all` are Chrome-only (NO HTTP fallback); multi-query batches accepted since GAP-WS-105 (one Chrome session per query); `deep-research` scans news by DEFAULT (opt-out `--no-news`); without a usable Chrome (missing binary, feature off, or `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`) production **fails closed with exit 2** — no auto `--no-news`, no web downgrade (v0.9.4, GAP-WS-113); `--pre-flight` is skipped on the news vertical
- `--chrome-path <PATH>` — global transport flag (works before or after `deep-research`); Flatpak multi-canal resolve on Linux (export shell → deploy ELF)
- `--global-timeout <SECS>` — total timeout in seconds for all queries (MUST be < external `timeout`)
- `--per-host-limit <N>` — max concurrent requests per host (default 2, MUST NOT exceed 2)
- `--retries <N>` — number of retries with exponential backoff (default 2)
- `--timeout <SECS>` — per-request timeout in seconds
- `--proxy <URL>` — FORBIDDEN in argv; use `HTTPS_PROXY` env var instead
- `--no-proxy` — bypass all proxy configuration
- `--lang <LANG>` — language filter (e.g. `en-us`, `pt-br`)
- `--country <CC>` — country filter (e.g. `us`, `br`)
- `--time-filter <d|w|m>` — time filter: day, week, month
- `--stream` — FORBIDDEN: placeholder flag, NOT implemented
- `-v, --verbose` — verbose output for diagnostics; since v0.7.8 accepts multiple occurrences (`-vv` = debug, `-vvv` = trace) via `ArgAction::Count`. `RUST_LOG` env var still overrides.
- `deep-research <QUERY>` — query fan-out subcommand (v0.7.0)
- `--max-sub-queries <N>` — maximum sub-queries produced (1..=12, default 5)
- `--sub-query-strategy <heuristic|manual>` — sub-query generation strategy
- `--sub-queries-file <PATH>` — read explicit sub-queries (manual strategy)
- `--aggregate <rrf|dedupe-by-url>` — aggregation algorithm
- `--depth <N>` — reflection rounds (planned, not executed in v0.7.0)
- `--synthesize` — produce final Markdown/PlainText/JSON report
- `--budget-tokens <N>` — token budget for the synthesis report
- `--synth-format <markdown|plain-text|json>` — synthesis output format
- `--probe-deep` — run a real search query and classify the body as `ok` or `captcha` (v0.7.3+)
- `--no-warmup` — skip the `GET https://duckduckgo.com/` warm-up before the first real query (v0.7.3+)
- `--no-cookie-persistence` — keep cookies in memory only; never write `cookies.json` to disk (v0.7.3+)
- `--cookies-path <PATH>` — override the default XDG cookie jar path (v0.7.3+)
- `--allow-lite-fallback` — **legacy no-op** since v0.9.4 (GAP-WS-113); kept for CLI compatibility, does not force Lite or remediate exit 3


## Exit Codes
| Code | Meaning | Agent Action |
|------|---------|--------------|
| `0` | Success | Parse `.resultados` |
| `1` | Runtime error | Read stderr; retry once with `-v` |
| `2` | Config error **or** Chrome missing / `NO_CHROME=1` | Fix args; install Chrome / unset `DUCKDUCKGO_SEARCH_CLI_NO_CHROME`; run `init-config --force` if needed |
| `3` | Anti-bot block | Back off 300+ s; rotate proxy / identity; re-run `--probe-deep` (Chrome). Do **not** rely on `--allow-lite-fallback` (no-op since v0.9.4) |
| `4` | Global timeout | Raise `--global-timeout`; reduce `--parallel` |
| `5` | Zero results (includes `vertical-sem-resultados` on `--vertical news`, v0.8.9) | Refine query; try different `--lang` or `--country` |
| `6` | Suspected block (`causa_zero != legitimo`, v0.8.0+) | Inspect `.metadados.causa_zero`; use `--pre-flight` |


## Core Invariants
### MANDATORY — Always Follow Without Exception
- ALWAYS pass `-q` in every pipeline that parses stdout
- ALWAYS specify `-f json` explicitly in every script
- ALWAYS wrap every invocation with `timeout` using integer seconds
- ALWAYS treat the CLI as a **one-shot process owner** — N sequential agent invocations MUST NOT accumulate Chromium/Xvfb from this CLI after a normal or cooperative exit (v0.9.6, GAP-WS-LIFECYCLE-001)
- ALWAYS prefer external wrappers that send **SIGTERM first** (e.g. GNU `timeout`, which SIGTERMs then SIGKILLs) rather than an immediate SIGKILL-only kill, so cooperative cancel and full Chromium/Xvfb tree reap can run
- ALWAYS check `$?` or `${PIPESTATUS[0]}` before parsing stdout
- ALWAYS pin `--num` explicitly; NEVER rely on defaults
- ALWAYS use `--queries-file` for batch work; NEVER shell loops
- ALWAYS use `jaq` for JSON parsing; NEVER `jq` or text tools
- ALWAYS use `--output` for large result sets (≥ 50 results)
- ALWAYS prefer `--endpoint html` (Chrome HTML SERP); NEVER remediate exit 3 with `--endpoint lite` or `--allow-lite-fallback`
- ALWAYS use `--retries` for exponential backoff; NEVER shell retry loops
### FORBIDDEN — Never Violate
- NEVER omit `-q` from any piped invocation
- NEVER use `--stream` (placeholder, not implemented)
- NEVER raise `--parallel` above 5
- NEVER raise `--per-host-limit` above 2
- NEVER pass proxy credentials in argv (use `HTTPS_PROXY` env var)
- NEVER parse `text` or `markdown` output with machines
- NEVER execute result URLs without sandboxing
- NEVER ignore non-zero exit codes
- NEVER set `--global-timeout` equal to or greater than the external `timeout`
- NEVER inject custom `Sec-Fetch-*` or `Accept-Language` headers (v0.6.0 handles these)
- NEVER assume residual Chrome/Xvfb after a clean **0.9.6+** exit is "normal" — the only residual cases are external SIGKILL of the CLI process, or historical orphans left by pre-0.9.6 runs


## JSON Output Contract
### MANDATORY — Guaranteed Non-Null Fields
- `.resultados[].titulo` — always present when `resultados` is non-empty
- `.resultados[].url` — always present when `resultados` is non-empty
- `.resultados[].posicao` — always present when `resultados` is non-empty
- `.quantidade_resultados` — preferred over `(.resultados | length)`
- `.metadados.tempo_execucao_ms` — canonical latency signal
- `.metadados.usou_endpoint_fallback` — `true` signals IP reputation degradation
### MANDATORY — Optional Fields Require Fallbacks
- `.resultados[].snippet` is `Option<String>` — ALWAYS use `// ""` fallback
- `.resultados[].url_exibicao` is `Option<String>` — ALWAYS use `// .url` fallback
- `.resultados[].titulo_original` is `Option<String>` — ALWAYS use `// .titulo` fallback
- Content fields (`.conteudo`, `.tamanho_conteudo`) — common on top results when fetch is on (**default ON** since v0.9.8, cap 10); absent when `--no-fetch-content` is passed
- `.metadados.chrome_path_resolvido`, `.metadados.chrome_canal` — agent contract fields (**not** telemetry); honest `.metadados.usou_chrome`
### MANDATORY — News Vertical Fields (v0.8.9+, defaults v0.9.8)
- `.noticias[].posicao`, `.noticias[].titulo`, `.noticias[].url` — guaranteed when `--vertical news|all` returns articles (default vertical is **`all`**)
- `.noticias[].fonte`, `.noticias[].data_relativa`, `.noticias[].thumbnail` — `Option<String>`, ALWAYS use `// ""` fallback
- News may also carry `conteudo` / `tamanho_conteudo` / `metodo_extracao_conteudo` when fetch is on (default ON)
- `.quantidade_noticias` and `.metadados.vertical_usada` — commonly present under default `all`; omit news with `--vertical web`
- Zero articles on a rendered news SERP → `causa_zero: vertical-sem-resultados` (legitimate zero, exit 5, NOT 6)
- **CR4c (GAP-WS-113):** body ≥4KB without a result-page signal is never `causa_zero: legitimo` — treat as `zero-resultados-suspeito` / exit 6
- Canonical formula: `timeout 90 duckduckgo-search-cli --vertical news "query" -q -f json | jaq '.noticias'`
- Preserve thin 0.9.7 envelope: `timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content -q -f json "query"`

```bash
jaq '.resultados[] | {
  titulo,
  url,
  snippet: (.snippet // ""),
  url_exibicao: (.url_exibicao // .url)
}'
```

### MANDATORY — Single vs Multi-Query Root
- Single query root: `{ query, resultados, metadados }`
- Multi-query root: `{ quantidade_queries, buscas: [{ query, resultados, metadados }] }`
- NEVER access `.resultados` directly on a multi-query response

```bash
# single query
duckduckgo-search-cli -q -f json "one" | jaq '.resultados | length'
# multi-query
duckduckgo-search-cli -q -f json "one" "two" | jaq '.buscas[0].resultados | length'
```


## Rate Limiting and Etiquette
### MANDATORY — Stay Below Anti-Bot Threshold
- MUST cap `--parallel` at 5 (default); values above 5 trigger HTTP 202 anti-bot
- MUST keep `--per-host-limit` at 2 (default); values above 2 increase block probability
- MUST use built-in `--retries` with exponential backoff; NEVER shell retry loops. Since v0.7.8 the `--retries N` flag is fully honored in `src/parallel.rs::execute_with_retry` (was hard-coded to 1 in v0.7.7 and earlier).
- MUST calculate `--global-timeout` as `(queries / parallel) * avg_secs * 1.5`
- Exit code `3` requires 300+ second backoff window before retry
- Since v0.7.8: `detectar_interstitial` in `src/probe_deep.rs` recognizes the `anomaly-modal` template that DDG rolled out in 2026-06; exit 3 with `cascata_motivo` set is the honest signal.


## Batch Mode
### MANDATORY — Use queries-file for All Batch Work
- NEVER loop over queries in shell; pay process startup cost once
- ALWAYS use `--queries-file` to reuse connection pools and UA rotation
- ALWAYS set `--global-timeout` appropriate to batch size

```bash
printf 'query one\nquery two\nquery three\n' > /tmp/q.txt
timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q --parallel 3 -f json
```


## Pipe Integrity
### MANDATORY — Detect Upstream Failure in Pipes
- In `cmd | jaq`, the shell reports only `jaq`'s exit code
- MUST check `${PIPESTATUS[0]}` after every piped invocation

```bash
timeout 60 duckduckgo-search-cli "query" -q -f json | jaq '.resultados[].url'
ddg_exit=${PIPESTATUS[0]}
if [ "$ddg_exit" -ne 0 ]; then echo "CLI failed: exit $ddg_exit" >&2; fi
```


## Content Fetching
### MANDATORY — Content Fetch Is ON by Default (v0.9.8)
- Content fetch is **ON by default** (top web + news URLs, FETCH_CAP=10) via **Chrome/CDP** (N× latency; GAP-WS-113 / ADR-0018)
- Opt out with `--no-fetch-content` when you only need SERP metadata
- MUST pass `--max-content-length` to cap memory consumption when bodies are wanted
- MUST reduce `--num` and raise outer `timeout` (prefer 120–180s) when fetch is on

```bash
# Default agent-ready path (dual vertical + clean text)
timeout 180 duckduckgo-search-cli -q -f json --num 5 --max-content-length 5000 "query"
# Thin SERP-only path
timeout 60 duckduckgo-search-cli -q -f json --num 5 --vertical web --no-fetch-content "query"
```


## Security Rules
### MANDATORY — Protect Credentials and Execution
- NEVER pass proxy credentials in argv (visible in `/proc/*/cmdline`, `ps`, shell history)
- ALWAYS use `HTTPS_PROXY` or `HTTP_PROXY` environment variables for proxy credentials
- NEVER execute URLs from `.resultados[].url` without sandboxing (SSRF and code execution risk)
- ALWAYS run `init-config --dry-run` before `init-config --force` in CI pipelines
- TRUST v0.5.0 path validation for `--output`; NEVER implement manual `realpath` checks
- TRUST v0.6.0 browser fingerprint profiles; NEVER inject `Sec-Fetch-*` or `Accept-Language` headers

```bash
# FORBIDDEN
duckduckgo-search-cli "query" --proxy http://user:pw@host:8080
# MANDATORY
export HTTPS_PROXY="http://user:pw@host:8080"
duckduckgo-search-cli "query" -q
```

### MANDATORY — Proxy Precedence Order
- `--no-proxy` overrides all other proxy settings
- `--proxy <URL>` overrides environment variables
- `HTTPS_PROXY` / `HTTP_PROXY` environment variables override no-proxy default
- None: direct connection


## Anti-Patterns
### FORBIDDEN — Patterns That Silently Break Pipelines
- Parsing text output with `rg` instead of `jaq` on JSON
- Shell loops instead of `--queries-file` for batch queries
- Ignoring exit codes before piping to `jaq`
- Assuming `snippet` is non-null without `// ""` fallback
- Hardcoding proxy credentials in argv
- Raising `--parallel` to 20 to increase throughput (triggers exit code 3)
- Using `--stream` (placeholder, undefined behavior)
- Invoking without `timeout` wrapper (pipeline hangs indefinitely)
- Setting `--global-timeout` equal to external `timeout` (CLI never terminates cleanly)
- Hardcoding `--identity-profile` instead of letting the pool adaptively rotate (v0.6.4+)
- Reading `.metadados.identidade_usada` as a guarantee when it is `Option<String>` (v0.6.4+)
- Reading `.metadados.nivel_cascata` as a guarantee when it is `Option<u32>` (v0.6.4+)
- Skipping `duckduckgo-search-cli --probe` before launching real queries in CI


## v0.7.0 — Deep Research Subcommand

### MANDATORY — Use the New Subcommand for Multi-Hop Research

For questions that benefit from query fan-out ("compare X vs Y in 2026", "history of Z", "what changed in library W"), use the `deep-research` subcommand instead of running a single search.

```bash
timeout 60 duckduckgo-search-cli -q -f json deep-research "best rust http client 2026" \
  | jaq '.resultados[] | {titulo, url, score}'
```

### MANDATORY — Deep Research Output Schema

- Top-level JSON has three keys: `metadados`, `resultados`, and optional `sintese`
- `.metadados.query_original` is the user's input
- `.metadados.sub_queries[]` lists every generated sub-query with `texto`, `estrategia`, `status`, `elapsed_ms`
- `.metadados.total_resultados_unicos` is the deduplicated count
- `.metadados.tempo_total_ms` is the end-to-end latency
- `.resultados[].score` is a normalised `[0.0, 1.0]` value — higher is better
- `.resultados[].fontes[]` lists the sub-queries that produced the result (traceability)
- `.sintese` is present only when `--synthesize` is enabled

```bash
# Extract a Markdown report (when --synthesize is on)
timeout 120 duckduckgo-search-cli -q -f json deep-research "topic" \
  --synthesize --synth-format markdown | jaq -r '.sintese'
```

### MANDATORY — Manual Sub-Queries File

When `--sub-query-strategy manual` is set, the CLI reads sub-queries from `--sub-queries-file PATH`. Comments (`#`) and blank lines are ignored. The file MUST contain at least one non-comment line.

```bash
cat > /tmp/qs.txt <<EOF
# Visão geral
what is tokio runtime 2026
# Comparação
tokio vs async-std
EOF

timeout 60 duckduckgo-search-cli -q -f json deep-research "tokio 2026" \
  --sub-query-strategy manual --sub-queries-file /tmp/qs.txt
```

### MANDATORY — Inherits Global Flags

`deep-research` accepts every global flag (`-n`, `--lang`, `--country`, `--parallel`, `--endpoint`, `--retries`, `--timeout`, `--global-timeout`, `--proxy`, `--fetch-content`, `--max-content-length`). The deep-research-specific knobs are layered on top.

### MANDATORY — Token Budget for Synthesis

`--budget-tokens` uses the heuristic 1 token ≈ 4 chars. The synthesis report is hard-capped at 20 references. Set `--budget-tokens 0` to disable the cap and rely on the 20-reference ceiling.

## v0.6.4 and v0.6.5 — Adaptive Anti-Bot Identity Pool (WS-26)

### MANDATORY — Recognize the New Flags
- `--probe` — pre-flight health check. MUST be used in CI before launching real queries.
- `--identity-profile <auto|chrome-win|chrome-mac|chrome-linux|edge-win|firefox-linux|safari-mac>` — pins the session to a specific identity. `auto` (default) rotates adaptively.
- `--seed <u64>` — deterministic seed for UA selection AND identity pool rotation.

### MANDATORY — Read the New Metadata Fields
- `.metadados.identidade_usada` — `Option<String>` — identity tag that produced the response (format `<family>-<platform>-<16hex>`)
- `.metadados.nivel_cascata` — `Option<u32>` (0..=4) — cascade level reached during the request

```bash
# Check which identity produced a response
timeout 30 duckduckgo-search-cli -q -f json "query" | jaq '.metadados.identidade_usada // "auto"'

# Diagnose repeated blocks via cascade level
timeout 30 duckduckgo-search-cli -q -f json "query" | jaq '.metadados.nivel_cascata // 0'
```

### MANDATORY — Anti-Bot Cascade Strategy
When exit code `3` is encountered, the CLI has already rotated through up to 5 identities internally. If `--identity-profile auto` is in effect and exit code `3` persists, the agent MUST:
1. Wait 300+ seconds before retry (the cascade level reached indicates how exhausted the pool is)
2. Rotate proxy with `--proxy socks5://127.0.0.1:9050` and/or let the identity pool adapt
3. Re-run `--probe-deep` (Chrome) to classify the interstitial
4. If the problem persists, file a bug with the `nivel_cascata` value captured — do **not** use `--allow-lite-fallback` (no-op since v0.9.4)

### MANDATORY — Probe Before Real Queries
```bash
# CI pipeline gate
timeout 15 duckduckgo-search-cli --probe || { echo "DDG blocked at network level, aborting" >&2; exit 1; }
timeout 30 duckduckgo-search-cli -q -f json "real query"
```


## Build
- Development build: `cargo build`
- Release build: `timeout 600 cargo build --release`
- Check compilation: `timeout 120 cargo check --all-targets`
- Cross-compilation targets: see `docs/CROSS_PLATFORM.md`


## Test
- Run all tests: `timeout 300 cargo nextest run`
- Run doc tests separately: `cargo test --doc`
- Run E2E integration tests: `timeout 300 cargo test --test integracao_pipeline`
- Run with all features: `timeout 300 cargo test --all-features`
- Minimum coverage: 80% — NEVER merge below this threshold


## Lint
- Run Clippy with warnings as errors: `timeout 180 cargo clippy --all-targets --all-features -- -D warnings`
- ZERO warnings are tolerated in production code
- Fix all Clippy suggestions before opening a pull request


## Format
- Check formatting: `cargo fmt --all --check`
- Apply formatting: `cargo fmt --all`
- ZERO differences are tolerated in commits
- Run format check in CI before any other gate


## Coverage
- Run with text report: `cargo llvm-cov --text`
- Run with HTML report: `cargo llvm-cov --html`
- Minimum target: 80% line coverage
- Recommended for new code: 90% line coverage
- Coverage gates apply to every pull request without exception


## Audit
- Check vulnerabilities: `timeout 120 cargo audit`
- Check licenses and supply chain: `timeout 120 cargo deny check advisories licenses bans sources`
- ZERO vulnerabilities are tolerated in releases
- Run audit in CI on every push to main


## Full Validation Sequence
- Run all 10 commands below in order before any release:
- `cargo fmt --all --check` — ZERO differences
- `timeout 180 cargo clippy --all-targets --all-features -- -D warnings` — ZERO warnings
- `timeout 120 cargo check --all-targets` — ZERO errors
- `RUSTDOCFLAGS="-D warnings" timeout 120 cargo doc --no-deps --all-features` — ZERO warnings
- `timeout 300 cargo nextest run` — ZERO failures
- `cargo llvm-cov --text` — minimum 80% coverage
- `timeout 120 cargo audit` — ZERO vulnerabilities
- `timeout 120 cargo deny check advisories licenses bans sources` — ZERO violations
- `timeout 120 cargo publish --dry-run --allow-dirty` — ZERO errors
- `cargo package --list` — ZERO sensitive files


## LLM Integration Patterns
### MANDATORY — Canonical Patterns for AI Agents
- Use `-q -f json` as the ONLY machine-readable output contract
- Use `jaq` as the ONLY JSON parser in pipelines
- Use `timeout` as the ONLY mechanism to bound execution time
- Use `${PIPESTATUS[0]}` as the ONLY way to detect upstream CLI failure
- Use `--queries-file` as the ONLY batch invocation mechanism
- Use environment variables as the ONLY credential storage mechanism

```bash
# Canonical agent invocation pattern
timeout 60 duckduckgo-search-cli -q -f json --num 15 "$QUERY" > /tmp/ddg_out.json
ddg_exit=$?
if [ "$ddg_exit" -ne 0 ]; then
  echo "DDG failed with exit $ddg_exit" >&2
  exit "$ddg_exit"
fi
jaq -r '.resultados[] | "\(.posicao): \(.titulo) — \(.url)"' /tmp/ddg_out.json
```

### MANDATORY — Context Loading Pattern for LLMs
- Content fetch is ON by default; cap length and timeout:

```bash
timeout 180 duckduckgo-search-cli -q -f json \
  --num 5 --max-content-length 5000 \
  "$QUERY" | jaq '.resultados[] | {titulo, url, conteudo: (.conteudo // "")}'
```

### MANDATORY — Multi-Query Pattern
- Use `--queries-file` with `--parallel 3` for batched LLM research:

```bash
printf '%s\n' "${QUERIES[@]}" > /tmp/queries.txt
timeout 300 duckduckgo-search-cli \
  --queries-file /tmp/queries.txt \
  -q -f json --parallel 3 --per-host-limit 1 --retries 3 \
  --global-timeout 280 > /tmp/multi_out.json
jaq -r '.buscas[].resultados[].url' /tmp/multi_out.json | sort -u
```


## Error Handling Reference
### MANDATORY — Full Handler Template

```bash
run_ddg() {
  local query="$1"
  local outfile="$2"
  timeout 60 duckduckgo-search-cli -q -f json --num 15 "$query" > "$outfile"
  local ec=$?
  case $ec in
    0) return 0 ;;
    3) echo "BLOCKED: anti-bot. Wait 300s and rotate proxy." >&2; return 3 ;;
    4) echo "TIMEOUT: raise --global-timeout." >&2; return 4 ;;
    5) echo "ZERO_RESULTS: rephrase query." >&2; return 5 ;;
    *) echo "ERROR($ec): check stderr." >&2; return "$ec" ;;
  esac
}
```

### MANDATORY — Pipe Integrity Template

```bash
timeout 60 duckduckgo-search-cli "query" -q -f json | jaq '.resultados[].url'
ddg_exit=${PIPESTATUS[0]}
[ "$ddg_exit" -eq 0 ] || { echo "CLI failed: exit $ddg_exit" >&2; exit "$ddg_exit"; }
```


## Configuration Files
- Config location: `$XDG_CONFIG_HOME/duckduckgo-search-cli/` (default `~/.config/duckduckgo-search-cli/`)
- Override location: `DUCKDUCKGO_SEARCH_CLI_HOME` environment variable
- `selectors.toml` — CSS selectors for HTML parsing
- `user-agents.toml` — User-Agent rotation pool
- Initialize config: `duckduckgo-search-cli init-config`
- Safe update: `duckduckgo-search-cli init-config --dry-run` then `--force`
- Reject `..` components in paths: automatic in v0.5.0+


## Quick Reference Card

| Rule | Instruction |
|------|-------------|
| R01 | MUST pass `-q` when piping to any parser |
| R02 | MUST specify `-f json` explicitly in scripts |
| R03 | NEVER parse `text` or `markdown` with machines |
| R04 | MUST pin `--num` explicitly |
| R05 | MUST cap `--parallel` at 5 |
| R06 | MUST use `--output` for large result sets |
| R07 | NEVER invoke without `timeout` |
| R08 | MUST use `--queries-file` for batch work |
| R09 | NEVER use `--stream` (not implemented) |
| R10 | MUST prefer `--endpoint html` (Chrome); NEVER remediate exit 3 with Lite |
| R11 | MUST distinguish single vs multi-query JSON root |
| R12 | MUST treat `titulo` and `url` as guaranteed non-null |
| R13 | NEVER assume optional fields are present |
| R14 | MUST use `${PIPESTATUS[0]}` to detect pipe failures |
| R15 | NEVER pass proxy credentials in argv |
| R16 | NEVER execute result URLs without sandboxing |
| R17 | NEVER inject `Sec-Fetch-*` headers (v0.6.0 handles them) |
| R18 | MUST run `duckduckgo-search-cli --probe-deep` before real queries on macOS runners to detect CAPTCHA early (v0.7.3+) |
| R19 | MUST treat the cookie jar (`cookies.json`) as a credential; opt out with `--no-cookie-persistence` (v0.7.3+) |
| R20 | MUST treat `--allow-lite-fallback` as a **legacy no-op** (v0.9.4, GAP-WS-113); it is not a remediation for exit 3 and does not force Lite |


## v0.7.3 — Session + Probe-Deep + BoringSSL (GAP-WS-27 fix)

### MANDATORY — Recognize the New Flags
- `--probe-deep` — runs a real search query and reports `status: "ok"` or `status: "captcha"`. Use this in CI gates for macOS runners to detect Cloudflare Bot Management interstitials before launching expensive pipelines.
- `--no-warmup` — skip the `GET https://duckduckgo.com/` warm-up that populates session cookies.
- `--no-cookie-persistence` — keep cookies in memory only; never write `cookies.json` to disk.
- `--cookies-path <PATH>` — override the default XDG cookie jar path. Use this to point at an encrypted volume.
- `--allow-lite-fallback` — **legacy no-op** since v0.9.4 (GAP-WS-113). Does not force Lite and is not an exit-3 remediation.

### MANDATORY — Build Prerequisites Changed (v0.8.6+ / production Chrome v0.9.4)
- v0.8.6+ does **not** require `cmake`, `perl`, NASM, or MSVC. Residual HTTP TLS is pure Rust via `reqwest` + `rustls-tls` under the test harness only.
- Production network transport is **Chrome-only** (feature `chrome` default; GAP-WS-113): `--probe`, `--probe-deep`, `--pre-flight`, `--fetch-content`, search, news, and `deep-research` all require a usable Chrome/Chromium. `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` fails closed with **exit 2**.

### MANDATORY — Treat the Cookie Jar as a Credential
- The `session` feature persists DuckDuckGo session cookies to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) with Unix permissions `0o600`. Read the file with the same care you would read an API key.

Upstream: https://github.com/danilo-aguiar-br/duckduckgo-search-cli
Schema contract valid for `duckduckgo-search-cli` **v0.9.8** (stable core since v0.7.0; news vertical v0.8.9; global flags v0.9.0; Chrome-only fail-closed GAP-WS-113; one-shot lifecycle GAP-WS-LIFECYCLE-001 / ADR-0017; agent-ready defaults GAP-WS-AGENT-READY-001 / ADR-0018 — default vertical `all`, fetch ON, additive metadata `chrome_path_resolvido` / `chrome_canal` / honest `usou_chrome`).
See `docs/AGENTS.pt-BR.md` for the Portuguese version.

## v0.9.8 — Agent-ready defaults (GAP-WS-AGENT-READY-001 / ADR-0018)

### MANDATORY — Defaults changed for agents
- Default `--vertical` is **`all`** (web + news). Opt out with `--vertical web`; deep-research uses `--no-news` to skip news.
- Content fetch is **ON by default** for top web + news (cap 10). Opt out with `--no-fetch-content`.
- Prefer `timeout 180` (or higher for deep-research) when accepting default fetch.
- Read agent metadata with fallbacks: `.metadados.chrome_path_resolvido // ""`, `.metadados.chrome_canal // ""`, `.metadados.usou_chrome // false` — these are **not** telemetry.
- `--chrome-path` and other transport flags are global (valid after `deep-research`).
- Flatpak multi-canal Chrome is supported on Linux (export shell → deploy ELF).
- Still Chrome-only production (v0.9.4) and one-shot process ownership (v0.9.6); atomwrite; no telemetry.


## v0.9.6 — One-shot lifecycle (GAP-WS-LIFECYCLE-001)

### MANDATORY — Process ownership model
- Each CLI invocation is **NASCE → EXECUTA → MORRE**: it owns the full Chromium/Xvfb process tree for that run and reaps it on success, error, timeout, SIGINT, and SIGTERM.
- Reap is implemented via `process_lifecycle`, `XvfbGuard`, cooperative `shutdown`, and `Drop` (process group / marker / tree kill). See `docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md` (ADR-0017).
- Prefer GNU `timeout` (SIGTERM, then SIGKILL after grace) so the CLI can cancel cooperatively before hard kill.
- Atomic writes apply to `--output`, config, and the cookie jar (same schema; no JSON contract change vs 0.9.5).

### Residual limits (honest)
- External **SIGKILL** of the CLI may leave orphans (OS limit — cooperative cancel never runs).
- Historical orphans from **pre-0.9.6** runs are **not** auto-cleaned; one-time host cleanup is optional for those only — not a required per-run step after 0.9.6 for *new* leaks.


## v0.7.6 — Cargo install fix (GAP-WS-48)

### MANDATORY — Build with Locked Lockfile
- `cargo install duckduckgo-search-cli --locked` is the MANDATORY install path.
- The lockfile pins `alloc-no-stdlib = "=2.0.4"` and `brotli-decompressor = "=5.0.1"` to avoid the GAP-WS-48 collision.
- `brotli` decoding was removed from the feature set (DDG never sends `Content-Encoding: br`).
- Build time dropped from ~37s to ~24s after brotli removal.

### MANDATORY — Verify the Install
- Run `cargo tree | rg 'brotli|alloc-no-stdlib|alloc-stdlib|wreq-util'` and confirm zero matches before launching real queries.
- Run a real query (`duckduckgo-search-cli "rust async runtime" -q -f json`) and confirm `quantidade_resultados >= 5`.

### FORBIDDEN
- NEVER run `cargo install` without `--locked` on a fresh system; the lockfile regeneration triggers GAP-WS-48.


## v0.7.7 — TLS fingerprint restoration (GAP-WS-49)

### MANDATORY — Verify the TLS Stack
- Confirm `wreq 6.0.0-rc.29` and `wreq-util 3.0.0-rc.12` are present via `cargo tree`.
- The `emulation` feature of `wreq-util` produces the JA4_o fingerprint that bypasses DDG anti-bot.
- Three direct pins in `Cargo.toml` must remain: `wreq-util 3.0.0-rc`, `brotli-decompressor =5.0.1`, `alloc-no-stdlib =2.0.4`.

### MANDATORY — Use `--locked` for Install
- The `Cargo.lock` shipped with v0.7.7 contains `cargo update -p alloc-no-stdlib@3.0.0 --precise 2.0.4`.
- `cargo install --version 0.7.7 --locked` is the supported path.

### FORBIDDEN
- NEVER downgrade to v0.7.6 without checking `gaps.md` and `docs/decisions/0001-tls-boring-via-wreq.md`.
- NEVER run real queries before verifying the binary contains the BoringSSL stack.


## v0.7.8 — Anti-bot detector overhaul + detector flags

### MANDATORY — Recognize the New Detector Markers
- `detectar_interstitial` in `src/probe_deep.rs` now recognizes 8 new Cloudflare markers including `anomaly-modal` and `anomaly.js?cc=botnet`.
- 1 new DDG marker: `anomaly-modal__title`.
- 8 unit tests in `src/probe_deep.rs::tests` validate each marker with HTML fixtures.

### MANDATORY — Probe-Deep Calibration Query
- The probe-deep query is the 9-word pangram `the quick brown fox jumps over the lazy dog` (constant `PROBE_CALIBRATION_QUERY` in `src/lib.rs`).
- The previous 1-word query `rust` returned the DDG home page without triggering the bot detector, producing false-negative probe results.

### MANDATORY — `--allow-lite-fallback` Is a No-Op (v0.9.4)
- Since GAP-WS-113 the flag is accepted for backward compatibility but does **not** force Lite or change transport.
- Exit 3 remediations: backoff, proxy/identity rotation, Chrome health via `--probe-deep` — never rely on `--allow-lite-fallback`.

### MANDATORY — `-v` Accumulates
- `-v` is now `ArgAction::Count` in `src/cli.rs`.
- Mapping: `-v` = info, `-vv` = debug, `-vvv` = trace.
- `RUST_LOG` env var still overrides.

### MANDATORY — `--retries N` Now Honored
- The `cfg.retries` value is propagated to `execute_with_retry` in `src/parallel.rs:644`.
- Clamp in `[1, 10]` to prevent `--retries 999` from triggering anti-bot.
- The pre-v0.7.8 bug ignored the flag (hard-coded to 1).

### MANDATORY — `buscar` Subcommand Hidden
- `duckduckgo-search-cli buscar` is still invokable but hidden from `--help` since v0.7.8.
- Top-level invocation remains the canonical entry point.

### FORBIDDEN
- NEVER treat silent zero-result outcomes (exit 5) as query quality issues; check if the detector flagged an interstitial first.
- NEVER use `--retries` above 10; the v0.7.8+ clamp rejects it.
- NEVER parse `duckduckgo-search-cli --help` output for CI scripts that expect `buscar` to be listed.
