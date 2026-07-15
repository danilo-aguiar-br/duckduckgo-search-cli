# Inventário de gaps — duckduckgo-search-cli

## Metadados

| Campo | Valor |
|-------|--------|
| Versão | **1.0.0** |
| Data | 2026-07-15 |
| Caminho | `gaps.md` (raiz; excluído crates.io) + `docs/gaps.md` |
| ADR lifecycle processo | `docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md` |
| ADR disco / prefixo | `docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md` |
| ADR e2e v0.9.9 | `docs/decisions/0019-e2e-gaps-news-timeout-probe-meta-v0-9-9.md` |

## Gaps abertos

| ID | Status |
|----|--------|
| — | **Nenhum.** Inventário limpo em v1.0.0. |

## Gaps e2e — todos RESOLVIDOS em v0.9.9

| ID | Status |
|----|--------|
| GAP-WS-NEWS-LIVE-001 | **RESOLVIDO** v0.9.9 — denylist promo; wait selectors `news-vertical` (sem early `article`); empty se só chrome |
| GAP-WS-NEWS-FETCH-WASTE-001 | **RESOLVIDO** v0.9.9 — skip promo no fetch |
| GAP-WS-TIMEOUT-DEFAULT-001 | **RESOLVIDO** v0.9.9 — default global 180s |
| GAP-WS-EXIT4-JSON-001 | **RESOLVIDO** v0.9.9 — JSON no exit 4 |
| GAP-WS-PROBE-403-001 | **RESOLVIDO** v0.9.9 — calibração + healthy |
| GAP-WS-PREFLIGHT-META-001 | **RESOLVIDO** v0.9.9 — `pre_flight_executado` |
| GAP-WS-META-TIMING-001 | **RESOLVIDO** v0.9.9 — wall clock + fetch |
| GAP-WS-META-NO-CHROME-001 | **RESOLVIDO** v0.9.9 — meta chrome limpa |
| GAP-WS-ERR-CHROME-PATH-001 | **RESOLVIDO** v0.9.9 — PathError `{message}` |
| GAP-WS-QUIET-CONFIG-001 | **RESOLVIDO** v0.9.9 — quiet = off |
| GAP-WS-STREAM-NOOP-001 | **RESOLVIDO** v0.9.9 — help + meta honestos |
| GAP-WS-NEWS-FIXTURE-001 | **RESOLVIDO** v0.9.9 — testes promo-only |
| GAP-WS-DOCS-TIMEOUT-001 | **RESOLVIDO** v0.9.9 — default = docs 180 |
| GAP-WS-L04-REGRESSION-001 | **RESOLVIDO** v0.9.9 — residual L-04 fechado |
| GAP-WS-NEWS-FANOUT-001 | **RESOLVIDO** v0.9.9 — parallel/deep |
| GAP-WS-SELECTORS-XDG-001 | **RESOLVIDO** v0.9.9 — empty se só promo |
| GAP-WS-PROBE-SCHEMA-001 | **RESOLVIDO** v0.9.9 — status string |
| GAP-WS-STREAM-MULTI-001 | **RESOLVIDO** v0.9.9 — help multi NDJSON |

## Histórico resolvido (releases anteriores e v1.0.0)

| ID | Versão |
|----|--------|
| GAP-WS-TMP-PROFILE-ORPHAN-001 | **1.0.0** |
| GAP-WS-LIFECYCLE-001 | 0.9.6 / 0.9.7 |
| GAP-WS-113 | 0.9.4 |
| GAP-WS-AGENT-READY-001 | 0.9.8 (L-04 residual → 0.9.9) |

## Mandatos

- Produção Chrome-only CDP; **one-shot** por invocação (NASCE → EXECUTA → MORRE); **sem telemetria**.
- One-shot inclui **processo** (Chromium + Xvfb) **e disco**: após exit **cooperativo** (sucesso, erro tipado, timeout, SIGINT, SIGTERM), a invocação não deve deixar perfil residual **identificável** desta CLI (`ddg-chrome-*` em `std::env::temp_dir()`).
- Disco de saída do usuário: atomwrite (`paths::atomic_write`, prefixo `.ddg-atomic-`). Metadados agent ≠ telemetria.
- Rules GraphRAG aplicáveis: `rules-rust-cli-one-shot`, `rules-rust-processos-externos`, `rules-rust-shutdown-fundamentos-sinais`, `rules-rust-shutdown-raii-panics-saida`, `rules-rust-shutdown-recursos-externos`.

## Evidência e2e v0.9.9 (host Fedora)

| Caso | Resultado |
|------|-----------|
| news OpenAI | `quantidade_noticias: 0`, `news_filtradas_promo: 5` |
| `--probe` | `healthy: true`, `status: "ok"` |
| NO_CHROME | `tentou_chrome: false`, path/canal null |
| chrome-path inválido | sem prefixo “invalid output path” |
| `--global-timeout 2` | JSON `erro: "timeout"` |

## Política

Status só muda com release + ADR. PT-BR com acentuação.

---

# GAP-WS-TMP-PROFILE-ORPHAN-001 — Perfis Chrome órfãos em temp e prefixo não auditável

| Campo | Valor |
|-------|--------|
| Identificador | `GAP-WS-TMP-PROFILE-ORPHAN-001` |
| Status | **RESOLVIDO** em **v1.0.0** (documentado 2026-07-15; implementado na mesma data) |
| Severidade | Média (disco / operação de agentes) |
| Relacionado | Residual de **GAP-WS-LIFECYCLE-001** / **ADR-0017**; fechado por **ADR-0020** |
| Código | `src/browser.rs`, `src/process_lifecycle.rs`, `src/main.rs`, `src/lib.rs`, `src/types.rs` |
| Prefixo | `USER_DATA_DIR_PREFIX = "ddg-chrome-"` (espelha atomwrite `.ddg-atomic-`) |

## Problema (histórico)

A CLI criava user-data-dir via `tempfile::tempdir()` com prefixo padrão **`.tmp`**. Em cancelamento, timeout (drop do future), panic path ou fan-out de `content_fetch`, diretórios de perfil podiam permanecer em `temp_dir` sem processo dono. Prefixo genérico impedia higiene seletiva sem risco a outras apps Rust.

## Consequências do problema

1. Acúmulo de disco em hosts de agentes multi-dia.
2. Auditoria ruidosa (“lixo em `/tmp`” vs Chrome desktop vs MCP).
3. `rm -rf …/.tmp*` inseguro (colide com terceiros).
4. Contrato one-shot incompleto no eixo disco.
5. Fan-out multiplica órfãos; deep-research com token isolado ignorava SIGTERM do main.

## Causa raiz do problema

One-shot de **processo** sem one-shot completo de **perfil em disco** + prefixo não auditável + `reap_all` sem `remove_dir_all` + SIGTERM só no token + deep com token isolado + dependência do Drop do `TempDir` (docs.rs: sinal/exit sem destructor vazam o dir).

## Relações causa × efeito

| Causa | Efeito | Fechamento v1.0.0 |
|-------|--------|-------------------|
| C1 — `tempdir()` default `.tmp` | Paths indistinguíveis | `Builder.prefix("ddg-chrome-")` + `0o700` Unix |
| C2 — remove só no `ChromeBrowser` | Dir fica sem Drop | `force_reap` global apaga dir |
| C3 — N browsers SERP+fetch | N órfãos no cancel | registry + reap_all + sweep |
| C4 — reap_all só kill | panic deixa disco | `remove_dir_all` + retry |
| C5 — “atexit” falso | falsa rede de segurança | `ExitReapGuard` + comentário honesto |
| C6 — SIGTERM só token | drain incompleto | reap síncrono + token unificado deep |
| C7 — TempDir sem Drop em exit | vazamento por crate | reap explícito não só Drop |
| C8 — SIGKILL/OOM | sem destructor | prefixo + `sweep_orphan_profiles` |
| C9 — stubs `org.chromium.*` | lixo Chromium | limite documentado |
| C10 — timeout dropa future | fetch sem shutdown | reap_all no timeout |
| C11 — deep token isolado | SIGTERM não cancela deep | token do `main` |
| C12 — Config Default 60 ≠ 180 | fence errada em callers | Default = 180 |

## Solução (implementada v1.0.0)

1. Prefixo `ddg-chrome-` + permissões owner-only (Unix).
2. `force_reap` / `reap_all_registered` removem `user_data_dir`.
3. `ExitReapGuard` no `main`; panic hook; reap em timeout e fim de pipeline/deep.
4. `sweep_orphan_profiles` na 1ª install do panic hook.
5. deep-research herda `CancellationToken` do main + timeout global com reap.
6. Testes unit (remove dir, sweep ignora `.tmp`, prefixo) + E2E lifecycle assert prefix.

## Benefícios da solução

- Auditoria seletiva; one-shot honesto em disco; menos residual sob SIGTERM/timeout; residual SIGKILL mensurável e limpável na próxima run; consistência com prefixos `.ddg-*`.

## Como foi solucionado (checklist — todos feitos)

1. [x] `Builder` prefix `ddg-chrome-` em `ChromeBrowser::launch`
2. [x] `force_reap` + `remove_dir_all` + retry
3. [x] Registry poison-safe; todas as sessões no registry até reap
4. [x] Cancel/timeout/fim de run: reap síncrono
5. [x] Panic hook + `ExitReapGuard` no `main`
6. [x] Unit + integration tests
7. [x] ADR-0020 + CHANGELOG + inventário **RESOLVIDO**

## Política dura de higiene em disco (v1.0.0 — código + testes)

Estas regras são **mandatórias** em `process_lifecycle` (`is_cli_owned_profile_name` / `is_forbidden_bulk_delete_name` / `remove_user_data_dir` / `sweep_orphan_profiles`):

1. **Não apaga** órfãos legados `.tmp*` (prefixo genérico do `tempfile` / apps Rust de terceiros).
2. **Não apaga** stubs globais `org.chromium.Chromium.*` (Chrome desktop, Flatpak, MCP).
3. **SIGKILL / OOM**: residual possível sem destructor; a **próxima invocação** faz `sweep_orphan_profiles` **somente** em `ddg-chrome-*` (`ExitReapGuard::new` no `main` + 1ª install do panic hook).

## Fora de escopo / limites honestos

- **SIGKILL / OOM 100% limpo** sem agente externo: impossível no SO; mitigado pelo sweep prefixado.
- **Limpeza manual de `.tmp*` legados 0.9.x**: operador/host (`tmpfiles`, reboot) — a CLI **recusa** auto-rm genérico.
- **MCP chrome-devtools** e **Chrome Flatpak do usuário**: outros produtos.
- **Stubs `org.chromium.Chromium.*`**: fora do contrato; nunca bulk-delete.

## Referências

- ADR-0020, ADR-0017
- docs.rs `tempfile::TempDir` / `Builder`
- GraphRAG rules-rust-cli-one-shot, processos-externos, shutdown-*
