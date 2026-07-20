// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential utility (shell completion script). No fan-out — justified.
//! Handler for the `completions` subcommand.

use crate::cli::{CompletionsArgs, RootArgs};
use crate::error::exit_codes;
use clap::CommandFactory;

/// Generates shell completion scripts on stdout.
pub fn execute_completions(args: &CompletionsArgs) -> i32 {
    let mut cmd = RootArgs::command();
    clap_complete::generate(
        args.shell,
        &mut cmd,
        "duckduckgo-search-cli",
        &mut std::io::stdout(),
    );
    exit_codes::SUCCESS
}
