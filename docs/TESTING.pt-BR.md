# Guia de Testes

[English](TESTING.md)

Este guia cobre execução, categorização e integração multiplataforma local dos
testes de `duckduckgo-search-cli`.

## Os gates — cada alias de `.cargo/config.toml`

Este projeto não tem CI ([NO_CI.pt-BR.md](../NO_CI.pt-BR.md)). Todo gate é um alias local
do cargo, então esta lista É o pipeline. `every_cargo_alias_is_documented_in_the_testing_guide`
reprova o build quando um alias entra lá e não é nomeado aqui.

### Linux, conjunto completo de features
- `cargo check-all` — `check --all-targets --all-features --locked`
- `cargo lint` — `clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test-all` — `test --all-features --locked` (unit + integração + doctests)
- `cargo docs` — `doc --no-deps --all-features --locked`; rode como
  `RUSTDOCFLAGS='-D warnings' cargo docs`, porque o alias não define a variável

### O perfil publicado, sem toolchain C (ADR-0029)
- `cargo check-nohttp` — `check --no-default-features --features chrome --locked`
- `cargo check-nohttp-all-targets` — o mesmo perfil incluindo a árvore de
  testes (v1.0.6, GAP-REL-002). O `check-nohttp` omite `--all-targets` e o
  `test-all` usa `--all-features`, onde o harness está sempre ligado, então
  entre os dois nada jamais compilou os testes sob o perfil que o usuário
  instala. Medido em 2026-08-21: 91 erros, três deles `E0432` — o mesmo código
  do defeito publicado na v1.0.2, escondido na árvore de testes.
- `cargo lint-nohttp` — mesmo alvo sob `clippy -- -D warnings`
- `cargo docs-nohttp` — `doc --no-deps --no-default-features --features chrome --locked`
- `--all-features` liga `http-test-harness`, que puxa `reqwest` + `rustls` +
  `aws-lc-sys`; este último é C. O perfil `chrome` é o que o usuário instala e é
  100% Rust.

### Cross-platform, sem linker
- `cargo check-windows` — alvo `x86_64-pc-windows-gnu`
- `cargo check-windows-msvc` — alvo `x86_64-pc-windows-msvc`
- `cargo lint-windows` — clippy no alvo gnu com `-D warnings`
- `cargo check-macos` — alvo `aarch64-apple-darwin`
- `cargo check-macos-intel` — alvo `x86_64-apple-darwin`
- `cargo lint-macos` — clippy no alvo aarch64 com `-D warnings`
- `cargo check-linux` — alvo `x86_64-unknown-linux-gnu` (v1.0.6)
- O pré-requisito é aditivo e não exige root: `rustup target add <triple>`

### O ponto cego depende do HOST (v1.0.6)
- Esta lista foi escrita num host Linux, onde os alvos descobertos eram macOS e
  Windows.
- Num host macOS ela INVERTE: `cargo check-macos` vira nativo e nada mais
  compila contra Linux, deixando todo item `#[cfg(target_os = "linux")]` sem
  cobertura de `rustc` — inclusive `src/browser/xvfb.rs`, de onde veio o
  `E0432` original.
- Em qualquer host, acrescente o alvo que você NÃO é, e rode o alias dele.

### O limite honesto dos gates cross-platform
- Eles são `cargo check` e `cargo clippy`, que NÃO linkam e NÃO executam.
- Eles não levam `--all-targets`, então cobrem só a lib e o binário.
- Testes, benches e examples ficam cobertos apenas no Linux, por `check-all` e `test-all`.
- Comportamento em tempo de execução no macOS e no Windows, portanto, NÃO é validado por gate algum.

### Cobertura e empacotamento
- `cargo cov` — `llvm-cov --all-features --summary-only`
- `cargo cov-html` — `llvm-cov --all-features --html`
- `cargo pkg-list` — `package --list`, que mostra exatamente o que o tarball leva
- `cargo publish-check` — `publish --dry-run --locked`
- `scripts/portability-lint.sh` — as checagens que um alias de cargo não expressa

## As réguas de contrato

Estes seis arquivos de integração não testam feature. Cada um MEDE uma regra que
o repositório só afirmava em prosa, e regra guardada por nada é regra que deriva.
Os cinco primeiros medem uma regra sobre o CÓDIGO, e o último mede uma regra
sobre os DOCUMENTOS.

### `tests/integration_stdout_boundary.rs`
- Guarda a fronteira do stdout para que NENHUM caminho de emissão não reduzido apareça em silêncio
- A v1.0.4 fechou "flag aceita e ignorada" enumerando as seis superfícies que o plano nomeou, e `--probe` / `--probe-deep` emitiam por um helper próprio que nunca chegava ao projetor
- Medido lá: `--count-only`, `--limit 1`, `--fields status` e `--truncate-content 5` devolveram 633 bytes cada contra baseline de 633 bytes, com exit 0
- O defeito foi o MÉTODO, porque uma lista não consegue reportar o que falta nela mesma
- Este arquivo varre TODA emissão em stdout dentro de `src/` e exige que cada uma fora do módulo de output seja declarada com motivo
- Um bypass novo reprova o build, e uma isenção OBSOLETA também reprova, então a allowlist não apodrece em decoração

### `tests/integration_hardcode_ssot.rs`
- Mede a proibição de hardcode em vez de afirmá-la
- `src/endpoints.rs` se declara a fonte única de verdade da identidade de rede do produto, e essa frase ficou anos guardada por nada
- Cinco call sites de produção tinham derivado de volta para literais crus sem nenhum build reprovar
- `src/extraction/web/mod.rs` casava `"duckduckgo.com/y.js"` duas vezes à mão e comparava contra um `"duckduckgo.com"` pelado
- `types/selectors.rs` semeava o filtro de anúncio padrão com cópia própria, e `zero_cause.rs` carregava cópia própria do fragmento de stealth shell
- O modo de falha de um fato de produto duplicado NÃO é erro de compilação, é divisão silenciosa de comportamento

### `tests/integration_golden_stdout.rs`
- Snapshots golden da FORMA de todo envelope de stdout offline
- Um golden byte a byte do `doctor` mudaria em cada host, porque carrega paths absolutos, contagem viva de processos chrome-like, um SHA de git e a versão do crate
- Sem normalização o ruído vira alerta e o snapshot é aprovado sem ser lido, e snapshot que ninguém lê é pior que nenhum
- Então o snapshot registra cada path JSON do envelope com o tipo da folha, ordenado, e valores aparecem só onde fazem parte do contrato
- Isso pega o que o `assert_conforms` não enxerga: uma chave que o schema nunca declarou e o envelope parou de emitir, ou uma chave opcional que sumiu calada
- Atualize com `cargo insta review`, ou apague o `.snap` e rode de novo, e então LEIA o diff

### `tests/integration_agent_ops_matrix.rs`
- Todo operador agent-native, em toda superfície offline, deve FAZER ou RECUSAR
- "Aceita e ignorada" é invisível para teste de comportamento, porque o envelope é válido e o exit code é 0
- A única diferença observável entre honrada e descartada em silêncio é o TAMANHO do que voltou, e por isso aqui se medem bytes
- Todo número vem da mesma forma de chamada `Command::output()`
- Essa forma evita a armadilha de medição da v1.0.4, onde a baseline lida por pipe manteve o newline final e as variantes lidas por `$(...)` o comeram, forjando redução de um byte em cinco superfícies

### `tests/integration_toolchain_boundary.rs`
- O perfil publicado NÃO pode conter toolchain C, medido a partir do `cargo`
- `NO_CI.md`, `scripts/check-macos.sh` e o bloco de aliases do `.cargo/config.toml` afirmam esse fato em prosa, e três documentos afirmando um fato não são uma medição dele
- Acrescentar uma dependência que puxe `rustls` para o conjunto padrão de features restauraria a exigência de C em silêncio
- O primeiro sintoma seria um gate de macOS ou Windows morrendo com exit 101 num host sem cross compiler
- Ele NÃO afirma NADA sobre o perfil de teste: `cargo test-all` roda `--all-features` e portanto EXIGE toolchain C, o que é limite real e declarado

### `tests/integration_docs_version_ruler.rs`
- Guarda o rótulo de versão para que NENHUM documento declare uma versão CORRENTE obsoleta
- Três testes, medidos verdes em 2026-08-21 com `cargo test --all-features --locked --test integration_docs_version_ruler`
- `no_document_declares_a_stale_current_version` varre todo `.md` e `.txt` da raiz do repositório, de `docs/` e de `skills/`, e reprova quando um documento anuncia como corrente algo diferente de `CARGO_PKG_VERSION`
- `bilingual_pairs_agree_on_the_current_version` reprova quando `X.md` e `X.pt-BR.md` declaram versões diferentes
- `the_version_scan_is_not_vacuous` impede que a régua passe por não estar medindo nada, que é como uma régua muda sobrevive para sempre

Por que ela existe, que é o que impede alguém de apagá-la:
- `tests/integration_docs_drift.rs` tem 20 testes e cobre 22 dos 44 documentos do repositório
- MEDIDO em 2026-08-21: ONZE documentos ainda anunciavam `1.0.5` como a linha corrente, e a lista deles era quase idêntica à lista dos arquivos que ficam FORA daquela régua
- Documento sob régua NÃO apodrece, porque a build quebra quando ele mente
- Documento fora dela apodrece em SILÊNCIO até um humano por acaso lê-lo
- É a MESMA classe do GAP-REL-001: lá existia uma configuração que nenhum gate compilava, aqui um documento que nenhum teste lia, e nas duas vezes tudo estava verde enquanto o usuário recebia algo quebrado

O que ela deliberadamente NÃO faz:
- Ela NÃO policia referência histórica
- `desde a v0.9.8`, `ADR-0027 na v1.0.2` e um título de changelog `## [1.0.4]` são CORRETOS e devem ficar congelados
- Só é medida a frase que ANUNCIA a versão corrente
- `CHANGELOG`, `docs/MIGRATION`, `gaps.md` e `docs/decisions/` ficam excluídos por design, porque registrar o passado É o conteúdo deles

A armadilha medida:
- A primeira versão da régua passou direto por `CONTRIBUTING.md:95`
- Aquela linha anunciava a `v1.0.5` como ainda em vigor enquanto a árvore já estava em `1.0.6`
- A afirmação estava NO MEIO da frase e não no começo da linha, então a varredura nunca a viu
- A formulação usada lá, `still current in`, foi acrescentada à lista de marcadores depois
- Note que esta própria lista precisou ser reescrita para declarar esse fato sem acionar a régua, o que é a demonstração mais barata possível de que ela funciona
- Uma lista de marcadores só é tão boa quanto as formulações que alguém pensou em escrever, então trate a lista como incompleta por padrão

## Notas de Teste v1.0.6

A v1.0.6 fecha dez gaps de release. Cada um foi publicado no crates.io antes de
ser achado, então cada item abaixo nomeia o gate que agora o pega.

- GAP-REL-001 — a versão PUBLICADA não compilava fora do Linux. Nenhum gate jamais perguntou o que o registry de fato serve. Fechado pelo gate pós-publicação documentado abaixo
- GAP-REL-002 — o perfil DISTRIBUÍDO não compilava com `--all-targets`, medidos 91 erros, três deles `E0432`. Fechado pelo alias novo `cargo check-nohttp-all-targets`, que é o perfil publicado INCLUINDO a árvore de testes
- GAP-REL-003 — um argumento `--disable-features` duplicado anulava `AutomationControlled`, então a flag de stealth era passada e não tinha efeito
- GAP-REL-004 — a mitigação de WebRTC estava INVERTIDA, o que reexpunha o IP local que a mitigação existia para esconder
- GAP-REL-005 — um snapshot golden codificava o sistema operacional que o gerou, então o snapshot reprovava em qualquer outro host
- GAP-REL-006 — `completions` entrava em panic em vez de sair `141` com o pipe fechado
- GAP-REL-007 — `config set A B --key C` gravava `C = B` e descartava `A`, guardando a chave errada em silêncio
- GAP-REL-008 — `--no-input` era declarada e nunca lida
- GAP-REL-009 — o CHANGELOG carregava um SEGUNDO título `## [Unreleased]`
- GAP-REL-010 — cortar a SERP por índice de BYTE entrava em panic com texto acentuado

### O gate de verificação pós-publicação (ADR-0032)
- O binário é `src/bin/verify_published.rs`, atrás de `required-features = ["release-gate"]`
- Invoque EXATAMENTE assim: `cargo run --bin verify_published --features release-gate`
- Ele compara o que o crates.io de fato serve contra esta árvore, que é a pergunta que nenhum outro gate faz
- Todo alias do `.cargo/config.toml` responde "a ÁRVORE compila?" e `cargo publish --dry-run` responde "o PACOTE está bem formado?"
- Nenhum dos dois responde à única pergunta que o usuário experimenta, que é se a versão que o registry SERVE compila
- Rode depois de todo `cargo publish` E depois de todo `cargo yank`
- A ORDEM É OBRIGATÓRIA: publique a versão sã ANTES de retirar as quebradas. `max_stable_version` é DERIVADO do estado de yank, então retirar primeiro promove uma versão quebrada mais antiga de volta à resolução
- Foi exatamente assim que a v1.0.2 voltou: a versão CORRIGIDA foi retirada e a v1.0.2 quebrada virou `max_stable_version` em silêncio, sem passar por gate nenhum
- Os exit codes reusam a taxonomia do crate — `0` o registry serve esta versão e nenhuma inferior está viva, `1` divergência, `2` resposta não parseável, `3` bloqueado pelo registry (HTTP 403), `4` timeout
- Racional completo em [ADR-0032](decisions/0032-post-publish-verification-gate-v1-0-6.md)

## Notas de Teste v1.0.2
- Wire JSON serializa em INGLÊS por padrão ([ADR-0027](decisions/0027-wire-en-default-v1-0-2.md)); afirme `.results` / `.metadata` / `.chrome_path_resolved` / `.chrome_channel` / `.used_chrome` no emit padrão; legado PT via `--wire-keys pt` continua coberto
- Underflow de orçamento fail-fast → exit `2` (ADR-0024/0025); cobertura unit/integração de `--print-budget`, `--allow-under-budget`, `--auto-contention-budget`, `budget_profile`
- `FETCH_CAP` padrão `4` (`--fetch-content-cap`); `DEFAULT_PAGES=1`
- Mute-audio sempre ligado (ADR-0026) — sem caminho de unmute; flags de launch do Chrome incluem mute + política de autoplay
- Testes unitários de agent ops: `--fields`/`--select`, `--filter`, `--sort`, `--dedupe-by`, `--limit`, `--count-only`, `--truncate-content`, `--max-output-bytes`
- `doctor --strict` / `doctor --probe-deep` (NÃO existe `doctor --probe`); root `--print-schema` e root `--probe` são pontos de entrada separados
- Defaults do deep-research: `max-sub-queries=3` / `fetch-content-cap=4` (overrides de modo full com caps maiores permanecem válidos quando explícitos)
- Notas de oneshot pipe-safe v1.0.1, one-shot de disco v1.0.0, lifecycle de processo v0.9.6, agent-ready v0.9.8 e Chrome-only v0.9.4 continuam válidas

## Notas de Teste v1.0.1 (Pass 52 / GAP-E2E-51-*)
- `cargo clippy --lib -- -D warnings` limpo (e gates do projeto em direção a zero warnings)
- Stream com fechamento cedo: `duckduckgo-search-cli -q --stream q1 q2 -n 10 | head -n 1` → exit da CLI `141`; órfãos oneshot `0` (`ensure_oneshot_cleanup` + SIG_IGN)
- Config dual: `config get KEY` e `config get --key KEY`; `config set KEY VALUE` e `config set --key KEY --value VALUE`; `config effective` emite JSON mesclado
- `-f ndjson` aceito como alias de stream multi-query (`--stream`)
- Wire: chaves portuguesas na serialização; aliases EN na desserialização (ADR-0023) cobertos por testes lib — padrão de serialize supersedido pelo ADR-0027 / EN na v1.0.2
- Vertical news: anti-bot falso corrigido; residual real do DDG ainda pode exit 6 ambientalmente
- Sem telemetria remota; sem knobs de env de produto para lifecycle/config
- E2E de lifecycle permanece env SOMENTE DE TESTE: `DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle -- --nocapture` (não é env de produto)
- Notas de oneshot pipe-safe v1.0.1, one-shot de disco v1.0.0, lifecycle de processo v0.9.6, agent-ready v0.9.8 e Chrome-only v0.9.4 continuam válidas

## Notas de Teste v0.9.8 (GAP-WS-AGENT-READY-001 / ADR-0018)
- Afirme que a vertical padrão é `all` (envelope web + notícias) salvo `--vertical web`
- Afirme que o fetch de conteúdo está LIGADO por padrão; `--no-fetch-content` não produz corpos `content` (wire EN; legado PT `conteudo` com `--wire-keys pt`)
- Linhas de news podem trazer `content` / `content_size` / `content_extraction_method` com fetch ligado (teto 4 (padrão v1.0.2); nomes PT via `--wire-keys pt`)
- Metadados agent presentes em sucesso/falha/deep: `chrome_path_resolved`, `chrome_channel`, `used_chrome` honesto (não telemetria; wire EN padrão v1.0.2)
- Flags de transporte aceitas após subcomandos (ex.: `deep-research … --chrome-path …`)
- Resolução multi-canal Flatpak coberta por testes unitários de classificação de path / wrapper→ELF
- E2E opcional gated quando Chrome/Chromium Flatpak está instalado: `DUCKDUCKGO_FLATPAK_E2E=1 cargo test -- --nocapture` (dependente do host; pule se ausente)
- Fórmula de preservação 0.9.7 continua verde: `--vertical web --no-fetch-content`
- Notas de oneshot pipe-safe v1.0.1, one-shot de disco v1.0.0, lifecycle de processo v0.9.6 e Chrome-only v0.9.4 continuam válidas

## Notas de Teste v1.0.0 (GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020)
- Gap RESOLVIDO na v1.0.0 — one-shot de disco + prefixo de perfil Chrome auditável (ver `gaps.md`, [ADR-0020](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md))
- E2E de lifecycle continua gated: `DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle -- --nocapture`
- Integração afirma o prefixo de perfil `ddg-chrome-` (não `.tmp` genérico) E o reap de processo (sem órfãos Chromium/Xvfb dessa run)
- Testes unitários cobrem `force_reap` / `sweep_orphan_profiles` / guards de propriedade do prefixo (nunca bulk-delete de `.tmp*` estrangeiro nem `org.chromium.Chromium.*`); o caminho cooperativo também usa `ExitReapGuard` + panic hook (operacional, sem impacto de schema)
- Residual de SIGKILL → sweep da próxima run SOMENTE em `ddg-chrome-*` de propriedade — não higiene em massa de temp de terceiros

## Notas de Teste v0.9.6 (GAP-WS-LIFECYCLE-001)
- Testes unitários cobrem `process_lifecycle` (process group / marker de reap) e `paths::atomic_write`
- E2E gated: `DUCKDUCKGO_LIFECYCLE_E2E=1 cargo test --test integration_browser_lifecycle -- --nocapture`
- O E2E exige Chrome/Chromium (e Xvfb em Linux headless) e afirma que a execução não deixa órfãos Chromium/Xvfb; DESDE a v1.0.0 também afirma o prefixo de perfil `ddg-chrome-` (não `.tmp` genérico) e o reap de disco (GAP-WS-TMP-PROFILE-ORPHAN-001 / ADR-0020)
- Cobertura unitária da higiene de disco: `force_reap` / `sweep_orphan_profiles` / guards de propriedade do prefixo
- SIGTERM (e SIGINT) cancelam o `CancellationToken` compartilhado no caminho de sinais (cancel cooperativo para Docker/`timeout`)
- As notas fail-closed da v0.9.4 (GAP-WS-113) continuam válidas: produção Chrome-only, `NO_CHROME` → exit 2, HTTP residual só sob `http-test-harness`

## Notas de Teste v0.9.4 (GAP-WS-113)
- O caminho de produção é Chrome-only; Chrome ausente / build sem feature `chrome` deve resultar em exit `2` em search/probe/fetch/deep-research (fail-closed). Env de produto `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` REMOVIDA / não lida
- Testes wiremock / SERP HTTP puro exigem `--features http-test-harness` e `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`
- `--allow-lite-fallback` é no-op — testes não devem afirmar sucesso Lite a partir dessa flag
- Builds com `--no-default-features` são offline/unitários apenas; não são caminho de rede de produção
- A auto-degradação do GAP-WS-106 (auto `--no-news` / web-only sem Chrome) foi SUPERSEDIDA pelo fail-closed do GAP-WS-113 (ADR-0016)

## Notas de Teste v0.8.9
- v0.8.9 adiciona `tests/integration_news_vertical.rs` cobrindo a vertical de notícias `--vertical <web|news|all>` (GAP-WS-104)
- v0.8.9 adiciona `tests/integration_deep_research_news.rs` cobrindo o fan-out dual web+news do deep-research (GAP-WS-105): uma sessão Chrome por sub-query roda `--vertical all`, o opt-out `--no-news`, o envelope agregado (`noticias[]`, `quantidade_noticias`, `metadados.total_noticias_unicas`), o RRF news próprio (mantido separado do RRF web), o campo estruturado `news_indisponivel: true` em voo, e o split dual do `--synthesize` ~70/30
- Fixtures HTML novas em `tests/fixtures/`:
  - `ddg_news_serp.html` — SERP da Estratégia A (seletores semânticos do `selectors.toml`; 7 artigos, 1 armadilha interna duckduckgo.com filtrada)
  - `ddg_news_serp_ofuscada.html` — SERP com classes ofuscadas exercitando o fallback classe-agnóstico da Estratégia B
  - `ddg_news_serp_vazia.html` — SERP vazia produzindo `noticias: []` e `causa_zero: vertical-sem-resultados`
- Contrato do modo web validado byte-idêntico à v0.8.8 (sem `noticias`/`quantidade_noticias`/`vertical_usada` emitidos no modo web)


## Notas de Teste v0.8.8
- Contagem de testes: 528 testes (382 unit + 146 integration/doc), 0 falhas
- v0.8.8 adiciona testes de regressão para 12 gaps corrigidos (GAP-WS-089 a GAP-WS-103)
- Truncamento `--num` testado nos paths Chrome headed e batch (GAP-WS-090, GAP-WS-094)
- Cobertura `fill_compat_fields()` para campos compat de metadata (GAP-WS-092, GAP-WS-093, GAP-WS-097)
- `ZeroResultsSuspeito` exit code 6 validado (GAP-WS-099)
- `tamanho_conteudo` reflete tamanho do texto truncado (GAP-WS-100)
- Limpeza de lock stale do Xvfb via `is_lock_stale()` verificação de PID (GAP-WS-089)


## Notas de Teste v0.8.7
- Testes E2E requerem Google Chrome ou Chromium instalado
- Linux: Xvfb é auto-instalado pela CLI em runtime via `try_auto_install_xvfb()`. Para pré-instalar à mão: `sudo apt-get install -y xvfb`
- macOS/Windows: sem dependência extra — Chrome roda em headless=new desde a v0.9.3 (Linux mantém Xvfb privado)
- Testar sem Chrome (offline/unitário apenas; não é produção): `cargo test --no-default-features`
- Forçar headless: passe a flag CLI `--chrome-headless` (env de produto `DUCKDUCKGO_CHROME_HEADLESS` REMOVIDA)
- Contagem na release v0.8.7: 548 testes (382 unit + integration + doc), 0 falhas
- Schema JSON deep-research: `.results[].title` sob o wire EN padrão (`.resultados[].titulo` só com `--wire-keys pt`), campo `.query` top-level disponível


## Adições de Testes em v0.7.3

A release v0.7.3 adicionou 13 testes, todos endereçando o GAP-WS-27 (CAPTCHA no macOS) e seus três fatores de causa raiz:

- `session_warmup` (5 testes unitários) — resolução de path XDG no Linux, macOS e Windows; criação de diretório ausente; override de path via `DUCKDUCKGO_SEARCH_CLI_HOME` (HISTÓRICO — veja a nota abaixo); estabilidade da constante `DEFAULT_COOKIES_FILENAME`.
  - `DUCKDUCKGO_SEARCH_CLI_HOME` NÃO é configuração de produto. Medido em
    2026-08-21 com `rg -n -F "DUCKDUCKGO_SEARCH_CLI_HOME" src/ tests/`: o único
    hit em `src/` é o doc comment em `src/platform.rs:172`, que afirma que o path
    de runtime NÃO é sobrescrito por ela, e há ZERO hit em `tests/`. Nada a lê —
    nem produção, nem o harness de teste atual.
  - O override suportado é a flag de CLI `--config-home <PATH>`, como o
    [AGENTS.pt-BR.md](AGENTS.pt-BR.md) já declara. NUNCA defina essa variável
    esperando que o produto a honre.
- `cookie_adapter` (3 testes unitários, renomeado de `wreq_cookie_adapter` na v0.8.6) — `PersistentJar::empty()` produz um `Arc<reqwest::cookie::Jar>` válido; roundtrip `parse_json` preserva cookies via extração do header `CookieStore::cookies()`; roundtrip `save`/`load` com permissões Unix `0o600` e semântica de escrita atômica.
- `probe_deep` (5 testes unitários) — `detect_interstitial` identifica corretamente os marcadores do Cloudflare (`cf-chl-bypass`, `cf-challenge`, `challenge-platform`, `Attention Required`, `__cf_chl_jschl_tk__`); `detect_interstitial` identifica corretamente os marcadores `robot-detected` e `bots, we have detected` do DuckDuckGo; `mitigation_suggestion` retorna passos concretos para cada tipo de interstitial; `InterstitialKind::None` é o default para uma resposta HTML normal; `execute_probe_deep` produz um JSON report válido.
- Total: 405 testes lib passando (era 279 em v0.7.2; total atual do projeto na v0.7.5). As mudanças v0.7.3 são puramente aditivas. Nenhum teste removido, nenhuma assinatura de teste alterada, nenhuma fixture renomeada.

### Gaps v0.7.3 fechados por estes testes
- `probe_deep::detect_interstitial` — valida que os marcadores são detectados (o custo de um falso negativo é um CAPTCHA não diagnosticado). Cinco marcadores do Cloudflare + dois do DuckDuckGo são testados em isolamento.
- `cookie_adapter::PersistentJar` — valida que a ponte JSON ↔ `reqwest::cookie::Jar` não perde cookies durante roundtrip (reescrito na v0.8.6 para usar extração de header `CookieStore::cookies()`). Uma regressão aqui silenciosamente descartaria cookies de sessão, reintroduzindo o GAP-WS-27.
- `session_warmup::default_cookies_path` — valida que a resolução XDG está correta por plataforma. Uma regressão aqui colocaria o cookie jar no diretório errado ou falharia em setar permissões `0o600` no Unix.


## Adições de Testes em v0.7.4

> HISTÓRICO — removido na v0.8.6. Toda a pilha de preflight do `build.rs`
> descrita abaixo deixou de existir quando o `wreq` foi trocado por `reqwest` +
> `rustls-tls`. O escape hatch `DDG_SKIP_NASM_CHECK` NÃO é variável de ambiente
> corrente e defini-la NÃO faz NADA. Leia esta seção como registro do que a
> v0.7.4 publicou, NUNCA como configuração atual.

v0.7.4 adiciona testes em tempo de build que validam o preflight do build.rs para detecção do assembler NASM em builds nativos Windows MSVC.

- `build::preflight::nasm` — 4 testes unitários validando:
  - `nasm_in_path` retorna `true` quando nasm.exe está no PATH
  - `nasm_in_path` retorna `false` quando nasm.exe está ausente
  - `known_nasm_dir` retorna `Some` para `C:\Program Files\NASM` e `C:\Program Files (x86)\NASM`
  - `known_nasm_dir` retorna `None` para caminhos desconhecidos
- GAP-WS-28 fechado por estes testes — a mensagem de panic, comando de fix e escape hatch DDG_SKIP_NASM_CHECK=1 são todos validados end-to-end no script de build.
- Contagem de testes: ~395 testes lib passando (era 292 na v0.7.3 = +3-5 novos testes de preflight de build).

### Gaps v0.7.4 fechados por estes testes
- `build::preflight::nasm_in_path` — valida a lógica de scan para nasm.exe no PATH. Uma regressão aqui faria o preflight v0.7.4+ ou falso-positivo (panic quando NASM está instalado) ou falso-negativo (deixa o build prosseguir para o erro críptico do CMake).
- `build::preflight::known_nasm_dir` — valida a heurística de detecção de NASM-instalado-mas-PATH-obsoleto. Uma regressão perderia a dica acionável de que o usuário só precisa atualizar o PATH.

## Adições de Testes em v0.7.5

> HISTÓRICO — removido na v0.8.6. Os quatro preflights (NASM, CMake, MSVC,
> Strawberry Perl), os quatro escape hatches `DDG_SKIP_*_CHECK=1`, a detecção
> `perl_in_path` e o helper `install-windows.ps1` estão TODOS fora da árvore
> atual. Nenhuma dessas quatro variáveis é lida hoje, e o Strawberry Perl NÃO é
> dependência do perfil publicado — esse perfil é 100% Rust, medido por
> `tests/integration_toolchain_boundary.rs`. Leia esta seção como registro,
> NUNCA como instrução de instalação corrente.

v0.7.5 estende o preflight de build para detectar 4 ferramentas (NASM, CMake 3.20+, MSVC C/C++, Strawberry Perl) e adiciona testes para os scripts auxiliares.

- `build::preflight::cmake` — 3 testes unitários validando heurísticas de cmake_in_path e known_cmake_dir.
- `build::preflight::msvc` — 2 testes unitários validando detecção de cl_in_path e link_in_path.
- `build::preflight::perl` — 3 testes unitários validando heurísticas de perl_in_path e known_perl_dir.
- `scripts::check_windows_toolchain` — 4 testes de integração validando schema de saída JSON e booleano all_present para várias combinações de ferramentas.
- `scripts::install_windows` — 1 teste de integração smoke-validando que o modo install-windows.ps1 --check-only emite um relatório parseável.
- GAP-WS-29/30/31 fechados por estes testes — cada um dos 4 caminhos de panic do preflight é testado em isolamento, e os 4 escape hatches DDG_SKIP_*_CHECK=1 são validados.
- Contagem de testes: 405 testes lib passando (era ~395 na v0.7.4 = +8-13 novos testes de preflight de build + script). Este é o total atual do projeto na v0.7.5.
- Cross-platform: rode localmente `cargo test --all-targets --all-features` (Windows/Linux/macOS). SEM GitHub Actions / job Windows host.

### Gaps v0.7.5 fechados por estes testes
- `build::preflight::cmake_in_path` — valida o scan de cmake.exe no PATH. Uma regressão deixaria o build v0.7.5+ prosseguir para o panic críptico failed to execute command: program not found do crate cmake.
- `build::preflight::cl_in_path` e `link_in_path` — valida detecção de compilador/linker MSVC. Ambos devem estar presentes; detecção parcial é tratada como ausente.
- `build::preflight::perl_in_path` — valida detecção do interpretador Perl. Strawberry Perl é o Perl Windows de fato; o teste usa o padrão de filename perl.exe.
- `scripts::check_windows_toolchain::json_output` — valida que a saída JSON do script de diagnóstico é parseável e contém as 7 entradas de ferramenta esperadas com booleano found e campos string path.
- `scripts::install_windows::check_only_mode` — valida que a flag --check-only produz um relatório sem tentar instalar nada, adequado para portões locais.

## Adições de Testes em v0.7.0

A release v0.7.0 adicionou testes nos quatro módulos novos, todos endereçando gaps até então em aberto:

- Doctests (12 testes) — acrescentados a `aggregation.rs`, `synthesis.rs`,
  `decomposition.rs` e `deep_research.rs`. Eles servem como documentação
  executável: cada módulo exporta ao menos um exemplo `no_run`.
- Testes property-based (7 testes, `proptest`) — `aggregation::canonicalize_url`
  é checado quanto a idempotência, remoção de fragmento, remoção de parâmetro de
  rastreio e invariante de host em minúsculas. `synthesis::estimate_tokens` é
  checado quanto a monotonicidade, e `synthesis::trim_to_budget` é checado tanto
  no teto quanto na invariante de idempotência. As regressões do proptest são
  escritas em `proptest-regressions/`, que está capturado no `.gitignore`.
- Testes de integração wiremock (17 testes, `tests/integration_deep_research.rs`)
  — smoke do pipeline, casamento de query-param, observabilidade da anomalia
  HTTP 202, observabilidade do HTTP 404 e 13 testes de cobertura de superfície
  que exercitam a API pública de cada módulo novo.
- Segurança de cancelamento (1 teste) — `decompose_respects_cancellation`
  valida que o decompositor heurístico retorna cedo quando seu
  `CancellationToken` é cancelado.
- Tratamento manual de arquivo (3 testes) — pulo de linha em branco e de
  comentário `#`, rejeição de arquivo só com comentários e rejeição de path
  ausente.
- Total: 392 testes passando (279 lib + 12 doc + 101 integração). As
  mudanças da v0.7.0 são puramente aditivas. Nenhum teste removido, nenhuma
  assinatura de teste alterada, nenhuma fixture renomeada.

### Gaps v0.7.0 fechados por estes testes
- Panic latente de UTF-8 em `synthesis::trim_to_budget` — usava indexação
  por byte sem checar fronteira de caractere. O proptest pegou o panic numa
  entrada multi-byte, o fix usa `floor_char_boundary`, e três testes de
  regressão agora trancam a invariante `is_char_boundary(out.len())`.
- Casos de borda vazio / um token / max zero em `decomposition.rs`.
- Segurança de cancelamento de `run_deep_research` — valida que o pipeline
  desiste antes de abrir N sub-queries quando o operador aperta `Ctrl+C`.

## Adições de Testes em v0.6.5

A release v0.6.5 adicionou 11 testes, todos endereçando gaps anteriormente em aberto:

- WS-11 (5 testes) — invariantes property-based para o parser HTML em
  `extraction.rs`. Valida que inputs vazios retornam `Vec` vazio, positions
  são densos e 1-based, URLs são normalizados para paths absolutos, o parser
  é determinístico, e HTML malformado não causa panic.
- WS-12 (4 testes) — circuit breaker per-host em `content_fetch.rs`.
  Valida que o estado closed permite requisições, o threshold abre o breaker,
  um único sucesso reseta o contador de falhas, e o estado half-open é
  alcançável após a janela de cooldown.
- WS-23 (1 teste) — teste de integração wiremock para o header
  `Retry-After` em respostas HTTP 429.

### Gaps v0.6.5 fechados por estes testes
- MP-26 (Windows HANDLE) — validado por `cargo test --all-features` rodado
  À MÃO num host Windows. NÃO existe job de Windows em lugar algum; veja o
  limite cross-platform no topo deste guia.
- CI-01 (6 erros de clippy) — `cargo clippy --all-targets --all-features -- -D warnings`
  agora passa.
- WS-12 (circuit breaker) — coberto por 4 testes unitários em
  `src/content_fetch.rs`. (HISTÓRICO — é o layout da v0.6.5. Hoje aquilo é um
  diretório, `src/content_fetch/`; o caminho fica como registro de onde os
  testes viviam na v0.6.5.)
- WS-23 (Retry-After) — coberto por 1 teste wiremock em
  `tests/integration_wiremock.rs`.

## Por Que Testes Categorizados

A suíte é dividida em quatro categorias para equilibrar velocidade, isolamento
e cobertura:

| Categoria      | Velocidade | Isolamento  | I/O real  | Contagem (v1.0.6) |
|----------------|------------|-------------|-----------|-------------------|
| Unitário       | < 1 s      | por função  | nenhum    | 806               |
| Integração     | < 30 s     | por teste   | localhost | 304               |
| Doc            | < 5 s      | por doc     | nenhum    | 12                |
| Loom           | n/a        | n/a         | n/a       | 0 (gated)         |

Esta tabela é CORRENTE e não histórica, então ela traz números medidos na
v1.0.6, e não os da v0.7.5 que constavam antes. Medido em 2026-08-21 por
execução, nunca por estimativa:

- `cargo test --all-features --locked --lib -- --list` — `806` testes unitários
- `cargo test --all-features --locked --tests -- --list` — `1110`, que inclui o
  alvo unittests, logo a integração são `304`
- `cargo test --doc --all-features --locked -- --list` — `12` doctests, 0 benchmarks
- A linha `Doc` dizia `0`, o que contradizia a linha do `cargo test-all` no topo
  deste guia; `cargo test-all` (`test --all-features --locked`) roda os doctests
- `806 + 304 + 12` fecha em `1122`, exatamente o que
  `cargo test --all-features --locked -- --list` devolve
- As contagens dentro das seções históricas por versão NÃO foram tocadas, porque
  cada uma descreve o que era verdade naquela versão

## Categorias de Teste

### Testes Unitários
Ficam nos módulos `src//tests` (mod tests). Rápidos, in-process, sem I/O.
Rode com:

```bash
cargo test --lib
```

### Testes de Integração
Ficam nos arquivos `tests/*.rs`. Usam wiremock (sem HTTP real), assert_cmd (sem
spawn real de subprocesso) e tempfile (sem escrita real fora do tmpdir).

```bash
# Todos os testes de integração
cargo test --tests

# Um único arquivo de teste de integração
cargo test --test integration_wiremock
```

### Doctests
Ficam nos exemplos `///` espalhados por `src/`. Compilados e executados por `cargo test --doc`.

```bash
cargo test --doc
```

### Testes Loom
Ficam em `tests/loom_atomics.rs`. Gated por `--cfg loom`. NÃO são compilados por
padrão — exigem opt-in explícito.

```bash
RUSTFLAGS="--cfg loom" cargo test --test loom_atomics --release
```

> Limitação conhecida: o Loom conflita com `hyper-util` e hoje compila mas
> não roda limpo. Issue acompanhada upstream.


## Como Rodar

### Desenvolvimento Local

```bash
# Ciclo rápido de feedback
timeout 300 cargo test --all-features --locked

# Categoria específica
cargo test --lib --locked
cargo test --tests --locked
cargo test --doc --locked
```

### Com Cobertura

```bash
# Instalar o cargo-llvm-cov
cargo install cargo-llvm-cov

# Rodar com relatório HTML
cargo llvm-cov --all-features --locked --html --open

# Rodar só com sumário em texto
cargo llvm-cov --all-features --locked --summary-only
```

Cobertura mínima de linhas: `80%`. A validação local deve reprovar abaixo desse limiar.

### Testes Property-Based (v0.6.5, WS-11)

5 invariantes em `src/extraction.rs`. (HISTÓRICO — é o layout da v0.6.5. Hoje
aquilo é um diretório, `src/extraction/`; o caminho fica como registro de onde
os invariantes viviam na v0.6.5.)

```bash
cargo test ws11_
# Roda os 5 testes de propriedade:
# - ws11_invariant_empty_inputs_yield_empty_results
# - ws11_invariant_positions_are_dense_and_one_based
# - ws11_invariant_urls_are_normalized_to_absolute
# - ws11_invariant_extraction_is_idempotent
# - ws11_invariant_malformed_html_does_not_panic
```

### Teste WireMock de Retry-After (v0.6.5, WS-23)

```bash
cargo test --test integration_wiremock test_retry_after_header_respected
```

### Testes de Circuit Breaker (v0.6.5, WS-12)

```bash
cargo test ws12_
# Testes: ws12_breaker_allows_when_closed,
#        ws12_breaker_opens_after_threshold_failures,
#        ws12_breaker_resets_on_success,
#        ws12_breaker_half_opens_after_cooldown
```


## Variáveis de Ambiente

| Variável                        | Efeito                                                |
|---------------------------------|-------------------------------------------------------|
| `RUST_TEST_THREADS`             | Número de threads paralelas de teste (padrão 1)        |
| `RUST_BACKTRACE`                | Defina `1` ou `full` para backtraces detalhados        |
| Filtro de log de produto        | CLI `-v`/`-q` + XDG `log_directive` apenas (não `RUST_LOG` de produto) |
| `CARGO_TERM_COLOR`              | Força cores ANSI (`always`, `never`, `auto`)          |
| `LOOM_MAX_PREEMPTIONS`          | Limite máximo de preempções nos testes loom            |
| `WIREMOCK_LOG`                  | Log de request/response do WireMock                    |
| `DUCKDUCKGO_LIFECYCLE_E2E`      | Env SOMENTE DE TESTE (não de produto). Defina `1` para rodar o E2E de lifecycle do browser (`tests/integration_browser_lifecycle.rs`; exige Chrome/Chromium, e Xvfb em Linux headless; v1.0.0+ afirma prefixo `ddg-chrome-` + reap processo/disco; v1.0.1 também stream`|head` → 141 + órfãos 0 — ADR-0020 / Pass 52) |


## Perfis de Validação Local

Três passagens de validação rodam a suíte. Cada uma é um comando que VOCÊ roda à
mão, num host por vez. NÃO existe matriz e NÃO existe scheduler.

1. `validate` — `cargo test --all-features --locked`, rodado no host em que
   você está sentado. Cobrir Linux, macOS e Windows significa rodar uma vez por
   host; nada roda por você, então host não visitado é host sem cobertura
2. `msrv` — `cargo check --all-targets --all-features --locked` no Rust 1.88 (MSRV desde v0.7.2)
3. `coverage` — `cargo llvm-cov --all-features --locked --fail-under-lines 80` no Linux

Mais um perfil manual de `cargo nextest` disponível localmente:

```toml
# .config/nextest.toml (fora do repo, por convenção do projeto)
[profile.default]
retries = 2
test-threads = 1
```


## Solução de Problemas

### Falhas em `flaky::lazy_template`
Testes loom podem ser instáveis. Rode de novo com:

```bash
RUSTFLAGS="--cfg loom" cargo test --test loom_atomics --release -- --test-threads=1
```

### Timeout de inicialização do `wiremock::MockServer`
Aumente a espera:

```bash
WIREMOCK_LOG=info cargo test --test integration_wiremock
```

### A cobertura cai abaixo de 80%
Confira no relatório HTML quais linhas ficaram descobertas:

```bash
cargo llvm-cov --html --open
```

O diff mostra quais linhas a suíte não exercita. Acrescente testes unitários ou
de integração para cobrir os ramos que faltam.

### Os testes passam num host e falham em outro
- Procure comportamento dependente de ambiente (paths, timeouts, locale)
- Procure não determinismo de `Instant::now()` no código sob teste
- Use `cargo nextest` com retentativas para detectar testes instáveis:

```bash
cargo nextest run --retries 3
```


## Adições de Testes em v0.7.6

A v0.7.6 fecha o GAP-WS-48 (fix de mesmo dia do `cargo install`) e adiciona testes de regressão para o conflito de dependência.

- `build::install::alloc_no_stdlib_pin` — 2 testes unitários validando que o pin `alloc-no-stdlib = "2.0.4"` é respeitado no `cargo install` e não sofre upgrade silencioso para 3.0.0.
- `build::install::brotli_decompressor_pin` — 1 teste unitário validando que o pin `brotli-decompressor = "5.0.1"` sobrevive à resolução em toolchain limpa.
- `integration::install_clean_toolchain` — 1 teste de integração que roda `cargo install --path . --offline` em um `target/` novo e asserta exit 0.
- GAP-WS-48 fechado por estes testes — cada pin de dependência que o fix da v0.7.6 depende tem um teste dedicado.
- Contagem de testes: 408 testes lib passando (era 405 na v0.7.5 = +3 novos testes de pin de install). Este é o total do projeto na v0.7.6.
- Gate local: os novos testes de install rodam na passagem de validação local `install-check` (histórico; CI removido) junto com os testes de preflight da v0.7.5.

### Gaps v0.7.6 fechados por estes testes
- `build::install::alloc_no_stdlib_pin` — previne o conflito `2.0.4` vs `3.0.0` de reaparecer silenciosamente. Uma regressão re-dispararia o panic original do `cargo install`.
- `build::install::brotli_decompressor_pin` — mantém o decoder brotli do BoringSSL fixado em versão conhecida como boa. Uma regressão quebraria o build do source no Linux.
- `integration::install_clean_toolchain` — portão de install end-to-end que captura qualquer novo conflito de dependência antes da publicação.


## Adições de Testes em v0.7.7

> v0.8.6+: Os testes `tls::emulation` abaixo foram REMOVIDOS quando `wreq` foi substituído por `reqwest` + `rustls-tls`. Ver ADR-0008. Os testes de preflight de build em v0.7.4–v0.7.5 (NASM, CMake, MSVC, Perl) também foram removidos pois os preflights não existem mais no `build.rs`.

A v0.7.7 fecha o GAP-WS-49 (regressão de fingerprint TLS) e adiciona testes de regressão para o stack `wreq` + `wreq-util` emulation. (Histórico — testes removidos na v0.8.6.)

- `tls::emulation::wreq_util_present` — 2 testes unitários validando que `wreq-util 3.0.0-rc` com `features = ["emulation"]` está na árvore de dependências resolvida. (Removido na v0.8.6.)
- `tls::emulation::brotli_feature_enabled` — 1 teste unitário validando que a feature `brotli` do `wreq` está habilitada (necessária para o stack de emulation compilar). (Removido na v0.8.6.)
- `tls::probe_deep::captcha_classification` — 1 teste de integração que roda `--probe-deep` contra endpoint real do DuckDuckGo e asserta que o envelope JSON contém `status`, `cascade_reason` e `mitigation_suggestion`.
- `tls::probe_deep::ok_envelope` — 1 teste de integração que asserta que o envelope de sucesso bate com o schema documentado em `docs/HOW_TO_USE.pt-BR.md`.
- GAP-WS-49 fechado por estes testes — o stack de emulation é trancado no nível de dependência e validado end-to-end.
- Contagem de testes: 413 testes lib + integration passando (era 408 na v0.7.6 = +5 novos testes de re-registro TLS). Este é o total do projeto na v0.7.7.
- Gate local: os testes TLS rodavam na passagem de validação local `tls-emulation` (histórico; CI removido) em v0.7.7–v0.8.5. (Removido na v0.8.6 — wreq eliminado.)

### Gaps v0.7.7 fechados por estes testes (histórico — substituído pela v0.8.6)
- `tls::emulation::wreq_util_present` — prevenia outra remoção acidental de `wreq-util`. (Substituído: wreq-util removido na v0.8.6.)
- `tls::emulation::brotli_feature_enabled` — mantinha a feature `brotli` no grafo de build. (Substituído: brotli removido na v0.8.6.)
- `tls::probe_deep::captcha_classification` — valida o formato do gate local para `--probe-deep`. Uma regressão deixaria o gate retornar exit 0 em resposta de captcha.
- `tls::probe_deep::ok_envelope` — valida o JSON do caminho de sucesso. Uma regressão quebraria consumidores de agente downstream que parseiam o envelope.


## Adições de Testes em v0.7.8

A v0.7.8 fecha 8 gaps (GAP-WS-50 até GAP-WS-57) e adiciona testes de regressão para cada. A renovação do detector é o maior delta.

- `probe_deep::markers::cloudflare` — 4 testes unitários validando os 4 markers novos do Cloudflare (`anomaly-modal`, `anomaly.js`, `botnet`, `Unfortunately, bots`) contra fixtures HTML reais em `tests/fixtures/`.
- `probe_deep::markers::ddg` — 1 teste unitário validando o novo marker `anomaly-modal__title` do DDG.
- `probe_deep::markers::legacy` — 3 testes unitários validando que markers legados (`cf-chl-bypass`, `cf-challenge`, `robot-detected`) ainda casam.
- `cli::verbose::count_levels` — 1 teste unitário validando que `-v` (1), `-vv` (2), `-vvv` (3) parseiam corretamente via `ArgAction::Count`.
- `cli::verbose::conflicts_with_quiet` — 1 teste unitário validando que `--verbose` e `--quiet` juntos falham a validação do clap.
- `search_retry::retries_honored` — 1 teste de integração em `tests/integration_search_retry.rs` validando que `--retries 5` produz `metadados.retentativas == 5` no JSON.
- `search_retry::clamp_to_ten` — 1 teste de integração validando que `--retries 999` é clampado para 10 com aviso.
- `search::fallback_lite_opt_in` *(histórico, v0.7.8–v0.9.3)* — 2 testes unitários validando que `--allow-lite-fallback` não aciona quando o usuário não passou o flag. Desde a v0.9.4 a flag é no-op (GAP-WS-113).
- `search::fallback_lite_with_interstitial` *(histórico, v0.7.8–v0.9.3)* — 2 testes unitários validando que o fallback aciona quando o detector classifica interstitial e o flag está on. Lite não é caminho de sucesso em produção desde a v0.9.4.
- Contagem de testes: 305 lib + 18 testes de integration passando (era 292 lib + 13 integration na v0.7.7 = +10 novos testes v0.7.8). Este é o total do projeto na v0.7.8.
- Gate local: os testes de marker rodam na passagem de validação local `detector-markers` (histórico; CI removido); os testes de retry rodam na passagem de validação local `retry-pipeline` (histórico; CI removido).

### Gaps v0.7.8 fechados por estes testes
- `probe_deep::markers::cloudflare` e `ddg` — trancam a lista de markers pós-2026. Uma regressão ao detector só-de-legacy re-abriria o GAP-WS-50.
- `cli::verbose::count_levels` — tranca a semântica de `ArgAction::Count`. Uma regressão ao `verbose: bool` único re-abriria o GAP-WS-53.
- `cli::verbose::conflicts_with_quiet` — previne a combinação contraditória de flags. Uma regressão deixaria operadores se frustrarem.
- `search_retry::retries_honored` — tranca a propagação de `cfg.retries`. Uma regressão ao `1` hard-coded re-abriria o GAP-WS-57.
- `search_retry::clamp_to_ten` — tranca o clamp `[1, 10]`. Uma regressão deixaria `--retries 999` acionar detecção anti-bot.
- `search::fallback_lite_opt_in` *(histórico)* — trancava o contrato de opt-in Lite em v0.7.8–v0.9.3. Supersedido pela v0.9.4 / GAP-WS-113: `--allow-lite-fallback` é no-op legado; testes não devem afirmar sucesso Lite a partir dessa flag.
- `search::fallback_lite_with_interstitial` *(histórico)* — trancava o predicado `detect_interstitial` do caminho Lite antigo. Supersedido pela produção Chrome-only (ADR-0016).


## Testes Chrome Stealth (v0.8.0, atualizado v0.8.7)
- Testes stealth do Chrome requerem Xvfb em Linux headless (v0.8.7+ auto-instala em 22+ distros)
- Execute com: `cargo test` (v0.8.7+ auto-spawna Xvfb privado; fallback manual: `xvfb-run --auto-servernum cargo test`)
- macOS/Windows: testes stealth rodam em headless=new desde v0.9.3 (sem Xvfb necessário)
- `tests/integration_stealth_block_classification.rs` valida injeção de sinais
  stealth e classificação de bloqueio. O nome `tests/integration_chrome_stealth.rs`
  constava aqui até a v1.0.6 e NUNCA existiu em disco; medido em 2026-08-21 com
  `fd -e rs . tests/`
- `tests/integration_deep_research.rs` valida pipeline Chrome no deep-research
- Testes unitários em `src/browser/tests.rs` validam argumentos de `flags_stealth()`
- CONTAGEM HISTÓRICA desta seção (v0.8.0, atualizada na v0.8.7): 378 testes
  passavam com a feature Chrome habilitada. NÃO é a suíte corrente; os números
  atuais estão na tabela de `Por Que Testes Categorizados`
- Para pular testes Chrome: `cargo test --no-default-features`
