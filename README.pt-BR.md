# duckduckgo-search-cli

[![docs.rs](https://img.shields.io/docsrs/duckduckgo-search-cli)](https://docs.rs/duckduckgo-search-cli)
[![crates.io](https://img.shields.io/crates/v/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![License](https://img.shields.io/crates/l/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-orange)](https://github.com/danilo-aguiar-br/duckduckgo-search-cli)
[![Downloads](https://img.shields.io/crates/d/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-blue)](https://www.rust-lang.org)

> Busca web na velocidade do terminal — dê ao seu agente de IA contexto sobre-humano.

[Read in English](README.md)


## O que é?
- Binário Rust único que transforma qualquer shell em ferramenta de busca de primeira classe
- Sem API key, sem tracking, busca via Chrome invisível ao usuário
- Schema JSON estável com `resultados[]` e `metadados`, ordem de campos congelada entre releases

### Features do Cargo
| Feature | Default | Descrição |
|---------|---------|-----------|
| `chrome` | **sim** | Rede de produção via Chrome real (`chromiumoxide`/CDP). Obrigatória para SERP, news, deep-research, probe, pre-flight e content fetch. |
| `http-test-harness` | não | SERP/probe HTTP residual só para testes (`DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`). Nunca é caminho silencioso de produção. |
| `console` | não | Subscriber `tokio-console` para debug de tasks em runtime. |

O install padrão já ativa `chrome`: `cargo install duckduckgo-search-cli --locked`. O docs.rs compila com `all-features = true` e `targets` multiplataforma (ver `[package.metadata.docs.rs]` no `Cargo.toml`).
- Exit codes determinísticos para agentes ramificarem sem ambiguidade
- Paralelismo nativo via `tokio::JoinSet` com controle de concorrência por host
- Funciona em Linux (glibc, musl/Alpine), macOS Intel + Apple Silicon Universal e Windows MSVC


## Por que usar?
- Sem API key para rotacionar e sem dashboard para monitorar
- Perfis de browser v0.6.0 imitam sessões reais para evitar bloqueios anti-bot
- Fetch de conteúdo **LIGADO por padrão (v0.9.8)** — texto limpo de top URLs (web + news) no JSON; opt-out `--no-fetch-content`
- Schema estável entre releases: nenhuma quebra de contrato para pipelines existentes
- **v0.8.0+ / ADR-0016 / ADR-0022 — Transporte Chrome nativo.** SERP de produção usa a stack TLS do Chrome no host (evita assinatura TLS de biblioteca/`rustls` que o Cloudflare bloqueia). **Proibido** spoof sintético de hardware fingerprint (canvas/WebGL/áudio). HTTP residual (harness) = rustls + `aws-lc-rs` (ADR-0021). v0.8.7+ Xvfb + mitigação de sinais de automação; v0.9.3+ headless=new no macOS/Windows.
- **v0.9.6 / v1.0.0 / v1.0.1 — One-shot.** Agentes podem invocar N vezes sem acumular Chromium/Xvfb órfãos. Desde a **v1.0.0**, perfis Chrome usam o prefixo auditável `ddg-chrome-*` (não `.tmp*` genérico) e são removidos no exit cooperativo; o sweep da próxima run limpa só esse prefixo. **v1.0.1** endurece reap pipe-safe (`ensure_oneshot_cleanup`, SIG_IGN em SIGPIPE) para que `| head` cedo ainda reap `ddg-chrome-*`. Reap de árvore+disco em sucesso, erro, timeout, SIGINT, SIGTERM e broken pipe. Ver ADR-0017 + ADR-0020.
- **v0.9.8 — Agent-ready.** Padrão `--vertical all` (web+news); fetch de conteúdo LIGADO (opt-out `--no-fetch-content`); multi-canal Flatpak; flags de transporte globais após `deep-research`; metadados `chrome_path_resolvido` / `chrome_canal` (não telemetria). Ver ADR-0018.
- **v1.0.0 — Disco + contrato estável.** One-shot processo e disco; política dura (nunca bulk-rm `.tmp*` / `org.chromium.Chromium.*`); sem telemetria remota.
- **v1.0.1 — Pass 52.** Dual `config get`/`set`/`unset` + `config effective`; `-f ndjson` alias de `--stream`; exit **141** em broken pipe; wire PT na serialização + aliases EN na desserialização (ADR-0023); false anti-bot de news corrigido; config só CLI+XDG.


## Pré-requisitos (v0.8.7+)
- Google Chrome ou Chromium (detectado automaticamente via `detect_chrome()`)
- Linux: Xvfb auto-instalado pela CLI via `try_auto_install_xvfb()` para 22+ distros (Fedora, Ubuntu, Debian, Arch, openSUSE, Alpine, Void, Gentoo, Amazon Linux e derivadas)
- macOS/Windows: sem dependência extra — Chrome roda em headless=new desde a v0.9.3
- Chrome é o ÚNICO transporte de rede de produção desde a v0.9.4 (GAP-WS-113) — chromiumoxide/CDP para busca, news, deep-research, `--probe`, `--probe-deep`, `--pre-flight` e `--fetch-content`
- HTTP/reqwest residual existe somente sob feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` (e helpers de cookie/UA) — nunca caminho silencioso de SERP em produção
- Sem Chrome utilizável (ou binário sem feature `chrome`) → **exit 2 fail-closed** (sem auto `--no-news`, sem rebaixamento Web, sem HTTP silencioso). Env de produto `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` foi **removida** / não é lida — use CLI / rebuild sem `chrome` só fora de produção.
- Feature `chrome` é default; `--allow-lite-fallback` é **no-op legado** (SERP permanece HTML Chrome)
- v0.8.7: `has_native_display()` detecta display nativo por plataforma antes de decidir headed vs headless
- v0.8.7+: Linux roda Chrome HEADED dentro de display Xvfb privado (ZERO janelas visíveis); v0.9.3 mudou macOS/Windows para headless=new
- v0.8.7: navegação de warm-up para duckduckgo.com antes da URL de busca (pré-carregamento de cookies Cloudflare)
- v0.8.7: coerência UA↔processo Chrome — apenas UA Chrome quando o browser é Chromium (`chrome_only_ua_for_platform()`)
- v0.8.7 / ADR-0022: só mitigação de sinais de automação CDP (`webdriver`, plugins, `window.chrome`, outer size, Permissions, leak CDP) — **sem** spoof de canvas/WebGL/áudio
- Cascata de fallback (Linux): Xvfb privado → auto-install Xvfb → headless (último recurso com warning); macOS/Windows usam headless=new desde v0.9.3
- Modo do Chrome: flags CLI `--chrome-visible` (debug) / `--chrome-headless` (forçar headless). Envs de produto `DUCKDUCKGO_CHROME_*` foram **removidas** — só flags CLI.
- **Contrato one-shot processo + disco (v0.9.6 processo / v1.0.0 disco / v1.0.1 pipe-safe):** cada invocação é dona da árvore Chromium, do Xvfb privado (Linux) e do perfil sob **`ddg-chrome-*`** (Unix `0o700`). Em sucesso, erro, timeout, SIGINT, SIGTERM ou **BrokenPipe (exit 141)** a CLI encerra a árvore completa via `ensure_oneshot_cleanup` (process group + PIDs + marker de `user-data-dir`) e **remove o perfil**. SIGPIPE permanece **SIG_IGN** para que Drop/reap ainda rodem quando `| head` fecha cedo. Nenhum browser de automação nem Xvfb **desta** execução pode sobreviver ao exit cooperativo. A próxima run varre só **`ddg-chrome-*`** stale — nunca bulk-delete de `.tmp*` estrangeiro nem `org.chromium.Chromium.*`. Sem telemetria remota. Residual: SIGKILL/OOM da CLI não é interceptável; órfãos de processo pré-0.9.6 e perfis `.tmp*` pré-1.0.0 não são limpos em massa. Ver ADR-0017 + ADR-0020.


## O que há de novo na v1.0.1 (2026-07-19)
- **Pass 48 contrato DR + Pass 52 oneshot/stream/config** — deep-research honra `-o`, JSON de timeout agent-stable, `--depth` heurístico, `-f tsv`, subcomandos XDG `config`/`man`; sem knobs de env de produto; **sem telemetria remota**.
- **One-shot pipe-safe (Pass 52 / GAP-E2E-51-001/007)** — `ensure_oneshot_cleanup` em todas as saídas inclusive pipe cedo; Unix **SIG_IGN** para SIGPIPE (não SIG_DFL) para Drop/reap do Chrome ainda rodar; stream `BrokenPipe` → exit **141**.
- **`-f ndjson`** aceito como alias do modo stream multi-query (`--stream`).
- **API dual de `config`** — `config get KEY` **ou** `config get --key KEY`; `config set KEY VALUE` **ou** `config set --key KEY --value VALUE`; também `config unset` dual e **`config effective`** (JSON mesclado CLI+XDG+defaults).
- **Wire JSON (ADR-0023)** — nomes de campos em português na **serialização** (BC para agentes); aliases `serde` em inglês só na **desserialização**.
- **Vertical news** — classificação anti-bot falsa em news isolada corrigida; residual de anti-bot real do DDG ainda pode gerar exit 6 ambientalmente (`noticias` vazias honestas, nunca sintéticas).
- Config de produto via **CLI + XDG apenas** — não ensine envs de produto como `DUCKDUCKGO_ZERO_CAUSE_STRICT` ou `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` como knobs vivos (notas históricas de migração marcam **removidas**).

## O que há de novo na v1.0.0 (2026-07-15)
- **GAP-WS-TMP-PROFILE-ORPHAN-001 RESOLVIDO (ADR-0020)** — one-shot honesto em **disco** além do processo: `user-data-dir` com prefixo **`ddg-chrome-`** (não `.tmp` padrão); `force_reap` remove o diretório; `ExitReapGuard` + panic hook + reap em timeout/fim de run.
- **Política dura de higiene em disco** — (1) nunca auto-rm `.tmp*` genérico; (2) nunca auto-rm `org.chromium.Chromium.*`; (3) residual SIGKILL/OOM limpo na **próxima** run via `sweep_orphan_profiles` **somente** em `ddg-chrome-*`.
- **deep-research** herda o `CancellationToken` do `main`; `Config::default().global_timeout_seconds` alinhado a 180.
- **Contrato estável 1.0.0** — SERP Chrome-only CDP, defaults agent-ready (0.9.8), honesty e2e (0.9.9), one-shot processo+disco, atomwrite, **sem telemetria remota**. Sem quebra de schema JSON vs 0.9.10/0.9.9.
- Inventário: `gaps.md`; ADR: `docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md`.

## O que há de novo na v0.9.8 (2026-07-14)
- **GAP-WS-AGENT-READY-001 RESOLVIDO (L-01…L-08)** — defaults agent-ready, Chrome multi-canal, dual web+news, texto limpo. ADR-0018; inventário em `gaps.md`.
- **L-01/L-02 Multi-canal Chrome** — export shell Flatpak rejeitado → resolve ELF real em `…/files/extra/chrome`; wrappers Chromium Fedora → ELF lib64. Ordem: `--chrome-path` → `CHROME_PATH` → host Chrome → host Chromium → Flatpak → Snap.
- **L-03 Dual padrão** — busca com `--vertical` padrão **`all`** (web + news). Opt-out: `--vertical web`. Deep-research já dual salvo `--no-news`.
- **L-04 News SERP** — multi-seletor; `usou_chrome` honesto em news-only / multi-query / deep / falhas.
- **L-05 Texto limpo LIGADO por padrão** — fetch de conteúdo ON para **web + news** (FETCH_CAP=10); opt-out **`--no-fetch-content`**. News pode trazer `conteudo` / `tamanho_conteudo` / `metodo_extracao_conteudo`.
- **L-06 Flags de transporte `global = true`** — `--chrome-path`, `--proxy`, `--vertical`, fetch, identity etc. funcionam **depois** de `deep-research` (e antes).
- **L-07 UA fan-out** — `coerce_chrome_user_agent` compartilhado; one-shot (0.9.6); Chrome-only (0.9.4); atomwrite; **sem telemetria remota**.
- **L-08 Docs/schemas/skills** — ADR-0018, inventário local `gaps.md`, skills EN/PT, CHANGELOG.
- **Metadados de agente (NÃO telemetria)** — `chrome_path_resolvido`, `chrome_canal`, `usou_chrome` honesto.
- **Residuais** — anti-bot pode zerar news; limite OS de SIGKILL; sem flag separada `--agent`.

## O que há de novo na v0.9.6 (2026-07-12)
- **GAP-WS-LIFECYCLE-001 fechado** — ownership one-shot real da árvore externa (Chromium multi-processo + Xvfb + `TempDir`)
- **`src/process_lifecycle.rs`** — process group (`setpgid` + `PR_SET_PDEATHSIG` no Linux), `killpg`, walk de árvore, kill por marker de `user-data-dir`, limpeza de lock/socket Xvfb, registry de sessão + panic hook
- **`ChromeBrowser`** — `XvfbGuard` sempre mata Xvfb no drop (inclusive se o launch falhar); `shutdown` assíncrono com deadline em `close`/`wait` e kill forçado; `force_reap_session` no `Drop`
- **`content_fetch`** — `take()` + `shutdown` assíncrono após drenar o JoinSet
- **Sinais** — SIGTERM (Unix) e SIGINT cancelam o `CancellationToken` (Docker/`timeout`/supervisores)
- **Atomwrite** — `paths::atomic_write` para `--output`, `init-config` e cookie jar
- **Testes** — unitários de process group/marker/atomwrite; E2E gated com `DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle`
- **Docs** — ADR-0017, `gaps.md` RESOLVIDO, este contrato. Sem quebra de schema JSON vs 0.9.5. Sem telemetria remota


### Migração v0.9.1 → v0.9.3 (endurecimento stealth)
- v0.9.1 (GAP-WS-107): macOS/Windows passaram a usar headed nativo Quartz/DWM + coerção de plataforma UA
- v0.9.2 (GAP-WS-108/109/110/111): `--enable-automation` do chromiumoxide removido via `.disable_default_args()`; UA Chrome alinhado à versão real instalada via `detect_chrome_major_version()` + `Emulation.setUserAgentOverride`; `--force-webrtc-ip-handling-policy=disable_non_proxied_udp`, `--disable-webrtc-hw-decoding`, `--disable-quic` adicionados a flags_stealth
- v0.9.3 (GAP-WS-112): macOS/Windows mudaram para headless=new (Quartz/DWM clampavam `--window-position`); Linux mantém Xvfb privado; escape hatch de debug é a flag CLI `--chrome-visible` (env de produto `DUCKDUCKGO_CHROME_VISIBLE` **removida**)


## Instalação
- Instale via Cargo com um único comando:

```bash
cargo install duckduckgo-search-cli
```


## Uso Rápido
- Busca básica com 15 resultados (padrão):

```bash
duckduckgo-search-cli "rust async programming"
```

- Busca com saída JSON e 10 resultados:

```bash
duckduckgo-search-cli -f json -n 10 "tokio tutorial"
```

- Busca para LLMs e agentes com parsing via jaq:

```bash
duckduckgo-search-cli "tokio JoinSet exemplos" --num 15 -q | jaq '.resultados'
```

- Busca com conteúdo de páginas embutido no JSON:

```bash
duckduckgo-search-cli --fetch-content -n 5 "melhores frameworks web rust"
```


## Receitas Práticas
- Extrair apenas URLs para um fetcher downstream:

```bash
duckduckgo-search-cli "site:example.com changelog 2025" --num 15 -f json \
  | jaq -r '.resultados[].url'
```

- Enviar bodies limpos para um summarizer:

```bash
duckduckgo-search-cli "tokio runtime internals" --num 15 \
  --fetch-content --max-content-length 4000 -f json \
  | jaq -r '.resultados[] | "# \(.titulo)\n\(.conteudo)\n"' > corpus.md
```

- Fan-out de múltiplas queries em uma única invocação:

```bash
duckduckgo-search-cli "rust rayon" "rust tokio" "rust crossbeam" \
  --num 15 --parallel 3 -f json
```

- Streaming NDJSON para pipelines reativos:

```bash
duckduckgo-search-cli "wasm runtimes" --num 15 --stream \
  | jaq -r 'select(.url) | .url' \
  | xargs -I{} my-downloader {}
```

- Roteamento via proxy corporativo:

```bash
duckduckgo-search-cli "vendor status page 2026" --num 15 \
  --proxy http://user:pass@proxy.internal:8080 -f json
```


## Configuração
- Grava os arquivos padrão no diretório XDG:

```bash
duckduckgo-search-cli init-config
```

- Dry-run para ver o que seria escrito:

```bash
duckduckgo-search-cli init-config --dry-run
```

- Sobrescrever arquivos existentes explicitamente:

```bash
duckduckgo-search-cli init-config --force
```


## Comandos

| Comando | Propósito |
|---|---|
| `duckduckgo-search-cli <QUERY>...` | Busca padrão (equivalente a `buscar`) |
| `duckduckgo-search-cli buscar <QUERY>...` | Subcommand explícito de busca |
| `duckduckgo-search-cli deep-research <QUERY>` | Fan-out de queries, agregação e síntese opcional (v0.7.0) |
| `duckduckgo-search-cli init-config` | Grava `selectors.toml` e `user-agents.toml` no XDG |

## Deep Research (v0.7.0)

Para perguntas de pesquisa multi-hop — "compare os quatro principais clientes HTTP Rust em 2026", "o que mudou no Tokio 1.40", "resuma a história do endpoint HTML do DuckDuckGo" — o `duckduckgo-search-cli` traz um pipeline de fan-out que decompõe a pergunta em 1..=12 sub-queries, dispara em paralelo, agrega e opcionalmente sintetiza um relatório com referências numeradas.

Desde a v0.8.9 (GAP-WS-105) o `deep-research` também varre a vertical de notícias do DuckDuckGo por PADRÃO: cada sub-query roda como `--vertical all`, com a MESMA sessão Chrome navegando a SERP web e depois a SERP de notícias. O envelope sempre traz a lista agregada `noticias[]` (vazia quando zero). Use `--no-news` para opt-out. **v0.9.4 GAP-WS-113:** sem Chrome a CLI **falha com exit 2** — sem auto-degradação `--no-news`.

```bash
# Decomposição heurística padrão (5 sub-queries, agregação RRF, sem síntese).
duckduckgo-search-cli deep-research "melhor cliente http rust 2026" -f json -q \
  | jaq '.resultados[] | {titulo, url, score}'

# Relatório em Markdown com orçamento de tokens e extração completa.
duckduckgo-search-cli deep-research "tokio vs async-std produção 2026" \
  --synthesize --budget-tokens 1500 --synth-format markdown \
  --fetch-content --max-content-length 6000 -f json -q

# Sub-queries manuais a partir de arquivo (comentários `#` e linhas vazias ignorados).
cat > /tmp/qs.txt <<EOF
# Visão geral
o que é tokio runtime 2026
# Comparação
tokio vs async-std vs smol
EOF
duckduckgo-search-cli deep-research "tokio runtime 2026" \
  --sub-queries-file /tmp/qs.txt --aggregate dedupe-by-url -f json -q
```

### Flags do Deep Research

- `--max-sub-queries N` máximo de sub-queries geradas (1..=12)
- `--sub-query-strategy` heurística ou manual
- `--sub-queries-file PATH` lista explícita de sub-queries
- `--aggregate` RRF (K=60) ou dedupe por URL canônica
- `--depth` rounds de reflexão planejados mas não executados em v0.7.0
- `--fetch-content` / `--no-fetch-content` — fetch de corpo **LIGADO** por padrão (v0.9.8) para top-K web + news (FETCH_CAP=10); opt-out `--no-fetch-content`
- `--synthesize` produz relatório final em Markdown, PlainText ou JSON
- `--budget-tokens N` limite de tokens do relatório
- `--synth-format` markdown, plain-text ou json
- `--no-news` desativa a varredura da vertical news (v0.8.9, GAP-WS-105); por padrão cada sub-query roda `--vertical all` via Chrome — **v0.9.4 GAP-WS-113:** sem Chrome utilizável a CLI **falha exit 2 fail-closed** (sem auto `--no-news`; a auto-degradação v0.9.0–0.9.3 foi supersedida)

### Schema JSON de saída

```jsonc
{
  "query": "melhor cliente http rust 2026",
  "metadados": {
    "query_original": "melhor cliente http rust 2026",
    "sub_queries": [
      { "texto": "...", "estrategia": "heuristic", "status": "ok", "elapsed_ms": 420 }
    ],
    "total_resultados_unicos": 27,
    "total_noticias_unicas": 9,
    "tempo_total_ms": 1850,
    "nivel_cascata": 0
  },
  "resultados": [
    { "titulo": "...", "url": "...", "score": 0.041, "fontes": ["..."] }
  ],
  "noticias": [
    { "posicao": 1, "titulo": "...", "url": "...", "fonte": "...", "data_relativa": "há 2 horas", "score": 0.032, "ocorrencias": 2 }
  ],
  "sintese": {
    "formato": "markdown",
    "corpo": "# Relatório\n\n...\n\n[1] Título — url",
    "tokens_estimados": 1200,
    "quantidade_referencias": 5
  }
}
```


## Flags Disponíveis

| Flag | Padrão | Descrição |
|---|---|---|
| `-n`, `--num` | `15` | Máximo de resultados por query (auto-pagina quando > 10) |
| `-f`, `--format` | `auto` | `json`, `text`, `markdown`/`md`, `tsv`, `ndjson` (alias de stream), ou `auto` (detecta TTY) |
| `-o`, `--output` | stdout | Grava no arquivo (valida path, cria diretórios, Unix 0o644) |
| `-t`, `--timeout` | `15` | Timeout por request em segundos |
| `--global-timeout` | `180` | Timeout global do pipeline (1..=3600 s; v0.9.9 agent-ready) |
| `-l`, `--lang` | `pt` | Código de idioma `kl` do DuckDuckGo |
| `-c`, `--country` | `br` | Código de país `kl` do DuckDuckGo |
| `-p`, `--parallel` | `5` | Requests concorrentes (1..=20) |
| `--pages` | `1` | Páginas por query (1..=5, auto-elevado por `--num`) |
| `--retries` | `2` | Retries extras em 429/403/timeout (0..=10) |
| `--endpoint` | `html` | `html` ou `lite` |
| `--vertical` | **`all`** (v0.9.8) | `web`, `news` ou `all` — padrão dual web+news; opt-out `--vertical web`. News só via Chrome (v0.8.9); batch multi-query desde GAP-WS-105 |
| `--time-filter` | (nenhum) | `d`, `w`, `m` ou `y` |
| `--safe-search` | `moderate` | `off`, `moderate` ou `on` |
| `--stream` | off | Emite uma linha NDJSON por resultado (multi-query). Alias: `-f ndjson`. Fechamento cedo → exit **141** |
| `--fetch-content` | **ligado** (v0.9.8) | Baixa top URLs (**web + news**, FETCH_CAP=10) e embute texto limpo; opt-out **`--no-fetch-content`** |
| `--no-fetch-content` | off | Desliga o fetch de conteúdo padrão (v0.9.8) |
| `--max-content-length` | `10000` | Limite de caracteres por body (1..=100_000) |
| `--per-host-limit` | `2` | Fetches concorrentes por host (1..=10) |
| `--proxy URL` | (nenhum) | Proxy HTTP/HTTPS/SOCKS5 via CLI (produto não herda `HTTP(S)_PROXY`) |
| `--no-proxy` | off | Desativa todas as fontes de proxy |
| `--queries-file PATH` | (nenhum) | Lê queries adicionais de arquivo (uma por linha) |
| `--match-platform-ua` | off | Filtra pool de user-agents para o SO atual |
| `--chrome-path PATH` | (auto) | Caminho manual do Chrome (feature `chrome`). Multi-canal (v0.9.8): Flatpak export→ELF. Flag global — funciona após `deep-research` |
| `-v`, `--verbose` | off | `-v` = DEBUG, `-vv`+ = TRACE em stderr. Log de produto = CLI `-v`/`-q` + XDG `log_directive` (não `RUST_LOG`) |
| `-q`, `--quiet` | off | Silencia **todo** tracing em stderr (incluindo ERROR) |
| `--probe` | off | Verificação de saúde pré-voo (1 requisição mínima, relatório JSON) |
| `--identity-profile` | `auto` | Fixa um perfil do pool de 12 identidades (`chrome-win`, `safari-mac`, ...) |
| `--seed N` | (aleatório) | Seed determinístico para seleção de UA e identidade |
| `--probe-deep` | off | Detector de interstitial CAPTCHA (v0.7.3+) |
| `--no-warmup` | off | Pula warm-up `GET https://duckduckgo.com/` (v0.7.3+) |
| `--no-cookie-persistence` | off | Cookies apenas em memória, sem gravar em disco (v0.7.3+) |
| `--cookies-path PATH` | XDG config | Sobrescreve path padrão do cookie jar (v0.7.3+) |
| `--allow-lite-fallback` | off | **NO-OP legado (GAP-WS-113)** — não força Lite; SERP HTML Chrome |
| `--pre-flight` | off | Probe mínimo antes da busca real (v0.7.9+). Aplica-se apenas à vertical web — pulado em `--vertical news` (v0.8.9) |


## Vertical de Notícias (v0.8.9; defaults supersedidos pela v0.9.8)

- **v0.9.8:** o padrão de `--vertical` é **`all`** (web + news). Opt-out com `--vertical web`. (Histórico v0.8.9: padrão era `web` — **supersedido pela v0.9.8**.)
- `--vertical news` retorna apenas notícias (`resultados: []`); `--vertical all` retorna web e notícias na mesma sessão Chrome
- Vertical news roteia EXCLUSIVAMENTE pelo Chrome (SERP exige JavaScript) — sem fallback HTTP
- Batch multi-query aceito desde o GAP-WS-105 (`--queries-file` e múltiplas queries posicionais) — cada query roda sua própria sessão Chrome; no `deep-research` a vertical news é o PADRÃO (opt-out `--no-news`)
- Campos com `--vertical news|all` (agora o padrão): `noticias[].{posicao,titulo,url,fonte,data_relativa,thumbnail}`, `quantidade_noticias` e `metadados.vertical_usada`; com fetch ON, news pode ter `conteudo` / `tamanho_conteudo` / `metodo_extracao_conteudo`
- Zero notícias legítimo classifica `causa_zero: vertical-sem-resultados` (exit 5, não 6)
- **v0.9.8 fetch de conteúdo:** LIGADO por padrão para **web + news** (FETCH_CAP=10); opt-out `--no-fetch-content`. (Claim histórico “fetch SOMENTE em `resultados[]`” é **supersedido pela v0.9.8**.)
- Metadados de agente: `chrome_path_resolvido`, `chrome_canal`, `usou_chrome` honesto (**não** telemetria)
- Flags de transporte são `global = true` — `--chrome-path` etc. funcionam depois de `deep-research`

```bash
timeout 90 duckduckgo-search-cli --vertical news "noticias brasil" -q -f json | jaq '.noticias'
timeout 90 duckduckgo-search-cli --vertical all "rust release" -q -f json | jaq '{web: .quantidade_resultados, news: .quantidade_noticias}'
```


## Variáveis de Ambiente

Configuração de produto é **CLI + XDG apenas** (sem knobs de env de produto). Nomes históricos abaixo estão **removidos / não lidos** e não devem ser ensinados como config viva.

| Variável / knob | Status | Use em vez disso |
|---|---|---|
| Filtro de log de produto | CLI + XDG | `-v` / `-vv` / `-q`, ou `config set log_directive duckduckgo_search_cli=debug` (precedência: `-q` > `-v` > XDG > `info`) |
| Proxy | CLI + XDG | `--proxy URL` / `--no-proxy`, ou `config set proxy_url …` (**não** herda `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY`) |
| Caminho do Chrome | CLI + XDG | `--chrome-path PATH` ou `config set chrome_path …` (`CHROME_PATH` não é config de produto) |
| `RUST_LOG` | **Não é config de produto** | Use CLI `-v`/`-q` ou XDG `log_directive` |
| `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` | **Não lidas** | `--proxy` / XDG `proxy_url` |
| `DUCKDUCKGO_CHROME_VISIBLE` | **Removida** | `--chrome-visible` |
| `DUCKDUCKGO_CHROME_HEADLESS` | **Removida** | `--chrome-headless` |
| `DUCKDUCKGO_CHROME_XVFB` | **Removida** | Xvfb privado é automático no Linux |
| `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` | **Removida** (não lida) | Chrome obrigatório via feature `chrome`; sem Chrome → exit 2 |
| `DUCKDUCKGO_ZERO_CAUSE_STRICT` | **Removida** | `--no-zero-cause-strict` para exit 5 legado |


## Formatos de Saída
- `json` (padrão em pipes): schema canônico com `resultados[]` e `metadados`, ordem de campos estável
- `text`: bloco legível `NN. Título\n   URL\n   snippet`
- `markdown`: `- [Título](URL)\n  > snippet`
- `--stream` ou `-f ndjson`: NDJSON multi-query — uma linha compacta por LF. Consumer fecha cedo → exit **141** (v1.0.1); reap one-shot do Chrome ainda roda


## Exit Codes

| Código | Significado |
|---|---|
| 0 | Sucesso |
| 1 | Erro de runtime (rede, parse, I/O) |
| 2 | Configuração inválida (flag fora de faixa, proxy malformado) |
| 3 | Bloqueio DuckDuckGo (anomalia HTTP 202) |
| 4 | Timeout global excedido |
| 5 | Zero resultados em todas as queries |
| 6 | Bloqueio suspeito (zero resultados com causa não legítima, v0.8.0+) |
| 141 | Broken pipe (consumer de stdout fechou cedo; v1.0.1 stream-safe) |


## Troubleshooting
### Bloqueio anti-bot (exit 3)
- Aumente `--retries` para dar mais tentativas ao cliente
- Rotacione user-agents via `init-config` editando `user-agents.toml`
- Adicione `--proxy socks5://127.0.0.1:9050` para rotacionar o IP de saída
- Os perfis de browser da v0.6.0 reduzem este problema ao imitar sessões reais

### Rate limit HTTP 429
- Reduza `--per-host-limit` para diminuir concorrência por host
- Ative `--match-platform-ua` para filtrar UAs ao SO atual
- Use `--proxy` para rotacionar o IP de saída

### Timeout (exit 4)
- Aumente `--global-timeout` para pipelines lentos
- Aumente `-t` para requests individuais em redes instáveis
- Verifique conectividade antes de re-executar

### Zero resultados (exit 5)
- Aguarde 60 segundos, pois normalmente é rate-limiting temporário
- Confira `--lang` e `--country` para garantir localização correta
- Confirme Chrome utilizável (`--probe` / `--chrome-path` / `--proxy`) — Lite e `--allow-lite-fallback` **não** remedeiam (GAP-WS-113 / v0.9.4)
- Revise `--time-filter` se estiver restringindo o período

### Chromium / Xvfb / perfis temp órfãos após muitas invocações
- Atualize para **1.0.1+** com `cargo install duckduckgo-search-cli --locked --force` para reap pipe-safe (**SIG_IGN** em SIGPIPE + `ensure_oneshot_cleanup` em todas as saídas, inclusive `| head` cedo / BrokenPipe → exit **141**)
- One-shot de **processo** em **0.9.6** (ADR-0017); one-shot de **disco** + perfis `ddg-chrome-*` em **1.0.0** (ADR-0020); **1.0.1** fecha o buraco de órfão em pipe cedo (Pass 52)
- Novas invocações reaping da árvore e removem o perfil; a próxima run varre só `ddg-chrome-*` stale (nunca bulk-delete de `.tmp*` estrangeiro nem `org.chromium.Chromium.*`)
- Órfãos de processo (pré-0.9.6) ou dirs `.tmp*` genéricos (pré-1.0.0) **não** são mass-auto-mortos: identifique Chrome de automação pelo `user-data-dir` na cmdline e encerre PIDs / remova dirs uma vez se necessário
- Prefira supervisores com **SIGTERM** primeiro (GNU `/usr/bin/timeout`); **SIGKILL/OOM** nu é limite residual do SO

### Path rejeitado em --output (exit 2)
- Caminhos com `..` são rejeitados para prevenir travessia de diretório
- Caminhos para diretórios de sistema (`/etc`, `/usr`, `/bin`) são bloqueados
- Use caminhos sob o diretório home, `/tmp` ou diretório de trabalho atual

### Pipe para jaq retorna vazio
- Verifique `echo ${PIPESTATUS[*]}` após o pipe
- Se o primeiro número for diferente de zero, o CLI errou antes de produzir output
- Sempre passe `-q -f json` ao usar pipe para manter stdout limpo

### Timeout wrapper Rust sombreia GNU coreutils
- O binário `~/.cargo/bin/timeout` (crate Rust `timeout-cli` v0.1.0) sombreia o GNU coreutils e re-parseia args do subprocesso
- Quando o operador executa `timeout 60 duckduckgo-search-cli -vv -q -f json "query"`, o `timeout` Rust interpreta `-v` e `-q` como flags próprias
- Sintoma: exit 2 com mensagem `the argument '--verbose' cannot be used multiple times`
- **Workaround**: use o binário GNU explicitamente: `/usr/bin/timeout 60 duckduckgo-search-cli -vv -q -f json "query"`
- Para detectar qual `timeout` está no PATH: `command -v timeout` e `file $(command -v timeout)`
- Script auxiliar em `scripts/detect-timeout-wrapper.sh` automatiza a detecção

## Skill de Agente
- Este repositório entrega uma Claude Agent Skill pronta para uso imediato
- Instalação em dois comandos:

```bash
git clone https://github.com/danilo-aguiar-br/duckduckgo-search-cli
cp -r duckduckgo-search-cli/skill/duckduckgo-search-cli-pt ~/.claude/skills/
cp -r duckduckgo-search-cli/skill/duckduckgo-search-cli-en ~/.claude/skills/
```

- Reinicie o Claude Code ou recarregue o Agent SDK para ativar
- Auto-ativação: o Claude dispara a skill quando o usuário menciona pesquisa ou verificação


## Documentação

| Guia | Por que importa |
|---|---|
| [`docs/AGENT_RULES.md`](docs/AGENT_RULES.md) | 30+ regras DEVE/JAMAIS para qualquer LLM invocar a CLI em produção |
| [`docs/COOKBOOK.md`](docs/COOKBOOK.md) | 15 receitas copy-paste para pesquisa, ETL, monitoramento e extração de conteúdo |
| [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) | Snippets para 16 agentes: Claude Code, Codex, Gemini CLI, Cursor, Windsurf, Aider e mais |


## Notas de Migração
### v0.3.x para v0.4.0
- `--num` agora é `15` por padrão (antes era o payload completo de uma página, ~11)
- Quando `--num > 10` e `--pages` permanece no default `1`, o CLI eleva automaticamente `--pages` para `ceil(num / 10)` limitado a 5
- Schema JSON inalterado: `resultados[]`, `metadados` e `titulo_original` permanecem idênticos à v0.3.x

Veja o [CHANGELOG](CHANGELOG.md) para o histórico completo de versões.


## Schema JSON (v0.8.9)

### Guia de migração para consumidores

Ao parsear o envelope JSON, consumidores DEVEM tratar estas mudanças
de schema:

| Versão | Caminho do campo | Tipo | Padrão | BC |
|---|---|---|---|---|
| v0.7.10 | `metadados.pre_flight_disparado` | bool | `false` | Aditivo |
| v0.8.9 | `noticias[]` | array | (ausente) | Aditivo — somente com `--vertical news\|all` |
| v0.8.9 | `quantidade_noticias` | u32 | (ausente) | Aditivo — somente com `--vertical news\|all` |
| v0.8.9 | `metadados.vertical_usada` | string | (ausente) | Aditivo — somente com `--vertical news\|all` |
| v0.8.9 | `noticias[]` (deep-research) | array | `[]` | Aditivo — SEMPRE presente no envelope do deep-research (GAP-WS-105) |
| v0.8.9 | `quantidade_noticias` (deep-research) | number | `0` | Aditivo — SEMPRE presente no envelope do deep-research (GAP-WS-105) |
| v0.8.9 | `metadados.total_noticias_unicas` (deep-research) | number | `0` | Aditivo (GAP-WS-105) |
| v0.8.9 | `metadados.sub_queries[].quantidade_noticias` / `.news_indisponivel` | number / bool | (ausente) | Aditivo — opcional (GAP-WS-105) |

Nenhum campo removido. Nenhum campo deprecado. A flag
`--require-results` do `deep-research` é local a esse subcomando e
emite exit code `70` (EX_SOFTWARE) quando os resultados agregados são
zero, em vez de `0` (zero-resultado silencioso).

Quando o endpoint probe-deep detecta um CAPTCHA, o envelope JSON
agora inclui o marcador específico que casou:

```json
{
  "type": "probe_deep",
  "cascata_motivo": "cloudflare",
  "sugestao_mitigacao": "Cloudflare challenge detected (marker: cf-turnstile). Re-run with --pre-flight..."
}
```

Consumidores devem tratar sentinelas que começam com `<` (ex.
`<ghost-block-no-marker>`, `<empty-body>`, `<no-marker>`) como
marcadores não-literais e omiti-las de listas voltadas ao usuário.

## Notas de Migração (v0.8.8 → v0.8.9)

- Flag nova `--vertical <web|news|all>` (padrão histórico `web`; **v0.9.8 padrão `all`**). `news` e `all` roteiam exclusivamente pelo Chrome (a SERP de notícias exige JavaScript; NÃO há fallback HTTP). Desde o GAP-WS-105 batches multi-query são ACEITOS — e o `deep-research` varre news por padrão (opt-out `--no-news`)
- Campos novos opcionais no envelope, emitidos SOMENTE com `--vertical news|all`: `noticias[]` na raiz com `posicao`, `titulo`, `url` (garantidos) e `fonte`, `data_relativa`, `thumbnail` (opcionais); `quantidade_noticias` na raiz; e `metadados.vertical_usada`. O modo web padrão permanece byte-idêntico à v0.8.8
- Valor novo de `causa_zero`: `vertical-sem-resultados` (zero notícias legítimo ⇒ exit 5, não 6). O total de zero resultados agora soma `quantidade_noticias`, então runs news-only com artigos saem com exit 0
- Fetch de conteúdo (default LIGADO na v0.9.8) atua sobre **web + news** (teto 10 URLs); opt-out com `--no-fetch-content`; news pode trazer `conteudo`
- Metadados agent: `chrome_path_resolvido`, `chrome_canal` (contrato local — não telemetria)
- GAP-WS-105 (mesmo release): o `deep-research` varre a vertical news por PADRÃO — cada sub-query roda como `--vertical all` na própria sessão Chrome. Opt-out com `--no-news`. **v0.9.4 GAP-WS-113:** sem Chrome utilizável a CLI **falha exit 2 fail-closed** — sem auto `--no-news` (a auto-degradação GAP-WS-106 da v0.9.0 é histórica e foi supersedida)
- Campos novos no envelope do deep-research, SEMPRE presentes: `noticias[]` na raiz (RRF exclusivo de news, dedupe por URL canônica, desempate por recência de `data_relativa`) com `posicao`, `titulo`, `url`, `score`, `ocorrencias` garantidos e `fonte`, `data_relativa`, `thumbnail` opcionais; `quantidade_noticias` na raiz; `metadados.total_noticias_unicas`; opcionais `metadados.sub_queries[].quantidade_noticias` e `.news_indisponivel`
- Síntese dual: com `--synthesize` o relatório ganha a seção "Notícias recentes" (~30% do `--budget-tokens`, web mantém ~70%); formato inalterado com `--no-news` ou zero notícias
- Exit codes do deep-research: 0 quando web OU news produziram resultados; 5 somente quando AMBOS estão vazios

```bash
timeout 90 duckduckgo-search-cli --vertical news "noticias brasil" -q -f json | jaq '.noticias'
timeout 90 duckduckgo-search-cli --vertical all "rust release" -q -f json | jaq '{web: .quantidade_resultados, news: .quantidade_noticias, vertical: .metadados.vertical_usada}'
timeout 180 duckduckgo-search-cli -q -f json deep-research "rust security advisories" | jaq '.noticias[:5]'
timeout 180 duckduckgo-search-cli -q -f json deep-research "tokio release" --no-news | jaq '.quantidade_noticias'
```


## Notas de Migração (v0.6.4 → v0.6.5)

- **Zero breaking changes.** Todas as flags CLI, schemas JSON e exit codes de v0.6.4 permanecem inalterados.
- **Build Windows corrigido (MP-26)**: `cargo install duckduckgo-search-cli` agora funciona no Windows. O build da v0.6.4 quebrava no Windows porque `windows-sys 0.59+` mudou `HANDLE` de `isize` para `*mut c_void` e o código fazia casts `handle as isize`. v0.6.5 usa `!handle.is_null() && handle != INVALID_HANDLE_VALUE`.
- **local multi-platform checks verde novamente (CI-01)**: v0.6.4 foi publicada com CI falhando em todos os 3 SOs por 6 erros de clippy latentes. v0.6.5 corrige todos e roda `cargo clippy --all-targets --all-features -- -D warnings` no CI.
- **Sem novas flags CLI ou campos JSON.** Todas as mudanças de v0.6.5 são internas ou melhorias de build/qualidade.
- **Uma nova dependência transitiva**: `indicatif 0.18` (ProgressBar em crawls longos; auto-esconde em pipes).
- **WS-12 circuit breaker**: quando `--fetch-content --parallel` é usado, o novo circuit breaker per-host abre após 3 falhas consecutivas e bloqueia requisições para esse host por 30 segundos antes de permitir uma probe. Isso protege crawls longos de falhas em cascata em um único domínio morto.
- **333 testes passando** (243 unit + 90 integration + 6 doc). 6 erros de clippy corrigidos, 5 novos property tests, 4 novos testes de circuit breaker, 1 novo teste wiremock de Retry-After.


## Notas de Migração (v0.6.x → v0.7.0)

- **Zero breaking changes.** Todas as flags CLI existentes, schemas JSON de `SearchOutput` e `MultiSearchOutput`, e exit codes de v0.6.x permanecem byte-for-byte idênticos em v0.7.0.


## Notas de Migração (v0.7.9 → v0.7.10)

- **Zero mudanças quebrantes.** Todas as flags CLI, schemas JSON de saída e exit codes de v0.7.9 permanecem inalterados.
- **`--identity-profile` agora propaga o pino de identidade para failure paths (GAP-WS-60)**: helper novo `identity_tag_for_cli_identity` em `src/identity.rs` reutiliza `IdentityProfile::tag()` canônico para garantir paridade de formato entre `failure_output` (pipeline.rs) e `error_output` (parallel.rs). Antes da correção, o pino `identidade_usada` era `null` em qualquer falha. Formato canônico: `<family>-<platform>-<seed16hex>` (ex.: `chrome-linux-33333333cccc0003`, `firefox-linux-99999999cccc0009`, `safari-macos-bbbbbbbbeeee000b`).
- **`--require-results` em `deep-research` (P4)**: quando setado e o fan-out agrega zero resultados, o subcomando retorna exit 4 (`GLOBAL_TIMEOUT`) com stderr `deep-research produced zero results for query "..."; --require-results set → exiting non-zero`. Fecha o padrão de descarte silencioso (GAP-WS-1114).
- **`--pre-flight` scheduler automático (P5)**: integrado em `execute_single_search`. Quando setado, o pipeline roda um probe mínimo em ~140ms antes da busca real. Em ambiente bloqueado, aborta com `pre_flight_blocked` e exit 3 sem gastar a query real. Default `false` para preservar comportamento v0.7.8.
- **`--probe-deep` standalone retorna exit 3 em captcha (B4)**: antes retornava exit 0 mesmo com `status: "captcha"` no JSON. Agora branching no exit code é confiável.
- **`--pre-flight` não emite mais dois JSON concatenados no stdout (B1)**: consumidores com `| jaq '.resultados'` não quebram mais.
- **`pre_flight_blocked` retorna exit 3 (B2)**: antes retornava exit 0 (SUCCESS), violando a tabela `EXIT CODES` do `--help`.
- **`--global-timeout` agora é `global = true` (B3)**: aceito em subcomandos como `deep-research`. Antes `deep-research --global-timeout 30 query` falhava com `unexpected argument`.
- **`cargo bench --bench pre_f_light_latency` corrigido (GAP-AUD-002)**: adicionado `[[bench]] harness = false` em `Cargo.toml`. Antes o harness padrão reportava `running 0 tests` em vez de rodar Criterion.
- **local pre-publish checklist novo (regra 1264)**: 7 gates sequenciais antes de `cargo publish`. Bloqueia publicação se qualquer gate falhar.
- **`insta = "1"` adicionado e snapshot test para os 8 marcadores Cloudflare 2026 (P6/P17)**: regressões viram diff de snapshot.
- **`src/proxy_detection.rs` novo módulo (P7)**: heurística de ISP BR (Vivo Fiber, Gigaweb, Cloudflare, Corporate) com 8 testes.
- **`src/ddg_class_watch.rs` novo watchdog (P19)**: monitora templates DDG em runtime.
- **`skill/duckduckgo-search-cli-{en,pt}/eval-queries.json` +4 queries (q47-q50)**: smoke test de `--version 0.7.10`, feature-test de pino, feature-test de pre-flight, feature-test de require-results.
- **Contagem de testes: 370 (349 lib + 21 integration)**, 0 clippy warnings, 0 fmt diff, cobertura 86.91% (gate ≥80%).

## Notas de Migração (v0.7.8 → v0.7.9)

- **Zero mudanças quebrantes.** Todas as flags CLI, schemas JSON de saída e exit codes de v0.7.8 permanecem inalterados.
- **`detectar_interstitial` agora classifica body sub-4KB sem `result-page-signal` como Cloudflare (GAP-WS-58)**: threshold conservador de 4KB evita falsos positivos. Helper `has_result_page_signal` checa classes DDG (`nrn-react-div`, `react-article`, `module--results`, `js-react-aria-results`).
- **5 marcadores Cloudflare novos + 1 marker DDG novo (GAP-WS-59)**: `anomaly.js`, `botnet`, `cf-error-code`, `cf-ray`, `Performance & Security by Cloudflare`, `Unfortunately, bots` parcial. Markers 2026 cobertos.
- **`--allow-lite-fallback` e `--pre-flight` viraram `global = true` (GAP-WS-59)**: fecham o caminho `unexpected argument` em subcomandos como `deep-research`.
- **`Config.pre_flight` adicionado com default `false`**: opt-in para preservar comportamento v0.7.8.
- **Helper `detectar_interstitial_com_match` (P1)**: retorna `(&'static str, InterstitialKind)` com marker literal em vez de detecção heurística anônima.
- **Helper `sugestao_mitigacao_com_marker` (P4b)**: injeta o marker real (ex.: `cf-challenge`, `anomaly-modal`) na mensagem de mitigação.
- **Campo `SearchMetadata.pre_flight_fired: bool` (P3)**: presente no envelope quando `cfg.pre_flight == true && ghost-block`.


## Notas de Migração (v0.7.7 → v0.7.8)

- **Zero mudanças quebrantes.** Todas as flags CLI, schemas JSON de saída e exit codes de v0.7.7 permanecem inalterados.
- **Renovação do detector anti-bot (GAP-WS-50, WS-51, WS-52; histórico pré-0.9.4)**: a função `detectar_interstitial` reconhece o interstitial DDG anomaly-modal (classes CSS `anomaly-modal__mask` e `anomaly-modal__title`, texto `Unfortunately, bots use DuckDuckGo too.`, challenge `anomaly.js?cc=botnet`). O subcomando `--probe-deep` usa query de calibração longa. **Nota (v0.9.4 GAP-WS-113):** `--allow-lite-fallback` é **no-op legado**; o fallback html→lite deixou de ser caminho de sucesso em produção.
- **Verbose `-vv` e `-vvv` agora suportados (GAP-WS-53)**: `--verbose` usa `ArgAction::Count`. Mapeamento: (sem flag) = `info`, `-v` = `debug`, `-vv`+ = `trace`. Filtro de log de produto é CLI `-v`/`-q` + XDG `log_directive` apenas (não `RUST_LOG`). Exemplos:
  - `duckduckgo-search-cli -v "rust async"` — logs nível debug
  - `duckduckgo-search-cli -vv "rust async"` — logs nível trace
  - `duckduckgo-search-cli -vvv "rust async" 2>debug.log` — logs nível trace para forense profunda
  - `duckduckgo-search-cli config set log_directive duckduckgo_search_cli=debug` — filtro XDG persistente
- **`--retries N` agora é honrado (GAP-WS-57)**: antes o valor estava hard-coded em 1, então `--retries 5` silenciosamente se comportava como `--retries 1`. A flag agora é lida de `Config.retries` com clamp em `[1, 10]` para evitar abuso (`--retries 999` dispara anti-bot). Exemplo: `duckduckgo-search-cli --retries 5 "rust async runtime"` retenta até 5 vezes (fallback Lite via `--allow-lite-fallback` é **no-op desde v0.9.4**).
- **`--allow-lite-fallback` (GAP-WS-52; histórico pré-0.9.4)**: historicamente habilitava fallback html→lite ciente de captcha. **v0.9.4 GAP-WS-113:** flag mantida por BC de scripts, mas é **no-op**; SERP permanece HTML Chrome. Exemplos históricos:
  - `duckduckgo-search-cli --probe-deep --allow-lite-fallback -q -f json` — pre-flight check com opt-in de auto-fallback
  - `duckduckgo-search-cli --allow-lite-fallback --retries 3 "long tail query" 2>cascata.log` — auto-fallback ativado, 3 retentativas por request, log do motivo da cascata em stderr
- **Subcomando `buscar` agora é hidden (GAP-WS-56)**: a forma canônica continua sendo a invocação top-level (`duckduckgo-search-cli "query"`). O subcomando `buscar` continua funcional mas não aparece em `--help`. O help de `buscar --help` deixou de duplicar o help global.
- **Supply chain (GAP-WS-54)**: `scraper` bumped de 0.20 para 0.27, o que remove transitivamente o `fxhash 0.2.1` unmaintained (RUSTSEC-2025-0057). `cargo audit --deny warnings` agora é gate local rígido em gates locais. `async-std` (RUSTSEC-2025-0052) continua apenas na feature opcional `chrome`.
- **Fix de drift de doc (GAP-WS-55)**: o comentário sobre `wreq` no `Cargo.toml` foi reescrito para refletir a decisão real (pin em `wreq 6.0.0-rc.29` mais os três pins diretos para `wreq-util`, `brotli-decompressor`, `alloc-no-stdlib`), não a regressão que nunca aconteceu mencionada no comentário obsoleto.
- **Contagem de testes: 305 (292 lib + 13 integration)**, 0 clippy warnings, 0 fmt diff, 0 cargo-deny warnings, `cargo doc --offline --no-deps` limpo.

## Notas de Migração (v0.7.1 → v0.7.2)

- **Zero breaking changes.** Todas as flags CLI, schemas JSON de saída e exit codes de v0.7.1 permanecem inalterados.
- **Fix de advisory de segurança (RUSTSEC-2026-0009)**: denial-of-service no `time 0.3.40` via RFC 2822 stack exhaustion estava sendo puxado transitivamente via `cookie_store 0.22.0` → `reqwest 0.12.28`. v0.7.2 pina `time = "0.3.47"` como dep direta para sobrescrever a constraint transitiva.
- **Migração do `rand` 0.10**: dev-deps (proptest 1.11+, getrandom 0.4+) unificadas em rand 0.10 e os métodos de conveniência movidos de `Rng` para `RngExt`. Todos os call sites internos atualizados: `random_range`, `random_bool`, `random`, e `IndexedRandom::choose`.
- **Bump de MSRV**: `rust-version` elevado de 1.75 para 1.88 (exigido por `time 0.3.47+` e `rand 0.10`).
- **Fix de higiene de CI**: 6 erros latentes do clippy que estavam quebrando silenciosamente a matriz de CI em v0.7.1 são agora capturados por `cargo clippy --all-targets --all-features -- -D warnings`.

## Notas de Migração (v0.7.0 → v0.7.1)

- **Zero breaking changes.** Todas as flags CLI, schemas JSON de saída e exit codes de v0.7.0 permanecem inalterados.
- **Migração de dependência (interna)**: `rand` atualizado de `0.8` para `0.9` para alinhar com `proptest 1.11+` (dev-dep). Todos os call sites internos atualizados:
  - `Rng::gen_range` → `Rng::random_range` (7 sites)
  - `Rng::gen_bool` → `Rng::random_bool` (2 sites)
  - `Rng::gen::<T>()` → `Rng::random::<T>()` (1 site)
  - `rand::thread_rng()` → `rand::rng()` (4 sites)
  - `rand::seq::SliceRandom::choose` → `rand::seq::IndexedRandom::choose` para chamadas `.choose()` em slices; `IteratorRandom::choose` mantido para chamadas `.choose()` em iterators
- **Bump de MSRV**: `rust-version` elevado de `1.75` para `1.85` para satisfazer o MSRV do `rand 0.9` e a onda de deps edition-2024 (`assert_cmd 2.2+`, `blake3 1.8+`, `clap 4.6+`, `proptest 1.11+`, `chrono 0.4.41+`, `idna 1.1+`, `icu_* 2.0+`, `home 0.5.11+`, `async-lock 3.4+`, etc.).
- **Limpeza do builder reqwest**: removidas as chamadas `ClientBuilder::gzip(true)` e `.brotli(true)` (métodos removidos em `reqwest 0.12+`; descompressão agora é automática via header `Accept-Encoding`).
- **higiene local**: dois avisos do actionlint (removed with Actions) shellcheck corrigidos:
  - `local gates:520` — command substitution `$(date ...)` para aspas em `"\$(date ...)"` (SC2046)
  - `local release process:505` — adicionado prefixo `--` ao glob `sha256sum -- *` (SC2035)
- **Ignore de advisory de segurança**: `RUSTSEC-2026-0009` (DoS no time 0.3.40 via stack exhaustion em RFC 2822) adicionado à lista ignore do `deny.toml`. A correção em `time 0.3.47` exige `rust-version 1.88+` que não conseguimos satisfazer no MSRV atual. Impacto: a CLI só faz parse de headers `Date` de respostas HTTP sob flags explícitas `--lang`/`--country` do usuário; o cap de tamanho do body da resposta já limita o comprimento da entrada.
- **392 testes passando** (279 lib + 12 doc + 101 integration). 0 avisos clippy, 0 avisos doc, 0 diferenças de fmt, 4 gates do cargo-deny verdes, `cargo publish --dry-run` limpo.
- **Novo subcomando público `deep-research`** para pesquisa multi-hop por LLM. Operadores que não invocam `deep-research` não veem mudança observável.
- **Quatro novos módulos públicos** expostos em `lib.rs` — `deep_research`, `decomposition`, `aggregation`, `synthesis` — composíveis a partir de crates downstream.
- **Novas dependências diretas** no `Cargo.toml`: `url = "2"`, `regex = "1"`, e `proptest = "1"` (somente dev). Todas as três são adições puras; nenhuma dependência foi atualizada ou removida.
- **Sem migração de schema JSON obrigatória**: os schemas `SearchOutput` e `MultiSearchOutput` permanecem inalterados.


## Notas de Migração (v0.6.3 → v0.6.4)

- **Zero breaking changes.** Todas as flags CLI, schemas JSON e exit codes de v0.6.3 permanecem inalterados.
- **Novas flags CLI (aditivas)**:
  - `--probe` — envia uma requisição mínima de pré-voo e reporta saúde em JSON
  - `--identity-profile` — fixa a sessão a uma identidade específica do pool de 12 identidades (`auto` por padrão para rotação adaptativa)
  - `--seed` — agora também controla rotação do pool de identidades (era só UA em v0.6.3)
- **Novos campos JSON de metadados (aditivos, `skip_serializing_if = "Option::is_none"`)**:
  - `metadados.identidade_usada` — tag de identidade (`<família>-<plataforma>-<16hex>`) usada para a resposta
  - `metadados.nivel_cascata` — nível de cascata (0..=4) atingido durante a requisição


## Destaques v0.6.5 (Windows HANDLE fix + gates locais verdes + circuit breaker)

v0.6.5 é uma release de qualidade focada em portabilidade Windows e higiene de CI. A maior melhoria prática é que **`cargo install duckduckgo-search-cli` agora funciona no Windows** pela primeira vez desde v0.6.4. Os 6 erros de clippy latentes que quebraram o CI em todos os 3 SOs em v0.6.4 também são corrigidos.

- **MP-26 (CRÍTICO)**: `src/platform.rs:51-69` reescrito para lidar com a mudança de ABI em `windows-sys 0.59+` (`HANDLE = *mut c_void`). Usa `INVALID_HANDLE_VALUE` de `windows_sys::Win32::Foundation` para a sentinela Win32 e `is_null()` para a verificação de nulidade.
- **CI-01**: 6 erros de clippy corrigidos — `doc_markdown` em 3 strings (`PowerShell`, `rules_rust.md`, `TempDir`), `needless_return`, `missing_debug_implementations` em `ChromeBrowser` e `CircuitBreakerMap`. `cargo clippy --all-targets --all-features -- -D warnings` passa.
- **WS-12 circuit breaker**: breaker per-host em `src/content_fetch.rs` (3 falhas → 30s de cooldown). Protege crawls `--fetch-content --parallel` contra falhas em cascata em domínios mortos.
- **WS-11 property tests**: 5 invariantes em `src/extraction.rs` (inputs vazios, positions densos, URLs absolutos, idempotência, sem panic em HTML malformado). Zero novas dependências.
- **WS-23 wiremock Retry-After**: teste de integração valida que o backoff de 429 respeita o header `Retry-After: 2`.
- **WS-25 indicatif ProgressBar**: `--fetch-content` mostra barra de progresso no stderr. Auto-esconde em pipes (sem contaminação do stdout JSON).
- **Lints FFI preventivos**: `improper_ctypes` e `improper_ctypes_definitions` agora são `deny` no `Cargo.toml`, bloqueando drift futuro de tipo FFI.
- **Adições ao CI**: smoke test `--version --help` em todos os 3 SOs; job `cargo build --no-default-features` para validar o build mínimo.


## Destaques v0.6.4 (WS-26 anti-bot)

v0.6.4 introduz um pool adaptativo de identidades anti-bot que endereça a causa raiz dos bloqueios HTTP 202/403/429 do DuckDuckGo. A versão anterior selecionava um único User-Agent no início e o reutilizava para toda a sessão, produzindo uma única fingerprint que sistemas anti-bot podiam classificar após a primeira requisição. O novo pool:

- Mantém 12 identidades (4 famílias de browser × 3 plataformas: Windows, macOS, Linux)
- Em bloqueio detectado (HTTP 202/403/429), rotaciona através de cascata de 5 níveis: mesma identidade → mesma família/plataforma diferente → família diferente/mesma plataforma → família+plataforma diferentes → aleatória
- Produz ordem de headers determinística via seed em `IdentityProfile::shuffled_headers()` (variantes de Accept-Language, variações de Sec-CH-UA-Arch, ordem aleatorizada)
- Reporta `identidade_usada` e `nivel_cascata` no NDJSON para visibilidade diagnóstica

Uso:

```bash
# Padrão — rotação adaptativa entre 12 identidades
duckduckgo-search-cli -q -n 10 -f json "query"

# Fixa uma identidade específica para testes reproduzíveis
duckduckgo-search-cli -q -n 10 -f json --identity-profile chrome-linux "query"

# Verificação de saúde pré-voo antes de lançar query real
duckduckgo-search-cli --probe

# Seed determinístico para debugar rotação anti-bot
duckduckgo-search-cli -q -n 10 -f json --seed 42 "query"
```


## Contribuindo
- Abra uma issue antes de criar um Pull Request para discutir a mudança proposta
- Leia os guias em `docs/` para entender a arquitetura antes de contribuir


## Licença
- Licenciado sob MIT OR Apache-2.0
- Escolha a licença que melhor atende às suas necessidades


## Notas de migração (v0.7.4 → v0.7.5)

- **Nenhuma mudança de runtime.** v0.7.5 é uma release de experiência de build e documentação: mesmas flags, mesmo schema JSON, mesmas dependências.
- **GAP-WS-29/30/31/32/33/34/35/36/37 fechados neste repositório.** O preflight do `build.rs` da v0.7.4 foi estendido para detectar também **CMake** (o crate `cmake` 0.1.58 precisa de `cmake.exe` no PATH ANTES de `enable_language(ASM_NASM)` ser avaliado), **compilador e linker MSVC** (`cl.exe`/`link.exe` — precisam de `Launch-VsDevShell.ps1` para configurar PATH, INCLUDE, LIB) e **Perl** (`perl.exe` para o gerador perlasm do BoringSSL). Novo preflight no `build.rs` aborta em segundos com a correção exata para cada uma das quatro ferramentas. Escape hatches: `DDG_SKIP_NASM_CHECK=1`, `DDG_SKIP_CMAKE_CHECK=1`, `DDG_SKIP_MSVC_CHECK=1`, `DDG_SKIP_PERL_CHECK=1`. Causa raiz: o sub-componente C++ CMake tools for Windows do Visual Studio Installer vem desmarcado por padrão — instalar apenas o workload C++ NÃO fornece CMake.
- **Helper estendido `scripts/install-windows.ps1`** — agora também detecta e auto-instala CMake (`winget install -e --id Kitware.CMake` ou choco) e Perl (`winget install -e --id StrawberryPerl.StrawberryPerl`), e reporta a instrução exata de instalação MSVC/`Launch-VsDevShell.ps1` (MSVC é grande demais para auto-instalar). Novo modo `--check-only` produz relatório tabular adequado para portões locais e suporte humano.
- **Novo `scripts/check-windows-toolchain.ps1`** — diagnóstico standalone (sem instalações) que verifica todas as 7 ferramentas (cargo, rustc, cmake, nasm, cl.exe, link.exe, perl) e emite saída texto ou JSON. Exit code 0 se todas presentes, 1 caso contrário. Use para tickets de suporte e portões locais.
- **Novo `docs/INSTALL-WINDOWS.pt-BR.md`** — guia passo-a-passo cobrindo 5 métodos de instalação (Visual Studio Installer + ferramentas standalone; tudo standalone via winget; apenas Chocolatey; script helper; diagnóstico standalone). Inclui troubleshooting para cada um dos 4 GAPs e os escape hatches `DDG_SKIP_*_CHECK`.
- **Documentação corrigida** — o claim falso de que "VS Build Tools com workload C++ fornece CMake" foi substituído em `docs/CROSS_PLATFORM.pt-BR.md`, `skill/duckduckgo-search-cli-pt/SKILL.md`, `llms.pt-BR.txt` e `llms-full.txt`. O workload C++ NÃO inclui o sub-componente C++ CMake tools — ele deve ser marcado manualmente no Visual Studio Installer.

## Notas de migração (v0.7.3 → v0.7.4)

- **Nenhuma mudança de runtime.** v0.7.4 é uma release de experiência de build e documentação: mesmas flags, mesmo schema JSON, mesmas dependências.
- **GAP-WS-28 fechado neste repositório.** `cargo install` no Windows MSVC nativo sem NASM falhava MINUTOS após o início do build com o erro críptico `CMake Error: No CMAKE_ASM_NASM_COMPILER could be found`. Um novo preflight no `build.rs` agora falha em SEGUNDOS com a correção exata (`winget install -e --id NASM.NASM`, ajuste de PATH, ou `scripts/install-windows.ps1`). Causa raiz: o BoringSSL exige assembly criptográfico em formato NASM a menos que `OPENSSL_NO_ASM` esteja definido, e o ramo do `btls-sys` v0.5.6 que o define para Windows é inalcançável em builds nativos (early return quando host == target no build script dele). Defina `DDG_SKIP_NASM_CHECK=1` para pular o preflight (ex.: toolchain files customizados).
- **Novo helper `scripts/install-windows.ps1`** — detecta NASM, instala via winget (fallback choco), corrige o PATH da sessão e roda `cargo install duckduckgo-search-cli --locked` repassando argumentos extras.
- **endurecimento dos gates locais**: os jobs Windows de gates locais agora verificam/instalam NASM explicitamente em vez de depender da imagem do host local.

## Notas de migração (v0.7.2 → v0.7.3)

- **QUEBRA DE AMBIENTE DE BUILD (apenas builds do código-fonte)**: A stack TLS mudou de `rustls` para BoringSSL via `wreq 6.0.0-rc.29`. Compilar do código-fonte no Linux agora requer `cmake`, `perl`, `pkg-config` e `libclang-dev`; no Windows MSVC requer o assembler NASM (`winget install -e --id NASM.NASM`), o **sub-componente C++ CMake tools for Windows** (selecionado manualmente no Visual Studio Installer — NÃO incluído por default no workload C++; ver `docs/INSTALL-WINDOWS.md` para passo a passo), o Strawberry Perl (`winget install -e --id StrawberryPerl.StrawberryPerl`) e a toolchain MSVC (cl.exe, link.exe, configurada via `Launch-VsDevShell.ps1`) no Windows MSVC. Atenção: `cargo install` SEMPRE compila do código-fonte — o crates.io não distribui binários pré-compilados — então esses pré-requisitos valem para todo usuário de `cargo install`, não apenas para o CI. Usuários Windows podem rodar `scripts/install-windows.ps1`, que instala NASM, CMake e Perl automaticamente quando ausentes (MSVC não é auto-instalado — operação intrusiva). Sem o sub-componente C++ CMake tools o build falha com `failed to execute command: program not found / is cmake not installed?`; sem o NASM falha com `No CMAKE_ASM_NASM_COMPILER could be found` (ver `gaps.md` GAP-WS-28/29/30/31/36). A matrix `local release process` instala os pacotes Linux automaticamente nos jobs Linux.
- **GAP-WS-27 fechado**: O interstitial de CAPTCHA no macOS está corrigido. Mesma query que retornava `quantidade_resultados: 0` na v0.7.2 retorna 5 resultados na v0.7.3 na mesma máquina. Ver `gaps.md` e `docs/decisions/0001-tls-boring-via-wreq.md`.
- **Novas flags CLI (aditivas)**:
  - `--no-warmup` — pula o warm-up `GET https://duckduckgo.com/` antes da primeira query real
  - `--no-cookie-persistence` — mantém cookies em memória apenas; nunca grava `cookies.json` em disco
  - `--cookies-path <PATH>` — sobrescreve o path XDG padrão do cookie jar
  - `--probe-deep` — executa uma query real e classifica o body como `ok` ou `captcha` baseado em marcadores Cloudflare e DuckDuckGo
  - `--allow-lite-fallback` — **histórico (pré-0.9.4)** opt-in html→lite; **v0.9.4 GAP-WS-113:** no-op legado
- **Novo estado persistente: cookie jar**: Um arquivo `cookies.json` agora é gravado em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). Permissões Unix são `0o600` (owner read+write only). **Trate este arquivo como trataria uma credencial** — ver `SECURITY.pt-BR.md`. Use `--no-cookie-persistence` para desabilitar.
- **Zero mudanças no schema JSON de saída**. Todos os campos da v0.7.2 permanecem presentes.
- **Novas dependências**: `wreq 6.0.0-rc.29`, `wreq-util 3.0.0-rc.12`, mais as transitivas `boring2 4.15.11`, `webpki-root-certs 1.0.7` e a toolchain C do BoringSSL.
- **Dependências removidas**: `reqwest 0.12.28`. `time 0.3.47` não é mais dep direta — puramente transitiva agora.
- **Contagem de testes: 292 lib** (era 279 na v0.7.2). +13 novos testes em `session_warmup` (5), `wreq_cookie_adapter` (3), e `probe_deep` (5). 0 warnings de clippy, 0 diff de fmt, 2 warnings de cargo-deny (RUSTSEC-2025-0057 + RUSTSEC-2025-0052, ambos já na lista de ignore).
- **Tamanho do binário**: +20 MB (BoringSSL é estaticamente vinculado). Tempo de build de release: ~40s mais longo que v0.7.2.


## Troubleshooting adicional (v0.7.3+)

1. **CAPTCHA interstitial detectado (v0.7.3+)** — rode `duckduckgo-search-cli --probe-deep -q -f json` para classificar o body da resposta. Se `status` for `captcha`, a resposta está bloqueada. O probe também reporta `sugestao_mitigacao` com próximos passos concretos (rotacionar proxy, trocar endpoint, back off). Trate o cookie jar como credencial: o arquivo `cookies.json` é gravado com permissões 0o600 e contém cookies de sessão do DuckDuckGo.
2. **Cookie jar crescendo sem controle** — cada invocação adiciona um cookie novo. O arquivo é reescrito inteiro a cada invocação, então o tamanho se mantém proporcional ao número de cookies únicos. Para resetar, apague o arquivo manualmente.
