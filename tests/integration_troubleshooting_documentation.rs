// SPDX-License-Identifier: MIT OR Apache-2.0
//! GAP-NEW-001 v0.8.0 — regression tests for the timeout-cli Rust wrapper
//! troubleshooting documentation.
//!
//! Validates that:
//! 1. README.md contains a Troubleshooting section that mentions
//!    `/usr/bin/timeout` GNU coreutils as a workaround.
//! 2. README.pt-BR.md contains the same guidance in Portuguese.
//! 3. `scripts/detect-timeout-wrapper.sh` exists and has correct shebang.

use std::path::Path;

#[test]
fn readme_en_documents_timeout_wrapper_workaround() {
    let readme_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    let content = std::fs::read_to_string(&readme_path)
        .unwrap_or_else(|e| panic!("failed to read README.md: {e}"));
    assert!(
        content.contains("/usr/bin/timeout"),
        "README.md must mention /usr/bin/timeout GNU coreutils workaround"
    );
    assert!(
        content.contains("timeout-cli") || content.contains("crate Rust"),
        "README.md must reference the Rust timeout-cli crate"
    );
    assert!(
        content.contains("detect-timeout-wrapper.sh"),
        "README.md must reference scripts/detect-timeout-wrapper.sh"
    );
}

#[test]
fn readme_pt_br_documents_timeout_wrapper_workaround() {
    let readme_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.pt-BR.md");
    let content = std::fs::read_to_string(&readme_path)
        .unwrap_or_else(|e| panic!("failed to read README.pt-BR.md: {e}"));
    assert!(
        content.contains("/usr/bin/timeout"),
        "README.pt-BR.md must mention /usr/bin/timeout GNU coreutils workaround"
    );
    assert!(
        content.contains("timeout-cli") || content.contains("crate Rust"),
        "README.pt-BR.md must reference the Rust timeout-cli crate"
    );
    assert!(
        content.contains("detect-timeout-wrapper.sh"),
        "README.pt-BR.md must reference scripts/detect-timeout-wrapper.sh"
    );
}

#[test]
fn detect_timeout_wrapper_script_exists_with_correct_shebang() {
    let script_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("detect-timeout-wrapper.sh");
    let content = std::fs::read_to_string(&script_path)
        .unwrap_or_else(|e| panic!("failed to read script: {e}"));
    assert!(
        content.starts_with("#!/usr/bin/env bash"),
        "script must start with #!/usr/bin/env bash shebang, got: {}",
        content.lines().next().unwrap_or("")
    );
    assert!(
        content.contains("GNU coreutils") || content.contains("coreutils"),
        "script must mention coreutils"
    );
    // Verify executable bit
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&script_path)
            .unwrap_or_else(|e| panic!("failed to stat script: {e}"));
        let mode = metadata.permissions().mode();
        assert!(
            mode & 0o111 != 0,
            "script must be executable (mode={mode:o}), got {mode:o}"
        );
    }
}

/// Running under GNU coreutils `timeout` must NOT produce the wrapper warning.
///
/// # What the previous version of this test measured
///
/// Nothing. It called `std::env::set_var("CARGO_BIN_EXE_timeout", ...)` and
/// then asserted that `std::env::var` of the same key returned `Ok`, which is a
/// property of the standard library. Its own comment admitted it could not
/// reach the function it was named after. The detector it claimed to guard was
/// itself unable to fire, because cargo — not the `timeout` binary — is what
/// sets that variable, and cargo only sets it while building tests.
///
/// This runs the real binary under the real GNU `timeout` and reads the real
/// stderr, so it fails if the warning ever starts firing on the wrong parent.
#[test]
fn gnu_coreutils_timeout_parent_produces_no_wrapper_warning() {
    let gnu = std::path::Path::new("/usr/bin/timeout");
    if !gnu.exists() {
        // Nothing to assert about a host without GNU coreutils installed.
        return;
    }
    let bin = assert_cmd::cargo::cargo_bin("duckduckgo-search-cli");
    let out = std::process::Command::new(gnu)
        .arg("20")
        .arg(&bin)
        .arg("--version")
        .output()
        .expect("GNU timeout runs the binary");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("timeout-cli Rust crate detected"),
        "GNU coreutils timeout is the RECOMMENDED parent, so warning about it \
         would send the operator in a circle. stderr was:\n{stderr}"
    );
}

/// Running under the Rust `timeout` wrapper MUST produce the warning.
///
/// # Why the negative case alone is not enough
///
/// A detector that never fires passes every "it did not fire" assertion. The
/// version this replaced could only ever have been proven by such an
/// assertion, which is how it survived while being structurally unable to
/// detect anything. This is the other half: a real wrapper as the real parent,
/// and the warning read back out of real stderr.
///
/// Skipped when the wrapper is not installed, because the host, not the code,
/// decides whether that binary exists.
#[test]
#[cfg(target_os = "linux")]
fn rust_timeout_wrapper_parent_produces_the_warning() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let wrapper = std::path::Path::new(&home).join(".cargo/bin/timeout");
    if !wrapper.exists() {
        return;
    }
    let bin = assert_cmd::cargo::cargo_bin("duckduckgo-search-cli");
    let out = std::process::Command::new(&wrapper)
        .arg("20")
        .arg(&bin)
        // `locale` initializes logging; `--version` is short-circuited by clap
        // before any subscriber exists, so it can never carry this warning.
        .arg("locale")
        .output()
        .expect("wrapper runs the binary");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("timeout-cli Rust crate detected"),
        "the Rust wrapper shadows GNU coreutils and intercepts -v, which is the \
         whole reason this warning exists. stderr was:\n{stderr}"
    );
}

/// The wrapper advice must name a script that exists and is runnable.
#[test]
fn timeout_wrapper_warning_points_at_a_real_script() {
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/detect-timeout-wrapper.sh");
    assert!(
        script.exists(),
        "the runtime warning tells the operator to run {}, so it must exist",
        script.display()
    );
}
