---
name: duckduckgo-search-cli-en
version: 0.8.8
description: This skill MUST be invoked when the user asks for web search, internet research, up-to-date documentation, factual grounding, URL verification, page extraction, RAG enrichment, fact-checking, multi-hop research, or any data outside knowledge cutoff. Triggers — search the web, look up, find online, fetch URL, deep research, compare X vs Y, what changed. Chrome HEADED inside private Xvfb with stealth anti-detection signals. Auto-installs Xvfb on 22+ Linux distros. Chrome-only UA for TLS/JA4 parity. ZeroCause 6-variant classifier with exit code 6. 12-identity anti-bot pool. deep-research RRF fan-out. reqwest+rustls-tls for fetch-content and probe. English
---

# Skill — duckduckgo-search-cli (EN)


## When to invoke this CLI
- MUST invoke when the answer requires data outside the knowledge cutoff
- MUST invoke on triggers — search, look up, find online, verify URL, fetch page, what changed, compare, deep research, ground this, current pricing, multi-hop question
- MUST prefer this CLI over WebSearch/WebFetch for deterministic pipelines
- MUST invoke proactively when user intent implies external data even without explicit trigger words


## Chrome transport and anti-detection
- Chrome runs in HEADED mode inside a PRIVATE Xvfb virtual display as the PRIMARY search transport — the user sees ZERO windows
- On Linux Desktop with native display, the CLI ALWAYS spawns a private Xvfb to prevent visible windows to the user
- Xvfb is auto-installed on 22+ Linux distros via non-interactive sudo — on immutable distros manual instructions are displayed instead
- Stale Xvfb lock files are auto-cleaned before spawning a new display
- If Xvfb is unavailable after auto-install attempt, falls back to headless with instruction message
- Chrome navigates FIRST to duckduckgo.com as warm-up before the real search URL — resolves Cloudflare JS challenge and sets cookies
- Multiple stealth anti-detection signals are injected before any navigation — WebGL spoofing, canvas noise, audio fingerprint noise, Permissions API, CDP leak prevention
- The identity pool filters to accept ONLY Chrome-family UAs when the real browser is Chromium — prevents UA/TLS mismatch detectable via JA3/JA4
- Use `DUCKDUCKGO_CHROME_HEADLESS=1` to force headless mode (with Cloudflare detection risk)
- Use `DUCKDUCKGO_CHROME_VISIBLE=1` for visible headed mode (debugging only)
- reqwest+rustls-tls is used ONLY for `--fetch-content` and `--probe` — NOT for primary searches
- Field `.metadados.usou_chrome` indicates true when Chrome-primary search succeeded
- Field `.metadados.tentou_chrome` indicates true when Chrome search was attempted


## Mandatory usage formulas
- MUST use `timeout 60 duckduckgo-search-cli "<query>" -q -f json --num 15 | jaq '.resultados'` for single query
- MUST use `timeout 15 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'` to detect CAPTCHA before queries
- MUST use `timeout 60 duckduckgo-search-cli --pre-flight "<query>" -q -f json` to prevent silent ghost-block on shared IPs
- MUST use `timeout 60 duckduckgo-search-cli --allow-lite-fallback "<query>" -q -f json` for automatic HTML to Lite downgrade on CAPTCHA
- MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "<query>" --sub-query-strategy manual --sub-queries-file /tmp/sub-queries.txt --aggregate rrf` for multi-hop research
- MUST use `timeout 120 duckduckgo-search-cli "<query>" -q -f json --num 10 --fetch-content --max-content-length 5000` to extract page content for LLM context
- MUST use `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --num 15 --global-timeout 280` for batching 3+ queries
- MUST use `timeout 60 duckduckgo-search-cli --proxy socks5://host:port "<query>" -q -f json` for proxied searches
- MUST use `timeout 60 duckduckgo-search-cli --safe-search off "<query>" -q -f json` to disable safe search filtering
- MUST use `timeout 60 duckduckgo-search-cli --config /path/to/config.toml "<query>" -q -f json` to load TOML configuration
- ALWAYS wrap with `timeout` in seconds — pipeline hangs without time limit
- ALWAYS use `-q` in pipelines — stderr tracing pollutes stdout without this flag
- ALWAYS use `-f json` for programmatic parsing — NEVER `-f text` or `-f markdown`
- ALWAYS use `jaq` (NEVER `jq`) to process JSON output
- ALWAYS apply `// ""` fallback on optional fields when using `jaq`
- The `--allow-lite-fallback` flag MUST come BEFORE the `deep-research` subcommand — Clap rejects it after the subcommand with exit 2
- MUST use `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'` for tabulated TSV
- MUST use `jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'` for markdown top 5 list
- MUST use `jaq -r '.resultados[] | "- \(.titulo) — \(.url)"'` for source citation block


## Complete flag reference with formulas
- `--num N` — number of results (minimum 1, `--num 0` rejected by clap) — MUST use `timeout 60 duckduckgo-search-cli "query" -q -f json --num 30`
- `--lang <CODE>` — search language (e.g., pt-BR, en-US) — MUST use `timeout 60 duckduckgo-search-cli --lang pt-BR "query" -q -f json`
- `--country <CODE>` / `-c` — country code for regional search (e.g., br, us, de) — `--region` is an ALIAS for `--country` — MUST use `timeout 60 duckduckgo-search-cli --country br "query" -q -f json`
- `--time-filter <PERIOD>` — time filter (d=day, w=week, m=month, y=year) — MUST use `timeout 60 duckduckgo-search-cli --time-filter d "query" -q -f json` for last 24h
- `--safe-search <LEVEL>` — safe search level (off, moderate, on; default moderate) — MUST use `timeout 60 duckduckgo-search-cli --safe-search off "query" -q -f json`
- `--endpoint <NAME>` — search endpoint (html or lite; default html) — MUST use `timeout 60 duckduckgo-search-cli --endpoint lite "query" -q -f json`
- `-f json` / `--format json` — output format — MANDATORY for programmatic parsing, NEVER use `-f text` or `-f markdown`
- `-q` / `--quiet` — suppress stderr tracing — MANDATORY in pipelines to prevent stdout pollution
- `-v` info / `-vv` debug / `-vvv` trace — additive verbosity levels — MUST use `timeout 60 duckduckgo-search-cli -vv "query" -f json 2>/tmp/debug.log`
- `--no-color` — disable colored output — MUST use `timeout 60 duckduckgo-search-cli --no-color "query" -q -f json`
- `--output <PATH>` — atomic write of full payload (rejected if `..` or system directories) — MUST use `timeout 60 duckduckgo-search-cli "query" -q -f json --output /tmp/results.json`
- `--retries N` — retries with exponential backoff (clamped [1, 10], NEVER > 10) — MUST use `timeout 60 duckduckgo-search-cli --retries 3 "query" -q -f json`
- `--timeout N` — per-request HTTP timeout in seconds — MUST use `timeout 60 duckduckgo-search-cli --timeout 20 "query" -q -f json`
- `--global-timeout N` — global operation timeout in seconds — MUST use `timeout 90 duckduckgo-search-cli --global-timeout 60 "query" -q -f json`
- `--config <PATH>` — load TOML configuration file — MUST use `timeout 60 duckduckgo-search-cli --config ./search.toml "query" -q -f json`
- `--probe` — minimal health check via reqwest — MUST use `timeout 10 duckduckgo-search-cli --probe -q -f json | jaq '.status'`
- `--probe-deep` — real query CAPTCHA detector — MUST use `timeout 15 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'`
- `--pre-flight` — auto-route via probe-deep first — MUST use `timeout 60 duckduckgo-search-cli --pre-flight "query" -q -f json`
- `--allow-lite-fallback` — opt-in HTML to Lite downgrade on CAPTCHA — MUST come BEFORE `deep-research` subcommand — MUST use `timeout 60 duckduckgo-search-cli --allow-lite-fallback "query" -q -f json`
- `--identity-profile <name>` — pin identity (auto/chrome-win/chrome-mac/chrome-linux/edge-win/firefox-linux/safari-mac) — MUST use `timeout 60 duckduckgo-search-cli --identity-profile chrome-linux "query" -q -f json`
- `--seed <u64>` — deterministic seed for UA + pool rotation — MUST use `timeout 60 duckduckgo-search-cli --seed 42 "query" -q -f json` for reproducibility
- `--no-warmup` — skip cookie warm-up to duckduckgo.com — MUST use `timeout 60 duckduckgo-search-cli --no-warmup "query" -q -f json`
- `--no-cookie-persistence` — in-memory cookies only — MUST use `timeout 60 duckduckgo-search-cli --no-cookie-persistence "query" -q -f json`
- `--cookies-path <PATH>` — redirect jar to encrypted volume — MUST use `timeout 60 duckduckgo-search-cli --cookies-path /secure/cookies.json "query" -q -f json`
- `--chrome-path <PATH>` — manual Chrome/Chromium binary path — MUST use `timeout 60 duckduckgo-search-cli --chrome-path /usr/bin/chromium "query" -q -f json`
- `--match-platform-ua` — force platform-matching User-Agent — MUST use `timeout 60 duckduckgo-search-cli --match-platform-ua "query" -q -f json`
- `--proxy <URL>` — HTTP/HTTPS/SOCKS5 proxy — MUST use `timeout 60 duckduckgo-search-cli --proxy socks5://host:port "query" -q -f json`
- `--no-proxy` — disable proxy (ignores --proxy and HTTP_PROXY/HTTPS_PROXY/ALL_PROXY env vars) — MUST use `timeout 60 duckduckgo-search-cli --no-proxy "query" -q -f json`
- `--queries-file <PATH>` — file with queries for batch (one per line) — MUST use `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --num 15`
- `--parallel N` — parallel queries (NEVER > 5) — MUST use `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--per-host-limit N` — per-host limit (NEVER > 2) — MUST use `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- `--fetch-content` — extract HTML content from result pages — MUST use `timeout 120 duckduckgo-search-cli "query" -q -f json --fetch-content --max-content-length 5000`
- `--max-content-length N` — byte limit per extracted page — MANDATORY with `--fetch-content`, ALWAYS combine
- `--sub-query-strategy <STRATEGY>` — `heuristic` (default, low quality) or `manual` — ALWAYS use `manual` with `--sub-queries-file`
- `--sub-queries-file <PATH>` — file with manual sub-queries (one per line) — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--aggregate <METHOD>` — deep-research aggregation method — MUST use `--aggregate rrf` for Reciprocal Rank Fusion
- `--max-sub-queries N` — sub-query limit in deep-research — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --max-sub-queries 5`
- `--synthesize` — generate synthesis in deep-research — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --synthesize --budget-tokens 2000`
- `--budget-tokens N` — token limit for synthesis — ALWAYS combine with `--synthesize`
- `--synth-format <FORMAT>` — deep-research synthesis format (markdown, plain-text, json) — the correct value is `plain-text`, NOT `plain` — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --synth-format plain-text`
- `--pages <N>` — pages per query (1 to 5, default 1; auto-elevated when --num > 10) — MUST use `timeout 120 duckduckgo-search-cli "query" -q -f json --num 20 --pages 3`
- `--require-results` — deep-research flag; exit 4 when fan-out aggregates zero results — MUST use `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --require-results`
- `--depth <N>` — deep-research reflection depth (0 to 3, default 0) — MUST use `timeout 180 duckduckgo-search-cli -q -f json deep-research "query" --depth 2`
- `--force` — init-config flag; overwrite existing configuration files — MUST use `duckduckgo-search-cli init-config --force`
- `--dry-run` — init-config flag; simulate config generation without writing files — MUST use `duckduckgo-search-cli init-config --dry-run`


## ZeroCause classifier for zero-result diagnosis
- MUST inspect `.metadados.causa_zero` on EVERY response with `quantidade_resultados == 0`
- 6 classified causes — `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`
- When `causa_zero != "legitimo"`, the CLI emits exit code 6 (`SUSPECTED_BLOCK`) by default
- Field `.metadados.sugestao_proxima_acao` contains actionable instruction when cause is non-legitimate
- To restore legacy behavior (exit 5 for all zeros) — set `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`
- In multi-query (batch), the `.causa_zero_histogram` field aggregates counts across sub-queries


## Guaranteed versus optional JSON fields
- GUARANTEED non-null — `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`
- OPTIONAL `Option<String>` — `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`, `.metadados.identidade_usada`
- OPTIONAL `Option<u32>` — `.metadados.nivel_cascata` (0..=4)
- CONDITIONAL on `--fetch-content` — `.resultados[].conteudo`, `.resultados[].tamanho_conteudo`, `.resultados[].metado_extracao_conteudo`
- The field `tamanho_conteudo` reflects the REAL size of the truncated text after extraction, NOT the original page size
- Chrome fields — `.metadados.usou_chrome` (bool), `.metadados.tentou_chrome` (bool)
- Diagnostic fields — `.metadados.causa_zero` (kebab-case enum), `.metadados.sugestao_proxima_acao` (string)
- Compression fields — `.metadados.bytes_brutos` (Option<u64>), `.metadados.bytes_descomprimidos` (Option<u64>)
- Pre-flight fields — `.metadados.pre_flight_disparado` (bool), `.metadados.endpoint_usado` ("html" or "lite")
- Compatibility fields exist at BOTH root and `.metadados` level — `quantidade_resultados`, `endpoint_usado`, `nivel_cascata` are accessible from either path
- Deep-research envelope — `.query` (string, top-level), `.sintese` (Markdown), `.metadados.sub_queries[]`, `.resultados[].titulo` (consistent with normal search via serde rename)
- ALWAYS distinguish roots — `.resultados[]` (single-query), `.buscas[]` (multi-query), `.resultados[]` (deep-research)
- Identity format — `<family>-<platform>-<16hex>` where 16hex is the first 16 chars of seed-derived hash


## Exit code map
- `0` — success
- `1` — runtime error
- `2` — argument error
- `3` — anti-bot detected (wait 300s, use `--allow-lite-fallback`)
- `4` — timeout (raise `--global-timeout` or reduce `--num`)
- `5` — zero results legitimate (reformulate query or change `--lang`)
- `6` — suspected block (inspect `.metadados.causa_zero`)
- MUST capture exit code BEFORE parsing stdout
- MUST use `${PIPESTATUS[0]}` when piped through `jaq`


## 5-level anti-bot cascade
- Level 0 — Same identity, no rotation
- Level 1 — Same family, different platform
- Level 2 — Different family, same platform
- Level 3 — Different family + platform + endpoint downgraded to lite
- Level 4 — Random identity (caller waits 30-60s before retry)
- FAILURE — Report with cause + retry_after_seconds
- IF `nivel_cascata == 4` observed, MUST rotate proxy or wait 300s


## Deep-research with manual sub-queries
- MUST generate 3-5 specific sub-queries — NEVER rely on the default heuristic strategy
- ALWAYS use `--sub-query-strategy manual --sub-queries-file` with LLM-generated questions
- Each sub-query MUST target a distinct aspect — architecture, benchmarks, pricing, limitations, comparisons
- Output includes `.query` (original query at top-level), `.sintese` (Markdown), `.metadados.sub_queries[]` (per-subquery status), `.resultados[]` (RRF-aggregated)
- Field `.resultados[].titulo` is consistent with normal search (serde rename applied)
- COMBINE with `--pre-flight` for blocked environments
- `--synth-format` accepts `markdown` (default), `plain-text` or `json` — the correct value is `plain-text`, NOT `plain`
- Use `--require-results` to fail fast (exit 4) when fan-out aggregates zero results
- Use `--depth <N>` for reflection depth (0 to 3, default 0)


## FORBIDDEN rules
- FORBIDDEN `-f text` or `-f markdown` for programmatic parsing — ALWAYS `-f json`
- FORBIDDEN omit `-q` in pipelines — stderr tracing pollutes stdout
- FORBIDDEN `--stream` — flag reserved, NOT implemented
- FORBIDDEN `--parallel > 5` without outbound IP control
- FORBIDDEN `--per-host-limit > 2` — triggers HTTP 202 anti-bot
- FORBIDDEN shell retry loops — use native `--retries`
- FORBIDDEN hardcode API keys, proxies, or User-Agents
- FORBIDDEN hardcode `--identity-profile` in CI — let the 12-identity pool adapt
- FORBIDDEN `--output` with `..` or system directories (`/etc`, `/usr`, `C:\Windows`)
- FORBIDDEN treat `identidade_usada` or `nivel_cascata` as guaranteed — both are `Option<T>`
- FORBIDDEN commit `cookies.json` — credential-adjacent file
- FORBIDDEN ignore `quantidade_resultados:0` — may be ghost-block (use `--pre-flight` or inspect `causa_zero`)
- FORBIDDEN ignore exit code 6 — indicates suspected block requiring action
- FORBIDDEN `--num 0` — rejected by clap
- FORBIDDEN `--synth-format plain` — the correct value is `plain-text`
- FORBIDDEN `--fetch-content` without `--max-content-length` — unbounded memory growth
- FORBIDDEN `--retries > 10` — guaranteed anti-bot trigger
- FORBIDDEN `--region br-pt` — use `--country br` (country code only, `--region` is alias for `--country`)
- FORBIDDEN `--proxy` and `--no-proxy` simultaneously — mutually exclusive


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
- Manual Xvfb install commands — `sudo apt-get install -y xvfb` (Debian/Ubuntu), `sudo dnf install -y xorg-x11-server-Xvfb` (Fedora), `sudo pacman -S --noconfirm xorg-server-xvfb` (Arch)
- The CLI displays auto-install instructions via stderr ALWAYS visible regardless of `-q` or log level
