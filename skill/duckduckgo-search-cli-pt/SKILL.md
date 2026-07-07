---
name: duckduckgo-search-cli-pt
description: Esta skill DEVE ser invocada quando o usuário pedir busca web, pesquisa na internet, documentação atualizada, grounding factual, verificação de URL, extração de página, enriquecimento RAG, fact-checking, pesquisa multi-hop, vertical de notícias, deep-research dual web+news, ou qualquer dado fora da knowledge cutoff. Triggers — pesquise, busca online, procure, verifique URL, traga página, o que mudou, compare X vs Y, pesquisa profunda, notícias recentes, pricing atual. Chrome HEADED dentro de Xvfb privado com stealth anti-detecção. Auto-install Xvfb em 22+ distros Linux. UA Chrome-only para paridade TLS/JA4. ZeroCause 7 variantes com exit code 6. Pool 12 identidades anti-bot. deep-research fan-out RRF dual web+news por padrão. Vertical news via --vertical news|all. reqwest+rustls-tls para fetch-content e probe. Português brasileiro
---

# Skill — duckduckgo-search-cli (PT-BR)


## Quando invocar esta CLI
- DEVE invocar quando a resposta exigir dado fora da knowledge cutoff
- DEVE invocar em triggers — pesquise, busca, procure, verifique URL, traga página, o que mudou, compare, pesquisa profunda, grounding, pricing atual, pergunta multi-hop, notícias recentes
- DEVE preferir esta CLI sobre WebSearch/WebFetch para pipelines determinísticos
- DEVE invocar para fact-checking, verificação de versão de biblioteca, post-mortem de incidente ou qualquer dado volátil
- DEVE invocar para vertical de notícias frescas (artigos com fonte, data relativa e thumbnail) via `--vertical news|all`


## Transporte Chrome e anti-detecção
- Chrome roda em modo HEADED dentro de display virtual Xvfb PRIVADO como transporte PRIMÁRIO de busca — o usuário vê ZERO janelas
- Em Linux Desktop com display nativo, a CLI SEMPRE spawna Xvfb privado para evitar janela visível ao usuário
- Xvfb é auto-instalado em 22+ distros Linux — em distros imutáveis o auto-install é pulado e instruções manuais são exibidas
- Stale lock files de Xvfb são limpos automaticamente antes de spawnar novo display
- Se Xvfb indisponível após tentativa de auto-install, fallback para headless com mensagem de instrução ao usuário
- Chrome navega PRIMEIRO para duckduckgo.com como warm-up antes da URL de busca real — resolve JS challenge e seta cookies
- Sinais stealth anti-detecção são injetados ANTES de qualquer navegação — WebGL, canvas, audio fingerprint, permissions API, prevenção de leak de automação
- O pool de identidades aceita APENAS UAs Chrome quando o browser real é Chromium — evita mismatch UA/TLS detectável via JA3/JA4
- reqwest+rustls-tls é usado APENAS para `--fetch-content` e `--probe` — NUNCA para buscas primárias
- Campo `.metadados.usou_chrome` indica `true` quando busca Chrome teve sucesso
- Campo `.metadados.tentou_chrome` indica `true` quando busca Chrome foi tentada


## Padrões obrigatórios de pipeline
- SEMPRE encapsular com `timeout` em segundos — pipeline trava sem limite temporal
- SEMPRE usar `-q` em pipelines — tracing de stderr polui stdout sem esta flag
- SEMPRE usar `-f json` para parsing programático — NUNCA `-f text` ou `-f markdown`
- SEMPRE usar `jaq` (NUNCA `jq`) para processar output JSON
- SEMPRE aplicar `// ""` como fallback em campos opcionais ao usar `jaq`
- SEMPRE capturar exit code ANTES de parsear stdout — DEVE usar `${PIPESTATUS[0]}` quando piped via `jaq`
- A flag `--allow-lite-fallback` DEVE vir ANTES do subcomando `deep-research` — Clap rejeita após o subcomando com exit 2
- DEVE usar `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'` para TSV tabulado
- DEVE usar `jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'` para lista markdown de top 5
- DEVE usar `jaq -r '.resultados[] | "- \(.titulo) — \(.url)"'` para bloco de citação de fontes


## Referência completa de flags com fórmulas
- `--num N` — quantidade de resultados (mínimo 1, `--num 0` rejeitado pelo clap) — DEVE usar `timeout 60 duckduckgo-search-cli "query" -q -f json --num 30`
- `--lang <CÓDIGO>` — idioma da busca (default `pt`) — DEVE usar `timeout 60 duckduckgo-search-cli --lang pt-BR "query" -q -f json`
- `--country <CÓDIGO>` / `-c` — country code (ex: br, us, de); `--region` é ALIAS de `--country` para compatibilidade — DEVE usar `timeout 60 duckduckgo-search-cli --country br "query" -q -f json`
- `--time-filter <PERÍODO>` — filtro temporal (d=dia, w=semana, m=mês, y=ano) — DEVE usar `timeout 60 duckduckgo-search-cli --time-filter d "query" -q -f json` para últimas 24h
- `--safe-search <NÍVEL>` — filtro de conteúdo (off/moderate/on, default moderate) — DEVE usar `timeout 60 duckduckgo-search-cli --safe-search off "query" -q -f json`
- `--endpoint <NOME>` — endpoint de busca (html/lite, default html) — DEVE usar `timeout 60 duckduckgo-search-cli --endpoint lite "query" -q -f json`
- `--vertical <web|news|all>` — vertical de busca (default web); news/all são Chrome-only (sem fallback HTTP); batches multi-query aceitos (uma sessão Chrome por query) — DEVE usar `timeout 90 duckduckgo-search-cli --vertical news "query" -q -f json | jaq '.noticias'`
- `--no-news` — flag do deep-research que pula a varredura news; por padrão cada sub-query roda `--vertical all` via Chrome; sem Chrome utilizável e sem `--no-news` o subcomando sai com exit 2 — DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --no-news` em ambientes sem Chrome
- `--pages <N>` — páginas por query (1 a 5, default 1, auto-elevado quando `--num` > 10) — DEVE usar `timeout 60 duckduckgo-search-cli "query" -q -f json --num 20 --pages 3`
- `-f json` / `--format json` — formato de saída JSON — OBRIGATÓRIO para parsing programático, NUNCA usar `-f text` ou `-f markdown`
- `-q` / `--quiet` — modo silencioso — OBRIGATÓRIO em pipelines para evitar tracing de stderr poluindo stdout
- `-v` info / `-vv` debug / `-vvv` trace (aditivo) — DEVE usar `timeout 60 duckduckgo-search-cli -vv "query" -f json 2>/tmp/debug.log`
- `--output <PATH>` — escrita atômica do payload completo (rejeitado se `..` ou diretórios de sistema) — DEVE usar `timeout 60 duckduckgo-search-cli "query" -q -f json --output /tmp/resultados.json`
- `--retries N` — retentativas com backoff exponencial (clamp [1, 10], default 2, NUNCA > 10) — DEVE usar `timeout 60 duckduckgo-search-cli --retries 3 "query" -q -f json`
- `--timeout N` — timeout por requisição HTTP em segundos (default 15) — DEVE usar `timeout 60 duckduckgo-search-cli --timeout 20 "query" -q -f json`
- `--global-timeout N` — timeout global da operação em segundos (clamp [1, 3600]) — DEVE usar `timeout 90 duckduckgo-search-cli --global-timeout 60 "query" -q -f json`
- `--no-color` — desabilita cores no output — DEVE usar `timeout 60 duckduckgo-search-cli --no-color "query" -q -f json`
- `--config <PATH>` — arquivo TOML de configuração — DEVE usar `timeout 60 duckduckgo-search-cli --config ./config.toml "query" -q -f json`
- `--probe` — health check mínimo via reqwest — DEVE usar `timeout 10 duckduckgo-search-cli --probe -q -f json | jaq '.status'`
- `--probe-deep` — detector CAPTCHA via query real — DEVE usar `timeout 15 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'`
- `--pre-flight` — auto-rota via probe-deep primeiro — DEVE usar `timeout 60 duckduckgo-search-cli --pre-flight "query" -q -f json`
- `--allow-lite-fallback` — opt-in rebaixamento HTML para Lite em CAPTCHA — DEVE usar `timeout 60 duckduckgo-search-cli --allow-lite-fallback "query" -q -f json`
- `--identity-profile <name>` — pina identidade (auto/chrome-win/chrome-mac/chrome-linux/edge-win/firefox-linux/safari-mac) — DEVE usar `timeout 60 duckduckgo-search-cli --identity-profile chrome-linux "query" -q -f json`
- `--seed <u64>` — seed determinístico para UA + rotação do pool — DEVE usar `timeout 60 duckduckgo-search-cli --seed 42 "query" -q -f json` para reprodutibilidade
- `--no-warmup` — pula warm-up de cookies duckduckgo.com — DEVE usar `timeout 60 duckduckgo-search-cli --no-warmup "query" -q -f json`
- `--no-cookie-persistence` — cookies apenas em memória — DEVE usar `timeout 60 duckduckgo-search-cli --no-cookie-persistence "query" -q -f json`
- `--cookies-path <PATH>` — redireciona jar para volume encriptado — DEVE usar `timeout 60 duckduckgo-search-cli --cookies-path /secure/cookies.json "query" -q -f json`
- `--chrome-path <PATH>` — caminho manual do binário Chrome/Chromium — DEVE usar `timeout 60 duckduckgo-search-cli --chrome-path /usr/bin/chromium "query" -q -f json`
- `--match-platform-ua` — forçar UA correspondente à plataforma do sistema — DEVE usar `timeout 60 duckduckgo-search-cli --match-platform-ua "query" -q -f json`
- `--proxy <URL>` — proxy HTTP/HTTPS/SOCKS5 (ex: socks5://host:port, http://user:pass@host:port); mutuamente exclusivo com `--no-proxy` — DEVE usar `timeout 60 duckduckgo-search-cli --proxy socks5://127.0.0.1:1080 "query" -q -f json`
- `--no-proxy` — desabilita proxy (ignora `--proxy` e env vars HTTP_PROXY/HTTPS_PROXY/ALL_PROXY) — DEVE usar `timeout 60 duckduckgo-search-cli --no-proxy "query" -q -f json`
- `--queries-file <PATH>` — arquivo com queries para batch (uma por linha) — DEVE usar `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --num 15`
- `--parallel N` — queries em paralelo (clamp [1, 20], default 1; manter ≤ 5 para não triggerar anti-bot) — DEVE usar `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--per-host-limit N` — limite por host (clamp [1, 10]; manter ≤ 2 para não disparar HTTP 202 anti-bot) — DEVE usar `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- `--fetch-content` — extrai conteúdo HTML das páginas — DEVE usar `timeout 120 duckduckgo-search-cli "query" -q -f json --fetch-content --max-content-length 5000`
- `--max-content-length N` — limite de bytes por página extraída (default 10000) — OBRIGATÓRIO com `--fetch-content`, SEMPRE combinar
- `--sub-query-strategy <ESTRATÉGIA>` — `heuristic` (padrão, baixa qualidade) ou `manual` — SEMPRE usar `manual` com `--sub-queries-file`
- `--sub-queries-file <PATH>` — arquivo com sub-queries manuais (uma por linha) — DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--aggregate <MÉTODO>` — método de agregação deep-research — DEVE usar `--aggregate rrf` para Reciprocal Rank Fusion
- `--max-sub-queries N` — limite de sub-queries no deep-research (default 5) — DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --max-sub-queries 5`
- `--synthesize` — gerar síntese no deep-research — DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --synthesize --budget-tokens 2000`
- `--budget-tokens N` — limite de tokens para síntese (default 4000) — SEMPRE combinar com `--synthesize`
- `--synth-format <FORMATO>` — formato de síntese deep-research (`markdown`, `plain-text`, `json`) — DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --synth-format plain-text` — o valor é `plain-text`, NUNCA `plain`
- `--require-results` — exit 4 quando zero resultados no fan-out do deep-research — DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --require-results`
- `--depth <N>` — profundidade de reflexão no deep-research (0 a 3, default 0) — DEVE usar `timeout 180 duckduckgo-search-cli -q -f json deep-research "query" --depth 2`
- `init-config` — subcomando que gera arquivo de configuração TOML padrão — DEVE usar `duckduckgo-search-cli init-config`
- `init-config --force` — sobrescreve arquivos de configuração existentes — DEVE usar `duckduckgo-search-cli init-config --force`
- `init-config --dry-run` — simula geração sem escrever no disco — DEVE usar `duckduckgo-search-cli init-config --dry-run`


## Diagnóstico — ZeroCause, exit codes e cascata anti-bot
- DEVE inspecionar `.metadados.causa_zero` em TODA resposta com `quantidade_resultados == 0`
- 7 causas classificadas — `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`, `vertical-sem-resultados`
- `vertical-sem-resultados` é zero LEGÍTIMO da vertical news (SERP renderizada sem articles) — emite exit 5, NÃO 6
- Quando `causa_zero != "legitimo"`, a CLI emite exit code 6 (`SUSPECTED_BLOCK`) por padrão
- Campo `.metadados.sugestao_proxima_acao` contém instrução PT-BR quando causa não é legítima
- Para restaurar comportamento legado (exit 5 para todos os zeros) — `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`
- Em multi-query (batch), o campo `.causa_zero_histogram` agrega contagens de cada causa entre sub-queries
- Exit codes — `0` sucesso; `1` runtime error; `2` erro de argumento; `3` anti-bot detectado (aguardar 300s, usar `--allow-lite-fallback`); `4` timeout (aumentar `--global-timeout` ou reduzir `--num`); `5` zero resultados legítimos (reformular query ou mudar `--lang`); `6` bloqueio suspeito (inspecionar `.metadados.causa_zero`)
- Cascata anti-bot de 5 níveis (campo `.metadados.nivel_cascata`, 0..=4) — nível 0 mesma identidade, nível 1 mesma família/plataforma diferente, nível 2 família diferente/mesma plataforma, nível 3 famílias e plataformas diferentes com endpoint rebaixado para lite, nível 4 identidade aleatória
- SE `nivel_cascata == 4` observado, DEVE rotacionar proxy ou aguardar 300s antes de retentar


## Contrato JSON — campos garantidos versus opcionais
- GARANTIDOS não-null — `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`
- OPCIONAIS `Option<String>` — `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`, `.metadados.identidade_usada`
- CONDICIONAIS com `--vertical news|all` — `.noticias[].{posicao,titulo,url}` garantidos, `.noticias[].{fonte,data_relativa,thumbnail}` opcionais, `.quantidade_noticias`, `.metadados.vertical_usada`; AUSENTES no modo web (contrato byte-idêntico)
- OPCIONAIS `Option<u32>` — `.metadados.nivel_cascata` (0..=4)
- CONDICIONAIS com `--fetch-content` — `.resultados[].conteudo`, `.resultados[].tamanho_conteudo`, `.resultados[].metado_extracao_conteudo`
- `tamanho_conteudo` reflete o tamanho REAL do texto truncado pelo `--max-content-length`
- Campos Chrome — `.metadados.usou_chrome` (bool), `.metadados.tentou_chrome` (bool)
- Campos diagnóstico — `.metadados.causa_zero` (enum kebab-case), `.metadados.sugestao_proxima_acao` (string PT-BR)
- Campos compressão — `.metadados.bytes_brutos` (Option<u64>), `.metadados.bytes_descomprimidos` (Option<u64>)
- Campos pre-flight — `.metadados.pre_flight_disparado` (bool), `.metadados.endpoint_usado` ("html" | "lite")
- Campos compat — `.metadados.quantidade_resultados`, `.metadados.endpoint_usado` e `.metadados.nivel_cascata` existem em AMBOS os níveis raiz e metadados
- Deep-research — `.query` (string, top-level do envelope), `.sintese` (Markdown), `.metadados.sub_queries[]`, `.resultados[].titulo` (consistente com busca normal via serde rename)
- News do deep-research — `.noticias[]` SEMPRE presente (vazio com zero ou `--no-news`) com `posicao`, `titulo`, `url`, `score`, `ocorrencias` garantidos e `fonte`, `data_relativa`, `thumbnail` opcionais; `.quantidade_noticias` e `.metadados.total_noticias_unicas` SEMPRE presentes; `.metadados.sub_queries[].{quantidade_noticias,news_indisponivel}` opcionais
- SEMPRE distinguir roots — `.resultados[]` (single-query), `.buscas[]` (multi-query), `.resultados[]` (deep-research)
- Identidade — formato `<family>-<platform>-<16hex>` onde 16hex são os primeiros 16 chars do hash derivado do seed


## Deep-research dual web+news
- DEVE gerar 3-5 sub-queries específicas — NUNCA depender da estratégia heurística padrão
- SEMPRE usar `--sub-query-strategy manual --sub-queries-file` com perguntas geradas pela LLM
- Cada sub-query DEVE atingir aspecto distinto — arquitetura, benchmarks, pricing, limitações, comparações
- O output inclui `.query` (query original no top-level), `.sintese` (Markdown), `.metadados.sub_queries[]` (status por sub-query), `.resultados[]` (agregado via RRF)
- Campo `.resultados[].titulo` é consistente com busca normal (serde rename aplicado)
- COMBINE com `--pre-flight` para ambientes bloqueados
- O deep-research varre a vertical news por PADRÃO — cada sub-query roda `--vertical all` na própria sessão Chrome — DEVE usar `timeout 180 duckduckgo-search-cli -q -f json deep-research "query" | jaq '.noticias[:5]'` para artigos frescos agregados
- O RRF de news é SEPARADO do RRF web — NUNCA compare `.noticias[].score` com `.resultados[].score`; dedupe por URL canônica, empates desfeitos por recência (`data_relativa` fica verbatim no JSON)
- Com `--synthesize` o relatório é dual — seção web ~70% do `--budget-tokens`, seção "Notícias recentes" ~30%; inalterado com `--no-news` ou zero notícias
- Exit codes deep-research — `0` quando web OU news produziram resultados; `5` somente quando AMBOS estão vazios
- DEVE passar `--no-news` em ambientes sem Chrome (CI) — caso contrário exit 2 antes do fan-out
- USAR `--require-results` para abortar com exit 4 quando NENHUMA sub-query retornar resultados
- USAR `--depth N` para controlar profundidade de reflexão (0 a 3, default 0)


## Regras PROIBIDAS
- PROIBIDO `-f text` ou `-f markdown` para parsing programático — SEMPRE `-f json`
- PROIBIDO omitir `-q` em pipelines — tracing de stderr polui stdout
- PROIBIDO `--stream` — flag reservada, SEM implementação
- PROIBIDO hardcodar API keys, proxies ou User-Agents
- PROIBIDO hardcodar `--identity-profile` em CI — deixar pool de 12 identidades adaptar
- PROIBIDO `--output` com `..` ou diretórios de sistema (`/etc`, `/usr`, `C:\Windows`)
- PROIBIDO tratar `identidade_usada` ou `nivel_cascata` como garantidos — ambos são `Option<T>`
- PROIBIDO commitar `cookies.json` — arquivo adjacente a credencial
- PROIBIDO ignorar `quantidade_resultados:0` — pode ser ghost-block (usar `--pre-flight` ou inspecionar `causa_zero`)
- PROIBIDO ignorar exit code 6 — indica bloqueio suspeito que requer ação
- PROIBIDO `--proxy` com credenciais em logs ou outputs visíveis
- PROIBIDO loops de retry em shell — usar `--retries` nativo
- PROIBIDO combinar `--proxy` com `--no-proxy` — mutuamente exclusivos


## Segurança e variáveis de ambiente
- Caminho do cookie jar — `~/.config/duckduckgo-search-cli/cookies.json` (modo Unix `0o600`)
- NUNCA logar ou ecoar conteúdo dos cookies
- NUNCA passar `--cookies-path` para volumes não encriptados em produção
- DEVE usar `--no-cookie-persistence` para sessões efêmeras
- `DUCKDUCKGO_CHROME_HEADLESS=1` — forçar modo headless (risco de detecção Cloudflare)
- `DUCKDUCKGO_CHROME_VISIBLE=1` — modo headed visível (depuração)
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` — restaurar exit 5 legado para todos os zeros (valores aceitos — false, 0, no, off, string vazia)
- `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` — proxy via variáveis de ambiente (respeitados automaticamente, desabilitados com `--no-proxy`)


## Pré-requisitos de build e runtime
- BUILD requer APENAS toolchain Rust — ZERO dependências nativas de compilação (reqwest+rustls-tls elimina cmake, nasm, perl)
- `cargo install duckduckgo-search-cli --locked --force` funciona em Linux, macOS e Windows SEM ferramentas extras
- RUNTIME Linux requer Google Chrome ou Chromium + Xvfb (auto-instalado pela CLI em 22+ distros)
- RUNTIME macOS e Windows requer Google Chrome ou Chromium (Xvfb não necessário)
- A CLI tenta auto-instalar Xvfb quando não encontrado — output do gerenciador de pacotes é exibido em tempo real
- Mensagens de instrução manual são exibidas via stderr SEMPRE visíveis, independente de `-q` ou nível de log
