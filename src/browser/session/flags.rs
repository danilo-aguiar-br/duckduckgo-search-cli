// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure / I/O-light (Chrome launch flags, mute policy, UA override)
//! Chrome launch flags, display policy, and CDP UA helpers (SRP split from `session`).

use super::{CHROME_AUTOPLAY_POLICY_FLAG, CHROME_MUTE_AUDIO_FLAG};
use crate::error::CliError;
use std::sync::{Mutex, OnceLock};

pub(super) fn chrome_launch_gate() -> &'static tokio::sync::Mutex<()> {
    static GATE: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    GATE.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// CLI-driven Chrome display overrides (GAP-SCRAPE-R-007).
///
/// Product config must come from clap flags (`--chrome-visible` /
/// `--chrome-headless` / `--chrome-xvfb`), not product environment variables.
/// Installed once per process from `build_config` / `run`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChromeDisplayCli {
    /// `--chrome-visible`
    pub force_visible: bool,
    /// `--chrome-headless`
    pub force_headless: bool,
    /// `--chrome-xvfb`
    pub force_xvfb: bool,
}

static CHROME_DISPLAY_CLI: Mutex<ChromeDisplayCli> = Mutex::new(ChromeDisplayCli {
    force_visible: false,
    force_headless: false,
    force_xvfb: false,
});

/// Install process-wide Chrome display policy from CLI/Config.
pub fn set_chrome_display_cli(policy: ChromeDisplayCli) {
    if let Ok(mut guard) = CHROME_DISPLAY_CLI.lock() {
        *guard = policy;
    }
}

/// Current Chrome display policy (CLI primary).
#[must_use]
pub(crate) fn chrome_display_cli() -> ChromeDisplayCli {
    CHROME_DISPLAY_CLI
        .lock()
        .map(|g| *g)
        .unwrap_or_else(|poisoned| *poisoned.into_inner())
}

/// Allowlisted process environment keys for Chrome CDP launch (GAP-SECDEV-008).
///
/// chromiumoxide merges `process_envs` onto the inherited environment (it does
/// not `env_clear`). We still only **set** safe keys from the parent so secrets
/// are never *intentionally* re-exported via the builder. Operators must not
/// run the CLI with unrelated secrets in the process environment.
pub(super) fn chrome_launch_env_allowlist() -> std::collections::HashMap<String, String> {
    const KEYS: &[&str] = &[
        "PATH",
        "HOME",
        "USER",
        "LOGNAME",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "TMPDIR",
        "TMP",
        "TEMP",
        "XDG_RUNTIME_DIR",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "DBUS_SESSION_BUS_ADDRESS",
        "LD_LIBRARY_PATH",
        "SystemRoot",
        "WINDIR",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "HOMEDRIVE",
        "HOMEPATH",
    ];
    let mut map = std::collections::HashMap::with_capacity(KEYS.len());
    for key in KEYS {
        if let Ok(val) = std::env::var(key) {
            if !val.is_empty() {
                map.insert((*key).to_string(), val);
            }
        }
    }
    map
}

/// Strip a leading `--` so chromiumoxide does not emit `----flag` (GAP-CHROME-MUTE-002).
///
/// chromiumoxide `ArgsBuilder` always formats switches as `--{key}` (or
/// `--{key}={values}`). If the CLI passes a full Chromium token like
/// `--mute-audio`, the crate treats the entire string as the *key* and
/// prepends another `--`, producing `----mute-audio`. Chromium **silently
/// ignores** unknown triple/quad-dash tokens — so mute, autoplay policy,
/// stealth `disable-features`, window position, etc. never applied.
///
/// Canonical SSOT strings keep the human/Chromium form (`--mute-audio`).
/// This function is the **only** boundary adapter into chromiumoxide.
#[must_use]
pub(crate) fn chromiumoxide_arg_token(flag: &str) -> &str {
    flag.strip_prefix("--").unwrap_or(flag)
}

/// Render the argv token chromiumoxide would emit for a CLI flag string.
///
/// Mirrors `chromiumoxide` `ArgsBuilder::into_iter` for empty-value keys
/// (boolean switches and already-`key=value` tokens). Used by tests and
/// fail-closed checks so we assert **process** form, not only source form.
#[must_use]
pub(crate) fn chromiumoxide_rendered_arg(flag: &str) -> String {
    format!("--{}", chromiumoxide_arg_token(flag))
}

/// Fail-closed guard: every Chrome launch arg list must mute audio (ADR-0026).
///
/// Called from [`ChromeBrowser::launch`] after assembling safe defaults +
/// [`flags_stealth`]. Returns `InvalidConfig` rather than launching a loud
/// browser if a future refactor drops the mute flags.
///
/// Accepts either Chromium form (`--mute-audio`) or chromiumoxide key form
/// (`mute-audio`) so pre- and post-normalization lists both validate.
pub(crate) fn ensure_chrome_audio_muted(
    args: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<(), CliError> {
    let mut has_mute = false;
    let mut has_autoplay = false;
    for arg in args {
        let raw = arg.as_ref();
        let s = chromiumoxide_arg_token(raw);
        // Accept both full Chromium tokens and bare chromiumoxide keys.
        if s == "mute-audio" || raw == CHROME_MUTE_AUDIO_FLAG {
            has_mute = true;
        }
        if s == chromiumoxide_arg_token(CHROME_AUTOPLAY_POLICY_FLAG)
            || s.starts_with("autoplay-policy=")
            || raw == CHROME_AUTOPLAY_POLICY_FLAG
            || raw.starts_with("--autoplay-policy=")
        {
            has_autoplay = true;
        }
    }
    if has_mute && has_autoplay {
        return Ok(());
    }
    Err(CliError::InvalidConfig {
        message: format!(
            "internal: Chrome launch args missing mute-audio operational standard \
             (ADR-0026 / GAP-CHROME-MUTE-001): mute={has_mute} autoplay_policy={has_autoplay}"
        ),
    })
}

/// Fail-closed: after chromiumoxide-token normalization, rendered argv must be
/// exactly `--mute-audio` / `--autoplay-policy=...` (not `----mute-audio`).
///
/// GAP-CHROME-MUTE-002 — the ADR-0026 string check alone was insufficient because
/// it validated *source* flags while chromiumoxide rewrote them into ignored tokens.
pub(crate) fn ensure_chrome_audio_muted_rendered(
    args: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<(), CliError> {
    let rendered: Vec<String> = args
        .into_iter()
        .map(|a| chromiumoxide_rendered_arg(a.as_ref()))
        .collect();
    let has_mute = rendered.iter().any(|r| r == "--mute-audio");
    let has_autoplay = rendered.iter().any(|r| r.starts_with("--autoplay-policy="));
    // Hard reject the double-prefix regression class.
    let has_quad_mute = rendered
        .iter()
        .any(|r| r.starts_with("---") && r.contains("mute-audio"));
    if has_mute && has_autoplay && !has_quad_mute {
        return Ok(());
    }
    Err(CliError::InvalidConfig {
        message: format!(
            "internal: Chrome rendered argv fails mute operational standard \
             (ADR-0026 / GAP-CHROME-MUTE-002): mute={has_mute} autoplay={has_autoplay} \
             quad_dash_mute={has_quad_mute} sample={:?}",
            rendered
                .iter()
                .filter(|r| r.contains("mute") || r.contains("autoplay"))
                .collect::<Vec<_>>()
        ),
    })
}

/// Builds the cross-platform stealth flag list for headless/headed Chrome.
///
/// **Operational standard (ADR-0026 / GAP-CHROME-MUTE-001):** always includes
/// [`crate::browser::CHROME_MUTE_AUDIO_FLAG`] and
/// [`crate::browser::CHROME_AUTOPLAY_POLICY_FLAG`]. Every production path
/// (SERP web/news, deep-research, probe, content-fetch pool) goes through
/// [`crate::browser::ChromeBrowser::launch`] → this list — no alternate flag builder exists.
pub fn flags_stealth(
    needs_sandbox_off: bool,
    proxy: Option<&str>,
    user_agent: &str,
) -> Vec<String> {
    let mut flags: Vec<String> = vec![
        "--disable-blink-features=AutomationControlled".to_string(),
        "--disable-features=AutomationControlled,TranslateUI".to_string(),
        "--window-size=1920,1080".to_string(),
        "--window-position=-32000,-32000".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-infobars".to_string(),
        "--disable-sync".to_string(),
        "--metrics-recording-only".to_string(),
        "--no-first-run".to_string(),
        // ADR-0026 / GAP-CHROME-MUTE-001 — operational standard (always ON).
        // Headed Chrome under Xvfb still talks to host PulseAudio; without mute,
        // deep-research SERP/news/content pages with autoplay/ads play speakers.
        // Chromium: --mute-audio (peter.sh chromium-command-line-switches).
        CHROME_MUTE_AUDIO_FLAG.to_string(),
        CHROME_AUTOPLAY_POLICY_FLAG.to_string(),
        // GAP-WS-110 v0.9.2: disable WebRTC — it leaks the real public/local IP
        // via ICE candidate gathering even behind a proxy, breaking anonymity and
        // producing a network stack inconsistent with the spoofed UA/platform.
        "--disable-features=WebRtcHideLocalIpsWithMdns".to_string(),
        "--enforce-webrtc-ip-permission-check".to_string(),
        // `--force-webrtc-ip-handling-policy` is the flag accepted by modern Chrome
        // (the variant without `force-` was removed and ignored). Restricts ICE to not
        // use non-proxied UDP, suppressing real-IP leaks.
        "--force-webrtc-ip-handling-policy=disable_non_proxied_udp".to_string(),
        "--disable-webrtc-hw-decoding".to_string(),
        // GAP-WS-111 v0.9.2: disable QUIC — its UDP stack (GQuic/HTTP3)
        // differs from the TLS path of the spoofed Chrome version and is a
        // strong automation signal for Cloudflare's L7 heuristics.
        "--disable-quic".to_string(),
        format!("--user-agent={user_agent}"),
    ];

    #[cfg(target_os = "linux")]
    {
        flags.push("--disable-dev-shm-usage".to_string());
        if needs_sandbox_off {
            flags.push("--no-sandbox".to_string());
        }
    }
    #[cfg(target_os = "windows")]
    {
        let _ = needs_sandbox_off;
        // Windows-only, and it is a deliberate trade rather than an oversight.
        //
        // Forcing software rendering makes WebGL report SwiftShader, which is
        // one of the signals an anti-bot stack reads as "headless". On Linux the
        // answer is a private Xvfb with the host GPU behind it, so the flag is
        // absent there on purpose (ADR-0022: no synthetic fingerprint).
        //
        // Windows has no equivalent: a headless Chrome under a session with no
        // interactive desktop hits driver paths that crash or hang on several
        // GPU/driver combinations, and a crashed browser reports nothing at all.
        // A weaker fingerprint beats no session. Revisit if the Windows path
        // ever gains a real compositor guarantee.
        flags.push("--disable-gpu".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        // Quartz always provides a real compositor, so neither a sandbox
        // override nor software rendering is needed.
        let _ = needs_sandbox_off;
    }

    if let Some(url_proxy) = proxy {
        // G5 / GAP-PROXY-ARGV-SECRET: never put user:pass@ on Chrome argv (/proc leak).
        // Credentials (if any) stay in the full URL for library clients; Chromium
        // receives host:port only. Prefer XDG `proxy_url` without embedding secrets
        // in process listings.
        flags.push(format!(
            "--proxy-server={}",
            chrome_proxy_server_arg(url_proxy)
        ));
    }

    flags
}

/// Build Chromium `--proxy-server` value **without** userinfo (G5).
///
/// `http://user:pass@host:8080` → `http://host:8080`
#[must_use]
pub(crate) fn chrome_proxy_server_arg(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return trimmed.to_string();
    }
    if let Ok(mut u) = url::Url::parse(trimmed) {
        let _ = u.set_username("");
        let _ = u.set_password(None);
        // Url may emit trailing `/` for empty path — Chromium accepts both.
        let mut s = u.to_string();
        if s.ends_with('/') && u.path() == "/" && u.query().is_none() && u.fragment().is_none() {
            s.pop();
        }
        return s;
    }
    // Fallback: strip `scheme://user:pass@` when parse fails.
    if let Some(at) = trimmed.rfind('@') {
        if let Some(scheme_end) = trimmed.find("://") {
            let scheme = &trimmed[..=scheme_end + 2];
            return format!("{scheme}{}", &trimmed[at + 1..]);
        }
    }
    trimmed.to_string()
}

/// Applies `Emulation.setUserAgentOverride` so `navigator.userAgent` and Client
/// Hints (`sec-ch-ua`, `userAgentData.brands`) reflect the real Chrome major
/// version detected at launch (GAP-WS-109 v0.9.2).
///
/// `--user-agent=` only rewrites `navigator.userAgent`; this CDP command also
/// aligns the brand list and platform metadata, closing the gap that Cloudflare
/// detects via mismatched Client Hints. Call BEFORE `AddScriptToEvaluateOnNewDocument`
/// and before any navigation. `brands` follows the Chromium GREASE convention.
pub(crate) async fn apply_ua_override(page: &chromiumoxide::Page, ua: &str, major: u32) {
    use chromiumoxide::cdp::browser_protocol::emulation::{
        SetUserAgentOverrideParams, UserAgentBrandVersion, UserAgentMetadata,
    };
    let major_s = major.to_string();
    // Full version mirrors the rewritten UA (`Chrome/<major>.0.0.0`) so that
    // Sec-CH-UA-Full-Version-List is coherent with both `brands` (major only)
    // and navigator.userAgent. Absent full_version_list, real Chrome omits the
    // header only when the server never requested it via Accept-CH; a Cloudflare
    // bot check that requests Sec-CH-UA-Full-Version-List would flag the omission.
    let full_version = format!("{major}.0.0.0");
    let brands = vec![
        UserAgentBrandVersion::new("Not?A_Brand", "24"),
        UserAgentBrandVersion::new("Chromium", &major_s),
        UserAgentBrandVersion::new("Google Chrome", &major_s),
    ];
    let full_version_list = vec![
        UserAgentBrandVersion::new("Not?A_Brand", "99.0.0.0"),
        UserAgentBrandVersion::new("Chromium", &full_version),
        UserAgentBrandVersion::new("Google Chrome", &full_version),
    ];
    let (platform, platform_version, architecture) = if cfg!(target_os = "macos") {
        ("macOS", "10.15.7", "arm")
    } else if cfg!(target_os = "windows") {
        ("Windows", "10.0.0", "x86")
    } else {
        ("Linux", "6.5.0", "x86")
    };
    let metadata = UserAgentMetadata {
        brands: Some(brands),
        full_version_list: Some(full_version_list),
        platform: platform.to_string(),
        platform_version: platform_version.to_string(),
        architecture: architecture.to_string(),
        model: String::new(),
        mobile: false,
        bitness: None,
        wow64: None,
        form_factors: None,
    };
    let params = SetUserAgentOverrideParams {
        user_agent: ua.to_string(),
        accept_language: None,
        platform: None,
        user_agent_metadata: Some(metadata),
    };
    let _ = page.execute(params).await;
}
