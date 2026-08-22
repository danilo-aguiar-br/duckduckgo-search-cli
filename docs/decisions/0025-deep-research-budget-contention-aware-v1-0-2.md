# ADR-0025 — Budget contention-aware + dual multiproc wall (v1.0.2)

## Status

Accepted — 2026-07-23

## Context

ADR-0024 introduced fail-fast budget gates with lab unit times (SERP=8s,
FETCH=5s). Under desktop hosts with 40+ Chrome processes, dual web+news
multiproc + fetch still exhausted `--global-timeout` (exit 4) while
`print-budget` reported `budget_ok=true` at GT=gated. Doctor soft text
pushed “prefer single-flight SERP”, conflicting with dual product goals.
Synthesis prose used `max(sources.len())` as sub-query count (false fan-out).

## Decision
1. Wall dual-aware (`src/budget/`): `runtime_dual_multiproc` vs
   `dual_vertical`; SERP/fetch wall parallel when multiproc ON; sequential
   when dual vertical with `-p 1`.
2. Contention factor from `count_chrome_like_processes()` (shared with
   doctor): bands low/high with XDG overrides; under factor>100%, floor wall
   by `work × factor`.
3. `print-budget` emits `suggested_global_timeout`, `shell_timeout_hint`,
   `runtime_dual_multiproc`, `parallelism`, `chrome_n`, counters.
4. `--auto-contention-budget` (default ON, XDG override): raise effective
   GT to suggested; `--no-auto-contention-budget` restores strict fail-fast.
5. Doctor: `ready_for_dual_deep_research`, `recommended_global_timeout`,
   dual-preserving next_action text.
6. Partial surface: `sub_queries_total/ok/erro`, `parcial`;
   `--require-all-sub-queries`; synth stats SSOT; timeout envelope OBS fields
   (agent contract, not phone-home telemetry).
7. Grace default 20s (XDG `deep_research_timeout_grace_seconds`).

## Consequences
- Happy path dual multiproc with chrome_n=0 still fits DEFAULT_GLOBAL 180.
- Contended hosts get honest suggested GT (often ≥300s).
- Agents must pass `--no-auto-contention-budget` to force exit 2 on tight GT.
- Skill may consume `suggested_global_timeout` instead of inventing GT_EFF.

## References
- `src/budget/*`, `src/process_count.rs`, `src/commands/doctor.rs`
- gaps.md V22 CLI-BUDGET-01…CLI-TEST-01, V21 partial/synth
- ADR-0024 budget contract
