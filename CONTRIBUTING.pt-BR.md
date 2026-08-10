# Como Contribuir para o duckduckgo-search-cli
Leia em [English](CONTRIBUTING.md).


## Boas-vindas
- Obrigado pelo seu interesse em contribuir com o duckduckgo-search-cli
- Cada contribuição melhora uma ferramenta usada por desenvolvedores e agentes de IA no mundo inteiro
- Este guia cobre o mínimo necessário para publicar uma mudança com sucesso
- Linha atual documentada aqui: **v1.0.5** (régua de fronteira do stdout, matriz de agent ops, identidade nunca truncada, envelope único de recusa)
- Quebra de wire introduzida na v1.0.2: veja [docs/MIGRATION.pt-BR.md](docs/MIGRATION.pt-BR.md)


## Início Rápido
Clone o repositório e rode os gates rápidos em cinco comandos:

```bash
git clone https://github.com/danilo-aguiar-br/duckduckgo-search-cli
cd duckduckgo-search-cli
cargo check-all    # gate 1 — compila
cargo lint         # gate 2 — clippy -D warnings
cargo fmt --check  # gate 3 — format
cargo test-all     # gate 5 — unit + integration + doctest
```

Aliases em [`.cargo/config.toml`](.cargo/config.toml) (`check-all`, `lint`,
`docs`, `test-all`, `check-windows`, `check-windows-msvc`, `lint-windows`,
`check-nohttp`, `lint-nohttp`, `cov`, `cov-html`, `publish-check`, `pkg-list`).
Prefira-os em vez de expandir cada flag à mão. Pipeline local completo:

```bash
cargo check-all && cargo lint && cargo fmt --check && \
  RUSTDOCFLAGS="-D warnings" cargo docs && cargo test-all
```


## Superfície da CLI (v1.0.5)
Os subcomandos públicos vivem na árvore clap em `src/cli/mod.rs`. A serialização de wire tem padrão **EN** (ADR-0027).

| Superfície | Notas |
| ---------- | ----- |
| Busca padrão | `duckduckgo-search-cli [OPTIONS] [QUERY]...` (sem subcomando) |
| `buscar` | Alias oculto da busca padrão |
| `init-config` | `--force`, `--dry-run` |
| `completions <SHELL>` | bash / zsh / fish / powershell / elvish |
| `deep-research` | fan-out + agregação; `--print-budget`, flags de budget |
| `commands` | árvore de comandos em JSON (descoberta do agente) |
| `schema` | catálogo ou `--name NAME` |
| `doctor` | `--strict`, `--probe-deep` (o root `--probe` é separado) |
| `locale` | locale de UI resolvido em JSON |
| `man` | página man (roff); `--file PATH` opcional |
| `config path` | imprime o diretório XDG de config em JSON |
| `config list` | lista as chaves de `config.toml` |
| `config get` | lê uma chave (posicional ou `--key`) |
| `config set` | grava uma chave (posicional ou `--key`/`--value`) |
| `config unset` | remove uma chave |
| `config effective` | JSON mesclado CLI > XDG > FACTORY |
| `help` | help do clap |

Agent ops (globais): `--fields`/`--select`, `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only`, `--truncate-content`, `--max-output-bytes`, `--wire-keys en|pt`.

Desde a v1.0.4 toda agent op age ou recusa pelo nome com exit code 2 — não existe terceiro desfecho. Desde a v1.0.5 strings de identidade (chaves de config, tags de locale, ids de schema, paths, códigos de erro do probe) são isentas de `--truncate-content`, e superfícies feitas só de identificadores recusam a flag em vez de devolver um envelope inalterado.

One-liners completos: [`INTEGRATIONS.pt-BR.md`](INTEGRATIONS.pt-BR.md) na raiz, catálogo [`docs/INTEGRATIONS.pt-BR.md`](docs/INTEGRATIONS.pt-BR.md), tabelas em [`README.pt-BR.md`](README.pt-BR.md).


## Código de Conduta
- Este projeto adota o [Contributor Covenant 2.1](CODE_OF_CONDUCT.pt-BR.md)
- Leia integralmente antes de abrir qualquer issue ou pull request
- Reporte violações pelo canal descrito em `CODE_OF_CONDUCT.pt-BR.md`


## Configuração do Ambiente de Desenvolvimento
### Pré-requisitos
- MSRV (versão mínima do Rust suportada): Rust 1.88 — declarado em `Cargo.toml` (`rust-version`) e travado em `rust-toolchain.toml`
- Execute `rustup update stable` para casar com a toolchain
- Instale o llvm-cov: `cargo install cargo-llvm-cov`
- Instale o cargo-audit: `cargo install cargo-audit`
- Instale o cargo-deny: `cargo install cargo-deny`
- Adicione os alvos de cross-check uma vez por host, sem root e sem pacote de sistema: `rustup target add x86_64-pc-windows-gnu x86_64-pc-windows-msvc aarch64-apple-darwin x86_64-apple-darwin`
- Este projeto **não** usa `cargo-nextest` — a suíte roda via `cargo test` / `cargo test-all` padrão


## Pré-requisitos Chrome para Desenvolvimento
- Instale Google Chrome ou Chromium para testes E2E
- Linux: o Xvfb é auto-instalado pela CLI em runtime via `try_auto_install_xvfb()` para 22+ distros
- Para desenvolvimento, instale manualmente: `sudo dnf install xorg-x11-server-Xvfb` (Fedora) ou `sudo apt-get install xvfb` (Debian/Ubuntu)
- macOS/Windows: sem dependência extra — o Chrome roda em **headless=new** desde a v0.9.3 (não headed nativo Quartz/DWM; esse caminho existiu só na v0.9.1 e foi supersedido)
- Executar testes E2E: `cargo test-all` (ou `cargo test --all-features --locked`; a CLI spawna Xvfb automaticamente se necessário)
- Executar testes sem Chrome: `cargo test --no-default-features`
- O headless de produto é a flag CLI **`--chrome-headless`** (não env de produto). A verbosidade é **`-v`/`-vv`/`-q`** ou a chave XDG `log_directive` — o produto **não** usa `RUST_LOG` (GAP-LOG-ENV-001). **Sem env de produto** para knobs de runtime — só flags CLI + XDG.
- A feature `chrome` é habilitada por padrão no `Cargo.toml`
- Os testes stealth do Chrome estão em `tests/integration_stealth_block_classification.rs`
- Os testes Chrome do deep-research estão em `tests/integration_deep_research.rs`
- **Wire JSON (v1.0.2, ADR-0027)** serializa chaves em **inglês** por padrão (`.results`, `.metadata`, …). Testes e fixtures ainda podem desserializar aliases PT. Agentes legados: `--wire-keys pt` ou `config set wire_keys pt`. Veja [docs/MIGRATION.pt-BR.md](docs/MIGRATION.pt-BR.md).
- **Defaults agent-ready (v0.9.8, ainda vigentes na v1.0.5)** afetam a latência E2E: o fetch de conteúdo está **LIGADO** e a vertical padrão é **`all`** (dual web+news). Prefira timeouts maiores, ou use `--vertical web --no-fetch-content` quando um smoke fino e rápido bastar.
- **Envs só de harness de teste** (não são config de produto — nunca documente como knobs de runtime para usuários finais):
  - `DUCKDUCKGO_FLATPAK_E2E=1` — **somente harness de teste, não config de produto**
  - `DUCKDUCKGO_LIFECYCLE_E2E=1` — **somente harness de teste, não config de produto**
  - `DUCKDUCKGO_CHROME_HEADLESS=1` — **somente harness de teste, não config de produto** (o produto usa a CLI `--chrome-headless`)
- **E2E Flatpak multi-canal (v0.9.8+)** — gated por `DUCKDUCKGO_FLATPAK_E2E=1` (**somente harness de teste, não config de produto**):

  ```bash
  DUCKDUCKGO_FLATPAK_E2E=1 cargo test --test integration_flatpak_chrome -- --nocapture
  ```

  Cobre o resolve Flatpak export→ELF (`files/extra/chrome`) quando há um deploy Flatpak do Chrome presente.
- **E2E de lifecycle (contrato v1.0.0, linha atual v1.0.5; GAP-WS-TMP-PROFILE-ORPHAN-001 + processo GAP-WS-LIFECYCLE-001)** — gated por `DUCKDUCKGO_LIFECYCLE_E2E=1` (**somente harness de teste, não config de produto**):

  ```bash
  DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle
  ```

  Exige Chrome; afirma que nenhum processo chrome residual permanece com o `user-data-dir` desta execução após a saída; prefixo de perfil **`ddg-chrome-`**. Testes unitários cobrem `force_reap` / `sweep_orphan_profiles` / guards de ownership (nunca bulk-delete de `.tmp*`) sem a env E2E. Ver **ADR-0020** (one-shot de disco) e **ADR-0017** (one-shot de processo).


## Estratégia de Branches
- Branch principal: `main`
- Branches de feature: `feature/nome-descritivo`, a partir de main
- Branches de fix: `fix/nome-do-bug`, a partir de main
- Abra o PR de volta para main
- Squash and Merge é o método padrão de merge


## Convenção de Commits
- Use prefixos convencionais: `feat:`, `fix:`, `deps:`, `docs:`, `test:`, `refactor:` (nunca `ci:` — CI/CD é proibido)
- Nunca adicione trailers `Co-authored-by:` de agentes de IA como dependabot, renovate, Claude, GPT, Copilot, Cursor ou Gemini
- Use squash and merge para PRs com múltiplos commits
- Escreva o assunto em termos do problema resolvido, não do arquivo tocado


## Padrões de Código
### Convenções obrigatórias
- Comentários de código, mensagens de log e nomes de campos de structs em português brasileiro, conforme `CLAUDE.md`
- Identificadores de API pública podem ser em inglês quando seguem o estilo Rust convencional, como `from` e `into`
- Nunca use `.unwrap()` ou `.expect()` em código de produção
- Propague erros com `?` e a variante tipada definida em `src/error.rs` (enum `CliError` via `thiserror`)
- O projeto usa `thiserror 2` puro — `anyhow` **não** é dependência
### I/O centralizado
- `src/output/` é o ÚNICO lugar autorizado a chamar `println!` ou `print!`
- Todos os outros módulos registram via `tracing`
- `tests/integration_stdout_boundary.rs` varre toda emissão de stdout e quebra o build em bypass não declarado
### TLS e anti-fingerprint (dual-plane — ADR-0021 / ADR-0022)
- **Produção SERP/probe/fetch:** transporte **Chrome nativo** (stack TLS do browser no host; ADR-0016). Objetivo: **não** expor assinatura TLS de biblioteca (`rustls` JA4 bot-class) que o Cloudflare bloqueia (GAP-WS-27). **Não** é "feature de fingerprint".
- **Proibido (ADR-0022):** spoof sintético de hardware fingerprint (canvas/WebGL/Audio/hwConcurrency forçados) — vira assinatura de automação compartilhada.
- **Stealth permitido:** só sinais de automação CDP (`webdriver`, plugins, `window.chrome`, leak DevTools) — veja `src/browser/stealth.rs`.
- **HTTP residual** (harness): `reqwest` + rustls + CryptoProvider **`aws-lc-rs`** (`tls_bootstrap`). Feature `rustls-tls-webpki-roots-no-provider` (sem `ring`).
- Nunca reative `native-tls` / OpenSSL. Nunca habilite as features de fetcher do chromiumoxide.
- Proxy residual: só `--proxy` ou config XDG — sem herança de `HTTP_PROXY`.
### Restrições de design
- Sem cache, sem MCP, sem API paga — restrições inegociáveis do blueprint v2


## Testes
### Três camadas de teste
- Testes unitários inline com `#[cfg(test)] mod testes` para funções puras
- Testes de integração em `tests/` usando `wiremock` — ZERO HTTP real
- Doctests dentro de blocos `///` na API pública — funcionam também como exemplos no docs.rs
### Execução de testes
- Rode a suíte com `cargo test-all` (ou `cargo test --all-features --locked`)
- Rode cobertura com `cargo cov` — mínimo de 80% obrigatório
- Rejeite na revisão local qualquer PR que empurre a cobertura abaixo do limite
### Vertical de notícias (v0.8.9)
- Fixtures em `tests/fixtures/`: `ddg_news_serp.html` (Estratégia A, 7 artigos + 1 armadilha interna filtrada), `ddg_news_serp_ofuscada.html` (fallback da Estratégia B), `ddg_news_serp_vazia.html` (SERP vazia → `zero_cause: vertical-no-results`)
- Testes de integração: `tests/integration_news_vertical.rs`, `tests/integration_deep_research_news.rs` — rode com `cargo test --features chrome --test integration_news_vertical --test integration_deep_research_news`
- Hot-fix sem recompilar: quebra de seletores no lado do DDG é corrigível via `config/selectors.toml` seção `[news]` (Estratégia A); a Estratégia B é a rede de segurança agnóstica a classes
- Veja `docs/TESTING.md` para a matriz completa de testes da vertical de notícias
### Réguas de contrato (v1.0.4 / v1.0.5)
- `tests/integration_schema_conformance.rs` valida envelopes reais contra `docs/schemas/*.json` nos dois idiomas de wire
- `tests/integration_agent_ops_matrix.rs` roda toda agent op contra toda superfície offline e exige menos bytes ou exit 2
- `tests/integration_stdout_boundary.rs` quebra o build em bypass novo de stdout e em isenção obsoleta
- `tests/integration_docs_drift.rs` falha com o conjunto exato de flags que divergiram entre o binário e os dois READMEs


## Matriz de Validação com 10 Gates
### Gates obrigatórios
Todo PR deve passar pelos 10 gates **localmente**. CI/CD e GitHub Actions são **proibidos** neste repo.
Prefira os aliases de [`.cargo/config.toml`](.cargo/config.toml) onde listados:

| # | Gate | Comando local |
|---|------|---------------|
| 1 | Compilação | `cargo check-all` |
| 2 | Clippy | `cargo lint` |
| 3 | Formatação | `cargo fmt --all -- --check` |
| 4 | Docs | `RUSTDOCFLAGS="-D warnings" cargo docs` |
| 5 | Testes | `cargo test-all` |
| 6 | Cobertura >= 80% | `cargo cov` |
| 7 | Auditoria de vuln | `cargo audit --deny warnings` |
| 8 | Supply chain | `cargo deny check advisories licenses bans sources` |
| 9 | Dry-run de publish | `cargo publish-check` |
| 10 | Conteúdo do pacote | `cargo pkg-list` |

### Gates cross-platform
Rode estes antes de cada tag também. Proibir CI remoto não elimina a necessidade de verificar outras plataformas — move essa verificação para o host do mantenedor, e a v1.0.2 chegou ao crates.io sem compilar em macOS nem Windows porque nenhum gate passava `--target`. A justificativa completa está em [NO_CI.md](NO_CI.md).

| # | Gate | Comando local |
|---|------|---------------|
| A | `use` não gated de item gated | `./scripts/portability-lint.sh` |
| B | ABI Windows GNU | `cargo check-windows` |
| C | ABI Windows MSVC | `cargo check-windows-msvc` |
| D | Clippy Windows | `cargo lint-windows` |
| E | macOS ARM | `./scripts/check-macos.sh` |
| F | macOS Intel | `./scripts/check-macos.sh x86_64-apple-darwin` |
| G | Host sem o harness HTTP | `cargo check-nohttp` e `cargo lint-nohttp` |


## Processo de Pull Request
### Antes de abrir o PR
- `cargo fmt --all -- --check` retorna ZERO diferenças
- `cargo lint` retorna ZERO warnings
- `cargo test-all` retorna ZERO falhas
- `RUSTDOCFLAGS="-D warnings" cargo docs` não retorna warnings
- `cargo audit --deny warnings` não reporta vulnerabilidade conhecida
- Os gates cross-platform acima passam quando a mudança toca código de `cfg`, de processo ou de browser
- `CHANGELOG.md` e `CHANGELOG.pt-BR.md` atualizados com a mudança
- O título do PR descreve o problema resolvido em termos do usuário
### Revisão
- Abra o PR contra `main` e mantenha o diff escopado em um problema
- Responda aos comentários de revisão na thread do PR em vez de fazer force-push silencioso
- Faça squash and merge quando todos os gates acima estiverem verdes no host do mantenedor


## Documentação
- Atualize as duas versões de idioma de qualquer documento que você tocar — EN e pt-BR devem permanecer tecnicamente idênticos
- Nunca traduza comandos, flags, exit codes ou nomes de arquivo; traduza títulos e prosa
- Documente uma flag nova em `README.md`, `README.pt-BR.md` e na página `docs/` relevante no mesmo PR
- Acrescente uma ADR em `docs/decisions/` quando a mudança inverte um default ou quebra um contrato publicado
- Acrescente ou atualize um JSON Schema em `docs/schemas/` sempre que um envelope emitido mudar de forma
- Todo arquivo sob `docs/generated/` deve nomear seu consumidor, ou o guard quebra o build


## Supply Chain
- Toda dependência nova deve passar em `cargo deny check`
- Se o candidato trouxer licença fora da allowlist ou advisory transitivo, encontre alternativa ou documente o ignore em `deny.toml`
- Documente cada ignore com linhas `# Why:` e `# How to apply:` no `deny.toml`
- Prefira crates com `trustScore >= 7` no `context7-cli` (veja `CLAUDE.md`)


## Como Reportar Bugs
### Template de bug report
- Abra uma issue com o título `[bug] descrição concisa do problema`
- Inclua a versão da CLI: `duckduckgo-search-cli --version`
- Inclua o sistema operacional e a versão do Rust: `rustc --version`
- Inclua o comando exato que reproduz o problema
- Inclua a saída completa, stderr incluído


## Como Solicitar Features
### Template de feature request
- Abra uma issue com o título `[feature] descrição concisa`
- Descreva o problema que a feature resolveria
- Descreva o comportamento esperado
- Inclua exemplos de uso ou casos reais


## Reportando Problemas de Segurança
- Veja [SECURITY.pt-BR.md](SECURITY.pt-BR.md) para o processo completo
- Nunca abra issue pública para vulnerabilidade
- Use o canal privado de advisory do GitHub para divulgação responsável


## Processo de Release
### Fluxo do mantenedor
- Suba o campo `version` no `Cargo.toml`
- Mova o conteúdo de `[Unreleased]` do `CHANGELOG.md` para um novo header de versão com data
- Espelhe a mesma entrada em `CHANGELOG.pt-BR.md`
- Rode os 10 gates de validação **localmente**, mais os gates cross-platform (sem Actions)
- Crie tag anotada: `git tag -a v1.0.X -m "descrição"`
- Push: `git push origin main && git push origin v1.0.X` (só a tag; **sem** workflow de release)
- Publique no crates.io **manualmente**: `cargo publish --locked`, após o dry-run e autorização explícita
- **Não** há matriz do GitHub Actions, Dependabot, zizmor, pre-commit hooks nem secrets de GitHub Actions — todos proibidos


## Pré-Publicação (somente local)
- CI/CD e GitHub Actions são **proibidos** neste repositório (não existe diretório `.github/workflows`)
- Antes de publicar: rode os 10 gates locais, os gates cross-platform e `cargo publish --dry-run --locked`
- Mantenedores publicam manualmente com `cargo publish --locked` após autorização explícita
- Janela de yank para release quebrada: 72 horas


## Reconhecimento
- Toda contribuição mergeada é creditada em `CHANGELOG.md` e `CHANGELOG.pt-BR.md` sob a versão que a entrega
- Relatores de segurança são creditados no Hall da Fama de [SECURITY.pt-BR.md](SECURITY.pt-BR.md), salvo pedido de anonimato
- Quem fecha um gap documentado é nomeado junto ao id do gap na ADR que registra a decisão
- Peça para ser creditado com outro nome, ou não ser creditado, e esse pedido é honrado


## Dúvidas
- Abra uma issue no GitHub com o título `[question] assunto conciso` para o que este guia não responder
- Leia [docs/HOW_TO_USE.pt-BR.md](docs/HOW_TO_USE.pt-BR.md) e [docs/COOKBOOK.pt-BR.md](docs/COOKBOOK.pt-BR.md) antes de perguntar sobre uso
- Rode `duckduckgo-search-cli commands` e `duckduckgo-search-cli schema` para descobrir a superfície viva em vez de adivinhar
- Nunca use issue para reportar vulnerabilidade — siga [SECURITY.pt-BR.md](SECURITY.pt-BR.md)


## Documentação Relacionada
- [NO_CI.md](NO_CI.md) — **política: CI/CD e GitHub Actions proibidos** (gates só locais)
- [CHANGELOG.md](CHANGELOG.md) e [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md) — histórico bilíngue sincronizado
- [SECURITY.pt-BR.md](SECURITY.pt-BR.md) — política de reporte responsável e versões suportadas
- [CODE_OF_CONDUCT.pt-BR.md](CODE_OF_CONDUCT.pt-BR.md) — Contributor Covenant 2.1
- [INVERSIONS.pt-BR.md](INVERSIONS.pt-BR.md) — inversões arquiteturais e seus critérios de no-go
- [docs/INSTALL-WINDOWS.pt-BR.md](docs/INSTALL-WINDOWS.pt-BR.md) — setup no Windows (NOTA: desde a v0.8.6, `reqwest`+`rustls-tls` substituiu BoringSSL/wreq, então os pré-requisitos nativos de build não são mais necessários)
- [INTEGRATIONS.pt-BR.md](INTEGRATIONS.pt-BR.md) — catálogo de integrações com 16+ agentes de IA
- [docs/INTEGRATIONS.pt-BR.md](docs/INTEGRATIONS.pt-BR.md) — guia completo de integração
- [docs/decisions/](docs/decisions/) — Architecture Decision Records (ADRs)
- [docs/CROSS_PLATFORM.pt-BR.md](docs/CROSS_PLATFORM.pt-BR.md) — comportamento por plataforma


## Workflow com Agent Teams
- Releases da v0.7.8 em diante usaram o fluxo de 8 fases via Agent Teams
- Cada teammate recebe prompt autocontido com Regra Zero, identidade, contexto e ferramentas
- O líder coordena, delega e verifica — não implementa diretamente
- Veja `CLAUDE.md` na raiz do repositório para o protocolo completo
- ADRs em `docs/decisions/` registram as decisões tomadas em cada release
- Desde a v0.7.10, releases usam `atomwrite` mais `TaskCreate` em vez de Agent Teams, por causa do bug conhecido de estado `Team does not exist` registrado no `mem 1244` do graphrag
- Cada patch roda `atomwrite read` (checksum) → `atomwrite write` → `cargo check --offline` → `cargo test --lib --offline`


## Notas da Release v0.7.8
### Oito gaps fechados (reformulação do detector anti-bot)
- GAP-WS-50 — listas expandidas em `src/probe_deep.rs` (8 marcadores Cloudflare + 1 DDG)
- GAP-WS-51 — constante `PROBE_CALIBRATION_QUERY` em `src/lib.rs` para a query canônica do probe
- GAP-WS-52 — predicado de fallback condicional em `src/search.rs` honra o detector real
- GAP-WS-53 — níveis `-vv` e `-vvv` adicionados em `src/cli.rs` com `ArgAction::Count`
- GAP-WS-54 — `scraper` bumpado para 0.27 resolve o RUSTSEC-2025-0057 transitivo
- GAP-WS-55 — bloco wreq reescrito em `Cargo.toml` com pin exato em 6.0.0-rc.29
- GAP-WS-56 — subcomando `Buscar` marcado como `#[command(hide = true)]`
- GAP-WS-57 — `retries` agora honrado em `src/parallel.rs` no laço de error_output
- ADR completa em `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`


## Notas da Release v0.7.9
### Ghost-block + markers 2026 (oito gaps fechados)
- GAP-WS-58 (CRITICAL) — `detectar_interstitial` classifica body sub-4KB sem `result-page-signal` como `InterstitialKind::Cloudflare`
- GAP-WS-59 (HIGH) — 5 marcadores Cloudflare novos + 1 marker DDG novo
- GAP-WS-59 (HIGH) — `--allow-lite-fallback` e `--pre-flight` viraram `global = true`
- v0.7.9 P1 — `detectar_interstitial_com_match` retorna `(&'static str, InterstitialKind)` com marker literal
- v0.7.9 P3 — `SearchMetadata.pre_flight_fired: bool` adicionado ao envelope
- v0.7.9 P4b — `sugestao_mitigacao_com_marker` injeta o marker real (por exemplo `cf-challenge`)
- `Config.pre_flight` adicionado com default `false`


## Notas da Release v0.7.10
### Pino de identidade + bench wiring + gate de pré-publicação (sete gaps fechados)
- GAP-WS-60 (CRITICAL) — `--identity-profile` propaga para `failure_output` e `error_output` via `identity_tag_for_cli_identity` em `src/identity.rs`
- GAP-AUD-001 (auditoria local) — o pino `identidade_usada` agora está presente nos caminhos de falha (era `null`)
- GAP-AUD-002 (auditoria local) — `[[bench]] harness = false` em `Cargo.toml` corrige o `cargo bench`, que rodava o test harness
- B1 (CRITICAL) — `--pre-flight` não emite mais dois objetos JSON concatenados no stdout
- B2 (CRITICAL) — `pre_flight_blocked` agora retorna exit 3 (era 0)
- B3 (MÉDIO) — `--global-timeout` virou global, aceito em subcomandos
- B4 (CRITICAL) — `--probe-deep` standalone retorna exit 3 quando detecta captcha
- v0.7.10 P4 — `--require-results` em `deep-research`, exit 4 quando o fan-out é zero
- v0.7.10 P5 — scheduler do probe-deep integrado em `execute_single_search`
- v0.7.10 P6 — snapshot test `cloudflare_markers_snapshot_v0_7_10` via `insta = "1"`
- v0.7.10 P7 — `src/proxy_detection.rs` novo módulo (Vivo Fiber, Gigaweb, Cloudflare)
- v0.7.10 P16 — `src/ddg_class_watch.rs` watchdog de runtime
- v0.7.10 P19 — checklist de pré-publicação é só local (script removido; gates 1–10 manuais)
- v0.7.10 P19 — `skills/duckduckgo-search-cli-{en,pt}/evals/queries.json` +4 queries (q47-q50)
