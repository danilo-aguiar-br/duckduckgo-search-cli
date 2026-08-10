---
name: duckduckgo-search-cli-en
description: This skill MUST auto-activate for live web search, internet research, current docs, grounding, URL verification, page extraction, RAG enrichment, fact-checking, multi-hop research, fresh news, dual web plus news deep-research, health checks, query batches, or data outside the knowledge cutoff. It MUST teach Chrome-only duckduckgo-search-cli with fail-closed exit 2 without Chrome, English wire keys by default, agent ops --fields --filter --limit --sort --dedupe-by --count-only --truncate-content, dual config API, ZeroCause exits 5 and 6, broken pipe 141, probe, probe-deep, pre-flight, doctor, schema, commands, locale, man, init-config, completions, config, ONE-SHOT Chromium Xvfb ddg-chrome hygiene, SIGTERM-first timeout, jaq, exits 0-6 and 130/141/143, proxy via CLI and XDG only, plus ready formulas for every flag and subcommand. Triggers include search the web, look up, find online, fetch URL, deep research, recent news, pricing. ALWAYS invoke proactively without tool naming. NEVER invent results.
---

# Skill — duckduckgo-search-cli (EN)

## Rule Zero
- You MUST run `duckduckgo-search-cli` for any live external fact outside the knowledge cutoff
- You MUST NEVER invent titles, URLs, snippets, news, or page bodies
- You MUST ALWAYS wrap agent runs with GNU `/usr/bin/timeout` (SIGTERM first)
- You MUST ALWAYS use `-q -f json` in agent pipelines unless multi-query NDJSON stream is intentional
- You MUST ALWAYS parse stdout with `jaq` — NEVER `jq`
- You MUST ALWAYS capture the CLI exit code before parsing — with pipes use `${PIPESTATUS[0]}`
- You MUST install or refresh with `cargo install duckduckgo-search-cli --locked --force`

## Mission and Triggers
- You MUST operate this CLI as the deterministic search primitive for agents
- You MUST cover web, news, dual vertical, deep-research, URL verification, page extraction, RAG enrichment, and fact-checking
- You MUST invoke proactively on search the web, look up, find online, fetch URL, verify sources, deep research, multi-hop, compare entities, what changed, recent news, current pricing, health check, or batch queries
- You MUST invoke for documentation lookup and factual grounding even when the user never names the tool
- You MUST NOT invoke for local formatting, pure refactor, or offline-only tasks
- Product config is CLI flags plus XDG `config.toml` only — NEVER product env kill-switches

## Contract — REQUIRED
- Production is Chrome-only via chromiumoxide and CDP for search, news, deep-research, probe, probe-deep, pre-flight, doctor, and content fetch
- Without usable Chrome or Chromium, or without feature `chrome`, exit 2 fail-closed — NEVER silent pure-HTTP downgrade — NEVER auto `--no-news`
- ONE-SHOT lifecycle — each run owns Chromium, Xvfb on Linux, and a temp profile prefix `ddg-chrome-*` — cooperative exit reaps process tree and removes that profile — next run sweeps only stale owned `ddg-chrome-*`
- Chrome audio is ALWAYS muted (`--mute-audio` plus autoplay policy) — NEVER invent unmute flags or PulseAudio wrappers; page media must not play on host speakers
- Residual disk audit MUST use only `find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'ddg-chrome-*' 2>/dev/null`
- FORBIDDEN bulk find or rm of `.tmp*` or `org.chromium.Chromium.*` as hygiene for this CLI
- Unix broken pipe yields exit 141 and oneshot cleanup still runs — NEVER treat 141 as a search failure
- Chrome path order MUST be CLI `--chrome-path` then XDG `chrome_path` then auto host Flatpak Snap — NEVER teach `CHROME_PATH` as a product knob
- `.metadata.chrome_channel` values MUST be exactly `manual|host|flatpak|snap` — NEVER `env`
- DEFAULT vertical is `all` — DEFAULT content fetch is ON for web and news under `--fetch-content-cap` default 4 — opt out with `--no-fetch-content`
- `--stream` is ALLOWED for multi-query NDJSON lines — `-f ndjson` is the stream alias — single-query `--stream` is ignored with a warning — NEVER treat stream as a full SERP hit event stream
- Dual config API MUST accept positional and flag forms — `config get KEY` or `config get --key KEY` — `config set KEY VALUE` or `config set --key KEY --value VALUE` — `config unset KEY` or `config unset --key KEY` — `config effective` shows CLI greater than XDG greater than defaults
- ALLOWED_KEYS only (full list live `config list`) — `ui_lang`, `chrome_path`, `proxy_url`, `default_global_timeout`, `default_vertical`, `fetch_content_default`, `log_directive`, `default_lang`, `default_country`, `default_max_sub_queries`, `default_fetch_content_cap`, `deep_research_allow_under_budget`, `budget_serp_seconds`, `budget_fetch_seconds`, `budget_safety_margin_percent`, `budget_contention_low`, `budget_contention_high`, `budget_contention_factor_mid_percent`, `budget_contention_factor_high_percent`, `deep_research_auto_contention_budget`, `deep_research_timeout_grace_seconds`, `budget_profile`, `default_parallelism`, `chrome_session_retries`, `default_sort`, `default_dedupe_by`, `max_output_bytes`, `default_content_truncate`, `allow_no_warmup`, `linux_cgroup_enabled`, `linux_cgroup_memory_max_mb`, `default_timeout`, `default_retries`, `default_pages`, `default_num_results`, `default_max_content_length`, `default_per_host_limit`, `default_cancel_grace_secs`, `wire_keys` — NEVER invent config keys
- Wire JSON serializes **English** field names by default — `results`, `title`, `metadata`, `result_count`, `chrome_channel`, `chrome_path_resolved`, `used_chrome`, `execution_time_ms` — Portuguese aliases still deserialize; legacy PT emit via `--wire-keys pt` or `config set wire_keys pt`
- Budget dual multiproc — `--print-budget`, `--auto-contention-budget`, `--no-auto-contention-budget`, `--allow-under-budget`, `budget_profile` (`lab`|`desktop_contended`|`thin`)
- Product log filter precedence MUST be `-q` greater than `-v` or `-vv` greater than XDG `log_directive` greater than `info` — NEVER teach `RUST_LOG` as product config
- Proxy MUST be CLI `--proxy` or `--no-proxy` or XDG `proxy_url` only — NEVER `HTTP_PROXY` `HTTPS_PROXY` `ALL_PROXY`
- Atomic `--output` only — FORBIDDEN paths with `..` or system directories
- MANDATORY outer timeout seconds — dual plus fetch 180 — SERP-only 90 — thin web-only 60 — deep-research (defaults ≤180 gated; full mode 600+) — batch 300 — probe 15 — probe-deep 20 — doctor 30
- Defaults — num 15 — format auto (agents MUST force json unless intentional multi-query stream) — timeout 15s — lang pt — country br — parallel 5 clamp 1..20 — pages 1 — retries 2 — endpoint html — vertical all — safe-search moderate — identity-profile auto — max-content-length 10000 — fetch-content-cap 4 — per-host-limit 2 — global-timeout 180 — cancel-grace-secs 5 — max-sub-queries 3 — aggregate rrf — depth 0 — budget-tokens 4000 — budget gate fail-fast exit 2

## Workflow
1. MUST choose mode — search, news, deep-research, batch, stream, probe, pre-flight, doctor, or SERP-only
2. MUST build the formula with GNU `timeout`, `-q`, `-f json`, and mode flags
3. MUST execute and capture exit code plus stdout — with pipes MUST use `${PIPESTATUS[0]}`
4. IF exit 0 with results — MUST extract with `jaq` and cite sources
5. IF zero results — MUST read `.metadata.zero_cause` and `.metadata.next_action_suggestion` before retry
6. IF exit non-zero — MUST branch on the Exit Codes section below and apply its remediation verbatim
7. After each run MUST assume Chromium Xvfb and `ddg-chrome-*` profile were reaped

## Execution Formulas — every flag and mode
MUST copy and adapt. ALWAYS keep `-q -f json` in agent pipelines unless multi-query stream is intentional.

### Base modes
- Dual plus fetch MANDATORY — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- SERP-only faster — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- Thin web-only — `timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content "QUERY" -q -f json`
- News-only — `timeout 180 duckduckgo-search-cli --vertical news "QUERY" -q -f json`
- Positional multi-query — `timeout 120 duckduckgo-search-cli -q -f json "query one" "query two"`
- Stdin multi-query — `printf '%s\n' "q1" "q2" | timeout 120 duckduckgo-search-cli -q -f json`
- Multi-query stream, `-f ndjson` is the alias — `timeout 120 duckduckgo-search-cli -q --stream "q1" "q2"`

### Search surface flags
- `-n` `--num` default 15 min 1 — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --num 30`
- `-f` `--format` values `json|text|markdown|md|tsv|ndjson|auto` — agents ALWAYS force json unless multi-query stream — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `-o` `--output` atomic write — FORBIDDEN `..` and system dirs — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --output /tmp/results.json`
- `-t` `--timeout` per request default 15s — `timeout 180 duckduckgo-search-cli --timeout 20 "QUERY" -q -f json`
- `-l` `--lang` SERP language default `pt` — NOT UI language — `timeout 180 duckduckgo-search-cli --lang en-US "QUERY" -q -f json`
- `-c` `--country` default `br` — `timeout 180 duckduckgo-search-cli --country us "QUERY" -q -f json`
- `--region` alias of `--country` — `timeout 180 duckduckgo-search-cli --region us "QUERY" -q -f json`
- `-p` `--parallel` default 5 clamp 1..20 — against anti-bot MUST stay at or below 5 — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--max-concurrency` alias of `--parallel` — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --max-concurrency 3 -q -f json`
- `--queries-file` one query per line — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --num 15`
- `--pages` 1..5 default 1 — auto-elevates when `--num` greater than 10 — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --num 20 --pages 3`
- `--retries` 0..10 default 2 — `timeout 180 duckduckgo-search-cli --retries 3 "QUERY" -q -f json`
- `--disable-retry` kills native retries — `timeout 180 duckduckgo-search-cli --disable-retry "QUERY" -q -f json`
- `--endpoint` `html|lite` default html — production SERP is HTML Chrome — lite does NOT remediate — `timeout 180 duckduckgo-search-cli --endpoint html "QUERY" -q -f json`
- `--vertical` `all|web|news` default all — `timeout 180 duckduckgo-search-cli --vertical news "QUERY" -q -f json`
- `--shared-session-verticals` share one Chrome session for dual — `timeout 180 duckduckgo-search-cli --shared-session-verticals "QUERY" -q -f json`
- `--time-filter` `d|w|m|y` — `timeout 180 duckduckgo-search-cli --time-filter d "QUERY" -q -f json`
- `--safe-search` `off|moderate|on` default moderate — `timeout 180 duckduckgo-search-cli --safe-search off "QUERY" -q -f json`
- `--identity-profile` `auto|chrome-win|chrome-mac|chrome-linux|edge-win|firefox-linux|safari-mac` default auto — NEVER hardcode in CI — `timeout 180 duckduckgo-search-cli --identity-profile chrome-linux "QUERY" -q -f json`
- `--stream` multi-query NDJSON only — `timeout 120 duckduckgo-search-cli -q --stream "q1" "q2"`
- `-v` `--verbose` and `-vv` — log product = CLI plus XDG — precedence `-q` greater than `-v` or `-vv` greater than XDG greater than info — `timeout 180 duckduckgo-search-cli -v "QUERY" -f json 2>/tmp/ddg-debug.log`
- `-q` `--quiet` MANDATORY in pipelines — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `--fetch-content` explicit redundant ON — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --fetch-content --max-content-length 5000`
- `--no-fetch-content` SERP-only opt-out — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- `--fetch-content-cap` 1..50 default 4 — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --fetch-content-cap 5`
- `--max-content-length` default 10000 range 1..100000 — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --max-content-length 5000`
- `--proxy` HTTP HTTPS SOCKS5 SOCKS5h — `timeout 180 duckduckgo-search-cli --proxy socks5://127.0.0.1:1080 "QUERY" -q -f json`
- `--no-proxy` mutually exclusive with `--proxy` — `timeout 180 duckduckgo-search-cli --no-proxy "QUERY" -q -f json`
- `--match-platform-ua` — `timeout 180 duckduckgo-search-cli --match-platform-ua "QUERY" -q -f json`
- `--per-host-limit` 1..10 default 2 — with fetch ON MUST stay at or below 2 — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- `--chrome-path` wrappers and Flatpak exports resolve to real ELF — `timeout 180 duckduckgo-search-cli --chrome-path /usr/lib64/chromium-browser/chromium-browser "QUERY" -q -f json`
- `--chrome-visible` headed Chrome — `timeout 180 duckduckgo-search-cli --chrome-visible "QUERY" -q -f json`
- `--chrome-headless` force headless — `timeout 180 duckduckgo-search-cli --chrome-headless "QUERY" -q -f json`
- `--chrome-xvfb` force Xvfb path on Linux — `timeout 180 duckduckgo-search-cli --chrome-xvfb "QUERY" -q -f json`
- `--dump-news-html` debug news HTML capture — `timeout 180 duckduckgo-search-cli --vertical news --dump-news-html /tmp/news.html "QUERY" -q -f json`
- `--no-color` and `--no-warmup` — `timeout 180 duckduckgo-search-cli --no-color --no-warmup "QUERY" -q -f json`
- `--no-cookie-persistence` — `timeout 180 duckduckgo-search-cli --no-cookie-persistence "QUERY" -q -f json`
- `--cookies-path` — `timeout 180 duckduckgo-search-cli --cookies-path /secure/cookies.json "QUERY" -q -f json`
- `--seed` — `timeout 180 duckduckgo-search-cli --seed 42 "QUERY" -q -f json`
- `--config` selector config directory NOT product toml file — `timeout 180 duckduckgo-search-cli --config /path/to/selectors-dir "QUERY" -q -f json`
- `--config-home` override XDG config home — `timeout 180 duckduckgo-search-cli --config-home /tmp/ddg-xdg "QUERY" -q -f json`
- `--allow-lite-fallback` no-op NOT remediation — `timeout 180 duckduckgo-search-cli --allow-lite-fallback "QUERY" -q -f json`
- `--global-timeout` 1..3600 default 180 — `timeout 200 duckduckgo-search-cli "QUERY" -q -f json --global-timeout 180`
- `--ui-lang` UI stderr `en|pt-BR` — NOT SERP `-l` — `timeout 180 duckduckgo-search-cli --ui-lang en "QUERY" -q -f json`
- `--cancel-grace-secs` 1..60 default 5 — `timeout 180 duckduckgo-search-cli --cancel-grace-secs 10 "QUERY" -q -f json`
- `--no-zero-cause-strict` legacy zeros as exit 5 — default strict maps suspected zeros to exit 6 — `timeout 180 duckduckgo-search-cli --no-zero-cause-strict "QUERY" -q -f json`
- `--base-url-html` `--base-url-lite` `--base-url-serp` test overrides — `timeout 180 duckduckgo-search-cli --base-url-html https://example.test "QUERY" -q -f json`
- `--no-input` refuse stdin queries declaratively — MUST use when no interactive input exists — `timeout 180 duckduckgo-search-cli --no-input "QUERY" -q -f json`
- `--pre-flight` auto-route via probe-deep on web — skipped for pure news — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- `--probe` — `timeout 15 duckduckgo-search-cli --probe -q -f json`
- `--probe-deep` — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json`
- `--wire-keys en|pt` — stdout key language remap, default `en` — `timeout 180 duckduckgo-search-cli --wire-keys en "QUERY" -q -f json`
- `--chrome-session-retries` — Chrome session retry budget — `timeout 180 duckduckgo-search-cli --chrome-session-retries 2 "QUERY" -q -f json`

### Agent ops — ALWAYS prefer the binary over jaq
- `--fields` / `--select` project columns — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --fields url,title,snippet`
- `--filter` fail-fast grammar `title~needle` / `host:docs.rs` — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --filter 'title~rust'`
- `--limit` post-SERP row cap (distinct from `-n/--num`) — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --limit 5`
- `--sort` — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --sort title`
- `--dedupe-by` — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --dedupe-by url`
- `--count-only` compact count payload — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --count-only`
- `--truncate-content` — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --truncate-content 500`
- `--max-output-bytes` — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --max-output-bytes 65536`
- `--pretty` + `--fields` → indented JSON; `--count-only` stays compact (intentional)

### Agent ops on introspection surfaces — act or refuse, NEVER ignore
- Every operation either acts or REFUSES with exit 2 naming the flag and the surface
- You MUST NEVER assume an accepted flag was applied — a refusal is loud, not silent
- `--fields` and `--truncate-content` and `--max-output-bytes` work on EVERY surface
- `--fields` here takes DOTTED PATHS into the envelope, not result columns
- Example: `duckduckgo-search-cli -q -f json --fields checks.name doctor`
- A path that matches nothing is exit 2, naming the level that broke and the keys available there
- The discriminator (`type` / `kind`) always survives projection and truncation
- `--filter` `--sort` `--dedupe-by` `--limit` `--count-only` need a ROW ARRAY
- Surfaces WITH rows: `doctor` (`checks`), `schema` (`schemas`), `locale` (`available`), `config list` and `config effective` (`allowed_keys`), `init-config` (`files`)
- Surfaces WITHOUT rows: `commands`, `config path`, `config get`, `config set`, `config unset`
- On a rowless surface those five exit 2; they are never silently dropped
- READ the matrix instead of probing: `duckduckgo-search-cli -q -f json --fields agent_ops commands`
- Root flags go BEFORE the subcommand: `--fields x doctor`, never `doctor --fields x`
- Measured: `commands` 6421 → 47 bytes; `doctor` 2524 → 38; `schema` 4726 → 1107

### Deep-research flags
- Base (defaults fit 180 — **max-sub-queries=3** / **fetch-content-cap=4**) — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY"`
- Full mode — `--fetch-content-cap` is root-only and MUST precede the subcommand — `timeout 620 duckduckgo-search-cli -q -f json --global-timeout 600 --fetch-content-cap 10 deep-research "QUERY" --max-sub-queries 5`
- Budget dry — `duckduckgo-search-cli -q -f json deep-research "QUERY" --print-budget`
- Under-budget override — `--allow-under-budget` (or XDG `deep_research_allow_under_budget`)
- Auto contention budget — `--auto-contention-budget` / `--no-auto-contention-budget` (or XDG `deep_research_auto_contention_budget`)
- Quality gate all sub-queries — `--require-all-sub-queries` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --require-all-sub-queries`
- Quality MANDATORY manual — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--max-sub-queries` 1..12 default 3 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --max-sub-queries 5`
- `--sub-query-strategy` `heuristic|manual` — quality MUST use manual plus file
- `--sub-queries-file` one sub-query per line — required with manual
- `--aggregate` `rrf|dedupe-by-url` default rrf — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --aggregate rrf`
- `--depth` 0..3 default 0 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --depth 2`
- `--synthesize` plus `--budget-tokens` default 4000 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --budget-tokens 2000`
- `--synth-format` `markdown|plain-text|json` — value MUST be `plain-text` NEVER `plain` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --synth-format plain-text`
- `--require-results` non-zero when fan-out aggregates zero — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --require-results`
- `--no-news` intentional web-only deep with Chrome available — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-news`
- Deep agent ops inherit root: `--fields` `--filter` `--limit` `--sort` `--dedupe-by` `--count-only` `--truncate-content` `--max-output-bytes` `--wire-keys`
- Deep fetch ON by default — SERP-only deep — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-fetch-content`
- Deep does NOT inherit the whole root surface — it REJECTS transport flags placed AFTER the subcommand with exit 2 usage
- Rejected after `deep-research` and therefore MANDATORY before it — `--lang` `--country` `--timeout` `--retries` `--pages` `--parallel` `--vertical` `--endpoint` `--time-filter` `--safe-search` `--per-host-limit` `--max-content-length` `--fetch-content-cap` `--identity-profile` `--queries-file` `--stream` `--pretty` `--seed` `--config` `--cookies-path` `--no-warmup` `--no-cookie-persistence` `--disable-retry` `--match-platform-ua` `--shared-session-verticals` `--dump-news-html` `--chrome-visible` `--chrome-headless` `--chrome-xvfb` `--chrome-session-retries` `--probe` `--probe-deep` and the three `--base-url-*`
- Deep ignores `--vertical` for mode selection — use `--no-news` to skip news
- News RRF is SEPARATE from web RRF — NEVER compare scores across `.news[]` and `.results[]`
- Deep exits 0 if web OR news produced results — exit 5 only when BOTH empty
- Deep schema metadata documents `partial` / `sub_queries_*` / `chrome_contention_advisory`

### All subcommands (one example each)
- Root search, no subcommand — hidden alias `buscar` is equivalent — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `init-config` plus `--force` and `--dry-run` — `duckduckgo-search-cli init-config --dry-run`
- `completions bash|zsh|fish|powershell|elvish` — `duckduckgo-search-cli completions bash`
- `deep-research` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY"`
- `commands` tree — `duckduckgo-search-cli commands -q -f json`
- `schema` list — `duckduckgo-search-cli schema -q -f json`
- `schema --name ID` — `duckduckgo-search-cli schema --name search-output -q -f json`
- root `--print-schema` — `duckduckgo-search-cli --print-schema` (same catalog as `schema` without `--name`)
- `doctor` — `timeout 30 duckduckgo-search-cli doctor -q -f json`
- `doctor --strict` — `timeout 30 duckduckgo-search-cli doctor --strict -q -f json`
- `doctor --probe-deep` — `timeout 30 duckduckgo-search-cli doctor --probe-deep -q -f json` (there is **no** `doctor --probe`)
- root `--probe` — `timeout 15 duckduckgo-search-cli --probe -q -f json` (NOT a doctor flag)
- root `--wire-keys en|pt` — `timeout 180 duckduckgo-search-cli --wire-keys pt "QUERY" -q -f json`
- `locale` UI locale diagnostics — `duckduckgo-search-cli locale -q -f json`
- `man` print manpage — `duckduckgo-search-cli man`
- `man --file PATH` write manpage — `duckduckgo-search-cli man --file /tmp/ddg.1`
- `config path` — `duckduckgo-search-cli config path`
- `config list` — `duckduckgo-search-cli config list`
- `config get proxy_url` — `duckduckgo-search-cli config get proxy_url`
- `config get --key chrome_path` — `duckduckgo-search-cli config get --key chrome_path`
- `config set --key proxy_url --value URL` — `duckduckgo-search-cli config set --key proxy_url --value "socks5://127.0.0.1:1080"`
- `config set KEY VALUE` — `duckduckgo-search-cli config set log_directive "duckduckgo_search_cli=debug"`
- `config set wire_keys en` — `duckduckgo-search-cli config set wire_keys en`
- `config set budget_profile desktop_contended` — `duckduckgo-search-cli config set budget_profile desktop_contended`
- `config unset KEY` — `duckduckgo-search-cli config unset proxy_url`
- `config effective` — `duckduckgo-search-cli config effective`

## Modes — REQUIRED behavior
- Content fetch applies to news cards when fetch is ON — `.news[].content` under fetch-content-cap
- Multi-query root is `.searches[]` for batch JSON or one NDJSON line per query when streaming — NEVER confuse with single `.results[]`
- Multi-query MUST inspect `.zero_cause_histogram` when present
- MUST inspect Chrome metadata — `.metadata.chrome_path_resolved` `.metadata.chrome_channel` `.metadata.used_chrome` `.metadata.chrome_attempted`
- ZeroCause seven values — `legitimate` `silent-filter` `ghost-block` `anti-bot` `invalid-response` `suspicious-zero-results` `vertical-no-results`
- `vertical-no-results` yields exit 5 — other non-`legitimate` zeros yield exit 6 under strict default
- Cascade `.metadata.cascade_level` is optional 0..4 — if 4 MUST rotate proxy or wait 300s

## JSON Contract and Parsing
Default wire keys are **English**. Legacy PT emit only with `--wire-keys pt` or XDG `wire_keys=pt`. ALWAYS parse with `jaq`. NEVER `jq`.

- Capture exit first — `out=$(timeout 180 duckduckgo-search-cli "QUERY" -q -f json); ec=$?; echo "$out" | jaq .; exit $ec`
- TSV web — `jaq -r '.results[] | [.position, .title, .url, (.snippet // "")] | @tsv'`
- News extract — `jaq -r '.news[] | [.position, .title, .url, (.source // ""), (.relative_date // "")] | @tsv'`
- Dual extract — `jaq '{web:.results,news:.news,path:.metadata.chrome_path_resolved,channel:.metadata.chrome_channel}'`
- Zero diagnosis — `jaq '{cause:.metadata.zero_cause,action:.metadata.next_action_suggestion,n:.metadata.result_count}'`
- Probe status — `jaq '.status'`
- Multi extract — `jaq -r '.searches[] | .query as $q | .results[0] | "\($q)\t\(.title)\t\(.url)"'`
- GUARANTEED — `.query` `.results[].position` `.results[].title` `.results[].url` `.metadata.execution_time_ms` `.metadata.result_count` `.metadata.used_fallback_endpoint`
- OPTIONAL — `.results[].snippet` `.results[].display_url` `.results[].original_title` `.metadata.identity_used` `.metadata.cascade_level`
- CONDITIONAL news — `.news[]` `.news_count` `.metadata.vertical_used`
- CONDITIONAL fetch — `.results[].content` `.news[].content`
- Chrome metadata — `.metadata.used_chrome` `.metadata.chrome_attempted` `.metadata.chrome_path_resolved` `.metadata.chrome_channel`
- Diagnosis — `.metadata.zero_cause` `.metadata.next_action_suggestion`
- Pre-flight — `.metadata.pre_flight_fired` `.metadata.endpoint_used`
- Compat — some fields exist at root AND under `.metadata`
- Roots — single `.results[]` — multi `.searches[]` / `.query_count` — deep `.results[]` plus `.news[]` plus `.synthesis` when synthesized
- ALWAYS use `// ""` on optionals — NEVER invent missing fields

## Exit Codes
- 0 success with results — parse with jaq and cite sources
- 1 runtime network I/O or parse — report stderr — retry with native `--retries`
- 2 invalid config or args OR missing Chrome or build without feature `chrome` — fix args or install Chrome
- 3 anti-bot soft-block — wait 300s — set `--chrome-path` — set `--proxy` — NEVER Lite
- 4 global timeout — raise `--global-timeout` — reduce num parallel fetch load
- 5 legitimate zero or `vertical-no-results` — reformulate query or adjust filters
- 6 suspected block non-legitimate ZeroCause — read `zero_cause` and `next_action_suggestion` — remediate Chrome or proxy
- 130 cancelled SIGINT — NOT a search failure
- 141 broken pipe consumer closed — normal for `| head` — NOT a search failure
- 143 cancelled SIGTERM — clean stop — oneshot reap ran

## Environment and product config
- Cookie jar Unix `~/.config/duckduckgo-search-cli/cookies.json` mode 0600 — Windows `%APPDATA%\duckduckgo-search-cli\cookies.json`
- NEVER log cookies — NEVER commit `cookies.json` — NEVER log `--proxy` credentials
- Test-only harness envs belong in TESTING docs — NEVER teach as production agent knobs
- There is NO remote telemetry — metadata fields are local diagnostics only

## Absolute Prohibitions
- FORBIDDEN invent search results titles URLs or snippets without running the CLI
- FORBIDDEN bare SIGKILL as the normal cancel path — ALWAYS SIGTERM first via `/usr/bin/timeout`
- FORBIDDEN expect auto cleanup after bare SIGKILL OOM or foreign orphans
- FORBIDDEN bulk find or rm of foreign temps — NEVER mass-delete `.tmp*` or `org.chromium.Chromium.*`
- FORBIDDEN residual audit for any prefix other than owned `ddg-chrome-*`
- FORBIDDEN `-f text` `-f markdown` or `-f tsv` for agent parsing — ALWAYS `-f json` or intentional multi-query `-f ndjson`
- FORBIDDEN treat `--stream` as a full SERP hit event stream
- FORBIDDEN use Lite or `--allow-lite-fallback` as remediation for exit 3 or 6
- FORBIDDEN silent pure-HTTP downgrade when Chrome is missing or fails
- FORBIDDEN auto `--no-news` when Chrome is absent
- FORBIDDEN hardcode API keys proxies or user-agents in commits
- FORBIDDEN hardcode `--identity-profile` in CI
- FORBIDDEN `--output` with `..` or system directories
- FORBIDDEN treat `identity_used` or `cascade_level` as guaranteed fields
- FORBIDDEN ignore zero results without reading `zero_cause` and `next_action_suggestion`
- FORBIDDEN ignore exit 6
- FORBIDDEN treat exit 141 as a search failure
- FORBIDDEN shell retry loops — use native `--retries`
- FORBIDDEN combine `--proxy` with `--no-proxy`
- FORBIDDEN rely on `HTTP_PROXY` `HTTPS_PROXY` or `ALL_PROXY` for product proxy
- FORBIDDEN teach `CHROME_PATH` or `RUST_LOG` as product knobs
- FORBIDDEN invent config keys outside ALLOWED_KEYS
- FORBIDDEN `--synth-format plain` — correct value is `plain-text`
- FORBIDDEN compare news RRF scores against web RRF scores
- FORBIDDEN reintroduce a Chrome launch without the mute-audio operational standard
- FORBIDDEN place a root-only flag AFTER `deep-research` — that is exit 2 usage, never a silent no-op
- FORBIDDEN read `.synth` or `.title_original` — the wire emits `.synthesis` and `.original_title`
