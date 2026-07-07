---
name: duckduckgo-search-cli-en
description: This skill MUST be invoked when the user asks for web search, internet research, up-to-date documentation, factual grounding, URL verification, page extraction, RAG enrichment, fact-checking, multi-hop research, news vertical, deep-research dual web+news, or any data outside knowledge cutoff. Triggers — search the web, look up, find online, fetch URL, deep research, compare X vs Y, what changed, recent news, current pricing. Chrome HEADED inside private Xvfb with stealth anti-detection signals. Auto-installs Xvfb on 22+ Linux distros. Chrome-only UA for TLS/JA4 parity. ZeroCause 7-variant classifier with exit code 6. 12-identity anti-bot pool. deep-research RRF fan-out dual web+news by default. News vertical via --vertical news|all. reqwest+rustls-tls for fetch-content and probe. English
---

# Skill — duckduckgo-search-cli (EN)


## When to invoke this CLI
- MUST invoke when the answer requires data outside the knowledge cutoff
- MUST invoke on triggers — search, look up, find online, verify URL, fetch page, what changed, compare, deep research, ground this, current pricing, multi-hop question, recent news
- MUST prefer this CLI over WebSearch/WebFetch for deterministic pipelines
- MUST invoke proactively when user intent implies external data even without explicit trigger words
- MUST invoke for the fresh-news vertical (articles with source, relative date and thumbnail) via `--vertical news|all`


## Chrome transport and anti-detection
- Chrome runs in HEADED mode inside a PRIVATE Xvfb virtual display as the PRIMARY search transport — the user sees ZERO windows
- On Linux Desktop with native display, the CLI ALWAYS spawns a private Xvfb to prevent visible windows to the user
- Xvfb is auto-installed on 22+ Linux distros via non-interactive sudo — on immutable distros manual instructions are displayed instead
- Stale Xvfb lock files are auto-cleaned before spawning a new display
- If Xvfb is unavailable after auto-install attempt, falls back to headless with instruction message
- Chrome navigates FIRST to duckduckgo.com as warm-up before the real search URL — resolves Cloudflare JS challenge and sets cookies
- Multiple stealth anti-detection signals are injected before any navigation — WebGL spoofing, canvas noise, audio fingerprint noise, Permissions API, CDP leak prevention
- The identity pool filters to accept ONLY Chrome-family UAs when the real browser is Chromium — prevents UA/TLS mismatch detectable via JA3/JA4
- reqwest+rustls-tls is used ONLY for `--fetch-content` and `--probe` — NOT for primary searches
- Field `.metadados.usou_chrome` indicates true when Chrome-primary search succeeded
- Field `.metadados.tentou_chrome` indicates true when Chrome search was attempted


## Mandatory pipeline patterns
- ALWAYS wrap with `timeout` in seconds — pipeline hangs without time limit
- ALWAYS use `-q` in pipelines — stderr tracing pollutes stdout without this flag
- ALWAYS use `-f json` for programmatic parsing — NEVER `-f text` or `-f markdown`
- ALWAYS use `jaq` (NEVER `jq`) to process JSON output
- ALWAYS apply `// ""` fallback on optional fields when using `jaq`
- ALWAYS capture exit code BEFORE parsing stdout — MUST use `${PIPESTATUS[0]}` when piped through `jaq`
- The 9 global flags `-n` `-f` `-o` `-t` `-l` `-c` `-p` `-q` `-v` are accepted BEFORE OR AFTER the `deep-research` subcommand — use whichever order you prefer
- MUST use `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'` for tabulated TSV
- MUST use `jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'` for markdown top 5 list
- MUST use `jaq -r '.resultados[] | "- \(.titulo) — \(.url)"'` for source citation block


## Complete flag reference with formulas
- `--num N` — number of results (minimum 1, `--num 0` rejected by clap) — MUST use `timeout 60 duckduckgo-search-cli "query" -q -f json --num 30`
- `--lang <CODE>` — search language (default `pt`) — MUST use `timeout 60 duckduckgo-search-cli --lang pt-BR "query" -q -f json`
- `--country <CODE>` / `-c` — country code (e.g., br, us, de); `--region` is an ALIAS for `--country` for backwards compatibility — MUST use `timeout 60 duckduckgo-search-cli --country br "query" -q -f json`
- `--time-filter <PERIOD>` — time filter (d=day, w=week, m=month, y=year) — MUST use `timeout 60 duckduckgo-search-cli --time-filter d "query" -q -f json` for last 24h
- `--safe-search <LEVEL>` — safe search level (off, moderate, on; default moderate) — MUST use `timeout 60 duckduckgo-search-cli --safe-search off "query" -q -f json`
- `--endpoint <NAME>` — search endpoint (html or lite; default html) — MUST use `timeout 60 duckduckgo-search-cli --endpoint lite "query" -q -f json`
- `--vertical <web|news|all>` — search vertical (default web); news/all require Chrome; in a Chrome-less build OR with `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` the CLI downgrades to Web with a stderr warning and proceeds (does NOT abort); multi-query batches accepted — MUST use `timeout 90 duckduckgo-search-cli --vertical news "query" -q -f json | jaq '.noticias'`
- `--no-news` — deep-research flag that skips the news scan; by default every sub-query runs `--vertical all` via Chrome; without a usable Chrome the subcommand AUTO-APPLIES `--no-news` with a stderr warning and proceeds web-only — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --no-news` in Chrome-less environments
- `--pages <N>` — pages per query (1 to 5, default 1; auto-elevated when --num > 10) — MUST use `timeout 60 duckduckgo-search-cli "query" -q -f json --num 20 --pages 3`
- `-f json` / `--format json` — output format — MANDATORY for programmatic parsing, NEVER use `-f text` or `-f markdown`
- `-q` / `--quiet` — suppress stderr tracing — MANDATORY in pipelines to prevent stdout pollution
- `-v` info / `-vv` debug / `-vvv` trace — additive verbosity levels — MUST use `timeout 60 duckduckgo-search-cli -vv "query" -f json 2>/tmp/debug.log`
- `--no-color` — disable colored output — MUST use `timeout 60 duckduckgo-search-cli --no-color "query" -q -f json`
- `--output <PATH>` — atomic write of full payload (rejected if `..` or system directories) — MUST use `timeout 60 duckduckgo-search-cli "query" -q -f json --output /tmp/results.json`
- `--retries N` — retries with exponential backoff (clamped [1, 10], default 2, NEVER > 10) — MUST use `timeout 60 duckduckgo-search-cli --retries 3 "query" -q -f json`
- `--timeout N` — per-request HTTP timeout in seconds (default 15) — MUST use `timeout 60 duckduckgo-search-cli --timeout 20 "query" -q -f json`
- `--global-timeout N` — global operation timeout in seconds (clamped [1, 3600]) — MUST use `timeout 90 duckduckgo-search-cli --global-timeout 60 "query" -q -f json`
- `--config <PATH>` — load TOML configuration file — MUST use `timeout 60 duckduckgo-search-cli --config ./search.toml "query" -q -f json`
- `--probe` — minimal health check via reqwest — MUST use `timeout 10 duckduckgo-search-cli --probe -q -f json | jaq '.status'`
- `--probe-deep` — real query CAPTCHA detector — MUST use `timeout 15 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'`
- `--pre-flight` — auto-route via probe-deep first — MUST use `timeout 60 duckduckgo-search-cli --pre-flight "query" -q -f json`
- `--allow-lite-fallback` — opt-in HTML to Lite downgrade on CAPTCHA — MUST use `timeout 60 duckduckgo-search-cli --allow-lite-fallback "query" -q -f json`
- `--identity-profile <name>` — pin identity (auto/chrome-win/chrome-mac/chrome-linux/edge-win/firefox-linux/safari-mac) — MUST use `timeout 60 duckduckgo-search-cli --identity-profile chrome-linux "query" -q -f json`
- `--seed <u64>` — deterministic seed for UA + pool rotation — MUST use `timeout 60 duckduckgo-search-cli --seed 42 "query" -q -f json` for reproducibility
- `--no-warmup` — skip cookie warm-up to duckduckgo.com — MUST use `timeout 60 duckduckgo-search-cli --no-warmup "query" -q -f json`
- `--no-cookie-persistence` — in-memory cookies only — MUST use `timeout 60 duckduckgo-search-cli --no-cookie-persistence "query" -q -f json`
- `--cookies-path <PATH>` — redirect jar to encrypted volume — MUST use `timeout 60 duckduckgo-search-cli --cookies-path /secure/cookies.json "query" -q -f json`
- `--chrome-path <PATH>` — manual Chrome/Chromium binary path — MUST use `timeout 60 duckduckgo-search-cli --chrome-path /usr/bin/chromium "query" -q -f json`
- `--match-platform-ua` — force platform-matching User-Agent — MUST use `timeout 60 duckduckgo-search-cli --match-platform-ua "query" -q -f json`
- `--proxy <URL>` — HTTP/HTTPS/SOCKS5 proxy (e.g., socks5://host:port, http://user:pass@host:port); mutually exclusive with `--no-proxy` — MUST use `timeout 60 duckduckgo-search-cli --proxy socks5://127.0.0.1:1080 "query" -q -f json`
- `--no-proxy` — disable proxy (ignores --proxy and HTTP_PROXY/HTTPS_PROXY/ALL_PROXY env vars) — MUST use `timeout 60 duckduckgo-search-cli --no-proxy "query" -q -f json`
- `--queries-file <PATH>` — file with queries for batch (one per line) — MUST use `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --num 15`
- `--parallel N` — parallel queries (clamped [1, 20], default 1; keep ≤ 5 to avoid anti-bot triggers) — MUST use `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--per-host-limit N` — per-host limit (clamped [1, 10]; keep ≤ 2 to avoid HTTP 202 anti-bot) — MUST use `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- `--fetch-content` — extract HTML content from result pages — MUST use `timeout 120 duckduckgo-search-cli "query" -q -f json --fetch-content --max-content-length 5000`
- `--max-content-length N` — byte limit per extracted page (default 10000) — MANDATORY with `--fetch-content`, ALWAYS combine
- `--sub-query-strategy <STRATEGY>` — `heuristic` (default, low quality) or `manual` — ALWAYS use `manual` with `--sub-queries-file`
- `--sub-queries-file <PATH>` — file with manual sub-queries (one per line) — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--aggregate <METHOD>` — deep-research aggregation method — MUST use `--aggregate rrf` for Reciprocal Rank Fusion
- `--max-sub-queries N` — sub-query limit in deep-research (default 5) — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --max-sub-queries 5`
- `--synthesize` — generate synthesis in deep-research — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --synthesize --budget-tokens 2000`
- `--budget-tokens N` — token limit for synthesis (default 4000) — ALWAYS combine with `--synthesize`
- `--synth-format <FORMAT>` — deep-research synthesis format (markdown, plain-text, json) — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --synth-format plain-text` — the correct value is `plain-text`, NOT `plain`
- `--require-results` — deep-research flag; exit 4 when fan-out aggregates zero results — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --require-results`
- `--depth <N>` — deep-research reflection depth (0 to 3, default 0) — MUST use `timeout 180 duckduckgo-search-cli -q -f json deep-research "query" --depth 2`
- `init-config` — subcommand that generates a default TOML configuration file — MUST use `duckduckgo-search-cli init-config`
- `init-config --force` — overwrite existing configuration files — MUST use `duckduckgo-search-cli init-config --force`
- `init-config --dry-run` — simulate generation without writing to disk — MUST use `duckduckgo-search-cli init-config --dry-run`


## Diagnosis — ZeroCause, exit codes and anti-bot cascade
- MUST inspect `.metadados.causa_zero` on EVERY response with `quantidade_resultados == 0`
- 7 classified causes — `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`, `vertical-sem-resultados`
- `vertical-sem-resultados` is a LEGITIMATE news-vertical zero (rendered SERP without articles) — emits exit 5, NOT 6
- When `causa_zero != "legitimo"`, the CLI emits exit code 6 (`SUSPECTED_BLOCK`) by default
- Field `.metadados.sugestao_proxima_acao` contains actionable instruction when cause is non-legitimate
- To restore legacy behavior (exit 5 for all zeros) — set `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`
- In multi-query (batch), the `.causa_zero_histogram` field aggregates counts across sub-queries
- Exit codes — `0` success; `1` runtime error; `2` argument error; `3` anti-bot detected (wait 300s, use `--allow-lite-fallback`); `4` timeout (raise `--global-timeout` or reduce `--num`); `5` zero results legitimate (reformulate query or change `--lang`); `6` suspected block (inspect `.metadados.causa_zero`)
- 5-level anti-bot cascade (field `.metadados.nivel_cascata`, 0..=4) — level 0 same identity, level 1 same family/different platform, level 2 different family/same platform, level 3 different families and platforms with endpoint downgraded to lite, level 4 random identity
- IF `nivel_cascata == 4` observed, MUST rotate proxy or wait 300s before retrying
- Parser errors (exit 2) emit an ACTIONABLE HINT on stderr when the unknown flag is a known global flag (`-n`, `-f`, `-o`, `-t`, `-l`, `-c`, `-p`, `-q`, `-v`, `--allow-lite-fallback`, `--pre-flight`, `--global-timeout`) — MUST read the stderr to determine if it is a typo, wrong order or non-existent flag
- Before reporting a block to the user, ALWAYS capture the full stderr — the hint indicates the exact corrective action


## JSON contract — guaranteed versus optional fields
- GUARANTEED non-null — `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`
- OPTIONAL `Option<String>` — `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`, `.metadados.identidade_usada`
- CONDITIONAL with `--vertical news|all` — `.noticias[].{posicao,titulo,url}` guaranteed, `.noticias[].{fonte,data_relativa,thumbnail}` optional, `.quantidade_noticias`, `.metadados.vertical_usada`; ABSENT in web mode (byte-identical contract)
- OPTIONAL `Option<u32>` — `.metadados.nivel_cascata` (0..=4)
- CONDITIONAL on `--fetch-content` — `.resultados[].conteudo`, `.resultados[].tamanho_conteudo`, `.resultados[].metado_extracao_conteudo`
- The field `tamanho_conteudo` reflects the REAL size of the truncated text after extraction, NOT the original page size
- Chrome fields — `.metadados.usou_chrome` (bool), `.metadados.tentou_chrome` (bool)
- Diagnostic fields — `.metadados.causa_zero` (kebab-case enum), `.metadados.sugestao_proxima_acao` (string)
- Compression fields — `.metadados.bytes_brutos` (Option<u64>), `.metadados.bytes_descomprimidos` (Option<u64>)
- Pre-flight fields — `.metadados.pre_flight_disparado` (bool), `.metadados.endpoint_usado` ("html" or "lite")
- Compatibility fields exist at BOTH root and `.metadados` level — `quantidade_resultados`, `endpoint_usado`, `nivel_cascata` are accessible from either path
- Deep-research envelope — `.query` (string, top-level), `.sintese` (Markdown), `.metadados.sub_queries[]`, `.resultados[].titulo` (consistent with normal search via serde rename)
- Deep-research news — `.noticias[]` ALWAYS present (empty on zero or `--no-news`) with `posicao`, `titulo`, `url`, `score`, `ocorrencias` guaranteed and `fonte`, `data_relativa`, `thumbnail` optional; `.quantidade_noticias` and `.metadados.total_noticias_unicas` ALWAYS present; `.metadados.sub_queries[].{quantidade_noticias,news_indisponivel}` optional
- ALWAYS distinguish roots — `.resultados[]` (single-query), `.buscas[]` (multi-query), `.resultados[]` (deep-research)
- Identity format — `<family>-<platform>-<16hex>` where 16hex is the first 16 chars of seed-derived hash


## Deep-research dual web+news
- MUST generate 3-5 specific sub-queries — NEVER rely on the default heuristic strategy
- ALWAYS use `--sub-query-strategy manual --sub-queries-file` with LLM-generated questions
- Each sub-query MUST target a distinct aspect — architecture, benchmarks, pricing, limitations, comparisons
- Output includes `.query` (original query at top-level), `.sintese` (Markdown), `.metadados.sub_queries[]` (per-subquery status), `.resultados[]` (RRF-aggregated)
- Field `.resultados[].titulo` is consistent with normal search (serde rename applied)
- COMBINE with `--pre-flight` for blocked environments
- deep-research scans the news vertical by DEFAULT — each sub-query runs `--vertical all` in its own Chrome session — MUST use `timeout 180 duckduckgo-search-cli -q -f json deep-research "query" | jaq '.noticias[:5]'` for aggregated fresh articles
- News RRF is SEPARATE from web RRF — NEVER compare `.noticias[].score` with `.resultados[].score`; dedupe by canonical URL, ties broken by recency (`data_relativa` stays verbatim in JSON)
- With `--synthesize` the report is dual — web section ~70% of `--budget-tokens`, "Notícias recentes" section ~30%; unchanged with `--no-news` or zero news
- Exit codes — `0` when web OR news produced results; `5` only when BOTH are empty
- IN Chrome-less environments (CI) deep-research AUTO-APPLIES `--no-news` with a stderr warning and proceeds web-only — explicit `--no-news` is OPTIONAL
- Use `--require-results` to fail fast (exit 4) when fan-out aggregates zero results
- Use `--depth <N>` for reflection depth (0 to 3, default 0)


## FORBIDDEN rules
- FORBIDDEN `-f text` or `-f markdown` for programmatic parsing — ALWAYS `-f json`
- FORBIDDEN omit `-q` in pipelines — stderr tracing pollutes stdout
- FORBIDDEN `--stream` — flag reserved, NOT implemented
- FORBIDDEN hardcode API keys, proxies, or User-Agents
- FORBIDDEN hardcode `--identity-profile` in CI — let the 12-identity pool adapt
- FORBIDDEN `--output` with `..` or system directories (`/etc`, `/usr`, `C:\Windows`)
- FORBIDDEN treat `identidade_usada` or `nivel_cascata` as guaranteed — both are `Option<T>`
- FORBIDDEN commit `cookies.json` — credential-adjacent file
- FORBIDDEN ignore `quantidade_resultados:0` — may be ghost-block (use `--pre-flight` or inspect `causa_zero`)
- FORBIDDEN ignore exit code 6 — indicates suspected block requiring action
- FORBIDDEN log `--proxy` credentials in visible outputs
- FORBIDDEN shell retry loops — use native `--retries`
- FORBIDDEN combine `--proxy` with `--no-proxy` — mutually exclusive


## Security and environment variables
- Cookie jar path — `~/.config/duckduckgo-search-cli/cookies.json` (Unix mode 0o600)
- MUST NOT log or echo cookie contents
- MUST NOT pass `--cookies-path` to unencrypted volumes in production
- MUST use `--no-cookie-persistence` for ephemeral sessions
- `DUCKDUCKGO_CHROME_HEADLESS=1` — force headless mode (Cloudflare detection risk)
- `DUCKDUCKGO_CHROME_VISIBLE=1` — visible headed mode (debugging only)
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` — restore legacy exit 5 for all zeros (accepted falsy values — false, 0, no, off, empty string)
- `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` — standard proxy env vars honored unless `--no-proxy` is set


## Build and runtime prerequisites
- BUILD deps — Rust toolchain ONLY — reqwest+rustls-tls eliminates all native build dependencies
- `cargo install duckduckgo-search-cli --locked --force` works on Linux, macOS and Windows
- RUNTIME Linux — Google Chrome or Chromium + Xvfb (auto-installed by CLI on 22+ distros)
- RUNTIME macOS and Windows — Google Chrome or Chromium (Xvfb not needed)
- The CLI displays auto-install instructions via stderr ALWAYS visible regardless of `-q` or log level
