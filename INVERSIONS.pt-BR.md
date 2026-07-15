# Inversões Arquiteturais

`duckduckgo-search-cli` deliberadamente inverte vários defaults comuns do
ecossistema Rust. Este documento explica cada inversão, por que foi feita
e qual é o trade-off. Leia antes de propor uma alternativa "padrão" em
PRs — toda inversão aqui tem uma rationale registrada que uma escolha
"mais idiomática" quebraria silenciosamente.

## Inversão 1 — `wreq` em vez de `reqwest` (v0.7.3–v0.8.5, REVERTIDA na v0.8.6)

> **Status: REVERTIDA na v0.8.6** — substituída por `reqwest` + `rustls-tls` (ADR-0008). Chrome headed (v0.8.0+) fornece fingerprint TLS real de navegador, tornando emulação BoringSSL redundante. A toolchain de build BoringSSL (NASM, CMake, Perl) bloqueava usuários Windows no `cargo install`.

- **Expectativa default**: novos projetos Rust CLI usam `reqwest` com `rustls-tls`.
- **O que fizemos (v0.7.3)**: substituímos `reqwest 0.12 + rustls` por `wreq 6.0.0-rc.29`
  (vincula estaticamente BoringSSL).
- **Por quê**: `rustls` produz um fingerprint TLS canônico que o Cloudflare
  Bot Management reconhece como não-navegador, disparando interstitials
  de CAPTCHA no DuckDuckGo. `wreq` + BoringSSL produz um fingerprint
  idêntico ao Chrome e Safari, eliminando o CAPTCHA no macOS. Veja
  `docs/decisions/0001-tls-boring-via-wreq.md`.
- **Trade-off**: `wreq 6.0.0-rc` é release candidate (não estável 1.0);
  tempo de compilação é ~40s mais longo devido a BoringSSL; builds
  requerem `cmake`, `perl`, `pkg-config`, `libclang-dev` no Linux e
  NASM/CMake/MSVC/Perl no Windows.
- **Por que revertida (v0.8.6)**: Chrome headed (transport primário desde v0.8.0) gera fingerprint TLS REAL de navegador, tornando emulação wreq/BoringSSL redundante. A toolchain de build BoringSSL (NASM, CMake, Perl, MSVC) era barreira total para usuários Windows (GAP-WS-066). Ver `docs/decisions/0008-reqwest-rustls-v0-8-6.md`.

## Inversão 2 — thiserror para libs, sem anyhow em código de biblioteca (v0.5.0+)

- **Expectativa default**: `anyhow::Result` é o padrão de fato para código
  Rust de aplicação.
- **O que fizemos**: definimos `enum CliError` (15 variantes) em
  `src/error.rs` via `thiserror`. Cada erro tem um `error_code()` e
  `exit_code()` tipados. Sem `anyhow` em `src/`.
- **Por quê**: exit codes (0..=6) e error codes (`http_error`,
  `rate_limited`, etc.) machine-readable são parte do contrato público.
  `anyhow` apagaria esses dados. Agentes de IA e CI scripts ramificam em
  `error_code` para decidir retry vs. fail.
- **Trade-off**: 15 braços de match em cada `?`. Novos tipos de erro
  requerem atualizar `exit_code()` e `error_code()`. Mitigação:
  o atributo `#[non_exhaustive]` em `CliError` permite compatibilidade
  forward para consumers downstream.
- **No-go para reversão**: remover erros tipados quebraria silenciosamente
  todo agente que casa em `error_code` para lógica de retry.

## Inversão 3 — `BTreeMap` para histograma em output multi-query (v0.8.0+)

- **Expectativa default**: `HashMap` para agregação.
- **O que fizemos**: `MultiSearchOutput.causa_zero_histogram: BTreeMap<String, u32>`.
- **Por quê**: ordem de iteração determinística entre runs é requerida
  para testes de golden-file snapshot e para output JSON reproduzível
  (snapshot tests via `insta = "1"`). `HashMap` introduz ordem
  aleatória de iteração → snapshot tests flaky.
- **Trade-off**: insert ligeiramente mais lento (O(log n) vs O(1)). O
  histograma tem <100 entradas na prática; custo é negligível.
- **No-go para reversão**: output JSON não-determinístico quebra o
  contrato de snapshot test.

## Inversão 4 — Nomes de campo em português brasileiro no JSON de saída (v0.2.0+)

- **Expectativa default**: ecossistema Rust usa identificadores em inglês.
- **O que fizemos**: campos de `SearchResult` serializam como `posicao`,
  `titulo`, `url`, `url_exibicao`, `snippet`, etc. (não `position`, `title`,
  `url`).
- **Por quê**: exemplos do README e receitas `jaq` em `docs/COOKBOOK.md`
  usam queries em português; campos em inglês quebravam esses pipelines
  (bug reportado pelo usuário em v0.1.0 → corrigido em v0.2.0). O naming
  em PT-BR é load-bearing no modelo mental do agente.
- **Trade-off**: pipelines de outros ecossistemas (`n8n`, `zapier`,
  `make.com`) precisam aprender os nomes de campo em português. A
  tabela de mapeamento completa está documentada em
  `docs/INTEGRATIONS.md`.
- **No-go para reversão**: mudar nomes de campo quebraria silenciosamente
  todo pipeline CI construído no contrato v0.2.0+. O guia de migração
  v0.1.0 → v0.2.0 foi um evento único.

## Inversão 5 — `#[serde(skip_serializing_if = "Option::is_none")]` em TODOS os campos Option

- **Expectativa default**: serializar `Option::None` como JSON `null`.
- **O que fizemos**: todo campo `Option<T>` em `types.rs` carrega
  `#[serde(skip_serializing_if = "Option::is_none")]`.
- **Por quê**: o envelope JSON deve ser mínimo — consumers não precisam
  diferenciar "campo ausente" de "campo é null". Campos ausentes
  significam "não aplicável para esta query" (ex., `causa_zero` é
  ausente quando results > 0, presente quando zero).
- **Trade-off**: pipelines não podem distinguir "campo faltando" de
  "campo era null na serialização". Mitigação: o SKILL.md documenta
  a semântica dos campos; o campo `causa_zero` é um aditivo diagnóstico
  (BC opt-out preserva o campo mesmo quando exit code é 5 legacy).
- **No-go para reversão**: ligar serialização de `null` dobraria o
  tamanho de cada output JSON e requereria todo consumer tratar ambos
  `null` e ausente.

## Inversão 6 — `--allow-lite-fallback` como OPT-IN (v0.7.8+; SUPERSEDIDA / NO-OP desde v0.9.4)

> **Status: SUPERSEDIDA / NO-OP desde v0.9.4 (GAP-WS-113 / ADR-0016)** — produção é Chrome-only; Lite nunca é caminho de sucesso. A flag permanece só por BC de scripts e não força degradação de endpoint.

- **Expectativa default**: fallback para endpoint lite quando html falha.
- **O que fizemos (v0.7.8–v0.9.3)**: fallback exigia a flag explícita `--allow-lite-fallback`.
  Sem ela, detecção anti-bot retornava exit 3 com `cascata_motivo`
  populado no JSON, NÃO fallback silencioso.
- **O que fazemos agora (v0.9.4+)**: a flag é **no-op legado**. A SERP permanece HTML
  canônico sob Chrome; remediação é instalar Chrome / `--chrome-path` / `--proxy`.
- **Por quê (original)**: fallback silencioso viola a intenção do usuário. O usuário
  pode querer saber que está sendo bloqueado (para fins de rate limit)
  em vez de receber resultados truncados de um endpoint degradado. v0.7.8
  GAP-WS-52 corrigiu o comportamento de fallback silencioso.
- **Por que no-op agora**: transporte dual (HTTP/Lite sob Chrome) produzia zero hits
  classificados como legítimos; GAP-WS-113 remove Lite como caminho de sucesso em produção.
- **Trade-off**: scripts que ainda passam a flag são inofensivos (no-op), mas não
  devem tratá-la como remediação ativa.
- **No-go para reversão**: reintroduzir Lite como caminho de sucesso silencioso
  restauraria o canal dual-transport fechado pela ADR-0016.

## Inversão 7 — `bin/safety-contracts` para gates de CI (v0.7.10+)

- **Expectativa default**: um único workflow CI roda todos os checks.
- **O que fizemos**: cada gate de CI é um script `bin/` discreto invocado
  individualmente pelo workflow. Exemplos: `bin/check-fmt`,
  `bin/check-clippy`, `bin/check-tests`, `bin/check-audit`,
  `bin/check-coverage`, `bin/check-version-drift`.
- **Por quê**: binários discretos deixam desenvolvedores rodarem o gate
  CI exato localmente antes de fazer push. Um único workflow `ci.yml`
  com bash embarcado era imensurável em isolamento.
- **Trade-off**: 9+ binários para manter. Mitigação: cada binário tem
  <50 linhas e tem um `README.md` por script.
- **No-go para reversão**: CI monolítico é um ponto de dor conhecido
  para debug de flakes.

## Inversão 8 — `atomwrite` como única ferramenta de edição de arquivo (v0.8.0+)

- **Expectativa default**: `std::fs::write` ou `tokio::fs::write` em
  código Rust, `sed -i`/`echo >` em scripts.
- **O que fizemos**: toda modificação de arquivo passa pela CLI
  `atomwrite` com `--expect-checksum` (locking otimista via BLAKE3)
  e escrita atômica (tempfile + fsync + rename).
- **Por quê**: um incidente de truncamento de `c24-framework34.html`
  (2026-06-15) no projeto upstream perdeu ~127 linhas de trabalho.
  `atomwrite` provê 6 camadas de defesa (L1 telemetria, L2 `--require-backup`,
  L3 `--confirm`, L4 `--preview`, L5 `--auto-rotate`, L6 `risk_assessment`
  no envelope). Veja ADR-0035.
- **Trade-off**: cada invocação de script tem uma cerimônia
  `CS=$(atomwrite read --json ...)`. Mitigação: aliases em `.cargo/config.toml`
  (`cargo check-all`, `cargo lint`, etc.) reduzem o boilerplate.
- **No-go para reversão**: sobrescritas silenciosas são exatamente o modo
  de falha que causou o incidente 2026-06-15.

## Inversão 9 — Sem telemetria, sem analytics, sem export OTLP (todas as versões)

- **Expectativa default**: CLIs de produção emitem telemetria de uso
  para endpoints controlados pelo vendor.
- **O que fizemos**: zero telemetria. `tracing` é usado para logs
  locais mas nunca exportado. Padrões `opentelemetry`, `OTLP`,
  `exporter` e `analytics` estão explicitamente ausentes da base
  de código. CI gate `rg -n 'opentelemetry|OTLP|exporter|tracing::span' src/` retorna 0.
- **Por quê**: privacidade primeiro. O usuário é o único dono dos seus
  dados de busca. Detecção anti-bot é mais difícil quando o fingerprint
  do cliente não inclui uma assinatura de agente de telemetria.
- **Trade-off**: zero observabilidade de uso em produção. Mitigação:
  logs locais `tracing` em stderr; flags `--verbose`/`-vv`/`-vvv`
  escalam verbosidade; o usuário pode fazer grep dos próprios logs.
- **No-go para reversão**: README e SKILL.md do projeto declaram
  explicitamente "sem telemetria". Adicionar telemetria requereria
  nova versão major.

## Inversão 10 — Headed-dentro-de-Xvfb em vez de headless (v0.8.7, GAP-WS-072 a WS-078; macOS/Windows atualizado na v0.9.3)

- **Expectativa default**: automação de browser usa headless clássico para execução invisível.
- **O que fizemos**: Chrome roda HEADED dentro de display virtual Xvfb privado no Linux. Em macOS/Windows, a **v0.9.3 (GAP-WS-112)** mudou para **headless=new** (headed nativo Quartz/DWM da v0.9.1 foi supersedido).
- **Por quê**: Cloudflare Bot Management 2026 detecta sinais de headless clássico; headed-em-Xvfb (Linux) e headless=new (macOS/Windows) passam anti-bot melhor. Xvfb fornece display X11 invisível para o usuário não ver NENHUMA janela no Linux.
- **Trade-off**: Linux requer Xvfb (a CLI auto-instala via `try_auto_install_xvfb()` para 22+ distros). macOS/Windows não precisam de dependência extra. A navegação de warm-up para duckduckgo.com adiciona ~800-1500ms de latência por busca.
- **No-go para reversão**: voltar ao headless legado detectável no Linux restauraria a detecção do Cloudflare.

## Inversão 11 — Vertical de notícias é Chrome-only e deep-research varre news por padrão (v0.8.9, GAP-WS-104/105; endurecido fail-closed na v0.9.4 / ADR-0016)

- **Expectativa default**: CLIs HTTP-first oferecem fallback HTTP para toda vertical, e features novas chegam opt-in.
- **O que fizemos**: `--vertical news|all` roteia EXCLUSIVAMENTE pelo transporte Chrome (a SERP de notícias exige JavaScript; NÃO há fallback HTTP) e o `deep-research` varre news por PADRÃO com a flag de opt-out `--no-news`.
- **Histórico da política de Chrome**: v0.8.9 falhava rápido (exit 2) sem Chrome e sem `--no-news`; v0.9.0 / GAP-WS-106 aplicava brevemente `--no-news` automaticamente com warning no stderr e prosseguia web-only; **v0.9.4 / GAP-WS-113 restaura fail-closed rígido** — sem Chrome utilizável (ou com `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`) toda op de rede, inclusive `deep-research` e `--vertical news|all`, **sai com exit 2** (sem auto `--no-news`, sem rebaixamento para Web). Ver ADR-0016.
- **Por quê**: a SERP de notícias é 100% renderizada por JS (scraping HTTP retorna casca vazia) e um deep-research cego para eventos recentes produz sínteses defasadas — news-by-default garante frescor sem flag extra. A auto-degradação suave mascarava Chrome ausente como sucesso vazio/web-only; fail-closed torna a dependência explícita.
- **Trade-off**: **dependência rígida de Chrome** para todas as ops de rede de produção desde a v0.9.4 (CI e hosts devem fornecer Chrome/Chromium — e Xvfb em Linux headless quando necessário); +2-4s por sub-query para news, sobrepostos no fan-out. Ver `docs/decisions/0010-news-vertical-v0-8-9.md`, `docs/decisions/0011-deep-research-news-dual-v0-8-9.md` e `docs/decisions/0016-chrome-only-universal-v0-9-4.md`.

## Inversão 12 — Ownership one-shot de processos para Chromium/Xvfb (v0.9.6, GAP-WS-LIFECYCLE-001 / ADR-0017)

- **Expectativa default**: automação de browser confia em `kill_on_drop` / `Child::kill` só no processo raiz e deixa o SO reparentar restos sob `systemd --user` / `init`.
- **O que fizemos**: ownership completo da árvore de processos da sessão em `src/process_lifecycle.rs` — process group (`setpgid`), `PR_SET_PDEATHSIG` no Linux, `killpg`, walk da árvore, kill por marker de `user-data-dir`, limpeza de lock/socket do Xvfb, session registry + panic hook; `XvfbGuard` RAII; shutdown assíncrono cooperativo do `ChromeBrowser` com deadline de close/wait e `force_reap_session` no `Drop`; `content_fetch` com take + shutdown assíncrono; SIGTERM e SIGINT cancelam o `CancellationToken` compartilhado; `paths::atomic_write` para `--output`, `init-config` e cookie jar.
- **Por quê**: o kill só na raiz do chromiumoxide deixava netos de Chromium e Xvfb órfãos em hosts de agentes de longa duração (centenas de browsers / GiB de RAM). Uma CLI one-shot deve ser NASCE → EXECUTA → MORRE para todo processo externo que ela inicia.
- **Trade-off**: **SIGKILL** da própria CLI não é interceptável (limite do SO); o upgrade não limpa órfãos históricos de execuções **anteriores à v0.9.6**. Operadores podem precisar de uma limpeza única do host após atualizar.
- **No-go para reversão**: voltar ao `kill_on_drop` só na raiz reintroduz acúmulo em enxame em sessões de agentes multi-dia.
- **Relacionado**: `docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md` (ADR-0017).

## Inversão 13 — Defaults agent-ready: dual vertical + texto limpo + Chrome multi-canal (v0.9.8, GAP-WS-AGENT-READY-001 / ADR-0018)

- **Expectativa default**: capacidades novas chegam opt-in; busca fica web-only; fetch de conteúdo é explícito; auto-detect de browser confia só em binários do gerenciador de pacotes do host; `--chrome-path` depois de `deep-research` é inválido; fetch nunca toca news.
- **O que fizemos**: padrão `--vertical all` (web + news; opt-out `--vertical web` / deep `--no-news`); fetch de conteúdo **LIGADO** para web + news (FETCH_CAP=10; opt-out `--no-fetch-content`); resolve multi-canal Chrome (export Flatpak → ELF de deploy `files/extra/chrome`; ordem `--chrome-path` → `CHROME_PATH` → host Chrome → host Chromium → Flatpak → Snap); flags de transporte `global = true` (incluindo `--chrome-path` após `deep-research`); metadados honestos de agente `chrome_path_resolvido` / `chrome_canal` / `usou_chrome` (**não** telemetria); news pode trazer `conteudo`; sem flag separada `--agent`.
- **Por quê**: agentes de IA precisam de SERP dual + texto limpo sem inventar flags; Chrome Flatpak é comum no Linux e era rejeitado silenciosamente quando só o shell de export era sondado; o clap rejeitava flags de transporte depois do subcomando.
- **Trade-off**: latência padrão maior e envelopes JSON maiores (limitados por FETCH_CAP=10); anti-bot ainda pode zerar news (web>0, news vazia → exit 0 degradação honesta); hosts precisam de ELF Chrome utilizável (incluindo path de deploy Flatpak). Consumidores finos optam por `--vertical web --no-fetch-content`.
- **No-go para reversão**: reintroduzir defaults web-only + fetch desligado quebra o contrato agent-ready documentado em skills, schemas e ADR-0018.
- **Relacionado**: `docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md` (ADR-0018); inventário `docs/gaps.md`. Preserva Inversão 12 (one-shot) e produção Chrome-only (0.9.4).

## Inversão 14 — Prefixo auditável de perfil Chrome + one-shot de disco (v1.0.0, GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020)

- **Expectativa padrão**: one-shot de processo basta; `tempfile::tempdir()` com `.tmp` genérico é aceitável; reap de PIDs deixa o reaper do SO/tmp limpar diretórios; bulk-delete de “todos os temp sobrando” é higiene de host ok.
- **O que fizemos**: prefixo **`ddg-chrome-`** via `tempfile::Builder` (Unix `0o700`); `force_reap` / `reap_all_registered` **removem o perfil** com `remove_dir_all`; `ExitReapGuard` + panic hook + reap em timeout/fim de run; `sweep_orphan_profiles` na próxima run **somente** para `ddg-chrome-*` de propriedade sem processo vivo; recusa dura de bulk-delete de `.tmp*` estrangeiro e `org.chromium.Chromium.*`; deep-research herda o `CancellationToken` do `main`.
- **Por quê**: o reap de processo (Inversão 12) ainda deixava árvores de perfil órfãs sob `.tmp` genérico após cancel/timeout/fan-out; mass-rm de `.tmp*` colide com outras apps Rust; stubs globais do Chromium não são da CLI.
- **Trade-off**: SIGKILL/OOM da CLI ainda pode deixar residual até a **próxima** invocação varrer só `ddg-chrome-*`; perfis históricos pré-1.0.0 em `.tmp*` **não** são mass-auto-apagados (operador limpa uma vez se precisar).
- **No-go para reverter**: voltar a `.tmp` genérico ou bulk-rm de prefixos temp estrangeiros reintroduz residual não auditável e risco de delete cross-app.
- **Relacionado**: `docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md` (ADR-0020); estende Inversão 12 (processo) com honestidade de disco; inventário `docs/gaps.md`.

## Como Propor uma Nova Inversão

1. Abra uma issue com a label "Proposta de Inversão".
2. Documente: qual default você está invertendo, por que o default
   falha no contexto deste projeto, qual é o trade-off, e um critério
   de no-go (quando esta inversão NÃO deve ser revertida).
3. Adicione uma seção a este arquivo seguindo o formato das inversões
   existentes.
4. Atualize o `description` do workspace `Cargo.toml` se a inversão
   afeta o contrato público.
5. Referencie a inversão na ADR relevante.
