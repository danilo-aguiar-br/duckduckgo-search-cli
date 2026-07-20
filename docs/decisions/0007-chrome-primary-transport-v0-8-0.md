# ADR-0007: Chrome Headed as Primary Search Transport (v0.8.0)


## Status
- Accepted (2026-06-21). Note: wreq references in this ADR are historical; wreq was replaced by reqwest+rustls in v0.8.6 (ADR-0008). **Hardware fingerprint spoof layers (canvas/WebGL/audio/hwConcurrency) listed below are superseded by [ADR-0022](0022-no-synthetic-fingerprint-spoof.md)** (static spoofs are forbidden).
- **Partially superseded by ADR-0016 (v0.9.4 / GAP-WS-113):** Chrome is no longer merely *primary* — it is the **only** production network transport. Residual reqwest for `--fetch-content` / `--probe` described below is **historical**; those paths use Chrome in production since v0.9.4


## Context
- wreq HTTP client with BoringSSL TLS (JA3 fingerprint) was blocked by Cloudflare
- Chrome headless mode was detected by Cloudflare anti-bot (6 stealth deficiencies)
- Chrome headless=new was detected by rendering pipeline differences
- Canvas, WebGL, AudioContext fingerprints were deterministic in headless mode
- outerHeight=0 and empty PluginArray exposed headless Chrome


## Decision
- Chrome headed mode inside private Xvfb virtual display is the PRIMARY search transport
- 17 JavaScript stealth signals are injected via CDP before page navigation
- Private Xvfb is auto-spawned via `spawn_virtual_display()` — no manual `xvfb-run` needed (v0.8.5+, enhanced in v0.8.7)
- **Historical (pre-v0.9.4):** reqwest+rustls-tls was used for `--fetch-content` and `--probe` HTTP requests (v0.8.6+ replaced wreq/BoringSSL)
- **Current (v0.9.4+ / ADR-0016):** production network I/O is Chrome-only; residual HTTP only under `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`
- Headless mode is FALLBACK when Xvfb is unavailable (Linux; macOS/Windows use headless=new since v0.9.3)


## Stealth Signals (17)
- `navigator.webdriver` set to `false`
- Canvas fingerprint noise injection
- WebGL renderer and vendor spoofing
- AudioContext noise injection
- `navigator.plugins` populated with realistic entries
- `navigator.languages` matches identity pool
- `chrome` runtime object spoofed
- `navigator.connection` set to realistic values
- `navigator.maxTouchPoints` set to realistic values
- `window.outerHeight` and `window.outerWidth` set to realistic values
- `navigator.hardwareConcurrency` set to 8
- `navigator.deviceMemory` set to 8
- `Notification.permission` set to `default`
- `navigator.permissions` query spoofed
- `WebGLRenderingContext.getParameter` spoofed
- `HTMLCanvasElement.toDataURL` noise injection
- `OfflineAudioContext` noise injection


## Consequences
- Linux servers MUST have Xvfb installed (v0.8.7+ auto-installs on 22+ distros via `try_auto_install_xvfb()`)
- Chrome or Chromium MUST be installed on all platforms
- Binary size increases by ~20 MB (BoringSSL + chromiumoxide)
- Search latency increases by ~500ms (Chrome startup + navigation)
- Cloudflare anti-bot is bypassed on 2026-06-21 test environment


## v0.8.7 Updates (GAP-WS-072 to GAP-WS-088)
- `xvfb-run` replaced by private Xvfb spawned via `spawn_virtual_display()` — user sees ZERO windows
- `has_native_display()` detects native display per platform (Linux $DISPLAY/$WAYLAND_DISPLAY, macOS Quartz, Windows DWM)
- `try_auto_install_xvfb()` auto-installs Xvfb on 22+ Linux distros via `detect_linux_distro()`
- `chrome_only_ua_for_platform()` ensures ONLY Chrome UA is used with Chromium TLS fingerprint (GAP-WS-074)
- Warm-up navigation to duckduckgo.com BEFORE search URL with random delay (GAP-WS-077)
- `navigator.webdriver` set to `undefined` (was `false` in v0.8.0) matching real Chrome behavior (GAP-WS-076)
- CDP WebSocket leak prevention and stack trace filtering added (GAP-WS-076)
- `AggregatedItem.title` serializes as `titulo` for schema parity with `SearchResult` (GAP-WS-087)
- `DeepResearchOutput.query` field added for schema parity with `SearchOutput` (GAP-WS-088)
- See ADR-0009 for the full v0.8.7 architectural decision


## Alternatives Considered
- Headless=new with more stealth: REJECTED (rendering pipeline differences detectable)
- Playwright/Puppeteer: REJECTED (Node.js dependency, not pure Rust)
- wreq with better TLS emulation: REJECTED (JA3 alone is insufficient)
- Rotating proxies: REJECTED (operational complexity, cost)

## Supersession
- Residual HTTP paths (probe/fetch/fallback) are **superseded by ADR-0016** (Chrome-only universal, v0.9.4 / GAP-WS-113).

