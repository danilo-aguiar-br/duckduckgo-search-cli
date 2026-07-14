// SPDX-License-Identifier: MIT OR Apache-2.0
//! GAP-WS-AGENT-READY-001 — optional Flatpak Chrome resolve + launch smoke.
//!
//! Gated by `DUCKDUCKGO_FLATPAK_E2E=1` so default CI does not hang.
//!
//! Run:
//! ```text
//! DUCKDUCKGO_FLATPAK_E2E=1 DUCKDUCKGO_CHROME_HEADLESS=1 \
//!   cargo test --features chrome --test integration_flatpak_chrome -- --nocapture
//! ```

#![cfg(feature = "chrome")]
#![cfg(target_os = "linux")]

use duckduckgo_search_cli::browser::{
    detect_chrome_resolved, resolve_chrome_candidate, ChromeBrowser, ChromeChannel,
};
use std::path::Path;
use std::time::Duration;

fn e2e_enabled() -> bool {
    std::env::var_os("DUCKDUCKGO_FLATPAK_E2E").is_some()
}

#[test]
fn resolve_flatpak_export_to_deploy_elf_when_installed() {
    if !e2e_enabled() {
        eprintln!("skip: set DUCKDUCKGO_FLATPAK_E2E=1");
        return;
    }
    let export = Path::new("/var/lib/flatpak/exports/bin/com.google.Chrome");
    if !export.is_file() {
        eprintln!("skip: Flatpak com.google.Chrome export not installed");
        return;
    }
    let resolved = resolve_chrome_candidate(export).expect("export must resolve to deploy ELF");
    assert!(
        resolved.to_string_lossy().contains("/flatpak/app/"),
        "expected deploy path, got {}",
        resolved.display()
    );
    assert!(resolved.is_file());
}

#[tokio::test]
async fn launch_flatpak_deploy_elf_when_installed() {
    if !e2e_enabled() {
        eprintln!("skip: set DUCKDUCKGO_FLATPAK_E2E=1");
        return;
    }
    let export = Path::new("/var/lib/flatpak/exports/bin/com.google.Chrome");
    if !export.is_file() {
        eprintln!("skip: Flatpak Chrome not installed");
        return;
    }
    let info = detect_chrome_resolved(Some(export)).expect("detect resolve");
    assert_eq!(info.channel, ChromeChannel::Manual);
    let browser = ChromeBrowser::launch(
        &info.path,
        None,
        Duration::from_secs(45),
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    )
    .await
    .expect("launch Flatpak deploy ELF");
    browser.shutdown().await.ok();
}
