# ADR-0029 — Optional HTTP stack removes the C toolchain from the default build (v1.0.3)

- Status: Accepted (2026-08-08)
- Related: ADR-0028 (local cross-platform gates), ADR-0021 (rustls-only TLS), GAP-WS-113 (Chrome-only transport), NO_CI.md
- Decisor: lead
- Context: e2e audit of v1.0.3 — the cross-platform gates promised by ADR-0028 could not run on the maintainer host

## Context

ADR-0028 promises that the `E0432` class cannot reach crates.io again, and rests that promise
on two local gates: `cargo check-windows` and `scripts/check-macos.sh`.

The v1.0.3 audit measured that **neither gate could execute**:

- `cargo check-windows` exited 101. Not from a code defect — the build script of
  `aws-lc-sys 0.42.0` aborted with `failed to find tool "x86_64-w64-mingw32-gcc"`.
  `aws-lc-sys` is C, and cross-compiling C to Windows needs a mingw cross compiler
  that is a **root-level system package**.
- `scripts/check-macos.sh` exited 2 because `zig`, `cargo-zigbuild` and the
  `aarch64-apple-darwin` target were all absent. ADR-0028 states the `zig` shim persists
  across sessions; on the audited host nothing had survived. That claim is corrected in
  ADR-0028 itself.

So the guarantee was structurally unenforceable: it depended on out-of-band host state that
no gate verified and no repository file could restore.

A second, independent constraint applies. The project mandates a **rust-native, self-contained**
binary and forbids a C toolchain. `aws-lc-sys` violated that mandate directly, and did so in the
*default* build — every downstream `cargo install` inherited the requirement.

## Measurement

`cargo tree -e all -i aws-lc-sys` reported exactly **one** root:

```
aws-lc-sys v0.42.0
└── aws-lc-rs v1.17.1
    └── rustls v0.23.42
        └── ... reqwest v0.12.28
            └── duckduckgo-search-cli v1.0.3
```

Two facts made the fix tractable:

- The `reqwest 0.13.4` that `chromiumoxide` pulls does **not** enable TLS and never reaches
  `aws-lc-sys`. Chrome/CDP needs no Rust TLS stack at all.
- `wiremock`, a dev-dependency, does **not** pull `aws-lc-sys` independently.

Therefore the entire C requirement hung on our own direct `reqwest`.

The decisive architectural fact is that this `reqwest` is **already dead in production**.
Transport is Chrome/CDP only (GAP-WS-113) and every residual HTTP path aborts in
`require_chrome_transport()`. `src/pipeline/single.rs` states it in the source:

- line 95 — "residual reqwest Client only for http-test-harness"
- line 106 — "HTTP warm-up is residual harness-only. Production warm-up is Chrome CDP"

The cookie jar and warm-up that defend against Cloudflare are performed by Chrome, not by
`reqwest`. Nothing in the anti-bot posture depends on the HTTP stack.

## Decision

Make `reqwest` and `rustls` **optional**, activated only by the pre-existing
`http-test-harness` feature. Add `http = "1"` as a direct dependency for the header types the
Chrome path shares — `reqwest::header` is a re-export of that same crate, so the move is a pure
import change with identical types.

`AggregatedSearchResult` moved from `search::execute` to a new transport-neutral
`search::aggregate`, because the Chrome pipeline returns that type and gating the HTTP module
would otherwise have taken it down.

We deliberately did **not** swap the crypto provider to `ring`: `ring` also compiles C and would
not have removed the toolchain requirement. Nor did we adopt a pure-Rust provider such as
`rustls-rustcrypto`, which is not production-audited. Removing the dependency beats replacing it.

## Consequences

Positive:

- The default build (`--features chrome`) contains no `aws-lc-sys`, no `cc` invocation, no C.
  Verified: `cargo tree -e all -i aws-lc-sys --no-default-features --features chrome` reports
  that the package does not match anything in the graph.
- The Windows and macOS gates run with no system package and no root. `rustup target add` is
  additive and unprivileged, which is the only prerequisite left.
- The rust-native mandate is satisfied in the artifact users actually install.

Negative, and stated plainly:

- The cross-platform gate can no longer pass `--all-targets`, because `tests/` uses `wiremock`
  and `reqwest`. It runs as `--no-default-features --features chrome`, covering lib and binary.
  Full `--all-targets` coverage remains on the Linux gate. `NO_CI.md` records this limit
  explicitly rather than leaving the narrower scope implied.
- Anyone exercising the residual HTTP harness must build with `--features http-test-harness`,
  which does reintroduce the C toolchain requirement for that configuration only.

## Alternatives rejected

- **Install `mingw64-gcc` and keep `aws-lc-rs`.** Unblocks the gate today but leaves C in the
  shipped artifact and keeps violating the rust-native mandate. It also re-creates the exact
  failure mode of ADR-0028: a guarantee resting on unversioned host state.
- **Swap the provider to `ring`.** Still C. No gain.
