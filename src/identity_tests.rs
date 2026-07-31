// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests for the identity pool (SRP split from identity.rs).

use super::*;

#[test]
fn pool_has_twelve_identities() {
    let pool = IdentityPool::new(Some(42));
    // 12 entries are loaded from the catalog; we just check that the
    // cursor returns one of them.
    let _ = pool.current();
}

#[test]
fn rotation_advances_cascade_level() {
    let mut pool = IdentityPool::new(Some(42));
    assert_eq!(pool.level(), 0);
    let first = pool.active_tag();
    pool.rotate_on_block();
    assert_eq!(pool.level(), 1);
    // After 1st rotation, identity changed.
    assert_ne!(pool.active_tag(), first);
}

#[test]
fn deterministic_seed_produces_same_sequence() {
    let mut a = IdentityPool::new(Some(99));
    let mut b = IdentityPool::new(Some(99));
    for _ in 0..3 {
        let ta = a.active_tag();
        let tb = b.active_tag();
        assert_eq!(ta, tb, "deterministic seed must produce same tag");
        a.rotate_on_block();
        b.rotate_on_block();
    }
}

#[test]
fn shuffled_headers_include_family_specific_values() {
    let pool = IdentityPool::new(Some(7));
    let headers = pool.current().shuffled_headers("pt", "br");
    let names: Vec<&str> = headers.iter().map(|(n, _)| *n).collect();
    assert!(names.contains(&"accept"));
    assert!(names.contains(&"accept-language"));
    assert!(names.contains(&"sec-fetch-dest"));
    // Chrome/Edge identities emit Sec-CH-UA.
    if matches!(
        pool.current().family,
        BrowserFamily::Chrome | BrowserFamily::Edge
    ) {
        assert!(names.contains(&"sec-ch-ua"));
        assert!(names.contains(&"sec-ch-ua-platform"));
    }
}

#[test]
fn tag_format_is_stable() {
    let pool = IdentityPool::new(Some(1));
    let tag = pool.active_tag();
    // Format: <family>-<platform>-<16hex>
    let parts: Vec<&str> = tag.split('-').collect();
    assert_eq!(parts.len(), 3, "tag must have 3 parts: {tag}");
    assert_eq!(parts[2].len(), 16, "seed part must be 16 hex chars: {tag}");
}

// v0.7.10 GAP-WS-60: tests for the new pin + lookup API and the
// `browser_profile_for_cli_identity` helper.

#[test]
fn pin_to_clamps_out_of_bounds_index() {
    let mut pool = IdentityPool::new(Some(42));
    let before = pool.active_tag();
    pool.pin_to(9999);
    // Out-of-bounds pin is a no-op; the active identity is unchanged.
    assert_eq!(pool.active_tag(), before);
}

#[test]
fn pin_to_within_bounds_changes_active_identity() {
    let mut pool = IdentityPool::new(Some(42));
    let before = pool.active_tag();
    // Pin to a different index; the active tag MUST change.
    let new_index = if 0 == pool.current_index() { 1 } else { 0 };
    pool.pin_to(new_index);
    assert_ne!(pool.active_tag(), before, "pin must rotate active identity");
}

#[test]
fn find_index_returns_correct_identity() {
    let pool = IdentityPool::new(Some(7));
    let chrome_linux = pool
        .find_index(BrowserFamily::Chrome, Platform::Linux)
        .expect("Chrome+Linux must exist in catalog");
    let identity = pool
        .get(chrome_linux)
        .expect("index must resolve to an identity");
    assert_eq!(identity.family, BrowserFamily::Chrome);
    assert_eq!(identity.platform, Platform::Linux);
    assert!(
        identity.user_agent.contains("X11; Linux"),
        "Chrome+Linux UA must declare Linux platform: {}",
        identity.user_agent
    );
}

#[test]
fn browser_profile_for_cli_identity_auto_returns_none() {
    let profile = browser_profile_for_cli_identity(CliIdentityProfile::Auto, Some(42));
    assert!(
        profile.is_none(),
        "Auto must signal the caller to use the default profile"
    );
}

#[test]
fn browser_profile_for_cli_identity_chrome_linux_returns_linux_ua() {
    let profile = browser_profile_for_cli_identity(CliIdentityProfile::ChromeLinux, Some(42))
        .expect("ChromeLinux must resolve to a BrowserProfile");
    assert!(
        profile.user_agent.contains("X11; Linux"),
        "pinned UA must declare Linux platform: {}",
        profile.user_agent
    );
    assert_eq!(profile.family, crate::http::BrowserFamily::Chrome);
    assert_eq!(profile.ua_platform, "Linux");
}

#[test]
fn browser_profile_for_cli_identity_safari_mac_returns_mac_ua() {
    let profile = browser_profile_for_cli_identity(CliIdentityProfile::SafariMac, Some(42))
        .expect("SafariMac must resolve to a BrowserProfile");
    assert!(
        profile.user_agent.contains("Macintosh"),
        "pinned UA must declare Macintosh platform: {}",
        profile.user_agent
    );
    assert_eq!(profile.family, crate::http::BrowserFamily::Safari);
    assert_eq!(profile.ua_platform, "macOS");
}

// GAP-WS-107b v0.9.1: Chrome UA platform coercion.

#[test]
fn ua_platform_matches_host_true_for_host_chrome_ua() {
    let ua = chrome_only_ua_for_platform();
    assert!(
        ua_platform_matches_host(&ua),
        "chrome_only_ua_for_platform() deve afirmar o SO do host: {ua}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn ua_platform_matches_host_rejects_cross_platform_ua_macos() {
    let linux_ua = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
    assert!(
        !ua_platform_matches_host(linux_ua),
        "UA Linux num host macOS deve ser rejeitado (mismatch)"
    );
    let win_ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
    assert!(
        !ua_platform_matches_host(win_ua),
        "UA Windows num host macOS deve ser rejeitado (mismatch)"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn ua_platform_matches_host_rejects_cross_platform_ua_linux() {
    let mac_ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
    assert!(
        !ua_platform_matches_host(mac_ua),
        "UA macOS num host Linux deve ser rejeitado (mismatch)"
    );
}

// GAP-E2E-V14-UA-METADATA-LIE: SSOT resolve coerces non-Chrome + wrong OS.
#[test]
fn resolve_effective_chrome_identity_rejects_firefox_safari() {
    let ff = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";
    let id = resolve_effective_chrome_identity(ff, Some(150));
    assert!(
        id.user_agent.contains("Chrome/150"),
        "must coerce Firefox → Chrome host major: {}",
        id.user_agent
    );
    assert!(
        !id.user_agent.contains("Firefox/"),
        "Firefox must not remain: {}",
        id.user_agent
    );
    assert!(id.tag.starts_with("chrome-"));

    let safari =
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
    let id2 = resolve_effective_chrome_identity(safari, Some(150));
    assert!(id2.user_agent.contains("Chrome/"));
    assert!(!id2.user_agent.contains("Version/17.0 Safari/605"));
    // pure Safari token without Chrome/ must not survive
    assert!(
        id2.user_agent.contains("Chrome/150"),
        "Safari must become Chrome: {}",
        id2.user_agent
    );
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_effective_chrome_identity_forces_linux_host() {
    let mac = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
    let id = resolve_effective_chrome_identity(mac, Some(150));
    assert!(
        id.user_agent.contains("Linux") || id.user_agent.contains("X11"),
        "Mac Chrome on Linux host must coerce platform: {}",
        id.user_agent
    );
    assert!(id.user_agent.contains("Chrome/150"));
    assert!(id.tag.contains("linux"));
}

// GAP-WS-109 v0.9.2: rewrite do major version do UA preserva a plataforma.
#[test]
fn rewrite_ua_chrome_version_swaps_major() {
    let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
    let rewritten = rewrite_ua_chrome_version(ua, 149);
    assert!(
        rewritten.contains("Chrome/149"),
        "major deve ser trocado para 149: {rewritten}"
    );
    assert!(
        rewritten.contains("Macintosh"),
        "plataforma Macintosh deve ser preservada"
    );
    assert!(
        !rewritten.contains("Chrome/146"),
        "old major 146 must not remain"
    );
}

#[test]
fn rewrite_ua_preserves_linux_platform() {
    let ua = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
    let rewritten = rewrite_ua_chrome_version(ua, 130);
    assert!(rewritten.contains("Chrome/130"));
    assert!(rewritten.contains("X11; Linux x86_64"));
}

#[test]
fn rewrite_ua_unchanged_when_no_chrome_token() {
    let ua = "Mozilla/5.0 (Macintosh) Safari/605";
    assert_eq!(rewrite_ua_chrome_version(ua, 149), ua);
}

// GAP-WS-109 v0.9.2: major rewrite must NOT alter the substring of
// plataforma do host (cfg-gated). Os testes acima cobrem Mac e Linux; este
// completa a matriz com Windows.
#[cfg(target_os = "windows")]
#[test]
fn rewrite_ua_preserves_platform_per_cfg() {
    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
    let rewritten = rewrite_ua_chrome_version(ua, 200);
    assert!(
        rewritten.contains("Chrome/200"),
        "major deve ser trocado para 200: {rewritten}"
    );
    assert!(
        rewritten.contains("Windows NT 10.0; Win64; x64"),
        "substring de plataforma Windows deve ser preservada: {rewritten}"
    );
    assert!(
        !rewritten.contains("Chrome/146"),
        "old major 146 must not remain"
    );
}
