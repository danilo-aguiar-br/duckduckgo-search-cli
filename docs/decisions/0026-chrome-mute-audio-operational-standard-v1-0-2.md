# ADR-0026 — Chrome mute-audio is the operational standard (v1.0.2)

- Status: Accepted (2026-07-23)
- Related: ADR-0009 (headed Xvfb), ADR-0016 (Chrome-only transport), ADR-0017 (one-shot lifecycle),
  ADR-0022 (no synthetic AudioContext fingerprint spoof — orthogonal), GAP-CHROME-MUTE-001
- Decisor: lead
- Context: `/causa-raiz` audit — intermittent host speaker sound during deep-research / SERP

## Context
1. Production transport is HEADED CHROME (Linux: private Xvfb; macOS/Windows: native or
   headless=new). Xvfb hides the window; it does NOT mute host audio. Chromium still
   talks to PulseAudio/ALSA/CoreAudio on the operator machine.

2. Deep-research and dual SERP navigate REAL pages (news, ads, autoplay video/audio).
   Without an explicit Chromium mute switch, media can play on the host speakers
   intermittently — agent-hostile UX (noise during long unattended runs).

3. Full CLI audit found:
   - ZERO Rust audio crates (`rodio`/`cpal`/etc.), zero terminal BEL emission for alerts.
   - SINGLE Chrome launch SSOT: `ChromeBrowser::launch` → `flags_stealth` +
     `CHROMIUMOXIDE_SAFE_DEFAULTS` (pipeline SERP, deep-research, probe, content-fetch pool).
   - No XDG/CLI toggle previously documented; mute was missing before this ADR.

4. External evidence (Chromium switches / automation practice):
   - `--mute-audio` — mutes audio sent to the audio device (Chromium command-line switches).
   - `--autoplay-policy=document-user-activation-required` — blocks media autoplay without
     a user gesture (defense in depth for SERP/news pages).
   - Same pattern used by Selenium/Puppeteer automation defaults for CI silence.

## Decision
1. Mute is always ON for every Chrome process launched by this CLI. No opt-out flag,
   no product env, no XDG key that re-enables host speakers. Agent-native default.

2. SSOT strings live in `src/browser/mod.rs`:
   - `CHROME_MUTE_AUDIO_FLAG` = `--mute-audio`
   - `CHROME_AUTOPLAY_POLICY_FLAG` = `--autoplay-policy=document-user-activation-required`

3. Belt and suspenders — both layers always include the mute pair:
   - `CHROMIUMOXIDE_SAFE_DEFAULTS` (applied after `.disable_default_args()`)
   - `flags_stealth(...)` (stealth / anti-bot flag list)

4. Fail closed — `ensure_chrome_audio_muted` runs inside `ChromeBrowser::launch` before
   `BrowserConfig` build. If a refactor drops mute, launch returns `InvalidConfig` instead
   of starting a loud browser.

5. Orthogonal to ADR-0022 — muting speakers is NOT AudioContext fingerprint spoof.
   We do NOT inject OfflineAudioContext noise; we only silence host output.

6. Out of scope / non-goals:
   - Capturing or analyzing page audio
   - OS-level PulseAudio policy (CLI owns Chromium flags only)
   - Re-enabling audio for “debug with sound” (use a real browser outside the CLI)

## Consequences

### Positive
- Deep-research / SERP / fetch-content never surprise the operator with page sound.
- Single launch path + unit tests + fail-closed guard make mute hard to regress.
- Agents need no shell wrapper (`PULSE_SERVER=`, `pacmd`) for silence.

### Negative / accepted
- If an operator ever needed to *hear* page media through the CLI, they cannot — by design.
- Duplicate mute flags in safe defaults + stealth are intentional redundancy (harmless).

## Implementation map

| Path | Role |
|------|------|
| `src/browser/mod.rs` | Constants + `CHROMIUMOXIDE_SAFE_DEFAULTS` |
| `src/browser/session.rs` | `flags_stealth`, `ensure_chrome_audio_muted`, `ChromeBrowser::launch` |
| `src/pipeline/chrome.rs` | Only launch site for SERP (calls `ChromeBrowser::launch`) |
| `src/content_fetch/enrich.rs` | Pool launch (same SSOT) |
| `src/probe.rs` | Probe / probe-deep (same SSOT) |
| Unit tests in `browser/mod.rs` | Matrix sandbox/proxy + safe-defaults + reject loud args |

## Amendment — GAP-CHROME-MUTE-002 (2026-07-23)

Symptom after V24: CLI still played host audio on video/news pages during
deep-research / fetch-content even though ADR-0026 strings were present in
source and unit tests passed.

Root cause (validated with `/proc/<pid>/cmdline`):
`chromiumoxide` `ArgsBuilder` formats every arg as `--{key}`. The CLI passed
full Chromium tokens (`--mute-audio`). The crate treated the entire string as
the key → process argv became `----mute-audio` (four dashes). Chromium
SILENTLY IGNORES unknown switches. Headed Xvfb mode does NOT get
chromiumoxide's built-in headless mute path — so host PulseAudio/PipeWire
stayed live.

Countermeasure:
1. `chromiumoxide_arg_token` strips one leading `--` at the BrowserConfig boundary.
2. `ensure_chrome_audio_muted_rendered` fail-closes if rendered argv is not
   exactly `--mute-audio` / `--autoplay-policy=...` (rejects `----mute-audio`).
3. Unit tests encode the double-prefix bug class and the matrix after strip.

## DoD
- [x] Mute constants + both arg lists
- [x] Fail-closed `ensure_chrome_audio_muted` at launch
- [x] GAP-CHROME-MUTE-002: token strip + rendered-argv fail-closed
- [x] Unit tests green (including quad-dash regression)
- [x] This ADR + gaps GAP-CHROME-MUTE-001 closed; MUTE-002 closed
- [x] CHANGELOG EN/PT; PROMPT_RULES anti-CF note
