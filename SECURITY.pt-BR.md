# Política de Segurança


## Versões com Suporte

- Somente a versão minor mais recente e a anterior recebem atualizações de segurança
- Versão **0.9.8** é a versão atual (GAP-WS-AGENT-READY-001 defaults agent-ready + Chrome multi-canal; inclui lifecycle 0.9.6 + correção HANDLE Windows MSVC 0.9.7)
- Linhas 0.9.x / 0.8.x mais antigas aparecem por contexto histórico; prefira atualizar para 0.9.8+
- Campos de metadados agent `chrome_path_resolvido` e `chrome_canal` são contrato JSON local para integradores — **não** são telemetria remota
- Fetch de conteúdo está **LIGADO por padrão** desde a v0.9.8 (opt-out `--no-fetch-content`); HTML das páginas buscadas continua sendo entrada não confiável parseada localmente

| Versão | Suportada |
|---|---|
| 0.9.8 | **Sim (atual; GAP-WS-AGENT-READY-001 dual vertical + fetch default ON + Flatpak multi-canal; ADR-0018)** |
| 0.9.7 | Sim (lifecycle 0.9.6 + null check de HANDLE no Windows MSVC) |
| 0.9.6 | Sim (lifecycle GAP-WS-LIFECYCLE-001; **não compila no Windows MSVC** — use 0.9.7+) |
| 0.9.5 | Sim (anterior; GAP-WS-113 + fix CI/release) |
| 0.9.4 | Sim (GAP-WS-113 Chrome-only fail-closed, sem auto-degradação, fallback Lite no-op) |
| 0.9.3 | Sim (anterior; GAP-WS-112 macOS/Windows headless=new) |
| 0.9.2 | Sim (GAP-WS-108/109/110/111 endurecimento stealth chromiumoxide) |
| 0.9.1 | Sim (GAP-WS-107 macOS/Windows headed nativo) |
| 0.9.0 | Sim (GAP-WS-106 flags globais; auto-degradação **supersedida pela 0.9.4**) |
| 0.8.9 | Sim (GAP-WS-104 vertical de notícias exclusiva do Chrome, ZeroCause `vertical-sem-resultados`, correções pós-revisão F1-F7) |
| 0.8.8 | Sim (`has_native_display()`, auto-install Xvfb 22+ distros, 17 sinais stealth, navegação warm-up, GAP-WS-060 até GAP-WS-103 fechados) |
| 0.8.0 | Sim (transporte Chrome-primary, classificação causal de zero-result, descompressão HTTP) |
| 0.7.10 | Sim (scheduler pre-flight, propagação de pino de identidade) |
| 0.7.8 | Sim (8 gaps do detector anti-bot fechados) |
| 0.7.7 | Sim (GAP-WS-49 corrigido regressão de fingerprint TLS) |
| 0.7.3 | Parcial (fix de stack TLS — rustls substituído por BoringSSL) |
| < 0.7.3 | Não |


## Reportando uma Vulnerabilidade

- NÃO abra uma issue pública no GitHub para vulnerabilidades de segurança.
- Reporte de forma privada via GitHub Security Advisories:
- Acesse `https://github.com/daniloaguiarbr/duckduckgo-search-cli/security/advisories/new`
- Preencha o formulário de advisory com:
- Uma descrição clara do problema
- Passos para reprodução (exemplo mínimo preferido)
- As versões afetadas
- Qualquer mitigação que você identificou
- Você deve receber uma resposta inicial dentro de 72 horas
- Um cronograma de divulgação coordenada será acordado antes de qualquer anúncio público


## Escopo

- Vulnerabilidades de interesse incluem, mas não se limitam a:
- Falhas na construção de requisições HTTP que possam habilitar SSRF, injeção de cabeçalho ou request smuggling contra o DuckDuckGo ou URLs buscadas
- Fraquezas no parsing de HTML no pipeline de extração que possam ser disparadas por uma resposta de servidor hostil (ex: DoS via DOM manipulado, XXE apesar do contexto HTML, seletores CPU-bomb)
- Vazamento de credenciais através do tratamento de `--proxy user:pass@...` em logs, mensagens de erro ou no JSON de saída (o mascaramento deve prevenir isso — reporte qualquer vazamento)
- **v0.7.3+**: Manipulação do cookie jar — o arquivo `cookies.json` contém cookies de sessão do DuckDuckGo e é gravado com permissões Unix 0o600. Reporte qualquer forma de ler este arquivo como outro usuário local, ou qualquer forma do CLI enviar esses cookies para uma origem que não seja DuckDuckGo.
- Ataques de path traversal ou symlink contra o caminho do arquivo de saída (`-o, --output`) ou o diretório de config XDG
- Configuracao incorreta de TLS que possa habilitar MITM — desde a v0.8.6 o projeto usa `reqwest` + `rustls-tls` (TLS Rust puro, substituindo BoringSSL/wreq da v0.7.3-v0.8.5). Reporte qualquer fallback para cipher suites inseguras
- Problemas de supply chain em dependências transitivas fixadas ainda não documentadas em `deny.toml`


## Fora do Escopo

- Negação de serviço causada pelo usuário passando flags patológicas (`--parallel 20 --pages 5 --fetch-content` em milhares de queries é esperado consumir recursos significativos)
- Vulnerabilidades no próprio DuckDuckGo — reporte-as ao DuckDuckGo
- Vulnerabilidades no Chrome/Chromium usados com `--features chrome` — reporte-as ao projeto Chromium
- Problemas que exigem uma conta de usuário local comprometida ou acesso de escrita ao `$XDG_CONFIG_HOME`
- Processos órfãos residuais de Chromium/Xvfb de execuções anteriores à v0.9.6, ou após um **SIGKILL** externo da própria CLI, são limites operacionais de higiene do host (o SO não entrega handlers em SIGKILL) — não são CVE, a menos que habilitem escalonamento de privilégio ou acesso cross-user


## Premissas de Design de Segurança

- A CLI é um cliente HTTP read-only — não escreve em sistemas remotos
- Todos os inputs externos (strings de query, paths de saída) são validados antes do uso
- **v0.7.3+**: Cookie jar persistido em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). O arquivo é gravado com permissões Unix `0o600` (owner read+write only). No Windows, o diretório herda a ACL do perfil do usuário. Os cookies são cookies de sessão emitidos por `duckduckgo.com` e `html.duckduckgo.com`. **Trate este arquivo como trataria qualquer credencial.** Use `--no-cookie-persistence` para manter cookies em memória apenas. Use `--cookies-path <PATH>` para realocar o arquivo para um volume encriptado.
- **v0.8.6+**: TLS via `rustls` (Rust puro, estaticamente vinculado pelo `reqwest`). v0.7.3-v0.8.5 usava BoringSSL via `wreq`; v0.8.6 substituiu por `reqwest` + `rustls-tls` (ADR-0008). Sem dependencia de OpenSSL/SChannel/SecureTransport do sistema
- Desde a v0.8.0 a CLI executa JavaScript via Chrome na fase de busca — o processo Chrome é isolado e roda dentro de display virtual Xvfb privado (v0.8.5+)
- Quando `--fetch-content` está ativo, páginas buscadas são parseadas com `scraper` (que usa `html5ever`); HTML não confiável é esperado
- **v0.9.8+**: o fetch de conteúdo é **LIGADO por padrão** para web + news (FETCH_CAP=10); opt-out com `--no-fetch-content`. Isso aumenta a superfície de parse HTML — ainda é o design esperado; páginas hostis continuam no escopo de relatórios de DoS de parsing
- **v0.9.8+ metadados de agente NÃO são telemetria**: `chrome_path_resolvido`, `chrome_canal` e `usou_chrome` honesto são apenas campos do contrato JSON local; sem exportação remota
- **v0.7.3+**: A CLI não é mais totalmente sem estado. O cookie jar persistente adiciona estado entre invocações. É um trade-off deliberado para reduzir a taxa de CAPTCHA no servidor do DuckDuckGo. O request de warm-up (`GET https://duckduckgo.com/`) é idempotente e não persiste nenhum dado identificador de usuário além dos próprios cookies.
- Arquivos de saída são criados com permissão `0o644` no Unix (proprietário escreve, mundo lê)
- Nada é escrito fora do caminho que o usuário passou


## Automação de Supply Chain Relacionada

- O projeto executa, em todo push e pull request:
- `cargo audit` contra o banco de dados de advisories do RustSec
- `cargo deny check advisories licenses bans sources` com a política declarada em `deny.toml`
- `dependabot` (semanal) abre PRs para atualizações de dependências `cargo` e `github-actions`
- Veja `.github/workflows/ci.yml` e `.github/dependabot.yml` para detalhes
## Melhorias de Segurança v0.6.5

- **MP-26 (segurança de tipo de HANDLE)**: `src/platform.rs:51-69` usa `is_null()` e
  `INVALID_HANDLE_VALUE` em vez de `handle != 0` e `handle as isize`. A
  API Win32 agora recebe um `HANDLE` tipado corretamente (`*mut c_void`) conforme
  a ABI do `windows-sys 0.59+`. Elimina UB latente em v0.6.4.
- **CI-01 (lints do clippy)**: `improper_ctypes` e `improper_ctypes_definitions`
  agora são `deny` em `Cargo.toml`, prevenindo drift futuro de tipos FFI. Implementações
  de `Debug` ausentes e regressões de `clippy::needless_return` são agora capturadas
  em `cargo clippy --all-targets --all-features -- -D warnings`.
- **Lints promovidos para deny**: `missing_safety_doc` e `unsafe_op_in_unsafe_fn`
  previnem superfície de API `unsafe` sub-especificada.

Para vulnerabilidades específicas em v0.6.4, o issue de cast de HANDLE Windows
foi o mais proeminente: uma falha de build no Windows que podia ser disparada
por `cargo install duckduckgo-search-cli`. v0.6.5 entrega a correção type-safe.

## Melhorias de Segurança v0.7.3

> **Nota (v0.8.6)**: A stack BoringSSL/wreq descrita abaixo foi substituida por `reqwest` + `rustls-tls` na v0.8.6 (ADR-0008). Esta secao e historica.

- **GAP-WS-27 (fingerprint TLS)**: O interstitial de CAPTCHA do Cloudflare Bot
  Management que afetava usuarios macOS em v0.7.2 (HTTP 200 com
  `quantidade_resultados: 0`) esta corrigido. A stack TLS mudou de `rustls`
  para BoringSSL (estaticamente vinculado por `wreq 6.0.0-rc.29`).
- **BoringSSL pinado via `wreq 6.0.0-rc`**: BoringSSL e a mesma biblioteca TLS
  que Chrome e Android usam em producao. CVEs contra BoringSSL
  sao rastreadas pelo Chromium e abordadas em commits upstream que
  `wreq` consome em cada release.
- **Endurecimento do cookie jar (0o600)**: O arquivo `cookies.json` escrito pela
  feature `session` em v0.7.3+ é criado com permissões Unix `0o600`
  (owner read+write only). No Windows, o arquivo herda a ACL do diretório
  de perfil do usuário.
- **Localização do cookie jar é XDG-aware**: Linux segue `XDG_CONFIG_HOME`
  (default `~/.config`). Windows usa `%APPDATA%`. macOS usa
  `~/Library/Application Support`. O path é sobrescritível via
  `--cookies-path <PATH>` para apontar para um volume encriptado.
- **Supply chain em build-time**: Compilar do source agora requer
  `cmake`, `perl`, `pkg-config` e `libclang-dev` no Linux. Esses são
  componentes de toolchain C que compilam a biblioteca estática BoringSSL.
  **`cargo install` sempre compila do source** — crates.io não distribui
  binários pre-built para nenhuma plataforma. Cada usuário Windows deve
  satisfazer os quatro pré-requisitos de build BoringSSL (NASM, CMake, MSVC, Perl)
  por conta própria. Veja `gaps.md` GAP-WS-28/29/30/31 e `docs/INSTALL-WINDOWS.md`
  para a lista completa de pré-requisitos e setup passo-a-passo.
- **MSRV inalterado desde v0.7.2**: `rust-version = "1.88"`.

## Melhorias de Segurança v0.7.9

- **GAP-WS-58 (CRÍTICO, ghost-block)**: `detectar_interstitial` agora classifica
  body sub-4KB sem `result-page-signal` como `InterstitialKind::Cloudflare`. Threshold
  conservador evita falsos positivos em responses válidos de baixa densidade.
  Antes da fix, ghost-block puro (HTML vazio do Cloudflare) passava despercebido
  e a CLI retornava exit 0 com `quantidade_resultados: 0`, mascarando o bloqueio.
- **GAP-WS-59 (ALTO, markers 2026)**: 5 marcadores Cloudflare novos
  (`anomaly.js`, `botnet`, `cf-error-code`, `cf-ray`, `Performance & Security by Cloudflare`)
  + 1 marker DDG novo (`Unfortunately, bots` parcial). Detector cobre variantes
  2026 que passavam despercebidas.
- **GAP-WS-59 (ALTO, flag global)**: `--allow-lite-fallback` e `--pre-flight` hoisted
  para `RootArgs` com `global = true`. Fechou o caminho `unexpected argument` em
  subcomandos como `deep-research` que poderia expor attack surface em CI scripts.
- **GAP-WS-106 (ALTO, ergonomia da CLI; histórico v0.9.0–v0.9.3)**: nove flags hoisted para `global = true`. Nessas releases, `deep-research` e `--vertical news|all` auto-degradavam com warning no stderr em vez de abortar com exit 2 quando o Chrome estava indisponível. **Supersedido por GAP-WS-113 / v0.9.4**: produção é Chrome-only fail-closed (exit 2) — sem auto `--no-news`, sem rebaixamento para Web.
- **Config.pre_flight**: adicionado com default `false` (opt-in). Sem mudança
  comportamental para usuários existentes.

## Melhorias de Segurança v0.9.8

- **GAP-WS-AGENT-READY-001 (ALTO, defaults agent-ready, ADR-0018)**: vertical dual e fetch de conteúdo LIGADOS por padrão aumentam a superfície local de parse HTML (ainda é o design esperado). Metadados de agente (`chrome_path_resolvido`, `chrome_canal`, `usou_chrome` honesto) **não** são telemetria e não são exportados remotamente.
- **Resolve multi-canal Chrome**: shells de export Flatpak não são executados como browser; a CLI resolve um ELF real sob `files/extra/chrome` (e similares). Prefira `--chrome-path` quando o operador quiser um binário explícito.
- **Flags de transporte `global = true`**: `--chrome-path` após `deep-research` deixa de falhar o parse do clap (exit 2) — flags aceitas antes ou depois do subcomando.
- **Sem telemetria remota**: one-shot, atomwrite e metadados de agente permanecem só locais.

## Melhorias de Segurança v0.9.6

- **GAP-WS-LIFECYCLE-001 (ALTO, ownership one-shot de Chromium/Xvfb, ADR-0017)**: a CLI é NASCE → EXECUTA → MORRE. `src/process_lifecycle.rs` é dono da árvore completa de processos (process group via `setpgid`, `PR_SET_PDEATHSIG` no Linux, `killpg`, walk da árvore, kill por marker de `user-data-dir`, limpeza de lock/socket do Xvfb, session registry + panic hook). `ChromeBrowser` usa `XvfbGuard`, shutdown assíncrono cooperativo com deadline de close/wait e `force_reap_session` no `Drop`. `content_fetch` assume ownership e executa shutdown assíncrono. Uma invocação normal ou cancelada de forma cooperativa não deve deixar Chromium/Xvfb órfãos **desta** execução.
- **Escritas atômicas (`paths::atomic_write`)**: `--output`, `init-config` e o cookie jar gravam via tempfile + fsync + rename, reduzindo arquivos de config, cookies ou saída parciais/corrompidos em crash no meio da escrita.
- **Cancelamento cooperativo de SIGTERM + SIGINT**: ambos os sinais cancelam o `CancellationToken` compartilhado para que os caminhos de shutdown rodem em vez de abandonar a árvore do browser.
- **Limite residual (documentado, não é vulnerabilidade)**: **SIGKILL** do próprio processo da CLI não é interceptável no nível do SO; órfãos históricos de execuções **anteriores à v0.9.6** não são limpos por um upgrade posterior. Operadores podem precisar de uma limpeza única do host após atualizar a partir de versões mais antigas.
- **Sem telemetria remota**: caminhos de lifecycle/reap emitem apenas `tracing` local; nada é exportado.

## Melhorias de Segurança v0.9.4

- **GAP-WS-113 (CRÍTICO, transporte Chrome-only universal, ADR-0016)**: o caminho de rede em produção é exclusivamente chromiumoxide/CDP. Chrome ausente ou `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` **falha com exit 2** em qualquer operação de rede — sem sucesso HTTP silencioso, sem auto-degradação Web/`--no-news`. Remove canal dual-transport que podia apresentar resultados vazios como zeros legítimos sob anti-bot.
- **`--allow-lite-fallback` no-op legado**: Lite nunca é caminho de sucesso em produção; a flag permanece só por BC de scripts e não força degradação de endpoint.
- **HTTP residual** apenas sob a feature de compilação `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` (testes).

## Melhorias de Segurança v0.7.10

- **GAP-WS-60 (CRÍTICO, propagação de pino de identidade)**: `--identity-profile` agora
  propaga o pino de identidade para TODOS os caminhos de output, incluindo
  `failure_output` (pipeline.rs) e `error_output` (parallel.rs). Antes da fix,
  o pino (`identidade_usada`) só aparecia no caminho de SUCESSO; em falha,
  era sempre `null`. Consumers agora podem correlacionar falhas a identidades
  específicas do pool de 12 para fins de auditoria e incident response.
  Helper novo: `identity_tag_for_cli_identity` em `src/identity.rs`.
- **Fix B4 (CRÍTICO, honestidade de exit code)**: `--probe-deep` standalone agora
  retorna exit 3 quando detecta captcha. Antes retornava exit 0 com
  `status: "captcha"` no JSON, permitindo bypass via `if [ $? -eq 0 ]`
  em shell scripts. Agora branching no exit code é confiável.
- **Fix B1 (CRÍTICO, integridade de stream JSON)**: `--pre-flight` emitia dois
  objetos JSON concatenados no stdout via `print_line_stdout` early-return.
  Consumers com `| jaq '.resultados'` quebravam. Removido early print;
  `SearchOutput` carrega o contexto do pre-flight e o caller serializa
  exatamente uma vez.
- **Fix B2 (CRÍTICO, honestidade de exit code)**: `pre_flight_blocked` agora retorna
  exit 3 (RATE_LIMITED_OR_BLOCKED) em vez de exit 0 (SUCCESS). Tabela
  `EXIT CODES` do `--help` prometia exit 3 para "DuckDuckGo 202 block anomaly"
  mas o caminho caía no `Ok(output)` que retornava SUCCESS.
- **GAP-AUD-002 (CRÍTICO, wiring de bench)**: `cargo bench --bench pre_flight_latency`
  agora roda Criterion corretamente após adicionar `[[bench]] harness = false`
  em `Cargo.toml`. Antes da fix, o harness default reportava `running 0 tests`
  em vez de executar os 5 cenários de benchmark, dando falsa impressão de
  "sem regressão" quando havia regressão real.
- **Gate de pre-publish (regra 1264)**: `scripts/pre-publish-gate.sh` adiciona
  7 gates sequenciais antes de `cargo publish` real: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test --all-features --locked`,
  `cargo llvm-cov --fail-under-lines 80`, `rg -n v0.7.9 skill/` (sem version drift),
  `cargo publish --dry-run --allow-dirty --no-verify`, e `gh run list --branch main`
  (CI verde). Bloqueia publicação se qualquer gate falhar. Janela de yank: 72h.
- **Seeding determinístico do pino de identidade**: o pino de identidade canônico
  usa seed determinístico por identidade (ex.: `chrome-linux-33333333cccc0003`),
  permitindo reprodução byte-a-byte de payloads JSON entre runs com a mesma
  seed. Sem randomness no pino.
- **MSRV inalterado desde v0.7.2**: `rust-version = "1.88"`.

## Melhorias de Segurança v0.7.8

- **RUSTSEC-2025-0057 (fxhash unmaintained) RESOLVIDO**: A dependência transitiva
  `fxhash 0.2.1` (RUSTSEC-2025-0057, marcada como unmaintained pelo
  banco de advisories do RustSec) foi removida em v0.7.8. O bump de `scraper
  0.20.0` para `scraper 0.27.0` removeu o caminho transitivo via
  `fxhash`. O gate `cargo audit --deny warnings` agora roda limpo para este
  advisory. `deny.toml` não precisa mais da exceção `RUSTSEC-2025-0057`. Apenas
  a ignore do `async-std` (RUSTSEC-2025-0052) permanece, escopada à feature
  opcional `chrome`.
- **Gate de supply chain endurecido**: `cargo audit --deny warnings` é agora
  um gate bloqueante em `.github/workflows/ci.yml` e
  `.github/workflows/release.yml`. Qualquer novo RUSTSEC advisory acima de
  severidade `MEDIUM` falhará o build da PR. A invocação anterior
  de `cargo audit` apenas avisava.
- **Rebalance do detector anti-bot (GAP-WS-52; histórico até v0.9.3)**: O
  predicado de fallback lia o resultado real do detector em vez de uma
  suposição fixa. Quando `--allow-lite-fallback` estava off mas o detector
  sinalizava um interstitial de CAPTCHA, a CLI emitia um `tracing::warn!`
  estruturado e seguia com o código apropriado — NÃO fazia fallback
  silenciosamente. **Desde a v0.9.4 / GAP-WS-113 a flag é no-op legado**
  (Chrome-only; Lite não é caminho de sucesso em produção).
- **Superfície de nível verbose (GAP-WS-53)**: `-vv` e `-vvv` flags adicionados
  a `src/cli.rs` via `ArgAction::Count`. Operadores agora podem escalar
  verbosidade de log sem recompilar. A flag `conflicts_with = "quiet"`
  previne intenção contraditória.
- **Subcomando `Buscar` escondido (GAP-WS-56)**: O subcomando legado
  `Buscar` está marcado com `#[command(hide = true)]`. Continua chamável
  para compatibilidade retroativa mas desaparece do `--help`. Reduz
  superfície de ataque confused-deputy contra CI scripts que parseiam
  output de `--help`.
- **`--retries` honrado end-to-end (GAP-WS-57)**: O contador de retry
  em `src/parallel.rs:644` agora lê `config.retries` em vez de uma
  constante hard-coded. O comportamento anterior silenciosamente descartava
  o valor `--retries` fornecido pelo usuário no caminho `error_output`.
- **Pin em `wreq 6.0.0-rc.29` (GAP-WS-55)**: O bloco `wreq` em
  `Cargo.toml` foi reescrito. O release anterior afirmava
  `wreq 5.3.0` mas o pin real em uso é `6.0.0-rc.29` com três pins diretos
  (`wreq-util`, `brotli-decompressor =5.0.1`, `alloc-no-stdlib =2.0.4`).
  O manifesto Cargo.toml agora bate com a realidade — elimina drift
  documentação-vs-código que tornava audits de supply chain enganosos.
- **MSRV inalterado desde v0.7.7**: `rust-version = "1.88"`.

Para vulnerabilidades introduzidas ou surfacadas por v0.7.7 especificamente, a
regressão de fingerprint TLS (GAP-WS-49) foi a mais proeminente: uma
falha de resolução `wreq-util` que quebrou emulação BoringSSL em certas
distribuições Linux. v0.7.7 entrega o fix de pin em `wreq-util` e
restaura operação normal.


## Sinais Stealth do Chrome (v0.8.5)
- Chrome em modo headed (dentro de display virtual Xvfb privado desde v0.8.5) injeta 17 sinais stealth JavaScript via CDP
- `navigator.webdriver` é definido como `undefined` para evitar detecção de bot (Chrome real tem `undefined`, não `false`)
- Spoofing de fingerprint Canvas previne identificação do navegador
- Spoofing de fingerprint WebGL via overrides de renderer e vendor
- Spoofing de fingerprint AudioContext com injeção de ruído
- Array `navigator.plugins` populado com entradas realistas
- `navigator.languages` definido para corresponder ao idioma do pool de identidade
- Objeto runtime `chrome` spoofado para parecer Chrome real
- `navigator.connection` definido para tipo de rede realista
- `navigator.maxTouchPoints` definido para valores de toque realistas
- Esses sinais NÃO são usados para propósitos maliciosos
- Propósito: contornar detecção anti-bot do Cloudflare para busca legítima
- Chrome roda com flag `--no-sandbox` no Linux para compatibilidade
- `--no-sandbox` é necessário ao rodar como root ou em containers
- Permissões do cookie jar permanecem `0o600` (owner read/write only)
- Nenhum dado do usuário é coletado ou transmitido pelos scripts stealth
