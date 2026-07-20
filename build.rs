//! Build script: embed git SHA for `--version` / `long_version`.
//!
//! Man pages are **not** mirrored here (historical drift). Generate them from
//! the runtime clap tree:
//! - `cargo run --bin gen_man -- duckduckgo-search-cli.1`
//! - `duckduckgo_search_cli::commands::render_man_page()`
//! - unit test `commands::man::tests::man_page_from_command_factory_*`

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    // Best-effort: also watch common branch tip files when present.
    println!("cargo:rerun-if-changed=.git/refs/heads/main");
    emit_git_sha();
}

fn emit_git_sha() {
    // GAP-PROC-003: explicit Stdio on build-time `git` (rules-rust-processos-externos).
    // Capture stdout only; never inherit stdin/stderr into cargo's console.
    use std::process::Stdio;
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_SHA={sha}");
}
