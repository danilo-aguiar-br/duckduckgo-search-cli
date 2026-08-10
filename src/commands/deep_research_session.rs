// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light assembly — reads two config files, builds one struct.
//! Transport gate and [`Config`] assembly for the `deep-research` subcommand.
//!
//! Split out of `commands::deep_research` in v1.0.3 (plan Fase 8). Everything
//! here answers one question: given a request that already passed validation and
//! the budget gate, what session does the fan-out run with?
//!
//! # Why the Chrome gate lives here and not in preflight
//!
//! Order is deliberate and CM-01b depends on it. The budget refusal must be
//! reachable on a host with no Chrome binary, so `require_chrome_transport`
//! cannot run before the budget stage. It is the first thing this module does
//! instead, because a session is exactly what cannot be built without Chrome.

use crate::cli::{CliArgs, CliIdentityProfile};
use crate::error::exit_codes;
use crate::http;
use crate::output;
use crate::selectors;
use crate::types::{Config, Endpoint, OutputFormat, SafeSearch, VerticalMode};
use std::path::PathBuf;

/// Inputs the session needs from the earlier stages, grouped to keep the
/// builder's signature readable rather than a wall of positional arguments.
pub(super) struct SessionRequest<'a> {
    /// Query after the trust boundary.
    pub(super) validated_query: &'a crate::security::ValidatedQuery,
    /// Domain args, used for the news vertical and fetch decisions.
    pub(super) dr: &'a crate::deep_research::DeepResearchArgs,
    /// Root parser flags shared with `buscar`.
    pub(super) search_defaults: &'a CliArgs,
    /// Global `-o/--output` target, already validated.
    pub(super) output_file: Option<PathBuf>,
    /// Requested global timeout, before any contention raise.
    pub(super) root_global_timeout_seconds: u64,
    /// `--allow-lite-fallback` (no-op in production; see `chrome_policy`).
    pub(super) allow_lite_fallback: bool,
    /// `--pre-flight`.
    pub(super) pre_flight: bool,
    /// Resolved `--identity-profile`.
    pub(super) identity_profile: CliIdentityProfile,
}

/// Enforce the Chrome transport gate and build the run [`Config`].
///
/// # Emission contract
///
/// On failure this function has ALREADY written its own envelope — a thin error
/// on stdout when Chrome is unavailable, a stderr line for a bad proxy or an
/// out-of-range numeric. The caller must return the carried exit code verbatim.
///
/// # Errors
///
/// Returns the exit code to propagate when Chrome is required but absent, when
/// `--proxy` / `--no-proxy` do not resolve, or when any bounded newtype rejects
/// a root flag value.
pub(super) fn build(req: SessionRequest<'_>) -> Result<Config, i32> {
    let SessionRequest {
        validated_query,
        dr,
        search_defaults,
        output_file,
        root_global_timeout_seconds,
        allow_lite_fallback,
        pre_flight,
        identity_profile,
    } = req;

    // GAP-WS-113: fail closed after budget gate (CM-01b) — no auto --no-news.
    if let Err(e) = crate::chrome_policy::require_chrome_transport() {
        if !crate::chrome_policy::http_test_harness_active() {
            let payload = crate::types::ThinErrorResponse::new(e.error_code(), format!("{e}"))
                .with_suggestion("Chrome is required for deep-research (GAP-WS-113).");
            let _ = output::emit_wire_line(&payload);
            return Err(e.exit_code());
        }
    }

    let ua_list = http::load_user_agents(search_defaults.match_platform_ua);
    let browser_profile = http::select_profile_from_list_seeded(&ua_list, search_defaults.seed);
    let user_agent = browser_profile.user_agent.clone();
    let selectors = selectors::load_selectors();
    let effective_num = search_defaults.num_results.unwrap_or(15);
    let proxy_config = match crate::http::ProxyConfig::try_from_options(
        search_defaults.proxy.as_deref(),
        search_defaults.no_proxy,
    ) {
        Ok(p) => p,
        Err(e) => {
            output::emit_stderr(e.to_string());
            return Err(exit_codes::INVALID_CONFIG);
        }
    };

    // Every field below can reject its input, so the whole assembly is one
    // fallible closure and a single `?` chain instead of per-field matches.
    let built = (|| -> Result<Config, crate::error::CliError> {
        Ok(Config {
            query: validated_query.clone(),
            queries: vec![validated_query.clone()],
            num_results: Some(crate::types::ResultCount::try_new(effective_num)?),
            format: OutputFormat::Json,
            timeout_seconds: crate::types::TimeoutSeconds::try_new(
                search_defaults.timeout_seconds,
            )?,
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
            fetch_content: resolve_fetch_content(dr, search_defaults),
            fetch_content_cap: search_defaults.fetch_content_cap,
            agent_ops: crate::output::AgentOps::from_group(&search_defaults.agent),
            max_output_bytes: search_defaults.agent.max_output_bytes,
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
            #[cfg(feature = "http-test-harness")]
            cookie_provider: None,
            #[cfg(feature = "http-test-harness")]
            persistent_jar: None,
            // Residual HTTP warm-up only (GAP-WS-113). Production SERP warm-up is
            // Chrome extract `page.goto(serp_origin)` + news session prime — always on.
            warmup_enabled: false,
            allow_lite_fallback,
            identity_profile,
            last_probe_cascade_level: None,
            shared_session_verticals: search_defaults.shared_session_verticals,
        })
    })();

    let config = match built {
        Ok(c) => c,
        Err(e) => {
            output::emit_stderr(e.to_string());
            return Err(exit_codes::INVALID_CONFIG);
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

    Ok(config)
}

/// Decide whether page bodies are fetched for this run.
///
/// Anti-token rule: when `--fields` is set and none of the requested keys is a
/// content key, fetching bodies would burn a request per result for output that
/// gets projected away. A parse failure here is impossible in practice because
/// preflight already rejected unknown tokens; if it somehow happens, the
/// conservative answer is to keep the caller's `--fetch-content` intent.
fn resolve_fetch_content(
    dr: &crate::deep_research::DeepResearchArgs,
    search_defaults: &CliArgs,
) -> bool {
    let mut fetch = dr.fetch_content;
    let fields_raw = search_defaults.agent.fields_or_select();
    if let Some(raw) = fields_raw.as_deref() {
        if let Ok(fs) = output::FieldSet::parse(raw) {
            if !fs.requests_content() {
                fetch = false;
            }
        }
    }
    fetch
}
