// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound orchestrator — fans out via deep_research::run → parallel.rs.
// Concurrency bound: root `--parallel` / `--max-concurrency`.
//! Handler for the `deep-research` subcommand.

use crate::budget::{
    budget_underflow_exit_code, budget_underflow_payload, print_budget_payload,
    validate_deep_research_budget_ex, BudgetDecision, DeepResearchBudgetInput,
};
use crate::cli::{CliArgs, CliIdentityProfile, DeepResearchArgs};
use crate::error::{exit_codes, CliError};
use crate::http;
use crate::output;
use crate::selectors;
use crate::types::bounded::DEEP_RESEARCH_TIMEOUT_GRACE_SECONDS;
use crate::types::{Config, Endpoint, OutputFormat, SafeSearch, VerticalMode};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Executes the `deep-research` subcommand (v1.0.2 budget contract).
///
/// Builds a default [`Config`] (15 results per sub-query), then delegates to
/// [`crate::deep_research::run_deep_research`].
///
/// Honors `--global-timeout` from the root parser (GAP-WS B3) and global
/// `-o/--output` (GAP-E2E-48-006): success and timeout envelopes share
/// [`output::emit_payload`].
///
/// v1.0.2: fail-fast budget gate **before** Chrome (GAP-AUD-DR-001 / CM-01).
pub async fn execute_deep_research(
    mut args: DeepResearchArgs,
    root_global_timeout_seconds: u64,
    search_defaults: &CliArgs,
    allow_lite_fallback: bool,
    pre_flight: bool,
    identity_profile: CliIdentityProfile,
    cancellation: CancellationToken,
) -> i32 {
    use crate::deep_research::run_deep_research;

    // Apply XDG deep knobs when CLI still at clap defaults (GAP-AUD-DR-006).
    let xdg = crate::commands::config_cmd::load_runtime_user_config();
    apply_xdg_to_deep_args(&mut args, search_defaults, &xdg);

    // CM-01b: print-budget + budget gate run BEFORE Chrome require so legacy
    // 5×10 loads fail-fast with exit 2 even without a Chrome binary.
    let effective_no_news = args.no_news;

    let require_results = args.require_results;
    // Trust boundary (GAP-SECDEV-008): deep-research must not bypass ValidatedQuery.
    // --print-budget may use a placeholder query.
    let query_raw = if args.print_budget && args.query.trim().is_empty() {
        "budget".to_string()
    } else {
        args.query.clone()
    };
    let validated_query = match crate::security::ValidatedQuery::try_new(&query_raw) {
        Ok(v) => v,
        Err(e) => {
            let payload = serde_json::json!({
                "error": e.error_code(),
                "message": format!("{e}"),
                "next_action_suggestion": "Provide a non-empty query without control/bidi characters (max 2048 chars).",
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

    // CM-15b: single map clap → domain (DRY). Override query (validated) and no_news.
    let mut dr = args.clone().into_domain();
    dr.query = validated_query.as_str().to_string();
    dr.no_news = effective_no_news;

    // Resolve budget estimate constants (XDG overrides when present).
    let mut budget_input = DeepResearchBudgetInput::from_cli(
        dr.max_sub_queries,
        dr.fetch_content,
        search_defaults.fetch_content_cap,
        !dr.no_news,
        dr.depth,
    );
    budget_input.parallelism = search_defaults.parallelism;
    budget_input.chrome_n = crate::process_count::count_chrome_like_processes() as u64;
    budget_input.shared_session_verticals = search_defaults.shared_session_verticals;
    if let Some(s) = xdg.budget_serp_seconds() {
        budget_input.serp_seconds = s;
    }
    if let Some(s) = xdg.budget_fetch_seconds() {
        budget_input.fetch_seconds = s;
    }
    if let Some(m) = xdg.budget_safety_margin_percent() {
        budget_input.margin_percent = m;
    }
    if let Some(v) = xdg.budget_contention_low() {
        budget_input.contention.low = v;
    }
    if let Some(v) = xdg.budget_contention_high() {
        budget_input.contention.high = v;
    }
    if let Some(v) = xdg.budget_contention_factor_mid_percent() {
        budget_input.contention.mid_percent = v;
    }
    if let Some(v) = xdg.budget_contention_factor_high_percent() {
        budget_input.contention.high_percent = v;
    }
    // Profile seeds first; individual XDG knobs above already overrode factory.
    // Re-apply individual knobs AFTER profile so explicit keys still win (G4).
    if let Some(profile) = xdg.budget_profile() {
        crate::budget::apply_budget_profile(&mut budget_input, profile);
        // Re-apply explicit per-key XDG overrides (higher precedence than profile).
        if let Some(s) = xdg.budget_serp_seconds() {
            budget_input.serp_seconds = s;
        }
        if let Some(s) = xdg.budget_fetch_seconds() {
            budget_input.fetch_seconds = s;
        }
        if let Some(m) = xdg.budget_safety_margin_percent() {
            budget_input.margin_percent = m;
        }
        if let Some(v) = xdg.budget_contention_low() {
            budget_input.contention.low = v;
        }
        if let Some(v) = xdg.budget_contention_high() {
            budget_input.contention.high = v;
        }
        if let Some(v) = xdg.budget_contention_factor_mid_percent() {
            budget_input.contention.mid_percent = v;
        }
        if let Some(v) = xdg.budget_contention_factor_high_percent() {
            budget_input.contention.high_percent = v;
        }
    }

    let allow_under = args.allow_under_budget
        || xdg.deep_research_allow_under_budget().unwrap_or(false);
    // Default ON (agent-ready): raise GT under contention (CLI-AUTO-01).
    let auto_contention = if args.no_auto_contention_budget {
        false
    } else if args.auto_contention_budget {
        true
    } else {
        xdg.deep_research_auto_contention_budget().unwrap_or(true)
    };

    // Fail-closed --fields/--filter even on --print-budget (agent-native; no
    // silent accept of unknown tokens that only break on full run).
    if let Some(raw) = search_defaults
        .fields
        .as_deref()
        .or(search_defaults.select.as_deref())
        .or(args.fields.as_deref())
        .or(args.select.as_deref())
    {
        if let Err(e) = crate::output::FieldSet::parse(raw) {
            let payload = serde_json::json!({
                "error": e.error_code(),
                "message": format!("{e}"),
                "next_action_suggestion": "Use --fields with allowlisted PT wire keys or EN aliases (e.g. title,url or titulo,url).",
            });
            let _ = output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    }
    if let Some(raw) = search_defaults
        .result_filter
        .as_deref()
        .or(args.result_filter.as_deref())
    {
        if let Err(e) = crate::output::ResultFilter::parse(raw) {
            let payload = serde_json::json!({
                "error": e.error_code(),
                "message": format!("{e}"),
                "next_action_suggestion": "Use --filter with titulo~/title~, url~, snippet~, or host:.",
            });
            let _ = output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    }

    if args.print_budget {
        let decision = validate_deep_research_budget_ex(
            budget_input,
            root_global_timeout_seconds,
            allow_under,
            auto_contention,
        );
        let effective = match &decision {
            BudgetDecision::Proceed {
                effective_global_timeout,
                ..
            } => *effective_global_timeout,
            BudgetDecision::Reject {
                global_timeout_seconds,
                ..
            } => *global_timeout_seconds,
        };
        let payload = print_budget_payload(
            budget_input,
            root_global_timeout_seconds,
            allow_under,
            auto_contention,
            effective,
        );
        match output::emit_payload(&payload.to_string(), output_file.as_deref()) {
            Ok(()) => return exit_codes::SUCCESS,
            Err(CliError::BrokenPipe) => return exit_codes::BROKEN_PIPE,
            Err(err) => {
                output::emit_stderr(format!("failed to emit budget: {err}"));
                return exit_codes::GENERIC_ERROR;
            }
        }
    }

    // CM-01 / CLI-AUTO-01: fail-fast or auto-raise under contention.
    let effective_global_timeout = match validate_deep_research_budget_ex(
        budget_input,
        root_global_timeout_seconds,
        allow_under,
        auto_contention,
    ) {
        BudgetDecision::Reject {
            estimate_seconds,
            gated_seconds,
            global_timeout_seconds,
        } => {
            output::emit_stderr(crate::i18n::tf(
                crate::i18n::Message::DeepResearchBudgetUnderflow,
                &[
                    ("timeout", &global_timeout_seconds.to_string()),
                    ("gated", &gated_seconds.to_string()),
                    ("estimate", &estimate_seconds.to_string()),
                ],
            ));
            let payload = budget_underflow_payload(
                estimate_seconds,
                gated_seconds,
                global_timeout_seconds,
                budget_input,
            );
            match output::emit_payload(&payload.to_string(), output_file.as_deref()) {
                Ok(()) => return budget_underflow_exit_code(),
                Err(CliError::BrokenPipe) => return exit_codes::BROKEN_PIPE,
                Err(_) => return budget_underflow_exit_code(),
            }
        }
        BudgetDecision::Proceed {
            estimate_seconds,
            gated_seconds,
            effective_global_timeout: eff,
            allow_under_budget: warned,
            auto_raised,
        } => {
            if warned {
                output::emit_stderr(crate::i18n::tf(
                    crate::i18n::Message::DeepResearchBudgetAllowOverride,
                    &[
                        ("timeout", &root_global_timeout_seconds.to_string()),
                        ("gated", &gated_seconds.to_string()),
                        ("estimate", &estimate_seconds.to_string()),
                    ],
                ));
            }
            if auto_raised && !search_defaults.quiet {
                output::emit_stderr(format!(
                    "auto-contention-budget: raised global timeout {root_global_timeout_seconds}s → {eff}s \
(chrome_n={}, factor={}%, keep -p>=2 for dual multiproc; outer shell timeout hint: {}s)",
                    budget_input.chrome_n,
                    crate::budget::input_contention_factor_percent(budget_input),
                    crate::budget::shell_timeout_hint(budget_input)
                ));
            }
            eff
        }
    };

    // GAP-WS-113: fail closed after budget gate (CM-01b) — no auto --no-news.
    if let Err(e) = crate::chrome_policy::require_chrome_transport() {
        if !crate::chrome_policy::http_test_harness_active() {
            let payload = serde_json::json!({
                "error": e.error_code(),
                "message": format!("{e}"),
                "next_action_suggestion": "Chrome is required for deep-research (GAP-WS-113).",
            });
            let _ = output::print_line_stdout(&payload.to_string());
            return e.exit_code();
        }
    }

    // CM-05: register in-flight so SIGTERM force-exit emits agent JSON before reap.
    let _deep_inflight = output::DeepInFlightGuard::arm(output_file.as_deref());

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
            fetch_content: {
                let mut fetch = dr.fetch_content;
                let fields_raw = search_defaults
                    .fields
                    .as_deref()
                    .or(search_defaults.select.as_deref());
                if let Some(raw) = fields_raw {
                    if let Ok(fs) = crate::output::FieldSet::parse(raw) {
                        if !fs.requests_content() {
                            fetch = false;
                        }
                    }
                }
                fetch
            },
            fetch_content_cap: search_defaults.fetch_content_cap,
            fields: search_defaults
                .fields
                .clone()
                .or_else(|| search_defaults.select.clone()),
            result_filter: search_defaults.result_filter.clone(),
            result_limit: search_defaults.result_limit,
            sort: search_defaults.sort.clone(),
            dedupe_by: search_defaults.dedupe_by.clone(),
            count_only: search_defaults.count_only,
            truncate_content: search_defaults.truncate_content,
            max_output_bytes: search_defaults.max_output_bytes,
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
            // Residual HTTP warm-up only (GAP-WS-113). Production SERP warm-up is
            // Chrome extract `page.goto(serp_origin)` + news session prime — always on.
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

    output::set_process_max_output_bytes(config.max_output_bytes);

    // GAP-DEEP-PROJECT-FILTER: parse once for success + timeout partial emit.
    // Invalid --fields/--filter already fail earlier on buscar parse; re-parse is
    // infallible when present (same allowlist). Fail closed if somehow invalid.
    let deep_fields = match config.fields.as_deref() {
        Some(raw) => match output::FieldSet::parse(raw) {
            Ok(fs) => Some(fs),
            Err(err) => {
                output::emit_stderr(err.to_string());
                return exit_codes::INVALID_CONFIG;
            }
        },
        None => None,
    };
    let deep_filter = match config.result_filter.as_deref() {
        Some(raw) => match output::ResultFilter::parse(raw) {
            Ok(f) => Some(f),
            Err(err) => {
                output::emit_stderr(err.to_string());
                return exit_codes::INVALID_CONFIG;
            }
        },
        None => None,
    };

    // GAP-WS-TMP-PROFILE-ORPHAN-001: use main's CancellationToken (SIGINT/SIGTERM)
    // and fence with global timeout so Chrome sessions are cancelled + reaped.
    // GAP-E2E-48-007 / CM-05: pin future so cancel can harvest partials after timeout.
    // v1.0.2 CM-05: emit envelope **before** heavy oneshot cleanup.
    let global_timeout = Duration::from_secs(effective_global_timeout);
    let deep_future = run_deep_research(dr, &config, cancellation.clone());
    tokio::pin!(deep_future);

    let result = match tokio::time::timeout(global_timeout, &mut deep_future).await {
        Ok(inner) => inner,
        Err(_elapsed) => {
            cancellation.cancel();
            // Best-effort partial harvest after cancel (one-shot still reaps below).
            let grace = Duration::from_secs(DEEP_RESEARCH_TIMEOUT_GRACE_SECONDS);
            let partial = match tokio::time::timeout(grace, deep_future).await {
                Ok(Ok(mut output)) => {
                    output::project::apply_to_deep_output(
                        &mut output,
                        deep_fields.as_ref(),
                        deep_filter.as_ref(),
                    );
                    Some(output)
                }
                _ => None,
            };
            // CM-05: emit agent envelope first (stdout / -o atomwrite), then reap.
            output::emit_stderr(crate::i18n::deep_research_timeout_exceeded(
                root_global_timeout_seconds,
            ));
            let exit = output::emit_timeout_envelope(
                root_global_timeout_seconds,
                partial.as_ref(),
                output_file.as_deref(),
                deep_fields.as_ref(),
            )
            .await;
            #[cfg(feature = "chrome")]
            crate::process_lifecycle::ensure_oneshot_cleanup();
            return exit;
        }
    };

    let exit = match result {
        Ok(mut output) => {
            // Agent-native order: filter → sort → dedupe → project → limit → truncate.
            let _pre = output::project::apply_to_deep_output(
                &mut output,
                None,
                deep_filter.as_ref(),
            );
            let sort_spec = match config.sort.as_deref() {
                Some(raw) => match output::SortSpec::parse(raw) {
                    Ok(s) => Some(s),
                    Err(err) => {
                        output::emit_stderr(err.to_string());
                        #[cfg(feature = "chrome")]
                        crate::process_lifecycle::ensure_oneshot_cleanup();
                        return exit_codes::INVALID_CONFIG;
                    }
                },
                None => None,
            };
            let dedupe = match config.dedupe_by.as_deref() {
                Some(raw) => match output::DedupeBy::parse(raw) {
                    Ok(d) => Some(d),
                    Err(err) => {
                        output::emit_stderr(err.to_string());
                        #[cfg(feature = "chrome")]
                        crate::process_lifecycle::ensure_oneshot_cleanup();
                        return exit_codes::INVALID_CONFIG;
                    }
                },
                None => None,
            };
            output::apply_sort_deep(&mut output, sort_spec.as_ref());
            output::apply_dedupe_deep(&mut output, dedupe);
            let _ = output::project::apply_to_deep_output(
                &mut output,
                deep_fields.as_ref(),
                None,
            );
            output::project::apply_result_limit_deep(&mut output, config.result_limit);
            output::apply_truncate_content_deep(
                &mut output,
                config.truncate_content.map(|n| n as usize),
            );

            // DEEP-E2E-08 / V18: cooperative SIGINT/SIGTERM must never map to
            // exit 5 (empty index). Cancel is recorded on the token and/or
            // sub-query error text "cancelled" before aggregation returns Ok.
            let cancelled = cancellation.is_cancelled()
                || output.metadata.sub_queries.iter().any(|s| {
                    let status_cancel = s.status.eq_ignore_ascii_case("cancelled");
                    let err_cancel = s.error.as_deref().is_some_and(|m| {
                        let lower = m.to_ascii_lowercase();
                        lower == "cancelled"
                            || lower.contains("cancel")
                            || lower.contains("sigterm")
                            || lower.contains("sigint")
                    });
                    let news_cancel = s.news_error.as_deref().is_some_and(|m| {
                        let lower = m.to_ascii_lowercase();
                        lower == "cancelled" || lower.contains("cancel")
                    });
                    status_cancel || err_cancel || news_cancel
                });

            // v0.7.10 P4 / GAP-WS-1114: --require-results + zero results → non-zero.
            // Cancel wins over require-results (agent must see 130/143, not timeout).
            if cancelled {
                // Emit partial envelope if any rows harvested; exit is signal-aware.
                let fields_for_emit = deep_fields.clone();
                let serialize = crate::concurrency::run_cpu_bound({
                    let out = output.clone();
                    move || output::project::format_deep_json(&out, fields_for_emit.as_ref())
                })
                .await;
                if let Ok(Ok(json)) = serialize {
                    let _ = output::emit_payload(&json, output_file.as_deref());
                }
                crate::signals::last_cancel_exit_code()
            } else if require_results && output.metadata.unique_result_count == 0 {
                let q = format!("{query_for_error:?}");
                output::emit_stderr(crate::i18n::tf(
                    crate::i18n::Message::DeepResearchZeroResultsRequire,
                    &[("query", &q)],
                ));
                exit_codes::GLOBAL_TIMEOUT
            } else if args.require_all_sub_queries && output.metadata.sub_queries_error > 0 {
                let payload = serde_json::json!({
                    "error": "sub_queries_incomplete",
                    "type": "deep_research_error",
                    "message": format!(
                        "require-all-sub-queries: {}/{} sub-queries failed",
                        output.metadata.sub_queries_error,
                        output.metadata.sub_queries_total
                    ),
                    "sub_queries_total": output.metadata.sub_queries_total,
                    "sub_queries_ok": output.metadata.sub_queries_ok,
                    "sub_queries_error": output.metadata.sub_queries_error,
                    "partial": true,
                    "next_action_suggestion":
                        "Retry with higher --global-timeout (see print-budget suggested_global_timeout), \
keep -p>=2 for dual multiproc, or drop --require-all-sub-queries for partial harvest.",
                });
                let _ = output::emit_payload(&payload.to_string(), output_file.as_deref());
                // Still emit success body if hits exist? Agent strict: error JSON is enough.
                exit_codes::INVALID_CONFIG
            } else {
                // GAP-WS-105 + GAP-E2E-V11-DEEP-ZERO-EXIT5 / V12:
                // exit 0 when either vertical produced results;
                // if empty AND any sub-query failed with chrome/config class → exit 2
                // (not exit 5 "empty index"); genuine empty → exit 5.
                let success_code = if output.results.is_empty() && output.news_count == 0 {
                    let wire_codes = output.metadata.sub_queries.iter().map(|s| {
                        // Prefer structured error when present; deep stores free text.
                        s.error.as_deref().and_then(|msg| {
                            if crate::error::chrome_classify::message_implies_chrome_or_config(msg)
                            {
                                Some(crate::error::codes::CHROME_UNAVAILABLE)
                            } else if msg.to_ascii_lowercase().contains("invalid") {
                                Some(crate::error::codes::INVALID_CONFIG)
                            } else {
                                None
                            }
                        })
                    });
                    let free_text = output
                        .metadata
                        .sub_queries
                        .iter()
                        .map(|s| s.error.as_deref())
                        .chain(
                            output
                                .metadata
                                .sub_queries
                                .iter()
                                .map(|s| s.news_error.as_deref()),
                        );
                    crate::error::exit_for_zero_results_with_errors(wire_codes, free_text)
                } else {
                    exit_codes::SUCCESS
                };

                // GAP-PAR-040c + GAP-E2E-48-006: serialize off worker; emit via unified -o route.
                // When --fields is set, project result-row keys via Value (score/fontes drop).
                if config.count_only {
                    let payload = output::count_only_deep(&output);
                    match output::emit_payload(&payload, output_file.as_deref()) {
                        Ok(()) => success_code,
                        Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
                        Err(err) => {
                            output::emit_stderr(crate::i18n::error_msg(
                                crate::i18n::Message::StdoutWriteFailed,
                                &err,
                            ));
                            exit_codes::GENERIC_ERROR
                        }
                    }
                } else {
                let fields_for_emit = deep_fields.clone();
                let serialize = crate::concurrency::run_cpu_bound(move || {
                    output::project::format_deep_json(&output, fields_for_emit.as_ref())
                })
                .await;
                match serialize {
                    Ok(Ok(json)) => match output::emit_payload(&json, output_file.as_deref()) {
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
                    Ok(Err(err)) | Err(err) => {
                        output::emit_stderr(crate::i18n::error_msg(
                            crate::i18n::Message::DeepResearchSerializeFailed,
                            &err,
                        ));
                        exit_codes::GENERIC_ERROR
                    }
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

/// Apply XDG deep-research knobs when CLI left clap defaults.
fn apply_xdg_to_deep_args(
    args: &mut DeepResearchArgs,
    search_defaults: &CliArgs,
    xdg: &crate::commands::config_cmd::UserConfig,
) {
    use crate::deep_research::DEFAULT_MAX_SUB_QUERIES;
    if args.max_sub_queries == DEFAULT_MAX_SUB_QUERIES {
        if let Some(n) = xdg.default_max_sub_queries() {
            args.max_sub_queries = n;
        }
    }
    if !args.allow_under_budget && xdg.deep_research_allow_under_budget() == Some(true) {
        args.allow_under_budget = true;
    }
    // fetch_content_cap lives on search_defaults (root flatten); applied in config_cmd.
    let _ = search_defaults;
}

