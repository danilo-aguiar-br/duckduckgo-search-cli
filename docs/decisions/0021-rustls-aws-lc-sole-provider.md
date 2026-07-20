# ADR-0021 — Sole rustls CryptoProvider (`aws-lc-rs`) + residual HTTP policy

- Status: Accepted (2026-07-19)
- Supersedes (feature string): ADR-0008 detail that cited `reqwest` feature `rustls-tls` (which enabled `ring`)
- Related: ADR-0008 (reqwest + rustls), ADR-0016 (Chrome-only production transport)
- Decisor: lead
- Context: Pass 40 `/r-auditoria` rustls mandatory rules

## Context

1. Residual HTTP uses `reqwest 0.12` with pure-Rust TLS. Feature `rustls-tls` expands to
   `rustls-tls-webpki-roots` → **`__rustls-ring`**, embedding the `ring` crypto provider.
2. `chromiumoxide` 0.9.x always depends on `reqwest 0.13` (no TLS features by default). Its
   optional features `rustls` / `zip8` only enable **`chromiumoxide_fetcher`** (download
   Chromium), which this CLI does **not** use (system Chrome detection).
3. Enabling chromiumoxide `rustls`+`zip8` pulled fetcher + `rustls-platform-verifier` and
   contributed to a **dual** `ring` + `aws-lc-rs` graph — forbidden by project rustls rules.
4. Production SERP/probe/fetch TLS is the **Chrome subprocess** (BoringSSL inside the browser
   binary, not linked as a Rust crate). Residual rustls must not reintroduce OpenSSL/native-tls.

## Decision

1. **Process CryptoProvider:** install `rustls::crypto::aws_lc_rs::default_provider()` once in
   binary `main` via `tls_bootstrap::install_rustls_crypto_provider()`, **before** the Tokio
   multi-thread runtime.
2. **reqwest features:** `default-features = false` +
   `rustls-tls-webpki-roots-no-provider` (+ cookies, gzip, deflate, socks, http2).
   Never `native-tls` / `default-tls` / plain `rustls-tls` (ring).
3. **Direct `rustls` pin:** `version = "0.23.18"` (lock resolves current 0.23.x),
   `default-features = false`, features `std`, `tls12`, `aws_lc_rs`, `logging`.
4. **chromiumoxide:** `default-features = false` only (bytes default). **Do not** enable
   `rustls` / `zip8` / `fetcher`.
5. **Proxy:** residual client never inherits `HTTP_PROXY`/`HTTPS_PROXY`. Proxy only via
   `--proxy` / `ProxyConfig::Url` (XDG/CLI config rule).
6. **Hot path:** do not construct `reqwest::Client` on production Chrome-only paths
   (`maybe_build_residual_client` / harness gate).
7. **cargo-deny:** ban `native-tls`, `openssl`, `openssl-sys`, `hyper-tls`, `tokio-native-tls`.

## Consequences

### Positive

- Single crypto provider (`aws-lc-rs`); `ring` absent from the tree.
- Smaller residual graph (no fetcher, no platform-verifier from chromiumoxide TLS path).
- Explicit bootstrap order safe for multi-thread Tokio.
- Aligns with musl/static/cross builds without system OpenSSL.

### Negative / accepted

- `chromiumoxide` still hard-depends on **reqwest 0.13** (no TLS features) — dual *package*
  version with our 0.12 residual client; not dual *TLS provider*. Track upstream.
- Operators who relied on silent `HTTP_PROXY` for residual HTTP must pass `--proxy`.
- Library consumers of `duckduckgo_search_cli` must install the CryptoProvider themselves
  if they build residual clients without using our binary `main`.

## Verification

- `cargo tree -i ring` → empty
- `cargo tree -i native-tls` / `openssl-sys` → empty
- `cargo tree -i aws-lc-rs` → present
- `cargo test --features chrome --lib --locked` green
