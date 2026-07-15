// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (Chrome CDP connection, feature-gated)
//! Cross-platform detection and launch of headless Chrome via `chromiumoxide`.
//!
//! This module is only compiled with the `chrome` feature, enabled via
//! `cargo build --features chrome`. In default mode (without feature) the binary
//! has NO dependency on chromiumoxide/tempfile/futures — zero overhead.
//!
//! ## Responsibilities
//!
//! 1. [`detect_chrome`] — detects the Chrome/Chromium executable path on the
//!    system, with a 3-layer hierarchy (manual flag → env var → auto-detection).
//! 2. [`ChromeBrowser`] — safe wrapper over `chromiumoxide::Browser` that
//!    ensures process cleanup and handler-task via `impl Drop`.
//! 3. [`extract_text_with_chrome`] — navigation + extraction of `document.body.innerText`
//!    with configurable timeout.
//!
//! ## Process Cleanup and Safety (GAP-WS-LIFECYCLE-001 / one-shot)
//!
//! `chromiumoxide::Browser` starts a multi-process Chrome tree. `kill_on_drop` only
//! kills the **root** Tokio `Child`. This wrapper:
//! 1. Prefers async [`ChromeBrowser::shutdown`] (`close` → deadline → `kill` → tree/marker reap).
//! 2. On [`Drop`], runs synchronous `force_reap` (process tree + user-data-dir marker + Xvfb).
//! 3. Spawns private Xvfb in its own process group with `PR_SET_PDEATHSIG` (Linux).
//!
//! Contract: after the CLI exits, no Chromium/Xvfb/profile from **this** invocation remains.

#![cfg(feature = "chrome")]

use crate::error::CliError;
#[cfg(target_os = "linux")]
use crate::process_lifecycle::apply_process_group_and_pdeathsig;
use crate::process_lifecycle::{
    self, cleanup_xvfb_display_files, force_reap, install_panic_reap_hook, register_session,
    unregister_session, SessionIds,
};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task::JoinHandle;

/// Cooperative close/wait budget before forced kill (L-08).
const SHUTDOWN_COOPERATIVE_DEADLINE: Duration = Duration::from_secs(3);

/// Minimum character count per line kept by the cleaning pipeline.
const MIN_LINE_LENGTH: usize = 20;

/// chromiumoxide's `DEFAULT_ARGS` minus `enable-automation` (GAP-WS-108 v0.9.2).
///
/// `chromiumoxide` injects `--enable-automation` by default, which sets the
/// `navigator.webdriver` legacy flag and is detectable by Cloudflare via the
/// `chrome.runtime` object, CDP exposure, and the infobar token. We call
/// `.disable_default_args()` and re-add the 23 safe defaults below — every arg
/// from `chromiumoxide::browser::config::DEFAULT_ARGS` EXCEPT `enable-automation`.
const CHROMIUMOXIDE_SAFE_DEFAULTS: &[&str] = &[
    "--disable-background-networking",
    "--enable-features=NetworkService,NetworkServiceInProcess",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-breakpad",
    "--disable-client-side-phishing-detection",
    "--disable-component-extensions-with-background-pages",
    "--disable-default-apps",
    "--disable-dev-shm-usage",
    "--disable-features=TranslateUI",
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
];

/// Comprehensive stealth scripts injected via CDP `Page.addScriptToEvaluateOnNewDocument`.
///
/// Layer 3a — Basic JS environment (6 signals):
///   webdriver, plugins, languages, chrome object, maxTouchPoints, connection
///
/// Layer 3b — Hardware fingerprint spoofing (GAP-NEW-007):
///   Canvas fingerprint, WebGL renderer/vendor, `AudioContext` channel data,
///   `hardwareConcurrency`, `deviceMemory`, `screen.colorDepth`
const STEALTH_SCRIPTS: &str = concat!(
    // --- Layer 3a: basic JS environment ---
    "Object.defineProperty(navigator,'webdriver',{get:()=>undefined});",
    "Object.defineProperty(navigator,'languages',{get:()=>['en-US','en']});",
    "Object.defineProperty(navigator,'maxTouchPoints',{get:()=>0});",
    "Object.defineProperty(navigator,'connection',{get:()=>({rtt:50,downlink:10,effectiveType:'4g',saveData:false})});",
    "Object.defineProperty(navigator,'vendor',{get:()=>'Google Inc.'});",
    // --- Layer 3a+: chrome object emulation ---
    "window.chrome={runtime:{PlatformOs:{MAC:'mac',WIN:'win',ANDROID:'android',CROS:'cros',LINUX:'linux',OPENBSD:'openbsd'},PlatformArch:{ARM:'arm',X86_32:'x86-32',X86_64:'x86-64',MIPS:'mips',MIPS64:'mips64'},PlatformNaclArch:{ARM:'arm',X86_32:'x86-32',X86_64:'x86-64',MIPS:'mips',MIPS64:'mips64'},RequestUpdateCheckStatus:{THROTTLED:'throttled',NO_UPDATE:'no_update',UPDATE_AVAILABLE:'update_available'},OnInstalledReason:{INSTALL:'install',UPDATE:'update',CHROME_UPDATE:'chrome_update',SHARED_MODULE_UPDATE:'shared_module_update'},OnRestartRequiredReason:{APP_UPDATE:'app_update',OS_UPDATE:'os_update',PERIODIC:'periodic'},connect:function(){},sendMessage:function(){}},app:{isInstalled:false,InstallState:{INSTALLED:'installed',DISABLED:'disabled',NOT_INSTALLED:'not_installed'},RunningState:{RUNNING:'running',CANNOT_RUN:'cannot_run',READY_TO_RUN:'ready_to_run'}},loadTimes:function(){return{requestTime:Date.now()/1000,startLoadTime:Date.now()/1000,commitLoadTime:Date.now()/1000,finishDocumentLoadTime:Date.now()/1000,finishLoadTime:Date.now()/1000,firstPaintTime:Date.now()/1000,firstPaintAfterLoadTime:0,navigationType:'Other',wasFetchedViaSpdy:true,wasNpnNegotiated:true,npnNegotiatedProtocol:'h2',wasAlternateProtocolAvailable:false,connectionInfo:'h2'}},csi:function(){return{onloadT:Date.now(),startE:Date.now(),pageT:0,tran:15}}};",
    // --- Layer 3a+: realistic PluginArray ---
    "(function(){function P(n,d,f,m){this.name=n;this.description=d;this.filename=f;this.length=m.length;for(var i=0;i<m.length;i++)this[i]=m[i]}var p=[new P('Chrome PDF Plugin','Portable Document Format','internal-pdf-viewer',[{type:'application/x-google-chrome-pdf',suffixes:'pdf',description:'Portable Document Format'}]),new P('Chrome PDF Viewer','','mhjfbmdgcfjbbpaeojofohoefgiehjai',[{type:'application/pdf',suffixes:'pdf',description:''}]),new P('Native Client','','internal-nacl-plugin',[{type:'application/x-nacl',suffixes:'',description:'Native Client Executable'},{type:'application/x-pnacl',suffixes:'',description:'Portable Native Client Executable'}])];Object.defineProperty(navigator,'plugins',{get:function(){return p}});Object.defineProperty(navigator,'mimeTypes',{get:function(){return p.reduce(function(a,pl){for(var i=0;i<pl.length;i++)a.push(pl[i]);return a},[])}})})()",
    ";",
    // --- Layer 3b: window outer dimensions (0 in headless = detection) ---
    "Object.defineProperty(window,'outerHeight',{get:function(){return window.innerHeight+85}});",
    "Object.defineProperty(window,'outerWidth',{get:function(){return window.innerWidth+15}});",
    // --- Layer 3b: Permissions API (notifications=denied in headless) ---
    "(function(){if(typeof Permissions!=='undefined'){var o=Permissions.prototype.query;Permissions.prototype.query=function(p){if(p&&p.name==='notifications')return Promise.resolve({state:Notification.permission==='denied'?'denied':'prompt',onchange:null});return o.apply(this,arguments)}}})()",
    ";",
    // --- Layer 3b: iframe contentWindow protection ---
    "(function(){try{var F=HTMLIFrameElement.prototype;var d=Object.getOwnPropertyDescriptor(F,'contentWindow');if(d&&d.get){var o=d.get;Object.defineProperty(F,'contentWindow',{get:function(){var w=o.call(this);if(w&&w.chrome)w.chrome=window.chrome;return w}})}}catch(e){}})()",
    ";",
    // --- Layer 3b: hardware fingerprint spoofing (GAP-NEW-007) ---
    "Object.defineProperty(navigator,'hardwareConcurrency',{get:()=>8});",
    "Object.defineProperty(navigator,'deviceMemory',{get:()=>8});",
    "Object.defineProperty(screen,'colorDepth',{get:()=>24});",
    // Canvas fingerprint: inject subtle per-session noise into pixel data
    "(function(){var o=HTMLCanvasElement.prototype.toDataURL;HTMLCanvasElement.prototype.toDataURL=function(){try{var c=this.getContext('2d');if(c){var i=c.getImageData(0,0,Math.min(this.width,16),Math.min(this.height,16));for(var j=0;j<i.data.length;j+=100)i.data[j]=(i.data[j]+1)%256;c.putImageData(i,0,0)}}catch(e){}return o.apply(this,arguments)}})();",
    // WebGL renderer/vendor: report plausible GPU instead of SwiftShader
    "(function(){var o=WebGLRenderingContext.prototype.getParameter;WebGLRenderingContext.prototype.getParameter=function(p){if(p===37445)return'Google Inc. (NVIDIA)';if(p===37446)return'ANGLE (NVIDIA, NVIDIA GeForce GTX 1650 Direct3D11 vs_5_0 ps_5_0, D3D11)';return o.call(this,p)};if(typeof WebGL2RenderingContext!=='undefined'){var o2=WebGL2RenderingContext.prototype.getParameter;WebGL2RenderingContext.prototype.getParameter=function(p){if(p===37445)return'Google Inc. (NVIDIA)';if(p===37446)return'ANGLE (NVIDIA, NVIDIA GeForce GTX 1650 Direct3D11 vs_5_0 ps_5_0, D3D11)';return o2.call(this,p)}}})();",
    // AudioContext fingerprint: add micro-noise to channel data
    "(function(){if(typeof AudioBuffer!=='undefined'){var o=AudioBuffer.prototype.getChannelData;AudioBuffer.prototype.getChannelData=function(c){var a=o.call(this,c);for(var i=0;i<a.length;i+=100)a[i]+=0.0000001*(i%7-3);return a}}})();",
    // --- Layer 3c: CDP leak prevention (GAP-WS-076) ---
    "(function(){var O=window.WebSocket;window.WebSocket=function(u,p){if(u&&typeof u==='string'&&(u.includes('/devtools/')||u.includes('ws://127.0.0.1')))return{close:function(){},send:function(){},addEventListener:function(){},readyState:3};return p?new O(u,p):new O(u)};window.WebSocket.prototype=O.prototype;window.WebSocket.CONNECTING=0;window.WebSocket.OPEN=1;window.WebSocket.CLOSING=2;window.WebSocket.CLOSED=3})();",
    // Extended Permissions API (clipboard, geolocation, camera, microphone)
    "(function(){if(typeof Permissions!=='undefined'){var o=Permissions.prototype.query;Permissions.prototype.query=function(p){if(p&&p.name){var s={notifications:'prompt',geolocation:'prompt','clipboard-read':'prompt','clipboard-write':'granted',camera:'prompt',microphone:'prompt'};if(s[p.name]!==undefined)return Promise.resolve({state:s[p.name],onchange:null})}return o.apply(this,arguments)}}})()",
    ";",
);

/// Platform-specific `navigator.platform` stealth script.
/// Injected alongside `STEALTH_SCRIPTS` to match the compile target.
#[cfg(target_os = "linux")]
const STEALTH_PLATFORM_SCRIPT: &str =
    "Object.defineProperty(navigator,'platform',{get:()=>'Linux x86_64'});";
#[cfg(target_os = "macos")]
const STEALTH_PLATFORM_SCRIPT: &str =
    "Object.defineProperty(navigator,'platform',{get:()=>'MacIntel'});";
#[cfg(target_os = "windows")]
const STEALTH_PLATFORM_SCRIPT: &str =
    "Object.defineProperty(navigator,'platform',{get:()=>'Win32'});";
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
const STEALTH_PLATFORM_SCRIPT: &str =
    "Object.defineProperty(navigator,'platform',{get:()=>'Linux x86_64'});";

// Transport policy (NO_CHROME_ENV, HTTP_TEST_ENV, require_chrome_transport, …)
// lives in `chrome_policy` so it compiles with `--no-default-features`.
// Re-export for callers that historically imported from `browser`.
pub use crate::chrome_policy::{
    chrome_disabled_by_env, http_test_harness_active, require_chrome_transport, HTTP_TEST_ENV,
    NO_CHROME_ENV,
};

/// Installation channel for a resolved Chrome/Chromium binary (agent metadata).
///
/// Not telemetry — contract field for LLM operators (`chrome_canal` in JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeChannel {
    /// Explicit `--chrome-path` (after shell→ELF resolution when needed).
    Manual,
    /// `CHROME_PATH` environment variable.
    Env,
    /// Native package / Applications / Program Files host install.
    Host,
    /// Flatpak deploy ELF (`files/extra/chrome` or equivalent).
    Flatpak,
    /// Snap install.
    Snap,
}

impl ChromeChannel {
    /// Stable string for JSON / logs (`manual|env|host|flatpak|snap`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Env => "env",
            Self::Host => "host",
            Self::Flatpak => "flatpak",
            Self::Snap => "snap",
        }
    }
}

/// Resolved Chrome executable ready for `BrowserConfigBuilder::chrome_executable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedChrome {
    /// Absolute or concrete path to a real binary (ELF/Mach-O/PE), never a shell export.
    pub path: PathBuf,
    /// How the path was obtained / which install channel it belongs to.
    pub channel: ChromeChannel,
}

/// Returns an ordered list of candidate paths for Chrome/Chromium by platform.
///
/// Order (Linux): host Google Chrome → host Chromium ELF (lib64) → Flatpak
/// deploy ELF → Flatpak exports (resolved later) → Snap. Windows consults
/// environment variables (`%PROGRAMFILES%`, `%LOCALAPPDATA%`) when available.
///
/// GAP-WS-AGENT-READY-001 v0.9.8: include Flatpak **deploy ELF** under
/// `…/app/<id>/current/active/files/extra/chrome`, not only shell exports.
pub fn chrome_candidate_paths() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "linux")]
    {
        // Host Google Chrome first, then host Chromium ELF (avoid shell wrappers).
        for base in [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/opt/google/chrome/chrome",
            "/usr/lib64/chromium-browser/chromium-browser",
            "/usr/lib/chromium-browser/chromium-browser",
            "/usr/lib/chromium-browser/chrome",
            "/usr/bin/chromium",
            "/usr/local/bin/chromium",
            "/usr/local/bin/google-chrome",
            // Flatpak deploy ELF (chromiumoxide can launch these with --no-sandbox).
            "/var/lib/flatpak/app/com.google.Chrome/current/active/files/extra/chrome",
            "/var/lib/flatpak/app/org.chromium.Chromium/current/active/files/extra/chrome",
            // Exports (shell) — resolved to deploy ELF by resolve_chrome_candidate.
            "/var/lib/flatpak/exports/bin/com.google.Chrome",
            "/var/lib/flatpak/exports/bin/org.chromium.Chromium",
            "/snap/bin/chromium",
            "/snap/bin/google-chrome",
        ] {
            candidates.push(PathBuf::from(base));
        }
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join(
                ".local/share/flatpak/app/com.google.Chrome/current/active/files/extra/chrome",
            ));
            candidates.push(home.join(
                ".local/share/flatpak/app/org.chromium.Chromium/current/active/files/extra/chrome",
            ));
            candidates.push(home.join(".local/share/flatpak/exports/bin/com.google.Chrome"));
            candidates.push(home.join(".local/share/flatpak/exports/bin/org.chromium.Chromium"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        for base in [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/opt/homebrew/bin/chromium",
            "/opt/homebrew/bin/google-chrome",
            "/usr/local/bin/chromium",
            "/usr/local/bin/google-chrome",
        ] {
            candidates.push(PathBuf::from(base));
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Known base paths.
        for base in [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files\Chromium\Application\chrome.exe",
        ] {
            candidates.push(PathBuf::from(base));
        }
        // User-dependent paths via %LOCALAPPDATA%.
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            let base = PathBuf::from(&localappdata);
            candidates.push(base.join(r"Google\Chrome\Application\chrome.exe"));
            candidates.push(base.join(r"Chromium\Application\chrome.exe"));
        }
    }

    candidates
}

/// Infers [`ChromeChannel`] from a resolved binary path (not the original wrapper).
#[must_use]
pub fn classify_chrome_channel(path: &Path) -> ChromeChannel {
    let s = path.to_string_lossy();
    if s.contains("/flatpak/app/") || s.contains("\\flatpak\\app\\") {
        return ChromeChannel::Flatpak;
    }
    if s.starts_with("/snap/") || s.contains("/snap/bin/") {
        return ChromeChannel::Snap;
    }
    ChromeChannel::Host
}

/// Resolves a user-facing path (ELF, Flatpak export shell, or Fedora wrapper)
/// into a real browser binary path.
///
/// GAP-WS-AGENT-READY-001 / GAP-NEW-005: shell scripts are never passed to
/// chromiumoxide. Flatpak exports (`flatpak run com.google.Chrome`) map to
/// `files/extra/chrome`. Fedora `/usr/bin/chromium-browser` wrappers map to
/// lib64/lib ELF paths.
///
/// Never uses `flatpak-spawn --host` with untrusted interpolation.
#[must_use]
pub fn resolve_chrome_candidate(path: &Path) -> Option<PathBuf> {
    if is_executable_chrome_binary(path) {
        return Some(path.to_path_buf());
    }
    if !path.is_file() {
        return None;
    }
    // Try reading shell content for flatpak run / known wrappers.
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let content_lower = content.to_lowercase();

    #[cfg(target_os = "linux")]
    {
        if content_lower.contains("flatpak") && content_lower.contains("run") {
            if let Some(app_id) = extract_flatpak_app_id(&content) {
                if let Some(elf) = flatpak_deploy_chrome_elf(&app_id) {
                    if is_executable_chrome_binary(&elf) {
                        tracing::info!(
                            from = %path.display(),
                            to = %elf.display(),
                            app_id = %app_id,
                            "resolved Flatpak export shell to deploy ELF"
                        );
                        return Some(elf);
                    }
                }
            }
            // Path-based fallback from known export locations.
            let s = path.to_string_lossy();
            if s.contains("com.google.Chrome") {
                if let Some(elf) = flatpak_deploy_chrome_elf("com.google.Chrome") {
                    if is_executable_chrome_binary(&elf) {
                        return Some(elf);
                    }
                }
            }
            if s.contains("org.chromium.Chromium") {
                if let Some(elf) = flatpak_deploy_chrome_elf("org.chromium.Chromium") {
                    if is_executable_chrome_binary(&elf) {
                        return Some(elf);
                    }
                }
            }
        }

        // Fedora/RHEL chromium-browser.sh wrapper (and symlink to it).
        let s = path.to_string_lossy();
        if s.contains("chromium-browser") || content_lower.contains("chromium-browser") {
            for candidate in [
                "/usr/lib64/chromium-browser/chromium-browser",
                "/usr/lib/chromium-browser/chromium-browser",
                "/usr/lib/chromium-browser/chrome",
            ] {
                let p = PathBuf::from(candidate);
                if is_executable_chrome_binary(&p) {
                    tracing::info!(
                        from = %path.display(),
                        to = %p.display(),
                        "resolved Chromium shell wrapper to host ELF"
                    );
                    return Some(p);
                }
            }
        }
    }

    let _ = content;
    None
}

/// Parses `com.google.Chrome` / `org.chromium.Chromium` from a Flatpak export script.
#[cfg(target_os = "linux")]
fn extract_flatpak_app_id(script: &str) -> Option<String> {
    for token in script.split_whitespace() {
        if token == "com.google.Chrome" || token.starts_with("com.google.Chrome") {
            return Some("com.google.Chrome".to_string());
        }
        if token == "org.chromium.Chromium" || token.starts_with("org.chromium.Chromium") {
            return Some("org.chromium.Chromium".to_string());
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn extract_flatpak_app_id(_script: &str) -> Option<String> {
    None
}

/// Locates Flatpak deploy ELF for a given app-id (system then user install).
#[cfg(target_os = "linux")]
fn flatpak_deploy_chrome_elf(app_id: &str) -> Option<PathBuf> {
    let mut paths = Vec::new();
    paths.push(PathBuf::from(format!(
        "/var/lib/flatpak/app/{app_id}/current/active/files/extra/chrome"
    )));
    // Some Chromium Flatpaks ship as `chrome` or `chromium`.
    paths.push(PathBuf::from(format!(
        "/var/lib/flatpak/app/{app_id}/current/active/files/bin/chromium"
    )));
    paths.push(PathBuf::from(format!(
        "/var/lib/flatpak/app/{app_id}/current/active/files/bin/chrome"
    )));
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(format!(
            ".local/share/flatpak/app/{app_id}/current/active/files/extra/chrome"
        )));
        paths.push(home.join(format!(
            ".local/share/flatpak/app/{app_id}/current/active/files/bin/chromium"
        )));
        paths.push(home.join(format!(
            ".local/share/flatpak/app/{app_id}/current/active/files/bin/chrome"
        )));
    }
    paths.into_iter().find(|p| is_executable_chrome_binary(p))
}

#[cfg(not(target_os = "linux"))]
fn flatpak_deploy_chrome_elf(_app_id: &str) -> Option<PathBuf> {
    None
}

/// Detects Chrome/Chromium with multi-channel resolution (GAP-WS-AGENT-READY-001).
///
/// Resolution order:
/// 1. `manual_path` (`--chrome-path`) — resolve shell wrappers; hard-fail if invalid.
/// 2. `CHROME_PATH` — resolve; warn and continue if invalid.
/// 3. `which` on common binary names — resolve wrappers.
/// 4. [`chrome_candidate_paths`] — first resolvable real binary wins.
///
/// # Errors
///
/// Returns an error if `manual_path` cannot be resolved to a real binary, or if
/// no Chrome/Chromium executable is found on the system.
pub fn detect_chrome(manual_path: Option<&Path>) -> Result<PathBuf, CliError> {
    Ok(detect_chrome_resolved(manual_path)?.path)
}

/// Like [`detect_chrome`] but returns channel metadata for agent JSON fields.
///
/// # Errors
///
/// Same as [`detect_chrome`].
pub fn detect_chrome_resolved(manual_path: Option<&Path>) -> Result<ResolvedChrome, CliError> {
    // Layer 1: manual --chrome-path (highest priority; fail closed after resolve).
    if let Some(p) = manual_path {
        if let Some(resolved) = resolve_chrome_candidate(p) {
            let channel = ChromeChannel::Manual;
            tracing::info!(
                path = %resolved.display(),
                requested = %p.display(),
                canal = channel.as_str(),
                "Chrome found via --chrome-path"
            );
            return Ok(ResolvedChrome {
                path: resolved,
                channel,
            });
        }
        return Err(CliError::PathError {
            message: format!(
                "--chrome-path {:?} is not a valid Chrome/Chromium binary (missing, shell wrapper without resolvable ELF, or not a file). \
                 On Fedora try /usr/lib64/chromium-browser/chromium-browser. \
                 For Flatpak Chrome try the deploy ELF under \
                 /var/lib/flatpak/app/com.google.Chrome/current/active/files/extra/chrome \
                 (export scripts under flatpak/exports/bin are resolved automatically when the deploy exists).",
                p.display()
            ),
        });
    }

    // Layer 2: CHROME_PATH env var override.
    if let Ok(env_path) = std::env::var("CHROME_PATH") {
        let p = PathBuf::from(&env_path);
        if let Some(resolved) = resolve_chrome_candidate(&p) {
            tracing::info!(
                path = %resolved.display(),
                canal = "env",
                "Chrome found via CHROME_PATH"
            );
            return Ok(ResolvedChrome {
                path: resolved,
                channel: ChromeChannel::Env,
            });
        }
        tracing::warn!(
            path = env_path,
            "CHROME_PATH set but not a resolvable Chrome/Chromium binary — trying auto-detection"
        );
    }

    // Layer 3: PATH lookup via `which` crate.
    for binary_name in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
    ] {
        if let Ok(p) = which::which(binary_name) {
            if let Some(resolved) = resolve_chrome_candidate(&p) {
                let channel = classify_chrome_channel(&resolved);
                tracing::info!(
                    binary = binary_name,
                    path = %resolved.display(),
                    canal = channel.as_str(),
                    "Chrome found via PATH lookup (which crate)"
                );
                return Ok(ResolvedChrome {
                    path: resolved,
                    channel,
                });
            }
            tracing::debug!(
                binary = binary_name,
                path = %p.display(),
                "which crate found candidate but could not resolve to a real binary"
            );
        }
    }

    // Layer 4: platform-specific well-known installation paths.
    for candidate in chrome_candidate_paths() {
        if let Some(resolved) = resolve_chrome_candidate(&candidate) {
            let channel = classify_chrome_channel(&resolved);
            tracing::info!(
                path = %resolved.display(),
                canal = channel.as_str(),
                "Chrome found at platform-specific path"
            );
            return Ok(ResolvedChrome {
                path: resolved,
                channel,
            });
        }
    }

    Err(CliError::PathError {
        message: "Chrome/Chromium not found. Install via package manager, Flatpak (com.google.Chrome), or provide --chrome-path / CHROME_PATH to a real binary (not a shell-only wrapper).".into(),
    })
}

/// v0.8.0 GAP-NEW-005: rejects shell-script wrappers (e.g. `chromium-browser.sh`)
/// which call the Rust `timeout` crate binary and kill Chrome in ~0.1s. Validates
/// that the candidate is a real ELF/Mach-O executable, not a text file.
///
/// Prefer [`resolve_chrome_candidate`] when accepting user paths — it maps known
/// shells to deploy/host ELFs before this check is applied to the result.
fn is_executable_chrome_binary(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let path_str = path.to_string_lossy();
    if path_str.ends_with(".sh") {
        tracing::debug!(path = %path.display(), "rejecting shell-script wrapper");
        return false;
    }
    // Verify ELF magic bytes (Linux) or Mach-O (macOS) to ensure executable format.
    // Read only the first bytes — Flatpak Chrome ELF is ~280 MiB.
    match std::fs::File::open(path).and_then(|mut f| {
        use std::io::Read;
        let mut magic = [0u8; 4];
        f.read_exact(&mut magic)?;
        Ok(magic)
    }) {
        Ok(bytes) => {
            let is_elf = &bytes[0..4] == b"\x7fELF";
            let is_macho = bytes[0..4] == [0xCF, 0xFA, 0xED, 0xFE]
                || bytes[0..4] == [0xFE, 0xED, 0xFA, 0xCE]
                || bytes[0..4] == [0xFE, 0xED, 0xFA, 0xCF]
                || bytes[0..4] == [0xCA, 0xFE, 0xBA, 0xBE];
            let is_pe = &bytes[0..2] == b"MZ";
            is_elf || is_macho || is_pe
        }
        _ => false,
    }
}

/// Returns `true` when the operator explicitly requested headed Chrome
/// via `DUCKDUCKGO_CHROME_XVFB=1` (for xvfb-run anti-bot evasion).
/// Without this env var, Chrome runs headless by default.
fn is_xvfb_requested() -> bool {
    std::env::var("DUCKDUCKGO_CHROME_XVFB").is_ok()
}

/// Detects whether the current platform has a native display server available.
/// Linux: checks `$DISPLAY` (X11) or `$WAYLAND_DISPLAY` (Wayland).
/// macOS/Windows: always returns true (Quartz/DWM always active on desktop).
fn has_native_display() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(d) = std::env::var("DISPLAY") {
            if !d.is_empty() {
                return true;
            }
        }
        if let Ok(d) = std::env::var("WAYLAND_DISPLAY") {
            if !d.is_empty() {
                return true;
            }
        }
        false
    }
    #[cfg(target_os = "macos")]
    {
        true
    }
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        return false;
    }
}

/// Returns `true` when the Xvfb lock file at `path` references a PID that
/// is no longer running (stale lock from a crashed/killed Xvfb). GAP-WS-089.
#[cfg(target_os = "linux")]
fn is_lock_stale(path: &str) -> bool {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let pid_str = contents.trim();
    let pid: i32 = match pid_str.parse() {
        Ok(p) if p > 0 => p,
        _ => return true,
    };
    // Safe check: /proc/{pid} existence is a non-signal probe.
    !std::path::Path::new(&format!("/proc/{pid}")).exists()
}

/// RAII guard for a private Xvfb process (L-02, L-06, L-09).
///
/// Always kills the process group / PID and removes lock/socket files on drop,
/// including when Chrome launch fails after Xvfb was already started.
struct XvfbGuard {
    child: std::process::Child,
    display: String,
    /// Process group id (== child pid when setpgid(0,0) succeeded).
    pgid: Option<i32>,
    reaped: bool,
}

impl XvfbGuard {
    fn session_bits(&self) -> (Option<u32>, Option<i32>, String) {
        (Some(self.child.id()), self.pgid, self.display.clone())
    }

    fn reap(&mut self) {
        if self.reaped {
            return;
        }
        self.reaped = true;
        if let Some(pgid) = self.pgid {
            process_lifecycle::kill_process_group(pgid);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        cleanup_xvfb_display_files(&self.display);
        tracing::info!(display = %self.display, "Xvfb virtual display stopped");
    }
}

impl Drop for XvfbGuard {
    fn drop(&mut self) {
        self.reap();
    }
}

/// Spawns a private Xvfb server on a free display number so Chrome can run
/// in headed mode (passing Cloudflare anti-bot) without showing a visible
/// window to the user.
///
/// Returns [`XvfbGuard`] on success, or `None` if Xvfb is not available or no
/// free display slot was found. The guard always reaps Xvfb on drop (L-06).
#[cfg(target_os = "linux")]
fn spawn_virtual_display() -> Option<XvfbGuard> {
    let xvfb_path = which::which("Xvfb").ok()?;

    for display_num in 99..200 {
        let lock_path = format!("/tmp/.X{display_num}-lock");
        if std::path::Path::new(&lock_path).exists() {
            // GAP-WS-089: check if the PID in the lock file is still alive;
            // remove stale locks left by crashed/killed Xvfb processes.
            if is_lock_stale(&lock_path) {
                let _ = std::fs::remove_file(&lock_path);
                // Also clean the companion Unix socket if present.
                let socket_path = format!("/tmp/.X11-unix/X{display_num}");
                let _ = std::fs::remove_file(&socket_path);
                tracing::info!(display_num, "removed stale Xvfb lock file");
            } else {
                continue;
            }
        }
        let disp = format!(":{display_num}");
        let mut cmd = std::process::Command::new(&xvfb_path);
        cmd.arg(&disp)
            .args(["-screen", "0", "1920x1080x24", "-nolisten", "tcp", "-ac"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        // L-02: own process group + PDEATHSIG so CLI death reaps Xvfb.
        apply_process_group_and_pdeathsig(&mut cmd);
        let child = cmd.spawn().ok()?;
        let pgid = Some(child.id() as i32);

        std::thread::sleep(std::time::Duration::from_millis(150));

        if std::path::Path::new(&lock_path).exists() {
            tracing::info!(
                xvfb_display = %disp,
                xvfb_pid = child.id(),
                "Xvfb virtual display started (process group + PDEATHSIG)"
            );
            return Some(XvfbGuard {
                child,
                display: disp,
                pgid,
                reaped: false,
            });
        }
        // Xvfb failed to create lock — reap this attempt and try next display.
        let mut failed = XvfbGuard {
            child,
            display: disp,
            pgid,
            reaped: false,
        };
        failed.reap();
    }
    None
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)] // only invoked under the cfg(target_os = "linux") launch path
fn spawn_virtual_display() -> Option<XvfbGuard> {
    None
}

/// Attempts to auto-install Xvfb on Linux when not found.
/// Shows visible messages to the user via eprintln (not just tracing).
/// Uses `sudo -n` (non-interactive) to avoid blocking on password prompts.
#[cfg(target_os = "linux")]
fn try_auto_install_xvfb() {
    if which::which("Xvfb").is_ok() {
        return;
    }
    let distro = detect_linux_distro();
    let variant = detect_linux_variant();
    if variant == "immutable" {
        eprintln!(
            "\x1b[33m[duckduckgo-search-cli]\x1b[0m Distro imutável detectada ({distro}) — \
             auto-install de Xvfb não é possível."
        );
        eprintln!("{}", xvfb_manual_instruction(&distro));
        return;
    }
    let (pkg_manager, args): (&str, Vec<&str>) = match distro.as_str() {
        "fedora" | "rhel" | "centos" | "rocky" | "almalinux" => {
            ("dnf", vec!["install", "-y", "xorg-x11-server-Xvfb"])
        }
        "ubuntu" | "debian" | "linuxmint" | "pop" | "zorin" | "elementary" | "kali" => {
            ("apt-get", vec!["install", "-y", "xvfb"])
        }
        "arch" | "manjaro" | "endeavouros" | "garuda" => {
            ("pacman", vec!["-S", "--noconfirm", "xorg-server-xvfb"])
        }
        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" | "sles" => {
            ("zypper", vec!["install", "-y", "xorg-x11-server-Xvfb"])
        }
        "alpine" => ("apk", vec!["add", "xvfb"]),
        "amzn" => ("yum", vec!["install", "-y", "Xvfb"]),
        "void" => ("xbps-install", vec!["-y", "xorg-server-xvfb"]),
        "gentoo" => ("emerge", vec!["--ask=n", "x11-base/xorg-server"]),
        _ => {
            eprintln!(
                "\x1b[33m[duckduckgo-search-cli]\x1b[0m Distro não reconhecida ({distro}) — \
                 auto-install de Xvfb não disponível."
            );
            eprintln!("{}", xvfb_manual_instruction(&distro));
            return;
        }
    };
    let install_cmd = format!("sudo {} {}", pkg_manager, args.join(" "));
    eprintln!(
        "\x1b[33m[duckduckgo-search-cli]\x1b[0m Xvfb não encontrado — \
         tentando instalar automaticamente via sudo (sem senha)..."
    );
    eprintln!("\x1b[36m  $ {install_cmd}\x1b[0m");
    let status = std::process::Command::new("sudo")
        .arg("-n")
        .arg(pkg_manager)
        .args(&args)
        .stderr(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .status();
    match status {
        Ok(s) if s.success() => {
            eprintln!("\x1b[32m[duckduckgo-search-cli]\x1b[0m Xvfb instalado com sucesso!");
        }
        Ok(_) => {
            eprintln!(
                "\x1b[31m[duckduckgo-search-cli]\x1b[0m Auto-install falhou \
                 (sudo sem senha não disponível)."
            );
            eprintln!("\x1b[33m  Instale manualmente:\x1b[0m\n\x1b[36m  $ {install_cmd}\x1b[0m");
        }
        Err(e) => {
            eprintln!(
                "\x1b[31m[duckduckgo-search-cli]\x1b[0m Erro ao executar gerenciador de pacotes: {e}"
            );
            eprintln!("\x1b[33m  Instale manualmente:\x1b[0m\n\x1b[36m  $ {install_cmd}\x1b[0m");
        }
    }
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)] // only invoked under the cfg(target_os = "linux") launch path
fn try_auto_install_xvfb() {}

/// Returns a distro-aware instruction string for manual Xvfb installation.
#[cfg(target_os = "linux")]
fn xvfb_manual_instruction(distro: &str) -> String {
    let specific = match distro {
        "fedora" | "rhel" | "centos" | "rocky" | "almalinux" => {
            Some("sudo dnf install -y xorg-x11-server-Xvfb")
        }
        "ubuntu" | "debian" | "linuxmint" | "pop" | "zorin" | "elementary" | "kali" => {
            Some("sudo apt-get install -y xvfb")
        }
        "arch" | "manjaro" | "endeavouros" | "garuda" => {
            Some("sudo pacman -S --noconfirm xorg-server-xvfb")
        }
        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" | "sles" => {
            Some("sudo zypper install -y xorg-x11-server-Xvfb")
        }
        "alpine" => Some("sudo apk add xvfb"),
        "void" => Some("sudo xbps-install -y xorg-server-xvfb"),
        "gentoo" => Some("sudo emerge x11-base/xorg-server"),
        "amzn" => Some("sudo yum install -y Xvfb"),
        "nixos" => Some("nix-env -iA nixpkgs.xorg.xorgserver"),
        "guix" => Some("guix install xorg-server"),
        _ => None,
    };
    let mut msg = String::from("\x1b[33m  Instale Xvfb manualmente:\x1b[0m\n");
    if let Some(cmd) = specific {
        msg.push_str(&format!("\x1b[36m  $ {cmd}\x1b[0m\n"));
    } else {
        msg.push_str(
            "\x1b[36m  Fedora/RHEL:       sudo dnf install -y xorg-x11-server-Xvfb\n\
             \x1b[36m  Ubuntu/Debian:     sudo apt-get install -y xvfb\n\
             \x1b[36m  Arch/Manjaro:      sudo pacman -S --noconfirm xorg-server-xvfb\n\
             \x1b[36m  openSUSE:          sudo zypper install -y xorg-x11-server-Xvfb\n\
             \x1b[36m  Alpine:            sudo apk add xvfb\n\
             \x1b[36m  NixOS:             nix-env -iA nixpkgs.xorg.xorgserver\n\
             \x1b[36m  Silverblue:        rpm-ostree install xorg-x11-server-Xvfb && systemctl reboot\x1b[0m\n",
        );
    }
    // Fedora Silverblue/Kinoite/ostree-based
    if distro == "fedora" {
        let variant = detect_linux_variant();
        if variant == "immutable" {
            msg = String::from("\x1b[33m  Instale Xvfb manualmente:\x1b[0m\n");
            msg.push_str(
                "\x1b[36m  $ rpm-ostree install xorg-x11-server-Xvfb && systemctl reboot\x1b[0m\n",
            );
        }
    }
    msg
}

/// Detects immutable/NixOS/Silverblue distros where package install is non-standard.
#[cfg(target_os = "linux")]
fn detect_linux_variant() -> &'static str {
    let content = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let lower = content.to_lowercase();
    if lower.contains("variant_id=silverblue")
        || lower.contains("variant_id=kinoite")
        || lower.contains("variant_id=sericea")
        || lower.contains("variant_id=onyx")
    {
        return "immutable";
    }
    if lower.contains("\nid=nixos")
        || lower.contains("\nid=\"nixos\"")
        || lower.contains("\nid=guix")
        || lower.contains("\nid=\"guix\"")
    {
        return "immutable";
    }
    if std::path::Path::new("/run/ostree-booted").exists() {
        return "immutable";
    }
    "mutable"
}

/// Detects the Linux distribution by reading /etc/os-release.
/// Returns the ID field (e.g. "fedora", "ubuntu", "arch").
#[cfg(target_os = "linux")]
fn detect_linux_distro() -> String {
    let content = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    for line in content.lines() {
        if let Some(id) = line.strip_prefix("ID=") {
            return id.trim_matches('"').to_lowercase();
        }
    }
    "unknown".to_string()
}

/// Indicates whether we are running inside a container or Flatpak/Snap wrapper, which
/// requires `--no-sandbox` for Chrome to work.
///
/// GAP-WS-AGENT-READY-001 v0.9.8: also true for Flatpak **deploy** ELFs under
/// `/flatpak/app/` and `files/extra/chrome` (not only export scripts).
pub fn needs_no_sandbox(chrome_path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        let s = chrome_path.to_string_lossy();
        if s.contains("flatpak/exports/bin")
            || s.contains("/flatpak/app/")
            || s.contains("files/extra/chrome")
            || s.starts_with("/snap/")
        {
            return true;
        }
        // Rodando como root (comum em Docker).
        #[cfg(unix)]
        {
            if std::env::var("DOCKER_CONTAINER").is_ok()
                || std::path::Path::new("/.dockerenv").exists()
            {
                return true;
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = chrome_path;
    }
    false
}

/// Detecta a major version do Chrome/Chromium instalado via `<path> --version` (GAP-WS-109 v0.9.2).
///
/// Faz parse do primeiro grupo de dígitos após `Chrome ` ou `Chromium ` na saída
/// de `--version` (ex.: `"Google Chrome 149.0.7827.201"` -> `149`). Retorna
/// `None` se o binário falhar ou a versão não for parseável. A invocação é
/// bloqueante mas rápida (~50 ms) — re-detectar em cada site é aceitável.
pub fn detect_chrome_major_version(path: &Path) -> Option<u32> {
    let output = std::process::Command::new(path)
        .arg("--version")
        .output()
        .ok()?;
    let line = String::from_utf8_lossy(&output.stdout);
    let after = line
        .split("Chrome ")
        .nth(1)
        .or_else(|| line.split("Chromium ").nth(1))?;
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok()
}

/// Monta a lista de flags stealth cross-platform para o Chrome headless.
pub fn flags_stealth(
    precisa_sandbox_off: bool,
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
        // GAP-WS-110 v0.9.2: disable WebRTC — it leaks the real public/local IP
        // via ICE candidate gathering even behind a proxy, breaking anonymity and
        // producing a fingerprint inconsistent with the spoofed UA/platform.
        "--disable-features=WebRtcHideLocalIpsWithMdns".to_string(),
        "--enforce-webrtc-ip-permission-check".to_string(),
        // `--force-webrtc-ip-handling-policy` é a flag aceita pelo Chrome moderno
        // (a variante sem `force-` foi removida e ignorada). Restringe ICE a não
        // usar UDP não-proxied, suprimindo vazamento de IP real.
        "--force-webrtc-ip-handling-policy=disable_non_proxied_udp".to_string(),
        "--disable-webrtc-hw-decoding".to_string(),
        // GAP-WS-111 v0.9.2: disable QUIC — its UDP fingerprint (GQuic/HTTP3)
        // differs from the TLS JA3/JA4 of the spoofed Chrome version and is a
        // strong automation signal for Cloudflare's L7 heuristics.
        "--disable-quic".to_string(),
        format!("--user-agent={user_agent}"),
    ];

    #[cfg(target_os = "linux")]
    {
        flags.push("--disable-dev-shm-usage".to_string());
        if precisa_sandbox_off {
            flags.push("--no-sandbox".to_string());
        }
    }
    #[cfg(target_os = "windows")]
    {
        let _ = precisa_sandbox_off;
        flags.push("--disable-gpu".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = precisa_sandbox_off;
    }

    if let Some(url_proxy) = proxy {
        flags.push(format!("--proxy-server={url_proxy}"));
    }

    flags
}

/// Applies `Emulation.setUserAgentOverride` so `navigator.userAgent` and Client
/// Hints (`sec-ch-ua`, `userAgentData.brands`) reflect the real Chrome major
/// version detected at launch (GAP-WS-109 v0.9.2).
///
/// `--user-agent=` only rewrites `navigator.userAgent`; this CDP command also
/// aligns the brand list and platform metadata, closing the gap that Cloudflare
/// detects via mismatched Client Hints. Call BEFORE `AddScriptToEvaluateOnNewDocument`
/// and before any navigation. `brands` follows the Chromium GREASE convention.
async fn apply_ua_override(page: &chromiumoxide::Page, ua: &str, major: u32) {
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

/// RAII wrapper over `chromiumoxide::Browser`. Keeps the browser and handler-task alive.
///
/// **Cleanup:** prefer [`ChromeBrowser::shutdown`] (async finalize with deadline).
/// [`Drop`] runs synchronous force-reap of the Chrome process tree, Xvfb, and
/// profile dir (GAP-WS-LIFECYCLE-001 one-shot contract).
pub struct ChromeBrowser {
    /// The underlying chromiumoxide browser handle.
    browser: Browser,
    /// Join handle for the event-loop task; aborted on `Drop` if still alive.
    handler: Option<JoinHandle<()>>,
    /// Profile directory — removed when this value is dropped after processes die.
    user_data: Option<tempfile::TempDir>,
    /// Absolute path copy for marker-based kill after [`tempfile::TempDir`] is gone.
    user_data_path: PathBuf,
    /// Root Chrome PID captured at launch (for process-tree kill).
    chrome_pid: Option<u32>,
    /// Private Xvfb process for headed-but-invisible Chrome (Linux).
    xvfb: Option<XvfbGuard>,
    /// Idempotency flag: true after finalize/`force_reap` completed (L-08).
    finalized: bool,
    /// Effective UA after GAP-WS-109 alignment: the pool UA with its major
    /// Chrome version rewritten to match the real installed Chrome. Applied to
    /// every page via `Emulation.setUserAgentOverride` so Client Hints and
    /// `navigator.userAgent` are coherent.
    effective_ua: String,
    /// Major Chrome version detected from `chrome --version` (fallback 146).
    chrome_major: u32,
}

impl std::fmt::Debug for ChromeBrowser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChromeBrowser")
            .field("handler_alive", &self.handler.is_some())
            .field("user_data_dir", &self.user_data_path)
            .field("chrome_pid", &self.chrome_pid)
            .field("finalized", &self.finalized)
            .finish_non_exhaustive()
    }
}

/// Decisão de modo de cabeça do Chrome extraída de `launch()` (GAP-WS-107 v0.9.1).
/// Pura + testável cross-platform via `cfg!`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChromeHeadMode {
    Headless,
    HeadedXvfb,
    HeadedNative,
}

/// Decide o modo de cabeça do Chrome de forma pura e cfg-gated (GAP-WS-107 v0.9.1).
///
/// macOS/Windows com display nativo agora retornam `HeadedNative` (antes caíam
/// em headless porque `spawn_virtual_display()` retorna `None` fora do Linux).
#[allow(clippy::too_many_arguments)]
fn decide_head_mode(
    force_headless: bool,
    force_visible: bool,
    xvfb_requested: bool,
    has_native_display: bool,
    xvfb_available: bool,
) -> ChromeHeadMode {
    if force_headless {
        return ChromeHeadMode::Headless;
    }
    if force_visible || xvfb_requested {
        return if cfg!(target_os = "linux") && xvfb_available {
            ChromeHeadMode::HeadedXvfb
        } else if cfg!(target_os = "linux") {
            ChromeHeadMode::Headless
        } else {
            ChromeHeadMode::HeadedNative
        };
    }
    if has_native_display {
        if cfg!(target_os = "linux") {
            return if xvfb_available {
                ChromeHeadMode::HeadedXvfb
            } else {
                ChromeHeadMode::Headless
            };
        }
        // GAP-WS-112 v0.9.3: deteccao automatica de SO — macOS (Quartz) e Windows
        // (DWM) clampam `--window-position` aos bounds da tela, entao headed nativo
        // abriria uma janela visivel a cada busca. `--headless=new` moderno combinado
        // com as fixes v0.9.2 (enable-automation removido, Client Hints coerentes
        // via Emulation.setUserAgentOverride, WebRTC/QUIC off) passa no DDG sem
        // abrir janela. Modo OBRIGATORIAMENTE distinto do Linux, que continua usando
        // Xvfb privado (HeadedXvfb). DUCKDUCKGO_CHROME_VISIBLE=1 ainda forca
        // HeadedNative para depuracao.
        return ChromeHeadMode::Headless;
    }
    if xvfb_available {
        ChromeHeadMode::HeadedXvfb
    } else {
        ChromeHeadMode::Headless
    }
}

impl ChromeBrowser {
    /// Launches headless Chrome with the stealth configuration.
    ///
    /// - `path`: Chrome executable (use [`detect_chrome`] to obtain it).
    /// - `proxy`: optional proxy URL (propagated to the browser process).
    /// - `timeout_launch`: time limit for process initialization.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary user-data directory cannot be created,
    /// if `BrowserConfig` construction fails, or if the Chrome process fails to
    /// launch within `timeout_launch`.
    ///
    /// # Cancel safety
    ///
    /// This function is cancel-safe. If the future is dropped before Chrome
    /// finishes launching, the spawned handler task is aborted and the temporary
    /// directory is cleaned up via [`Drop`].
    pub async fn launch(
        path: &Path,
        proxy: Option<&str>,
        timeout_launch: Duration,
        user_agent: &str,
    ) -> Result<Self, CliError> {
        tracing::info!(
            path = %path.display(),
            proxy = proxy.unwrap_or(""),
            user_agent = user_agent,
            "Launching headless Chrome"
        );

        let sandbox_off = needs_no_sandbox(path);

        // GAP-WS-109 v0.9.2: align the pool UA's Chrome major version with the
        // real installed Chrome so `navigator.userAgent` and Client Hints
        // (sec-ch-ua, userAgentData.brands) are coherent. `--user-agent=` only
        // rewrites navigator.userAgent; `Emulation.setUserAgentOverride` (applied
        // per-page below) covers both. We compute the effective UA once here.
        let chrome_major = detect_chrome_major_version(path).unwrap_or(146);
        let effective_ua = crate::identity::rewrite_ua_chrome_version(user_agent, chrome_major);

        install_panic_reap_hook();

        let flags = flags_stealth(sandbox_off, proxy, &effective_ua);
        // GAP-WS-TMP-PROFILE-ORPHAN-001: auditable prefix (not tempfile default `.tmp`)
        // so operators can `find … -name 'ddg-chrome-*'` and force_reap can remove disk.
        let mut tmp_builder = tempfile::Builder::new();
        tmp_builder.prefix(crate::process_lifecycle::USER_DATA_DIR_PREFIX);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tmp_builder.permissions(std::fs::Permissions::from_mode(0o700));
        }
        let user_data = tmp_builder.tempdir().map_err(|e| CliError::PathError {
            message: format!("failed to create user-data-dir TempDir: {e}"),
        })?;
        let user_data_path = user_data.path().to_path_buf();

        let force_visible = std::env::var("DUCKDUCKGO_CHROME_VISIBLE").is_ok();
        let xvfb_requested = is_xvfb_requested();
        let force_headless = std::env::var("DUCKDUCKGO_CHROME_HEADLESS").is_ok();

        // Headed Chrome passes Cloudflare anti-bot; headless is detectable.
        // Priority: HEADLESS env (force) > VISIBLE env > native display > Xvfb auto-spawn > headless fallback.
        // GAP-WS-107 v0.9.1: decisão extraída para `decide_head_mode` (pura, cfg-gated);
        // macOS/Windows com display nativo agora usam headed nativo em vez de cair em headless.
        // XvfbGuard: if Browser::launch fails later, Drop kills Xvfb (L-06).
        #[allow(unused_mut)] // reassigned only under cfg(target_os = "linux")
        let mut xvfb_guard: Option<XvfbGuard> = None;
        #[allow(unused_mut)] // reassigned only under cfg(target_os = "linux")
        let mut xvfb_available = false;
        #[allow(unused_mut)] // reassigned only under cfg(target_os = "linux")
        let mut virtual_display: Option<String> = None;

        #[cfg(target_os = "linux")]
        {
            // On Linux a private Xvfb is preferred over the native display so
            // Chrome never shows a visible window (GNOME/Mutter clamps
            // --window-position to screen bounds).
            if !force_headless {
                if let Some(guard) = spawn_virtual_display() {
                    tracing::info!(
                        xvfb = %guard.display,
                        "Using private Xvfb for invisible headed mode"
                    );
                    virtual_display = Some(guard.display.clone());
                    xvfb_available = true;
                    xvfb_guard = Some(guard);
                } else {
                    try_auto_install_xvfb();
                    if let Some(guard) = spawn_virtual_display() {
                        tracing::info!(
                            xvfb = %guard.display,
                            "Xvfb auto-installed — using private display"
                        );
                        virtual_display = Some(guard.display.clone());
                        xvfb_available = true;
                        xvfb_guard = Some(guard);
                    } else {
                        let distro = detect_linux_distro();
                        eprintln!(
                            "\x1b[33m[duckduckgo-search-cli]\x1b[0m Xvfb não disponível — \
                             Chrome vai rodar em modo headless (menor evasão anti-bot)."
                        );
                        eprintln!("{}", xvfb_manual_instruction(&distro));
                        tracing::warn!("Xvfb unavailable — falling back to headless Chrome");
                    }
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            // macOS (Quartz) and Windows (DWM) have a real compositor — no Xvfb.
            // A headed Chrome window is controlled via stealth flags and the
            // pool is filtered to host-matching Chrome UAs (GAP-WS-107b).
            if force_headless {
                tracing::info!("Headless forced via DUCKDUCKGO_CHROME_HEADLESS");
            }
        }

        let mode = decide_head_mode(
            force_headless,
            force_visible,
            xvfb_requested,
            has_native_display(),
            xvfb_available,
        );

        let mut builder = BrowserConfig::builder()
            .chrome_executable(path)
            .user_data_dir(user_data.path())
            .launch_timeout(timeout_launch)
            // GAP-WS-108 v0.9.2: drop chromiumoxide defaults (they include
            // `--enable-automation`) and re-add the safe subset explicitly.
            .disable_default_args()
            // v0.9.8 R-12: surface unparseable CDP frames as errors instead of
            // silent drops (docs.rs BrowserConfigBuilder::surface_invalid_messages).
            .surface_invalid_messages()
            .args(CHROMIUMOXIDE_SAFE_DEFAULTS.iter().copied())
            .args(flags);

        if matches!(mode, ChromeHeadMode::HeadedXvfb) {
            if let Some(ref display) = virtual_display {
                builder = builder
                    .env("DISPLAY", display)
                    .env("WAYLAND_DISPLAY", "")
                    .arg(("ozone-platform", "x11"));
            }
        }

        match mode {
            ChromeHeadMode::HeadedXvfb => {
                builder = builder.with_head();
                tracing::info!(
                    force_visible,
                    xvfb_requested,
                    virtual_display = virtual_display.as_deref().unwrap_or("none"),
                    "Chrome running in headed mode via private Xvfb (anti-bot evasion)"
                );
            }
            ChromeHeadMode::HeadedNative => {
                builder = builder.with_head();
                tracing::info!(
                    force_visible,
                    xvfb_requested,
                    "Chrome running in headed mode on native display (anti-bot evasion)"
                );
            }
            ChromeHeadMode::Headless => {
                builder = builder.new_headless_mode();
                if !force_headless {
                    tracing::info!(
                        "No usable display — falling back to headless Chrome (anti-bot risk)"
                    );
                }
            }
        }

        if sandbox_off {
            builder = builder.no_sandbox();
        }

        let config = builder.build().map_err(|e| CliError::InvalidConfig {
            message: format!("invalid BrowserConfig: {e}"),
        })?;

        let (mut browser, mut handler) =
            Browser::launch(config)
                .await
                .map_err(|e| CliError::HttpError {
                    message: format!("failed to launch Chrome process: {e}"),
                    cause: None,
                })?;

        // Capture root Chrome PID for process-tree / marker reaping (L-01).
        let chrome_pid = browser.get_mut_child().and_then(|c| c.as_mut_inner().id());

        // Handler task: pumps events until handler returns None (closed).
        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(err) = event {
                    tracing::info!(?err, "CDP handler event with error — continuing");
                }
            }
        });

        let (xvfb_pid, xvfb_pgid, display_for_reg) = match xvfb_guard.as_ref() {
            Some(g) => {
                let (pid, pgid, disp) = g.session_bits();
                (pid, pgid, Some(disp))
            }
            None => (None, None, None),
        };

        register_session(SessionIds {
            chrome_pid,
            xvfb_pid,
            xvfb_pgid,
            user_data_dir: user_data_path.clone(),
            display: display_for_reg,
        });

        tracing::info!(
            ?chrome_pid,
            user_data = %user_data_path.display(),
            xvfb = ?virtual_display,
            "Chrome session launched (one-shot lifecycle registered)"
        );

        Ok(Self {
            browser,
            handler: Some(handler_task),
            user_data: Some(user_data),
            user_data_path,
            chrome_pid,
            xvfb: xvfb_guard,
            finalized: false,
            effective_ua,
            chrome_major,
        })
    }

    /// Accesses the internal `Browser` to create pages.
    pub fn browser_mut(&mut self) -> &mut Browser {
        &mut self.browser
    }

    /// Returns the effective UA after GAP-WS-109 version alignment.
    pub fn effective_ua(&self) -> &str {
        &self.effective_ua
    }

    /// Returns the detected Chrome major version (fallback 146).
    pub fn chrome_major(&self) -> u32 {
        self.chrome_major
    }

    /// Absolute path of this session's Chrome user-data-dir (profile [`tempfile::TempDir`]).
    ///
    /// Used as the unique kill-marker for process-tree reaping and by lifecycle
    /// integration tests (GAP-WS-LIFECYCLE-001).
    pub fn user_data_dir(&self) -> &Path {
        &self.user_data_path
    }

    /// Shuts down the browser with one-shot finalize (prefer over bare Drop).
    ///
    /// Order: cooperative `close`+`wait` under deadline → forced `kill` →
    /// process-tree + user-data-dir marker sweep → Xvfb reap → [`tempfile::TempDir`] drop.
    ///
    /// # Errors
    ///
    /// Transient CDP errors are logged and swallowed so cleanup always completes.
    ///
    /// # Cancel safety
    ///
    /// If this future is dropped mid-flight, [`Drop`] runs `force_reap_session`.
    pub async fn shutdown(mut self) -> Result<(), CliError> {
        if self.finalized {
            return Ok(());
        }
        tracing::info!(
            chrome_pid = ?self.chrome_pid,
            user_data = %self.user_data_path.display(),
            "shutting down Chrome (cooperative close with deadline)"
        );

        let close_result =
            tokio::time::timeout(SHUTDOWN_COOPERATIVE_DEADLINE, self.browser.close()).await;
        match close_result {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => tracing::info!(?err, "error closing browser — continuing"),
            Err(_) => tracing::warn!("browser.close() deadline exceeded — forcing kill"),
        }

        let wait_result =
            tokio::time::timeout(SHUTDOWN_COOPERATIVE_DEADLINE, self.browser.wait()).await;
        match wait_result {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => tracing::info!(?err, "error awaiting browser wait()"),
            Err(_) => tracing::warn!("browser.wait() deadline exceeded — forcing kill"),
        }

        // If the root is still alive, force kill via chromiumoxide then tree/marker.
        let still_alive = self.browser.try_wait().ok().flatten().is_none();
        if still_alive {
            tracing::info!("Chrome still alive after close — Browser::kill + tree reap");
            if let Some(Err(err)) = self.browser.kill().await {
                tracing::info!(?err, "Browser::kill error — continuing tree reap");
            }
        }

        if let Some(h) = self.handler.take() {
            h.abort();
            let _ = h.await;
        }

        self.force_reap_session();
        Ok(())
    }

    /// Builds the session snapshot used for force reaping.
    fn session_ids(&self) -> SessionIds {
        let (xvfb_pid, xvfb_pgid, display) = match self.xvfb.as_ref() {
            Some(g) => {
                let (pid, pgid, disp) = g.session_bits();
                (pid, pgid, Some(disp))
            }
            None => (None, None, None),
        };
        SessionIds {
            chrome_pid: self.chrome_pid,
            xvfb_pid,
            xvfb_pgid,
            user_data_dir: self.user_data_path.clone(),
            display,
        }
    }

    /// Synchronous force reap: Chrome tree + marker + Xvfb + profile dir (idempotent).
    fn force_reap_session(&mut self) {
        if self.finalized {
            return;
        }
        self.finalized = true;

        let session = self.session_ids();
        force_reap(&session);
        unregister_session(&self.user_data_path);

        // Reap Xvfb via guard (also cleans locks); mark reaped to avoid double work in Drop.
        if let Some(mut guard) = self.xvfb.take() {
            guard.reap();
        }

        // Drop TempDir after processes are dead so files are not held open.
        if let Some(dir) = self.user_data.take() {
            let path = dir.path().to_path_buf();
            drop(dir);
            if path.exists() {
                let _ = std::fs::remove_dir_all(&path);
            }
            tracing::info!(path = %path.display(), "user-data-dir removed");
        }

        tracing::info!(
            chrome_pid = ?self.chrome_pid,
            "ChromeBrowser session force-reaped (one-shot)"
        );
    }
}

impl Drop for ChromeBrowser {
    fn drop(&mut self) {
        if let Some(h) = self.handler.take() {
            h.abort();
        }
        if !self.finalized {
            tracing::warn!(
                chrome_pid = ?self.chrome_pid,
                "ChromeBrowser dropped without shutdown() — running force_reap_session"
            );
            self.force_reap_session();
        }
    }
}

/// Extracts raw HTML from a URL using headless Chrome with stealth injection.
///
/// Strategy:
/// 1. Opens a blank page and injects `navigator.webdriver = false` via CDP.
/// 2. Navigates to the target URL.
/// 3. Waits for navigation completion + 1500ms for JS rendering.
/// 4. Extracts `document.documentElement.outerHTML`.
/// 5. Truncates at `max_size` bytes and closes the page.
///
/// The `timeout` applies to the entire operation via `tokio::time::timeout`.
///
/// # Errors
///
/// Returns an error if the page cannot be opened, JS evaluation fails,
/// or the operation exceeds `timeout`.
///
/// # Cancel safety
///
/// This function is cancel-safe. The outer `tokio::time::timeout` wraps
/// the entire navigation, so dropping the future aborts the CDP session
/// and releases the browser tab.
pub async fn extract_html_with_chrome(
    browser: &mut ChromeBrowser,
    url: &str,
    max_size: usize,
    timeout: Duration,
) -> Result<String, CliError> {
    let work = async {
        // GAP-WS-109 v0.9.2: capture effective UA + major before borrowing the
        // browser mutably via `browser_mut()` (avoids borrow-checker conflict).
        let ua_str = browser.effective_ua().to_string();
        let ua_major = browser.chrome_major();

        let page = browser
            .browser_mut()
            .new_page("about:blank")
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to open blank page for {url:?}: {e}"),
                cause: None,
            })?;

        // Inject comprehensive stealth scripts before any navigation.
        // Layer 3a: webdriver, plugins, languages, chrome, maxTouchPoints, connection
        // Layer 3b: Canvas, WebGL, AudioContext, hardwareConcurrency, deviceMemory (GAP-NEW-007)
        // GAP-WS-109 v0.9.2: align navigator.userAgent + Client Hints to the real
        // Chrome major version BEFORE scripts/navigation so the override takes effect.
        apply_ua_override(&page, &ua_str, ua_major).await;
        let stealth_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_SCRIPTS);
        let _ = page.execute(stealth_cmd).await;
        let platform_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_PLATFORM_SCRIPT);
        let _ = page.execute(platform_cmd).await;

        // GAP-WS-077: warm-up navigation to duckduckgo.com
        // Cloudflare resolves the JS challenge on first visit and sets cookies.
        let _ = page.goto("https://duckduckgo.com/").await;
        let _ = page.wait_for_navigation().await;
        tokio::time::sleep(Duration::from_millis(800 + (url.len() as u64 % 700))).await;

        // Navigate to the target URL.
        page.goto(url).await.map_err(|e| CliError::HttpError {
            message: format!("failed to navigate to {url:?}: {e}"),
            cause: None,
        })?;

        // Wait for full navigation to complete (respects redirects).
        let _ = page.wait_for_navigation().await;

        // Poll for real SERP: Cloudflare may serve a JS challenge that
        // auto-resolves after a few seconds. We check every 500ms for up
        // to 8 seconds whether the page contains search result markers.
        let mut raw_html = String::new();
        for attempt in 0..16u32 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let js_result = page
                .evaluate("document.documentElement.outerHTML")
                .await
                .map_err(|e| CliError::HttpError {
                    message: format!("failed to extract outerHTML on {url:?}: {e}"),
                    cause: None,
                })?;
            raw_html = js_result.into_value().unwrap_or_default();
            if raw_html.contains("result__a") || raw_html.contains("result__snippet") {
                tracing::info!(attempt, "SERP detected after polling");
                break;
            }
            if attempt == 15 {
                tracing::info!(
                    body_len = raw_html.len(),
                    "polling exhausted — using last HTML"
                );
            }
        }

        // Close the page immediately to release the target.
        let _ = page.close().await;

        // Truncate at byte boundary.
        if raw_html.len() > max_size {
            Ok::<String, CliError>(raw_html[..max_size].to_string())
        } else {
            Ok::<String, CliError>(raw_html)
        }
    };

    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| CliError::HttpError {
            message: format!("Chrome timeout exceeded for {url:?}"),
            cause: None,
        })?
}

/// Polls the most recently opened page until `selector` matches in the
/// rendered DOM, or `timeout` elapses. GAP-WS-104 v0.8.9.
///
/// Returns `true` as soon as `document.querySelector(selector)` yields an
/// element, `false` on timeout or when no page is open. A `false` return is
/// NOT fatal: callers extract the last HTML anyway and let the extraction
/// cascade decide (Estratégia B may still recover results).
///
/// # Cancel safety
///
/// Cancel-safe: the loop only awaits `page.evaluate` and `tokio::time::sleep`.
pub async fn wait_for_selector_with_chrome(
    browser: &mut ChromeBrowser,
    selector: &str,
    poll_interval: Duration,
    timeout: Duration,
) -> bool {
    let pages = match browser.browser_mut().pages().await {
        Ok(pages) => pages,
        Err(error) => {
            tracing::warn!(%error, "wait_for_selector: failed to list pages");
            return false;
        }
    };
    let Some(page) = pages.last() else {
        tracing::warn!("wait_for_selector: no open page to poll");
        return false;
    };
    wait_for_selector_on_page(page, selector, poll_interval, timeout).await
}

/// Core polling loop shared by [`wait_for_selector_with_chrome`] and
/// [`extract_news_html_with_chrome`]. Uses `tokio::time::sleep` between
/// attempts (never blocks the async runtime).
async fn wait_for_selector_on_page(
    page: &chromiumoxide::Page,
    selector: &str,
    poll_interval: Duration,
    timeout: Duration,
) -> bool {
    wait_for_any_selector_on_page(page, &[selector], poll_interval, timeout).await
}

/// Polls until **any** of `selectors` matches (GAP-WS-AGENT-READY-001 L-04).
///
/// News SERP React markup is fragile; waiting only on
/// `[data-react-module-id="news"]` often times out while article cards already
/// exist. Callers pass a cascade of selectors.
async fn wait_for_any_selector_on_page(
    page: &chromiumoxide::Page,
    selectors: &[&str],
    poll_interval: Duration,
    timeout: Duration,
) -> bool {
    if selectors.is_empty() {
        return false;
    }
    // Build OR of querySelector checks; escape each selector for single-quoted JS.
    let parts: Vec<String> = selectors
        .iter()
        .map(|selector| {
            let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
            format!("!!document.querySelector('{escaped}')")
        })
        .collect();
    let js = format!("({})", parts.join("||"));
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        match page.evaluate(js.clone()).await {
            Ok(result) => {
                if result.into_value::<bool>().unwrap_or(false) {
                    return true;
                }
            }
            Err(error) => {
                tracing::trace!(%error, "wait_for_selector: evaluate failed — retrying");
            }
        }
        if tokio::time::Instant::now() + poll_interval > deadline {
            tracing::info!(
                selectors = ?selectors,
                "wait_for_selector: timeout — none of the selectors found"
            );
            return false;
        }
        tokio::time::sleep(poll_interval).await;
    }
}

/// Extracts the Chrome-rendered HTML of a news SERP (`ia=news&iar=news`),
/// polling for the React news module before reading `outerHTML`.
/// GAP-WS-104 v0.8.9.
///
/// Mirrors [`extract_html_with_chrome`] (stealth injection + GAP-WS-077
/// warm-up), but instead of polling for static web-SERP markers it polls
/// `wait_selector` — the news module only exists after JavaScript
/// hydration. On poll timeout the last HTML is still returned so the
/// extraction cascade can decide.
///
/// # Errors
///
/// Returns an error if the page cannot be opened, navigation or JS
/// evaluation fails, or the operation exceeds `timeout`.
///
/// # Cancel safety
///
/// Cancel-safe: the outer `tokio::time::timeout` wraps the entire
/// navigation, so dropping the future aborts the CDP session.
pub async fn extract_news_html_with_chrome(
    browser: &mut ChromeBrowser,
    url: &str,
    wait_selector: &str,
    poll_interval: Duration,
    max_size: usize,
    timeout: Duration,
) -> Result<String, CliError> {
    let work = async {
        // GAP-WS-109 v0.9.2: capture effective UA + major before borrowing the
        // browser mutably via `browser_mut()` (avoids borrow-checker conflict).
        let ua_str = browser.effective_ua().to_string();
        let ua_major = browser.chrome_major();

        let page = browser
            .browser_mut()
            .new_page("about:blank")
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to open blank page for {url:?}: {e}"),
                cause: None,
            })?;

        // Inject comprehensive stealth scripts before any navigation
        // (same layers as extract_html_with_chrome).
        // GAP-WS-109 v0.9.2: align navigator.userAgent + Client Hints to the real
        // Chrome major version BEFORE scripts/navigation so the override takes effect.
        apply_ua_override(&page, &ua_str, ua_major).await;
        let stealth_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_SCRIPTS);
        let _ = page.execute(stealth_cmd).await;
        let platform_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_PLATFORM_SCRIPT);
        let _ = page.execute(platform_cmd).await;

        // GAP-WS-077: warm-up navigation to duckduckgo.com — Cloudflare
        // resolves the JS challenge on first visit and sets cookies.
        let _ = page.goto("https://duckduckgo.com/").await;
        let _ = page.wait_for_navigation().await;
        tokio::time::sleep(Duration::from_millis(800 + (url.len() as u64 % 700))).await;

        // Navigate to the news SERP.
        page.goto(url).await.map_err(|e| CliError::HttpError {
            message: format!("failed to navigate to {url:?}: {e}"),
            cause: None,
        })?;
        let _ = page.wait_for_navigation().await;

        // Poll for React news hydration (v0.9.9):
        // Do NOT treat bare `article` / `.result__a` as ready — those appear in
        // chrome/footer and stop the poll before /news.js populates the vertical
        // (live e2e: premature extract → only promo links + no-results-message).
        let poll_budget = (timeout / 2).max(Duration::from_secs(14)).min(timeout);
        let news_ready_selectors: &[&str] = &[
            wait_selector,
            "[data-testid=\"news-vertical\"] article",
            "[data-testid=\"news-vertical\"] a[data-testid=\"result-title-a\"]",
            "[data-react-module-id=\"news\"] article",
            "[data-testid=\"news-vertical\"] a[href*=\"uddg=\"]",
            // Terminal empty state after news API settles.
            "[data-testid=\"no-results-message\"]",
        ];
        let found =
            wait_for_any_selector_on_page(&page, news_ready_selectors, poll_interval, poll_budget)
                .await;
        if !found {
            tracing::warn!(
                primary = wait_selector,
                "news module not detected after multi-selector polling — extracting last HTML anyway"
            );
        }
        // Extra settle: news.js XHR may still paint cards after first selector match.
        tokio::time::sleep(Duration::from_millis(1200)).await;

        let js_result = page
            .evaluate("document.documentElement.outerHTML")
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to extract outerHTML on {url:?}: {e}"),
                cause: None,
            })?;
        let raw_html: String = js_result.into_value().unwrap_or_default();

        // Local-only debug dump (GAP-WS-SELECTORS-XDG / news live). Not telemetry —
        // only when operator sets an absolute path env; never network upload.
        if let Ok(dump_path) = std::env::var("DUCKDUCKGO_DUMP_NEWS_HTML") {
            if !dump_path.is_empty() && !dump_path.contains("..") {
                if let Err(e) = std::fs::write(&dump_path, &raw_html) {
                    tracing::warn!(error = %e, path = %dump_path, "failed to dump news HTML");
                } else {
                    tracing::info!(path = %dump_path, bytes = raw_html.len(), "dumped news SERP HTML");
                }
            }
        }

        // Close the page immediately to release the target.
        let _ = page.close().await;

        // Truncate at a valid UTF-8 boundary (the news SERP is heavy —
        // callers pass a 1 MiB cap instead of the web-SERP 256 KiB).
        if raw_html.len() > max_size {
            let mut end = max_size;
            while end > 0 && !raw_html.is_char_boundary(end) {
                end -= 1;
            }
            Ok::<String, CliError>(raw_html[..end].to_string())
        } else {
            Ok::<String, CliError>(raw_html)
        }
    };

    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| CliError::HttpError {
            message: format!("Chrome timeout exceeded for {url:?}"),
            cause: None,
        })?
}

/// Extracts the main text from a URL using headless Chrome.
///
/// Wrapper over [`extract_html_with_chrome`] that applies text cleaning
/// (normalizes whitespace, discards short lines, truncates at `max_size`).
///
/// # Errors
///
/// Returns an error if the page cannot be opened, JS evaluation fails,
/// or the operation exceeds `timeout`.
///
/// # Cancel safety
///
/// This function is cancel-safe. The outer `tokio::time::timeout` wraps
/// the entire navigation, so dropping the future aborts the CDP session
/// and releases the browser tab.
pub async fn extract_text_with_chrome(
    browser: &mut ChromeBrowser,
    url: &str,
    max_size: usize,
    timeout: Duration,
) -> Result<String, CliError> {
    let work = async {
        // GAP-WS-109 v0.9.2: capture effective UA + major before borrowing the
        // browser mutably via `browser_mut()` (avoids borrow-checker conflict).
        let ua_str = browser.effective_ua().to_string();
        let ua_major = browser.chrome_major();

        let page = browser
            .browser_mut()
            .new_page("about:blank")
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to open blank page for {url:?}: {e}"),
                cause: None,
            })?;

        // Inject comprehensive stealth scripts before any navigation.
        // Layer 3a: webdriver, plugins, languages, chrome, maxTouchPoints, connection
        // Layer 3b: Canvas, WebGL, AudioContext, hardwareConcurrency, deviceMemory (GAP-NEW-007)
        // GAP-WS-109 v0.9.2: align navigator.userAgent + Client Hints to the real
        // Chrome major version BEFORE scripts/navigation so the override takes effect.
        apply_ua_override(&page, &ua_str, ua_major).await;
        let stealth_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_SCRIPTS);
        let _ = page.execute(stealth_cmd).await;
        let platform_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_PLATFORM_SCRIPT);
        let _ = page.execute(platform_cmd).await;

        // Navigate to the target URL.
        page.goto(url).await.map_err(|e| CliError::HttpError {
            message: format!("failed to navigate to {url:?}: {e}"),
            cause: None,
        })?;

        // Wait for full navigation to complete (respects redirects).
        let _ = page.wait_for_navigation().await;

        // Allow time for JS rendering.
        tokio::time::sleep(Duration::from_millis(1500)).await;

        let js_result = page
            .evaluate("document.body ? document.body.innerText : ''")
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to execute innerText on {url:?}: {e}"),
                cause: None,
            })?;

        let raw_text: String = js_result.into_value().unwrap_or_default();

        // Close the page immediately to release the target.
        let _ = page.close().await;

        Ok::<String, CliError>(clean_text(&raw_text, max_size))
    };

    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| CliError::HttpError {
            message: format!("Chrome timeout exceeded for {url:?}"),
            cause: None,
        })?
}

/// Cleans raw text: normalizes whitespace, discards short lines, truncates at `max_size`.
fn clean_text(raw: &str, max_size: usize) -> String {
    let lines: Vec<String> = raw
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| line.chars().count() >= MIN_LINE_LENGTH)
        .collect();
    let joined = lines.join("\n");
    truncate_at_word(&joined, max_size)
}

/// Truncates respecting word boundary. Mirrors the implementation in `content.rs`.
fn truncate_at_word(text: &str, max_size: usize) -> String {
    if max_size == 0 {
        return String::new();
    }
    let total: usize = text.chars().count();
    if total <= max_size {
        return text.to_string();
    }
    let prefix: String = text.chars().take(max_size).collect();
    if let Some(pos) = prefix.rfind(char::is_whitespace) {
        return prefix[..pos].trim_end().to_string();
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// Serializes tests that mutate process-global env vars (`set_var`/`remove_var`
    /// are not thread-safe and race under parallel `cargo test`).
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn chrome_candidate_paths_not_empty() {
        let paths = chrome_candidate_paths();
        assert!(!paths.is_empty(), "deve retornar ao menos um candidato");
    }

    #[test]
    fn detect_chrome_manual_path_nonexistent_fails() {
        let p = Path::new("/tmp/caminho/absolutamente/inexistente/chrome-xyz");
        assert!(
            detect_chrome(Some(p)).is_err(),
            "caminho manual inválido deve falhar"
        );
    }

    #[test]
    fn stealth_flags_include_anti_detection() {
        let f = flags_stealth(false, None, "TestAgent/1.0");
        assert!(f.iter().any(|x| x.contains("AutomationControlled")));
        assert!(f.iter().any(|x| x == "--window-size=1920,1080"));
        assert!(f.iter().any(|x| x == "--window-position=-32000,-32000"));
        assert!(f.iter().any(|x| x == "--disable-infobars"));
        assert!(f.iter().any(|x| x == "--user-agent=TestAgent/1.0"));
        assert!(
            !f.iter().any(|x| x == "--disable-extensions"),
            "--disable-extensions is a bot detection signal and must not be present"
        );
    }

    #[test]
    fn stealth_flags_include_proxy_when_provided() {
        let f = flags_stealth(false, Some("http://proxy:8080"), "TestAgent/1.0");
        assert!(f.iter().any(|x| x == "--proxy-server=http://proxy:8080"));
    }

    #[test]
    fn stealth_flags_no_sandbox_only_when_required_on_linux() {
        let f_com = flags_stealth(true, None, "TestAgent/1.0");
        let f_sem = flags_stealth(false, None, "TestAgent/1.0");
        #[cfg(target_os = "linux")]
        {
            assert!(f_com.iter().any(|x| x == "--no-sandbox"));
            assert!(!f_sem.iter().any(|x| x == "--no-sandbox"));
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (f_com, f_sem);
        }
    }

    #[test]
    fn clean_text_removes_short_lines() {
        let raw = "ok\noutra linha com tamanho bastante suficiente de vinte chars\ncurta\n";
        let clean = clean_text(raw, 1000);
        assert!(clean.contains("outra linha"));
        assert!(!clean.contains("ok\n"));
    }

    #[test]
    fn clean_text_truncates_at_word() {
        let raw =
            "linha um com mais de vinte caracteres definitivamente aqui presentes\n".repeat(10);
        let clean = clean_text(&raw, 50);
        assert!(clean.chars().count() <= 50);
    }

    #[test]
    fn precisa_no_sandbox_flatpak_path() {
        let p = Path::new("/var/lib/flatpak/exports/bin/com.google.Chrome");
        #[cfg(target_os = "linux")]
        assert!(needs_no_sandbox(p));
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }

    #[test]
    fn precisa_no_sandbox_snap_path() {
        let p = Path::new("/snap/bin/chromium");
        #[cfg(target_os = "linux")]
        assert!(needs_no_sandbox(p));
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }

    #[test]
    fn needs_no_sandbox_default_returns_false() {
        let p = Path::new("/usr/bin/chromium");
        #[cfg(target_os = "linux")]
        {
            // False UNLESS DOCKER_CONTAINER or /.dockerenv is present.
            let expected = std::env::var("DOCKER_CONTAINER").is_ok()
                || std::path::Path::new("/.dockerenv").exists();
            assert_eq!(needs_no_sandbox(p), expected);
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }

    #[test]
    fn needs_no_sandbox_flatpak_deploy_elf_path() {
        let p =
            Path::new("/var/lib/flatpak/app/com.google.Chrome/current/active/files/extra/chrome");
        #[cfg(target_os = "linux")]
        assert!(needs_no_sandbox(p));
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }

    #[test]
    fn classify_chrome_channel_flatpak_and_host() {
        assert_eq!(
            classify_chrome_channel(Path::new(
                "/var/lib/flatpak/app/com.google.Chrome/current/active/files/extra/chrome"
            )),
            ChromeChannel::Flatpak
        );
        assert_eq!(
            classify_chrome_channel(Path::new("/usr/lib64/chromium-browser/chromium-browser")),
            ChromeChannel::Host
        );
    }

    #[test]
    fn resolve_chrome_candidate_rejects_missing() {
        assert!(resolve_chrome_candidate(Path::new("/no/such/chrome-binary-xyz")).is_none());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn resolve_flatpak_export_script_when_deploy_present() {
        let export = Path::new("/var/lib/flatpak/exports/bin/com.google.Chrome");
        if !export.is_file() {
            return;
        }
        let resolved = resolve_chrome_candidate(export);
        if let Some(path) = resolved {
            assert!(
                is_executable_chrome_binary(&path),
                "resolved path must be a real binary: {}",
                path.display()
            );
            assert!(
                path.to_string_lossy().contains("files/extra/chrome")
                    || is_executable_chrome_binary(&path)
            );
        }
    }

    #[test]
    fn is_xvfb_requested_false_by_default() {
        let _guard = env_lock();
        let prev = std::env::var("DUCKDUCKGO_CHROME_XVFB").ok();
        std::env::remove_var("DUCKDUCKGO_CHROME_XVFB");
        let result = is_xvfb_requested();
        if let Some(v) = prev {
            std::env::set_var("DUCKDUCKGO_CHROME_XVFB", v);
        }
        assert!(!result);
    }

    #[test]
    fn is_xvfb_requested_true_when_set() {
        let _guard = env_lock();
        let prev = std::env::var("DUCKDUCKGO_CHROME_XVFB").ok();
        std::env::set_var("DUCKDUCKGO_CHROME_XVFB", "1");
        let result = is_xvfb_requested();
        std::env::remove_var("DUCKDUCKGO_CHROME_XVFB");
        if let Some(v) = prev {
            std::env::set_var("DUCKDUCKGO_CHROME_XVFB", v);
        }
        assert!(result);
    }

    #[test]
    fn headed_requires_explicit_opt_in() {
        let _guard = env_lock();
        let prev_vis = std::env::var("DUCKDUCKGO_CHROME_VISIBLE").ok();
        let prev_xvfb = std::env::var("DUCKDUCKGO_CHROME_XVFB").ok();
        std::env::remove_var("DUCKDUCKGO_CHROME_VISIBLE");
        std::env::remove_var("DUCKDUCKGO_CHROME_XVFB");
        let result = is_xvfb_requested();
        if let Some(v) = prev_vis {
            std::env::set_var("DUCKDUCKGO_CHROME_VISIBLE", v);
        }
        if let Some(v) = prev_xvfb {
            std::env::set_var("DUCKDUCKGO_CHROME_XVFB", v);
        }
        assert!(!result);
    }

    #[test]
    fn has_native_display_respects_env() {
        #[cfg(target_os = "linux")]
        {
            let _guard = env_lock();
            let orig_display = std::env::var("DISPLAY").ok();
            let orig_wayland = std::env::var("WAYLAND_DISPLAY").ok();

            std::env::remove_var("DISPLAY");
            std::env::remove_var("WAYLAND_DISPLAY");
            assert!(!has_native_display(), "no display vars = no native display");

            std::env::set_var("DISPLAY", ":0");
            assert!(
                has_native_display(),
                "DISPLAY=:0 should detect native display"
            );

            std::env::remove_var("DISPLAY");
            std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
            assert!(
                has_native_display(),
                "WAYLAND_DISPLAY should detect native display"
            );

            // Restore
            std::env::remove_var("WAYLAND_DISPLAY");
            if let Some(d) = orig_display {
                std::env::set_var("DISPLAY", d);
            }
            if let Some(d) = orig_wayland {
                std::env::set_var("WAYLAND_DISPLAY", d);
            }
        }
        #[cfg(target_os = "macos")]
        {
            assert!(has_native_display(), "macOS always has a display");
        }
        #[cfg(target_os = "windows")]
        {
            assert!(has_native_display(), "Windows always has a display");
        }
    }

    // GAP-WS-107 v0.9.1: decide_head_mode é a decisão de cabeça extraída de launch().

    #[test]
    fn decide_head_mode_force_headless_overrides_all() {
        assert_eq!(
            decide_head_mode(true, false, false, true, true),
            ChromeHeadMode::Headless,
            "force_headless deve vencer todos os outros inputs"
        );
        assert_eq!(
            decide_head_mode(true, true, true, true, true),
            ChromeHeadMode::Headless,
            "force_headless deve vencer mesmo com force_visible + xvfb"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn decide_head_mode_macos_native_display_uses_headless_new() {
        // GAP-WS-112 v0.9.3: macOS (Quartz) clampa --window-position, então
        // headed nativo abriria janela visível. Padrão agora é Headless
        // (headless=new), validado empiricamente 4/4 no DDG com fixes v0.9.2.
        // Supersedes a regressão do GAP-WS-107 que forçava HeadedNative.
        assert_eq!(
            decide_head_mode(false, false, false, true, false),
            ChromeHeadMode::Headless,
            "macOS com display nativo deve usar Headless (headless=new) — sem janela visível"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn decide_head_mode_macos_force_visible_still_allows_headed_native() {
        // Escape hatch: DUCKDUCKGO_CHROME_VISIBLE=1 continua forçando HeadedNative
        // no macOS para depuração visual, mesmo com o padrão headless=new.
        assert_eq!(
            decide_head_mode(false, true, false, true, false),
            ChromeHeadMode::HeadedNative,
            "DUCKDUCKGO_CHROME_VISIBLE=1 no macOS deve forçar HeadedNative para debug"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn decide_head_mode_windows_native_display_uses_headless_new() {
        // GAP-WS-112 v0.9.3: Windows (DWM) clampa --window-position, então
        // headed nativo abriria janela visível. Padrão agora é Headless.
        assert_eq!(
            decide_head_mode(false, false, false, true, false),
            ChromeHeadMode::Headless,
            "Windows com display nativo deve usar Headless (headless=new) — sem janela visível"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn decide_head_mode_linux_native_prefers_xvfb() {
        assert_eq!(
            decide_head_mode(false, false, false, true, true),
            ChromeHeadMode::HeadedXvfb,
            "Linux com display nativo + Xvfb deve usar HeadedXvfb"
        );
        assert_eq!(
            decide_head_mode(false, false, false, true, false),
            ChromeHeadMode::Headless,
            "Linux com display nativo mas SEM Xvfb cai em Headless (sem regressão)"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn decide_head_mode_linux_no_display_uses_xvfb() {
        assert_eq!(
            decide_head_mode(false, false, false, false, true),
            ChromeHeadMode::HeadedXvfb,
            "Linux sem display nativo + Xvfb deve usar HeadedXvfb"
        );
        assert_eq!(
            decide_head_mode(false, false, false, false, false),
            ChromeHeadMode::Headless,
            "Linux sem display e sem Xvfb cai em Headless"
        );
    }

    // GAP-WS-108 v0.9.2: safe defaults must NEVER inject --enable-automation.

    #[test]
    fn safe_defaults_exclude_enable_automation() {
        assert!(
            !CHROMIUMOXIDE_SAFE_DEFAULTS
                .iter()
                .any(|a| a.contains("enable-automation")),
            "--enable-automation seta navigator.webdriver legacy e é detectável; não pode estar nos safe defaults"
        );
        // Sanity: a constante NÃO está vazia (regressão de exclusão acidental).
        assert!(
            !CHROMIUMOXIDE_SAFE_DEFAULTS.is_empty(),
            "safe defaults não podem ficar vazios após remover enable-automation"
        );
    }

    #[test]
    fn flags_stealth_disables_webrtc_and_quic() {
        let f = flags_stealth(false, None, "Mozilla/5.0 Chrome/146.0.0.0");
        // GAP-WS-110: WebRTC vazava IP real mesmo atrás de proxy.
        assert!(
            f.iter().any(|x| x.contains("WebRtcHideLocalIpsWithMdns")),
            "deve suprimir mDNS WebRTC"
        );
        assert!(
            f.iter()
                .any(|x| x == "--enforce-webrtc-ip-permission-check"),
            "deve forçar verificação de permissão de IP WebRTC"
        );
        assert!(
            f.iter()
                .any(|x| x == "--force-webrtc-ip-handling-policy=disable_non_proxied_udp"),
            "deve restringir ICE a UDP não-proxied"
        );
        assert!(
            f.iter().any(|x| x == "--disable-webrtc-hw-decoding"),
            "deve desabilitar decodificação WebRTC por hardware"
        );
        // GAP-WS-111: QUIC tem fingerprint UDP distinto do TLS do UA spoofado.
        assert!(
            f.iter().any(|x| x == "--disable-quic"),
            "deve desabilitar QUIC (GQuic/HTTP3)"
        );
    }

    #[test]
    fn flags_stealth_still_excludes_disable_extensions() {
        let f = flags_stealth(false, None, "Mozilla/5.0 Chrome/146.0.0.0");
        assert!(
            !f.iter().any(|x| x == "--disable-extensions"),
            "--disable-extensions é sinal de automação detectável; não pode voltar à lista stealth"
        );
    }

    /// Testa o parser de `chrome --version` sem depender de um Chrome real
    /// instalado: cria um shim executável que imprime "Google Chrome 146.0.0.0".
    #[test]
    fn detect_chrome_major_version_parses_output() {
        let tmp = tempfile::NamedTempFile::new().expect("temp file");
        let shim = tmp.path().with_extension("sh");
        std::fs::write(
            &shim,
            "#!/bin/sh\necho \"Google Chrome 146.0.7561.0 beta\"\n",
        )
        .expect("write shim");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&shim).expect("metadata").permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&shim, perm).expect("chmod");
        }
        let major = detect_chrome_major_version(&shim);
        assert_eq!(
            major,
            Some(146),
            "parser deve extrair major 146 de \"Google Chrome 146.0.7561.0 beta\""
        );

        // Chromium variant também deve ser parseada.
        std::fs::write(&shim, "#!/bin/sh\necho \"Chromium 999.0.0.0\"\n").expect("rewrite shim");
        assert_eq!(
            detect_chrome_major_version(&shim),
            Some(999),
            "parser deve extrair major 999 de \"Chromium 999.0.0.0\""
        );

        // Output sem marcador conhecido -> None.
        std::fs::write(&shim, "#!/bin/sh\necho \"outro navegador 42\"\n").expect("rewrite shim");
        assert_eq!(
            detect_chrome_major_version(&shim),
            None,
            "output sem \"Chrome \"/\"Chromium \" deve retornar None"
        );

        // Path inexistente -> None (sem panic).
        assert_eq!(
            detect_chrome_major_version(std::path::Path::new("/nao/existe/chrome-xyz")),
            None,
            "path inexistente deve retornar None sem panic"
        );
    }
}
