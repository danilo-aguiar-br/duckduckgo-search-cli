// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound orchestrator (single-query SERP path)
//! Single-query search execution (SRP split from `pipeline`).

use crate::content_fetch;
use crate::error::CliError;
use crate::http;
use crate::probe_deep;
use crate::search;
use crate::types::{Config, SearchMetadata, SearchOutput};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use super::failure::{
    chrome_transport_failure_output, failure_output, news_only_chrome_failure_output,
};
use super::{
    calculate_selectors_hash, classify_zero_result, derive_cascade_level_from_attempts, do_warmup,
    fill_chrome_agent_metadata, next_action_suggestion_for_zero, pre_flight_applies,
    ZeroClassificationInputs,
};

#[cfg(feature = "chrome")]
use super::{
    execute_chrome_all_search_pub, execute_chrome_search,
};

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

    let config_proxy = cfg.proxy_config.clone();

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
    // GAP F4 v0.8.9: reassignments of `effective_identity_tag` happen
    // only under `#[cfg(feature = "chrome")]` — same pattern as `chrome_result`.
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
            // Move the profile; clone only the UA string needed as a parallel field.
            let ua = pinned.user_agent.clone();
            (pinned, ua, Some(tag))
        }
        None => (cfg.browser_profile.clone(), cfg.user_agent.as_str().to_string(), None),
    };

    // GAP-TLS-014: residual reqwest Client only for http-test-harness.
    // Production Chrome-only path skips TLS pool / cookie jar construction.
    let residual_client = http::maybe_build_residual_client(
        &effective_profile,
        cfg.timeout_seconds.get(),
        cfg.language.as_str(),
        cfg.country.as_str(),
        &config_proxy,
        cfg.cookie_provider.clone(),
    )?;

    // GAP-WS-113: HTTP warm-up is residual harness-only. Production warm-up is
    // Chrome CDP (GAP-WS-077 inside browser launch / extract paths).
    if cfg.warmup_enabled {
        if let Some(ref client) = residual_client {
            if let Err(e) = do_warmup(client, cfg).await {
                tracing::warn!(error = %e, "warm-up request failed; continuing without it");
            }
        }
    }

    // v0.7.10 P5: probe-deep scheduler — when `cfg.pre_flight == true`,
    // run a minimal probe before the real search and short-circuit on
    // captcha/ghost-block so the operator does not waste a full
    // search round-trip on an already-blocked environment.
    // GAP F2 v0.8.9: o pre-flight sonda o endpoint HTML web via reqwest — um
    // sinal irrelevante (e potencialmente falso-positivo fatal) para a vertical
    // news, which is Chrome-only without HTTP fallback. A false positive would abort
    // the news search with exit 3 without ever trying it. The probe runs only when
    // execution includes the web vertical (`web` and `all`).
    if cfg.pre_flight && !pre_flight_applies(cfg) {
        tracing::info!(
            vertical = cfg.vertical.as_str(),
            "pre-flight nao se aplica a vertical news (Chrome-only, sem endpoint HTTP); probe pulado"
        );
    }
    // GAP-WS-113: production pre-flight runs on the SHARED Chrome SERP session
    // inside `execute_chrome_web_search_on_browser` (one launch per invocation).
    // Residual HTTP pre-flight remains harness-only below.
    if pre_flight_applies(cfg) {
        if let Some(ref client) = residual_client {
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
                    let mut pre = SearchOutput {
                        query: cfg.query.as_str().to_string(),
                        engine: "duckduckgo".to_string(),
                        endpoint: cfg.endpoint.as_str().to_string(),
                        timestamp: crate::types::utc_now(),
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
                            pre_flight_executed: true,
                            pre_flight_status: Some("blocked".into()),
                            news_promo_filtered: None,
                            stream_requested: None,
                            stream_effective: None,
                            zero_cause: None,
                            next_action_suggestion: None,
                            bytes_raw: None,
                            bytes_decompressed: None,
                            cascade_level_observed: None,
                            result_count_compat: None,
                            endpoint_used_compat: None,
                            vertical_used: Some(cfg.vertical.as_str().to_string()),
                            chrome_path_resolved: None,
                            chrome_channel: None,
                            run_id: Some(crate::types::RunId::generate()),
                            flags_ignored: None,
                        },
                    };
                    fill_chrome_agent_metadata(&mut pre.metadata, cfg);
                    return Ok(pre);
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "pre-flight request failed; continuing with real search");
            }
        }
        } // residual_client
    } // pre_flight_applies

    tracing::info!(query = %cfg.query, endpoint = cfg.endpoint.as_str(), "Executing search");

    // v0.8.0 / ADR-0016: Chrome-only production transport — native browser TLS stack
    // (avoids library TLS bot-class signatures blocked by Cloudflare; ADR-0022:
    // no synthetic hardware fingerprint spoof). Residual HTTP is harness-only.
    #[allow(unused_assignments, unused_mut)]
    let mut chrome_attempted = false;
    #[allow(unused_assignments, unused_mut)]
    let mut chrome_result_used = false;
    #[allow(unused_mut)]
    let mut chrome_result: Option<search::AggregatedSearchResult> = None;

    // GAP-WS-104 v0.8.9: resultado da vertical news (resultados + body bruto
    // rendered, consumed by zero-cause classification). `None` in
    // default web mode — the JSON contract remains byte-identical pre-v0.8.9.
    #[cfg(feature = "chrome")]
    let mut news_outcome: Option<(Vec<crate::types::NewsResult>, String, u32)> = None;
    // CM-09: when dual news fails mid-flight, keep news=None (unavailable) and
    // stash a short diagnostic for the agent envelope (not silent Some([])).
    #[cfg(feature = "chrome")]
    let mut news_vertical_fail: Option<String> = None;

    // GAP-WS-113: production always requires Chrome. Harness may skip Chrome.
    if let Err(err) = crate::chrome_policy::require_chrome_transport() {
        if !crate::chrome_policy::http_test_harness_active() {
            return Ok(chrome_transport_failure_output(cfg, &err, start));
        }
    }

    // GAP-E2E-V14-UA-METADATA-LIE: emit_user_agent tracks the Chrome SSOT when
    // chromiumoxide runs so SearchMetadata.user_agent ≡ launch UA (anti-CF #4).
    #[cfg_attr(not(feature = "chrome"), allow(unused_mut))]
    let mut emit_user_agent = effective_user_agent.clone();

    #[cfg(feature = "chrome")]
    if !crate::chrome_policy::chrome_disabled_by_env()
        && !crate::chrome_policy::http_test_harness_active()
    {
        chrome_attempted = true;
        let candidate =
            crate::identity::browser_profile_for_cli_identity(cfg.identity_profile, None)
                .map(|p| p.user_agent) // owned profile — move the UA field
                .unwrap_or_else(|| effective_user_agent.clone());
        // Detect host major once (process-wide cache in detect.rs) so rewrite
        // happens before launch and metadata can report the same string session uses.
        let host_major = match crate::browser::detect_chrome_resolved(cfg.chrome_path.as_deref()) {
            Ok(resolved) => crate::browser::detect_chrome_major_version_async(&resolved.path)
                .await
                .ok()
                .flatten(),
            Err(_) => None,
        };
        let chrome_identity =
            crate::identity::resolve_effective_chrome_identity(&candidate, host_major);
        let chrome_ua = chrome_identity.user_agent.clone();
        // Always bind emit + tag to SSOT when Chrome path is taken (even on later fail
        // the attempted identity is the honest one for agents).
        emit_user_agent = chrome_identity.user_agent.clone();
        effective_identity_tag = Some(chrome_identity.tag);

        if cfg.vertical.includes_news() {
            // GAP-WS-104 / GAP-PAR-021: `--vertical news|all` via
            // `execute_chrome_all_search_pub` — dual multi-process Chromes
            // (web ∥ news) when budget ≥ 2; shared serial session when
            // `--shared-session-verticals` or `-p 1`. Multi-query pays
            // acquire_many(2) so peak Chrome OS ≤ effective (GAP-PAR-021b).
            // News is Chrome-only (`ia=news&iar=news`); cancel → exit 130/143.
            let outcome = execute_chrome_all_search_pub(cfg, &chrome_ua, cancellation).await?;
            // Chrome session was used for news and/or web (L-04 honest used_chrome).
            chrome_result_used = true;
            if let Some(result) = outcome.web {
                chrome_result = Some(result);
            }
            match outcome.news {
                Ok(outcome_news) => news_outcome = Some(outcome_news),
                Err(err) => {
                    // Cooperative cancel must surface as exit 130/143, not as a
                    // structured zero-results envelope.
                    if matches!(err, CliError::Cancelled) {
                        return Err(err);
                    }
                    if !cfg.vertical.includes_web() {
                        // GAP F1 v0.8.9: news-only never propagates raw Err —
                        // `-f json` always emits a structured JSON envelope.
                        return Ok(news_only_chrome_failure_output(cfg, &err, start));
                    }
                    // CM-09: do NOT collapse Err → Some([]) (that fakes legitimate zero).
                    news_outcome = None;
                    let short = err.error_code().to_string();
                    news_vertical_fail = Some(short);
                    tracing::warn!(
                        error = %err,
                        "Chrome dual news vertical failed — marking news unavailable (CM-09)"
                    );
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
                }
                Err(err) => {
                    // Cooperative cancel → Err(Cancelled) → exit 130/143.
                    if matches!(err, CliError::Cancelled) {
                        return Err(err);
                    }
                    // GAP-WS-113: never fall back to reqwest — structured failure.
                    // Pre-flight on shared session returns Blocked → pre_flight envelope.
                    if matches!(err, CliError::Blocked) {
                        tracing::warn!("Chrome shared-session pre-flight blocked (GAP-WS-113)");
                        let mut out = chrome_transport_failure_output(cfg, &err, start);
                        out.error = Some("pre_flight_blocked".to_string());
                        out.metadata.pre_flight_fired = true;
                        out.metadata.pre_flight_executed = true;
                        out.metadata.pre_flight_status = Some("blocked".into());
                        out.metadata.used_chrome = true;
                        out.metadata.chrome_attempted = true;
                        out.metadata.next_action_suggestion = Some(
                            "Pre-flight Chrome detected a block (GAP-WS-113). Wait 300s or use --proxy."
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
    } else if let Some(ref client) = residual_client {
        // Residual HTTP path for wiremock tests only (feature http-test-harness).
        let flag_rate_limit = Arc::new(AtomicBool::new(false));
        let search_result = search::search_with_pagination(
            client,
            cfg,
            cfg.query.as_str(),
            &flag_rate_limit,
            cancellation,
        )
        .await;
        let failure_output_val = match &search_result {
            Err(reason) if reason.is_cancellation() => {
                // HTTP harness cancel → typed Cancelled → exit 130/143.
                return Err(CliError::Cancelled);
            }
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
        // GAP-WS-113 / V17: Chrome did not produce a web result and harness is off.
        // Message stays neutral — next_action_suggestion taxonomy (failure.rs)
        // distinguishes missing binary vs session/Xvfb/warm-up/proxy remediation.
        let err = CliError::InvalidConfig {
            message: "Chrome transport did not return SERP results (GAP-WS-113)."
                .into(),
        };
        return Ok(chrome_transport_failure_output(cfg, &err, start));
    };

    // GAP-WS-090: truncate results to --num when Chrome headed returns a full
    // page (typically 10). Without this, --num is silently ignored.
    if let Some(max) = cfg.num_results.map(|n| n.get()) {
        let max = max as usize;
        if agregado.results.len() > max {
            agregado.results.truncate(max);
        }
    }

    let quantidade = u32::try_from(agregado.results.len()).unwrap_or(u32::MAX);
    let selectors_hash = calculate_selectors_hash(&cfg.selectors);
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = crate::types::utc_now();
    let run_id = crate::types::RunId::generate();
    // Retries = attempts - 1 (the first request does not count as a retry).
    let retries_count = agregado.attempts.saturating_sub(1);

    // GAP-AUD-002 + GAP-AUD-010 v0.8.0: cascade_level_observed deve refletir
    // the cascade level actually exercised. We prefer the cache from
    // probe-deep (`cfg.last_probe_cascade_level`) when available (case
    // --pre-flight within the same process invocation). Otherwise,
    // we derive from the observable signal: 1 retry with fallback = level 1;
    // 2+ retries = level 2+. Without retries = level 0.
    let cascade_level_observed = cfg
        .last_probe_cascade_level
        .or_else(|| Some(derive_cascade_level_from_attempts(&agregado)));

    let mut metadata_val = SearchMetadata {
        execution_time_ms: elapsed_ms,
        selectors_hash,
        retries: retries_count,
        retries_configured: Some(cfg.retries.get()),
        used_fallback_endpoint: agregado.used_fallback_lite,
        concurrent_fetches: 0,
        fetch_successes: 0,
        fetch_failures: 0,
        used_chrome: chrome_result_used,
        chrome_attempted,
        // GAP-E2E-V14-UA-METADATA-LIE: emit SSOT Chrome UA when chromiumoxide ran.
        user_agent: emit_user_agent.clone(),
        used_proxy: config_proxy.is_active(),
        identity_used: effective_identity_tag.clone(),
        cascade_level: None,
        // GAP-E2E-V14-ALLOW-LITE-SILENT-NOOP: agent-visible no-op flag list.
        flags_ignored: if cfg.allow_lite_fallback {
            Some(vec!["allow-lite-fallback".to_string()])
        } else {
            None
        },
        pre_flight_fired: false,
        // GAP-WS-PREFLIGHT-META-001: executed when flag on and web path applies.
        pre_flight_executed: cfg.pre_flight && cfg.vertical.includes_web(),
        pre_flight_status: if cfg.pre_flight && cfg.vertical.includes_web() {
            Some("ok".into())
        } else {
            None
        },
        news_promo_filtered: None,
        stream_requested: if cfg.stream_mode { Some(true) } else { None },
        stream_effective: if cfg.stream_mode {
            Some(false) // single-query stream is ignored
        } else {
            None
        },
        zero_cause: None,
        next_action_suggestion: None,
        // GAP-NEW-002 v0.8.0: HTTP decompression byte counters. When
        // , the compression ratio is
        // . When iguais,
        // o body veio como identity (sem encoding) ou via .
        bytes_raw: Some(agregado.bytes_in),
        bytes_decompressed: Some(agregado.bytes_out),
        cascade_level_observed,
        result_count_compat: None,
        endpoint_used_compat: None,
        // GAP-WS-104: `None` no modo web default preserva o contrato JSON
        // byte-identical pre-v0.8.9 (`skip_serializing_if`).
        vertical_used: Some(cfg.vertical.as_str().to_string()),
        chrome_path_resolved: None,
        chrome_channel: None,
        run_id: Some(run_id),
    };
    fill_chrome_agent_metadata(&mut metadata_val, cfg);

    // GAP-AUD-003 v0.8.0: classificar zero-result causalmente.
    // Only runs on the zero path (`quantidade == 0`) to avoid cost on success.
    // GAP-WS-104: in news-only mode the web pipeline does not run — classification
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
        metadata_val.next_action_suggestion =
            next_action_suggestion_for_zero(cause).map(str::to_string);
    }

    // GAP-NEW-004 v0.8.0: lite auto-fallback — wire-in happens after
    // construction of  (see block below).

    let mut output = SearchOutput {
        query: cfg.query.as_str().to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: agregado.effective_endpoint.as_str().to_string(),
        timestamp,
        region: search::format_kl(cfg.language.as_str(), cfg.country.as_str()),
        result_count: quantidade,
        results: agregado.results,
        pages_fetched: agregado.pages_fetched,
        news: None,
        news_count: None,
        error: None,
        message: None,
        metadata: metadata_val,
    };

    // GAP-WS-104 v0.8.9: wiring of the news vertical into the envelope. Populates
    // `noticias`/`quantidade_noticias` SOMENTE quando `--vertical news|all`
    // ran (in default web mode `news_outcome` is `None` — contract
    // byte-identical). Cap `--num` with the same GAP-WS-090 web pattern.
    #[cfg(feature = "chrome")]
    if let Some((mut news_results, news_body, promo_filtered)) = news_outcome.take() {
        if let Some(max) = cfg.num_results.map(|n| n.get()) {
            let max = max as usize;
            if news_results.len() > max {
                news_results.truncate(max);
            }
        }
        let news_quantidade = u32::try_from(news_results.len()).unwrap_or(u32::MAX);
        output.news = Some(news_results);
        output.news_count = Some(news_quantidade);
        if promo_filtered > 0 {
            output.metadata.news_promo_filtered = Some(promo_filtered);
        }

        // Zero news: interstitial anti-bot no body renderizado ⇒ AntiBot;
        // otherwise ⇒ VerticalNoResults (LEGITIMATE zero ⇒ exit 5, not 6).
        // Precedence rules:
        // - modo all com web>0: sucesso segue a web — zero_cause fica None;
        // - all mode with web==0: web classification (more informative) already
        //   ran and is preserved (`zero_cause.is_none()` fails);
        // - news-only: web classification was skipped — this block decides.
        if news_quantidade == 0 && output.result_count == 0 && output.metadata.zero_cause.is_none()
        {
            let cause = if crate::probe_deep::detect_interstitial(&news_body)
                != crate::probe_deep::InterstitialKind::None
            {
                crate::types::ZeroCause::AntiBot
            } else {
                crate::types::ZeroCause::VerticalNoResults
            };
            output.metadata.zero_cause = Some(cause);
            output.metadata.next_action_suggestion =
                next_action_suggestion_for_zero(cause).map(str::to_string);
        }

        // GAP F3 v0.8.9: no modo `all` com web>0, `causa_zero` permanece `None`
        // by semantics — the field describes the envelope total zero — and the
        // JSON contract does NOT gain new fields. To avoid discarding the
        // news block diagnosis silently, the warning goes to stderr
        // via `tracing::warn` (fora do stdout JSON).
        if news_quantidade == 0
            && output.result_count > 0
            && crate::probe_deep::detect_interstitial(&news_body)
                != crate::probe_deep::InterstitialKind::None
        {
            tracing::warn!(
                next_action_suggestion = "news vertical blocked by anti-bot interstitial; \
                 web returned results. Wait 300s and re-run with --vertical news, \
                 or use --proxy to rotate the egress IP.",
                "news vertical blocked by anti-bot in all mode — empty news"
            );
        }
    }

    // CM-09: surface news vertical unavailability on the SearchOutput (agent-honest).
    // Does not mark the whole search as error when web results exist.
    #[cfg(feature = "chrome")]
    if let Some(code) = news_vertical_fail.take() {
        if output.news.is_none() {
            let msg = format!("news_vertical_unavailable:{code}");
            if output.message.is_none() {
                output.message = Some(msg);
            }
            if output.metadata.next_action_suggestion.is_none() {
                output.metadata.next_action_suggestion = Some(
                    "News vertical failed mid-flight while web may still be valid. \
                     Retry with --vertical news, wait 300s, or use --proxy."
                        .to_string(),
                );
            }
        }
    }

    // GAP-WS-113: auto-fallback Lite permanently removed (was GAP-NEW-004).

    // Enriquecimento opcional via --fetch-content (iter. 5).
    // Residual Client is Some only under http-test-harness; Chrome path uses CDP.
    content_fetch::enrich_with_content(&mut output, residual_client.as_ref(), cfg, cancellation)
        .await;
    // GAP-WS-META-TIMING-001: wall clock includes content fetch.
    output.metadata.execution_time_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;

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

