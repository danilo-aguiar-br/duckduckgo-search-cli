# Suporte Multiplataforma

[English](CROSS_PLATFORM.md)


## Por Que Zero Dependências Importam
- v1.0.6 (release atual): wire JSON serializa chaves em inglês por padrão ([ADR-0027](decisions/0027-wire-en-default-v1-0-2.md)): `results`, `title`, `metadata`, `result_count`, `used_chrome`, `chrome_channel`, `chrome_path_resolved`, `execution_time_ms`, … Emit legado PT: `--wire-keys pt` ou `config set wire_keys pt`. Agent ops (`--fields`/`--filter`/`--sort`/`--limit`/`--count-only`/…); orçamento dual/contenção fail-fast ([ADR-0024](decisions/0024-deep-research-budget-contract-v1-0-2.md)/[0025](decisions/0025-deep-research-budget-contention-aware-v1-0-2.md)); Chrome sempre mudo ([ADR-0026](decisions/0026-chrome-mute-audio-operational-standard-v1-0-2.md)); defaults `max-sub-queries=3` / `fetch-content-cap=4` / `DEFAULT_PAGES=1`. Sem telemetria remota
- v1.0.1+ (Pass 52): oneshot pipe-safe — `ensure_oneshot_cleanup` em todas as saídas (inclusive pipe cedo); SIGPIPE Unix permanece SIG_IGN (não SIG_DFL) para Drop/reap ainda rodarem; stream BrokenPipe → exit 141 com órfãos Chrome 0. Também API dual de `config` + `config effective`, alias `-f ndjson` de `--stream`, aliases EN na desserialização wire (ADR-0023; serialize PT BC — supersedido na serialização pelo ADR-0027). GAP-WS-TMP-PROFILE-ORPHAN-001 / [ADR-0020](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md) — one-shot de disco além do processo: perfis Chrome com prefixo auditável `ddg-chrome-*` (não `.tmp` genérico); `force_reap` / `ExitReapGuard` remove o diretório do perfil; `sweep_orphan_profiles` na próxima run limpa SOMENTE `ddg-chrome-*` stale de propriedade (nunca bulk-rm de `.tmp*` estrangeiro nem `org.chromium.Chromium.*`). Inventário: `gaps.md`. Sem telemetria remota
- v0.9.8+: GAP-WS-AGENT-READY-001 / ADR-0018 — vertical padrão `all` (web + notícias); fetch de conteúdo LIGADO por padrão (top web + news, teto 4 (padrão v1.0.2); opt-out `--no-fetch-content`); Chrome multi-canal no Linux (shell Flatpak de export → ELF de deploy; Chrome/Chromium do host → Flatpak → Snap); flags de transporte `global = true` incluindo `--chrome-path` após subcomandos; metadados agent `chrome_path_resolved` / `chrome_channel` / `used_chrome` honesto (NÃO telemetria; nomes PT legados via `--wire-keys pt`). Continua propriedade one-shot de processos da v0.9.6 (GAP-WS-LIFECYCLE-001 / ADR-0017). Prefira timeouts com SIGTERM primeiro (GNU `timeout`). Sem telemetria
- v0.9.6+: propriedade one-shot de processos (GAP-WS-LIFECYCLE-001 / ADR-0017) — cada invocação da CLI reap completa a árvore Chromium/Xvfb na saída (process group, walk da árvore, marker de `user-data-dir`; no Linux também `setpgid` + PDEATHSIG). Órfãos de processo históricos pré-0.9.6 e detritos de perfil `.tmp` pré-1.0.0 NÃO são limpos em massa; residual de SIGKILL pode deixar dirs até o sweep da próxima run só em `ddg-chrome-*`
- v0.9.4+: transporte de rede de produção é Chrome-only (GAP-WS-113 / ADR-0016). Build sem Chrome utilizável → exit 2 fail-closed (env de produto `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` REMOVIDA / não lida). `--allow-lite-fallback` é no-op. HTTP residual só em `http-test-harness`. Feature `chrome` é o padrão
- v0.9.3+: macOS/Windows mudaram para headless=new (GAP-WS-112) — Quartz/DWM faziam clamp de `--window-position`, deixando a janela headed-native visível; Linux mantém Xvfb privado (`HeadedXvfb`)
- v0.9.2+: hardening de stealth — chromiumoxide `--enable-automation` removido, UA alinhado à versão real do Chrome via Client Hints, WebRTC e QUIC desativados (GAP-WS-108/109/110/111)
- v0.9.1+: macOS/Windows com headed nativo Quartz/DWM + coerção de plataforma no UA (`ua_platform_matches_host`) (GAP-WS-107) — supersedido em v0.9.3
- v0.8.9+: vertical de notícias via `--vertical <web|news|all>` (GAP-WS-104) — roteada exclusivamente pelo transporte Chrome-primary em TODAS as plataformas (a SERP de notícias exige JavaScript; NÃO há fallback HTTP)
- v0.8.8+: limpeza de lock files Xvfb obsoletos via `is_lock_stale()` com verificação de PID (GAP-WS-089). Flag `--num` honrada no caminho Chrome headed (GAP-WS-090). Exit code 6 para bloqueios suspeitos via classificador ZeroCause (GAP-WS-099)
- v0.8.7+: `has_native_display()` detecta display nativo por plataforma. Xvfb auto-instalado via `try_auto_install_xvfb()` para 22+ distros Linux. 17 sinais stealth injetados via CDP. Alinhamento UA/TLS via `chrome_only_ua_for_platform()`. Navegação warm-up para duckduckgo.com
- v0.8.6+: `duckduckgo-search-cli` usa `reqwest` + `rustls-tls` — TLS puro Rust com zero dependências nativas de C. `cmake`, `perl`, `pkg-config`, `libclang-dev` e NASM NÃO são mais necessários
- v0.8.0–v0.9.3 (histórico): Chrome headed (via `chromiumoxide`) era o transporte de busca primário, com HTTP residual como fallback — o stack TLS do cliente HTTP importava menos para anti-bot. Desde a v0.9.4 (GAP-WS-113 / ADR-0016) o Chrome é o ÚNICO transporte de rede de produção
- Instalar o binário pré-compilado em um container Alpine recém-criado exige zero pacotes extras do sistema
- Builds musl estáticos vinculam cada byte em tempo de compilação — o binário roda em qualquer kernel Linux
- Sem Java Virtual Machine, sem runtime Python, sem gerenciador de processos Node.js para instalar
- O tempo de inicialização fica abaixo de 100 milissegundos porque o runtime Rust é uma camada estática fina
- A codificação UTF-8 no Windows é aplicada automaticamente via `SetConsoleOutputCP(65001)` na inicialização
- SIGPIPE permanece SIG_IGN (padrão Rust / política v1.0.1) para que `| head` produza EPIPE → exit 141 e o reap one-shot do Chrome (`ensure_oneshot_cleanup`) ainda rode — NÃO use SIG_DFL (mataria o processo antes do Drop)
- v0.7.3–v0.8.5 apenas: a compilação do BoringSSL exigia `cmake`, `perl`, `pkg-config` e `libclang-dev` no Linux. Isto NÃO é mais necessário a partir da v0.8.6


## Matriz de Suporte

| Target | SO | Status | Observações |
|---|---|---|---|
| `x86_64-unknown-linux-gnu` | Ubuntu, Debian, Fedora, RHEL | Suportado | Requer glibc 2.17+ |
| `x86_64-unknown-linux-musl` | Alpine, containers mínimos | Suportado | Binário estático, zero deps do sistema |
| `aarch64-apple-darwin` | Apple Silicon M1/M2/M3 | Suportado | Desempenho ARM64 nativo |
| `x86_64-apple-darwin` | Intel Mac | Suportado | Parte do binário Universal macOS |
| `x86_64-pc-windows-msvc` | Windows 10/11 | Suportado | UTF-8 configurado automaticamente |


## Honestidade — superfície de validação (leia isto)

Este repositório é CI-less (`NO_CI.md`: sem GitHub Actions / sem pipeline remoto). Trate a matriz acima como intenção de código + docs, não como “verde em todo host toda noite”.

| Superfície | Realidade neste projeto |
|---|---|
| E2E real (gated network / Chrome) | Exercitado localmente no Linux pelos mantenedores. Não é matriz multi-OS de CI. |
| Runtime Windows / macOS | Paths `cfg(target_os = …)` existem; e2e real completo em Win/mac NÃO é reivindicado por CI do projeto (não há). |
| Cross-check Windows a partir de Linux | Funciona, e é gate obrigatório. `cargo check-windows` (`x86_64-pc-windows-gnu`, `--no-default-features --features chrome`) exige só `rustup target add`. Desde a v1.0.3 / ADR-0029 o perfil default não tem C, então o bloqueio antigo em `aws-lc-sys` / `x86_64-w64-mingw32-gcc` acabou. |
| Cross-build Windows a partir de Linux | Continua sem garantia. `cargo check` não linka; um `cargo build --target x86_64-pc-windows-msvc` completo exige linker real + Windows SDK. Prefira build nativo no Windows MSVC. |
| Cross-check macOS a partir de Linux | Funciona, e é gate obrigatório. `cargo check-macos` (`aarch64-apple-darwin`) e `cargo check-macos-intel` (`x86_64-apple-darwin`) exigem só `rustup target add` — sem `zig`, sem `cargo-zigbuild`, sem SDK da Apple. |
| Aliases de cargo que são o gate real | Definidos em `.cargo/config.toml`: macOS `cargo check-macos` / `cargo check-macos-intel` / `cargo lint-macos`; Windows `cargo check-windows` / `cargo check-windows-msvc` / `cargo lint-windows`; Linux `cargo check-linux`; perfil distribuído `cargo check-nohttp` / `cargo check-nohttp-all-targets` / `cargo lint-nohttp`. Todos usam `--no-default-features --features chrome --locked`. |
| Cross-build macOS a partir de Linux | Não é tentado. Linkar binário macOS exige o SDK da Apple, que o `zig` não distribui; ver ADR-0028. |
| `doctor --strict` | Detecta major do Chrome; exit não-zero se Chrome ausente ou major muito à frente do baseline PDL. Sem novas vars de env de produto. |


## A armadilha do cfg-stripping — a classe de defeito que este guia existe para evitar
### O que o compilador realmente faz
- O `rustc` REMOVE itens desabilitados por `cfg` ANTES de resolver nomes, não depois
- Um `use` é resolução de nome, então ele roda contra uma árvore da qual o item já sumiu
- Um `use` sem gate de um item com gate é, portanto, `E0432` em toda plataforma onde o gate é falso
- O erro diz `unresolved import`, e a nota diz `found an item that was configured out`
### Por que call sites corretos não salvam você
- Colocar `#[cfg(target_os = "linux")]` em cada CALL SITE NÃO é suficiente
- O `use` no topo do módulo é ele próprio um item, e precisa do PRÓPRIO gate
- Um módulo pode estar perfeito no Linux e falhar ao compilar no macOS só por esse motivo
- Isto não é bug de runtime e nenhum teste pega; só compilar para o alvo pega
### O histórico medido neste projeto
- A v1.0.0 estava limpa: o lint de portabilidade sai 0 e 16 símbolos de plataforma estão corretamente guardados
- A v1.0.1 introduziu o defeito em `src/browser/session.rs:7`, duas violações
- A v1.0.2 carregou a mesma classe em `src/browser/session/mod.rs:20`, e essa é a versão que o crates.io serviu
- Ambas nasceram da MESMA operação: dividir um módulo grande em diretório
- A v1.0.6 encontrou mais três `E0432` desta classe escondidos na árvore de testes, invisíveis para `--all-features`
### A regra que decorre disso
- NUNCA conclua que uma plataforma compila lendo o código
- Ler prova que um caminho `cfg` EXISTE; nunca prova que a crate resolve naquele alvo
- Só `cargo check --target <triple>` prova compilação, porque só ele roda resolução de nomes lá
- Rode `cargo check-linux`, `cargo check-macos`, `cargo check-windows` e `cargo check-windows-msvc` depois de QUALQUER split de módulo
- Rode também `cargo check-nohttp-all-targets`, porque `cargo test-all` usa `--all-features` e é cego ao perfil distribuído
- O ponto cego INVERTE conforme o host: em host Linux macOS e Windows ficam descobertos, em host macOS o Linux fica
- `cargo check` nunca linka, então o std do alvo basta e nenhuma toolchain C é exigida


## Linux
### glibc — x86_64-unknown-linux-gnu
- Targeia Ubuntu 20.04+, Debian 11+, Fedora 37+, RHEL 8+
- Requer glibc versão 2.17 ou superior — presente em todas as distribuições atuais
- Baixe o binário pré-compilado do GitHub Releases ou instale via `cargo install`
- v0.8.6+: compilar do código-fonte exige apenas o toolchain Rust — sem compilador C, `cmake`, `perl`, `pkg-config` ou `libclang-dev` (TLS é puro Rust via `reqwest` + `rustls`)
- v0.7.3–v0.8.5 apenas: compilar do código-fonte exigia a toolchain C do BoringSSL (`cmake`, `perl`, `pkg-config`, `libclang-dev`). Isto NÃO é mais necessário a partir da v0.8.6
- v0.9.6+ (GAP-WS-LIFECYCLE-001 / ADR-0017): contrato one-shot de processos — cada invocação reap a árvore Chromium/Xvfb via `process_lifecycle` (kill de process group, walk da árvore, marker de `user-data-dir`). No Linux, filhos Xvfb/Chrome usam `setpgid` e `PR_SET_PDEATHSIG(SIGKILL)` para que a árvore do display virtual morra com o pai da CLI; `XvfbGuard` limpa arquivos de lock/socket
- v1.0.0+ (GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020): one-shot de disco — o `TempDir` de perfil usa o prefixo `ddg-chrome-` (Unix `0o700`), não `.tmp` genérico; `force_reap` / `ExitReapGuard` faz `remove_dir_all` após matar o processo; `sweep_orphan_profiles` na próxima run atua SOMENTE em `ddg-chrome-*` stale de propriedade (nunca bulk-rm de `.tmp*` estrangeiro nem `org.chromium.Chromium.*`). Inventário: `gaps.md`
- v1.0.1+ (Pass 52): oneshot pipe-safe — `ensure_oneshot_cleanup` em todas as saídas inclusive pipe cedo; SIGPIPE Unix permanece SIG_IGN para Drop/reap ainda rodarem quando `| head` fecha cedo; stream BrokenPipe → exit 141. Dual `config get`/`set`/`unset` + `config effective`; alias `-f ndjson` de `--stream`. Sem telemetria remota
- v0.9.8+ (GAP-WS-AGENT-READY-001 / ADR-0018) Chrome multi-canal (Linux Flatpak): shells de export Flatpak (`/var/lib/flatpak/exports/bin/com.google.Chrome`, usuário `~/.local/share/flatpak/exports/bin/…`) e wrappers Fedora Chromium resolvem para ELF reais de deploy (`files/extra/chrome`, `files/bin/chromium`). Ordem de candidatos: CLI `--chrome-path` → XDG `config set chrome_path` → Chrome do host → Chromium do host → Flatpak → Snap (env `CHROME_PATH` NÃO é lida). Paths de deploy Flatpak podem exigir `--no-sandbox`. Metadados reportam `chrome_channel` (`manual|host|flatpak|snap`) e `chrome_path_resolved` (contrato agent, NÃO telemetria; wire EN padrão desde v1.0.2 / ADR-0027 — legado PT `chrome_canal` / `chrome_path_resolvido` via `--wire-keys pt`). E2E opcional: `DUCKDUCKGO_FLATPAK_E2E=1`
- v1.0.2+: wire inglês padrão (ADR-0027); agent ops; orçamento dual/contenção + mute-audio sempre; FETCH_CAP padrão 4; DEFAULT_PAGES=1
- Funciona dentro do WSL2 (Windows Subsystem for Linux) sem nenhuma configuração extra
### musl — x86_64-unknown-linux-musl
- Targeia Alpine Linux, containers Docker mínimos e ambientes embarcados
- O binário é 100% vinculado estaticamente — zero dependências de runtime no sistema host
- Funciona em imagens Docker `FROM scratch` porque nenhuma libc é carregada em runtime
- Compile localmente com `cargo build --release --target x86_64-unknown-linux-musl`
- Requer `musl-tools` na máquina de build: `apt install musl-tools` no Debian ou `apk add musl-dev` no Alpine
- Binários musl pré-compilados são anexados aos GitHub Releases (quando publicados) com verificação via `SHA256SUMS.txt`


## macOS
### Apple Silicon — aarch64-apple-darwin
- Roda nativamente em processadores M1, M2 e M3 sem tradução Rosetta
- A execução ARM64 nativa elimina completamente o overhead de tradução de instruções
- Disponível como binário independente ou como parte do binário Universal macOS mesclado com `lipo`
- Instale via `cargo install duckduckgo-search-cli --locked --version 1.0.6` para compilar para a arquitetura do host
- SEMPRE fixe a versão, porque as publicadas `1.0.2` e `1.0.1` NÃO compilam no macOS nem no Windows (GAP-REL-001)
### Intel — x86_64-apple-darwin
- Targeia Macs Intel Core i5/i7/i9 rodando macOS 10.15 Catalina ou superior
- Roda sob Rosetta 2 no Apple Silicon sem penalidade de desempenho para a maioria das cargas de trabalho
- O binário Universal inclui ambas as fatias — o macOS seleciona a fatia correta automaticamente
### Lifecycle de processos (v0.9.6+ processo / v1.0.0 disco)
- O reap one-shot também se aplica no macOS — a árvore multi-processo do Chrome + perfil por sessão sob prefixo `ddg-chrome-*` (não `.tmp` genérico) / marker de `user-data-dir` são limpos na saída via shutdown de `ChromeBrowser` e force-reap no `Drop` (`remove_dir_all`)
- PDEATHSIG é específico do Linux — no macOS o reap usa walk da árvore, marker e RAII Drop, não sinal de morte do pai
- Prefira timeouts que enviam SIGTERM primeiro para que o cancel cooperativo execute o caminho de reap; residual de SIGKILL nu pode deixar dirs até o `sweep_orphan_profiles` da próxima run SOMENTE em `ddg-chrome-*` de propriedade (nunca bulk-rm de `.tmp*` / `org.chromium.Chromium.*`)
### Gatekeeper e Primeira Execução
- Binários pré-compilados baixados do GitHub não são assinados — o Gatekeeper os coloca em quarentena na primeira execução
- Remova a flag de quarentena uma única vez com este comando:

```bash
xattr -dr com.apple.quarantine /usr/local/bin/duckduckgo-search-cli
```

- Alternativamente, instale via `cargo install` — binários compilados pelo Cargo ignoram o Gatekeeper
- Assinatura ad-hoc para builds locais: `codesign -s - /usr/local/bin/duckduckgo-search-cli`


## Windows
### Pré-requisitos
- Windows 10 versão 1903 ou superior, ou Windows 11 (qualquer versão)
- PowerShell 5.1+ ou PowerShell 7+ — ambos funcionam sem configuração adicional
- Adicione o binário a um diretório no `%PATH%` como uma pasta de ferramentas personalizada
- Instale via `cargo install duckduckgo-search-cli --locked --version 1.0.6` — o Cargo coloca o binário em `%USERPROFILE%\.cargo\bin`
- SEMPRE fixe a versão, porque as publicadas `1.0.2` e `1.0.1` NÃO compilam no Windows nem no macOS (GAP-REL-001)
- v0.8.6+: nenhuma ferramenta extra além do toolchain Rust — TLS é puro Rust via `reqwest` + `rustls`
- v0.7.3–v0.8.5 apenas: o build nativo MSVC exigia quatro ferramentas extras — (1) assembler NASM, (2) CMake 3.20+, (3) MSVC C/C++ toolchain, (4) Strawberry Perl. Nenhuma dessas é necessária a partir da v0.8.6
### Lifecycle de processos (v0.9.6+ processo / v1.0.0 disco)
- O reap one-shot também se aplica no Windows — a árvore multi-processo do Chrome + perfil por sessão sob prefixo `ddg-chrome-*` (não `.tmp` genérico) / marker de `user-data-dir` são limpos na saída via shutdown de `ChromeBrowser` e force-reap no `Drop` (`remove_dir_all`)
- PDEATHSIG é específico do Linux — no Windows o reap usa walk da árvore, marker e RAII Drop (sem Xvfb)
- Prefira cancelamento cooperativo (parada graceful / equivalente a SIGTERM) em vez de hard-kill imediato para que o caminho de reap complete; residual: hard-kill externo pode deixar detritos de processo/disco até o sweep da próxima run só em `ddg-chrome-*` de propriedade (nunca bulk-rm de `.tmp*` estrangeiro / `org.chromium.Chromium.*`)
### Saída UTF-8 no Console
- `main.rs` chama `SetConsoleOutputCP(65001)` na inicialização — UTF-8 está ativo antes de qualquer saída ser escrita
- Windows Terminal e PowerShell 7 exibem caracteres acentuados e glifos CJK sem distorção
- O `cmd.exe` legado se beneficia da mesma troca automática de página de código — sem necessidade de `chcp 65001` manual
- Nenhuma ação do usuário é necessária — a codificação correta é definida programaticamente em cada invocação
### Uso no PowerShell
- Sintaxe de pipeline padrão funciona sem modificação: `duckduckgo-search-cli "rust async" | Select-String "tokio"`
- Saída JSON integra nativamente: `duckduckgo-search-cli -f json "query" | ConvertFrom-Json`
- Exit codes aparecem em `$LASTEXITCODE` — ramifique com `if ($LASTEXITCODE -ne 0)`
- Use `--output result.json` para saída em arquivo ao fazer pipe entre processos no PowerShell
### v0.6.5 — Correção de Cast HANDLE no Windows (MP-26)
- v0.6.4 era impossível de compilar no Windows. `windows-sys 0.59+`
  mudou o tipo `HANDLE` de `isize` para `*mut c_void`, mas o código de
  inicialização de plataforma em `src/platform.rs` usava casts `handle as isize`.
  `cargo install` no Windows falhava com 4 erros E0308.
- v0.6.5 corrige isto usando `!handle.is_null() && handle != INVALID_HANDLE_VALUE`
  e passando o `HANDLE` diretamente para `GetConsoleMode` e `SetConsoleMode`
  (cuja assinatura moderna aceita `HANDLE` por valor, não `isize`).
- Valide Windows localmente, não remotamente: este projeto não tem
  pipeline remoto, então o gate é o alias de cargo do próprio repositório.
  Rode `cargo check-windows` e `cargo check-windows-msvc` antes de publicar
  para que uma regressão de compilação no Windows apareça na sua máquina.


## Docker e Containers
### Imagem Alpine Mínima
- Use o binário do target musl para o menor footprint possível de imagem
- A imagem base Alpine adiciona aproximadamente 7 MB — a imagem combinada fica abaixo de 12 MB
- Nenhum passo `apk add` é necessário em runtime — cada dependência está compilada no binário
- Proxy, idioma e timeout são configurados por flag de CLI, NUNCA por variável de ambiente
- Passe `--proxy`, `--lang` e `--timeout` na própria invocação dentro do container
- Persista o proxy no armazenamento XDG com `config set proxy_url` quando o valor for fixo
### Exemplo de Dockerfile

```dockerfile
FROM rust:1.88-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3.19
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/duckduckgo-search-cli /usr/local/bin/
ENTRYPOINT ["duckduckgo-search-cli"]
```

- O estágio de build produz um binário totalmente estático usando o toolchain musl
- O estágio final copia apenas o binário — nenhum toolchain Rust é incluído na imagem de runtime
- Substitua `alpine:3.19` por `scratch` para produzir um container absolutamente mínimo
- Monte um volume gravável se você usa `--output` para persistir resultados fora do container


## Compatibilidade de Shell
### Bash e Zsh
- Faça pipe da saída diretamente para `jaq`, `rg` ou qualquer ferramenta compatível com POSIX sem escaping
- Verificação de exit code: `duckduckgo-search-cli -f json "query" && echo OK || echo "Exit: $?"`
- Expansão de chaves e divisão de palavras se comportam normalmente — coloque consultas multi-palavra entre aspas duplas
- Funções e aliases de shell se compõem facilmente porque o binário escreve em stdout e não lê nada do stdin
### Fish
- O shell Fish trata o binário de forma idêntica a qualquer outro comando externo
- Variável de status após o comando: `if test $status -eq 0`
- Strings de consulta com espaços requerem aspas duplas: `duckduckgo-search-cli "consulta com espaços"`
- Use blocos `begin ... end` para capturar exit codes em pipelines do Fish
### PowerShell
- Os resultados fazem pipe para `ConvertFrom-Json` para acesso nativo a objetos em scripts PowerShell
- A flag `-q` suprime o tracing para stderr — stdout limpo para parsing com `ConvertFrom-Json`
- Saída em arquivo: `duckduckgo-search-cli -f json -o result.json "query"; if ($?) { Get-Content result.json }`
- Funciona de forma idêntica no PowerShell 5.1 e PowerShell 7 no Windows e no macOS
### Nushell
- O pipeline estruturado do Nushell aceita saída JSON nativamente via `from json`
- Exemplo: `duckduckgo-search-cli -f json "query" | from json | get results`
- O binário escreve resultados em stdout e diagnósticos em stderr — o Nushell respeita essa separação
- Verificação de exit code: `if ($env.LAST_EXIT_CODE != 0) { error make {msg: "busca falhou"} }`


## Tamanho do Binário e Tempo de Inicialização
- `x86_64-unknown-linux-gnu`: aproximadamente 3,8 MB de binário de release com strip
- `x86_64-unknown-linux-musl`: aproximadamente 4,2 MB de binário de release estático
- `aarch64-apple-darwin`: aproximadamente 3,5 MB de binário de release com strip
- `x86_64-apple-darwin`: aproximadamente 3,8 MB de binário de release com strip
- `x86_64-pc-windows-msvc`: aproximadamente 4,0 MB de binário de release com strip
- Tempo de inicialização em todos os targets: abaixo de 100 milissegundos medidos a partir de cold start
- Sem fase de compilação JIT — o Rust compila para código de máquina nativo em tempo de build
- Footprint de memória por requisição de busca: abaixo de 20 MB de resident set size em uso típico


## Compilando a Partir do Código-Fonte
### Pré-requisitos
- Toolchain Rust versão 1.88 ou superior — instale via `rustup` em rustup.rs
- Para targets musl no Linux: `sudo apt install musl-tools` ou `apk add musl-dev` no Alpine
- v0.8.6+: nenhuma dependência de build adicional além do toolchain Rust em qualquer plataforma. TLS é puro Rust via `reqwest` + `rustls`. macOS ainda precisa de `xcode-select --install` para o linker
- v0.7.3–v0.8.5 apenas (BoringSSL): exigia `cmake`, `perl`, `pkg-config`, `libclang-dev` no Linux; Visual Studio Build Tools 2019+ com NASM, CMake, Strawberry Perl no Windows. Ver `scripts/install-windows.ps1` e `docs/INSTALL-WINDOWS.pt-BR.md` para instruções históricas de setup
- Compilação cruzada: `rustup target add <target>` antes de executar `cargo build`
- Para o binário Universal macOS: adicione os targets `aarch64-apple-darwin` e `x86_64-apple-darwin`
### Comandos de Build por Target

```bash
# Linux glibc (padrão em hosts Linux)
cargo build --release

# Linux musl — binário totalmente estático
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl

# macOS Apple Silicon
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# macOS Intel
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# Binário Universal macOS (une as duas fatias macOS)
lipo -create -output duckduckgo-search-cli-universal \
  target/aarch64-apple-darwin/release/duckduckgo-search-cli \
  target/x86_64-apple-darwin/release/duckduckgo-search-cli

# Windows MSVC — execute no Windows com o toolchain MSVC instalado
cargo build --release --target x86_64-pc-windows-msvc
```


## Estado do registry — leia antes de instalar
- A v1.0.6 é a primeira release com o defeito de cfg-stripping fechado (GAP-REL-001)
- MEDIDO em 2026-08-21: o crates.io serve `1.0.2` como `max_stable_version`, e a `1.0.2` NÃO compila em macOS nem Windows
- Enquanto a `1.0.6` não estiver publicada, `cargo install ... --version 1.0.6` falha na hora com `could not find duckduckgo-search-cli with version 1.0.6`
- Essa falha rápida e nomeada é DELIBERADA: instalar sem fixar versão resolve em silêncio para a `1.0.2` quebrada e morre minutos depois com um `E0432` críptico
- Falha rápida que nomeia o que falta vence falha lenta que não nomeia
- Confira você mesmo o registry antes de reportar bug de instalação: `cargo run --bin verify_published --features release-gate`
- Esse gate sai com 0 somente quando o registry serve de fato a versão que esta árvore carrega


## Instalação
### cargo install (todas as plataformas)
- Instalação padrão com um único comando em todas as plataformas suportadas:

```bash
cargo install duckduckgo-search-cli --locked --version 1.0.6
```

- SEMPRE fixe a versão, porque um `cargo install duckduckgo-search-cli` puro pode resolver para um build que NÃO compila no Windows nem no macOS
- As versões publicadas `1.0.2` e `1.0.1` NÃO compilam no Windows nem no macOS, falhando com `E0432` de import não resolvido (GAP-REL-001)
- Você DEVE instalar a `1.0.6` ou superior, que é a primeira versão publicada com essa classe de defeito fechada
- O Cargo busca a crate do crates.io, compila para a arquitetura do host e coloca o binário em `~/.cargo/bin`
- A Versão Mínima Suportada do Rust (MSRV) é 1.88 — execute `rustup update` se seu toolchain for mais antigo
- v0.8.6+: nenhuma dependência adicional de sistema em qualquer plataforma — TLS é puro Rust via `reqwest` + `rustls`
- v0.7.3–v0.8.5 apenas: adicionalmente requeria `cmake`, `perl`, `pkg-config` e `libclang-dev` no Linux para a stack BoringSSL
- Verifique a instalação: `duckduckgo-search-cli --version`
### Verificação pós-instalação
- O binário expõe exatamente dez comandos de topo: `buscar`, `init-config`, `completions`, `deep-research`, `commands`, `schema`, `doctor`, `locale`, `man`, `config`
- O subcomando `config` expõe exatamente seis operações: `path`, `list`, `get`, `set`, `unset`, `effective`
- O subcomando de listagem é `config list`, e `config list-keys` NÃO existe neste binário
- Rode `doctor` primeiro, porque ele é a única checagem que sonda o Chrome de que a produção realmente depende

```bash
duckduckgo-search-cli doctor
duckduckgo-search-cli commands
duckduckgo-search-cli schema
duckduckgo-search-cli locale
duckduckgo-search-cli man
duckduckgo-search-cli init-config
duckduckgo-search-cli completions bash
duckduckgo-search-cli config path
duckduckgo-search-cli config list
duckduckgo-search-cli config effective
```

- `commands` imprime o inventário de comandos legível por máquina, então prova a superfície instalada sem adivinhação
- `schema` imprime o catálogo de JSON Schema, e `--name` restringe a um schema único
- `locale` imprime o locale de UI resolvido, e `man` imprime a man page em roff gerada da mesma árvore clap do `--help`
- `init-config` grava `selectors.toml` e `user-agents.toml` no diretório XDG, e `config path` diz onde eles ficaram
- `config unset <chave>` remove uma chave persistida quando você quer voltar ao padrão
### Binários Pré-compilados
- Binários pré-compilados para todos os cinco targets são anexados aos GitHub Releases quando o mantenedor os publica manualmente (`cargo install` sempre compila do source)
- Cada release inclui um arquivo `SHA256SUMS.txt` para verificação de integridade antes da execução
- Baixe, verifique e instale no Linux ou macOS:

```bash
# Substitua X.Y.Z pela versão de release desejada
curl -LO https://github.com/danilo-aguiar-br/duckduckgo-search-cli/releases/download/vX.Y.Z/duckduckgo-search-cli-x86_64-unknown-linux-musl.tar.gz
sha256sum --check SHA256SUMS.txt
tar -xzf duckduckgo-search-cli-x86_64-unknown-linux-musl.tar.gz
chmod +x duckduckgo-search-cli
sudo mv duckduckgo-search-cli /usr/local/bin/
duckduckgo-search-cli --version
```

- Reporte problemas específicos de plataforma no rastreador de issues do repositório no GitHub


## v0.7.6 — Correção do `cargo install` (GAP-WS-48)

A v0.7.5 era impossível de compilar via `cargo install` em máquinas
limpas. Em 2026-06-14, `cargo install duckduckgo-search-cli` falhava
com 36 erros do tipo `E0277 the trait bound 'StandardAlloc: alloc::Allocator<T> is not satisfied`
porque o solver puxava `alloc-no-stdlib 3.0.0` (transitivamente de
`brotli-decompressor 5.0.2`) que colide com a expectativa do
`brotli 8.0.3` de `alloc-no-stdlib = "2.0"`.

A v0.7.6 corrige isto removendo a dep não utilizada `wreq-util` e
abandonando a feature `brotli` do `wreq` (DuckDuckGo nunca serve
`Content-Encoding: br`). O grafo de dependências volta ao estado limpo
e `cargo install` sucede em ~35,7s.

GAP-WS-48 residual — NÃO totalmente fechado sem `--locked`: mesmo
com o fix da v0.7.6, `cargo install` sem `--locked` ainda pode quebrar
em 2026-06-14+ porque o solver pode escolher a `alloc-stdlib 0.2.3`
publicada recentemente (que depende de `alloc-no-stdlib >=2.0.4, <4`) e
regenerar o lockfile com a versão conflitante 3.0.0. A receita robusta é:

```bash
# Sempre use --locked para respeitar o Cargo.lock commitado
cargo install duckduckgo-search-cli --locked

# Fixe a versão E o lock
cargo install duckduckgo-search-cli --version 0.7.7 --locked
```

A v0.7.7 commita um `Cargo.lock` preparado com
`cargo update -p alloc-no-stdlib@3.0.0 --precise 2.0.4` para que
`--locked` rejeite a resolução ruim. Sem `--locked`, o solver é livre
para reintroduzir o conflito.


## v0.7.7 — Correção do Fingerprint TLS (GAP-WS-49)

A v0.7.6 publicou um binário que passava em todos os smoke tests mas
retornava ZERO resultados reais. O `wreq 6.0.0-rc.29` sozinho NÃO
inclui a feature `emulation`; a emulação de fingerprint JA3/JA4 vivia
no `wreq-util 3.0.0-rc.12` via `default = ["emulation"]`. A v0.7.6
havia removido o `wreq-util` para fechar o GAP-WS-48 de `cargo install`,
e o handshake BoringSSL-sem-emulação tornou-se trivialmente detectável
pelo Cloudflare Bot Management. A DDG servia `anomaly-modal` (45
ocorrências no HTML body) para cada query real.

A v0.7.7 corrige isto re-adicionando `wreq-util 3.0.0-rc.12` com
`default-features = false, features = ["emulation"]` e três pins diretos
no `Cargo.toml`:

- `brotli-decompressor = "=5.0.1"` (a 5.0.2 publicada em 2026-06-14
  alarga o range de `alloc-no-stdlib` e puxa 3.0.0)
- `alloc-no-stdlib = "=2.0.4"` (5.0.1+ exige esta versão exata)
- Feature `"brotli"` do `wreq` re-habilitada (mandatória para `emulation`)

O resultado: queries reais voltam a retornar 5+ resultados com fingerprint
TLS JA3/JA4 idêntico ao Chrome/Safari, correspondendo à sonda de
navegador que a DDG espera. `cargo build --release --offline` sucede em
24,04s (mais rápido que v0.7.6 porque `brotli-decompressor 5.0.1` é
menor que 5.0.2).

Ressalva para `cargo install`: use `--locked` (veja nota do GAP-WS-48
residual acima). Sem `--locked`, o solver pode puxar
`alloc-stdlib 0.2.3` e o conflito retorna.


## v0.7.8 — Renovação do Detector Anti-Bot + Verbose Acumulado (8 gaps)

A v0.7.8 (working tree, aguardando tag) fecha 8 gaps na superfície
de detecção anti-bot. Ver `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`
para a decisão arquitetural completa. Mudanças principais:

- `detectar_interstitial` expandido (GAP-WS-50): `CLOUDFLARE_MARKERS`
  cresceu para 8 entradas (`anomaly-modal`, `anomaly-modal__mask`,
  `anomaly-modal__title`, `anomaly.js?cc=botnet`, `cf-turnstile`,
  `cf-spinner`, `Just a moment`, `cf-mitigated`) mais 1 marcador DDG
  novo (`Unfortunately, bots use DuckDuckGo too.`). O detector agora
  captura o interstitial `anomaly-modal` pós-2026 que a v0.7.7 perdeu.
- Probe-deep usa query de calibração longa (GAP-WS-51): o literal
  `q=rust` (4 chars) foi substituído pelo pangrama de 9 palavras
  `the quick brown fox jumps over the lazy dog` exposto como
  `PROBE_CALIBRATION_QUERY` em `src/lib.rs:91, 509`. Queries longas
  acionam o bot scoring upstream de forma confiável, tornando o probe
  honesto.
- `--allow-lite-fallback` agora consulta o detector (GAP-WS-52): o
  predicado em `src/search.rs:559` migrou de
  `accumulated_results.is_empty()` para
  `detectar_interstitial(&first_html) != InterstitialKind::None`. Quando
  a flag está OFF e o detector ainda flagra interstitial, um
  `tracing::warn!` estruturado é emitido com
  `kind = interstitial_kind.as_str()`.
- Verbose agora é cumulativo (GAP-WS-53 / v1.0.1): sem flag → `info` (ou XDG
  `log_directive`); `-v` → `debug`; `-vv`+ → `trace`; `-q` → `off`. Filtro de
  produto é CLI `-v`/`-q` + XDG `log_directive` apenas (não `RUST_LOG`).
- `scraper` bumpado para 0.27 (GAP-WS-54): fecha RUSTSEC-2025-0057
  (`fxhash 0.2.1` unmaintained). `cargo audit --deny warnings` agora é
  gate local em gates locais.
- Comentário do `wreq` reescrito (GAP-WS-55): o texto anterior
  alegava uma "regressão para 5.3.0" que nunca aconteceu. O novo
  comentário documenta o pin real em `wreq 6.0.0-rc.29` e os três pins
  diretos.
- Subcomando `buscar` escondido (GAP-WS-56): `#[command(hide = true)]`
  mantém invocável mas remove do `--help` para reduzir ruído.
- `--retries` agora é honrado (GAP-WS-57): o valor estava hard-coded
  para 1 em `src/parallel.rs:644`; corrigido para ler `cfg.retries` com
  clamp `[1, 10]` para que `--retries 999` não acione anti-bot.

Impacto multiplataforma: zero breaking changes. Schema JSON e exit
codes inalterados. Tamanho do binário inalterado. Delta de tempo de
build dentro de ±5% em todos os targets. O novo `scraper 0.27` pode
serializar `Selector` levemente diferente, mas nenhum call site
precisou de refactor.


## Matriz Comparativa v0.7.5 → v0.7.8

| Concern | v0.7.5 | v0.7.7 | v0.7.8 |
|---|---|---|---|
| `cargo install` no Linux | Quebrado (GAP-WS-48) | Funciona com `--locked` | Funciona com `--locked` |
| Queries reais retornam resultados | Sim | Sim (restaurado via fix TLS) | Sim (com markers melhores) |
| Detecta DDG `anomaly-modal` | Não | Não | Sim (8 markers novos) |
| Probe-deep sinal honesto | Query curta `rust` | Query curta `rust` | Pangrama 9-palavras |
| Fallback opt-in honrado | Predicado invertido | Predicado invertido | Guiado por detector |
| `-vv` flag de debug | Não suportado | Não suportado | Sim (`ArgAction::Count`) |
| `cargo audit` limpo | 1 advisory transitivo | 1 advisory transitivo | Limpo (RUSTSEC-2025-0057 fechado) |
| Subcomando `buscar` | Visível no `--help` | Visível no `--help` | Escondido |
| `--retries N` honrado | Não (hard-coded 1) | Não (hard-coded 1) | Sim (clamp `[1, 10]`) |


## GAP-WS-48 Residual — Quando o Sintoma Retorna

Se um usuário reportar `E0277 the trait bound 'StandardAlloc: alloc::Allocator<T> is not satisfied`
em `cargo install` da v0.7.7 ou v0.7.8, a causa é quase sempre uma
destas:

- Faltou `--locked`: o solver regenerou o lockfile e puxou
   `alloc-stdlib 0.2.3` → `alloc-no-stdlib 3.0.0`. Correção:
   `cargo install duckduckgo-search-cli --locked`.
- Misturando lock da v0.7.6 com source da v0.7.7: alguns usuários
   cachearam o lock da v0.7.6 e esqueceram de atualizar. Correção:
   `cargo update` ou remova `Cargo.lock` e reconstrua com `--locked`.
- Mirror de registry customizado: o mirror pode estar desatualizado
   e servir `brotli-decompressor 5.0.2` em vez de 5.0.1. Correção:
   configure o mirror para upstream crates.io, ou use um `Cargo.lock`
   mais recente.

A receita robusta para máquinas limpas é:

```bash
# Linux/macOS — versão explícita + lock travado
cargo install duckduckgo-search-cli --version 0.7.7 --locked

# Windows MSVC — o mesmo, mais developer shell para cl.exe
cargo install duckduckgo-search-cli --version 0.7.7 --locked
```

Verifique após a instalação:

```bash
duckduckgo-search-cli --version          # espere 0.7.7 (ou 0.7.8)
duckduckgo-search-cli -q -n 5 "rust async runtime"  # espere 5 resultados
```


## Requisitos do Chrome (v0.8.5+, obrigatório em produção desde v0.9.4)
- Linux: `sudo apt install google-chrome-stable xvfb` (Debian/Ubuntu)
- Linux: `sudo dnf install google-chrome-stable xorg-x11-server-Xvfb` (Fedora)
- Linux: Xvfb é auto-spawned pela CLI via `spawn_virtual_display()` (v0.8.5+) — sem necessidade de `xvfb-run` manual. v0.8.8 adiciona limpeza de lock files obsoletos — `is_lock_stale()` verifica o PID no lock X11 sob `temp_dir()/.X{N}-lock` via `/proc/{pid}` e remove locks de processos mortos (GAP-HARD-X11-001: sem `/tmp` hardcoded)
- Linux: se Xvfb não estiver instalado, Chrome cai para headless (com risco de detecção anti-bot)
- macOS: Instale o Chrome em https://www.google.com/chrome/ (Chrome roda headless=new desde v0.9.3; stealth coerente via correções v0.9.2)
- Windows: Instale o Chrome em https://www.google.com/chrome/ (Chrome roda headless=new desde v0.9.3; stealth coerente via correções v0.9.2)
- Chrome é auto-detectado via `detect_chrome()` em `src/browser.rs`
- Sem Chrome utilizável → exit 2 fail-closed (GAP-WS-113); env de produto `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` REMOVIDA / não lida
- Compilar sem Chrome (`cargo build --no-default-features`) não é viável em produção — operações de rede falham fechadas com exit 2; use apenas para testes offline/unitários


## v0.9.8 — Agent-ready multi-canal dual+clean (GAP-WS-AGENT-READY-001)
- Vertical padrão `--vertical all`; fetch de conteúdo LIGADO (web + news, teto 4 no padrão v1.0.2); opt-out `--vertical web` / `--no-fetch-content` / deep `--no-news`
- Resolução Chrome multi-canal (export Flatpak → ELF de deploy; wrappers do host → ELF)
- Flags de transporte globais, incluindo `--chrome-path` depois de `deep-research`
- Metadados agent (wire EN desde v1.0.2): `chrome_path_resolved`, `chrome_channel`, `used_chrome` honesto (não é telemetria; PT legado via `--wire-keys pt`)
- Design: [`docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md`](decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md)


## v1.0.2 — Wire EN padrão + agent ops + orçamento dual + mute
- Serialização wire em inglês por padrão ([ADR-0027](decisions/0027-wire-en-default-v1-0-2.md)); `--wire-keys en|pt` + XDG `wire_keys`
- Agent ops: `--fields`/`--select`, `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only`, `--truncate-content`, `--max-output-bytes`
- Orçamento dual/contenção fail-fast ([ADR-0024](decisions/0024-deep-research-budget-contract-v1-0-2.md)/[0025](decisions/0025-deep-research-budget-contention-aware-v1-0-2.md)): `--print-budget`, `--allow-under-budget`, `--auto-contention-budget`, `budget_profile`
- Chrome sempre mudo ([ADR-0026](decisions/0026-chrome-mute-audio-operational-standard-v1-0-2.md)); sem unmute
- Defaults: `max-sub-queries=3`, `fetch-content-cap=4` (FETCH_CAP), `DEFAULT_PAGES=1`
- Root `--print-schema` e root `--probe`, ambos flags de RAIZ e NÃO subcomandos
- `--probe-deep` também é flag de RAIZ, e o `doctor` a aceita como alias equivalente para que `doctor --probe-deep` seja parseado sem armadilha de ordem de flag
- Design: ADR-0024, ADR-0025, ADR-0026, ADR-0027


## v1.0.1 — Oneshot pipe-safe + config dual + aliases de stream (Pass 52)
- `ensure_oneshot_cleanup` em todas as saídas inclusive pipe cedo; SIGPIPE Unix SIG_IGN (não SIG_DFL) para Drop/reap do Chrome ainda rodar
- Stream BrokenPipe → exit 141; órfãos oneshot 0 após `| head`
- Dual `config get`/`set`/`unset` (posicional ou flags) + `config effective`
- Alias `-f ndjson` de `--stream` multi-query
- Wire serialize PT BC + aliases EN na desserialização (ADR-0023; serialize supersedido pelo ADR-0027 na v1.0.2)
- Anti-bot falso de news corrigido; residual real do DDG ambiental
- Config de produto só CLI+XDG; sem telemetria remota


## v1.0.0 — One-shot de disco + prefixo de perfil auditável (GAP-WS-TMP-PROFILE-ORPHAN-001)
- Completa o one-shot de processo (0.9.6) com honestidade de disco — gap RESOLVIDO na v1.0.0
- O `user-data-dir` do Chrome usa o prefixo `ddg-chrome-*` (Unix `0o700`), não `.tmp*` genérico
- `force_reap` remove o diretório do perfil após matar o processo (`remove_dir_all` + retry); `ExitReapGuard` + panic hook + reap em timeout/fim de run
- `sweep_orphan_profiles` na próxima run limpa SOMENTE `ddg-chrome-*` stale de propriedade
- Política dura: nunca bulk-delete de `.tmp*` estrangeiro nem `org.chromium.Chromium.*`
- Residual: SIGKILL/OOM podem deixar dirs sem destructor; a próxima invocação varre só `ddg-chrome-*`
- Sem telemetria; sem quebra de schema JSON
- Design: [`docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md`](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md) (ADR-0020); inventário: `gaps.md`


## v0.9.6 — Propriedade one-shot de processos (GAP-WS-LIFECYCLE-001)
- Cada invocação reap a árvore Chromium/Xvfb (`process_lifecycle`: process group, walk da árvore, marker de `user-data-dir`)
- Linux: `setpgid` + `PR_SET_PDEATHSIG` nos filhos Xvfb/Chrome; `XvfbGuard` limpa lock/socket
- Multiplataforma: shutdown de `ChromeBrowser` + force-reap no Drop e reap por marker/árvore aplicam-se em Linux, macOS e Windows; PDEATHSIG é só Linux
- SIGTERM cancela o `CancellationToken` cooperativo; prefira supervisores com SIGTERM primeiro (GNU `timeout`)
- Escrita atômica de output/config/cookies; sem telemetria; sem quebra de schema JSON
- Residual: SIGKILL não interceptável; órfãos de processo históricos pré-0.9.6 não são limpos automaticamente (honestidade de disco de perfil concluída na v1.0.0 / ADR-0020)
- Design: [`docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md`](decisions/0017-browser-lifecycle-one-shot-v0-9-6.md)


## v0.9.4 — Chrome-only universal (GAP-WS-113)
- Transporte de rede de produção é Chrome-only (`chromiumoxide`/CDP); feature `chrome` é o padrão
- Chrome ausente (ou build sem feature `chrome`) → exit 2 fail-closed (sem auto `--no-news`, sem sucesso HTTP web). Env de produto `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` REMOVIDA / não lida
- `--allow-lite-fallback` é no-op legado
- HTTP residual apenas sob `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`
- Builds com `--no-default-features` não são viáveis em produção para operações de rede


## v0.9.1 — v0.9.3 — Hardening de Stealth & Headless no macOS/Windows
- v0.9.1 (GAP-WS-107): macOS/Windows mudaram para headed nativo Quartz/DWM + coerção de plataforma no UA (`ua_platform_matches_host`)
- v0.9.2 (GAP-WS-108): chromiumoxide `--enable-automation` removido via `.disable_default_args()` + 23 defaults seguros re-adicionados
- v0.9.2 (GAP-WS-109): UA alinhado à versão real do Chrome via `detect_chrome_major_version()` + `Emulation.setUserAgentOverride` com `UserAgentMetadata` coerente
- v0.9.2 (GAP-WS-110/111): `--force-webrtc-ip-handling-policy=disable_non_proxied_udp`, `--disable-webrtc-hw-decoding`, `--disable-quic` adicionados ao flags_stealth
- v0.9.3 (GAP-WS-112): macOS/Windows mudaram para headless=new (`ChromeHeadMode::Headless`) — Quartz/DWM faziam clamp de `--window-position`; Linux mantém `HeadedXvfb`
- Flag CLI `--chrome-visible` permanece a saída de depuração que força `HeadedNative` (env de produto `DUCKDUCKGO_CHROME_VISIBLE` REMOVIDA)


## v0.8.0 — Chrome headed como transporte primário de busca
- A v0.8.0 tornou o Chrome headed (via `chromiumoxide`) o transporte de busca PRIMÁRIO
- A busca HTTP-only permaneceu como fallback residual naquele momento
- O classificador ZeroCause foi adicionado para evasão anti-bot
- A descompressão HTTP passou a ser integrada nativamente
- Supersedido na v0.9.4 (GAP-WS-113 / ADR-0016): a produção é Chrome-only, sem caminho de sucesso HTTP na SERP


## v0.8.6 — Migração de wreq/BoringSSL para reqwest/rustls
- A v0.8.6 substitui a stack TLS inteira
- A crate `wreq`, que linkava BoringSSL estaticamente, deu lugar ao `reqwest` com a feature `rustls-tls`
- O TLS passou a ser puro Rust, com zero dependências nativas de C
- `cmake`, `perl`, `pkg-config` e `libclang-dev` NÃO são mais dependências de build no Linux
- Assembler NASM, CMake e Strawberry Perl NÃO são mais dependências de build no Windows
- `cargo install duckduckgo-search-cli --locked --version 1.0.6` funciona no Windows APENAS com o toolchain Rust
- A colisão GAP-WS-48 de `alloc-no-stdlib 2.0.4` contra `3.0.0` foi eliminada
- O tempo de build ficou menor, porque a compilação C do BoringSSL sumiu
- O binário ficou menor, porque não há mais BoringSSL linkado estaticamente
- Desde a v0.8.0 o Chrome headed é o transporte de busca, e desde a v0.9.4 é o ÚNICO transporte de rede de produção (ADR-0016)
- O HTTP residual é só de teste, sob `http-test-harness`
- A stack TLS do cliente HTTP importa menos para anti-bot, porque o Chrome trata o TLS da SERP via CDP

Leia este documento em [English](CROSS_PLATFORM.md).
