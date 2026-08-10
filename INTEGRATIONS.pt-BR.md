# Integrações
Leia em [English](INTEGRATIONS.md).

`duckduckgo-search-cli` se integra com mais de 16 agentes de IA e plataformas de automação via contrato JSON estável, exit codes determinísticos e binário sem dependências. Este arquivo é um ponteiro para o catálogo completo de integrações.

## Catálogo Completo
Veja [`docs/INTEGRATIONS.pt-BR.md`](docs/INTEGRATIONS.pt-BR.md) para o guia completo de integrações, incluindo:

- 16 agentes de IA suportados (Claude, GPT, Gemini, Cursor, OpenCode, etc.)
- Aliases de flags introduzidas em cada versão
- Tabela resumo consolidando todas as integrações
- Receitas de instalação por plataforma
- Semântica de exit codes para tomada de decisão dos agentes
- Snippets por integração com `timeout`, `jaq` e `PIPESTATUS`

## Referência Rápida
```bash
# Invocação canônica (v1.0.5 — chaves wire em inglês por padrão)
timeout 180 duckduckgo-search-cli -q -f json --num 15 "query"

# Exit codes
0  sucesso              → parse .results
1  erro de runtime      → leia stderr; tente novamente com -v
2  erro de configuração → reexecute init-config --force; TAMBÉM recusa de agent ops (ver v1.0.4/v1.0.5)
3  bloqueio anti-bot    → aguarde 300+ s; Chrome + `--proxy` / rotacione identidade (NÃO lite / NÃO `--allow-lite-fallback`)
4  timeout global       → aumente --global-timeout; reduza --parallel
5  zero resultados      → refine a query ou tente --lang diferente
6  bloqueio suspeito    → inspecionar .metadata.zero_cause; aguardar 300+ s ou rotacionar proxy

# Inventário completo de comandos (v1.0.5)
# Busca padrão (sem subcomando):
duckduckgo-search-cli [OPTIONS] [QUERY]...
duckduckgo-search-cli -q -f json "query"                    # busca padrão
# Alias oculto (equivalente à busca padrão; omitido de --help):
duckduckgo-search-cli buscar -q -f json "query"
# Subcomandos:
duckduckgo-search-cli init-config                           # grava selectors.toml + user-agents.toml no XDG
duckduckgo-search-cli init-config --dry-run                 # simula sem gravar em disco
duckduckgo-search-cli init-config --force                   # sobrescreve arquivos existentes
duckduckgo-search-cli completions bash                      # bash|zsh|fish|powershell|elvish
duckduckgo-search-cli deep-research --print-budget -q       # dry-run de budget (sem Chrome)
duckduckgo-search-cli deep-research "query" -q -f json      # fan-out + agregação
duckduckgo-search-cli commands -q                           # árvore de comandos em JSON (descoberta do agente)
duckduckgo-search-cli schema                                # lista IDs de JSON Schema
duckduckgo-search-cli schema --name search-output
duckduckgo-search-cli --print-schema                        # alias raiz do catálogo schema
duckduckgo-search-cli --probe -q -f json                    # pre-flight raiz (separado de doctor)
duckduckgo-search-cli --probe-deep -q -f json               # pre-flight profundo (classificação de captcha)
duckduckgo-search-cli doctor -q                             # diagnóstico de ambiente / Chrome em JSON
duckduckgo-search-cli doctor --strict
duckduckgo-search-cli doctor --probe-deep
duckduckgo-search-cli locale -q                             # locale de UI resolvido em JSON
duckduckgo-search-cli man | man -l -                        # página man (roff) a partir da árvore clap
duckduckgo-search-cli man --file /tmp/ddg.1
# CRUD de config (RuntimeConfig SSOT — CLI > XDG > FACTORY; sem env de produto):
duckduckgo-search-cli config path
duckduckgo-search-cli config list
duckduckgo-search-cli config get wire_keys
duckduckgo-search-cli config set wire_keys en
duckduckgo-search-cli config set budget_profile lab
duckduckgo-search-cli config unset proxy_url
duckduckgo-search-cli config effective
duckduckgo-search-cli help deep-research

# Agent ops (globais; toda superfície desde a v1.0.4; sem jq):
#   --fields / --select, --filter, --sort, --dedupe-by, --limit,
#   --count-only, --truncate-content, --max-output-bytes, --wire-keys en|pt
# Desde a v1.0.4 cada operador AGE ou RECUSA por nome; não existe terceiro resultado.
# Desde a v1.0.5 --probe e --probe-deep também os honram.
duckduckgo-search-cli "query" -q -f json \
  --fields url,title --filter 'title~rust' --sort title --limit 5
duckduckgo-search-cli "query" -q -f json --count-only
duckduckgo-search-cli "query" -q -f json --wire-keys pt     # serialize PT legado
duckduckgo-search-cli commands -q -f json --fields agent_ops
duckduckgo-search-cli --probe -q -f json --fields status

# Wire PT legado (agentes pré-1.0.2):
#   --wire-keys pt   OU   config set wire_keys pt

Versão atual: 1.0.5
```

## Mudança que Quebra Integração — `discriminator` → `discriminator_key`
- Renomeie o campo que você lê da matriz de capacidades publicada por `commands` sob `agent_ops`.
- Leia `agent_ops[].discriminator_key` na v1.0.5 e adiante.
- Pare de ler `agent_ops[].discriminator`, que o envelope não emite mais.
- Espere a mudança de significado junto com a de nome: o slot agora carrega a CHAVE, que é sempre `type`.
- Saiba que a v1.0.4 publicava o VALOR (`doctor`, `schema_catalog`, `config_list`) sob um campo nomeado como chave.
- Entenda o defeito: um agente que confiava na matriz antiga procurava uma chave chamada `doctor`.
- Migre lendo o nome da chave em `discriminator_key` e comparando-o com o campo `type` do envelope recebido.
- Verifique a migração com `duckduckgo-search-cli commands -q -f json --fields agent_ops`.
- Trate esta como a única renomeação observável de envelope na v1.0.5.
- Note que o slot `discriminator` sobrevive nas linhas do catálogo `schema`, onde continua nomeando o VALOR de `type` que o schema descreve.

## Destaques v1.0.5 para Integrações
- **`--probe` e `--probe-deep` honram os operadores agent-native** — as duas superfícies passam agora pelo projetor em vez do helper de bypass `emit_probe_payload`.
- **Medido na v1.0.4** — `--count-only`, `--limit 1`, `--fields status` e `--truncate-content 5` devolviam cada um **633 bytes contra uma baseline de 633 bytes, no exit 0**, justamente na superfície de health check que o agente consulta primeiro.
- **Recusas do probe chegam ao chamador** — `emit_probe` retorna o exit code em vez de ser descartado por `let _ =`, então a recusa não é mais engolida como o no-op antigo.
- **As chaves de wire continuam valendo no probe** — `KeyPolicy::ProcessWire` roda a redução sobre o documento em inglês primeiro e mapeia as chaves por último, então os caminhos de `--fields` significam a mesma coisa nos dois idiomas.
- **Identificadores são isentos de `--truncate-content`** — uma string que o agente devolve para um programa é IDENTIDADE, não conteúdo.
- **A isenção cobre** chaves de config, tags de locale, ids de schema, caminhos de filesystem e códigos de erro do probe, declarados por superfície em `EnvelopeShape::identity` e publicados via `commands`.
- **`schema --truncate-content 12` não mutila mais identificador de contrato** — antes transformava `invoke` em `duckduckgo-s` e `id` em `searc`.
- **Superfícies só de identidade RECUSAM em vez de devolver um no-op** — `config list`, `config path`, `config get/set/unset`, `config effective` e `locale` saem com exit `2` sob `--truncate-content`.
- **Essas superfícies devolviam 1138 bytes contra uma baseline de 1138 bytes** — byte a byte a assinatura do defeito original e indistinguível para o chamador.
- **A recusa tem UM contrato em toda superfície** — stdout carrega a forma publicada `error-response` `{"error","message"}`, stderr carrega a frase localizada, e o exit code permanece `2`.
- **A `message` do stdout permanece em inglês de propósito** — ela é a metade de máquina do contrato, enquanto `--ui-lang` governa apenas o stderr.
- **`commands` renomeou `discriminator` para `discriminator_key`** — veja a seção de mudança que quebra integração acima.
- **Quatro novas chaves XDG** — `probe_launch_timeout_seconds`, `probe_extract_timeout_seconds`, `probe_deep_launch_timeout_seconds` e `probe_deep_extract_timeout_seconds`.
- **Os tetos do probe resolvem CLI, depois XDG, depois o default compilado** — um `--timeout` menor ainda vence.
- **`--fields` agora vale para uma busca que estourou o tempo** — a velocidade da rede não decide mais se a flag é honrada.
- **O caminho de stream multi-query e o caminho não-stream dizem a mesma coisa** sobre a mesma entrada de `--fields` / `--filter`.
- Design e evidência: entrada `[1.0.5]` do `CHANGELOG.pt-BR.md`.

## Destaques v1.0.4 para Integrações
- **Os sete operadores agent-native agem ou recusam por nome** — `--fields`, `--filter`, `--limit`, `--sort`, `--dedupe-by`, `--count-only` e `--truncate-content` não têm terceiro resultado.
- **Antes da v1.0.4 eles eram aceitos e ignorados em silêncio fora da superfície de busca** — `doctor --fields type` produzia 2523 bytes contra uma baseline de 2524 bytes, sendo o byte de diferença a quebra de linha final.
- **`--fields` e `--truncate-content` têm significado em qualquer objeto JSON** e valem em todo lugar.
- **As cinco operações de linha exigem um array de linhas** — uma superfície sem ele as recusa com exit `2`, nomeando a flag, a superfície e o que É suportado ali.
- **O array de linhas é DECLARADO por superfície, nunca inferido** — `doctor` carrega tanto `checks` quanto `failed_checks`, e `config effective` carrega tanto `allowed_keys` quanto `precedence`.
- **Medido depois** — `commands` 6421 → 47 bytes com `--fields version`; `doctor` 2524 → 38 com `--fields type,status`; `schema` 4726 → 1107 com `--fields schemas.id`.
- **A matriz de capacidades é publicada** em `commands` sob `agent_ops`, então o chamador aprende o contrato em vez de descobri-lo coletando exit codes.
- **Um caminho de `--fields` que não casa com nada é erro** que nomeia o nível onde o caminho quebrou e as chaves disponíveis ALI.
- **As chaves de wire em inglês são o padrão** — mantenha o português com `--wire-keys pt` ou `config set wire_keys pt`.
- **Três campos pararam de emitir português sob o wire inglês** — `AggregatedItem.display_url`, `AggregatedNewsItem.source` e `AggregatedNewsItem.relative_date` não emitem mais `url_exibicao`, `fonte` e `data_relativa`.
- **`deep-research-output.schema.json` declarava os nomes em inglês**, então uma linha de notícia real com veículo ou data FALHAVA o contrato que o produto publica para ela.
- **Sete envelopes publicados ganharam discriminador** — as cinco formas de `config`, `locale` e `init-config` agora emitem `type` a partir de um enum verificado pelo compilador.
- **Os 24 schemas publicados se particionam em roteáveis e deliberadamente não roteáveis**, e o motivo é publicado no catálogo como `routing`.
- **O caminho de stream multi-query não descarta mais erro de parse** — erros de `--fields` e `--filter` recusam com exit `2` nos dois caminhos.
- Design e evidência: entrada `[1.0.4]` do `CHANGELOG.pt-BR.md`.

## Destaques v1.0.2 para Integrações
- **ADR-0027 wire EN default** — stdout serializa chaves em inglês (`.results`, `.title`, `.metadata`, `.result_count`, `.metadata.chrome_channel`, `.metadata.chrome_path_resolved`, `.metadata.used_chrome`, …). Desserialização ainda aceita aliases PT.
- **Agentes legados** — mantenha chaves PT com `--wire-keys pt` ou `config set wire_keys pt`. Renomes completos: [docs/MIGRATION.pt-BR.md](docs/MIGRATION.pt-BR.md).
- **Agent ops (sem jq)** — `--fields`/`--select`, `--filter`, `--limit`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`.
- **RuntimeConfig SSOT** — CLI > XDG > FACTORY; **sem env de produto**, **sem telemetria remota**.
- **Descoberta** — `commands`, `schema`, `doctor`, `locale`, `man` para auto-descoberta do agente.
- Design: [`docs/decisions/0027-wire-en-default-v1-0-2.md`](docs/decisions/0027-wire-en-default-v1-0-2.md).

## Destaques v1.0.1 para Integrações
- **API dual de config** — `config get/set/unset` aceita posicional `KEY`/`VALUE` **e** `--key`/`--value`.
- **`-f ndjson`** é alias do modo `--stream`; stream `BrokenPipe` → exit **141** (e2e pipe|head).
- **Oneshot + SIGPIPE** — `ensure_oneshot_cleanup` + kill residual de Chrome + remoção forçada de perfil; **SIG_IGN** para SIGPIPE para o Drop/reap rodar (pipe orphans=0).
- **ADR-0023** — wire serializa em PT + aliases de deserialização EN (compatível; **supersedido na serialização por ADR-0027 / v1.0.2**).
- **`config effective`**, doctor `channel=`, XDG `default_lang`/`default_country`, filtro de qualidade de depth.
- **Sem env de produto**, **sem telemetria remota**. Gates locais apenas.
- Inventário: `gaps.md` Pass 52 / GAP-E2E-51.

## Destaques v1.0.0 para Integrações
- **GAP-WS-TMP-PROFILE-ORPHAN-001 (ADR-0020)** — perfis Chrome com prefixo auditável **`ddg-chrome-*`** (não `.tmp` genérico); exit cooperativo remove o diretório; `sweep_orphan_profiles` na próxima run limpa **somente** `ddg-chrome-*` stale de propriedade desta CLI.
- **Higiene dura de disco** — nunca bulk-delete de `.tmp*` estrangeiro nem `org.chromium.Chromium.*`; residual SIGKILL/OOM → sweep da próxima run só em `ddg-chrome-*`.
- **deep-research** herda o `CancellationToken` do `main` (SIGTERM cancela fan-out para o reap de disco rodar).
- **Contrato estável 1.0.0** — one-shot processo+disco, defaults agent-ready, Chrome-only CDP, atomwrite, **sem telemetria remota**. Sem quebra de schema JSON vs 0.9.10/0.9.9.
- Design: [`docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md`](docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md); inventário: `gaps.md`.

## Destaques v0.9.8 para Integrações
- **GAP-WS-AGENT-READY-001 (ADR-0018)** — defaults agent-ready para hosts Linux reais.
- **Padrão `--vertical all`** — busca normal retorna web + news; opt-out com `--vertical web` (deep: `--no-news`).
- **Fetch de conteúdo LIGADO por padrão** — texto limpo para top URLs web + news (teto **4** na v1.0.2; era 10 na v0.9.8); opt-out com `--no-fetch-content`.
- **News pode incluir `content`** (wire EN desde v1.0.2; legado PT `conteudo` com `--wire-keys pt`) — mesmo pipeline de readability da web (supersede a regra v0.8.9 “só web”).
- **Chrome multi-canal** — export/wrapper Flatpak resolve para ELF de deploy; ordem: `--chrome-path` → `CHROME_PATH` → Chrome host → Chromium host → Flatpak → Snap.
- **Flags de transporte `global = true`** — `--chrome-path`, `--proxy`, `--vertical`, flags de fetch, identidade etc. aceitas **antes ou depois** de `deep-research`.
- **Metadados agent honestos (não telemetria)** — v1.0.2 EN: `chrome_path_resolved`, `chrome_channel`, `used_chrome` (legado PT: `chrome_path_resolvido`, `chrome_canal`, `usou_chrome` via `--wire-keys pt`).
- **Fórmula canônica** — prefira timeout maior com fetch ligado:

  ```bash
  timeout 180 duckduckgo-search-cli -q -f json --num 15 "query"
  timeout 180 duckduckgo-search-cli -q -f json deep-research "query" --chrome-path /caminho/chrome
  # Envelope fino pré-0.9.8:
  timeout 60 duckduckgo-search-cli -q -f json --vertical web --no-fetch-content "query"
  # Parse wire EN v1.0.2:
  timeout 180 duckduckgo-search-cli -q -f json --num 15 "query" | jaq '.results[] | {title, url}'
  ```

- Design: [`docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md`](docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md); inventário: `gaps.md`.

## Destaques v0.9.6 para Integrações
- **Contrato de processo one-shot (GAP-WS-LIFECYCLE-001, ADR-0017)** — cada invocação da CLI encerra completamente a árvore de processos Chromium/Xvfb na saída. Agentes podem invocar o binário N vezes sem vazar RAM de Chromium/Xvfb entre execuções.
- **Cancelamento cooperativo com SIGTERM/SIGINT** — supervisores que enviam SIGTERM primeiro (ex.: `timeout`, Docker stop) cancelam de forma cooperativa para que o caminho de reap do lifecycle rode.
- **Prefira timeouts que enviam SIGTERM primeiro** — use o `timeout` do GNU (SIGTERM e depois SIGKILL após o grace) em vez de wrappers que matam só com SIGKILL, para a limpeza de processos poder completar.
- **Nota de upgrade de versões <0.9.6** — órfãos históricos de execuções pré-0.9.6 **não** são limpos automaticamente; operadores podem precisar de um kill manual único. Novas execuções após o upgrade não vazam.
- **Limites residuais** — SIGKILL não é interceptável; se o supervisor matar com SIGKILL imediatamente, o reap pode não rodar.
- **Sem telemetria** — o endurecimento de lifecycle não emite telemetria.
- **Sem quebra no schema JSON** — envelope de saída, exit codes e flags permanecem iguais; drop-in para integrações existentes.
- Detalhes de design: [`docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md`](docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md) (ADR-0017 / GAP-WS-LIFECYCLE-001).

## Destaques v0.9.4 para Integrações
- **GAP-WS-113 (transporte Chrome-only universal, ADR-0016)** — produção é **somente Chrome** via chromiumoxide/CDP (feature `chrome` é padrão). Todas as operações de rede — busca, notícias, deep-research, `--probe`, `--probe-deep`, `--pre-flight`, `--fetch-content` — exigem Chrome utilizável.
- **Fail-closed sem Chrome** — Chrome ausente ou `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` → **exit 2** (`INVALID_CONFIG`). Sem auto `--no-news`, sem rebaixamento para Web, sem caminho de sucesso HTTP silencioso.
- **`--allow-lite-fallback` é no-op legado** — nunca força Lite; a SERP permanece HTML canônico sob Chrome. Não use como remediação; instale Chrome / `--chrome-path` / `--proxy`.
- **HTTP residual** apenas sob a feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` (testes wiremock). O caminho de sucesso em produção é exclusivamente chromiumoxide.
- **Fórmula canônica** — hosts precisam de Chrome/Chromium (e Xvfb em Linux headless quando necessário):

  ```bash
  timeout 60 duckduckgo-search-cli -q -f json --num 15 "query"
  timeout 180 duckduckgo-search-cli -q -f json deep-research "query"
  ```

## Destaques v0.9.0 para Integrações (histórico)
- **GAP-WS-106 (flags globais)** — nove flags agora são `global = true` e aceitas ANTES OU DEPOIS do subcomando `deep-research`: `-q`, `-o`, `-n`, `-f`, `-t`, `-l`, `-c`, `-p`, `-v` (mais suas formas longas). Autores de pipeline não precisam mais decorar a ordem das flags em relação ao subcomando. Estende o precedente de hoisting do GAP-WS-59.
- **GAP-WS-106 (auto-degradação sem Chrome) — HISTÓRICO, supersedido por GAP-WS-113 / v0.9.4** — em v0.9.0–v0.9.3, `deep-research` sem Chrome aplicava `--no-news` automaticamente com warning no stderr e prosseguia web-only; `--vertical news|all` rebaixava para `Web` em vez de abortar. **Desde a v0.9.4 esses caminhos falham com exit 2** (sem auto-degradação).
- **GAP-WS-106 (erros acionáveis)** — quando o parser rejeita uma flag conhecida posicionada após o subcomando, uma dica é anexada apontando a posição correta (caso agora raro, dado que as 9 flags mais usadas são globais).
- **Sem mudanças no schema JSON na v0.9.0** — o envelope era byte-idêntico ao v0.8.9; apenas o comportamento de parser/exit-code mudou.

## Destaques v0.8.9 para Integrações
- **GAP-WS-104 (vertical de notícias, flag `--vertical`)** — nova flag `--vertical <web|news|all>` (padrão histórico `web`; **v0.9.8 padrão `all`**). `news` e `all` são Chrome-only (sem fallback HTTP), aceitam qualquer número de queries (multi-query via `--queries-file` ou posicionais múltiplas é aceito desde o GAP-WS-105). Desde a v0.9.8 `--vertical` é flag **global** (também aceita depois de `deep-research`).
- **Envelope de notícias (wire EN v1.0.2 padrão)** — `.news[].{position,title,url}` são garantidos não-null; `.news[].{source,relative_date,thumbnail}` são opcionais (`Option<String>` — sempre aplique fallback `// ""` no `jaq`). `.news_count` e `.metadata.vertical_used` aparecem quando vertical != web. **v0.9.8:** o padrão já é `all`. Chaves PT legadas (`.noticias`, `.quantidade_noticias`, `.metadados.vertical_usada`, …) **somente** com `--wire-keys pt`.
- **Nova variante ZeroCause `vertical-no-results`** (legado PT `vertical-sem-resultados` com `--wire-keys pt`) — busca news/all com zero hits é classificada como legítima e emite exit 5 (não exit 6).
- **Contabilidade de exit code** — a contagem total de resultados usada nas decisões de exit code é `result_count + news_count` (legado PT: `quantidade_resultados + quantidade_noticias`).
- **Escopo de `--fetch-content` (ATUALIZADO v0.9.8 / atual v1.0.5)** — a extração de conteúdo aplica-se a **web + news** (teto **4** na v1.0.2; era 10 na v0.9.8). A regra histórica v0.8.9 “somente web” foi **supersedida**. Opt-out com `--no-fetch-content`.
- **Fórmula canônica** — `timeout 90 duckduckgo-search-cli --vertical news "query" -q -f json | jaq '.news'`
- **Pipeline RAG de notícias** — extraia campos garantidos com fallbacks opcionais:

  ```bash
  timeout 90 duckduckgo-search-cli --vertical news "rust 1.88 release" -q -f json \
    | jaq -r '.news[] | [.position, .title, .url, (.source // ""), (.relative_date // "")] | @tsv'
  ```

- **Web + notícias combinados (`--vertical all`)** — uma passada Chrome retorna as duas roots:

  ```bash
  # v1.0.2 wire EN (padrão):
  timeout 90 duckduckgo-search-cli --vertical all "query" -q -f json \
    | jaq '{web: [.results[].url], news: [.news[].url]}'
  # Wire PT legado:
  timeout 90 duckduckgo-search-cli --vertical all "query" -q -f json --wire-keys pt \
    | jaq '{web: [.resultados[].url], news: [.noticias[].url]}'
  ```

- **Zero breaking changes no schema JSON**. Todos os campos v0.8.8 permanecem. Os campos de notícias são aditivos e só são emitidos quando a vertical de notícias está ativa.

## Destaques v0.8.8 para Integrações
- **Fix GAP-WS-089 (limpeza de lock stale do Xvfb)** — `spawn_virtual_display()` agora verifica se o PID dentro de `/tmp/.X{N}-lock` está vivo antes de pular o slot. Locks stale de execuções canceladas ou crashadas são removidos automaticamente, prevenindo exaustão do pool Xvfb após ~100 execuções falhas.
- **Fix GAP-WS-090 (`--num` honrado no path Chrome headed)** — busca Chrome primary agora trunca resultados para `min(num, len)` antes de computar `quantidade_resultados`. Antes, `--num 1` retornava 10 resultados (uma página DDG completa).
- **Fix GAP-WS-091 (alias `--region` adicionado)** — `--country`/`-c` agora aceita `alias = "region"`, alinhando a CLI com a documentação SKILL que referencia `--region`.
- **Fix GAP-WS-092/093/097 (`fill_compat_fields()` popula metadados)** — `metadados.quantidade_resultados`, `metadados.endpoint_usado` e `metadados.nivel_cascata` agora são populados via `fill_compat_fields()` antes da emissão JSON. Antes, esses campos existiam apenas no nível raiz ou eram sempre `null`.
- **Fix GAP-WS-094 (`--num` honrado no path batch/paralelo)** — `execute_query_with_cancellation()` agora trunca resultados pelo `--num` no path batch, igualando o fix single-query do GAP-WS-090.
- **Fix GAP-WS-095 (`identidade_usada` populado no Chrome headed)** — quando Chrome headed tem sucesso com `identity_profile = Auto`, a CLI agora busca a identidade correspondente no pool pela UA e popula `identidade_usada` em vez de retornar `null`.
- **Fix GAP-WS-099 (`ZeroResultsSuspeito` emite exit 6)** — a variante `ZeroResultsSuspeito` estava faltando no match arm do exit code 6. Agora emite corretamente exit 6 (`SUSPECTED_BLOCK`) em vez de cair para exit 5. BC opt-out: `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`.
- **Fix GAP-WS-100 (`tamanho_conteudo` reflete tamanho truncado)** — `content_size` agora usa `text.len()` (pós-truncamento) em vez de `size_original` (body HTML bruto). `--max-content-length 500` agora reporta `tamanho_conteudo: 500` em vez do tamanho original do HTML.
- **Fix GAP-WS-102 (deep-research `nivel_cascata` não mais null)** — metadados do deep-research agora leem de `cascade_level_observed` (campo real) em vez de `cascade_level` (campo compat populado após retorno do pipeline).
- **Fix GAP-WS-103 (exit 6 documentado no `--help`)** — a seção EXIT CODES do `--help` agora lista exit code 6 (`Suspected block`). Antes, apenas códigos 0–5 eram documentados.
- **Zero breaking changes no schema JSON**. Todos os campos v0.8.7 permanecem. Novos campos compat são aditivos.

## Destaques v0.7.10 para Integrações
- **Fix GAP-WS-60 (CRÍTICO, propagação de pino de identidade)** — `--identity-profile` agora propaga a identidade selecionada para `failure_output` (pipeline.rs) e `error_output` (parallel.rs) via novo helper `identity_tag_for_cli_identity` em `src/identity.rs`. Antes da fix, o pino de identidade (`identidade_usada`) só aparecia no caminho de SUCESSO; em falha, era sempre `null`. Consumers agora podem correlacionar uma falha a uma identidade específica do pool de 12.
- **Fix GAP-AUD-002 (CRÍTICO, wiring de bench)** — `cargo bench --bench pre_flight_latency` agora roda Criterion corretamente após adicionar `[[bench]] harness = false` em `Cargo.toml`. Antes da fix, o binário do bench era compilado mas invocado pelo test harness, que reportava `running 0 tests` em vez de rodar os 5 cenários. Bench salva resultados em `target/criterion/`.
- **`--require-results` (NOVO flag, `deep-research`)** — quando setado e o fan-out agrega zero resultados, o subcomando retorna exit 4 (`GLOBAL_TIMEOUT`) com mensagem `deep-research produced zero results for query ...; --require-results set → exiting non-zero` no stderr. Fecha o GAP-WS-1114 (silent-discard pattern).
- **`--pre-flight` (NOVO flag, global)** — ativa o scheduler automático de probe-deep dentro de `execute_single_search`. Quando o ambiente está bloqueado, detecta captcha/ghost-block em ~140 ms antes de gastar a query real, e aborta com `pre_flight_blocked` (exit 3). Default `false` para preservar o comportamento da v0.7.8.
- **`--probe-deep` agora retorna exit 3 quando detecta captcha** (fix B4, v0.7.10) — antes retornava exit 0 mesmo com `status: "captcha"`. Consumers podem ramificar no exit code em vez de parsear o JSON.
- **Pino de identidade canônico** — formato `<family>-<platform>-<16hex>`, ex.: `chrome-linux-33333333cccc0003`, `firefox-linux-99999999cccc0009`, `safari-macos-bbbbbbbbeeee000b`. Seed determinístico por identidade.
- **Zero breaking changes**. Todos os campos JSON de v0.7.9 permanecem. Schema `SearchMetadata.identity_used: Option<String>` continua opcional (`None` quando cascade `auto`).
- **Checklist local de pre-publish (NOVO)** — 7 gates sequenciais antes de `cargo publish`: fmt, clippy, test, cobertura ≥80%, sem refs stale de v0.7.9 sob `skills/`, publish dry-run válido, gates locais verdes. Regra 1264 (cargo publish dry-run obrigatório antes do real).

## Destaques v0.7.9 para Integrações
- **Fix GAP-WS-58 (CRÍTICO, ghost-block)** — `detectar_interstitial` agora classifica body sub-4KB sem `result-page-signal` como `InterstitialKind::Cloudflare`. O helper `has_result_page_signal` checa classes DDG (`nrn-react-div`, `react-article`, `module--results`, `js-react-aria-results`). Threshold conservador de 4KB evita falsos positivos.
- **Fix GAP-WS-59 (ALTO, markers 2026)** — 5 marcadores Cloudflare novos (`anomaly.js`, `botnet`, `cf-error-code`, `cf-ray`, `Performance & Security by Cloudflare`) mais 1 marker DDG novo (`Unfortunately, bots` parcial). `CLOUDFLARE_MARKERS` e `DDG_MARKERS` atualizados em `src/probe_deep.rs`.
- **Fix GAP-WS-59 (ALTO, flag global)** — `--allow-lite-fallback` e `--pre-flight` hoisted para `RootArgs` com `global = true`. Fechou o caminho `unexpected argument` em subcomandos como `deep-research`.
- **GAP-WS-54 (supply chain)** — `scraper` atualizado de 0.20 para 0.27, removendo transitivamente o `fxhash 0.2.1` unmaintained (RUSTSEC-2025-0057). `cargo audit --deny warnings` agora é gate local rígido. `async-std` (RUSTSEC-2025-0052) continua apenas na feature opcional `chrome`.
- **GAP-WS-55 (drift de doc)** — comentário sobre `wreq` no `Cargo.toml` reescrito para refletir a decisão real (pin em `wreq 6.0.0-rc.29` mais os três pins diretos para `wreq-util`, `brotli-decompressor`, `alloc-no-stdlib`), não a regressão que nunca aconteceu mencionada no comentário obsoleto.
- **`Config.pre_flight` adicionado** com default `false` para opt-in.
- **Contagem de testes: 305 (292 lib + 13 integration)**, 0 clippy warnings, 0 fmt diff, 0 cargo-deny warnings, `cargo doc --offline --no-deps` limpo.
- **Zero breaking changes**. Campos JSON existentes preservados.

## Destaques v0.7.8 para Integrações
- **Detecção honesta de interstitial** — `probe_deep` query de calibração de 9 palavras (substitui a probe fixa de 1 palavra) aciona o tightening upstream real do bot scoring. `cascata_motivo` agora é populado em `exit 3` (anti-bot) com `cloudflare_anomaly_modal` quando o interstitial do Cloudflare é detectado.
- **`--allow-lite-fallback` honrado (histórico até v0.9.3)** — exit 3 (anti-bot) com `cascata_motivo` preenchido substituía o exit 5 silencioso quando um interstitial era detectado e o fallback lite estava habilitado. **Desde a v0.9.4 / GAP-WS-113 a flag é no-op legado** (Chrome-only; Lite não é caminho de sucesso em produção).
- **`--retries` honrado** — valores em `[1, 10]` clampados para prevenir abuso. `--retries 5` produz `metadata.retries == 5` no wire EN v1.0.2 (legado PT: `metadados.retentativas` com `--wire-keys pt`; verificado por regression test).
- **Níveis verbose multi-ocorrência** — `-vv` para debug, `-vvv` para trace (aditivos, `ArgAction::Count`).

## Destaques v0.7.5 para Integrações
- **`--query` (NOVO alias)** — equivalente a passar a query como argumento posicional. Permite syntax `duckduckgo-search-cli --query "rust async" --num 10` para integrações que preferem flags nomeadas em vez de posicionais.
- **`--max-content-length` (NOVO cap)** — limita memória consumida por `--fetch-content` em corpus grande. Default 5000 bytes.
- **Headers `Sec-Fetch-*` consistentes** em todas as famílias de browser, eliminando inconsistência de fingerprint que disparava detecção anti-bot.
- **Fix GAP-WS-29 (CRÍTICO, experiência de build, Windows)** — `cargo install` em Windows MSVC nativo sem o sub-componente **C++ CMake tools for Windows** do Visual Studio Installer falhava minutos adentro do build do BoringSSL com o críptico `program not found / is 'cmake' not installed?`. O preflight do `build.rs` agora detecta isso e aborta em SEGUNDOS com a correção exata (`winget install -e --id Kitware.Cmake` OU Visual Studio Installer → Modify → Workloads → Desktop development with C++ → expandir → marcar C++ CMake tools for Windows). Nova escape hatch: `DDG_SKIP_CMAKE_CHECK=1`.
- **Fix GAP-WS-30 (CRÍTICO, experiência de build, Windows)** — o CMake do BoringSSL usa o generator Visual Studio 17 2022, que exige `cl.exe` (compilador) e `link.exe` (linker). O preflight do `build.rs` agora detecta os dois e aborta com a correção (abrir um Developer PowerShell for VS 2022, ou rodar `Launch-VsDevShell.ps1`). MSVC NÃO é instalado automaticamente (download de 5+ GB, intrusivo demais). Nova escape hatch: `DDG_SKIP_MSVC_CHECK=1`.
- **Fix GAP-WS-31 (CRÍTICO, experiência de build, Windows)** — o gerador perlasm do BoringSSL emite assembly de cripto em formato NASM e exige `perl.exe`. O preflight do `build.rs` agora detecta perl e reporta a correção (`winget install -e --id StrawberryPerl.StrawberryPerl`). Nova escape hatch: `DDG_SKIP_PERL_CHECK=1`.
- **Fix GAP-WS-32/35/36 (MÉDIO, documentação)** — todas as afirmações remanescentes de que "binários pré-compilados do `cargo install` não são afetados" (ou variantes PT/EN) foram qualificadas em `skills/duckduckgo-search-cli-en/SKILL.md`, `skills/duckduckgo-search-cli-pt/SKILL.md`, `llms-full.txt`, `docs/CROSS_PLATFORM.md`, `README.md` e `README.pt-BR.md`. **O `crates.io` NUNCA distribui binários**; `cargo install` sempre compila do fonte. Usuários em Windows precisam satisfazer os quatro pré-requisitos de build do BoringSSL (NASM, CMake, MSVC, Perl) antes de `cargo install` funcionar.
- **Cobertura do preflight do `build.rs` ampliada** — a v0.7.4 checava apenas NASM. A v0.7.5 checa os quatro pré-requisitos de build do BoringSSL (nasm, cmake, cl.exe, link.exe, perl) e suporta quatro escape hatches independentes `DDG_SKIP_*_CHECK=1`.
- **Novo `scripts/check-windows-toolchain.ps1`** — diagnóstico standalone (sem instalar nada) que checa as 7 ferramentas (cargo, rustc, cmake, nasm, cl.exe, link.exe, perl) e emite saída em texto ou JSON. Exit code 0 se todas presentes, 1 caso contrário. Útil para tickets de suporte e gates de CI.
- **Novos `docs/INSTALL-WINDOWS.md` (EN) + `docs/INSTALL-WINDOWS.pt-BR.md` (PT)** — guia passo a passo cobrindo 5 métodos de instalação (VS Installer + standalone; standalone todo via winget; Chocolatey; script auxiliar; diagnóstico standalone). Inclui troubleshooting para cada um dos 4 GAPs e as escape hatches `DDG_SKIP_*_CHECK`.
- **Jobs Windows de CI atualizados** — `local gates` e `local release process` agora verificam CMake, instalam Perl e verificam MSVC Build Tools (além do passo NASM existente) em todo job Windows. Isso elimina a dependência implícita do tooling pré-instalado na imagem `Windows host`.
- **Zero breaking changes no schema JSON**. Todos os campos v0.7.4 permanecem. Todos os campos v0.7.3 permanecem.
- **Fix GAP-WS-27 (CRÍTICO, herdado da v0.7.3)**: o interstitial de CAPTCHA no macOS que retornava HTTP 200 com `quantidade_resultados: 0` enquanto o Windows retornava resultados completos está fechado. Stack TLS mudou de `rustls` para BoringSSL via `wreq 6.0.0-rc.29`. `cargo install` sempre compila do fonte — o crates.io não distribui binários pré-compilados para nenhuma plataforma. A mudança de toolchain de build é o trade-off do fix TLS BoringSSL (GAP-WS-27 fechado). Builds de fonte no Linux exigem `cmake`, `perl`, `pkg-config` e `libclang-dev`; builds de fonte no Windows exigem NASM, CMake, MSVC e Perl (ver `gaps.md` GAP-WS-28/29/30/31 e `docs/INSTALL-WINDOWS.md`).
- **Feature `session` (persistência de cookies + warm-up)**:
  - Novas flags: `--no-warmup`, `--no-cookie-persistence`, `--cookies-path <PATH>`.
  - Cookie jar persistido em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows) ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) com permissões Unix `0o600`.
  - O warm-up adiciona um `GET https://duckduckgo.com/` antes da primeira query real para popular os cookies de sessão.
- **Feature `probe-deep` (detecção de interstitial CAPTCHA)**:
  - Novas flags: `--probe-deep` (roda uma query de busca real e classifica o body como `ok` ou `captcha`), `--allow-lite-fallback` (opt-in histórico para fallback html→lite quando CAPTCHA era detectado via GAP-WS-52; **desde a v0.9.4 / GAP-WS-113 esta flag é no-op legado** — a SERP permanece HTML Chrome; não trate como remediação ativa).
  - Novos campos JSON no relatório da resposta do probe (v1.0.2 EN): `status`, `cascade_reason`, `mitigation_suggestion`, `http_status`, `latency_ms` (legado PT `cascata_motivo` / `sugestao_mitigacao` com `--wire-keys pt`).
- **Zero breaking changes no schema JSON**. Todos os campos v0.7.2 permanecem.

## Destaques v0.7.0 para Integrações
- **Novo subcomando `deep-research`**: agentes que precisam de respostas multi-hop
  podem usar `duckduckgo-search-cli deep-research "pergunta" --synthesize`
  e receber um relatório Markdown de volta, sem orquestração extra. Herda
  todas as flags globais (`-q -f json`, `--num`, `--parallel`, `--proxy`,
  `--fetch-content`) mais os knobs específicos de deep-research
  (`--max-sub-queries`, `--sub-queries-file`, `--aggregate`,
  `--budget-tokens`, `--synth-format`).
- **Retrocompatível**: zero mudanças em `buscar`, `init-config`,
  schema JSON de config padrão ou qualquer exit code. Pipelines existentes
  continuam funcionando sem alteração.
- **Pool de 12 identidades anti-bot** — 4 famílias de browser × 3 plataformas com rotação em cascata de 5 níveis. `--identity-profile chrome-linux` para fixar uma identidade específica. `--seed 42` para reprodutibilidade.
- **Detalhe do fan-out de `deep-research`** — até 12 sub-queries, agregação RRF, síntese Markdown opcional com budget de tokens, disponível via `duckduckgo-search-cli deep-research "query" --synthesize --synth-format markdown`.
- **Cookies persistidos em `~/.config/duckduckgo-search-cli/cookies.json`** (XDG, modo `0o600`). Use `--cookies-path` para redirecionar ou `--no-cookie-persistence` para desabilitar.

## Destaques v0.6.5 para Integrações
- **FIX MP-26**: o build Windows agora compila. Use `cargo install duckduckgo-search-cli`
  em qualquer plataforma sem patches manuais.
- **FIX CI-01**: checagens locais multi-plataforma agora verdes nos 3 SOs (Linux/macOS/Windows).
  Agentes rodando em runners Windows podem confiar no binário.
- **WS-12 Circuit breaker**: `--fetch-content --parallel` não cascateia mais
  falhas entre hosts — um domínio lento não bloqueia o resto do crawl.
- **WS-25 ProgressBar**: a saída do `indicatif` no stderr some automaticamente em pipes,
  então pipelines JSON no stdout permanecem limpos.
- **CLI estável em entrypoints comuns** — invocação canônica `timeout 60 duckduckgo-search-cli -q -f json --num 15 "query"` produz JSON determinístico em `stdout` (separado de logs em `stderr`).
- **Exit codes documentados** — 0 sucesso, 1 runtime, 2 config, 3 anti-bot, 4 timeout, 5 zero resultados. Mapeamento consistente em todas as versões.
- **Anti-bot via rotação de UA** — `BrowserProfile` injeta headers `Sec-Fetch-*` por família e Client Hints. Headers duplicados são rejeitados (nunca adicionar manualmente).

Veja `CHANGELOG.pt-BR.md` para o changelog completo da v0.6.5 e as notas de
migração de versões anteriores.

## Destaques v0.7.6 para Integrações
- **GAP-WS-48 fechado (ALTO, experiência de build)**: `cargo install` quebrava
  em certas plataformas por um conflito do resolver do `cargo` entre
  `alloc-no-stdlib 2.0.4` e `alloc-no-stdlib 3.0.0` trazidos transitivamente
  pela stack do `wreq`. A v0.7.6 fixa `alloc-no-stdlib =2.0.4` diretamente no
  `Cargo.toml`. Reinstalar do crates.io agora funciona sem limpeza manual
  de dependências.
- **Sem mudanças no contrato da CLI**: todas as flags, campos JSON e exit codes
  da v0.7.5 permanecem. Substituição drop-in.
- **Mudança de CI**: o job `pre-publish` agora resolve o grafo de dependências
  durante o release — se o pin voltar a derivar, o release falha
  antes de chegar ao crates.io.
- **TLS BoringSSL via `wreq`** (substitui `reqwest+rustls` desde v0.7.3). Fingerprint JA4_o idêntico ao Chrome/Safari elimina o CAPTCHA do Cloudflare no macOS. Ver `docs/decisions/0001-tls-boring-via-wreq.md`.
- **Detecção de interstitial via `probe_deep`** — query de calibração substitui a probe fixa. Markers `CLOUDFLARE_MARKERS` e `DDG_MARKERS` atualizados em `src/probe_deep.rs`.

## Destaques v0.7.7 para Integrações
- **GAP-WS-49 fechado (CRÍTICO, regressão de runtime)**: uma falha de resolução
  de `wreq-util` na v0.7.6 quebrou a emulação de fingerprint TLS BoringSSL
  em certas distribuições Linux. O resultado era interceptação silenciosa
  por CAPTCHA em hosts que antes funcionavam. A v0.7.7 fixa `wreq-util`
  diretamente no `Cargo.toml` — sem mais drift de resolução.
- **Sem mudanças no contrato da CLI**: todas as flags, campos JSON e exit
  codes da v0.7.6 permanecem. Substituição drop-in.
- **Caminho de upgrade recomendado**: v0.7.5 → v0.7.7 é o salto mais limpo
  para quem pulou a v0.7.6. O delta v0.7.6 → v0.7.7 é
  apenas de dependências.
- **Emulação de fingerprint TLS restaurada** via pin direto em `wreq-util`. `alloc-no-stdlib` resolvido entre 2.0.4 e 3.0.0.
- **`cargo install` confiável** — pin em `wreq 6.0.0-rc.29` + `brotli-decompressor = "=5.0.1"` + `alloc-no-stdlib = "=2.0.4"`. Resolução de deps reproduzível cross-platform.

## Destaques v0.7.8 para Integrações (replicado)
- **GAP-WS-50**: lista de interstitial do `probe-deep` ampliada — 8 marcadores
  Cloudflare mais 1 marcador de anomalia DDG. A taxa de falso-negativo na
  detecção de CAPTCHA caiu de forma mensurável nas execuções de benchmark.
- **Probe de calibração de 9 palavras** — `the quick brown fox jumps over the lazy dog` aciona o tightening upstream real do bot scoring. Markers Cloudflare e DDG atualizados em `src/probe_deep.rs`.
- **GAP-WS-52 (histórico até v0.9.3)**: `--allow-lite-fallback` consultava
  o resultado real do detector. Quando o detector sinalizava CAPTCHA e a flag
  estava desligada, a CLI emitia um `tracing::warn!` estruturado e saía com o
  código apropriado em vez de degradar em silêncio. **Desde a v0.9.4 / GAP-WS-113
  a flag é no-op legado** (Chrome-only; Lite nunca é caminho de sucesso em produção).
- **GAP-WS-53**: níveis `-vv` (debug) e `-vvv` (trace) adicionados. Operadores
  investigando buscas falhas podem escalar verbosidade sem
  recompilar. A flag tem `conflicts_with = "quiet"`.
- **GAP-WS-54**: `scraper` elevado para `0.27.0`. Remove o transitivo
  `fxhash 0.2.1` (RUSTSEC-2025-0057, unmaintained). `cargo audit
  --deny warnings` agora é gate bloqueante em CI e release.
- **GAP-WS-55**: bloco `wreq` no `Cargo.toml` reescrito para casar com o
  pin realmente em uso (`6.0.0-rc.29` mais três pins diretos). Elimina
  o drift entre documentação e código.
- **GAP-WS-56**: subcomando legado `Buscar` ocultado do `--help` via
  `#[command(hide = true)]`. Continua chamável por retrocompatibilidade.
- **GAP-WS-57**: flag `--retries` agora honrada de ponta a ponta em
  `src/parallel.rs:644`. O comportamento anterior descartava o valor
  em silêncio no caminho `error_output`. Integradores que dependem do
  retry verão o efeito real.
- **Zero breaking changes no schema JSON**. Todos os campos v0.7.7
  permanecem. Substituição drop-in assim que a v0.7.8 for publicada.

Veja `CHANGELOG.pt-BR.md` para o changelog completo de v0.7.6/v0.7.7/v0.7.8 e
o ADR em `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`
para o racional de design completo da revisão da v0.7.8.

## Aviso de Compatibilidade
Esta CLI segue SemVer. Breaking changes só ocorrem em minor bumps (0.x.0).
Para consumers que não atualizam regularmente, a v0.6.5 é o último release com contrato
totalmente estável antes das mudanças de identidade anti-bot introduzidas em v0.7.0+
