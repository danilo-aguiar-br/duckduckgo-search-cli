# ADR-0011: Deep-Research Dual Web+News by Default (v0.8.9)


## Status
- Accepted (2026-07-06)
- Extends: ADR-0007 (Chrome Primary Transport, v0.8.0), ADR-0009 (Headed Xvfb Private, v0.8.7), ADR-0010 (News Vertical, v0.8.9)
- Closes: GAP-WS-105 (deep-research fanned out ONLY the web vertical; the news vertical introduced by GAP-WS-104 was invisible to multi-hop research)


## Context
- ADR-0010 added the news vertical (`--vertical <web|news|all>`) to single-query search, but `deep-research` still fanned out sub-queries against the web vertical only
- Research questions about recent events aggregated stale institutional pages while the fresh-article signal (source, relative date, thumbnail) existed one flag away and was never collected
- ADR-0010 config guards explicitly rejected `--vertical news|all` for the `deep-research` subcommand, so there was no way to opt in
- News is Chrome-only by design (ADR-0010): the news SERP requires JavaScript hydration and the html/lite endpoints have no news vertical, so any deep-research integration inherits the no-HTTP-fallback constraint
- RRF scores are rank-derived (`1/(k+rank)`) and only meaningful within the list they were computed over — web and news ranks come from distinct SERPs, so their scores are structurally incomparable


## Decision
- `deep-research` scans web AND news by DEFAULT: every sub-query of the fan-out runs vertical `all`; the new opt-out flag `--no-news` restores the web-only v0.8.8 behavior
- One Chrome session PER SUB-QUERY serves both verticals: the session navigates the web SERP first, then the news SERP (`run_chrome_all_search` orchestration shared between the single-query pipeline and the parallel fan-out)
- Fail-fast validation BEFORE the fan-out (exit 2, `INVALID_CONFIG`): without `--no-news`, builds lacking the `chrome` feature, `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1`, and `detect_chrome` failure all abort with a remediation message (mirrors the ADR-0010 `build_config` guards)
- News RRF is computed in its OWN score space via `aggregate_news`, SEPARATE from the web RRF: rank-based fusion over distinct lists yields incomparable scores, so `noticias[].score` is NEVER fused or compared with `resultados[].score`
- News dedupe reuses the canonical-URL blake3 hash (`canonical_hash`) of the web aggregation; ties in the news ranking are broken by recency using `relative_date_to_minutes` (PT "há N ..."/"ontem" and EN "N ... ago"/"yesterday" forms, overflow-safe via `checked_mul`, unparsed dates sort last), then by stable first-seen order
- `ChromeAllSearchOutcome { web, news }` captures per-vertical outcomes: `web: Option<...>` (`None` ⇒ caller degrades web to reqwest), `news: Result<...>` (`Err` ⇒ Chrome launch/navigation failed, news has no fallback); the orchestration function itself returns `Err` ONLY for cancellation (Ctrl+C / global timeout via `tokio::select!`), so partial results always surface
- Per-sub-query news semantics in `SubQueryOutcome`: `news: None` without `--no-news` means the news scan was expected but Chrome fell mid-flight ⇒ `news_indisponivel: true` and `quantidade_noticias` omitted; `Some([])` is an honest zero ⇒ `quantidade_noticias: 0` and no `news_indisponivel`
- Dual synthesis: with news items present, `synthesize_dual` splits `--budget-tokens` ~70% web / ~30% news (`saturating_mul(7)/10`) and renders a two-section report; without news it degrades to the single-section `synthesize`
- Exit code: 0 when EITHER vertical produced results (web OR news > 0); exit 5 (`ZERO_RESULTS`) only when both are empty
- Envelope contract: root `noticias[]` (aggregated `AggregatedNewsItem`) and `quantidade_noticias` are ALWAYS serialized (empty/0 with `--no-news`); `metadados.total_noticias_unicas` is always present; `metadados.sub_queries[]` gains optional `quantidade_noticias` and `news_indisponivel`
- Rejected alternative: cross-vertical RRF fusion into a single ranked list — RRF scores from distinct lists are incomparable; fusing them would produce a meaningless ordering
- Rejected alternative: the internal `duckduckgo.com/news.js?vqd=` endpoint — the `vqd` token rotates per query and the endpoint is undocumented and unstable (same rejection as ADR-0010)
- Rejected alternative: a parallel news session track (separate Chrome session per vertical) — it would DOUBLE the Chrome sessions per sub-query, doubling resource cost and anti-bot exposure for no ranking benefit


## Consequences
- Deep-research reports now cite fresh articles with source and relative date alongside web evidence, by default
- News availability inside deep-research depends on Chrome + (on Linux) Xvfb; `--no-news` is the explicit escape hatch and the ONLY degraded path by design
- Consumers get two independent ranked lists (`resultados[]`, `noticias[]`) and must NOT merge them by score
- Mid-flight Chrome loss degrades gracefully: affected sub-queries report `news_indisponivel: true` while web results still aggregate; the run does not abort
- Chrome-less environments fail fast with exit 2 and a remediation message instead of silently dropping the news vertical
- `--budget-tokens` now feeds two sections; web synthesis keeps ~70% of the budget, so web-only consumers see slightly shorter summaries when news is present


## Files Changed
- src/cli.rs: `--no-news` flag on the `deep-research` subcommand
- src/lib.rs: `execute_deep_research` fail-fast Chrome guards (exit 2), vertical selection, exit-code OR rule
- src/deep_research.rs: `DeepResearchArgs.no_news`, `SubQueryOutcome.{quantidade_noticias, news_indisponivel}`, `DeepResearchOutput.{noticias, quantidade_noticias}`, `DeepResearchMetadata.total_noticias_unicas`, news aggregation stage
- src/aggregation.rs: `AggregatedNewsItem`, `aggregate_news` (news-only RRF), `relative_date_to_minutes` (recency tiebreak, `checked_mul`)
- src/synthesis.rs: `synthesize_dual` (~70/30 budget split, dual-section rendering)
- src/pipeline.rs: `ChromeAllSearchOutcome`, `run_chrome_all_search` (single session, web SERP → news SERP, cancellation via `tokio::select!`)
- docs/schemas/: `deep-research-output.schema.json` (new), `multi-search-output.schema.json` (batch news note)


## References
- ADR-0007 (Chrome Primary Transport, v0.8.0)
- ADR-0009 (Headed Xvfb Private, v0.8.7)
- ADR-0010 (News Vertical, v0.8.9)
- gaps.md (GAP-WS-105)
- CHANGELOG.md [0.8.9]
