# ADR-0008 — Revert to reqwest + rustls-tls (v0.8.6)

- Status: Accepted (2026-06-22)
- Supersedes: ADR-0001 (wreq/BoringSSL, v0.7.3)
- Superseded (production residual HTTP SERP): **ADR-0016** (v0.9.4 / GAP-WS-113) — production network transport is Chrome-only; the reqwest/rustls stack remains for build simplicity and **test-only** `http-test-harness`, not as a production SERP fallback
- Decisor: lead
- Contexto: GAP-WS-066 (cargo install fails on Windows — btls-sys requires NASM+CMake)

## Context

ADR-0001 adopted wreq/BoringSSL in v0.7.3 to solve GAP-WS-27 (CAPTCHA caused by rustls TLS fingerprint). Since v0.8.0, Chrome headed via chromiumoxide is the primary search transport and produces a REAL browser TLS fingerprint. The wreq/BoringSSL stack is now redundant for anti-bot evasion but imposes a heavy build-time cost: NASM, CMake, Perl, and MSVC are required on Windows, making `cargo install` impossible without manual setup of 4 external tools.

## Decision

Replace `wreq 6.0.0-rc.29` + `wreq-util 3.0.0-rc.12` with `reqwest 0.12` + feature `rustls-tls`. Remove `brotli`, `brotli-decompressor`, and `alloc-no-stdlib` pins. Remove BoringSSL preflight checks from build.rs. Rename `wreq_cookie_adapter.rs` to `cookie_adapter.rs` using reqwest cookie API.

## Consequences

### Positive
- `cargo install` works on Windows with ZERO extra tools (just Rust)
- Build time reduced by 3-5 minutes (BoringSSL C compilation eliminated)
- Binary size reduced by ~20 MB (no static BoringSSL)
- Dependency tree reduced from ~382 to ~340 crates
- TLS stack unified: rustls everywhere (chromiumoxide + reqwest)
- cmake, perl, pkg-config, libclang-dev no longer needed on any platform

### Negative
- HTTP fallback (when Chrome is unavailable) loses BoringSSL TLS fingerprint emulation
- DuckDuckGo may block or degrade results from pure rustls fingerprint in HTTP-only mode
- This trade-off was acceptable while Chrome headed was the primary transport (v0.8.0–v0.9.3)

### Trade-offs accepted
- Accept degraded HTTP-only fallback in exchange for universal cross-platform compilation (historical through v0.9.3)
- Accept losing wreq-util TLS emulation in exchange for eliminating 4 build prerequisites
- Accept this is a breaking change in build requirements (simpler, not harder)

### Supersession note (v0.9.4 / ADR-0016)

Production residual HTTP SERP / HTTP-only fallback described above is **superseded by ADR-0016** (GAP-WS-113). Since v0.9.4, chromiumoxide/CDP is the only production network transport; missing Chrome fails closed with exit 2. Residual HTTP lives only under `http-test-harness` + `DUCKDUCKGO_SEARCH_CLI_HTTP_TEST=1`. The pure-Rust TLS decision for the `reqwest` client remains valid for builds and tests.

## Files changed
- Cargo.toml: wreq/wreq-util removed, reqwest added
- build.rs: BoringSSL preflights removed (NASM, CMake, MSVC, Perl)
- src/http.rs: wreq::Client -> reqwest::Client, .emulation() removed
- src/cookie_adapter.rs: rewritten for reqwest cookie API (was wreq_cookie_adapter.rs)
- 12 source files: mechanical wreq:: -> reqwest:: replacement
- docs/CROSS_PLATFORM.md, docs/INSTALL-WINDOWS.md: updated for zero-prereq build
