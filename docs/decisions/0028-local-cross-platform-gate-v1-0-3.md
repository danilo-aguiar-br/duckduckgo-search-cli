# ADR-0028 — Local cross-platform compile gates (v1.0.3)

- Status: Accepted (2026-08-07)
- Related: NO_CI.md (no remote CI), ADR-0017 (one-shot lifecycle), ADR-0009 (headed Xvfb)
- Decisor: lead
- Context: `/causa-raiz` audit — v1.0.2 shipped to crates.io unable to compile on macOS or Windows

## Context

1. A user reported `E0432` running `cargo install duckduckgo-search-cli` on macOS:
   `detect_linux_distro` and `xvfb_manual_instruction` were not found in `browser::xvfb`.

2. The audit found **three** compile defects, not one. All of them survived every gate in
   `NO_CI.md`, because **none of those gates passed `--target`**:

   - `src/browser/session/mod.rs` imported two `#[cfg(target_os = "linux")]` items through an
     ungated `use`. Rust strips `cfg`-disabled items **before** name resolution, so correctly
     gated call sites do not rescue an ungated import.
   - `src/browser/detect.rs` matched `std::env::var_os` with `if let Ok(..)` instead of
     `if let Some(..)` (`E0308`), twice. That is Windows-only code that had **never** been
     compiled by anything.
   - Thirteen further items were unused off-Linux, which `cargo lint` rejects under
     `-D warnings`.

3. `cargo publish --dry-run --locked` does not catch any of this: it builds for the host only.
   `docs.rs` does build the Apple and Windows targets declared in `Cargo.toml`
   `[package.metadata.docs.rs]`, so those documentation builds were red in public as well.

4. Forbidding remote CI (NO_CI.md) removes the *runner*, not the *requirement*. The gap was
   never "we have no GitHub Actions"; it was that the local gate set was never extended past
   the host triple.

## Decision

1. **Two real cross-target compile gates become mandatory** before tag and `cargo publish`:

   - `cargo check-windows` / `cargo lint-windows` — aliases in `.cargo/config.toml`, targeting
     `x86_64-pc-windows-gnu`.
   - `scripts/check-macos.sh` — targeting `aarch64-apple-darwin`.

2. **A cheap structural pre-check runs first**: `scripts/portability-lint.sh` fails in
   milliseconds on an ungated `use` of a platform-only item. It exists so the common regression
   is caught without paying for a cross-target build, **not** as a replacement for one.

3. **Windows is the primary cross gate.** It satisfies both `not(target_os = "linux")` and
   `not(unix)`, so it covers the entire `cfg`-regression class that broke macOS. It needs only
   the target std plus `x86_64-w64-mingw32-gcc` for `aws-lc-sys`.

4. **macOS gets a script, not a cargo alias.** Two constraints force this:

   - `cargo check --target aarch64-apple-darwin` **fails**. `aws-lc-sys` (C, reached through
     `rustls` → `reqwest`) drives the *host* `cc` with `-arch arm64 -mmacosx-version-min=11.0`,
     which a Linux gcc/clang cannot honour. This is upstream
     [aws/aws-lc-rs#1023](https://github.com/aws/aws-lc-rs/issues/1023).
   - `cargo-zigbuild` solves it, because `zig cc` bundles darwin libc headers. But it must be
     invoked as the binary `cargo-zigbuild check`; `cargo zigbuild check` is rejected, since
     cargo already consumes `zigbuild` as the subcommand. A cargo alias expands to a cargo
     subcommand and therefore **cannot** express this call.

5. **No Apple SDK is required for `check`**, because `cargo check` never links. Only the target
   std and a C compiler able to target darwin are needed. Verified from a clean state
   (`cargo clean -p aws-lc-sys --target aarch64-apple-darwin`, 1445 files / 216.7 MiB removed):
   `aws-lc-sys` recompiles for darwin in ~23 s and the full crate checks clean, exit 0. The
   emitted object is genuinely Apple — `file` reports `Mach-O 64-bit arm64 object` for
   `asn1_lib.o` inside `libaws_lc_0_42_0_crypto.a`. No `CC`, `CXX`, `AR` or `AWS_LC_SYS_*`
   override was needed; cargo-zigbuild injects `zig cc` / `zig ar` wrappers on its own.
   Upstream aws/aws-lc-rs#1023 does **not** reproduce with zig 0.16.0 + cargo-zigbuild 0.23.0 +
   aws-lc-sys 0.42.0.

6. **This gate must never be promoted to a build or release gate.** Measured: `cargo-zigbuild
   zigbuild --target aarch64-apple-darwin --all-features --locked` exits **101** at the *link*
   step with `error: unable to find framework 'CoreFoundation'. searched paths:  none` and the
   same for `'IOKit'`. zig ships darwin libc headers but **not** the macOS SDK, so Apple
   frameworks do not exist on this host. Compilation is provable here; linking is not.

## Prerequisites (once per host, no root)

```bash
rustup target add x86_64-pc-windows-gnu aarch64-apple-darwin
# Fedora: sudo dnf install mingw64-gcc     # provides x86_64-w64-mingw32-gcc
cargo install cargo-zigbuild --locked
python3 -m venv "$HOME/.local/zig-venv"
"$HOME/.local/zig-venv/bin/pip" install ziglang
printf '#!/usr/bin/env bash\nexec "%s/bin/python" -m ziglang "$@"\n' "$HOME/.local/zig-venv" \
    > "$HOME/.local/bin/zig"
chmod +x "$HOME/.local/bin/zig"
```

The venv is not about PEP 668 — measured on this host, `pip3 install --user ziglang` succeeds
and `externally-managed-environment` never fires. The real hazard is that `pip3` and `python3`
can resolve to *different* interpreters: here `pip3` belongs to `/usr/local/bin/python3.11`
while `python3` is `/usr/bin/python3`, so `python3 -c "import ziglang"` fails even though the
wheel installed correctly. A dedicated venv pins both to the same interpreter and needs no root.

`scripts/check-macos.sh` prepends `~/.local/bin` to `PATH` itself, so the shim also works from
non-interactive shells.

> **Correction (2026-08-08, superseded by ADR-0029).** This section originally claimed the shim
> was "verified persistent across sessions". The v1.0.3 e2e audit measured the opposite: on the
> maintainer host, `zig`, `cargo-zigbuild`, `~/.local/zig-venv` and the `aarch64-apple-darwin`
> target were **all absent**, and `scripts/check-macos.sh` exited 2. A one-off check that a
> binary is on `PATH` is not evidence of persistence — nothing in this repository installs or
> pins that toolchain, so its presence was never a repository invariant.
>
> The point is not that the shim was fragile; it is that a compile guarantee must not depend on
> unversioned host state that no gate verifies. ADR-0029 removes the dependency instead of
> re-installing it: with `reqwest`/`rustls` optional, the default build has no C, so the macOS
> gate needs neither `zig` nor `cargo-zigbuild`.

## Consequences

- Positive: the defect class that shipped in v1.0.2 cannot reach crates.io again without a
  maintainer ignoring a red gate. Both gates were run against the v1.0.3 fixes and pass with
  zero errors and zero warnings.
  **Amended 2026-08-08:** true when written, but the guarantee lapsed silently once the host
  toolchain disappeared, because a gate that cannot execute is indistinguishable from one that
  was never run. ADR-0029 restores the guarantee on a footing the repository controls.
- Positive: `docs.rs` builds for the declared Apple and Windows targets go green again.
- Cost: two extra toolchain installs on the maintainer host, and roughly one extra full
  dependency compile per target on first run (incremental afterwards).
- Limit: these are **compile** gates. They prove the code compiles for those targets; they do not
  link a macOS binary and they do not execute anything. Runtime behaviour on macOS and Windows
  remains unverified by automation, and no ADR should claim otherwise.
- Limit: `cargo-zigbuild --help` is the only place the subcommand list appears (`check`, `clippy`,
  `doc`, `install`, `run`, `rustc`, `test`, `zigbuild`, `zig`, `help`). `cargo zigbuild --help`
  prints only the help of the `zigbuild` verb, because cargo dispatches it to
  `cargo-zigbuild zigbuild`. Anyone auditing this gate will hit that first.
- Limit: `aws-lc-sys` is C, which sits against the goal of a rust-native, self-contained crate.
  It is reached through `rustls`, and no mature pure-Rust crypto provider exists for that stack
  today. Recorded here as a known deviation, not resolved by this ADR.
