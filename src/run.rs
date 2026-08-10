// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: orchestrator (CLI entry: parse → config → pipeline → emit/exit)
//! Process entrypoint [`run`] (SRP split from `lib.rs`).

use clap::Parser;
use tokio_util::sync::CancellationToken;

use crate::cli::{RootArgs, Subcommand};
use crate::commands::{
    execute_commands, execute_completions, execute_config, execute_deep_research, execute_doctor,
    execute_init_config, execute_locale, execute_man, execute_schema,
};
use crate::error::exit_codes;
use crate::i18n;
use crate::logging;
use crate::output;
use crate::pipeline;
use crate::platform;
use crate::{set_zero_cause_strict, zero_cause_is_non_legitimate, zero_cause_strict};

/// Reports a fail-fast agent-ops config error and returns the exit code to use.
///
/// # Why this is a function and not four copies
///
/// `--fields`, `--filter`, `--sort` and `--dedupe-by` are validated BEFORE any
/// Chrome session starts, so a typo costs nothing. Each of the four used to
/// carry a byte-identical fourteen-line rejection block; the only thing that
/// differed was the parser above it. Fifty-eight duplicated lines in the
/// middle of a very long function are fifty-eight lines where a fix can land
/// three times out of four — which is how `configuration_error(&err)` outlived
/// `localized_detail` here for a whole release.
///
/// The caller keeps the `return`, so control flow is unchanged: this only
/// decides what is written and which code is handed back.
fn reject_agent_ops_config(err: &crate::error::CliError, quiet: bool) -> i32 {
    if quiet {
        // -q means machine-first: the routable envelope on stdout, no prose.
        let payload = crate::types::ThinErrorResponse::new("invalid_config", format!("{err}"));
        let _ = output::emit_wire_line(&payload);
    } else {
        output::emit_stderr(err.localized_detail());
    }
    #[cfg(feature = "chrome")]
    crate::process_lifecycle::ensure_oneshot_cleanup();
    exit_codes::INVALID_CONFIG
}

/// Library entry point. Called by `main.rs`.
///
/// Returns the appropriate exit code (0 success, 1 generic error, 2 invalid config, etc.).
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future cancels all
/// in-flight HTTP requests via the [`CancellationToken`].
pub async fn run(cancellation: CancellationToken) -> i32 {
    // i18n Phase 1: Windows UTF-8 console BEFORE any user-visible I/O
    // (rules-rust multi-idioma init order). Clap may still print English
    // help before full locale init — that is intentional (agent-stable).
    platform::init();

    // v0.9.0 GAP-WS-106 Sintoma A: intercept clap errors to append a
    // placement tip when an `UnknownArgument` matches a known global flag
    // (the user likely passed it AFTER a subcommand). `try_parse` returns
    // Err instead of exiting, so we format + exit explicitly. `DisplayHelp` /
    // `DisplayVersion` are NOT user errors — defer to `e.exit()`.
    let mut root = match RootArgs::try_parse() {
        Ok(r) => r,
        Err(e) => {
            let kind = e.kind();
            if matches!(
                kind,
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                // GAP-E2E-V19-HELP-TOKEN-COST: agent-native — help on non-TTY
                // goes to stderr so stdout stays free for machine payload pipes.
                // Use clap's own render (respects `-h` short vs `--help` long and
                // subcommand context); do NOT re-emit root `write_long_help`.
                // `--version` stays on stdout (stable machine contract).
                if kind == clap::error::ErrorKind::DisplayHelp && !crate::platform::stdout_is_tty()
                {
                    use std::io::Write;
                    // `Error::render` preserves short/long and subcommand help text.
                    let rendered = e.render().ansi().to_string();
                    let _ = std::io::stderr().write_all(rendered.as_bytes());
                    std::process::exit(exit_codes::SUCCESS);
                }
                // TTY help / version: clap default (stdout) for human ergonomics.
                e.exit();
            }
            // Best-effort UI locale before early clap error (flag not parsed).
            i18n::initialize(None);
            let mut msg = format!("{e}");
            if kind == clap::error::ErrorKind::UnknownArgument {
                if let Some(raw) = e
                    .get(clap::error::ContextKind::InvalidArg)
                    .and_then(|v| v.to_string().split_whitespace().next().map(str::to_owned))
                    .map(|s| s.trim_start_matches('-').to_owned())
                {
                    if crate::cli::is_known_global_flag(&raw) {
                        // v1.0.3 GAP-AGENT-HINT: render the canonical LONG form.
                        // Rendering `--{raw}` for a short flag produced `--f`,
                        // which clap rejects exactly like the original mistake.
                        let canonical = crate::cli::canonical_long_flag(&raw).unwrap_or(&raw);
                        msg.push_str(&i18n::flag_must_precede_subcommand(canonical));
                    }
                }
            }
            // rules-rust-cli-stdin-stdout: stderr only for human/agent errors;
            // when stdout is not a TTY emit structured JSON on stderr for parsers.
            if crate::platform::stdout_is_tty() {
                output::emit_stderr(&msg);
            } else {
                let payload = serde_json::json!({
                    "type": "error",
                    "error": {
                        "category": "usage",
                        "code": "invalid_config",
                        "message": msg,
                        "retryable": false
                    }
                });
                // Value implements Display (compact JSON) — avoid intermediate String.
                output::emit_stderr(&payload);
            }
            std::process::exit(exit_codes::INVALID_CONFIG);
        }
    };

    // GAP-SCRAPE-R2-015 / GAP-XDG-RUNTIME-001: optional --config-home first so
    // XDG load sees the right directory, then apply config.toml onto defaults.
    crate::platform::set_config_home(root.config_home.clone());
    let xdg = crate::runtime::load_runtime_user_config();
    crate::runtime::apply_user_config_to_root(&mut root, &xdg);

    // GAP-E2E-V11-CHROME-FLAKY / V12: install process-wide Chrome session retry budget
    // from CLI/XDG (no product env). Used by pipeline launch retry + classify.
    crate::error::set_chrome_session_retries(root.buscar.chrome_session_retries);

    // Dispatch subcommand (or fall through to default = Buscar).
    // v0.7.9 GAP-WS-59: capture the global flags before any potential
    // partial move of `root.buscar` so we can pass them to `build_config`
    // after the match.
    // v0.7.10 B3 fix: also capture `global_timeout_seconds` here — it
    // lives on `RootArgs` and must be hoisted out before consuming
    // `root.buscar` (which is a `Box<CliArgs>`).
    let allow_lite_fallback = root.allow_lite_fallback;
    let pre_flight = root.pre_flight;
    let root_global_timeout_seconds = root.global_timeout_seconds;
    // GAP-SCRAPE-R2-009..015: install process policies from CLI (no product env).
    crate::signals::set_cancel_grace_secs(root.cancel_grace_secs);
    crate::retry::set_retry_disabled(root.buscar.disable_retry);
    crate::endpoints::set_endpoint_policy(crate::endpoints::EndpointPolicy {
        html: root.buscar.base_url_html.clone(),
        lite: root.buscar.base_url_lite.clone(),
        serp: root.buscar.base_url_serp.clone(),
    });
    set_zero_cause_strict(!root.no_zero_cause_strict);
    // v0.7.10 GAP-WS-60 fix: capture `identity_profile` from `root.buscar`
    // before consuming `args` via the `match`. `Box<CliArgs>` is dereferenced
    // to read the field without moving the box itself.
    let identity_profile = root.buscar.identity_profile;

    // GAP-E2E-V19-JSON-PRETTY-DEFAULT: compact JSON unless --pretty.
    output::set_json_pretty(root.buscar.pretty);
    // V30: wire key language (EN default; PT opt-in via --wire-keys / XDG).
    output::set_process_wire_keys(root.wire_keys);

    // i18n Phases 3–7: detect / negotiate / publish OnceLock before any
    // translated human stderr from subcommands.
    i18n::initialize(root.ui_lang.as_deref());

    // Initialize logging BEFORE subcommand dispatch so deep-research
    // respects -q/--verbose (stderr only; stdout remains agent payload).
    // GAP-LOG-ENV-001: XDG `log_directive` replaces product RUST_LOG.
    // GAP-E2E-V19-STDERR-INFO-DEFAULT: non-TTY default warn (agent quiet).
    let disable_colors = platform::should_disable_color(root.buscar.no_color);
    logging::initialize_logging(
        root.buscar.verbose,
        root.buscar.quiet,
        disable_colors,
        xdg.log_directive(),
    );

    // Install the agent-native output cap BEFORE subcommand dispatch.
    //
    // It used to be installed only on the search path (further down) and in
    // deep-research, so `--max-output-bytes` parsed, validated and then did
    // nothing on `commands`, `schema`, `doctor`, `locale`, `config` and the
    // probes — the surfaces whose envelopes an agent most wants capped. A flag
    // that is accepted and ignored is worse than one that is rejected: the
    // caller believes a budget is in force. Enforcement itself lives at
    // `output::emit::write_to_stdout`, the one place every stdout byte passes.
    output::set_process_max_output_bytes(root.buscar.agent.max_output_bytes);

    // Same reasoning, same defect, the other eight knobs. `--max-output-bytes`
    // was made universal in v1.0.3; `--fields`, `--filter`, `--limit`,
    // `--sort`, `--dedupe-by`, `--count-only` and `--truncate-content` were
    // not, so every subcommand below still ACCEPTED them and emitted a
    // byte-for-byte unchanged envelope. Installing them here lets each
    // introspection surface either honour a knob or refuse it by name — see
    // `output::envelope_ops`.
    output::set_process_agent_ops(output::AgentOps::from_root(&root.buscar));

    // GAP-E2E-V14-PRINT-SCHEMA-ROOT-MISSING: agent discovery without subcommand.
    if root.print_schema {
        return execute_schema(&crate::cli::SchemaArgs { name: None });
    }

    // GAP-E2E-V11-UNKNOWN-AS-QUERY: only bare root (no subcommand) is guarded.
    let mut bare_root_search = false;
    let mut args = match root.subcommand {
        Some(Subcommand::InitConfig(ref args)) => {
            return execute_init_config(args);
        }
        Some(Subcommand::Completions(ref args)) => {
            return execute_completions(args);
        }
        Some(Subcommand::Commands(args)) => {
            return execute_commands(args);
        }
        Some(Subcommand::Schema(ref args)) => {
            return execute_schema(args);
        }
        Some(Subcommand::Doctor(doc_args)) => {
            // GAP-E2E-V14: `doctor --probe-deep` → same path as root `--probe-deep`
            // (avoids clap UnknownArgument + wrong single-hyphen tip).
            if doc_args.probe_deep {
                let mut probe_args = root.buscar.clone();
                probe_args.probe_deep = true;
                return crate::probe::execute_probe_deep(&probe_args).await;
            }
            // GAP-PAR-043: async so chrome --version runs under run_cpu_bound.
            return execute_doctor(doc_args).await;
        }
        Some(Subcommand::Locale(args)) => {
            return execute_locale(args);
        }
        Some(Subcommand::Man(ref args)) => {
            return execute_man(args);
        }
        Some(Subcommand::Config(cmd)) => {
            return execute_config(cmd);
        }
        Some(Subcommand::Buscar(args)) => *args,
        Some(Subcommand::DeepResearch(dr_args)) => {
            // V13: after-subcommand knobs live on DeepResearchArgs; merge onto root defaults.
            let dr_args = *dr_args;
            let search_defaults = crate::cli::merge_deep_search_defaults(&root.buscar, &dr_args);
            // GAP-WS-TMP-PROFILE-ORPHAN-001: propagate main cancellation so
            // SIGINT/SIGTERM cancel deep-research Chrome sessions (not a local token).
            return execute_deep_research(
                dr_args,
                root_global_timeout_seconds,
                &search_defaults,
                allow_lite_fallback,
                pre_flight,
                identity_profile,
                cancellation,
            )
            .await;
        }
        // Bare root path (no subcommand) — enable unknown-subcommand guard below.
        None => {
            bare_root_search = true;
            root.buscar
        }
    };

    // Logging and platform already initialized before subcommand dispatch.

    // v0.6.4 WS-26: Intercept --probe BEFORE query validation. The probe
    // is a pre-flight health check that does NOT require a query — it sends
    // 1 minimal request to the configured endpoint and reports status as JSON.
    if args.probe {
        return crate::probe::execute_probe(&args).await;
    }

    // v0.7.3 PR3: deep probe — runs a real query and detects CAPTCHA
    // interstitials in the response body. Emits a JSON report on stdout.
    if args.probe_deep {
        return crate::probe::execute_probe_deep(&args).await;
    }

    // GAP-E2E-V11-PREFLIGHT-NO-QUERY / V12: `--pre-flight` alone is a health
    // calibration path (like `--probe`), not a silent "no query provided".
    // When the operator also supplies a QUERY, pre-flight runs on the shared
    // SERP session as before (`config.pre_flight = true` below).
    if pre_flight && args.queries.is_empty() && args.queries_file.is_none() {
        // GAP-E2E-V11-PREFLIGHT-NO-QUERY: standalone calibration without inventing
        // a QUERY. Empty stdin (TTY or zero-byte pipe) → probe health path.
        // Non-empty stdin lines are injected into `args.queries` so build_config
        // does not re-read a drained pipe.
        match crate::pipeline::read_queries_from_stdin_if_pipe() {
            Ok(lines) if lines.is_empty() => {
                tracing::info!(
                    "pre-flight without QUERY — running standalone transport health (probe path)"
                );
                return crate::probe::execute_probe(&args).await;
            }
            Ok(lines) => {
                args.queries = lines;
            }
            Err(err) => {
                tracing::warn!(%err, "pre-flight stdin read failed — falling through to config");
            }
        }
    }

    // GAP-E2E-V11-UNKNOWN-AS-QUERY / V12: hyphenated first token that is not a
    // known subcommand is almost always a typo (e.g. `not-a-real-subcommand`),
    // not a SERP query. Bare multi-word queries without hyphens stay valid.
    if bare_root_search {
        if let Some(token) = args.queries.first() {
            if crate::cli::looks_like_unknown_subcommand_token(token) {
                let tip = if token.contains('_') {
                    let normalized = token.replace('_', "-");
                    format!(
                        " Did you mean subcommand `{normalized}`? \
                         Underscore aliases are not valid subcommands."
                    )
                } else {
                    format!(" For a search query that contains hyphens, use: buscar \"{token}\"")
                };
                let msg = format!(
                    "unknown subcommand or invalid bare token `{token}`; \
                     known subcommands: buscar, deep-research, doctor, config, \
                     schema, locale, man, completions, commands, init-config.\
                     {tip}"
                );
                if args.quiet {
                    let payload = crate::types::ThinErrorResponse::new("invalid_config", &msg)
                        .with_search_shape();
                    let _ = output::emit_wire_line(&payload);
                } else {
                    output::emit_stderr(&msg);
                }
                return exit_codes::INVALID_CONFIG;
            }
        }
    }

    // Convert CliArgs into internal Config.
    let mut config = match crate::runtime::build_config(&args) {
        Ok(c) => c,
        Err(err) => {
            // GAP-WS-QUIET-CONFIG-001: with -q, avoid tracing/stderr noise; prefer JSON stdout.
            if args.quiet {
                let payload =
                    crate::types::ThinErrorResponse::new("invalid_config", format!("{err}"))
                        .with_search_shape();
                let _ = output::emit_wire_line(&payload);
            } else {
                tracing::error!(?err, "Invalid configuration");
                output::emit_stderr(err.localized_detail());
            }
            return exit_codes::INVALID_CONFIG;
        }
    };
    // v0.7.9 GAP-WS-59: inject the hoisted global flags into the
    // locally-built `Config`. `build_config` is `&CliArgs`-based and
    // the globals live on `RootArgs`; we apply them here so the
    // function signature stays minimal for the unit tests.
    // v0.7.10 B3 fix: also override `global_timeout_seconds` from the
    // hoisted `root_global_timeout_seconds` so the user-supplied value
    // is honored (the default value of 60 lives on `RootArgs`, not in
    // `CliArgs`).
    config.allow_lite_fallback = allow_lite_fallback;
    config.pre_flight = pre_flight;
    // GAP-SCRAPE-R-007: install CLI Chrome display policy before any launch.
    #[cfg(feature = "chrome")]
    crate::browser::set_chrome_display_cli(crate::browser::ChromeDisplayCli {
        force_visible: config.chrome_force_visible,
        force_headless: config.chrome_force_headless,
        force_xvfb: config.chrome_force_xvfb,
    });
    config.global_timeout_seconds =
        match crate::types::GlobalTimeoutSeconds::try_new(root_global_timeout_seconds) {
            Ok(v) => v,
            Err(e) => {
                output::emit_stderr(e.to_string());
                return exit_codes::INVALID_CONFIG;
            }
        };
    // v0.7.10 GAP-WS-60 fix: propagate `--identity-profile` into the Config
    // so the pipeline can fix the selected identity on the `IdentityPool`.
    config.identity_profile = identity_profile;

    let format = config.format;
    let output_file = config.output_file.clone();
    // Capture before pipeline moves `config` (GAP-FIELDS-PROJECT / GAP-RESULT-FILTER / --limit).
    let config_fields = config.agent_ops.fields.clone();
    let config_filter = config.agent_ops.filter.clone();
    let config_limit = config.agent_ops.limit;
    let config_sort = config.agent_ops.sort.clone();
    let config_dedupe = config.agent_ops.dedupe_by.clone();
    let config_count_only = config.agent_ops.count_only;
    let config_truncate = config.agent_ops.truncate_content;
    let config_max_output_bytes = config.max_output_bytes;
    output::set_process_max_output_bytes(config_max_output_bytes);
    let global_timeout = std::time::Duration::from_secs(config.global_timeout_seconds.get());

    // Fail-fast validate --fields / --filter / --limit BEFORE Chrome SERP
    // (GAP-E2E-V19-FILTER-SYNTAX-FOOTGUN): typos must not burn a live session.
    let field_set = match output::config_fields_parse(config_fields.as_deref()) {
        Ok(v) => v,
        Err(err) => return reject_agent_ops_config(&err, args.quiet),
    };
    let result_filter = match output::config_filter_parse(config_filter.as_deref()) {
        Ok(v) => v,
        Err(err) => return reject_agent_ops_config(&err, args.quiet),
    };
    let sort_spec = match output::parse_sort_opt(config_sort.as_deref()) {
        Ok(v) => v,
        Err(err) => return reject_agent_ops_config(&err, args.quiet),
    };
    let dedupe_by = match output::parse_dedupe_opt(config_dedupe.as_deref()) {
        Ok(v) => v,
        Err(err) => return reject_agent_ops_config(&err, args.quiet),
    };

    // GAP-WS-113 / GAP-E2E-V14-ALLOW-LITE-SILENT-NOOP: legacy no-op. Never force Lite.
    // Agent honesty: stderr warning (unless -q) + metadata flags_ignored on emit.
    if config.allow_lite_fallback {
        tracing::warn!(
            "GAP-WS-113: --allow-lite-fallback is ignored (legacy no-op); SERP stays HTML Chrome-only"
        );
        if !args.quiet {
            output::emit_stderr(
                "Warning: --allow-lite-fallback is a legacy no-op (Chrome-only SERP since v0.9.4); \
                 it does not remediate blocks. See flags_ignored in metadata.",
            );
        }
        config.endpoint = crate::types::Endpoint::Html;
    }

    // Wrap the pipeline in `tokio::time::timeout` — if it expires, cancel everything
    // and return exit code 4 (TIMEOUT_GLOBAL).
    let internal_cancellation = cancellation.clone();
    let pipeline_future = pipeline::execute_pipeline(config, internal_cancellation);

    let pipeline_result = match tokio::time::timeout(global_timeout, pipeline_future).await {
        Ok(result) => result,
        Err(_elapsed) => {
            // Propagate cancellation to any task still in-flight (one-shot reap).
            cancellation.cancel();
            // GAP-WS-TMP-PROFILE-ORPHAN-001: timeout drops the pipeline future;
            // content_fetch may not reach async shutdown. Force process+disk reap.
            #[cfg(feature = "chrome")]
            crate::process_lifecycle::ensure_oneshot_cleanup();
            let secs = global_timeout.as_secs();
            tracing::error!(
                seconds = secs,
                "global timeout exceeded — execution aborted"
            );
            // GAP-WS-EXIT4-JSON-001 v0.9.9: agent contract — always emit JSON on stdout
            // for -f json / pipe (auto→json when not TTY). Keep human stderr line.
            if !args.quiet {
                output::emit_stderr(i18n::global_timeout_exceeded(secs));
            }
            let q = args
                .queries
                .first()
                .cloned()
                .unwrap_or_else(|| "(timeout)".to_string());
            let timed_out = crate::types::SearchOutput {
                query: q,
                engine: "duckduckgo".into(),
                endpoint: "html".into(),
                timestamp: crate::types::utc_now(),
                region: format!("{}-{}", args.country, args.language),
                result_count: 0,
                results: vec![],
                pages_fetched: 0,
                news: None,
                news_count: None,
                error: Some(crate::error::codes::TIMEOUT.to_string()),
                message: Some(format!("global timeout of {secs}s exceeded")),
                metadata: crate::types::SearchMetadata {
                    execution_time_ms: secs.saturating_mul(1000),
                    selectors_hash: String::new(),
                    retries: 0,
                    retries_configured: None,
                    used_fallback_endpoint: false,
                    concurrent_fetches: 0,
                    fetch_successes: 0,
                    fetch_failures: 0,
                    used_chrome: false,
                    chrome_attempted: true,
                    user_agent: String::new(),
                    identity_used: None,
                    cascade_level: None,
                    used_proxy: args.proxy.is_some(),
                    pre_flight_fired: false,
                    pre_flight_executed: pre_flight,
                    pre_flight_status: if pre_flight {
                        Some("skipped".into())
                    } else {
                        None
                    },
                    news_promo_filtered: None,
                    stream_requested: if args.stream_mode || args.format.enables_stream_mode() {
                        Some(true)
                    } else {
                        None
                    },
                    stream_effective: if args.stream_mode || args.format.enables_stream_mode() {
                        Some(false)
                    } else {
                        None
                    },
                    zero_cause: None,
                    next_action_suggestion: Some(
                        "Raise --global-timeout (default 180s since v0.9.9) or use --vertical web --no-fetch-content for a thinner path."
                            .into(),
                    ),
                    bytes_raw: None,
                    bytes_decompressed: None,
                    cascade_level_observed: None,
                    result_count_compat: Some(0),
                    endpoint_used_compat: Some("html".into()),
                    vertical_used: None,
                    chrome_path_resolved: None,
                    chrome_channel: None,
                    run_id: Some(crate::types::RunId::generate()),
                    flags_ignored: None,
                },
            };
            // Prefer JSON for agent pipelines (explicit json, auto, or output file).
            let emit_json = matches!(
                format,
                crate::types::OutputFormat::Json | crate::types::OutputFormat::Auto
            ) || output_file.is_some();
            if emit_json {
                // v1.0.5: this used to serialize and `print_line_stdout` the
                // envelope directly, which meant `--fields` was honoured on a
                // search that SUCCEEDED and silently dropped on one that timed
                // out. Same invocation, same flag, two behaviours decided by
                // whether the network was fast enough. Routing through the same
                // emit as the success path removes the fork.
                let _ = output::emit_result_with_fields_async(
                    &crate::pipeline::PipelineResult::Single(Box::new(timed_out)),
                    crate::types::OutputFormat::Json,
                    output_file.as_deref(),
                    field_set.clone(),
                )
                .await;
            } else if let Err(e) = output::emit_result_async(
                &crate::pipeline::PipelineResult::Single(Box::new(timed_out)),
                format,
                output_file.as_deref(),
            )
            .await
            {
                let _ = e;
            }
            return exit_codes::GLOBAL_TIMEOUT;
        }
    };

    match pipeline_result {
        Ok(mut output) => {
            // GAP-WS-092 + GAP-WS-093: populate compat fields before emission.
            output.fill_compat_fields();

            // GAP-FIELDS-PROJECT / GAP-RESULT-FILTER / --limit: binary-side reduce
            // using pre-pipeline validated FieldSet / ResultFilter (no re-parse).
            let had_filter = result_filter.is_some();
            // Order: filter → sort → dedupe → project → limit → truncate-content
            let pre_filter_count = output::apply_project_filter(
                &mut output,
                None, // project after sort/dedupe
                result_filter.as_ref(),
            );
            output::apply_sort_pipeline(&mut output, sort_spec.as_ref());
            output::apply_dedupe_pipeline(&mut output, dedupe_by);
            let _ = output::apply_project_filter(&mut output, field_set.as_ref(), None);
            output::apply_result_limit(&mut output, config_limit);
            output::apply_truncate_pipeline(&mut output, config_truncate.map(|n| n as usize));
            if had_filter && pre_filter_count > 0 && output.total_results() == 0 {
                output::mark_filter_empty(&mut output);
            }

            // B2 fix: surface anti-bot (pre_flight_blocked) as exit 3
            // instead of exit 5 (zero results). The payload still travels
            // through `emit_result` so consumers see a single, well-formed
            // JSON object — the exit code is the only thing that changes.
            //
            // PipelineResult has 3 variants: Single(SearchOutput),
            // Multi(MultiSearchOutput), and Stream(StreamStats). We
            // inspect the inner SearchOutput / MultiSearchOutput for the
            // `error: "pre_flight_blocked"` marker when available.
            let pre_flight_blocked = match &output {
                crate::pipeline::PipelineResult::Single(s) => {
                    s.error.as_deref() == Some("pre_flight_blocked")
                }
                crate::pipeline::PipelineResult::Multi(m) => m
                    .searches
                    .iter()
                    .any(|b| b.error.as_deref() == Some("pre_flight_blocked")),
                crate::pipeline::PipelineResult::Stream(_) => false,
            };
            let total = output.total_results();

            // GAP-AUD-003 v0.8.0: causal classification of zero-result.
            // Stream variant returns false because stream emits incrementally
            // and the histogram per sub-query already carries the classification.
            let zero_cause_non_legitimo = match &output {
                crate::pipeline::PipelineResult::Single(s) => {
                    zero_cause_is_non_legitimate(s.metadata.zero_cause)
                }
                crate::pipeline::PipelineResult::Multi(m) => m
                    .searches
                    .iter()
                    .any(|b| zero_cause_is_non_legitimate(b.metadata.zero_cause)),
                crate::pipeline::PipelineResult::Stream(_) => false,
            };

            // BC opt-out: `--no-zero-cause-strict` maps exit 6 → exit 5 (GAP-SCRAPE-R2-012).
            // Default ON (strict). No product env.
            let strict = zero_cause_strict();

            // GAP-AUD-005 + GAP-AUD-006 v0.8.0: reorder exit-code logic.
            // BEFORE: pre_flight_blocked always exited with code 3, ignoring the
            // BC opt-out. AFTER: pre_flight_blocked && !strict → exit 5
            // (legacy); pre_flight_blocked && strict → exit 3 (RATE_LIMITED).
            // This keeps legacy retry pipelines working while the opt-out is
            // active, even when pre-flight fires.
            // GAP-WS-113 / Pass 43 / V12: Chrome transport / config failures must not look like empty index.
            // DRY: `error::chrome_classify::is_chrome_or_config_wire` + free-text fallback for mislabels.
            let chrome_transport_config_error = match &output {
                crate::pipeline::PipelineResult::Single(s) => {
                    crate::error::is_chrome_or_config_wire(s.error.as_deref())
                        || s.error.as_deref().is_some_and(
                            crate::error::chrome_classify::message_implies_chrome_or_config,
                        )
                }
                crate::pipeline::PipelineResult::Multi(m) => m.searches.iter().any(|b| {
                    crate::error::is_chrome_or_config_wire(b.error.as_deref())
                        || b.error.as_deref().is_some_and(
                            crate::error::chrome_classify::message_implies_chrome_or_config,
                        )
                }),
                crate::pipeline::PipelineResult::Stream(_) => false,
            };

            let exit_code = if chrome_transport_config_error {
                tracing::warn!(
                    "Chrome transport/config failure (GAP-WS-113/V12); emitting exit 2 (INVALID_CONFIG)"
                );
                exit_codes::INVALID_CONFIG
            } else if pre_flight_blocked && !strict {
                tracing::warn!(
                    "pre-flight detected anti-bot block + BC opt-out; emitting exit 5 (ZERO_RESULTS)"
                );
                exit_codes::ZERO_RESULTS
            } else if pre_flight_blocked {
                tracing::warn!("pre-flight detected anti-bot block; emitting exit 3");
                exit_codes::RATE_LIMITED_OR_BLOCKED
            } else if total == 0 && strict && zero_cause_non_legitimo {
                tracing::warn!(
                    "Zero results with non-legitimo causa_zero; emitting exit 6 (SUSPECTED_BLOCK)"
                );
                tracing::warn!("  opt-out via --no-zero-cause-strict to restore exit 5");
                exit_codes::SUSPECTED_BLOCK
            } else if total == 0 {
                tracing::warn!("Zero results returned across all queries");
                exit_codes::ZERO_RESULTS
            } else {
                exit_codes::SUCCESS
            };

            // GAP-PAR-040a: format/serde off the Tokio worker after multi-process SERP.
            if config_count_only {
                let payload = output::count_only_pipeline(&output);
                if let Err(err) = output::enforce_max_output_bytes(
                    &payload,
                    config_max_output_bytes.map(|n| n as usize),
                ) {
                    if args.quiet {
                        let p = crate::types::ThinErrorResponse::new(
                            "invalid_config",
                            format!("{err}"),
                        );
                        let _ = output::emit_wire_line(&p);
                    } else {
                        output::emit_stderr(err.localized_detail());
                    }
                    #[cfg(feature = "chrome")]
                    crate::process_lifecycle::ensure_oneshot_cleanup();
                    return exit_codes::INVALID_CONFIG;
                }
                if let Err(err) = output::emit_payload(&payload, output_file.as_deref()) {
                    if output::is_broken_pipe(&err) {
                        #[cfg(feature = "chrome")]
                        crate::process_lifecycle::ensure_oneshot_cleanup();
                        return exit_codes::BROKEN_PIPE;
                    }
                    output::emit_stderr(err.localized_detail());
                    #[cfg(feature = "chrome")]
                    crate::process_lifecycle::ensure_oneshot_cleanup();
                    return exit_codes::GENERIC_ERROR;
                }
                #[cfg(feature = "chrome")]
                crate::process_lifecycle::ensure_oneshot_cleanup();
                return exit_code;
            }
            if let Err(err) = output::emit_result_with_fields_async(
                &output,
                format,
                output_file.as_deref(),
                field_set,
            )
            .await
            {
                if output::is_broken_pipe(&err) {
                    // Pipe closed by consumer (e.g. `| jaq`, `| head`).
                    // rules-rust-cli-stdin-stdout: exit 141 (128+SIGPIPE).
                    #[cfg(feature = "chrome")]
                    crate::process_lifecycle::ensure_oneshot_cleanup();
                    return exit_codes::BROKEN_PIPE;
                }
                tracing::error!(?err, "Failed to emit result");
                output::emit_stderr(err.localized_detail());
                #[cfg(feature = "chrome")]
                crate::process_lifecycle::ensure_oneshot_cleanup();
                return exit_codes::GENERIC_ERROR;
            }

            #[cfg(feature = "chrome")]
            crate::process_lifecycle::ensure_oneshot_cleanup();
            exit_code
        }
        Err(err) => {
            // Propagate the typed exit code (Cancelled → 130 SIGINT / 143 SIGTERM).
            // Never collapse every pipeline Err into GENERIC_ERROR (1).
            let code = crate::signals::exit_code_for_error(&err);
            tracing::error!(?err, exit = code, "Pipeline execution failed");
            output::emit_stderr(err.localized_detail());
            // GAP-WS-TMP-PROFILE-ORPHAN-001: cancel/error may leave sessions registered.
            #[cfg(feature = "chrome")]
            crate::process_lifecycle::ensure_oneshot_cleanup();
            code
        }
    }
}
