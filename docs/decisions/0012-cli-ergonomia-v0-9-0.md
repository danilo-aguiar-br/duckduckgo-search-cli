# ADR-0012: CLI Ergonomics — Global Flags, Actionable Errors, Chrome-less Auto-degradation (v0.9.0)


## Status
- Accepted (2026-07-07)
- Extends: ADR-0010 (News Vertical, v0.8.9), ADR-0011 (Deep-Research Dual Web+News, v0.8.9)
- Closes: GAP-WS-106 (three symptoms with a common root cause — absence of a uniform ergonomic policy at the clap parser level)


## Context
- Three apparently independent symptoms shared one root cause: there was no organizing principle for parser ergonomics, so each new flag inherited whatever locality its author happened to choose
- Symptom A: clap emitted a generic `unexpected argument` message when a flag was misplaced after the `deep-research` subcommand; the user could not tell whether it was a typo, a wrong position, or a nonexistent flag — zero actionability
- Symptom B: the most-used flags (`-q`, `-o`, `-n`, `-f`, `-t`, `-l`, `-c`, `-p`, `-v`) were declared as plain fields of `CliArgs` without `global = true`, so clap accepted them ONLY before the `deep-research` subcommand — `duckduckgo-search-cli "query" -q -o out.json` aborted with `unexpected argument`
- Symptom C: builds without the `chrome` feature treated feature absence as a fatal failure — `deep-research` without `--no-news` AND `--vertical news|all` both aborted with exit 2 `INVALID_CONFIG` (no auto-degradation); CI pipelines were forced to know and pass the `--no-news` flag on every deep-research invocation, and shell aliases diverged between chrome and no-chrome builds
- Prior piecemeal fixes (GAP-WS-058/059/B3) hoisted only `--allow-lite-fallback`, `--pre-flight`, and `--global-timeout` to `global = true`; the most-used flags remained local, creating inconsistency between sibling flags
- `--allow-lite-fallback` being `global = true` while `-q` was NOT made the inconsistency visible and pointed at the missing organizing principle


## Decision
- Hoist nine flags to `global = true` IN-PLACE (without moving the fields out of `CliArgs`): `-n`, `-f`, `-o`, `-t`, `-l`, `-c`, `-p`, `-q`, `-v` — they may now appear before OR after the `deep-research` subcommand (mirrors the precedent set by `--allow-lite-fallback`/`--pre-flight`/`--global-timeout`)
- `verbose` (`-v`) was also hoisted to preserve the existing `conflicts_with = "quiet"` invariant between the two flags
- Intercept clap errors in `run()` via `RootArgs::try_parse()` instead of `RootArgs::parse()`: on `ErrorKind::UnknownArgument`, look up the offending flag and — when it is a known global flag — append a PT-BR hint pointing to the correct placement; then print to stderr and exit with `INVALID_CONFIG`
- Add `pub fn is_known_global_flag(&str) -> bool` in `src/cli.rs` — a small allowlist of the 8 hoisted shorts/longs plus the existing `CliArgs` locals; the hint is emitted ONLY for known flags (genuine typos still get the default clap message, no misleading suggestion)
- Auto-degradation in `execute_deep_research` (Sintoma C, deep-research path): compute `effective_no_news = true` when the build lacks `chrome`, `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` is set, or `detect_chrome` fails; emit a stderr warning via `output::emit_stderr`; use `effective_no_news` in `DrArgs` and `Config.vertical`; the previous fail-fast exit 2 (`INVALID_CONFIG`) is removed
- Auto-degradation in `build_config` (Sintoma C, normal search path): when `--vertical news|all` is requested but the build lacks `chrome` or `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`, emit a stderr warning and downgrade to `VerticalMode::Web`; the previous `Err(InvalidConfig)` is removed
- `--no-news` is kept as an explicit no-op opt-out for backward compatibility — passing it in a chrome-enabled build still disables the news scan
- Rejected alternative: implement `clap::error::ErrorFormatter` trait — would require rebuilding the `RichFormatter` (colours, context, suggestions) for a marginal benefit; intercepting `try_parse` is surgical and preserves all default clap behaviour
- Rejected alternative: `Command::error_formatter::<F>()` — requires the builder API and loses the typed `RootArgs` of the derive API; not worth the migration cost
- Rejected alternative: hoisting by MOVING fields from `CliArgs` to `RootArgs` — would require rewriting ~30 references in `build_config`; in-place `global = true` on `#[command(flatten)] Args` fields works for parsing (validated by clap issue #5984 and by the existing `--allow-lite-fallback` precedent)


## Consequences
- Users write flags in any order without aborts; shell aliases become portable between chrome and no-chrome builds
- clap errors now point at the exact correction (PT-BR hint), reducing support burden and onboarding friction
- CI pipelines no longer need to defensively pass `--no-news`; existing `--no-news` calls remain valid (no-op when Chrome is present, explicit opt-out when it is absent)
- The inconsistency between sibling flags (`-q` local, `--allow-lite-fallback` global) is eliminated — all nine most-used flags now follow the same global precedent
- No JSON schema changes, no new envelope fields, no new exit codes — existing consumers keep working unchanged
- Documented gap: the embedded skill `duckduckgo-search-cli-pt` in CLAUDE.md still says `PROIBIDO --vertical news sem chrome`; after v0.9.0 it is accepted with a warning. CLAUDE.md is intentionally untouched (project rule forbids editing it); the imprecision will be corrected in a future skill maintenance pass outside v0.9.0 scope


## Files Changed
- src/cli.rs: `global = true` on nine `#[arg]` (`-n`, `-f`, `-o`, `-t`, `-l`, `-c`, `-p`, `-q`, `-v`); new `pub fn is_known_global_flag(&str) -> bool`; 3 regression unit tests in `mod tests`
- src/lib.rs: `run()` block `try_parse` with PT-BR conditional hint; `execute_deep_research` block `effective_no_news`; `build_config` block `let vertical = if args.vertical == CliVertical::Web`; `convert_vertical` gains `#[cfg_attr(not(feature = "chrome"), allow(dead_code))]`
- tests/global_flags.rs: new — 2 E2E tests covering Sintomas A (hint on misplaced known flag) and C (`--vertical all` without chrome no longer aborts with exit 2)
- Cargo.toml: `version = "0.9.0"`


## Validation
- `cargo build` — ZERO errors, ZERO warnings
- `cargo build --no-default-features` — ZERO errors, ZERO warnings
- `cargo test --lib cli::` — 34 tests passing (includes 3 new regression tests)
- `cargo test --test global_flags` — 2 E2E tests passing


## Superseded transport policy (v0.9.4)

- The **auto-degradation** decisions in this ADR (auto `--no-news` / Web downgrade without Chrome) are **historical only** for v0.9.0–v0.9.3
- **ADR-0016 / GAP-WS-113 (v0.9.4)** restores fail-closed exit 2 without Chrome for all production network operations
- Global flag hoisting and actionable clap hints from this ADR remain in force

## References
- ADR-0010 (News Vertical, v0.8.9) — chrome-less guard history; transport policy finalized in ADR-0016
- ADR-0011 (Deep-Research Dual Web+News, v0.8.9) — auto-degradation temporary; finalized fail-closed in ADR-0016
- ADR-0016 (Chrome-only universal, v0.9.4) — **supersedes** this ADR's transport auto-degradation
- gaps.md (GAP-WS-106, GAP-WS-113)
- CHANGELOG.md [0.9.0], [0.9.4]
- docs.rs `clap::Arg::global` — confirms `global = true` usage in any position
- docs.rs `clap::error::ErrorFormatter` — confirms the trait exists (decided NOT to use)
- clap issue #5984 — confirms `global = true` on `flatten Args` fields works for parsing
- Commit: fc035c2
