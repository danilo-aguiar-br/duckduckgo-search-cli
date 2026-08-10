//! Build script: embed git SHA for `--version` / `long_version`.
//!
//! Man pages are **not** mirrored here (historical drift). Generate them from
//! the runtime clap tree:
//! - `cargo run --bin gen_man -- duckduckgo-search-cli.1`
//! - `duckduckgo_search_cli::commands::render_man_page()`
//! - unit test `commands::man::tests::man_page_from_command_factory_*`

fn main() {
    // No `cargo:rerun-if-changed` on purpose — see `emit_git_sha`.
    emit_git_sha();
}

/// Emit `GIT_SHA`: the commit this code descends from, plus whether the tree
/// it was compiled from matched that commit.
///
/// # Why the narrow rerun list had to go
///
/// This script used to declare nine watched inputs — `build.rs`, six `cli`
/// modules, `.git/HEAD` and `.git/refs/heads/main`. Declaring ANY input tells
/// cargo to re-run the script only when one of them changes, so editing
/// `src/output/mod.rs` left the recorded identity untouched. That was harmless
/// while the identity was just the commit hash, which does not move when a file
/// is edited. It is fatal for the `-dirty` suffix: the tree would become dirty
/// and the binary would keep reporting itself clean, which is worse than not
/// reporting at all.
///
/// Declaring nothing makes cargo run this on every build. The cost is two cheap
/// `git` invocations; the crate itself is only recompiled when the emitted
/// string actually changes, so a clean tree still hits the cache.
///
/// # Why `--porcelain` and not `git describe --dirty`
///
/// `describe` needs a reachable tag and reports `-dirty` from the same tracked
/// diff, but it says nothing when no tag exists — and this repository releases
/// by tag AFTER the build being audited. `status --porcelain` answers the
/// question directly and identically on every host.
fn emit_git_sha() {
    let sha = match git_output(&["rev-parse", "--short=12", "HEAD"]) {
        Some(s) if !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit()) => s,
        _ => {
            // No repository, no git, or a detached state we cannot name. There
            // is nothing to be dirty RELATIVE TO, so no suffix is added.
            println!("cargo:rustc-env=GIT_SHA=unknown");
            return;
        }
    };
    let label = if working_tree_is_dirty() {
        format!("{sha}-dirty")
    } else {
        sha
    };
    println!("cargo:rustc-env=GIT_SHA={label}");
}

/// Whether the working tree differs from `HEAD`, including untracked files.
///
/// `--porcelain` prints one line per changed path and nothing at all when the
/// tree is clean, so emptiness is the whole test. A failed invocation is
/// treated as clean: claiming `-dirty` because `git` could not answer would
/// make the marker unreliable in the opposite direction.
fn working_tree_is_dirty() -> bool {
    git_output(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty())
}

/// Run `git` with explicit stdio and return trimmed stdout on success.
///
/// GAP-PROC-003: explicit `Stdio` on build-time `git`
/// (rules-rust-processos-externos). Capture stdout only; never inherit
/// stdin/stderr into cargo's console.
fn git_output(args: &[&str]) -> Option<String> {
    use std::process::Stdio;
    std::process::Command::new("git")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}
