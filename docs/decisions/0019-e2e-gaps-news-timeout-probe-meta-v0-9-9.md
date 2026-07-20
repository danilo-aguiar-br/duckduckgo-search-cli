# ADR-0019 — E2E gaps: news quality, timeout 180, probe honesty, agent meta (v0.9.9)

## Status

Accepted — implemented in **v0.9.9**.

## Context

Post-release e2e on 0.9.8 (Fedora + Chromium host) found critical product bugs:

1. News vertical returned DDG UI promos (iOS/Android/Duck.ai) — false success.
2. Default agent-ready path (vertical `all` + content fetch) exceeded `--global-timeout` default of 60s → exit 4.
3. Exit 4 produced empty stdout (agent parsers broke).
4. `--probe` false-negative (403) while real SERP worked.
5. Misleading agent metadata (`pre_flight_disparado`, chrome path under `NO_CHROME`, wall-clock timing).

## Decision

| Area | Decision |
|------|----------|
| News | Denylist promo URLs; full-document fallback only after filter; prefer honest empty over promo |
| Timeout | `DEFAULT_GLOBAL_TIMEOUT = 180` |
| Exit 4 | Emit JSON envelope (`erro: "timeout"`) before exit |
| Probe | Calibration URL with `q=`; healthy via SERP signals; `status: "ok"|"blocked"` |
| Meta | `pre_flight_executado`, `news_filtradas_promo`, stream honesty fields; NO_CHROME clears chrome meta |
| Quiet | `-q` → tracing off |
| PathError | Display is `{message}` only (no forced "invalid output path" prefix) |
| Lifecycle | Keep one-shot: `Browser::close`/`wait` + `process_lifecycle` / `XvfbGuard` (docs-rs chromiumoxide) |
| Telemetry | None — local agent metadata only |

## Consequences

- News may return `quantidade_noticias: 0` when only chrome UI is present (honest).
- Operators depending on 60s global timeout must pass `--global-timeout 60` explicitly.
- Probe JSON schema aligns with string `status` + optional `http_status`.

## Gaps closed

All inventário e2e IDs plus NEWS-FANOUT, SELECTORS-XDG (honest empty), PROBE-SCHEMA, STREAM-MULTI honesty — see root `gaps.md` (local gitignored inventory).
