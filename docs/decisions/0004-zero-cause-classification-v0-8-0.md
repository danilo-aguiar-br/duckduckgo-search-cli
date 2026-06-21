# ADR-0004 — Zero-result causal classification + exit 6 SUSPECTED_BLOCK (v0.8.0)

## Contexto e problema

A versão v0.7.10 (commit `bbd9df8`) fechou os gaps GAP-WS-50 a WS-57 (anti-bot detector overhaul) mas deixou em aberto o GAP-AUD-003 documentado em `gaps.md:478+` (auditoria local de 2026-06-19). O problema central: quando `duckduckgo-search-cli` retornava `quantidade_resultados: 0`, o operador não conseguia distinguir entre quatro causas semanticamente distintas:

- **Zero legítimo** — query genuinamente sem matches no índice do DDG naquele instante.
- **Filtro silencioso do DDG** — query contém termos sinalizados e foi dropada sem interstitial detectável.
- **Ghost-block do Cloudflare** — HTTP 200 com HTML sub-4KB sem marcadores literais.
- **Anti-bot explícito** — HTTP 202 / 403 / interstitial CF/DDG detectado.
- **Resposta inválida ou truncada** — body vazio, JSON malformado, proxy interceptando.

Três variantes empíricas foram reproduzidas em 2026-06-19, todas retornando exit 5 (`ZERO_RESULTS`) indistinguíveis entre si:

- **Variante A**: `tempo_execucao_ms > 0`, `pre_flight_disparado: false`, `retentativas: 0`, `fetches_simultaneos: 0`.
- **Variante B**: todos os campos de `metadados` retornam `null` incluindo `tempo_execucao_ms`.
- **Variante C** (inferida): `tempo_execucao_ms > 0`, `retentativas > 0`, `fetches_simultaneos > 0`, `resultados: []`.

A consequência operacional foi tripla: (1) operador gasta tempo em queries que retornam zero por bloqueio sem feedback causal; (2) pipelines automatizados falham em cascata sem distinguir causa raiz entre dados ausentes e ambiente bloqueado; (3) cada retry imediato piora o score do Cloudflare e endurece o bloqueio progressivamente.

## Opções consideradas

### Opção 1 — Adicionar `causa_zero: Option<ZeroCause>` no envelope + exit code 6 SUSPECTED_BLOCK (escolhida)

Adicionar enum `ZeroCause` em `src/types.rs` com 5 variantes (`Legitimo`, `FiltroSilencioso`, `GhostBlock`, `AntiBot`, `RespostaInvalida`), `#[non_exhaustive] #[serde(rename_all = "kebab-case")]`. Implementar classificador puro `pipeline::classify_zero_result` que encadeia sinais (body vazio → pre_flight fired → interstitial marker → filtro silencioso → legítimo). Wire-in no `execute_single_search` e `execute_query_with_cancellation` apenas quando `quantidade_resultados == 0` (zero overhead no caminho de sucesso). Atualizar `lib.rs:241-243` para emitir exit 6 quando `causa_zero != Legitimo`, mantendo exit 5 para zero genuíno. BC opt-out via env var `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` para consumidores que ramificam em exit 5 e não querem migrar.

**Prós**: diferenciação causal completa, BC preservada via opt-out, telemetria rica para observabilidade, segue convenção semver-additive (códigos 0-5 estáveis, novos adicionados sem reassign).

**Contras**: adiciona 2 campos novos ao envelope JSON (causa_zero + sugestao_proxima_acao), exige 11 sites de `SearchMetadata { ... }` atualizados, requer expor `first_body` em `AggregatedSearchResult` (custo de memória).

### Opção 2 — Manter exit 5 e adicionar apenas telemetria `causa_zero` no envelope (rejeitada)

Seria menos invasiva mas não fecha o gap para pipelines que ramificam exclusivamente em exit code. Operadores em shell scripts não teriam como distinguir bloqueio de zero legítimo sem parsing de JSON.

### Opção 3 — Substituir exit 5 por exit 6 incondicional (rejeitada)

Quebraria BC de todos os pipelines v0.7.x que ramificam em exit 5. Sem opt-out.

## Decisão

Adotada a **Opção 1** com refinamentos:

1. **Enum `ZeroCause` com 5 variantes** marcadas `#[non_exhaustive]` para permitir adições futuras sem breaking change. Serializa como kebab-case via `#[serde(rename_all = "kebab-case")]`.
2. **Classificador puro** `pipeline::classify_zero_result` aceita `ZeroClassificationInputs` struct (não 7 params soltos — atende regra de máximo 5 params de `rules-rust-principios-legibilidade`).
3. **Chain causal documentada inline** em comentários Rust:
   - CR1 — RespostaInvalida (body vazio + telemetria zerada = Variante B).
   - CR2 — AntiBot (pre_flight disparou).
   - CR3 — GhostBlock ou AntiBot (marker detection via `probe_deep::detectar_interstitial_com_match`).
   - CR4 — FiltroSilencioso (body < 4KB + latência >= 200ms + sem signal).
   - CR5 — Legitimo (default).
4. **Wire-in conservador**: classificador roda APENAS quando `quantidade == 0` (zero overhead no caminho de sucesso).
5. **Exit code 3-way** em `lib.rs:241-243`: pre_flight_blocked → 3, non-legitimo + strict → 6, zero → 5, success → 0.
6. **BC opt-out** via env var `DUCKDUCKGO_ZERO_CAUSE_STRICT` (default ON, aceita `false`/`0`/`no`/`off`/vazio).
7. **Histograma agregado** em `MultiSearchOutput.causa_zero_histogram: BTreeMap<String, u32>` — `BTreeMap` garante ordem lexicográfica determinística no JSON output.
8. **Strings PT-BR determinísticas** por variante via `sugestao_proxima_acao_para_zero`, alinhadas ao padrão `sugestao_mitigacao_com_marker` em `probe_deep.rs`.
9. **`AggregatedSearchResult.first_body: String`** exposto para o classificador ter acesso ao body da primeira página sem precisar re-fetch.

## Cadeia causal patch → efeito

```
Patches em src/types.rs (campos novos)
  → classificador tem tipo para classificar
    → wire-in em pipeline.rs + parallel.rs popula causa_zero + sugestao
      → exit code branch em lib.rs distingue legitimo de non-legitimo
        → exit 6 sinaliza bloqueio em pipelines automatizados
          → operadores migram para --pre-flight automaticamente
            → bot score do Cloudflare decai
              → taxa de zero-result por bloqueio converge para zero
                → exit 5 volta a ser signal útil de "dados ausentes"
```

## Consequências

### Positivas

- Operador recebe classificação causal no envelope JSON (`metadados.causa_zero: "anti-bot"`).
- Exit code distinto para bloqueio suspeito (6) facilita detecção em pipelines automatizados.
- Sugestão automática orienta o operador para a próxima ação concreta via `metadados.sugestao_proxima_acao`.
- Pipelines automatizados podem distinguir `Legitimo` (não tem dados) de `AntiBot` (ambiente bloqueado) sem rodar probe manualmente.
- Telemetria de `causa_zero_histogram` alimenta dashboards de observabilidade com histograma por causa.
- Reduz desperdício de retry em ambiente bloqueado porque operador sabe imediatamente que retry piora score.
- Subcomando `deep-research` propaga classificação causal para todas as sub-consultas, melhorando qualidade da síntese.

### Tradeoffs

- 2 campos novos no envelope (`causa_zero` + `sugestao_proxima_acao`) — mínimo impacto em consumidores que ignoram campos extras.
- `first_body: String` em `AggregatedSearchResult` adiciona ~4KB de uso de memória por busca multi-query.
- 11 sites de literal `SearchMetadata { ... }` atualizados — risco de regressão coberto por 488 testes passando.
- `BTreeMap` em vez de `HashMap` no histograma tem custo O(log n) insignificante para n <= 12 sub-queries.

## Validação

- `cargo check --offline` exit 0.
- `cargo clippy --all-targets --offline -- -D warnings` exit 0 (zero warnings).
- `cargo test --offline` 488 testes passando (361 lib + 127 integration), 0 falhando.
- 12 unit tests novos em `src/pipeline.rs` cobrindo todas as 5 variantes do enum + mensagens de sugestão.
- Reprodução empírica em 2026-06-19: `timeout 30 duckduckgo-search-cli "rust serde derive" -q -f json` confirmou Variante A do GAP-AUD-003 com `causa_zero: "anti-bot"` (após classificador wire-in).
- `timeout 15 duckduckgo-search-cli --probe-deep -q -f json` retornou `status: "captcha"` HTTP 202 confirmando ambiente bloqueado — o classificador captura esse cenário via Variante A → AntiBot.

## Referências cruzadas

- `src/types.rs:23-34` — `ZeroCause` enum.
- `src/types.rs:148-164` — `SearchMetadata.zero_cause` + `sugestao_proxima_acao`.
- `src/types.rs:236-243` — `MultiSearchOutput.causa_zero_histogram`.
- `src/pipeline.rs:491-558` — `classify_zero_result` + `sugestao_proxima_acao_para_zero`.
- `src/pipeline.rs:418-441` — wire-in em `execute_single_search`.
- `src/parallel.rs:601-625` — wire-in em `execute_query_with_cancellation`.
- `src/parallel.rs:272-289` — agregação do histograma.
- `src/lib.rs:241-290` — branch de exit code 3-way.
- `src/error.rs:196` — `assert_eq!(SUSPECTED_BLOCK, 6)`.
- `src/search.rs:430-447` — `AggregatedSearchResult.first_body`.
- `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md` — predecessor que fechou 8 gaps mas deixou classificação causal em aberto.
- `docs/decisions/0003-pre-flight-scheduler-v0-7-10.md` — predecessor que adicionou pre-flight opt-in mas não integrou classificador causal.
- `gaps.md:478+` — GAP-AUD-003 original.