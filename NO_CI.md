# NO CI / NO GitHub Actions

Read this in [Portuguese](NO_CI.pt-BR.md).

This repository **forbids** continuous integration on GitHub Actions,
Dependabot, pre-commit CI, and any remote publish pipeline.

## Forbidden

- `.github/workflows/**` (ci, release, audit, docs, etc.)
- Dependabot / Renovate configs that open automated PRs for CI
- `scripts/pre-publish-gate.sh` or any gate that calls `gh run list`
- Pre-commit hooks that require a remote runner
- Badges that claim green CI status

## Required local gates (maintainers)

Run before every tag and before `cargo publish`:

```bash
./scripts/portability-lint.sh   # seconds; fails fast on ungated `use` of gated item
cargo check-all
cargo check-nohttp              # host, `chrome` profile only — catches items orphaned off-harness
cargo check-nohttp-all-targets  # v1.0.6: the SHIPPED profile, INCLUDING tests (GAP-REL-002)
cargo lint-nohttp
cargo check-windows             # rustc against a NON-Linux target (GNU ABI)
cargo check-windows-msvc        # the ABI Windows users actually install
cargo check-macos               # alias form of the aarch64-apple-darwin check
cargo check-macos-intel         # alias form of the x86_64-apple-darwin check
./scripts/check-macos.sh        # rustc against aarch64-apple-darwin, plus the aws-lc-sys probe
./scripts/check-macos.sh x86_64-apple-darwin   # Intel Mac half of the universal binary
cargo lint
cargo lint-windows
cargo lint-macos                # clippy on aarch64-apple-darwin, the macOS half of `lint-windows`
cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo docs
RUSTDOCFLAGS="-D warnings" cargo docs-nohttp   # same, on the DEFAULT feature set
cargo test-all          # or at least: cargo test --lib --all-features --locked
cargo deny check        # when deny.toml is present
cargo publish-check     # alias for `publish --dry-run --locked`
cargo pkg-list          # alias for `package --list`: what the tarball would carry
```

Aliases live in [`.cargo/config.toml`](.cargo/config.toml).

Two aliases exist there and stay **outside** this mandatory battery, deliberately:

- `cov` (`.cargo/config.toml:121`) and `cov-html` (`:124`) both call
  `cargo llvm-cov`, a subcommand this repository does not vendor and does not
  require any host to install. A gate that cannot run on the maintainer's host
  is indistinguishable from one never run, so coverage stays an on-demand tool
  rather than a release gate.

`check-macos` (`:77`), `check-macos-intel` (`:78`), `lint-macos` (`:79`),
`publish-check` (`:127`) and `pkg-list` (`:130`) are in the battery above.
`check-macos` and `check-macos-intel` duplicate what `scripts/check-macos.sh`
compiles; the script stays listed because it additionally exits **2** with the
exact `rustup target add` line and re-runs the `aws-lc-sys` tree probe.

### The blind spot depends on the HOST (v1.0.6)

The list above was written on a Linux host, where the uncovered targets were
macOS and Windows. On a macOS host the blind spot **inverts**: `cargo
check-macos` becomes a native check, and nothing compiles against
`x86_64-unknown-linux-gnu` any more — which leaves every
`#[cfg(target_os = "linux")]` item, `src/browser/xvfb.rs` included, with no
`rustc` coverage at all. That is the very module the `E0432` came from.

On a Windows host the uncovered targets are **Linux and macOS**: both Windows
gates become native checks, so `cargo check-linux` and the two macOS checks are
the ones that have to be added there.

Whichever host you are on, add the target you are NOT:

```bash
rustup target add x86_64-unknown-linux-gnu   # on a macOS or Windows host
cargo check-linux
```

`cargo check` does not link, so the target's `std` is enough and no C toolchain
is involved.

### After publishing, and after every yank (v1.0.6, ADR-0032)

Rationale lives in
[`docs/decisions/0032-post-publish-verification-gate-v1-0-6.md`](docs/decisions/0032-post-publish-verification-gate-v1-0-6.md).

```bash
cargo run --bin verify_published --features release-gate
```

Every gate above answers "does the tree compile?". `cargo publish --dry-run`
answers "is the package well-formed?". Neither answers the question the user
actually experiences: **does the version the registry SERVES compile?**

Yank is a registry mutation, not a no-op. `max_stable_version` is derived from
the yank state of every version, so yanking a fixed release promotes an older
one. Measured 2026-08-21: v1.0.5 was published and then yanked, which restored
the broken v1.0.2 as the default that `cargo install` resolves to — eleven days
after the fix had shipped, with no signal anywhere.

Rules:

- A published version that fails `./scripts/portability-lint.sh` is yanked. Not
  optional. The v0.9.6 entry in `CHANGELOG.md` recorded "Yank optional" and that
  is why this is the second occurrence of the class, not the first.
- Publish the working version **before** yanking the broken one. The Cargo book
  requires that order so dependents are never left without a compatible release.
- Run `verify_published` after `cargo publish` and again after each `cargo
  yank`. Exit 0 means the registry serves this version and no version measured
  broken is still installable.

### Why a non-Linux gate is mandatory

Forbidding remote CI does **not** remove the need to verify other platforms —
it moves that verification onto the maintainer's host.

v1.0.2 shipped to crates.io unable to compile on macOS **or** Windows. Three
independent defects survived every gate above, because none of them passed
`--target`:

- `src/browser/session/mod.rs` imported `#[cfg(target_os = "linux")]` items
  through an ungated `use` (E0432). `cfg`-stripping runs *before* name
  resolution, so gated call sites do not save an ungated import.
- `src/browser/detect.rs` matched `std::env::var_os` with `if let Ok(..)`
  instead of `if let Some(..)` (E0308) — Windows-only code that had never been
  compiled.
- Two bindings became unused off-Linux, which `-D warnings` rejects.

`cargo check` does not link, so `cargo check-windows` needs only the target std.
Prerequisites, once per host — **no root, no system package**:

```bash
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-pc-windows-msvc
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin
```

### v1.0.4: the two targets that were documented but ungated

`docs/CROSS_PLATFORM` lists four supported targets. Two of them —
`x86_64-pc-windows-msvc` and `x86_64-apple-darwin` — had no gate at all: the
Windows gate mired the GNU ABI, and the macOS script defaulted to Apple Silicon
and was never invoked for Intel. Both now run, and both cost only a `rustup
target add`, because `cargo check` does not link.

What these gates still do NOT prove is unchanged and worth restating: they prove
the code **compiles** for those targets. No binary has ever been **executed** on
a macOS or Windows host. Runtime behaviour there remains unverified.

### v1.0.3: no C toolchain (ADR-0029)

Until v1.0.3 this section also told you to `sudo dnf install mingw64-gcc`, because
`aws-lc-sys` (C, reached through `rustls` → `reqwest`) drove a cross `cc`. That
requirement is **gone**: `reqwest` and `rustls` are now optional and activated only by
`http-test-harness`, so the default `chrome` build carries no C at all.

Verify it rather than trusting it:

```bash
cargo tree -e all -i aws-lc-sys --no-default-features --features chrome
# expected: "package ID specification `aws-lc-sys` did not match any packages"
```

The v1.0.3 audit is the reason this matters. Both cross-platform gates were found
**unrunnable** on the maintainer host — `cargo check-windows` died in the `aws-lc-sys`
build script, and `scripts/check-macos.sh` exited 2 with `zig` absent. A gate that
cannot execute is indistinguishable from one that was never run, so the ADR-0028
guarantee had silently lapsed. Removing the C dependency puts the gate back on
footing this repository controls.

### Known limit of the cross-platform gates

The cross gates run as `--no-default-features --features chrome` and therefore
**do not pass `--all-targets`**. Integration tests use `wiremock` and `reqwest`,
which only exist under `http-test-harness`; enabling that feature would drag the
C toolchain back in and defeat the purpose.

Consequence, stated so nobody assumes wider coverage than exists: the cross gates
cover the **library and binary** for Windows and macOS. Tests, benches and examples
are covered for Linux only, by `cargo check-all` and `cargo test-all`. A defect that
lives exclusively in a `#[cfg(test)]` block on a non-Linux target would not be caught.

Windows satisfies both `not(target_os = "linux")` and `not(unix)`, so that gate
alone already covers the whole `cfg`-regression class that broke macOS. It does
not cover defects unique to `cfg(target_os = "macos")` branches, which is why
`scripts/check-macos.sh` exists as well.

macOS still needs its own script, but **not** for the reason it used to. Until
v1.0.3, `cargo check --target aarch64-apple-darwin` failed outright: `aws-lc-sys`
(C, pulled in by rustls) drove the *host* `cc` with `-arch arm64`, which a Linux
compiler cannot honour (upstream aws/aws-lc-rs#1023), and `cargo-zigbuild` was
the workaround. ADR-0029 removed the C dependency, so plain `cargo check` now
works and **neither `zig` nor `cargo-zigbuild` is needed**.

What the script still does that a cargo alias cannot:

- checks that the target std is installed and exits **2** with the exact
  `rustup target add` line, instead of dying inside a build script;
- re-runs the `aws-lc-sys` tree probe after a successful check, so the day a
  dependency puts C back on the default path this gate fails loudly rather than
  quietly starting to need a cross compiler again.

Rationale lives in
[`docs/decisions/0029-optional-http-stack-no-c-toolchain-v1-0-3.md`](docs/decisions/0029-optional-http-stack-no-c-toolchain-v1-0-3.md).
[`0028`](docs/decisions/0028-local-cross-platform-gate-v1-0-3.md) records why the
gate exists at all; its zig prerequisites are superseded by 0029.

## Release (manual only)

1. Bump `version` in `Cargo.toml` / `Cargo.lock` and update `CHANGELOG.md`.
2. Pass local gates above.
3. Commit on `main` (or merge a release branch into `main`).
4. Annotated tag: `git tag -a vX.Y.Z -m "Release vX.Y.Z: …"`.
5. Push: `git push origin main && git push origin vX.Y.Z`.
6. Optional GitHub Release notes via `gh release create` (no Actions). That
   command only creates the release object; **no workflow is triggered by it**,
   because this repository has no `.github/workflows/**` for anything to
   dispatch. Read it as a note-publishing step, never as a loophole into CI.
7. Publish: `cargo publish --locked`.
8. Verify the registry: `cargo run --bin verify_published --features release-gate`.
9. Yank the broken versions, in this order and **only after** step 7 succeeded:
   `cargo yank --version 1.0.2` then `cargo yank --version 1.0.1`. Publishing
   first is mandatory: `max_stable_version` is derived from yank state, so
   yanking before the fixed release exists promotes a broken version back to
   being the default.
10. Re-run the gate of step 8 after **each** yank, not once at the end. Yank is
    a registry mutation, and the mutation is exactly what step 8 measures.

There is **no** automatic crates.io upload on tag push.

## Optional local tooling

Host-only settings (mold/lld, sccache, `target-cpu=native`) belong in the
**user** `~/.cargo/config.toml`, never in this repository’s published config.
Do not bake host CPU features into crates.io artifacts.
