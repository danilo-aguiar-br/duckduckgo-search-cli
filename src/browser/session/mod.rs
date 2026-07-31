// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (Chrome CDP session launch / one-shot lifecycle).
// Parallelism: one OS Chrome process per session; pool fan-out lives in content_fetch.
//! [`ChromeBrowser`] RAII session: launch, stealth flags, CDP lifecycle, force-reap.
//!
//! Launch flags / mute / UA helpers live in [`flags`] (SRP).

mod flags;

pub use flags::{flags_stealth, set_chrome_display_cli, ChromeDisplayCli};
#[allow(unused_imports)] // re-exported for browser tests / extract; not all used in launch path
pub(crate) use flags::{
    apply_ua_override, chrome_display_cli, chrome_proxy_server_arg, chromiumoxide_arg_token,
    chromiumoxide_rendered_arg, ensure_chrome_audio_muted, ensure_chrome_audio_muted_rendered,
};

use flags::{chrome_launch_env_allowlist, chrome_launch_gate};

use super::detect::{detect_chrome_major_version_async, needs_no_sandbox};
use super::xvfb::{
    detect_linux_distro, has_native_display, spawn_virtual_display, try_auto_install_xvfb,
    xvfb_manual_instruction, XvfbGuard,
};
use super::{
    CHROME_AUTOPLAY_POLICY_FLAG, CHROME_MUTE_AUDIO_FLAG, CHROMIUMOXIDE_SAFE_DEFAULTS,
    SHUTDOWN_COOPERATIVE_DEADLINE,
};
use crate::error::{chrome_launch_error, CliError};
use crate::process_lifecycle::{
    force_reap, install_panic_reap_hook, register_session, unregister_session, SessionIds,
};
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task::JoinHandle;

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

/// Chrome head-mode decision extracted from `launch()` (GAP-WS-107 v0.9.1).
/// Pure + cross-platform testable via `cfg!`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChromeHeadMode {
    Headless,
    HeadedXvfb,
    HeadedNative,
}

/// Decides Chrome head mode in a pure, cfg-gated way (GAP-WS-107 v0.9.1).
///
/// macOS/Windows with native display now return `HeadedNative` (previously fell
/// em headless porque `spawn_virtual_display()` retorna `None` fora do Linux).
#[allow(clippy::too_many_arguments)]
pub(crate) fn decide_head_mode(
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
        // Xvfb privado (HeadedXvfb). `--chrome-visible` still forces
        // HeadedNative for visual debugging (GAP-SCRAPE-R-007).
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
        //
        // GAP-PAR-042: probe is blocking (`std::thread::sleep` poll + subprocess).
        // Offload via run_cpu_bound so multi-process pool launches do not stall
        // Tokio workers; process-wide path cache collapses N probes to 1.
        let chrome_major = detect_chrome_major_version_async(path)
            .await?
            .unwrap_or(146);
        let effective_ua = crate::identity::rewrite_ua_chrome_version(user_agent, chrome_major);
        // GAP-E2E-V11-UA-MAJOR-SKEW: log the *effective* UA (post major rewrite),
        // never the stale pool string alone.
        tracing::info!(
            chrome_major,
            effective_ua = %effective_ua,
            pool_ua = %user_agent,
            "Chrome identity aligned to host major"
        );

        install_panic_reap_hook();

        // GAP-E2E-V11-CHROME-FLAKY: single-flight cold-start across the process.
        let _launch_gate = chrome_launch_gate().lock().await;

        let flags = flags_stealth(sandbox_off, proxy, &effective_ua);
        // ADR-0026: fail closed before BrowserConfig if mute operational standard
        // is missing from either safe defaults or stealth flags.
        let launch_arg_sources = CHROMIUMOXIDE_SAFE_DEFAULTS
            .iter()
            .copied()
            .chain(flags.iter().map(String::as_str));
        // Materialize once so source + rendered checks share the same list.
        let launch_arg_sources: Vec<&str> = launch_arg_sources.collect();
        ensure_chrome_audio_muted(launch_arg_sources.iter().copied())?;
        // GAP-CHROME-MUTE-002: also validate the *rendered* argv chromiumoxide
        // will emit (must be `--mute-audio`, never `----mute-audio`).
        ensure_chrome_audio_muted_rendered(launch_arg_sources.iter().copied())?;
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

        // GAP-SCRAPE-R2-001/002: CLI flags via process policy only (no product env).
        let display = chrome_display_cli();
        let force_visible = display.force_visible;
        let xvfb_requested = display.force_xvfb;
        let force_headless = display.force_headless;

        // Headed Chrome passes Cloudflare anti-bot; headless is detectable.
        // Priority: --chrome-headless > --chrome-visible / --chrome-xvfb > native display
        // > Xvfb auto-spawn > headless fallback.
        // GAP-WS-107: decision extracted into `decide_head_mode` (pure, cfg-gated).
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
                if let Some(guard) = spawn_virtual_display().await {
                    tracing::info!(
                        xvfb = %guard.display,
                        "Using private Xvfb for invisible headed mode"
                    );
                    virtual_display = Some(guard.display.clone());
                    xvfb_available = true;
                    xvfb_guard = Some(guard);
                } else {
                    try_auto_install_xvfb();
                    if let Some(guard) = spawn_virtual_display().await {
                        tracing::info!(
                            xvfb = %guard.display,
                            "Xvfb auto-installed — using private display"
                        );
                        virtual_display = Some(guard.display.clone());
                        xvfb_available = true;
                        xvfb_guard = Some(guard);
                    } else {
                        let distro = detect_linux_distro();
                        crate::output::emit_stderr(
                            crate::i18n::Message::XvfbUnavailableHeadlessFallback
                                .text(crate::i18n::language()),
                        );
                        crate::output::emit_stderr(xvfb_manual_instruction(&distro));
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
                tracing::info!("Headless forced via --chrome-headless (CLI)");
            }
        }

        let mode = decide_head_mode(
            force_headless,
            force_visible,
            xvfb_requested,
            has_native_display(),
            xvfb_available,
        );

        // GAP-CHROME-MUTE-002: strip leading `--` before chromiumoxide.
        // Without this, ArgsBuilder emits `----mute-audio` (ignored by Chromium)
        // and headed Xvfb sessions play host audio on video/news pages.
        let safe_defaults_ox: Vec<&str> = CHROMIUMOXIDE_SAFE_DEFAULTS
            .iter()
            .copied()
            .map(chromiumoxide_arg_token)
            .collect();
        let flags_ox: Vec<&str> = flags.iter().map(|s| chromiumoxide_arg_token(s.as_str())).collect();

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
            .args(safe_defaults_ox)
            .args(flags_ox)
            // GAP-SECDEV-008: only push allowlisted env keys through the builder.
            .envs(chrome_launch_env_allowlist());

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

        // GAP-E2E-V11-EXIT-TAXONOMY: launch failures are Chrome transport, not HTTP.
        // In-process multi-attempt retry lives in `pipeline::chrome::launch_chrome_browser`
        // (full session rebuild). Gate above serialises cold-start.
        let (mut browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| chrome_launch_error(&e))?;

        // Capture root Chrome PID for process-tree / marker reaping (L-01).
        let chrome_pid = browser.get_mut_child().and_then(|c| c.as_mut_inner().id());

        // Handler task: pumps events until handler returns None (closed).
        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(err) = event {
                    // GAP-E2E-48-011: Chrome major ahead of chromiumoxide PDL often
                    // yields InvalidMessage noise (e.g. Network.*ExtraInfo). Keep
                    // hot-path at debug so agent stderr stays clean without `-q`.
                    tracing::debug!(?err, "CDP handler event with error — continuing");
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
            effective_ua = %effective_ua,
            chrome_major,
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
