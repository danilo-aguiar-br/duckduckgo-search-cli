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
//! | [`detect`] | Path candidates, channel, version probe, `--no-sandbox` heuristic |
//! | [`xvfb`] | Private Xvfb display RAII (Linux) |
//! | [`session`] | [`ChromeBrowser`] launch / stealth flags / one-shot reap |
//! | [`extract`] | CDP navigation + HTML/text extract helpers |
//! | [`stealth`] | Static CDP stealth script payloads |
//!
//! ## Process Cleanup and Safety (GAP-WS-LIFECYCLE-001 / one-shot)
//!
//! Prefer async [`ChromeBrowser::shutdown`]; [`Drop`] force-reaps the Chrome
//! process tree, Xvfb, and profile dir. Contract: after the CLI exits, no
//! Chromium/Xvfb/profile from **this** invocation remains.

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

/// chromiumoxide's `DEFAULT_ARGS` minus `enable-automation` (GAP-WS-108 v0.9.2).
///
/// `chromiumoxide` injects `--enable-automation` by default, which sets the
/// `navigator.webdriver` legacy flag and is detectable by Cloudflare via the
/// `chrome.runtime` object, CDP exposure, and the infobar token. We call
/// `.disable_default_args()` and re-add the 23 safe defaults below — every arg
/// from `chromiumoxide::browser::config::DEFAULT_ARGS` EXCEPT `enable-automation`.
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
    chrome_candidate_paths, classify_chrome_channel, detect_chrome, detect_chrome_major_version,
    detect_chrome_resolved, needs_no_sandbox, resolve_chrome_candidate, ChromeChannel,
    ResolvedChrome,
};
pub use extract::{
    extract_html_with_chrome, extract_news_html_with_chrome, extract_text_with_chrome,
    wait_for_selector_with_chrome,
};
pub use session::{flags_stealth, set_chrome_display_cli, ChromeBrowser, ChromeDisplayCli};

#[cfg(test)]
mod tests {
    use super::*;
    use super::detect::is_executable_chrome_binary;
    use super::extract::clean_text;
    use super::session::{
        chrome_display_cli, decide_head_mode, set_chrome_display_cli, ChromeDisplayCli,
        ChromeHeadMode,
    };
    use super::xvfb::has_native_display;
    use std::path::Path;
    use std::sync::{Mutex, MutexGuard};

    /// Serializes tests that mutate process-global state (display policy / OS env).
    fn env_lock() -> MutexGuard<'static, ()> {
        // Direct const constructor (MSRV ≥ 1.63) — no LazyLock/OnceLock wrapper.
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn chrome_candidate_paths_not_empty() {
        let paths = chrome_candidate_paths();
        assert!(!paths.is_empty(), "must return at least one candidate");
    }

    #[test]
    fn detect_chrome_manual_path_nonexistent_fails() {
        let p = Path::new("/tmp/caminho/absolutamente/inexistente/chrome-xyz");
        assert!(
            detect_chrome(Some(p)).is_err(),
            "path manual invalid deve failurer"
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
            // True when process sandbox, container markers, or path-based sandbox.
            let expected = crate::platform::is_flatpak_sandbox()
                || crate::platform::is_snap_sandbox()
                || crate::platform::is_container();
            assert_eq!(needs_no_sandbox(p), expected);
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn chrome_candidate_paths_include_host_and_sandbox_prefixes() {
        let paths = chrome_candidate_paths();
        let as_str: Vec<_> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert!(
            as_str.iter().any(|s| s.contains("google-chrome")),
            "expected host google-chrome candidates: {as_str:?}"
        );
        assert!(
            as_str.iter().any(|s| s.contains("flatpak")),
            "expected flatpak candidates: {as_str:?}"
        );
        assert!(
            as_str.iter().any(|s| s.contains("/snap/")),
            "expected snap candidates: {as_str:?}"
        );
        assert!(
            as_str.iter().any(|s| s.contains("google-chrome-beta")),
            "expected beta channel candidate: {as_str:?}"
        );
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
    fn chrome_display_cli_defaults_false() {
        let _guard = env_lock();
        set_chrome_display_cli(ChromeDisplayCli::default());
        let d = chrome_display_cli();
        assert!(!d.force_visible);
        assert!(!d.force_headless);
        assert!(!d.force_xvfb);
    }

    #[test]
    fn chrome_display_cli_force_xvfb_via_policy() {
        let _guard = env_lock();
        set_chrome_display_cli(ChromeDisplayCli {
            force_visible: false,
            force_headless: false,
            force_xvfb: true,
        });
        assert!(chrome_display_cli().force_xvfb);
        // Reset for other tests.
        set_chrome_display_cli(ChromeDisplayCli::default());
    }

    #[test]
    fn headed_requires_explicit_cli_opt_in() {
        let _guard = env_lock();
        set_chrome_display_cli(ChromeDisplayCli::default());
        let d = chrome_display_cli();
        assert!(
            !d.force_visible && !d.force_xvfb,
            "headed modes require --chrome-visible or --chrome-xvfb"
        );
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

    // GAP-WS-107: decide_head_mode is the pure head-mode decision extracted from launch().

    #[test]
    fn decide_head_mode_force_headless_overrides_all() {
        assert_eq!(
            decide_head_mode(true, false, false, true, true),
            ChromeHeadMode::Headless,
            "force_headless (--chrome-headless) must win over all other inputs"
        );
        assert_eq!(
            decide_head_mode(true, true, true, true, true),
            ChromeHeadMode::Headless,
            "force_headless must win even with force_visible + xvfb"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn decide_head_mode_macos_native_display_uses_headless_new() {
        // GAP-WS-112: macOS Quartz clamps --window-position, so native headed
        // would open a visible window. Default is Headless (headless=new).
        assert_eq!(
            decide_head_mode(false, false, false, true, false),
            ChromeHeadMode::Headless,
            "macOS with native display must use Headless (headless=new)"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn decide_head_mode_macos_force_visible_still_allows_headed_native() {
        // Escape hatch: --chrome-visible forces HeadedNative on macOS for visual debug.
        assert_eq!(
            decide_head_mode(false, true, false, true, false),
            ChromeHeadMode::HeadedNative,
            "--chrome-visible on macOS must force HeadedNative for debug"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn decide_head_mode_windows_native_display_uses_headless_new() {
        // GAP-WS-112: Windows DWM clamps --window-position; default is Headless.
        assert_eq!(
            decide_head_mode(false, false, false, true, false),
            ChromeHeadMode::Headless,
            "Windows with native display must use Headless (headless=new)"
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
            "Linux with display nativo mas SEM Xvfb cai em Headless (without regressão)"
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
            "--enable-automation seta navigator.webdriver legacy e é detectable; must not estar nos safe defaults"
        );
        // Sanity: a constante NOT está vazia (regressão de exclusão acidental).
        assert!(
            !CHROMIUMOXIDE_SAFE_DEFAULTS.is_empty(),
            "safe defaults must notm ficar empty after remover enable-automation"
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
            "deve restringir ICE a UDP non-proxied"
        );
        assert!(
            f.iter().any(|x| x == "--disable-webrtc-hw-decoding"),
            "deve desabilitar decodificação WebRTC por hardware"
        );
        // GAP-WS-111: QUIC UDP stack differs from the Chrome TLS path for the spoofed UA.
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
            "--disable-extensions é sinal de automação detectable; must not voltar à lista stealth"
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

        // Chromium variant also deve ser parseada.
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

    /// GAP-PROC-002: non-zero exit must not treat stdout as a valid version.
    #[cfg(unix)]
    #[test]
    fn detect_chrome_major_version_rejects_nonzero_exit() {
        let tmp = tempfile::NamedTempFile::new().expect("temp file");
        let shim = tmp.path().with_extension("sh");
        std::fs::write(
            &shim,
            "#!/bin/sh\necho \"Google Chrome 146.0.0.0\"\nexit 1\n",
        )
        .expect("write shim");
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&shim).expect("metadata").permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&shim, perm).expect("chmod");
        }
        assert_eq!(
            detect_chrome_major_version(&shim),
            None,
            "non-zero exit must ignore version stdout"
        );
    }

    /// GAP-PROC-006: `.bat`/`.cmd`/`.ps1` wrappers are never accepted as Chrome.
    #[test]
    fn resolve_chrome_rejects_batch_and_shell_wrappers() {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in ["chrome.bat", "chrome.cmd", "chrome.ps1", "chrome.sh"] {
            let p = dir.path().join(name);
            std::fs::write(&p, b"@echo off\r\n").expect("write");
            assert!(
                resolve_chrome_candidate(&p).is_none(),
                "{name} must be rejected (BatBadBut / shell wrapper policy)"
            );
        }
    }
}
