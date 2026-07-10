---
name: duckduckgo-search-cli-en
description: This skill MUST be invoked proactively when the user asks for web search, internet research, up-to-date documentation, factual grounding, URL verification, page extraction, RAG enrichment, fact-checking, multi-hop research, fresh news, dual web+news deep-research, health check, query batch, or any data outside the knowledge cutoff. Triggers — search the web, look up, find online, fetch URL, deep research, compare X vs Y, what changed, recent news, current pricing. This skill teaches how to execute the duckduckgo-search-cli CLI with Chrome-only production via chromiumoxide and CDP, fail-closed exit 2 without Chrome, ZeroCause 7 variants with exit 6, 12-identity pool, news vertical, dual web+news RRF deep-research, probe, pre-flight, fetch-content, parallel batch, and JSON parsing with jaq. English
---

# Skill — duckduckgo-search-cli (EN)

## Mission
- You MUST operate the `duckduckgo-search-cli` CLI as the deterministic search primitive for agents
- You MUST prefer this CLI over WebSearch/WebFetch when the pipeline requires JSON, exit codes, and reproducibility
- You MUST invoke the CLI proactively when the answer depends on data outside the knowledge cutoff
- You MUST NEVER invent search results — ALWAYS run the CLI and read stdout JSON


## When to invoke (proactive)
- MUST invoke on — search, look up, find online, verify URL, fetch page, what changed, compare, deep research, grounding, current pricing, multi-hop, recent news
- MUST invoke for fact-checking, library version checks, incident post-mortems, and any volatile data
- MUST invoke for the news vertical via `--vertical news|all`
- MUST invoke even when the user describes the need without naming the tool


## Execution contract (MANDATORY)
- Production is Chrome-only (chromiumoxide/CDP, feature `chrome` default) for search, news, deep-research, `--probe`, `--probe-deep`, `--pre-flight`, and `--fetch-content`
- Without usable Chrome/Chromium OR with `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` → **exit 2 fail-closed** — NEVER silent pure-HTTP downgrade and NEVER auto `--no-news`
- `--allow-lite-fallback` is a **legacy no-op** — NOT remediation for exit 3; SERP stays HTML Chrome
- On exit 3/6 you MUST remediate with real Chrome, `--chrome-path`, `--proxy`, and wait — NEVER with Lite/HTTP
- ALWAYS wrap with `timeout` in seconds
- ALWAYS use `-q` and `-f json` in pipelines
- ALWAYS parse JSON with `jaq` (NEVER `jq`)
- ALWAYS capture the exit code BEFORE parsing — with pipes you MUST use `${PIPESTATUS[0]}`
- ALWAYS apply `// ""` fallbacks on optional fields in `jaq`
- Runtime Linux — Chrome/Chromium + Xvfb (auto-install on 22+ distros); macOS/Windows — Chrome/Chromium (headless=new)
- Fields `.metadados.usou_chrome` / `.metadados.tentou_chrome` report Chrome attempt and success


## Workflow — numbered steps
1. MUST choose the mode — simple search, news, deep-research, batch, probe, or fetch-content
2. MUST build the formula with `timeout`, `-q`, `-f json`, and mode flags
3. MUST execute and capture exit code + stdout
4. IF exit 0 and `quantidade_resultados > 0` — MUST extract with `jaq` and cite sources
5. IF `quantidade_resultados == 0` — MUST read `.metadados.causa_zero` and `.metadados.sugestao_proxima_acao`
6. IF exit 2 — MUST install/provide Chrome or fix args; NEVER set `NO_CHROME=1` in production
7. IF exit 3 or 6 — MUST wait, rotate proxy, and revalidate Chrome; NEVER use Lite as remediation
8. IF exit 4 — MUST raise `--global-timeout` or reduce `--num`/`--parallel`
9. IF exit 5 with `causa_zero == "legitimo"` or `vertical-sem-resultados` — MUST reformulate the query


## Search formulas (global flags)
MUST copy and adapt. Every formula is imperative.

- MANDATORY base — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json`
- `--num` / `-n` (default 15, minimum 1) — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json --num 30`
- `--lang` / `-l` (default `pt`) — `timeout 60 duckduckgo-search-cli --lang en-US "QUERY" -q -f json`
- `--country` / `-c` (default `br`; alias `--region`) — `timeout 60 duckduckgo-search-cli --country us "QUERY" -q -f json`
- `--time-filter` (`d|w|m|y`) — `timeout 60 duckduckgo-search-cli --time-filter d "QUERY" -q -f json`
- `--safe-search` (`off|moderate|on`, default moderate) — `timeout 60 duckduckgo-search-cli --safe-search off "QUERY" -q -f json`
- `--endpoint` (`html|lite`, default html) — under production Chrome the SERP is canonical HTML; `--endpoint lite` does NOT remediate blocks and is NOT a success path — `timeout 60 duckduckgo-search-cli --endpoint html "QUERY" -q -f json`
- `--pages` (1..5, default 1; auto-elevates when `--num > 10`) — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json --num 20 --pages 3`
- `-f` / `--format` (`json|text|markdown|md|auto`; default `auto` = text in TTY and json in a pipe; `--output` forces json) — for agents ALWAYS `-f json`
- `-q` / `--quiet` — MANDATORY in pipelines
- `-v` / `-vv` — correct stderr levels — `0`=INFO, `-v`=DEBUG, `-vv` or more=TRACE — `timeout 60 duckduckgo-search-cli -v "QUERY" -f json 2>/tmp/ddg-debug.log`
- `--no-color` — `timeout 60 duckduckgo-search-cli --no-color "QUERY" -q -f json`
- `--output` / `-o` — atomic write; FORBIDDEN `..` and system dirs — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json --output /tmp/results.json`
- `--retries` (0..10, default 2) — `timeout 60 duckduckgo-search-cli --retries 3 "QUERY" -q -f json`
- `--timeout` / `-t` (default 15s, per request) — `timeout 60 duckduckgo-search-cli --timeout 20 "QUERY" -q -f json`
- `--global-timeout` (1..3600, default 60) — `timeout 90 duckduckgo-search-cli --global-timeout 60 "QUERY" -q -f json`
- `--config` — `timeout 60 duckduckgo-search-cli --config ./config.toml "QUERY" -q -f json`
- `--seed` — reproducibility — `timeout 60 duckduckgo-search-cli --seed 42 "QUERY" -q -f json`
- `--identity-profile` (`auto|chrome-win|chrome-mac|chrome-linux|edge-win|firefox-linux|safari-mac`) — `timeout 60 duckduckgo-search-cli --identity-profile chrome-linux "QUERY" -q -f json`
- `--match-platform-ua` — `timeout 60 duckduckgo-search-cli --match-platform-ua "QUERY" -q -f json`
- `--no-warmup` — `timeout 60 duckduckgo-search-cli --no-warmup "QUERY" -q -f json`
- `--no-cookie-persistence` — `timeout 60 duckduckgo-search-cli --no-cookie-persistence "QUERY" -q -f json`
- `--cookies-path` — `timeout 60 duckduckgo-search-cli --cookies-path /secure/cookies.json "QUERY" -q -f json`
- `--chrome-path` — `timeout 60 duckduckgo-search-cli --chrome-path /usr/bin/chromium "QUERY" -q -f json`
- `--proxy` (HTTP/HTTPS/SOCKS5/SOCKS5h) — `timeout 60 duckduckgo-search-cli --proxy socks5://127.0.0.1:1080 "QUERY" -q -f json`
- `--no-proxy` — mutually exclusive with `--proxy` — `timeout 60 duckduckgo-search-cli --no-proxy "QUERY" -q -f json`
- `--allow-lite-fallback` — no-op; legacy scripts do not break; do NOT use as remediation
- `--stream` — FORBIDDEN (unimplemented placeholder)
- stdin (one query per line) — `printf '%s\n' "q1" "q2" | timeout 120 duckduckgo-search-cli -q -f json`


## News vertical
- MUST use `--vertical news` for news-only and `--vertical all` for web+news
- news/all require Chrome; without Chrome → exit 2 fail-closed
- `--fetch-content` does NOT apply to news cards
- News formula — `timeout 90 duckduckgo-search-cli --vertical news "QUERY" -q -f json | jaq '.noticias'`
- All formula — `timeout 90 duckduckgo-search-cli --vertical all "QUERY" -q -f json | jaq '{web:.resultados,news:.noticias}'`
- News extract — `jaq -r '.noticias[] | [.posicao, .titulo, .url, (.fonte // ""), (.data_relativa // "")] | @tsv'`


## Diagnosis — probe, pre-flight, ZeroCause
- `--probe` — minimal Chrome health check — `timeout 15 duckduckgo-search-cli --probe -q -f json | jaq '.status'`
- `--probe-deep` — CAPTCHA/interstitial detector — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'`
- `--pre-flight` — auto-route via probe-deep — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- MUST inspect `.metadados.causa_zero` on EVERY zero
- 7 causes — `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`, `vertical-sem-resultados`
- `vertical-sem-resultados` → exit 5 (legitimate empty news), NOT exit 6
- `causa_zero != "legitimo"` → exit 6 by default (`SUSPECTED_BLOCK`)
- `.metadados.sugestao_proxima_acao` MUST be followed when present (points to Chrome/proxy/wait — NOT Lite)
- Legacy opt-out — `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` restores exit 5 for all zeros
- Multi-query — `.causa_zero_histogram` aggregates causes
- Cascade `.metadados.nivel_cascata` (0..4, Option) — if 4, MUST rotate proxy or wait 300s


## Exit codes
| Code | Meaning | MANDATORY action |
|------|---------|------------------|
| 0 | Success with results | Parse JSON and cite |
| 1 | Runtime (network/I/O/parse) | Report stderr; retry with `--retries` |
| 2 | Invalid config/args OR missing Chrome/`NO_CHROME=1` | Fix args; install Chrome; NEVER `NO_CHROME=1` in production |
| 3 | Anti-bot soft-block | Wait 300s; `--chrome-path`; `--proxy`; NEVER Lite |
| 4 | Global timeout | Raise `--global-timeout`; reduce load |
| 5 | Legitimate zero | Reformulate query/`--lang`/`--time-filter` |
| 6 | Suspected block | Read `causa_zero` + `sugestao_proxima_acao` |
| 130 | Cancelled (SIGINT) | Do NOT treat as a search failure; re-run if needed |


## Batch, content, and parallel
- `--queries-file` — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --num 15`
- `--parallel` / `-p` (default 5, clamp 1..20; prefer ≤5 against anti-bot) — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--per-host-limit` (1..10, default 2; with fetch-content prefer ≤2) — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- Multi-query positional — `timeout 120 duckduckgo-search-cli -q -f json "query one" "query two"`
- Multi-query root — `.buscas[]` (NEVER confuse with single-query `.resultados[]`)
- `--fetch-content` + `--max-content-length` (default 10000 **characters** of extracted text, 1..100000) — `timeout 120 duckduckgo-search-cli "QUERY" -q -f json --fetch-content --max-content-length 5000`
- Also valid under deep-research — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --fetch-content --max-content-length 5000`
- `tamanho_conteudo` = truncated post-extraction text size


## Deep-research dual web+news
- By default each sub-query runs vertical `all` (web+news) under Chrome
- Without Chrome → exit 2 fail-closed (no auto `--no-news`)
- MUST prefer LLM-generated manual sub-queries — NEVER rely only on `heuristic`
- Base formula — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY"`
- Quality MANDATORY manual — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--max-sub-queries` (1..12, default 5) — `... deep-research "QUERY" --max-sub-queries 5`
- `--aggregate` (`rrf` default | `dedupe-by-url`) — `... --aggregate rrf`
- `--synthesize` + `--budget-tokens` (default 4000) — `... --synthesize --budget-tokens 2000`
- `--synth-format` (`markdown|plain-text|json`) — correct value is `plain-text`, NEVER `plain`
- `--require-results` — non-zero exit if fan-out aggregates zero — `... --require-results`
- `--depth` (0..3, default 0) — `... --depth 2`
- `--no-news` — intentionally skip news; use ONLY with Chrome available and explicit intent
- Global flags `-n -f -o -t -l -c -p -q -v` are accepted BEFORE or AFTER `deep-research`
- News RRF is SEPARATE from web RRF — NEVER compare scores between `.noticias[]` and `.resultados[]`
- Deep-research exits — 0 if web OR news produced results; 5 only when BOTH are empty
- Envelope — `.query`, `.sintese`, `.resultados[]`, `.noticias[]`, `.metadados.sub_queries[]`, `.quantidade_noticias`, `.metadados.total_noticias_unicas`


## Parsing and JSON contract
- TSV — `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'`
- Top 5 markdown — `jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'`
- Citations — `jaq -r '.resultados[] | "- \(.titulo) — \(.url)"'`
- GUARANTEED — `.query`, `.resultados[].posicao|.titulo|.url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`
- OPTIONAL — `.resultados[].snippet|.url_exibicao|.titulo_original`, `.metadados.identidade_usada`, `.metadados.nivel_cascata`
- CONDITIONAL news — `.noticias[]`, `.quantidade_noticias`, `.metadados.vertical_usada`
- CONDITIONAL fetch-content — `.resultados[].conteudo|.tamanho_conteudo|.metodo_extracao_conteudo`
- Chrome — `.metadados.usou_chrome`, `.metadados.tentou_chrome`
- Diagnosis — `.metadados.causa_zero`, `.metadados.sugestao_proxima_acao`
- Pre-flight — `.metadados.pre_flight_disparado`, `.metadados.endpoint_usado`
- Compat — `quantidade_resultados`, `endpoint_usado`, `nivel_cascata` exist at root AND under `.metadados`
- Identity — format `<family>-<platform>-<16hex>`
- Distinguish roots — single `.resultados[]` | multi `.buscas[]` | deep-research `.resultados[]`+`.noticias[]`


## Helper subcommands
- `init-config` — `duckduckgo-search-cli init-config`
- `init-config --force` — overwrite
- `init-config --dry-run` — simulate without writing
- `completions` — `duckduckgo-search-cli completions bash` (also zsh, fish, powershell, elvish)
- Install — `cargo install duckduckgo-search-cli --locked --force`


## Security and environment
- Default cookie jar on Unix — `~/.config/duckduckgo-search-cli/cookies.json` (0o600); Windows — `%APPDATA%\duckduckgo-search-cli\cookies.json`
- NEVER log cookies; NEVER commit `cookies.json`
- NEVER log `--proxy` credentials
- MUST use `--no-cookie-persistence` for ephemeral sessions
- `DUCKDUCKGO_CHROME_HEADLESS=1` — force headless (anti-bot risk)
- `DUCKDUCKGO_CHROME_VISIBLE=1` — visible headed (debug)
- `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` — **FORBIDDEN in production** → exit 2 on any network op
- `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` — tests only with feature `http-test-harness`
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` — legacy exit 5 on zeros
- `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` — honored unless `--no-proxy`


## FORBIDDEN rules
- FORBIDDEN `-f text` or `-f markdown` for agent parsing — ALWAYS `-f json`
- FORBIDDEN omit `-q` in pipelines
- FORBIDDEN `--stream`
- FORBIDDEN hardcode API keys, proxies, or UAs in commits
- FORBIDDEN hardcode `--identity-profile` in CI — let the pool adapt
- FORBIDDEN `--output` with `..` or system directories
- FORBIDDEN treat `identidade_usada` or `nivel_cascata` as guaranteed
- FORBIDDEN ignore zero results without reading `causa_zero`
- FORBIDDEN ignore exit 6
- FORBIDDEN shell retry loops — use native `--retries`
- FORBIDDEN combine `--proxy` with `--no-proxy`
- FORBIDDEN use `--allow-lite-fallback` or Lite as block remediation
- FORBIDDEN set `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` in production
- FORBIDDEN `--synth-format plain` — correct value is `plain-text`
