// SPDX-License-Identifier: MIT OR Apache-2.0
//! The shipped profile must contain no C toolchain, measured from `cargo`.
//!
//! # Why this ruler exists
//!
//! `NO_CI.md`, `scripts/check-macos.sh` and the alias block in
//! `.cargo/config.toml` all state the same fact in prose: the profile a user
//! installs — `--no-default-features --features chrome` — is pure Rust, and
//! only the `http-test-harness` feature drags in `aws-lc-sys`, which needs a C
//! compiler and CMake.
//!
//! Three documents asserting a fact is not the same as one measurement of it.
//! The fact is also fragile in a way prose cannot protect: adding one dependency
//! that happens to pull `rustls` into the default feature set would restore the
//! C requirement silently, and the first symptom would be a macOS or Windows
//! gate dying with exit 101 on a host that has no cross compiler.
//!
//! # What this does NOT claim
//!
//! It says nothing about the test profile. `cargo test-all` runs
//! `--all-features` and therefore DOES require a C toolchain, which is a real
//! and declared limit: the residual HTTP harness needs `reqwest`. Measuring the
//! shipped profile is the promise that was made; measuring the test profile
//! would be measuring a promise nobody made.

use std::collections::BTreeSet;
use std::process::{Command, Stdio};

/// Crates whose presence implies a C compiler or CMake at build time.
///
/// `windows-sys`, `linux-raw-sys`, `js-sys` and `web-sys` are deliberately NOT
/// here: despite the `-sys` suffix they are pure-Rust binding crates with no
/// build script that invokes a compiler.
const C_TOOLCHAIN_CRATES: &[(&str, &str)] = &[
    ("aws-lc-sys", "vendored BoringSSL; drives cc and cmake"),
    (
        "openssl-sys",
        "links the system OpenSSL through a C build script",
    ),
    ("ring", "ships C and per-architecture assembly"),
    ("cmake", "invokes CMake from a build script by definition"),
    ("bindgen", "runs libclang over C headers"),
];

/// `cargo tree` output for the given feature selection.
fn tree(args: &[&str]) -> Option<String> {
    let out = Command::new(env!("CARGO"))
        .args(["tree", "-e", "normal,build", "--prefix", "none"])
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The profile users install must build without a C compiler.
#[test]
fn shipped_profile_pulls_no_c_toolchain() {
    let Some(text) = tree(&["--no-default-features", "--features", "chrome"]) else {
        // `cargo tree` needs the registry; offline hosts cannot answer.
        return;
    };

    let mut found: BTreeSet<String> = BTreeSet::new();
    for line in text.lines() {
        let name = line.split_whitespace().next().unwrap_or_default();
        if let Some((_, why)) = C_TOOLCHAIN_CRATES.iter().find(|(c, _)| *c == name) {
            found.insert(format!("{name} — {why}"));
        }
    }

    assert!(
        found.is_empty(),
        "`--no-default-features --features chrome` is the profile users install \
         and every cross-compile gate checks. It must build with rustc alone.\n\
         These dependencies require a C toolchain:\n{}\n\n\
         The cross gates for macOS and Windows run on a Linux host with no cross \
         compiler, so this does not merely slow the build — it makes those gates \
         impossible to run, and a gate that cannot run is indistinguishable from \
         one that was never written.",
        found.into_iter().collect::<Vec<_>>().join("\n")
    );
}

/// The ruler must be able to see a C crate, or it proves nothing.
///
/// `--all-features` genuinely pulls `aws-lc-sys` through `rustls`, so the
/// detector has a live positive case in this very repository. Without this, a
/// typo in `C_TOOLCHAIN_CRATES` would leave the test above passing forever.
#[test]
fn c_toolchain_detector_is_not_vacuous() {
    let Some(text) = tree(&["--all-features"]) else {
        return;
    };
    assert!(
        text.lines()
            .any(|l| l.split_whitespace().next() == Some("aws-lc-sys")),
        "`--all-features` is expected to pull `aws-lc-sys` through the residual \
         HTTP harness. If that is no longer true the boundary moved, and the \
         test above is passing for a reason nobody checked."
    );
}

/// Every declared C crate must carry a reason a reader can weigh.
#[test]
fn c_toolchain_entries_are_documented() {
    for (name, why) in C_TOOLCHAIN_CRATES {
        assert!(
            why.len() > 20,
            "{name} carries the reason {why:?}, too short to tell the next \
             reader why its presence means a C compiler is required"
        );
    }
}
