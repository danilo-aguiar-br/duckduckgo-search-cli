# Instalando duckduckgo-search-cli no Windows (v0.8.6+)

[English](INSTALL-WINDOWS.md)

Desde a v0.8.6, `duckduckgo-search-cli` usa `reqwest` com `rustls-tls` no lugar de `wreq`/BoringSSL. Isso elimina a necessidade de NASM, CMake, Perl e MSVC. O único pré-requisito é o Rust.


## Pré-requisitos

- Windows 10 versão 1903 ou superior, ou Windows 11
- Toolchain Rust instalada via [rustup](https://rustup.rs/)


## Instalação

```powershell
cargo install duckduckgo-search-cli
duckduckgo-search-cli --version
```

Só isso. Sem shell especial, sem compiladores extras, sem assembler.


## Obrigatório: Chrome (transporte de rede de produção, v0.9.4+)

Ver [ADR-0016](decisions/0016-chrome-only-universal-v0-9-4.md) / **GAP-WS-113** para a política de produção Chrome-only.
Ver [ADR-0018](decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md) para os defaults agent-ready da **v0.9.8**.

- Chrome/Chromium é **obrigatório em produção** (feature `chrome` é o padrão; GAP-WS-113). Busca, news, `deep-research`, `--probe`, `--probe-deep`, `--pre-flight` e fetch de conteúdo usam chromiumoxide/CDP
- Sem Chrome utilizável (ou com `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`) as operações de rede **falham fechadas com exit 2**
- No Windows o Chrome roda em headless=new desde a v0.9.3 (Linux usa um display Xvfb privado)
- Desde a v0.9.6 a árvore de processos do Chrome é encerrada na saída (posse one-shot); a produção ainda exige Chrome instalado para operações de rede (ver [ADR-0017](decisions/0017-browser-lifecycle-one-shot-v0-9-6.md))
- **v0.9.8**: padrão `--vertical all` e fetch de conteúdo **LIGADO** (top web + news, teto 10). Prefira timeouts mais longos (ex.: **180s+**) ao aceitar os padrões; caminho SERP fino: `--vertical web --no-fetch-content` com ~60s
- Instale o Google Chrome em https://www.google.com/chrome/
- Sem necessidade de `xvfb` no Windows
- Chrome é auto-detectado nos caminhos de instalação padrão; sobrescreva com `--chrome-path` ou `CHROME_PATH`


## Histórico: v0.7.3 a v0.8.5 (era BoringSSL)

As versões v0.7.3 a v0.8.5 dependiam de `wreq`/BoringSSL, que exigia quatro ferramentas nativas de build no Windows:

1. Assembler NASM
2. CMake 3.20+
3. Compilador + linker MSVC (Visual Studio Build Tools)
4. Strawberry Perl

Se você está instalando uma versão mais antiga (v0.7.3 a v0.8.5), ainda precisa dessas ferramentas. Consulte a [versão v0.8.5 deste documento](https://github.com/danilo-aguiar-br/duckduckgo-search-cli/blob/v0.8.5/docs/INSTALL-WINDOWS.pt-BR.md) para o guia passo a passo completo.

Desde a v0.8.6, nenhuma delas é necessária.


## Troubleshooting

### `cargo install` falha com erros de rede

Certifique-se de que sua toolchain Rust está atualizada: `rustup update stable`

### Quer instalar uma versão específica

```powershell
cargo install duckduckgo-search-cli --version 0.8.6 --force
```


## Veja também

- `docs/CROSS_PLATFORM.md` — visão geral de pré-requisitos de build por plataforma
- `docs/decisions/0016-chrome-only-universal-v0-9-4.md` — produção Chrome-only (GAP-WS-113)
- `docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md` — defaults agent-ready (v0.9.8)
- `docs/MIGRATION.pt-BR.md` — v0.9.7 → v0.9.8 defaults com breaking
