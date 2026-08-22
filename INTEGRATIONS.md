# Integrations
Read this in [Portuguese](INTEGRATIONS.pt-BR.md).

`duckduckgo-search-cli` integrates with 16+ AI agents and automation platforms
via its stable JSON contract, deterministic exit codes, and zero-dependency
binary install. This file is a pointer to the full integration catalog.

## Full Catalog
See [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) for the complete
integration guide, including:

- 16 supported AI agents (Claude, GPT, Gemini, Cursor, OpenCode, etc.)
- Flag aliases introduced in each version
- Summary table consolidating all integrations
- Per-platform installation recipes
- Exit code semantics for agent decision-making
- Per-integration snippets with `timeout`, `jaq`, and `PIPESTATUS`

## Quick Reference
```bash
# Canonical invocation (v1.0.6 — English wire keys by default)
timeout 180 duckduckgo-search-cli -q -f json --num 15 "query"

# Exit codes
0  success         → parse .results
1  runtime error   → read stderr; retry once with -v
2  config error    → re-run init-config --force; ALSO agent-ops refusal (see v1.0.4/v1.0.5)
3  anti-bot block  → back off 300+ s; Chrome + `--proxy` / rotate identity (NOT lite / NOT `--allow-lite-fallback`)
4  global timeout  → raise --global-timeout; reduce --parallel
5  zero results    → refine query or try different --lang
6  suspected block → inspect .metadata.zero_cause; wait 300s or rotate proxy

# Full command inventory (v1.0.6)
# Default search (no subcommand):
duckduckgo-search-cli [OPTIONS] [QUERY]...
duckduckgo-search-cli -q -f json "query"                    # default search
# Hidden alias (same as default search; omitted from --help):
duckduckgo-search-cli buscar -q -f json "query"
# Subcommands:
duckduckgo-search-cli init-config                           # write selectors.toml + user-agents.toml to XDG
duckduckgo-search-cli init-config --dry-run                 # report actions without writing
duckduckgo-search-cli init-config --force                   # overwrite existing files
duckduckgo-search-cli completions bash                      # bash|zsh|fish|powershell|elvish
duckduckgo-search-cli deep-research --print-budget -q       # budget dry-run (no Chrome)
duckduckgo-search-cli deep-research "query" -q -f json      # fan-out + aggregate
duckduckgo-search-cli commands -q                           # JSON command tree (agent discovery)
duckduckgo-search-cli schema                                # list JSON Schema IDs
duckduckgo-search-cli schema --name search-output
duckduckgo-search-cli --print-schema                        # root alias of schema catalog
duckduckgo-search-cli --probe -q -f json                    # root pre-flight (separate from doctor)
duckduckgo-search-cli --probe-deep -q -f json               # deep pre-flight (captcha classification)
duckduckgo-search-cli doctor -q                             # environment / Chrome diagnostics JSON
duckduckgo-search-cli doctor --strict
duckduckgo-search-cli doctor --probe-deep
duckduckgo-search-cli locale -q                             # resolved UI locale JSON
duckduckgo-search-cli man | man -l -                        # roff man page from clap tree
duckduckgo-search-cli man --file /tmp/ddg.1
# config CRUD (RuntimeConfig SSOT — CLI > XDG > FACTORY; no product env):
duckduckgo-search-cli config path
duckduckgo-search-cli config list
duckduckgo-search-cli config get wire_keys
duckduckgo-search-cli config set wire_keys en
duckduckgo-search-cli config set budget_profile lab
duckduckgo-search-cli config unset proxy_url
duckduckgo-search-cli config effective
duckduckgo-search-cli help deep-research

# Agent ops (global; every surface since v1.0.4; no jq required):
#   --fields / --select, --filter, --sort, --dedupe-by, --limit,
#   --count-only, --truncate-content, --max-output-bytes, --wire-keys en|pt
# Since v1.0.4 each operator ACTS or REFUSES by name; there is no third outcome.
# Since v1.0.5 --probe and --probe-deep honour them too.
duckduckgo-search-cli "query" -q -f json \
  --fields url,title --filter 'title~rust' --sort title --limit 5
duckduckgo-search-cli "query" -q -f json --count-only
duckduckgo-search-cli "query" -q -f json --wire-keys pt     # legacy PT serialize
duckduckgo-search-cli commands -q -f json --fields agent_ops
duckduckgo-search-cli --probe -q -f json --fields status

# Legacy PT wire keys (pre-1.0.2 agents):
#   --wire-keys pt   OR   config set wire_keys pt

Current version: 1.0.6
```

## Breaking Change for Integrations — `discriminator` → `discriminator_key`
- Rename the field you read from the capability matrix published by `commands` under `agent_ops`.
- Read `agent_ops[].discriminator_key` on v1.0.5 and later.
- Stop reading `agent_ops[].discriminator`, which the envelope no longer emits.
- Expect the meaning to change with the name: the slot now carries the KEY, which is always `type`.
- Know that v1.0.4 published the VALUE (`doctor`, `schema_catalog`, `config_list`) under a field named for a key.
- Understand the defect: an agent that trusted the old matrix went looking for a key named `doctor`.
- Migrate by reading the key name from `discriminator_key` and matching it against the `type` field of the envelope you received.
- Verify the migration with `duckduckgo-search-cli commands -q -f json --fields agent_ops`.
- Treat this as the only observable envelope rename in v1.0.5.
- Note that the `discriminator` slot survives inside the `schema` catalog rows, where it still names the `type` VALUE a schema describes.

## v1.0.6 Highlights for Integrations
- **The agent-facing surface did NOT change** — no verb, flag, wire key, exit code or envelope shape moved in this release, so an integration written against v1.0.5 needs no migration.
- **`verify_published` is MAINTAINER tooling, outside the product contract** — it lives behind `required-features = ["release-gate"]`, is never built into the shipped binary, and must not be treated as a command an agent can invoke.
- **The release now ends at the registry, not at the package** — the gate runs after `cargo publish` and after every `cargo yank`, because `max_stable_version` is derived from yank state.
- **Security: `h2` moved from `0.4.15` to `0.4.18`** (RUSTSEC-2026-0258), including the copy `chromiumoxide` brings into the default profile.
- Design and evidence: `CHANGELOG.md` entry `[1.0.6]` and `docs/decisions/0032-post-publish-verification-gate-v1-0-6.md`.

## v1.0.5 Highlights for Integrations
- **`--probe` and `--probe-deep` honour the agent-native operators** — both surfaces now run through the projector instead of the bypass helper `emit_probe_payload`.
- **Measured on v1.0.4** — `--count-only`, `--limit 1`, `--fields status` and `--truncate-content 5` each returned **633 bytes against a 633-byte baseline, at exit 0**, on the health-check surface an agent reaches for first.
- **Probe refusals reach the caller** — `emit_probe` returns the exit code instead of being discarded by `let _ =`, so a refusal is no longer swallowed like the old no-op.
- **Wire keys still apply to the probe** — `KeyPolicy::ProcessWire` runs the reduction on the English document first and maps keys last, so `--fields` paths mean the same thing in both languages.
- **Identity strings are exempt from `--truncate-content`** — a string the agent hands back to a program is IDENTITY, not content.
- **The exemption covers** config keys, locale tags, schema ids, filesystem paths and probe error codes, declared per surface in `EnvelopeShape::identity` and published through `commands`.
- **`schema --truncate-content 12` no longer mutilates contract identifiers** — it used to turn `invoke` into `duckduckgo-s` and `id` into `searc`.
- **All-identity surfaces REFUSE instead of returning a no-op** — `config list`, `config path`, `config get/set/unset`, `config effective` and `locale` exit `2` under `--truncate-content`.
- **Those surfaces used to return 1138 bytes against a 1138-byte baseline** — byte-for-byte the signature of the original defect and indistinguishable to the caller.
- **Refusal has ONE contract across every surface** — stdout carries the published `error-response` shape `{"error","message"}`, stderr carries the localized sentence, and the exit code stays `2`.
- **The stdout `message` stays English on purpose** — it is the machine half of the contract, while `--ui-lang` governs stderr only.
- **`commands` renamed `discriminator` to `discriminator_key`** — see the breaking-change section above.
- **Four new XDG keys** — `probe_launch_timeout_seconds`, `probe_extract_timeout_seconds`, `probe_deep_launch_timeout_seconds` and `probe_deep_extract_timeout_seconds`.
- **Probe ceilings resolve CLI, then XDG, then the compiled default** — a shorter `--timeout` still wins.
- **`--fields` now applies to a timed-out search** — network speed no longer decides whether the flag is honoured.
- **The multi-query stream path and the non-stream path say the same thing** about the same `--fields` / `--filter` input.
- Design and evidence: `CHANGELOG.md` entry `[1.0.5]`.

## v1.0.4 Highlights for Integrations
- **The seven agent-native operators act or refuse by name** — `--fields`, `--filter`, `--limit`, `--sort`, `--dedupe-by`, `--count-only` and `--truncate-content` have no third outcome.
- **Before v1.0.4 they were accepted and silently ignored off the search surface** — `doctor --fields type` produced 2523 bytes against a 2524-byte baseline, the one byte being the trailing newline.
- **`--fields` and `--truncate-content` have meaning on any JSON object** and apply everywhere.
- **The five row operations need an array of rows** — a surface without one refuses them with exit `2`, naming the flag, the surface and what IS supported there.
- **The row array is DECLARED per surface, never inferred** — `doctor` carries both `checks` and `failed_checks`, and `config effective` carries both `allowed_keys` and `precedence`.
- **Measured after** — `commands` 6421 → 47 bytes with `--fields version`; `doctor` 2524 → 38 with `--fields type,status`; `schema` 4726 → 1107 with `--fields schemas.id`.
- **The capability matrix is published** in `commands` under `agent_ops`, so a caller learns the contract instead of discovering it by collecting exit codes.
- **A `--fields` path that matches nothing is an error** naming the level where the path broke and the keys available THERE.
- **English wire keys are the default** — keep Portuguese with `--wire-keys pt` or `config set wire_keys pt`.
- **Three fields stopped emitting Portuguese under the English wire** — `AggregatedItem.display_url`, `AggregatedNewsItem.source` and `AggregatedNewsItem.relative_date` no longer emit `url_exibicao`, `fonte` and `data_relativa`.
- **`deep-research-output.schema.json` declared the English names**, so a real news row with a publisher or a date used to FAIL the contract the product publishes for it.
- **Seven published envelopes gained a discriminator** — the five `config` shapes, `locale` and `init-config` now emit `type` from a compiler-checked enum.
- **All 24 published schemas partition into routable and deliberately-unroutable**, and the reason is published in the catalog as `routing`.
- **The multi-query stream path no longer discards a parse error** — `--fields` and `--filter` errors refuse with exit `2` on both paths.
- Design and evidence: `CHANGELOG.md` entry `[1.0.4]`.

## v1.0.2 Highlights for Integrations
- **ADR-0027 wire EN default** — stdout serializes English keys (`.results`, `.title`, `.metadata`, `.result_count`, `.metadata.chrome_channel`, `.metadata.chrome_path_resolved`, `.metadata.used_chrome`, …). Deserialize still accepts PT aliases.
- **Legacy agents** — keep PT keys with `--wire-keys pt` or `config set wire_keys pt`. Full renames: [docs/MIGRATION.md](docs/MIGRATION.md).
- **Agent ops (no jq)** — `--fields`/`--select`, `--filter`, `--limit`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`.
- **RuntimeConfig SSOT** — CLI > XDG > FACTORY; **no product env**, **no remote telemetry**.
- **Discovery** — `commands`, `schema`, `doctor`, `locale`, `man` for agent self-discovery.
- Design: [`docs/decisions/0027-wire-en-default-v1-0-2.md`](docs/decisions/0027-wire-en-default-v1-0-2.md).

## v1.0.1 Highlights for Integrations
- **Config dual API** — `config get/set/unset` accepts positional `KEY`/`VALUE` **and** `--key`/`--value`.
- **`-f ndjson`** aliases to `--stream` mode; stream `BrokenPipe` → exit **141** (pipe|head e2e).
- **Oneshot + SIGPIPE** — `ensure_oneshot_cleanup` + residual Chrome kill + force profile remove; **SIG_IGN** for SIGPIPE so Drop/reap runs (pipe orphans=0).
- **ADR-0023** — wire PT serialize + EN deserialize aliases (backward compatible; **superseded for serialize by ADR-0027 / v1.0.2**).
- **`config effective`**, doctor `channel=`, XDG `default_lang`/`default_country`, depth quality filter.
- **No product env**, **no remote telemetry**. Local gates only.
- Inventory: `gaps.md` Pass 52 / GAP-E2E-51.

## v1.0.0 Highlights for Integrations
- **GAP-WS-TMP-PROFILE-ORPHAN-001 (ADR-0020)** — Chrome profiles use auditable prefix **`ddg-chrome-*`** (not generic `.tmp`); cooperative exit removes the profile directory; next-run `sweep_orphan_profiles` cleans **only** stale owned `ddg-chrome-*`.
- **Hard disk hygiene** — never bulk-delete foreign `.tmp*` or `org.chromium.Chromium.*`; SIGKILL/OOM residual is next-run sweep of `ddg-chrome-*` only.
- **deep-research** inherits the main `CancellationToken` (SIGTERM cancels fan-out so disk reap can run).
- **Stable 1.0.0 contract** — process+disk one-shot, agent-ready defaults, Chrome-only CDP, atomwrite, **no remote telemetry**. No JSON schema break vs 0.9.10/0.9.9.
- Design: [`docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md`](docs/decisions/0020-chrome-profile-disk-oneshot-v1-0-0.md); inventory: `gaps.md`.

## v0.9.8 Highlights for Integrations
- **GAP-WS-AGENT-READY-001 (ADR-0018)** — agent-ready defaults for real Linux hosts.
- **Default `--vertical all`** — plain search returns web + news; opt out with `--vertical web` (deep: `--no-news`).
- **Content fetch ON by default** — cleaned text for top web + news URLs (cap 4 in v1.0.2; was 10 at v0.9.8); opt out with `--no-fetch-content`.
- **News may include `content`** (EN wire since v1.0.2; legacy PT `conteudo` with `--wire-keys pt`) — same readability pipeline as web (supersedes the v0.8.9 “fetch only web results” rule).
- **Multi-canal Chrome** — Flatpak export/wrapper shells resolve to deploy ELF; order: `--chrome-path` → `CHROME_PATH` → host Chrome → host Chromium → Flatpak → Snap.
- **Transport flags `global = true`** — `--chrome-path`, `--proxy`, `--vertical`, fetch flags, identity, etc. accepted **before or after** `deep-research`.
- **Honest agent metadata (not telemetry)** — v1.0.2 EN: `chrome_path_resolved`, `chrome_channel`, `used_chrome` (legacy PT wire: `chrome_path_resolvido`, `chrome_canal`, `usou_chrome` via `--wire-keys pt`).
- **Canonical formula** — prefer longer timeout when fetch is on:

  ```bash
  timeout 180 duckduckgo-search-cli -q -f json --num 15 "query"
  timeout 180 duckduckgo-search-cli -q -f json deep-research "query" --chrome-path /path/to/chrome
  # Preserve pre-0.9.8 thin envelope:
  timeout 60 duckduckgo-search-cli -q -f json --vertical web --no-fetch-content "query"
  # v1.0.2 EN wire parse:
  timeout 180 duckduckgo-search-cli -q -f json --num 15 "query" | jaq '.results[] | {title, url}'
  ```

- Design: [`docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md`](docs/decisions/0018-agent-ready-multi-canal-dual-clean-v0-9-8.md); inventory: `gaps.md`.

## v0.9.6 Highlights for Integrations
- **One-shot process contract (GAP-WS-LIFECYCLE-001, ADR-0017)** — each CLI invocation fully reaps its Chromium/Xvfb process tree on exit. Agents may invoke the binary N times without leaking Chromium/Xvfb RAM across runs.
- **Cooperative cancel on SIGTERM/SIGINT** — supervisors that send SIGTERM first (e.g. `timeout`, Docker stop) cancel cooperatively so the lifecycle reap path runs.
- **Prefer timeouts that send SIGTERM first** — use GNU `timeout` (SIGTERM, then SIGKILL after grace) rather than hard-kill-only wrappers so process cleanup can complete.
- **Upgrade note from <0.9.6** — historical orphans from pre-0.9.6 runs are **not** auto-cleaned; operators may need a one-time manual kill. New runs after upgrade do not leak.
- **Residual limits** — SIGKILL is not interceptable; if a supervisor kills with SIGKILL immediately, reap may not run.
- **No telemetry** — lifecycle hardening does not emit telemetry.
- **No JSON schema break** — output envelope, exit codes, and flags are unchanged; drop-in for existing integrations.
- Design details: [`docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md`](docs/decisions/0017-browser-lifecycle-one-shot-v0-9-6.md) (ADR-0017 / GAP-WS-LIFECYCLE-001).

## v0.9.4 Highlights for Integrations
- **GAP-WS-113 (Chrome-only universal transport, ADR-0016)** — production is **Chrome-only** via chromiumoxide/CDP (feature `chrome` is default). All network ops — search, news, deep-research, `--probe`, `--probe-deep`, `--pre-flight`, `--fetch-content` — require a usable Chrome.
- **Fail-closed without Chrome** — missing Chrome or `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` → **exit 2** (`INVALID_CONFIG`). No auto `--no-news`, no Web downgrade, no silent HTTP success path.
- **`--allow-lite-fallback` is a legacy no-op** — never forces Lite; SERP stays HTML canonical under Chrome. Do not use it as remediation; install Chrome / `--chrome-path` / `--proxy` instead.
- **HTTP residual** only behind feature `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1` (wiremock tests). Production success path is chromiumoxide-only.
- **Canonical formula** — hosts need Chrome/Chromium (and Xvfb on headless Linux when required):

  ```bash
  timeout 60 duckduckgo-search-cli -q -f json --num 15 "query"
  timeout 180 duckduckgo-search-cli -q -f json deep-research "query"
  ```

## v0.9.0 Highlights for Integrations (historical)
- **GAP-WS-106 (global flags)** — nine flags are now `global = true` and accepted BEFORE OR AFTER the `deep-research` subcommand: `-q`, `-o`, `-n`, `-f`, `-t`, `-l`, `-c`, `-p`, `-v` (plus their long forms). Pipeline authors no longer need to remember flag ordering relative to the subcommand. Extends the GAP-WS-59 hoisting precedent.
- **GAP-WS-106 (auto-degradation without Chrome) — HISTORICAL, superseded by GAP-WS-113 / v0.9.4** — in v0.9.0–v0.9.3, `deep-research` without Chrome auto-applied `--no-news` with a stderr warning and proceeded web-only; `--vertical news|all` downgraded to `Web` instead of aborting. **Since v0.9.4 those paths fail closed with exit 2** (no auto-degradation).
- **GAP-WS-106 (actionable errors)** — when the parser rejects a known flag positioned after the subcommand, a hint is appended pointing to the correct position (rare now that the 9 most-used flags are global).
- **No JSON schema changes in v0.9.0** — the envelope was byte-identical to v0.8.9; only parser/exit-code behavior changed.

## v0.8.9 Highlights for Integrations
- **GAP-WS-104 (news vertical, `--vertical` flag)** — new flag `--vertical <web|news|all>` (historical default was `web`; **v0.9.8 default is `all`**). `news` and `all` are Chrome-only (no HTTP fallback), accept any number of queries (multi-query via `--queries-file` or multiple positionals is accepted since GAP-WS-105). Since v0.9.8 `--vertical` is a **global** root flag (also accepted after `deep-research`).
- **News envelope (v1.0.2 EN wire default)** — `.news[].{position,title,url}` are guaranteed non-null; `.news[].{source,relative_date,thumbnail}` are optional (`Option<String>` — always apply `// ""` fallback in `jaq`). `.news_count` and `.metadata.vertical_used` appear when vertical != web. **v0.9.8:** default vertical is already `all`. Legacy PT keys (`.noticias[]`, `.quantidade_noticias`, `.metadados.vertical_usada`, …) only with `--wire-keys pt`.
- **New ZeroCause variant `vertical-no-results`** (EN serialize; legacy PT string `vertical-sem-resultados` with `--wire-keys pt`) — a news/all search with zero hits is classified as legitimate and emits exit 5 (not exit 6).
- **Exit-code accounting** — the total result count used for exit code decisions is `result_count + news_count` (legacy PT: `quantidade_resultados + quantidade_noticias`).
- **`--fetch-content` scope (UPDATED v0.9.8 / current v1.0.5)** — content extraction applies to **web + news** top URLs (cap 4 in v1.0.2; was 10 at v0.9.8). Historical v0.8.9 rule “only web `results[]`” is **superseded**. Opt out with `--no-fetch-content`.
- **Canonical formula** — `timeout 90 duckduckgo-search-cli --vertical news "query" -q -f json | jaq '.news'`
- **News RAG pipeline** — extract guaranteed fields with optional fallbacks:

  ```bash
  timeout 90 duckduckgo-search-cli --vertical news "rust 1.88 release" -q -f json \
    | jaq -r '.news[] | [.position, .title, .url, (.source // ""), (.relative_date // "")] | @tsv'
  ```

- **Combined web + news (`--vertical all`)** — one Chrome pass returns both roots:

  ```bash
  # v1.0.2 EN wire (default):
  timeout 90 duckduckgo-search-cli --vertical all "query" -q -f json \
    | jaq '{web: [.results[].url], news: [.news[].url]}'
  # Legacy PT wire:
  timeout 90 duckduckgo-search-cli --vertical all "query" -q -f json --wire-keys pt \
    | jaq '{web: [.resultados[].url], news: [.noticias[].url]}'
  ```

- **No breaking changes to JSON output schema**. All v0.8.8 fields remain present. News fields are additive and only emitted when the news vertical is active.

## v0.8.8 Highlights for Integrations
- **GAP-WS-089 fix (Xvfb stale lock cleanup)** — `spawn_virtual_display()` now checks whether the PID inside `/tmp/.X{N}-lock` is alive before skipping the slot. Stale locks from crashed or cancelled runs are removed automatically, preventing Xvfb pool exhaustion after ~100 failed runs.
- **GAP-WS-090 fix (`--num` honored in Chrome headed path)** — Chrome primary search now truncates results to `min(num, len)` before computing `quantidade_resultados`. Previously `--num 1` returned 10 results (a full DDG page).
- **GAP-WS-091 fix (`--region` alias added)** — `--country`/`-c` now accepts `alias = "region"`, aligning the CLI with the SKILL documentation that references `--region`.
- **GAP-WS-092/093/097 fix (`fill_compat_fields()` populates metadados)** — `metadados.quantidade_resultados`, `metadados.endpoint_usado`, and `metadados.nivel_cascata` are now populated via `fill_compat_fields()` before JSON emission. Previously these fields existed only at the root level or were always `null`.
- **GAP-WS-094 fix (`--num` honored in batch/parallel path)** — `execute_query_with_cancellation()` now truncates results by `--num` in the batch path, matching the single-query fix from GAP-WS-090.
- **GAP-WS-095 fix (`identidade_usada` populated in Chrome headed)** — when Chrome headed succeeds with `identity_profile = Auto`, the CLI now looks up the matching identity from the pool by UA and populates `identidade_usada` instead of returning `null`.
- **GAP-WS-099 fix (`ZeroResultsSuspeito` emits exit 6)** — the `ZeroResultsSuspeito` variant was missing from the exit code 6 match arm. It now correctly emits exit 6 (`SUSPECTED_BLOCK`) instead of falling through to exit 5. BC opt-out: `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`.
- **GAP-WS-100 fix (`tamanho_conteudo` reflects truncated size)** — `content_size` now uses `text.len()` (post-truncation) instead of `size_original` (raw HTML body). `--max-content-length 500` now reports `tamanho_conteudo: 500` instead of the original HTML size.
- **GAP-WS-102 fix (deep-research `nivel_cascata` no longer null)** — deep-research metadata now reads from `cascade_level_observed` (the real field) instead of `cascade_level` (the compat field populated after pipeline return).
- **GAP-WS-103 fix (exit 6 documented in `--help`)** — the EXIT CODES section of `--help` now lists exit code 6 (`Suspected block`). Previously only codes 0–5 were documented.
- **No breaking changes to JSON output schema**. All v0.8.7 fields remain present. New compat fields are additive.

## v0.8.6 Highlights for Integrations
- **The BoringSSL TLS stack was REMOVED here** — `wreq` and BoringSSL were replaced by `reqwest` + `rustls`, which is why the v0.7.5 bullets about a BoringSSL build are historical and not current instructions.
- **The four Windows build prerequisites died with it** — NASM, CMake, MSVC and Perl stopped being required by `build.rs`, and the `DDG_SKIP_*_CHECK=1` escape hatches stopped existing as a live knob.
- **No JSON output change** — the swap is a dependency and build-experience change, invisible on the wire.
- Rationale: `docs/decisions/0008-reqwest-rustls-v0-8-6.md`, later narrowed by `docs/decisions/0021-rustls-aws-lc-sole-provider.md`.

## v0.7.10 Highlights for Integrations
- **GAP-WS-60 fix (CRITICAL, identity pin propagation)** — `--identity-profile` now propagates the selected identity to `failure_output` (pipeline.rs) and `error_output` (parallel.rs) through the new helper `identity_tag_for_cli_identity` in `src/identity.rs`. Before the fix, the identity pin (`identidade_usada`) appeared only on the SUCCESS path and was always `null` on failure. Consumers can now correlate a failure with a specific identity from the pool of 12.
- **GAP-AUD-002 fix (CRITICAL, bench wiring)** — `cargo bench --bench pre_flight_latency` now runs Criterion correctly after adding `[[bench]] harness = false` to `Cargo.toml`. Before the fix, the bench binary was compiled but invoked by the test harness, which reported `running 0 tests` instead of running the 5 scenarios. The bench writes results to `target/criterion/`.
- **`--require-results` (NEW flag, `deep-research`)** — when set and the fan-out aggregates zero results, the subcommand returns exit 4 (`GLOBAL_TIMEOUT`) with the message `deep-research produced zero results for query ...; --require-results set → exiting non-zero` on stderr. Closes GAP-WS-1114 (silent-discard pattern).
- **`--pre-flight` (NEW flag, global)** — enables the automatic probe-deep scheduler inside `execute_single_search`. When the environment is blocked, it detects captcha/ghost-block in ~140 ms before spending the real query, and aborts with `pre_flight_blocked` (exit 3). Default `false` to preserve v0.7.8 behaviour.
- **`--probe-deep` now returns exit 3 when it detects captcha** (B4 fix, v0.7.10) — it previously returned exit 0 even with `status: "captcha"`. Consumers can branch on the exit code instead of parsing the JSON.
- **Canonical identity pin** — format `<family>-<platform>-<16hex>`, e.g. `chrome-linux-33333333cccc0003`, `firefox-linux-99999999cccc0009`, `safari-macos-bbbbbbbbeeee000b`. Deterministic seed per identity.
- **Zero breaking changes**. All v0.7.9 JSON fields remain. The schema `SearchMetadata.identity_used: Option<String>` stays optional (`None` under the `auto` cascade).
- **local pre-publish checklist (NEW)** — 7 sequential gates before `cargo publish`: fmt, clippy, test, coverage ≥80%, no stale v0.7.9 refs under `skills/`, valid publish dry-run, local gates green. Rule 1264 (cargo publish dry-run required before real).

## v0.7.9 Highlights for Integrations
- **GAP-WS-58 fix (CRITICAL, ghost-block)** — `detectar_interstitial` now classifies a sub-4KB body without `result-page-signal` as `InterstitialKind::Cloudflare`. The helper `has_result_page_signal` checks DDG classes (`nrn-react-div`, `react-article`, `module--results`, `js-react-aria-results`). The conservative 4KB threshold avoids false positives.
- **GAP-WS-59 fix (HIGH, markers 2026)** — 5 new Cloudflare markers (`anomaly.js`, `botnet`, `cf-error-code`, `cf-ray`, `Performance & Security by Cloudflare`) plus 1 new DDG marker (partial `Unfortunately, bots`). `CLOUDFLARE_MARKERS` and `DDG_MARKERS` updated in `src/probe_deep.rs`.
- **GAP-WS-59 fix (HIGH, global flag)** — `--allow-lite-fallback` and `--pre-flight` hoisted to `RootArgs` with `global = true`. Closed the `unexpected argument` path on subcommands such as `deep-research`.
- **GAP-WS-54 (supply chain)** — `scraper` upgraded from 0.20 to 0.27, transitively removing the unmaintained `fxhash 0.2.1` (RUSTSEC-2025-0057). `cargo audit --deny warnings` is now a hard local gate. `async-std` (RUSTSEC-2025-0052) remains only in the optional `chrome` feature.
- **GAP-WS-55 (documentation drift)** — the `wreq` comment in `Cargo.toml` was rewritten to reflect the real decision (pin on `wreq 6.0.0-rc.29` plus the three direct pins for `wreq-util`, `brotli-decompressor` and `alloc-no-stdlib`), not the regression that never happened and was described in the stale comment.
- **`Config.pre_flight` added** with default `false` for opt-in.
- **Test count: 305 (292 lib + 13 integration)**, 0 clippy warnings, 0 fmt diff, 0 cargo-deny warnings, clean `cargo doc --offline --no-deps`.
- **Zero breaking changes**. Existing JSON fields preserved.

## v0.7.8 Highlights for Integrations
- **Honest interstitial detection** — the `probe_deep` 9-word calibration query (replacing the fixed 1-word probe) triggers the real upstream tightening of bot scoring. `cascata_motivo` is now populated on `exit 3` (anti-bot) with `cloudflare_anomaly_modal` when the Cloudflare interstitial is detected.
- **`--allow-lite-fallback` honored (historical through v0.9.3)** — exit 3 (anti-bot) with `cascata_motivo` filled replaced the silent exit 5 when an interstitial was detected and lite fallback was enabled. **Since v0.9.4 / GAP-WS-113 the flag is a legacy no-op** (Chrome-only; Lite is never a production success path).
- **`--retries` honored** — values in `[1, 10]` are clamped to prevent abuse. `--retries 5` produces `metadata.retries == 5` on the v1.0.2 EN wire (legacy PT: `metadados.retentativas` with `--wire-keys pt`; verified by regression test).
- **Multi-occurrence verbose levels** — `-vv` for debug, `-vvv` for trace (additive, `ArgAction::Count`).

## v0.7.5 Highlights for Integrations
- **`--query` (NEW alias)** — equivalent to passing the query as a positional argument. Enables the syntax `duckduckgo-search-cli --query "rust async" --num 10` for integrations that prefer named flags over positionals.
- **`--max-content-length` (NEW cap)** — limits the memory consumed by `--fetch-content` on a large corpus. Default 5000 bytes.
- **Consistent `Sec-Fetch-*` headers** across every browser family, eliminating the fingerprint inconsistency that triggered anti-bot detection.
- **GAP-WS-29 fixed (CRITICAL, build experience, Windows)** — `cargo install` on native Windows MSVC without the **C++ CMake tools for Windows** sub-component of the Visual Studio Installer previously failed minutes into the BoringSSL build with the cryptic `program not found / is 'cmake' not installed?`. The `build.rs` preflight now detects this and aborts in SECONDS with the exact fix (`winget install -e --id Kitware.Cmake` OR Visual Studio Installer → Modify → Workloads → Desktop development with C++ → expand → check C++ CMake tools for Windows). New escape hatch: `DDG_SKIP_CMAKE_CHECK=1`.
- **GAP-WS-30 fixed (CRITICAL, build experience, Windows)** — BoringSSL CMake uses the Visual Studio 17 2022 generator which requires `cl.exe` (compiler) and `link.exe` (linker). The `build.rs` preflight now detects both and aborts with the fix (open a Developer PowerShell for VS 2022, or run `Launch-VsDevShell.ps1`). MSVC is NOT auto-installed (5+ GB download, too intrusive). New escape hatch: `DDG_SKIP_MSVC_CHECK=1`.
- **SUPERSEDED FROM HERE TO THE END OF THE BoringSSL BLOCK (v0.8.6, ADR-0008)** — the bullets that follow describe the BoringSSL era. Perl, NASM, CMake and MSVC are **not** current build prerequisites, and `DDG_SKIP_*_CHECK=1` is **not** a live escape hatch: those variables belonged to a `build.rs` preflight that no longer exists, and this project does not accept environment variables as product knobs. Kept as a record of what was true then, never as instructions for today.
- **GAP-WS-31 fixed (CRITICAL, build experience, Windows)** — BoringSSL perlasm generator emits crypto assembly in NASM format and requires `perl.exe`. The `build.rs` preflight now detects perl and reports the fix (`winget install -e --id StrawberryPerl.StrawberryPerl`). New escape hatch: `DDG_SKIP_PERL_CHECK=1`.
- **GAP-WS-32/35/36 fixed (MEDIUM, documentation)** — All remaining claims that "pre-built binaries from `cargo install` are unaffected" (or its PT/EN variants) are now qualified across `skills/duckduckgo-search-cli-en/SKILL.md`, `skills/duckduckgo-search-cli-pt/SKILL.md`, `llms-full.txt`, `docs/CROSS_PLATFORM.md`, `README.md`, and `README.pt-BR.md`. **`crates.io` NEVER distributes binaries**; `cargo install` always compiles from source. Users on Windows must satisfy the four BoringSSL build prerequisites (NASM, CMake, MSVC, Perl) themselves before `cargo install` can succeed.
- **`build.rs` preflight coverage expanded** — v0.7.4 only checked for NASM. v0.7.5 checks for all four BoringSSL build prerequisites (nasm, cmake, cl.exe, link.exe, perl) and supports four independent `DDG_SKIP_*_CHECK=1` escape hatches.
- **New `scripts/check-windows-toolchain.ps1`** — standalone diagnostic (no installs) that checks all 7 tools (cargo, rustc, cmake, nasm, cl.exe, link.exe, perl) and emits text or JSON output. Exit code 0 if all present, 1 otherwise. Useful for support tickets and LOCAL gates — this repository forbids CI, see `NO_CI.md`.
- **New `docs/INSTALL-WINDOWS.md` (EN) + `docs/INSTALL-WINDOWS.pt-BR.md` (PT)** — step-by-step guide covering 5 installation methods (VS Installer + standalone; all-winget standalone; Chocolatey; helper script; standalone diagnostic). Includes troubleshooting for each of the 4 GAPs and the `DDG_SKIP_*_CHECK` escape hatches.
- **HISTORICAL NOTE — CI Windows jobs updated (those GitHub Actions were later REMOVED)** — `local gates` and `local release process` then verified CMake, installed Perl, and verified MSVC Build Tools (in addition to the existing NASM step) in every Windows job. That eliminated the implicit dependency on the `Windows host` image's pre-installed tooling. No such job exists today: CI is forbidden and every gate is local.
- **Zero breaking changes to JSON output schema**. All v0.7.4 fields remain present. All v0.7.3 fields remain present.
- **GAP-WS-27 fixed (CRITICAL, inherited from v0.7.3)**: The macOS CAPTCHA interstitial that returned HTTP 200 with `quantidade_resultados: 0` while Windows returned full results is closed. TLS stack changed from `rustls` to BoringSSL via `wreq 6.0.0-rc.29` — **HISTORICAL: that stack was removed in v0.8.6 (ADR-0008) and the current stack is rustls, narrowed by ADR-0021**. `cargo install` always compiles from source — crates.io does not distribute pre-built binaries for any platform. The build toolchain change is the trade-off for the BoringSSL TLS fix (GAP-WS-27 closed). Source builds on Linux require `cmake`, `perl`, `pkg-config`, and `libclang-dev`; source builds on Windows require NASM, CMake, MSVC, and Perl (see `gaps.md` GAP-WS-28/29/30/31 and `docs/INSTALL-WINDOWS.md`).
- **`session` feature (cookie persistence + warm-up)**:
  - New flags: `--no-warmup`, `--no-cookie-persistence`, `--cookies-path <PATH>`.
  - Cookie jar persisted to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) with Unix permissions `0o600`.
  - Warm-up adds one `GET https://duckduckgo.com/` before the first real query to populate session cookies.
- **`probe-deep` feature (CAPTCHA interstitial detection)**:
  - New flags: `--probe-deep` (run a real search query and classify the body as `ok` or `captcha`), `--allow-lite-fallback` (historical opt-in for html→lite fallback when CAPTCHA was detected via GAP-WS-52; **since v0.9.4 / GAP-WS-113 this flag is a legacy no-op** — SERP stays HTML Chrome; do not treat it as active remediation).
  - New JSON report fields on the probe response (v1.0.2 EN): `status`, `cascade_reason`, `mitigation_suggestion`, `http_status`, `latency_ms` (legacy PT `cascata_motivo` / `sugestao_mitigacao` with `--wire-keys pt`).
- **Zero breaking changes to JSON output schema**. All v0.7.2 fields remain present.

## v0.7.0 Highlights for Integrations
- **New subcommand `deep-research`**: agents that need multi-hop answers can
  drop in `duckduckgo-search-cli deep-research "question" --synthesize`
  and get a Markdown report back, with no extra orchestration. Inherits
  every global flag (`-q -f json`, `--num`, `--parallel`, `--proxy`,
  `--fetch-content`) plus deep-research-specific knobs
  (`--max-sub-queries`, `--sub-queries-file`, `--aggregate`,
  `--budget-tokens`, `--synth-format`).
- **Backward-compatible**: zero changes to `buscar`, `init-config`,
  default-config JSON schema, or any exit code. Existing pipelines keep
  working unchanged.
- **Pool of 12 anti-bot identities** — 4 browser families × 3 platforms with a 5-level cascade rotation. Use `--identity-profile chrome-linux` to pin a specific identity and `--seed 42` for reproducibility.
- **`deep-research` fan-out detail** — up to 12 sub-queries, RRF aggregation, optional Markdown synthesis with a token budget, available through `duckduckgo-search-cli deep-research "query" --synthesize --synth-format markdown`.
- **Cookies persisted to `~/.config/duckduckgo-search-cli/cookies.json`** (XDG, mode `0o600`). Use `--cookies-path` to redirect or `--no-cookie-persistence` to disable.

## v0.6.5 Highlights for Integrations
- **MP-26 FIX**: Windows build now compiles. Use `cargo install duckduckgo-search-cli`
  on any platform without manual patches.
- **CI-01 FIX**: local multi-platform checks now green on all 3 SOs (Linux/macOS/Windows).
  Agents running on Windows runners can rely on the binary.
- **WS-12 Circuit breaker**: `--fetch-content --parallel` no longer cascades
  failures across hosts — one slow domain won't block the rest of the crawl.
- **WS-25 ProgressBar**: `indicatif` output to stderr auto-hides in pipes,
  so JSON pipelines on stdout stay clean.
- **Stable CLI on common entrypoints** — the canonical invocation `timeout 60 duckduckgo-search-cli -q -f json --num 15 "query"` produces deterministic JSON on `stdout` (separated from logs on `stderr`).
- **Documented exit codes** — 0 success, 1 runtime, 2 config, 3 anti-bot, 4 timeout, 5 zero results. Consistent mapping across every version.
- **Anti-bot via UA rotation** — `BrowserProfile` injects per-family `Sec-Fetch-*` headers and Client Hints. Duplicate headers are rejected (never add them by hand).

See `CHANGELOG.md` for the complete v0.6.5 changelog and migration notes
from earlier versions.

## v0.7.6 Highlights for Integrations
- **GAP-WS-48 closed (HIGH, build experience)**: `cargo install` was breaking
  on certain platforms due to a `cargo` resolver conflict between
  `alloc-no-stdlib 2.0.4` and `alloc-no-stdlib 3.0.0` brought in transitively
  by the `wreq` stack. v0.7.6 pins `alloc-no-stdlib =2.0.4` directly in
  `Cargo.toml`. Reinstalling from crates.io now works without manual
  dependency cleanup.
- **No CLI contract changes**: All v0.7.5 flags, JSON fields, and exit codes
  remain present. Drop-in replacement.
- **CI change**: The `pre-publish` job now resolves the dependency graph
  during release — if the pin ever drifts again, the release will fail
  before reaching crates.io.
- **BoringSSL TLS via `wreq`** (replacing `reqwest+rustls` since v0.7.3). The JA4_o fingerprint identical to Chrome/Safari eliminates the Cloudflare CAPTCHA on macOS. See `docs/decisions/0001-tls-boring-via-wreq.md`.
- **Interstitial detection through `probe_deep`** — the calibration query replaces the fixed probe. `CLOUDFLARE_MARKERS` and `DDG_MARKERS` updated in `src/probe_deep.rs`.

## v0.7.7 Highlights for Integrations
- **GAP-WS-49 closed (CRITICAL, runtime regression)**: A `wreq-util`
  resolution failure in v0.7.6 broke BoringSSL TLS fingerprint emulation
  on certain Linux distributions. The result was silent CAPTCHA
  interception on hosts that previously worked. v0.7.7 pins `wreq-util`
  directly in `Cargo.toml` — no more resolution drift.
- **No CLI contract changes**: All v0.7.6 flags, JSON fields, and exit
  codes remain present. Drop-in replacement.
- **Recommended upgrade path**: v0.7.5 → v0.7.7 is the cleanest jump
  for users who skipped v0.7.6. The v0.7.6 → v0.7.7 delta is
  dependency-only.
- **TLS fingerprint emulation restored** through the direct pin on `wreq-util`. `alloc-no-stdlib` resolved between 2.0.4 and 3.0.0.
- **Reliable `cargo install`** — pins on `wreq 6.0.0-rc.29` + `brotli-decompressor = "=5.0.1"` + `alloc-no-stdlib = "=2.0.4"`. Reproducible cross-platform dependency resolution.

## v0.7.8 Highlights for Integrations (replicated)
- **GAP-WS-50**: `probe-deep` interstitial list expanded — 8 Cloudflare
  markers plus 1 DDG anomaly marker. False-negative rate on CAPTCHA
  detection dropped measurably in benchmark runs.
- **9-word calibration probe** — `the quick brown fox jumps over the lazy dog` triggers the real upstream tightening of bot scoring. Cloudflare and DDG markers updated in `src/probe_deep.rs`.
- **GAP-WS-52 (historical through v0.9.3)**: `--allow-lite-fallback` consulted
  the real detector result. When the detector flagged a CAPTCHA but the flag
  was off, the CLI emitted a structured `tracing::warn!` and exited with the
  appropriate code instead of silently degrading. **Since v0.9.4 / GAP-WS-113
  the flag is a legacy no-op** (Chrome-only; Lite is never a production success path).
- **GAP-WS-53**: `-vv` (debug) and `-vvv` (trace) levels added. Operators
  investigating failed searches can escalate verbosity without
  recompiling. The flag `conflicts_with = "quiet"`.
- **GAP-WS-54**: `scraper` bumped to `0.27.0`. Removes the transitive
  `fxhash 0.2.1` (RUSTSEC-2025-0057, unmaintained). `cargo audit
  --deny warnings` is now a blocking gate in CI and release.
- **GAP-WS-55**: `wreq` block in `Cargo.toml` rewritten to match the
  actual pin in use (`6.0.0-rc.29` plus three direct pins). Eliminates
  documentation-vs-code drift.
- **GAP-WS-56**: Legacy `Buscar` subcommand hidden from `--help` via
  `#[command(hide = true)]`. Remains callable for backward compatibility.
- **GAP-WS-57**: `--retries` flag now honored end-to-end in
  `src/parallel.rs:644`. The previous behavior silently dropped the
  value in the `error_output` path. Integrators relying on retry
  behavior will see it actually take effect.
- **No breaking changes to JSON output schema**. All v0.7.7 fields
  remain present. Drop-in replacement once v0.7.8 is published.

See `CHANGELOG.md` for the complete v0.7.6/v0.7.7/v0.7.8 changelog and
the ADR at `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`
for the full design rationale of the v0.7.8 overhaul.

## Compatibility Notice
This CLI follows SemVer. Breaking changes only happen on minor bumps (0.x.0).
For consumers that do not upgrade regularly, v0.6.5 is the last release with a
fully stable contract before the anti-bot identity changes introduced in v0.7.0+
