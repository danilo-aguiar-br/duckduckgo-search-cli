// SPDX-License-Identifier: MIT OR Apache-2.0
//! Build pipeline [`Config`] from effective CLI args (SSOT).
//!
//! Callers must already have applied XDG via [`super::apply`].
//! Factory seeds apply only when CLI left defaults (e.g. omitted `--num`).

use crate::cli::{CliArgs, CliEndpoint, CliSafeSearch, CliTimeFilter, CliVertical};
use crate::error::CliError;
use crate::http;
use crate::pipeline;
use crate::selectors;
use crate::types::{Config, Endpoint, OutputFormat, SafeSearch, TimeFilter, VerticalMode};

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

/// Converts the `CliVertical` enum (clap) into the internal `VerticalMode` type.
/// GAP-WS-104 v0.8.9. v0.9.0 GAP-WS-106: only called from the `chrome`
/// branch of `build_config`; without the feature the call site is
/// cfg-removed, hence `allow(dead_code)` for the no-chrome build.
#[cfg_attr(not(feature = "chrome"), allow(dead_code))]
fn convert_vertical(source: CliVertical) -> VerticalMode {
    VerticalMode::from(source)
}

#[cfg(feature = "chrome")]
/// Build pipeline [`Config`] from clap [`CliArgs`] after XDG apply.
///
/// Factory seeds (e.g. default `--num`) apply only when CLI left defaults.
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when the operation fails.
pub fn build_config(args: &CliArgs) -> Result<Config, CliError> {
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
    let effective_num = args.num_results.unwrap_or(super::factory::FACTORY_DEFAULT_NUM_RESULTS);
    let effective_pages = if args.pages > 1 {
        args.pages
    } else if effective_num > super::factory::FACTORY_SERP_PAGE_SIZE {
        effective_num.div_ceil(super::factory::FACTORY_SERP_PAGE_SIZE).min(super::factory::FACTORY_MAX_AUTO_PAGES)
    } else {
        1
    };

    // v0.7.3 PR2: build the cookie jar / warm-up machinery.
    // Anti-CF §6 / G23: --no-warmup is fail-closed outside lab harness.
    if args.no_warmup && !args.allow_no_warmup {
        // Emit once at the CLI boundary (`lib::run`); avoid double JSON on -q.
        return Err(crate::error::CliError::InvalidConfig {
            message: "--no-warmup is blocked for real SERP ops (warm duckduckgo.com first). Lab/harness only: pass --allow-no-warmup (hidden) or `config set allow_no_warmup true`".into(),
        });
    }

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
        fetch_content: {
            // GAP-FIELDS-PROJECT: when --fields/--select omits content keys, skip
            // page fetch (token + Chrome cost). Explicit content keys re-enable it.
            let mut fetch = !args.no_fetch_content;
            let fields_raw = args.fields.as_deref().or(args.select.as_deref());
            if let Some(raw) = fields_raw {
                let fs = crate::output::FieldSet::parse(raw)?;
                if !fs.requests_content() {
                    fetch = false;
                }
            }
            fetch
        },
        fetch_content_cap: args.fetch_content_cap,
        fields: args.fields.clone().or_else(|| args.select.clone()),
        result_filter: args.result_filter.clone(),
        result_limit: args.result_limit,
        sort: args.sort.clone(),
        dedupe_by: args.dedupe_by.clone(),
        count_only: args.count_only,
        truncate_content: args.truncate_content,
        max_output_bytes: args.max_output_bytes,
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
