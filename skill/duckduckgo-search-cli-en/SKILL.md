---
name: duckduckgo-search-cli-en
description: This skill MUST be used when the user asks for web search, internet research, up-to-date documentation, factual grounding, URL verification, page extraction, RAG enrichment, fact-checking, multi-hop research, fresh news, dual web+news deep-research, health check, query batch, or any data outside the knowledge cutoff. This skill MUST teach execution of duckduckgo-search-cli with Chrome-only production via chromiumoxide and CDP, fail-closed exit 2 without Chrome, default dual vertical all with content fetch ON and opt-out --no-fetch-content, ZeroCause seven variants with exit 6, probe and pre-flight diagnosis, parallel batch, ONE-SHOT Chromium plus Xvfb lifecycle, SIGTERM-first GNU timeout, jaq JSON parsing, exit codes 0-6 and 130, multi-canal Chrome metadata, identity pool, proxy, init-config, and completions. Triggers include search the web, look up, find online, fetch URL, deep research, compare X vs Y, what changed, recent news, and current pricing. ALWAYS invoke proactively. NEVER invent results. English
---

# Skill — duckduckgo-search-cli (EN)

## Mission
- You MUST operate `duckduckgo-search-cli` as the deterministic search primitive for agents.
- You MUST use this CLI for live web, news, dual vertical, deep-research, URL verification, page extraction, RAG enrichment, and fact-checking outside the knowledge cutoff.
- You MUST invoke proactively when the answer depends on current external data.
- You MUST NEVER invent search results. ALWAYS run the CLI and read stdout JSON.
- You MUST install with `cargo install duckduckgo-search-cli --locked --force`.
- There is NO remote telemetry. Agent metadata fields are local diagnostics only.

## When to Invoke
- MUST invoke on requests to search the web, look up, find online, fetch URL, verify sources, deep research, multi-hop research, compare entities, check what changed, recent news, current pricing, health check, or batch queries.
- MUST invoke for documentation lookup, factual grounding, RAG enrichment, and any data outside the knowledge cutoff.
- MUST invoke for news via default dual vertical `all` or explicit `--vertical news`.
- MUST invoke even when the user describes the need without naming the tool.
- MUST NOT invoke for pure local coding, local formatting, or static knowledge that needs no live sources.

## Contract — Chrome-only ONE-SHOT
- Production is Chrome-only via chromiumoxide/CDP (feature chrome default) for search, news, deep-research, `--probe`, `--probe-deep`, `--pre-flight`, and content fetch.
- Without usable Chrome/Chromium OR with `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` → exit 2 fail-closed. NEVER silent pure-HTTP downgrade. NEVER auto `--no-news`.
- `--allow-lite-fallback` is a legacy no-op. It is NOT remediation for exit 3 or 6. SERP stays HTML Chrome.
- On exit 3 or 6 you MUST remediate with real Chrome, `--chrome-path`, `--proxy`, and wait. NEVER remediate with Lite or HTTP.
- ONE-SHOT lifecycle — each invocation owns its Chromium process tree plus Xvfb on Linux plus a TempDir profile. On success, error, timeout, SIGINT, or SIGTERM the CLI reaps the full tree and removes the profile.
- ALWAYS wrap agent invocations with GNU timeout (SIGTERM first) via `/usr/bin/timeout`.
- MUST NOT expect automatic cleanup after bare SIGKILL of the CLI or of historical orphan processes.
- ALWAYS use `-q` and `-f json` in agent pipelines.
- ALWAYS parse JSON with `jaq`. NEVER use `jq`.
- ALWAYS capture the CLI exit code BEFORE parsing. With pipes you MUST use `${PIPESTATUS[0]}`.
- ALWAYS apply `// ""` fallbacks on optional fields in `jaq`.
- Runtime Linux requires Chrome/Chromium plus Xvfb. macOS/Windows require Chrome/Chromium headless=new.
- Agent metadata (NOT telemetry) MUST be inspected when present — `.metadados.chrome_path_resolvido`, `.metadados.chrome_canal` (`manual|env|host|flatpak|snap`), `.metadados.usou_chrome`, `.metadados.tentou_chrome` on single, multi (`.buscas[].metadados`), deep, and failure envelopes.
- Multi-canal Chrome resolution — Fedora host ELF `/usr/lib64/chromium-browser/chromium-browser`; wrapper `/usr/bin/chromium-browser` auto-resolves; Flatpak export `/var/lib/flatpak/exports/bin/com.google.Chrome` resolves to deploy ELF `.../files/extra/chrome`.
- DEFAULT vertical is `all` (web + news dual).
- DEFAULT content fetch is ON for web and news (top FETCH_CAP=10 URLs). Opt out with `--no-fetch-content`. `--fetch-content` is explicit redundant ON.
- `--max-content-length` default is 10000 (range 1..100000).
- `--stream` is FORBIDDEN (unimplemented).
- Transport and search flags with global=true work before or after `deep-research` — chrome-path, proxy, no-proxy, vertical, fetch flags, num, format, output, timeout, lang, country, parallel, quiet, verbose, identity-profile, match-platform-ua, pre-flight, allow-lite-fallback, global-timeout.
- Agent outer timeout guidance (GNU timeout seconds) — dual+fetch default 180; SERP-only with `--no-fetch-content` 90; thin web-only 60; deep-research 180; batch 300; probe 15; probe-deep 20.
- Defaults — num 15; format auto (agents MUST force json); timeout 15s; lang pt; country br; parallel 5 clamp 1..20; pages 1; retries 2; endpoint html; vertical all; safe-search moderate; identity-profile auto; max-content-length 10000; per-host-limit 2; global-timeout 60; max-sub-queries 5; aggregate rrf; depth 0; budget-tokens 4000.
- Atomic `--output` only. FORBIDDEN paths with `..` or system directories.
- MANDATORY base wrapper — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`

## Workflow
1. MUST choose the mode — simple search, news, deep-research, batch, probe, pre-flight, or SERP-only opt-out.
2. MUST build the formula with GNU `timeout` (SIGTERM first), `-q`, `-f json`, and mode flags. Raise outer timeout for dual+fetch (180), deep-research (180), or batch (300).
3. MUST execute and capture exit code plus stdout. With pipes MUST use `${PIPESTATUS[0]}` for the CLI status.
4. IF exit 0 and results present — MUST extract with `jaq` and cite sources. NEVER invent URLs or titles.
5. IF zero results — MUST read `.metadados.causa_zero` and `.metadados.sugestao_proxima_acao` before any retry.
6. IF exit 2 — MUST install or provide Chrome or fix args. NEVER set `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` in production.
7. IF exit 3 or 6 — MUST wait, rotate proxy, revalidate Chrome path and canal metadata. NEVER use Lite as remediation.
8. IF exit 4 — MUST raise `--global-timeout` or reduce `--num` / `--parallel` / fetch load.
9. IF exit 5 with `legitimo` or `vertical-sem-resultados` — MUST reformulate the query or adjust lang/time-filter/vertical.
10. IF exit 130 — MUST NOT treat as a search failure. Re-run only if the user still needs results.
11. After each invocation MUST assume the CLI reaped Chromium, Xvfb, and that session profile (ONE-SHOT contract).

## Execution Formulas — every flag
MUST copy and adapt. Every formula is imperative. ALWAYS keep `-q -f json` in agent pipelines.

- MANDATORY dual+fetch base — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- SERP-only faster path — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- Thin web-only — `timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content "QUERY" -q -f json`
- `-n` / `--num` (default 15, minimum 1) — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --num 30`
- `-f` / `--format` (`json|text|markdown|md|auto`; default auto) — agents ALWAYS force json — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `-o` / `--output` atomic write; FORBIDDEN `..` and system dirs — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --output /tmp/results.json`
- `-t` / `--timeout` per request default 15s — `timeout 180 duckduckgo-search-cli --timeout 20 "QUERY" -q -f json`
- `-l` / `--lang` default `pt` — `timeout 180 duckduckgo-search-cli --lang en-US "QUERY" -q -f json`
- `-c` / `--country` default `br` — `timeout 180 duckduckgo-search-cli --country us "QUERY" -q -f json`
- `--region` alias of `--country` — `timeout 180 duckduckgo-search-cli --region us "QUERY" -q -f json`
- `-p` / `--parallel` default 5 clamp 1..20 — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--queries-file` — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --num 15`
- `--pages` 1..5 default 1; auto-elevates when `--num > 10` — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --num 20 --pages 3`
- `--retries` 0..10 default 2 — `timeout 180 duckduckgo-search-cli --retries 3 "QUERY" -q -f json`
- `--endpoint` `html|lite` default html; production SERP is HTML Chrome; lite does NOT remediate blocks — `timeout 180 duckduckgo-search-cli --endpoint html "QUERY" -q -f json`
- `--vertical all` DEFAULT dual web+news — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `--vertical web` opt-out dual — `timeout 60 duckduckgo-search-cli --vertical web "QUERY" -q -f json`
- `--vertical news` news-only — `timeout 180 duckduckgo-search-cli --vertical news "QUERY" -q -f json`
- `--time-filter` `d|w|m|y` — `timeout 180 duckduckgo-search-cli --time-filter d "QUERY" -q -f json`
- `--safe-search` `off|moderate|on` default moderate — `timeout 180 duckduckgo-search-cli --safe-search off "QUERY" -q -f json`
- `--probe` — `timeout 15 duckduckgo-search-cli --probe -q -f json`
- `--probe-deep` — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json`
- `--pre-flight` auto-route via probe-deep on web — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- `--identity-profile` `auto|chrome-win|chrome-mac|chrome-linux|edge-win|firefox-linux|safari-mac` default auto — `timeout 180 duckduckgo-search-cli --identity-profile chrome-linux "QUERY" -q -f json`
- `--stream` FORBIDDEN unimplemented — NEVER use
- `-v` / `--verbose` and `-vv` (no flag INFO, `-v` DEBUG, `-vv` TRACE) — `timeout 180 duckduckgo-search-cli -v "QUERY" -f json 2>/tmp/ddg-debug.log`
- `-q` / `--quiet` MANDATORY in pipelines — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `--fetch-content` explicit redundant ON — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --fetch-content --max-content-length 5000`
- `--no-fetch-content` opt-out SERP-only — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- `--max-content-length` default 10000 range 1..100000 — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --max-content-length 5000`
- `--proxy` HTTP/HTTPS/SOCKS5/SOCKS5h — `timeout 180 duckduckgo-search-cli --proxy socks5://127.0.0.1:1080 "QUERY" -q -f json`
- `--no-proxy` mutually exclusive with `--proxy` — `timeout 180 duckduckgo-search-cli --no-proxy "QUERY" -q -f json`
- `--match-platform-ua` — `timeout 180 duckduckgo-search-cli --match-platform-ua "QUERY" -q -f json`
- `--per-host-limit` 1..10 default 2 — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- `--chrome-path` wrappers and Flatpak exports auto-resolve to real ELF — `timeout 180 duckduckgo-search-cli --chrome-path /usr/lib64/chromium-browser/chromium-browser "QUERY" -q -f json`
- `--no-color` — `timeout 180 duckduckgo-search-cli --no-color "QUERY" -q -f json`
- `--no-warmup` — `timeout 180 duckduckgo-search-cli --no-warmup "QUERY" -q -f json`
- `--no-cookie-persistence` — `timeout 180 duckduckgo-search-cli --no-cookie-persistence "QUERY" -q -f json`
- `--cookies-path` — `timeout 180 duckduckgo-search-cli --cookies-path /secure/cookies.json "QUERY" -q -f json`
- `--seed` — `timeout 180 duckduckgo-search-cli --seed 42 "QUERY" -q -f json`
- `--config` — `timeout 180 duckduckgo-search-cli --config ./config.toml "QUERY" -q -f json`
- `--allow-lite-fallback` legacy no-op NOT remediation — `timeout 180 duckduckgo-search-cli --allow-lite-fallback "QUERY" -q -f json`
- `--global-timeout` 1..3600 default 60 — `timeout 180 duckduckgo-search-cli --global-timeout 90 "QUERY" -q -f json`
- Positional multi-query — `timeout 120 duckduckgo-search-cli -q -f json "query one" "query two"`
- Stdin multi-query one query per line — `printf '%s\n' "q1" "q2" | timeout 120 duckduckgo-search-cli -q -f json`

## News Vertical
- DEFAULT search is dual `all` — web plus news without extra flags.
- MUST use `--vertical news` for news-only and `--vertical web` to skip news.
- news and all require Chrome. Without Chrome → exit 2 fail-closed.
- Content fetch applies to news cards when fetch is ON — `.noticias[].conteudo` for top URLs under FETCH_CAP=10.
- `--pre-flight` applies only to the web vertical. With `--vertical news` it is skipped.
- News-only SERP formula — `timeout 90 duckduckgo-search-cli --vertical news --no-fetch-content "QUERY" -q -f json`
- Dual SERP formula — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- Dual+fetch formula — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- News extract MUST use — `jaq -r '.noticias[] | [.posicao, .titulo, .url, (.fonte // ""), (.data_relativa // "")] | @tsv'`
- Dual extract MUST use — `jaq '{web:.resultados,news:.noticias,path:.metadados.chrome_path_resolvido,canal:.metadados.chrome_canal}'`

## Diagnosis
- `--probe` minimal Chrome health check — `timeout 15 duckduckgo-search-cli --probe -q -f json`
- `--probe-deep` CAPTCHA/interstitial detector — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json`
- `--pre-flight` auto-route via probe-deep — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- MUST inspect `.metadados.causa_zero` on EVERY zero-result response.
- MUST follow `.metadados.sugestao_proxima_acao` when present. It points to Chrome, proxy, or wait — NEVER Lite.
- ZeroCause seven CLI string values (keep as-is) — `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`, `vertical-sem-resultados`.
- `vertical-sem-resultados` → exit 5 (legitimate empty vertical). NOT exit 6.
- Other non-`legitimo` zeros → exit 6 by default (suspected block) unless `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`.
- Multi-query MUST inspect `.causa_zero_histogram` when present.
- Cascade `.metadados.nivel_cascata` is optional 0..4. If 4, MUST rotate proxy or wait 300s.
- MUST inspect multi-canal Chrome metadata — `.metadados.chrome_path_resolvido` and `.metadados.chrome_canal` (`manual|env|host|flatpak|snap`) plus `usou_chrome` / `tentou_chrome`.
- Probe status extract — `jaq '.status'`
- Zero diagnosis extract — `jaq '{causa:.metadados.causa_zero, acao:.metadados.sugestao_proxima_acao, n:.metadados.quantidade_resultados}'`

## Exit Codes
| Code | Meaning | MANDATORY action |
|------|---------|------------------|
| 0 | Success with results | Parse JSON with jaq and cite sources |
| 1 | Runtime network I/O or parse | Report stderr; retry with native `--retries` |
| 2 | Invalid config/args OR missing Chrome OR NO_CHROME=1 | Fix args; install Chrome; NEVER NO_CHROME=1 in production |
| 3 | Anti-bot soft-block | Wait 300s; set `--chrome-path`; set `--proxy`; NEVER Lite |
| 4 | Global timeout | Raise `--global-timeout`; reduce num parallel fetch load |
| 5 | Legitimate zero or vertical-sem-resultados | Reformulate query or adjust lang time-filter vertical |
| 6 | Suspected block non-legitimo ZeroCause | Read causa_zero and sugestao_proxima_acao; remediate Chrome/proxy |
| 130 | Cancelled SIGINT/SIGTERM path | Do NOT treat as search failure; re-run if still needed |

## Multi-query and Batch
- MUST use outer timeout 300 for batch jobs.
- `--queries-file` — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --num 15`
- `-p` / `--parallel` default 5 clamp 1..20; against anti-bot MUST keep at or below 5 — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--per-host-limit` default 2; with fetch ON MUST keep at or below 2 — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- Positional multi-query — `timeout 120 duckduckgo-search-cli -q -f json "query one" "query two"`
- Stdin multi-query — `printf '%s\n' "q1" "q2" | timeout 120 duckduckgo-search-cli -q -f json`
- Multi-query root is `.buscas[]`. NEVER confuse with single-query `.resultados[]`.
- Default fetch enriches web and news under FETCH_CAP=10. Multi-query metadata MUST include path and canal per `.buscas[].metadados`.
- `tamanho_conteudo` equals truncated post-extraction text size when content is present.

## Deep-research
- By default each sub-query runs vertical `all` (web+news) under Chrome.
- Without Chrome → exit 2 fail-closed. No auto `--no-news`.
- MUST use LLM-generated manual sub-queries with `--sub-query-strategy manual` and `--sub-queries-file`. NEVER rely only on heuristic expansion.
- Base formula — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY"`
- Quality MANDATORY manual — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--max-sub-queries` 1..12 default 5 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --max-sub-queries 5`
- `--sub-query-strategy` with `manual` plus `--sub-queries-file` is MANDATORY for quality research.
- `--sub-queries-file` one sub-query per line — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--aggregate` `rrf` default or `dedupe-by-url` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --aggregate rrf`
- `--depth` 0..3 default 0 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --depth 2`
- `--synthesize` plus `--budget-tokens` default 4000 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --budget-tokens 2000`
- `--synth-format` `markdown|plain-text|json` — value MUST be `plain-text` NEVER `plain` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --synth-format plain-text`
- `--require-results` non-zero exit if fan-out aggregates zero — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --require-results`
- `--no-news` intentional skip of news only with Chrome available and explicit intent — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-news`
- Content fetch under deep-research applies to web and news aggregate and is ON by default — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --max-content-length 5000`
- SERP-only deep opt-out — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-fetch-content`
- Global flags including chrome-path, proxy, vertical, fetch flags, num, format, output, timeout, lang, country, parallel, quiet, verbose, identity-profile, match-platform-ua, pre-flight, allow-lite-fallback, global-timeout are accepted BEFORE or AFTER `deep-research`.
- News RRF is SEPARATE from web RRF. NEVER compare scores between `.noticias[]` and `.resultados[]`.
- Deep-research exits 0 if web OR news produced results. Exit 5 only when BOTH are empty.
- Deep envelope fields — `.query`, `.sintese`, `.resultados[]`, `.noticias[]`, `.metadados.sub_queries[]`, `.quantidade_noticias`, `.metadados.total_noticias_unicas`, `.metadados.usou_chrome`, `.metadados.chrome_path_resolvido`, `.metadados.chrome_canal`.

## JSON Contract and Parsing
Portuguese field names stay as the CLI emits them. ALWAYS parse with `jaq`. NEVER `jq`.

- Capture exit first — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json | tee /tmp/out.json | jaq .; echo exit=${PIPESTATUS[0]}`
- TSV web — `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'`
- Top 5 links — `jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'`
- Citations — `jaq -r '.resultados[] | "- \(.titulo) — \(.url)"'`
- Zero diagnosis — `jaq '{causa:.metadados.causa_zero, acao:.metadados.sugestao_proxima_acao, n:.metadados.quantidade_resultados}'`
- GUARANTEED fields — `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`
- OPTIONAL fields — `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`, `.metadados.identidade_usada`, `.metadados.nivel_cascata`
- CONDITIONAL news — `.noticias[]`, `.quantidade_noticias`, `.metadados.vertical_usada` (present under default vertical all)
- CONDITIONAL fetch-content — `.resultados[].conteudo` and `.noticias[].conteudo` when fetch is ON
- Chrome metadata — `.metadados.usou_chrome`, `.metadados.tentou_chrome`, `.metadados.chrome_path_resolvido`, `.metadados.chrome_canal`
- Diagnosis fields — `.metadados.causa_zero`, `.metadados.sugestao_proxima_acao`
- Pre-flight fields — `.metadados.pre_flight_disparado`, `.metadados.endpoint_usado`
- Compat — `quantidade_resultados`, `endpoint_usado`, `nivel_cascata` exist at root AND under `.metadados`
- Identity format — `<family>-<platform>-<16hex>`
- Distinguish roots — single `.resultados[]` | multi `.buscas[]` | deep-research `.resultados[]` plus `.noticias[]`
- ALWAYS use `// ""` on optionals. ALWAYS use `jaq`. NEVER invent missing fields.

## Helpers
- `init-config` — `duckduckgo-search-cli init-config`
- `init-config --force` overwrite — `duckduckgo-search-cli init-config --force`
- `init-config --dry-run` simulate without writing — `duckduckgo-search-cli init-config --dry-run`
- `completions bash` — `duckduckgo-search-cli completions bash`
- `completions zsh` — `duckduckgo-search-cli completions zsh`
- `completions fish` — `duckduckgo-search-cli completions fish`
- `completions powershell` — `duckduckgo-search-cli completions powershell`
- `completions elvish` — `duckduckgo-search-cli completions elvish`
- Install MANDATORY — `cargo install duckduckgo-search-cli --locked --force`

## Environment
- Default cookie jar Unix — `~/.config/duckduckgo-search-cli/cookies.json` mode 0o600. Windows — `%APPDATA%\duckduckgo-search-cli\cookies.json`.
- NEVER log cookies. NEVER commit `cookies.json`.
- NEVER log `--proxy` credentials.
- MUST use `--no-cookie-persistence` for ephemeral sessions.
- `DUCKDUCKGO_CHROME_HEADLESS=1` force headless (anti-bot risk).
- `DUCKDUCKGO_CHROME_VISIBLE=1` visible headed for debug.
- `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` FORBIDDEN in production → exit 2 on any network op.
- `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` tests only with feature http-test-harness.
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` restores exit 5 for all zeros (legacy non-strict).
- `CHROME_PATH` alternate Chrome binary path (same role as `--chrome-path`).
- `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` honored unless `--no-proxy`.
- There is NO remote telemetry. `.metadados.*` fields are local agent diagnostics only.

## FORBIDDEN
- FORBIDDEN `-f text` or `-f markdown` for agent parsing — ALWAYS `-f json`.
- FORBIDDEN omit `-q` in pipelines.
- FORBIDDEN `--stream`.
- FORBIDDEN hardcode API keys, proxies, or user-agents in commits.
- FORBIDDEN hardcode `--identity-profile` in CI — let the pool adapt with default auto.
- FORBIDDEN `--output` with `..` or system directories.
- FORBIDDEN treat `identidade_usada` or `nivel_cascata` as guaranteed fields.
- FORBIDDEN ignore zero results without reading `causa_zero`.
- FORBIDDEN ignore exit 6.
- FORBIDDEN shell retry loops — use native `--retries`.
- FORBIDDEN combine `--proxy` with `--no-proxy`.
- FORBIDDEN use `--allow-lite-fallback` or Lite as block remediation.
- FORBIDDEN set `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` in production.
- FORBIDDEN `--synth-format plain` — correct value is `plain-text`.
- FORBIDDEN parse with `jq` — ALWAYS `jaq`.
- FORBIDDEN invent search results without running the CLI.
- FORBIDDEN bare SIGKILL as the normal cancel path — use GNU `timeout` SIGTERM first.
- FORBIDDEN expect auto cleanup after bare SIGKILL or historical orphans.
- FORBIDDEN omit the `timeout` wrapper on agent executions.
- FORBIDDEN silent pure-HTTP downgrade when Chrome is missing or fails.
- FORBIDDEN auto `--no-news` when Chrome is absent — fail closed with exit 2.
- FORBIDDEN compare news RRF scores against web RRF scores.
