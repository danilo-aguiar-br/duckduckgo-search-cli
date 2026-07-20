# Como Usar o duckduckgo-search-cli

[English](HOW_TO_USE.md)

Busca web em tempo real no seu terminal — 15 resultados frescos em menos de 3 segundos.


## Por Que Este Guia
- Siga este guia e execute sua primeira busca web em menos de 60 segundos
- Aprenda os comandos principais, padrões avançados e integrações com pipelines shell
- Entenda cada exit code e saiba exatamente como se recuperar de cada erro


## Pré-requisitos
### Obrigatórios
- Acesso à rede para duckduckgo.com
- Rust 1.88+ ao instalar via `cargo install` (MSRV desde v0.7.2)
- Binários pré-compilados do GitHub Releases não exigem instalação do Rust (quando publicados; nota: `cargo install` SEMPRE compila do source — ver `gaps.md` GAP-WS-27/28/29/30/31 e `docs/INSTALL-WINDOWS.pt-BR.md`)
- **v0.8.6+**: Nenhuma ferramenta nativa de build C necessaria. TLS e Rust puro via `reqwest` + `rustls-tls`. `cmake`, `perl`, NASM, MSVC NAO sao mais necessarios. **(v0.7.3-v0.8.5 exigia cmake, perl, NASM para BoringSSL — removido na v0.8.6, ver ADR-0008.)**
### Opcionais
- `jaq` (substituto Rust do jq) para processar JSON em pipelines
- Um proxy SOCKS5 para rotação de IP quando houver rate-limiting


## Instalação
### Cargo (Recomendado)
- Execute: `cargo install duckduckgo-search-cli`
- Localização do binário: `~/.cargo/bin/duckduckgo-search-cli`
- Verifique: `duckduckgo-search-cli --version`
### Binários Pré-compilados
- Baixe em [GitHub Releases](https://github.com/danilo-aguiar-br/duckduckgo-search-cli/releases)
- Disponível para Linux (glibc + musl), macOS Universal e Windows MSVC
- Nenhuma instalação do Rust necessária — binário estático único


## Primeiro Comando
### Busca Básica
```bash
duckduckgo-search-cli "programação async em rust"
```
- Padrão: 15 resultados, formato detectado automaticamente pelo TTY
- Adicione `-f json` para saída legível por máquina
- Adicione `-q` para suprimir logs de tracing ao usar pipe
### Saída Esperada
```
 1. Título do primeiro resultado
    https://exemplo.com/pagina
    Texto do snippet descrevendo o conteúdo da página...

 2. Título do segundo resultado
    ...
```
- Use `-f json` para obter saída estruturada para scripts e agentes
- Use `-f markdown` para obter uma lista linkável para relatórios


## Comandos Principais
### Busca em Texto
```bash
# Saída legível por humanos (padrão no TTY)
duckduckgo-search-cli -n 5 "query"
```
- Formato padrão no TTY é `text`
- Formato padrão em pipes é `json`
- Use `-n N` para controlar a quantidade de resultados (padrão: 15)
### Saída JSON
```bash
# Saída legível por máquina para scripts e LLMs
duckduckgo-search-cli -q -n 10 -f json "query"
```
- Sempre passe `-q` ao usar pipe para suprimir logs de tracing
- Schema: array `resultados[]` com `titulo`, `url`, `snippet`
- Ordem dos campos congelada entre versões — segura para parsing automatizado
### Relatório Markdown
```bash
# Lista linkável para relatórios e documentos
duckduckgo-search-cli -n 15 -f markdown -o relatorio.md "query"
```
- Formato: `- [Título](URL)\n  > snippet`
- Use `-o` para salvar diretamente em arquivo
### Salvar em Arquivo
```bash
# Escrita atômica — segura para scripts concorrentes
duckduckgo-search-cli -q -n 10 -f json -o resultados.json "query"
```
- Cria diretórios pai automaticamente
- Permissões Unix definidas como `0o644`
- Caminhos com `..` são rejeitados (proteção contra path traversal)
- Arquivos são gravados via temp+rename atômico (`paths::atomic_write` / atomwrite) — o mesmo padrão para `init-config` e o cookie jar (v0.9.6)
### Stream NDJSON (v1.0.1)
```bash
# Stream multi-query — qualquer uma das formas funciona:
timeout 120 duckduckgo-search-cli -q --stream q1 q2 q3 -n 5
timeout 120 duckduckgo-search-cli -q -f ndjson q1 q2 q3 -n 5
# Fechamento cedo do consumer é esperado/bom:
timeout 120 duckduckgo-search-cli -q --stream q1 q2 -n 10 | head -n 1
# Espere exit 141 da CLI; perfil Chrome ainda é reaped (one-shot pipe-safe)
```
- `-f ndjson` é **alias** do modo stream multi-query (`--stream`)
- Somente multi-query: single-query `--stream` / `-f ndjson` é **ignorado com warning** (saída agregada continua; `stream_efetivo=false`)
- Um objeto JSON compacto por linha LF conforme os resultados chegam (não é stream de eventos por hit da SERP)
- Consumer fecha no meio do stream → exit **141** (BrokenPipe); `ensure_oneshot_cleanup` ainda reap `ddg-chrome-*`
- Chaves wire JSON serializam em **português** (BC); aliases em inglês aceitos só na **desserialização** ([ADR-0023](decisions/0023-wire-pt-bc-english-deserialize-aliases.md))
- Prefira `-q` para manter stderr limpo para parsers


## Subcomandos `config` (v1.0.1, só CLI + XDG)
- Config de produto é **flags CLI + arquivos XDG** apenas — sem knobs de env de produto (`DUCKDUCKGO_*` de runtime de produto foram **removidas**; só notas históricas)
- API dual aceita em get/set/unset (posicional **ou** flags):

```bash
# get (somente ALLOWED_KEYS — ex.: proxy_url, log_directive, default_global_timeout)
duckduckgo-search-cli config get proxy_url
duckduckgo-search-cli config get --key proxy_url
# set
duckduckgo-search-cli config set proxy_url "http://host:8080"
duckduckgo-search-cli config set --key log_directive --value "duckduckgo_search_cli=debug"
# unset
duckduckgo-search-cli config unset proxy_url
duckduckgo-search-cli config unset --key proxy_url
# visão efetiva mesclada (CLI + XDG + defaults)
duckduckgo-search-cli config effective
```
- Chaves XDG permitidas: `ui_lang`, `chrome_path`, `proxy_url`, `default_global_timeout`, `default_vertical`, `fetch_content_default`, `log_directive`, `default_lang`, `default_country`
- Também: `config path`, `config list`, `init-config`, `man`
- Sem telemetria remota


## Arquitetura Chrome-Primary (v0.8.7+)
- Chrome é o transporte PRIMARY de busca desde a v0.8.0
- Desde a v0.8.5 no Linux, Chrome roda HEADED dentro de display virtual Xvfb privado (NÃO headless); desde v0.9.3 macOS/Windows usam headless=new (stealth coerente via correções v0.9.2)
- A CLI auto-spawna Xvfb via `spawn_virtual_display()` — o usuário vê ZERO janelas
- v0.8.7 adiciona `has_native_display()` para detectar display nativo por plataforma
- v0.8.7 adiciona `try_auto_install_xvfb()` — auto-instala Xvfb em 22+ distros Linux (sudo não-interativo)
- v0.8.7 adiciona navegação warm-up para duckduckgo.com ANTES da URL de busca (GAP-WS-077)
- v0.8.7 filtra pool de identidades para UA Chrome-only com fingerprint TLS Chromium (GAP-WS-074)
- Transporte de rede de produção é **Chrome-only** (v0.9.4, GAP-WS-113): busca, news, `deep-research`, `--probe`, `--probe-deep`, `--pre-flight` e `--fetch-content` usam chromiumoxide/CDP. HTTP residual (`reqwest`) existe apenas sob a feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`
- Posse one-shot de processos (v0.9.6 / GAP-WS-LIFECYCLE-001 / [ADR-0017](decisions/0017-browser-lifecycle-one-shot-v0-9-6.md)): cada invocação é dona da árvore Chromium, do Xvfb privado (Linux) e do diretório de perfil
- **One-shot de disco (v1.0.0 / GAP-WS-TMP-PROFILE-ORPHAN-001 RESOLVIDO / [ADR-0020](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md)):** perfil Chrome sob `temp_dir` usa o prefixo **`ddg-chrome-*`** (modo Unix `0o700`), não `.tmp*` genérico. Completa o one-shot de processo com limpeza honesta de disco
- **One-shot pipe-safe (v1.0.1 / Pass 52):** `ensure_oneshot_cleanup` em todas as saídas inclusive pipe cedo; SIGPIPE Unix permanece **SIG_IGN** para Drop/reap ainda rodarem; stream BrokenPipe → exit **141** com órfãos 0
- `src/process_lifecycle.rs` — spawn em process group, reap por árvore/marker; `XvfbGuard` RAII sempre mata o Xvfb no drop; `ChromeBrowser` no Drop / `force_reap` mata processos **e** faz `remove_dir_all` do perfil de propriedade; `ExitReapGuard` + panic hook na saída cooperativa
- Reap completo da árvore + perfil em sucesso, erro, timeout, SIGINT, SIGTERM e BrokenPipe (SIGTERM cancela o `CancellationToken` principal para que Docker/GNU `timeout`/supervisores disparem cancel cooperativo; o **deep-research herda o mesmo token** e o SIGTERM cancela o fan-out)
- A próxima invocação roda `sweep_orphan_profiles` **somente** em `ddg-chrome-*` obsoletos de propriedade da CLI — **política dura:** nunca auto-rm de `.tmp*` estrangeiros; nunca auto-rm de `org.chromium.Chromium.*`
- Prefira GNU `/usr/bin/timeout` (SIGTERM primeiro) para o cancel cooperativo + reap; SIGKILL/OOM nu da CLI pode deixar residual (a próxima run varre só `ddg-chrome-*`). Órfãos de processo históricos pré-0.9.6 e perfis genéricos `.tmp*` pré-1.0.0 **não** são apagados em massa pela CLI
- Sem quebra de schema JSON no lifecycle; atomwrite para gravações em disco; sem telemetria remota (continua Chrome-only desde a v0.9.4)
- Chrome contorna detecção anti-bot do Cloudflare via 17 sinais stealth (aprimorados na v0.8.7)
- Instalar Chrome: `sudo apt install google-chrome-stable` (Debian/Ubuntu)
- Xvfb é auto-instalado pela CLI (v0.8.7+) — instalação manual: `sudo apt install xvfb` ou `sudo dnf install xorg-x11-server-Xvfb`
- Saída JSON inclui `metadados.usou_chrome: true` honesto quando Chrome foi usado (incluindo news-only)
- JSON pode incluir metadados agent (não telemetria): `metadados.chrome_path_resolvido`, `metadados.chrome_canal` (`manual|env|host|flatpak|snap`)
- Saída JSON inclui `metadados.tentou_chrome: true` quando Chrome foi tentado
- Sobrescrever binário: CLI `--chrome-path /caminho/do/chrome` ou XDG `config set chrome_path` (flag global — funciona após `deep-research`; env `CHROME_PATH` **não** é lida)
- Forçar headless: flag CLI `--chrome-headless` (com risco de detecção Cloudflare; env de produto `DUCKDUCKGO_CHROME_HEADLESS` **removida**)
- Forçar headed visível: flag CLI `--chrome-visible` (para depuração; env de produto `DUCKDUCKGO_CHROME_VISIBLE` **removida**)


## Defaults Agent-Ready (v0.9.8, GAP-WS-AGENT-READY-001 / ADR-0018)
- O padrão de `--vertical` é **`all`** (web + notícias no mesmo envelope). Opt-out com `--vertical web` (deep-research: `--no-news`)
- Fetch de conteúdo está **LIGADO por padrão** nas top URLs web + news (teto 10). Opt-out com `--no-fetch-content`
- Prefira timeouts externos mais longos com fetch ligado (ex.: `timeout 180`) — o caminho padrão é mais lento que SERP fina
- Metadados agent (contrato local, **não** telemetria): `metadados.chrome_path_resolvido`, `metadados.chrome_canal`, `usou_chrome` honesto
- Flags de transporte são **globais** (`global = true`), incluindo `--chrome-path`, `--proxy`, `--vertical`, flags de fetch — aceitas antes ou depois de `deep-research`
- Chrome multi-canal Flatpak no Linux: shells de export / wrappers resolvem para ELF de deploy; ordem de candidatos CLI `--chrome-path` → XDG `chrome_path` → Chrome do host → Chromium do host → Flatpak → Snap (env `CHROME_PATH` **não** é lida)
- Lifecycle one-shot de processo (v0.9.6) + one-shot de disco (v1.0.0, `ddg-chrome-*`) e produção Chrome-only (v0.9.4) continuam válidos; atomwrite para gravações em disco; sem telemetria
### Preservar envelope fino pré-0.9.8
```bash
# Só web, sem corpos de página (estilo 0.9.7):
timeout 60 duckduckgo-search-cli --vertical web --no-fetch-content -n 10 -q -f json "query"
```
### Dual web+news com texto limpo (padrão)
```bash
timeout 180 duckduckgo-search-cli -q -n 10 -f json "query" \
  | jaq '{web: (.resultados|length), noticias: (.quantidade_noticias // 0), chrome: .metadados.chrome_canal}'
```


## Vertical de Notícias (v0.8.9+, padrões alterados na v0.9.8)
- `--vertical <web|news|all>` seleciona a vertical de busca — **padrão `all` desde a v0.9.8** (GAP-WS-AGENT-READY-001 / ADR-0018); use `--vertical web` para só web
- `news` e `all` são Chrome-only — NÃO há fallback HTTP para a vertical de notícias
- `news` e `all` aceitam batches multi-query desde o GAP-WS-105 (`--queries-file`, múltiplas queries posicionais) — uma sessão Chrome por query; no `deep-research` a vertical news é o PADRÃO (opt-out `--no-news`)
- `--pre-flight` aplica-se SOMENTE à vertical web — é pulado com `--vertical news`
- Os seletores de notícias são hot-fixáveis em `config/selectors.toml` seção `[news]` — sem recompilação
### Busca de Notícias Canônica
```bash
timeout 90 duckduckgo-search-cli --vertical news "query" -q -f json | jaq '.noticias'
```
- Os resultados de notícias ficam em `.noticias[]` — separados do array web `.resultados[]`
- Campos garantidos: `.noticias[].posicao`, `.noticias[].titulo`, `.noticias[].url`
- Campos opcionais (use fallback `// ""`): `.noticias[].fonte`, `.noticias[].data_relativa`, `.noticias[].thumbnail`
- Com fetch padrão LIGADO, linhas de news também podem trazer `conteudo` / `tamanho_conteudo` / `metodo_extracao_conteudo` (top 10)
- `.quantidade_noticias` e `.metadados.vertical_usada` costumam estar presentes no padrão `all` (ou quando vertical != web)
### Web + Notícias Combinados
```bash
# O padrão de vertical já é `all` na v0.9.8 — a flag é opcional
timeout 180 duckduckgo-search-cli --vertical all "query" -q -f json \
  | jaq '{web: .resultados, noticias: .noticias}'
```
- `--vertical all` retorna ambos os arrays em um único envelope
### Semântica de Exit da Vertical News
- Contabilização total de resultados: exit 5 ocorre apenas quando `resultados + quantidade_noticias == 0`
- SERP de notícias renderizada com zero artigos classifica como `causa_zero: vertical-sem-resultados` — zero LEGÍTIMO (exit 5, NÃO 6)
- Todas as outras variantes de ZeroCause mantêm a semântica de exit 6 (inspecione `.metadados.causa_zero`)
- **v1.0.1 / GAP-E2E-51-006:** `--vertical news` isolada faz prime de sessão via navegação SERP web e retenta extracts interstitial/vazios com backoff — classificação anti-bot **falsa** corrigida. Residual de anti-bot **real** do DDG ainda pode gerar exit **6** + `causa_zero: anti-bot` ambientalmente (`noticias` vazias honestas, nunca artigos sintéticos). stderr pode logar avisos de retry/prime fora de `-q`.


## Padrões Avançados
### Buscar Conteúdo das Páginas
```bash
# Fetch de conteúdo LIGADO por padrão (v0.9.8). Flag explícita ainda válida; limite o tamanho:
duckduckgo-search-cli -q -n 5 --max-content-length 8000 -f json "query"

# Opt-out total dos corpos de página:
duckduckgo-search-cli -q -n 5 --no-fetch-content -f json "query"
```
- Campo `conteudo` aparece nos top resultados (web + news, teto 10) quando o fetch está habilitado (padrão LIGADO)
- Use `--max-content-length` para limitar caracteres por página (padrão: 10000)
- Use `--per-host-limit 1` para evitar sobrecarregar um único domínio
- Prefira `timeout 120`–`180` com fetch ligado
### Busca Paralela com Múltiplas Queries
```bash
# Uma query por linha no arquivo queries.txt
duckduckgo-search-cli -q \
  --queries-file queries.txt \
  --parallel 3 \
  --per-host-limit 1 \
  --retries 3 \
  -n 10 -f json \
  -o resultados.json
```
- `--parallel` controla requisições simultâneas (1..=20)
- `--per-host-limit` limita fetches por domínio (1..=10)
- Resultados agrupados por query em `.buscas[]` no modo multi-query
### Busca Filtrada por Tempo
```bash
# Apenas resultados das últimas 24 horas
duckduckgo-search-cli -q -n 10 --time-filter d -f json "query de notícias recentes"
```
- Valores: `d` (dia), `w` (semana), `m` (mês), `y` (ano)
- A SERP de produção permanece HTML via Chrome (GAP-WS-113); **não** use `--endpoint lite` como estratégia de frescor ou anti-bot
### Roteamento via Proxy
```bash
# Rotear via proxy SOCKS5
duckduckgo-search-cli -q -n 10 --proxy socks5://127.0.0.1:9050 -f json "query"

# Rotear via proxy HTTP corporativo
duckduckgo-search-cli -q -n 10 --proxy http://usuario:senha@proxy.interno:8080 -f json "query"
```
- Proxy de produto é **somente CLI + XDG**: `--proxy` / `--no-proxy` e/ou `config set proxy_url` — `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` **nunca** são lidas
- Precedência: `--no-proxy` > `--proxy <URL>` > XDG `proxy_url` > nenhum
### Controle de Idioma
```bash
# Resultados em português
duckduckgo-search-cli -q -n 10 --lang pt -f json "query"

# Resultados em inglês dos EUA
duckduckgo-search-cli -q -n 10 --lang en --country us -f json "query"
```
- Padrão de idioma: `pt`, padrão de país: `br`
- Usa os códigos de região `kl` do DuckDuckGo


## Integração com Scripts Shell
### Extrair URLs dos Resultados
```bash
duckduckgo-search-cli -q -n 10 -f json "query" \
  | jaq -r '.resultados[].url'
```
- Saída com uma URL por linha, pronta para `xargs` ou fetchers downstream
### Filtrar por Palavras-chave no Snippet
```bash
duckduckgo-search-cli -q -n 20 -f json "query" \
  | jaq -r '.resultados[] | select(.snippet | test("rust")) | .titulo'
```
- `test()` no `jaq` aplica regex contra o texto do snippet
### Contar Resultados
```bash
duckduckgo-search-cli -q -n 10 -f json "query" \
  | jaq '.resultados | length'
```
- Verifique a contagem real retornada versus o `-n` solicitado
### Tratar Exit Codes em Scripts
```bash
duckduckgo-search-cli -q -n 10 -f json "query" > /tmp/saida.json
case $? in
  0) echo "OK" ;;
  3) echo "Bloqueio anti-bot — aguarde 300s, rotacione proxy/identidade, --probe-deep (Chrome) — não Lite" >&2 ;;
  4) echo "Timeout global excedido" >&2 ;;
  5) echo "Zero resultados — tente query mais ampla" >&2 ;;
  6) echo "Bloqueio suspeito — inspecione .metadados.causa_zero" >&2 ;;
  *) echo "Erro: exit $?" >&2 ;;
esac
```
- Sempre verifique `$?` antes de consumir o arquivo de saída
- Exit code 3 é temporário — aguarde 300+ s, rotacione proxy/identidade, verifique o Chrome (`--probe-deep`); nunca Lite


## Integração com Agentes de IA
### Claude Code
```bash
# Em uma chamada de ferramenta Bash do Claude Code:
RESULTADOS=$(duckduckgo-search-cli -q -n 10 -f json "$QUERY" \
  | jaq -r '.resultados[] | "## \(.titulo)\n\(.snippet)\nURL: \(.url)\n"')
```
- Instale a skill incluída para ativação automática sem engenharia de prompt
- Caminho da skill: `skills/duckduckgo-search-cli-pt/SKILL.md`
### OpenAI Codex / GPT
```bash
# Injeta JSON estruturado como contexto em messages[].content
duckduckgo-search-cli -q -n 10 -f json "$QUERY" | jaq '.resultados'
```
- O schema estável `resultados[]` mapeia limpo para campos de tool call response
- O fetch de conteúdo padrão embute corpos limpos das páginas para grounding mais profundo (opt-out com `--no-fetch-content`)
### Gemini
```bash
# Texto completo das páginas como dados de grounding (fetch LIGADO por padrão desde a v0.9.8)
duckduckgo-search-cli -q -n 5 \
  --max-content-length 5000 \
  -f json "$QUERY" \
  | jaq -r '.resultados[].conteudo // empty'
```
- Pipe do conteúdo para o modo JSON do Gemini para síntese de fatos de cauda longa
### Qualquer LLM via Pipe
```bash
duckduckgo-search-cli -q -n 10 -f json "$QUERY" \
  | jaq -r '.resultados[] | "## \(.titulo)\n\(.snippet)\n"'
```
- A saída é Markdown puro — cole diretamente em qualquer janela de contexto
- Veja `docs/INTEGRATIONS.md` para 16 snippets prontos por agente


## Erros Comuns
### Bloqueio Anti-bot HTTP 202 (exit 3)
- O DuckDuckGo retornou uma página de desafio, não resultados reais
- Aguarde **300+ segundos** antes de tentar novamente (**não** use `--endpoint lite` nem `--allow-lite-fallback`)
- Rotacione o IP / identidade de saída via CLI `--proxy socks5://127.0.0.1:9050` ou XDG `config set proxy_url` (`HTTP_PROXY` / `HTTPS_PROXY` **nunca** são lidas)
- Verifique a saúde do Chrome: `--probe-deep` e/ou `--chrome-path`
- Aumente as tentativas: `--retries 5`
- Execute `duckduckgo-search-cli init-config` para atualizar perfis de browser
### Timeout Global (exit 4)
- O pipeline excedeu o `--global-timeout` (padrão: 180 segundos desde v0.9.9)
- Aumente o valor: `--global-timeout 120`
- Reduza a contagem de resultados: `-n 5`
- Reduza `--num` / `--pages` ou aumente `--global-timeout` em links lentos (**não** mude para Lite)
### Zero Resultados (exit 5)
- Geralmente é rate-limiting temporário, não um bloqueio permanente
- Aguarde 60 segundos e repita a mesma query
- Amplie a query removendo termos muito específicos
- Remova `--time-filter` se estiver definido — ele restringe o pool de resultados
- Inspecione `.metadados.causa_zero` — body ≥4KB sem cards orgânicos **nunca** é `legitimo` (GAP-WS-113; em geral exit 6)
- Com `--vertical news`, `causa_zero: vertical-sem-resultados` significa SERP renderizada sem artigos — zero legítimo (v0.8.9)
### Configuração Inválida (exit 2)
- Uma flag está fora da faixa permitida ou o caminho é inválido
- Chrome ausente / não detectado (fail-closed GAP-WS-113; env de produto `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` **removida** / não lida)
- `--timeout 0` é rejeitado — mínimo é 1 segundo
- `--output ../../../etc/passwd` é rejeitado — path traversal bloqueado
- `--global-timeout 0` é rejeitado — mínimo é 1 segundo
- `--parallel 0` é rejeitado — mínimo é 1
### Chromium / Xvfb / dirs de perfil órfãos / crescimento de RAM após muitas invocações de agente
- **Processo** corrigido na **v0.9.6** (GAP-WS-LIFECYCLE-001 / [ADR-0017](decisions/0017-browser-lifecycle-one-shot-v0-9-6.md)); **perfil em disco** corrigido na **v1.0.0** (GAP-WS-TMP-PROFILE-ORPHAN-001 **RESOLVIDO** / [ADR-0020](decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md)); **reap pipe-safe** corrigido na **v1.0.1** (Pass 52 / SIG_IGN + `ensure_oneshot_cleanup`)
- Em saída cooperativa **e** BrokenPipe (exit 141), cada invocação faz reap de Chromium + Xvfb e `remove_dir_all` do perfil de propriedade **`ddg-chrome-*`** sob `temp_dir` (Unix `0o700`); `force_reap` / `ExitReapGuard` / `ensure_oneshot_cleanup` mata processos **e** remove o perfil
- A próxima run `sweep_orphan_profiles` limpa **somente** `ddg-chrome-*` obsoletos de propriedade da CLI — **política dura:** nunca auto-rm de `.tmp*` estrangeiros; nunca auto-rm de `org.chromium.Chromium.*`
- Prefira supervisores que enviam **SIGTERM** primeiro (GNU `/usr/bin/timeout`) para o cancel cooperativo + reap rodarem; deep-research herda o `CancellationToken` principal
- Auditoria do operador (perfis de propriedade 1.0.0+ apenas):
  ```bash
  find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'ddg-chrome-*'
  ```
- Limites residuais (honestos): **SIGKILL/OOM** da CLI pode deixar residual; a próxima run varre só `ddg-chrome-*`. Órfãos de processo pré-0.9.6 continuam higiene do operador. Perfis genéricos `.tmp*` pré-1.0.0 **não** são apagados em massa pela CLI — julgue com cuidado se limpar; **não** recomende `rm` em massa de `/tmp/.tmp*` nem de `org.chromium.Chromium.*`
- Atualize: `cargo install duckduckgo-search-cli --locked --force` para **1.0.1+**
### Broken pipe no meio do stream (exit 141, v1.0.1)
- Esperado quando o consumer fecha cedo (`| head`, `| jaq 'first'`, agente cancela a leitura)
- A CLI mapeia `ErrorKind::BrokenPipe` → exit **141** (128+SIGPIPE)
- O reap one-shot do Chrome ainda roda — não é bug de órfão
- **Não** trate 141 como falha de busca quando NDJSON parcial foi intencional


## Referência de Códigos de Saída

| Código | Significado | Ação Recomendada |
|--------|------------|-----------------|
| 0 | Sucesso | Processar resultados normalmente |
| 1 | Erro de runtime (rede, parse, I/O) | Verificar stderr para detalhes |
| 2 | Config inválida **ou** Chrome ausente (fail-closed GAP-WS-113; env de produto `NO_CHROME` **removida**) | Corrigir argumento; instalar Chrome |
| 3 | Bloqueio anti-bot | Aguardar 300s; proxy/identidade; `--probe-deep` (Chrome) — **não** Lite |
| 4 | Timeout global excedido | Aumentar `--global-timeout` |
| 5 | Zero resultados (legítimos; inclui `vertical-sem-resultados` com `--vertical news`, v0.8.9) | Ampliar query ou remover filtros |
| 6 | Bloqueio suspeito (causa_zero != legitimo, v0.8.0+) | Inspecionar `.metadados.causa_zero` |
| 141 | Broken pipe (consumer de stdout fechou cedo; v1.0.1) | Esperado com `| head` / cancel cedo; Chrome ainda reaped |


## Próximos Passos
- Veja `docs/COOKBOOK.md` para 15 receitas copy-paste de pesquisa, ETL e monitoramento
- Veja `docs/INTEGRATIONS.md` para 16 guias de integração com agentes de IA
- Veja `docs/AGENTS-GUIDE.md` para o contrato completo stdin/stdout e referência de schema
- Veja `docs/CROSS_PLATFORM.md` para guias de configuração em Linux, macOS, Windows e Docker
- Veja `docs/AGENT_RULES.md` para 30+ regras DEVE/JAMAIS para uso em produção com agentes


## v0.7.3 — Sessão + Probe-Deep + BoringSSL (correção do GAP-WS-27)

> **Nota (v0.8.6)**: A stack wreq/BoringSSL descrita nesta secao foi substituida por `reqwest` + `rustls-tls` na v0.8.6 (ADR-0008). Esta secao e historica (v0.7.3-v0.8.5).

v0.7.3 fecha atomicamente o GAP-WS-27 (CAPTCHA no macOS) substituindo a stack TLS `rustls` por BoringSSL embarcado via `wreq 6.0.0-rc.29`, mais persistencia de cookies de sessao e deteccao profunda de CAPTCHA.

### Mudanca da Stack TLS (wreq + BoringSSL) — Historica, substituida na v0.8.6

A CLI agora usa `wreq 6.0.0-rc.29` em vez de `reqwest 0.12` + `rustls-tls`. O `wreq` traz o BoringSSL embarcado (via `boring2 v4.15.11`) e produz um fingerprint `JA4_o` idêntico ao Chrome/Safari real, fechando a porta de entrada do Cloudflare Bot Management que gerava o CAPTCHA.

- Dependências adicionadas: `wreq = "6.0.0-rc"` com features `tokio-rt, webpki-roots, cookies, gzip, brotli, deflate, zstd, socks, form, query`; `wreq-util = "3.0.0-rc.12"`.
- Dependências removidas: `reqwest`, `rustls`, `cookie_store`, `cookie` (em deps diretas).
- ADR formal: `docs/decisions/0001-tls-boring-via-wreq.md`.

### Pré-requisitos de Build Mudaram (v0.7.3+)

Compilar do source no Linux agora requer `cmake`, `perl`, `pkg-config` e `libclang-dev` (BoringSSL). **`cargo install` SEMPRE compila do código-fonte** — crates.io não distribui binários pré-compilados para nenhuma plataforma; usuários que instalam via `cargo install` precisam de TODAS essas dependências instaladas. Veja `docs/INSTALL-WINDOWS.pt-BR.md` para o passo a passo do Windows MSVC (NASM, CMake 3.20+, MSVC C/C++ toolchain, Strawberry Perl — fechados como GAP-WS-28/29/30/31 progressivamente em v0.7.4 e v0.7.5) e `gaps.md` para a análise completa.

```bash
# Debian/Ubuntu
sudo apt-get install cmake perl pkg-config libclang-dev
# Fedora/RHEL
sudo dnf install cmake perl pkg-config clang-devel
# Alpine
apk add cmake perl pkgconf clang-dev
```

### Persistência de Cookies de Sessão

A feature `session` persiste cookies do DuckDuckGo em `cookies.json` para que requisições subsequentes reutilizem a sessão, e faz um `GET https://duckduckgo.com/` de warm-up antes da primeira query real para popular os cookies de sessão.

- Localização do cookie jar:
  - macOS: `~/Library/Application Support/duckduckgo-search-cli/cookies.json`
  - Linux: `~/.config/duckduckgo-search-cli/cookies.json`
  - Windows: `%APPDATA%\duckduckgo-search-cli\cookies.json`
- Permissões Unix: `0o600` (owner read+write only).
- O cookie jar contém cookies de sessão do DuckDuckGo. Trate como credencial.

#### Flags de Sessão

```bash
# Desabilitar warm-up (pular GET /warm-up)
duckduckgo-search-cli --no-warmup "query"

# Manter cookies só em memória (não gravar cookies.json)
duckduckgo-search-cli --no-cookie-persistence "query"

# Apontar para um cookie jar em volume criptografado
duckduckgo-search-cli --cookies-path /Volumes/encrypted/cookies.json "query"
```

### Detecção Profunda de CAPTCHA (probe-deep)

`--probe-deep` faz uma query de teste real e classifica o body retornado como `ok` ou `captcha`, expondo o JSON:

```bash
duckduckgo-search-cli --probe-deep -q -f json
# {"status": "ok", "endpoint": "html", "http_status": 202,
#  "latency_ms": 97, "cascata_motivo": "none",
#  "sugestao_mitigacao": "no interstitial detected"}
```

Use `--probe-deep` em CI antes de lançar queries caras, especialmente em runners macOS onde o GAP-WS-27 se manifestava.

#### `--allow-lite-fallback` (no-op legado desde v0.9.4)

`--probe-deep` detecta e reporta via Chrome. `--allow-lite-fallback` é **no-op legado** (GAP-WS-113) — não força Lite nem remedia exit 3:

```bash
duckduckgo-search-cli --probe-deep --allow-lite-fallback -q -f json "query"
# Flag aceita por compatibilidade; sem caminho de sucesso Lite em produção.
```

### Validação Empírica (v0.7.3)

```bash
# Antes (v0.7.2): quantidade_resultados: 0, ms: 1695
# Depois (v0.7.3): quantidade_resultados: 5, ms: 735
duckduckgo-search-cli "rust wreq emulation browser fingerprint 2026" -q -f json --num 5
```


## v0.7.4 — Preflight NASM no Windows (GAP-WS-28)

v0.7.4 fecha o GAP-WS-28 (build do Windows MSVC falha após minutos com a mensagem críptica "CMake Error: No CMAKE_ASM_NASM_COMPILER could be found" quando o NASM está ausente) adicionando um preflight no build.rs que detecta nasm.exe no PATH e falha em segundos com a correção exata.

- Novo comportamento em builds nativos Windows MSVC:
  - Se nasm.exe não está no PATH: build entra em panic em segundos com `NASM assembler not found in PATH. Fix (PowerShell): winget install -e --id NASM.NASM ; $env:Path += ";C:\Program Files\NASM"` e uma dica sobre known_nasm_dir() quando o binário existe mas o PATH está obsoleto.
  - Se nasm.exe está no PATH: build segue como antes.
- Escape hatch: DDG_SKIP_NASM_CHECK=1 para usuários com ambientes de build customizados.
- endurecimento dos gates locais: jobs Windows host em local gates e local release process verificam/instalam NASM explicitamente.
- Zero mudanças de runtime — mesmas flags, mesmo schema JSON de saída, mesmas dependências da v0.7.3.

## v0.7.5 — Preflight 4 ferramentas + scripts + INSTALL-WINDOWS (GAP-WS-29/30/31)

v0.7.5 estende o preflight da v0.7.4 para detectar as quatro ferramentas que o build do BoringSSL precisa no Windows MSVC nativo, e entrega dois scripts auxiliares novos e um guia de instalação dedicado.

- GAP-WS-29/30/31 fechados pelo preflight estendido: detecta CMake 3.20+ (com o sub-componente C++ CMake tools for Windows, que vem desmarcado por padrão no Visual Studio Installer), MSVC C/C++ compiler e linker (cl.exe, link.exe, presentes apenas em Developer Command Prompt for VS 2022 ou após sourcear Launch-VsDevShell.ps1), e interpretador Perl (Strawberry Perl é a escolha de fato). Cada ferramenta ausente dispara panic em segundos com a correção exata e uma dica de uma linha sobre o script auxiliar.
- Escape hatches: DDG_SKIP_NASM_CHECK=1, DDG_SKIP_CMAKE_CHECK=1, DDG_SKIP_MSVC_CHECK=1, DDG_SKIP_PERL_CHECK=1. Use para pular o preflight em ambientes de build customizados.
- Novo scripts/install-windows.ps1 — detecta NASM, CMake, Perl; auto-instala via winget (fallback choco) e corrige o PATH da sessão. Para MSVC, imprime a invocação exata de Launch-VsDevShell.ps1 para rodar após instalar o VS Build Tools. MSVC não é auto-instalado (download de 5+ GB, requer admin, invasivo demais para um script one-shot).
- Novo scripts/check-windows-toolchain.ps1 — diagnóstico standalone que verifica todas as 7 ferramentas (cargo, rustc, cmake, nasm, cl.exe, link.exe, perl) e emite saída texto ou JSON. Exit code 0 se todas presentes, 1 caso contrário. Adequado para tickets de suporte e portões locais.
- Novo docs/INSTALL-WINDOWS.pt-BR.md — guia passo a passo cobrindo 5 métodos de instalação (Visual Studio Installer mais ferramentas standalone, tudo-standalone via winget, somente Chocolatey, script auxiliar, diagnóstico standalone). Inclui troubleshooting para cada um dos 4 GAPs e dos 4 escape hatches DDG_SKIP_*_CHECK.
- checagens multi-plataforma locais continua instalando as 4 ferramentas explicitamente nos jobs Windows host.
- Zero mudanças de runtime — mesmas flags, mesmo schema JSON de saída, mesmas dependências da v0.7.4. O crates.io NÃO distribui binários pré-compilados para nenhuma plataforma.
- Contagem de testes: 405 testes lib (eram 392 na v0.7.0, 333 na v0.6.5; total atual do projeto na v0.7.5).

## v0.7.2 — rand 0.10 RngExt + time 0.3.47 RUSTSEC-2026-0009 + MSRV 1.88

v0.7.2 é uma release de manutenção que endereça duas dependências upstream:

- `time = "0.3.47"` pinado como dependência direta para sobrescrever `time 0.3.40` que vinha transitivamente via `cookie_store 0.22.0` e `reqwest 0.12.28`. Resolve `RUSTSEC-2026-0009` (stack exhaustion DoS em time 0.3.40).
- `rand 0.10.1` reorganizou os métodos `random_range`, `random_bool` e `random` do trait `Rng` para o trait extension `RngExt`. Substituído `use rand::Rng;` por `use rand::RngExt;` em `src/identity.rs`, `src/parallel.rs` e `src/search.rs`.
- MSRV subiu de 1.85 para 1.88 (exigido por `time 0.3.47` e `rand 0.10`).


## v0.7.1 — Patch de Manutenção

v0.7.1 é uma release puramente de manutenção sem novas flags CLI e sem novos campos JSON. Sincroniza `Cargo.lock` self-version 0.7.0 → 0.7.1 e conserta warnings de clippy latentes.


## v0.7.0 — Subcomando `deep-research`

v0.7.0 introduz o subcomando `deep-research` para pesquisa multi-hop com fan-out de sub-queries.

```bash
duckduckgo-search-cli -q -f json deep-research "tokio vs async-std 2026" \
  --synthesize --synth-format markdown | jaq -r '.sintese'
```

Campos novos: `.metadados.sub_queries[]`, `.metadados.total_resultados_unicos`, `.metadados.tempo_total_ms`, `.resultados[].score`, `.resultados[].fontes[]`, `.sintese` (opt-in via `--synthesize`).


## v0.6.4 — Pool Adaptativo de Identidades Anti-Bot (WS-26)

### Problema
As heurísticas anti-bot do DuckDuckGo classificam uma única combinação de User-Agent + IP + ordem de headers após a primeira requisição. Reutilizar a mesma identidade em todas as chamadas de paginação e em múltiplas queries produz uma única fingerprint que é bloqueada com HTTP 202 (anomalia), HTTP 403 ou HTTP 429.

### Solução
v0.6.4 introduz um pool de 12 identidades com rotação em cascata de 5 níveis:

| Nível | Estratégia |
|-------|------------|
| 0     | Identidade atual (sem rotação) |
| 1     | Mesma família, plataforma diferente |
| 2     | Família diferente, mesma plataforma |
| 3     | Família e plataforma diferentes + endpoint rebaixado para lite |
| 4     | Identidade aleatória + sleep recomendado de 30-60s antes de retentar |

### Uso

#### Probe antes de lançar uma query real

```bash
duckduckgo-search-cli --probe
```

O probe envia uma requisição mínima e reporta status, latência e presença de Set-Cookie como JSON. Exit 0 significa que o endpoint está acessível da sua combinação IP/UA; exit 1 significa que a requisição falhou.

#### Fixa uma identidade específica (determinístico para testes)

```bash
duckduckgo-search-cli -q -n 10 -f json --identity-profile chrome-linux "query"
```

Perfis válidos: `auto` (padrão), `chrome-win`, `chrome-mac`, `chrome-linux`, `edge-win`, `firefox-linux`, `safari-mac`.

#### Rotação de identidade reproduzível (debug de anti-bot)

```bash
duckduckgo-search-cli -q -n 10 -f json --seed 42 "query"
```

A mesma seed produz a mesma sequência de identidades entre execuções. Use para reproduzir bloqueios anti-bot durante debug.

#### Inspecionar qual identidade produziu uma resposta

```bash
duckduckgo-search-cli -q -n 5 -f json "query" | jaq '.metadados.identidade_usada'
# Output: "chrome-linux-11111111aaaa0001"
```


## v0.6.5 — Instalação no Windows corrigida, gates locais verdes, circuit breaker, ProgressBar

v0.6.5 é uma release de qualidade sem novas flags CLI e sem novos campos JSON.
Ela foca em tornar a ferramenta confiável nos três alvos de plataforma e em
crawls longos.

### Windows agora funciona out of the box (MP-26)

`cargo install duckduckgo-search-cli` no Windows falhava em v0.6.4 porque
o upstream `windows-sys 0.59+` mudou o tipo `HANDLE` de `isize` para
`*mut c_void`. v0.6.5 corrige isto com:

```rust
// src/platform.rs:51-69 — verificação type-safe de HANDLE
let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
    if unsafe { GetConsoleMode(handle, &mut mode) } != 0 { ... }
}
```

O cast `handle as isize` (que seria UB) foi removido completamente.

### Circuit breaker protege crawls longos (WS-12)

Quando `--fetch-content --parallel` raspa muitas páginas do mesmo domínio,
3 falhas consecutivas nesse host agora abrem o circuito por 30 segundos.
Todas as requisições para esse host são curto-circuitadas durante o cooldown,
prevenindo falhas em cascata que bloqueariam o crawl inteiro.

Você não precisa fazer nada — o breaker é automático. Mas pode observá-lo
no stderr se `--verbose` estiver ativo.

### ProgressBar no stderr, não no stdout (WS-25)

`--fetch-content` agora mostra uma barra de progresso no stderr. A saída JSON
no stdout permanece limpa para pipes. A barra se esconde em contextos não-TTY
(CI, logs).

### checagens multi-plataforma locais verde em todos os 3 SOs (CI-01)

v0.6.4 foi publicada com CI quebrado em Linux, macOS e Windows. v0.6.5
restaura a matrix verde corrigindo 6 erros de clippy latentes e adicionando
smoke tests por plataforma (`--version --help`) ao pipeline CI.

### Novos lints bloqueiam drift FFI futuro

`improper_ctypes = "deny"` e `improper_ctypes_definitions = "deny"` estão
agora ativos. Eles teriam pego o problema de HANDLE da v0.6.4 em tempo de
compilação se estivessem ativos então.

O campo `identidade_usada` reporta a identidade que produziu a resposta bem-sucedida. O campo `nivel_cascata` reporta o nível de cascata atingido (0-4).


## v0.7.0 — Pipeline de Deep Research

Para perguntas de pesquisa multi-hop, use o subcomando `deep-research`. Ele decompõe uma query em até 12 sub-queries, dispara em paralelo, agrega via RRF ou dedup por URL canônica, e opcionalmente produz um relatório Markdown.

```bash
# 1. Fan-out rápido (sem síntese, 5 sub-queries por padrão).
timeout 60 duckduckgo-search-cli -q -f json deep-research "melhor cliente http rust 2026" \
  | jaq '.resultados | length'

# 2. Relatório Markdown sintetizado com orçamento de tokens.
timeout 120 duckduckgo-search-cli -q -f json deep-research "tokio vs async-std 2026" \
  --synthesize --synth-format markdown --budget-tokens 1500 \
  | jaq -r '.sintese'

# 3. Sub-queries manuais (comentários `#` e linhas vazias são ignorados).
cat > /tmp/qs.txt <<EOF
# Visão geral
o que é tokio runtime 2026
# Comparação
tokio vs async-std
EOF
timeout 60 duckduckgo-search-cli -q -f json deep-research "tokio 2026" \
  --sub-queries-file /tmp/qs.txt --aggregate dedupe-by-url \
  | jaq '.metadados.sub_queries | length'
```

O subcomando `deep-research` herda toda flag global (`-q -f json`, `--num`, `--lang`, `--country`, `--parallel`, `--endpoint`, `--proxy`, `--retries`, `--global-timeout`, `--fetch-content`, `--max-content-length`) e adiciona:

- `--max-sub-queries N` — teto do fan-out (1..=12, padrão 5)
- `--sub-query-strategy` — `heuristic` (padrão) ou `manual`
- `--sub-queries-file PATH` — obrigatório para `manual`; comentários e linhas vazias são ignorados
- `--aggregate` — `rrf` (padrão, K=60) ou `dedupe-by-url`
- `--synthesize` — produz o relatório final
- `--budget-tokens N` — teto do tamanho da síntese (1 token ≈ 4 chars)
- `--synth-format` — `markdown` (padrão), `plain-text` ou `json`
- `--no-news` — desativa a varredura da vertical news (v0.8.9, GAP-WS-105); por padrão cada sub-query roda `--vertical all` via Chrome e o envelope sempre traz `noticias[]` + `quantidade_noticias`. Desde a v0.9.4 (GAP-WS-113) a produção é **fail-closed**: sem Chrome utilizável a CLI sai com **exit 2** (auto-degradação do GAP-WS-106 supersedida — sem auto `--no-news`, sem caminho web-only)

```bash
# 4. Notícias agregadas da varredura dual padrão (v0.8.9, GAP-WS-105).
timeout 180 duckduckgo-search-cli -q -f json deep-research "rust security advisories" \
  | jaq '.noticias[:5]'

# 5. Opt-out de news com Chrome disponível (o fan-out web ainda exige Chrome).
timeout 120 duckduckgo-search-cli -q -f json deep-research "tokio 2026" --no-news \
  | jaq '.quantidade_noticias'
# CI sem Chrome: instale Chrome/Chromium (e Xvfb em Linux headless). Espere exit 2 se NO_CHROME=1.
```


## v0.7.3 — Sessão + Probe-Deep + BoringSSL

A stack TLS mudou de `rustls` para BoringSSL via `wreq 6.0.0-rc.29`. Isso fecha o GAP-WS-27 do CAPTCHA do macOS (Cloudflare Bot Management detectou `rustls` como fingerprint de não-navegador via JA4_o). BoringSSL produz JA4_o idêntico ao Chrome/Safari. Ver `docs/decisions/0001-tls-boring-via-wreq.md` para a decisão arquitetural.

### Pré-requisitos de build

Compilar do código-fonte no Linux agora requer:

```bash
# Debian / Ubuntu
sudo apt install cmake perl pkg-config libclang-dev

# Fedora / RHEL
sudo dnf install cmake perl pkg-config clang-devel

# Alpine
sudo apk add cmake perl pkg-config clang-dev
```

Usuários que instalam o binário pré-compilado do crates.io não precisam dessas deps.

### Sessão + cookie jar

Cada invocação agora começa com um warm-up `GET https://duckduckgo.com/` (pode ser pulado com `--no-warmup`) que popula os cookies de sessão. Os cookies são persistidos em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) com permissões Unix `0o600`. O path é sobrescrevível via `--cookies-path <PATH>`. Trate este arquivo como credencial. Use `--no-cookie-persistence` para manter cookies em memória apenas.

### Detecção de CAPTCHA via probe-deep

`--probe-deep` executa uma query real e classifica o body como `ok` ou `captcha` baseado em marcadores Cloudflare e DuckDuckGo (`cf-chl-bypass`, `cf-challenge`, `challenge-platform`, `Attention Required`, `__cf_chl_jschl_tk__`, `robot-detected`, `bots, we have detected`). O relatório inclui `status`, `cascata_motivo`, `sugestao_mitigacao`, `http_status` e `latency_ms`. Use isto em portões locais para runners macOS para detectar CAPTCHA cedo.

```bash
# Em CI antes de queries reais em macOS
timeout 30 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'
```

### `--allow-lite-fallback` (no-op legado, v0.9.4)

`--allow-lite-fallback` é **no-op legado** desde o GAP-WS-113. Não troca endpoints e não remedia CAPTCHA/exit 3. A produção permanece SERP HTML via Chrome.


## v0.7.4 — Preflight NASM no Windows (GAP-WS-28)

O preflight de `build.rs` da v0.7.4 detecta `nasm.exe` no PATH para builds Windows MSVC e falha em segundos com a correção exata (`winget install -e --id NASM.NASM` mais ajuste de PATH). Saída de escape: `DDG_SKIP_NASM_CHECK=1`. A checagens multi-plataforma locais verifica/instala NASM explicitamente. Sem mudanças de runtime.


## v0.7.5 — Preflight de 4 ferramentas + scripts auxiliares + INSTALL-WINDOWS

A v0.7.5 estende o preflight da v0.7.4 para detectar as quatro ferramentas que o build BoringSSL precisa no Windows MSVC: NASM, CMake 3.20+, MSVC C/C++ toolchain, Strawberry Perl. Novos `scripts/install-windows.ps1` auto-instala o que pode; novo `scripts/check-windows-toolchain.ps1` é um diagnóstico standalone; novo `docs/INSTALL-WINDOWS.md` percorre 5 métodos de instalação. Saídas de escape: `DDG_SKIP_NASM_CHECK=1`, `DDG_SKIP_CMAKE_CHECK=1`, `DDG_SKIP_MSVC_CHECK=1`, `DDG_SKIP_PERL_CHECK=1`. Sem mudanças de runtime. Contagem de testes: 405 testes lib.


## v0.7.6 — Correção do `cargo install` (GAP-WS-48)

A v0.7.5 era impossível de compilar em máquinas limpas. `cargo install duckduckgo-search-cli`
falhava com 36 erros `E0277` trait-bound porque o solver puxava
`alloc-no-stdlib 3.0.0` transitivamente de `brotli-decompressor 5.0.2`,
que colide com `brotli 8.0.3` esperando `alloc-no-stdlib = "2.0"`.

A v0.7.6 removeu a dep morta `wreq-util` e abandonou a feature `brotli`
do `wreq` (DDG nunca serve `Content-Encoding: br`). Build sucede em
~35,7s. **Sempre use `--locked`** para evitar o GAP-WS-48 residual: o
solver pode reintroduzir `alloc-stdlib 0.2.3` se o lockfile for
regenerado.

```bash
# Instalação robusta — pin de versão + lock travado
cargo install duckduckgo-search-cli --version 0.7.7 --locked
```


## v0.7.7 — Correção do Fingerprint TLS (GAP-WS-49)

A v0.7.6 publicou um binário que passava nos smoke tests de `--probe`
e `--probe-deep` mas retornava ZERO resultados reais. A causa: remover
`wreq-util` para corrigir o GAP-WS-48 também removeu a feature
`emulation`, deixando o handshake BoringSSL com um fingerprint
trivialmente detectável pelo Cloudflare Bot Management. A DDG servia
`anomaly-modal` para cada query real.

A v0.7.7 re-adiciona `wreq-util 3.0.0-rc.12` com
`default-features = false, features = ["emulation"]` e fixa três deps
diretos no `Cargo.toml`:

- `brotli-decompressor = "=5.0.1"`
- `alloc-no-stdlib = "=2.0.4"`
- Feature `"brotli"` do `wreq` re-habilitada

**Verificação prática após upgrade para v0.7.7**:

```bash
# Sanity check — v0.7.7 deve retornar 5+ resultados reais
timeout 30 duckduckgo-search-cli -q -n 5 -f json "rust async runtime" \
  | jaq '.quantidade_resultados'
# Esperado: 5
# Se ver 0, o lockfile está errado — re-execute com --locked
```


## v0.7.8 — Renovação do Detector Anti-Bot + Verbose Acumulado

A v0.7.8 (working tree) fecha 8 gaps. Ver
`docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md` para a decisão
arquitetural completa.

### `detectar_interstitial` reconhece `anomaly-modal` da DDG (GAP-WS-50)

O interstitial `anomaly-modal` (rollout pós-2026 da DDG) estava
escapando do detector legado (que só conhecia `cf-chl-bypass`,
`cf-challenge`, `robot-detected`, `bots, we have detected`). A v0.7.8
expande a lista de markers para 8 Cloudflare + 1 DDG:

- Cloudflare: `anomaly-modal`, `anomaly-modal__mask`, `anomaly-modal__title`,
  `anomaly.js?cc=botnet`, `cf-turnstile`, `cf-spinner`, `Just a moment`,
  `cf-mitigated`
- DDG: `Unfortunately, bots use DuckDuckGo too.`

Sem mudança de CLI. Fluxos afetados usam os novos markers
automaticamente.

### Probe-deep usa query de calibração longa (GAP-WS-51)

O literal hard-coded `q=rust` (4 chars) foi substituído pelo pangrama
`the quick brown fox jumps over the lazy dog` exposto como
`PROBE_CALIBRATION_QUERY` em `src/lib.rs:91, 509`. Queries curtas não
acionavam o bot scoring upstream e reportavam um falso `status: ok`.

```bash
# Use --probe-deep como gate local; a v0.7.8 é honesta
timeout 30 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'
# Exit 0 apenas quando nenhum interstitial é detectado pelo detector expandido
```

### `--allow-lite-fallback` era guiado por detector (GAP-WS-52; no-op desde v0.9.4)

Historicamente o predicado de fallback migrou de
`accumulated_results.is_empty()` para
`detectar_interstitial(&first_html) != InterstitialKind::None`.
Desde a v0.9.4 (GAP-WS-113) `--allow-lite-fallback` é **no-op legado** em produção.

```bash
# Receita real — gate local com probe-deep (Chrome); sem remediação Lite
PROBE=$(timeout 30 duckduckgo-search-cli --probe-deep -q -f json)
if [ "$(echo "$PROBE" | jaq -r '.status')" != "ok" ]; then
  echo "CI: anti-bot detectado, recusando queries" >&2
  exit 1
fi

# Produção: SERP HTML via Chrome apenas. --allow-lite-fallback é aceito mas não faz nada.
timeout 60 duckduckgo-search-cli -q -n 10 -f json "rust async runtime" \
  | jaq '.metadados.usou_chrome, .quantidade_resultados'
```

### Verbose agora é cumulativo (GAP-WS-53)

```bash
# nível debug com -v (sem -v o padrão é info; filtro de produto é CLI+XDG, não RUST_LOG)
duckduckgo-search-cli -v -q -n 5 "query"

# nível debug — veja URLs, headers, redirects
duckduckgo-search-cli -vv -q -n 5 "query" 2>&1 | rg -i 'request|response'

# nível trace — corpos completos request/response para debug de protocolo
duckduckgo-search-cli -vvv -q -n 5 "query" 2>&1 | rg 'TRACE'

# Filtro de log de produto persistente via XDG (não RUST_LOG)
duckduckgo-search-cli config set log_directive duckduckgo_search_cli=debug
duckduckgo-search-cli -q -n 5 "query" 2>&1 | head -50
```

### `--retries N` agora é honrado (GAP-WS-57)

O valor estava hard-coded para 1 em `src/parallel.rs:644`. A v0.7.8 lê
`cfg.retries` e faz clamp em `[1, 10]` para que `--retries 999` não
acione anti-bot.

```bash
# Honre --retries com --parallel para crawls multi-query robustos
duckduckgo-search-cli -q \
  --queries-file queries.txt \
  --parallel 3 \
  --retries 5 \
  --per-host-limit 1 \
  -n 10 -f json -o results.json
# Cada host com falha agora retenta até 5 vezes (era 1 na v0.7.7)
```

### Subcomando `buscar` escondido (GAP-WS-56)

```bash
# Invocação direta ainda funciona (mantido para compatibilidade)
duckduckgo-search-cli buscar "rust async" -q -n 5

# Mas --help não mostra mais; use top-level como forma canônica
duckduckgo-search-cli "rust async" -q -n 5
```

### Outros internos da v0.7.8

- **`scraper 0.20 → 0.27`** (GAP-WS-54): fecha RUSTSEC-2025-0057
  (`fxhash 0.2.1` unmaintained). `cargo audit --deny warnings` agora é
  gate local em gates locais.
- **Comentário do `wreq` reescrito** (GAP-WS-55): o texto anterior
  alegava uma regressão para 5.3.0 que nunca aconteceu. O novo comentário
  documenta o pin real em `wreq 6.0.0-rc.29` e os três pins diretos.


## Matriz Comparativa v0.7.5 → v0.7.8

| Feature | v0.7.5 | v0.7.7 | v0.7.8 |
|---|---|---|---|
| `--probe-deep` sinal honesto | Não (curto `q=rust`) | Não (curto `q=rust`) | Sim (pangrama longo) |
| `--allow-lite-fallback` opt-in | Predicado invertido | Predicado invertido | Guiado por detector |
| Detecta interstitial `anomaly-modal` | Não | Não | Sim (8 markers novos) |
| `-vvv` debug | Não suportado | Não suportado | Sim (cumulativo) |
| `--retries N` honrado | Não (hard-coded 1) | Não (hard-coded 1) | Sim (clamp `[1, 10]`) |
| Subcomando `buscar` | Visível no `--help` | Visível no `--help` | Escondido |
| `cargo audit` limpo | 1 advisory transitivo | 1 advisory transitivo | Limpo |
