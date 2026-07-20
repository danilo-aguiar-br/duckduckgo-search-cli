# ADR-0022 — No synthetic hardware fingerprint spoof (Cloudflare)

- Status: Accepted (2026-07-19)
- Supersedes (partial): hardware-fingerprint spoof layers described in
  [ADR-0007](0007-chrome-primary-transport-v0-8-0.md) and
  [ADR-0009](0009-headed-xvfb-private-v0-8-7.md) (canvas noise, WebGL GPU lie,
  AudioContext noise, forced `hardwareConcurrency` / `deviceMemory` / `colorDepth`,
  fixed `navigator.languages` / `navigator.connection`)
- Related: ADR-0016 (Chrome-only production transport), ADR-0021 (rustls residual),
  GAP-WS-27 (library TLS bot-class signatures)
- Decisor: lead
- Context: Pass 41 `/r-auditoria` + GraphRAG browser anti-bot rules + operator mandate
  “proibido ter fingerprint porque a Cloudflare bloqueia”

## Context

1. Cloudflare Bot Management **blocks bot-class client signatures**, including:
   - Library TLS stacks such as `rustls` (JA4_o) — fixed by **native Chrome transport**
     (ADR-0016), not by advertising a “fingerprint product feature”.
   - **Stable synthetic hardware fingerprints** shared across all CLI sessions
     (deterministic canvas `+1`, fixed “GTX 1650 Direct3D11” WebGL on every OS,
     fixed audio noise, forced concurrency=8). GraphRAG: each instance must not
     share the same automation fingerprint; never use the same fingerprint under
     massive concurrency.

2. Pass 40 documentation re-framed production as “Chrome TLS fingerprint real”,
   which inverted the goal: the product must **avoid** bot-class fingerprints,
   not market fingerprinting.

3. Layer 3b in `src/browser/stealth.rs` (GAP-NEW-007) implemented exactly the
   forbidden static spoofs.

## Decision

1. **Remove** all synthetic hardware fingerprint spoofs from CDP stealth scripts.
2. **Keep** automation-signal mitigation only: `webdriver`, plugins/`mimeTypes`,
   `window.chrome` stubs, outer window size, Permissions quirks, DevTools
   WebSocket leak block.
3. **Keep** native Chrome as production SERP transport (ADR-0016) so residual
   `reqwest`+rustls is never the production TLS path.
4. **Keep** UA↔Chrome process coherence (`coerce_chrome_user_agent`) — this is
   profile consistency, not hardware fingerprint spoofing.
5. **Docs/doctor/SECURITY** must use the canonical wording: native Chrome TLS
   transport; forbidden synthetic fingerprint spoof; residual rustls harness only.

## Consequences

### Positive

- No shared automation canvas/WebGL/audio signature across all users/sessions.
- Parallel multi-query Chrome sessions no longer clone the same spoofed GPU/canvas.
- Aligns with GraphRAG anti-bot rules and operator mandate.
- Smaller CDP payload (memory / one-shot).

### Negative / accepted

- Real host GPU/canvas values are visible to the page (normal browser behaviour).
- Historical ADR-0007/0009 still describe old spoof lists — this ADR supersedes
  those layers; do not reintroduce them.

## Verification

- `rg 'toDataURL|GTX 1650|getChannelData|hardwareConcurrency' src/browser/stealth.rs` empty of spoof
- Smoke SERP Chrome still returns organic results
- Doctor `tls_stack` cites ADR-0022 / no synthetic fingerprint spoof
