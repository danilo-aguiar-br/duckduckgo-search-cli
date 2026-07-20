// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: orchestrator (config assembly, delegation to pipeline)
// html_root_url requires a string literal (no env!/concat! in this attr).
// Keep in sync with package.version in Cargo.toml (docs.rs deep links).
#![doc(html_root_url = "https://docs.rs/duckduckgo-search-cli/1.0.1")]
#![doc(html_playground_url = "https://play.rust-lang.org")]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::private_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::invalid_codeblock_attributes)]
#![warn(rustdoc::invalid_html_tags)]
#![warn(rustdoc::bare_urls)]
#![warn(rustdoc::redundant_explicit_links)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::multiple_unsafe_ops_per_block)]
#![deny(unsafe_op_in_unsafe_fn)]
// Cannot `forbid(unsafe_code)`: platform FFI (Windows console, libc kill/pre_exec,
// SIGPIPE) requires minimal documented `unsafe`. Density inventory (Pass 44):
// - `signals::restore_sigpipe` — `libc::signal(SIGPIPE, SIG_DFL)` (Unix)
// - `platform::init` — Win32 console UTF-8 / VTP (Windows)
// - `process_lifecycle::unix` — `libc::kill` / `pre_exec` setpgid+prctl (Unix)
// - `process_lifecycle::windows` — OpenProcess/Terminate/Toolhelp (Windows)
// Zero transmute/from_raw/static mut/union. See gaps.md §AP.
//! # duckduckgo-search-cli
//!
//! Rust CLI for searching `DuckDuckGo` via real Chrome (`chromiumoxide`/CDP), with structured JSON
//! output for LLM agents. No paid API. Production network is Chrome-only (GAP-WS-113).
//! No cache. Universal cross-platform (Linux including Alpine/NixOS/Flatpak/Snap,
//! macOS including Apple Silicon, Windows including cmd.exe and `PowerShell`).
//!
//! ## Module Structure
//!
//! | Module        | Responsibility                                               |
//! |---------------|--------------------------------------------------------------|
//! | [`cli`]       | Clap structs (command-line argument parsing).                |
//! | [`http`]      | `reqwest::Client` construction and User-Agent selection.  |
//! | [`search`]    | URL building and HTTP request to the `DuckDuckGo` endpoint.    |
//! | [`extraction`]| HTML parsing with `scraper` and ad filtering.                |
//! | [`pipeline`]  | Single/multi orchestration, deduplication and source reading.|
//! | [`parallel`]  | Multi-query fan-out with `JoinSet`, Semaphore, `CancellationToken`.|
//! | [`concurrency`] | Bounded-concurrency policy (`--parallel` / `--max-concurrency`).|
//! | [`output`]    | JSON/stdout payload + human stderr via [`output::emit_stderr`] (MP-06).|
//! | [`platform`]  | Cross-platform initialization (UTF-8 on Windows, TTY detect).|
//! | [`types`]     | Shared structs and enums.                                    |
//! | [`error`]     | Error codes and exit codes (`is_retryable` for agents).      |
//! | [`retry`]     | Named `RetryConfig`, full-jitter backoff, `Retry-After`.     |
//! | [`security`]  | Threat model + STRIDE + [`ValidatedQuery`] boundary.           |
//! | [`content`]   | SSRF + encoding + readability; residual HTTP for harness. |
//! | [`content_fetch`] | Parallel `--fetch-content` (Chrome pool + Semaphore). |
//! | [`selectors`] | Loading of external `SelectorConfig` (iter. 6).      |
//! | [`signals`]   | Cross-platform signal handlers (SIGPIPE, Ctrl+C).            |
//! | [`config_init`] | `init-config` subcommand (iter. 6).                       |
//! | [`paths`]     | Path validation and sanitization for I/O.                    |
//! | [`logging`] | Local stderr logging (developer diagnostics; no product telemetry).    |
//! | [`i18n`]      | UI locale (`en`/`pt-BR`) for human stderr; stdout stays EN.  |
//! | `browser`     | Headless Chrome cross-platform under feature `chrome` (iter.7).|
//! | [`chrome_policy`] | Always-on GAP-WS-113 Chrome-only transport policy.        |
//!
//! ## Features
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `chrome` | **yes** | Production network transport via real Chrome (`chromiumoxide`/CDP). Required for SERP, news, deep-research, probe, pre-flight, and content fetch. |
//! | `http-test-harness` | no | Residual HTTP SERP/probe paths for tests only (`DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`). Never a silent production SERP path. |
//! | `console` | no | Composes `tokio-console` with the stderr fmt layer (`RUSTFLAGS=--cfg tokio_unstable cargo run --features console`). |
//!
//! Feature-gated items are described in prose (not `#[doc(cfg(...))]`) so docs build on
//! **stable** and on docs.rs without nightly `#![feature(doc_cfg)]` (Oct 2025 `doc_auto_cfg` → `doc_cfg` merge).
//!
//! ## Scraping / robots.txt policy (Pass 45)
//!
//! This CLI **does not** fetch or honor `robots.txt` (operator product mandate). It is a
//! one-shot DuckDuckGo search client with optional page enrichment for agents — not a
//! site-wide polite crawler. Outbound load is bounded by Semaphore, per-host limits,
//! stagger jitter, circuit breaker, and HTTP `Retry-After` — not by REP Crawl-delay.
//! Attacker-influenced SERP URLs still pass the shared SSRF gate before HTTP or Chrome
//! navigation. See `gaps.md` §AQ / N/A-SCRAPE-001.
//!
//! ## Entry Point
//!
//! The public function [`run`] is called by `main.rs` and returns an exit code
//! as specified in section 17.7 of the specification.

pub mod aggregation;
pub mod chrome_policy;
pub mod cli;
pub mod concurrency;
pub mod commands;
pub mod config;
pub mod config_init;
pub mod content;
pub mod content_fetch;
pub mod cookie_adapter;
pub mod decomposition;
pub mod decompress;
pub mod deep_research;
pub mod endpoints;
pub mod error;
pub mod extraction;
pub mod http;
pub mod i18n;
pub mod identity;
pub mod output;
pub mod parallel;
pub mod paths;
pub mod pipeline;
pub mod platform;
pub mod probe_deep;
pub mod zero_cause;
pub mod probe;
pub mod retry;
pub mod search;
pub mod security;
pub mod selectors;
pub mod session_warmup;
pub mod signals;
pub mod synthesis;
pub mod logging;
pub(crate) use logging::initialize_logging_for_command;
/// Process-wide rustls CryptoProvider install (binary `main` + tests).
pub mod tls_bootstrap;
pub mod types;
pub mod validation;

// browser.rs declares `#![cfg(feature = "chrome")]` at the module root (line 25),
// which already excludes the entire module when the feature is off. Re-declaring
// `#[cfg(feature = "chrome")]` here is redundant and triggers clippy::duplicated_attributes.
// The previous `#[cfg_attr(docsrs, doc(cfg(...)))]` was removed in v0.6.6 because
// `doc(cfg)` is unstable and requires `#![feature(doc_cfg)]` since doc_auto_cfg
// was merged into doc_cfg in Oct 2025 (see rust-lang/rust#43781).
//
// Transport policy (require_chrome_transport / http_test_harness_active) lives in
// `chrome_policy` and is always compiled — see GAP-WS-113 / no-default-features CI.
pub mod browser;

// GAP-WS-LIFECYCLE-001: process-group / tree reap helpers (feature-gated like browser).
#[cfg(feature = "chrome")]
pub mod process_lifecycle;

// Long calibration query for probe-deep (GAP-WS-51).
//
// DuckDuckGo trata queries curtas e longas de forma diferente: queries
// de 1 palavra raramente acionam o sistema de bot detection, fazendo
// com que `--probe-deep` retorne "ok" mesmo quando uma query real de
// production would be blocked. This 43-character string ensures that
// the HTTP payload has a realistic size, replicating the real scenario.

use crate::cli::{
    CliArgs, CliEndpoint, CliSafeSearch, CliTimeFilter, CliVertical, RootArgs, Subcommand,
};
use crate::commands::{
    execute_commands, execute_completions, execute_config, execute_deep_research, execute_doctor,
    execute_man,
    execute_init_config, execute_locale, execute_schema,
};
use crate::error::exit_codes;
use crate::error::CliError;
use crate::types::{Config, Endpoint, OutputFormat, SafeSearch, TimeFilter, VerticalMode};
use clap::Parser;
use tokio_util::sync::CancellationToken;

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
                // Prints to stdout and exits 0 — preserves `--help` / `--version`.
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
                        msg.push_str(&i18n::flag_must_precede_subcommand(&raw));
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
    let xdg = crate::commands::config_cmd::load_runtime_user_config();
    crate::commands::config_cmd::apply_user_config_to_root(&mut root, &xdg);

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

    // i18n Phases 3–7: detect / negotiate / publish OnceLock before any
    // translated human stderr from subcommands.
    i18n::initialize(root.ui_lang.as_deref());

    // Initialize logging BEFORE subcommand dispatch so deep-research
    // respects -q/--verbose (stderr only; stdout remains agent payload).
    // GAP-LOG-ENV-001: XDG `log_directive` replaces product RUST_LOG.
    let disable_colors = platform::should_disable_color(root.buscar.no_color);
    logging::initialize_logging(
        root.buscar.verbose,
        root.buscar.quiet,
        disable_colors,
        xdg.log_directive(),
    );

    let args = match root.subcommand {
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
        Some(Subcommand::Doctor(args)) => {
            return execute_doctor(args);
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
            let search_defaults = root.buscar;
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
        None => root.buscar,
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

    // Convert CliArgs into internal Config.
    let mut config = match build_config(&args) {
        Ok(c) => c,
        Err(err) => {
            // GAP-WS-QUIET-CONFIG-001: with -q, avoid tracing/stderr noise; prefer JSON stdout.
            if args.quiet {
                let payload = serde_json::json!({
                    "erro": "invalid_config",
                    "mensagem": format!("{err}"),
                    "quantidade_resultados": 0,
                    "resultados": [],
                });
                let _ = output::print_line_stdout(&payload.to_string());
            } else {
                tracing::error!(?err, "Invalid configuration");
                output::emit_stderr(i18n::configuration_error(&err));
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
    config.global_timeout_seconds = match crate::types::GlobalTimeoutSeconds::try_new(root_global_timeout_seconds) {
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
    let global_timeout = std::time::Duration::from_secs(config.global_timeout_seconds.get());

    // GAP-WS-113: --allow-lite-fallback is a legacy no-op. Never force Lite.
    if config.allow_lite_fallback {
        tracing::warn!(
            "GAP-WS-113: --allow-lite-fallback is ignored (legacy no-op); SERP stays HTML Chrome-only"
        );
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
                },
            };
            // Prefer JSON for agent pipelines (explicit json, auto, or output file).
            let emit_json = matches!(
                format,
                crate::types::OutputFormat::Json | crate::types::OutputFormat::Auto
            ) || output_file.is_some();
            if emit_json {
                // GAP-PAR-040c: serialize timeout envelope off the async worker.
                if let Ok(json) = output::serialize_json_async(timed_out.clone()).await {
                    let _ = output::print_line_stdout(&json);
                }
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
            // BC opt-out. DEPOIS: pre_flight_blocked && !strict → exit 5
            // (legacy); pre_flight_blocked && strict → exit 3 (RATE_LIMITED).
            // Isso garante que pipelines de retry legacy continuam funcionando
            // quando opt-out ativo, mesmo quando pre-flight dispara.
            // GAP-WS-113 / Pass 43: Chrome transport / config failures must not look like empty index.
            // Compare against stable `error::codes` (same strings `CliError::error_code` emits).
            let is_chrome_or_config_wire = |code: Option<&str>| -> bool {
                matches!(
                    code,
                    Some(crate::error::codes::INVALID_CONFIG)
                        | Some(crate::error::codes::CHROME_UNAVAILABLE)
                        | Some(crate::error::codes::CHROME_DISABLED_BY_ENV)
                        | Some(crate::error::codes::CHROME_NOT_FOUND)
                )
            };
            let chrome_transport_config_error = match &output {
                crate::pipeline::PipelineResult::Single(s) => {
                    is_chrome_or_config_wire(s.error.as_deref())
                }
                crate::pipeline::PipelineResult::Multi(m) => m
                    .searches
                    .iter()
                    .any(|b| is_chrome_or_config_wire(b.error.as_deref())),
                crate::pipeline::PipelineResult::Stream(_) => false,
            };

            let exit_code = if chrome_transport_config_error {
                tracing::warn!(
                    "Chrome transport/config failure (GAP-WS-113); emitting exit 2 (INVALID_CONFIG)"
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
            if let Err(err) =
                output::emit_result_async(&output, format, output_file.as_deref()).await
            {
                if output::is_broken_pipe(&err) {
                    // Pipe closed by consumer (e.g. `| jaq`, `| head`).
                    // rules-rust-cli-stdin-stdout: exit 141 (128+SIGPIPE).
                    #[cfg(feature = "chrome")]
                    crate::process_lifecycle::ensure_oneshot_cleanup();
                    return exit_codes::BROKEN_PIPE;
                }
                tracing::error!(?err, "Failed to emit result");
                output::emit_stderr(i18n::generic_error(&err));
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
            output::emit_stderr(i18n::generic_error(&err));
            // GAP-WS-TMP-PROFILE-ORPHAN-001: cancel/error may leave sessions registered.
            #[cfg(feature = "chrome")]
            crate::process_lifecycle::ensure_oneshot_cleanup();
            code
        }
    }
}

/// Executes the v0.6.4 --probe pre-flight health check.
///
/// Sends ONE minimal GET request to the configured endpoint and emits a
/// JSON report on stdout with `status`, `latency_ms`, `has_set_cookie`,
/// `endpoint`, and `identity` fields. Exits 0 if the request succeeded
/// (any HTTP status, including 202/403/429 — the probe reports but does
/// not retry), 1 if the network/TLS/DNS layer failed.
///
/// GAP-WS-113: health probe via real Chrome navigation (no reqwest 200 lies).
#[cfg(feature = "chrome")]
fn build_config(args: &CliArgs) -> Result<Config, CliError> {
    // Strong clap ValueEnum → domain enum (invalid values rejected at parse time).
    // GAP-E2E-51-005: `-f ndjson` maps to JSON format + stream mode.
    let format: OutputFormat = args.format.into();
    let stream_mode = args.stream_mode || args.format.enables_stream_mode();

    args.validate_parallelism()?;
    args.validate_pages()?;
    args.validate_retries()?;
    args.validate_max_content_length()?;
    // v0.7.10 B3 fix: `global_timeout_seconds` validation happens on
    // `RootArgs` in `run()`. The unit tests that call `build_config`
    // directly bypass `run`, so they exercise the default
    // `DEFAULT_GLOBAL_TIMEOUT` which always validates as `Ok`.
    let _ = crate::cli::DEFAULT_GLOBAL_TIMEOUT;
    args.validate_proxy()?;
    args.validate_per_host_limit()?;
    args.validate_timeout_seconds()?;
    if let Some(path) = &args.output_file {
        crate::paths::validate_output_path(path)?;
    }
    // Cookie jar is session credentials — same write-path policy as `-o`.
    if let Some(path) = &args.cookies_path {
        crate::paths::validate_output_path(path)?;
    }

    let file_queries = match &args.queries_file {
        Some(path) => pipeline::read_queries_from_file(path)?,
        None => Vec::new(),
    };

    let queries_stdin = if args.queries.is_empty() && args.queries_file.is_none() {
        pipeline::read_queries_from_stdin_if_pipe()?
    } else {
        Vec::new()
    };

    let raw_queries =
        pipeline::combine_and_dedup_queries(args.queries.clone(), file_queries, queries_stdin);

    // Trust-boundary: ValidatedQuery (NFC + charset) → cleaned, re-deduped list.
    let queries = crate::security::validate_query_list(&raw_queries)?;

    // GAP-WS-113: news|all require Chrome — fail closed, never silently downgrade.
    if args.vertical != CliVertical::Web {
        if let Err(e) = crate::chrome_policy::require_chrome_transport() {
            if !crate::chrome_policy::http_test_harness_active() {
                return Err(e);
            }
        }
    }
    let vertical = convert_vertical(args.vertical);

    let first_query = queries[0].clone();

    // Load UA list — tries external file, falls back to embedded defaults.
    let ua_list = http::load_user_agents(args.match_platform_ua);
    let browser_profile = http::select_profile_from_list_seeded(&ua_list, args.seed);
    // Config keeps both the profile and a denormalized UA string — one field clone.
    let user_agent = browser_profile.user_agent.clone();

    // Load CSS selectors — tries external TOML file, falls back to embedded defaults.
    // --config overrides the default config directory.
    let selectors = if let Some(ref dir) = args.config_path {
        selectors::load_selectors_from_dir(dir)
    } else {
        selectors::load_selectors()
    };

    // --- Default for --num and auto-pagination (v0.4.0) ---
    //
    // Semantics (decided in v0.4.0):
    // - If the user does NOT pass `--num`, we use 15 as the effective default.
    // - If the effective `num` is > 10 and the user did NOT customize `--pages`
    //   (i.e., `paginas == 1`, which is the clap default), we auto-raise
    //   `paginas` to `ceil(num/10)`, capped at 5 (MAX_PAGES
    //   validated in `validar_paginas`).
    // - If the user passes `--pages > 1` explicitly, we RESPECT that value
    //   without overriding (edge case: `--pages 1` explicit is
    //   indistinguishable from the default; accepted trade-off).
    let effective_num = args.num_results.unwrap_or(15);
    let effective_pages = if args.pages > 1 {
        args.pages
    } else if effective_num > 10 {
        effective_num.div_ceil(10).min(5)
    } else {
        1
    };

    // v0.7.3 PR2: build the cookie jar / warm-up machinery.
    let (persistent_jar, warmup_enabled) = if args.no_cookie_persistence {
        (
            crate::cookie_adapter::PersistentJar::empty(None),
            !args.no_warmup,
        )
    } else {
        let path = match args.cookies_path.as_ref() {
            Some(p) => p.clone(),
            None => crate::cookie_adapter::default_cookies_path()?,
        };
        (
            crate::cookie_adapter::PersistentJar::load(Some(path)),
            !args.no_warmup,
        )
    };
    let cookie_provider = persistent_jar.as_provider();

    // Also honor CLI kill switch if build_config is used without run() install.
    if args.disable_retry {
        crate::retry::set_retry_disabled(true);
    }
    crate::endpoints::set_endpoint_policy(crate::endpoints::EndpointPolicy {
        html: args.base_url_html.clone(),
        lite: args.base_url_lite.clone(),
        serp: args.base_url_serp.clone(),
    });
    let retries_raw = if crate::retry::retry_disabled() {
        0
    } else {
        args.retries
    };
    let proxy_config = crate::http::ProxyConfig::try_from_options(
        args.proxy.as_deref(),
        args.no_proxy,
    )?;

    Ok(Config {
        query: first_query,
        queries,
        num_results: Some(crate::types::ResultCount::try_new(effective_num)?),
        format,
        timeout_seconds: crate::types::TimeoutSeconds::try_new(args.timeout_seconds)?,
        language: crate::types::SerpLanguage::try_new(&args.language)?,
        allow_lite_fallback: false,
        pre_flight: false,
        last_probe_cascade_level: None,
        country: crate::types::SerpCountry::try_new(&args.country)?,
        verbose: args.verbose,
        quiet: args.quiet,
        user_agent: crate::types::UserAgentString::try_new(&user_agent)?,
        browser_profile,
        parallelism: crate::types::ParallelismDegree::try_new(args.parallelism)?,
        pages: crate::types::PageCount::try_new(effective_pages)?,
        retries: crate::types::RetryBudget::try_new(retries_raw)?,
        endpoint: Endpoint::from(args.endpoint),
        vertical,
        time_filter: args.time_filter.map(TimeFilter::from),
        safe_search: SafeSearch::from(args.safe_search),
        stream_mode,
        output_file: args.output_file.clone(),
        fetch_content: !args.no_fetch_content,
        fetch_content_cap: args.fetch_content_cap,
        max_content_length: crate::types::ContentLengthLimit::try_new(args.max_content_length)?,
        proxy_config,
        // v0.7.10 B3 fix: `global_timeout_seconds` lives on `RootArgs`,
        // not on `CliArgs`. The caller (`run`) hoists the value and
        // overrides this field right after `build_config` returns.
        // The default below only runs in unit tests that bypass `run`.
        global_timeout_seconds: crate::types::GlobalTimeoutSeconds::try_new(
            crate::cli::DEFAULT_GLOBAL_TIMEOUT,
        )?,
        match_platform_ua: args.match_platform_ua,
        per_host_limit: crate::types::PerHostLimit::try_new(args.per_host_limit)?,
        chrome_path: args.chrome_path.clone(),
        chrome_force_visible: args.chrome_visible,
        chrome_force_headless: args.chrome_headless,
        chrome_force_xvfb: args.chrome_xvfb,
        dump_news_html: args.dump_news_html.clone(),
        selectors,
        cookie_provider: Some(cookie_provider),
        persistent_jar: Some(persistent_jar),
        warmup_enabled,
        identity_profile: args.identity_profile,
        shared_session_verticals: args.shared_session_verticals,
    })
}

/// Converts the `CliVertical` enum (clap) into the internal `VerticalMode` type.
/// GAP-WS-104 v0.8.9. v0.9.0 GAP-WS-106: only called from the `chrome`
/// branch of `build_config`; without the feature the call site is
/// cfg-removed, hence `allow(dead_code)` for the no-chrome build.
#[cfg_attr(not(feature = "chrome"), allow(dead_code))]
fn convert_vertical(source: CliVertical) -> VerticalMode {
    VerticalMode::from(source)
}

impl From<CliEndpoint> for Endpoint {
    fn from(source: CliEndpoint) -> Self {
        match source {
            CliEndpoint::Html => Endpoint::Html,
            CliEndpoint::Lite => Endpoint::Lite,
        }
    }
}

impl From<CliVertical> for VerticalMode {
    fn from(source: CliVertical) -> Self {
        match source {
            CliVertical::Web => VerticalMode::Web,
            CliVertical::News => VerticalMode::News,
            CliVertical::All => VerticalMode::All,
        }
    }
}

impl From<CliTimeFilter> for TimeFilter {
    fn from(source: CliTimeFilter) -> Self {
        match source {
            CliTimeFilter::D => TimeFilter::Day,
            CliTimeFilter::W => TimeFilter::Week,
            CliTimeFilter::M => TimeFilter::Month,
            CliTimeFilter::Y => TimeFilter::Year,
        }
    }
}

impl From<CliSafeSearch> for SafeSearch {
    fn from(source: CliSafeSearch) -> Self {
        match source {
            CliSafeSearch::Off => SafeSearch::Off,
            CliSafeSearch::Moderate => SafeSearch::Moderate,
            CliSafeSearch::On => SafeSearch::Strict,
        }
    }
}


/// Process-wide zero-cause strict mode (default true). GAP-SCRAPE-R2-012.
static ZERO_CAUSE_STRICT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Install CLI `--no-zero-cause-strict` policy (default: strict ON).
pub fn set_zero_cause_strict(strict: bool) {
    ZERO_CAUSE_STRICT.store(strict, std::sync::atomic::Ordering::SeqCst);
}

/// Whether non-legitimate zero-result causes emit exit 6.
#[must_use]
pub fn zero_cause_strict() -> bool {
    ZERO_CAUSE_STRICT.load(std::sync::atomic::Ordering::SeqCst)
}

/// GAP-AUD-003 + GAP-WS-104: zero-result causes that indicate suspected block
/// suspeito (exit 6 sob strict). `Legitimate` e `VerticalNoResults` ficam
/// OUTSIDE the list — are legitimate zeros (exit 5): the news vertical rendered
/// sem articles, sem sinal de interstitial anti-bot.
fn zero_cause_is_non_legitimate(cause: Option<crate::types::ZeroCause>) -> bool {
    matches!(
        cause,
        Some(crate::types::ZeroCause::GhostBlock)
            | Some(crate::types::ZeroCause::AntiBot)
            | Some(crate::types::ZeroCause::InvalidResponse)
            | Some(crate::types::ZeroCause::SilentFilter)
            | Some(crate::types::ZeroCause::SuspiciousZeroResults)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> CliArgs {
        CliArgs {
            queries: vec!["rust async".to_string()],
            num_results: Some(5),
            vertical: crate::cli::CliVertical::Web,
            format: crate::cli::CliOutputFormat::Json,
            output_file: None,
            timeout_seconds: 15,
            language: "pt".to_string(),
            country: "br".to_string(),
            parallelism: 5,
            shared_session_verticals: false,
            queries_file: None,
            pages: 1,
            retries: 2,
            disable_retry: false,
            base_url_html: None,
            base_url_lite: None,
            base_url_serp: None,
            endpoint: CliEndpoint::Html,
            time_filter: None,
            safe_search: CliSafeSearch::Moderate,
            probe: false,
            identity_profile: crate::cli::CliIdentityProfile::Auto,
            stream_mode: false,
            verbose: 0,
            quiet: false,
            fetch_content: false,
            no_fetch_content: true,
            fetch_content_cap: crate::cli::DEFAULT_FETCH_CONTENT_CAP,
            max_content_length: crate::cli::DEFAULT_MAX_CONTENT_LENGTH,
            proxy: None,
            no_proxy: false,
            // v0.7.10 B3 fix: `global_timeout_seconds` is no longer on
            // `CliArgs`; it lives on `RootArgs` and is hoisted in `run`.
            match_platform_ua: false,
            per_host_limit: crate::cli::DEFAULT_PER_HOST_LIMIT,
            chrome_path: None,
            chrome_visible: false,
            chrome_headless: false,
            chrome_xvfb: false,
            dump_news_html: None,
            no_color: false,
            seed: None,
            config_path: None,
            no_warmup: false,
            no_cookie_persistence: false,
            cookies_path: None,
            probe_deep: false,
        }
    }

    #[test]
    fn build_config_with_valid_args() {
        let args = base_args();
        let cfg = build_config(&args).expect("should build config");
        assert_eq!(cfg.query.as_str(), "rust async");
        assert_eq!(cfg.queries.len(), 1);
        assert_eq!(cfg.queries[0].as_str(), "rust async");
        assert_eq!(cfg.format, OutputFormat::Json);
        assert_eq!(cfg.num_results.map(|n| n.get()), Some(5));
        assert_eq!(cfg.parallelism.get(), 5);
        assert_eq!(cfg.pages.get(), 1);
        assert!(!cfg.stream_mode);
    }

    #[test]
    fn build_config_ndjson_format_enables_stream_mode() {
        // GAP-E2E-51-005: `-f ndjson` → domain JSON + stream_mode.
        let mut args = base_args();
        args.format = crate::cli::CliOutputFormat::Ndjson;
        args.stream_mode = false;
        args.queries = vec!["a".into(), "b".into()];
        let cfg = build_config(&args).expect("should build");
        assert_eq!(cfg.format, OutputFormat::Json);
        assert!(cfg.stream_mode);
    }

    // v0.7.10 GAP-WS-60 regression: `build_config` must propagate
    // `args.identity_profile` into `Config.identity_profile` so the
    // pipeline can pin to a fixed identity.
    #[test]
    fn build_config_propagates_identity_profile_default_auto() {
        let args = base_args();
        let cfg = build_config(&args).expect("should build config");
        assert_eq!(
            cfg.identity_profile,
            crate::cli::CliIdentityProfile::Auto,
            "default identity_profile must be Auto"
        );
    }

    #[test]
    fn build_config_propagates_identity_profile_chrome_linux() {
        let mut args = base_args();
        args.identity_profile = crate::cli::CliIdentityProfile::ChromeLinux;
        let cfg = build_config(&args).expect("should build config");
        assert_eq!(
            cfg.identity_profile,
            crate::cli::CliIdentityProfile::ChromeLinux,
            "ChromeLinux flag must reach Config"
        );
    }

    // GAP-WS-105 v0.8.9: multi-query + --vertical news e aceito — cada
    // query do batch roda sua propria sessao Chrome no fan-out.
    // v0.9.0 GAP-WS-106: so faz sentido com a feature `chrome` (sem ela,
    // build_config rebaixa para Web).
    #[cfg(feature = "chrome")]
    #[test]
    fn build_config_accepts_multi_query_with_news_vertical() {
        let mut args = base_args();
        args.vertical = crate::cli::CliVertical::News;
        args.queries = vec!["rust".to_string(), "tokio".to_string()];
        let cfg = build_config(&args).expect("multi-query + --vertical news must be accepted");
        assert_eq!(cfg.vertical, crate::types::VerticalMode::News);
        assert_eq!(cfg.queries.len(), 2);
    }

    // GAP-WS-104 v0.8.9: --vertical propaga para Config.vertical.
    // v0.9.0 GAP-WS-106: so faz sentido com a feature `chrome` (sem ela,
    // build_config rebaixa para Web).
    #[cfg(feature = "chrome")]
    #[test]
    fn build_config_propagates_vertical_all() {
        let mut args = base_args();
        args.vertical = crate::cli::CliVertical::All;
        let cfg = build_config(&args).expect("should build config");
        assert_eq!(cfg.vertical, crate::types::VerticalMode::All);
    }

    #[test]
    fn build_config_rejects_all_empty_queries() {
        let mut args = base_args();
        args.queries = vec!["   ".to_string(), "".to_string()];
        let result = build_config(&args);
        assert!(result.is_err());
    }

    #[test]
    fn clap_rejects_unknown_format_at_parse_time() {
        // Invalid formats are no longer a post-parse `build_config` concern:
        // `CliOutputFormat` ValueEnum rejects them with clap exit 2.
        use clap::Parser;
        let err = RootArgs::try_parse_from(["bin", "-f", "xml", "q"]);
        assert!(err.is_err(), "unknown -f value must fail clap parse");
    }

    #[test]
    fn build_config_rejects_zero_parallelism() {
        let mut args = base_args();
        args.parallelism = 0;
        assert!(build_config(&args).is_err());
    }

    #[test]
    fn build_config_rejects_parallelism_above_max() {
        let mut args = base_args();
        args.parallelism = 50;
        assert!(build_config(&args).is_err());
    }

    #[test]
    fn build_config_applies_default_num_15_when_omitted() {
        // v0.4.0: when `--num` is omitted (None), the effective default is 15
        // and this auto-raises `--pages` to 2 (since 15 > 10 and pages=1 is the default).
        let mut args = base_args();
        args.num_results = None;
        args.pages = 1;
        let cfg = build_config(&args).expect("should build");
        assert_eq!(cfg.num_results.map(|n| n.get()), Some(15), "default 15 quando None");
        assert_eq!(cfg.pages.get(), 2, "auto-eleva para ceil(15/10) = 2");
    }

    #[test]
    fn build_config_respects_explicit_pages_above_1() {
        // If the user passes `--pages 3` explicitly, do NOT override with
        // auto-pagination, even if the effective num would require fewer.
        let mut args = base_args();
        args.num_results = Some(20);
        args.pages = 3;
        let cfg = build_config(&args).expect("should build");
        assert_eq!(cfg.num_results.map(|n| n.get()), Some(20));
        assert_eq!(cfg.pages.get(), 3, "honors explicit user --pages");
    }

    #[test]
    fn build_config_auto_paginates_when_num_above_10() {
        // Casos de fronteira do auto-paginador.
        let casos = [
            (11u32, 2u32), // ceil(11/10) = 2
            (15, 2),       // ceil(15/10) = 2
            (20, 2),       // ceil(20/10) = 2
            (21, 3),       // ceil(21/10) = 3
            (45, 5),       // ceil(45/10) = 5
            (60, 5),       // ceil(60/10) = 6 mas clamp em 5
        ];
        for (num, expected_pages) in casos {
            let mut args = base_args();
            args.num_results = Some(num);
            args.pages = 1;
            let cfg =
                build_config(&args).unwrap_or_else(|e| panic!("should build for num={num}: {e}"));
            assert_eq!(
                cfg.pages.get(), expected_pages,
                "para num={num}, paginas deveria ser {expected_pages}"
            );
        }
    }

    #[test]
    fn build_config_no_auto_paginate_when_num_10_or_less() {
        // If effective num <= 10, keep paginas=1 (no auto-pagination).
        for num in [1u32, 5, 10] {
            let mut args = base_args();
            args.num_results = Some(num);
            args.pages = 1;
            let cfg = build_config(&args).expect("should build");
            assert_eq!(cfg.pages.get(), 1, "num={num} should not auto-paginate");
        }
    }

    #[test]
    fn build_config_combines_multiple_positional_queries() {
        let mut args = base_args();
        args.queries = vec![
            "alfa".to_string(),
            "beta".to_string(),
            "alfa".to_string(), // duplicata
            "gama".to_string(),
        ];
        let cfg = build_config(&args).expect("should build config");
        assert_eq!(
            cfg.queries.iter().map(|q| q.as_str()).collect::<Vec<_>>(),
            vec!["alfa", "beta", "gama"]
        );
        assert_eq!(cfg.query.as_str(), "alfa");
    }

    // --- GAP-WS-104 v0.8.9: VerticalNoResults is a LEGITIMATE zero (exit 5) ---

    #[test]
    fn vertical_sem_resultados_is_legitimo_zero() {
        assert!(!zero_cause_is_non_legitimate(Some(
            crate::types::ZeroCause::VerticalNoResults
        )));
        assert!(!zero_cause_is_non_legitimate(Some(
            crate::types::ZeroCause::Legitimate
        )));
        assert!(!zero_cause_is_non_legitimate(None));
    }

    #[test]
    fn blocking_zero_causes_remain_non_legitimo() {
        for cause in [
            crate::types::ZeroCause::GhostBlock,
            crate::types::ZeroCause::AntiBot,
            crate::types::ZeroCause::InvalidResponse,
            crate::types::ZeroCause::SilentFilter,
            crate::types::ZeroCause::SuspiciousZeroResults,
        ] {
            assert!(zero_cause_is_non_legitimate(Some(cause)), "{cause:?}");
        }
    }

}
