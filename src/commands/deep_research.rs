// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound orchestrator — fans out via deep_research::run → parallel.rs.
// Concurrency bound: root `--parallel` / `--max-concurrency`.
//! Handler for the `deep-research` subcommand.

use crate::cli::{CliArgs, CliIdentityProfile, DeepResearchArgs};
use crate::error::{exit_codes, CliError};
use crate::http;
use crate::output;
use crate::selectors;
use crate::types::bounded::{
    estimate_deep_research_seconds, DEEP_RESEARCH_TIMEOUT_GRACE_SECONDS,
};
use crate::types::{Config, Endpoint, OutputFormat, SafeSearch, VerticalMode};
use std::path::Path;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Executes the `deep-research` subcommand (v0.7.0 / v1.0.1 contract).
///
/// Builds a default [`Config`] (15 results per sub-query), then delegates to
/// [`crate::deep_research::run_deep_research`].
///
/// Honors `--global-timeout` from the root parser (GAP-WS B3) and global
/// `-o/--output` (GAP-E2E-48-006): success and timeout envelopes share
/// [`output::emit_payload`].
pub async fn execute_deep_research(
    args: DeepResearchArgs,
    root_global_timeout_seconds: u64,
    search_defaults: &CliArgs,
    allow_lite_fallback: bool,
    pre_flight: bool,
    identity_profile: CliIdentityProfile,
    cancellation: CancellationToken,
) -> i32 {
    use crate::deep_research::{run_deep_research, DeepResearchArgs as DrArgs};

    // GAP-WS-113: fail closed — no auto --no-news degradation.
    if let Err(e) = crate::chrome_policy::require_chrome_transport() {
        if !crate::chrome_policy::http_test_harness_active() {
            let payload = serde_json::json!({
                "erro": e.error_code(),
                "mensagem": format!("{e}"),
                "sugestao_proxima_acao": "Chrome is required for deep-research (GAP-WS-113).",
            });
            let _ = output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    }
    let effective_no_news = args.no_news;

    let require_results = args.require_results;
    // Trust boundary (GAP-SECDEV-008): deep-research must not bypass ValidatedQuery.
    let validated_query = match crate::security::ValidatedQuery::try_new(&args.query) {
        Ok(v) => v,
        Err(e) => {
            let payload = serde_json::json!({
                "erro": e.error_code(),
                "mensagem": format!("{e}"),
                "sugestao_proxima_acao": "Provide a non-empty query without control/bidi characters (max 2048 chars).",
            });
            let _ = output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    };
    let query_for_error = validated_query.as_str().to_string();

    // GAP-E2E-48-006: honor global `-o` (same contract as buscar).
    let output_file = search_defaults.output_file.clone();
    if let Some(ref path) = output_file {
        if let Err(e) = crate::paths::validate_output_path(path) {
            output::emit_stderr(e.to_string());
            return exit_codes::INVALID_CONFIG;
        }
    }

    let dr = DrArgs {
        query: validated_query.as_str().to_string(),
        max_sub_queries: args.max_sub_queries,
        sub_query_strategy: args.sub_query_strategy.into(),
        sub_queries_file: args.sub_queries_file.clone(),
        aggregation: args.aggregation.into(),
        depth: args.depth,
        fetch_content: !args.no_fetch_content,
        synthesize: args.synthesize,
        budget_tokens: args.budget_tokens,
        synth_format: args.synth_format.into(),
        no_news: effective_no_news,
    };

    // CM-06: warn when global timeout is below conservative workload estimate.
    let est = estimate_deep_research_seconds(
        dr.max_sub_queries,
        dr.fetch_content,
        search_defaults.fetch_content_cap,
        !dr.no_news,
    );
    if root_global_timeout_seconds < est {
        output::emit_stderr(format!(
            "Warning: --global-timeout {root_global_timeout_seconds}s is below estimated deep-research lower bound ~{est}s \
(max-sub-queries={}, fetch_content={}, dual_vertical={}). \
Raise --global-timeout, use --no-fetch-content, --no-news, or lower --max-sub-queries / --fetch-content-cap.",
            dr.max_sub_queries,
            dr.fetch_content,
            !dr.no_news
        ));
    }

    let ua_list = http::load_user_agents(search_defaults.match_platform_ua);
    let browser_profile = http::select_profile_from_list_seeded(&ua_list, search_defaults.seed);
    let user_agent = browser_profile.user_agent.clone();
    let selectors = selectors::load_selectors();
    let effective_num = search_defaults.num_results.unwrap_or(15);
    let proxy_config = crate::http::ProxyConfig::try_from_options(
        search_defaults.proxy.as_deref(),
        search_defaults.no_proxy,
    );
    let proxy_config = match proxy_config {
        Ok(p) => p,
        Err(e) => {
            output::emit_stderr(e.to_string());
            return exit_codes::INVALID_CONFIG;
        }
    };
    let config = match (|| -> Result<Config, crate::error::CliError> {
        Ok(Config {
            query: validated_query.clone(),
            queries: vec![validated_query.clone()],
            num_results: Some(crate::types::ResultCount::try_new(effective_num)?),
            format: OutputFormat::Json,
            timeout_seconds: crate::types::TimeoutSeconds::try_new(search_defaults.timeout_seconds)?,
            language: crate::types::SerpLanguage::try_new(&search_defaults.language)?,
            country: crate::types::SerpCountry::try_new(&search_defaults.country)?,
            pre_flight,
            verbose: search_defaults.verbose,
            quiet: search_defaults.quiet,
            user_agent: crate::types::UserAgentString::try_new(&user_agent)?,
            browser_profile,
            parallelism: crate::types::ParallelismDegree::try_new(search_defaults.parallelism)?,
            pages: crate::types::PageCount::try_new(1)?,
            retries: crate::types::RetryBudget::try_new(search_defaults.retries)?,
            endpoint: Endpoint::from(search_defaults.endpoint),
            // GAP-WS-105 v0.8.9: news is DEFAULT in deep-research; --no-news
            // downgrades to pure web vertical.
            vertical: if dr.no_news {
                VerticalMode::Web
            } else {
                VerticalMode::All
            },
            time_filter: None,
            safe_search: SafeSearch::Moderate,
            stream_mode: false,
            // GAP-E2E-48-006: propagate global `-o` (was hardcoded None).
            output_file: output_file.clone(),
            fetch_content: dr.fetch_content,
            fetch_content_cap: search_defaults.fetch_content_cap,
            max_content_length: crate::types::ContentLengthLimit::try_new(
                search_defaults.max_content_length,
            )?,
            proxy_config,
            global_timeout_seconds: crate::types::GlobalTimeoutSeconds::try_new(
                root_global_timeout_seconds,
            )?,
            match_platform_ua: search_defaults.match_platform_ua,
            per_host_limit: crate::types::PerHostLimit::try_new(search_defaults.per_host_limit)?,
            chrome_path: search_defaults.chrome_path.clone(),
            chrome_force_visible: search_defaults.chrome_visible,
            chrome_force_headless: search_defaults.chrome_headless,
            chrome_force_xvfb: search_defaults.chrome_xvfb,
            dump_news_html: search_defaults.dump_news_html.clone(),
            selectors,
            cookie_provider: None,
            persistent_jar: None,
            warmup_enabled: false,
            allow_lite_fallback,
            identity_profile,
            last_probe_cascade_level: None,
            shared_session_verticals: search_defaults.shared_session_verticals,
        })
    })() {
        Ok(c) => c,
        Err(e) => {
            output::emit_stderr(e.to_string());
            return exit_codes::INVALID_CONFIG;
        }
    };

    // GAP-SCRAPE-R-007: CLI Chrome display policy before deep-research launches.
    #[cfg(feature = "chrome")]
    crate::browser::set_chrome_display_cli(crate::browser::ChromeDisplayCli {
        force_visible: config.chrome_force_visible,
        force_headless: config.chrome_force_headless,
        force_xvfb: config.chrome_force_xvfb,
    });

    // GAP-WS-TMP-PROFILE-ORPHAN-001: use main's CancellationToken (SIGINT/SIGTERM)
    // and fence with global timeout so Chrome sessions are cancelled + reaped.
    // GAP-E2E-48-007 / CM-05: pin future so cancel can harvest partials after timeout.
    let global_timeout = Duration::from_secs(root_global_timeout_seconds);
    let deep_future = run_deep_research(dr, &config, cancellation.clone());
    tokio::pin!(deep_future);

    let result = match tokio::time::timeout(global_timeout, &mut deep_future).await {
        Ok(inner) => inner,
        Err(_elapsed) => {
            cancellation.cancel();
            // Best-effort partial harvest after cancel (one-shot still reaps below).
            let grace = Duration::from_secs(DEEP_RESEARCH_TIMEOUT_GRACE_SECONDS);
            let partial = match tokio::time::timeout(grace, deep_future).await {
                Ok(Ok(output)) => Some(output),
                _ => None,
            };
            #[cfg(feature = "chrome")]
            crate::process_lifecycle::ensure_oneshot_cleanup();
            output::emit_stderr(crate::i18n::deep_research_timeout_exceeded(
                root_global_timeout_seconds,
            ));
            let exit = emit_timeout_envelope(
                root_global_timeout_seconds,
                partial.as_ref(),
                output_file.as_deref(),
            )
            .await;
            return exit;
        }
    };

    let exit = match result {
        Ok(output) => {
            // v0.7.10 P4 / GAP-WS-1114: --require-results + zero results → non-zero.
            if require_results && output.metadata.unique_result_count == 0 {
                let q = format!("{query_for_error:?}");
                output::emit_stderr(crate::i18n::tf(
                    crate::i18n::Message::DeepResearchZeroResultsRequire,
                    &[("query", &q)],
                ));
                exit_codes::GLOBAL_TIMEOUT
            } else {
                // GAP-WS-105: exit 0 when either vertical produced results;
                // exit 5 only when web AND news are both empty.
                let success_code = if output.results.is_empty() && output.news_count == 0 {
                    exit_codes::ZERO_RESULTS
                } else {
                    exit_codes::SUCCESS
                };

                // GAP-PAR-040c + GAP-E2E-48-006: serialize off worker; emit via unified -o route.
                match output::serialize_json_async(output).await {
                    Ok(json) => match output::emit_payload(&json, output_file.as_deref()) {
                        Ok(()) => success_code,
                        Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
                        Err(err) => {
                            output::emit_stderr(crate::i18n::error_msg(
                                crate::i18n::Message::StdoutWriteFailed,
                                &err,
                            ));
                            exit_codes::GENERIC_ERROR
                        }
                    },
                    Err(err) => {
                        output::emit_stderr(crate::i18n::error_msg(
                            crate::i18n::Message::DeepResearchSerializeFailed,
                            &err,
                        ));
                        exit_codes::GENERIC_ERROR
                    }
                }
            }
        }
        Err(err) => {
            output::emit_stderr(crate::i18n::error_msg(
                crate::i18n::Message::DeepResearchFailed,
                &err,
            ));
            // Signal-aware: SIGINT → 130, SIGTERM → 143 (graceful-shutdown rules).
            crate::signals::exit_code_for_error(&err)
        }
    };
    #[cfg(feature = "chrome")]
    crate::process_lifecycle::ensure_oneshot_cleanup();
    exit
}

/// Agent-stable timeout envelope (GAP-E2E-48-007). Exit remains 4.
async fn emit_timeout_envelope(
    seconds: u64,
    partial: Option<&crate::deep_research::DeepResearchOutput>,
    output_path: Option<&Path>,
) -> i32 {
    let mut payload = serde_json::json!({
        "erro": "timeout",
        "mensagem": format!("global timeout of {seconds}s exceeded (deep-research)"),
        "segundos": seconds,
        "comando": "deep-research",
        "tipo": "deep_research_error",
    });
    if let Some(out) = partial {
        if let Ok(partial_json) = serde_json::to_value(out) {
            payload["resultados_parciais"] = partial_json;
            payload["parcial"] = serde_json::json!(true);
        }
    } else {
        payload["parcial"] = serde_json::json!(false);
    }
    let body = payload.to_string();
    match output::emit_payload(&body, output_path) {
        Ok(()) => exit_codes::GLOBAL_TIMEOUT,
        Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
        Err(err) => {
            output::emit_stderr(format!("failed to emit timeout envelope: {err}"));
            exit_codes::GLOBAL_TIMEOUT
        }
    }
}
