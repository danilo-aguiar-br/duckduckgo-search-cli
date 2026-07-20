# ADR-0018 — Agent-ready multi-canal Chrome, dual web+news, clean text (v0.9.8)

## Status

Accepted — implemented in **v0.9.8**.

## Context

GAP-WS-AGENT-READY-001: on Linux hosts with **Google Chrome Flatpak** and **Chromium RPM** side by side, auto-detect rejected Flatpak exports (shell scripts) and never listed deploy ELFs under `files/extra/chrome`. Search defaulted to vertical `web` only; `fetch_content` was opt-in; news cards had no clean body; `--chrome-path` after `deep-research` failed clap parse (exit 2); fan-out skipped UA coercion; news-only could report `usou_chrome=false` after a real Chrome session.

## Decision

1. **Multi-canal resolve** (`browser.rs`): resolve Flatpak export shells and Fedora Chromium wrappers to real ELF binaries; include Flatpak deploy paths in candidates; `needs_no_sandbox` for `/flatpak/app/` and `files/extra/chrome`; expose `ChromeChannel` + `detect_chrome_resolved`.
2. **Default vertical `all`** for search (deep already dual unless `--no-news`); opt-out `--vertical web`.
3. **Default content fetch ON** (`!no_fetch_content`); `--no-fetch-content` opt-out; top-10 URL cap per vertical; news `conteudo` via Chrome readability.
4. **Transport flags `global = true`**: `--chrome-path`, `--proxy`, `--vertical`, fetch flags, identity, etc.
5. **Shared `coerce_chrome_user_agent`** for single-path and fan-out.
6. **Honest agent metadata** (not telemetry): `chrome_path_resolvido`, `chrome_canal`, `usou_chrome` on **single-query**, **multi-query fan-out**, **failure envelopes**, and **deep-research** (`DeepResearchMetadata`).
7. **One-shot** preserved (ADR-0017); chromiumoxide-only production (ADR-0016); **atomwrite** for disk; **no telemetry**.
8. **`BrowserConfigBuilder::surface_invalid_messages`** enabled at launch (docs.rs) so unparseable CDP frames surface as errors instead of silent drops.
9. **Closed product decisions (not open backlog):** keep internal CDP+readability pipeline (no forced crate swap); no separate `--agent` flag (defaults are agent-ready; opt-out via `--vertical web` / `--no-fetch-content` / `--no-news`).
10. **Inventory** lives only in root `gaps.md` (gitignored + Cargo exclude; local audit, not published).

## Consequences

- Breaking: default JSON may include `noticias` and `conteudo` more often.
- Flatpak Chrome becomes usable when deploy ELF exists; launch still requires host-compatible libs (`--no-sandbox`).
- Longer default latency when fetch is on (bounded by FETCH_CAP=10).
- Deep envelope gains additive fields `usou_chrome`, `chrome_path_resolvido`, `chrome_canal`.

## References

- GAP-WS-AGENT-READY-001 L-01…L-08 + residuals R-01…R-12
- ADR-0016 Chrome-only, ADR-0017 one-shot lifecycle
- chromiumoxide `BrowserConfigBuilder::{chrome_executable, no_sandbox, surface_invalid_messages}`
