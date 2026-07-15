---
name: duckduckgo-search-cli-pt
description: Esta skill DEVE ser usada quando o usuário pedir busca web, pesquisa na internet, docs atualizadas, grounding factual, verificação de URL, extração de página, enriquecimento RAG, fact-checking, pesquisa multi-hop, notícias frescas, deep-research dual web e news, health check, batch de queries ou dado fora da knowledge cutoff. Esta skill DEVE ensinar a executar duckduckgo-search-cli com Chrome-only via chromiumoxide e CDP, fail-closed exit 2 sem Chrome, default dual vertical all com fetch LIGADO e opt-out --no-fetch-content, ZeroCause com exit 6, probe e pre-flight, batch paralelo, lifecycle one-shot Chromium Xvfb e higiene de disco ddg-chrome, timeout SIGTERM-first, parsing JSON com jaq, exits 0 a 6 e 130, metadados multi-canal Chrome, pool de identidades, proxy, init-config e completions. Triggers incluem pesquise, busca online, procure, verifique URL, traga página, o que mudou, compare X vs Y, pesquisa profunda, notícias recentes e pricing atual. NUNCA invente resultados. SEMPRE invoque proativamente.
---

# Skill — duckduckgo-search-cli (PT-BR)

## Missão

- Você DEVE operar `duckduckgo-search-cli` como primitiva de busca determinística para agentes
- Você DEVE usar esta CLI em vez de WebSearch/WebFetch quando o pipeline exigir JSON, exit codes e reprodutibilidade
- Você DEVE invocar proativamente quando a resposta depender de dado fora da knowledge cutoff
- Você NUNCA DEVE inventar resultados — SEMPRE execute a CLI e leia o stdout JSON
- Você DEVE instalar ou atualizar com `cargo install duckduckgo-search-cli --locked --force`
- NÃO existe telemetria remota — metadados agent são diagnósticos locais

## Quando invocar

- DEVE invocar em — pesquise, busca online, procure, verifique URL, traga página, o que mudou, compare X vs Y, pesquisa profunda, grounding, pricing atual, multi-hop, notícias recentes
- DEVE invocar para fact-checking, versão de biblioteca, post-mortem, RAG, documentação atualizada e qualquer dado volátil
- DEVE invocar para `--vertical news|all`, deep-research dual, batch, probe, pre-flight e fetch-content
- DEVE invocar mesmo quando o usuário descreve a necessidade sem nomear a ferramenta
- NÃO DEVE invocar para tarefas puramente locais sem fontes vivas

## Contrato — Chrome-only ONE-SHOT

- Produção é Chrome-only via chromiumoxide/CDP para busca, news, deep-research, `--probe`, `--probe-deep`, `--pre-flight` e `--fetch-content`
- Sem Chrome/Chromium utilizável OU `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` → exit 2 fail-closed
- NUNCA rebaixamento silencioso para HTTP puro; NUNCA auto `--no-news`
- `--allow-lite-fallback` é no-op legado — NÃO remedia exit 3 nem exit 6; SERP permanece HTML Chrome
- Em exit 3 ou 6 DEVE remediar com Chrome real, `--chrome-path`, `--proxy` e espera — NUNCA Lite/HTTP
- ONE-SHOT (processo + disco) — cada invocação dona da árvore Chromium + Xvfb (Linux) + perfil `ddg-chrome-*` (NÃO `.tmp` genérico); reap em sucesso/erro/timeout/SIGINT/SIGTERM; a próxima run varre SÓ `ddg-chrome-*` stale de propriedade da CLI
- PROIBIDO bulk-rm de `.tmp*` estrangeiro e de `org.chromium.Chromium.*` — a auditoria de residual DEVE inspecionar exclusivamente `ddg-chrome-*` no temp do sistema
- Auditoria residual OBRIGATÓRIA pós-incidente — `find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'ddg-chrome-*' 2>/dev/null`; NUNCA use globs `.tmp*` nem `org.chromium.Chromium.*` para limpeza
- SEMPRE encapsular com GNU `timeout` (SIGTERM primeiro) — dual+fetch DEFAULT exige `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- NÃO DEVE esperar limpeza de órfãos históricos nem após SIGKILL nu da CLI
- SEMPRE `-q` e `-f json` em pipelines de agente; SEMPRE parsear com `jaq` — NUNCA `jq`
- SEMPRE capturar exit ANTES de parsear — com pipe DEVE usar `${PIPESTATUS[0]}`; SEMPRE `// ""` em opcionais
- `--stream` é PROIBIDO em pipelines de agente — NÃO é stream confiável de eventos SERP; NUNCA use `--stream`
- Runtime Linux — Chrome/Chromium + Xvfb; macOS/Windows — Chrome/Chromium (headless=new)
- Metadados agent (NÃO telemetria) — DEVE ler `.metadados.chrome_path_resolvido` e `.metadados.chrome_canal` (`manual|env|host|flatpak|snap`) em single, multi (`.buscas[].metadados`), deep e envelopes de falha; também `.metadados.usou_chrome` e `.metadados.tentou_chrome`
- DEFAULT vertical `all` (web+news dual); DEFAULT fetch LIGADO web+news (FETCH_CAP=10); opt-out `--no-fetch-content`; `--fetch-content` é ON explícito redundante; `--max-content-length` default 10000 (1..100000)
- Paths Linux — Fedora ELF host `/usr/lib64/chromium-browser/chromium-browser` (wrapper `/usr/bin/chromium-browser` resolve sozinho); Flatpak export `/var/lib/flatpak/exports/bin/com.google.Chrome` resolve para ELF `.../files/extra/chrome`
- Flags globais (`chrome-path`, `proxy`, `no-proxy`, `vertical`, fetch, `num`, `format`, `output`, `timeout`, `lang`, `country`, `parallel`, `quiet`, `verbose`, `identity-profile`, `match-platform-ua`, `pre-flight`, `allow-lite-fallback`, `global-timeout`) aceitas ANTES ou DEPOIS de `deep-research`
- Timeouts externos OBRIGATÓRIOS — dual+fetch → 180; SERP-only `--no-fetch-content` → 90; web thin → 60; deep-research → 180; batch → 300; probe → 15; probe-deep → 20
- Defaults globais — num 15; format auto (agentes forçam json); timeout 15s; lang pt; country br; parallel 5 clamp 1..20; pages 1; retries 2; endpoint html; vertical all; safe-search moderate; identity-profile auto; max-content-length 10000; per-host-limit 2; global-timeout 180; max-sub-queries 5; aggregate rrf; depth 0; budget-tokens 4000
- `--output` é atômico; PROIBIDO caminhos com `..` ou dirs de sistema
- Sem telemetria remota

## Workflow

1. DEVE escolher o modo — busca simples, news, deep-research, batch, probe, pre-flight ou SERP-only
2. DEVE montar a fórmula com `timeout` SIGTERM-first, `-q`, `-f json` e flags do modo
3. DEVE executar e capturar exit code + stdout (`${PIPESTATUS[0]}` ao pipar para `jaq`)
4. SE exit 0 e quantidade maior que zero — DEVE extrair com `jaq` e citar fontes
5. SE quantidade zero — DEVE ler `.metadados.causa_zero` e `.metadados.sugestao_proxima_acao`
6. SE exit 2 — DEVE instalar Chrome ou corrigir args; NUNCA `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` em produção
7. SE exit 3 ou 6 — DEVE aguardar, rotacionar proxy e revalidar Chrome; NUNCA Lite
8. SE exit 4 — DEVE elevar `--global-timeout` ou reduzir `--num`/`--parallel`/fetch
9. SE exit 5 com `legitimo` ou `vertical-sem-resultados` — DEVE reformular a query
10. SE exit 130 — NÃO DEVE tratar como falha de busca; reexecutar se necessário
11. Após cada invocação DEVE assumir reap de Chromium, Xvfb e perfil `ddg-chrome-*` (ONE-SHOT processo+disco)

## Fórmulas de execução — todas as flags

DEVE copiar e adaptar. SEMPRE mantenha `-q -f json` em pipelines de agente.

- Base dual+fetch OBRIGATÓRIA — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- SERP-only mais rápido — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- Web thin — `timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content "QUERY" -q -f json`
- `-n`/`--num` (default 15, mín 1) — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --num 30`
- `-f`/`--format` (`json|text|markdown|md|auto`, default auto) — agente SEMPRE `-f json` — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `-o`/`--output` (atômico; PROIBIDO `..` e dirs de sistema) — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --output /tmp/resultados.json`
- `-t`/`--timeout` (default 15s/request) — `timeout 180 duckduckgo-search-cli --timeout 20 "QUERY" -q -f json`
- `-l`/`--lang` (default `pt`) — `timeout 180 duckduckgo-search-cli --lang pt-BR "QUERY" -q -f json`
- `-c`/`--country` (default `br`) — `timeout 180 duckduckgo-search-cli --country br "QUERY" -q -f json`
- `--region` (alias de `--country`) — `timeout 180 duckduckgo-search-cli --region br "QUERY" -q -f json`
- `-p`/`--parallel` (default 5, clamp 1..20; anti-bot DEVE ≤5) — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--queries-file` — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --num 15`
- `--pages` (1..5, default 1; auto-eleva se `--num` > 10) — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --num 20 --pages 3`
- `--retries` (0..10, default 2) — `timeout 180 duckduckgo-search-cli --retries 3 "QUERY" -q -f json`
- `--endpoint` (`html|lite`, default html) — SERP produção é HTML Chrome; lite NÃO remedia — `timeout 180 duckduckgo-search-cli --endpoint html "QUERY" -q -f json`
- `--vertical all` (DEFAULT dual) — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `--vertical web` — `timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content "QUERY" -q -f json`
- `--vertical news` — `timeout 180 duckduckgo-search-cli --vertical news "QUERY" -q -f json`
- `--time-filter` (`d|w|m|y`) — `timeout 180 duckduckgo-search-cli --time-filter d "QUERY" -q -f json`
- `--safe-search` (`off|moderate|on`, default moderate) — `timeout 180 duckduckgo-search-cli --safe-search off "QUERY" -q -f json`
- `--probe` — `timeout 15 duckduckgo-search-cli --probe -q -f json`
- `--probe-deep` — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json`
- `--pre-flight` (auto-rota via probe-deep na vertical web) — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- `--identity-profile` (`auto|chrome-win|chrome-mac|chrome-linux|edge-win|firefox-linux|safari-mac`, default auto) — `timeout 180 duckduckgo-search-cli --identity-profile chrome-linux "QUERY" -q -f json`
- `--stream` — PROIBIDO em pipelines de agente (NÃO é stream confiável de eventos SERP); NUNCA use
- `-v`/`--verbose` e `-vv` (0 INFO, `-v` DEBUG, `-vv` TRACE) — `timeout 180 duckduckgo-search-cli -v "QUERY" -f json 2>/tmp/ddg-debug.log`
- `-q`/`--quiet` — OBRIGATÓRIO em pipeline — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `--fetch-content` (ON explícito redundante) — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --fetch-content --max-content-length 5000`
- `--no-fetch-content` (SERP-only) — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- `--max-content-length` (default 10000, 1..100000) — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --max-content-length 5000`
- `--proxy` (HTTP/HTTPS/SOCKS5/SOCKS5h) — `timeout 180 duckduckgo-search-cli --proxy socks5://127.0.0.1:1080 "QUERY" -q -f json`
- `--no-proxy` (exclusivo com `--proxy`) — `timeout 180 duckduckgo-search-cli --no-proxy "QUERY" -q -f json`
- `--match-platform-ua` — `timeout 180 duckduckgo-search-cli --match-platform-ua "QUERY" -q -f json`
- `--per-host-limit` (1..10, default 2; com fetch DEVE ≤2) — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- `--chrome-path` (global; wrappers e Flatpak resolvem para ELF real; OK após `deep-research`) — `timeout 180 duckduckgo-search-cli --chrome-path /usr/lib64/chromium-browser/chromium-browser "QUERY" -q -f json`
- `--no-color` — `timeout 180 duckduckgo-search-cli --no-color "QUERY" -q -f json`
- `--no-warmup` — `timeout 180 duckduckgo-search-cli --no-warmup "QUERY" -q -f json`
- `--no-cookie-persistence` — `timeout 180 duckduckgo-search-cli --no-cookie-persistence "QUERY" -q -f json`
- `--cookies-path` — `timeout 180 duckduckgo-search-cli --cookies-path /secure/cookies.json "QUERY" -q -f json`
- `--seed` — `timeout 180 duckduckgo-search-cli --seed 42 "QUERY" -q -f json`
- `--config` — `timeout 180 duckduckgo-search-cli --config ./config.toml "QUERY" -q -f json`
- `--allow-lite-fallback` — no-op; NÃO remedia — `timeout 180 duckduckgo-search-cli --allow-lite-fallback "QUERY" -q -f json`
- `--global-timeout` (1..3600, default 180) — `timeout 200 duckduckgo-search-cli "QUERY" -q -f json --global-timeout 180`
- stdin multi-query — `printf '%s\n' "q1" "q2" | timeout 120 duckduckgo-search-cli -q -f json`
- multi-query posicional — `timeout 120 duckduckgo-search-cli -q -f json "query um" "query dois"`

## Vertical news

- Default dual `all` — web + news sem flags extras
- DEVE usar `--vertical news` só notícias; `--vertical web` para pular news
- news e all exigem Chrome; sem Chrome → exit 2 fail-closed
- Fetch SE aplica a cards news — `.noticias[].conteudo` com fetch ligado (teto FETCH_CAP=10)
- `--pre-flight` só na vertical web; com `--vertical news` é pulado
- News SERP — `timeout 90 duckduckgo-search-cli --vertical news --no-fetch-content "QUERY" -q -f json | jaq '.noticias'`
- All SERP — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json | jaq '{web:.resultados,news:.noticias,path:.metadados.chrome_path_resolvido,canal:.metadados.chrome_canal}'`
- Dual+fetch — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- News com fetch — `timeout 180 duckduckgo-search-cli --vertical news "QUERY" -q -f json --max-content-length 5000`
- Extração news — `jaq -r '.noticias[] | [.posicao, .titulo, .url, (.fonte // ""), (.data_relativa // "")] | @tsv'`
- DEVE distinguir `.resultados[]` (web) de `.noticias[]` (news)

## Diagnóstico

- `--probe` — `timeout 15 duckduckgo-search-cli --probe -q -f json | jaq '.status'`
- `--probe-deep` — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'`
- `--pre-flight` — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- DEVE inspecionar `.metadados.causa_zero` em TODO zero
- ZeroCause 7 valores — `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`, `vertical-sem-resultados`
- `vertical-sem-resultados` → exit 5; demais não-`legitimo` → exit 6 (salvo `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`)
- DEVE seguir `.metadados.sugestao_proxima_acao` quando presente (Chrome/proxy/espera — NUNCA Lite)
- Multi-query — `.causa_zero_histogram` agrega causas
- Cascata `.metadados.nivel_cascata` (0..4, opcional) — se 4, DEVE rotacionar proxy ou aguardar 300s
- Remediação exit 3/6 — `timeout 180 duckduckgo-search-cli --chrome-path /usr/bin/chromium --proxy socks5://127.0.0.1:1080 "QUERY" -q -f json`
- Chrome em falha — `jaq '{path:.metadados.chrome_path_resolvido,canal:.metadados.chrome_canal,causa:.metadados.causa_zero}'`
- Residual disco — `find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'ddg-chrome-*' 2>/dev/null`

## Exit codes

| Code | Significado | Ação OBRIGATÓRIA |
|------|-------------|------------------|
| 0 | Sucesso com resultados | Parsear JSON com jaq e citar |
| 1 | Runtime (rede, I/O, parse) | Relatar stderr; retentar com `--retries` |
| 2 | Config/args inválidos OU Chrome ausente/NO_CHROME=1 | Corrigir args; instalar Chrome; NUNCA NO_CHROME em produção |
| 3 | Anti-bot soft-block | Aguardar 300s; `--chrome-path`; `--proxy`; NUNCA Lite |
| 4 | Timeout global | Elevar `--global-timeout`; reduzir carga |
| 5 | Zero legítimo ou `vertical-sem-resultados` | Reformular query, `--lang` ou `--time-filter` |
| 6 | Bloqueio suspeito (ZeroCause) | Ler `causa_zero` e `sugestao_proxima_acao` |
| 130 | Cancelado (SIGINT/SIGTERM) | NÃO tratar como falha de busca; reexecutar se necessário |

## Multi-query e batch

- DEVE usar timeout externo 300 em batch
- Batch — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --num 15`
- Anti-bot — parallel ≤5; com fetch `--per-host-limit` ≤2 — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- Posicional — `timeout 120 duckduckgo-search-cli -q -f json "query um" "query dois"`
- Stdin — `printf '%s\n' "q1" "q2" | timeout 120 duckduckgo-search-cli -q -f json`
- Root multi — `.buscas[]` — NUNCA confunda com single `.resultados[]`
- Extração multi — `jaq -r '.buscas[] | .query as $q | .resultados[0] | "\($q)\t\(.titulo)\t\(.url)"'`
- Fetch default enriquece web+news (FETCH_CAP=10); cada `.buscas[].metadados` DEVE expor path/canal
- Batch com fetch — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --max-content-length 5000`
- Campos condicionais — `.resultados[].conteudo`, `.noticias[].conteudo`, `.tamanho_conteudo`, `.metodo_extracao_conteudo` (`tamanho_conteudo` = texto truncado pós-extração)

## Deep-research

- Default dual `all` (web+news) sob Chrome; sem Chrome → exit 2 fail-closed (sem auto `--no-news`)
- DEVE gerar sub-queries manuais da LLM com `--sub-query-strategy manual` e `--sub-queries-file` — NUNCA confiar só em heuristic
- Base — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY"`
- Manual OBRIGATÓRIO — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--max-sub-queries` (1..12, default 5) — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --max-sub-queries 5`
- `--sub-query-strategy` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--sub-queries-file` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-queries-file /tmp/sq.txt`
- `--aggregate` (`rrf` default | `dedupe-by-url`) — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --aggregate rrf`
- `--depth` (0..3, default 0) — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --depth 2`
- Fetch default LIGADO em web e news agregados — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --max-content-length 5000`
- SERP-only deep — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-fetch-content`
- Metadados — `.metadados.usou_chrome`, `.metadados.chrome_path_resolvido`, `.metadados.chrome_canal`
- `--synthesize` + `--budget-tokens` (default 4000) — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --budget-tokens 2000`
- `--synth-format` (`markdown|plain-text|json`) — NUNCA `plain` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --synth-format plain-text`
- `--synth-format markdown` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --synth-format markdown`
- `--require-results` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --require-results`
- `--no-news` SÓ com intenção explícita e Chrome — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-news`
- Flags globais aceitas ANTES ou DEPOIS de `deep-research`
- RRF news SEPARADO do RRF web — NUNCA compare scores entre `.noticias[]` e `.resultados[]`
- Exit — 0 se web OU news tiverem resultados; 5 só se AMBOS vazios
- Envelope — `.query`, `.sintese`, `.resultados[]`, `.noticias[]`, `.metadados.sub_queries[]`, `.quantidade_noticias`, `.metadados.total_noticias_unicas`

## Contrato JSON e parsing

- SEMPRE `jaq` — NUNCA `jq`; SEMPRE `${PIPESTATUS[0]}` após pipe; SEMPRE `// ""` em opcionais
- TSV web — `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'`
- Top 5 — `jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'`
- Citações — `jaq -r '.resultados[] | "- \(.titulo) — \(.url)"'`
- Exit sem perder código — `out=$(timeout 180 duckduckgo-search-cli "QUERY" -q -f json); ec=$?; echo "$out" | jaq '.metadados.quantidade_resultados'; exit $ec`
- Zero — `jaq '{causa:.metadados.causa_zero,acao:.metadados.sugestao_proxima_acao,n:.metadados.quantidade_resultados}'`
- GARANTIDOS — `.query`, `.resultados[].posicao|.titulo|.url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`
- OPCIONAIS — `.resultados[].snippet|.url_exibicao|.titulo_original`, `.metadados.identidade_usada`, `.metadados.nivel_cascata`
- CONDICIONAIS news — `.noticias[]`, `.quantidade_noticias`, `.metadados.vertical_usada`
- CONDICIONAIS fetch — `.resultados[].conteudo`, `.noticias[].conteudo`
- Chrome — `.metadados.usou_chrome`, `.metadados.tentou_chrome`, `.metadados.chrome_path_resolvido`, `.metadados.chrome_canal`
- Diagnóstico — `.metadados.causa_zero`, `.metadados.sugestao_proxima_acao`
- Pre-flight — `.metadados.pre_flight_disparado`, `.metadados.endpoint_usado`
- Compat — `quantidade_resultados`, `endpoint_usado`, `nivel_cascata` em root E em `.metadados`
- Identidade — family-platform-hex16
- Roots — single `.resultados[]` | multi `.buscas[]` | deep `.resultados[]`+`.noticias[]`
- NUNCA invente campos ausentes; NUNCA trate opcionais como garantidos

## Subcomandos auxiliares

- Instalação — `cargo install duckduckgo-search-cli --locked --force`
- `init-config` — `duckduckgo-search-cli init-config`
- `init-config --force` — `duckduckgo-search-cli init-config --force`
- `init-config --dry-run` — `duckduckgo-search-cli init-config --dry-run`
- `completions bash` — `duckduckgo-search-cli completions bash`
- `completions zsh` — `duckduckgo-search-cli completions zsh`
- `completions fish` — `duckduckgo-search-cli completions fish`
- `completions powershell` — `duckduckgo-search-cli completions powershell`
- `completions elvish` — `duckduckgo-search-cli completions elvish`

## Ambiente

- Cookie jar Unix — `~/.config/duckduckgo-search-cli/cookies.json` (0o600); Windows — `%APPDATA%\duckduckgo-search-cli\cookies.json`
- NUNCA logar cookies nem credenciais de `--proxy`; NUNCA commitar `cookies.json`
- DEVE usar `--no-cookie-persistence` em sessões efêmeras
- `DUCKDUCKGO_CHROME_HEADLESS=1` — força headless (risco anti-bot)
- `DUCKDUCKGO_CHROME_VISIBLE=1` — headed visível (debug)
- `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` — PROIBIDO em produção → exit 2
- `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` — só testes com feature http-test-harness
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` — exit 5 legado em todos os zeros
- `CHROME_PATH` — binário Chrome/Chromium alternativo
- `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` — respeitados salvo `--no-proxy`
- Cancelamento cooperativo — GNU `timeout` (SIGTERM); NÃO SIGKILL como caminho normal
- Sem telemetria remota; metadados Chrome são agent-local

## PROIBIDO

- PROIBIDO `-f text` ou `-f markdown` para parsing de agente — SEMPRE `-f json`
- PROIBIDO omitir `-q` em pipelines
- PROIBIDO `--stream`
- PROIBIDO hardcodar API keys, proxies ou UAs em commits
- PROIBIDO hardcodar `--identity-profile` em CI — DEVE deixar o pool no default `auto`
- PROIBIDO `--output` com `..` ou diretórios de sistema
- PROIBIDO tratar `identidade_usada` ou `nivel_cascata` como garantidos
- PROIBIDO ignorar zero sem ler `causa_zero`
- PROIBIDO ignorar exit 6
- PROIBIDO loops de retry em shell — use `--retries` nativo
- PROIBIDO combinar `--proxy` com `--no-proxy`
- PROIBIDO usar `--allow-lite-fallback` ou Lite como remediação de exit 3/6
- PROIBIDO setar `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` em produção
- PROIBIDO `--synth-format plain` — correto é `plain-text`
- PROIBIDO inventar resultados sem executar a CLI
- PROIBIDO parsear com `jq` — SEMPRE `jaq`
- PROIBIDO SIGKILL nu como caminho normal de cancelamento — SEMPRE GNU `timeout` SIGTERM-first
- PROIBIDO esperar cleanup de órfãos após SIGKILL nu ou de órfãos históricos
- PROIBIDO omitir wrapper `timeout` em execuções de agente
- PROIBIDO rebaixar para HTTP puro quando Chrome falhar ou estiver ausente
- PROIBIDO auto `--no-news` ou esconder ausência de Chrome — fail-closed exit 2
- PROIBIDO comparar scores RRF de news com scores RRF de web
- PROIBIDO bulk-rm de `.tmp*` estrangeiro ou de `org.chromium.Chromium.*` — audite e limpe SÓ `ddg-chrome-*` com `find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'ddg-chrome-*' 2>/dev/null`
