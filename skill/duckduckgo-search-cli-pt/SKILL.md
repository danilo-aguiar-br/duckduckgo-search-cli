---
name: duckduckgo-search-cli-pt
description: Esta skill DEVE ser usada quando o usuário pedir busca web, pesquisa na internet, documentação atualizada, grounding factual, verificação de URL, extração de página, enriquecimento RAG, fact-checking, pesquisa multi-hop, notícias frescas, deep-research dual web e news, health check, batch de queries ou dado fora da knowledge cutoff. Triggers incluem pesquise, busca online, procure, verifique URL, traga página, o que mudou, compare X vs Y, pesquisa profunda, notícias recentes e pricing atual. Esta skill DEVE ensinar a executar a CLI duckduckgo-search-cli com produção Chrome-only via chromiumoxide e CDP, fail-closed exit 2 sem Chrome, ZeroCause com exit 6, probe e pre-flight, fetch-content, batch paralelo, lifecycle one-shot Chromium e Xvfb com reap, timeout SIGTERM-first, parsing JSON com jaq, códigos de saída 0 a 6 e 130, pool de identidades, proxy, init-config e completions. NUNCA invente resultados. SEMPRE invoque proativamente. OBRIGATÓRIO preferir esta CLI para pipelines determinísticos.
---

# Skill — duckduckgo-search-cli (PT-BR)

## Missão

- Você DEVE operar a CLI `duckduckgo-search-cli` como primitiva de busca determinística para agentes
- Você DEVE preferir esta CLI a WebSearch/WebFetch quando o pipeline exigir JSON, exit codes e reprodutibilidade
- Você DEVE invocar a CLI proativamente quando a resposta depender de dado fora da knowledge cutoff
- Você NUNCA DEVE inventar resultados de busca — SEMPRE execute a CLI e leia o stdout JSON
- Você DEVE instalar ou atualizar com `cargo install duckduckgo-search-cli --locked --force`

## Quando invocar

- DEVE invocar em — pesquise, busca online, procure, verifique URL, traga página, o que mudou, compare, pesquisa profunda, grounding, pricing atual, multi-hop, notícias recentes
- DEVE invocar para fact-checking, versão de biblioteca, post-mortem e qualquer dado volátil
- DEVE invocar para vertical de notícias via `--vertical news` ou `--vertical all`
- DEVE invocar para deep-research dual web e news, batch paralelo, probe, pre-flight e fetch-content
- DEVE invocar mesmo quando o usuário descreve a necessidade sem nomear a ferramenta
- NÃO DEVE invocar para tarefas puramente locais (reescrever código, formatar JSON local, hello world sem fontes)

## Contrato de execução e lifecycle ONE-SHOT

- Produção é Chrome-only via chromiumoxide/CDP para busca, news, deep-research, `--probe`, `--probe-deep`, `--pre-flight` e `--fetch-content`
- Sem Chrome ou Chromium utilizável OU com `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` → exit 2 fail-closed
- NUNCA há rebaixamento silencioso para HTTP puro
- NUNCA há auto `--no-news`
- `--allow-lite-fallback` é no-op legado — NÃO remedia exit 3 nem exit 6; SERP permanece HTML Chrome
- Em exit 3 ou 6 DEVE remediar com Chrome real, `--chrome-path`, `--proxy` e espera — NUNCA com Lite ou HTTP
- Contrato ONE-SHOT — cada invocação é dona da árvore Chromium + Xvfb (Linux) + perfil TempDir
- Em sucesso, erro, timeout, SIGINT ou SIGTERM a CLI encerra a árvore completa (process group + marker user-data-dir) e remove o perfil
- SEMPRE encapsular com GNU `timeout` (SIGTERM primeiro) para cancelamento cooperativo e reap rodarem
- Fórmula de wrapper OBRIGATÓRIA — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json`
- NÃO DEVE esperar limpeza automática de órfãos históricos nem de processos após SIGKILL nu da CLI
- SEMPRE usar `-q` e `-f json` em pipelines de agente
- SEMPRE processar JSON com `jaq` — NUNCA `jq`
- SEMPRE capturar exit code ANTES de parsear — com pipe DEVE usar `${PIPESTATUS[0]}`
- SEMPRE aplicar `// ""` em campos opcionais no `jaq`
- Runtime Linux — Chrome/Chromium + Xvfb; macOS/Windows — Chrome/Chromium (headless=new)
- Campos `.metadados.usou_chrome` e `.metadados.tentou_chrome` indicam tentativa e sucesso Chrome

## Workflow numerado

1. DEVE escolher o modo — busca simples, news, deep-research, batch, probe ou fetch-content
2. DEVE montar a fórmula com `timeout` SIGTERM-first, `-q`, `-f json` e flags do modo
3. DEVE executar e capturar exit code + stdout
4. SE exit 0 e quantidade de resultados maior que zero — DEVE extrair com `jaq` e citar fontes
5. SE quantidade de resultados for zero — DEVE ler `.metadados.causa_zero` e `.metadados.sugestao_proxima_acao`
6. SE exit 2 — DEVE instalar Chrome ou corrigir args; NUNCA setar `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` em produção
7. SE exit 3 ou 6 — DEVE aguardar, rotacionar proxy e revalidar Chrome; NUNCA usar Lite como remediação
8. SE exit 4 — DEVE elevar `--global-timeout` ou reduzir `--num` e `--parallel`
9. SE exit 5 com `causa_zero` igual a `legitimo` ou `vertical-sem-resultados` — DEVE reformular a query
10. SE exit 130 — NÃO DEVE tratar como falha de busca; reexecutar se necessário
11. Após cada invocação DEVE assumir que a CLI encerrou Chromium, Xvfb e o perfil daquela sessão (contrato one-shot)

## Fórmulas de busca (todas as flags)

DEVE copiar e adaptar. Cada linha é fórmula pronta.

- Base OBRIGATÓRIA — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json`
- `-n` / `--num` (default 15, mínimo 1) — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json --num 30`
- `-f` / `--format` (`json|text|markdown|md|auto`) — para agente SEMPRE `-f json` — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json`
- `-o` / `--output` (escrita atômica; PROIBIDO `..` e dirs de sistema) — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json --output /tmp/resultados.json`
- `-t` / `--timeout` (default 15s por request) — `timeout 60 duckduckgo-search-cli --timeout 20 "QUERY" -q -f json`
- `-l` / `--lang` (default `pt`) — `timeout 60 duckduckgo-search-cli --lang pt-BR "QUERY" -q -f json`
- `-c` / `--country` (default `br`) — `timeout 60 duckduckgo-search-cli --country br "QUERY" -q -f json`
- `--region` (alias de `--country`) — `timeout 60 duckduckgo-search-cli --region br "QUERY" -q -f json`
- `-p` / `--parallel` (default 5, clamp 1..20; anti-bot preferir ≤5) — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--queries-file` — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --num 15`
- `--pages` (1..5, default 1; auto-eleva se `--num` maior que 10) — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json --num 20 --pages 3`
- `--retries` (0..10, default 2) — `timeout 60 duckduckgo-search-cli --retries 3 "QUERY" -q -f json`
- `--endpoint` (`html|lite`, default html) — em produção SERP é HTML Chrome; lite NÃO remedia bloqueio — `timeout 60 duckduckgo-search-cli --endpoint html "QUERY" -q -f json`
- `--vertical web` (default) — `timeout 60 duckduckgo-search-cli --vertical web "QUERY" -q -f json`
- `--vertical news` — `timeout 90 duckduckgo-search-cli --vertical news "QUERY" -q -f json`
- `--vertical all` — `timeout 90 duckduckgo-search-cli --vertical all "QUERY" -q -f json`
- `--time-filter` (`d|w|m|y`) — `timeout 60 duckduckgo-search-cli --time-filter d "QUERY" -q -f json`
- `--safe-search` (`off|moderate|on`, default moderate) — `timeout 60 duckduckgo-search-cli --safe-search off "QUERY" -q -f json`
- `--probe` — `timeout 15 duckduckgo-search-cli --probe -q -f json`
- `--identity-profile` (`auto|chrome-win|chrome-mac|chrome-linux|edge-win|firefox-linux|safari-mac`) — `timeout 60 duckduckgo-search-cli --identity-profile chrome-linux "QUERY" -q -f json`
- `--stream` — PROIBIDO (placeholder sem implementação); NUNCA use
- `-v` / `-vv` (stderr — 0 INFO, -v DEBUG, -vv TRACE) — `timeout 60 duckduckgo-search-cli -v "QUERY" -f json 2>/tmp/ddg-debug.log`
- `-q` / `--quiet` — OBRIGATÓRIO em pipeline — `timeout 60 duckduckgo-search-cli "QUERY" -q -f json`
- `--fetch-content` — `timeout 120 duckduckgo-search-cli "QUERY" -q -f json --fetch-content --max-content-length 5000`
- `--max-content-length` (default 10000 caracteres de texto extraído, 1..100000) — `timeout 120 duckduckgo-search-cli "QUERY" -q -f json --fetch-content --max-content-length 5000`
- `--proxy` (HTTP/HTTPS/SOCKS5/SOCKS5h) — `timeout 60 duckduckgo-search-cli --proxy socks5://127.0.0.1:1080 "QUERY" -q -f json`
- `--no-proxy` (mutuamente exclusivo com `--proxy`) — `timeout 60 duckduckgo-search-cli --no-proxy "QUERY" -q -f json`
- `--match-platform-ua` — `timeout 60 duckduckgo-search-cli --match-platform-ua "QUERY" -q -f json`
- `--per-host-limit` (1..10, default 2; com fetch-content preferir ≤2) — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- `--chrome-path` — `timeout 60 duckduckgo-search-cli --chrome-path /usr/bin/chromium "QUERY" -q -f json`
- `--no-color` — `timeout 60 duckduckgo-search-cli --no-color "QUERY" -q -f json`
- `--no-warmup` — `timeout 60 duckduckgo-search-cli --no-warmup "QUERY" -q -f json`
- `--no-cookie-persistence` — `timeout 60 duckduckgo-search-cli --no-cookie-persistence "QUERY" -q -f json`
- `--cookies-path` — `timeout 60 duckduckgo-search-cli --cookies-path /secure/cookies.json "QUERY" -q -f json`
- `--probe-deep` — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json`
- `--seed` — `timeout 60 duckduckgo-search-cli --seed 42 "QUERY" -q -f json`
- `--config` — `timeout 60 duckduckgo-search-cli --config ./config.toml "QUERY" -q -f json`
- `--allow-lite-fallback` — no-op; NÃO use como remediação — `timeout 60 duckduckgo-search-cli --allow-lite-fallback "QUERY" -q -f json`
- `--pre-flight` — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- `--global-timeout` (1..3600, default 60) — `timeout 90 duckduckgo-search-cli --global-timeout 60 "QUERY" -q -f json`
- stdin multi-query (uma query por linha) — `printf '%s\n' "q1" "q2" | timeout 120 duckduckgo-search-cli -q -f json`
- multi-query posicional — `timeout 120 duckduckgo-search-cli -q -f json "query um" "query dois"`

## Vertical news

- DEVE usar `--vertical news` para só notícias e `--vertical all` para web e news juntos
- news e all exigem Chrome; sem Chrome → exit 2 fail-closed
- `--fetch-content` NÃO se aplica a cards de news
- `--pre-flight` aplica-se somente à vertical web; com `--vertical news` é pulado
- Fórmula news — `timeout 90 duckduckgo-search-cli --vertical news "QUERY" -q -f json | jaq '.noticias'`
- Fórmula all — `timeout 90 duckduckgo-search-cli --vertical all "QUERY" -q -f json | jaq '{web:.resultados,news:.noticias}'`
- Extração news — `jaq -r '.noticias[] | [.posicao, .titulo, .url, (.fonte // ""), (.data_relativa // "")] | @tsv'`
- DEVE distinguir `.resultados[]` (web) de `.noticias[]` (news)

## Diagnóstico

- `--probe` — health check mínimo Chrome — `timeout 15 duckduckgo-search-cli --probe -q -f json | jaq '.status'`
- `--probe-deep` — detector CAPTCHA e interstitial — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'`
- `--pre-flight` — auto-rota via probe-deep — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- DEVE inspecionar `.metadados.causa_zero` em TODO zero de resultados
- ZeroCause tem 7 variantes — `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`, `vertical-sem-resultados`
- `vertical-sem-resultados` → exit 5 (news vazio legítimo), NÃO exit 6
- Causas diferentes de `legitimo` → exit 6 por padrão (exceto `vertical-sem-resultados`)
- DEVE seguir `.metadados.sugestao_proxima_acao` quando presente (Chrome, proxy, espera — NUNCA Lite)
- Opt-out legado — `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` restaura exit 5 para todos os zeros
- Multi-query — `.causa_zero_histogram` agrega causas
- Cascata `.metadados.nivel_cascata` (0..4, opcional) — se 4, DEVE rotacionar proxy ou aguardar 300s
- Remediação exit 3 ou 6 — `timeout 60 duckduckgo-search-cli --chrome-path /usr/bin/chromium --proxy socks5://127.0.0.1:1080 "QUERY" -q -f json`

## Exit codes

| Code | Significado | Ação OBRIGATÓRIA |
|------|-------------|------------------|
| 0 | Sucesso com resultados | Parsear JSON com jaq e citar |
| 1 | Runtime (rede, I/O, parse) | Relatar stderr; retentar com `--retries` |
| 2 | Config ou args inválidos OU Chrome ausente ou NO_CHROME=1 | Corrigir args; instalar Chrome; NUNCA NO_CHROME em produção |
| 3 | Anti-bot soft-block | Aguardar 300s; `--chrome-path`; `--proxy`; NUNCA Lite |
| 4 | Timeout global | Elevar `--global-timeout`; reduzir carga |
| 5 | Zero legítimo | Reformular query, `--lang` ou `--time-filter` |
| 6 | Bloqueio suspeito (ZeroCause) | Ler `causa_zero` e `sugestao_proxima_acao` |
| 130 | Cancelado (SIGINT) | NÃO tratar como falha de busca; reexecutar se necessário |

## Batch e fetch-content

- `--queries-file` — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --num 15`
- `--parallel` / `-p` — preferir ≤5 contra anti-bot — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--per-host-limit` — com fetch-content preferir ≤2 — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- Root multi-query — `.buscas[]` — NUNCA confunda com `.resultados[]` de busca single
- Extração multi — `jaq -r '.buscas[] | .query as $q | .resultados[0] | "\($q)\t\(.titulo)\t\(.url)"'`
- `--fetch-content` + `--max-content-length` — `timeout 120 duckduckgo-search-cli "QUERY" -q -f json --fetch-content --max-content-length 5000`
- Também válido em deep-research — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --fetch-content --max-content-length 5000`
- Campos condicionais — `.resultados[].conteudo`, `.resultados[].tamanho_conteudo`, `.resultados[].metodo_extracao_conteudo`
- `tamanho_conteudo` = tamanho do texto truncado pós-extração
- `--fetch-content` NÃO enriquece cards de news

## Deep-research

- Por padrão cada sub-query roda vertical `all` (web e news) em Chrome
- Sem Chrome → exit 2 fail-closed (sem auto `--no-news`)
- DEVE preferir sub-queries manuais geradas pela LLM — NUNCA confiar só em strategy heuristic
- Fórmula base — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY"`
- Manual de qualidade OBRIGATÓRIO — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--max-sub-queries` (1..12, default 5) — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --max-sub-queries 5`
- `--sub-query-strategy` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--sub-queries-file` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-queries-file /tmp/sq.txt`
- `--aggregate` (`rrf` default | `dedupe-by-url`) — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --aggregate rrf`
- `--depth` (0..3, default 0) — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --depth 2`
- `--fetch-content` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --fetch-content --max-content-length 5000`
- `--synthesize` + `--budget-tokens` (default 4000) — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --budget-tokens 2000`
- `--synth-format` (`markdown|plain-text|json`) — valor correto é `plain-text`, NUNCA `plain` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --synth-format plain-text`
- `--synth-format markdown` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --synth-format markdown`
- `--require-results` — exit não-zero se fan-out zerar — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --require-results`
- `--no-news` — pula news intencionalmente; use SÓ com Chrome disponível e intenção explícita — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-news`
- Flags globais `-n -f -o -t -l -c -p -q -v` aceitas ANTES ou DEPOIS de `deep-research`
- RRF news é SEPARADO do RRF web — NUNCA compare scores entre `.noticias[]` e `.resultados[]`
- Exit deep-research — 0 se web OU news tiverem resultados; 5 só se AMBOS vazios
- Envelope — `.query`, `.sintese`, `.resultados[]`, `.noticias[]`, `.metadados.sub_queries[]`, `.quantidade_noticias`, `.metadados.total_noticias_unicas`

## Parsing JSON

- SEMPRE use `jaq` — NUNCA `jq`
- SEMPRE capture `${PIPESTATUS[0]}` após pipe para `jaq`
- SEMPRE use `// ""` em campos opcionais
- TSV web — `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'`
- Top 5 — `jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'`
- Citações — `jaq -r '.resultados[] | "- \(.titulo) — \(.url)"'`
- Status e exit com pipe — `out=$(timeout 60 duckduckgo-search-cli "QUERY" -q -f json); ec=$?; echo "$out" | jaq '.metadados.quantidade_resultados'; exit $ec`
- GARANTIDOS — `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`
- OPCIONAIS — `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`, `.metadados.identidade_usada`, `.metadados.nivel_cascata`
- CONDICIONAIS news — `.noticias[]`, `.quantidade_noticias`, `.metadados.vertical_usada`
- CONDICIONAIS fetch-content — `.resultados[].conteudo`, `.resultados[].tamanho_conteudo`, `.resultados[].metodo_extracao_conteudo`
- Chrome — `.metadados.usou_chrome`, `.metadados.tentou_chrome`
- Diagnóstico — `.metadados.causa_zero`, `.metadados.sugestao_proxima_acao`
- Pre-flight — `.metadados.pre_flight_disparado`, `.metadados.endpoint_usado`
- Compat — `quantidade_resultados`, `endpoint_usado`, `nivel_cascata` existem em root E em `.metadados`
- Identidade — formato family-platform-hex16
- Distinguir roots — single `.resultados[]` | multi `.buscas[]` | deep-research `.resultados[]` e `.noticias[]`

## Subcomandos

- Instalação — `cargo install duckduckgo-search-cli --locked --force`
- `init-config` — `duckduckgo-search-cli init-config`
- `init-config --force` — `duckduckgo-search-cli init-config --force`
- `init-config --dry-run` — `duckduckgo-search-cli init-config --dry-run`
- `completions bash` — `duckduckgo-search-cli completions bash`
- `completions zsh` — `duckduckgo-search-cli completions zsh`
- `completions fish` — `duckduckgo-search-cli completions fish`
- `completions powershell` — `duckduckgo-search-cli completions powershell`
- `completions elvish` — `duckduckgo-search-cli completions elvish`

## Segurança e ambiente

- Cookie jar default Unix — `~/.config/duckduckgo-search-cli/cookies.json` (modo 0o600)
- Cookie jar Windows — `%APPDATA%\duckduckgo-search-cli\cookies.json`
- NUNCA logar cookies; NUNCA commitar `cookies.json`
- NUNCA logar credenciais de `--proxy`
- DEVE usar `--no-cookie-persistence` em sessões efêmeras
- `DUCKDUCKGO_CHROME_HEADLESS=1` — força headless (risco anti-bot)
- `DUCKDUCKGO_CHROME_VISIBLE=1` — headed visível (debug)
- `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` — PROIBIDO em produção → exit 2 em qualquer op de rede
- `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` — só testes com feature http-test-harness
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` — exit 5 legado em todos os zeros
- `CHROME_PATH` — caminho alternativo do binário Chrome/Chromium
- `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` — respeitados salvo `--no-proxy`
- Lifecycle one-shot — NÃO DEVE matar a CLI com SIGKILL se quiser encerramento cooperativo; use GNU `timeout` (SIGTERM)

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
- PROIBIDO inventar resultados sem executar a CLI
- PROIBIDO parsear stdout com `jq` — SEMPRE `jaq`
- PROIBIDO esperar cleanup de órfãos após SIGKILL nu da CLI
- PROIBIDO omitir wrapper `timeout` em execuções de agente
- PROIBIDO rebaixar para HTTP puro quando Chrome falhar
