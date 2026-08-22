// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential utility (shell completion script). No fan-out — justified.
//! Handler for the `completions` subcommand.

use crate::cli::{CompletionsArgs, RootArgs};
use crate::error::exit_codes;
use crate::output;
use clap::CommandFactory;
use std::io::{self, Write};

/// Generates shell completion scripts on stdout.
///
/// # Broken pipe (GAP-REL-006)
///
/// `clap_complete::generate` takes a `Write` and `unwrap`s the write error
/// internally, so handing it `io::stdout()` turns `completions bash | head -1`
/// into a panic instead of the exit 141 this CLI contracts for. Rendering into
/// a buffer first keeps the failure in our hands, and the buffer is a shell
/// script — bounded by the argv surface, never by remote input.
pub fn execute_completions(args: &CompletionsArgs) -> i32 {
    let mut cmd = RootArgs::command();
    let mut script: Vec<u8> = Vec::new();
    clap_complete::generate(args.shell, &mut cmd, "duckduckgo-search-cli", &mut script);

    let mut out = io::stdout().lock();
    match out.write_all(&script).and_then(|()| out.flush()) {
        Ok(()) => exit_codes::SUCCESS,
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => exit_codes::BROKEN_PIPE,
        Err(e) => {
            output::emit_stderr(format!("failed to write completion script: {e}"));
            exit_codes::GENERIC_ERROR
        }
    }
}
