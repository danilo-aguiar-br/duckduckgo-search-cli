### Flags

> **SSOT:** generated from `duckduckgo-search-cli --help` and subcommand `--help` on binary **v1.0.5** (66 root flags declared in the option column, plus deep/doctor/init/man exclusives and the hidden aliases `--region` and `--max-concurrency`). Prefer `commands` / `schema` for low-token agent discovery. Portuguese prose lives **only** in [`README.pt-BR.md`](README.pt-BR.md) — this file is English-only.

#### Root / default search (complete inventory from `--help`)

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

#### `deep-research` only (in addition to global root flags)

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

#### `doctor` only

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `--strict` | off | Doctor: fail closed on non-OK checks. |

#### `init-config` only

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `--force` | off | init-config: overwrite existing config files. |
| `--dry-run` | off | init-config: simulate without writing files. |

#### `schema` only

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `--name` | (none) | schema: emit a named schema body instead of the catalog. |

#### `man` only

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `--file` | (none) | man: write roff to this path (atomic). Uses `--file`, not `-o`. |
