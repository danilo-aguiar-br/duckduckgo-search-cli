// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (CLI parsing via clap derive, zero runtime)
//! CLI argument definitions via `clap` derive.
//!
//! This module contains ONLY declarative clap structs. ZERO business logic.
//! Conversion of `CliArgs` into `Config` used by the pipeline occurs
//! in the `lib.rs` module (`run` function).
//!
//! In iteration 6 the `init-config` subcommand was added — backward-compatible,
//! since when no subcommand is passed, the previous search behavior is preserved
//! via `#[command(subcommand)]` with `Option<Subcommand>`.

use clap::{
    builder::ValueHint, ArgAction, Args, Parser, Subcommand as ClapSubcommand, ValueEnum,
};
use std::path::PathBuf;

// Shell completion generation (MP-04).
pub use clap_complete::Shell as CompletionShell;

// GAP-DRY-DEFAULTS-001: SSOT is `types::bounded` — re-export for clap defaults.
pub use crate::types::bounded::{
    DEFAULT_BUDGET_TOKENS, DEFAULT_CANCEL_GRACE_SECS, DEFAULT_CONTENT_LENGTH as DEFAULT_MAX_CONTENT_LENGTH,
    DEFAULT_GLOBAL_TIMEOUT_SECONDS as DEFAULT_GLOBAL_TIMEOUT, DEFAULT_PAGES, DEFAULT_PARALLELISM,
    DEFAULT_PER_HOST_LIMIT, DEFAULT_RESULT_COUNT, DEFAULT_RETRIES, DEFAULT_SERP_COUNTRY,
    DEFAULT_SERP_LANG, DEFAULT_TIMEOUT_SECONDS, MAX_CONTENT_LENGTH as MAX_CONTENT_LENGTH_LIMIT,
    MAX_GLOBAL_TIMEOUT_SECONDS as MAX_GLOBAL_TIMEOUT, MAX_PAGES, MAX_PARALLELISM, MAX_PER_HOST_LIMIT,
    MAX_RETRIES, MAX_TIMEOUT_SECONDS,
};

/// Default URLs enriched per vertical under `--fetch-content` (GAP-SCRAPE-R-004).
pub const DEFAULT_FETCH_CONTENT_CAP: usize = 10;
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

/// Long version string: `CARGO_PKG_VERSION (git:SHA)` from `build.rs`.
pub const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (git:",
    env!("GIT_SHA"),
    ")"
);


// Compile-time invariants: defaults must never exceed hard caps (rules-rust const).
const _: () = assert!(DEFAULT_PER_HOST_LIMIT <= MAX_PER_HOST_LIMIT);
const _: () = assert!(DEFAULT_PARALLELISM <= MAX_PARALLELISM && DEFAULT_PARALLELISM >= 1);
const _: () = assert!(DEFAULT_MAX_CONTENT_LENGTH <= MAX_CONTENT_LENGTH_LIMIT);
const _: () = assert!(DEFAULT_GLOBAL_TIMEOUT <= MAX_GLOBAL_TIMEOUT && DEFAULT_GLOBAL_TIMEOUT >= 1);
const _: () = assert!(MAX_PAGES >= 1);
const _: () = assert!(MAX_RETRIES >= 1);

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

/// Selectable `DuckDuckGo` endpoint via `--endpoint`.
///
/// Production SERP under chromiumoxide always navigates the **HTML** canonical
/// page (GAP-WS-113). `Lite` remains a clap value for backward compatibility
/// but is not a production success or remediation path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliEndpoint {
    /// Full HTML endpoint (`html.duckduckgo.com`) — production Chrome SERP.
    Html,
    /// Lightweight endpoint (`lite.duckduckgo.com`) — legacy value; not a
    /// production success path under GAP-WS-113 Chrome-only.
    Lite,
}

/// Search vertical accepted by `--vertical` (GAP-WS-104 / GAP-WS-113 / AGENT-READY-001).
///
/// Default is **`All`** (web + news) since v0.9.8. `News` and `All` require usable
/// Chrome/Chromium and are routed EXCLUSIVELY through the Chrome/CDP transport —
/// the `DuckDuckGo` news vertical (`ia=news&iar=news`) needs JavaScript rendering.
/// Without Chrome (binary missing or crate built without `chrome` feature) the CLI fails
/// closed with exit 2. Multi-query batches are accepted. Content fetch (default on)
/// also applies to news article URLs when enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum CliVertical {
    /// Organic web results only (`--vertical web`).
    Web,
    /// News vertical only (`ia=news&iar=news`). Requires Chrome; fail-closed without it.
    News,
    /// Both verticals in the same Chrome session (best-effort news). **Default** since v0.9.8.
    #[default]
    All,
}

/// Time filter accepted by `--time-filter` (DDG `df` parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliTimeFilter {
    /// Last day.
    D,
    /// Last week.
    W,
    /// Last month.
    M,
    /// Last year.
    Y,
}

/// Safe-search accepted by `--safe-search` (DDG `kp` parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliSafeSearch {
    /// Disable all content filters.
    Off,
    /// DDG default moderate filtering.
    Moderate,
    /// Strict filtering of adult content.
    On,
}

/// Browser identity profile accepted by `--identity-profile`.
///
/// `Auto` (default) selects from the 12-identity pool adaptively, rotating on
/// detected blocks. The other variants pin the session to a single identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliIdentityProfile {
    /// Adaptive selection from the 12-identity pool (default).
    Auto,
    /// Chrome on Windows.
    ChromeWin,
    /// Chrome on macOS.
    ChromeMac,
    /// Chrome on Linux.
    ChromeLinux,
    /// Edge on Windows.
    EdgeWin,
    /// Firefox on Linux.
    FirefoxLinux,
    /// Safari on macOS.
    SafariMac,
}

impl CliIdentityProfile {
    /// Returns the family and platform tuple for the profile, or `None` for `Auto`.
    pub fn family_and_platform(
        self,
    ) -> Option<(crate::identity::BrowserFamily, crate::identity::Platform)> {
        use crate::identity::{BrowserFamily, Platform};
        match self {
            Self::Auto => None,
            Self::ChromeWin => Some((BrowserFamily::Chrome, Platform::Windows)),
            Self::ChromeMac => Some((BrowserFamily::Chrome, Platform::MacOS)),
            Self::ChromeLinux => Some((BrowserFamily::Chrome, Platform::Linux)),
            Self::EdgeWin => Some((BrowserFamily::Edge, Platform::Windows)),
            Self::FirefoxLinux => Some((BrowserFamily::Firefox, Platform::Linux)),
            Self::SafariMac => Some((BrowserFamily::Safari, Platform::MacOS)),
        }
    }
}

/// Returns `true` when `arg` (without leading dashes) names a flag known
/// to the root parser (`CliArgs` + hoisted globals). Used by `run()` to
/// decide whether to append a "reposicione antes do subcomando" hint when
/// clap reports `ErrorKind::UnknownArgument` after a subcommand (GAP-WS-106
/// Sintoma A). Covers the 8 hoisted globals (accepted in any position) AND
/// the local-only flags of `CliArgs` (still rejected after a subcommand —
/// the hint is precisely for those).
pub fn is_known_global_flag(arg: &str) -> bool {
    matches!(
        arg,
        // 8 globals hoisted in v0.9.0
        "q" | "quiet"
            | "o" | "output"
            | "n" | "num"
            | "f" | "format"
            | "l" | "lang"
            | "c" | "country"
            | "t" | "timeout"
            | "p" | "parallel"
            | "max-concurrency"
            | "v" | "verbose"
            | "ui-lang"
            // local-only CliArgs flags (rejected after subcommand)
            | "queries-file"
            | "pages"
            | "retries"
            | "disable-retry"
            | "base-url-html"
            | "base-url-lite"
            | "base-url-serp"
            | "cancel-grace-secs"
            | "no-zero-cause-strict"
            | "config-home"
            | "endpoint"
            | "vertical"
            | "time-filter"
            | "safe-search"
            | "probe"
            | "identity-profile"
            | "stream"
            | "fetch-content"
            | "no-fetch-content"
            | "max-content-length"
            | "proxy"
            | "no-proxy"
            | "match-platform-ua"
            | "per-host-limit"
            | "chrome-path"
            | "no-color"
            | "no-warmup"
            | "no-cookie-persistence"
            | "cookies-path"
            | "probe-deep"
            | "seed"
            | "config"
    )
}

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
    duckduckgo-search-cli -q -f json \"query\" | jaq '.resultados[].url'\n\
    Logs go to stderr (-q suppresses them). JSON goes to stdout.\n\
\n\
AGENT DISCOVERY:\n\
    duckduckgo-search-cli commands     # JSON command tree\n\
    duckduckgo-search-cli schema       # list JSON Schema IDs (or --name <id>)\n\
    duckduckgo-search-cli doctor       # environment / Chrome diagnostics JSON\n\
    duckduckgo-search-cli locale       # resolved UI locale (en/pt-BR) as JSON\n\
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
    /// v0.7.9 GAP-WS-59: hoisted to `RootArgs` with `global = true`.
    #[arg(
        long = "allow-lite-fallback",
        global = true,
        action = ArgAction::SetTrue,
        help_heading = HEADING_NETWORK
    )]
    pub allow_lite_fallback: bool,

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
    DeepResearch(DeepResearchArgs),
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

/// Arguments for the `man` subcommand.
#[derive(Debug, Clone, Args, Default)]
pub struct ManArgs {
    /// Optional path to write the man page (atomic). When omitted, writes roff to stdout.
    /// Uses `--file` (not `-o`) to avoid clashing with the global `--output` flag.
    #[arg(long = "file", value_name = "PATH")]
    pub file: Option<std::path::PathBuf>,
}

/// `config` subcommand — XDG persistence (GAP-V101-XDG-001).
#[derive(Debug, Clone, ClapSubcommand)]
pub enum ConfigCmd {
    /// Print the resolved XDG config directory path as JSON.
    Path(ConfigPathArgs),
    /// List all keys in `config.toml` as JSON.
    List(ConfigListArgs),
    /// Get one key (JSON object `{ "key", "value" }`).
    Get(ConfigGetArgs),
    /// Set one key (creates `config.toml` with mode 0600 when needed).
    Set(ConfigSetArgs),
    /// Unset (remove) one key from `config.toml`.
    Unset(ConfigUnsetArgs),
    /// Show merged effective values (CLI > XDG > defaults) for allowed keys.
    Effective(ConfigEffectiveArgs),
}

/// Arguments for `config path`.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigPathArgs {}

/// Arguments for `config list`.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigListArgs {}

/// Arguments for `config effective`.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigEffectiveArgs {}

/// Arguments for `config get` (GAP-E2E-51-003: positional **or** `--key`).
///
/// Accepted forms:
/// - `config get KEY`
/// - `config get --key KEY`
#[derive(Debug, Clone, Args)]
pub struct ConfigGetArgs {
    /// Configuration key as a positional argument (`config get KEY`).
    #[arg(value_name = "KEY", required_unless_present = "key_flag")]
    pub key_positional: Option<String>,

    /// Configuration key via flag (`config get --key KEY`).
    #[arg(
        long = "key",
        value_name = "KEY",
        required_unless_present = "key_positional"
    )]
    pub key_flag: Option<String>,
}

impl ConfigGetArgs {
    /// Resolved key from positional or `--key` (clap guarantees one is present).
    ///
    /// # Panics
    ///
    /// Panics if neither positional `KEY` nor `--key` is set (clap forbids that).
    #[must_use]
    pub fn key(&self) -> &str {
        self.key_flag
            .as_deref()
            .or(self.key_positional.as_deref())
            .expect("clap requires positional KEY or --key")
    }
}

/// Arguments for `config set` (GAP-E2E-51-003: positional **or** flags).
///
/// Accepted forms:
/// - `config set KEY VALUE`
/// - `config set --key KEY --value VALUE`
/// - mixed (`config set KEY --value VALUE`, `config set --key KEY VALUE`)
#[derive(Debug, Clone, Args)]
pub struct ConfigSetArgs {
    /// Configuration key as a positional argument.
    #[arg(value_name = "KEY", required_unless_present = "key_flag")]
    pub key_positional: Option<String>,

    /// Value as a positional argument.
    #[arg(value_name = "VALUE", required_unless_present = "value_flag")]
    pub value_positional: Option<String>,

    /// Configuration key via flag (`--key`).
    #[arg(
        long = "key",
        value_name = "KEY",
        required_unless_present = "key_positional"
    )]
    pub key_flag: Option<String>,

    /// Value via flag (`--value`).
    #[arg(
        long = "value",
        value_name = "VALUE",
        required_unless_present = "value_positional"
    )]
    pub value_flag: Option<String>,
}

impl ConfigSetArgs {
    /// Resolved key from positional or `--key`.
    ///
    /// # Panics
    ///
    /// Panics if neither positional `KEY` nor `--key` is set (clap forbids that).
    #[must_use]
    pub fn key(&self) -> &str {
        self.key_flag
            .as_deref()
            .or(self.key_positional.as_deref())
            .expect("clap requires positional KEY or --key")
    }

    /// Resolved value from positional or `--value`.
    ///
    /// # Panics
    ///
    /// Panics if neither positional `VALUE` nor `--value` is set (clap forbids that).
    #[must_use]
    pub fn value(&self) -> &str {
        self.value_flag
            .as_deref()
            .or(self.value_positional.as_deref())
            .expect("clap requires positional VALUE or --value")
    }
}

/// Arguments for `config unset` (positional **or** `--key`).
///
/// Accepted forms:
/// - `config unset KEY`
/// - `config unset --key KEY`
#[derive(Debug, Clone, Args)]
pub struct ConfigUnsetArgs {
    /// Configuration key as a positional argument.
    #[arg(value_name = "KEY", required_unless_present = "key_flag")]
    pub key_positional: Option<String>,

    /// Configuration key via flag (`--key`).
    #[arg(
        long = "key",
        value_name = "KEY",
        required_unless_present = "key_positional"
    )]
    pub key_flag: Option<String>,
}

impl ConfigUnsetArgs {
    /// Resolved key from positional or `--key`.
    ///
    /// # Panics
    ///
    /// Panics if neither positional `KEY` nor `--key` is set (clap forbids that).
    #[must_use]
    pub fn key(&self) -> &str {
        self.key_flag
            .as_deref()
            .or(self.key_positional.as_deref())
            .expect("clap requires positional KEY or --key")
    }
}

/// Arguments for the `locale` subcommand (UI language diagnostics).
#[derive(Debug, Clone, Args, Default)]
pub struct LocaleArgs {}

/// Arguments for the `commands` subcommand (agent-ready command tree).
#[derive(Debug, Clone, Args, Default)]
pub struct CommandsArgs {}

/// Arguments for the `schema` subcommand.
#[derive(Debug, Clone, Args, Default)]
pub struct SchemaArgs {
    /// Schema id to emit (e.g. `search-output`). When omitted, lists all ids.
    #[arg(long = "name", value_name = "ID")]
    pub name: Option<String>,
}

/// Arguments for the `doctor` subcommand.
#[derive(Debug, Clone, Args, Default)]
pub struct DoctorArgs {
    /// Exit non-zero when Chrome is missing, or when the detected Chrome major
    /// is wildly ahead of the chromiumoxide PDL baseline (GAP / OPP-DOCTOR-STRICT).
    ///
    /// JSON stdout shape stays agent-stable (additive fields only). Without
    /// this flag, doctor still reports `ok=false` when Chrome is missing, but
    /// does **not** fail solely for a far-ahead Chrome major.
    #[arg(long = "strict")]
    pub strict: bool,
}

/// Arguments for the `deep-research` subcommand (v0.7.0).
#[derive(Debug, Clone, Args)]
pub struct DeepResearchArgs {
    /// The original user query to research.
    #[arg(value_name = "QUERY")]
    pub query: String,

    /// Maximum number of sub-queries to produce by decomposition (1..=12, default 5).
    #[arg(
        long = "max-sub-queries",
        value_name = "N",
        default_value_t = crate::deep_research::DEFAULT_MAX_SUB_QUERIES
    )]
    pub max_sub_queries: usize,

    /// Decomposition strategy: `heuristic` (5 templates, default) or `manual`.
    #[arg(
        long = "sub-query-strategy",
        value_enum,
        default_value_t = CliSubQueryStrategy::Heuristic
    )]
    pub sub_query_strategy: CliSubQueryStrategy,

    /// File with one sub-query per line (only used with `--sub-query-strategy manual`).
    #[arg(
        long = "sub-queries-file",
        value_name = "PATH",
        value_hint = ValueHint::FilePath
    )]
    pub sub_queries_file: Option<PathBuf>,

    /// Aggregation strategy: `rrf` (default, K=60) or `dedupe-by-url`.
    #[arg(
        long = "aggregate",
        value_enum,
        default_value_t = CliAggregationStrategy::Rrf
    )]
    pub aggregation: CliAggregationStrategy,

    /// Reflection depth (0..=3). 0 = single pass. Each round runs heuristic
    /// follow-up sub-queries from top titles/snippets and re-aggregates (v1.0.1).
    #[arg(long = "depth", value_name = "N", default_value_t = 0)]
    pub depth: u32,

    /// Affirms content extraction (default ON since v0.9.8; kept for scripts).
    #[arg(long = "fetch-content", action = ArgAction::SetTrue, help_heading = HEADING_CONTENT)]
    pub fetch_content: bool,

    /// Disables content extraction for deep-research (opt-out of v0.9.8 default).
    #[arg(
        long = "no-fetch-content",
        action = ArgAction::SetTrue,
        conflicts_with = "fetch_content",
        help_heading = HEADING_CONTENT
    )]
    pub no_fetch_content: bool,

    /// Produces a synthesised report at the end of the pipeline.
    #[arg(long = "synthesize", action = ArgAction::SetTrue, help_heading = HEADING_OUTPUT)]
    pub synthesize: bool,

    /// Approximate token budget for the synthesised report (default 4000).
    /// 1 token ≈ 4 characters (English text heuristic).
    #[arg(
        long = "budget-tokens",
        value_name = "N",
        default_value_t = DEFAULT_BUDGET_TOKENS as usize
    )]
    pub budget_tokens: usize,

    /// Format of the synthesised report.
    #[arg(
        long = "synth-format",
        value_enum,
        default_value_t = CliSynthFormat::Markdown
    )]
    pub synth_format: CliSynthFormat,

    /// Fail with a non-zero exit code when the fan-out aggregates zero
    /// results. Default `false` preserves v0.7.0–v0.7.9 behavior (exit 0
    /// even with an empty payload). v0.7.10 GAP-WS-1114.
    #[arg(long = "require-results", action = ArgAction::SetTrue, help_heading = HEADING_DIAGNOSTICS)]
    pub require_results: bool,

    /// Desativa a varredura da vertical news no deep-research (util em
    /// CI/testes sem Chrome). GAP-WS-105 v0.8.9: por padrao o deep-research
    /// aplica a vertical `all` (web + news) a cada sub-query.
    #[arg(long = "no-news", action = ArgAction::SetTrue, help_heading = HEADING_NETWORK)]
    pub no_news: bool,
}

/// CLI wrapper for the decomposition strategy enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliSubQueryStrategy {
    /// Heuristic fan-out using 5 canonical templates (default).
    Heuristic,
    /// Read sub-queries from a file or stdin.
    Manual,
}

/// CLI wrapper for the aggregation strategy enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliAggregationStrategy {
    /// Reciprocal Rank Fusion with K=60 (default).
    Rrf,
    /// Canonical-URL deduplication, keep first occurrence.
    DedupeByUrl,
}

/// CLI wrapper for the synthesis format enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliSynthFormat {
    /// Markdown with H2/H3 headings and `[n](url)` links.
    Markdown,
    /// Linear numbered list without markup.
    PlainText,
    /// Structured JSON tree.
    Json,
}

/// Output format accepted by `-f` / `--format` (rules: strong types, no free `String`).
///
/// Converted to domain [`crate::types::OutputFormat`] before pipeline dispatch.
///
/// `ndjson` is accepted as an agent-friendly alias that enables multi-query
/// stream mode (`--stream`) rather than a distinct non-stream format
/// (GAP-E2E-51-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum CliOutputFormat {
    /// Structured JSON (pipes / LLM agents).
    Json,
    /// Human-readable plain text.
    Text,
    /// Markdown with headers and links. Accepts alias `md`.
    #[value(alias = "md")]
    Markdown,
    /// Tab-separated values (stable columns for agents/scripts).
    Tsv,
    /// Multi-query NDJSON stream alias for `--stream` (GAP-E2E-51-005).
    ///
    /// Maps to domain JSON format and forces `stream_mode = true` in
    /// `build_config`. Single-query runs ignore stream mode with a warning.
    Ndjson,
    /// Auto: `text` on TTY, `json` in pipes (and when `--output` forces file).
    #[default]
    Auto,
}

impl CliOutputFormat {
    /// Whether this format alias enables multi-query NDJSON stream mode.
    #[must_use]
    pub const fn enables_stream_mode(self) -> bool {
        matches!(self, Self::Ndjson)
    }
}

impl From<CliOutputFormat> for crate::types::OutputFormat {
    fn from(value: CliOutputFormat) -> Self {
        match value {
            CliOutputFormat::Json | CliOutputFormat::Ndjson => Self::Json,
            CliOutputFormat::Text => Self::Text,
            CliOutputFormat::Markdown => Self::Markdown,
            CliOutputFormat::Tsv => Self::Tsv,
            CliOutputFormat::Auto => Self::Auto,
        }
    }
}

impl From<CliSubQueryStrategy> for crate::deep_research::SubQueryStrategy {
    fn from(value: CliSubQueryStrategy) -> Self {
        match value {
            CliSubQueryStrategy::Heuristic => Self::Heuristic,
            CliSubQueryStrategy::Manual => Self::Manual,
        }
    }
}

impl From<CliAggregationStrategy> for crate::deep_research::AggregationStrategyKind {
    fn from(value: CliAggregationStrategy) -> Self {
        match value {
            CliAggregationStrategy::Rrf => Self::Rrf,
            CliAggregationStrategy::DedupeByUrl => Self::DedupeByUrl,
        }
    }
}

impl From<CliSynthFormat> for crate::synthesis::SynthFormat {
    fn from(value: CliSynthFormat) -> Self {
        match value {
            CliSynthFormat::Markdown => Self::Markdown,
            CliSynthFormat::PlainText => Self::PlainText,
            CliSynthFormat::Json => Self::Json,
        }
    }
}

/// Arguments for the `completions` subcommand (MP-04).
#[derive(Debug, Clone, Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for (bash, zsh, fish, powershell, elvish).
    #[arg(value_enum)]
    pub shell: CompletionShell,
}

/// Arguments specific to the `init-config` subcommand.
#[derive(Debug, Clone, Args)]
pub struct InitConfigArgs {
    /// Overwrites existing files. Without this flag, files already present
    /// are kept intact.
    #[arg(long = "force", action = ArgAction::SetTrue)]
    pub force: bool,

    /// Simulates execution without writing any file to disk. Reports the actions
    /// that would be taken.
    #[arg(long = "dry-run", action = ArgAction::SetTrue)]
    pub dry_run: bool,
}

/// Search arguments (shared between the direct mode and the `buscar` subcommand).
#[derive(Debug, Clone, Args)]
pub struct CliArgs {
    /// Search queries (free text). Accepts multiple space-separated values
    /// or via stdin (one per line) if none are passed here or via `--queries-file`.
    #[arg(value_name = "QUERY")]
    pub queries: Vec<String>,

    /// Maximum number of results to return per query (default: 15, with
    /// auto-pagination to 2 pages when `--pages` is not customized).
    /// If omitted, uses 15; if `--num > 10` and `--pages == 1` (default),
    /// `--pages` is auto-elevated to `ceil(num/10)` up to a maximum of 5.
    #[arg(short = 'n', long = "num", value_name = "N", global = true, value_parser = clap::value_parser!(u32).range(1..))]
    pub num_results: Option<u32>,

    /// Output format: `json`, `text`, `markdown` (`md`), `tsv`, `ndjson`, or `auto`.
    /// `auto` uses `text` in a TTY and `json` in a pipe (and forces `json` when
    /// `--output` is provided). `ndjson` enables multi-query stream mode
    /// (alias for `--stream`).
    #[arg(
        short = 'f',
        long = "format",
        value_name = "FMT",
        value_enum,
        global = true,
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
        global = true,
        help_heading = HEADING_OUTPUT
    )]
    pub output_file: Option<PathBuf>,

    /// Per-query timeout in seconds (default: 15).
    #[arg(
        short = 't',
        long = "timeout",
        value_name = "SECS",
        global = true,
        default_value_t = DEFAULT_TIMEOUT_SECONDS,
        value_parser = clap::value_parser!(u64).range(1..),
        help_heading = HEADING_NETWORK
    )]
    pub timeout_seconds: u64,

    /// Language for `DuckDuckGo`'s `kl` search parameter (default: [`DEFAULT_SERP_LANG`]).
    ///
    /// This is **not** the CLI UI language — use `--ui-lang` / `locale`
    /// for human stderr localization (`en` / `pt-BR`). XDG key
    /// `default_lang` overrides when the CLI flag is left at the built-in
    /// default (GAP-E2E-51-013).
    #[arg(
        short = 'l',
        long = "lang",
        value_name = "LANG",
        global = true,
        default_value = DEFAULT_SERP_LANG
    )]
    pub language: String,

    /// Country for `DuckDuckGo`'s `kl` parameter (default: [`DEFAULT_SERP_COUNTRY`]).
    /// `--region` is accepted as an alias for backwards compatibility.
    /// XDG key `default_country` overrides when left at the built-in default.
    #[arg(
        short = 'c',
        long = "country",
        alias = "region",
        value_name = "CC",
        global = true,
        default_value = DEFAULT_SERP_COUNTRY
    )]
    pub country: String,

    /// Number of concurrent requests (default 5, maximum 20).
    ///
    /// Alias: `--max-concurrency` (rules-rust parallel checklist). Both names
    /// bind the same field and gate every fan-out (`JoinSet` + `Semaphore` in
    /// multi-query search and `--fetch-content`).
    #[arg(
        short = 'p',
        long = "parallel",
        visible_alias = "max-concurrency",
        value_name = "N",
        global = true,
        default_value_t = DEFAULT_PARALLELISM,
        value_parser = clap::value_parser!(u32).range(1..=MAX_PARALLELISM as i64),
        help_heading = HEADING_NETWORK
    )]
    pub parallelism: u32,

    /// Force one shared Chrome session for web+news (`--vertical all`) instead of
    /// dual multi-process Chromes (GAP-PAR-021). Lower RSS / anti-bot surface;
    /// higher wall-clock (web then news serial). Default: dual when `-p ≥ 2`.
    #[arg(
        long = "shared-session-verticals",
        action = ArgAction::SetTrue,
        global = true,
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
        global = true,
        value_parser = clap::value_parser!(u32).range(1..=MAX_PAGES as i64),
        help_heading = HEADING_NETWORK
    )]
    pub pages: u32,

    /// Number of additional retries on transient HTTP/network failures (0..=10).
    ///
    /// Applies truncated exponential full-jitter backoff (see `retry::RetryConfig`).
    /// Retries only idempotent GET search requests. Set to `0` or use
    /// `--disable-retry` as an incident kill switch. Default 2.
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
        global = true,
        action = ArgAction::SetTrue,
        help_heading = HEADING_NETWORK
    )]
    pub disable_retry: bool,

    /// Override HTML SERP base URL (trailing slash recommended). Default:
    /// `https://html.duckduckgo.com/html/`. For wiremock/tests only in normal
    /// use (GAP-SCRAPE-R2-009).
    #[arg(
        long = "base-url-html",
        value_name = "URL",
        global = true,
        help_heading = HEADING_NETWORK
    )]
    pub base_url_html: Option<String>,

    /// Override Lite base URL. Default: `https://lite.duckduckgo.com/lite/`.
    #[arg(
        long = "base-url-lite",
        value_name = "URL",
        global = true,
        help_heading = HEADING_NETWORK
    )]
    pub base_url_lite: Option<String>,

    /// Override SERP / warm-up base URL. Default: `https://duckduckgo.com/`.
    #[arg(
        long = "base-url-serp",
        value_name = "URL",
        global = true,
        help_heading = HEADING_NETWORK
    )]
    pub base_url_serp: Option<String>,

    /// Preferred endpoint: `html` (default) or `lite` (legacy value only).
    ///
    /// Production Chrome SERP always uses the canonical HTML page (GAP-WS-113).
    /// Passing `lite` does not open a pure-HTTP success path and is not
    /// remediation for anti-bot blocks.
    #[arg(long = "endpoint", value_enum, default_value_t = CliEndpoint::Html)]
    pub endpoint: CliEndpoint,

    /// Search vertical: `web`, `news`, or `all` (default **`all`** since v0.9.8).
    ///
    /// `news` and `all` require usable Chrome/Chromium and are routed
    /// EXCLUSIVELY through chromiumoxide/CDP — the `DuckDuckGo` news vertical
    /// (`ia=news&iar=news`) needs JavaScript rendering. Without a usable Chrome
    /// binary (or when built without the `chrome` feature) the CLI fails closed
    /// with exit 2 (GAP-WS-113). No product env kill-switch. Multi-query batches
    /// are accepted. Content fetch also applies to news article URLs when
    /// enabled (default on).
    #[arg(long = "vertical", value_enum, default_value_t = CliVertical::All, global = true)]
    pub vertical: CliVertical,

    /// Time filter: `d` (day), `w` (week), `m` (month), `y` (year). Default: no filter.
    #[arg(long = "time-filter", value_enum)]
    pub time_filter: Option<CliTimeFilter>,

    /// Safe-search: `off`, `moderate` (default) or `on`.
    #[arg(long = "safe-search", value_enum, default_value_t = CliSafeSearch::Moderate)]
    pub safe_search: CliSafeSearch,

    /// Chrome health probe: minimal reachability check via chromiumoxide/CDP
    /// and reports status + latency (+ cookie signals when available) as JSON,
    /// then exits. Requires usable Chrome; fails closed with exit 2 when Chrome
    /// is missing (no product env kill-switch).
    #[arg(
        long = "probe",
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub probe: bool,

    /// Forces a specific browser identity profile from the 12-identity pool.
    /// Default `auto` rotates adaptively on block (HTTP 202/403/429).
    /// When set, the chosen identity is used for the whole session.
    #[arg(long = "identity-profile", value_enum, default_value_t = CliIdentityProfile::Auto, global = true)]
    pub identity_profile: CliIdentityProfile,

    /// Multi-query only: emit per-query NDJSON as each search completes.
    /// Single-query mode ignores this flag (warning). Not a full event stream of
    /// individual SERP hits (GAP-WS-STREAM-NOOP-001 / STREAM-MULTI-001 v0.9.9).
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

    /// Affirms content extraction (default ON since v0.9.8; kept for scripts).
    /// Prefer omitting this flag or using `--no-fetch-content` to disable.
    /// Extraction uses Chrome/CDP + readability for web and news URLs
    /// (GAP-WS-113 / AGENT-READY-001). Limited by `--parallel` / `--per-host-limit`.
    #[arg(
        long = "fetch-content",
        global = true,
        action = ArgAction::SetTrue,
        help_heading = HEADING_CONTENT
    )]
    pub fetch_content: bool,

    /// Disables page content extraction (opt-out of the v0.9.8 agent-ready default).
    #[arg(
        long = "no-fetch-content",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "fetch_content",
        help_heading = HEADING_CONTENT
    )]
    pub no_fetch_content: bool,

    /// Max URLs to enrich per vertical under `--fetch-content` (`1..=50`, default 10).
    /// Agent-ready cost bound (GAP-SCRAPE-R-004).
    #[arg(
        long = "fetch-content-cap",
        value_name = "N",
        default_value_t = DEFAULT_FETCH_CONTENT_CAP,
        global = true,
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
        global = true,
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
        global = true,
        help_heading = HEADING_NETWORK
    )]
    pub proxy: Option<String>,

    /// Disables any proxy (explicit no-proxy; residual HTTP never inherits env proxies).
    #[arg(
        long = "no-proxy",
        action = ArgAction::SetTrue,
        conflicts_with = "proxy",
        global = true,
        help_heading = HEADING_NETWORK
    )]
    pub no_proxy: bool,

    /// Restricts UAs loaded from `user-agents.toml` to the current platform (linux/macos/windows).
    /// Only takes effect if the external TOML file is found; otherwise uses built-in defaults.
    #[arg(
        long = "match-platform-ua",
        global = true,
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
        global = true,
        value_parser = clap::value_parser!(u32).range(1..=MAX_PER_HOST_LIMIT as i64),
        help_heading = HEADING_CONTENT
    )]
    pub per_host_limit: u32,

    /// Manual path to the Chrome/Chromium executable used for all production
    /// network ops (search, news, deep-research, probe, pre-flight,
    /// fetch-content). Feature `chrome` is default. When omitted, the CLI
    /// auto-detects Chrome/Chromium via PATH and well-known install locations
    /// (GAP-SCRAPE-R2-003: no product env — use this flag only).
    #[arg(
        long = "chrome-path",
        value_name = "PATH",
        value_hint = ValueHint::ExecutablePath,
        global = true,
        help_heading = HEADING_CHROME
    )]
    pub chrome_path: Option<PathBuf>,

    /// Force headed Chrome (visible window / native display). Debug override.
    /// GAP-SCRAPE-R-007: CLI primary (not product env).
    #[arg(
        long = "chrome-visible",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "chrome_headless",
        help_heading = HEADING_CHROME
    )]
    pub chrome_visible: bool,

    /// Force headless Chrome (`--headless=new`). Overrides Xvfb auto path.
    #[arg(
        long = "chrome-headless",
        global = true,
        action = ArgAction::SetTrue,
        conflicts_with = "chrome_visible",
        help_heading = HEADING_CHROME
    )]
    pub chrome_headless: bool,

    /// Request private Xvfb headed mode on Linux (invisible headed anti-bot).
    #[arg(
        long = "chrome-xvfb",
        global = true,
        action = ArgAction::SetTrue,
        help_heading = HEADING_CHROME
    )]
    pub chrome_xvfb: bool,

    /// Write news SERP HTML after Chrome extract to this path (local debug only).
    /// GAP-SCRAPE-R-008: CLI path, not product env.
    #[arg(
        long = "dump-news-html",
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        global = true,
        help_heading = HEADING_CHROME
    )]
    pub dump_news_html: Option<PathBuf>,

    /// Disables colored output (respects `NO_COLOR` env var per no-color.org).
    #[arg(
        long = "no-color",
        action = ArgAction::SetTrue,
        help_heading = HEADING_OUTPUT
    )]
    pub no_color: bool,

    /// Disables the warm-up `GET https://duckduckgo.com/` request that
    /// populates session cookies before the first real query. v0.7.3 PR2.
    /// Default `false` (warm-up enabled). Disabling saves one request
    /// per invocation but increases CAPTCHA risk on macOS.
    #[arg(
        long = "no-warmup",
        action = ArgAction::SetTrue,
        help_heading = HEADING_CHROME
    )]
    pub no_warmup: bool,

    /// Disables persistence of the cookie jar to disk. Cookies live only
    /// in memory for the duration of the process. v0.7.3 PR2.
    /// Default `false` (cookies are persisted to
    /// `~/.config/duckduckgo-search-cli/cookies.json` on Unix or
    /// `%APPDATA%\duckduckgo-search-cli\cookies.json` on Windows).
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
    /// exits before running the real query. Requires usable Chrome; without
    /// Chrome fails closed with exit 2 (GAP-WS-113). Default `false`.
    #[arg(
        long = "probe-deep",
        action = ArgAction::SetTrue,
        help_heading = HEADING_DIAGNOSTICS
    )]
    pub probe_deep: bool,

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
    /// # Errors
    ///
    /// Returns [`crate::error::CliError::InvalidConfig`] if `--parallel` is zero or
    /// exceeds [`MAX_PARALLELISM`].
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
    /// Returns [`crate::error::CliError::InvalidConfig`] if `--pages` is zero or
    /// exceeds [`MAX_PAGES`].
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
    /// # Errors
    ///
    /// Returns [`crate::error::CliError::InvalidConfig`] if `--max-content-length` is zero or
    /// exceeds [`MAX_CONTENT_LENGTH_LIMIT`].
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
    /// method now lives on `RootArgs` and is the canonical entry point.
    /// `CliArgs` consumers must use `root.validate_global_timeout()`.
    /// Removed the duplicate here to avoid two implementations drifting
    /// (rule: `rules-rust-tratamento-de-erros` — single source of truth).
    #[allow(clippy::empty_line_after_doc_comments)]
    /// Validates that `--proxy`, when provided, is a parseable URL with a supported scheme.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CliError::InvalidConfig`] / [`crate::error::CliError::ProxyError`]
    /// if `--proxy` is not a valid URL or uses an unsupported scheme
    /// (only `http`, `https`, `socks5`, and `socks5h` are accepted).
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
    /// # Errors
    ///
    /// Returns [`crate::error::CliError::InvalidConfig`] if `--retries` exceeds [`MAX_RETRIES`].
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
    /// Returns [`crate::error::CliError::InvalidConfig`] if `--per-host-limit` is zero or
    /// exceeds [`MAX_PER_HOST_LIMIT`].
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
    /// # Errors
    ///
    /// Returns [`crate::error::CliError::InvalidConfig`] if `--timeout` is zero.
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// Helper: parses arguments via `RootArgs` and returns the full
    /// `RootArgs` (used by tests that need access to the global
    /// `global_timeout_seconds` field, which v0.7.10 B3 hoisted out
    /// of `CliArgs`).
    fn parse_root(argv: &[&str]) -> Result<RootArgs, clap::Error> {
        RootArgs::try_parse_from(argv)
    }

    /// Helper: parses arguments via root and extracts `CliArgs` (default flow = Buscar).
    /// Replicates the convenience behavior of tests prior to the introduction of the subcommand.
    fn parse_buscar(argv: &[&str]) -> Result<CliArgs, clap::Error> {
        let root = RootArgs::try_parse_from(argv)?;
        match root.subcommand {
            Some(Subcommand::Buscar(a)) => Ok(*a),
            Some(Subcommand::InitConfig(_))
            | Some(Subcommand::Completions(_))
            | Some(Subcommand::DeepResearch(_))
            | Some(Subcommand::Commands(_))
            | Some(Subcommand::Schema(_))
            | Some(Subcommand::Doctor(_))
            | Some(Subcommand::Locale(_))
            | Some(Subcommand::Man(_))
            | Some(Subcommand::Config(_)) => Err(clap::Error::raw(
                clap::error::ErrorKind::InvalidSubcommand,
                "subcomando nao-busca retornado em contexto que esperava busca",
            )),
            None => Ok(root.buscar),
        }
    }

    #[test]
    fn cli_passes_schema_validation() {
        // `debug_assert` do clap valida a struct em tempo de chamada.
        RootArgs::command().debug_assert();
    }

    #[test]
    fn parseia_query_simples() {
        let args = parse_buscar(&["bin", "rust async"]).expect("should parse");
        assert_eq!(args.queries, vec!["rust async".to_string()]);
        // Default is Auto (resolved at runtime via TTY detection).
        assert_eq!(args.format, CliOutputFormat::Auto);
        assert!(args.output_file.is_none());
        assert_eq!(args.timeout_seconds, 15);
        assert_eq!(args.language, "pt");
        assert_eq!(args.country, "br");
        assert_eq!(args.parallelism, DEFAULT_PARALLELISM);
        assert_eq!(args.pages, 1);
        assert_eq!(args.retries, 2);
        assert_eq!(args.endpoint, CliEndpoint::Html);
        assert!(args.time_filter.is_none());
        assert_eq!(args.safe_search, CliSafeSearch::Moderate);
        assert!(!args.stream_mode);
        assert!(args.queries_file.is_none());
        assert_eq!(args.verbose, 0);
        assert!(!args.quiet);
        assert!(!args.fetch_content);
        assert_eq!(args.max_content_length, DEFAULT_MAX_CONTENT_LENGTH);
        assert!(args.proxy.is_none());
        assert!(!args.no_proxy);
        // v0.7.10 B3 fix: `global_timeout_seconds` lives on `RootArgs`.
        // Verify the default via the root struct.
        let root = parse_root(&["bin", "q"]).unwrap();
        assert_eq!(root.global_timeout_seconds, DEFAULT_GLOBAL_TIMEOUT);
        assert!(!args.match_platform_ua);
    }

    #[test]
    fn parseia_fetch_content_e_max_content_length() {
        let args = parse_buscar(&[
            "bin",
            "--fetch-content",
            "--max-content-length",
            "500",
            "rust",
        ])
        .expect("should parse --fetch-content");
        assert!(args.fetch_content);
        assert_eq!(args.max_content_length, 500);
    }

    #[test]
    fn parseia_proxy_e_no_proxy_mutuamente_exclusivos() {
        let ok = parse_buscar(&[
            "bin",
            "--proxy",
            "http://user:pass@proxy.local:8080",
            "rust",
        ])
        .expect("should parse --proxy");
        assert_eq!(
            ok.proxy.as_deref(),
            Some("http://user:pass@proxy.local:8080")
        );
        assert!(!ok.no_proxy);

        let no = parse_buscar(&["bin", "--no-proxy", "rust"]).expect("should parse --no-proxy");
        assert!(no.no_proxy);
        assert!(no.proxy.is_none());

        let err = parse_buscar(&["bin", "--proxy", "http://x", "--no-proxy", "rust"]);
        assert!(err.is_err(), "--proxy + --no-proxy deve conflitar");
    }

    #[test]
    fn parseia_global_timeout() {
        // v0.7.10 B3 fix: `global_timeout_seconds` now lives on
        // `RootArgs` (global). Test via `parse_root` and the root
        // struct's field, not the inner `CliArgs`.
        let root = parse_root(&["bin", "--global-timeout", "30", "rust"]).unwrap();
        assert_eq!(root.global_timeout_seconds, 30);
    }

    #[test]
    fn validate_max_content_length_range() {
        let mut args = parse_buscar(&["bin", "q"]).unwrap();
        args.max_content_length = 0;
        assert!(args.validate_max_content_length().is_err());
        args.max_content_length = MAX_CONTENT_LENGTH_LIMIT + 1;
        assert!(args.validate_max_content_length().is_err());
        args.max_content_length = 5000;
        assert!(args.validate_max_content_length().is_ok());
    }

    #[test]
    fn validate_global_timeout_range() {
        // v0.7.10 B3 fix: `global_timeout_seconds` lives on `RootArgs`,
        // not on `CliArgs`. The clap `value_parser` rejects values
        // outside `1..=MAX_GLOBAL_TIMEOUT` at parse time (exit 2), so
        // this test only exercises the in-memory validation path for
        // an in-bounds value (sanity check).
        let root = parse_root(&["bin", "--global-timeout", "120", "q"]).unwrap();
        assert_eq!(root.global_timeout_seconds, 120);
        assert!(root.validate_global_timeout().is_ok());
        // `value_parser` rejects out-of-range at parse time, so we
        // verify that path here.
        assert!(
            parse_root(&["bin", "--global-timeout", "0", "q"]).is_err(),
            "clap value_parser must reject 0 (out of range 1..=3600)"
        );
        assert!(
            parse_root(&["bin", "--global-timeout", "99999", "q"]).is_err(),
            "clap value_parser must reject 99999 (out of range 1..=3600)"
        );
    }

    #[test]
    fn validate_proxy_accepts_supported_schemes() {
        let mut args = parse_buscar(&["bin", "q"]).unwrap();
        for ok in [
            "http://proxy:8080",
            "https://user:pass@proxy:8443",
            "socks5://127.0.0.1:9050",
            "socks5h://host:1080",
        ] {
            args.proxy = Some(ok.to_string());
            assert!(
                args.validate_proxy().is_ok(),
                "proxy {ok:?} deveria ser aceito"
            );
        }
        args.proxy = Some("ftp://proxy".to_string());
        assert!(args.validate_proxy().is_err());
        args.proxy = Some("nao-eh-uma-url".to_string());
        assert!(args.validate_proxy().is_err());
        args.proxy = None;
        assert!(args.validate_proxy().is_ok());
    }

    #[test]
    fn parses_resilience_and_filter_flags() {
        let args = parse_buscar(&[
            "bin",
            "--pages",
            "3",
            "--retries",
            "5",
            "--endpoint",
            "lite",
            "--time-filter",
            "w",
            "--safe-search",
            "on",
            "rust",
        ])
        .expect("should parse resilience flags");
        assert_eq!(args.pages, 3);
        assert_eq!(args.retries, 5);
        assert_eq!(args.endpoint, CliEndpoint::Lite);
        assert_eq!(args.time_filter, Some(CliTimeFilter::W));
        assert_eq!(args.safe_search, CliSafeSearch::On);
    }

    #[test]
    fn parseia_vertical_default_e_customizado() {
        let default_args = parse_buscar(&["bin", "q"]).unwrap();
        assert_eq!(default_args.vertical, CliVertical::All);

        let news_args = parse_buscar(&["bin", "--vertical", "news", "q"]).unwrap();
        assert_eq!(news_args.vertical, CliVertical::News);

        let all_args = parse_buscar(&["bin", "--vertical", "all", "q"]).unwrap();
        assert_eq!(all_args.vertical, CliVertical::All);

        assert!(
            parse_buscar(&["bin", "--vertical", "banana", "q"]).is_err(),
            "unknown --vertical value must be rejected by clap"
        );
    }

    #[test]
    fn validate_pages_accepts_range_and_rejects_invalid() {
        let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
        for v in [1u32, 2, 5] {
            args.pages = v;
            assert!(args.validate_pages().is_ok(), "pages {v}");
        }
        args.pages = 0;
        assert!(args.validate_pages().is_err());
        args.pages = 6;
        assert!(args.validate_pages().is_err());
    }

    #[test]
    fn validate_retries_rejects_above_max() {
        let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
        args.retries = 0;
        assert!(args.validate_retries().is_ok());
        args.retries = 10;
        assert!(args.validate_retries().is_ok());
        args.retries = 11;
        assert!(args.validate_retries().is_err());
    }

    #[test]
    fn parseia_multiplas_queries_posicionais() {
        let args = parse_buscar(&["bin", "rust async", "tokio runtime", "async channels"])
            .expect("should parse multiple queries");
        assert_eq!(
            args.queries,
            vec![
                "rust async".to_string(),
                "tokio runtime".to_string(),
                "async channels".to_string(),
            ]
        );
    }

    #[test]
    fn parseia_flags_customizadas() {
        let args = parse_buscar(&[
            "bin",
            "--num",
            "10",
            "--format",
            "json",
            "--timeout",
            "30",
            "--lang",
            "en",
            "--country",
            "us",
            "--parallel",
            "8",
            "--verbose",
            "teste de busca",
        ])
        .expect("should parse with flags");
        assert_eq!(args.queries, vec!["teste de busca".to_string()]);
        assert_eq!(args.num_results, Some(10));
        assert_eq!(args.timeout_seconds, 30);
        assert_eq!(args.language, "en");
        assert_eq!(args.country, "us");
        assert_eq!(args.parallelism, 8);
        assert_eq!(args.verbose, 1);
    }

    #[test]
    fn ui_lang_flag_is_global_and_distinct_from_serp_lang() {
        let root = parse_root(&["bin", "--ui-lang", "pt-BR", "hello"])
            .expect("should parse --ui-lang");
        assert_eq!(root.ui_lang.as_deref(), Some("pt-BR"));
        // SERP language stays independent (default pt).
        assert_eq!(root.buscar.language, "pt");
        // Accepted after a subcommand via global = true.
        let root2 = parse_root(&["bin", "locale", "--ui-lang", "en"]).expect("locale + ui-lang");
        assert!(matches!(root2.subcommand, Some(Subcommand::Locale(_))));
        assert_eq!(root2.ui_lang.as_deref(), Some("en"));
    }

    #[test]
    fn parseia_flag_output_curta_e_longa() {
        let args = parse_buscar(&["bin", "-o", "/tmp/saida.json", "q"]).expect("should parse -o");
        assert_eq!(
            args.output_file.as_deref(),
            Some(std::path::Path::new("/tmp/saida.json"))
        );

        let args2 = parse_buscar(&["bin", "--output", "/tmp/x.md", "--format", "markdown", "q"])
            .expect("should parse --output");
        assert_eq!(
            args2.output_file.as_deref(),
            Some(std::path::Path::new("/tmp/x.md"))
        );
        assert_eq!(args2.format, CliOutputFormat::Markdown);

        let args_md = parse_buscar(&["bin", "-f", "md", "q"]).expect("should parse -f md alias");
        assert_eq!(args_md.format, CliOutputFormat::Markdown);

        assert!(
            parse_buscar(&["bin", "-f", "xml", "q"]).is_err(),
            "unknown -f value must be rejected by ValueEnum at parse time"
        );
    }

    #[test]
    fn parseia_arquivo_queries_e_stream() {
        let args = parse_buscar(&["bin", "--queries-file", "queries.txt", "--stream"])
            .expect("should parse --queries-file and --stream");
        assert!(args.stream_mode);
        assert_eq!(
            args.queries_file.as_deref(),
            Some(std::path::Path::new("queries.txt"))
        );
        assert!(args.queries.is_empty());
    }

    #[test]
    fn parse_format_ndjson_enables_stream_alias() {
        // GAP-E2E-51-005: `-f ndjson` is accepted and marks the stream alias.
        let args = parse_buscar(&["bin", "-f", "ndjson", "q1", "q2"])
            .expect("should parse -f ndjson");
        assert_eq!(args.format, CliOutputFormat::Ndjson);
        assert!(args.format.enables_stream_mode());
        // The clap flag itself stays false; build_config ORs the alias in.
        assert!(!args.stream_mode);
    }

    #[test]
    fn parse_config_get_accepts_positional_and_flag_key() {
        // GAP-E2E-51-003: both `config get KEY` and `config get --key KEY`.
        let root = parse_root(&["bin", "config", "get", "ui_lang"]).expect("positional get");
        match root.subcommand {
            Some(Subcommand::Config(ConfigCmd::Get(args))) => {
                assert_eq!(args.key(), "ui_lang");
            }
            other => panic!("expected config get, got {other:?}"),
        }

        let root = parse_root(&["bin", "config", "get", "--key", "chrome_path"]).expect("flag get");
        match root.subcommand {
            Some(Subcommand::Config(ConfigCmd::Get(args))) => {
                assert_eq!(args.key(), "chrome_path");
            }
            other => panic!("expected config get --key, got {other:?}"),
        }
    }

    #[test]
    fn parse_config_set_accepts_positional_and_flag_forms() {
        // GAP-E2E-51-003: `config set KEY VALUE` and `config set --key K --value V`.
        let root = parse_root(&["bin", "config", "set", "ui_lang", "en"]).expect("positional set");
        match root.subcommand {
            Some(Subcommand::Config(ConfigCmd::Set(args))) => {
                assert_eq!(args.key(), "ui_lang");
                assert_eq!(args.value(), "en");
            }
            other => panic!("expected config set positional, got {other:?}"),
        }

        let root = parse_root(&[
            "bin",
            "config",
            "set",
            "--key",
            "ui_lang",
            "--value",
            "pt-BR",
        ])
        .expect("flag set");
        match root.subcommand {
            Some(Subcommand::Config(ConfigCmd::Set(args))) => {
                assert_eq!(args.key(), "ui_lang");
                assert_eq!(args.value(), "pt-BR");
            }
            other => panic!("expected config set flags, got {other:?}"),
        }
    }

    #[test]
    fn parse_config_unset_accepts_positional_and_flag_key() {
        let root = parse_root(&["bin", "config", "unset", "proxy_url"]).expect("positional unset");
        match root.subcommand {
            Some(Subcommand::Config(ConfigCmd::Unset(args))) => {
                assert_eq!(args.key(), "proxy_url");
            }
            other => panic!("expected config unset, got {other:?}"),
        }

        let root =
            parse_root(&["bin", "config", "unset", "--key", "proxy_url"]).expect("flag unset");
        match root.subcommand {
            Some(Subcommand::Config(ConfigCmd::Unset(args))) => {
                assert_eq!(args.key(), "proxy_url");
            }
            other => panic!("expected config unset --key, got {other:?}"),
        }
    }

    #[test]
    fn parse_config_effective_subcommand() {
        let root = parse_root(&["bin", "config", "effective"]).expect("config effective");
        assert!(matches!(
            root.subcommand,
            Some(Subcommand::Config(ConfigCmd::Effective(_)))
        ));
    }

    #[test]
    fn verbose_e_quiet_sao_mutuamente_exclusivos() {
        let result = parse_buscar(&["bin", "--verbose", "--quiet", "query qualquer"]);
        assert!(result.is_err(), "verbose + quiet deve failurer a validação");
    }

    #[test]
    fn verbose_curto_acumula_via_arg_action_count() {
        let v1 = parse_buscar(&["bin", "-v", "q"]).expect("-v deve parsear");
        assert_eq!(v1.verbose, 1, "-v single deve produzir verbose == 1");
        let vv = parse_buscar(&["bin", "-vv", "q"]).expect("-vv deve parsear");
        assert_eq!(vv.verbose, 2, "-vv deve produzir verbose == 2");
        let vvv = parse_buscar(&["bin", "-vvv", "q"]).expect("-vvv deve parsear");
        assert_eq!(vvv.verbose, 3, "-vvv deve produzir verbose == 3");
        let long = parse_buscar(&["bin", "--verbose", "--verbose", "q"])
            .expect("--verbose repetido deve parsear");
        assert_eq!(long.verbose, 2, "--verbose repetido deve acumular para 2");
    }

    #[test]
    fn max_concurrency_alias_sets_parallelism() {
        let args = parse_buscar(&["bin", "rust", "--max-concurrency", "7"])
            .expect("--max-concurrency alias must parse");
        assert_eq!(args.parallelism, 7);
        let short = parse_buscar(&["bin", "rust", "-p", "3"]).expect("-p still works");
        assert_eq!(short.parallelism, 3);
    }

    #[test]
    fn shared_session_verticals_flag_parses() {
        let default = parse_buscar(&["bin", "rust"]).expect("default parse");
        assert!(
            !default.shared_session_verticals,
            "default must prefer dual multi-process verticals"
        );
        let shared = parse_buscar(&["bin", "rust", "--shared-session-verticals"])
            .expect("--shared-session-verticals must parse");
        assert!(shared.shared_session_verticals);
    }

    #[test]
    fn validate_parallelism_accepts_allowed_range() {
        let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
        for value in [1u32, 5, 10, MAX_PARALLELISM] {
            args.parallelism = value;
            assert!(
                args.validate_parallelism().is_ok(),
                "--parallel {value} deveria ser aceito"
            );
        }
    }

    #[test]
    fn validate_parallelism_rejects_invalid_values() {
        let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
        args.parallelism = 0;
        assert!(args.validate_parallelism().is_err());
        args.parallelism = MAX_PARALLELISM + 1;
        assert!(args.validate_parallelism().is_err());
        args.parallelism = 100;
        assert!(args.validate_parallelism().is_err());
    }

    #[test]
    fn parses_init_config_subcommand_with_flags() {
        let root = RootArgs::try_parse_from(["bin", "init-config", "--force", "--dry-run"])
            .expect("should parse init-config");
        let Some(Subcommand::InitConfig(args)) = root.subcommand else {
            panic!("expected InitConfig subcommand");
        };
        assert!(args.force);
        assert!(args.dry_run);
    }

    #[test]
    fn parses_init_config_subcommand_without_flags() {
        let root = RootArgs::try_parse_from(["bin", "init-config"])
            .expect("should parse init-config without flags");
        let Some(Subcommand::InitConfig(args)) = root.subcommand else {
            panic!("expected InitConfig subcommand");
        };
        assert!(!args.force);
        assert!(!args.dry_run);
    }

    #[test]
    fn parses_doctor_strict_flag() {
        let plain = RootArgs::try_parse_from(["bin", "doctor"]).expect("doctor");
        let Some(Subcommand::Doctor(args)) = plain.subcommand else {
            panic!("expected Doctor");
        };
        assert!(!args.strict);

        let strict = RootArgs::try_parse_from(["bin", "doctor", "--strict"]).expect("doctor --strict");
        let Some(Subcommand::Doctor(args)) = strict.subcommand else {
            panic!("expected Doctor");
        };
        assert!(args.strict);
    }

    #[test]
    fn parses_explicit_buscar_subcommand() {
        let root = RootArgs::try_parse_from(["bin", "buscar", "rust"])
            .expect("should parse buscar subcommand");
        let Some(Subcommand::Buscar(args)) = root.subcommand else {
            panic!("expected Buscar subcommand");
        };
        assert_eq!(args.queries, vec!["rust".to_string()]);
    }

    #[test]
    fn search_subcommand_stays_small_when_boxed() {
        // Regression guarantee: Subcommand::Buscar is still Box — clippy lint large_enum.
        // v0.7.0 added DeepResearchArgs; the largest variant dictates the enum
        // size. We assert the enum stays under 256 bytes (deep-research fields
        // include 5 strings + 1 PathBuf, well below that cap).
        let enum_size = std::mem::size_of::<Subcommand>();
        assert!(
            enum_size <= 256,
            "Subcommand grew unexpectedly: {enum_size} bytes"
        );
    }

    #[test]
    fn parse_without_subcommand_uses_search_flatten() {
        let root = RootArgs::try_parse_from(["bin", "rust async"])
            .expect("should parse without subcommand");
        assert!(root.subcommand.is_none());
        assert_eq!(root.buscar.queries, vec!["rust async".to_string()]);
    }

    #[test]
    fn parseia_per_host_limit() {
        let args = parse_buscar(&["bin", "--per-host-limit", "5", "q"]).unwrap();
        assert_eq!(args.per_host_limit, 5);
        let default = parse_buscar(&["bin", "q"]).unwrap();
        assert_eq!(default.per_host_limit, DEFAULT_PER_HOST_LIMIT);
    }

    #[test]
    fn validate_per_host_limit_range() {
        let mut args = parse_buscar(&["bin", "q"]).unwrap();
        args.per_host_limit = 0;
        assert!(args.validate_per_host_limit().is_err());
        args.per_host_limit = MAX_PER_HOST_LIMIT + 1;
        assert!(args.validate_per_host_limit().is_err());
        args.per_host_limit = 2;
        assert!(args.validate_per_host_limit().is_ok());
    }

    #[test]
    fn validate_timeout_seconds_rejects_zero() {
        let mut args = parse_buscar(&["bin", "q"]).unwrap();
        args.timeout_seconds = 0;
        assert!(args.validate_timeout_seconds().is_err());
        args.timeout_seconds = 1;
        assert!(args.validate_timeout_seconds().is_ok());
        args.timeout_seconds = 15;
        assert!(args.validate_timeout_seconds().is_ok());
    }

    #[test]
    fn buscar_subcommand_hidden_from_root_help() {
        // GAP-WS-56: Buscar inflates the --help output because it flattens
        // every CliArgs flag. Hiding it removes the duplicate listing since
        // the no-subcommand invocation already exposes all those flags.
        let mut cmd = RootArgs::command();
        let help = cmd.render_long_help().to_string();
        assert!(
            !help.contains("buscar"),
            "buscar subcommand must be hidden from root --help, found: {help}"
        );

        // The subcommand must still be invokable even though hidden from --help.
        let root = RootArgs::try_parse_from(["bin", "buscar", "rust"])
            .expect("buscar subcommand must remain invokable when explicit");
        match root.subcommand {
            Some(Subcommand::Buscar(args)) => {
                assert_eq!(args.queries, vec!["rust".to_string()]);
            }
            other => panic!("expected Buscar subcommand, got {other:?}"),
        }
    }

    // v0.7.9 GAP-WS-59: --allow-lite-fallback and --pre-flight are
    // declared on `RootArgs` with `global = true` so they are accepted
    // both before AND after subcommands such as `deep-research`.
    // The pre-v0.7.9 symptom was an `unexpected argument` exit 2 when
    // the flag was passed after a positional subcommand.
    #[test]
    fn allow_lite_fallback_is_global() {
        // No-subcommand mode: the global flag is parsed and the
        // query is stored in the `buscar` flatten (no `Subcommand` set).
        let pre = RootArgs::try_parse_from(["bin", "--allow-lite-fallback", "rust"])
            .expect("--allow-lite-fallback must parse before any subcommand");
        assert!(pre.allow_lite_fallback);
        assert!(!pre.pre_flight);
        assert!(
            pre.subcommand.is_none(),
            "no subcommand expected, got {:?}",
            pre.subcommand
        );
        assert_eq!(pre.buscar.queries, vec!["rust".to_string()]);

        let post = RootArgs::try_parse_from([
            "bin",
            "deep-research",
            "--allow-lite-fallback",
            "--pre-flight",
            "rust",
        ])
        .expect("globals must be accepted after deep-research subcommand");
        assert!(post.allow_lite_fallback);
        assert!(post.pre_flight);
        match post.subcommand {
            Some(Subcommand::DeepResearch(_)) => {}
            other => panic!("expected DeepResearch subcommand, got {other:?}"),
        }

        let neither = RootArgs::try_parse_from(["bin", "rust"]).expect("baseline");
        assert!(!neither.allow_lite_fallback);
        assert!(!neither.pre_flight);
    }

    // v0.7.10 P4 #16: --require-results flag is parsed and defaults to
    // false. Ensures that pipelines which don't pass the flag preserve
    // the v0.7.0–v0.7.9 behavior of returning exit 0 even with zero
    // aggregated results.
    #[test]
    fn deep_research_require_results_flag_parses() {
        // Default — flag absent → false.
        let pre = RootArgs::try_parse_from(["bin", "deep-research", "rust"]).expect("default");
        if let Some(Subcommand::DeepResearch(dr)) = pre.subcommand {
            assert!(!dr.require_results, "default must be false");
        } else {
            panic!("expected DeepResearch subcommand");
        }

        // Flag present → true.
        let post = RootArgs::try_parse_from(["bin", "deep-research", "--require-results", "rust"])
            .expect("flag present");
        if let Some(Subcommand::DeepResearch(dr)) = post.subcommand {
            assert!(
                dr.require_results,
                "--require-results must set bool to true"
            );
        } else {
            panic!("expected DeepResearch subcommand");
        }
    }

    // v0.9.0 GAP-WS-106 Sintoma B: `-q` agora é `global = true` e pode
    // aparecer APÓS o subcomando `deep-research` (before abortava com
    // `unexpected argument`).
    #[test]
    fn quiet_global_aceito_apos_subcomando() {
        let r = RootArgs::try_parse_from(["bin", "deep-research", "rust", "-q"])
            .expect("-q deve ser aceito apos deep-research (global)");
        assert!(
            r.buscar.quiet,
            "quiet deve ser true quando -q aparece apos subcomando"
        );
    }

    // v0.9.0 GAP-WS-106 Sintoma B + AGENT-READY L-06: transport flags global.
    #[test]
    fn chrome_path_accepted_after_deep_research_global() {
        let r = RootArgs::try_parse_from([
            "bin",
            "deep-research",
            "rust",
            "--chrome-path",
            "/usr/lib64/chromium-browser/chromium-browser",
            "--no-news",
        ])
        .expect("--chrome-path must be accepted after deep-research (global L-06)");
        assert_eq!(
            r.buscar.chrome_path.as_deref(),
            Some(std::path::Path::new(
                "/usr/lib64/chromium-browser/chromium-browser"
            ))
        );
    }

    #[test]
    fn output_global_aceito_apos_subcomando() {
        let r = RootArgs::try_parse_from(["bin", "deep-research", "rust", "-o", "/tmp/x.json"])
            .expect("-o deve ser aceito apos deep-research (global)");
        assert_eq!(
            r.buscar.output_file.as_deref(),
            Some(std::path::Path::new("/tmp/x.json"))
        );
    }

    // v0.9.0 GAP-WS-106 Sintoma A: `is_known_global_flag` cobre as 8 flags
    // hoisted + verbose + todas as long flags locais de CliArgs.
    #[test]
    fn is_known_global_flag_cobre_todas_as_flags_do_root_parser() {
        for short in ["q", "o", "n", "f", "l", "c", "t", "p", "v"] {
            assert!(
                is_known_global_flag(short),
                "short -{short} deve ser conhecida"
            );
        }
        for long in [
            "quiet",
            "output",
            "num",
            "format",
            "lang",
            "country",
            "timeout",
            "parallel",
            "max-concurrency",
            "verbose",
            "queries-file",
            "pages",
            "retries",
            "endpoint",
            "vertical",
            "time-filter",
            "safe-search",
            "probe",
            "identity-profile",
            "stream",
            "fetch-content",
            "max-content-length",
            "proxy",
            "no-proxy",
            "match-platform-ua",
            "per-host-limit",
            "chrome-path",
            "no-color",
            "no-warmup",
            "no-cookie-persistence",
            "cookies-path",
            "probe-deep",
            "seed",
            "config",
        ] {
            assert!(
                is_known_global_flag(long),
                "long --{long} deve ser conhecida"
            );
        }
        assert!(
            !is_known_global_flag("zzz"),
            "flag inexistente nao deve casar"
        );
    }
}
