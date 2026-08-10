// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (CLI enums via clap, zero runtime)
//! Search / `buscar` clap enums (SRP split from `buscar_args`).

use clap::ValueEnum;

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

/// Maps a flag token (short or long, without leading dashes) to its canonical
/// **long** name.
///
/// v1.0.3 GAP-AGENT-HINT: the misplaced-flag tip in [`crate::run`] used to
/// render `--{token}` verbatim. For a short flag that produced an invalid
/// hybrid — a user typing `-f` was told to use `--f`, which clap rejects with
/// exit 2 just like the original mistake. Returning the canonical long form
/// makes the tip actionable.
///
/// Returns `None` when `arg` has no short form, i.e. it is already canonical.
/// Callers render `canonical_long_flag(raw).unwrap_or(raw)`.
///
/// The pairs mirror [`is_known_global_flag`]; keep both in sync.
#[must_use]
pub fn canonical_long_flag(arg: &str) -> Option<&'static str> {
    Some(match arg {
        "q" | "quiet" => "quiet",
        "o" | "output" => "output",
        "n" | "num" => "num",
        "f" | "format" => "format",
        "l" | "lang" => "lang",
        "c" | "country" => "country",
        "t" | "timeout" => "timeout",
        "p" | "parallel" => "parallel",
        "v" | "verbose" => "verbose",
        _ => return None,
    })
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
