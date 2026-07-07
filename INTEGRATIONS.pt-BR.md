# Integrações

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
# Invocação canônica
timeout 60 duckduckgo-search-cli -q -f json --num 15 "query"

# Exit codes
0  sucesso              → parse .resultados
1  erro de runtime      → leia stderr; tente novamente com -v
2  erro de configuração → reexecute init-config --force
3  bloqueio anti-bot    → aguarde 300+ s; troque --endpoint lite
4  timeout global       → aumente --global-timeout; reduza --parallel
5  zero resultados      → refine a query ou tente --lang diferente
6  bloqueio suspeito    → inspecionar .metadados.causa_zero; aguardar 300+ s ou rotacionar proxy

# Versão atual: v0.8.9
```

## Destaques v0.8.9 para Integrações

- **GAP-WS-104 (vertical de notícias, flag `--vertical`)** — nova flag `--vertical <web|news|all>` (default `web`). `news` e `all` são Chrome-only (sem fallback HTTP), aceitam exatamente UMA query e são incompatíveis com `deep-research` e multi-query (`--queries-file`) — o clap rejeita com exit 2.
- **Envelope de notícias** — `.noticias[].{posicao,titulo,url}` são garantidos não-null; `.noticias[].{fonte,data_relativa,thumbnail}` são opcionais (`Option<String>` — sempre aplique fallback `// ""` no `jaq`). `.quantidade_noticias` e `.metadados.vertical_usada` estão presentes SOMENTE quando vertical != web — a saída do modo web é byte-idêntica à v0.8.8.
- **Nova variante ZeroCause `vertical-sem-resultados`** — busca news/all com zero hits é classificada como legítima e emite exit 5 (não exit 6).
- **Contabilidade de exit code** — a contagem total de resultados usada nas decisões de exit code é `resultados + quantidade_noticias`.
- **Escopo de `--fetch-content`** — a extração de conteúdo atua SOMENTE sobre `resultados[]` (web); entradas de `noticias[]` nunca são buscadas.
- **Fórmula canônica** — `timeout 90 duckduckgo-search-cli --vertical news "query" -q -f json | jaq '.noticias'`
- **Pipeline RAG de notícias** — extraia campos garantidos com fallbacks opcionais:

  ```bash
  timeout 90 duckduckgo-search-cli --vertical news "rust 1.88 release" -q -f json \
    | jaq -r '.noticias[] | [.posicao, .titulo, .url, (.fonte // ""), (.data_relativa // "")] | @tsv'
  ```

- **Web + notícias combinados (`--vertical all`)** — uma passada Chrome retorna as duas roots:

  ```bash
  timeout 90 duckduckgo-search-cli --vertical all "query" -q -f json \
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

## Destaques v0.7.9 para Integrações

- **GAP-WS-54 (supply chain)** — `scraper` atualizado de 0.20 para 0.27, removendo transitivamente o `fxhash 0.2.1` unmaintained (RUSTSEC-2025-0057). `cargo audit --deny warnings` agora é gate rígido de CI em `ci.yml` e `release.yml`. `async-std` (RUSTSEC-2025-0052) continua apenas na feature opcional `chrome`.
- **GAP-WS-55 (drift de doc)** — comentário sobre `wreq` no `Cargo.toml` reescrito para refletir a decisão real (pin em `wreq 6.0.0-rc.29` mais os três pins diretos para `wreq-util`, `brotli-decompressor`, `alloc-no-stdlib`), não a regressão que nunca aconteceu mencionada no comentário obsoleto.
- **Contagem de testes: 305 (292 lib + 13 integration)**, 0 clippy warnings, 0 fmt diff, 0 cargo-deny warnings, `cargo doc --offline --no-deps` limpo.

## Destaques v0.7.8 para Integrações

- **Detecção honesta de interstitial** — `probe_deep` query de calibração de 9 palavras (substitui a probe fixa de 1 palavra) aciona o tightening upstream real do bot scoring. `cascata_motivo` agora é populado em `exit 3` (anti-bot) com `cloudflare_anomaly_modal` quando o interstitial do Cloudflare é detectado.
- **`--allow-lite-fallback` honrado** — exit 3 (anti-bot) com `cascata_motivo` preenchido agora substitui o exit 5 silencioso quando um interstitial é detectado e o fallback lite está habilitado.
- **`--retries` honrado** — valores em `[1, 10]` clampados para prevenir abuso. `--retries 5` agora produz `metadados.retentativas == 5` (verificado por regression test).
- **Níveis verbose multi-ocorrência** — `-vv` para debug, `-vvv` para trace (aditivos, `ArgAction::Count`).

## Destaques v0.7.5 para Integrações

- **`--query` (NOVO alias)** — equivalente a passar a query como argumento posicional. Permite syntax `duckduckgo-search-cli --query "rust async" --num 10` para integrações que preferem flags nomeadas em vez de posicionais.
- **`--max-content-length` (NOVO cap)** — limita memória consumida por `--fetch-content` em corpus grande. Default 5000 bytes.
- **Headers `Sec-Fetch-*` consistentes** em todas as famílias de browser, eliminando inconsistência de fingerprint que disparava detecção anti-bot.

## Destaques v0.7.0 para Integrações

- **Pool de 12 identidades anti-bot** — 4 famílias de browser × 3 plataformas com rotação em cascata de 5 níveis. `--identity-profile chrome-linux` para fixar uma identidade específica. `--seed 42` para reprodutibilidade.
- **Subcomando `deep-research` (NOVO)** — fan-out multi-query (até 12 sub-queries), agregação RRF, síntese Markdown opcional com budget de tokens. Disponível via `duckduckgo-search-cli deep-research "query" --synthesize --synth-format markdown`.
- **Cookies persistidos em `~/.config/duckduckgo-search-cli/cookies.json`** (XDG, modo `0o600`). Use `--cookies-path` para redirecionar ou `--no-cookie-persistence` para desabilitar.

## Destaques v0.6.5 para Integrações

- **CLI estável em entrypoints comuns** — invocação canônica `timeout 60 duckduckgo-search-cli -q -f json --num 15 "query"` produz JSON determinístico em `stdout` (separado de logs em `stderr`).
- **Exit codes documentados** — 0 sucesso, 1 runtime, 2 config, 3 anti-bot, 4 timeout, 5 zero resultados. Mapeamento consistente em todas as versões.
- **Anti-bot via rotação de UA** — `BrowserProfile` injeta headers `Sec-Fetch-*` por família e Client Hints. Headers duplicados são rejeitados (nunca adicionar manualmente).

## Destaques v0.7.6 para Integrações

- **TLS BoringSSL via `wreq`** (substitui `reqwest+rustls` desde v0.7.3). Fingerprint JA4_o idêntico ao Chrome/Safari elimina o CAPTCHA do Cloudflare no macOS. Ver `docs/decisions/0001-tls-boring-via-wreq.md`.
- **Detecção de interstitial via `probe_deep`** — query de calibração substitui a probe fixa. Markers `CLOUDFLARE_MARKERS` e `DDG_MARKERS` atualizados em `src/probe_deep.rs`.

## Destaques v0.7.7 para Integrações

- **GAP-WS-49 fechado** — TLS fingerprint emulation restaurado via pin direto em `wreq-util`. `alloc-no-stdlib` resolvido entre 2.0.4 e 3.0.0.
- **`cargo install` confiável** — pin em `wreq 6.0.0-rc.29` + `brotli-decompressor = "=5.0.1"` + `alloc-no-stdlib = "=2.0.4"`. Resolução de deps reproduzível cross-platform.

## Destaques v0.7.8 para Integrações (replicado)

- **Probe calibração 9-palavras** — `the quick brown fox jumps over the lazy dog` aciona tightening upstream real do bot scoring. Markers Cloudflare e DDG atualizados em `src/probe_deep.rs`.
- **`--allow-lite-fallback` funcional** — exit 3 com `cascata_motivo` substitui exit 5 silencioso quando interstitial detectado e fallback lite habilitado.

## Aviso de Compatibilidade

Esta CLI segue SemVer. Breaking changes só ocorrem em minor bumps (0.x.0).
Para consumers que não atualizam regularmente, a v0.6.5 é o último release com contrato totalmente estável antes das mudanças de identidade anti-bot introduzidas em v0.7.0+
