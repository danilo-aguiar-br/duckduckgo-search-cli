//! v0.7.10 P9 — Example: pre-flight ghost-block detection.
//!
//! Demonstrates the recommended invocation pattern when the operator
//! suspects that the IP is on a bot-management list (e.g., shared
//! residential IP in Brazil, corporate VPN endpoint). The combination
//! of `--pre-flight` + `--allow-lite-fallback` ensures:
//!
//! 1. `pre-flight` runs the ghost-block detector locally on every
//!    response (zero extra HTTP cost).
//! 2. When the detector flags a ghost-block, `--allow-lite-fallback`
//!    triggers automatic Lite endpoint fallback.
//! 3. The envelope JSON reports `metadados.pre_flight_disparado: true`
//!    so observability tools can alert when this path is exercised.
//!
//! Run with:
//!   `cargo run --example pre_flight -- "rust async runtime 2026"`
//!
//! Expected output when the IP is blocked (likely in CI runners):
//!   `{ "metadados": { "pre_flight_disparado": true, ... }, ... }`
//!
//! When the IP is clean:
//!   `{ "metadados": { "pre_flight_disparado": false, ... }, ... }`

use std::process::{Command, Stdio};

fn main() {
    let query = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "rust async runtime 2026".to_string());

    println!("=== duckduckgo-search-cli v0.7.10 pre-flight example ===");
    println!("Query: {query}");
    println!();

    // CARGO_BIN_EXE_<name> is set at build time by Cargo; falls back to
    // PATH lookup when not running under `cargo run --example`.
    let bin = std::env::var("CARGO_BIN_EXE_duckduckgo-search-cli")
        .unwrap_or_else(|_| "duckduckgo-search-cli".to_string());

    // Recommended invocation pattern: pre-flight + allow-lite-fallback.
    // Both flags are global so they can be placed before or after the
    // subcommand.
    // GAP-PROC-005: explicit Stdio — inherit only stdout/stderr for demo UX;
    // never inherit stdin (agent/pipeline may already own it).
    let status = Command::new(&bin)
        .args([
            "--pre-flight",
            "--allow-lite-fallback",
            "-q",
            "-f",
            "json",
            "--num",
            "5",
            &query,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("failed to invoke duckduckgo-search-cli");

    println!();
    println!("=== exit status: {status} ===");

    if !status.success() {
        eprintln!(
            "duckduckgo-search-cli exited non-zero. \
             See the JSON envelope for the captcha detection reason."
        );
        std::process::exit(status.code().unwrap_or(1));
    }
}
