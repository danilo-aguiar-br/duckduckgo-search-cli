// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (Chrome CDP connection, feature-gated)
//! Cross-platform detection and launch of headless Chrome via `chromiumoxide`.
//!
//! This module is only compiled with the `chrome` feature, enabled via
//! `cargo build --features chrome`. In default mode (without feature) the binary
//! has NO dependency on chromiumoxide/tempfile/futures — zero overhead.
//!
//! ## Layout (Pass 36 / GAP-COMP-001 — SRP split)
//!
//! | Submodule | Responsibility |
//! |-----------|----------------|
//! | `detect` | Path candidates, channel, version probe, `--no-sandbox` heuristic |
//! | `xvfb` | Private Xvfb display RAII (Linux) |
//! | `session` | [`ChromeBrowser`] launch / stealth flags / one-shot reap |
//! | `extract` | CDP navigation + HTML/text extract helpers |
//! | `stealth` | Static CDP stealth script payloads |
//!
//! ## Process Cleanup and Safety (GAP-WS-LIFECYCLE-001 / one-shot)
//!
//! Prefer async [`ChromeBrowser::shutdown`]; [`Drop`] force-reaps the Chrome
//! process tree, Xvfb, and profile dir. Contract: after the CLI exits, no
//! Chromium/Xvfb/profile from **this** invocation remains.
//!
//! ## Audio mute operational standard (ADR-0026 / GAP-CHROME-MUTE-001 + MUTE-002)
//!
//! Every Chrome process is launched with [`CHROME_MUTE_AUDIO_FLAG`] and
//! [`CHROME_AUTOPLAY_POLICY_FLAG`] (safe defaults **and** [`flags_stealth`]).
//! `ChromeBrowser::launch` fail-closes if mute is missing. No opt-out.
//!
//! **GAP-CHROME-MUTE-002:** flags are normalized via
//! `session::chromiumoxide_arg_token` before `BrowserConfig::args` so
//! chromiumoxide does not emit `----mute-audio` (ignored by Chromium).

use std::time::Duration;

/// Cooperative close/wait budget before forced kill (L-08).
pub(crate) const SHUTDOWN_COOPERATIVE_DEADLINE: Duration = Duration::from_secs(3);

/// Base delay after SERP warm-up navigation before target navigation (ms).
pub(crate) const SERP_WARMUP_BASE_MS: u64 = 800;
/// Jitter modulus applied to warm-up delay from URL length (ms).
pub(crate) const SERP_WARMUP_JITTER_MS: u64 = 700;

/// Post-navigation settle before reading `document.body.innerText` (ms).
///
/// Named policy constant (GAP-SCRAPE-009) — not user XDG config.
pub(crate) const CONTENT_JS_SETTLE_MS: u64 = 1500;

/// Maximum wall time for a single Chrome content navigation + extract (seconds).
///
/// Named policy constant (GAP-SCRAPE-016). Also used for pool cold-start launch
/// timeout (GAP-SCRAPE-R-002).
pub(crate) const CONTENT_CHROME_TIMEOUT_SECS: u64 = 30;

/// SERP HTML poll interval while waiting for result markers (ms).
///
/// GAP-SCRAPE-R-003.
pub(crate) const SERP_POLL_INTERVAL_MS: u64 = 500;
/// Max SERP poll attempts at [`SERP_POLL_INTERVAL_MS`] (~8s window).
pub(crate) const SERP_POLL_ATTEMPTS: u32 = 16;
/// Minimum news-hydration poll budget (seconds), clamped by operation timeout.
pub(crate) const SERP_POLL_MIN_BUDGET_SECS: u64 = 14;
/// Extra settle after news selectors match (ms) — news.js XHR may still paint.
pub(crate) const NEWS_POST_READY_SETTLE_MS: u64 = 1200;

/// Minimum character count per line kept by the cleaning pipeline.
///
/// SSOT: [`crate::validation::limits::MIN_LINE_LENGTH`] (GAP-SCRAPE-010).
pub(crate) use crate::validation::limits::MIN_LINE_LENGTH;

/// chromiumoxide's `DEFAULT_ARGS` minus `enable-automation` (GAP-WS-108 v0.9.2),
/// plus ADR-0026 mute-audio operational defaults.
///
/// `chromiumoxide` injects `--enable-automation` by default, which sets the
/// `navigator.webdriver` legacy flag and is detectable by Cloudflare via the
/// `chrome.runtime` object, CDP exposure, and the infobar token. We call
/// `.disable_default_args()` and re-add the safe subset below — every arg
/// from `chromiumoxide::browser::config::DEFAULT_ARGS` EXCEPT `enable-automation`,
/// plus [`CHROME_MUTE_AUDIO_FLAG`] / [`CHROME_AUTOPLAY_POLICY_FLAG`] so every
/// launch path is silent even if `flags_stealth` is refactored incorrectly.
pub(crate) const CHROMIUMOXIDE_SAFE_DEFAULTS: &[&str] = &[
    "--disable-background-networking",
    "--enable-features=NetworkService,NetworkServiceInProcess",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-breakpad",
    "--disable-client-side-phishing-detection",
    "--disable-component-extensions-with-background-pages",
    "--disable-default-apps",
    "--disable-dev-shm-usage",
    // GAP-REL-003: `--disable-features` deliberately does NOT appear here.
    // Chromium's `CommandLine` keeps one value per switch name, so this entry
    // (`TranslateUI`) and the stealth entry (`AutomationControlled,TranslateUI`)
    // used to collide, and only concatenation order decided which survived.
    // `flags_stealth` is the single source for this switch and already carries
    // `TranslateUI`; every launch path concatenates both lists, so nothing is
    // lost. `ensure_no_duplicate_valued_switch` fails the launch if the split
    // ever comes back.
    "--disable-hang-monitor",
    "--disable-ipc-flooding-protection",
    "--disable-popup-blocking",
    "--disable-prompt-on-repost",
    "--disable-renderer-backgrounding",
    "--disable-sync",
    "--force-color-profile=srgb",
    "--metrics-recording-only",
    "--no-first-run",
    "--password-store=basic",
    "--use-mock-keychain",
    "--enable-blink-features=IdleDetection",
    "--lang=en_US",
    // ADR-0026 / GAP-CHROME-MUTE-001 — operational standard (also in flags_stealth).
    CHROME_MUTE_AUDIO_FLAG,
    CHROME_AUTOPLAY_POLICY_FLAG,
];

/// Chromium switch that mutes host speakers for automated Chrome (ADR-0026).
///
/// SSOT string. Always present in `CHROMIUMOXIDE_SAFE_DEFAULTS` and
/// [`flags_stealth`]. Deep-research / SERP / fetch-content must never play
/// page media on the operator's speakers.
pub const CHROME_MUTE_AUDIO_FLAG: &str = "--mute-audio";

/// Chromium autoplay policy: media requires a user gesture (ADR-0026 defense-in-depth).
pub const CHROME_AUTOPLAY_POLICY_FLAG: &str = "--autoplay-policy=document-user-activation-required";

// Stealth CDP payloads (Pass 35).
mod stealth;

// Pass 36 SRP split (GAP-COMP-001).
mod detect;
mod extract;
mod session;
mod xvfb;

// Transport policy re-export (available when chrome feature is on).
pub use crate::chrome_policy::{
    chrome_disabled_by_env, http_test_harness_active, require_chrome_transport, HTTP_TEST_ENV,
    NO_CHROME_ENV,
};

// Public facade — preserve historical `browser::` paths.
pub use detect::{
    chrome_candidate_paths, classify_chrome_channel, clear_chrome_version_cache, detect_chrome,
    detect_chrome_major_version, detect_chrome_major_version_async, detect_chrome_resolved,
    needs_no_sandbox, resolve_chrome_candidate, ChromeChannel, ResolvedChrome,
};
pub use extract::{
    extract_html_with_chrome, extract_news_html_with_chrome, extract_text_with_chrome,
    wait_for_selector_with_chrome,
};
pub use session::{flags_stealth, set_chrome_display_cli, ChromeBrowser, ChromeDisplayCli};
// ADR-0026 mute SSOT re-exported for tests / external audit of launch policy.

#[cfg(test)]
mod tests;
