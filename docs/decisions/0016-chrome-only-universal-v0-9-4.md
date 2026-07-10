# ADR-0016: Chrome-only universal transport (GAP-WS-113 / v0.9.4)

## Status

Accepted — 2026-07-10

## Context

ADR-0007 made Chrome **primary** but still allowed `reqwest` for probe, fetch-content, and silent SERP fallback. In production, dual transport produced:

- HTTP soft-block / CAPTCHA with probe reporting `200 OK`
- `--allow-lite-fallback` forcing Lite under Chrome → zero hits + `causa_zero: legitimo` on ~26KB bodies
- Agents treating empty results as empty index

## Decision

1. **chromiumoxide/CDP is the only production network transport** for search, news, deep-research, probe, probe-deep, pre-flight, and fetch-content.
2. Chrome failure is a **structured error** — never silent zero results.
3. Lite is **never** a success path; `--allow-lite-fallback` is a no-op.
4. `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` **fails closed** (exit 2).
5. Residual HTTP lives only under feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` for tests.

## Consequences

- Production hosts need Chrome/Chromium (and Xvfb on headless Linux when required).
- Wiremock integration tests must enable `http-test-harness` and `HTTP_TEST=1`.
- ADR-0007 residual HTTP paths are **superseded** by this decision.
- Classifier rejects large bodies without organic cards as non-legitimo.

## Related

- `gaps.md` GAP-WS-113
- Supersedes residual HTTP / dual-transport parts of ADR-0007
- Supersedes auto-degradation transport policy of ADR-0012 (GAP-WS-106)
- Updates ADR-0010 / ADR-0011 chrome-less behavior to fail-closed exit 2
- `src/browser.rs` `require_chrome_transport`
- `src/pipeline.rs`, `src/parallel.rs`, `src/lib.rs`, `src/content_fetch.rs`
