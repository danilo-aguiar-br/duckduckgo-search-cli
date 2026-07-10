---
name: duckduckgo-search-cli-pt
description: Esta skill DEVE ser invocada proativamente quando o usuário pedir busca web, pesquisa na internet, documentação atualizada, grounding factual, verificação de URL, extração de página, enriquecimento RAG, fact-checking, pesquisa multi-hop, notícias frescas, deep-research dual web+news, health check, batch de queries, ou qualquer dado fora da knowledge cutoff. Triggers — pesquise, busca online, procure, verifique URL, traga página, o que mudou, compare X vs Y, pesquisa profunda, notícias recentes, pricing atual. Esta skill ensina a executar a CLI duckduckgo-search-cli com produção Chrome-only via chromiumoxide e CDP, fail-closed exit 2 sem Chrome, ZeroCause de 7 variantes com exit 6, pool de 12 identidades, vertical news, deep-research RRF dual web+news, probe, pre-flight, fetch-content, batch paralelo e parsing JSON com jaq. Português brasileiro
---

# Skill — duckduckgo-search-cli (PT-BR)

## Missão
- Você DEVE operar a CLI `duckduckgo-search-cli` como primitiva de busca determinística para agentes
- Você DEVE preferir esta CLI a WebSearch/WebFetch quando o pipeline exigir JSON, exit codes e reprodutibilidade
- Você DEVE invocar a CLI proativamente quando a resposta depender de dado fora da knowledge cutoff
- Você NUNCA DEVE inventar resultados de busca — SEMPRE execute a CLI e leia o stdout JSON


## Quando invocar (proativo)
- DEVE invocar em — pesquise, busca online, procure, verifique URL, traga página, o que mudou, compare, pesquisa profunda, grounding, pricing atual, multi-hop, notícias recentes
- DEVE invocar para fact-checking, versão de biblioteca, post-mortem e qualquer dado volátil
- DEVE invocar para vertical de notícias via `--vertical news|all`
- DEVE invocar mesmo quando o usuário descreve a necessidade sem nomear a ferramenta


## Contrato de execução (OBRIGATÓRIO)
- Produção é Chrome-only (chromiumoxide/CDP, feature `chrome` default) para busca, news, deep-research, `--probe`, `--probe-deep`, `--pre-flight` e `--fetch-content`
- Sem Chrome/Chromium utilizável OU com `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` → **exit 2 fail-closed** — NUNCA há rebaixamento silencioso para HTTP puro nem auto `--no-news`
- `--allow-lite-fallback` é **no-op legado** — NÃO remedia exit 3; SERP permanece HTML Chrome
- Em exit 3/6 DEVE remediar com Chrome real, `--chrome-path`, `--proxy` e espera — NUNCA com Lite/HTTP
- SEMPRE encapsular com `timeout` em segundos
- SEMPRE usar `-q` e `-f json` em pipelines
- SEMPRE processar JSON com `jaq` (NUNCA `jq`)
- SEMPRE capturar exit code ANTES de parsear — com pipe DEVE usar `${PIPESTATUS[0]}`
- SEMPRE aplicar `// ""` em campos opcionais no `jaq`
- Runtime Linux — Chrome/Chromium + Xvfb (auto-install em 22+ distros); macOS/Windows — Chrome/Chromium (headless=new)
- Campo `.metadados.usou_chrome` / `.metadados.tentou_chrome` indicam tentativa e sucesso Chrome


## Workflow — passos numerados
1. DEVE escolher o modo — busca simples, news, deep-research, batch, probe ou fetch-content
2. DEVE montar a fórmula com `timeout`, `-q`, `-f json` e flags do modo
3. DEVE executar e capturar exit code + stdout
4. SE exit 0 e `quantidade_resultados > 0` — DEVE extrair com `jaq` e citar fontes
5. SE `quantidade_resultados == 0` — DEVE ler `.metadados.causa_zero` e `.metadados.sugestao_proxima_acao`
6. SE exit 2 — DEVE instalar/fornecer Chrome ou corrigir args; NUNCA setar `NO_CHROME=1` em produção
7. SE exit 3 ou 6 — DEVE aguardar, rotacionar proxy e revalidar Chrome; NUNCA usar Lite como remediação
8. SE exit 4 — DEVE elevar `--global-timeout` ou reduzir `--num`/`--parallel`
9. SE exit 5 com `causa_zero == "legitimo"` ou `vertical-sem-resultados` — DEVE reformular a query


## Fórmulas de busca (flags globais)
DEVE copiar e adaptar. Cada fórmula é imperativa.

- Base OBRIGATÓRIA — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json`
- `--num` / `-n` (default 15, mínimo 1) — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json --num 30`
- `--lang` / `-l` (default `pt`) — `timeout 60 duckduckgo-search-cli --lang pt-BR "QUERY" -q -f json`
- `--country` / `-c` (default `br`; alias `--region`) — `timeout 60 duckduckgo-search-cli --country br "QUERY" -q -f json`
- `--time-filter` (`d|w|m|y`) — `timeout 60 duckduckgo-search-cli --time-filter d "QUERY" -q -f json`
- `--safe-search` (`off|moderate|on`, default moderate) — `timeout 60 duckduckgo-search-cli --safe-search off "QUERY" -q -f json`
- `--endpoint` (`html|lite`, default html) — em produção Chrome a SERP é HTML canônico; `--endpoint lite` NÃO remedia bloqueio e NÃO é caminho de sucesso — `timeout 60 duckduckgo-search-cli --endpoint html "QUERY" -q -f json`
- `--pages` (1..5, default 1; auto-eleva se `--num > 10`) — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json --num 20 --pages 3`
- `-f` / `--format` (`json|text|markdown|md|auto`; default `auto` = text em TTY e json em pipe; `--output` força json) — para agente SEMPRE `-f json`
- `-q` / `--quiet` — OBRIGATÓRIO em pipeline
- `-v` / `-vv` — níveis stderr corretos — `0`=INFO, `-v`=DEBUG, `-vv` ou mais=TRACE — `timeout 60 duckduckgo-search-cli -v "QUERY" -f json 2>/tmp/ddg-debug.log`
- `--no-color` — `timeout 60 duckduckgo-search-cli --no-color "QUERY" -q -f json`
- `--output` / `-o` — escrita atômica; PROIBIDO `..` e dirs de sistema — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json --output /tmp/resultados.json`
- `--retries` (0..10, default 2) — `timeout 60 duckduckgo-search-cli --retries 3 "QUERY" -q -f json`
- `--timeout` / `-t` (default 15s, por request) — `timeout 60 duckduckgo-search-cli --timeout 20 "QUERY" -q -f json`
- `--global-timeout` (1..3600, default 60) — `timeout 90 duckduckgo-search-cli --global-timeout 60 "QUERY" -q -f json`
- `--config` — `timeout 60 duckduckgo-search-cli --config ./config.toml "QUERY" -q -f json`
- `--seed` — reprodutibilidade — `timeout 60 duckduckgo-search-cli --seed 42 "QUERY" -q -f json`
- `--identity-profile` (`auto|chrome-win|chrome-mac|chrome-linux|edge-win|firefox-linux|safari-mac`) — `timeout 60 duckduckgo-search-cli --identity-profile chrome-linux "QUERY" -q -f json`
- `--match-platform-ua` — `timeout 60 duckduckgo-search-cli --match-platform-ua "QUERY" -q -f json`
- `--no-warmup` — `timeout 60 duckduckgo-search-cli --no-warmup "QUERY" -q -f json`
- `--no-cookie-persistence` — `timeout 60 duckduckgo-search-cli --no-cookie-persistence "QUERY" -q -f json`
- `--cookies-path` — `timeout 60 duckduckgo-search-cli --cookies-path /secure/cookies.json "QUERY" -q -f json`
- `--chrome-path` — `timeout 60 duckduckgo-search-cli --chrome-path /usr/bin/chromium "QUERY" -q -f json`
- `--proxy` (HTTP/HTTPS/SOCKS5/SOCKS5h) — `timeout 60 duckduckgo-search-cli --proxy socks5://127.0.0.1:1080 "QUERY" -q -f json`
- `--no-proxy` — mutuamente exclusivo com `--proxy` — `timeout 60 duckduckgo-search-cli --no-proxy "QUERY" -q -f json`
- `--allow-lite-fallback` — no-op; scripts legados não quebram; NÃO use como remediação
- `--stream` — PROIBIDO (placeholder sem implementação)
- stdin (uma query por linha) — `printf '%s\n' "q1" "q2" | timeout 120 duckduckgo-search-cli -q -f json`


## Vertical news
- DEVE usar `--vertical news` para só notícias e `--vertical all` para web+news
- news/all exigem Chrome; sem Chrome → exit 2 fail-closed
- `--fetch-content` NÃO se aplica a cards de news
- Fórmula news — `timeout 90 duckduckgo-search-cli --vertical news "QUERY" -q -f json | jaq '.noticias'`
- Fórmula all — `timeout 90 duckduckgo-search-cli --vertical all "QUERY" -q -f json | jaq '{web:.resultados,news:.noticias}'`
- Extração news — `jaq -r '.noticias[] | [.posicao, .titulo, .url, (.fonte // ""), (.data_relativa // "")] | @tsv'`


## Diagnóstico — probe, pre-flight, ZeroCause
- `--probe` — health check mínimo Chrome — `timeout 15 duckduckgo-search-cli --probe -q -f json | jaq '.status'`
- `--probe-deep` — detector CAPTCHA/interstitial — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'`
- `--pre-flight` — auto-rota via probe-deep — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- DEVE inspecionar `.metadados.causa_zero` em TODO zero
- 7 causas — `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`, `vertical-sem-resultados`
- `vertical-sem-resultados` → exit 5 (legítimo news vazio), NÃO exit 6
- `causa_zero != "legitimo"` → exit 6 por padrão (`SUSPECTED_BLOCK`)
- `.metadados.sugestao_proxima_acao` DEVE ser seguida quando presente (aponta Chrome/proxy/espera — NÃO Lite)
- Opt-out legado — `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` restaura exit 5 para todos os zeros
- Multi-query — `.causa_zero_histogram` agrega causas
- Cascata `.metadados.nivel_cascata` (0..4, Option) — se 4, DEVE rotacionar proxy ou aguardar 300s


## Exit codes
| Code | Significado | Ação OBRIGATÓRIA |
|------|-------------|------------------|
| 0 | Sucesso com resultados | Parsear JSON e citar |
| 1 | Runtime (rede/I/O/parse) | Relatar stderr; retentar com `--retries` |
| 2 | Config/args inválidos OU Chrome ausente/`NO_CHROME=1` | Corrigir args; instalar Chrome; NUNCA `NO_CHROME=1` em produção |
| 3 | Anti-bot soft-block | Aguardar 300s; `--chrome-path`; `--proxy`; NUNCA Lite |
| 4 | Timeout global | Elevar `--global-timeout`; reduzir carga |
| 5 | Zero legítimo | Reformular query/`--lang`/`--time-filter` |
| 6 | Bloqueio suspeito | Ler `causa_zero` + `sugestao_proxima_acao` |
| 130 | Cancelado (SIGINT) | NÃO tratar como falha de busca; reexecutar se necessário |


## Batch, conteúdo e parallel
- `--queries-file` — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --num 15`
- `--parallel` / `-p` (default 5, clamp 1..20; para anti-bot preferir ≤5) — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--per-host-limit` (1..10, default 2; com fetch-content preferir ≤2) — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- Multi-query positional — `timeout 120 duckduckgo-search-cli -q -f json "query um" "query dois"`
- Root multi-query — `.buscas[]` (NUNCA confunda com `.resultados[]` single)
- `--fetch-content` + `--max-content-length` (default 10000 **caracteres** de texto extraído, 1..100000) — `timeout 120 duckduckgo-search-cli "QUERY" -q -f json --fetch-content --max-content-length 5000`
- Também válido em deep-research — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --fetch-content --max-content-length 5000`
- `tamanho_conteudo` = tamanho do texto truncado pós-extração


## Deep-research dual web+news
- Por padrão cada sub-query roda vertical `all` (web+news) em Chrome
- Sem Chrome → exit 2 fail-closed (sem auto `--no-news`)
- DEVE preferir sub-queries manuais geradas pela LLM — NUNCA confiar só em `heuristic`
- Fórmula base — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY"`
- Manual OBRIGATÓRIO de qualidade — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--max-sub-queries` (1..12, default 5) — `... deep-research "QUERY" --max-sub-queries 5`
- `--aggregate` (`rrf` default | `dedupe-by-url`) — `... --aggregate rrf`
- `--synthesize` + `--budget-tokens` (default 4000) — `... --synthesize --budget-tokens 2000`
- `--synth-format` (`markdown|plain-text|json`) — valor correto é `plain-text`, NUNCA `plain`
- `--require-results` — exit não-zero se fan-out zerar — `... --require-results`
- `--depth` (0..3, default 0) — `... --depth 2`
- `--no-news` — pula news intencionalmente; use SÓ com Chrome disponível e intenção explícita
- Flags globais `-n -f -o -t -l -c -p -q -v` aceitas ANTES ou DEPOIS de `deep-research`
- RRF news é SEPARADO do RRF web — NUNCA compare scores entre `.noticias[]` e `.resultados[]`
- Exit deep-research — 0 se web OU news tiverem resultados; 5 só se AMBOS vazios
- Envelope — `.query`, `.sintese`, `.resultados[]`, `.noticias[]`, `.metadados.sub_queries[]`, `.quantidade_noticias`, `.metadados.total_noticias_unicas`


## Parsing e contrato JSON
- TSV — `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'`
- Top 5 markdown — `jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'`
- Citações — `jaq -r '.resultados[] | "- \(.titulo) — \(.url)"'`
- GARANTIDOS — `.query`, `.resultados[].posicao|.titulo|.url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`
- OPCIONAIS — `.resultados[].snippet|.url_exibicao|.titulo_original`, `.metadados.identidade_usada`, `.metadados.nivel_cascata`
- CONDICIONAIS news — `.noticias[]`, `.quantidade_noticias`, `.metadados.vertical_usada`
- CONDICIONAIS fetch-content — `.resultados[].conteudo|.tamanho_conteudo|.metodo_extracao_conteudo`
- Chrome — `.metadados.usou_chrome`, `.metadados.tentou_chrome`
- Diagnóstico — `.metadados.causa_zero`, `.metadados.sugestao_proxima_acao`
- Pre-flight — `.metadados.pre_flight_disparado`, `.metadados.endpoint_usado`
- Compat — `quantidade_resultados`, `endpoint_usado`, `nivel_cascata` existem em root E em `.metadados`
- Identidade — formato `<family>-<platform>-<16hex>`
- Distinguir roots — single `.resultados[]` | multi `.buscas[]` | deep-research `.resultados[]`+`.noticias[]`


## Subcomandos auxiliares
- `init-config` — `duckduckgo-search-cli init-config`
- `init-config --force` — sobrescreve
- `init-config --dry-run` — simula sem gravar
- `completions` — `duckduckgo-search-cli completions bash` (também zsh, fish, powershell, elvish)
- Instalação — `cargo install duckduckgo-search-cli --locked --force`


## Segurança e ambiente
- Cookie jar default Unix — `~/.config/duckduckgo-search-cli/cookies.json` (0o600); Windows — `%APPDATA%\duckduckgo-search-cli\cookies.json`
- NUNCA logar cookies; NUNCA commitar `cookies.json`
- NUNCA logar credenciais de `--proxy`
- DEVE usar `--no-cookie-persistence` em sessões efêmeras
- `DUCKDUCKGO_CHROME_HEADLESS=1` — força headless (risco anti-bot)
- `DUCKDUCKGO_CHROME_VISIBLE=1` — headed visível (debug)
- `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` — **PROIBIDO em produção** → exit 2 em qualquer op de rede
- `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` — só testes com feature `http-test-harness`
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` — exit 5 legado em zeros
- `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` — respeitados salvo `--no-proxy`


## Regras PROIBIDAS
- PROIBIDO `-f text` ou `-f markdown` para parsing de agente — SEMPRE `-f json`
- PROIBIDO omitir `-q` em pipelines
- PROIBIDO `--stream`
- PROIBIDO hardcodar API keys, proxies ou UAs em commits
- PROIBIDO hardcodar `--identity-profile` em CI — deixe o pool adaptar
- PROIBIDO `--output` com `..` ou diretórios de sistema
- PROIBIDO tratar `identidade_usada` ou `nivel_cascata` como garantidos
- PROIBIDO ignorar zero de resultados sem ler `causa_zero`
- PROIBIDO ignorar exit 6
- PROIBIDO loops de retry em shell — use `--retries` nativo
- PROIBIDO combinar `--proxy` com `--no-proxy`
- PROIBIDO usar `--allow-lite-fallback` ou Lite como remediação de bloqueio
- PROIBIDO setar `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` em produção
- PROIBIDO `--synth-format plain` — o valor correto é `plain-text`
