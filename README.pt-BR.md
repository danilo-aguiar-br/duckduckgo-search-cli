# duckduckgo-search-cli

> **Idioma:** este arquivo é o **SSOT em português (pt-BR)**. O inglês está **somente** em [`README.md`](README.md) — sem monólito bilíngue embutido no README EN.

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
- Schema JSON estável com `results[]` e `metadata` (wire EN na v1.0.2 — ADR-0027); chaves PT legadas via `--wire-keys pt`

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
- **v0.8.0+ / ADR-0016 / ADR-0022 / ADR-0026 — Transporte Chrome nativo.** SERP de produção usa a stack TLS do Chrome no host (evita assinatura TLS de biblioteca/`rustls` que o Cloudflare bloqueia). **Proibido** spoof sintético de hardware fingerprint (canvas/WebGL/áudio). **Sempre mudo desde a v1.0.2** (`--mute-audio` + política de autoplay — ADR-0026; sem unmute) para deep-research não tocar mídia nos alto-falantes. HTTP residual (harness) = rustls + `aws-lc-rs` (ADR-0021). v0.8.7+ Xvfb + mitigação de sinais de automação; v0.9.3+ headless=new no macOS/Windows.
- **v0.9.6 / v1.0.0 / v1.0.1 — One-shot.** Agentes podem invocar N vezes sem acumular Chromium/Xvfb órfãos. Desde a **v1.0.0**, perfis Chrome usam o prefixo auditável `ddg-chrome-*` (não `.tmp*` genérico) e são removidos no exit cooperativo; o sweep da próxima run limpa só esse prefixo. **v1.0.1** endurece reap pipe-safe (`ensure_oneshot_cleanup`, SIG_IGN em SIGPIPE) para que `| head` cedo ainda reap `ddg-chrome-*`. Reap de árvore+disco em sucesso, erro, timeout, SIGINT, SIGTERM e broken pipe. Ver ADR-0017 + ADR-0020.
- **v0.9.8 — Agent-ready.** Padrão `--vertical all` (web+news); fetch de conteúdo LIGADO (opt-out `--no-fetch-content`); multi-canal Flatpak; flags de transporte globais após `deep-research`; metadados de agente `chrome_path_resolved` / `chrome_channel` (não telemetria). Ver ADR-0018.
- **v1.0.0 — Disco + contrato estável.** One-shot processo e disco; política dura (nunca bulk-rm `.tmp*` / `org.chromium.Chromium.*`); sem telemetria remota.
- **v1.0.1 — Pass 52.** Dual `config get`/`set`/`unset` + `config effective`; `-f ndjson` alias de `--stream`; exit **141** em broken pipe; wire PT na serialização + aliases EN na desserialização (ADR-0023, supersedido no serialize pela v1.0.2); false anti-bot de news corrigido; config só CLI+XDG.
- **v1.0.2 — Wire EN + agent ops + mute + budget.** Serialização em inglês (ADR-0027) + `--wire-keys`; agent ops (`--fields`/`--filter`/`--sort`/`--count-only`); RuntimeConfig SSOT; `budget_profile`; `chrome_session_retries`; no-warmup fail-closed; cgroup opt-in; mute obrigatório (ADR-0026); budget dual/contention + `--print-budget` + doctor dual; defaults `max-sub-queries=3` / `fetch-content-cap=4`. Migração: [`docs/MIGRATION.pt-BR.md`](docs/MIGRATION.pt-BR.md) · [EN](docs/MIGRATION.md).


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


## O que há de novo na v1.0.5 (2026-08-10)
- Feche a classe por régua, não por lista — `--probe` e `--probe-deep` continuavam ignorando todo operador agent-native depois da v1.0.4.
- Meça o defeito: na v1.0.4 `--count-only`, `--limit 1`, `--fields status` e `--truncate-content 5` devolveram 633 bytes contra uma linha base de 633, com exit 0.
- Roteie o probe pelo projetor e devolva o código de saída em `emit_probe`, para a recusa chegar ao chamador em vez de ser engolida por `let _ = …`.
- Trate como identidade, não conteúdo, toda string que o agente devolve a um programa, então `--truncate-content` não mutila mais chaves de config, tags de locale, ids de schema, caminhos ou códigos de erro do probe.
- Recuse `--truncate-content` com exit 2 em `config list`, `config path`, `config get/set/unset`, `config effective` e `locale`, onde toda string é identificador.
- Unifique a recusa em `output::emit_envelope_or_refuse` — `{"error", "message"}` no stdout, prosa localizada no stderr, código de saída inalterado.
- Renomeie o campo publicado da matriz para `agent_ops[].discriminator_key`, porque todo envelope carrega a chave `type` e o campo antigo carregava o valor.
- Adicione `tests/integration_stdout_boundary.rs` e `tests/integration_agent_ops_matrix.rs` como réguas que reprovam o build diante de bypass novo ou isenção obsoleta.
- Promova quatro tetos do probe às chaves XDG `probe_launch_timeout_seconds`, `probe_extract_timeout_seconds`, `probe_deep_launch_timeout_seconds` e `probe_deep_extract_timeout_seconds`.
- Traduza as oito frases de recusa para `en` e `pt_br`, enquanto o `message` do stdout permanece em inglês de propósito.
- Declare as flags agent-native uma única vez via `#[command(flatten)] AgentOpsArgs`, e honre `--fields` numa busca que estourou o tempo.
- Remova `docs/generated/flag-desc-{en,pt}.json`, os últimos órfãos do gerador Python apagado na v1.0.4.
- Implemente `--truncate-content` no `deep-research`, onde a função tinha corpo vazio enquanto as três irmãs cortavam.
- Varra Chrome órfão pelo marcador `ddg-chrome-*` em macOS e Windows, no lugar de um bloco `cfg` vazio e de um no-op.
- Prenda `html_root_url` à versão publicada com uma régua, em vez de deixá-lo congelado três releases atrás.
- Pare de ler a variável de teste do Cargo `CARGO_BIN_EXE_timeout` no log de produção.
- Meça português em quatro eixos — comentários, prosa de asserção e de `tracing`, identificadores Rust e híbridos EN mais PT — a partir de uma SSOT em `tests/common/language.rs`.
- Sufixe `--version` com `-dirty` quando a árvore não estiver limpa, para dois binários diferentes pararem de reportar uma identidade só.
- Adicione `cargo docs-nohttp`, que roda rustdoc no conjunto de features PADRÃO e pegou seis links intra-doc que o `cargo docs` não enxergava.
- Prenda `rust-toolchain.toml` ao MSRV declarado com uma régua, para um channel derivando não manter os gates verdes.
- Localize o CORPO do erro sob `--ui-lang pt-BR`, tornando `localized_detail` exaustiva e roteando por ela os nove sítios de emissão do `run.rs`.
- Estenda a régua de flags a `llms-full.txt` e às flags exclusivas de subcomando, fechando uma deriva de 27 flags e um default errado de `--global-timeout`.
- Consuma a entrada da agregação por valor e escape TSV em passada única, removendo 26 clones por laço e cinco cópias por célula.

## O que há de novo na v1.0.4 (2026-08-09)
- Abola "flag aceita e ignorada" — toda operação agent-native agora age ou recusa pelo nome, sem terceiro desfecho.
- Aplique `--fields` e `--truncate-content` em qualquer objeto JSON, e recuse os cinco operadores de linha com exit 2 onde a superfície não declara array de linhas.
- Declare o array de linhas por superfície em vez de inferi-lo, porque `doctor` tem `checks` e `failed_checks` e `config effective` tem `allowed_keys` e `precedence`.
- Publique a matriz de capacidade em `commands`, sob `agent_ops`, para o chamador aprender o contrato em vez de colecionar exit codes.
- Meça o resultado: `commands` 6421 → 47 bytes com `--fields version`, `doctor` 2524 → 38 com `--fields type,status`, `schema` 4726 → 1107 com `--fields schemas.id`.
- Corrija três campos que emitiam chave em português sob o wire inglês padrão — `AggregatedItem.display_url`, `AggregatedNewsItem.source` e `AggregatedNewsItem.relative_date`.
- Dê às cinco formas de `config`, ao `locale` e ao `init-config` um discriminador `type` real, para todo schema publicado rotear ou declarar por que não roteia.
- Apague `scripts/regen_cli_flags_readme.py` e substitua o gerador por `tests/integration_docs_drift.rs`, régua que reprova na divergência em vez de reescrever sob demanda.
- Propague o erro de parse de `--fields` / `--filter` no caminho de stream multi-query, que antes o descartava com `.ok()`.

## O que há de novo na v1.0.3 (2026-08-07)
- Corrija a regressão cross-platform — o crate não compilava em macOS nem em Windows na v1.0.2.
- Separe o `use` sem gate em `src/browser/session/mod.rs`, porque o Rust elimina itens desabilitados por `cfg` antes da resolução de nomes (`E0432`).
- Corrija `E0308` duas vezes em `src/browser/detect.rs`, que fazia `if let Ok(..)` sobre `std::env::var_os` num caminho Windows que nenhum gate havia compilado.
- Aplique gate nos 13 warnings fora do Linux que o `-D warnings` rejeitava.
- Reconstrua `tests/integration_content_fetch.rs` sobre `common::lean_config`; ele havia parado de compilar em toda plataforma (17 erros).
- Adicione `cargo check-windows` / `cargo lint-windows`, `scripts/check-macos.sh` e `scripts/portability-lint.sh` como gates locais exigidos antes de tag e `cargo publish`.
- Registre no [`ADR-0028`](docs/decisions/0028-local-cross-platform-gate-v1-0-3.md) por que proibir CI remoto move a verificação cross-platform para o host em vez de dispensá-la.

## O que há de novo na v1.0.2 (2026-07)
- **Wire JSON em inglês por padrão (ADR-0027)** — chaves serializadas: `results`, `title`, `metadata`, `result_count`, `used_chrome`, `chrome_channel`, `chrome_path_resolved`, `execution_time_ms`, `news`, `searches`, … A desserialização ainda aceita aliases em português. **Guia de migração:** [`docs/MIGRATION.pt-BR.md`](docs/MIGRATION.pt-BR.md) · [EN](docs/MIGRATION.md).
- **`--wire-keys en|pt`** + XDG `wire_keys` — opt-in de chaves PT legadas no emit (`--wire-keys pt` ou `config set wire_keys pt`).
- **Agent ops (sem jq obrigatório)** — `--fields`/`--select`, `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only`, `--truncate-content`, `--max-output-bytes` (e defaults XDG).
- **RuntimeConfig SSOT** — precedência **CLI > XDG > FACTORY**; `config` é só CRUD; `config effective` mostra o merge.
- **`budget_profile`** — `lab` | `desktop_contended` | `thin` via `config set budget_profile …`.
- **`chrome_session_retries`** — retries de launch SERP via CLI + XDG.
- **no-warmup fail-closed** — `--no-warmup` exige `--allow-no-warmup` (hidden) ou XDG `allow_no_warmup=true` (só lab).
- **cgroup Linux opt-in** — XDG `linux_cgroup_enabled` + `linux_cgroup_memory_max_mb` (doctor reporta `linux_cgroup`; n/a fora do Linux).
- **Chrome sempre mudo (ADR-0026)** — padrão operacional: todo caminho de launch passa `--mute-audio` efetivo + política de autoplay (MUTE-002 corrigiu argv com quatro traços). Sem unmute.
- **Budget dual/contention no deep-research (ADR-0025)** — estimativa de wall-clock modela dual multiproc vs dual sequencial e contenção do Chrome no host; fail-fast exit 2 em `budget_underflow` (escape: `--allow-under-budget`).
- **`--print-budget`** — estimativa JSON a seco sem Chrome (QUERY opcional); emite `suggested_global_timeout`, `shell_timeout_hint`, `runtime_dual_multiproc`, `chrome_n`.
- **`--auto-contention-budget`** (padrão ON) eleva o global timeout efetivo; `--no-auto-contention-budget` para fail-fast estrito.
- **`doctor` dual** — reporta `ready_for_dual_deep_research`, `recommended_global_timeout` e remediações que preservam dual.
- **Defaults agent-happy sob `timeout 180`** — `--max-sub-queries` padrão **3**, `--fetch-content-cap` padrão **4**; grace timeout deep **20s**.
- **Campos parciais de agente** — envelopes de timeout/deep carregam contadores `partial` / `sub_queries_*` (contrato local, sem phone-home).
- Residual honesto: schema deep-research de metadata pode atrasar campos live do envelope (`partial` / `sub_queries_*`); trate schemas como best-effort.

## O que há de novo na v1.0.1 (2026-07-19)
- **Pass 48 contrato DR + Pass 52 oneshot/stream/config** — deep-research honra `-o`, JSON de timeout agent-stable, `--depth` heurístico, `-f tsv`, subcomandos XDG `config`/`man`; sem knobs de env de produto; **sem telemetria remota**.
- **One-shot pipe-safe (Pass 52 / GAP-E2E-51-001/007)** — `ensure_oneshot_cleanup` em todas as saídas inclusive pipe cedo; Unix **SIG_IGN** para SIGPIPE (não SIG_DFL) para Drop/reap do Chrome ainda rodar; stream `BrokenPipe` → exit **141**.
- **`-f ndjson`** aceito como alias do modo stream multi-query (`--stream`).
- **API dual de `config`** — `config get KEY` **ou** `config get --key KEY`; `config set KEY VALUE` **ou** `config set --key KEY --value VALUE`; também `config unset` dual e **`config effective`** (JSON mesclado CLI+XDG+defaults).
- **Wire JSON (ADR-0023, serialize supersedido pela v1.0.2)** — nomes em português na **serialização** na linha 1.x; aliases `serde` em inglês só na **desserialização**. **v1.0.2** inverte o padrão de serialize para EN — ver acima.
- **Vertical news** — classificação anti-bot falsa em news isolada corrigida; residual de anti-bot real do DDG ainda pode gerar exit 6 ambientalmente (`news` vazias honestas, nunca sintéticas).
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
- **L-04 News SERP** — multi-seletor; flag honesta de uso do Chrome em news-only / multi-query / deep / falhas (wire EN v1.0.2: `used_chrome`; legado PT `usou_chrome` só com `--wire-keys pt`).
- **L-05 Texto limpo LIGADO por padrão** — fetch de conteúdo ON para **web + news** (FETCH_CAP=4 (v1.0.2; era 10 na v0.9.8)); opt-out **`--no-fetch-content`**. News pode trazer body quando o fetch está ON (EN v1.0.2: `content` / `content_size` / `content_extraction_method`; legado PT com `--wire-keys pt`).
- **L-06 Flags de transporte `global = true`** — `--chrome-path`, `--proxy`, `--vertical`, fetch, identity etc. funcionam **depois** de `deep-research` (e antes).
- **L-07 UA fan-out** — `coerce_chrome_user_agent` compartilhado; one-shot (0.9.6); Chrome-only (0.9.4); atomwrite; **sem telemetria remota**.
- **L-08 Docs/schemas/skills** — ADR-0018, inventário local `gaps.md`, skills EN/PT, CHANGELOG.
- **Metadados de agente (NÃO telemetria)** — wire EN v1.0.2: `chrome_path_resolved`, `chrome_channel`, `used_chrome` honesto (nomes PT históricos `chrome_path_resolvido` / `chrome_canal` / `usou_chrome` só com `--wire-keys pt`).
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

- Busca para LLMs e agentes com parsing via jaq (wire EN v1.0.2):

```bash
duckduckgo-search-cli "tokio JoinSet exemplos" --num 15 -q | jaq '.results'
```

- Agent ops sem jaq:

```bash
duckduckgo-search-cli "rust async" -q -f json --fields url,title --filter 'title~async' --sort title --count-only
duckduckgo-search-cli "rust async" -q -f json --wire-keys pt   # chaves PT legadas
```

- Busca com conteúdo de páginas embutido no JSON:

```bash
duckduckgo-search-cli --fetch-content -n 5 "melhores frameworks web rust"
```


## Receitas Práticas
- Extrair apenas URLs para um fetcher downstream:

```bash
duckduckgo-search-cli "site:example.com changelog 2025" --num 15 -f json \
  | jaq -r '.results[].url'
```

- Enviar bodies limpos para um summarizer:

```bash
duckduckgo-search-cli "tokio runtime internals" --num 15 \
  --fetch-content --max-content-length 4000 -f json \
  | jaq -r '.results[] | "# \(.title)\n\(.content)\n"' > corpus.md
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

- Roteamento via proxy corporativo (CLI `--proxy` ou XDG `proxy_url` — env não lida):

```bash
duckduckgo-search-cli "vendor status page 2026" --num 15 \
  --proxy http://user:pass@proxy.internal:8080 -f json
```

- Dry-run de budget (deep-research, sem Chrome):

```bash
duckduckgo-search-cli deep-research --print-budget -q
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

- Knobs XDG (RuntimeConfig SSOT — CLI > XDG > FACTORY):

```bash
duckduckgo-search-cli config set wire_keys en
duckduckgo-search-cli config set budget_profile desktop_contended
duckduckgo-search-cli config set chrome_session_retries 2
duckduckgo-search-cli config effective
```


## Comandos

Todos os subcomandos com um exemplo (v1.0.5). O `buscar` oculto é equivalente ao modo de busca padrão.

| Comando | Exemplo |
|---|---|
| **Busca padrão** | `duckduckgo-search-cli -q -f json "rust async" \| jaq '.results[].url'` |
| `buscar` (oculto) | `duckduckgo-search-cli buscar -q -f json "query"` (equivalente à busca padrão; omitido de `--help`) |
| `init-config` | `duckduckgo-search-cli init-config --force` |
| `completions` | `duckduckgo-search-cli completions bash > …/duckduckgo-search-cli` |
| `deep-research` | `duckduckgo-search-cli deep-research "tokio vs async-std" -q -f json --print-budget` |
| `commands` | `duckduckgo-search-cli commands -q` |
| `schema` | `duckduckgo-search-cli schema --name search-output` |
| root `--print-schema` | `duckduckgo-search-cli --print-schema` (mesmo catálogo que `schema` sem `--name`) |
| root `--probe` | `duckduckgo-search-cli --probe -q -f json` (separado de `doctor`) |
| `doctor` | `duckduckgo-search-cli doctor -q` / `doctor --strict` / `doctor --probe-deep` |
| `locale` | `duckduckgo-search-cli locale -q` |
| `man` | `duckduckgo-search-cli man \| man -l -` |
| `config path` | `duckduckgo-search-cli config path` |
| `config list` | `duckduckgo-search-cli config list` |
| `config get` | `duckduckgo-search-cli config get wire_keys` |
| `config set` | `duckduckgo-search-cli config set budget_profile lab` |
| `config unset` | `duckduckgo-search-cli config unset proxy_url` |
| `config effective` | `duckduckgo-search-cli config effective` |
| `help` | `duckduckgo-search-cli help deep-research` |

**Flags agent-native (busca + deep-research):**

```bash
duckduckgo-search-cli "query" -q -f json --fields url,title --filter 'url~github' --sort title --limit 5
duckduckgo-search-cli "query" -q -f json --count-only
duckduckgo-search-cli "query" -q -f json --wire-keys pt   # serialize PT legado
```

### Matriz de capacidade agent-native por superfície

A tabela acima lista os subcomandos; ela não diz qual operador agent-native cada um aceita. Desde a v1.0.4 essa matriz é dado publicado, e desde a v1.0.5 toda superfície age ou recusa pelo nome.

- Leia a matriz viva com `duckduckgo-search-cli commands`, sob `agent_ops`, em vez de confiar neste resumo estático.
- Aplique `--fields` / `--select` e `--max-output-bytes` em toda superfície, porque eles têm sentido em qualquer objeto JSON.
- Aplique os cinco operadores de linha — `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only` — só onde a superfície declara array de linhas: `doctor` (`checks`), `schema` (`schemas`), `locale` (`available`), `config list` e `config effective` (`allowed_keys`), `init-config` (`files`).
- Espere que as superfícies sem linhas — `commands`, `config path`, `config get`, `config set`, `config unset`, root `--probe` e root `--probe-deep` — RECUSEM esses cinco operadores com exit 2.
- Espere que `locale`, `config list` e `config effective` RECUSEM `--truncate-content` com exit 2, porque nessas superfícies toda string é identificador que o chamador devolve a um programa.
- Espere que `config path`, `config get`, `config set` e `config unset` suportem apenas `--fields` e `--max-output-bytes`.
- Espere que `doctor`, `schema`, `init-config`, `commands`, root `--probe` e root `--probe-deep` suportem `--truncate-content`, porque esses envelopes carregam prosa real.
- Leia toda recusa da mesma forma: `{"error", "message"}` no stdout na forma publicada `error-response`, prosa localizada no stderr, exit 2.
- Leia `agent_ops[].discriminator_key` como a CHAVE que roteia o envelope (`type`), nunca como o valor que ela carrega.

```bash
duckduckgo-search-cli commands -q -f json | jaq '.agent_ops'
duckduckgo-search-cli doctor -q -f json --fields type,status
duckduckgo-search-cli commands -q -f json --count-only   # exit 2: superfície sem linhas
```

## Deep Research (v0.7.0)

Para perguntas de pesquisa multi-hop — "compare os quatro principais clientes HTTP Rust em 2026", "o que mudou no Tokio 1.40", "resuma a história do endpoint HTML do DuckDuckGo" — o `duckduckgo-search-cli` traz um pipeline de fan-out que decompõe a pergunta em 1..=12 sub-queries, dispara em paralelo, agrega e opcionalmente sintetiza um relatório com referências numeradas.

Desde a v0.8.9 (GAP-WS-105) o `deep-research` também varre a vertical de notícias do DuckDuckGo por PADRÃO: cada sub-query roda como `--vertical all`, com a MESMA sessão Chrome navegando a SERP web e depois a SERP de notícias. O envelope sempre traz a lista agregada `news[]` (vazia quando zero; wire EN v1.0.2). Legado PT: `noticias[]` só com `--wire-keys pt`. Use `--no-news` para opt-out. **v0.9.4 GAP-WS-113:** sem Chrome a CLI **falha com exit 2** — sem auto-degradação `--no-news`.

```bash
# Decomposição heurística padrão (3 sub-queries na v1.0.2, agregação RRF, sem síntese).
duckduckgo-search-cli deep-research "melhor cliente http rust 2026" -f json -q \
  | jaq '.results[] | {title, url, score}'

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

Inventário **completo e SSOT** (gerado de `deep-research --help`, v1.0.5): ver [## Flags Disponíveis](#flags-disponíveis) → *Só `deep-research`*. Resumo operacional:

- `--max-sub-queries N` — máximo de sub-queries (`1..=12`, padrão **3** na v1.0.2)
- `--sub-query-strategy` — decomposição (ex.: `heuristic` / `manual`)
- `--sub-queries-file PATH` — lista explícita (com strategy `manual`)
- `--aggregate` — agregação (RRF / dedupe)
- `--depth` — rounds de reflexão (`0..=3`; v1.0.1+)
- `--fetch-content` / `--no-fetch-content` — body **LIGADO** por padrão (FETCH_CAP=**4** na v1.0.2)
- `--synthesize` / `--synth-format` / `--budget-tokens` — relatório final
- `--print-budget` — dry-run de budget sem Chrome
- `--allow-under-budget` / `--auto-contention-budget` / `--no-auto-contention-budget`
- `--require-all-sub-queries` / `--require-results`
- `--no-news` — opt-out da vertical news (padrão: news ligado via Chrome; sem Chrome → exit **2** fail-closed, GAP-WS-113)

### Schema JSON de saída (wire EN v1.0.2)

```jsonc
{
  "query": "melhor cliente http rust 2026",
  "kind": "deep_research",
  "metadata": {
    "original_query": "melhor cliente http rust 2026",
    "sub_queries": [
      { "text": "...", "strategy": "heuristic", "status": "ok", "elapsed_ms": 420 }
    ],
    "unique_result_count": 27,
    "unique_news_count": 9,
    "total_time_ms": 1850,
    "cascade_level": 0
  },
  "results": [
    { "title": "...", "url": "...", "score": 0.041, "sources": ["..."] }
  ],
  "news": [
    { "position": 1, "title": "...", "url": "...", "source": "...", "relative_date": "há 2 horas", "score": 0.032, "occurrences": 2 }
  ],
  "news_count": 9,
  "synthesis": {
    "format": "markdown",
    "body": "# Relatório\n\n...\n\n[1] Título — url",
    "estimated_tokens": 1200,
    "reference_count": 5
  }
}
```

> Prefira `jaq '.results'` / `.metadata` na **v1.0.2+**. Chaves PT legadas: `--wire-keys pt`. Tabela completa: [`docs/MIGRATION.pt-BR.md`](docs/MIGRATION.pt-BR.md).


## Flags Disponíveis

> **SSOT:** gerado a partir de `duckduckgo-search-cli --help` e `--help` dos subcomandos no binário **v1.0.5** (70 flags da raiz + exclusivas de deep/doctor/init/schema/man). Prefira `commands` / `schema` para descoberta de agente com baixo custo de tokens. O inglês mora **somente** em [`README.md`](README.md) — este arquivo é o SSOT em português.

### Raiz / busca padrão (inventário completo de `--help`)

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `-n`, `--num` | `15` | Máximo de resultados por query (padrão 15; páginas via `--pages`, padrão 1). |
| `-l`, `--lang` | `pt` | Código de idioma `kl` do DuckDuckGo (SERP). Padrão `pt`. |
| `-c`, `--country` | `br` | Código de país `kl` do DuckDuckGo. `--region` é alias legado. Padrão `br`. |
| `--queries-file` | (nenhum) | Arquivo com queries adicionais (uma por linha). Linhas vazias ignoradas. |
| `--endpoint` | `html` | `html` (produção) ou `lite` (valor legado; não é caminho de sucesso sob GAP-WS-113). |
| `--vertical` | **`all`** (v0.9.8) | `web`, `news` ou `all` (**padrão `all`** desde v0.9.8). News só via Chrome. |
| `--time-filter` | (nenhum) | Filtro temporal: `d` / `w` / `m` / `y`. Padrão: nenhum. |
| `--safe-search` | `moderate` | Safe-search: `off`, `moderate` (padrão) ou `on`. |
| `--identity-profile` | `auto` | Fixa perfil do pool de 12 identidades (`chrome-win`, `safari-mac`, …). Padrão `auto` rotaciona no bloqueio. |
| `--cookies-path` | (nenhum) | Sobrescreve path do cookie jar (padrão: dir XDG + `cookies.json`). |
| `--seed` | (nenhum) | Seed determinístico para UA + identidade (reprodutibilidade). |
| `--config` | (nenhum) | Path do diretório de configuração (sobrescreve o path padrão do SO). |
| `-f`, `--format` | `auto` | Formato: `json`, `text`, `markdown`/`md`, `tsv`, `ndjson` ou `auto` (TTY-aware). |
| `-o`, `--output` | (nenhum) | Grava no arquivo em vez de stdout (cria pais; Unix 0o644). |
| `--stream` | off | Multi-query: emite NDJSON conforme cada busca completa. Alias: `-f ndjson`. Fechamento cedo → exit **141**. |
| `--fields` | (nenhum) | Projeta cada linha para os campos wire listados (vírgula; tokens EN ou PT). |
| `--select` | (nenhum) | Alias de `--fields` (nome agent-native / ETL). |
| `--filter` | (nenhum) | Filtra linhas após extract do SERP (agent-native; sem jq). |
| `--limit` | (nenhum) | Limita linhas **após** extract + `--filter` (agent-native; sem jq). |
| `--sort` | (nenhum) | Ordena linhas após filter (agent-native; sem jq). |
| `--dedupe-by` | (nenhum) | Deduplica por URL canônica após sort (agent-native; sem jq). |
| `--count-only` | off | Emite só contagens compactas EN — sem linhas de resultado. |
| `--truncate-content` | (nenhum) | Trunca cada `content` a N escalares Unicode (anti-token). |
| `--max-output-bytes` | (nenhum) | Fail-closed se o payload formatado de stdout exceder N bytes. |
| `--pretty` | off | JSON indentado (`to_string_pretty`). Padrão é JSON **compacto** para orçamento de tokens. |
| `--no-color` | off | Desativa cor (respeita `NO_COLOR` / no-color.org). |
| `--print-schema` | off | Imprime catálogo JSON Schema em stdout (igual a `schema` sem `--name`). |
| `--ui-lang` | (nenhum) | Idioma da UI em stderr humano (`en`|`pt-BR`). **Não** é o SERP `-l`/`--lang`. |
| `--config-home` | (nenhum) | Sobrescreve diretório de config XDG/plataforma (selectors, cookies, ui-lang). |
| `--wire-keys` | **`en`** (v1.0.2) | Serializa chaves JSON em inglês (`en`, **padrão v1.0.2**) ou português legado (`pt`). Também `config set wire_keys`. |
| `-t`, `--timeout` | `15` | Timeout por query em segundos (padrão 15). |
| `-p`, `--parallel` | `5` | Requests concorrentes (`1..=20`, padrão 5). Alias: `--max-concurrency`. |
| `--shared-session-verticals` | off | Força uma sessão Chrome compartilhada para web+news (`--vertical all`) em vez de dual multi-process. |
| `--pages` | `1` | Páginas por query (`1..=5`, padrão **1**; pode auto-elevar com `--num`). |
| `--retries` | `2` | Retries extras em falhas HTTP/rede transitórias (`0..=10`, padrão 2). |
| `--disable-retry` | off | Força zero retries (kill switch). Equivalente a `--retries 0`. |
| `--base-url-html` | (nenhum) | Sobrescreve URL base do SERP HTML (wiremock/testes). |
| `--base-url-lite` | (nenhum) | Sobrescreve URL base Lite. |
| `--base-url-serp` | (nenhum) | Sobrescreve URL base SERP / warm-up. |
| `--proxy` | (nenhum) | Proxy HTTP/HTTPS/SOCKS5 via CLI (produto **não** herda `HTTP(S)_PROXY`). |
| `--no-proxy` | off | Desativa todas as fontes de proxy (no-proxy explícito). |
| `--allow-lite-fallback` | off | **NO-OP legado (GAP-WS-113)** — não força Lite; SERP permanece HTML Chrome. Mantido para scripts não saírem com exit 2. |
| `--global-timeout` | `180` | Timeout global do pipeline (`1..=3600` s, padrão 180). Diferente de `--timeout` por request. Global nos subcomandos. |
| `--cancel-grace-secs` | `5` | Graça cooperativa de cancelamento antes de hard exit (`1..=60` s, padrão 5). |
| `--probe` | off | Probe de saúde do Chrome via CDP: reachability mínima + latência em JSON. |
| `-v`, `--verbose` | off | `-v` = DEBUG, `-vv`+ = TRACE em stderr. Log de produto = CLI `-v`/`-q` + XDG `log_directive` (não `RUST_LOG`). |
| `-q`, `--quiet` | off | Silencia **todo** tracing em stderr (incluindo ERROR). |
| `-V`, `--version` | — | Imprime `NOME VERSÃO (git:SHA)`. O SHA carrega `-dirty` quando a árvore não está limpa (v1.0.5). |
| `-h`, `--help` | — | Imprime a ajuda do comando raiz ou de qualquer subcomando. |
| `--no-input` | off | Contrato de agente: nunca prompt / nunca lê TTY interativo. |
| `--probe-deep` | off | Health check profundo via Chrome/CDP com detecção de interstitial (CAPTCHA); relatório JSON. |
| `--require-results` | off | Falha com exit 5 quando a busca zera (gate de agente). Em deep-research pode usar exit 70 no modo require. |
| `--pre-flight` | off | Calibração pre-flight de ghost-block/interstitial no SERP Chrome. **Não** libera pure-HTTP nem Lite (GAP-WS-113). Só vertical web. |
| `--no-zero-cause-strict` | off | Desliga mapeamento estrito de zero-cause (exit 5 legado para todos os zeros). Padrão strict ON → exit 6 para zeros não legítimos. |
| `--fetch-content` | **ligado** (v0.9.8) | Afirma extração de conteúdo (**padrão LIGADO** desde v0.9.8 para web+news). Prefira omitir ou `--no-fetch-content`. |
| `--no-fetch-content` | off | Desliga extração de body (opt-out do padrão agent-ready v0.9.8). |
| `--fetch-content-cap` | **`4`** (v1.0.2) | Máximo de URLs a enriquecer por vertical (`1..=50`, padrão **4** na v1.0.2; era 10 na v0.9.8). |
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

### Só `deep-research` (além das flags globais da raiz)

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `--max-sub-queries` | **`3`** (v1.0.2) | Máximo de sub-queries na decomposição (`1..=12`, padrão **3** na v1.0.2). |
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

### Só `doctor`

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `--strict` | off | Doctor: falha fechada em checks não-OK. |

### Só `init-config`

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `--force` | off | init-config: sobrescreve arquivos de config existentes. |
| `--dry-run` | off | init-config: simula sem gravar arquivos. |

### Só `schema`

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `--name` | (nenhum) | schema: emite corpo de schema nomeado em vez do catálogo. |

### Só `man`

| Flag | Padrão | Descrição |
| ---- | ------ | --------- |
| `--file` | (nenhum) | man: grava roff neste path (atômico). Usa `--file`, não `-o`. |

## Vertical de Notícias (v0.8.9; defaults supersedidos pela v0.9.8; wire EN desde v1.0.2)

- **v0.9.8:** o padrão de `--vertical` é **`all`** (web + news). Opt-out com `--vertical web`. (Histórico v0.8.9: padrão era `web` — **supersedido pela v0.9.8**.)
- `--vertical news` retorna apenas notícias (`results: []`); `--vertical all` retorna web e notícias na mesma sessão Chrome
- Vertical news roteia EXCLUSIVAMENTE pelo Chrome (SERP exige JavaScript) — sem fallback HTTP
- Batch multi-query aceito desde o GAP-WS-105 (`--queries-file` e múltiplas queries posicionais) — cada query roda sua própria sessão Chrome; no `deep-research` a vertical news é o PADRÃO (opt-out `--no-news`)
- **Wire EN v1.0.2 (padrão)** com `--vertical news|all`: `news[].{position,title,url,source,relative_date,thumbnail}`, `news_count` e `metadata.vertical_used`; com fetch ON, news pode ter `content` / `content_size` / `content_extraction_method`. Chaves PT legadas (`noticias`, `quantidade_noticias`, `metadados.vertical_usada`, …) só com `--wire-keys pt`
- Zero notícias legítimo classifica `zero_cause: vertical-no-results` (exit 5, não 6); string de causa legada PT com `--wire-keys pt`
- **v0.9.8 fetch de conteúdo:** LIGADO por padrão para **web + news** (FETCH_CAP=4 (v1.0.2; era 10 na v0.9.8)); opt-out `--no-fetch-content`. (Claim histórico “fetch SOMENTE em `results[]` web” é **supersedido pela v0.9.8**.)
- Metadados de agente: `chrome_path_resolved`, `chrome_channel`, `used_chrome` honesto (**não** telemetria; nomes PT legados com `--wire-keys pt`)
- Flags de transporte são `global = true` — `--chrome-path` etc. funcionam depois de `deep-research`

```bash
timeout 90 duckduckgo-search-cli --vertical news "noticias brasil" -q -f json | jaq '.news'
timeout 90 duckduckgo-search-cli --vertical all "rust release" -q -f json | jaq '{web: .result_count, news: .news_count}'
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
- `json` (padrão em pipes): schema canônico com `results[]` e `metadata` (wire EN v1.0.2); ordem de campos estável; legado PT via `--wire-keys pt`
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
| 130 | Cancelado por SIGINT (Ctrl+C). Cancelamento cooperativo, não é falha |
| 141 | Broken pipe (consumer de stdout fechou cedo; v1.0.1 stream-safe) |
| 143 | Cancelado por SIGTERM — o que o `timeout` envia primeiro. Não é falha |


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
- Atualize para **1.0.5** com `cargo install duckduckgo-search-cli --locked --force` para reap pipe-safe (**SIG_IGN** em SIGPIPE + `ensure_oneshot_cleanup` em todas as saídas, inclusive `| head` cedo / BrokenPipe → exit **141**) mais wire EN e agent ops
- One-shot de **processo** em **0.9.6** (ADR-0017); one-shot de **disco** + perfis `ddg-chrome-*` em **1.0.0** (ADR-0020); **1.0.1** fecha o buraco de órfão em pipe cedo (Pass 52)
- Novas invocações reaping da árvore e removem o perfil; a próxima run varre só `ddg-chrome-*` stale (nunca bulk-delete de `.tmp*` estrangeiro nem `org.chromium.Chromium.*`)
- Órfãos de processo (pré-0.9.6) ou dirs `.tmp*` genéricos (pré-1.0.0) **não** são mass-auto-mortos: identifique Chrome de automação pelo `user-data-dir` na cmdline e encerre PIDs / remova dirs uma vez se necessário
- Prefira supervisores com **SIGTERM** primeiro (GNU `/usr/bin/timeout`); **SIGKILL/OOM** nu é limite residual do SO
- **Wire breaking:** se scripts ainda parseiam `resultados`/`metadados`, atualize para chaves EN ou use `--wire-keys pt` — ver [`docs/MIGRATION.pt-BR.md`](docs/MIGRATION.pt-BR.md)

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
cp -r duckduckgo-search-cli/skills/duckduckgo-search-cli-pt ~/.claude/skills/
cp -r duckduckgo-search-cli/skills/duckduckgo-search-cli-en ~/.claude/skills/
```

- Reinicie o Claude Code ou recarregue o Agent SDK para ativar
- Auto-ativação: o Claude dispara a skill quando o usuário menciona pesquisa ou verificação


## Documentação

| Guia | Por que importa |
|---|---|
| [`docs/AGENT_RULES.md`](docs/AGENT_RULES.md) | 30+ regras DEVE/JAMAIS para qualquer LLM invocar a CLI em produção |
| [`docs/COOKBOOK.md`](docs/COOKBOOK.md) | 15 receitas copy-paste para pesquisa, ETL, monitoramento e extração de conteúdo |
| [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) | Snippets para 16 agentes: Claude Code, Codex, Gemini CLI, Cursor, Windsurf, Aider e mais |


## Notas de Migração anteriores à v1.0.0 — consolidadas

O histórico de releases não é colado aqui. Toda nota da v0.3.x até a v0.9.x mora no [`CHANGELOG.pt-BR.md`](CHANGELOG.pt-BR.md); abaixo fica apenas o que continua acionável em uma instalação 1.0.x.

- Leia o [`CHANGELOG.pt-BR.md`](CHANGELOG.pt-BR.md) para o histórico completo v0.3.x → v0.9.x, incluindo a migração de TLS/ambiente de build da v0.7.x e as notas de toolchain Windows.
- Leia [`docs/INSTALL-WINDOWS.pt-BR.md`](docs/INSTALL-WINDOWS.pt-BR.md) quando um build a partir do código-fonte no Windows falhar por componente de toolchain ausente.
- Leia [`docs/MIGRATION.pt-BR.md`](docs/MIGRATION.pt-BR.md) para a única quebra de wire na linha 1.x — as chaves portuguesas viraram inglesas na serialização da v1.0.2; use `--wire-keys pt` para manter o emit legado.
- Espere `--num` com padrão 15 e auto-elevação de `--pages` até 5 quando uma única página do DuckDuckGo não satisfaz a contagem pedida (v0.4.0).
- Espere `--identity-profile auto` rotacionando um pool de 12 identidades por uma cascata de 5 níveis quando um bloqueio é detectado (v0.6.4).
- Espere um circuit breaker por host abrindo após 3 falhas consecutivas e esfriando por 30 segundos sob `--fetch-content --parallel` (v0.6.5).
- Espere `deep-research` saindo com 0 quando web OU news produziram resultados, e com 5 somente quando AMBOS estão vazios (v0.8.9).
- Espere `--synthesize` destinando cerca de 30% do `--budget-tokens` às notícias recentes e cerca de 70% à web, com o formato inalterado sob `--no-news` ou zero notícias (v0.8.9).
- Espere `--allow-lite-fallback` como no-op legado desde a v0.9.4 — a flag é mantida só para scripts antigos não saírem com exit 2 em flag desconhecida.
- Espere o cookie jar gravado com Unix `0o600` sob o diretório de config XDG; use `--no-cookie-persistence` para desligar.

Veja o [CHANGELOG](CHANGELOG.pt-BR.md) para o histórico completo de versões.


## Schema JSON (v0.8.9; nomes de campo no wire EN v1.0.2)

### Guia de migração para consumidores

> **v1.0.2 / ADR-0027:** o stdout serializa chaves em **inglês** por padrão (`news`, `news_count`, `metadata`, …). Os caminhos históricos em português abaixo descrevem o envelope **como emitido na linha 1.x / com `--wire-keys pt`**. Prefira paths EN no wire atual.

Ao parsear o envelope JSON, consumidores DEVEM tratar estas mudanças
de schema:

| Versão | Caminho do campo (histórico PT / wire EN v1.0.2) | Tipo | Padrão | BC |
|---|---|---|---|---|
| v0.7.10 | `metadados.pre_flight_disparado` → `metadata.pre_flight_fired` | bool | `false` | Aditivo |
| v0.8.9 | `noticias[]` → `news[]` | array | (ausente) | Aditivo — somente com `--vertical news\|all` |
| v0.8.9 | `quantidade_noticias` → `news_count` | u32 | (ausente) | Aditivo — somente com `--vertical news\|all` |
| v0.8.9 | `metadados.vertical_usada` → `metadata.vertical_used` | string | (ausente) | Aditivo — somente com `--vertical news\|all` |
| v0.8.9 | `noticias[]` → `news[]` (deep-research) | array | `[]` | Aditivo — SEMPRE presente no envelope do deep-research (GAP-WS-105) |
| v0.8.9 | `quantidade_noticias` → `news_count` (deep-research) | number | `0` | Aditivo — SEMPRE presente no envelope do deep-research (GAP-WS-105) |
| v0.8.9 | `metadados.total_noticias_unicas` → `metadata.unique_news_count` | number | `0` | Aditivo (GAP-WS-105) |
| v0.8.9 | `metadados.sub_queries[].quantidade_noticias` / `.news_indisponivel` → `metadata.sub_queries[].news_count` / `.news_unavailable` | number / bool | (ausente) | Aditivo — opcional (GAP-WS-105) |

Nenhum campo removido. Nenhum campo deprecado. A flag
`--require-results` do `deep-research` é local a esse subcomando e
emite exit code `70` (EX_SOFTWARE) quando os resultados agregados são
zero, em vez de `0` (zero-resultado silencioso).

Quando o endpoint probe-deep detecta um CAPTCHA, o envelope JSON
agora inclui o marcador específico que casou (wire EN v1.0.2):

```json
{
  "type": "probe_deep",
  "cascade_reason": "cloudflare",
  "mitigation_suggestion": "Cloudflare challenge detected (marker: cf-turnstile). Re-run with --pre-flight..."
}
```

Consumidores devem tratar sentinelas que começam com `<` (ex.
`<ghost-block-no-marker>`, `<empty-body>`, `<no-marker>`) como
marcadores não-literais e omiti-las de listas voltadas ao usuário.


## Contribuindo
- Abra uma issue antes de criar um Pull Request para discutir a mudança proposta
- Leia os guias em `docs/` para entender a arquitetura antes de contribuir


## Licença
- Licenciado sob MIT OR Apache-2.0
- Escolha a licença que melhor atende às suas necessidades


## Troubleshooting adicional (v0.7.3+)

1. **CAPTCHA interstitial detectado (v0.7.3+)** — rode `duckduckgo-search-cli --probe-deep -q -f json` para classificar o body da resposta. Se `status` for `captcha`, a resposta está bloqueada. O probe também reporta `mitigation_suggestion` com próximos passos concretos (rotacionar proxy, trocar endpoint, back off). Trate o cookie jar como credencial: o arquivo `cookies.json` é gravado com permissões 0o600 e contém cookies de sessão do DuckDuckGo.
2. **Cookie jar crescendo sem controle** — cada invocação adiciona um cookie novo. O arquivo é reescrito inteiro a cada invocação, então o tamanho se mantém proporcional ao número de cookies únicos. Para resetar, apague o arquivo manualmente.
