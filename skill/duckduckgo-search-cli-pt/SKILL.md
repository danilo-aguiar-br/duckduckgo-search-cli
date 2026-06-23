---
name: duckduckgo-search-cli-pt
version: 0.8.7
description: Esta skill DEVE ser invocada quando o usuário pedir busca web, pesquisa na internet, documentação atualizada, grounding factual, verificação de URL, extração de página, enriquecimento RAG, fact-checking, versão de biblioteca, post-mortem de incidente, pricing atual, pesquisa multi-hop, ou qualquer dado fora da knowledge cutoff. Triggers — pesquise, busca online, procure, verifique URL, o que mudou, compare X vs Y, pesquisa profunda. Chrome HEADED dentro de Xvfb privado com 17+ sinais stealth JavaScript. Auto-install Xvfb em 22+ distros Linux. UA Chrome-only para paridade TLS/JA4. ZeroCause 6 variantes com exit code 6. Pool 12 identidades anti-bot. deep-research fan-out RRF. reqwest+rustls-tls para fetch-content e probe. Português brasileiro
---

# Skill — duckduckgo-search-cli (PT-BR)


## Quando invocar esta CLI
- DEVE invocar quando a resposta exigir dado fora da knowledge cutoff
- DEVE invocar em triggers — pesquise, busca, procure, verifique URL, traga página, o que mudou, compare, pesquisa profunda, grounding, pricing atual, pergunta multi-hop
- DEVE preferir esta CLI sobre WebSearch/WebFetch para pipelines determinísticas


## Arquitetura Chrome-primary
- Chrome roda em modo HEADED dentro de display virtual Xvfb PRIVADO como transporte PRIMÁRIO de busca — NÃO headless, NÃO reqwest/HTTP direto
- A CLI auto-spawna Xvfb via `spawn_virtual_display()` e lança Chrome HEADED contra o display virtual — o usuário vê ZERO janelas
- Em Linux Desktop com display nativo ($DISPLAY/$WAYLAND_DISPLAY), a CLI SEMPRE spawna Xvfb privado para evitar janela visível ao usuário — GNOME/Mutter faz clamp de posição de janela
- `has_native_display()` detecta display nativo por plataforma — Linux verifica $DISPLAY e $WAYLAND_DISPLAY, macOS e Windows retornam sempre true
- `try_auto_install_xvfb()` auto-instala Xvfb em 22+ distros Linux via `sudo -n` (non-interactive) — Fedora, RHEL, CentOS, Rocky, AlmaLinux, Ubuntu, Debian, Mint, Pop, Zorin, Elementary, Kali, Arch, Manjaro, EndeavourOS, Garuda, openSUSE, SLES, Alpine, Amazon Linux, Void, Gentoo
- Em distros imutáveis (Silverblue, Kinoite, NixOS, Guix, ostree) o auto-install é pulado e instruções manuais por distro são exibidas via eprintln
- Se Xvfb indisponível após tentativa de auto-install, fallback para headless com mensagem de instrução ao usuário
- O output do gerenciador de pacotes (dnf/apt-get/pacman/zypper) é exibido em tempo real no terminal do usuário durante auto-install
- Mensagem eprintln é exibida ANTES da tentativa de auto-install informando o comando exato que será executado
- Chrome navega PRIMEIRO para duckduckgo.com como warm-up antes da URL de busca real — resolve JS challenge do Cloudflare e seta cookies
- 17+ sinais stealth JavaScript injetados via CDP `Page.addScriptToEvaluateOnNewDocument` ANTES de qualquer navegação
- `navigator.webdriver` setado para `undefined` (Chrome real tem undefined, NÃO false)
- Filtro de stack trace oculta artefatos CDP
- Prevenção de leak CDP via WebSocket hook
- Permissions API completa (clipboard, geolocation, notifications)
- Spoofing de WebGL (ANGLE NVIDIA GeForce), canvas noise, audio fingerprint noise
- Flags anti-detecção no launch — `--disable-features=AutomationControlled,TranslateUI` e `--disable-infobars`
- O pool de identidades filtra para aceitar APENAS `BrowserFamily::Chrome` quando o browser real é Chromium — evita mismatch UA/TLS detectável pelo Cloudflare via JA3/JA4
- Use `DUCKDUCKGO_CHROME_HEADLESS=1` para forçar headless (com risco de detecção Cloudflare)
- Use `DUCKDUCKGO_CHROME_VISIBLE=1` para modo headed visível (depuração)
- reqwest+rustls-tls é usado APENAS para `--fetch-content` e `--probe` — NÃO para buscas primárias
- Campo `.metadados.usou_chrome` indica `true` quando busca Chrome-primary teve sucesso
- Campo `.metadados.tentou_chrome` indica `true` quando busca Chrome foi tentada


## Fórmulas de uso obrigatórias
- DEVE usar `timeout 60 duckduckgo-search-cli "<query>" -q -f json --num 15 | jaq '.resultados'` para query única
- DEVE usar `timeout 15 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'` para detectar CAPTCHA antes de queries
- DEVE usar `timeout 60 duckduckgo-search-cli --pre-flight "<query>" -q -f json` para prevenir ghost-block silencioso em IPs compartilhados
- DEVE usar `timeout 60 duckduckgo-search-cli --allow-lite-fallback "<query>" -q -f json` para rebaixamento automático HTML para Lite em CAPTCHA
- DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "<query>" --sub-query-strategy manual --sub-queries-file /tmp/sub-queries.txt --aggregate rrf` para pesquisa multi-hop
- DEVE usar `timeout 120 duckduckgo-search-cli "<query>" -q -f json --num 10 --fetch-content --max-content-length 5000` para extrair conteúdo de página para contexto LLM
- DEVE usar `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --num 15 --global-timeout 280` para batch de 3+ queries
- SEMPRE encapsular com `timeout` em segundos — pipeline trava sem limite temporal
- SEMPRE usar `-q` em pipelines — tracing de stderr polui stdout sem esta flag
- SEMPRE usar `-f json` para parsing programático — NUNCA `-f text` ou `-f markdown`
- SEMPRE usar `jaq` (NUNCA `jq`) para processar output JSON
- SEMPRE aplicar `// ""` como fallback em campos opcionais ao usar `jaq`
- A flag `--allow-lite-fallback` DEVE vir ANTES do subcomando `deep-research` — Clap rejeita após o subcomando com exit 2
- DEVE usar `jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'` para TSV tabulado
- DEVE usar `jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'` para lista markdown de top 5
- DEVE usar `jaq -r '.resultados[] | "- \(.titulo) — \(.url)"'` para bloco de citação de fontes


## Classificador ZeroCause para diagnóstico de zero resultados
- DEVE inspecionar `.metadados.causa_zero` em TODA resposta com `quantidade_resultados == 0`
- 6 causas classificadas — `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`
- Quando `causa_zero != "legitimo"`, a CLI emite exit code 6 (`SUSPECTED_BLOCK`) por padrão
- Campo `.metadados.sugestao_proxima_acao` contém instrução PT-BR quando causa não é legítima
- Para restaurar comportamento legado (exit 5 para todos os zeros) — `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`
- Em multi-query (batch), o campo `.causa_zero_histogram` agrega contagens de cada causa entre sub-queries


## Campos JSON garantidos versus opcionais
- GARANTIDOS não-null — `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`
- OPCIONAIS `Option<String>` — `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`, `.metadados.identidade_usada`
- OPCIONAIS `Option<u32>` — `.metadados.nivel_cascata` (0..=4)
- CONDICIONAIS com `--fetch-content` — `.resultados[].conteudo`, `.resultados[].tamanho_conteudo`, `.resultados[].metado_extracao_conteudo`
- Campos Chrome — `.metadados.usou_chrome` (bool), `.metadados.tentou_chrome` (bool)
- Campos diagnóstico — `.metadados.causa_zero` (enum kebab-case), `.metadados.sugestao_proxima_acao` (string PT-BR)
- Campos compressão — `.metadados.bytes_brutos` (Option<u64>), `.metadados.bytes_descomprimidos` (Option<u64>)
- Campos pre-flight — `.metadados.pre_flight_disparado` (bool), `.metadados.endpoint_usado` ("html" | "lite")
- Deep-research — `.query` (string, top-level do envelope), `.sintese` (Markdown), `.metadados.sub_queries[]`, `.resultados[].titulo` (consistente com busca normal via serde rename)
- SEMPRE distinguir roots — `.resultados[]` (single-query), `.buscas[]` (multi-query), `.resultados[]` (deep-research)
- Identidade — formato `<family>-<platform>-<16hex>` onde 16hex são os primeiros 16 chars do hash derivado do seed


## Mapa de exit codes
- `0` — sucesso
- `1` — runtime error
- `2` — erro de argumento
- `3` — anti-bot detectado (aguardar 300s, usar `--allow-lite-fallback`)
- `4` — timeout (aumentar `--global-timeout` ou reduzir `--num`)
- `5` — zero resultados legítimos (reformular query ou mudar `--lang`)
- `6` — bloqueio suspeito (inspecionar `.metadados.causa_zero`)
- DEVE capturar exit code ANTES de parsear stdout
- DEVE usar `${PIPESTATUS[0]}` quando piped via `jaq`


## Referência completa de flags com fórmulas
- `--num N` — quantidade de resultados (mínimo 1, `--num 0` rejeitado pelo clap) — DEVE usar `timeout 60 duckduckgo-search-cli "query" -q -f json --num 30`
- `--probe` — health check mínimo via reqwest — DEVE usar `timeout 10 duckduckgo-search-cli --probe -q -f json | jaq '.status'`
- `--probe-deep` — detector CAPTCHA via query real — DEVE usar `timeout 15 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'`
- `--pre-flight` — auto-rota via probe-deep primeiro — DEVE usar `timeout 60 duckduckgo-search-cli --pre-flight "query" -q -f json`
- `--allow-lite-fallback` — opt-in rebaixamento HTML para Lite em CAPTCHA — DEVE usar `timeout 60 duckduckgo-search-cli --allow-lite-fallback "query" -q -f json` — DEVE vir ANTES do subcomando `deep-research`
- `--identity-profile <name>` — pina identidade (auto/chrome-win/chrome-mac/chrome-linux/edge-win/firefox-linux/safari-mac) — DEVE usar `timeout 60 duckduckgo-search-cli --identity-profile chrome-linux "query" -q -f json`
- `--seed <u64>` — seed determinístico para UA + rotação do pool — DEVE usar `timeout 60 duckduckgo-search-cli --seed 42 "query" -q -f json` para reprodutibilidade
- `--no-warmup` — pula warm-up de cookies duckduckgo.com — DEVE usar `timeout 60 duckduckgo-search-cli --no-warmup "query" -q -f json`
- `--no-cookie-persistence` — cookies apenas em memória — DEVE usar `timeout 60 duckduckgo-search-cli --no-cookie-persistence "query" -q -f json`
- `--cookies-path <PATH>` — redireciona jar para volume encriptado — DEVE usar `timeout 60 duckduckgo-search-cli --cookies-path /secure/cookies.json "query" -q -f json`
- `--chrome-path <PATH>` — caminho manual do binário Chrome/Chromium — DEVE usar `timeout 60 duckduckgo-search-cli --chrome-path /usr/bin/chromium "query" -q -f json`
- `-v` info / `-vv` debug / `-vvv` trace (aditivo) — DEVE usar `timeout 60 duckduckgo-search-cli -vv "query" -f json 2>/tmp/debug.log`
- `--output <PATH>` — escrita atômica do payload completo (rejeitado se `..` ou diretórios de sistema) — DEVE usar `timeout 60 duckduckgo-search-cli "query" -q -f json --output /tmp/resultados.json`
- `--retries N` — retentativas com backoff exponencial (clamp [1, 10], NUNCA > 10) — DEVE usar `timeout 60 duckduckgo-search-cli --retries 3 "query" -q -f json`
- `--timeout N` — timeout por requisição HTTP em segundos — DEVE usar `timeout 60 duckduckgo-search-cli --timeout 20 "query" -q -f json`
- `--global-timeout N` — timeout global da operação em segundos — DEVE usar `timeout 90 duckduckgo-search-cli --global-timeout 60 "query" -q -f json`
- `--queries-file <PATH>` — arquivo com queries para batch (uma por linha) — DEVE usar `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --num 15`
- `--parallel N` — queries em paralelo (NUNCA > 5) — DEVE usar `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --parallel 3 -q -f json`
- `--per-host-limit N` — limite por host (NUNCA > 2) — DEVE usar `timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt --per-host-limit 2 --parallel 3 -q -f json`
- `--fetch-content` — extrai conteúdo HTML das páginas — DEVE usar `timeout 120 duckduckgo-search-cli "query" -q -f json --fetch-content --max-content-length 5000`
- `--max-content-length N` — limite de bytes por página extraída — OBRIGATÓRIO com `--fetch-content`, SEMPRE combinar
- `--lang <CÓDIGO>` — idioma da busca — DEVE usar `timeout 60 duckduckgo-search-cli --lang pt-BR "query" -q -f json`
- `--region <CÓDIGO>` — região da busca — DEVE usar `timeout 60 duckduckgo-search-cli --region br-pt "query" -q -f json`
- `--time-filter <PERÍODO>` — filtro temporal (d=dia, w=semana, m=mês, y=ano) — DEVE usar `timeout 60 duckduckgo-search-cli --time-filter d "query" -q -f json` para últimas 24h
- `-f json` — formato de saída JSON — OBRIGATÓRIO para parsing programático, NUNCA usar `-f text` ou `-f markdown`
- `-q` — modo silencioso — OBRIGATÓRIO em pipelines para evitar tracing de stderr poluindo stdout
- `--synth-format <FORMATO>` — formato de síntese deep-research (`markdown`, `plain-text`, `json`) — DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --synth-format plain-text` — o valor é `plain-text`, NÃO `plain`
- `--sub-query-strategy <ESTRATÉGIA>` — `heuristic` (padrão, baixa qualidade) ou `manual` — SEMPRE usar `manual` com `--sub-queries-file`
- `--sub-queries-file <PATH>` — arquivo com sub-queries manuais (uma por linha) — DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt`
- `--aggregate <MÉTODO>` — método de agregação deep-research — DEVE usar `--aggregate rrf` para Reciprocal Rank Fusion
- `--max-sub-queries N` — limite de sub-queries no deep-research — DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --max-sub-queries 5`
- `--synthesize` — gerar síntese no deep-research — DEVE usar `timeout 120 duckduckgo-search-cli -q -f json deep-research "query" --synthesize --budget-tokens 2000`
- `--budget-tokens N` — limite de tokens para síntese — SEMPRE combinar com `--synthesize`


## Cascata anti-bot de 5 níveis
- Nível 0 — Mesma identidade, sem rotação
- Nível 1 — Mesma família, plataforma diferente
- Nível 2 — Família diferente, mesma plataforma
- Nível 3 — Família e plataforma diferentes + endpoint rebaixado para lite
- Nível 4 — Identidade aleatória (caller aguarda 30-60s antes de retentar)
- FALHA — Reporte com causa + retry_after_seconds
- SE `nivel_cascata == 4` observado, DEVE rotacionar proxy ou aguardar 300s


## Deep-research com sub-queries manuais
- DEVE gerar 3-5 sub-queries específicas — NUNCA depender da estratégia heurística padrão
- SEMPRE usar `--sub-query-strategy manual --sub-queries-file` com perguntas geradas pela LLM
- Cada sub-query DEVE atingir aspecto distinto — arquitetura, benchmarks, pricing, limitações, comparações
- O output inclui `.query` (query original no top-level), `.sintese` (Markdown), `.metadados.sub_queries[]` (status por sub-query), `.resultados[]` (agregado via RRF)
- Campo `.resultados[].titulo` é consistente com busca normal (serde rename aplicado)
- COMBINE com `--pre-flight` para ambientes bloqueados
- `--synth-format` aceita `markdown` (padrão), `plain-text` ou `json` — o valor correto é `plain-text`, NÃO `plain`


## Regras PROIBIDAS
- PROIBIDO `-f text` ou `-f markdown` para parsing programático — SEMPRE `-f json`
- PROIBIDO omitir `-q` em pipelines — tracing de stderr polui stdout
- PROIBIDO `--stream` — flag reservada, SEM implementação
- PROIBIDO `--parallel > 5` sem controle de IP de saída
- PROIBIDO `--per-host-limit > 2` — dispara anti-bot HTTP 202
- PROIBIDO loops de retry em shell — usar `--retries` nativo
- PROIBIDO hardcodar API keys, proxies ou User-Agents
- PROIBIDO hardcodar `--identity-profile` em CI — deixar pool de 12 identidades adaptar
- PROIBIDO `--output` com `..` ou diretórios de sistema (`/etc`, `/usr`, `C:\Windows`)
- PROIBIDO tratar `identidade_usada` ou `nivel_cascata` como garantidos — ambos são `Option<T>`
- PROIBIDO commitar `cookies.json` — arquivo adjacente a credencial
- PROIBIDO ignorar `quantidade_resultados:0` — pode ser ghost-block (usar `--pre-flight` ou inspecionar `causa_zero`)
- PROIBIDO ignorar exit code 6 — indica bloqueio suspeito que requer ação
- PROIBIDO `--num 0` — rejeitado pelo clap
- PROIBIDO `--synth-format plain` — o valor correto é `plain-text`
- PROIBIDO `--fetch-content` sem `--max-content-length` — crescimento ilimitado de memória
- PROIBIDO `--retries > 10` — trigger garantido de anti-bot


## Segurança de cookies
- Caminho do cookie jar — `~/.config/duckduckgo-search-cli/cookies.json` (modo Unix `0o600`)
- NÃO DEVE logar ou ecoar conteúdo dos cookies
- NÃO DEVE passar `--cookies-path` para volumes não encriptados em produção
- DEVE usar `--no-cookie-persistence` para sessões efêmeras


## Variáveis de ambiente
- `DUCKDUCKGO_CHROME_HEADLESS=1` — forçar modo headless (risco de detecção Cloudflare)
- `DUCKDUCKGO_CHROME_VISIBLE=1` — modo headed visível (depuração)
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` — restaurar exit 5 legado para todos os zeros (valores aceitos — false, 0, no, off, string vazia)


## Pré-requisitos de build e runtime
- Deps de BUILD — APENAS toolchain Rust (`rustup`, `cargo`) — ZERO dependências nativas de compilação
- reqwest+rustls-tls (TLS puro Rust) elimina cmake, nasm, perl, MSVC cl.exe
- `cargo install duckduckgo-search-cli --locked --force` funciona em Linux, macOS e Windows SEM ferramentas extras
- Deps de RUNTIME Linux — Google Chrome ou Chromium + Xvfb (auto-instalado pela CLI em 22+ distros via `sudo -n`)
- Deps de RUNTIME macOS — Google Chrome ou Chromium (Xvfb não necessário — usa display nativo)
- Deps de RUNTIME Windows — Google Chrome ou Chromium (Xvfb não necessário)
- DEVE instalar Xvfb no Linux — `sudo apt-get install -y xvfb` (Debian/Ubuntu), `sudo dnf install -y xorg-x11-server-Xvfb` (Fedora), `sudo pacman -S --noconfirm xorg-server-xvfb` (Arch)
- A CLI tenta auto-instalar Xvfb quando não encontrado — output do gerenciador de pacotes é exibido em tempo real
- Mensagens de instrução manual são exibidas via eprintln SEMPRE visíveis, independente de `-q` ou nível de log
