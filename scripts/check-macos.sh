#!/usr/bin/env bash
# macOS compile gate, runnable from a Linux host (NO_CI.md — gates are local).
#
# v1.0.3 (ADR-0029) rewrote this script. It used to need `zig`, `cargo-zigbuild`
# and a python venv, because `aws-lc-sys` (C, reached through rustls → reqwest)
# drove the host `cc` with `-arch arm64`, which a Linux gcc cannot honour
# (upstream aws/aws-lc-rs#1023). `zig cc` was the workaround.
#
# That whole apparatus is gone. `reqwest` and `rustls` are now optional and only
# the `http-test-harness` feature pulls them, so the profile users actually
# install — `--no-default-features --features chrome` — contains no C at all.
# Plain `cargo check --target aarch64-apple-darwin` works.
#
# Why the rewrite mattered: the v1.0.3 audit found the old script exiting 2 on
# the maintainer host, with zig, cargo-zigbuild, the venv and the target all
# absent, even though ADR-0028 claimed the shim was persistent. A gate that
# cannot execute is indistinguishable from one that was never run, so the
# cross-platform guarantee had lapsed silently. Depending on nothing is the only
# dependency that cannot rot.
#
# Scope, stated plainly: this checks the LIBRARY and BINARY. It does not pass
# `--all-targets`, because the integration tests need `wiremock` + `reqwest`,
# which would drag the C toolchain back in. Tests and benches are covered on
# Linux by `cargo check-all` / `cargo test-all`.
#
# Prerequisite (once per host, additive, NO root):
#   rustup target add aarch64-apple-darwin
#
# Usage:
#   scripts/check-macos.sh [TARGET]      # TARGET defaults to aarch64-apple-darwin
#
# Exit codes:
#   0 — the crate checks cleanly for the macOS target
#   1 — the crate fails to check for the macOS target
#   2 — a prerequisite is missing (message names which one)

set -uo pipefail

TARGET="${1:-aarch64-apple-darwin}"

if ! rustup target list --installed 2>/dev/null | rg -q "^${TARGET}$"; then
    echo "check-macos: rust std for ${TARGET} not installed — run: rustup target add ${TARGET}" >&2
    exit 2
fi

echo "check-macos: target ${TARGET}, profile --no-default-features --features chrome (no C toolchain)"

cargo check --target "${TARGET}" --no-default-features --features chrome --locked
status=$?

if [ "$status" -ne 0 ]; then
    echo "check-macos: FAILED for ${TARGET}" >&2
    exit 1
fi

# Guard the property this script exists to protect: if anything ever puts a C
# crate back on the default path, the gate must fail loudly here rather than
# silently start requiring a cross compiler again.
if cargo tree -e all -i aws-lc-sys --no-default-features --features chrome >/dev/null 2>&1; then
    echo "check-macos: FAILED — aws-lc-sys is back in the default dependency graph (ADR-0029)" >&2
    exit 1
fi

echo "check-macos: OK (${TARGET}, zero C dependencies)"
exit 0
