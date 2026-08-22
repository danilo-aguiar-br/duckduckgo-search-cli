# Instalando duckduckgo-search-cli no Windows (atual: v1.0.6; notas TLS desde v0.8.6+)

[English](INSTALL-WINDOWS.md)

Desde a v0.8.6, `duckduckgo-search-cli` usa `reqwest` com `rustls-tls` no lugar de `wreq`/BoringSSL. Isso elimina a necessidade de NASM, CMake, Perl e MSVC. O único pré-requisito é o Rust. Release atual: v1.0.6.


## Pré-requisitos
- Windows 10 versão 1903 ou superior, ou Windows 11
- Toolchain Rust instalada via [rustup](https://rustup.rs/)


## Estado do registry — leia antes de instalar
- A v1.0.6 é a primeira release com o defeito de cfg-stripping fechado (GAP-REL-001)
- MEDIDO em 2026-08-21: o crates.io serve `1.0.2` como `max_stable_version`, e a `1.0.2` NÃO compila em macOS nem Windows
- Enquanto a `1.0.6` não estiver publicada, `cargo install ... --version 1.0.6` falha na hora com `could not find duckduckgo-search-cli with version 1.0.6`
- Essa falha rápida e nomeada é DELIBERADA: instalar sem fixar versão resolve em silêncio para a `1.0.2` quebrada e morre minutos depois com um `E0432` críptico
- Falha rápida que nomeia o que falta vence falha lenta que não nomeia
- Confira você mesmo o registry antes de reportar bug de instalação: `cargo run --bin verify_published --features release-gate`
- Esse gate sai com 0 somente quando o registry serve de fato a versão que esta árvore carrega


## Instalação

```powershell
cargo install duckduckgo-search-cli --locked --version 1.0.6
duckduckgo-search-cli --version
```

- SEMPRE fixe a versão, porque um `cargo install duckduckgo-search-cli` puro pode resolver para um build que NÃO compila no Windows
- As versões publicadas `1.0.2` e `1.0.1` NÃO compilam no Windows nem no macOS, falhando com `E0432` de import não resolvido
- Você DEVE instalar a `1.0.6` ou superior, que é a primeira versão publicada com essa classe de defeito fechada
- A invocação segura é `cargo install duckduckgo-search-cli --locked --version 1.0.6`

Sem shell especial, sem compiladores extras, sem assembler.


## Verificação pós-instalação
- O binário expõe exatamente dez comandos de topo: `buscar`, `init-config`, `completions`, `deep-research`, `commands`, `schema`, `doctor`, `locale`, `man`, `config`
- O subcomando `config` expõe exatamente seis operações: `path`, `list`, `get`, `set`, `unset`, `effective`
- O subcomando de listagem é `config list`, e `config list-keys` NÃO existe neste binário
- Rode `doctor` primeiro, porque ele é a única checagem que sonda o Chrome de que a produção realmente depende

```powershell
duckduckgo-search-cli doctor
duckduckgo-search-cli commands
duckduckgo-search-cli schema
duckduckgo-search-cli locale
duckduckgo-search-cli man
duckduckgo-search-cli init-config
duckduckgo-search-cli config list
duckduckgo-search-cli config effective
```

- `doctor` reporta a detecção do Chrome e sai com código não-zero sob `--strict` quando o Chrome está ausente
- `commands` imprime o inventário de comandos legível por máquina, então prova a superfície instalada sem adivinhação
- `schema` imprime o catálogo de JSON Schema, e `--name` restringe a um schema único
- `locale` imprime o locale de UI resolvido, e `man` imprime a man page em roff gerada da mesma árvore clap do `--help`
- `init-config` grava o arquivo de config XDG, e `config path` diz onde ele ficou


## Obrigatório: Chrome (transporte de rede de produção, v0.9.4+)

Ver [ADR-0016](decisions/0016-chrome-only-universal-v0-9-4.md) / GAP-WS-113 para a política de produção Chrome-only.
Ver [ADR-0018](decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md) para os defaults agent-ready da v0.9.8.

- Chrome/Chromium é OBRIGATÓRIO em produção (feature `chrome` é o padrão; GAP-WS-113). Busca, news, `deep-research`, `--probe`, `--probe-deep`, `--pre-flight` e fetch de conteúdo usam chromiumoxide/CDP
- Sem Chrome utilizável as operações de rede falham fechadas com exit 2 (env de produto `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` REMOVIDA / não lida — Chrome obrigatório via feature `chrome`)
- No Windows o Chrome roda em headless=new desde a v0.9.3 (Linux usa um display Xvfb privado)
- Desde a v0.9.6 a árvore de processos do Chrome é encerrada na saída (posse one-shot); a produção ainda exige Chrome instalado para operações de rede (ver [ADR-0017](decisions/0017-browser-lifecycle-one-shot-v0-9-6.md))
- v1.0.0 one-shot de disco (GAP-WS-TMP-PROFILE-ORPHAN-001 / [ADR-0020](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md)): prefixo de perfil Chrome é `ddg-chrome-*` sob o diretório temp do processo; árvore de processos e diretório de perfil são reaped no exit cooperativo (`force_reap` + `ExitReapGuard`); residual após SIGKILL é limpo na próxima run com `sweep_orphan_profiles` de SOMENTE `ddg-chrome-*` de propriedade. Política rígida: JAMAIS faça bulk-rm de `.tmp*` estrangeiro ou `org.chromium.Chromium.*` (nem outro temp Chromium). Audite residual sob `%TEMP%` / `$env:TEMP` listando apenas diretórios com nome `ddg-chrome-*`. Ver [ADR-0017](decisions/0017-browser-lifecycle-one-shot-v0-9-6.md) + [ADR-0020](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md)
- v1.0.1 / Pass 52: multi-query `--stream` / `-f ndjson` NDJSON; API dual `config` + `config effective`; BrokenPipe → exit 141 com reap oneshot SIG_IGN; wire PT serialize BC + aliases EN na desserialização ([ADR-0023](decisions/0023-wire-pt-bc-english-deserialize-aliases.md)); config de produto só CLI+XDG
- v1.0.2: wire inglês por padrão ([ADR-0027](decisions/0027-wire-en-default-v1-0-2.md)) + `--wire-keys en|pt`; agent ops; orçamento dual/contenção; Chrome sempre mudo ([ADR-0026](decisions/0026-chrome-mute-audio-operational-standard-v1-0-2.md)); FETCH_CAP padrão 4; DEFAULT_PAGES=1
- v0.9.8: padrão `--vertical all` e fetch de conteúdo LIGADO (top web + news, teto 4 (padrão v1.0.2)). Prefira timeouts mais longos (ex.: `Start-Process` do PowerShell ou timeout externo de 180s+) ao aceitar os padrões; caminho SERP fino: `--vertical web --no-fetch-content` com ~60s
- Instale o Google Chrome em https://www.google.com/chrome/
- Sem necessidade de `xvfb` no Windows
- Chrome é auto-detectado nos caminhos de instalação padrão; sobrescreva com CLI `--chrome-path` ou XDG `config set chrome_path` (env `CHROME_PATH` NÃO é lida)


## Histórico: v0.7.3 a v0.8.5 (era BoringSSL)

As versões v0.7.3 a v0.8.5 dependiam de `wreq`/BoringSSL, que exigia quatro ferramentas nativas de build no Windows:

- Assembler NASM
- CMake 3.20+
- Compilador + linker MSVC (Visual Studio Build Tools)
- Strawberry Perl

Se você está instalando uma versão mais antiga (v0.7.3 a v0.8.5), ainda precisa dessas ferramentas. Consulte a [versão v0.8.5 deste documento](https://github.com/danilo-aguiar-br/duckduckgo-search-cli/blob/v0.8.5/docs/INSTALL-WINDOWS.pt-BR.md) para o guia passo a passo completo.

Desde a v0.8.6, nenhuma delas é necessária.


## Troubleshooting

### `cargo install` falha com erros de rede

Certifique-se de que sua toolchain Rust está atualizada: `rustup update stable`

### Quer instalar uma versão específica

```powershell
cargo install duckduckgo-search-cli --locked --version 1.0.6 --force
```


## Veja também
- `docs/CROSS_PLATFORM.pt-BR.md` — visão geral de pré-requisitos de build por plataforma
- `docs/decisions/0016-chrome-only-universal-v0-9-4.md` — produção Chrome-only (GAP-WS-113)
- `docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md` — one-shot de processo (ADR-0017 / GAP-WS-LIFECYCLE-001)
- `docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md` — one-shot de disco + `ddg-chrome-*` (ADR-0020 / GAP-WS-TMP-PROFILE-ORPHAN-001)
- `docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md` — defaults agent-ready (v0.9.8)
- `docs/MIGRATION.pt-BR.md` — v0.9.7 → v0.9.8 defaults com breaking; v0.9.9/v0.9.10 → v1.0.0 one-shot de disco
