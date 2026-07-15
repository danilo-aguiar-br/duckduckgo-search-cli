# Como Contribuir

- Obrigado pelo seu interesse em contribuir!
- Este guia cobre o mínimo necessário para publicar uma mudança com sucesso.


## Início Rápido

```bash
git clone https://github.com/danilo-aguiar-br/duckduckgo-search-cli
cd duckduckgo-search-cli
cargo check-all    # gate 1 — compila
cargo lint         # gate 2 — clippy -D warnings
cargo fmt --check  # gate 3 — format
cargo test-all     # gate 5 — todos os testes (unit + integration + doctest)
```

- Aliases disponíveis em `.cargo/config.toml`.


## Matriz de Validação com 10 Gates

- Todo PR deve passar pelos 10 gates (rodar localmente; sem GitHub Actions nesta conta):

| # | Gate | Comando local |
|---|------|---------------|
| 1 | Compilação | `cargo check-all` |
| 2 | Clippy | `cargo lint` |
| 3 | Formatação | `cargo fmt --check` |
| 4 | Docs | `RUSTDOCFLAGS="-D warnings" cargo docs` |
| 5 | Testes | `cargo test-all` |
| 6 | Cobertura >= 80% | `cargo cov` |
| 7 | Auditoria de vuln | `cargo audit` |
| 8 | Supply chain | `cargo deny check advisories licenses bans sources` |
| 9 | Dry-run de publish | `cargo publish-check` |
| 10 | Conteúdo do pacote | `cargo pkg-list` |


## Pré-requisitos de Desenvolvimento Chrome (v0.8.9+)
- Instale Google Chrome ou Chromium para testes E2E
- Linux: Xvfb é auto-instalado pela CLI em runtime via `try_auto_install_xvfb()` para 22+ distros
- Para desenvolvimento, instale manualmente: `sudo dnf install xorg-x11-server-Xvfb` (Fedora) ou `sudo apt-get install xvfb` (Debian/Ubuntu)
- macOS/Windows: sem dependência extra — Chrome roda em **headless=new** desde a v0.9.3 (não headed nativo Quartz/DWM; esse caminho era só v0.9.1 e foi supersedido)
- Executar testes E2E: `cargo test --all-features` (CLI spawna Xvfb automaticamente se necessário)
- Executar testes sem Chrome: `cargo test --no-default-features`
- Forçar headless para testes: `DUCKDUCKGO_CHROME_HEADLESS=1 cargo test`
- A feature `chrome` é habilitada por padrão no `Cargo.toml`
- **Defaults agent-ready (v0.9.8)** afetam latência E2E: fetch de conteúdo **LIGADO** e vertical padrão **`all`** (dual web+news). Prefira timeouts maiores ou use `--vertical web --no-fetch-content` para smoke fino/rápido.
- **E2E Flatpak multi-canal (v0.9.8)** — gated por `DUCKDUCKGO_FLATPAK_E2E=1`:

  ```bash
  DUCKDUCKGO_FLATPAK_E2E=1 cargo test --test integration_flatpak_chrome -- --nocapture
  ```

  Cobre resolve Flatpak export→ELF (`files/extra/chrome`) quando há Chrome Flatpak instalado.
- **E2E de lifecycle (v0.9.6, GAP-WS-LIFECYCLE-001)** — gated por `DUCKDUCKGO_LIFECYCLE_E2E=1`:

  ```bash
  DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle -- --nocapture
  ```

  Exige Chrome; afirma que nenhum processo chrome residual permanece com o `user-data-dir` desta execução após a saída. Testes unitários cobrem `process_lifecycle` sem a env E2E.


## Padrões de Código

- Idioma: comentários de código, mensagens de log e nomes de campos de structs devem ser em português brasileiro conforme `CLAUDE.md`
- Identificadores de API pública podem ser em inglês quando isso corresponder ao estilo Rust convencional (ex: `from`, `into`)
- Tratamento de erros: propague erros com `?` e a variante tipada definida em `src/error.rs` (enum `CliError` via `thiserror`)
- O projeto usa `thiserror 2` puro — `anyhow` NÃO está nas dependências
- NUNCA use `.unwrap()` ou `.expect()` em código de produção — propague com `?`
- I/O: o módulo `output.rs` é o ÚNICO lugar autorizado a chamar `println!` / `print!`
- Todos os outros módulos registram via `tracing`
- TLS: somente `rustls` — não reative `native-tls` pois quebra NixOS, Alpine e builds musl estáticos
- Sem cache, sem MCP, sem API paga — restrições de design inegociáveis conforme o blueprint v2


## Testes

- Três camadas de teste são obrigatórias:
- Testes unitários inline (`#[cfg(test)] mod testes`) para funções puras
- Testes de integração em `tests/` usando `wiremock` — ZERO HTTP real
- Doctests dentro de blocos `///` na API pública — funcionam também como exemplos no docs.rs
- `cargo llvm-cov` deve manter >= 80% geral
- Qualquer PR que reduza a cobertura abaixo do limite falhará no CI


### News Vertical (v0.8.9)
- Fixtures em `tests/fixtures/`: `ddg_news_serp.html` (Estratégia A, 7 artigos + 1 armadilha interna filtrada), `ddg_news_serp_ofuscada.html` (fallback Estratégia B), `ddg_news_serp_vazia.html` (SERP vazia → `causa_zero: vertical-sem-resultados`)
- Testes de integração: `tests/integration_news_vertical.rs`, `tests/integration_deep_research_news.rs` — rode com `cargo test --features chrome --test integration_news_vertical --test integration_deep_research_news`
- Hot-fix sem recompilar: quebra de seletores no lado do DDG é corrigível via `config/selectors.toml` seção `[news]` (Estratégia A); a Estratégia B é a rede de segurança agnóstica a classes
- Veja `docs/TESTING.md` para a matriz completa de testes da vertical news


## Supply Chain

- Toda nova dependência deve passar por `cargo deny check`
- Se o candidato trouxer uma nova licença fora da allowlist ou um advisory transitivo, você deve encontrar uma alternativa ou documentar o ignore em `deny.toml` com linhas `# Why:` e `# How to apply:`
- Prefira crates com `trustScore >= 7` no `context7` (veja `CLAUDE.md`)


## Higiene de Commits

- NUNCA adicione trailers `Co-authored-by:` de agentes de IA (dependabot, renovate, Claude, GPT, Copilot, Cursor, Gemini, etc.) — o CI bloqueia esses trailers
- Use `squash and merge` para PRs com múltiplos commits
- Mensagens de commit seguem prefixos convencionais: `feat:`, `fix:`, `deps:`, `ci:`, `docs:`, `test:`, `refactor:`


## Reportando Problemas de Segurança

- Veja [SECURITY.pt-BR.md](SECURITY.pt-BR.md) para detalhes completos
- NÃO abra issues públicas para vulnerabilidades
- Use GitHub Security Advisories em vez disso


## Processo de Release

- Releases são orientadas por tags:
- Atualize `version` em `Cargo.toml`
- Atualize `CHANGELOG.md` (mova o conteúdo de `[Unreleased]` para um novo cabeçalho de versão com data)
- Execute `git tag v0.X.Y && git push origin v0.X.Y`
- `.github/workflows/release.yml` cuida do resto: matriz de build (5 targets + macOS Universal), GitHub Release, publicação no crates.io
- Mantenedores: certifique-se de que o secret `CRATES_IO_TOKEN` está configurado antes de criar a tag

## Notas da Release v0.7.8

### Oito Gaps Fechados (Reformulação do Detector Anti-Bot)
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
- v0.7.10 P19 — `scripts/pre-publish-gate.sh` 7 gates antes de `cargo publish`
- v0.7.10 P19 — `skill/duckduckgo-search-cli-{en,pt}/eval-queries.json` +4 queries (q47-q50)

### Mudança de Workflow (regra 1244)
- A partir de v0.7.10, releases usam `atomwrite` direto + `TaskCreate` em vez de Agent Teams, devido a bug conhecido de estado `Team does not exist` documentado em `mem 1244` do graphrag

## Configuração do Ambiente de Desenvolvimento
### Pré-requisitos
- MSRV (Versão Mínima do Rust Suportada): Rust 1.88 — declarado em `Cargo.toml` (`rust-version`) e travado em `rust-toolchain.toml`
- Execute `rustup update stable` para garantir a versão correta
- Instale llvm-cov com: `cargo install cargo-llvm-cov`
- Instale cargo-audit com: `cargo install cargo-audit`
- Instale cargo-deny com: `cargo install cargo-deny`
- O projeto NÃO usa `cargo-nextest` — a suíte roda via `cargo test` padrão


## Pré-requisitos Chrome para Desenvolvimento (v0.8.9+)
- Instale Google Chrome ou Chromium para testes E2E
- Linux: Xvfb é auto-instalado pela CLI em runtime via `try_auto_install_xvfb()` para 22+ distros
- Para desenvolvimento, instale manualmente: `sudo dnf install xorg-x11-server-Xvfb` (Fedora) ou `sudo apt-get install xvfb` (Debian/Ubuntu)
- macOS/Windows: sem dependência extra — Chrome roda em **headless=new** desde a v0.9.3 (não headed nativo Quartz/DWM; supersedido)
- Executar testes E2E: `cargo test --all-features` (CLI spawna Xvfb automaticamente se necessário)
- Execute testes sem Chrome: `cargo test --no-default-features`
- Forçar headless para testes: `DUCKDUCKGO_CHROME_HEADLESS=1 cargo test`
- A feature `chrome` é habilitada por padrão no `Cargo.toml`
- Testes stealth do Chrome estão em `tests/integration_chrome_stealth.rs`
- Testes Chrome do deep-research estão em `tests/integration_deep_research.rs`
- **Defaults agent-ready (v0.9.8)** afetam latência E2E: fetch ON + vertical dual; use timeouts maiores ou `--vertical web --no-fetch-content` para smoke fino
- **E2E Flatpak multi-canal (v0.9.8)** — `DUCKDUCKGO_FLATPAK_E2E=1 cargo test --test integration_flatpak_chrome -- --nocapture`
- **E2E de lifecycle (v0.9.6, GAP-WS-LIFECYCLE-001)** — gated por `DUCKDUCKGO_LIFECYCLE_E2E=1`:

  ```bash
  DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle -- --nocapture
  ```

  Exige Chrome; afirma que nenhum processo chrome residual permanece com o `user-data-dir` desta execução após a saída. Testes unitários cobrem `process_lifecycle` sem a env E2E.


## Código de Conduta
### Contrato Social
- Este projeto adota o [Contributor Covenant](CODE_OF_CONDUCT.md)
- Leia integralmente antes de abrir qualquer issue ou pull request
- Reporte violações seguindo o canal descrito em `CODE_OF_CONDUCT.md`


## Estratégia de Branches
### Fluxo de Branches
- Ramificação principal: `main`
- Branches de feature: `feature/nome-descritivo` a partir de main
- Branches de fix: `fix/nome-do-bug` a partir de main
- Abra PR de volta para main
- Squash and Merge é o método padrão de merge


## Checklist de Pull Request
### Itens Verificáveis Antes de Abrir PR
- `cargo fmt --all -- --check` retorna ZERO diferenças
- `cargo clippy --all-targets --all-features -- -D warnings` retorna ZERO warnings
- `cargo test --all-features` retorna ZERO falhando
- `cargo doc --no-deps` sem warnings
- `cargo audit --deny warnings` sem vulnerabilidades conhecidas
- CHANGELOG.md e CHANGELOG.pt-BR.md atualizados com a mudança
- Título do PR descreve o problema resolvido em termos do usuário


## Documentação Relacionada
### Links Úteis
- [CHANGELOG.md](CHANGELOG.md) e [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md) — histórico bilíngue sincronizado
- [SECURITY.md](SECURITY.md) — política de reporte responsável e versões suportadas
- [INSTALL-WINDOWS.md](INSTALL-WINDOWS.md) — pré-requisitos BoringSSL no Windows (NASM, CMake, MSVC, Perl) — NOTA: desde v0.8.6, `reqwest`+`rustls-tls` substituiu BoringSSL/wreq, então esses pré-requisitos nativos de build não são mais necessários
- [INTEGRATIONS.md](INTEGRATIONS.md) — catálogo de integrações com 16+ agentes de IA
- [docs/INTEGRATIONS.md](docs/INTEGRATIONS.md) — guia completo de integração
- [docs/INSTALL-WINDOWS.pt-BR.md](docs/INSTALL-WINDOWS.pt-BR.md) — versão em português
- [docs/decisions/](docs/decisions/) — Architecture Decision Records (ADRs)
- [docs/CROSS_PLATFORM.md](docs/CROSS_PLATFORM.md) — comportamento por plataforma


## Bloqueio Pré-Publicação
### Bloqueio Pré-Publicação
- O job `pre-publish` em `.github/workflows/release.yml` exige `CARGO_INSTALL_FLAGS=--locked`
- Compilação do `cargo install` é parte do gate — sem network ímpar no build
- Mantenedores: nunca publique sem essa flag ativa
- Falha do `pre-publish` cancela a release no crates.io


## Workflow com Agent Teams
### Orquestração de Releases
- Releases da v0.7.8+ usaram o fluxo de 8 fases via Agent Teams
- Cada teammate recebe prompt autocontido com Regra Zero, identidade, contexto, ferramentas
- Líder coordena, delega, verifica — não implementa diretamente
- Ver `CLAUDE.md` na raiz para o protocolo completo
- ADRs em `docs/decisions/` documentam decisões tomadas por cada release


## Como Reportar Bugs
### Template de Bug Report
- Abra uma issue com título descritivo no formato: `[bug] descrição concisa do problema`
- Inclua versão da CLI: `duckduckgo-search-cli --version`
- Inclua sistema operacional e versão do Rust: `rustc --version`
- Inclua comando exato que reproduz o problema
- Inclua saída completa incluindo stderr


## Como Solicitar Features
### Template de Feature Request
- Abra uma issue com título descritivo no formato: `[feature] descrição concisa`
- Descreva o problema que a feature resolveria
- Descreva o comportamento esperado
- Inclua exemplos de uso ou casos reais
