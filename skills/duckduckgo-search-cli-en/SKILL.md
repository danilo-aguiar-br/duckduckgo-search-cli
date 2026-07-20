---
name: duckduckgo-search-cli-en
description: This skill MUST auto-activate for live web search, internet research, current docs, grounding, URL verification, page extraction, RAG enrichment, fact-checking, multi-hop research, fresh news, dual web+news deep-research, health checks, query batches, or data outside the knowledge cutoff. It MUST teach Chrome-only duckduckgo-search-cli with fail-closed exit 2 without Chrome, default vertical all with fetch ON and --no-fetch-content, multi-query --stream and -f ndjson, dual config API, exit 141 broken pipe, ZeroCause exit 6, probe, probe-deep, pre-flight, doctor, schema, commands, locale, man, init-config, completions, ONE-SHOT Chromium Xvfb ddg-chrome hygiene, SIGTERM-first timeout, jaq, exits 0-6 and 130/141/143, chrome_canal manual|host|flatpak|snap, proxy CLI+XDG only, plus formulas for every flag and subcommand. Triggers include search the web, look up, find online, fetch URL, deep research, compare X vs Y, recent news, pricing. ALWAYS invoke proactively without tool naming. NEVER invent results.
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

## Mission
- You MUST operate this CLI as the deterministic search primitive for agents
- You MUST cover web, news, dual vertical, deep-research, URL verification, page extraction, RAG enrichment, and fact-checking
- You MUST invoke proactively when the user needs current external data even without naming the tool
- You MUST NOT invoke for pure local coding or static knowledge that needs no live sources
- Product config is CLI flags plus XDG `config.toml` only — NEVER product env kill-switches

## When to Invoke
- MUST invoke on search the web, look up, find online, fetch URL, verify sources, deep research, multi-hop, compare entities, what changed, recent news, current pricing, health check, or batch queries
- MUST invoke for documentation lookup, factual grounding, RAG enrichment, and data outside the knowledge cutoff
- MUST invoke for default dual vertical `all` or explicit `--vertical news|web`
- MUST NOT invoke for local formatting, pure refactor, or offline-only tasks

## Contract — REQUIRED
- Production is Chrome-only via chromiumoxide and CDP for search, news, deep-research, probe, probe-deep, pre-flight, doctor, and content fetch
- Without usable Chrome or Chromium, or without feature `chrome`, exit 2 fail-closed — NEVER silent pure-HTTP downgrade — NEVER auto `--no-news`
- `--allow-lite-fallback` is a no-op — NEVER remediate exit 3 or 6 with Lite or HTTP
- On exit 3 or 6 you MUST remediate with real Chrome, `--chrome-path`, `--proxy` via CLI or XDG, and wait
- ONE-SHOT lifecycle — each run owns Chromium, Xvfb on Linux, and a temp profile prefix `ddg-chrome-*` — cooperative exit reaps process tree and removes that profile — next run sweeps only stale owned `ddg-chrome-*`
- Residual disk audit MUST use only `find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'ddg-chrome-*' 2>/dev/null`
- FORBIDDEN bulk find or rm of `.tmp*` or `org.chromium.Chromium.*` as hygiene for this CLI
- Unix broken pipe yields exit 141 and oneshot cleanup still runs — NEVER treat 141 as a search failure
- Chrome path order MUST be CLI `--chrome-path` then XDG `chrome_path` then auto host Flatpak Snap — NEVER teach `CHROME_PATH` as a product knob
- `.metadados.chrome_canal` values MUST be exactly `manual|host|flatpak|snap` — NEVER `env`
- DEFAULT vertical is `all` — DEFAULT content fetch is ON for web and news under `--fetch-content-cap` default 10 — opt out with `--no-fetch-content`
- `--stream` is ALLOWED for multi-query NDJSON lines — `-f ndjson` is the stream alias — single-query `--stream` is ignored with a warning — NEVER treat stream as a full SERP hit event stream
- Dual config API MUST accept positional and flag forms — `config get KEY` or `config get --key KEY` — `config set KEY VALUE` or `config set --key KEY --value VALUE` — `config unset KEY` or `config unset --key KEY` — `config effective` shows CLI greater than XDG greater than defaults
- ALLOWED_KEYS only — `ui_lang`, `chrome_path`, `proxy_url`, `default_global_timeout`, `default_vertical`, `fetch_content_default`, `log_directive`, `default_lang`, `default_country` — NEVER invent config keys
- Wire JSON serializes Portuguese field names — English aliases exist only on deserialize
- Product log filter precedence MUST be `-q` greater than `-v` or `-vv` greater than XDG `log_directive` greater than `info` — NEVER teach `RUST_LOG` as product config
- Proxy MUST be CLI `--proxy` or `--no-proxy` or XDG `proxy_url` only — NEVER `HTTP_PROXY` `HTTPS_PROXY` `ALL_PROXY`
- Atomic `--output` only — FORBIDDEN paths with `..` or system directories
- MANDATORY outer timeout seconds — dual plus fetch 180 — SERP-only 90 — thin web-only 60 — deep-research 180 — batch 300 — probe 15 — probe-deep 20 — doctor 30
- Defaults — num 15 — format auto (agents MUST force json unless intentional multi-query stream) — timeout 15s — lang pt — country br — parallel 5 clamp 1..20 — pages 1 — retries 2 — endpoint html — vertical all — safe-search moderate — identity-profile auto — max-content-length 10000 — fetch-content-cap 10 — per-host-limit 2 — global-timeout 180 — cancel-grace-secs 5 — max-sub-queries 5 — aggregate rrf — depth 0 — budget-tokens 4000
- Correct Pattern base — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`

## Workflow
1. MUST choose mode — search, news, deep-research, batch, stream, probe, pre-flight, doctor, or SERP-only
2. MUST build the formula with GNU `timeout`, `-q`, `-f json`, and mode flags
3. MUST execute and capture exit code plus stdout — with pipes MUST use `${PIPESTATUS[0]}`
4. IF exit 0 with results — MUST extract with `jaq` and cite sources
5. IF zero results — MUST read `.metadados.causa_zero` and `.metadados.sugestao_proxima_acao` before retry
6. IF exit 2 — MUST install Chrome or fix args — fail-closed
7. IF exit 3 or 6 — MUST wait, rotate proxy via CLI or XDG, revalidate Chrome path and canal — NEVER Lite
8. IF exit 4 — MUST raise `--global-timeout` or reduce load
9. IF exit 5 with `legitimo` or `vertical-sem-resultados` — MUST reformulate query or adjust lang time-filter vertical
10. IF exit 130 141 or 143 — MUST NOT treat as a search failure
11. After each run MUST assume Chromium Xvfb and `ddg-chrome-*` profile were reaped

## Execution Formulas — every flag and mode
MUST copy and adapt. ALWAYS keep `-q -f json` in agent pipelines unless multi-query stream is intentional.

### Base modes
- Dual plus fetch MANDATORY — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- SERP-only faster — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- Thin web-only — `timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content "QUERY" -q -f json`
- News-only — `timeout 180 duckduckgo-search-cli --vertical news "QUERY" -q -f json`
- Positional multi-query — `timeout 120 duckduckgo-search-cli -q -f json "query one" "query two"`
- Stdin multi-query — `printf '%s\n' "q1" "q2" | timeout 120 duckduckgo-search-cli -q -f json`
- Multi-query stream — `timeout 120 duckduckgo-search-cli -q --stream "q1" "q2"`
- Multi-query stream alias — `timeout 120 duckduckgo-search-cli -q -f ndjson "q1" "q2"`

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
- `--vertical all` DEFAULT dual — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `--vertical web` — `timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content "QUERY" -q -f json`
- `--vertical news` — `timeout 180 duckduckgo-search-cli --vertical news "QUERY" -q -f json`
- `--shared-session-verticals` share one Chrome session for dual — `timeout 180 duckduckgo-search-cli --shared-session-verticals "QUERY" -q -f json`
- `--time-filter` `d|w|m|y` — `timeout 180 duckduckgo-search-cli --time-filter d "QUERY" -q -f json`
- `--safe-search` `off|moderate|on` default moderate — `timeout 180 duckduckgo-search-cli --safe-search off "QUERY" -q -f json`
- `--identity-profile` `auto|chrome-win|chrome-mac|chrome-linux|edge-win|firefox-linux|safari-mac` default auto — NEVER hardcode in CI — `timeout 180 duckduckgo-search-cli --identity-profile chrome-linux "QUERY" -q -f json`
- `--stream` multi-query NDJSON only — `timeout 120 duckduckgo-search-cli -q --stream "q1" "q2"`
- `-v` `--verbose` and `-vv` — log product = CLI plus XDG — precedence `-q` greater than `-v` or `-vv` greater than XDG greater than info — `timeout 180 duckduckgo-search-cli -v "QUERY" -f json 2>/tmp/ddg-debug.log`
- `-q` `--quiet` MANDATORY in pipelines — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `--fetch-content` explicit redundant ON — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --fetch-content --max-content-length 5000`
- `--no-fetch-content` SERP-only opt-out — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- `--fetch-content-cap` 1..50 default 10 — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --fetch-content-cap 5`
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
- `--no-color` — `timeout 180 duckduckgo-search-cli --no-color "QUERY" -q -f json`
- `--no-warmup` — `timeout 180 duckduckgo-search-cli --no-warmup "QUERY" -q -f json`
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
- `--base-url-html` test override — `timeout 180 duckduckgo-search-cli --base-url-html https://example.test "QUERY" -q -f json`
- `--base-url-lite` test override — `timeout 180 duckduckgo-search-cli --base-url-lite https://example.test "QUERY" -q -f json`
- `--base-url-serp` test override — `timeout 180 duckduckgo-search-cli --base-url-serp https://example.test "QUERY" -q -f json`
- `--pre-flight` auto-route via probe-deep on web — skipped for pure news — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- `--probe` — `timeout 15 duckduckgo-search-cli --probe -q -f json`
- `--probe-deep` — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json`

### Deep-research flags
- Base — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY"`
- Quality MANDATORY manual — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--max-sub-queries` 1..12 default 5 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --max-sub-queries 5`
- `--sub-query-strategy` `heuristic|manual` — quality MUST use manual plus file
- `--sub-queries-file` one sub-query per line — required with manual
- `--aggregate` `rrf|dedupe-by-url` default rrf — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --aggregate rrf`
- `--depth` 0..3 default 0 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --depth 2`
- `--synthesize` plus `--budget-tokens` default 4000 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --budget-tokens 2000`
- `--synth-format` `markdown|plain-text|json` — value MUST be `plain-text` NEVER `plain` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --synth-format plain-text`
- `--require-results` non-zero when fan-out aggregates zero — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --require-results`
- `--no-news` intentional web-only deep with Chrome available — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-news`
- Deep fetch ON by default — SERP-only deep — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-fetch-content`
- Deep ignores `--vertical` for mode selection — use `--no-news` to skip news — global transport flags still accepted before or after `deep-research`
- News RRF is SEPARATE from web RRF — NEVER compare scores across `.noticias[]` and `.resultados[]`
- Deep exits 0 if web OR news produced results — exit 5 only when BOTH empty

### Diagnosis and discovery subcommands
- `doctor` — `timeout 30 duckduckgo-search-cli doctor -q -f json`
- `doctor --strict` — `timeout 30 duckduckgo-search-cli doctor --strict -q -f json`
- `schema` list — `duckduckgo-search-cli schema -q -f json`
- `schema --name ID` — `duckduckgo-search-cli schema --name search-output -q -f json`
- `commands` tree — `duckduckgo-search-cli commands -q -f json`
- `locale` UI locale diagnostics — `duckduckgo-search-cli locale -q -f json`
- `man` print manpage — `duckduckgo-search-cli man`
- `man --file PATH` write manpage — `duckduckgo-search-cli man --file /tmp/ddg.1`
- `init-config` — `duckduckgo-search-cli init-config`
- `init-config --force` — `duckduckgo-search-cli init-config --force`
- `init-config --dry-run` — `duckduckgo-search-cli init-config --dry-run`
- `config path` — `duckduckgo-search-cli config path`
- `config list` — `duckduckgo-search-cli config list`
- `config get proxy_url` — `duckduckgo-search-cli config get proxy_url`
- `config get --key chrome_path` — `duckduckgo-search-cli config get --key chrome_path`
- `config set --key proxy_url --value URL` — `duckduckgo-search-cli config set --key proxy_url --value "socks5://127.0.0.1:1080"`
- `config set KEY VALUE` — `duckduckgo-search-cli config set log_directive "duckduckgo_search_cli=debug"`
- `config unset KEY` — `duckduckgo-search-cli config unset proxy_url`
- `config effective` — `duckduckgo-search-cli config effective`
- `completions bash|zsh|fish|powershell|elvish` — `duckduckgo-search-cli completions bash`

## Modes — REQUIRED behavior
- DEFAULT search is dual `all` without extra flags
- MUST use `--vertical news` for news-only and `--vertical web` to skip news
- news and all require Chrome — without Chrome exit 2
- Content fetch applies to news cards when fetch is ON — `.noticias[].conteudo` under fetch-content-cap
- Multi-query root is `.buscas[]` for batch JSON or one NDJSON line per query when streaming — NEVER confuse with single `.resultados[]`
- Multi-query MUST inspect `.causa_zero_histogram` when present
- MUST inspect Chrome metadata — `.metadados.chrome_path_resolvido` `.metadados.chrome_canal` `.metadados.usou_chrome` `.metadados.tentou_chrome`
- ZeroCause seven values — `legitimo` `filtro-silencioso` `ghost-block` `anti-bot` `resposta-invalida` `zero-resultados-suspeito` `vertical-sem-resultados`
- `vertical-sem-resultados` yields exit 5 — other non-`legitimo` zeros yield exit 6 under strict default
- MUST follow `.metadados.sugestao_proxima_acao` when present — it points to Chrome proxy or wait — NEVER Lite
- Cascade `.metadados.nivel_cascata` is optional 0..4 — if 4 MUST rotate proxy or wait 300s

## JSON Contract and Parsing
Portuguese field names stay as the CLI emits them. ALWAYS parse with `jaq`. NEVER `jq`.

- Capture exit first — `out=$(timeout 180 duckduckgo-search-cli "QUERY" -q -f json); ec=$?; echo "$out" | jaq .; exit $ec`
- TSV web — `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'`
- News extract — `jaq -r '.noticias[] | [.posicao, .titulo, .url, (.fonte // ""), (.data_relativa // "")] | @tsv'`
- Dual extract — `jaq '{web:.resultados,news:.noticias,path:.metadados.chrome_path_resolvido,canal:.metadados.chrome_canal}'`
- Zero diagnosis — `jaq '{causa:.metadados.causa_zero,acao:.metadados.sugestao_proxima_acao,n:.metadados.quantidade_resultados}'`
- Probe status — `jaq '.status'`
- Multi extract — `jaq -r '.buscas[] | .query as $q | .resultados[0] | "\($q)\t\(.titulo)\t\(.url)"'`
- GUARANTEED — `.query` `.resultados[].posicao` `.resultados[].titulo` `.resultados[].url` `.metadados.tempo_execucao_ms` `.metadados.quantidade_resultados` `.metadados.usou_endpoint_fallback`
- OPTIONAL — `.resultados[].snippet` `.resultados[].url_exibicao` `.resultados[].titulo_original` `.metadados.identidade_usada` `.metadados.nivel_cascata`
- CONDITIONAL news — `.noticias[]` `.quantidade_noticias` `.metadados.vertical_usada`
- CONDITIONAL fetch — `.resultados[].conteudo` `.noticias[].conteudo`
- Chrome metadata — `.metadados.usou_chrome` `.metadados.tentou_chrome` `.metadados.chrome_path_resolvido` `.metadados.chrome_canal`
- Diagnosis — `.metadados.causa_zero` `.metadados.sugestao_proxima_acao`
- Pre-flight — `.metadados.pre_flight_disparado` `.metadados.endpoint_usado`
- Compat — some fields exist at root AND under `.metadados`
- Roots — single `.resultados[]` — multi `.buscas[]` — deep `.resultados[]` plus `.noticias[]` plus `.sintese` when synthesized
- ALWAYS use `// ""` on optionals — NEVER invent missing fields

## Exit Codes
- 0 success with results — parse with jaq and cite sources
- 1 runtime network I/O or parse — report stderr — retry with native `--retries`
- 2 invalid config or args OR missing Chrome or build without feature `chrome` — fix args or install Chrome
- 3 anti-bot soft-block — wait 300s — set `--chrome-path` — set `--proxy` — NEVER Lite
- 4 global timeout — raise `--global-timeout` — reduce num parallel fetch load
- 5 legitimate zero or `vertical-sem-resultados` — reformulate query or adjust filters
- 6 suspected block non-legitimo ZeroCause — read causa_zero and sugestao_proxima_acao — remediate Chrome or proxy
- 130 cancelled SIGINT — NOT a search failure
- 141 broken pipe consumer closed — normal for `| head` — NOT a search failure
- 143 cancelled SIGTERM — clean stop — oneshot reap ran

## Environment and product config
- Product configuration is CLI flags plus XDG only — `--proxy` `--chrome-path` `--config-home` `-v` `-q` `log_directive` and `config set`
- Cookie jar Unix `~/.config/duckduckgo-search-cli/cookies.json` mode 0600 — Windows `%APPDATA%\duckduckgo-search-cli\cookies.json`
- NEVER log cookies — NEVER commit `cookies.json` — NEVER log `--proxy` credentials
- MUST use `--no-cookie-persistence` for ephemeral sessions
- Test-only harness envs belong in TESTING docs — NEVER teach as production agent knobs
- There is NO remote telemetry — metadata fields are local diagnostics only

## Absolute Prohibitions
- FORBIDDEN invent search results titles URLs or snippets without running the CLI
- FORBIDDEN omit the outer GNU `timeout` wrapper on agent executions
- FORBIDDEN bare SIGKILL as the normal cancel path — ALWAYS SIGTERM first via `/usr/bin/timeout`
- FORBIDDEN expect auto cleanup after bare SIGKILL OOM or foreign orphans
- FORBIDDEN bulk find or rm of foreign temps — NEVER mass-delete `.tmp*` or `org.chromium.Chromium.*`
- FORBIDDEN residual audit for any prefix other than owned `ddg-chrome-*`
- FORBIDDEN `-f text` `-f markdown` or `-f tsv` for agent parsing — ALWAYS `-f json` or intentional multi-query `-f ndjson`
- FORBIDDEN omit `-q` in pipelines
- FORBIDDEN treat `--stream` as a full SERP hit event stream
- FORBIDDEN parse with `jq` — ALWAYS `jaq`
- FORBIDDEN omit `${PIPESTATUS[0]}` when piping CLI stdout
- FORBIDDEN use Lite or `--allow-lite-fallback` as remediation for exit 3 or 6
- FORBIDDEN silent pure-HTTP downgrade when Chrome is missing or fails
- FORBIDDEN auto `--no-news` when Chrome is absent
- FORBIDDEN hardcode API keys proxies or user-agents in commits
- FORBIDDEN hardcode `--identity-profile` in CI
- FORBIDDEN `--output` with `..` or system directories
- FORBIDDEN treat `identidade_usada` or `nivel_cascata` as guaranteed fields
- FORBIDDEN ignore zero results without reading `causa_zero` and `sugestao_proxima_acao`
- FORBIDDEN ignore exit 6
- FORBIDDEN treat exit 141 as a search failure
- FORBIDDEN shell retry loops — use native `--retries`
- FORBIDDEN combine `--proxy` with `--no-proxy`
- FORBIDDEN rely on `HTTP_PROXY` `HTTPS_PROXY` or `ALL_PROXY` for product proxy
- FORBIDDEN teach `CHROME_PATH` or `RUST_LOG` as product knobs
- FORBIDDEN invent config keys outside ALLOWED_KEYS
- FORBIDDEN `--synth-format plain` — correct value is `plain-text`
- FORBIDDEN compare news RRF scores against web RRF scores
