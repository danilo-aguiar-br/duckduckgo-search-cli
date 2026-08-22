# ADR-0024 — Contrato de orçamento do deep-research (v1.0.2)

## Status

Accepted — 2026-07-22

## Context

Na v1.0.1 o `deep-research` agent-ready partia com defaults pesados
(5 sub-queries × dual web+news × fetch ON × cap 10) cuja estimativa interna
era `~540 s`, enquanto `--global-timeout` e as skills usavam `180 s`.
A política CM-06 era `warn-only` (GAP-E2E-51-020): o run entrava em
timeout / exit 124 com stdout vazio — o sintoma “deep-research com erro”.

Auditoria: `gaps.md` GAP-AUD-DR-001…012.

## Decision
1. Fail-fast (CM-01): se `global_timeout < gated_estimate` e não
   `--allow-under-budget` / XDG `deep_research_allow_under_budget`, abortar
   `exit 2` com JSON `erro=budget_underflow` ANTES DE CHROME.
2. Defaults thin-enough (CM-02/03): `max_sub_queries=3`,
   `fetch_content_cap=4`, dual+fetch ON, depth 0 → estimate 144 s,
   gated (+10%) `159 s ≤ 180`.
3. Fórmula SSOT em `src/budget/` inclui `--depth` e margem
   `BUDGET_SAFETY_MARGIN_PERCENT` (default 10).
4. Timeout envelope (CM-05): emitir JSON em stdout/`-o` (atomwrite)
   ANTES de `ensure_oneshot_cleanup`.
5. `XDG` expõe knobs de orçamento (sem product env).
6. `--print-budget`: dry estimate sem Chrome.

## Consequences
- Happy path documentado `timeout 180 … deep-research` torna-se coerente.
- Carga 1.0.1 exige flags explícitas ou timeout maior (MIGRATION).
- Doctor reporta `deep_research_budget_ok`.
- probe-deep includes `deep_research_budget` SSOT (CM-10).
- SIGTERM force-exit emits minimal deep cancel envelope before oneshot reap (CM-05).
- Dual news `Err` must not collapse to empty list (CM-09).

## Alternatives rejected
- Só aumentar `DEFAULT_GLOBAL_TIMEOUT` para 540: outer `timeout 180` das
  skills continuaria matando o processo.
- Só warn-only (status 1.0.1): não bloqueia a causa raiz.

## References
- `src/budget/mod.rs`, `src/commands/deep_research.rs`
- `gaps.md` GAP-AUD-DR-001, GAP-E2E-51-020
- ADR-0018 agent-ready, ADR-0017/0020 one-shot lifecycle
