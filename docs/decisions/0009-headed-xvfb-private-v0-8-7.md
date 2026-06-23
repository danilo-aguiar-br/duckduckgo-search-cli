# ADR-0009: Headed-inside-Xvfb with Auto-Install and Schema Parity (v0.8.7)


## Status
- Accepted (2026-06-23)
- Extends: ADR-0007 (Chrome Primary Transport, v0.8.0)
- Closes: GAP-WS-072 to GAP-WS-088 (17 gaps)


## Context
- v0.8.5 introduced Chrome headed via Xvfb but had 17 deficiencies (GAP-WS-072 to GAP-WS-088)
- Chrome window was visible to users on Linux Desktop (GNOME clamps off-screen windows)
- UA pool could select Safari/Firefox UA with Chromium TLS fingerprint (mismatch detected by Cloudflare)
- Chrome navigated directly to search URL without warm-up (Cloudflare interstitial on first visit)
- Xvfb was not auto-installed and error messages were invisible with -q flag
- deep-research schema had inconsistencies (.title vs .titulo, missing .query)


## Decision
- Chrome ALWAYS runs headed inside a PRIVATE Xvfb virtual display (user sees ZERO windows)
- `has_native_display()` detects native display per platform (Linux $DISPLAY/$WAYLAND_DISPLAY, macOS Quartz, Windows DWM)
- `spawn_virtual_display()` creates a private Xvfb even when native display exists (avoids visible window)
- `try_auto_install_xvfb()` auto-installs Xvfb on 22+ Linux distros via `detect_linux_distro()`
- `detect_linux_variant()` detects immutable distros (Silverblue, Kinoite, NixOS, Guix, ostree) and skips auto-install
- `chrome_only_ua_for_platform()` filters identity pool to accept ONLY Chrome UA when browser is Chromium (GAP-WS-074)
- Warm-up navigation to duckduckgo.com BEFORE search URL with random delay (GAP-WS-077)
- `navigator.webdriver` set to `undefined` (not `false`) matching real Chrome (GAP-WS-076)
- CDP WebSocket leak prevention and stack trace filtering added (GAP-WS-076)
- Anti-detection flags added: --disable-features=AutomationControlled,TranslateUI, --disable-infobars (GAP-WS-075)
- `AggregatedItem.title` serializes as "titulo" via serde rename for schema parity (GAP-WS-087)
- `DeepResearchOutput.query` field added for schema parity with SearchOutput (GAP-WS-088)
- Env vars: `DUCKDUCKGO_CHROME_HEADLESS=1` forces headless, `DUCKDUCKGO_CHROME_VISIBLE=1` forces visible headed


## Stealth Signals (17, enhanced in v0.8.7)
- `navigator.webdriver` set to `undefined` (was `false` in v0.8.0)
- Canvas fingerprint noise injection
- WebGL renderer and vendor spoofing (ANGLE NVIDIA GeForce)
- AudioContext noise injection (OfflineAudioContext)
- `navigator.plugins` populated with 5 realistic entries
- `navigator.languages` matches identity pool
- `chrome` runtime object spoofed
- `navigator.connection` set to realistic values
- `navigator.maxTouchPoints` set to realistic values
- `window.outerHeight` and `window.outerWidth` set to realistic values
- `navigator.hardwareConcurrency` set to 8
- `navigator.deviceMemory` set to 8
- `Notification.permission` set to `default`
- `navigator.permissions.query` spoofed (clipboard, geolocation)
- `WebGLRenderingContext.getParameter` spoofed
- `HTMLCanvasElement.toDataURL` noise injection
- `OfflineAudioContext` noise injection


## Auto-Install Coverage (22+ distros)
- Fedora, RHEL, CentOS, Rocky, AlmaLinux (dnf)
- Ubuntu, Debian, Mint, Pop, Zorin, Elementary, Kali (apt-get)
- Arch, Manjaro, EndeavourOS, Garuda (pacman)
- openSUSE, SLES (zypper)
- Alpine (apk)
- Amazon Linux (yum)
- Void (xbps-install)
- Gentoo (emerge)
- Immutable distros detected and skipped: Silverblue, Kinoite, NixOS, Guix, ostree


## Consequences
- Xvfb is auto-installed on first run (non-interactive sudo -n)
- User sees ZERO Chrome windows even on Linux Desktop
- UA/TLS mismatch eliminated (Chrome-only UA with Chromium TLS)
- Warm-up navigation reduces Cloudflare interstitial rate
- deep-research JSON output uses .titulo and .query consistently
- Fallback cascade: private Xvfb -> auto-install -> retry -> headless
- macOS and Windows use native headed mode (no Xvfb needed)


## Files Changed
- src/browser.rs: has_native_display(), try_auto_install_xvfb(), detect_linux_distro(), detect_linux_variant(), xvfb_manual_instruction(), flags_stealth(), STEALTH_SCRIPTS enhanced
- src/pipeline.rs: chrome_only_ua_for_platform() filter
- src/identity.rs: Chrome-only filtering for Chromium browser
- src/aggregation.rs: #[serde(rename = "titulo")] on AggregatedItem.title
- src/deep_research.rs: query field added to DeepResearchOutput


## References
- ADR-0007 (Chrome Primary Transport, v0.8.0)
- ADR-0008 (reqwest+rustls-tls, v0.8.6)
- gaps.md (GAP-WS-072 to GAP-WS-088)
