// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (CLI parsing via clap derive, zero runtime).
//! Search / `buscar` clap types and enums (GAP-CLI-MOD-SPLIT).

use clap::{
    builder::ValueHint, ArgAction, Args,
};
use std::path::PathBuf;

use super::{
    DEFAULT_MAX_CONTENT_LENGTH, DEFAULT_PAGES, DEFAULT_PARALLELISM, DEFAULT_PER_HOST_LIMIT,
    DEFAULT_RETRIES, DEFAULT_SERP_COUNTRY, DEFAULT_SERP_LANG, DEFAULT_TIMEOUT_SECONDS,
    MAX_CONTENT_LENGTH_LIMIT, MAX_PAGES, MAX_PARALLELISM, MAX_PER_HOST_LIMIT, MAX_RETRIES,
};
use crate::error::{DEFAULT_CHROME_SESSION_RETRIES, MAX_CHROME_SESSION_RETRIES};

/// Default URLs enriched per vertical under `--fetch-content` (GAP-SCRAPE-R-004).
/// Default nested content-fetch URL cap (v1.0.2 budget + token contract).
///
/// With deep-research defaults (`max_sub_queries=3`, dual vertical, fetch ON)
/// and a 10% gate margin, cap=4 keeps gated estimate ≤ 180s (GAP-AUD-DR-001).
pub const DEFAULT_FETCH_CONTENT_CAP: usize = 4;
/// Hard upper bound for `--fetch-content-cap`.
pub const MAX_FETCH_CONTENT_CAP: usize = 50;

/// Help heading: output / presentation flags.
pub const HEADING_OUTPUT: &str = "Output";
/// Help heading: network / transport flags.
pub const HEADING_NETWORK: &str = "Network";
/// Help heading: Chrome / CDP flags.
pub const HEADING_CHROME: &str = "Chrome";
/// Help heading: content extraction flags.
pub const HEADING_CONTENT: &str = "Content fetch";
/// Help heading: diagnostics / logging flags.
pub const HEADING_DIAGNOSTICS: &str = "Diagnostics";

/// Parse and validate a proxy URL at clap parse time (exit 2 on bad input).
fn parse_proxy_url(s: &str) -> Result<String, String> {
    let parsed = url::Url::parse(s).map_err(|e| format!("invalid --proxy URL ({s:?}): {e}"))?;
    match parsed.scheme() {
        "http" | "https" | "socks5" | "socks5h" => Ok(s.to_string()),
        other => Err(format!(
            "scheme {other:?} not supported in --proxy (use http/https/socks5/socks5h)"
        )),
    }
}

/// Parse `--fetch-content-cap` at clap parse time (`1..=MAX_FETCH_CONTENT_CAP`).
fn parse_fetch_content_cap(s: &str) -> Result<usize, String> {
    let n: usize = s
        .parse()
        .map_err(|e| format!("invalid --fetch-content-cap value {s:?}: {e}"))?;
    if n < 1 {
        return Err(format!("--fetch-content-cap must be at least 1 (got {n})"));
    }
    if n > MAX_FETCH_CONTENT_CAP {
        return Err(format!(
            "--fetch-content-cap cannot exceed {MAX_FETCH_CONTENT_CAP} (got {n})"
        ));
    }
    Ok(n)
}

/// Parse `--max-content-length` at clap parse time (`1..=MAX_CONTENT_LENGTH_LIMIT`).
fn parse_max_content_length(s: &str) -> Result<usize, String> {
    let n: usize = s
        .parse()
        .map_err(|e| format!("invalid --max-content-length value {s:?}: {e}"))?;
    if n == 0 {
        return Err(format!("--max-content-length must be at least 1 (got {n})"));
    }
    if n > MAX_CONTENT_LENGTH_LIMIT {
        return Err(format!(
            "--max-content-length cannot exceed {MAX_CONTENT_LENGTH_LIMIT} (got {n})"
        ));
    }
    Ok(n)
}


pub use super::buscar_enums::{
    is_known_global_flag, CliEndpoint, CliIdentityProfile, CliOutputFormat, CliSafeSearch,
    CliTimeFilter, CliVertical,
};

/// Search arguments (shared between the direct mode and the `buscar` subcommand).
#[derive(Debug, Clone, Args)]
pub struct CliArgs {
    /// Search queries (free text). Accepts multiple space-separated values
    /// or via stdin (one per line) if none are passed here or via `--queries-file`.
    #[arg(value_name = "QUERY")]
    pub queries: Vec<String>,

    /// Maximum number of results to return per query (default: 15; pages controlled by
    /// `--pages`, default 1).
    #[arg(short = 'n', long = "num", value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
    pub num_results: Option<u32>,

    /// Output format: `json`, `text`, `markdown` (`md`), `tsv`, `ndjson`, or `auto`.
    /// `auto` uses `text` in a TTY and `json` in a pipe (and forces `json` when
    #[arg(
        short = 'f',
        long = "format",
        value_name = "FMT",
        value_enum,
        default_value_t = CliOutputFormat::Auto,
        help_heading = HEADING_OUTPUT
    )]
    pub format: CliOutputFormat,

    /// Writes output to the specified file instead of printing to stdout.
    /// Missing parent directories are created. On Unix, permissions 0o644 are applied.
    #[arg(
        short = 'o',
        long = "output",
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        help_heading = HEADING_OUTPUT
    )]
    pub output_file: Option<PathBuf>,

    /// Per-query timeout in seconds (default: 15).
    #[arg(
        short = 't',
        long = "timeout",
        value_name = "SECS",
        default_value_t = DEFAULT_TIMEOUT_SECONDS,
        value_parser = clap::value_parser!(u64).range(1..),
        help_heading = HEADING_NETWORK
    )]
    pub timeout_seconds: u64,

    /// Language for `DuckDuckGo`'s `kl` search parameter (default: [`DEFAULT_SERP_LANG`]).
    ///
    #[arg(
        short = 'l',
        long = "lang",
        value_name = "LANG",
        default_value = DEFAULT_SERP_LANG
    )]
    pub language: String,

    /// Country for `DuckDuckGo`'s `kl` parameter (default: [`DEFAULT_SERP_COUNTRY`]).
    /// `--region` is accepted as an alias for backwards compatibility.
    #[arg(
        short = 'c',
        long = "country",
        alias = "region",
        value_name = "CC",
        default_value = DEFAULT_SERP_COUNTRY
    )]
    pub country: String,

    /// Number of concurrent requests (default 5, maximum 20).
    ///
    #[arg(
        short = 'p',
        long = "parallel",
        visible_alias = "max-concurrency",
        value_name = "N",
        default_value_t = DEFAULT_PARALLELISM,
        value_parser = clap::value_parser!(u32).range(1..=MAX_PARALLELISM as i64),
        help_heading = HEADING_NETWORK
    )]
    pub parallelism: u32,

    /// Force one shared Chrome session for web+news (`--vertical all`) instead of
    /// dual multi-process Chromes (GAP-PAR-021). Lower RSS / anti-bot surface;
    #[arg(
        long = "shared-session-verticals",
        action = ArgAction::SetTrue,
        help_heading = HEADING_NETWORK
    )]
    pub shared_session_verticals: bool,

    /// File containing additional queries (one per line). Empty lines are ignored.
    #[arg(
        long = "queries-file",
        value_name = "PATH",
        value_hint = ValueHint::FilePath
    )]
    pub queries_file: Option<PathBuf>,

    /// Number of pages to fetch per query (1..=5). Default 1.
    #[arg(
        long = "pages",
        value_name = "N",
        default_value_t = DEFAULT_PAGES,
        value_parser = clap::value_parser!(u32).range(1..=MAX_PAGES as i64),
        help_heading = HEADING_NETWORK
    )]
    pub pages: u32,

    /// Number of additional retries on transient HTTP/network failures (0..=10).
    ///
    #[arg(
        long = "retries",
        value_name = "N",
        default_value_t = DEFAULT_RETRIES,
        value_parser = clap::value_parser!(u32).range(0..=MAX_RETRIES as i64),
        help_heading = HEADING_NETWORK
    )]
    pub retries: u32,

    /// Force zero retries (incident kill switch). Equivalent to `--retries 0`
    /// for the retry loop; GAP-SCRAPE-R2-010 (CLI only, no product env).
    #[arg(
        long = "disable-retry",
        action = ArgAction::SetTrue,
        help_heading = HEADING_NETWORK
    )]
    pub disable_retry: bool,

    /// Override HTML SERP base URL (trailing slash recommended). Default:
    /// `https://html.duckduckgo.com/html/`. For wiremock/tests only in normal
    #[arg(
        long = "base-url-html",
        value_name = "URL",
        help_heading = HEADING_NETWORK
    )]
    pub base_url_html: Option<String>,

    /// Override Lite base URL. Default: `https://lite.duckduckgo.com/lite/`.
    #[arg(
        long = "base-url-lite",
        value_name = "URL",
        help_heading = HEADING_NETWORK
    )]
    pub base_url_lite: Option<String>,

    /// Override SERP / warm-up base URL. Default: `https://duckduckgo.com/`.
    #[arg(
        long = "base-url-serp",
        value_name = "URL",
        help_heading = HEADING_NETWORK
    )]
    pub base_url_serp: Option<String>,

    /// Preferred endpoint: `html` (default) or `lite` (legacy value only).
    ///
    #[arg(long = "endpoint", value_enum, default_value_t = CliEndpoint::Html)]
    pub endpoint: CliEndpoint,

    /// Search vertical: `web`, `news`, or `all` (default **`all`** since v0.9.8).
    ///
    #[arg(long = "vertical", value_enum, default_value_t = CliVertical::All)]
    pub vertical: CliVertical,

    /// Time filter: `d` (day), `w` (week), `m` (month), `y` (year). Default: no filter.
    #[arg(long = "time-filter", value_enum)]
    pub time_filter: Option<CliTimeFilter>,

    /// Safe-search: `off`, `moderate` (default) or `on`.
    #[arg(long = "safe-search", value_enum, default_value_t = CliSafeSearch::Moderate)]
    pub safe_search: CliSafeSearch,

    /// Chrome health probe: minimal reachability check via chromiumoxide/CDP
    /// and reports status + latency (+ cookie signals when available) as JSON,
    #[arg(
        long = "probe",
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub probe: bool,

    /// Forces a specific browser identity profile from the 12-identity pool.
    /// Default `auto` rotates adaptively on block (HTTP 202/403/429).
    #[arg(long = "identity-profile", value_enum, default_value_t = CliIdentityProfile::Auto)]
    pub identity_profile: CliIdentityProfile,

    /// Multi-query only: emit per-query NDJSON as each search completes.
    /// Single-query mode ignores this flag (warning). Not a full event stream of
    #[arg(
        long = "stream",
        action = ArgAction::SetTrue,
        help_heading = HEADING_OUTPUT
    )]
    pub stream_mode: bool,

    /// Sets the verbosity level of stderr logs (repeatable).
    /// 0 = INFO (default), 1+ = DEBUG, 2+ = TRACE. Use `-v`, `-vv`, `-vvv` to accumulate.
    #[arg(
        short = 'v',
        long = "verbose",
        global = true,
        action = ArgAction::Count,
        conflicts_with = "quiet",
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub verbose: u8,

    /// Suppresses all stderr logs, keeping only the main output on stdout.
    #[arg(
        short = 'q',
        long = "quiet",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "verbose",
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub quiet: bool,

    /// Agent contract: never prompt / never read interactive TTY for answers.
    ///
    #[arg(
        long = "no-input",
        global = true,
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub no_input: bool,

    /// Project each result row to the listed wire fields (comma-separated).
    ///
    #[arg(
        long = "fields",
        value_name = "LIST",
        help_heading = HEADING_OUTPUT
    )]
    pub fields: Option<String>,

    /// Alias of `--fields` (agent-native / ETL-familiar name).
    #[arg(
        long = "select",
        value_name = "LIST",
        conflicts_with = "fields",
        help_heading = HEADING_OUTPUT
    )]
    pub select: Option<String>,

    /// Filter result rows after SERP extract (agent-native, no jq).
    ///
    #[arg(
        long = "filter",
        value_name = "EXPR",
        help_heading = HEADING_OUTPUT
    )]
    pub result_filter: Option<String>,

    /// Cap result rows **after** SERP extract + `--filter` (agent-native, no jq).
    ///
    #[arg(
        long = "limit",
        value_name = "N",
        value_parser = clap::value_parser!(u32).range(1..),
        help_heading = HEADING_OUTPUT
    )]
    pub result_limit: Option<u32>,

    /// Sort result rows after filter (agent-native; no jq).
    ///
    #[arg(
        long = "sort",
        value_name = "KEY[:DIR]",
        help_heading = HEADING_OUTPUT
    )]
    pub sort: Option<String>,

    /// Deduplicate rows by canonical URL after sort (agent-native; no jq).
    ///
    #[arg(
        long = "dedupe-by",
        value_name = "FIELD",
        help_heading = HEADING_OUTPUT
    )]
    pub dedupe_by: Option<String>,

    /// Emit only compact EN counts (`{"count":N,"web":W,"news":N}`) — no result rows.
    #[arg(
        long = "count-only",
        action = ArgAction::SetTrue,
        help_heading = HEADING_OUTPUT
    )]
    pub count_only: bool,

    /// Truncate each row `content` to N Unicode scalars (explicit anti-token).
    #[arg(
        long = "truncate-content",
        value_name = "N",
        value_parser = clap::value_parser!(u32).range(1..),
        help_heading = HEADING_OUTPUT
    )]
    pub truncate_content: Option<u32>,

    /// Fail-closed if the formatted stdout payload exceeds N bytes.
    #[arg(
        long = "max-output-bytes",
        value_name = "N",
        value_parser = clap::value_parser!(u64).range(1..),
        help_heading = HEADING_OUTPUT
    )]
    pub max_output_bytes: Option<u64>,

    /// Emit indented JSON (`to_string_pretty`). Default is **compact** JSON
    /// for agent token budgets (GAP-E2E-V19-JSON-PRETTY-DEFAULT).
    #[arg(
        long = "pretty",
        action = ArgAction::SetTrue,
        help_heading = HEADING_OUTPUT
    )]
    pub pretty: bool,

    /// Affirms content extraction (default ON since v0.9.8; kept for scripts).
    /// Prefer omitting this flag or using `--no-fetch-content` to disable.
    #[arg(
        long = "fetch-content",
        action = ArgAction::SetTrue,
        help_heading = HEADING_CONTENT
    )]
    pub fetch_content: bool,

    /// Disables page content extraction (opt-out of the v0.9.8 agent-ready default).
    #[arg(
        long = "no-fetch-content",
        action = ArgAction::SetTrue,
        conflicts_with = "fetch_content",
        help_heading = HEADING_CONTENT
    )]
    pub no_fetch_content: bool,

    /// Max URLs to enrich per vertical under `--fetch-content` (`1..=50`, default **4** in v1.0.2).
    /// Agent-ready cost bound (GAP-SCRAPE-R-004 / GAP-AUD-DR-001 budget contract).
    #[arg(
        long = "fetch-content-cap",
        value_name = "N",
        default_value_t = DEFAULT_FETCH_CONTENT_CAP,
        value_parser = parse_fetch_content_cap,
        help_heading = HEADING_CONTENT
    )]
    pub fetch_content_cap: usize,

    /// Maximum size (in characters) of the extracted content per page (`1..=100_000`).
    /// Only effective with `--fetch-content`. Default `10_000`.
    #[arg(
        long = "max-content-length",
        value_name = "N",
        default_value_t = DEFAULT_MAX_CONTENT_LENGTH,
        value_parser = parse_max_content_length,
        help_heading = HEADING_CONTENT
    )]
    pub max_content_length: usize,

    /// HTTP/HTTPS/SOCKS5 proxy URL (e.g., `http://user:pass@host:port`, `socks5://host:port`).
    /// Sole proxy source for residual HTTP (no `HTTP_PROXY` env inheritance; XDG/CLI only).
    #[arg(
        long = "proxy",
        value_name = "URL",
        value_hint = ValueHint::Url,
        value_parser = parse_proxy_url,
        conflicts_with = "no_proxy",
        help_heading = HEADING_NETWORK
    )]
    pub proxy: Option<String>,

    /// Disables any proxy (explicit no-proxy; residual HTTP never inherits env proxies).
    #[arg(
        long = "no-proxy",
        action = ArgAction::SetTrue,
        conflicts_with = "proxy",
        help_heading = HEADING_NETWORK
    )]
    pub no_proxy: bool,

    /// Restricts UAs loaded from `user-agents.toml` to the current platform (linux/macos/windows).
    /// Only takes effect if the external TOML file is found; otherwise uses built-in defaults.
    #[arg(
        long = "match-platform-ua",
        action = ArgAction::SetTrue,
        help_heading = HEADING_CHROME
    )]
    pub match_platform_ua: bool,

    /// Concurrent fetch limit PER HOST in `--fetch-content` mode (1..=10, default 2).
    /// Protects hosts from bursts — complements the global `--parallel` with a per-host gate.
    #[arg(
        long = "per-host-limit",
        value_name = "N",
        default_value_t = DEFAULT_PER_HOST_LIMIT,
        value_parser = clap::value_parser!(u32).range(1..=MAX_PER_HOST_LIMIT as i64),
        help_heading = HEADING_CONTENT
    )]
    pub per_host_limit: u32,

    /// Manual path to the Chrome/Chromium executable used for all production
    /// network ops (search, news, deep-research, probe, pre-flight,
    #[arg(
        long = "chrome-path",
        value_name = "PATH",
        value_hint = ValueHint::ExecutablePath,
        help_heading = HEADING_CHROME
    )]
    pub chrome_path: Option<PathBuf>,

    /// Force headed Chrome (visible window / native display). Debug override.
    /// GAP-SCRAPE-R-007: CLI primary (not product env).
    #[arg(
        long = "chrome-visible",
        action = ArgAction::SetTrue,
        conflicts_with = "chrome_headless",
        help_heading = HEADING_CHROME
    )]
    pub chrome_visible: bool,

    /// Force headless Chrome (`--headless=new`). Overrides Xvfb auto path.
    #[arg(
        long = "chrome-headless",
        action = ArgAction::SetTrue,
        conflicts_with = "chrome_visible",
        help_heading = HEADING_CHROME
    )]
    pub chrome_headless: bool,

    /// Request private Xvfb headed mode on Linux (invisible headed anti-bot).
    #[arg(
        long = "chrome-xvfb",
        action = ArgAction::SetTrue,
        help_heading = HEADING_CHROME
    )]
    pub chrome_xvfb: bool,

    /// Additional Chrome session launch attempts after the first (transient CDP failures).
    ///
    #[arg(
        long = "chrome-session-retries",
        value_name = "N",
        default_value_t = DEFAULT_CHROME_SESSION_RETRIES,
        value_parser = clap::value_parser!(u32).range(0..=MAX_CHROME_SESSION_RETRIES as i64),
        help_heading = HEADING_CHROME
    )]
    pub chrome_session_retries: u32,

    /// Write news SERP HTML after Chrome extract to this path (local debug only).
    /// GAP-SCRAPE-R-008: CLI path, not product env.
    #[arg(
        long = "dump-news-html",
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        help_heading = HEADING_CHROME
    )]
    pub dump_news_html: Option<PathBuf>,

    /// Disables colored output (respects `NO_COLOR` env var per no-color.org).
    #[arg(
        long = "no-color",
        global = true,
        action = ArgAction::SetTrue,
        help_heading = HEADING_OUTPUT
    )]
    pub no_color: bool,

    /// Disables the warm-up `GET https://duckduckgo.com/` request that
    /// populates session cookies before the first real query. v0.7.3 PR2.
    #[arg(
        long = "no-warmup",
        action = ArgAction::SetTrue,
        help_heading = HEADING_CHROME
    )]
    pub no_warmup: bool,

    /// Lab/harness only: permit `--no-warmup` (default false; XDG `allow_no_warmup`).
    ///
    #[arg(
        long = "allow-no-warmup",
        action = ArgAction::SetTrue,
        hide = true,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub allow_no_warmup: bool,

    /// Disables persistence of the cookie jar to disk. Cookies live only
    /// in memory for the duration of the process. v0.7.3 PR2.
    #[arg(
        long = "no-cookie-persistence",
        action = ArgAction::SetTrue,
        help_heading = HEADING_CHROME
    )]
    pub no_cookie_persistence: bool,

    /// Overrides the cookie jar file path. v0.7.3 PR2.
    /// Default is the XDG config dir joined with `cookies.json`.
    #[arg(
        long = "cookies-path",
        value_name = "PATH",
        value_hint = ValueHint::FilePath
    )]
    pub cookies_path: Option<PathBuf>,

    /// Deep health check via Chrome/CDP, including interstitial detection
    /// (Cloudflare / DDG bot challenge). Emits a JSON report on stdout and
    #[arg(
        long = "probe-deep",
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub probe_deep: bool,

    /// Fail with exit 5 when the search returns zero results (agent gate).
    ///
    #[arg(
        long = "require-results",
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub require_results: bool,

    /// Seed for deterministic User-Agent selection (debugging reproducibility).
    #[arg(long = "seed", value_name = "N")]
    pub seed: Option<u64>,

    /// Path to configuration directory (overrides default OS config path).
    #[arg(
        long = "config",
        value_name = "PATH",
        value_hint = ValueHint::DirPath
    )]
    pub config_path: Option<PathBuf>,
}

impl CliArgs {
    /// Validates that the parallelism degree is within the range `[1, MAX_PARALLELISM]`.
    ///
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when the operation fails.
    pub fn validate_parallelism(&self) -> Result<(), crate::error::CliError> {
        if self.parallelism == 0 {
            return Err(crate::error::CliError::invalid_config(format!(
                "--parallel must be at least 1 (got {})",
                self.parallelism
            )));
        }
        if self.parallelism > MAX_PARALLELISM {
            return Err(crate::error::CliError::invalid_config(format!(
                "--parallel cannot exceed {} (got {})",
                MAX_PARALLELISM, self.parallelism
            )));
        }
        Ok(())
    }

    /// Validates that the number of pages is within the range `[1, MAX_PAGES]`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when pages is out of range.
    pub fn validate_pages(&self) -> Result<(), crate::error::CliError> {
        if self.pages == 0 {
            return Err(crate::error::CliError::invalid_config(format!(
                "--pages must be at least 1 (got {})",
                self.pages
            )));
        }
        if self.pages > MAX_PAGES {
            return Err(crate::error::CliError::invalid_config(format!(
                "--pages cannot exceed {} (got {})",
                MAX_PAGES, self.pages
            )));
        }
        Ok(())
    }

    /// Validates that `--max-content-length` is within the range `[1, MAX_CONTENT_LENGTH_LIMIT]`.
    ///
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when the operation fails.
    pub fn validate_max_content_length(&self) -> Result<(), crate::error::CliError> {
        if self.max_content_length == 0 {
            return Err(crate::error::CliError::invalid_config(format!(
                "--max-content-length must be at least 1 (got {})",
                self.max_content_length
            )));
        }
        if self.max_content_length > MAX_CONTENT_LENGTH_LIMIT {
            return Err(crate::error::CliError::invalid_config(format!(
                "--max-content-length cannot exceed {} (got {})",
                MAX_CONTENT_LENGTH_LIMIT, self.max_content_length
            )));
        }
        Ok(())
    }

    /// v0.7.10 B3 fix: removed from `CliArgs` because the field is
    /// hoisted to `RootArgs`. The corresponding `validate_global_timeout`
    #[allow(clippy::empty_line_after_doc_comments)]
    /// Validates that `--proxy`, when provided, is a parseable URL with a supported scheme.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when the proxy URL is invalid.
    pub fn validate_proxy(&self) -> Result<(), crate::error::CliError> {
        let Some(url) = self.proxy.as_deref() else {
            return Ok(());
        };
        let parsed = url::Url::parse(url).map_err(|e| {
            crate::error::CliError::proxy_error(format!("invalid --proxy URL ({url:?}): {e}"))
        })?;
        match parsed.scheme() {
            "http" | "https" | "socks5" | "socks5h" => Ok(()),
            other => Err(crate::error::CliError::proxy_error(format!(
                "scheme {other:?} not supported in --proxy (use http/https/socks5)"
            ))),
        }
    }

    /// Validates that the number of retries is within the range `[0, MAX_RETRIES]`.
    ///
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when the operation fails.
    pub fn validate_retries(&self) -> Result<(), crate::error::CliError> {
        if self.retries > MAX_RETRIES {
            return Err(crate::error::CliError::invalid_config(format!(
                "--retries cannot exceed {} (got {})",
                MAX_RETRIES, self.retries
            )));
        }
        Ok(())
    }

    /// Validates that `--per-host-limit` is within the range `[1, MAX_PER_HOST_LIMIT]`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when per-host limit is out of range.
    pub fn validate_per_host_limit(&self) -> Result<(), crate::error::CliError> {
        if self.per_host_limit == 0 {
            return Err(crate::error::CliError::invalid_config(format!(
                "--per-host-limit must be at least 1 (got {})",
                self.per_host_limit
            )));
        }
        if self.per_host_limit > MAX_PER_HOST_LIMIT {
            return Err(crate::error::CliError::invalid_config(format!(
                "--per-host-limit cannot exceed {} (got {})",
                MAX_PER_HOST_LIMIT, self.per_host_limit
            )));
        }
        Ok(())
    }

    /// Validates that `--timeout` is at least 1 second.
    ///
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError`] when the operation fails.
    pub fn validate_timeout_seconds(&self) -> Result<(), crate::error::CliError> {
        if self.timeout_seconds == 0 {
            return Err(crate::error::CliError::invalid_config(format!(
                "--timeout must be at least 1 (got {})",
                self.timeout_seconds
            )));
        }
        Ok(())
    }
}
