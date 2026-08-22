// SPDX-License-Identifier: MIT OR Apache-2.0
//! Browser module unit tests (SRP split from browser/mod.rs).

use super::*;
// Only the Linux Flatpak resolution test below uses this.
#[cfg(target_os = "linux")]
use super::detect::is_executable_chrome_binary;
use super::extract::clean_text;
use super::session::{
    chrome_display_cli, decide_head_mode, set_chrome_display_cli, ChromeDisplayCli, ChromeHeadMode,
};
use super::xvfb::has_native_display;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// Serializes tests that mutate process-global state (display policy / OS env).
fn env_lock() -> MutexGuard<'static, ()> {
    // Direct const constructor (MSRV ≥ 1.63) — no LazyLock/OnceLock wrapper.
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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
        "invalid manual path must fail"
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
    let raw = "ok\nanother line long enough to clear the twenty character floor\nshort\n";
    let clean = clean_text(raw, 1000);
    assert!(clean.contains("another line"));
    assert!(!clean.contains("ok\n"));
}

#[test]
fn clean_text_truncates_at_word() {
    let raw = "linha um com mais de vinte caracteres definitivamente aqui presentes\n".repeat(10);
    let clean = clean_text(&raw, 50);
    assert!(clean.chars().count() <= 50);
}

#[test]
fn needs_no_sandbox_flatpak_path() {
    let p = Path::new("/var/lib/flatpak/exports/bin/com.google.Chrome");
    #[cfg(target_os = "linux")]
    assert!(needs_no_sandbox(p));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = p;
    }
}

#[test]
fn needs_no_sandbox_snap_path() {
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
    let as_str: Vec<_> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
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
    let p = Path::new("/var/lib/flatpak/app/com.google.Chrome/current/active/files/extra/chrome");
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
        "Linux with a native display + Xvfb must use HeadedXvfb"
    );
    assert_eq!(
        decide_head_mode(false, false, false, true, false),
        ChromeHeadMode::Headless,
        "Linux with a native display but WITHOUT Xvfb falls back to Headless (no regression)"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn decide_head_mode_linux_no_display_uses_xvfb() {
    assert_eq!(
        decide_head_mode(false, false, false, false, true),
        ChromeHeadMode::HeadedXvfb,
        "Linux without a native display + Xvfb must use HeadedXvfb"
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
        "--enable-automation sets the legacy navigator.webdriver and is detectable; it must not be in the safe defaults"
    );
    // Sanity: the constant is NOT empty (guards accidental deletion).
    assert!(
        !CHROMIUMOXIDE_SAFE_DEFAULTS.is_empty(),
        "safe defaults must notm ficar empty after remover enable-automation"
    );
}

#[test]
fn flags_stealth_mutes_audio_by_default() {
    // ADR-0026 / GAP-CHROME-MUTE-001 — matrix: sandbox on/off + proxy.
    for (sandbox_off, proxy) in [
        (false, None),
        (true, None),
        (false, Some("http://127.0.0.1:8080")),
        (true, Some("socks5://127.0.0.1:1080")),
    ] {
        let f = flags_stealth(sandbox_off, proxy, "Mozilla/5.0 Chrome/146.0.0.0");
        assert!(
            f.iter().any(|x| x == CHROME_MUTE_AUDIO_FLAG),
            "CLI must mute Chrome audio (sandbox_off={sandbox_off}, proxy={proxy:?})"
        );
        assert!(
            f.iter().any(|x| x == CHROME_AUTOPLAY_POLICY_FLAG),
            "CLI must set autoplay policy (sandbox_off={sandbox_off}, proxy={proxy:?})"
        );
        super::session::ensure_chrome_audio_muted(f.iter().map(String::as_str))
            .expect("flags_stealth must satisfy mute operational standard");
    }
}

#[test]
fn safe_defaults_include_mute_audio_operational_standard() {
    assert!(
        CHROMIUMOXIDE_SAFE_DEFAULTS.contains(&CHROME_MUTE_AUDIO_FLAG),
        "CHROMIUMOXIDE_SAFE_DEFAULTS must include --mute-audio (ADR-0026 belt)"
    );
    assert!(
        CHROMIUMOXIDE_SAFE_DEFAULTS.contains(&CHROME_AUTOPLAY_POLICY_FLAG),
        "CHROMIUMOXIDE_SAFE_DEFAULTS must include autoplay policy (ADR-0026 belt)"
    );
    super::session::ensure_chrome_audio_muted(CHROMIUMOXIDE_SAFE_DEFAULTS.iter().copied())
        .expect("safe defaults alone must satisfy mute operational standard");
}

#[test]
fn ensure_chrome_audio_muted_rejects_loud_launch_args() {
    let err = super::session::ensure_chrome_audio_muted(["--no-first-run", "--disable-sync"])
        .expect_err("missing mute must fail closed");
    let msg = err.to_string();
    assert!(
        msg.contains("mute-audio") || msg.contains("ADR-0026"),
        "error must name the operational standard: {msg}"
    );
}

/// GAP-REL-003 — a repeated valued switch must fail closed.
///
/// Chromium keeps one value per switch name, so a second `--disable-features=`
/// silently discards the first. That is how `AutomationControlled` stopped
/// reaching `FeatureList` while the flag list still read correctly in review.
#[test]
fn duplicate_valued_switch_is_rejected() {
    let err = super::session::ensure_no_duplicate_valued_switch([
        "--disable-features=AutomationControlled,TranslateUI",
        "--window-size=1920,1080",
        "--disable-features=WebRtcHideLocalIpsWithMdns",
    ])
    .expect_err("a repeated --disable-features must fail closed");
    let msg = err.to_string();
    assert!(
        msg.contains("disable-features"),
        "error must name the collapsed switch: {msg}"
    );
}

/// Valueless switches repeat harmlessly and must NOT trip the guard.
#[test]
fn duplicate_valueless_switch_is_allowed() {
    super::session::ensure_no_duplicate_valued_switch([
        "--no-first-run",
        "--no-first-run",
        "--disable-sync",
    ])
    .expect("valueless switches are idempotent, so repeating them is not a defect");
}

/// A valued switch repeated with the SAME value is redundant, not a defect.
///
/// Whichever copy Chromium keeps carries the same meaning, so failing here
/// would be a false positive — and a guard that cries wolf gets switched off.
/// `--mute-audio` and `--autoplay-policy` are deliberately present in both
/// `CHROMIUMOXIDE_SAFE_DEFAULTS` and `flags_stealth` as a belt-and-braces for
/// ADR-0026, and that duplication must keep working.
#[test]
fn duplicate_valued_switch_with_identical_value_is_allowed() {
    super::session::ensure_no_duplicate_valued_switch([
        "--autoplay-policy=user-gesture-required",
        "--window-size=1920,1080",
        "--autoplay-policy=user-gesture-required",
    ])
    .expect("same name with the same value is redundant, never behaviour-changing");
}

/// The real launch argv — safe defaults PLUS stealth — must satisfy the guard.
///
/// This is the combination that actually reaches Chrome, and it is where the
/// `--disable-features` collision lived: `TranslateUI` in the safe defaults
/// versus `AutomationControlled,TranslateUI` in stealth, with only
/// concatenation order deciding which one survived.
#[test]
fn launch_argv_has_no_conflicting_valued_switch() {
    for sandbox_off in [false, true] {
        for proxy in [None, Some("http://proxy:8080")] {
            let stealth = flags_stealth(sandbox_off, proxy, "Mozilla/5.0 Chrome/146.0.0.0");
            let launch: Vec<&str> = CHROMIUMOXIDE_SAFE_DEFAULTS
                .iter()
                .copied()
                .chain(stealth.iter().map(String::as_str))
                .collect();
            super::session::ensure_no_duplicate_valued_switch(launch.iter().copied())
                .unwrap_or_else(|e| {
                    panic!("real launch argv (sandbox_off={sandbox_off}, proxy={proxy:?}) conflicts: {e}")
                });
        }
    }
}

/// The real stealth argv must satisfy the guard on every launch shape.
///
/// This is the regression that matters: the guard exists to protect
/// `flags_stealth`, so the production list itself has to pass it.
#[test]
fn flags_stealth_never_repeats_a_valued_switch() {
    for sandbox_off in [false, true] {
        for proxy in [None, Some("http://proxy:8080")] {
            let f = flags_stealth(sandbox_off, proxy, "Mozilla/5.0 Chrome/146.0.0.0");
            super::session::ensure_no_duplicate_valued_switch(f.iter().map(String::as_str))
                .unwrap_or_else(|e| {
                    panic!("flags_stealth(sandbox_off={sandbox_off}, proxy={proxy:?}) repeats a valued switch: {e}")
                });
        }
    }
}

/// GAP-REL-004 — the WebRTC mitigation must not re-expose local IPs.
///
/// `WebRtcHideLocalIpsWithMdns` is the feature that HIDES local addresses
/// behind `.local` mDNS names in ICE candidates. Disabling it does the
/// opposite of the stated intent, so it must never come back.
#[test]
fn stealth_flags_do_not_disable_mdns_ip_hiding() {
    let f = flags_stealth(false, None, "Mozilla/5.0 Chrome/146.0.0.0");
    assert!(
        !f.iter().any(|a| a.contains("WebRtcHideLocalIpsWithMdns")),
        "disabling WebRtcHideLocalIpsWithMdns re-exposes the real local IP in \
         ICE candidates, which is the leak the flag list claims to prevent: {f:?}"
    );
    assert!(
        f.iter()
            .any(|a| a == "--force-webrtc-ip-handling-policy=disable_non_proxied_udp"),
        "the actual candidate-gathering restriction must still be present: {f:?}"
    );
}

/// GAP-REL-003 — `AutomationControlled` must survive into the rendered argv.
#[test]
fn stealth_flags_keep_automation_controlled_in_disable_features() {
    let f = flags_stealth(false, None, "Mozilla/5.0 Chrome/146.0.0.0");
    let disable_features: Vec<&String> = f
        .iter()
        .filter(|a| a.starts_with("--disable-features="))
        .collect();
    assert_eq!(
        disable_features.len(),
        1,
        "exactly one --disable-features may exist, or Chromium drops all but the last: {f:?}"
    );
    assert!(
        disable_features[0].contains("AutomationControlled"),
        "the surviving --disable-features must still carry AutomationControlled: {disable_features:?}"
    );
}

/// G5 — Chrome argv must never embed proxy userinfo (`user:pass@`).
#[test]
fn chrome_proxy_server_arg_strips_userinfo() {
    use super::session::{chrome_proxy_server_arg, flags_stealth};

    assert_eq!(
        chrome_proxy_server_arg("http://user:s3cret@proxy.example:8080"),
        "http://proxy.example:8080"
    );
    assert_eq!(
        chrome_proxy_server_arg("socks5://u:p@127.0.0.1:9050"),
        "socks5://127.0.0.1:9050"
    );
    assert_eq!(
        chrome_proxy_server_arg("http://127.0.0.1:9"),
        "http://127.0.0.1:9"
    );
    let flags = flags_stealth(
        false,
        Some("http://alice:bob@proxy.local:3128"),
        "Mozilla/5.0 Chrome/150.0.0.0",
    );
    let proxy_flag = flags
        .iter()
        .find(|f| f.starts_with("--proxy-server="))
        .expect("proxy flag");
    assert!(
        !proxy_flag.contains("alice") && !proxy_flag.contains("bob"),
        "credentials must not appear in Chrome argv: {proxy_flag}"
    );
    assert!(proxy_flag.contains("proxy.local:3128"));
}

/// GAP-CHROME-MUTE-002 — chromiumoxide ArgsBuilder prepends `--` to the key.
/// Passing `--mute-audio` yields `----mute-audio`, which Chromium ignores.
#[test]
fn chromiumoxide_arg_token_prevents_quad_dash_mute() {
    use super::session::{chromiumoxide_arg_token, chromiumoxide_rendered_arg};

    assert_eq!(chromiumoxide_arg_token("--mute-audio"), "mute-audio");
    assert_eq!(chromiumoxide_arg_token("mute-audio"), "mute-audio");
    assert_eq!(
        chromiumoxide_arg_token("--autoplay-policy=document-user-activation-required"),
        "autoplay-policy=document-user-activation-required"
    );

    // Without strip: double-prefix regression class (4 dashes).
    let buggy = format!("--{CHROME_MUTE_AUDIO_FLAG}");
    assert_eq!(
        buggy, "----mute-audio",
        "documents the chromiumoxide double-prefix bug class"
    );

    // With strip: exact Chromium switch.
    assert_eq!(
        chromiumoxide_rendered_arg(CHROME_MUTE_AUDIO_FLAG),
        "--mute-audio"
    );
    assert_eq!(
        chromiumoxide_rendered_arg(CHROME_AUTOPLAY_POLICY_FLAG),
        CHROME_AUTOPLAY_POLICY_FLAG
    );
}

#[test]
fn launch_arg_sources_render_valid_mute_argv() {
    use super::session::{
        ensure_chrome_audio_muted, ensure_chrome_audio_muted_rendered, flags_stealth,
    };

    for (sandbox_off, proxy) in [
        (false, None),
        (true, None),
        (false, Some("http://127.0.0.1:8080")),
    ] {
        let flags = flags_stealth(sandbox_off, proxy, "Mozilla/5.0 Chrome/150.0.0.0");
        let sources: Vec<&str> = CHROMIUMOXIDE_SAFE_DEFAULTS
            .iter()
            .copied()
            .chain(flags.iter().map(String::as_str))
            .collect();
        ensure_chrome_audio_muted(sources.iter().copied()).expect("source list must include mute");
        ensure_chrome_audio_muted_rendered(sources.iter().copied())
            .expect("rendered argv must be --mute-audio not ----mute-audio");
    }
}

#[test]
fn flags_stealth_disables_webrtc_and_quic() {
    let f = flags_stealth(false, None, "Mozilla/5.0 Chrome/146.0.0.0");
    // GAP-WS-110: WebRTC leaked the real IP even behind a proxy.
    //
    // GAP-REL-004: this used to assert the PRESENCE of
    // `--disable-features=WebRtcHideLocalIpsWithMdns` under the label "must
    // suppress WebRTC mDNS". That label inverts the mechanism. mDNS hiding is
    // the protection: Chrome replaces local addresses with `.local` names in
    // ICE candidates. Suppressing it re-exposes the real IP, so the assertion
    // pinned the leak as a requirement. The mitigation lives in the two
    // switches asserted below, which actually restrict candidate gathering.
    assert!(
        !f.iter().any(|x| x.contains("WebRtcHideLocalIpsWithMdns")),
        "must NOT disable mDNS IP hiding — that re-exposes the real local IP"
    );
    assert!(
        f.iter()
            .any(|x| x == "--enforce-webrtc-ip-permission-check"),
        "must enforce the WebRTC IP permission check"
    );
    assert!(
        f.iter()
            .any(|x| x == "--force-webrtc-ip-handling-policy=disable_non_proxied_udp"),
        "must restrict ICE to non-proxied UDP"
    );
    assert!(
        f.iter().any(|x| x == "--disable-webrtc-hw-decoding"),
        "must disable WebRTC hardware decoding"
    );
    // GAP-WS-111: QUIC UDP stack differs from the Chrome TLS path for the spoofed UA.
    assert!(
        f.iter().any(|x| x == "--disable-quic"),
        "must disable QUIC (GQuic/HTTP3)"
    );
}

#[test]
fn flags_stealth_still_excludes_disable_extensions() {
    let f = flags_stealth(false, None, "Mozilla/5.0 Chrome/146.0.0.0");
    assert!(
        !f.iter().any(|x| x == "--disable-extensions"),
        "--disable-extensions is a detectable automation signal; it must not return to the stealth list"
    );
}

/// Exercises the `chrome --version` parser without depending on a real Chrome
/// install: creates an executable shim that prints "Google Chrome 146.0.0.0".
/// Process-wide path cache: clear between shim rewrites (GAP-PAR-042).
#[test]
fn detect_chrome_major_version_parses_output() {
    clear_chrome_version_cache();
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
        "parser must extract major 146 from \"Google Chrome 146.0.7561.0 beta\""
    );
    // GAP-PAR-042: second call hits process-wide cache (same path).
    assert_eq!(
        detect_chrome_major_version(&shim),
        Some(146),
        "cache hit must return same major without re-spawn"
    );

    // The Chromium variant must parse too.
    clear_chrome_version_cache();
    std::fs::write(&shim, "#!/bin/sh\necho \"Chromium 999.0.0.0\"\n").expect("rewrite shim");
    assert_eq!(
        detect_chrome_major_version(&shim),
        Some(999),
        "parser must extract major 999 from \"Chromium 999.0.0.0\""
    );

    // Output sem marcador conhecido -> None.
    clear_chrome_version_cache();
    std::fs::write(&shim, "#!/bin/sh\necho \"outro navegador 42\"\n").expect("rewrite shim");
    assert_eq!(
        detect_chrome_major_version(&shim),
        None,
        "output without \"Chrome \"/\"Chromium \" must return None"
    );

    // Path inexistente -> None (sem panic).
    clear_chrome_version_cache();
    assert_eq!(
        detect_chrome_major_version(std::path::Path::new("/does/not/exist/chrome-xyz")),
        None,
        "a nonexistent path must return None without panicking"
    );
}

/// GAP-PROC-002: non-zero exit must not treat stdout as a valid version.
#[cfg(unix)]
#[test]
fn detect_chrome_major_version_rejects_nonzero_exit() {
    clear_chrome_version_cache();
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
