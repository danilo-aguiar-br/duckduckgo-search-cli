// SPDX-License-Identifier: MIT OR Apache-2.0
//! GAP-WS-LIFECYCLE-001 — integration tests for one-shot Chromium/Xvfb cleanup.
//!
//! Heavy tests require Chrome and are gated by `DUCKDUCKGO_LIFECYCLE_E2E=1`
//! (rules-rust-testes-sem-travar: do not hang default CI).
//!
//! Run:
//! ```text
//! DUCKDUCKGO_LIFECYCLE_E2E=1 DUCKDUCKGO_CHROME_HEADLESS=1 \
//!   cargo test --features chrome --test integration_browser_lifecycle -- --nocapture
//! ```

#![cfg(feature = "chrome")]

use duckduckgo_search_cli::browser::{detect_chrome, ChromeBrowser};
use std::path::Path;
use std::time::Duration;

fn e2e_enabled() -> bool {
    std::env::var_os("DUCKDUCKGO_LIFECYCLE_E2E").is_some()
}

#[cfg(target_os = "linux")]
fn count_procs_with_marker(marker: &str) -> usize {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return 0;
    };
    let mut n = 0usize;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.parse::<u32>().is_err() {
            continue;
        }
        let cmdline = format!("/proc/{name}/cmdline");
        if let Ok(bytes) = std::fs::read(cmdline) {
            if bytes.windows(marker.len()).any(|w| w == marker.as_bytes()) {
                n += 1;
            }
        }
    }
    n
}

#[tokio::test(flavor = "multi_thread")]
async fn drop_without_shutdown_reaps_session() {
    if !e2e_enabled() {
        eprintln!("skip: set DUCKDUCKGO_LIFECYCLE_E2E=1 to run");
        return;
    }
    let Ok(path) = detect_chrome(None) else {
        eprintln!("skip: Chrome not installed");
        return;
    };
    std::env::set_var("DUCKDUCKGO_CHROME_HEADLESS", "1");

    let browser = ChromeBrowser::launch(
        &path,
        None,
        Duration::from_secs(30),
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/146.0.0.0 Safari/537.36",
    )
    .await
    .expect("launch chrome");

    let marker = browser.user_data_dir().to_string_lossy().into_owned();
    assert!(
        marker.contains("ddg-chrome-"),
        "GAP-WS-TMP-PROFILE-ORPHAN-001: user-data-dir must use ddg-chrome- prefix, got {marker}"
    );
    drop(browser);
    tokio::time::sleep(Duration::from_millis(500)).await;

    #[cfg(target_os = "linux")]
    {
        let left = count_procs_with_marker(&marker);
        assert_eq!(
            left, 0,
            "expected 0 processes with user-data-dir {marker}, found {left}"
        );
        assert!(
            !Path::new(&marker).exists(),
            "user-data-dir should be removed: {marker}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_removes_profile_and_processes() {
    if !e2e_enabled() {
        eprintln!("skip: set DUCKDUCKGO_LIFECYCLE_E2E=1 to run");
        return;
    }
    let Ok(path) = detect_chrome(None) else {
        eprintln!("skip: Chrome not installed");
        return;
    };
    std::env::set_var("DUCKDUCKGO_CHROME_HEADLESS", "1");

    let browser = ChromeBrowser::launch(
        &path,
        None,
        Duration::from_secs(30),
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/146.0.0.0 Safari/537.36",
    )
    .await
    .expect("launch");

    let marker = browser.user_data_dir().to_string_lossy().into_owned();
    assert!(
        marker.contains("ddg-chrome-"),
        "GAP-WS-TMP-PROFILE-ORPHAN-001: user-data-dir must use ddg-chrome- prefix, got {marker}"
    );
    browser.shutdown().await.expect("shutdown");
    tokio::time::sleep(Duration::from_millis(400)).await;

    #[cfg(target_os = "linux")]
    {
        assert_eq!(
            count_procs_with_marker(&marker),
            0,
            "orphans with marker {marker}"
        );
        assert!(
            !Path::new(&marker).exists(),
            "profile dir must be gone: {marker}"
        );
    }
}

/// Unit-level (no Chrome): registry `force_reap` removes a real ddg-chrome dir.
#[test]
fn force_reap_all_clears_registered_profile_dir() {
    use duckduckgo_search_cli::process_lifecycle::{
        force_reap, register_session, unregister_session, SessionIds, USER_DATA_DIR_PREFIX,
    };
    let dir = std::env::temp_dir().join(format!(
        "{USER_DATA_DIR_PREFIX}e2e-reg-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("Preferences"), b"{}").expect("write");
    register_session(SessionIds {
        chrome_pid: None,
        xvfb_pid: None,
        xvfb_pgid: None,
        user_data_dir: dir.clone(),
        display: None,
    });
    force_reap(&SessionIds {
        chrome_pid: None,
        xvfb_pid: None,
        xvfb_pgid: None,
        user_data_dir: dir.clone(),
        display: None,
    });
    unregister_session(&dir);
    assert!(!dir.exists(), "registered profile must be removed: {dir:?}");
}
