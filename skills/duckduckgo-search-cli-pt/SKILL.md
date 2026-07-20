---
name: duckduckgo-search-cli-pt
description: Esta skill DEVE auto-ativar para busca web, pesquisa na internet, docs atuais, grounding, verificação de URL, extração de página, enriquecimento RAG, fact-checking, pesquisa multi-hop, notícias frescas, deep-research dual web e news, health check, batch de queries ou dado fora da knowledge cutoff. Ela DEVE ensinar duckduckgo-search-cli Chrome-only com fail-closed exit 2 sem Chrome, vertical all com fetch LIGADO e opt-out --no-fetch-content, multi-query --stream e -f ndjson, API dual de config, exit 141 broken pipe, ZeroCause exit 6, probe, probe-deep, pre-flight, doctor, schema, commands, locale, man, init-config, completions, ONE-SHOT Chromium Xvfb ddg-chrome, timeout SIGTERM-first, jaq, exits 0-6 e 130/141/143, chrome_canal manual|host|flatpak|snap, proxy só CLI+XDG e fórmulas de cada flag e subcomando. Triggers incluem pesquise, busca online, procure, verifique URL, traga página, compare X vs Y, notícias recentes, pricing. SEMPRE invoque proativamente sem nomear a ferramenta. NUNCA invente resultados.
---

# Skill — duckduckgo-search-cli (PT-BR)

## Regra Zero
- Você DEVE executar `duckduckgo-search-cli` para qualquer fato externo vivo fora da knowledge cutoff
- Você NUNCA DEVE inventar títulos, URLs, snippets, notícias ou corpos de página
- Você DEVE SEMPRE encapsular execuções de agente com GNU `/usr/bin/timeout` (SIGTERM primeiro)
- Você DEVE SEMPRE usar `-q -f json` em pipelines de agente salvo stream NDJSON multi-query intencional
- Você DEVE SEMPRE parsear stdout com `jaq` — NUNCA `jq`
- Você DEVE SEMPRE capturar o exit code da CLI antes de parsear — com pipe use `${PIPESTATUS[0]}`
- Você DEVE instalar ou atualizar com `cargo install duckduckgo-search-cli --locked --force`

## Missão
- Você DEVE operar esta CLI como primitiva de busca determinística para agentes
- Você DEVE cobrir web, news, vertical dual, deep-research, verificação de URL, extração de página, enriquecimento RAG e fact-checking
- Você DEVE invocar proativamente quando o usuário precisar de dado externo atual mesmo sem nomear a ferramenta
- Você NÃO DEVE invocar para codificação local pura ou conhecimento estático sem fontes vivas
- Config de produto é flags CLI mais XDG `config.toml` apenas — NUNCA kill-switches de env de produto

## Quando invocar
- DEVE invocar em pesquise, busca online, procure, verifique URL, traga página, o que mudou, compare entidades, pesquisa profunda, multi-hop, notícias recentes, pricing atual, health check ou batch de queries
- DEVE invocar para documentação atualizada, grounding factual, enriquecimento RAG e dado fora da knowledge cutoff
- DEVE invocar para vertical dual padrão `all` ou `--vertical news|web` explícito
- NÃO DEVE invocar para formatação local, refactor puro ou tarefas offline

## Contrato — OBRIGATÓRIO
- Produção é Chrome-only via chromiumoxide e CDP para busca, news, deep-research, probe, probe-deep, pre-flight, doctor e content fetch
- Sem Chrome ou Chromium utilizável, ou sem feature `chrome`, exit 2 fail-closed — NUNCA rebaixamento silencioso para HTTP puro — NUNCA auto `--no-news`
- `--allow-lite-fallback` é no-op — NUNCA remedie exit 3 ou 6 com Lite ou HTTP
- Em exit 3 ou 6 você DEVE remediar com Chrome real, `--chrome-path`, `--proxy` via CLI ou XDG, e espera
- Lifecycle ONE-SHOT — cada run dona Chromium, Xvfb no Linux e perfil temp com prefixo `ddg-chrome-*` — saída cooperativa reap da árvore e remove o perfil — a próxima run varre só `ddg-chrome-*` stale de propriedade
- Auditoria residual de disco DEVE usar somente `find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'ddg-chrome-*' 2>/dev/null`
- PROIBIDO bulk find ou rm de `.tmp*` ou `org.chromium.Chromium.*` como higiene desta CLI
- No Unix broken pipe produz exit 141 e o cleanup oneshot ainda roda — NUNCA trate 141 como falha de busca
- Ordem de path Chrome DEVE ser CLI `--chrome-path` depois XDG `chrome_path` depois auto host Flatpak Snap — NUNCA ensine `CHROME_PATH` como knob de produto
- Valores de `.metadados.chrome_canal` DEVE ser exatamente `manual|host|flatpak|snap` — NUNCA `env`
- DEFAULT vertical é `all` — DEFAULT content fetch LIGADO para web e news sob `--fetch-content-cap` default 10 — opt-out com `--no-fetch-content`
- `--stream` é PERMITIDO para multi-query NDJSON — `-f ndjson` é o alias de stream — query única com `--stream` é ignorada com warning — NUNCA trate stream como stream completo de hits SERP
- API dual de config DEVE aceitar formas posicionais e por flags — `config get KEY` ou `config get --key KEY` — `config set KEY VALUE` ou `config set --key KEY --value VALUE` — `config unset KEY` ou `config unset --key KEY` — `config effective` mostra CLI maior que XDG maior que defaults
- Somente ALLOWED_KEYS — `ui_lang`, `chrome_path`, `proxy_url`, `default_global_timeout`, `default_vertical`, `fetch_content_default`, `log_directive`, `default_lang`, `default_country` — NUNCA invente chaves de config
- Wire JSON serializa nomes de campo em português — aliases ingleses só no deserialize
- Precedência do filtro de log de produto DEVE ser `-q` maior que `-v` ou `-vv` maior que XDG `log_directive` maior que `info` — NUNCA ensine `RUST_LOG` como config de produto
- Proxy DEVE ser CLI `--proxy` ou `--no-proxy` ou XDG `proxy_url` apenas — NUNCA `HTTP_PROXY` `HTTPS_PROXY` `ALL_PROXY`
- `--output` é atômico — PROIBIDO caminhos com `..` ou dirs de sistema
- Timeouts externos OBRIGATÓRIOS em segundos — dual mais fetch 180 — SERP-only 90 — web thin 60 — deep-research 180 — batch 300 — probe 15 — probe-deep 20 — doctor 30
- Defaults — num 15 — format auto (agentes DEVE forçar json salvo stream multi-query intencional) — timeout 15s — lang pt — country br — parallel 5 clamp 1..20 — pages 1 — retries 2 — endpoint html — vertical all — safe-search moderate — identity-profile auto — max-content-length 10000 — fetch-content-cap 10 — per-host-limit 2 — global-timeout 180 — cancel-grace-secs 5 — max-sub-queries 5 — aggregate rrf — depth 0 — budget-tokens 4000
- Padrão Correto base — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`

## Workflow
1. DEVE escolher o modo — busca, news, deep-research, batch, stream, probe, pre-flight, doctor ou SERP-only
2. DEVE montar a fórmula com GNU `timeout`, `-q`, `-f json` e flags do modo
3. DEVE executar e capturar exit code mais stdout — com pipe DEVE usar `${PIPESTATUS[0]}`
4. SE exit 0 com resultados — DEVE extrair com `jaq` e citar fontes
5. SE zero resultados — DEVE ler `.metadados.causa_zero` e `.metadados.sugestao_proxima_acao` antes de retentar
6. SE exit 2 — DEVE instalar Chrome ou corrigir args — fail-closed
7. SE exit 3 ou 6 — DEVE aguardar, rotacionar proxy via CLI ou XDG, revalidar path e canal Chrome — NUNCA Lite
8. SE exit 4 — DEVE elevar `--global-timeout` ou reduzir carga
9. SE exit 5 com `legitimo` ou `vertical-sem-resultados` — DEVE reformular query ou ajustar lang time-filter vertical
10. SE exit 130 141 ou 143 — NÃO DEVE tratar como falha de busca
11. Após cada run DEVE assumir reap de Chromium Xvfb e perfil `ddg-chrome-*`

## Fórmulas de execução — todas as flags e modos
DEVE copiar e adaptar. SEMPRE mantenha `-q -f json` em pipelines de agente salvo stream multi-query intencional.

### Modos base
- Dual mais fetch OBRIGATÓRIO — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- SERP-only mais rápido — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- Web thin — `timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content "QUERY" -q -f json`
- News-only — `timeout 180 duckduckgo-search-cli --vertical news "QUERY" -q -f json`
- Multi-query posicional — `timeout 120 duckduckgo-search-cli -q -f json "query um" "query dois"`
- Multi-query stdin — `printf '%s\n' "q1" "q2" | timeout 120 duckduckgo-search-cli -q -f json`
- Stream multi-query — `timeout 120 duckduckgo-search-cli -q --stream "q1" "q2"`
- Alias stream multi-query — `timeout 120 duckduckgo-search-cli -q -f ndjson "q1" "q2"`

### Flags de superfície de busca
- `-n` `--num` default 15 mín 1 — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --num 30`
- `-f` `--format` valores `json|text|markdown|md|tsv|ndjson|auto` — agente SEMPRE força json salvo stream multi-query — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `-o` `--output` escrita atômica — PROIBIDO `..` e dirs de sistema — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --output /tmp/resultados.json`
- `-t` `--timeout` por request default 15s — `timeout 180 duckduckgo-search-cli --timeout 20 "QUERY" -q -f json`
- `-l` `--lang` idioma SERP default `pt` — NÃO é idioma de UI — `timeout 180 duckduckgo-search-cli --lang pt-BR "QUERY" -q -f json`
- `-c` `--country` default `br` — `timeout 180 duckduckgo-search-cli --country br "QUERY" -q -f json`
- `--region` alias de `--country` — `timeout 180 duckduckgo-search-cli --region br "QUERY" -q -f json`
- `-p` `--parallel` default 5 clamp 1..20 — anti-bot DEVE ficar em no máximo 5 — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--max-concurrency` alias de `--parallel` — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --max-concurrency 3 -q -f json`
- `--queries-file` uma query por linha — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 --num 15`
- `--pages` 1..5 default 1 — auto-eleva se `--num` maior que 10 — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --num 20 --pages 3`
- `--retries` 0..10 default 2 — `timeout 180 duckduckgo-search-cli --retries 3 "QUERY" -q -f json`
- `--disable-retry` desliga retries nativos — `timeout 180 duckduckgo-search-cli --disable-retry "QUERY" -q -f json`
- `--endpoint` `html|lite` default html — SERP produção é HTML Chrome — lite NÃO remedia — `timeout 180 duckduckgo-search-cli --endpoint html "QUERY" -q -f json`
- `--vertical all` DEFAULT dual — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `--vertical web` — `timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content "QUERY" -q -f json`
- `--vertical news` — `timeout 180 duckduckgo-search-cli --vertical news "QUERY" -q -f json`
- `--shared-session-verticals` compartilha uma sessão Chrome no dual — `timeout 180 duckduckgo-search-cli --shared-session-verticals "QUERY" -q -f json`
- `--time-filter` `d|w|m|y` — `timeout 180 duckduckgo-search-cli --time-filter d "QUERY" -q -f json`
- `--safe-search` `off|moderate|on` default moderate — `timeout 180 duckduckgo-search-cli --safe-search off "QUERY" -q -f json`
- `--identity-profile` `auto|chrome-win|chrome-mac|chrome-linux|edge-win|firefox-linux|safari-mac` default auto — NUNCA hardcode em CI — `timeout 180 duckduckgo-search-cli --identity-profile chrome-linux "QUERY" -q -f json`
- `--stream` multi-query NDJSON apenas — `timeout 120 duckduckgo-search-cli -q --stream "q1" "q2"`
- `-v` `--verbose` e `-vv` — log de produto = CLI mais XDG — precedência `-q` maior que `-v` ou `-vv` maior que XDG maior que info — `timeout 180 duckduckgo-search-cli -v "QUERY" -f json 2>/tmp/ddg-debug.log`
- `-q` `--quiet` OBRIGATÓRIO em pipelines — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json`
- `--fetch-content` ON explícito redundante — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --fetch-content --max-content-length 5000`
- `--no-fetch-content` opt-out SERP-only — `timeout 90 duckduckgo-search-cli --no-fetch-content "QUERY" -q -f json`
- `--fetch-content-cap` 1..50 default 10 — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --fetch-content-cap 5`
- `--max-content-length` default 10000 range 1..100000 — `timeout 180 duckduckgo-search-cli "QUERY" -q -f json --max-content-length 5000`
- `--proxy` HTTP HTTPS SOCKS5 SOCKS5h — `timeout 180 duckduckgo-search-cli --proxy socks5://127.0.0.1:1080 "QUERY" -q -f json`
- `--no-proxy` exclusivo com `--proxy` — `timeout 180 duckduckgo-search-cli --no-proxy "QUERY" -q -f json`
- `--match-platform-ua` — `timeout 180 duckduckgo-search-cli --match-platform-ua "QUERY" -q -f json`
- `--per-host-limit` 1..10 default 2 — com fetch LIGADO DEVE ficar em no máximo 2 — `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- `--chrome-path` wrappers e Flatpak resolvem para ELF real — `timeout 180 duckduckgo-search-cli --chrome-path /usr/lib64/chromium-browser/chromium-browser "QUERY" -q -f json`
- `--chrome-visible` Chrome headed — `timeout 180 duckduckgo-search-cli --chrome-visible "QUERY" -q -f json`
- `--chrome-headless` força headless — `timeout 180 duckduckgo-search-cli --chrome-headless "QUERY" -q -f json`
- `--chrome-xvfb` força caminho Xvfb no Linux — `timeout 180 duckduckgo-search-cli --chrome-xvfb "QUERY" -q -f json`
- `--dump-news-html` captura HTML de news para debug — `timeout 180 duckduckgo-search-cli --vertical news --dump-news-html /tmp/news.html "QUERY" -q -f json`
- `--no-color` — `timeout 180 duckduckgo-search-cli --no-color "QUERY" -q -f json`
- `--no-warmup` — `timeout 180 duckduckgo-search-cli --no-warmup "QUERY" -q -f json`
- `--no-cookie-persistence` — `timeout 180 duckduckgo-search-cli --no-cookie-persistence "QUERY" -q -f json`
- `--cookies-path` — `timeout 180 duckduckgo-search-cli --cookies-path /secure/cookies.json "QUERY" -q -f json`
- `--seed` — `timeout 180 duckduckgo-search-cli --seed 42 "QUERY" -q -f json`
- `--config` diretório de selectors NÃO arquivo toml de produto — `timeout 180 duckduckgo-search-cli --config /caminho/para/dir-selectors "QUERY" -q -f json`
- `--config-home` sobrescreve home XDG — `timeout 180 duckduckgo-search-cli --config-home /tmp/ddg-xdg "QUERY" -q -f json`
- `--allow-lite-fallback` no-op NÃO remedia — `timeout 180 duckduckgo-search-cli --allow-lite-fallback "QUERY" -q -f json`
- `--global-timeout` 1..3600 default 180 — `timeout 200 duckduckgo-search-cli "QUERY" -q -f json --global-timeout 180`
- `--ui-lang` UI stderr `en|pt-BR` — NÃO é SERP `-l` — `timeout 180 duckduckgo-search-cli --ui-lang pt-BR "QUERY" -q -f json`
- `--cancel-grace-secs` 1..60 default 5 — `timeout 180 duckduckgo-search-cli --cancel-grace-secs 10 "QUERY" -q -f json`
- `--no-zero-cause-strict` zeros legados como exit 5 — default strict mapeia zeros suspeitos para exit 6 — `timeout 180 duckduckgo-search-cli --no-zero-cause-strict "QUERY" -q -f json`
- `--base-url-html` override de teste — `timeout 180 duckduckgo-search-cli --base-url-html https://example.test "QUERY" -q -f json`
- `--base-url-lite` override de teste — `timeout 180 duckduckgo-search-cli --base-url-lite https://example.test "QUERY" -q -f json`
- `--base-url-serp` override de teste — `timeout 180 duckduckgo-search-cli --base-url-serp https://example.test "QUERY" -q -f json`
- `--pre-flight` auto-rota via probe-deep na web — pulado em news puro — `timeout 60 duckduckgo-search-cli --pre-flight "QUERY" -q -f json`
- `--probe` — `timeout 15 duckduckgo-search-cli --probe -q -f json`
- `--probe-deep` — `timeout 20 duckduckgo-search-cli --probe-deep -q -f json`

### Flags de deep-research
- Base — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY"`
- Qualidade OBRIGATÓRIA manual — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--max-sub-queries` 1..12 default 5 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --max-sub-queries 5`
- `--sub-query-strategy` `heuristic|manual` — qualidade DEVE usar manual mais arquivo
- `--sub-queries-file` uma sub-query por linha — obrigatório com manual
- `--aggregate` `rrf|dedupe-by-url` default rrf — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --aggregate rrf`
- `--depth` 0..3 default 0 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --depth 2`
- `--synthesize` mais `--budget-tokens` default 4000 — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --budget-tokens 2000`
- `--synth-format` `markdown|plain-text|json` — valor DEVE ser `plain-text` NUNCA `plain` — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --synthesize --synth-format plain-text`
- `--require-results` exit não zero se fan-out agregar zero — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --require-results`
- `--no-news` deep web-only com intenção explícita e Chrome disponível — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-news`
- Fetch deep LIGADO por padrão — SERP-only deep — `timeout 180 duckduckgo-search-cli -q -f json deep-research "QUERY" --no-fetch-content`
- Deep ignora `--vertical` para seleção de modo — use `--no-news` para pular news — flags globais de transporte ainda aceitas antes ou depois de `deep-research`
- RRF news é SEPARADO do RRF web — NUNCA compare scores entre `.noticias[]` e `.resultados[]`
- Deep exit 0 se web OU news tiverem resultados — exit 5 só se AMBOS vazios

### Subcomandos de diagnóstico e descoberta
- `doctor` — `timeout 30 duckduckgo-search-cli doctor -q -f json`
- `doctor --strict` — `timeout 30 duckduckgo-search-cli doctor --strict -q -f json`
- `schema` lista — `duckduckgo-search-cli schema -q -f json`
- `schema --name ID` — `duckduckgo-search-cli schema --name search-output -q -f json`
- `commands` árvore — `duckduckgo-search-cli commands -q -f json`
- `locale` diagnóstico de locale de UI — `duckduckgo-search-cli locale -q -f json`
- `man` imprime manpage — `duckduckgo-search-cli man`
- `man --file PATH` grava manpage — `duckduckgo-search-cli man --file /tmp/ddg.1`
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

## Modos — comportamento OBRIGATÓRIO
- DEFAULT da busca é dual `all` sem flags extras
- DEVE usar `--vertical news` para só notícias e `--vertical web` para pular news
- news e all exigem Chrome — sem Chrome exit 2
- Content fetch aplica-se a cards news com fetch LIGADO — `.noticias[].conteudo` sob fetch-content-cap
- Root multi-query é `.buscas[]` em JSON de batch ou uma linha NDJSON por query no stream — NUNCA confunda com single `.resultados[]`
- Multi-query DEVE inspecionar `.causa_zero_histogram` quando presente
- DEVE inspecionar metadados Chrome — `.metadados.chrome_path_resolvido` `.metadados.chrome_canal` `.metadados.usou_chrome` `.metadados.tentou_chrome`
- ZeroCause sete valores — `legitimo` `filtro-silencioso` `ghost-block` `anti-bot` `resposta-invalida` `zero-resultados-suspeito` `vertical-sem-resultados`
- `vertical-sem-resultados` produz exit 5 — demais zeros não-`legitimo` produzem exit 6 no strict default
- DEVE seguir `.metadados.sugestao_proxima_acao` quando presente — aponta para Chrome proxy ou espera — NUNCA Lite
- Cascata `.metadados.nivel_cascata` é opcional 0..4 — se 4 DEVE rotacionar proxy ou aguardar 300s

## Contrato JSON e parsing
Nomes de campo em português permanecem como a CLI emite. SEMPRE parseie com `jaq`. NUNCA `jq`.

- Capture exit primeiro — `out=$(timeout 180 duckduckgo-search-cli "QUERY" -q -f json); ec=$?; echo "$out" | jaq .; exit $ec`
- TSV web — `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'`
- Extração news — `jaq -r '.noticias[] | [.posicao, .titulo, .url, (.fonte // ""), (.data_relativa // "")] | @tsv'`
- Extração dual — `jaq '{web:.resultados,news:.noticias,path:.metadados.chrome_path_resolvido,canal:.metadados.chrome_canal}'`
- Diagnóstico zero — `jaq '{causa:.metadados.causa_zero,acao:.metadados.sugestao_proxima_acao,n:.metadados.quantidade_resultados}'`
- Status probe — `jaq '.status'`
- Extração multi — `jaq -r '.buscas[] | .query as $q | .resultados[0] | "\($q)\t\(.titulo)\t\(.url)"'`
- GARANTIDOS — `.query` `.resultados[].posicao` `.resultados[].titulo` `.resultados[].url` `.metadados.tempo_execucao_ms` `.metadados.quantidade_resultados` `.metadados.usou_endpoint_fallback`
- OPCIONAIS — `.resultados[].snippet` `.resultados[].url_exibicao` `.resultados[].titulo_original` `.metadados.identidade_usada` `.metadados.nivel_cascata`
- CONDICIONAIS news — `.noticias[]` `.quantidade_noticias` `.metadados.vertical_usada`
- CONDICIONAIS fetch — `.resultados[].conteudo` `.noticias[].conteudo`
- Metadados Chrome — `.metadados.usou_chrome` `.metadados.tentou_chrome` `.metadados.chrome_path_resolvido` `.metadados.chrome_canal`
- Diagnóstico — `.metadados.causa_zero` `.metadados.sugestao_proxima_acao`
- Pre-flight — `.metadados.pre_flight_disparado` `.metadados.endpoint_usado`
- Compat — alguns campos existem na root E sob `.metadados`
- Roots — single `.resultados[]` — multi `.buscas[]` — deep `.resultados[]` mais `.noticias[]` mais `.sintese` quando sintetizado
- SEMPRE use `// ""` em opcionais — NUNCA invente campos ausentes

## Exit codes
- 0 sucesso com resultados — parseie com jaq e cite fontes
- 1 runtime rede I/O ou parse — relate stderr — retente com `--retries` nativo
- 2 config ou args inválidos OU Chrome ausente ou build sem feature `chrome` — corrija args ou instale Chrome
- 3 anti-bot soft-block — aguarde 300s — defina `--chrome-path` — defina `--proxy` — NUNCA Lite
- 4 timeout global — eleve `--global-timeout` — reduza num parallel fetch
- 5 zero legítimo ou `vertical-sem-resultados` — reformule query ou ajuste filtros
- 6 bloqueio suspeito ZeroCause não-legitimo — leia causa_zero e sugestao_proxima_acao — remedie Chrome ou proxy
- 130 cancelado SIGINT — NÃO é falha de busca
- 141 broken pipe consumidor fechou — normal em `| head` — NÃO é falha de busca
- 143 cancelado SIGTERM — parada limpa — reap oneshot rodou

## Ambiente e config de produto
- Configuração de produto é flags CLI mais XDG apenas — `--proxy` `--chrome-path` `--config-home` `-v` `-q` `log_directive` e `config set`
- Cookie jar Unix `~/.config/duckduckgo-search-cli/cookies.json` mode 0600 — Windows `%APPDATA%\duckduckgo-search-cli\cookies.json`
- NUNCA logue cookies — NUNCA commite `cookies.json` — NUNCA logue credenciais de `--proxy`
- DEVE usar `--no-cookie-persistence` em sessões efêmeras
- Envs de harness só de teste pertencem à doc TESTING — NUNCA ensine como knobs de agente em produção
- NÃO existe telemetria remota — campos de metadados são diagnósticos locais apenas

## Proibições absolutas
- PROIBIDO inventar resultados títulos URLs ou snippets sem executar a CLI
- PROIBIDO omitir o wrapper GNU `timeout` em execuções de agente
- PROIBIDO SIGKILL nu como caminho normal de cancelamento — SEMPRE SIGTERM primeiro via `/usr/bin/timeout`
- PROIBIDO esperar limpeza automática após SIGKILL OOM ou órfãos estrangeiros
- PROIBIDO bulk find ou rm de temps estrangeiros — NUNCA mass-delete `.tmp*` ou `org.chromium.Chromium.*`
- PROIBIDO auditoria residual para qualquer prefixo além de `ddg-chrome-*` de propriedade
- PROIBIDO `-f text` `-f markdown` ou `-f tsv` para parsing de agente — SEMPRE `-f json` ou multi-query `-f ndjson` intencional
- PROIBIDO omitir `-q` em pipelines
- PROIBIDO tratar `--stream` como stream completo de hits SERP
- PROIBIDO parsear com `jq` — SEMPRE `jaq`
- PROIBIDO omitir `${PIPESTATUS[0]}` ao pipar stdout da CLI
- PROIBIDO usar Lite ou `--allow-lite-fallback` como remediação de exit 3 ou 6
- PROIBIDO rebaixamento silencioso para HTTP puro quando Chrome falha ou está ausente
- PROIBIDO auto `--no-news` quando Chrome está ausente
- PROIBIDO hardcodar API keys proxies ou user-agents em commits
- PROIBIDO hardcodar `--identity-profile` em CI
- PROIBIDO `--output` com `..` ou diretórios de sistema
- PROIBIDO tratar `identidade_usada` ou `nivel_cascata` como campos garantidos
- PROIBIDO ignorar zero sem ler `causa_zero` e `sugestao_proxima_acao`
- PROIBIDO ignorar exit 6
- PROIBIDO tratar exit 141 como falha de busca
- PROIBIDO loops de retry em shell — use `--retries` nativo
- PROIBIDO combinar `--proxy` com `--no-proxy`
- PROIBIDO confiar em `HTTP_PROXY` `HTTPS_PROXY` ou `ALL_PROXY` para proxy de produto
- PROIBIDO ensinar `CHROME_PATH` ou `RUST_LOG` como knobs de produto
- PROIBIDO inventar chaves de config fora de ALLOWED_KEYS
- PROIBIDO `--synth-format plain` — o valor correto é `plain-text`
- PROIBIDO comparar scores RRF de news com scores RRF de web
