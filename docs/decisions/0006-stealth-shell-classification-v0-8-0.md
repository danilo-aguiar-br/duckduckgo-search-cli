# ADR 0006: Stealth Shell Classification for `classify_zero_result` (v0.8.0)

## Status

Accepted (2026-06-19)

## Context

Em 2026-06, observamos que o DuckDuckGo serve "stealth shell" — HTTP 200 com HTML de 14KB que imita a home page mas não contém marcadores literais de interstitial (`anomaly-modal`, `cf-chl-bypass`, etc.). O classificador `classify_zero_result` (v0.7.x) só marcava `GhostBlock` quando `body.len() < 4000`, causando falso negativo para stealth shell de 14KB.

Reproduzido em produção real: query "resultado do primeiro jogo do Brasil na Copa do Mundo 2026" retornava `quantidade_resultados: 0` com `causa_zero: "legitimo"`, padahal o IP do operador estava sob bloqueio stealth do Cloudflare (confirmado por `--probe-deep` retornando `status: "captcha"`).

A classificar incorretamente como Legitimo:
- Operador não sabe que precisa tentar `--endpoint lite` ou aguardar 5 minutos
- Pipelines automatizados tratam zero-result como "consultar outra fonte" quando na verdade é bloqueio
- Logs de telemetria (`metadados.bytes_descomprimidos`) registram 14KB mas sem classificação útil

## Decision

Adicionar branch CR4b em `classify_zero_result` (`src/pipeline.rs:639-649`) que classifica como `GhostBlock` quando TODAS as 4 condições são satisfeitas simultaneamente:

1. `body.len() >= 4000` — stealth shell é maior que ghost block tradicional
2. `!probe_deep::has_result_page_signal(body)` — não é result page real (sem `result__a`)
3. `kind == InterstitialKind::None` — sem marker literal de Cloudflare/DDG
4. Body contém assinatura DDG (`search_form`, `DuckDuckGo`, ou `dropdown__button`)

A branch é conservadora — exige 4 condições simultâneas, eliminando falso positivo em resultados legítimos de baixa densidade (ex.: 1 resultado com 8KB de metadata).

## Alternatives Considered

### Threshold dinâmico baseado em histórico de tamanho de body

- **Rejeitado**: requer histórico de observações; cold start problemático em instalação nova
- Histórico seria específico por IP/região; não generalizável
- Aumenta complexidade do classificador sem benefício claro

### ML classifier treinado em corpus de stealth shells

- **Rejeitado**: adiciona dependência externa (`tract`, `onnx`, ou `linfa`)
- Aumenta binário em ~5-20MB
- Impossível treinar offline sem corpus rotulado pelo operador
- Cold start: novo usuário sem corpus recebe classificação errada

### Marker probing ativo (enviar 2-3 requests e comparar)

- **Rejeitado**: aumenta latência em 2-3x (multiplicador de requests)
- Viola princípio de "single request per query" do design original
- Anti-bot pode detectar probing pattern e bloquear mais agressivamente

### Manter threshold de 4000 + classificar 14KB como Legitimo

- **Status quo rejeitado**: falha do classificador é o bug que estamos resolvendo
- Falso negativo é pior que falso positivo (operador age errado em ambos os casos)

## Consequences

### Positivo

- 100% de detecção de stealth shell conhecida em 2026-06 (validado contra fixture comprimida)
- Classificador retorna `GhostBlock` em vez de `Legitimo` para ambiente bloqueado real
- Sinergia com auto-fallback lite (Phase F GAP-NEW-004): `GhostBlock` dispara fallback automático para `Endpoint::Lite`
- Operador recebe `sugestao_proxima_acao` acionável: "Aguarde 60s, troque de IP ou use --pre-flight"

### Negativo

- Marcador pode regredir se DDG mudar markup (mitigação: proptest em `src/pipeline.rs::property_tests_stealth_shell`)
- Threshold fixo de 4000 não se adapta a mudanças de tamanho do stealth shell (mitigação: ADR pode ser atualizado em release futuro se necessário)
- 4 condições simultâneas podem ser保守 demais em alguns casos (trade-off aceito: falso negativo raro > falso positivo comum)

### Validation

- `tests/integration_stealth_block_classification.rs` — 5 testes de regressão cobrindo CR4b + casos adjacentes
- `benches/zero_cause_bench.rs` — benchmark de overhead do classificador
- `tests/integration_e2e_real_world.rs:77` — caso Brasil 1x1 Marrocos
- Proptest em `src/pipeline.rs` (v0.8.0) com padding variável 5KB-100KB para fuzzing da 4-condição

## References

- ADR 0004 (zero-cause classification) — contexto pai
- ADR 0005 (HTTP decompression) — pre-requisito para classificador ter acesso ao body descomprimido
- `docs/decisions/0004-zero-cause-classification-v0-8-0.md` — classificador ZeroCause
- `src/pipeline.rs:639-649` — implementação CR4b
- `tests/integration_stealth_block_classification.rs` — regression tests
- `tests/integration_e2e_real_world.rs:77` — Brasil x Marrocos
