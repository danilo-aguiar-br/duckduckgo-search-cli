// SPDX-License-Identifier: MIT OR Apache-2.0
//! Generate `duckduckgo-search-cli.1` from the runtime clap derive tree.
//!
//! Usage:
//! ```text
//! cargo run --bin gen_man -- /path/to/duckduckgo-search-cli.1
//! ```
//! When no path is given, writes to stdout.

fn main() {
    let bytes = match duckduckgo_search_cli::commands::render_man_page() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("gen_man: {e}");
            std::process::exit(1);
        }
    };

    let mut args = std::env::args().skip(1);
    if let Some(path) = args.next() {
        if let Err(e) = std::fs::write(&path, &bytes) {
            eprintln!("gen_man: write {path}: {e}");
            std::process::exit(1);
        }
    } else {
        use std::io::Write;
        let _ = std::io::stdout().write_all(&bytes);
    }
}
