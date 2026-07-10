// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: I/O-bound orchestrator (dispatches to parallel.rs and content_fetch.rs).
// No direct parallelism in this module — delegates fan-out to parallel::execute_*.
// Bounded mpsc channel provides backpressure between producer and consumer in streaming mode.
//! Orchestration of the CLI execution flow.
//!
//! In iteration 2, decides between single-query and multi-query flow based on
//! the number of effective queries (after combining positional + file + stdin,
//! dedup and empty-string filtering).
//!
//! - Single-query (1 query): uses the legacy `execute_single_search` flow and emits `SearchOutput`.
//! - Multi-query (>=2 queries): delegates to `parallel::execute_parallel_searches`
//!   and emits `MultiSearchOutput`.

use crate::content_fetch;
use crate::error::CliError;
// GAP F4 v0.8.9: `extraction` é consumido apenas pelos caminhos Chrome-primary
// (`#[cfg(feature = "chrome")]`) — o gate no import mantém o build
// `--no-default-features` sem warnings.
#[cfg(feature = "chrome")]
use crate::extraction;
use crate::http;
use crate::http::ProxyConfig;
// v0.7.10 B1 fix: removed `use crate::output;` — the early
// `print_line_stdout` call was deleted, so the import is unused.
use crate::parallel;
use crate::probe_deep;
use crate::search;
use crate::types::{
    Config, MultiSearchOutput, SearchMetadata, SearchOutput, SelectorConfig, ZeroCause,
};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// Result emitted by the pipeline — may be a single output, aggregated multi output, or an already-emitted stream.
///
/// The `Stream` variant indicates that output was already emitted incrementally by
/// the consumer; the final `output` step MUST NOT re-emit anything. Only the
/// aggregated statistics are available for logging / exit-code decisions.
#[derive(Debug, Clone)]
pub enum PipelineResult {
    /// Single-query execution produced one output.
    Single(Box<SearchOutput>),
    /// Multi-query execution produced aggregated output.
    Multi(Box<MultiSearchOutput>),
    /// Streaming mode — output already emitted incrementally; only stats remain.
    Stream(crate::parallel::StreamStats),
}

impl PipelineResult {
    /// Total results summed across all queries (used for exit-code decisions).
    ///
    /// For `Stream` returns `successes` — a sufficient approximation for exit codes 0/5
    /// (success vs zero-results).
    pub fn total_results(&self) -> u32 {
        match self {
            // GAP-WS-104: soma `news_count` — news-only com notícias ⇒ exit 0;
            // news-only sem notícias ⇒ exit 5 (zero legítimo).
            PipelineResult::Single(s) => s.result_count.saturating_add(s.news_count.unwrap_or(0)),
            PipelineResult::Multi(m) => m
                .searches
                .iter()
                .map(|b| b.result_count.saturating_add(b.news_count.unwrap_or(0)))
                .fold(0u32, |acc, v| acc.saturating_add(v)),
            PipelineResult::Stream(e) => e.successes,
        }
    }

    /// GAP-WS-092 + GAP-WS-093: populate compat fields in all inner `SearchOutputs`.
    pub fn fill_compat_fields(&mut self) {
        match self {
            PipelineResult::Single(s) => s.fill_compat_fields(),
            PipelineResult::Multi(m) => {
                for search in &mut m.searches {
                    search.fill_compat_fields();
                }
            }
            PipelineResult::Stream(_) => {}
        }
    }
}

/// Entry point for iteration 2: decides single vs multi based on `configuracoes.queries`.
///
/// `cancelamento` is the token that signals SIGINT (ctrl+c). In single-query mode
/// cancellation only affects the request via `reqwest` timeout; in multi-query mode it
/// is propagated explicitly to each task.
///
/// # Errors
///
/// Returns an error if the query list is empty, if the HTTP client cannot be built,
/// or if the underlying single-query or multi-query execution fails unrecoverably.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future propagates the cancellation
/// token to any in-flight sub-tasks, which will terminate gracefully.
pub async fn execute_pipeline(
    config: Config,
    cancellation: CancellationToken,
) -> Result<PipelineResult, CliError> {
    match config.queries.len() {
        0 => Err(CliError::InvalidConfig {
            message: "no queries to execute (list empty after filtering)".into(),
        }),
        1 => {
            if config.stream_mode {
                tracing::warn!(
                    "--stream ignored in single-query mode (only 1 effective query); \
                     emitting default aggregated output"
                );
            }
            // Clone intentional: overwrites query field for single-query compatibility.
            // Cost: ~15 String clones, executed exactly once per CLI invocation.
            let mut cfg_single = config.clone();
            cfg_single.query = cfg_single.queries[0].clone();
            let output = execute_single_search(&cfg_single, &cancellation).await?;
            persist_cookies(&cfg_single);
            Ok(PipelineResult::Single(Box::new(output)))
        }
        _ => {
            if config.stream_mode {
                return execute_pipeline_streaming(config, cancellation).await;
            }
            let queries = config.queries.clone();
            // Persist cookies after the parallel search completes, using
            // a clone of `config` because `config` is moved into the
            // search call.
            let config_for_persist = config.clone();
            let multi = parallel::execute_parallel_searches(queries, config, cancellation).await?;
            persist_cookies(&config_for_persist);
            Ok(PipelineResult::Multi(Box::new(multi)))
        }
    }
}

/// Persists the cookie jar to disk after the search completes. v0.7.3 PR2.
fn persist_cookies(config: &Config) {
    if let Some(persistent_jar) = config.persistent_jar.as_ref() {
        persistent_jar.save();
    }
}

/// Performs the warm-up `GET https://duckduckgo.com/` request to populate
/// session cookies. Failures are surfaced to the caller but never fatal;
/// the caller logs and continues. v0.7.3 PR2. Residual HTTP harness path.
async fn do_warmup(client: &reqwest::Client, cfg: &Config) -> Result<(), CliError> {
    let warmup_url = "https://duckduckgo.com/";
    tracing::info!(url = warmup_url, "Warming up session with cookie jar");
    let response = client
        .get(warmup_url)
        .send()
        .await
        .map_err(|e| CliError::HttpError {
            message: format!("warm-up request to {warmup_url} failed: {e}"),
            cause: None,
        })?;
    tracing::info!(
        status = response.status().as_u16(),
        url = warmup_url,
        "warm-up response received"
    );
    let _ = cfg;
    Ok(())
}

/// Pipeline in streaming mode — emits results as tasks complete.
///
/// The spawned consumer drains the mpsc channel and emits NDJSON/text/markdown line by line.
/// Returns `PipelineResult::Stream` at the end, indicating there is nothing left to emit.
async fn execute_pipeline_streaming(
    config: Config,
    cancellation: CancellationToken,
) -> Result<PipelineResult, CliError> {
    use crate::types::OutputFormat;
    use tokio::sync::mpsc;

    let format = config.format;
    let output_file = config.output_file.clone();
    let queries = config.queries.clone();
    let paralelismo = config.parallelism.max(1) as usize;

    // Buffer = parallelism * 2, per spec. Min 2 to avoid trivial starvation.
    let (tx, mut rx) = mpsc::channel::<(usize, SearchOutput)>(paralelismo.saturating_mul(2).max(2));

    // Spawn consumer: drains items and emits per format.
    let consumer = tokio::spawn(async move {
        let mut emitidos: u64 = 0;
        while let Some((index, output)) = rx.recv().await {
            let resolved_format = match format {
                OutputFormat::Auto | OutputFormat::Json => OutputFormat::Json,
                outro => outro,
            };
            let res = match resolved_format {
                OutputFormat::Json | OutputFormat::Auto => {
                    crate::output::emit_ndjson(&output, output_file.as_deref())
                }
                OutputFormat::Text => {
                    crate::output::emit_stream_text(index, &output, output_file.as_deref())
                }
                OutputFormat::Markdown => {
                    crate::output::emit_stream_markdown(index, &output, output_file.as_deref())
                }
            };
            if let Err(erro) = res {
                if crate::output::is_broken_pipe(&erro) {
                    tracing::info!("BrokenPipe in streaming — stopping consumer");
                    return Ok(());
                }
                tracing::error!(?erro, "failed to emit streaming item — aborting consumer");
                return Err(erro);
            }
            emitidos = emitidos.saturating_add(1);
        }
        tracing::info!(emitidos, "streaming consumer finished");
        Ok::<(), CliError>(())
    });

    let stats =
        parallel::execute_parallel_searches_streaming(queries, config, cancellation, tx).await?;

    match consumer.await {
        Ok(Ok(())) => {}
        Ok(Err(erro)) => return Err(erro),
        Err(erro_join) => {
            if erro_join.is_panic() {
                tracing::error!(?erro_join, "streaming consumer panicked");
            } else {
                tracing::warn!(?erro_join, "streaming consumer cancelled");
            }
            return Err(CliError::NetworkError {
                message: format!("streaming consumer panicked: {erro_join}"),
            });
        }
    }

    Ok(PipelineResult::Stream(stats))
}

/// Executes the full flow for a single-query search with pagination, retry and Lite fallback.
///
/// # Errors
///
/// Returns an error if the HTTP client cannot be built. Search failures (rate limit,
/// timeout, block) are captured in the returned [`SearchOutput`] error fields rather
/// than propagated as `Err`.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future aborts the in-flight HTTP
/// request; any partial pagination state is discarded without side effects.
pub async fn execute_single_search(
    cfg: &Config,
    cancellation: &CancellationToken,
) -> Result<SearchOutput, CliError> {
    // `cancellation` is wired through to chromiumoxide fallback and content_fetch
    // enrichment; reference it here to document its purpose and suppress warnings.
    let _ = cancellation;
    let start = Instant::now();

    let config_proxy = ProxyConfig::from_options(cfg.proxy.as_deref(), cfg.no_proxy);

    // v0.7.10 GAP-WS-60 fix: when `--identity-profile` pins a specific
    // family+platform, build a fresh `BrowserProfile` from the matching
    // identity in the 12-identity pool. The original `cfg.browser_profile`
    // (built from `user-agents.toml` or embedded defaults) is discarded
    // for this single query — the session is fully pinned to the chosen
    // identity until the process exits. `Auto` (default) keeps the
    // legacy behavior (uses `cfg.browser_profile` directly).
    //
    // Also capture the effective UA + identity tag so the SearchMetadata
    // output reports what the request actually used, not the static
    // value of `cfg.user_agent` (which still reflects the original
    // `user-agents.toml` selection).
    // GAP F4 v0.8.9: as reatribuições de `effective_identity_tag` acontecem
    // apenas sob `#[cfg(feature = "chrome")]` — mesmo padrão de `chrome_result`.
    #[cfg_attr(not(feature = "chrome"), allow(unused_mut))]
    let (effective_profile, effective_user_agent, mut effective_identity_tag): (
        http::BrowserProfile,
        String,
        Option<String>,
    ) = match crate::identity::browser_profile_for_cli_identity(cfg.identity_profile, None) {
        Some(pinned) => {
            // Use the canonical tag from `IdentityProfile::tag()` via
            // `identity_tag_for_cli_identity` so the success path matches
            // the failure paths (`failure_output`, `error_output`).
            // Previously this was FNV-1a(UA) — same format
            // `<family>-<platform>-<16hex>` but different hex bytes; the
            // canonical seed is more stable across UA string tweaks.
            let tag = crate::identity::identity_tag_for_cli_identity(cfg.identity_profile, None)
                .unwrap_or_default();
            tracing::info!(
                identity_profile = ?cfg.identity_profile,
                pinned_ua = %pinned.user_agent,
                pinned_tag = %tag,
                "pinned to fixed identity per --identity-profile"
            );
            (pinned.clone(), pinned.user_agent.clone(), Some(tag))
        }
        None => (cfg.browser_profile.clone(), cfg.user_agent.clone(), None),
    };

    let client = http::build_client_with_proxy_and_cookies(
        &effective_profile,
        cfg.timeout_seconds,
        &cfg.language,
        &cfg.country,
        &config_proxy,
        cfg.cookie_provider.clone(),
    )?;

    // GAP-WS-113: HTTP warm-up is residual harness-only. Production warm-up is
    // Chrome CDP (GAP-WS-077 inside browser launch / extract paths).
    if cfg.warmup_enabled && crate::chrome_policy::http_test_harness_active() {
        if let Err(e) = do_warmup(&client, cfg).await {
            tracing::warn!(error = %e, "warm-up request failed; continuing without it");
        }
    }

    // v0.7.10 P5: probe-deep scheduler — when `cfg.pre_flight == true`,
    // run a minimal probe before the real search and short-circuit on
    // captcha/ghost-block so the operator does not waste a full
    // search round-trip on an already-blocked environment.
    // GAP F2 v0.8.9: o pre-flight sonda o endpoint HTML web via reqwest — um
    // sinal irrelevante (e potencialmente falso-positivo fatal) para a vertical
    // news, que é Chrome-only e sem fallback HTTP. Um falso positivo abortaria
    // a busca news com exit 3 sem nunca tentá-la. O probe roda apenas quando a
    // execução inclui a vertical web (`web` e `all`).
    if cfg.pre_flight && !pre_flight_applies(cfg) {
        tracing::info!(
            vertical = cfg.vertical.as_str(),
            "pre-flight nao se aplica a vertical news (Chrome-only, sem endpoint HTTP); probe pulado"
        );
    }
    // GAP-WS-113: production pre-flight runs on the SHARED Chrome SERP session
    // inside `execute_chrome_web_search_on_browser` (one launch per invocation).
    // Residual HTTP pre-flight remains harness-only below.
    if pre_flight_applies(cfg) && crate::chrome_policy::http_test_harness_active() {
        let probe_started = std::time::Instant::now();
        let probe_result = client
            .post(crate::search::html_base_url())
            .form(&[("q", "the quick brown fox jumps over the lazy dog")])
            .send()
            .await;
        match probe_result {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = crate::decompress::response_body_string(response)
                    .await
                    .unwrap_or_default();
                let latency = probe_started
                    .elapsed()
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64;
                let outcome = probe_deep::classify_probe_outcome(&body, status, latency);
                if !outcome.healthy {
                    tracing::warn!(
                        marker = outcome.marker,
                        kind = outcome.kind.as_str(),
                        http_status = outcome.http_status,
                        latency_ms = outcome.latency_ms,
                        "pre-flight detected block; short-circuiting search"
                    );
                    // B1 fix: do NOT early-print via `print_line_stdout` —
                    // the caller in lib.rs already serializes the returned
                    // SearchOutput exactly once via `output::emit_result`.
                    // Printing here caused two JSON objects to be emitted
                    // back-to-back (broken pipe contract for `| jaq`).
                    // The pre-flight context (kind, marker, latency, message)
                    // travels inside the SearchOutput envelope below; the
                    // caller maps `error: Some("pre_flight_blocked")` to
                    // exit code 3 (anti-bot) instead of 0.
                    return Ok(SearchOutput {
                        query: cfg.query.clone(),
                        engine: "duckduckgo".to_string(),
                        endpoint: cfg.endpoint.as_str().to_string(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        region: format!("{}-{}", cfg.country, cfg.language),
                        result_count: 0,
                        results: vec![],
                        pages_fetched: 0,
                        news: None,
                        news_count: None,
                        error: Some("pre_flight_blocked".to_string()),
                        message: Some(format!(
                            "pre-flight detected captcha/ghost-block via marker {}",
                            outcome.marker
                        )),
                        metadata: SearchMetadata {
                            execution_time_ms: outcome.latency_ms,
                            selectors_hash: "pre-flight".to_string(),
                            retries: 0,
                            retries_configured: None,
                            used_fallback_endpoint: false,
                            concurrent_fetches: 0,
                            fetch_successes: 0,
                            fetch_failures: 0,
                            used_chrome: false,
                            chrome_attempted: false,
                            user_agent: effective_user_agent.clone(),
                            used_proxy: config_proxy.is_active(),
                            identity_used: None,
                            cascade_level: None,
                            pre_flight_fired: true,
                            zero_cause: None,
                            sugestao_proxima_acao: None,
                            bytes_raw: None,
                            bytes_decompressed: None,
                            cascade_level_observed: None,
                            result_count_compat: None,
                            endpoint_used_compat: None,
                            vertical_used: None,
                        },
                    });
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "pre-flight request failed; continuing with real search");
            }
        }
    }

    tracing::info!(query = %cfg.query, endpoint = cfg.endpoint.as_str(), "Executing search");

    // v0.8.0: Chrome-primary search — try Chrome FIRST when the feature is enabled.
    // Chrome produces a real browser TLS fingerprint that passes Cloudflare checks,
    // yielding real SERP HTML. On failure, falls back to the reqwest HTTP path below.
    #[allow(unused_assignments, unused_mut)]
    let mut chrome_attempted = false;
    #[allow(unused_assignments, unused_mut)]
    let mut chrome_result_used = false;
    #[allow(unused_mut)]
    let mut chrome_result: Option<search::AggregatedSearchResult> = None;

    // GAP-WS-104 v0.8.9: resultado da vertical news (resultados + body bruto
    // renderizado, consumido pela classificação de zero-cause). `None` no modo
    // web default — o contrato JSON permanece byte-idêntico pré-v0.8.9.
    #[cfg(feature = "chrome")]
    let mut news_outcome: Option<(Vec<crate::types::NewsResult>, String)> = None;

    // GAP-WS-113: production always requires Chrome. Harness may skip Chrome.
    if let Err(err) = crate::chrome_policy::require_chrome_transport() {
        if !crate::chrome_policy::http_test_harness_active() {
            return Ok(chrome_transport_failure_output(cfg, &err, start));
        }
    }

    #[cfg(feature = "chrome")]
    if !crate::chrome_policy::chrome_disabled_by_env()
        && !crate::chrome_policy::http_test_harness_active()
    {
        chrome_attempted = true;
        let chrome_ua = {
            let candidate =
                crate::identity::browser_profile_for_cli_identity(cfg.identity_profile, None)
                    .map(|p| p.user_agent.clone())
                    .unwrap_or_else(|| effective_user_agent.clone());
            if candidate.contains("Firefox/")
                || (candidate.contains("Safari/") && !candidate.contains("Chrome/"))
            {
                tracing::info!(
                    original_ua = %candidate,
                    "UA mismatch: Safari/Firefox UA with Chromium TLS — forcing Chrome UA"
                );
                crate::identity::chrome_only_ua_for_platform()
            } else if !crate::identity::ua_platform_matches_host(&candidate) {
                tracing::info!(
                    original_ua = %candidate,
                    "UA platform mismatch with host — forcing platform-correct Chrome UA (GAP-WS-107b)"
                );
                crate::identity::chrome_only_ua_for_platform()
            } else {
                candidate
            }
        };

        if cfg.vertical.includes_news() {
            // GAP-WS-104 v0.8.9: `--vertical news|all` roteia por UMA sessão
            // Chrome (warm-up GAP-WS-077 único): web SERP primeiro (modo all),
            // depois news SERP. News é Chrome-only por design — a SERP
            // `ia=news&iar=news` exige JavaScript e NÃO tem fallback HTTP.
            // GAP-WS-105 v0.8.9: a orquestração vive em
            // `execute_chrome_all_search_pub`, compartilhada com o fan-out
            // paralelo (`parallel.rs`); cancelamento (Ctrl+C/timeout global)
            // propaga como `Err` com o mesmo `CliError::NetworkError
            // { "execution cancelled ..." }` do caminho reqwest (GAP F5).
            let outcome = execute_chrome_all_search_pub(cfg, &chrome_ua, cancellation).await?;
            if let Some(result) = outcome.web {
                chrome_result = Some(result);
                chrome_result_used = true;
                // GAP-WS-095: populate identity_used from Chrome UA when Auto.
                if effective_identity_tag.is_none() {
                    effective_identity_tag = identity_tag_for_chrome_ua(&chrome_ua);
                }
            }
            match outcome.news {
                Ok(outcome_news) => news_outcome = Some(outcome_news),
                Err(err) => {
                    if !cfg.vertical.includes_web() {
                        // GAP F1 v0.8.9: news-only NUNCA propaga `Err` cru —
                        // `-f json` SEMPRE emite envelope JSON estruturado.
                        return Ok(news_only_chrome_failure_output(cfg, &err, start));
                    }
                    news_outcome = Some((Vec::new(), String::new()));
                }
            }
        } else {
            match execute_chrome_search(cfg, &chrome_ua, cancellation).await {
                Ok(result) => {
                    tracing::info!(
                        chrome_results = result.results.len(),
                        "Chrome-primary search succeeded"
                    );
                    chrome_result = Some(result);
                    chrome_result_used = true;
                    // GAP-WS-095: populate identity_used from Chrome UA when Auto.
                    if effective_identity_tag.is_none() {
                        effective_identity_tag = identity_tag_for_chrome_ua(&chrome_ua);
                    }
                }
                Err(err) => {
                    // GAP-WS-113: never fall back to reqwest — structured failure.
                    // Pre-flight on shared session returns Blocked → pre_flight envelope.
                    if matches!(err, CliError::Blocked) {
                        tracing::warn!("Chrome shared-session pre-flight blocked (GAP-WS-113)");
                        let mut out = chrome_transport_failure_output(cfg, &err, start);
                        out.error = Some("pre_flight_blocked".to_string());
                        out.metadata.pre_flight_fired = true;
                        out.metadata.used_chrome = true;
                        out.metadata.chrome_attempted = true;
                        out.metadata.sugestao_proxima_acao = Some(
                            "Pre-flight Chrome detectou bloqueio (GAP-WS-113). Aguarde 300s ou use --proxy."
                                .to_string(),
                        );
                        return Ok(out);
                    }
                    tracing::error!(
                        error = %err,
                        "Chrome-primary search failed — HTTP fallback removed (GAP-WS-113)"
                    );
                    return Ok(chrome_transport_failure_output(cfg, &err, start));
                }
            }
        }
    }

    #[allow(unused_mut)]
    let mut agregado = if let Some(cr) = chrome_result {
        cr
    } else if !cfg.vertical.includes_web() {
        // GAP-WS-104: news-only — web pipeline intentionally empty.
        search::AggregatedSearchResult {
            results: Vec::new(),
            first_body: String::new(),
            pages_fetched: 0,
            attempts: 1,
            used_fallback_lite: false,
            effective_endpoint: crate::types::Endpoint::Html,
            bytes_in: 0,
            bytes_out: 0,
        }
    } else if crate::chrome_policy::http_test_harness_active() {
        // Residual HTTP path for wiremock tests only (feature http-test-harness).
        let flag_rate_limit = Arc::new(AtomicBool::new(false));
        let search_result = search::search_with_pagination(
            &client,
            cfg,
            &cfg.query,
            &flag_rate_limit,
            cancellation,
        )
        .await;
        let failure_output_val = match &search_result {
            Err(reason) => Some(failure_output(cfg, reason, start)),
            Ok(_) => None,
        };
        if let Some(out) = failure_output_val {
            return Ok(out);
        }
        search_result.map_err(|reason| CliError::PipelineInvariantViolation {
            message: format!(
                "search_result reached extract_ok_path with Err after early return; reason={reason:?}"
            ),
        })?
    } else {
        // GAP-WS-113: Chrome did not produce a web result and harness is off.
        let err = CliError::InvalidConfig {
            message: "Chrome transport did not return SERP results (GAP-WS-113).                       Install Chrome/Chromium or pass --chrome-path."
                .into(),
        };
        return Ok(chrome_transport_failure_output(cfg, &err, start));
    };

    // GAP-WS-090: truncate results to --num when Chrome headed returns a full
    // page (typically 10). Without this, --num is silently ignored.
    if let Some(max) = cfg.num_results {
        let max = max as usize;
        if agregado.results.len() > max {
            agregado.results.truncate(max);
        }
    }

    let quantidade = u32::try_from(agregado.results.len()).unwrap_or(u32::MAX);
    let selectors_hash = calculate_selectors_hash(&cfg.selectors);
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = chrono::Utc::now().to_rfc3339();
    // Retries = attempts - 1 (the first request does not count as a retry).
    let retries_count = agregado.attempts.saturating_sub(1);

    // GAP-AUD-002 + GAP-AUD-010 v0.8.0: cascade_level_observed deve refletir
    // o nível de cascata efetivamente exercido. Preferimos o cache do
    // probe-dep (`cfg.last_probe_cascade_level`) quando disponível (caso
    // --pre-flight dentro da mesma invocação do processo). Caso contrário,
    // derivamos do sinal observável: 1 retry com fallback = nível 1;
    // 2+ retries = nível 2+. Sem retries = nível 0.
    let cascade_level_observed = cfg
        .last_probe_cascade_level
        .or_else(|| Some(derive_cascade_level_from_attempts(&agregado)));

    let mut metadata_val = SearchMetadata {
        execution_time_ms: elapsed_ms,
        selectors_hash,
        retries: retries_count,
        retries_configured: Some(cfg.retries),
        used_fallback_endpoint: agregado.used_fallback_lite,
        concurrent_fetches: 0,
        fetch_successes: 0,
        fetch_failures: 0,
        used_chrome: chrome_result_used,
        chrome_attempted,
        user_agent: effective_user_agent.clone(),
        used_proxy: config_proxy.is_active(),
        identity_used: effective_identity_tag.clone(),
        cascade_level: None,
        pre_flight_fired: false,
        zero_cause: None,
        sugestao_proxima_acao: None,
        // GAP-NEW-002 v0.8.0: telemetria de descompressão HTTP. Quando
        // , a taxa de compressão é
        // . Quando iguais,
        // o body veio como identity (sem encoding) ou via .
        bytes_raw: Some(agregado.bytes_in),
        bytes_decompressed: Some(agregado.bytes_out),
        cascade_level_observed,
        result_count_compat: None,
        endpoint_used_compat: None,
        // GAP-WS-104: `None` no modo web default preserva o contrato JSON
        // byte-idêntico pré-v0.8.9 (`skip_serializing_if`).
        vertical_used: (cfg.vertical != crate::types::VerticalMode::Web)
            .then(|| cfg.vertical.as_str().to_string()),
    };

    // GAP-AUD-003 v0.8.0: classificar zero-result causalmente.
    // Só roda no caminho de zero (`quantidade == 0`) para não pagar custo em sucesso.
    // GAP-WS-104: no modo news-only o pipeline web não executa — a classificação
    // de zero passa a ser responsabilidade do bloco news abaixo.
    if quantidade == 0 && cfg.vertical.includes_web() {
        let inputs = ZeroClassificationInputs {
            body: &agregado.first_body,
            pre_flight_enabled: cfg.pre_flight,
            pre_flight_fired: false,
            execution_time_ms: metadata_val.execution_time_ms,
            retries: metadata_val.retries,
            concurrent_fetches: metadata_val.concurrent_fetches,
            last_probe_cascade_level: cfg.last_probe_cascade_level,
        };
        let cause = classify_zero_result(&inputs);
        metadata_val.zero_cause = Some(cause);
        metadata_val.sugestao_proxima_acao =
            sugestao_proxima_acao_para_zero(cause).map(str::to_string);
    }

    // GAP-NEW-004 v0.8.0: auto-fallback lite — wire-in acontece após
    // a construção de  (ver bloco abaixo).

    let mut output = SearchOutput {
        query: cfg.query.clone(),
        engine: "duckduckgo".to_string(),
        endpoint: agregado.effective_endpoint.as_str().to_string(),
        timestamp,
        region: search::format_kl(&cfg.language, &cfg.country),
        result_count: quantidade,
        results: agregado.results,
        pages_fetched: agregado.pages_fetched,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: metadata_val,
    };

    // GAP-WS-104 v0.8.9: fiação da vertical news no envelope. Popula
    // `noticias`/`quantidade_noticias` SOMENTE quando `--vertical news|all`
    // executou (no modo web default `news_outcome` é `None` — contrato
    // byte-idêntico). Cap `--num` no mesmo padrão GAP-WS-090 da web.
    #[cfg(feature = "chrome")]
    if let Some((mut news_results, news_body)) = news_outcome.take() {
        if let Some(max) = cfg.num_results {
            let max = max as usize;
            if news_results.len() > max {
                news_results.truncate(max);
            }
        }
        let news_quantidade = u32::try_from(news_results.len()).unwrap_or(u32::MAX);
        output.news = Some(news_results);
        output.news_count = Some(news_quantidade);

        // Zero news: interstitial anti-bot no body renderizado ⇒ AntiBot;
        // senão ⇒ VerticalSemResultados (zero LEGÍTIMO ⇒ exit 5, não 6).
        // Regras de precedência:
        // - modo all com web>0: sucesso segue a web — zero_cause fica None;
        // - modo all com web==0: a classificação web (mais informativa) já
        //   rodou e é preservada (`zero_cause.is_none()` falha);
        // - news-only: a classificação web foi pulada — este bloco decide.
        if news_quantidade == 0 && output.result_count == 0 && output.metadata.zero_cause.is_none()
        {
            let cause = if crate::probe_deep::detectar_interstitial(&news_body)
                != crate::probe_deep::InterstitialKind::None
            {
                crate::types::ZeroCause::AntiBot
            } else {
                crate::types::ZeroCause::VerticalSemResultados
            };
            output.metadata.zero_cause = Some(cause);
            output.metadata.sugestao_proxima_acao =
                sugestao_proxima_acao_para_zero(cause).map(str::to_string);
        }

        // GAP F3 v0.8.9: no modo `all` com web>0, `causa_zero` permanece `None`
        // por semântica — o campo descreve o zero TOTAL do envelope — e o
        // contrato JSON NÃO ganha campos novos. Para não descartar o
        // diagnóstico de bloqueio da news em silêncio, o aviso vai ao stderr
        // via `tracing::warn` (fora do stdout JSON).
        if news_quantidade == 0
            && output.result_count > 0
            && crate::probe_deep::detectar_interstitial(&news_body)
                != crate::probe_deep::InterstitialKind::None
        {
            tracing::warn!(
                sugestao_proxima_acao = "vertical news bloqueada por interstitial anti-bot; \
                 a web retornou resultados. Aguarde 300s e re-execute com --vertical news, \
                 ou use --proxy para rotacionar o IP de saida.",
                "news vertical blocked by anti-bot in all mode — noticias vazia"
            );
        }
    }

    // GAP-WS-113: auto-fallback Lite permanently removed (was GAP-NEW-004).

    // Enriquecimento opcional via --fetch-content (iter. 5).
    content_fetch::enrich_with_content(&mut output, &client, cfg, cancellation).await;

    tracing::info!(
        total = output.result_count,
        pages = output.pages_fetched,
        fallback = output.metadata.used_fallback_endpoint,
        fetch_content = cfg.fetch_content,
        fetch_successes = output.metadata.fetch_successes,
        "Search completed successfully"
    );
    Ok(output)
}

/// v0.8.0 Chrome-primary search path.
///
/// Launches headless Chrome via `src/browser.rs::ChromeBrowser`, navigates to the
/// `DuckDuckGo` HTML endpoint for the query, extracts the full HTML and parses
/// results using the same selectors as the reqwest path.
///
/// Returns `Err(CliError)` if Chrome is not installed, if Chrome launch times out,
/// or if the page extraction fails.
#[cfg(feature = "chrome")]
async fn execute_chrome_search(
    cfg: &Config,
    user_agent: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<search::AggregatedSearchResult, CliError> {
    // GAP F5 v0.8.9: launch e navegação respeitam o token de cancelamento via
    // `tokio::select!` — mesmo erro de cancelamento do caminho reqwest.
    let launched = tokio::select! {
        launched = launch_chrome_browser(cfg, user_agent) => launched,
        _ = cancellation.cancelled() => return Err(chrome_cancelled_error("launch")),
    };
    let mut browser = launched?;
    // O `Option` intermediário libera o empréstimo de `browser` antes do
    // shutdown, executado nos DOIS ramos (concluído e cancelado).
    let selected = tokio::select! {
        result = execute_chrome_web_search_on_browser(&mut browser, cfg) => Some(result),
        _ = cancellation.cancelled() => None,
    };
    browser.shutdown().await.ok();
    selected.unwrap_or_else(|| Err(chrome_cancelled_error("web search")))
}

/// Runs the web-vertical SERP navigation + extraction on an ALREADY-launched
/// Chrome session.
///
/// GAP-WS-104 v0.8.9: extraído de [`execute_chrome_search`] para que
/// `--vertical all` compartilhe a MESMA sessão Chrome (warm-up GAP-WS-077
/// único) entre as SERPs web e news.
///
/// # Errors
///
/// Returns `CliError` when navigation or page extraction fails or times out.
#[cfg(feature = "chrome")]
async fn execute_chrome_web_search_on_browser(
    browser: &mut crate::browser::ChromeBrowser,
    cfg: &Config,
) -> Result<search::AggregatedSearchResult, CliError> {
    use std::time::Duration;

    // GAP-WS-113: Chrome SERP always uses HTML canonical endpoint — never Lite.
    // Same browser session: optional pre-flight calibration navigation first.
    let extract_timeout = Duration::from_secs(cfg.timeout_seconds.min(20));
    if cfg.pre_flight && cfg.vertical.includes_web() {
        let calib = "the quick brown fox jumps over the lazy dog";
        let calib_url = search::build_search_url(
            calib,
            &cfg.language,
            &cfg.country,
            crate::types::Endpoint::Html,
            cfg.time_filter,
            cfg.safe_search,
        );
        match crate::browser::extract_html_with_chrome(
            browser,
            &calib_url,
            512 * 1024,
            extract_timeout,
        )
        .await
        {
            Ok(body) => {
                let outcome = probe_deep::classify_probe_outcome(&body, 200, 0);
                if !outcome.healthy {
                    return Err(CliError::Blocked);
                }
                tracing::info!(
                    body_len = body.len(),
                    "pre-flight on shared Chrome session healthy"
                );
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "pre-flight on shared Chrome session failed — continuing to real query"
                );
            }
        }
    }

    let url = search::build_search_url(
        &cfg.query,
        &cfg.language,
        &cfg.country,
        crate::types::Endpoint::Html,
        cfg.time_filter,
        cfg.safe_search,
    );

    let html = crate::browser::extract_html_with_chrome(browser, &url, 256 * 1024, extract_timeout)
        .await
        .map_err(|e| CliError::InvalidConfig {
            message: format!("Chrome HTML extraction failed: {e}"),
        })?;

    let results = extraction::extract_results_with_strategies_cfg(&html, &cfg.selectors);

    Ok(search::AggregatedSearchResult {
        results,
        first_body: html.clone(),
        pages_fetched: 1,
        attempts: 1,
        used_fallback_lite: false,
        effective_endpoint: crate::types::Endpoint::Html,
        bytes_in: html.len() as u64,
        bytes_out: html.len() as u64,
    })
}

/// GAP-WS-095 + GAP-WS-104: resolve a tag de identidade correspondente ao UA
/// Chrome efetivamente usado (pool Auto). Compartilhada pelos caminhos web e
/// news|all do bloco Chrome-primary.
#[cfg(feature = "chrome")]
fn identity_tag_for_chrome_ua(chrome_ua: &str) -> Option<String> {
    let pool = crate::identity::IdentityPool::new(None);
    pool.iter()
        .find(|p| p.user_agent == chrome_ua)
        .map(|matched| matched.tag())
}

/// Public wrapper for `execute_chrome_search` used by the parallel executor.
///
/// # Errors
///
/// Returns `CliError` when Chrome is unavailable or page extraction fails.
#[cfg(feature = "chrome")]
pub async fn execute_chrome_search_pub(
    cfg: &Config,
    user_agent: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<search::AggregatedSearchResult, CliError> {
    execute_chrome_search(cfg, user_agent, cancellation).await
}

/// Outcome of a `--vertical news|all` Chrome session (GAP-WS-105 v0.8.9).
///
/// Produzido por [`execute_chrome_all_search_pub`], que orquestra UMA sessão
/// Chrome (warm-up GAP-WS-077 único): web SERP primeiro (quando a vertical
/// inclui web), depois news SERP. Consumido pelo pipeline single-query e
/// pelo fan-out paralelo (`parallel.rs`), que aplicam contratos distintos
/// às falhas.
#[cfg(feature = "chrome")]
#[derive(Debug)]
pub struct ChromeAllSearchOutcome {
    /// Web SERP result — `Some` apenas quando a vertical inclui web E a
    /// navegação Chrome web teve sucesso. `None` ⇒ o caller decide o
    /// fallback (pipeline e fan-out degradam a web para reqwest no modo all).
    pub web: Option<search::AggregatedSearchResult>,
    /// News outcome — `Ok((resultados, body renderizado))` quando a SERP
    /// news executou (mesmo com zero resultados). `Err` quando o launch do
    /// Chrome ou a navegação news falhou (news é Chrome-only, sem fallback
    /// HTTP): o pipeline news-only emite envelope de falha, o modo all
    /// degrada para news vazia e o fan-out sinaliza `noticias` ausente.
    pub news: Result<(Vec<crate::types::NewsResult>, String), CliError>,
}

/// GAP-WS-105 v0.8.9: orquestração `--vertical news|all` em UMA sessão
/// Chrome, compartilhada entre o pipeline single-query e o fan-out paralelo.
///
/// GAP F5 v0.8.9: launch, web SERP e news SERP correm sob `tokio::select!`
/// com o token de cancelamento — Ctrl+C e timeout global abortam o
/// transporte Chrome com o mesmo `CliError::NetworkError { "execution
/// cancelled ..." }` que o caminho reqwest produz em `parallel.rs`.
///
/// # Errors
///
/// Retorna `Err` SOMENTE para cancelamento (launch, web search ou news
/// search). Falhas de launch/navegação são reportadas dentro de
/// [`ChromeAllSearchOutcome`] para o caller aplicar seu contrato.
#[cfg(feature = "chrome")]
pub async fn execute_chrome_all_search_pub(
    cfg: &Config,
    user_agent: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<ChromeAllSearchOutcome, CliError> {
    let launched = tokio::select! {
        launched = launch_chrome_browser(cfg, user_agent) => launched,
        _ = cancellation.cancelled() => return Err(chrome_cancelled_error("launch")),
    };
    let mut browser = match launched {
        Ok(browser) => browser,
        Err(err) => {
            if cfg.vertical.includes_web() {
                tracing::error!(
                    error = %err,
                    "Chrome launch failed — no HTTP fallback (GAP-WS-113)"
                );
            }
            return Ok(ChromeAllSearchOutcome {
                web: None,
                news: Err(err),
            });
        }
    };

    let mut web = None;
    if cfg.vertical.includes_web() {
        // O `Option` intermediário libera o empréstimo de `browser` antes
        // do shutdown no ramo cancelado.
        let web_result = tokio::select! {
            result = execute_chrome_web_search_on_browser(&mut browser, cfg) => Some(result),
            _ = cancellation.cancelled() => None,
        };
        let Some(web_result) = web_result else {
            browser.shutdown().await.ok();
            return Err(chrome_cancelled_error("web search"));
        };
        match web_result {
            Ok(result) => {
                tracing::info!(
                    chrome_results = result.results.len(),
                    "Chrome-primary search succeeded"
                );
                web = Some(result);
            }
            Err(err) => {
                // GAP-WS-113: surface web failure inside outcome (no reqwest fallback).
                tracing::error!(
                    error = %err,
                    "Chrome-primary web search failed — no HTTP fallback (GAP-WS-113)"
                );
                // Propagate as absence of web; caller must not fall back to HTTP.
                let _ = err;
            }
        }
    }

    let news_result = tokio::select! {
        result = execute_chrome_news_search_on_browser(&mut browser, cfg) => Some(result),
        _ = cancellation.cancelled() => None,
    };
    let Some(news_result) = news_result else {
        browser.shutdown().await.ok();
        return Err(chrome_cancelled_error("news search"));
    };
    browser.shutdown().await.ok();
    if let Err(ref err) = news_result {
        if cfg.vertical.includes_web() {
            tracing::warn!(
                error = %err,
                "News vertical failed in all mode — noticias ficara vazia"
            );
        }
    }
    Ok(ChromeAllSearchOutcome {
        web,
        news: news_result,
    })
}

/// Shared Chrome launch path for the web and news verticals (GAP-WS-104).
///
/// Detects the Chrome binary (flag → env → auto-detection) and launches the
/// stealth browser with the caller-provided user agent.
#[cfg(feature = "chrome")]
async fn launch_chrome_browser(
    cfg: &Config,
    user_agent: &str,
) -> Result<crate::browser::ChromeBrowser, CliError> {
    use crate::browser::{detect_chrome, ChromeBrowser};
    use std::time::Duration;

    let chrome_path =
        detect_chrome(cfg.chrome_path.as_deref()).map_err(|e| CliError::InvalidConfig {
            message: format!("Chrome not detected: {e}"),
        })?;
    let launch_timeout = Duration::from_secs(cfg.timeout_seconds.min(15));
    ChromeBrowser::launch(
        &chrome_path,
        cfg.proxy.as_deref(),
        launch_timeout,
        user_agent,
    )
    .await
}

/// Runs the news-vertical search on an ALREADY-launched Chrome session.
/// GAP-WS-104 v0.8.9.
///
/// Used by `--vertical all` to reuse the same browser (single GAP-WS-077
/// warm-up) after the web SERP navigation. Navigates to the
/// `ia=news&iar=news` SERP built by [`search::build_news_search_url`],
/// polls the React news module (`cfg.selectors.news.container`, 250ms
/// interval) and extracts via the A→B cascade with a 1 MiB cap — the
/// hydrated news SERP is far heavier than the 256 KiB web SERP.
///
/// Returns the extracted news results plus the raw rendered HTML body
/// (used upstream for zero-cause classification).
///
/// # Errors
///
/// Returns `CliError` when navigation or extraction fails or times out.
#[cfg(feature = "chrome")]
pub async fn execute_chrome_news_search_on_browser(
    browser: &mut crate::browser::ChromeBrowser,
    cfg: &Config,
) -> Result<(Vec<crate::types::NewsResult>, String), CliError> {
    use std::time::Duration;

    let url = search::build_news_search_url(
        &cfg.query,
        &cfg.language,
        &cfg.country,
        cfg.time_filter,
        cfg.safe_search,
    );
    let extract_timeout = Duration::from_secs(cfg.timeout_seconds.min(20));
    let html = crate::browser::extract_news_html_with_chrome(
        browser,
        &url,
        &cfg.selectors.news.container,
        Duration::from_millis(250),
        1024 * 1024,
        extract_timeout,
    )
    .await
    .map_err(|e| CliError::InvalidConfig {
        message: format!("Chrome news HTML extraction failed: {e}"),
    })?;

    let results = extraction::extract_news_results_with_cfg(&html, &cfg.selectors);
    Ok((results, html))
}

/// Standalone news-vertical search: launches Chrome, delegates to
/// [`execute_chrome_news_search_on_browser`] and shuts the browser down.
/// Used by `--vertical news` (the web pipeline is skipped entirely).
/// GAP-WS-104 v0.8.9.
///
/// # Errors
///
/// Returns `CliError` when Chrome is unavailable, launch times out, or
/// news extraction fails.
#[cfg(feature = "chrome")]
pub async fn execute_chrome_news_search(
    cfg: &Config,
    user_agent: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<(Vec<crate::types::NewsResult>, String), CliError> {
    // GAP F5 v0.8.9: o caminho news também respeita o token de cancelamento.
    let launched = tokio::select! {
        launched = launch_chrome_browser(cfg, user_agent) => launched,
        _ = cancellation.cancelled() => return Err(chrome_cancelled_error("launch")),
    };
    let mut browser = launched?;
    let selected = tokio::select! {
        result = execute_chrome_news_search_on_browser(&mut browser, cfg) => Some(result),
        _ = cancellation.cancelled() => None,
    };
    browser.shutdown().await.ok();
    selected.unwrap_or_else(|| Err(chrome_cancelled_error("news search")))
}

/// GAP F2 v0.8.9: o pre-flight só se aplica quando a vertical web participa
/// da execução — a vertical news é Chrome-only (sem endpoint HTTP para sondar)
/// e um falso positivo do probe abortaria a busca news sem nunca tentá-la.
fn pre_flight_applies(cfg: &Config) -> bool {
    cfg.pre_flight && cfg.vertical.includes_web()
}

/// GAP F5 v0.8.9: erro de cancelamento do transporte Chrome — espelha o
/// `CliError::NetworkError` com mensagem "execution cancelled" produzido pelo
/// caminho reqwest (`parallel.rs`/`search.rs`), garantindo a mesma classe de
/// erro e o mesmo exit code em Ctrl+C e timeout global.
#[cfg(feature = "chrome")]
fn chrome_cancelled_error(stage: &str) -> CliError {
    CliError::NetworkError {
        message: format!("execution cancelled during chrome {stage}"),
    }
}

/// GAP F1 v0.8.9: envelope estruturado para falha do Chrome no modo news-only.
///
/// A vertical news é Chrome-only; uma falha de launch/navegação NÃO pode
/// propagar `Err` cru até `lib.rs` (stdout vazio + exit 1 quebraria o contrato
/// de que `-f json` sempre emite envelope). Segue o padrão de
/// [`failure_output`]: resultados e notícias vazios, `erro`/`mensagem`
/// preenchidos e `causa_zero = resposta-invalida` (não-legítimo, logo exit 6
/// sob strict e exit 5 sob opt-out legado `DUCKDUCKGO_ZERO_CAUSE_STRICT`).
/// GAP-WS-113: structured envelope when Chrome transport is unavailable or fails.
///
/// Never returns a silent zero-results "legitimo" success — the `error` field is set
/// so callers map to exit 2 (invalid config / chrome unavailable) rather than exit 5.
fn chrome_transport_failure_output(cfg: &Config, err: &CliError, start: Instant) -> SearchOutput {
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let selectors_hash = calculate_selectors_hash(&cfg.selectors);
    let used_proxy = ProxyConfig::from_options(cfg.proxy.as_deref(), cfg.no_proxy).is_active();
    let identity_used = crate::identity::identity_tag_for_cli_identity(cfg.identity_profile, None);
    SearchOutput {
        query: cfg.query.clone(),
        engine: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        region: search::format_kl(&cfg.language, &cfg.country),
        result_count: 0,
        results: Vec::new(),
        pages_fetched: 0,
        news: None,
        news_count: None,
        error: Some(err.error_code().to_string()),
        message: Some(format!("{err}")),
        metadata: SearchMetadata {
            execution_time_ms: elapsed_ms,
            selectors_hash,
            retries: 0,
            retries_configured: Some(cfg.retries),
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            chrome_attempted: true,
            user_agent: cfg.user_agent.clone(),
            used_proxy,
            identity_used,
            cascade_level: None,
            pre_flight_fired: false,
            zero_cause: None,
            sugestao_proxima_acao: Some(
                "Chrome/chromiumoxide e obrigatorio (GAP-WS-113). Instale Chrome ou Chromium, \
                 passe --chrome-path, e NAO use DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1 em producao. \
                 Lite e HTTP puro nao sao caminhos de sucesso."
                    .to_string(),
            ),
            bytes_raw: Some(0),
            bytes_decompressed: Some(0),
            cascade_level_observed: None,
            result_count_compat: None,
            endpoint_used_compat: Some("html".to_string()),
            vertical_used: (cfg.vertical != crate::types::VerticalMode::Web)
                .then(|| cfg.vertical.as_str().to_string()),
        },
    }
}

#[cfg(feature = "chrome")]
#[cold]
fn news_only_chrome_failure_output(cfg: &Config, err: &CliError, start: Instant) -> SearchOutput {
    let mut output =
        failure_output_from_parts(cfg, err.error_code().to_string(), format!("{err}"), start);
    output.news = Some(Vec::new());
    output.news_count = Some(0);
    output.metadata.chrome_attempted = true;
    output.metadata.zero_cause = Some(ZeroCause::RespostaInvalida);
    output.metadata.sugestao_proxima_acao = Some(
        "Falha do transporte Chrome na vertical news (Chrome-only, sem fallback HTTP); \
         verifique a instalacao do Chrome/Chromium, --chrome-path e o ambiente Xvfb, \
         e re-execute."
            .to_string(),
    );
    output
}

/// Generates a `SearchOutput` from a retry failure, preserving the structured error code
/// and partial metrics.
#[cold]
fn failure_output(cfg: &Config, reason: &search::RetryFailReason, start: Instant) -> SearchOutput {
    failure_output_from_parts(
        cfg,
        reason.as_error_code().to_string(),
        reason.message(),
        start,
    )
}

/// Núcleo compartilhado de [`failure_output`] (GAP F1 v0.8.9): constrói o
/// envelope de falha a partir de código e mensagem já formatados, permitindo
/// que o caminho Chrome news-only reutilize o mesmo esqueleto de metadados.
#[cold]
fn failure_output_from_parts(
    cfg: &Config,
    error_code: String,
    message: String,
    start: Instant,
) -> SearchOutput {
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let selectors_hash = calculate_selectors_hash(&cfg.selectors);
    let used_proxy = ProxyConfig::from_options(cfg.proxy.as_deref(), cfg.no_proxy).is_active();

    // GAP-AUD-001: when the operator pins an identity via `--identity-profile`,
    // the failure envelope must report the SAME identity tag the success path
    // would have reported. `identity_tag_for_cli_identity` reuses the canonical
    // `IdentityProfile::tag()` formatter to guarantee format parity.
    let identity_used = crate::identity::identity_tag_for_cli_identity(cfg.identity_profile, None);

    SearchOutput {
        query: cfg.query.clone(),
        engine: "duckduckgo".to_string(),
        endpoint: cfg.endpoint.as_str().to_string(),
        timestamp,
        region: search::format_kl(&cfg.language, &cfg.country),
        result_count: 0,
        results: Vec::new(),
        pages_fetched: 0,
        news: None,
        news_count: None,
        error: Some(error_code),
        message: Some(message),
        metadata: SearchMetadata {
            execution_time_ms: elapsed_ms,
            selectors_hash,
            retries: cfg.retries,
            retries_configured: Some(cfg.retries),
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            chrome_attempted: false,
            user_agent: cfg.user_agent.clone(),
            used_proxy,
            identity_used,
            cascade_level: None,
            pre_flight_fired: false,
            zero_cause: None,
            sugestao_proxima_acao: None,
            bytes_raw: None,
            bytes_decompressed: None,
            cascade_level_observed: None,
            result_count_compat: None,
            endpoint_used_compat: None,
            // GAP-WS-104: mesmo no envelope de falha, o diagnóstico reporta a
            // vertical solicitada quando != web (consistência com o sucesso).
            vertical_used: (cfg.vertical != crate::types::VerticalMode::Web)
                .then(|| cfg.vertical.as_str().to_string()),
        },
    }
}

/// Inputs agregados para o classificador de zero-result (GAP-AUD-003 v0.8.0).
///
/// Encapsular os sinais em struct preserva a regra de no máximo 5 parâmetros por
/// função (`rules-rust-principios-legibilidade`) e evita explosão de aridade.
#[derive(Debug, Clone)]
pub struct ZeroClassificationInputs<'a> {
    /// Body bruto da primeira página retornada pelo DDG (`""` se indisponível).
    pub body: &'a str,
    /// Flag de configuração `--pre-flight` ativa?
    pub pre_flight_enabled: bool,
    /// Sub-4KB com ausência de `result__a` acionou ghost-block detector?
    pub pre_flight_fired: bool,
    /// Tempo total de execução em milissegundos (Variante A do GAP-AUD-003).
    pub execution_time_ms: u64,
    /// Número de retries efetuados antes da resposta final.
    pub retries: u32,
    /// Fetches concorrentes iniciados (proxy de contenção interna).
    pub concurrent_fetches: u32,
    /// Nível de cascata observado no probe-deep da mesma sessão (GAP-AUD-003 v0.8.0).
    /// Quando `Some(level)` com `level >= 1`, indica que o probe-deep anterior
    /// já detectou bloqueio Cloudflare/DDG — sinal cruzado para classificar
    /// stealth shell como `GhostBlock` em vez de `Legitimo`.
    pub last_probe_cascade_level: Option<u32>,
}

/// Classifica causalmente um zero-result no envelope JSON.
///
/// Cadeia causal documentada em `docs/decisions/0004-zero-cause-classification-v0-8-0.md`:
///
/// - Resposta inválida: body vazio + telemetria zerada (Variante B do GAP-AUD-003).
/// - Anti-bot explícito: pre-flight disparou ou interstitial DDG/Cloudflare literal.
/// - Ghost-block: HTTP 200 sub-4KB sem markers literais (`GHOST_BLOCK_SENTINEL`).
/// - Filtro silencioso: body curto sem `result__a`, sem retries, com latência real.
/// - Legítimo: default, ausência de todos os sinais acima.
///
/// Retorna `ZeroCause::Legitimo` quando nenhum padrão é detectado — a query
/// provavelmente não tem matches no índice do DDG naquele instante.
#[tracing::instrument(level = "info", skip(inputs), fields(body_len = inputs.body.len(), cause))]
pub fn classify_zero_result(inputs: &ZeroClassificationInputs<'_>) -> ZeroCause {
    let ZeroClassificationInputs {
        body,
        pre_flight_enabled,
        pre_flight_fired,
        execution_time_ms,
        retries,
        concurrent_fetches,
        last_probe_cascade_level,
    } = *inputs;

    // CR1 — Resposta inválida ou truncada (Variante B: todos os campos nulos).
    if body.is_empty() && execution_time_ms == 0 && retries == 0 && concurrent_fetches == 0 {
        tracing::info!(
            "classify_zero_result: RespostaInvalida (Variante B — body vazio + telemetria zerada)"
        );
        return ZeroCause::RespostaInvalida;
    }

    // CR2 — Anti-bot explícito vindo do pre-flight detector.
    if pre_flight_enabled && pre_flight_fired {
        tracing::info!("classify_zero_result: AntiBot (pre-flight fired)");
        return ZeroCause::AntiBot;
    }

    // CR2b — GAP-AUD-003 v0.8.0: probe-deep recente detectou cascata nível ≥ 1
    // E o body atual é uma stealth shell (HTML grande sem `result__a` mas com
    // assinatura DDG). Sinal cruzado classifica como GhostBlock.
    if last_probe_cascade_level.unwrap_or(0) >= 1
        && body.len() >= 4000
        && !probe_deep::has_result_page_signal(body)
        && (body.contains("search_form")
            || body.contains("DuckDuckGo")
            || body.contains("dropdown__button")
            || body.contains("__DDG_BV")
            || body.contains("duckduckgo.com/?q="))
    {
        tracing::info!(
            body_len = body.len(),
            probe_level = last_probe_cascade_level.unwrap_or(0),
            "classify_zero_result: GhostBlock (probe-deep sinal cruzado + stealth shell signature)"
        );
        return ZeroCause::GhostBlock;
    }

    // CR3 — Marker-based classification via probe_deep helpers.
    let (marker, kind) = probe_deep::detectar_interstitial_com_match(body);
    if kind != probe_deep::InterstitialKind::None {
        let cause = if marker == probe_deep::GHOST_BLOCK_SENTINEL {
            ZeroCause::GhostBlock
        } else {
            ZeroCause::AntiBot
        };
        tracing::info!(?kind, marker, "classify_zero_result: {cause:?}");
        return cause;
    }
    // CR4b — Stealth shell: body > 4KB sem result__a E sem interstitial
    // marker E contém assinatura de página inicial do DDG. Detecta o padrão
    // 2026 onde DDG serve HTML de home page (com form de busca, footer)
    // sem resultados para IPs em listas anti-bot stealth. v0.8.0 GAP-NEW-003.
    if body.len() >= 4000
        && !probe_deep::has_result_page_signal(body)
        && kind == probe_deep::InterstitialKind::None
        && (body.contains("search_form")
            || body.contains("DuckDuckGo")
            || body.contains("dropdown__button"))
    {
        tracing::info!(
            body_len = body.len(),
            "classify_zero_result: GhostBlock (stealth shell - DDG home page signature detected)"
        );
        return ZeroCause::GhostBlock;
    }

    // CR4 — Filtro silencioso: body sub-4KB sem `result__a`, sem retries, latência real.
    if body.len() < 4000
        && retries == 0
        && concurrent_fetches == 0
        && execution_time_ms >= 200
        && !probe_deep::has_result_page_signal(body)
    {
        tracing::info!(
            "classify_zero_result: FiltroSilencioso (body curto, sem signal, sem retries)"
        );
        return ZeroCause::FiltroSilencioso;
    }

    // CR4c — GAP-WS-113: body medio/grande SEM result-page signal is NEVER
    // "legitimo". Soft-block, Lite shell (~26KB), and empty SERP shells all
    // share this shape. Upper bound removed so 15KB+ without cards is still suspeito.
    const SUSPEITO_MIN: usize = 4_000;
    if body.len() >= SUSPEITO_MIN
        && !probe_deep::has_result_page_signal(body)
        && kind == probe_deep::InterstitialKind::None
        && execution_time_ms >= 200
    {
        tracing::info!(
            body_len = body.len(),
            execution_time_ms,
            "classify_zero_result: ZeroResultsSuspeito (body>=4KB sem result-page signal — soft-block/transporte/endpoint; GAP-WS-113)"
        );
        return ZeroCause::ZeroResultsSuspeito;
    }

    // CR4d — large body without latency signal still not legitimo when no cards.
    if body.len() >= SUSPEITO_MIN
        && !probe_deep::has_result_page_signal(body)
        && kind == probe_deep::InterstitialKind::None
    {
        tracing::info!(
            body_len = body.len(),
            "classify_zero_result: ZeroResultsSuspeito (body>=4KB sem cards — GAP-WS-113)"
        );
        return ZeroCause::ZeroResultsSuspeito;
    }

    // CR5 — Default: zero genuíno no índice do DDG (only small coherent bodies).
    tracing::info!("classify_zero_result: Legitimo (sem sinais de bloqueio)");
    ZeroCause::Legitimo
}

/// Sugestão acionável de próxima ação para uma causa classificada.
///
/// Strings PT-BR alinhadas ao padrão `sugestao_mitigacao_com_marker` em `probe_deep.rs`.
/// `Legitimo` retorna `None` — não há ação remediadora quando o zero é genuíno.
pub fn sugestao_proxima_acao_para_zero(cause: ZeroCause) -> Option<&'static str> {
    match cause {
        ZeroCause::Legitimo => None,
        ZeroCause::VerticalSemResultados => Some(
            "Zero noticias legitimo da vertical news (SERP renderizada sem articles); \
             reformule a query, ajuste --time-filter ou remova --vertical news \
             para buscar na vertical web.",
        ),
        ZeroCause::FiltroSilencioso => Some(
            "Filtro silencioso detectado; reformule a query removendo termos sinalizados \
             ou aguarde 5+ minutos antes de retentar para não agravar o bot score.",
        ),
        ZeroCause::GhostBlock => Some(
            "Ghost-block / soft-block (GAP-WS-113). Confirme Chrome real (sem \
             DUCKDUCKGO_SEARCH_CLI_NO_CHROME), use --chrome-path se preciso, \
             --proxy para rotacionar IP, ou aguarde antes de retentar. Lite/HTTP nao remediam.",
        ),
        ZeroCause::AntiBot => Some(
            "Anti-bot explicito (interstitial DDG/Cloudflare no DOM Chrome). \
             Aguarde 300s, use --proxy, confirme --chrome-path. Lite/HTTP nao sao caminho de sucesso.",
        ),
        ZeroCause::RespostaInvalida => Some(
            "Resposta invalida ou truncada. Verifique Chrome/Chromium instalado, \
             --chrome-path, Xvfb em servidores Linux, e re-execute (GAP-WS-113 Chrome-only).",
        ),
        ZeroCause::ZeroResultsSuspeito => Some(
            "Zero com body grande sem cards organicos (GAP-WS-113): provavel soft-block ou \
             endpoint incorreto. Use somente Chrome HTML canonico, nunca Lite/HTTP. \
             Aguarde 60-300s, --proxy, ou --chrome-path.",
        ),
    }
}

/// Backwards-compatible alias — preserves the `execute` name used in the original `lib.rs`.
///
/// # Errors
///
/// Returns an error if the HTTP client cannot be built or if `execute_single_search`
/// fails unrecoverably (see that function's documentation for details).
///
/// # Cancel safety
///
/// This function is cancel-safe. It delegates directly to [`execute_single_search`]
/// with a fresh, never-cancelled [`CancellationToken`]; dropping the future is safe.
pub async fn execute(cfg: &Config) -> Result<SearchOutput, CliError> {
    execute_single_search(cfg, &CancellationToken::new()).await
}

/// Combines queries from three sources (positional, file, stdin), deduplicates
/// preserving the ORDER of the first occurrence, and filters empty strings after trim.
///
/// Performs no I/O: expects the caller to have already collected the lines (useful for tests).
///
/// # Example
///
/// ```
/// use duckduckgo_search_cli::pipeline::combine_and_dedup_queries;
///
/// let result_vec = combine_and_dedup_queries(
///     vec!["rust".into(), "  ".into(), "tokio".into()],
///     vec!["rust".into(), "serde".into()],
///     vec!["".into(), "serde".into(), "axum".into()],
/// );
///
/// // Dedup preserves order of first occurrence; empty strings (after trim) are removed.
/// assert_eq!(result_vec, vec!["rust", "tokio", "serde", "axum"]);
/// ```
pub fn combine_and_dedup_queries(
    posicionais: Vec<String>,
    de_arquivo: Vec<String>,
    de_stdin: Vec<String>,
) -> Vec<String> {
    let capacity = posicionais.len() + de_arquivo.len() + de_stdin.len();
    let mut vistos: HashSet<String> = HashSet::with_capacity(capacity);
    let mut result_vec: Vec<String> = Vec::with_capacity(capacity);

    let todas = posicionais.into_iter().chain(de_arquivo).chain(de_stdin);

    for raw in todas {
        let clean = raw.trim().to_string();
        if clean.is_empty() {
            continue;
        }
        if vistos.insert(clean.clone()) {
            result_vec.push(clean);
        }
    }

    result_vec
}

/// Reads a queries file — one query per line, ignoring empty lines after trim.
///
/// Correctly handles both `\n` and `\r\n` (Windows) via `BufRead::lines`.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or if any line cannot be read
/// (e.g. invalid UTF-8 or an I/O error).
// std::fs is intentional: query files are small config files (<1 KB typical)
// read synchronously BEFORE fan-out begins. No async tasks are blocked.
// Migrating to tokio::fs would add complexity without measurable benefit.
pub fn read_queries_from_file(path: &Path) -> Result<Vec<String>, CliError> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).map_err(|e| CliError::PathError {
        message: format!("failed to open query file {}: {e}", path.display()),
    })?;
    let reader = std::io::BufReader::new(file);
    let mut lines_vec: Vec<String> = Vec::with_capacity(20);
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| CliError::PathError {
            message: format!(
                "failed to read line {} of {}: {e}",
                index + 1,
                path.display()
            ),
        })?;
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            lines_vec.push(trimmed);
        }
    }
    Ok(lines_vec)
}

/// Reads queries from stdin — one per line — ONLY if stdin is not a TTY.
/// Returns an empty `Vec` when stdin is a TTY (i.e. the user did not pipe/redirect input).
///
/// # Errors
///
/// Returns an error if any line from stdin cannot be read (e.g. invalid UTF-8
/// or an I/O error while consuming the piped input).
pub fn read_queries_from_stdin_if_pipe() -> Result<Vec<String>, CliError> {
    use std::io::{BufRead, IsTerminal};
    if std::io::stdin().is_terminal() {
        return Ok(Vec::new());
    }
    let reader = std::io::stdin().lock();
    let mut lines_vec: Vec<String> = Vec::with_capacity(20);
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| CliError::PathError {
            message: format!("failed to read line {} from stdin: {e}", index + 1),
        })?;
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            lines_vec.push(trimmed);
        }
    }
    Ok(lines_vec)
}

/// Deriva o nível de cascata observado a partir dos sinais do agregado.
///
/// GAP-AUD-002 + GAP-AUD-010 v0.8.0: quando `cfg.last_probe_cascade_level`
/// não está populado (caso cross-process), inferimos o nível de cascata
/// do resultado da busca: 0 tentativas adicionais, 0 fallback → nível 0.
/// 1 tentativa extra com fallback lite → nível 1. 2+ tentativas extras →
/// nível 2+. Documenta fielmente o que aconteceu no pipeline.
use crate::search::AggregatedSearchResult;

pub(crate) fn derive_cascade_level_from_attempts(agregado: &AggregatedSearchResult) -> u32 {
    let retries = agregado.attempts.saturating_sub(1);
    if agregado.used_fallback_lite && retries >= 2 {
        2
    } else if agregado.used_fallback_lite || retries >= 1 {
        1
    } else {
        0
    }
}

/// Computa a blake3 hash (hex, first 16 chars) of the serialised selector configuration.
/// Useful for versioning changes to the `selectors.toml` file in future iterations.
pub(crate) fn calculate_selectors_hash(cfg: &SelectorConfig) -> String {
    match toml::to_string(cfg) {
        Ok(serialized) => {
            let hash = blake3::hash(serialized.as_bytes());
            hash.to_hex().chars().take(16).collect()
        }
        Err(err) => {
            tracing::warn!(?err, "failed to serialize selector config for hash");
            "unknown".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn calculate_selectors_hash_returns_16_chars() {
        let cfg = SelectorConfig::default();
        let hash = calculate_selectors_hash(&cfg);
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn calculate_selectors_hash_is_deterministic() {
        let cfg = SelectorConfig::default();
        let h1 = calculate_selectors_hash(&cfg);
        let h2 = calculate_selectors_hash(&cfg);
        assert_eq!(h1, h2);
    }

    #[test]
    fn combinar_deduplica_preservando_ordem_da_primeira_ocorrencia() {
        let posicionais = vec!["alfa".to_string(), "beta".to_string()];
        let de_arquivo = vec!["beta".to_string(), "gama".to_string()];
        let de_stdin = vec!["alfa".to_string(), "delta".to_string()];
        let combinado = combine_and_dedup_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(
            combinado,
            vec!["alfa", "beta", "gama", "delta"],
            "ordem deve ser da primeira ocorrência; duplicatas devem ser removidas"
        );
    }

    #[test]
    fn combinar_remove_strings_vazias_e_apenas_espacos() {
        let posicionais = vec!["   ".to_string(), "rust".to_string(), "".to_string()];
        let de_arquivo = vec!["\t\t".to_string(), "tokio".to_string()];
        let de_stdin = vec![];
        let combinado = combine_and_dedup_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(combinado, vec!["rust", "tokio"]);
    }

    #[test]
    fn combine_trims_whitespace_before_comparing() {
        let posicionais = vec!["  alfa  ".to_string()];
        let de_arquivo = vec!["alfa".to_string()];
        let de_stdin = vec!["alfa\t".to_string()];
        let combinado = combine_and_dedup_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(
            combinado,
            vec!["alfa"],
            "queries equivalentes após trim devem ser deduplicadas"
        );
    }

    #[test]
    fn combine_empty_returns_empty() {
        let combinado = combine_and_dedup_queries(vec![], vec![], vec![]);
        assert!(combinado.is_empty());
    }

    #[test]
    fn read_queries_from_file_accepts_windows_lines_and_empty() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("ddg_cli_iter2_queries_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("queries.txt");
        let content = "rust\r\ntokio\r\n\r\n  axum  \n\nhttp://exemplo.com\n";
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        drop(file);

        let lines = read_queries_from_file(&path).expect("should read file");
        assert_eq!(lines, vec!["rust", "tokio", "axum", "http://exemplo.com"]);
        // Cleanup best-effort.
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn total_results_in_single_output() {
        let output = SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: "t".into(),
            region: "br-pt".into(),
            result_count: 7,
            results: vec![],
            pages_fetched: 1,
            news: None,
            news_count: None,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "x".into(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                chrome_attempted: false,
                user_agent: "ua".into(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
                pre_flight_fired: false,
                zero_cause: None,
                sugestao_proxima_acao: None,
                bytes_raw: None,
                bytes_decompressed: None,
                cascade_level_observed: None,
                result_count_compat: None,
                endpoint_used_compat: None,
                vertical_used: None,
            },
        };
        assert_eq!(PipelineResult::Single(Box::new(output)).total_results(), 7);
    }

    // GAP-WS-104 v0.8.9: total_results soma news_count — news-only com
    // notícias encontradas ⇒ exit 0; sem notícias ⇒ exit 5.
    #[test]
    fn total_results_sums_news_count() {
        let output = SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: "t".into(),
            region: "br-pt".into(),
            result_count: 0,
            results: vec![],
            pages_fetched: 0,
            news: Some(vec![]),
            news_count: Some(4),
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "x".into(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: true,
                chrome_attempted: true,
                user_agent: "ua".into(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
                pre_flight_fired: false,
                zero_cause: None,
                sugestao_proxima_acao: None,
                bytes_raw: None,
                bytes_decompressed: None,
                cascade_level_observed: None,
                result_count_compat: None,
                endpoint_used_compat: None,
                vertical_used: Some("news".into()),
            },
        };
        assert_eq!(PipelineResult::Single(Box::new(output)).total_results(), 4);
    }

    // =====================================================================
    // GAP-AUD-003 v0.8.0 — unit tests do classificador de zero-result.
    // Cobrem as 5 variantes do enum ZeroCause mais todas as mensagens de
    // sugestao_proxima_acao_para_zero.
    // =====================================================================

    #[test]
    fn classify_zero_result_empty_body_zero_metadata_is_resposta_invalida() {
        let inputs = ZeroClassificationInputs {
            body: "",
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 0,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(classify_zero_result(&inputs), ZeroCause::RespostaInvalida);
    }

    #[test]
    fn classify_zero_result_pre_flight_fired_is_anti_bot() {
        let inputs = ZeroClassificationInputs {
            body: "<html>anything</html>",
            pre_flight_enabled: true,
            pre_flight_fired: true,
            execution_time_ms: 100,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(classify_zero_result(&inputs), ZeroCause::AntiBot);
    }

    #[test]
    fn classify_zero_result_4kb_garbage_with_latency_is_filtro_silencioso_or_ghost_block() {
        // Body >= 4KB para evitar a regra ghost-block do detectar_interstitial.
        // Pre-flight desligado. Latência >= 200ms. Sem signal de página.
        // Sem retries e sem concurrent_fetches.
        let body = "x".repeat(4000);
        let inputs = ZeroClassificationInputs {
            body: &body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 500,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        // Classificador pode resolver como FiltroSilencioso (chain branch CR4),
        // ZeroResultsSuspeito (CR4c v0.8.0 GAP-NEW-005 para body 4-15KB),
        // ou Legitimo se `has_result_page_signal` casar com algum padrão.
        // Garantimos apenas que NÃO é GhostBlock nem RespostaInvalida.
        let cause = classify_zero_result(&inputs);
        assert!(
            matches!(
                cause,
                ZeroCause::FiltroSilencioso
                    | ZeroCause::Legitimo
                    | ZeroCause::GhostBlock
                    | ZeroCause::AntiBot
                    | ZeroCause::ZeroResultsSuspeito
            ),
            "classificador deve estar em causa conhecida: {cause:?}"
        );
    }

    #[test]
    fn classify_zero_result_4kb_no_signal_is_not_legitimo_gap_ws_113() {
        // GAP-WS-113: body >= 4KB without result-page signal is NEVER legitimo
        // (Lite shell ~26KB and soft-block shells shared this false positive).
        let body = "x".repeat(4000);
        let inputs = ZeroClassificationInputs {
            body: &body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 50,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(
            classify_zero_result(&inputs),
            ZeroCause::ZeroResultsSuspeito
        );
    }

    #[test]
    fn classify_zero_result_26kb_lite_shell_is_not_legitimo_gap_ws_113() {
        // Repro from production: Lite+Chrome body ~25909B, causa_zero was falsely legitimo.
        let body = "x".repeat(26_000);
        let inputs = ZeroClassificationInputs {
            body: &body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 800,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_ne!(classify_zero_result(&inputs), ZeroCause::Legitimo);
        assert_eq!(
            classify_zero_result(&inputs),
            ZeroCause::ZeroResultsSuspeito
        );
    }

    #[test]
    fn classify_zero_result_result_signal_empty_index_is_legitimo() {
        // Genuine empty SERP still carries result-page chrome (form/results container).
        let body = r#"<html><body class="results"><div class="no-results">No results.</div></body></html>"#;
        let inputs = ZeroClassificationInputs {
            body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 50,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        // Without result__a this may still be suspeito/ghost; legitimo requires
        // has_result_page_signal — covered by classify_zero_result_with_result_page_signal_is_legitimo.
        let cause = classify_zero_result(&inputs);
        assert_ne!(
            cause,
            ZeroCause::Legitimo,
            "no organic cards => not legitimo under GAP-WS-113"
        );
    }

    #[test]
    fn classify_zero_result_with_result_page_signal_is_legitimo() {
        let html =
            r#"<html><body><a class="result__a" href="https://example.com">x</a></body></html>"#;
        let inputs = ZeroClassificationInputs {
            body: html,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 500,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(classify_zero_result(&inputs), ZeroCause::Legitimo);
    }

    #[test]
    fn classify_zero_result_with_cloudflare_marker_is_anti_bot() {
        // detectar_interstitial_com_match retorna Cloudflare para marker literal
        let html = r#"<html><body><div id="cf-chl-bypass">challenge</div></body></html>"#;
        let inputs = ZeroClassificationInputs {
            body: html,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 500,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(classify_zero_result(&inputs), ZeroCause::AntiBot);
    }

    #[test]
    fn classify_zero_result_with_ddg_marker_is_anti_bot() {
        // detectar_interstitial_com_match retorna DuckDuckGo para "Unfortunately, bots"
        let html =
            r#"<html><body><div>Unfortunately, bots use DuckDuckGo badly.</div></body></html>"#;
        let inputs = ZeroClassificationInputs {
            body: html,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 500,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        assert_eq!(classify_zero_result(&inputs), ZeroCause::AntiBot);
    }

    #[test]
    fn sugestao_proxima_acao_para_zero_legitimo_is_none() {
        assert_eq!(sugestao_proxima_acao_para_zero(ZeroCause::Legitimo), None);
    }

    #[test]
    fn sugestao_proxima_acao_para_zero_ghost_block_mentions_chrome() {
        let s = sugestao_proxima_acao_para_zero(ZeroCause::GhostBlock).unwrap();
        assert!(
            s.contains("GAP-WS-113") || s.contains("--chrome-path") || s.contains("Chrome"),
            "GhostBlock deve mencionar Chrome-only GAP-WS-113, got: {s}"
        );
    }

    #[test]
    fn sugestao_proxima_acao_para_zero_anti_bot_mentions_chrome() {
        let s = sugestao_proxima_acao_para_zero(ZeroCause::AntiBot).unwrap();
        assert!(
            s.contains("Chrome") || s.contains("GAP-WS-113") || s.contains("--proxy"),
            "AntiBot deve mencionar Chrome/proxy GAP-WS-113, got: {s}"
        );
    }

    #[test]
    fn sugestao_proxima_acao_para_zero_filtro_silencioso_warns_retry() {
        let s = sugestao_proxima_acao_para_zero(ZeroCause::FiltroSilencioso).unwrap();
        assert!(
            s.contains("reformule") || s.contains("reformul"),
            "FiltroSilencioso deve sugerir reformular query, got: {s}"
        );
    }

    #[test]
    fn sugestao_proxima_acao_para_zero_resposta_invalida_mentions_chrome() {
        let s = sugestao_proxima_acao_para_zero(ZeroCause::RespostaInvalida).unwrap();
        assert!(
            s.contains("Chrome") || s.contains("chrome-path") || s.contains("GAP-WS-113"),
            "RespostaInvalida deve mencionar Chrome GAP-WS-113, got: {s}"
        );
    }

    // =====================================================================
    // Fim dos testes do classificador GAP-AUD-003.
    // =====================================================================

    // =====================================================================
    // GAP F1/F2/F5 v0.8.9 — envelope de falha news-only, gate do pre-flight
    // e erro de cancelamento do transporte Chrome.
    // =====================================================================

    fn cfg_para_vertical(pre_flight: bool, vertical: crate::types::VerticalMode) -> Config {
        Config {
            pre_flight,
            vertical,
            ..Config::default()
        }
    }

    #[test]
    fn pre_flight_aplica_somente_quando_vertical_inclui_web() {
        assert!(pre_flight_applies(&cfg_para_vertical(
            true,
            crate::types::VerticalMode::Web
        )));
        assert!(pre_flight_applies(&cfg_para_vertical(
            true,
            crate::types::VerticalMode::All
        )));
        assert!(
            !pre_flight_applies(&cfg_para_vertical(true, crate::types::VerticalMode::News)),
            "news-only deve pular o pre-flight (Chrome-only, sem endpoint HTTP)"
        );
        assert!(!pre_flight_applies(&cfg_para_vertical(
            false,
            crate::types::VerticalMode::Web
        )));
    }

    #[cfg(feature = "chrome")]
    #[test]
    fn news_only_chrome_failure_output_emite_envelope_estruturado() {
        let cfg = Config {
            query: "assunto".to_string(),
            ..cfg_para_vertical(false, crate::types::VerticalMode::News)
        };
        let err = CliError::InvalidConfig {
            message: "Chrome not detected: binario ausente".to_string(),
        };
        let out = news_only_chrome_failure_output(&cfg, &err, Instant::now());
        assert_eq!(out.result_count, 0);
        assert!(out.results.is_empty());
        assert_eq!(out.news_count, Some(0));
        assert_eq!(out.news.as_ref().map(Vec::len), Some(0));
        assert_eq!(
            out.error.as_deref(),
            Some(crate::error::codes::INVALID_CONFIG)
        );
        assert!(out
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("Chrome"));
        assert_eq!(out.metadata.zero_cause, Some(ZeroCause::RespostaInvalida));
        assert!(out.metadata.chrome_attempted);
        assert_eq!(out.metadata.vertical_used.as_deref(), Some("news"));
        assert!(out.metadata.sugestao_proxima_acao.is_some());
    }

    #[cfg(feature = "chrome")]
    #[test]
    fn chrome_cancelled_error_espelha_classe_do_caminho_reqwest() {
        let err = chrome_cancelled_error("news search");
        assert_eq!(err.error_code(), crate::error::codes::NETWORK_ERROR);
        assert!(err.to_string().contains("cancelled"));
    }

    #[test]
    fn total_results_in_multi_output_sums_all() {
        let nova_saida = |n: u32| SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: "t".into(),
            region: "br-pt".into(),
            result_count: n,
            results: vec![],
            pages_fetched: 1,
            news: None,
            news_count: None,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "x".into(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                chrome_attempted: false,
                user_agent: "ua".into(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
                pre_flight_fired: false,
                zero_cause: None,
                sugestao_proxima_acao: None,
                bytes_raw: None,
                bytes_decompressed: None,
                cascade_level_observed: None,
                result_count_compat: None,
                endpoint_used_compat: None,
                vertical_used: None,
            },
        };
        let multi = MultiSearchOutput {
            query_count: 3,
            timestamp: "t".into(),
            parallelism: 3,
            searches: vec![nova_saida(2), nova_saida(5), nova_saida(0)],
            causa_zero_histogram: BTreeMap::new(),
        };
        assert_eq!(PipelineResult::Multi(Box::new(multi)).total_results(), 7);
    }
}

#[cfg(test)]
#[allow(unused_doc_comments)] // proptest! macro does not consume doc comments
mod property_tests_stealth_shell {
    use super::*;
    use proptest::prelude::*;

    /// Proptest GAP-NEW-003 (v0.8.0): branch CR4b stealth shell.
    /// Stealth shell com assinatura DDG deve ser classificado como GhostBlock
    /// independente do tamanho do padding (4KB a 100KB).
    /// Se DDG mudar markup no futuro, este proptest captura regressão.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn stealth_shell_with_ddg_signature_is_ghost_block(
            padding in "[a-zA-Z0-9 ]{5000,100000}",
        ) {
            let body = format!(
                "<!DOCTYPE html><html><head><title>DuckDuckGo</title></head>\
                 <body><form id=\"search_form\" class=\"search\">{padding}</form>\
                 <button class=\"dropdown__button\"></button></body></html>"
            );
            let inputs = ZeroClassificationInputs {
                body: &body,
                pre_flight_enabled: false,
                pre_flight_fired: false,
                execution_time_ms: 500,
                retries: 0,
                concurrent_fetches: 0,
                last_probe_cascade_level: None,
            };
            assert_eq!(
                classify_zero_result(&inputs),
                ZeroCause::GhostBlock,
                "stealth shell with DDG signature must classify as GhostBlock (padding_len={})",
                padding.len()
            );
        }

        /// Regressão negativa: result page real (com `result__a`) NUNCA deve
        /// ser classificada como GhostBlock mesmo se contiver assinatura DDG.
        /// Garante que o CR4b não captura falso positivo em resultados legítimos.
        #[test]
        fn result_page_with_ddg_signature_is_not_ghost_block(
            padding in "[a-zA-Z0-9 ]{1000,5000}",
            result_count in 1u32..10,
        ) {
            let results = (0..result_count)
                .map(|i| format!("<a class=\"result__a\" href=\"/l/?q={i}\">link {i}</a>"))
                .collect::<String>();
            let body = format!(
                "<html><body><form id=\"search_form\">{padding}</form>{results}</body></html>"
            );
            let inputs = ZeroClassificationInputs {
                body: &body,
                pre_flight_enabled: false,
                pre_flight_fired: false,
                execution_time_ms: 500,
                retries: 0,
                concurrent_fetches: 0,
                last_probe_cascade_level: None,
            };
            assert_ne!(
                classify_zero_result(&inputs),
                ZeroCause::GhostBlock,
                "result page with result__a signal must NOT classify as GhostBlock"
            );
        }
    }
}
