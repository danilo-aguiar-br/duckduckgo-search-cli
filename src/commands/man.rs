// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential utility (render man page). No fan-out — justified.
//! Man-page generation from the **same** clap derive tree as the runtime CLI.
//!
//! Single source of truth: [`crate::cli::RootArgs`] via `CommandFactory`.
//! Avoids the historical drift of a hand-maintained builder in `build.rs`.

use crate::cli::{ManArgs, RootArgs};
use crate::error::{exit_codes, CliError};
use crate::output;
use clap::CommandFactory;
use std::io::{self, Write};
use std::path::Path;

/// Renders `duckduckgo-search-cli.1` roff bytes from [`RootArgs::command`].
///
/// # Errors
///
/// Returns I/O or clap_mangen render errors.
pub fn render_man_page() -> std::io::Result<Vec<u8>> {
    let cmd = RootArgs::command();
    let man = clap_mangen::Man::new(cmd);
    let mut buffer = Vec::new();
    man.render(&mut buffer)
        .map_err(|e| std::io::Error::other(format!("clap_mangen render failed: {e}")))?;
    Ok(buffer)
}

/// Writes the man page to `path` (parent dirs must exist).
///
/// # Errors
///
/// Returns I/O errors from render or write.
pub fn write_man_page(path: impl AsRef<Path>) -> std::io::Result<()> {
    let bytes = render_man_page()?;
    let mut f = std::fs::File::create(path)?;
    f.write_all(&bytes)?;
    Ok(())
}

/// `man` subcommand handler (GAP-E2E-48-004): never runs a SERP for the token `man`.
pub fn execute_man(args: &ManArgs) -> i32 {
    match render_man_page() {
        Ok(bytes) => {
            if let Some(path) = args.file.as_deref() {
                match crate::paths::validate_output_path(path)
                    .and_then(|_| crate::paths::atomic_write(path, &bytes))
                {
                    Ok(()) => exit_codes::SUCCESS,
                    Err(e) => {
                        output::emit_stderr(e.to_string());
                        exit_codes::INVALID_CONFIG
                    }
                }
            } else {
                let mut out = io::stdout().lock();
                match out.write_all(&bytes).and_then(|()| out.flush()) {
                    Ok(()) => exit_codes::SUCCESS,
                    Err(e) if e.kind() == io::ErrorKind::BrokenPipe => exit_codes::BROKEN_PIPE,
                    Err(e) => {
                        output::emit_stderr(format!("failed to write man page: {e}"));
                        exit_codes::GENERIC_ERROR
                    }
                }
            }
        }
        Err(e) => {
            output::emit_stderr(format!("failed to render man page: {e}"));
            let _ = CliError::InvalidConfig {
                message: e.to_string(),
            };
            exit_codes::GENERIC_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn man_page_from_command_factory_includes_agent_ready_contract() {
        let bytes = render_man_page().expect("render man");
        let text = String::from_utf8_lossy(&bytes);
        // Defaults / flags that previously drifted in build.rs mirror.
        assert!(
            text.contains("global-timeout") || text.contains("global\\-timeout"),
            "man must document --global-timeout"
        );
        assert!(
            text.contains("180"),
            "man must mention default global timeout 180"
        );
        assert!(
            text.contains("format") || text.contains("\\-f"),
            "man must document format flag"
        );
        assert!(
            text.contains("vertical") || text.contains("all"),
            "man must document vertical / all default surface"
        );
        assert!(
            text.contains("ui-lang") || text.contains("ui\\-lang"),
            "man must document --ui-lang (UI locale; not SERP -l/--lang)"
        );
        assert!(
            text.contains("locale"),
            "man must document locale subcommand"
        );
    }
}
