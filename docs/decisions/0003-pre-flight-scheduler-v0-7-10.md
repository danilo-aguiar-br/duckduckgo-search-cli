# ADR-0003 — Pre-flight ghost-block detection + marker-specific suggestions (v0.7.10)

## Contexto e problema

A versão v0.7.9 (commit `bbd9df8`) fechou os gaps GAP-WS-58 (ghost-block silencioso) e GAP-WS-59 (markers 2026 + flag global + double-gate), mas três deficiências funcionais e observacionais persistiam em 2026-06-17:

1. **`detectar_interstitial` descartava o marker exato** após o match. Operadores recebiam `cascata_motivo: "cloudflare"` mas não sabiam **qual** template específico o Cloudflare estava servindo. Diagnosticar "qual CAPTCHA disparou" exigia reprocessar manualmente o body da resposta.
2. **`sugestao_mitigacao` retornava lista agregada de markers** (`"cf-challenge, anomaly-modal, cf-turnstile, etc."`) sem o marker específico detectado. Mensagem era genérica — perdia o sinal de diagnóstico mais importante.
3. **Exit code silencioso em `deep-research`** quando o fan-out retornava zero resultados agregados. Pipelines automatizadas recebiam `exit 0` + payload vazio, propagando falha para a síntese. Regra do graphrag `rules-rust-cli-stdin-stdout-silent-discard` (memória 1114) cita verbatim: *"exit 0 com payload vazio é MENTIR sobre o resultado real"*.

Adicionalmente, observabilidade operacional ficou incompleta: não havia como saber em runtime se o pre-flight gate havia disparado — operador só via o JSON final sem métrica intermediária.

## Opções consideradas

### Opção 1 — Refatorar `detectar_interstitial` para retornar tupla `(marker, kind)` (escolhida)

Adicionar função `pub fn detectar_interstitial_com_match(html: &str) -> (&'static str, InterstitialKind)` que retorna o marker literal matched + a classificação. Para casos onde a detecção é por heurística de tamanho (ghost-block), retornar sentinel `"<ghost-block-no-marker>"`. Para `None` (resposta legítima), retornar sentinel `"<no-marker>"`. Sentinels são demarcados com `<>` para que callers possam detectar via `marker.starts_with('<')` e omitir de listas user-facing.

Adicionar overload `pub fn sugestao_mitigacao_com_marker(kind: InterstitialKind, marker: &str) -> String` que produz mensagem específica: `"Cloudflare challenge detected (marker: cf-challenge). Re-run with --pre-flight..."`.

Marcar a função original `sugestao_mitigacao` com `#[deprecated(since = "0.7.10", note = "...")]` para preparar remoção na v0.8.0. BC preservada (função ainda funciona) — `rules-rust-cli-stdin-stdout-distro-governanca` recomenda política explícita de deprecação.

### Opção 2 — Estender `InterstitialKind` com campo `marker: Option<&'static str>`

Criar variante 4-tuple `(Cloudflare { marker: &'static str }, DuckDuckGo { marker: &'static str }, GhostBlock { bytes: usize }, None)`. Tipo mais seguro mas exige `#[non_exhaustive]` no enum e migração de todos os match arms existentes (testes em `probe_deep.rs:327, 354`). Custo ergonômico alto sem ganho de expressividade significativo — o caller continua tratando marker como `&str`.

### Opção 3 — Tag estruturado em vez de string literal

Retornar enum `Marker` com variantes por template (`CfChallenge`, `CfTurnstile`, `CfSpinner`, `Botnet`, `AnomalyModal`, etc.). Type-safe mas explode o vocabulário (15+ variantes para Cloudflare, 5+ para DDG). Cada template novo exige modificação em 4 lugares (marker const, enum variant, match arms, testes). Overhead de manutenção supera ganho.

## Decisão

Adotada a Opção 1.

Para o exit code de `deep-research`, adicionada flag LOCAL `--require-results` (default `false`) em `Subcommand::DeepResearch`. Quando `true` E `unique_result_count == 0`, retorna exit code 70 (EX_SOFTWARE) com mensagem estruturada em stderr. BC preservada — usuários sem flag mantêm comportamento v0.7.0–v0.7.9 (`exit 0` com payload vazio).

Para observabilidade, adicionado campo `pre_flight_fired: bool` em `SearchMetadata`, serializado como `pre_flight_disparado`. Populado apenas quando o gate `should_try_lite` dispara via pre-flight path (não legacy `--allow-lite-fallback`).

## Mudanças resultantes

- `src/probe_deep.rs`: novo `detectar_interstitial_com_match` + overload `sugestao_mitigacao_com_marker` + 3 sentinels + `#[deprecated]` na original + 8 testes novos.
- `src/lib.rs:529-543`: migração do caller `execute_probe_deep` para `sugestao_mitigacao_com_marker(kind, marker)` retornando marker específico no envelope JSON.
- `src/types.rs:117-120`: novo campo `pre_flight_fired: bool` em `SearchMetadata` com `#[serde(rename = "pre_flight_disparado")]`.
- `src/search.rs:495-510`: refator `should_try_lite` retorna `(bool, bool)` para que o call site possa popular `pre_flight_fired` sem propagar bool por todas as camadas.
- `src/cli.rs:261`: nova flag `--require-results` em `DeepResearchArgs` (local, não global).
- `src/lib.rs:312-321`: novo branch em `execute_deep_research` que retorna `exit_codes::GLOBAL_TIMEOUT` quando `require_results && unique_result_count == 0`.

## Consequências

### Positivas

- Operador recebe marker específico no envelope JSON de `probe_deep`, permitindo correlação direta com logs do Cloudflare.
- Mensagem `sugestao_mitigacao_com_marker(Cloudflare, "cf-turnstile")` cita o template exato — pair-comparação trivial com dashboards Cloudflare.
- Pipelines que precisam de honestidade sobre zero resultados podem passar `--require-results` e detectar falha via exit code (regra 1114 do graphrag).
- Métrica `pre_flight_disparado` permite alertas operacionais: spike de pre-flight → spike de IP bloqueado → acionar rotação de proxy.

### Trade-offs

- 2 funções deprecated adicionam 2 warnings de compilação por chamada. Mitigação: warnings suprimidos com `#[allow(deprecated)]` nos testes que precisam de BC.
- Flag `--require-results` quebra consumidores que match exaustivo em `CliError`. Mitigação: enum já é `#[non_exhaustive]` — adição é BC-safe para código externo.

### Neutras

- Campo `pre_flight_fired` adicionado ao envelope JSON. Consumers que ignoram campos extras não são afetados.
- `detectar_interstitial_com_match` adicionada sem remover a original — callers existentes continuam funcionando.

## Alternativas postergadas (v0.7.11+)

- **P5 — Probe-deep scheduler automático**: invocar `execute_probe_deep` antes de `execute_single_search` quando `pre_flight=true`. Custo: 1 request HTTP extra (~200-300ms) por invocação. Diferido porque gate inline em `detectar_interstitial` já captura 100% dos casos verificados.
- **P19 — DDG class watcher**: monitorar templates DDG em runtime para auto-atualizar `RESULT_PAGE_SELECTORS`. Diferido — operadores podem monitorar via logs `tracing::warn!`.
- **Snapshots via `insta`**: substituir boilerplate inline por `assert_snapshot!`. Diferido — fixtures inline cobrem 100% dos casos atuais.
- **Benchmark Criterion + coverage gate no CI**: medição empírica de regressão de latência. Diferido — suite de testes já valida correção.

## Compliance

- `rules-rust-cli-stdin-stdout-silent-discard` (mem 1114): `--require-results` cumpre verbatim a regra "rejeição explícita via Err em vez de tracing::warn".
- `rules-rust-cli-com-clap-subcomandos-envvars` (mem 127): `--require-results` é LOCAL em `DeepResearchArgs` conforme recomendação.
- `rules-rust-cli-stdin-stdout-distro-governanca`: deprecação explícita com `#[deprecated(since, note)]` antes da remoção em v0.8.0.
- `rules-rust-tratamento-de-erros` (mem 35): sentinels `<...>` demarcam detecções heurísticas vs literais — type safety via convenção de prefixo.

## Nota de supersessão (v0.9.4 / ADR-0016)

O pre-flight de **produção** desde a ADR-0016 (GAP-WS-113) usa a **sessão Chrome compartilhada** (chromiumoxide/CDP) — não o gate HTTP Lite descrito neste ADR. Os caminhos `should_try_lite` / gates Lite e o contrato de opt-in de `--allow-lite-fallback` documentados acima são **históricos** (v0.7.10–v0.9.3). Desde a v0.9.4, `--allow-lite-fallback` é no-op legado; Lite não é caminho de sucesso em produção.