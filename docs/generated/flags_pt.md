# Flags


- Este arquivo NÃO é gerado a partir de script nenhum: as tabelas foram conferidas À MÃO contra o binário v1.0.6
- O `scripts/regen_cli_flags_readme.py` foi apagado na v1.0.4, e hoje nada regenera este arquivo
- O inventário cobre as 66 flags de raiz declaradas na coluna de opções de `duckduckgo-search-cli --help`
- Ele cobre também as exclusivas de `deep-research`, `doctor`, `init-config`, `schema` e `man`, lidas no `--help` de cada subcomando
- Ele cobre também os aliases ocultos `--region` e `--max-concurrency`, que o clap aceita e nunca imprime
- A única guarda automatizada é a régua `generated_flag_tables_have_no_phantom_flags` em `tests/integration_docs_drift.rs`
- Essa régua passa cada flag longa destas tabelas ao clap, então ela pega flag INVENTADA
- Essa régua NUNCA pega flag AUSENTE, porque só lê os tokens que as tabelas já carregam
- Prefira `commands` / `schema` para descoberta de agente com baixo custo de tokens
- O inglês mora SOMENTE em [`README.md`](README.md) — este arquivo é o SSOT em português

## Raiz / busca padrão (inventário completo de `--help`)

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `-n`, `--num` | `15` | Máximo de resultados por query (padrão 15; páginas via `--pages`, padrão 1). |
| `-l`, `--lang` | `pt` | Código de idioma `kl` do DuckDuckGo (SERP). Padrão `pt`. |
| `-c`, `--country` | `br` | Código de país `kl` do DuckDuckGo. `--region` é alias legado. Padrão `br`. |
| `--queries-file` | (nenhum) | Arquivo com queries adicionais (uma por linha). Linhas vazias ignoradas. |
| `--endpoint` | `html` | `html` (produção) ou `lite` (valor legado; não é caminho de sucesso sob GAP-WS-113). |
| `--vertical` | `all` (v0.9.8) | `web`, `news` ou `all` (padrão `all` desde v0.9.8). News só via Chrome. |
| `--time-filter` | (nenhum) | Filtro temporal: `d` / `w` / `m` / `y`. Padrão: nenhum. |
| `--safe-search` | `moderate` | Safe-search: `off`, `moderate` (padrão) ou `on`. |
| `--identity-profile` | `auto` | Fixa perfil do pool de 12 identidades (`chrome-win`, `safari-mac`, …). Padrão `auto` rotaciona no bloqueio. |
| `--cookies-path` | (nenhum) | Sobrescreve path do cookie jar (padrão: dir XDG + `cookies.json`). |
| `--seed` | (nenhum) | Seed determinístico para UA + identidade (reprodutibilidade). |
| `--config` | (nenhum) | Path do diretório de configuração (sobrescreve o path padrão do SO). |
| `-f`, `--format` | `auto` | Formato: `json`, `text`, `markdown`/`md`, `tsv`, `ndjson` ou `auto` (TTY-aware). |
| `-o`, `--output` | (nenhum) | Grava no arquivo em vez de stdout (cria pais; Unix 0o644). |
| `--stream` | off | Multi-query: emite NDJSON conforme cada busca completa. Alias: `-f ndjson`. Fechamento cedo → exit 141. |
| `--fields` | (nenhum) | Projeta cada linha para os campos wire listados (vírgula; tokens EN ou PT). |
| `--select` | (nenhum) | Alias de `--fields` (nome agent-native / ETL). |
| `--filter` | (nenhum) | Filtra linhas após extract do SERP (agent-native; sem jq). |
| `--limit` | (nenhum) | Limita linhas após extract + `--filter` (agent-native; sem jq). |
| `--sort` | (nenhum) | Ordena linhas após filter (agent-native; sem jq). |
| `--dedupe-by` | (nenhum) | Deduplica por URL canônica após sort (agent-native; sem jq). |
| `--count-only` | off | Emite só contagens compactas EN — sem linhas de resultado. |
| `--truncate-content` | (nenhum) | Trunca cada `content` a N escalares Unicode (anti-token). |
| `--max-output-bytes` | (nenhum) | Fail-closed se o payload formatado de stdout exceder N bytes. |
| `--pretty` | off | JSON indentado (`to_string_pretty`). Padrão é JSON compacto para orçamento de tokens. |
| `--no-color` | off | Desativa cor (respeita `NO_COLOR` / no-color.org). |
| `--print-schema` | off | Imprime catálogo JSON Schema em stdout (igual a `schema` sem `--name`). |
| `--ui-lang` | (nenhum) | Idioma da UI em stderr humano (`en`|`pt-BR`). Não é o SERP `-l`/`--lang`. |
| `--config-home` | (nenhum) | Sobrescreve diretório de config XDG/plataforma (selectors, cookies, ui-lang). |
| `--wire-keys` | `en` (v1.0.2) | Serializa chaves JSON em inglês (`en`, padrão v1.0.2) ou português legado (`pt`). Também `config set wire_keys`. |
| `-t`, `--timeout` | `15` | Timeout por query em segundos (padrão 15). |
| `-p`, `--parallel` | `5` | Requests concorrentes (`1..=20`, padrão 5). Alias: `--max-concurrency`. |
| `--shared-session-verticals` | off | Força uma sessão Chrome compartilhada para web+news (`--vertical all`) em vez de dual multi-process. |
| `--pages` | `1` | Páginas por query (`1..=5`, padrão 1; pode auto-elevar com `--num`). |
| `--retries` | `2` | Retries extras em falhas HTTP/rede transitórias (`0..=10`, padrão 2). |
| `--disable-retry` | off | Força zero retries (kill switch). Equivalente a `--retries 0`. |
| `--base-url-html` | (nenhum) | Sobrescreve URL base do SERP HTML (wiremock/testes). |
| `--base-url-lite` | (nenhum) | Sobrescreve URL base Lite. |
| `--base-url-serp` | (nenhum) | Sobrescreve URL base SERP / warm-up. |
| `--proxy` | (nenhum) | Proxy HTTP/HTTPS/SOCKS5 via CLI (produto não herda `HTTP(S)_PROXY`). |
| `--no-proxy` | off | Desativa todas as fontes de proxy (no-proxy explícito). |
| `--allow-lite-fallback` | off | NO-OP legado (GAP-WS-113) — não força Lite; SERP permanece HTML Chrome. Mantido para scripts não saírem com exit 2. |
| `--global-timeout` | `180` | Timeout global do pipeline (`1..=3600` s, padrão 180). Diferente de `--timeout` por request. Global nos subcomandos. |
| `--cancel-grace-secs` | `5` | Graça cooperativa de cancelamento antes de hard exit (`1..=60` s, padrão 5). |
| `--probe` | off | Probe de saúde do Chrome via CDP: reachability mínima + latência em JSON. |
| `-v`, `--verbose` | off | `-v` = DEBUG, `-vv`+ = TRACE em stderr. Log de produto = CLI `-v`/`-q` + XDG `log_directive` (não `RUST_LOG`). |
| `-q`, `--quiet` | off | Silencia todo tracing em stderr (incluindo ERROR). |
| `--no-input` | off | Contrato de agente: nunca prompt / nunca lê TTY interativo. |
| `--probe-deep` | off | Health check profundo via Chrome/CDP com detecção de interstitial (CAPTCHA); relatório JSON. |
| `--require-results` | off | Falha com exit 5 quando a busca zera (gate de agente). Em deep-research pode usar exit 70 no modo require. |
| `--pre-flight` | off | Calibração pre-flight de ghost-block/interstitial no SERP Chrome. Não libera pure-HTTP nem Lite (GAP-WS-113). Só vertical web. |
| `--no-zero-cause-strict` | off | Desliga mapeamento estrito de zero-cause (exit 5 legado para todos os zeros). Padrão strict ON → exit 6 para zeros não legítimos. |
| `--fetch-content` | ligado (v0.9.8) | Afirma extração de conteúdo (padrão LIGADO desde v0.9.8 para web+news). Prefira omitir ou `--no-fetch-content`. |
| `--no-fetch-content` | off | Desliga extração de body (opt-out do padrão agent-ready v0.9.8). |
| `--fetch-content-cap` | `4` (v1.0.2) | Máximo de URLs a enriquecer por vertical (`1..=50`, padrão 4 na v1.0.2; era 10 na v0.9.8). |
| `--max-content-length` | `10000` | Máximo de caracteres do body extraído por página (`1..=100_000`, padrão 10000). |
| `--per-host-limit` | `2` | Fetches concorrentes por host no modo content (`1..=10`, padrão 2). |
| `--match-platform-ua` | off | Filtra pool de UAs ao SO atual quando existe `user-agents.toml` externo. |
| `--chrome-path` | (nenhum) | Path manual do Chrome/Chromium para todas as ops de rede. Multi-canal (v0.9.8): Flatpak export→ELF. Global após `deep-research`. |
| `--chrome-visible` | off | Força Chrome headed (janela visível). Override de debug. |
| `--chrome-headless` | off | Força Chrome headless (`--headless=new`). Sobrescreve caminho Xvfb auto. |
| `--chrome-xvfb` | off | Pede headed com Xvfb privado no Linux (headed invisível anti-bot). |
| `--chrome-session-retries` | `2` | Tentativas extras de launch de sessão Chrome após a primeira (CDP transitório; padrão 2). |
| `--dump-news-html` | (nenhum) | Grava HTML do SERP news após extract neste path (debug local). |
| `--no-warmup` | off | Pula warm-up `GET https://duckduckgo.com/` que popula cookies de sessão. |
| `--no-cookie-persistence` | off | Cookies só em memória; nunca grava o jar em disco. |

## Só `deep-research` (além das flags globais da raiz)

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `--max-sub-queries` | `3` (v1.0.2) | Máximo de sub-queries na decomposição (`1..=12`, padrão 3 na v1.0.2). |
| `--sub-query-strategy` | `heuristic` | Estratégia de decomposição (ex.: `heuristic`). |
| `--sub-queries-file` | (nenhum) | Arquivo com sub-queries explícitas para deep-research (uma por linha). |
| `--aggregate` | `rrf` | Modo de agregação dos resultados do deep-research. |
| `--depth` | `0` | Profundidade / intensidade do fan-out de pesquisa. |
| `--budget-tokens` | `4000` | Teto de tokens para deep-research. |
| `--synth-format` | `markdown` | Formato de saída da síntese do relatório deep-research. |
| `--synthesize` | off | Ativa o passo de síntese (LLM/relatório) no deep-research. |
| `--allow-under-budget` | off | Permite seguir quando o budget estimado fica abaixo do perfil pedido. |
| `--print-budget` | off | Dry-run do orçamento de budget sem Chrome / sem inventar query placeholder. |
| `--auto-contention-budget` | off | Ativa ajuste de budget ciente de contenção (política padrão). |
| `--no-auto-contention-budget` | off | Desliga o auto-ajuste de budget por contenção. |
| `--require-all-sub-queries` | off | Falha se qualquer sub-query não completar com sucesso. |
| `--no-news` | off | Opt-out do vertical news no deep-research (news é padrão ligado). |

## Só `doctor`

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `--strict` | off | Doctor: falha fechada em checks não-OK. |

## Só `init-config`

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `--force` | off | init-config: sobrescreve arquivos de config existentes. |
| `--dry-run` | off | init-config: simula sem gravar arquivos. |

## Só `schema`

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `--name` | (nenhum) | schema: emite corpo de schema nomeado em vez do catálogo. |

## Só `man`

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `--file` | (nenhum) | man: grava roff neste path (atômico). Usa `--file`, não `-o`. |
