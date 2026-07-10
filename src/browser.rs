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
//! ## Process Cleanup and Safety (`rules_rust.md` — Memory Management)
//!
//! `chromiumoxide::Browser` starts a child Chrome process. Without explicit cleanup,
//! the process becomes a zombie. The [`Drop`] implementation on [`ChromeBrowser`]
//! aborts the handler task and signals `kill_on_drop` internally. For complete
//! synchronous cleanup, prefer calling [`ChromeBrowser::shutdown`] before drop.

#![cfg(feature = "chrome")]

use crate::error::CliError;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task::JoinHandle;

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

/// Returns an ordered list of candidate paths for Chrome/Chromium by platform.
///
/// Includes native installations, Flatpak, and Snap. Windows consults
/// environment variables (`%PROGRAMFILES%`, `%LOCALAPPDATA%`) when available.
pub fn chrome_candidate_paths() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "linux")]
    {
        for base in [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/local/bin/chromium",
            "/usr/local/bin/google-chrome",
            "/opt/google/chrome/chrome",
            "/snap/bin/chromium",
            "/snap/bin/google-chrome",
            "/var/lib/flatpak/exports/bin/com.google.Chrome",
            "/var/lib/flatpak/exports/bin/org.chromium.Chromium",
            // v0.8.0 GAP-NEW-005: prefer raw Chromium binary over wrapper script
            // (Fedora/RHEL install the actual binary at this path; the .sh wrapper
            //  invokes PATH `timeout` which is shadowed by the Rust crate timeout-cli).
            "/usr/lib64/chromium-browser/chromium-browser",
            "/usr/lib/chromium-browser/chromium-browser",
            "/usr/lib/chromium-browser/chrome",
        ] {
            candidates.push(PathBuf::from(base));
        }
        if let Some(home) = dirs::home_dir() {
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

/// Detects the Chrome/Chromium executable with a 3-layer hierarchy.
///
/// Resolution order:
/// 1. `manual_path` (typically `--chrome-path`). If provided but invalid,
///    returns an error — does NOT fall back silently.
/// 2. `CHROME_PATH` environment variable (if set and points to an existing file).
/// 3. Auto-detection via [`chrome_candidate_paths`] — first found wins.
///
/// Returns `Err` if no candidate is found.
///
/// # Errors
///
/// Returns an error if `manual_path` is provided but does not point to an
/// existing file, or if no Chrome/Chromium executable is found on the system.
pub fn detect_chrome(manual_path: Option<&Path>) -> Result<PathBuf, CliError> {
    // Layer 1: manual --chrome-path argument (highest priority, bypasses all checks).
    if let Some(p) = manual_path {
        if is_executable_chrome_binary(p) {
            tracing::info!(path = %p.display(), "Chrome found via --chrome-path");
            return Ok(p.to_path_buf());
        }
        return Err(CliError::PathError {
            message: format!(
                "--chrome-path {:?} is not a valid Chrome/Chromium binary (missing, not a file, or shell script)",
                p.display()
            ),
        });
    }

    // Layer 2: CHROME_PATH env var override.
    if let Ok(env_path) = std::env::var("CHROME_PATH") {
        let p = PathBuf::from(&env_path);
        if is_executable_chrome_binary(&p) {
            tracing::info!(path = %p.display(), "Chrome found via CHROME_PATH");
            return Ok(p);
        }
        tracing::warn!(
            path = env_path,
            "CHROME_PATH set but file is missing or is a shell script — trying auto-detection"
        );
    }

    // Layer 3: PATH lookup via `which` crate (cross-platform: Linux/macOS/Windows).
    for binary_name in [
        "chromium",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
    ] {
        if let Ok(p) = which::which(binary_name) {
            if is_executable_chrome_binary(&p) {
                tracing::info!(
                    binary = binary_name,
                    path = %p.display(),
                    "Chrome found via PATH lookup (which crate)"
                );
                return Ok(p);
            }
            tracing::debug!(
                binary = binary_name,
                path = %p.display(),
                "which crate found candidate but rejected: not a real binary"
            );
        }
    }

    // Layer 4: platform-specific well-known installation paths.
    for candidate in chrome_candidate_paths() {
        if is_executable_chrome_binary(&candidate) {
            tracing::info!(path = %candidate.display(), "Chrome found at platform-specific path");
            return Ok(candidate);
        }
    }

    Err(CliError::PathError {
        message: "Chrome/Chromium not found. Install via your package manager or provide --chrome-path or CHROME_PATH.".into(),
    })
}

/// v0.8.0 GAP-NEW-005: rejects shell-script wrappers (e.g. `chromium-browser.sh`)
/// which call the Rust `timeout` crate binary and kill Chrome in ~0.1s. Validates
/// that the candidate is a real ELF/Mach-O executable, not a text file.
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
    match std::fs::read(path) {
        Ok(bytes) if bytes.len() >= 4 => {
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

/// Spawns a private Xvfb server on a free display number so Chrome can run
/// in headed mode (passing Cloudflare anti-bot) without showing a visible
/// window to the user.
///
/// Returns `(child_process, display_string)` on success, or `None` if Xvfb
/// is not available or no free display slot was found.
#[cfg(target_os = "linux")]
fn spawn_virtual_display() -> Option<(std::process::Child, String)> {
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
        let child = std::process::Command::new(&xvfb_path)
            .arg(&disp)
            .args(["-screen", "0", "1920x1080x24", "-nolisten", "tcp", "-ac"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;

        std::thread::sleep(std::time::Duration::from_millis(150));

        if std::path::Path::new(&lock_path).exists() {
            tracing::info!(xvfb_display = %disp, "Xvfb virtual display started");
            return Some((child, disp));
        }
        // Xvfb failed to create lock — try next display number.
    }
    None
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)] // only invoked under the cfg(target_os = "linux") launch path
fn spawn_virtual_display() -> Option<(std::process::Child, String)> {
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
pub fn needs_no_sandbox(chrome_path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        // Wrapper Flatpak ou Snap.
        let s = chrome_path.to_string_lossy();
        if s.contains("flatpak/exports/bin") || s.starts_with("/snap/") {
            return true;
        }
        // Rodando como root (comum em Docker).
        // SAFETY: libc::geteuid is thread-safe and has no side effects.
        #[cfg(unix)]
        {
            // Simplification: detect via Docker environment variable.
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
/// **Cleanup:** prefer calling [`ChromeBrowser::shutdown`] explicitly (async).
/// [`Drop`] only aborts the handler task — the Chrome process may take a few ms
/// to terminate. For long-running applications, ALWAYS use `shutdown`.
pub struct ChromeBrowser {
    /// The underlying chromiumoxide browser handle.
    browser: Browser,
    /// Join handle for the event-loop task; aborted on `Drop` if still alive.
    handler: Option<JoinHandle<()>>,
    /// Keeps `TempDir` alive to ensure user-data-dir is removed on drop.
    _user_data: tempfile::TempDir,
    /// Private Xvfb process for headed-but-invisible Chrome.
    /// Killed on drop so the virtual display does not leak.
    _xvfb: Option<std::process::Child>,
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
            .field("user_data_dir", &self._user_data.path())
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

        let flags = flags_stealth(sandbox_off, proxy, &effective_ua);
        let user_data = tempfile::tempdir().map_err(|e| CliError::PathError {
            message: format!("failed to create user-data-dir TempDir: {e}"),
        })?;

        let force_visible = std::env::var("DUCKDUCKGO_CHROME_VISIBLE").is_ok();
        let xvfb_requested = is_xvfb_requested();
        let force_headless = std::env::var("DUCKDUCKGO_CHROME_HEADLESS").is_ok();

        // Headed Chrome passes Cloudflare anti-bot; headless is detectable.
        // Priority: HEADLESS env (force) > VISIBLE env > native display > Xvfb auto-spawn > headless fallback.
        // GAP-WS-107 v0.9.1: decisão extraída para `decide_head_mode` (pura, cfg-gated);
        // macOS/Windows com display nativo agora usam headed nativo em vez de cair em headless.
        #[allow(unused_mut)] // reassigned only under cfg(target_os = "linux")
        let mut xvfb_child: Option<std::process::Child> = None;
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
                if let Some((child, vdisplay)) = spawn_virtual_display() {
                    tracing::info!(
                        xvfb = %vdisplay,
                        "Using private Xvfb for invisible headed mode"
                    );
                    xvfb_child = Some(child);
                    virtual_display = Some(vdisplay);
                    xvfb_available = true;
                } else {
                    try_auto_install_xvfb();
                    if let Some((child, vdisplay)) = spawn_virtual_display() {
                        tracing::info!(
                            xvfb = %vdisplay,
                            "Xvfb auto-installed — using private display"
                        );
                        xvfb_child = Some(child);
                        virtual_display = Some(vdisplay);
                        xvfb_available = true;
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

        let (browser, mut handler) =
            Browser::launch(config)
                .await
                .map_err(|e| CliError::HttpError {
                    message: format!("failed to launch Chrome process: {e}"),
                    cause: None,
                })?;

        // Handler task: pumps events until handler returns None (closed).
        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(err) = event {
                    tracing::info!(?err, "CDP handler event with error — continuing");
                }
            }
        });

        Ok(Self {
            browser,
            handler: Some(handler_task),
            _user_data: user_data,
            _xvfb: xvfb_child,
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

    /// Shuts down the browser and awaits handler cleanup. Prefer this over Drop.
    ///
    /// # Errors
    ///
    /// Returns an error only if the underlying `close()` or `wait()` calls
    /// propagate a fatal CDP protocol error; transient errors are logged and
    /// swallowed so cleanup always completes.
    ///
    /// # Cancel safety
    ///
    /// This function is cancel-safe. If dropped before completion, the handler
    /// task is aborted and the Chrome process is terminated via `kill_on_drop`.
    pub async fn shutdown(mut self) -> Result<(), CliError> {
        tracing::info!("shutting down Chrome via close() + wait()");
        if let Err(err) = self.browser.close().await {
            tracing::info!(?err, "error closing browser — continuing");
        }
        if let Err(err) = self.browser.wait().await {
            tracing::info!(?err, "error awaiting browser wait()");
        }
        if let Some(h) = self.handler.take() {
            h.abort();
            let _ = h.await;
        }
        Ok(())
    }
}

impl Drop for ChromeBrowser {
    fn drop(&mut self) {
        if let Some(h) = self.handler.take() {
            h.abort();
        }
        if let Some(ref mut xvfb) = self._xvfb {
            let _ = xvfb.kill();
            let _ = xvfb.wait();
            tracing::info!("Xvfb virtual display stopped");
        }
        tracing::info!(
            "ChromeBrowser dropped — chromiumoxide Browser::drop handles remaining cleanup"
        );
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
    // Escape for embedding inside a single-quoted JS string literal.
    let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
    let js = format!("!!document.querySelector('{escaped}')");
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
            tracing::info!(selector, "wait_for_selector: timeout — selector not found");
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

        // Poll for React hydration of the news module. Budget: half the
        // outer timeout, leaving room for extraction. `false` is non-fatal.
        let found =
            wait_for_selector_on_page(&page, wait_selector, poll_interval, timeout / 2).await;
        if !found {
            tracing::warn!(
                selector = wait_selector,
                "news module not detected after polling — extracting last HTML anyway"
            );
        }

        let js_result = page
            .evaluate("document.documentElement.outerHTML")
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to extract outerHTML on {url:?}: {e}"),
                cause: None,
            })?;
        let raw_html: String = js_result.into_value().unwrap_or_default();

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
    fn is_xvfb_requested_false_by_default() {
        std::env::remove_var("DUCKDUCKGO_CHROME_XVFB");
        assert!(!is_xvfb_requested());
    }

    #[test]
    fn is_xvfb_requested_true_when_set() {
        std::env::set_var("DUCKDUCKGO_CHROME_XVFB", "1");
        let result = is_xvfb_requested();
        std::env::remove_var("DUCKDUCKGO_CHROME_XVFB");
        assert!(result);
    }

    #[test]
    fn headed_requires_explicit_opt_in() {
        std::env::remove_var("DUCKDUCKGO_CHROME_VISIBLE");
        std::env::remove_var("DUCKDUCKGO_CHROME_XVFB");
        assert!(!is_xvfb_requested());
    }

    #[test]
    fn has_native_display_respects_env() {
        #[cfg(target_os = "linux")]
        {
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
