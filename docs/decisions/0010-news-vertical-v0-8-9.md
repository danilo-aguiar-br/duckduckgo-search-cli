# ADR-0010: News Vertical Routed Exclusively Through Chrome (v0.8.9)


## Status
- Accepted (2026-07-06)
- Extends: ADR-0007 (Chrome Primary Transport, v0.8.0), ADR-0009 (Headed Xvfb Private, v0.8.7)
- Closes: GAP-WS-104 (search covered ONLY the web vertical; the news vertical was never visited)


## Context
- The CLI scraped ONLY the default web SERP; the news vertical (`ia=news&iar=news`) was never visited
- The domain model had no concept of a search vertical: `build_search_url` emitted only `q`, `kl`, `kp`, `df`
- The legacy `html.duckduckgo.com` and `lite.duckduckgo.com` endpoints structurally lack a news vertical
- The modern news SERP is a React module (`data-react-module-id="news"`) with obfuscated per-build classes that requires JavaScript hydration — a plain HTTP fetch returns an unhydrated shell
- Journalistic queries returned institutional pages instead of fresh articles; news-only metadata (source, relative date, thumbnail) was discarded; RAG pipelines fed by the CLI were blind to events from the last hours
- Rejected alternative: the internal `duckduckgo.com/news.js?vqd=` endpoint was discarded because the `vqd` token rotates per query and the endpoint is undocumented and unstable


## Decision
- New flag `--vertical <web|news|all>` (default `web`); `VerticalMode` enum added to the domain model
- News is routed EXCLUSIVELY through the Chrome-primary transport — NO HTTP fallback exists (the news SERP requires JavaScript rendering; html/lite endpoints have no news vertical)
- `--vertical all` runs web AND news in the SAME Chrome session (single warm-up, best-effort news)
- After navigation the CLI polls the rendered DOM for `[data-react-module-id="news"]` (`tokio::time::sleep` loop with timeout); on timeout it still extracts and lets the cascade decide
- Extraction cascade: Strategy A (semantic selectors anchored on `data-react-module-id`, hot-fixable via the `[news]` section of `config/selectors.toml` without recompiling) -> Strategy B (class-agnostic fallback keyed on external anchors + relative-date heuristic for PT "há N ..." and EN "N ... ago" patterns)
- Internal duckduckgo.com links filtered out; results deduped by URL preserving order; protocol-relative thumbnails resolved to `https://`; news HTML capture capped at 1 MiB (web SERP keeps 256 KiB); `--num` caps news results (GAP-WS-090 pattern)
- Backward-compatible contract: root `noticias[]` + `quantidade_noticias` and `metadados.vertical_usada` are emitted ONLY with `--vertical news|all` via `#[serde(skip_serializing_if = "Option::is_none")]` — the default web mode stays byte-identical to v0.8.8
- Per-item contract: `posicao`, `titulo`, `url` guaranteed; `fonte`, `data_relativa` (verbatim, no absolute-date conversion), `thumbnail` optional
- New `ZeroCause` variant `vertical-sem-resultados`: legitimate zero news (rendered SERP without articles) => exit 5, NOT 6; an anti-bot interstitial in the news body still classifies as `anti-bot`
- Exit-code total now sums `quantidade_resultados + quantidade_noticias`: news-only with articles found => exit 0; `--vertical all` with web>0 and news=0 => success with `noticias: []`
- Config guards (exit 2, `INVALID_CONFIG`): `--vertical news|all` rejects multiple queries (`--queries-file` or multiple positional), the `deep-research` subcommand, builds without the `chrome` feature, and `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`
- `--pre-flight` probes the web HTML endpoint ONLY when the execution includes the web vertical (`web`/`all`); news-only logs an informational skip notice on stderr (F2 post-review fix — the unconditional probe caused false-positive exit 3 without ever attempting the news SERP)
- `--fetch-content` keeps acting ONLY on `resultados[]` — news results are never content-fetched
- Chrome runtime failure under news-only emits a structured JSON envelope (`resultados: []`, `noticias: []`, `erro`/`mensagem` filled, `causa_zero: resposta-invalida`) instead of empty stdout with exit 1 (F1 post-review fix)


## Consequences
- Journalistic queries now return fresh articles with source, relative date and thumbnail
- News availability depends on Chrome + (on Linux) Xvfb — there is no degraded HTTP path by design
- Selector breakage on DDG's side is hot-fixable via `config/selectors.toml` `[news]` (Strategy A) with an automatic class-agnostic safety net (Strategy B)
- Existing consumers of the web-mode contract are unaffected: no new fields appear unless `--vertical news|all` is passed
- Multi-query, deep-research and chrome-less builds fail fast with exit 2 instead of silently ignoring the vertical
- `data_relativa` stays verbatim (locale-dependent) — downstream absolute-date normalization is a future iteration


## Files Changed
- src/types.rs: `VerticalMode`, `NewsResult`, `SearchOutput.news`/`news_count`, `SearchMetadata.vertical_used`, `NewsSelectors`, `ZeroCause::VerticalSemResultados`
- src/cli.rs: `--vertical` flag and config guards
- src/search.rs: `build_news_search_url` (`ia=news&iar=news`)
- src/extraction.rs: `extract_news_results_with_cfg` (cascade A -> B, relative-date heuristic)
- src/pipeline.rs: news routing, exit-code sum, pre-flight scoped to web vertical, cancellation token across Chrome transport (F5)
- config/selectors.toml: `[news]` section
- docs/schemas/: `news-result.schema.json` (new), `search-output.schema.json`, `search-metadata.schema.json`


## References
- ADR-0007 (Chrome Primary Transport, v0.8.0)
- ADR-0009 (Headed Xvfb Private, v0.8.7)
- gaps.md (GAP-WS-104)
- CHANGELOG.md [0.8.9]
- searxng/searxng#6257 (html/lite endpoints lack a news vertical)
