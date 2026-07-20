# duckduckgo-search-cli — Instruções para Agentes

[English](AGENTS.md)

## Regra Zero
- Leia este documento INTEGRALMENTE antes de invocar `duckduckgo-search-cli`.
- TODAS as suas invocações DEVEM estar em TOTAL conformidade com as regras aqui.
- Violações resultam em erros de execução, pipelines bloqueados e perda de resultados.
- A Regra Zero se aplica a cada chamada, cada script, cada pipeline, sem exceção.


## Visão Geral
- `duckduckgo-search-cli` é uma CLI Rust para busca DuckDuckGo via Chrome/CDP (produção Chrome-only desde a v0.9.4, GAP-WS-113).
- Projetada para consumo por LLMs e agentes de IA em pipelines automatizados.
- Saída estruturada em JSON, Markdown, texto simples ou TSV.
- Códigos de saída são semanticamente definidos para tratamento preciso de erros.
- Versão: **v1.0.1** (Pass 52: multi-query `--stream` / `-f ndjson` linhas NDJSON SearchOutput; API dual de `config` get/set/unset + `config effective`; broken-pipe exit **141** com SIG_IGN SIGPIPE para o reap oneshot rodar; wire PT serialize BC + aliases EN no deserialize ADR-0023; deep-research `-o` + timeout JSON agent-stable; TSV; `man`/`config` XDG; depth reflection) — MSRV: Rust 1.88.


## Instalação
- Instale via Cargo: `cargo install duckduckgo-search-cli`
- Verifique a instalação: `duckduckgo-search-cli --version`
- Atualize para a versão mais recente: `cargo install duckduckgo-search-cli --force`


## Início Rápido
- OBRIGATÓRIO: SEMPRE envolva com `timeout` e passe `-q -f json`:

```bash
timeout 30 duckduckgo-search-cli -q -f json --num 15 "rust async runtime"
```

- Processe a saída com `jaq` — JAMAIS com `jq` ou ferramentas de texto:

```bash
timeout 30 duckduckgo-search-cli -q -f json --num 10 "consulta" | jaq -r '.resultados[].url'
```

- Verifique o exit code ANTES de parsear:

```bash
timeout 60 duckduckgo-search-cli -q -f json --num 15 "consulta" > /tmp/out.json
case $? in
  0) jaq '.resultados[].url' /tmp/out.json ;;
  3) echo "bloqueado, aguardando 300s"; sleep 300 ;;
  4) echo "timeout global, aumente --global-timeout" ;;
  5) echo "zero resultados, reformule a consulta" ;;
  6) echo "bloqueio suspeito, inspecione .metadados.causa_zero" ;;
  *) echo "erro inesperado" ;;
esac
```


## Referência de Flags
- `-q, --quiet` — silencia logs de tracing; stdout carrega apenas o payload
- `-f, --format <FORMAT>` — formato de saída: `json` (OBRIGATÓRIO em scripts), `markdown`, `text`, `tsv`, `ndjson` (alias do modo stream multi-query — igual a `--stream`), `auto`
- `-n, --num <N>` — número de resultados por página (padrão 15, máximo 30)
- `--pages <N>` — número de páginas a buscar (padrão 2, auto-paginação)
- `--parallel <N>` — requisições concorrentes em multi-query (DEVE ser ≤ 5)
- `--queries-file <FILE>` — arquivo com uma consulta por linha para modo lote
- `--fetch-content` / `--no-fetch-content` — fetch de conteúdo **LIGADO por padrão** desde a v0.9.8 (top URLs web + news, teto 10; latência N×). Opt-out com `--no-fetch-content`; `--fetch-content` continua válida como opt-in explícito
- Caminho thin-web rápido: `--vertical web --no-fetch-content` (só metadados da SERP; sem dual news, sem fetch de corpo)
- `--max-content-length <N>` — limite de bytes buscados por página (recomendado sempre que o fetch estiver ligado)
- `-o, --output <FILE>` — grava o payload atomicamente em arquivo com validação de caminho (busca **e** `deep-research`); quando definido, stdout fica vazio
- `--config-home <PATH>` — sobrescreve o diretório de config XDG/plataforma (selectors, cookies, `config.toml`); **sem** env de produto para home
- `--endpoint <html|lite>` — endpoint de busca (padrão `html`; SERP de produção é HTML via Chrome apenas — **não** use `lite` como remediação de exit 3, GAP-WS-113)
- `--vertical <web|news|all>` — vertical de busca (**padrão `all` desde a v0.9.8**). Opt-out com `--vertical web`. `news`/`all` são Chrome-only (SEM fallback HTTP); batches multi-query aceitos desde o GAP-WS-105 (uma sessão Chrome por query); o `deep-research` varre news por PADRÃO (opt-out `--no-news`); sem Chrome utilizável (binário ausente ou build sem feature `chrome`) a produção **falha fechada com exit 2** — sem auto `--no-news`, sem rebaixamento para web (v0.9.4, GAP-WS-113); Chrome é exigido via feature/build, **não** via kill-switch de env em runtime; `--pre-flight` é pulado na vertical de notícias
- `--chrome-path <PATH>` — flag de transporte global (funciona antes ou depois de `deep-research`); resolução multi-canal Flatpak no Linux (shell de export → ELF de deploy)
- `--global-timeout <SEGS>` — timeout total em segundos para todas as consultas (DEVE ser < `timeout` externo). No `deep-research`, se o valor estiver abaixo da estimativa conservadora de workload, a CLI emite **aviso de budget no stderr** (eleve o timeout, use `--no-fetch-content` / `--no-news`, ou reduza `--max-sub-queries`)
- `--per-host-limit <N>` — máximo de requisições concorrentes por host (padrão 2, NÃO exceder 2)
- `--retries <N>` — número de tentativas com backoff exponencial (padrão 2)
- `--timeout <SEGS>` — timeout por requisição em segundos
- `--proxy <URL>` — única fonte residual de proxy HTTP (flag CLI). Proxy se configura **somente** via CLI `--proxy` / `--no-proxy` e XDG `config set proxy_url` — **nunca** herda `HTTP_PROXY` / `HTTPS_PROXY`
- Prefira URLs de proxy sem segredo em argv; persista com `config set proxy_url …` (XDG) quando necessário
- `--no-proxy` — ignora toda configuração de proxy (CLI + XDG)
- `--lang <LANG>` — filtro de idioma (ex.: `en-us`, `pt-br`)
- `--country <CC>` — filtro de país (ex.: `us`, `br`)
- `--time-filter <d|w|m>` — filtro de tempo: dia, semana, mês
- `--stream` — apenas multi-query: emite NDJSON por query (`SearchOutput`) conforme cada busca completa. Em query única a flag é **ignorada** (warning). Não é stream completo de hits individuais da SERP. `-f ndjson` é alias do modo stream
- `-v, --verbose` — saída detalhada para diagnóstico; desde v0.7.8 aceita múltiplas ocorrências via `ArgAction::Count` (`-v` = debug, `-vv`+ = trace). Filtro de log de produto é **CLI `-v`/`-q` + XDG `log_directive` apenas** (precedência: `-q` > `-v`/`-vv` > XDG `log_directive` > padrão `info`). **Não** ensine `RUST_LOG` como config de produto
- `config path|list|get|set|unset|effective` — persistência XDG em `config.toml` (sem env de produto). API dual para get/set/unset: **posicional** (`config get KEY`, `config set KEY VALUE`, `config unset KEY`) **ou** flags (`config get --key KEY`, `config set --key KEY --value VALUE`, `config unset --key KEY`). `config effective` mostra o merge CLI > XDG > padrões. Chaves: `ui_lang`, `chrome_path`, `proxy_url`, `default_global_timeout`, `default_vertical`, `fetch_content_default`, `log_directive`, `default_lang`, `default_country`
- `deep-research <QUERY>` — subcomando de fan-out de queries (v0.7.0); honra `-o` global (arquivo atômico, stdout vazio) e `--global-timeout` global
- `--max-sub-queries <N>` — máximo de sub-queries produzidas (1..=12, padrão 5)
- `--sub-query-strategy <heuristic|manual>` — estratégia de geração de sub-queries
- `--sub-queries-file <PATH>` — lê sub-queries explícitas (estratégia manual)
- `--aggregate <rrf|dedupe-by-url>` — algoritmo de agregação
- `--depth <N>` — rounds de reflexão heurística (0..=3); cada round gera follow-ups a partir de títulos/snippets
- `--synthesize` — produz relatório final em Markdown/PlainText/JSON
- `--budget-tokens <N>` — orçamento de tokens para o relatório de síntese
- `--synth-format <markdown|plain-text|json>` — formato de saída da síntese
- `--probe-deep` — executa uma query real e classifica o body como `ok` ou `captcha` (v0.7.3+)
- `--no-warmup` — pula o warm-up `GET https://duckduckgo.com/` antes da primeira query real (v0.7.3+)
- `--no-cookie-persistence` — mantém cookies em memória apenas; nunca grava `cookies.json` em disco (v0.7.3+)
- `--cookies-path <PATH>` — sobrescreve o path XDG padrão do cookie jar (v0.7.3+)
- `--allow-lite-fallback` — **no-op legado** desde a v0.9.4 (GAP-WS-113); mantido por compatibilidade da CLI, não força Lite nem remedia exit 3


## Códigos de Saída
| Código | Significado | Ação do Agente |
|--------|-------------|----------------|
| `0` | Sucesso | Parsear `.resultados` |
| `1` | Erro de runtime | Ler stderr; retry único com `-v` |
| `2` | Erro de configuração **ou** Chrome ausente / binário sem feature `chrome` | Corrigir args; instalar Chrome ou rebuild com `--features chrome` (padrão); `init-config --force` se necessário |
| `3` | Bloqueio anti-bot | Aguardar 300+ s; rotacionar proxy/identidade; reexecutar `--probe-deep` (Chrome). **Não** confiar em `--allow-lite-fallback` (no-op desde v0.9.4) |
| `4` | Timeout global | Elevar `--global-timeout`; reduzir `--parallel` / workload. No `deep-research`, exit 4 emite envelope JSON com `erro=timeout` e opcional `resultados_parciais` (honra `-o`; stdout vazio com `-o`) |
| `5` | Zero resultados (inclui `vertical-sem-resultados` com `--vertical news`, v0.8.9) | Refinar consulta; tentar diferente `--lang` ou `--country` |
| `6` | Bloqueio suspeito (`causa_zero != legitimo`, v0.8.0+) | Inspecionar `.metadados.causa_zero`; usar `--pre-flight` |
| `130` | Cancelado via **SIGINT** / Ctrl+C (`128+2`) | Não tratar como falha de busca; interrupção do usuário/agente |
| `141` | Broken pipe (consumidor de stdout fechou; `128+SIGPIPE`) | Normal em `| head` / leitor que fecha cedo — não é falha de busca |
| `143` | Cancelado via **SIGTERM** (`128+15`; `timeout`/Docker/systemd) | Parada limpa; perfil Chrome temp reaped de forma cooperativa |


## Invariantes Centrais
### OBRIGATÓRIO — Siga Sempre Sem Exceção
- SEMPRE passe `-q` em todo pipeline que parseia stdout
- SEMPRE especifique `-f json` explicitamente em todo script
- SEMPRE envolva toda invocação com `timeout` usando segundos inteiros
- SEMPRE trate a CLI como **proprietária one-shot de processo + disco** — N invocações sequenciais de agente NÃO DEVEM acumular Chromium/Xvfb **nem** perfis de propriedade `ddg-chrome-*` após saída normal ou cooperativa (processo: v0.9.6 GAP-WS-LIFECYCLE-001; disco: v1.0.0 GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020). No Unix, SIGPIPE permanece **SIG_IGN** para writes broken-pipe virarem EPIPE → exit **141** e `ensure_oneshot_cleanup` ainda rodar
- SEMPRE prefira wrappers externos que enviem **SIGTERM primeiro** (ex.: GNU `timeout`, que manda SIGTERM e depois SIGKILL) em vez de kill imediato só com SIGKILL, para o cancelamento cooperativo e o reap completo da árvore Chromium/Xvfb + perfil rodarem (deep-research herda o `CancellationToken` principal)
- SEMPRE verifique `$?` ou `${PIPESTATUS[0]}` antes de parsear stdout
- SEMPRE fixe `--num` explicitamente; JAMAIS dependa de padrões
- SEMPRE use `--queries-file` para trabalho em lote; JAMAIS loops de shell
- SEMPRE use `jaq` para parsing JSON; JAMAIS `jq` ou ferramentas de texto
- SEMPRE use `--output` para conjuntos grandes (≥ 50 resultados)
- SEMPRE prefira `--endpoint html` (SERP HTML via Chrome); NUNCA remedie exit 3 com `--endpoint lite` ou `--allow-lite-fallback`
- SEMPRE use `--retries` para backoff exponencial; JAMAIS loops de retry no shell
### PROIBIDO — Jamais Viole
- JAMAIS omita `-q` em qualquer invocação em pipe
- JAMAIS assuma que `--stream` é um stream completo de eventos da SERP; é NDJSON multi-query apenas (query única ignora)
- JAMAIS eleve `--parallel` acima de 5
- JAMAIS eleve `--per-host-limit` acima de 2
- JAMAIS passe segredos de proxy de longa duração em argv quando evitável (prefira XDG `config set proxy_url` / flags de curta duração); JAMAIS confie em herança de `HTTP_PROXY` / `HTTPS_PROXY` (não lidas)
- JAMAIS parsear saída `text` ou `markdown` com máquinas
- JAMAIS execute URLs de resultados sem sandbox
- JAMAIS ignore exit codes não-zero
- JAMAIS defina `--global-timeout` igual ou maior que o `timeout` externo
- JAMAIS injete headers `Sec-Fetch-*` ou `Accept-Language` customizados (v0.6.0 os gerencia)
- JAMAIS assuma que Chrome/Xvfb residual ou `ddg-chrome-*` de propriedade após saída cooperativa limpa **1.0.0+** é "normal" — casos residuais são SIGKILL/OOM externo da CLI (a próxima run varre só `ddg-chrome-*`), órfãos de processo históricos pré-0.9.6, ou perfis genéricos `.tmp*` pré-1.0.0 (a CLI nunca faz bulk-rm de `.tmp*` estrangeiros nem de `org.chromium.Chromium.*`)
- JAMAIS apague em massa `/tmp/.tmp*` nem `org.chromium.Chromium.*` como higiene desta CLI na 1.0.0+ — perfis de propriedade são `ddg-chrome-*`; audite com `find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'ddg-chrome-*'`


## Contrato da Saída JSON
### OBRIGATÓRIO — Campos Garantidos Não-Nulos
- `.resultados[].titulo` — sempre presente quando `resultados` é não-vazio
- `.resultados[].url` — sempre presente quando `resultados` é não-vazio
- `.resultados[].posicao` — sempre presente quando `resultados` é não-vazio
- `.quantidade_resultados` — prefira sobre `(.resultados | length)`
- `.metadados.tempo_execucao_ms` — sinal canônico de latência
- `.metadados.usou_endpoint_fallback` — `true` sinaliza degradação da reputação do IP
### OBRIGATÓRIO — Campos Opcionais Exigem Fallbacks
- `.resultados[].snippet` é `Option<String>` — SEMPRE use fallback `// ""`
- `.resultados[].url_exibicao` é `Option<String>` — SEMPRE use fallback `// .url`
- `.resultados[].titulo_original` é `Option<String>` — SEMPRE use fallback `// .titulo`
- Campos de conteúdo (`.conteudo`, `.tamanho_conteudo`) — comuns nos top resultados com fetch ligado (**padrão LIGADO** desde a v0.9.8, teto 10); ausentes com `--no-fetch-content`
- `.metadados.chrome_path_resolvido`, `.metadados.chrome_canal` — campos de contrato agent (**não** telemetria); `.metadados.usou_chrome` honesto
### OBRIGATÓRIO — Campos da Vertical de Notícias (v0.8.9+, padrões v0.9.8)
- `.noticias[].posicao`, `.noticias[].titulo`, `.noticias[].url` — garantidos quando `--vertical news|all` retorna artigos (vertical padrão é **`all`**)
- `.noticias[].fonte`, `.noticias[].data_relativa`, `.noticias[].thumbnail` — `Option<String>`, SEMPRE use fallback `// ""`
- News também pode trazer `conteudo` / `tamanho_conteudo` / `metodo_extracao_conteudo` com fetch ligado (padrão LIGADO)
- `.quantidade_noticias` e `.metadados.vertical_usada` — costumam estar presentes no padrão `all`; omita news com `--vertical web`
- Zero artigos em SERP de notícias renderizada → `causa_zero: vertical-sem-resultados` (zero legítimo, exit 5, NÃO 6)
- **CR4c (GAP-WS-113):** body ≥4KB sem sinal de página de resultados nunca é `causa_zero: legitimo` — trate como `zero-resultados-suspeito` / exit 6
- **Honestidade anti-bot news (GAP-E2E-51-006):** news isolada faz prime de sessão via SERP web e retenta extratos vazios/interstitial com backoff full-jitter (`--retries`). Se o DDG ainda bloquear, espere exit estruturado **6** com `metadados.causa_zero: anti-bot` e `noticias: []` vazias — **nunca** fake-success. Prefira `timeout` maior / `--proxy` / aguardar 300s; dual `--vertical all` ainda pode retornar web quando news estiver bloqueada.
- Fórmula canônica: `timeout 90 duckduckgo-search-cli --vertical news "query" -q -f json | jaq '.noticias'`
- Preservar envelope fino 0.9.7: `timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content -q -f json "query"`

```bash
jaq '.resultados[] | {
  titulo,
  url,
  snippet: (.snippet // ""),
  url_exibicao: (.url_exibicao // .url)
}'
```

### OBRIGATÓRIO — Raiz JSON Única vs Múltipla
- Raiz de query única: `{ query, resultados, metadados }`
- Raiz de múltiplas queries: `{ quantidade_queries, buscas: [{ query, resultados, metadados }] }`
- JAMAIS acesse `.resultados` diretamente em resposta de múltiplas queries

```bash
# query única
duckduckgo-search-cli -q -f json "uma" | jaq '.resultados | length'
# múltiplas queries
duckduckgo-search-cli -q -f json "uma" "duas" | jaq '.buscas[0].resultados | length'
```


## Rate Limiting e Etiqueta
### OBRIGATÓRIO — Fique Abaixo do Limiar Anti-Bot
- DEVE limitar `--parallel` em 5 (padrão); valores acima de 5 acionam HTTP 202 anti-bot
- DEVE manter `--per-host-limit` em 2 (padrão); valores acima de 2 aumentam probabilidade de bloqueio
- DEVE usar `--retries` interno com backoff exponencial; JAMAIS loops de retry no shell
- DEVE calcular `--global-timeout` como `(consultas / parallel) * média_segs * 1.5`
- Exit code `3` exige janela de backoff de 300+ segundos antes do retry


## Modo Lote
### OBRIGATÓRIO — Use queries-file para Todo Trabalho em Lote
- JAMAIS itere sobre consultas em loops de shell; pague o custo de startup do processo uma vez
- SEMPRE use `--queries-file` para reutilizar pools de conexão e rotação de UA
- SEMPRE defina `--global-timeout` adequado ao tamanho do lote

```bash
printf 'consulta um\nconsulta dois\nconsulta tres\n' > /tmp/q.txt
timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q --parallel 3 -f json
```


## Integridade do Pipe
### OBRIGATÓRIO — Detecte Falha Upstream em Pipes
- Em `cmd | jaq`, o shell reporta apenas o exit code do `jaq`
- DEVE verificar `${PIPESTATUS[0]}` após toda invocação em pipe

```bash
timeout 60 duckduckgo-search-cli "consulta" -q -f json | jaq '.resultados[].url'
ddg_exit=${PIPESTATUS[0]}
if [ "$ddg_exit" -ne 0 ]; then echo "CLI falhou: exit $ddg_exit" >&2; fi
```


## Busca de Conteúdo
### OBRIGATÓRIO — Fetch de Conteúdo LIGADO por Padrão (v0.9.8)
- Fetch de conteúdo está **LIGADO por padrão** (top URLs web + news, FETCH_CAP=10) via **Chrome/CDP** (latência N×; GAP-WS-113 / ADR-0018)
- Opt-out com `--no-fetch-content` quando só precisar de metadados da SERP
- DEVE passar `--max-content-length` para limitar memória quando quiser corpos
- DEVE reduzir `--num` e elevar o `timeout` externo (prefira 120–180s) com fetch ligado

```bash
# Caminho agent-ready padrão (vertical dual + texto limpo)
timeout 180 duckduckgo-search-cli -q -f json --num 5 --max-content-length 5000 "consulta"
# Caminho SERP fina
timeout 60 duckduckgo-search-cli -q -f json --num 5 --vertical web --no-fetch-content "consulta"
```


## Regras de Segurança
### OBRIGATÓRIO — Proteja Credenciais e Execução
- Prefira não colocar credenciais de proxy de longa duração em argv (visíveis em `/proc/*/cmdline`, `ps`, histórico de shell)
- SEMPRE configure proxy via CLI `--proxy` / `--no-proxy` e/ou XDG `config set proxy_url` **SOMENTE** — **nunca** `HTTP_PROXY` / `HTTPS_PROXY` (não herdadas; não são config de produto)
- JAMAIS execute URLs de `.resultados[].url` sem sandbox (risco de SSRF e execução de código)
- SEMPRE execute `init-config --dry-run` antes de `init-config --force` em pipelines de validação local
- CONFIE na validação de caminho do v0.5.0 para `--output`; JAMAIS implemente checks manuais de `realpath`
- CONFIE nos perfis de fingerprint de browser do v0.6.0; JAMAIS injete headers `Sec-Fetch-*` ou `Accept-Language`

```bash
# Proxy via CLI ou config XDG SOMENTE — HTTP_PROXY / HTTPS_PROXY NÃO são lidas
duckduckgo-search-cli -q --proxy http://host:8080 "consulta"
duckduckgo-search-cli config set proxy_url "http://host:8080"
# API dual: posicional OU --key/--value
duckduckgo-search-cli config get proxy_url
duckduckgo-search-cli config get --key proxy_url
duckduckgo-search-cli config set --key proxy_url --value "http://host:8080"
duckduckgo-search-cli config list
duckduckgo-search-cli config unset proxy_url
duckduckgo-search-cli config unset --key proxy_url
duckduckgo-search-cli config path
duckduckgo-search-cli config effective
```

### OBRIGATÓRIO — Ordem de Precedência de Proxy
- `--no-proxy` sobrescreve CLI `--proxy` e XDG `proxy_url`
- `--proxy <URL>` sobrescreve XDG `proxy_url`
- XDG `config set proxy_url …` aplica quando nenhuma flag CLI de proxy está definida
- Nenhum: conexão direta
- `HTTP_PROXY` / `HTTPS_PROXY` **nunca** são lidas


## Anti-Padrões
### PROIBIDO — Padrões Que Quebram Pipelines Silenciosamente
- Parsear saída de texto com `rg` em vez de `jaq` no JSON
- Loops de shell em vez de `--queries-file` para consultas em lote
- Ignorar exit codes antes de passar para `jaq`
- Assumir que `snippet` é não-nulo sem fallback `// ""`
- Hardcodar credenciais de proxy em argv
- Elevar `--parallel` para 20 para aumentar throughput (aciona exit code 3)
- Tratar `--stream` como stream completo de eventos da SERP (só NDJSON multi-query; query única ignorada)
- Confiar em `HTTP_PROXY` / `HTTPS_PROXY` ou `DUCKDUCKGO_SEARCH_CLI_HOME` (nenhuma é config de produto)
- Invocar sem envoltório `timeout` (pipeline trava indefinidamente)
- Definir `--global-timeout` igual ao `timeout` externo (CLI nunca termina limpa)
- Hardcodar `--identity-profile` em vez de deixar o pool rotacionar adaptativamente (v0.6.5+)
- Ler `.metadados.identidade_usada` como garantia quando é `Option<String>` (v0.6.5+)
- Ler `.metadados.nivel_cascata` como garantia quando é `Option<u32>` (v0.6.5+)
- Pular `duckduckgo-search-cli --probe` antes de lançar queries reais em CI


## v0.7.0 — Subcomando Deep Research

### OBRIGATÓRIO — Use o Novo Subcomando para Pesquisa Multi-Hop

Para perguntas que se beneficiam de fan-out de queries ("compare X vs Y em 2026", "história de Z", "o que mudou na biblioteca W"), use o subcomando `deep-research` em vez de rodar uma única busca.

```bash
timeout 60 duckduckgo-search-cli -q -f json deep-research "melhor cliente http rust 2026" \
  | jaq '.resultados[] | {titulo, url, score}'
```

### OBRIGATÓRIO — Schema de Saída do Deep Research

- O JSON de topo tem três chaves: `metadados`, `resultados` e `sintese` opcional
- `.metadados.query_original` é a entrada do usuário
- `.metadados.sub_queries[]` lista cada sub-query gerada com `texto`, `estrategia`, `status`, `elapsed_ms`
- `.metadados.total_resultados_unicos` é a contagem deduplicada
- `.metadados.tempo_total_ms` é a latência end-to-end
- `.resultados[].score` é um valor normalizado `[0.0, 1.0]` — maior é melhor
- `.resultados[].fontes[]` lista as sub-queries que produziram o resultado (rastreabilidade)
- `.sintese` aparece apenas quando `--synthesize` está ativo

```bash
# Extrair o relatório Markdown (quando --synthesize está ativo)
timeout 120 duckduckgo-search-cli -q -f json deep-research "tópico" \
  --synthesize --synth-format markdown | jaq -r '.sintese'
```

### OBRIGATÓRIO — Arquivo de Sub-Queries Manual

Quando `--sub-query-strategy manual` é definido, a CLI lê sub-queries de `--sub-queries-file PATH`. Comentários (`#`) e linhas em branco são ignorados. O arquivo DEVE conter pelo menos uma linha que não seja comentário.

```bash
cat > /tmp/qs.txt <<EOF
# Visão geral
o que é tokio runtime 2026
# Comparação
tokio vs async-std
EOF

timeout 60 duckduckgo-search-cli -q -f json deep-research "tokio 2026" \
  --sub-query-strategy manual --sub-queries-file /tmp/qs.txt
```

### OBRIGATÓRIO — Herda Flags Globais

`deep-research` aceita todas as flags globais (`-n`, `--lang`, `--country`, `--parallel`, `--endpoint`, `--retries`, `--timeout`, `--global-timeout`, `--proxy`, `--fetch-content` / `--no-fetch-content`, `--max-content-length`, `-o/--output`). Os knobs específicos do deep-research são sobrepostos.

### OBRIGATÓRIO — `-o` / `--output` Global no Deep Research

`deep-research` honra `-o FILE` global: o JSON de sucesso (ou de timeout) é escrito atomicamente em `FILE` e **stdout fica vazio**. A segurança de path é a mesma da busca (`..` rejeitado).

```bash
timeout 180 duckduckgo-search-cli -q -f json -o /tmp/dr.json \
  deep-research "rust tokio timeout" --no-fetch-content --max-sub-queries 2 --no-news
# stdout vazio; payload em /tmp/dr.json
```

### OBRIGATÓRIO — Envelope de Timeout Exit 4

Quando `--global-timeout` dispara durante `deep-research`, o processo sai com **4** e emite envelope JSON com `erro=timeout` (mais `mensagem`, `segundos`, `comando`, `tipo`). Se o grace cooperativo colheu trabalho, o envelope pode incluir `resultados_parciais` e `parcial=true`. O envelope honra `-o` (arquivo atômico, stdout vazio).

```bash
timeout 90 duckduckgo-search-cli -q -f json -o /tmp/dr-to.json --global-timeout 3 \
  deep-research "x" --max-sub-queries 6
# exit 4; jaq '.erro, .parcial, .resultados_parciais' /tmp/dr-to.json
```

### OBRIGATÓRIO — Aviso de Budget Quando Timeout Está Abaixo da Estimativa

Antes do fan-out, se `--global-timeout` estiver abaixo da estimativa conservadora de lower-bound do deep-research (SERP + fetch opcional × verticais × sub-queries), a CLI imprime um **aviso no stderr** e continua. Eleve `--global-timeout`, passe `--no-fetch-content` / `--no-news`, ou reduza `--max-sub-queries` / teto de fetch.

### OBRIGATÓRIO — Orçamento de Tokens para Síntese

`--budget-tokens` usa a heurística 1 token ≈ 4 chars. O relatório sintetizado tem teto rígido de 20 referências. Defina `--budget-tokens 0` para desabilitar o teto e contar apenas com o limite de 20 referências.

## v0.6.4 e v0.6.5 — Pool Adaptativo de Identidades Anti-Bot (WS-26)

### OBRIGATÓRIO — Reconhecer as Novas Flags
- `--probe` — verificação de saúde pré-voo. DEVE ser usada em CI antes de lançar queries reais.
- `--identity-profile <auto|chrome-win|chrome-mac|chrome-linux|edge-win|firefox-linux|safari-mac>` — fixa a sessão em uma identidade específica. `auto` (padrão) rotaciona adaptativamente.
- `--seed <u64>` — seed determinístico para seleção de UA E rotação do pool de identidades.

### OBRIGATÓRIO — Ler os Novos Campos de Metadados
- `.metadados.identidade_usada` — `Option<String>` — tag de identidade que produziu a resposta (formato `<família>-<plataforma>-<16hex>`)
- `.metadados.nivel_cascata` — `Option<u32>` (0..=4) — nível de cascata atingido durante a requisição

```bash
# Verifica qual identidade produziu a resposta
timeout 30 duckduckgo-search-cli -q -f json "query" | jaq '.metadados.identidade_usada // "auto"'

# Diagnostica bloqueios repetidos via nível de cascata
timeout 30 duckduckgo-search-cli -q -f json "query" | jaq '.metadados.nivel_cascata // 0'
```

### OBRIGATÓRIO — Estratégia de Cascata Anti-Bot
Quando exit code `3` é encontrado, a CLI já rotacionou por até 5 identidades internamente. Se `--identity-profile auto` está em efeito e exit code `3` persiste, o agente DEVE:
1. Aguardar 300+ segundos antes de retentar (o nível de cascata atingido indica o quão esgotado o pool está)
2. Rotacionar proxy com `--proxy socks5://127.0.0.1:9050` e/ou deixar o pool de identidades adaptar
3. Reexecutar `--probe-deep` (Chrome) para classificar o interstitial
4. Se o problema persistir, abrir bug com o valor de `nivel_cascata` capturado — **não** use `--allow-lite-fallback` (no-op desde v0.9.4)

### OBRIGATÓRIO — Probe Antes de Queries Reais
```bash
# Gate de pipeline CI
timeout 15 duckduckgo-search-cli --probe || { echo "DDG bloqueado em nível de rede, abortando" >&2; exit 1; }
timeout 30 duckduckgo-search-cli -q -f json "query real"
```


## Compilação
- Build de desenvolvimento: `cargo build`
- Build de release: `timeout 600 cargo build --release`
- Verificar compilação: `timeout 120 cargo check --all-targets`
- Alvos de cross-compilation: ver `docs/CROSS_PLATFORM.md`


## Testes
- Executar todos os testes: `timeout 300 cargo nextest run`
- Executar testes de documentação separadamente: `cargo test --doc`
- Executar testes de integração E2E: `timeout 300 cargo test --test integration_pipeline`
- Executar com todas as features: `timeout 300 cargo test --all-features`
- Cobertura mínima: 80% — JAMAIS faça merge abaixo deste limite


## Linting
- Executar Clippy com warnings como erros: `timeout 180 cargo clippy --all-targets --all-features -- -D warnings`
- ZERO warnings são tolerados em código de produção
- Corrija todas as sugestões do Clippy antes de abrir um pull request


## Formatação
- Verificar formatação: `cargo fmt --all --check`
- Aplicar formatação: `cargo fmt --all`
- ZERO diferenças são toleradas em commits
- Execute verificação de formatação no CI antes de qualquer outro gate


## Cobertura
- Executar com relatório texto: `cargo llvm-cov --text`
- Executar com relatório HTML: `cargo llvm-cov --html`
- Meta mínima: 80% de cobertura de linhas
- Recomendado para código novo: 90% de cobertura de linhas
- Gates de cobertura se aplicam a todo pull request sem exceção


## Auditoria
- Verificar vulnerabilidades: `timeout 120 cargo audit`
- Verificar licenças e supply chain: `timeout 120 cargo deny check advisories licenses bans sources`
- ZERO vulnerabilidades são toleradas em releases
- Execute auditoria no CI a cada push para main


## Sequência de Validação Completa
- Execute os 10 comandos abaixo em ordem antes de qualquer release:
- `cargo fmt --all --check` — ZERO diferenças
- `timeout 180 cargo clippy --all-targets --all-features -- -D warnings` — ZERO warnings
- `timeout 120 cargo check --all-targets` — ZERO erros
- `RUSTDOCFLAGS="-D warnings" timeout 120 cargo doc --no-deps --all-features` — ZERO warnings
- `timeout 300 cargo nextest run` — ZERO falhas
- `cargo llvm-cov --text` — mínimo 80% de cobertura
- `timeout 120 cargo audit` — ZERO vulnerabilidades
- `timeout 120 cargo deny check advisories licenses bans sources` — ZERO violações
- `timeout 120 cargo publish --dry-run --allow-dirty` — ZERO erros
- `cargo package --list` — ZERO arquivos sensíveis


## Padrões de Integração com LLMs
### OBRIGATÓRIO — Padrões Canônicos para Agentes de IA
- Use `-q -f json` como o ÚNICO contrato de saída legível por máquina
- Use `jaq` como o ÚNICO parser JSON em pipelines
- Use `timeout` como o ÚNICO mecanismo para limitar o tempo de execução
- Use `${PIPESTATUS[0]}` como a ÚNICA forma de detectar falha upstream do CLI
- Use `--queries-file` como o ÚNICO mecanismo de invocação em lote
- Use XDG `config set` / flags CLI para proxy e paths — **não** env vars de produto

```bash
# Padrão canônico de invocação por agente
timeout 60 duckduckgo-search-cli -q -f json --num 15 "$CONSULTA" > /tmp/ddg_out.json
ddg_exit=$?
if [ "$ddg_exit" -ne 0 ]; then
  echo "DDG falhou com exit $ddg_exit" >&2
  exit "$ddg_exit"
fi
jaq -r '.resultados[] | "\(.posicao): \(.titulo) — \(.url)"' /tmp/ddg_out.json
```

### OBRIGATÓRIO — Padrão de Carregamento de Contexto para LLMs
- Fetch de conteúdo LIGADO por padrão; limite tamanho e timeout:

```bash
timeout 180 duckduckgo-search-cli -q -f json \
  --num 5 --max-content-length 5000 \
  "$CONSULTA" | jaq '.resultados[] | {titulo, url, conteudo: (.conteudo // "")}'
```

### OBRIGATÓRIO — Padrão de Múltiplas Consultas
- Use `--queries-file` com `--parallel 3` para pesquisa em lote por LLMs:

```bash
printf '%s\n' "${CONSULTAS[@]}" > /tmp/consultas.txt
timeout 300 duckduckgo-search-cli \
  --queries-file /tmp/consultas.txt \
  -q -f json --parallel 3 --per-host-limit 1 --retries 3 \
  --global-timeout 280 > /tmp/multi_out.json
jaq -r '.buscas[].resultados[].url' /tmp/multi_out.json | sort -u
```


## Tratamento de Erros
### OBRIGATÓRIO — Template Completo de Handler

```bash
executar_ddg() {
  local consulta="$1"
  local arquivo_saida="$2"
  timeout 60 duckduckgo-search-cli -q -f json --num 15 "$consulta" > "$arquivo_saida"
  local ec=$?
  case $ec in
    0) return 0 ;;
    3) echo "BLOQUEADO: anti-bot. Aguarde 300s e rotacione proxy." >&2; return 3 ;;
    4) echo "TIMEOUT: aumente --global-timeout." >&2; return 4 ;;
    5) echo "ZERO_RESULTADOS: reformule a consulta." >&2; return 5 ;;
    *) echo "ERRO($ec): verifique stderr." >&2; return "$ec" ;;
  esac
}
```

### OBRIGATÓRIO — Template de Integridade do Pipe

```bash
timeout 60 duckduckgo-search-cli "consulta" -q -f json | jaq '.resultados[].url'
ddg_exit=${PIPESTATUS[0]}
[ "$ddg_exit" -eq 0 ] || { echo "CLI falhou: exit $ddg_exit" >&2; exit "$ddg_exit"; }
```


## Arquivos de Configuração
- Localização padrão: `$XDG_CONFIG_HOME/duckduckgo-search-cli/` (padrão `~/.config/duckduckgo-search-cli/`)
- Override de localização: CLI `--config-home <PATH>` **somente** (sem env de produto `DUCKDUCKGO_SEARCH_CLI_HOME`)
- `config.toml` — chaves de produto persistentes via subcomando `config` (`path` / `list` / `get` / `set` / `unset` / `effective`)
- API dual: `get`/`set`/`unset` aceitam args **posicionais** **ou** flags `--key` / `--value`
- Chaves permitidas: `ui_lang`, `chrome_path`, `proxy_url`, `default_global_timeout`, `default_vertical`, `fetch_content_default`, `log_directive`, `default_lang`, `default_country`
- `selectors.toml` — seletores CSS para parsing de HTML
- `user-agents.toml` — pool de rotação de User-Agent
- Inicializar templates de selectors/UA: `duckduckgo-search-cli init-config`
- Atualização segura: `duckduckgo-search-cli init-config --dry-run` depois `--force`
- Componentes `..` em caminhos são rejeitados automaticamente no v0.5.0+

```bash
duckduckgo-search-cli config path
duckduckgo-search-cli config list
duckduckgo-search-cli config set proxy_url "socks5://127.0.0.1:1080"
duckduckgo-search-cli config get proxy_url
duckduckgo-search-cli config get --key proxy_url
duckduckgo-search-cli config set --key log_directive --value "duckduckgo_search_cli=debug"
duckduckgo-search-cli config unset proxy_url
duckduckgo-search-cli config effective
# override one-shot de home (sem env):
duckduckgo-search-cli --config-home /tmp/ddg-cfg -q -f json "consulta"
```


## Cartão de Referência Rápida

| Regra | Instrução |
|-------|-----------|
| R01 | DEVE passar `-q` ao canalizar para qualquer parser |
| R02 | DEVE especificar `-f json` explicitamente em scripts |
| R03 | JAMAIS parsear `text` ou `markdown` com máquinas |
| R04 | DEVE fixar `--num` explicitamente |
| R05 | DEVE limitar `--parallel` em 5 |
| R06 | DEVE usar `--output` para conjuntos grandes |
| R07 | JAMAIS invocar sem `timeout` |
| R08 | DEVE usar `--queries-file` para trabalho em lote |
| R09 | `--stream` / `-f ndjson` NDJSON multi-query SearchOutput; query única ignora com warning |
| R10 | DEVE preferir `--endpoint html` (Chrome); NUNCA remediar exit 3 com Lite |
| R11 | DEVE distinguir raiz JSON única vs múltipla |
| R12 | DEVE tratar `titulo` e `url` como garantidos não-nulos |
| R13 | JAMAIS assumir que campos opcionais estão presentes |
| R14 | DEVE usar `${PIPESTATUS[0]}` para detectar falhas em pipes |
| R15 | JAMAIS passar credenciais de proxy em argv |
| R16 | JAMAIS executar URLs de resultados sem sandbox |
| R17 | JAMAIS injetar headers `Sec-Fetch-*` (v0.6.0 os gerencia) |
| R18 | DEVE rodar `duckduckgo-search-cli --probe-deep` antes de queries reais em runners macOS para detectar CAPTCHA cedo (v0.7.3+) |
| R19 | DEVE tratar o cookie jar (`cookies.json`) como credencial; desabilite com `--no-cookie-persistence` (v0.7.3+) |
| R20 | DEVE tratar `--allow-lite-fallback` como **no-op legado** (v0.9.4, GAP-WS-113); não é remediação para exit 3 e não força Lite |


## v0.7.3 — Session + Probe-Deep + BoringSSL (correção do GAP-WS-27)

### OBRIGATÓRIO — Reconhecer as Novas Flags
- `--probe-deep` — executa uma query real e reporta `status: "ok"` ou `status: "captcha"`. Use isto em portões locais para runners macOS para detectar interstitials do Cloudflare Bot Management antes de lançar pipelines custosas.
- `--no-warmup` — pula o warm-up `GET https://duckduckgo.com/` que popula os cookies de sessão.
- `--no-cookie-persistence` — mantém cookies em memória apenas; nunca grava `cookies.json` em disco.
- `--cookies-path <PATH>` — sobrescreve o path XDG padrão do cookie jar. Use isto para apontar para um volume encriptado.
- `--allow-lite-fallback` — **no-op legado** desde a v0.9.4 (GAP-WS-113). Não força Lite e não é remediação de exit 3.

### OBRIGATÓRIO — Pré-requisitos de Build (v0.8.6+ / Chrome de produção v0.9.4)
- v0.8.6+ NÃO requer `cmake`, `perl`, NASM ou MSVC. TLS residual em HTTP de harness de teste é Rust puro via `reqwest` + `rustls-tls`
- v0.7.3–v0.8.5 exigia cmake, perl, NASM para BoringSSL via `wreq` — removido na v0.8.6 (ADR-0008)
- Produção é **Chrome-only** (feature `chrome` padrão; GAP-WS-113): `--probe`, `--probe-deep`, `--pre-flight`, `--fetch-content`, busca, news e `deep-research` exigem binário Chrome/Chromium utilizável e build com feature `chrome`. Chrome ausente ou binário sem feature `chrome` falha fechada com **exit 2**. **Não** há kill-switch de env em runtime (`DUCKDUCKGO_SEARCH_CLI_NO_CHROME` está morto / não é lido). No Linux o Chrome roda headed em Xvfb privado; no macOS/Windows em headless=new desde a v0.9.3

### OBRIGATÓRIO — Trate o Cookie Jar como Credencial
- A feature `session` persiste cookies de sessão do DuckDuckGo em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) com permissões Unix `0o600`. Leia o arquivo com o mesmo cuidado que leria uma API key.

Upstream: https://github.com/danilo-aguiar-br/duckduckgo-search-cli
Contrato de schema válido para `duckduckgo-search-cli` **v1.0.1** (núcleo estável desde v0.7.0; vertical news v0.8.9; flags globais v0.9.0; fail-closed Chrome-only GAP-WS-113; one-shot de processo GAP-WS-LIFECYCLE-001 / ADR-0017; defaults agent-ready GAP-WS-AGENT-READY-001 / ADR-0018 — vertical padrão `all`, fetch LIGADO, metadados aditivos `chrome_path_resolvido` / `chrome_canal` / `usou_chrome` honesto; one-shot de disco GAP-WS-TMP-PROFILE-ORPHAN-001 RESOLVIDO / ADR-0020 — prefixo `ddg-chrome-*`, `force_reap` / `ExitReapGuard` + `remove_dir_all`, `sweep_orphan_profiles` na próxima run só em perfis de propriedade; timeout global padrão 180s desde v0.9.9; **Pass 52 / v1.0.1:** multi-query `--stream` / `-f ndjson` linhas NDJSON SearchOutput; API dual de `config` + `config effective`; exit **141** broken pipe com SIG_IGN SIGPIPE + `ensure_oneshot_cleanup`; wire PT serialize BC + aliases EN no deserialize **ADR-0023**; atomwrite; sem telemetria remota; sem quebra de schema JSON no lifecycle).
Versão em inglês: `docs/AGENTS.md`.

## v1.0.1 — Contrato agent Pass 52 (stream / config dual / 141 / ADR-0023)

### OBRIGATÓRIO — Stream, config, oneshot, wire
- Multi-query `--stream` está **IMPLEMENTADO**: emite uma linha NDJSON por query concluída (`SearchOutput`). `-f ndjson` é alias do modo stream. Em query única `--stream` é ignorado com warning — não é stream de hits individuais da SERP.
- API dual de `config`: `get`/`set`/`unset` aceitam posicional **ou** `--key`/`--value`; `config effective` despeja o merge CLI > XDG > padrões. Config de produto é **somente CLI + XDG** — sem knobs de env de produto para home, kill-switch de Chrome ou strict de zero-cause.
- Broken pipe em write de stream/stdout → exit **141** (`128+SIGPIPE`). No Unix SIGPIPE fica **SIG_IGN** para o write falhar com EPIPE e o reap oneshot (`ensure_oneshot_cleanup`) ainda rodar antes do exit.
- Wire JSON: chaves em português no **serialize** (contrato agent BC); aliases ingleses `serde(alias)` só no **deserialize** (**ADR-0023**). Não renomeie as chaves PT de saída.
- Produção Chrome-only CDP; Chrome ausente / build sem feature `chrome` → exit **2** fail-closed. Sem telemetria remota; metadados agent são contrato JSON local apenas.

## v1.0.0 — One-shot de disco + prefixo de perfil auditável (GAP-WS-TMP-PROFILE-ORPHAN-001)

### OBRIGATÓRIO — Propriedade de processo + disco
- GAP-WS-TMP-PROFILE-ORPHAN-001 está **RESOLVIDO** na **v1.0.0** (ADR-0020: `docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md`). Completa o one-shot de processo (v0.9.6 / ADR-0017 / GAP-WS-LIFECYCLE-001) com one-shot de **disco**.
- Prefixo do dir de perfil Chrome: **`ddg-chrome-*`** sob `temp_dir` (NÃO `.tmp` genérico); modo Unix do perfil **`0o700`** quando aplicável.
- `force_reap` / `ExitReapGuard` mata processos **e** faz `remove_dir_all` no perfil de propriedade; caminhos cooperativos fazem reap da árvore + dir em sucesso, erro, timeout, SIGINT, SIGTERM.
- A próxima invocação `sweep_orphan_profiles` limpa **somente** `ddg-chrome-*` obsoletos de propriedade da CLI.
- **Política dura:** nunca auto-rm de `.tmp*` estrangeiros; nunca auto-rm de `org.chromium.Chromium.*`.
- Prefira GNU `timeout` (SIGTERM primeiro). deep-research herda o `CancellationToken` principal (SIGTERM cancela o fan-out). Timeout global padrão continua **180s** (desde 0.9.9).
- Sem quebra de schema JSON no lifecycle; atomwrite; sem telemetria remota.
- Auditoria do operador: `find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'ddg-chrome-*'`

### Limites residuais (honestos)
- SIGKILL/OOM da CLI pode deixar residual; a próxima run varre só `ddg-chrome-*`.
- Perfis genéricos `.tmp*` históricos pré-1.0.0 **não** são apagados em massa pela CLI (julgamento cuidadoso do operador — não scripts de bulk-rm).
- Órfãos de processo pré-0.9.6 continuam higiene do operador.


## v0.9.8 — Defaults agent-ready (GAP-WS-AGENT-READY-001 / ADR-0018)

### OBRIGATÓRIO — Defaults alterados para agentes
- O padrão de `--vertical` é **`all`** (web + notícias). Opt-out com `--vertical web`; no deep-research use `--no-news` para pular notícias.
- Fetch de conteúdo está **LIGADO por padrão** nas top web + news (teto 10). Opt-out com `--no-fetch-content`.
- Prefira `timeout 180` (ou maior no deep-research) ao aceitar o fetch padrão.
- Leia metadados agent com fallbacks: `.metadados.chrome_path_resolvido // ""`, `.metadados.chrome_canal // ""`, `.metadados.usou_chrome // false` — **não** são telemetria.
- `--chrome-path` e demais flags de transporte são globais (válidas após `deep-research`).
- Chrome multi-canal Flatpak é suportado no Linux (shell de export → ELF de deploy).
- Continua produção Chrome-only (v0.9.4), posse one-shot do processo (v0.9.6) e one-shot de disco (v1.0.0, `ddg-chrome-*`); atomwrite; sem telemetria.


## v0.9.6 — Lifecycle one-shot (GAP-WS-LIFECYCLE-001)

### OBRIGATÓRIO — Modelo de propriedade do processo
- Cada invocação da CLI é **NASCE → EXECUTA → MORRE**: ela é dona da árvore completa Chromium/Xvfb daquele run e faz o **reap** em sucesso, erro, timeout, SIGINT e SIGTERM.
- O reap é implementado via `process_lifecycle`, `XvfbGuard`, `shutdown` cooperativo e `Drop` (process group / marker / tree kill). Ver `docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md` (ADR-0017).
- Prefira GNU `timeout` (SIGTERM e depois SIGKILL após graça) para a CLI cancelar de forma cooperativa antes do kill duro.
- Escritas atômicas se aplicam a `--output`, config e cookie jar (mesmo schema; sem mudança de contrato JSON vs 0.9.5).
- **Supersessão (v1.0.0):** limpeza de perfil em disco e prefixo `ddg-chrome-*` fechados por GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020 — o one-shot só de processo era incompleto no eixo disco.

### Limites residuais (honestos)
- **SIGKILL** externo da CLI pode deixar órfãos (limite do SO — o cancelamento cooperativo nunca roda); desde a v1.0.0 a próxima run varre só `ddg-chrome-*` de propriedade.
- Órfãos históricos de runs **pré-0.9.6** **não** são limpos automaticamente; limpeza pontual no host é opcional só para esses — não é passo obrigatório a cada run após 0.9.6 para *novos* vazamentos de processo.
