// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential utility (write config template once). No fan-out — justified.
//! Handler for the `init-config` subcommand.

use crate::cli::InitConfigArgs;
use crate::error::exit_codes;
use crate::output;
use crate::platform;

/// Executes `init-config` and prints a JSON report on stdout.
pub fn execute_init_config(args: &InitConfigArgs) -> i32 {
    crate::initialize_logging_for_command(0, false, false);
    platform::init();

    let report = match crate::config_init::initialize_config(args.force, args.dry_run) {
        Ok(r) => r,
        Err(err) => {
            tracing::error!(?err, "failed to initialize config");
            output::emit_stderr(crate::i18n::generic_error(&err));
            return exit_codes::GENERIC_ERROR;
        }
    };

    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            if let Err(err) = output::print_line_stdout(&json) {
                if output::is_broken_pipe(&err) {
                    return exit_codes::BROKEN_PIPE;
                }
                tracing::error!(?err, "failed to emit report");
                return exit_codes::GENERIC_ERROR;
            }
        }
        Err(err) => {
            tracing::error!(?err, "failed to serialize JSON report");
            return exit_codes::GENERIC_ERROR;
        }
    }

    let had_error = report.files.iter().any(|a| {
        matches!(
            a.action_taken,
            crate::config_init::ConfigFileAction::Error { .. }
        )
    });
    if had_error {
        return exit_codes::GENERIC_ERROR;
    }

    exit_codes::SUCCESS
}
