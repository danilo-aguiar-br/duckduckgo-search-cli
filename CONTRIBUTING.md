# Contributing to duckduckgo-search-cli

Thanks for your interest in contributing to duckduckgo-search-cli.
Every contribution improves a tool used by developers and AI agents worldwide.
Read this in [Português](CONTRIBUTING.pt-BR.md).


## Quick Start
### Setup in five commands

```bash
git clone https://github.com/danilo-aguiar-br/duckduckgo-search-cli
cd duckduckgo-search-cli
cargo check-all    # gate 1 — compile
cargo lint         # gate 2 — clippy -D warnings
cargo fmt --check  # gate 3 — format
cargo test-all     # gate 5 — unit + integration + doctest
```

Aliases live in [`.cargo/config.toml`](.cargo/config.toml) (`check-all`, `lint`,
`docs`, `test-all`, `cov`, `publish-check`, `pkg-list`). Prefer them over
expanding every flag by hand. Full local pipeline:

```bash
cargo check-all && cargo lint && cargo fmt --check && \
  RUSTDOCFLAGS="-D warnings" cargo docs && cargo test-all
```

Current line documented here: **v1.0.1**.


## Development Setup
### Prerequisites
- MSRV (Minimum Supported Rust Version): Rust 1.88 — declared in `Cargo.toml` (`rust-version`) and pinned in `rust-toolchain.toml`
- Run `rustup update stable` to match the toolchain
- Install llvm-cov: `cargo install cargo-llvm-cov`
- Install cargo-audit: `cargo install cargo-audit`
- Install cargo-deny: `cargo install cargo-deny`
- This project does **not** use `cargo-nextest` — the suite runs via plain `cargo test` / `cargo test-all`


## Chrome Development Prerequisites (v0.8.9+; current product **v1.0.1**)
- Install Google Chrome or Chromium for E2E tests
- Linux: Xvfb is auto-installed by the CLI at runtime via `try_auto_install_xvfb()` for 22+ distros
- For development, install manually: `sudo dnf install xorg-x11-server-Xvfb` (Fedora) or `sudo apt-get install xvfb` (Debian/Ubuntu)
- macOS/Windows: no extra dependency — Chrome runs in **headless=new** since v0.9.3 (not headed native Quartz/DWM; that path was v0.9.1 only and is superseded)
- Run E2E tests: `cargo test-all` (or `cargo test --all-features --locked`; CLI auto-spawns Xvfb if needed)
- Run tests without Chrome: `cargo test --no-default-features`
- Product headless toggle is the CLI flag **`--chrome-headless`** (not a product env). Verbosity is **`-v`/`-vv`/`-q`** or XDG `log_directive` — product does **not** use `RUST_LOG` (GAP-LOG-ENV-001)
- The `chrome` feature is enabled by default in `Cargo.toml`
- Chrome stealth tests are in `tests/integration_chrome_stealth.rs`
- Deep-research Chrome tests are in `tests/integration_deep_research.rs`
- **Agent-ready defaults (v0.9.8, still current in v1.0.1)** affect E2E latency: content fetch is **ON** and default vertical is **`all`** (dual web+news). Prefer longer timeouts or use `--vertical web --no-fetch-content` when a thin/fast smoke is enough.
- **Test-harness-only env vars** (not product config — never document as runtime knobs for end users):
  - `DUCKDUCKGO_FLATPAK_E2E=1` — **test harness only, not product config**
  - `DUCKDUCKGO_LIFECYCLE_E2E=1` — **test harness only, not product config**
  - `DUCKDUCKGO_CHROME_HEADLESS=1` — **test harness only, not product config** (product uses CLI `--chrome-headless`)
- **Flatpak multi-canal E2E (v0.9.8+)** — gated behind `DUCKDUCKGO_FLATPAK_E2E=1` (**test harness only, not product config**):

  ```bash
  DUCKDUCKGO_FLATPAK_E2E=1 cargo test --test integration_flatpak_chrome -- --nocapture
  ```

  Covers Flatpak export→ELF resolve (`files/extra/chrome`) when a Flatpak Chrome deploy is present.
- **Lifecycle E2E (v1.0.0 contract, current line v1.0.1; GAP-WS-TMP-PROFILE-ORPHAN-001 + process GAP-WS-LIFECYCLE-001)** — gated behind `DUCKDUCKGO_LIFECYCLE_E2E=1` (**test harness only, not product config**):

  ```bash
  DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle
  ```

  Requires Chrome; asserts no residual chrome process remains with this run's `user-data-dir` after exit; profile path prefix **`ddg-chrome-`**. Unit tests cover `force_reap` / `sweep_orphan_profiles` / ownership guards (never `.tmp*` bulk delete) without the E2E env var. See **ADR-0020** (disk one-shot) and **ADR-0017** (process one-shot).


## Code of Conduct
### Contrato Social
- Este projeto adota o [Contributor Covenant](CODE_OF_CONDUCT.md)
- Leia integralmente antes de abrir qualquer issue ou pull request
- Reporte violações seguindo o canal descrito em `CODE_OF_CONDUCT.md`


## Branching Strategy
### Fluxo de Branches
- Ramificação principal: `main`
- Branches de feature: `feature/nome-descritivo` a partir de main
- Branches de fix: `fix/nome-do-bug` a partir de main
- Abra PR de volta para main
- Squash and Merge é o método padrão de merge


## Coding Standards
### Convenções Obrigatórias
- Comentários de código, mensagens de log e nomes de campos de structs em português brasileiro conforme `CLAUDE.md`
- Identificadores de API pública podem ser em inglês quando seguem estilo Rust convencional como `from` e `into`
- Nunca use `.unwrap()` ou `.expect()` em código de produção
- Propague erros com `?` e a variante tipada definida em `src/error.rs` (enum `CliError` via `thiserror`)
- O projeto usa `thiserror 2` puro — `anyhow` NÃO está nas dependências
### I/O Centralizado
- O módulo `output.rs` é o ÚNICO lugar permitido para chamar `println!` ou `print!`
- Todos os outros módulos registram via `tracing`
### TLS e anti-fingerprint (dual-plane — ADR-0021 / ADR-0022)
- **Producao SERP/probe/fetch:** transporte **Chrome nativo** (stack TLS do browser no host; ADR-0016). Objetivo: **nao** expor assinatura TLS de biblioteca (`rustls` JA4 bot-class) que o Cloudflare bloqueia (GAP-WS-27). **Nao** e “feature de fingerprint”.
- **Proibido (ADR-0022):** spoof sintético de hardware fingerprint (canvas/WebGL/Audio/hwConcurrency forçados) — vira assinatura de automacao.
- **Stealth permitido:** so sinais de automacao CDP (`webdriver`, plugins, `window.chrome`, leak DevTools) — ver `src/browser/stealth.rs`.
- **HTTP residual** (harness): `reqwest` + rustls + CryptoProvider **`aws-lc-rs`** (`tls_bootstrap`). Feature `rustls-tls-webpki-roots-no-provider` (sem `ring`).
- Nunca reative `native-tls` / OpenSSL. Nunca habilite features fetcher do chromiumoxide.
- Proxy residual: so `--proxy` / config XDG — sem heranca de `HTTP_PROXY`.
### Restrições de Design
- Sem cache, sem MCP, sem API paga — restrições inegociáveis do blueprint v2


## Testing
### Três Camadas de Teste
- Testes unitários inline com `#[cfg(test)] mod testes` para funções puras
- Testes de integração em `tests/` usando `wiremock` — ZERO HTTP real
- Doctests dentro de blocos `///` em APIs públicas — duplos como exemplos no docs.rs
### Execução de Testes
- Execute testes com `cargo test --all-features` (runner padrão)
- Execute cobertura com `cargo llvm-cov` — mínimo 80% obrigatório
- Qualquer PR que reduza a cobertura abaixo do limite deve ser rejeitado na revisão local


### News Vertical (v0.8.9)
- Fixtures em `tests/fixtures/`: `ddg_news_serp.html` (Estratégia A, 7 artigos + 1 armadilha interna filtrada), `ddg_news_serp_ofuscada.html` (fallback Estratégia B), `ddg_news_serp_vazia.html` (SERP vazia → `causa_zero: vertical-sem-resultados`)
- Testes de integração: `tests/integration_news_vertical.rs`, `tests/integration_deep_research_news.rs` — rode com `cargo test --features chrome --test integration_news_vertical --test integration_deep_research_news`
- Hot-fix sem recompilar: quebra de seletores no lado do DDG é corrigível via `config/selectors.toml` seção `[news]` (Estratégia A); a Estratégia B é a rede de segurança agnóstica a classes
- Veja `docs/TESTING.md` para a matriz completa de testes da vertical news


## 10-Gate Validation Matrix
### Required gates
Every PR should pass all 10 gates **locally**. CI/CD and GitHub Actions are **forbidden** in this repo.
Prefer the aliases from [`.cargo/config.toml`](.cargo/config.toml) where listed:

| # | Gate | Local command |
|---|------|---------------|
| 1 | Compilation | `cargo check-all` |
| 2 | Clippy | `cargo lint` |
| 3 | Format | `cargo fmt --all -- --check` |
| 4 | Docs | `RUSTDOCFLAGS="-D warnings" cargo docs` |
| 5 | Tests | `cargo test-all` |
| 6 | Coverage >= 80% | `cargo cov` (or `cargo llvm-cov --workspace --all-features`) |
| 7 | Vuln audit | `cargo audit --deny warnings` |
| 8 | Supply chain | `cargo deny check advisories licenses bans sources` |
| 9 | Publish dry-run | `cargo publish-check` |
| 10 | Package content | `cargo pkg-list` |


## Pull Request Checklist
### Itens Verificáveis Antes de Abrir PR
- `cargo fmt --all -- --check` retorna ZERO diferenças
- `cargo clippy --all-targets --all-features -- -D warnings` retorna ZERO warnings
- `cargo test --all-features` retorna ZERO falhando
- `cargo doc --no-deps` sem warnings
- `cargo audit --deny warnings` sem vulnerabilidades conhecidas
- CHANGELOG.md e CHANGELOG.pt-BR.md atualizados com a mudança
- Título do PR descreve o problema resolvido em termos do usuário


## Commit Convention
### Prefixos Convencionais
- Use prefixos: `feat:`, `fix:`, `deps:`, `docs:`, `test:`, `refactor:` (não use `ci:` — CI/CD proibido)
- Nunca adicione trailers `Co-authored-by:` de agentes de IA como dependabot, renovate, Claude, GPT, Copilot, Cursor ou Gemini
- Use squash and merge para PRs com múltiplos commits


## Supply Chain
### Gestão de Dependências
- Toda nova dependência deve passar em `cargo deny check`
- Se o candidato traz nova licença fora da allowlist ou advisory transitivo, encontre alternativa ou documente o ignore em `deny.toml`
- Documente com linhas `# Why:` e `# How to apply:` no `deny.toml`
- Prefira crates com `trustScore >= 7` no `context7` (veja `CLAUDE.md`)


## Documentação Relacionada
### Links Úteis
- [NO_CI.md](NO_CI.md) — **política: proibido CI/CD e GitHub Actions** (gates só locais)
- [CHANGELOG.md](CHANGELOG.md) e [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md) — histórico bilíngue sincronizado
- [SECURITY.md](SECURITY.md) — política de reporte responsável e versões suportadas
- [INSTALL-WINDOWS.md](INSTALL-WINDOWS.md) — pré-requisitos BoringSSL no Windows (NASM, CMake, MSVC, Perl) — NOTE: since v0.8.6, `reqwest`+`rustls-tls` replaced BoringSSL/wreq, so these native build prerequisites are no longer required
- [INTEGRATIONS.md](INTEGRATIONS.md) — catálogo de integrações com 16+ agentes de IA
- [docs/INTEGRATIONS.md](docs/INTEGRATIONS.md) — guia completo de integração
- [docs/INSTALL-WINDOWS.pt-BR.md](docs/INSTALL-WINDOWS.pt-BR.md) — versão em português
- [docs/decisions/](docs/decisions/) — Architecture Decision Records (ADRs)
- [docs/CROSS_PLATFORM.md](docs/CROSS_PLATFORM.md) — comportamento por plataforma


## Pre-Publish (local only)
### Bloqueio Pré-Publicação
- **Proibido** CI/CD e GitHub Actions neste repositório (sem `.github/workflows` (diretório removido))
- Antes de publicar: rode os 10 gates locais e `cargo publish --dry-run --locked`
- Mantenedores: publique manualmente com `cargo publish --locked` após autorização explícita


## Workflow com Agent Teams
### Orquestração de Releases
- Releases da v0.7.8+ usaram o fluxo de 8 fases via Agent Teams
- Cada teammate recebe prompt autocontido com Regra Zero, identidade, contexto, ferramentas
- Líder coordena, delega, verifica — não implementa diretamente
- Ver `CLAUDE.md` na raiz para o protocolo completo
- ADRs em `docs/decisions/` documentam decisões tomadas por cada release


## How to Report Bugs
### Template de Bug Report
- Abra uma issue com título descritivo no formato: `[bug] descrição concisa do problema`
- Inclua versão da CLI: `duckduckgo-search-cli --version`
- Inclua sistema operacional e versão do Rust: `rustc --version`
- Inclua comando exato que reproduz o problema
- Inclua saída completa incluindo stderr


## How to Request Features
### Template de Feature Request
- Abra uma issue com título descritivo no formato: `[feature] descrição concisa`
- Descreva o problema que a feature resolveria
- Descreva o comportamento esperado
- Inclua exemplos de uso ou casos reais


## Reporting Security Issues
### Reporte Responsável
- Veja [SECURITY.md](SECURITY.md) para o processo completo
- Não abra issues públicas para vulnerabilidades
- Use private security report channel para divulgação responsável


## Release Process
### Fluxo de Release para Mantenedores
- Bump do campo `version` em `Cargo.toml`
- Atualize `CHANGELOG.md` movendo conteúdo de `[Unreleased]` para novo header de versão com data
- Sincronize `CHANGELOG.pt-BR.md` com a mesma entrada bilíngue
- Execute os 10 gates de validação **localmente** (sem Actions)
- Crie tag anotada: `git tag -a v0.X.Y -m "descrição"`
- Push: `git push origin main && git push origin v0.X.Y` (tag apenas; **sem** workflow de release)
- Publique no crates.io **manualmente**: `cargo publish --locked` (após dry-run e autorização)
- **Não** há matrix GitHub Actions, Dependabot (removed with Actions), zizmor (removed with Actions), pre-commit hooks (removed) nem secrets de GitHub Actions (proibidos)


## Notas da Release v0.7.8
### Oito Gaps Fechados (Anti-Bot Detector Overhaul)
- GAP-WS-50 — listas expandidas em `src/probe_deep.rs` (8 marcadores Cloudflare + 1 DDG)
- GAP-WS-51 — constante `PROBE_CALIBRATION_QUERY` em `src/lib.rs` para query canônica do probe
- GAP-WS-52 — predicado de fallback condicional em `src/search.rs` honra o detector real
- GAP-WS-53 — níveis `-vv` e `-vvv` adicionados em `src/cli.rs` com `ArgAction::Count`
- GAP-WS-54 — `scraper` bumpado para 0.27 resolve RUSTSEC-2025-0057 transitivo
- GAP-WS-55 — bloco wreq reescrito em `Cargo.toml` com pin exato em 6.0.0-rc.29
- GAP-WS-56 — subcomando `Buscar` marcado como `#[command(hide = true)]`
- GAP-WS-57 — `retries` agora honrado em `src/parallel.rs` no laço de error_output
- ADR completa em `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`


## Notas da Release v0.7.9

### Ghost-Block + Markers 2026 (Oito Gaps Fechados)
- GAP-WS-58 (CRITICAL) — `detectar_interstitial` classifica body sub-4KB sem `result-page-signal` como `InterstitialKind::Cloudflare`
- GAP-WS-59 (HIGH) — 5 marcadores Cloudflare novos + 1 marker DDG novo
- GAP-WS-59 (HIGH) — `--allow-lite-fallback` e `--pre-flight` viraram `global = true`
- v0.7.9 P1 — `detectar_interstitial_com_match` retorna `(&'static str, InterstitialKind)` com marker literal
- v0.7.9 P3 — `SearchMetadata.pre_flight_fired: bool` adicionado ao envelope
- v0.7.9 P4b — `sugestao_mitigacao_com_marker` injeta marker real (ex.: `cf-challenge`)
- `Config.pre_flight` adicionado com default `false`


## Notas da Release v0.7.10

### Pino de Identidade + Bench Wiring + Pre-Publish Gate (Sete Gaps Fechados)
- GAP-WS-60 (CRITICAL) — `--identity-profile` propaga para `failure_output` e `error_output` via `identity_tag_for_cli_identity` em `src/identity.rs`
- GAP-AUD-001 (auditoria local) — pino `identidade_usada` agora presente em failure paths (era `null`)
- GAP-AUD-002 (auditoria local) — `[[bench]] harness = false` em `Cargo.toml` corrige `cargo bench` que rodava test harness
- B1 (CRITICAL) — `--pre-flight` não emite mais dois JSON concatenados no stdout
- B2 (CRITICAL) — `pre_flight_blocked` agora retorna exit 3 (era 0)
- B3 (MÉDIO) — `--global-timeout` virou global, aceito em subcomandos
- B4 (CRITICAL) — `--probe-deep` standalone retorna exit 3 quando detecta captcha
- v0.7.10 P4 — `--require-results` em `deep-research`, exit 4 quando fan-out zero
- v0.7.10 P5 — probe-deep scheduler integrado em `execute_single_search`
- v0.7.10 P6 — snapshot test `cloudflare_markers_snapshot_v0_7_10` via `insta = "1"`
- v0.7.10 P7 — `src/proxy_detection.rs` novo módulo (Vivo Fiber, Gigaweb, Cloudflare)
- v0.7.10 P16 — `src/ddg_class_watch.rs` watchdog runtime
- v0.7.10 P19 — pre-publish checklist (local) local (script removido; gates 1–10 manuais)
- v0.7.10 P19 — `skill/duckduckgo-search-cli-{en,pt}/eval-queries.json` +4 queries (q47-q50)

### Mudança de Workflow (regra 1244)
- A partir de v0.7.10, releases usam `atomwrite` direto + `TaskCreate` em vez de Agent Teams, devido a bug conhecido de estado `Team does not exist` documentado em `mem 1244` do graphrag
- Lead continua orquestrando, mas edições atômicas vão via `atomwrite --workspace . write --expect-checksum <CS>`
- Cada patch: `atomwrite read` (checksum) → `atomwrite write` → `cargo check --offline` → `cargo test --lib --offline`
