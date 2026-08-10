# Política de Segurança
Leia em [English](SECURITY.md).


## Versões com Suporte
- Somente a release atual recebe correções de segurança de rotina
- Versão **1.0.5** é a versão atual (régua de fronteira do stdout, matriz de agent ops, identidade nunca truncada, envelope único de recusa; sem env de produto, sem telemetria remota)
- Versões **1.0.4** e **1.0.3** recebem correção apenas para achados **Críticos**, até a próxima minor sair
- Toda linha abaixo da **1.0.3** está sem suporte — **1.0.2 e anteriores não compilam em macOS nem Windows**
- Linhas mais antigas permanecem na tabela apenas por contexto histórico; atualize para **1.0.5**
- Campos de metadados agent `chrome_path_resolved` / `chrome_channel` (legado PT: `chrome_path_resolvido` / `chrome_canal`) são contrato JSON local para integradores — **não** são telemetria remota
- Fetch de conteúdo está **LIGADO por padrão** desde a v0.9.8 (opt-out `--no-fetch-content`); HTML das páginas buscadas continua sendo entrada não confiável parseada localmente
- Pass 52 **não** inventa CVEs; o endurecimento de lifecycle e pipe de stream é correção operacional, não advisory de segurança

| Versão | Suportada |
|---|---|
| 1.0.5 | **Sim (atual; régua de fronteira do stdout, matriz de agent ops, identidade isenta de truncamento, envelope único de recusa)** |
| 1.0.4 | Só crítico (flags agent-native agem ou recusam pelo nome; sete envelopes ganharam discriminador; wire EN imposto nos tipos de domínio) |
| 1.0.3 | Só crítico (hotfix cross-platform — restaura a compilação em macOS e Windows) |
| 1.0.2 | Não (**não compila em macOS nem Windows** — use 1.0.3+; wire EN default ADR-0027, RuntimeConfig SSOT, agent ops, budget contention, mute-audio padrão) |
| 1.0.1 | Não (histórico; Pass 52 SIG_IGN+limpeza oneshot, BrokenPipe→141) |
| 1.0.0 | Não (histórico; GAP-WS-TMP-PROFILE-ORPHAN-001 one-shot processo+disco, só `ddg-chrome-*`; ADR-0020) |
| 0.9.10 | Não (linha crates.io histórica; runtime ≈ 0.9.9) |
| 0.9.9 | Não (histórico; e2e news/timeout/probe/meta; timeout global padrão 180s; ADR-0019) |
| 0.9.8 | Não (histórico; GAP-WS-AGENT-READY-001 dual vertical + fetch default ON + Flatpak multi-canal; ADR-0018) |
| 0.9.7 | Não (histórico; lifecycle 0.9.6 + null check de HANDLE no Windows MSVC) |
| 0.9.6 | Não (histórico; lifecycle GAP-WS-LIFECYCLE-001; **não compila no Windows MSVC**) |
| 0.9.5 | Não (histórico; GAP-WS-113 + fix de release) |
| 0.9.4 | Não (histórico; GAP-WS-113 Chrome-only fail-closed, sem auto-degradação, fallback Lite no-op) |
| 0.9.3 | Não (histórico; GAP-WS-112 macOS/Windows headless=new) |
| 0.9.2 | Não (histórico; GAP-WS-108/109/110/111 endurecimento stealth chromiumoxide) |
| 0.9.1 | Não (histórico; GAP-WS-107 macOS/Windows headed nativo) |
| 0.9.0 | Não (histórico; GAP-WS-106 flags globais; auto-degradação **supersedida pela 0.9.4**) |
| 0.8.9 | Não (histórico; GAP-WS-104 vertical de notícias exclusiva do Chrome, ZeroCause `vertical-sem-resultados`, correções pós-revisão F1-F7) |
| 0.8.8 | Não (histórico; `has_native_display()`, auto-install Xvfb 22+ distros, 17 sinais stealth, navegação warm-up, GAP-WS-060 até GAP-WS-103 fechados) |
| 0.8.0 | Não (histórico; transporte Chrome-primary, classificação causal de zero-result, descompressão HTTP) |
| 0.7.10 | Não (histórico; scheduler pre-flight, propagação de pino de identidade) |
| 0.7.8 | Não (histórico; 8 gaps do detector anti-bot fechados) |
| 0.7.7 | Não (histórico; GAP-WS-49 corrigido regressão de fingerprint TLS) |
| 0.7.3 | Não (histórico; fix de stack TLS — rustls substituído por BoringSSL) |
| < 0.7.3 | Não |


## Reportando uma Vulnerabilidade
- Reporte vulnerabilidades por advisory privado no GitHub: https://github.com/danilo-aguiar-br/duckduckgo-search-cli/security/advisories/new
- Inclua uma descrição clara da vulnerabilidade e os passos para reprodução
- Inclua a versão afetada e o impacto potencial
- Inclua qualquer mitigação que você já identificou
- NÃO abra uma issue pública no GitHub para vulnerabilidades de segurança
- Espere um aviso de recebimento dentro de 72 horas


## SLA de Resposta e de Correção
- Confirme o recebimento de todo relato dentro de **72 horas**
- Confirme ou rejeite o achado dentro de **7 dias** após o aviso de recebimento
- Corrija severidade **Crítica** (CVSS 9.0–10.0) dentro de **7 dias** após a confirmação
- Corrija severidade **Alta** (CVSS 7.0–8.9) dentro de **30 dias** após a confirmação
- Corrija severidade **Média** (CVSS 4.0–6.9) dentro de **90 dias** após a confirmação
- Corrija severidade **Baixa** (CVSS 0.1–3.9) na próxima release programada
- Comunique um cronograma revisado ao relator sempre que um prazo acima não puder ser cumprido


## Política de Divulgação
- Período de embargo: 90 dias a partir do recebimento do relato
- A vulnerabilidade NÃO será divulgada publicamente antes do fim do embargo
- Correção e divulgação coordenadas acontecem ao fim do período de embargo
- Se a correção não puder sair em 90 dias, o cronograma é comunicado ao relator


## Escopo
- Em escopo: falhas na construção de requisições HTTP que possam habilitar SSRF, injeção de cabeçalho ou request smuggling contra o DuckDuckGo ou URLs buscadas
- Em escopo: fraquezas no parsing de HTML no pipeline de extração disparadas por resposta de servidor hostil (DoS via DOM manipulado, XXE apesar do contexto HTML, seletores CPU-bomb)
- Em escopo: vazamento de credenciais no tratamento de `--proxy user:pass@...` em logs, mensagens de erro ou no JSON de saída — o mascaramento deve prevenir isso, então reporte qualquer vazamento
- Em escopo: ataques de path traversal ou symlink contra o caminho do arquivo de saída (`-o, --output`) ou o diretório de config XDG
- Em escopo: manipulação do cookie jar — o arquivo `cookies.json` da v0.7.3+ contém cookies de sessão do DuckDuckGo e é gravado com permissões Unix 0o600. Reporte qualquer forma de ler este arquivo como outro usuário local, ou qualquer forma da CLI enviar esses cookies para uma origem que não seja DuckDuckGo.
- Em escopo: configuração incorreta de TLS que possa habilitar MITM — o HTTP residual em Rust usa `reqwest` + **rustls** com CryptoProvider único **`aws-lc-rs`** (ADR-0021; sem `native-tls`/OpenSSL). O SERP de produção usa o processo Chrome (ADR-0016). Reporte fallback para cipher suites inseguras ou reintrodução de `native-tls`
- Em escopo: problemas de supply chain em dependências transitivas fixadas ainda não documentadas em `deny.toml`


## Fora do Escopo
- Negação de serviço causada pelo usuário passando flags patológicas (`--parallel 20 --pages 5 --fetch-content` em milhares de queries é esperado consumir recursos significativos)
- Vulnerabilidades no próprio DuckDuckGo — reporte-as ao DuckDuckGo
- Vulnerabilidades no Chrome/Chromium usados com `--features chrome` — reporte-as ao projeto Chromium
- Problemas que exigem uma conta de usuário local comprometida ou acesso de escrita ao `$XDG_CONFIG_HOME`
- Processos órfãos residuais de Chromium/Xvfb de execuções anteriores à v0.9.6, diretórios de perfil legados de binários pré-1.0.0 (prefixo genérico `.tmp*`), ou residual após **SIGKILL**/OOM externo da própria CLI, são limites operacionais de higiene do host (o SO não entrega handlers em SIGKILL) — não são CVE, a menos que habilitem escalonamento de privilégio ou acesso cross-user. Desde a v1.0.0 a CLI **nunca** faz bulk-delete de `/tmp/.tmp*` estrangeiro nem de stubs `org.chromium.Chromium.*`; o sweep da próxima run só mira `ddg-chrome-*` de propriedade desta CLI


## Premissas de Design de Segurança
- A CLI é um cliente HTTP read-only — não escreve em sistemas remotos
- Todos os inputs externos (strings de query, paths de saída) são validados antes do uso
- Ataques de path traversal são bloqueados: paths de saída com componentes `..` são rejeitados com exit code 2
- URLs de proxy são mascaradas nos logs: credenciais viram `[...]` antes de qualquer saída
- **v0.7.3+**: Cookie jar persistido em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). O arquivo é gravado com permissões Unix `0o600` (owner read+write only). No Windows, o diretório herda a ACL do perfil do usuário. Os cookies são cookies de sessão emitidos por `duckduckgo.com` e `html.duckduckgo.com`. **Trate este arquivo como trataria qualquer credencial.** Use `--no-cookie-persistence` para manter cookies em memória apenas. Use `--cookies-path <PATH>` para realocar o arquivo para um volume encriptado.
- **v0.7.8+**: A superfície da flag de verbosidade foi ampliada. `-v` é info, `-vv` é debug, `-vvv` é trace (GAP-WS-53). Operadores que investigam anomalias escalam o detalhe de log sem recompilar. A flag `conflicts_with = "quiet"` impede intenção contraditória. Use isto ao reportar suspeita de vulnerabilidade — a saída de `-vvv` é o diagnóstico mais útil que os mantenedores podem receber.
- O binário não executa subprocessos nem comandos de shell a partir de resultados de busca
- **v0.8.6+ / Pass 40 (ADR-0021)**: TLS residual via **rustls** + provider de processo **`aws-lc-rs`** (`tls_bootstrap` no `main`). Feature `rustls-tls-webpki-roots-no-provider`. SERP de producao: TLS do Chrome (ADR-0016). Sem OpenSSL/SChannel/SecureTransport no binario Rust
- Desde a v0.8.0 a CLI executa JavaScript via Chrome na fase de busca — o processo Chrome é isolado e roda dentro de display virtual Xvfb privado (v0.8.5+)
- Quando `--fetch-content` está ativo, páginas buscadas são parseadas com `scraper` (que usa `html5ever`); HTML não confiável é esperado
- **v0.9.8+**: o fetch de conteúdo é **LIGADO por padrão** para web + news (FETCH_CAP=4 na v1.0.2; era 10 na v0.9.8); opt-out com `--no-fetch-content`. Isso aumenta a superfície de parse HTML — ainda é o design esperado; páginas hostis continuam no escopo de relatórios de DoS de parsing
- **v0.9.8+ / v1.0.2 metadados de agente NÃO são telemetria**: `chrome_path_resolved`, `chrome_channel` e `used_chrome` (legado PT: `chrome_path_resolvido`, `chrome_canal`, `usou_chrome`) são apenas campos do contrato JSON local; sem exportação remota. Wire default EN desde 1.0.2 (ADR-0027); legado PT via `--wire-keys pt`
- **v0.7.3+**: A CLI não é mais totalmente sem estado. O cookie jar persistente adiciona estado entre invocações. É um trade-off deliberado para reduzir a taxa de CAPTCHA no servidor do DuckDuckGo. O request de warm-up (`GET https://duckduckgo.com/`) é idempotente e não persiste nenhum dado identificador de usuário além dos próprios cookies.
- Arquivos de saída são criados com permissão `0o644` no Unix (proprietário escreve, mundo lê)
- Nada é escrito fora do caminho que o usuário passou


## Automação de Supply Chain Relacionada
- Execute **localmente** (CI/CD e GitHub Actions são **proibidos** neste repo):
- `cargo audit --deny warnings` contra o banco RustSec
- `cargo deny check advisories licenses bans sources` com a política em `deny.toml`
- Atualizações de deps: `cargo update` / `cargo deny check` localmente — **sem** Dependabot, **sem** Actions


## Política de Atualização de Segurança
- Entregue toda correção de segurança como release de patch no crates.io — não há pipeline de CI, então toda release é cortada manualmente (veja [NO_CI.md](NO_CI.md))
- Rode os 10 gates locais de validação antes de publicar qualquer release de segurança (veja [CONTRIBUTING.pt-BR.md](CONTRIBUTING.pt-BR.md))
- Registre a correção em `CHANGELOG.md` e `CHANGELOG.pt-BR.md` sob a versão publicada
- Faça yank de uma release quebrada no crates.io em até **72 horas** quando a correção não couber nessa janela
- Nunca faça backport silencioso — a entrada do changelog nomeia a versão que carrega a correção
- Não anuncie nada antes de terminar o embargo da Política de Divulgação acima


## Hall da Fama
- Credite todo relator que seguir esta política, salvo pedido explícito de anonimato
- Acrescente o nome do relator e a versão corrigida a esta lista no momento da divulgação
- Nenhum pesquisador reportou vulnerabilidade confirmada até agora — esta lista está vazia de propósito


## Boas Práticas para Usuários
- Instale do crates.io com `cargo install duckduckgo-search-cli --locked` e mantenha o binário na release atual
- Trate `cookies.json` como credencial: ele é modo `0o600` no Unix, e `--no-cookie-persistence` mantém a sessão só em memória
- Realoque o cookie jar para um volume encriptado com `--cookies-path <PATH>` em hosts compartilhados
- Nunca passe credenciais de proxy no histórico de um shell compartilhado — prefira a chave XDG `proxy_url` a `--proxy user:pass@...`
- Mantenha `--output` dentro de um diretório seu; caminhos com `..` são rejeitados com exit code 2
- Encapsule invocações de agente com um `timeout` externo para um Chrome travado não sobreviver à execução
- Leia `.metadata.zero_cause` antes de retentar uma execução com zero resultados em vez de repetir às cegas
- Reporte qualquer suspeita de vazamento pelo canal privado de advisory acima, nunca por issue pública


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

## Melhorias de Segurança v1.0.0
- **GAP-WS-TMP-PROFILE-ORPHAN-001 (ALTO, one-shot de perfil Chrome em disco, ADR-0020)**: fecha o residual em que o reap de processo (0.9.6) deixava árvores `user-data-dir` órfãs sob prefixos genéricos de tempfile. Perfis usam o prefixo auditável **`ddg-chrome-`** com modo Unix **`0o700`**; `force_reap` / `reap_all_registered` removem o diretório após o kill; `ExitReapGuard` + panic hook + reap em timeout/fim de run cobrem exits cooperativos.
- **Sweep seletivo apenas**: a próxima invocação com `sweep_orphan_profiles` remove **`ddg-chrome-*`** sem processo dono vivo. **Política dura (não opcional):** nunca auto-`rm` em massa `.tmp*` genérico; nunca auto-`rm` stubs `org.chromium.Chromium.*` — são estrangeiros ou do Chromium e fora do bulk delete.
- **Guards de ownership**: `is_cli_owned_profile_name` / `is_forbidden_bulk_delete_name` / `remove_user_data_dir` recusam prefixos estrangeiros para o blast radius de limpeza não expandir por bug ou path hostil.
- **Herança de cancel em deep-research**: herda o `CancellationToken` do `main` para SIGTERM cancelar o fan-out e o reap de disco poder rodar.
- **Limite residual (documentado, não é vulnerabilidade)**: **SIGKILL**/OOM da CLI não é interceptável; uma invocação posterior pode varrer só `ddg-chrome-*` desta CLI. Perfis históricos pré-1.0.0 em `.tmp*` **não** são bulk-deleted por design.
- **Sem telemetria remota**: lifecycle de disco e sweep emitem apenas `tracing` local.

## Melhorias de Segurança v1.0.2
- **ADR-0027 (wire EN default)**: a serialização JSON de stdout usa chaves em **inglês** (`results`, `title`, `metadata`, `result_count`, `chrome_channel`, `chrome_path_resolved`, `used_chrome`, …). Desserialização ainda aceita aliases PT. Remap legado: `--wire-keys pt` ou `config set wire_keys pt`.
- **RuntimeConfig SSOT** (`src/runtime/`): precedência CLI > XDG > FACTORY; sem env de produto para knobs de runtime.
- **Agent ops** (sem jq): `--fields`/`--select`, `--filter`, `--limit`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`.
- **Mute-audio padrão (ADR-0026)**: Chrome lança com mute obrigatório (sem unmute de produto).
- **Sem telemetria remota**: metadados de agente e lifecycle permanecem só locais.

## Melhorias de Segurança v0.9.8
- **GAP-WS-AGENT-READY-001 (ALTO, defaults agent-ready, ADR-0018)**: vertical dual e fetch de conteúdo LIGADOS por padrão aumentam a superfície local de parse HTML (ainda é o design esperado). Metadados de agente (`chrome_path_resolvido` / `chrome_canal` / `usou_chrome` no wire PT histórico; EN em 1.0.2: `chrome_path_resolved` / `chrome_channel` / `used_chrome`) **não** são telemetria e não são exportados remotamente.
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
- **GAP-WS-113 (CRÍTICO, transporte Chrome-only universal, ADR-0016)**: o caminho de rede em produção é exclusivamente chromiumoxide/CDP. Chrome ausente (ou binário sem feature `chrome`) **falha com exit 2** em qualquer operação de rede — sem sucesso HTTP silencioso, sem auto-degradação Web/`--no-news`. A env de produto `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` foi **removida** / não é lida. Remove canal dual-transport que podia apresentar resultados vazios como zeros legítimos sob anti-bot.
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
- **Pre-publish (regra 1264, só local)**: gates manuais antes de `cargo publish`
  (`fmt`, `clippy -D warnings`, `test --locked`, `llvm-cov`, dry-run). **Sem**
  GitHub Actions removido. Pre-publish é checklist manual/local apenas (ver `NO_CI.md`).
  Janela de yank: 72h.
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
- **Gate de supply chain endurecido**: `cargo audit --deny warnings` deve
  passar **localmente** antes de publish/merge. CI/CD e GitHub Actions são
  proibidos; qualquer advisory RUSTSEC acima de `MEDIUM` deve falhar o gate local.
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


## Mitigação de sinais de automação do Chrome (v0.8.5+ / ADR-0022)
- SERP de produção usa **TLS nativo do Chrome** (ADR-0016). Objetivo: **não** expor assinatura TLS de **biblioteca** (`rustls` JA4 bot-class) que o Cloudflare Bot Management bloqueia (GAP-WS-27) — **não** é “feature de fingerprint”
- **Proibido (ADR-0022):** spoof sintético de hardware fingerprint (ruído de canvas, mentira de GPU WebGL, ruído de AudioContext, `hardwareConcurrency`/`deviceMemory`/`colorDepth`/`languages`/`connection` forçados). Spoofs estáticos viram assinatura de automação compartilhada
- **Permitido:** só mitigação de sinais de automação CDP — `navigator.webdriver` → `undefined`, `plugins`/`mimeTypes` realistas, stubs `window.chrome`, tamanho outer da janela, Permissions, bloqueio de leak WebSocket DevTools (GAP-WS-076)
- Propósito: busca legítima no DuckDuckGo sem perfil de cliente bot-class
- Chrome pode usar `--no-sandbox` no Linux quando necessário (root/containers)
- Cookie jar permanece `0o600`; sem telemetria de produto
