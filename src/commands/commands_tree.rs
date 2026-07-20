// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential utility (print command tree). No fan-out — justified.
//! Handler for the `commands` subcommand — agent-ready command tree as JSON.

use crate::cli::{CommandsArgs, RootArgs};
use crate::error::exit_codes;
use crate::output;
use clap::CommandFactory;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct CommandNode {
    name: String,
    about: Option<String>,
    hidden: bool,
    subcommands: Vec<CommandNode>,
}

/// Emits a JSON tree of all clap commands for LLM/agent discovery.
pub fn execute_commands(_args: CommandsArgs) -> i32 {
    let cmd = RootArgs::command();
    let tree = walk_command(&cmd);
    let payload = serde_json::json!({
        "type": "commands",
        "version": env!("CARGO_PKG_VERSION"),
        "binary": env!("CARGO_PKG_NAME"),
        "root": tree,
    });
    match serde_json::to_string_pretty(&payload) {
        Ok(json) => match output::print_line_stdout(&json) {
            Ok(()) => exit_codes::SUCCESS,
            Err(err) if output::is_broken_pipe(&err) => exit_codes::BROKEN_PIPE,
            Err(err) => {
                output::emit_stderr(crate::i18n::error_msg(
                    crate::i18n::Message::CommandsTreeEmitFailed,
                    &err,
                ));
                exit_codes::GENERIC_ERROR
            }
        },
        Err(err) => {
            output::emit_stderr(crate::i18n::error_msg(
                crate::i18n::Message::CommandsTreeSerializeFailed,
                &err,
            ));
            exit_codes::GENERIC_ERROR
        }
    }
}

fn walk_command(cmd: &clap::Command) -> CommandNode {
    let name = cmd.get_name().to_string();
    let about = cmd.get_about().map(|s| s.to_string());
    let hidden = cmd.is_hide_set();
    let mut subcommands = Vec::new();
    for sub in cmd.get_subcommands() {
        subcommands.push(walk_command(sub));
    }
    CommandNode {
        name,
        about,
        hidden,
        subcommands,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_includes_agent_discovery_subcommands() {
        let cmd = RootArgs::command();
        let tree = walk_command(&cmd);
        let names: Vec<_> = tree.subcommands.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"commands"), "missing commands: {names:?}");
        assert!(names.contains(&"schema"), "missing schema: {names:?}");
        assert!(names.contains(&"doctor"), "missing doctor: {names:?}");
        assert!(names.contains(&"locale"), "missing locale: {names:?}");
        assert!(names.contains(&"deep-research"), "missing deep-research: {names:?}");
    }
}
