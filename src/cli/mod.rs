// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (CLI parsing via clap derive, zero runtime)
//! CLI argument definitions via `clap` derive.
//!
//! This module contains ONLY declarative clap structs. ZERO business logic.
//!
//! Layout (GAP-CLI-MOD-SPLIT):
//! - `buscar_args` — search / `buscar` flags and enums
//! - `deep_research_args` — `deep-research` flags (CM-15)
//! - `config_args` — XDG `config` subcommand types
//! - `subcommand_args` — utility subcommand args (doctor, schema, …)
//!
//! Conversion of `CliArgs` into `Config` used by the pipeline occurs
//! in the `lib.rs` module (`run` function).

use clap::{builder::ValueHint, ArgAction, Parser, Subcommand as ClapSubcommand};
use std::path::PathBuf;

// Shell completion generation (MP-04).
pub use clap_complete::Shell as CompletionShell;

// GAP-DRY-DEFAULTS-001: SSOT is `types::bounded` — re-export for clap defaults.
pub use crate::types::bounded::{
    DEFAULT_BUDGET_TOKENS, DEFAULT_CANCEL_GRACE_SECS,
    DEFAULT_CONTENT_LENGTH as DEFAULT_MAX_CONTENT_LENGTH,
    DEFAULT_GLOBAL_TIMEOUT_SECONDS as DEFAULT_GLOBAL_TIMEOUT, DEFAULT_PAGES, DEFAULT_PARALLELISM,
    DEFAULT_PER_HOST_LIMIT, DEFAULT_RESULT_COUNT, DEFAULT_RETRIES, DEFAULT_SERP_COUNTRY,
    DEFAULT_SERP_LANG, DEFAULT_TIMEOUT_SECONDS, MAX_CONTENT_LENGTH as MAX_CONTENT_LENGTH_LIMIT,
    MAX_GLOBAL_TIMEOUT_SECONDS as MAX_GLOBAL_TIMEOUT, MAX_PAGES, MAX_PARALLELISM,
    MAX_PER_HOST_LIMIT, MAX_RETRIES, MAX_TIMEOUT_SECONDS,
};

mod agent_ops_args;
mod buscar_args;
mod buscar_enums;
mod buscar_validate;
mod config_args;
mod deep_research_args;
mod guard;
mod subcommand_args;

pub use agent_ops_args::AgentOpsArgs;
pub use guard::looks_like_unknown_subcommand_token;

pub use buscar_args::{
    canonical_long_flag, is_known_global_flag, CliArgs, CliEndpoint, CliIdentityProfile,
    CliOutputFormat, CliSafeSearch, CliTimeFilter, CliVertical, DEFAULT_FETCH_CONTENT_CAP,
    HEADING_CHROME, HEADING_CONTENT, HEADING_DIAGNOSTICS, HEADING_NETWORK, HEADING_OUTPUT,
    MAX_FETCH_CONTENT_CAP,
};
pub use config_args::{
    ConfigCmd, ConfigEffectiveArgs, ConfigGetArgs, ConfigListArgs, ConfigPathArgs, ConfigSetArgs,
    ConfigUnsetArgs,
};
pub use deep_research_args::{
    merge_deep_search_defaults, CliAggregationStrategy, CliSubQueryStrategy, CliSynthFormat,
    DeepResearchArgs,
};
pub use subcommand_args::{
    CommandsArgs, CompletionsArgs, DoctorArgs, InitConfigArgs, LocaleArgs, ManArgs, SchemaArgs,
};

/// Long version string: `CARGO_PKG_VERSION (git:SHA)` from `build.rs`.
pub const LONG_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (git:", env!("GIT_SHA"), ")",);

const _: () = assert!(DEFAULT_PER_HOST_LIMIT <= MAX_PER_HOST_LIMIT);
const _: () = assert!(DEFAULT_PARALLELISM <= MAX_PARALLELISM && DEFAULT_PARALLELISM >= 1);
const _: () = assert!(DEFAULT_MAX_CONTENT_LENGTH <= MAX_CONTENT_LENGTH_LIMIT);
const _: () = assert!(DEFAULT_GLOBAL_TIMEOUT <= MAX_GLOBAL_TIMEOUT && DEFAULT_GLOBAL_TIMEOUT >= 1);
const _: () = assert!(MAX_PAGES >= 1);
const _: () = assert!(MAX_RETRIES >= 1);

/// CLI for searching `DuckDuckGo` via real Chrome (chromiumoxide/CDP), with
/// structured JSON output for LLM consumption.
///
/// Root accepts an optional subcommand. When no subcommand is passed, the
/// default behavior is `buscar` — maintains full backward compatibility with
/// previous versions of the CLI.
///
/// Production network transport is Chrome-only (GAP-WS-113). Residual pure
/// HTTP exists only behind the `http-test-harness` feature for tests.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "duckduckgo-search-cli",
    version,
    long_version = LONG_VERSION,
    author,
    propagate_version = true,
    // Cap help wrapping so a 200-column terminal does not produce a different
    // document from an 80-column one. Without this, `--help` line counts vary by
    // terminal width (measured: 141 lines at COLUMNS=80, 105 at COLUMNS=200),
    // which made the help contract tests assert on the environment instead of on
    // the flag surface. Narrow terminals still wrap narrower — this is a ceiling.
    max_term_width = 100,
    about = "DuckDuckGo search via real Chrome (chromiumoxide/CDP), JSON for LLMs.",
    long_about = "Rust CLI that searches DuckDuckGo through real Chrome/Chromium \
                  (chromiumoxide + CDP). Production is Chrome-only (GAP-WS-113): \
                  SERP, news, deep-research, probe, pre-flight and fetch-content \
                  all require a usable Chrome binary. Without Chrome the CLI \
                  fails closed with exit 2. No paid APIs and no silent pure-HTTP \
                  production path. \
                  Returns structured organic results as JSON ready for LLM \
                  consumption.",
    after_long_help = "\
EXIT CODES:\n\
    0    Success — at least one query returned results\n\
    1    Runtime error (network, parse, I/O)\n\
    2    Invalid configuration (bad flag/proxy) OR Chrome missing (GAP-WS-113)\n\
    3    DuckDuckGo anti-bot soft-block (remediate with Chrome / --chrome-path / --proxy; NOT Lite)\n\
    4    Global timeout exceeded\n\
    5    Zero results across all queries (legitimate)\n\
    6    Suspected block (zero results with non-legitimate causa_zero)\n\
    130  Cancelled via SIGINT / Ctrl+C (128+2; cooperative cancel)\n\
    141  Broken pipe (stdout consumer closed early; SIGPIPE / ErrorKind::BrokenPipe)\n\
    143  Cancelled via SIGTERM / Ctrl+Break (128+15; timeout/Docker/systemd)\n\
\n\
PIPE USAGE:\n\
    duckduckgo-search-cli -q -f json \"query\" | jaq '.results[].url'\n\
    Logs go to stderr (-q suppresses them). JSON goes to stdout (compact).\n\
    Prefer binary ops over jq: --fields, --filter, --limit (post-SERP).\n\
    -n/--num = DDG request size; --limit = rows kept after filter (agent-native).\n\
    --pretty = indented JSON (default is compact).\n\
\n\
AGENT DISCOVERY (prefer over --help for low token cost):\n\
    duckduckgo-search-cli commands     # JSON command tree\n\
    duckduckgo-search-cli schema       # list JSON Schema IDs (or --name <id>)\n\
    duckduckgo-search-cli doctor       # environment / Chrome diagnostics JSON\n\
    duckduckgo-search-cli locale       # resolved UI locale (en/pt-BR) as JSON\n\
    Non-TTY --help writes to stderr (stdout stays free for pipes).\n\
\n\
UI LANGUAGE (human stderr only; stdout JSON stays stable):\n\
    --ui-lang en|pt-BR                 # flag (not -l/--lang SERP language)\n\
    XDG ui-lang preference file        # persisted via config dir\n\
    locale subcommand                  # diagnostics\n\
\n\
RUNTIME:\n\
    Requires Google Chrome or Chromium (feature chrome is default).\n\
    Linux may auto-install Xvfb for private headed Chrome; macOS/Windows use headless=new."
)]
pub struct RootArgs {
    /// Optional subcommand (`init-config`). No subcommand = search (default).
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    /// Search arguments (also accepted without a subcommand for backward compatibility).
    #[command(flatten)]
    pub buscar: CliArgs,

    /// LEGACY NO-OP (GAP-WS-113 / v0.9.4): previously forced the Lite endpoint.
    ///
    /// Kept so existing scripts do not fail on an unknown flag. Production SERP
    /// always uses HTML under chromiumoxide; Lite is never a success path and
    /// this flag does NOT remediate exit 3/6.
    ///
    /// When set, the binary records `flags_ignored: ["allow-lite-fallback"]` in
    /// search metadata and emits a stderr warning (GAP-E2E-V14-ALLOW-LITE-SILENT-NOOP).
    ///
    /// v0.7.9 GAP-WS-59: hoisted to `RootArgs` with `global = true`.
    #[arg(
        long = "allow-lite-fallback",
        global = true,
        action = ArgAction::SetTrue,
        help_heading = HEADING_NETWORK
    )]
    pub allow_lite_fallback: bool,

    /// Print the schema catalog as JSON on stdout and exit 0 (agent discovery).
    ///
    /// Equivalent to the `schema` subcommand without `--name` (GAP-E2E-V14-PRINT-SCHEMA-ROOT-MISSING).
    /// Machine stdout only — no help banner.
    #[arg(
        long = "print-schema",
        global = true,
        action = ArgAction::SetTrue,
        help_heading = HEADING_OUTPUT
    )]
    pub print_schema: bool,

    /// Pre-flight ghost-block / interstitial calibration on the shared Chrome
    /// SERP session before the real query (GAP-WS-113 Chrome-only).
    ///
    /// When enabled, a blocked/calibration response is classified early so the
    /// operator can act (Chrome path, proxy, wait). Does NOT switch production
    /// SERP to Lite and does NOT unlock a pure-HTTP success path.
    /// Default `false` — opt-in only.
    #[arg(
        long = "pre-flight",
        global = true,
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub pre_flight: bool,

    /// Global timeout for the entire execution in seconds (1..=3600). Default 180 (v0.9.9 agent-ready).
    /// Different from `--timeout`, which is per-request.
    ///
    /// v0.7.10 B3 fix: hoisted to `RootArgs` with `global = true` so the
    /// flag is honored by subcommands such as `deep-research` and the
    /// default search path alike (previously caused exit 2 "unexpected
    /// argument" inside subcommands).
    #[arg(
        long = "global-timeout",
        value_name = "SECS",
        global = true,
        default_value_t = DEFAULT_GLOBAL_TIMEOUT,
        value_parser = clap::value_parser!(u64).range(1..=MAX_GLOBAL_TIMEOUT),
        help_heading = HEADING_NETWORK
    )]
    pub global_timeout_seconds: u64,

    /// UI language for human-facing stderr messages (`en` or `pt-BR`).
    ///
    /// **Not** the DuckDuckGo search language (`-l` / `--lang`, SERP `kl`).
    /// Precedence: this flag → persisted XDG `ui-lang` file → OS locale
    /// (`sys-locale`) → default `en` (GAP-SCRAPE-R2-014: no product env).
    /// Machine stdout (JSON/NDJSON/schemas) is never translated.
    #[arg(
        long = "ui-lang",
        value_name = "LOCALE",
        global = true,
        help_heading = HEADING_OUTPUT
    )]
    pub ui_lang: Option<String>,

    /// Cooperative cancel grace before hard exit (seconds, 1..=60). Default 5.
    /// GAP-SCRAPE-R2-011: CLI only (no product env).
    #[arg(
        long = "cancel-grace-secs",
        value_name = "SECS",
        global = true,
        default_value_t = DEFAULT_CANCEL_GRACE_SECS,
        value_parser = clap::value_parser!(u64).range(1..=60),
        help_heading = HEADING_NETWORK
    )]
    pub cancel_grace_secs: u64,

    /// Disable strict zero-cause exit mapping (legacy exit 5 for all zeros).
    /// Default is strict ON (exit 6 for non-legitimate zeros). GAP-SCRAPE-R2-012.
    #[arg(
        long = "no-zero-cause-strict",
        global = true,
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub no_zero_cause_strict: bool,

    /// Override XDG/platform config directory (selectors, cookies, ui-lang).
    /// GAP-SCRAPE-R2-015: CLI only (replaces product env CLI_HOME).
    #[arg(
        long = "config-home",
        value_name = "PATH",
        value_hint = ValueHint::DirPath,
        global = true,
        help_heading = HEADING_OUTPUT
    )]
    pub config_home: Option<PathBuf>,

    /// Wire JSON key language for stdout serialization (`en` default, `pt` legacy).
    ///
    /// Domain types always serialize English; `pt` remaps keys at the emit
    /// boundary (ADR-0027 / V30). Precedence: this flag → XDG `wire_keys` → `en`.
    #[arg(
        long = "wire-keys",
        value_name = "LANG",
        global = true,
        value_enum,
        default_value_t = crate::output::WireKeys::En,
        help_heading = HEADING_OUTPUT
    )]
    pub wire_keys: crate::output::WireKeys,
}

impl RootArgs {
    /// v0.7.10 B3 fix: validation lives on `RootArgs` now.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError::InvalidConfig`] when
    /// `global_timeout_seconds` is `0` or exceeds `MAX_GLOBAL_TIMEOUT` (3600 seconds).
    pub fn validate_global_timeout(&self) -> Result<(), crate::error::CliError> {
        if self.global_timeout_seconds == 0 {
            return Err(crate::error::CliError::invalid_config(format!(
                "--global-timeout must be at least 1 (got {})",
                self.global_timeout_seconds
            )));
        }
        if self.global_timeout_seconds > MAX_GLOBAL_TIMEOUT {
            return Err(crate::error::CliError::invalid_config(format!(
                "--global-timeout cannot exceed {} seconds (got {})",
                MAX_GLOBAL_TIMEOUT, self.global_timeout_seconds
            )));
        }
        Ok(())
    }
}

/// Supported subcommands. Chosen architecture: `Option<Subcommand>` at the root
/// allows invocation without a subcommand (direct search) OR with an explicit subcommand.
///
/// `Buscar` is `Box`ed to avoid a large enum variant (`CliArgs` has
/// many clap-derived fields).
#[derive(Debug, Clone, ClapSubcommand)]
pub enum Subcommand {
    /// Search on `DuckDuckGo` (equivalent to the no-subcommand mode). Hidden from --help to avoid duplication with the no-subcommand mode.
    #[command(hide = true)]
    Buscar(Box<CliArgs>),
    /// Initializes configuration files (`selectors.toml`, `user-agents.toml`)
    /// in the default OS configuration directory.
    InitConfig(InitConfigArgs),
    /// Generates shell completion scripts for the specified shell.
    Completions(CompletionsArgs),
    /// Runs a deep research pipeline: query fan-out, aggregation, and
    /// optional synthesis into a Markdown/PlainText/Json report.
    DeepResearch(Box<DeepResearchArgs>),
    /// Emits the full command tree as JSON (agent discovery; rules-rust-cli-stdin-stdout).
    Commands(CommandsArgs),
    /// Emits JSON Schema catalog or a named schema body (agent discovery).
    Schema(SchemaArgs),
    /// Diagnoses environment, Chrome/Chromium, and runtime prerequisites as JSON.
    Doctor(DoctorArgs),
    /// Prints resolved UI locale diagnostics as JSON (i18n; agent-readable).
    Locale(LocaleArgs),
    /// Prints the man page (roff) generated from the same clap tree as `--help`.
    Man(ManArgs),
    /// Reads or writes persistent XDG config (`config.toml`) without product env vars.
    #[command(subcommand)]
    Config(ConfigCmd),
}

#[cfg(test)]
mod tests;
