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

/// One row of the published agent-native capability matrix.
///
/// An agent that wants to reduce an envelope should read this instead of
/// probing with flags and collecting exit codes. `rows` names the array the
/// row operations act on; `null` means this envelope has none, so `--filter`,
/// `--sort`, `--dedupe-by`, `--limit` and `--count-only` are refused with
/// exit 2 and only `--fields`, `--truncate-content` and `--max-output-bytes`
/// apply.
///
/// # `discriminator_key`, not `discriminator`
///
/// v1.0.4 published this field as `discriminator` while the table behind it
/// held the discriminator VALUE (`doctor`, `schema_catalog`, `config_list`)
/// and every emitted envelope carries the KEY `type`. An agent that trusted
/// the matrix went looking for a key named `doctor` and found nothing. The
/// slot always meant the key — that is what the reduction code compares
/// against — so the table was corrected and the field renamed to say which of
/// the two it is. `schema` publishes the VALUE, under `discriminator`.
///
/// # `identity`
///
/// The keys `--truncate-content` will not shorten on this surface, because
/// their values are handed back to a program rather than read by a human.
#[derive(Debug, Serialize)]
struct SurfaceCapability {
    surface: &'static str,
    discriminator_key: Option<&'static str>,
    rows: Option<&'static str>,
    identity: &'static [&'static str],
    supports: Vec<&'static str>,
}

/// Envelope of the `commands` subcommand.
///
/// A struct rather than a `json!` literal so the discriminator is a
/// compiler-checked constant. It was a hand-typed `"type": "commands"` until
/// v1.0.4, one of the literals that let `probe_deep` drift from `probe-deep`
/// in a sibling surface.
#[derive(Debug, Serialize)]
struct CommandsReport {
    #[serde(rename = "type")]
    kind: crate::types::CommandsKind,
    version: &'static str,
    binary: &'static str,
    root: CommandNode,
    /// Which agent-native reductions each surface can honour (v1.0.4).
    agent_ops: Vec<SurfaceCapability>,
    /// Where each class of failure is written, and in what shape (v1.0.5).
    error_contract: Vec<ErrorChannel>,
    /// Where the agent-native flags must sit on the command line (v1.0.5).
    flag_position: FlagPosition,
}

/// One failure class: its channel, its schema and its discriminator.
#[derive(Debug, Clone, Copy, Serialize)]
struct ErrorChannel {
    /// Stable identifier for the class.
    class: &'static str,
    /// `stdout` or `stderr` — measured, not asserted.
    stream: &'static str,
    /// Published schema id this envelope conforms to.
    schema: &'static str,
    /// How a parser tells this envelope apart from a success payload.
    discriminator: &'static str,
    /// What produces it.
    raised_by: &'static str,
}

/// Where the eight agent-native flags are accepted.
#[derive(Debug, Clone, Copy, Serialize)]
struct FlagPosition {
    /// `before-subcommand` — they belong to the root argument group.
    accepted: &'static str,
    /// What clap does when they appear after the subcommand.
    otherwise: &'static str,
}

/// The three failure contracts this binary emits.
///
/// # Why this is published instead of merely being true
///
/// The 2026-08-10 audit measured all three and found them consistent
/// individually and undiscoverable together. An agent told "parse stdout for
/// `type == error`" sees the validation class, misses the usage class entirely
/// because it goes to stderr, and mis-handles the thin class because its
/// `error` is a STRING where the others carry an OBJECT.
///
/// Nothing here changes behaviour. Every channel below is what the binary
/// already did; the change is that an agent can now ask instead of discovering
/// it by collecting exit codes. `error_contract_matches_the_binary` in
/// `tests/e2e_cli.rs` drives one invocation per class and fails if a stream,
/// a shape or a discriminator ever moves.
const ERROR_CONTRACT: &[ErrorChannel] = &[
    ErrorChannel {
        class: "usage",
        stream: "stderr",
        schema: "classified-error-output",
        discriminator: "type=error, error.category=usage",
        raised_by: "clap: unknown flag, bad value, or an agent-native flag \
                    placed after the subcommand",
    },
    ErrorChannel {
        class: "validation",
        stream: "stdout",
        schema: "classified-error-output",
        discriminator: "type=error, error.category=validation",
        raised_by: "the product rejecting a well-formed request, e.g. \
                    `schema --name` with an unknown id",
    },
    ErrorChannel {
        class: "runtime",
        stream: "stdout",
        schema: "error-response",
        discriminator: "no `type` key; `error` is a STRING, not an object",
        raised_by: "a search that could not run, e.g. an unreadable \
                    `--queries-file`",
    },
];

/// Where the eight agent-native flags are accepted, published for agents.
///
/// Measured: `doctor --fields checks` exits 2 with `unexpected argument`. That
/// is a usage error indistinguishable at a glance from a refusal, which is how
/// an earlier audit concluded the binary had two refusal shapes. It has one;
/// the flag was simply in the wrong position, and nothing said so.
const FLAG_POSITION: FlagPosition = FlagPosition {
    accepted: "before-subcommand",
    otherwise: "clap rejects with exit 2 and `error.category=usage`; the eight \
                flags belong to the root argument group, so `--fields` must \
                precede `doctor`, not follow it",
};

/// Reductions meaningful on any JSON object envelope.
const ALWAYS_SUPPORTED: &[&str] = &["--fields", "--max-output-bytes"];
/// Reduction that needs prose to shorten; refused where every string is an identifier.
const CONTENT_SUPPORTED: &[&str] = &["--truncate-content"];
/// Reductions that need an array of rows to act on.
const ROW_SUPPORTED: &[&str] = &[
    "--filter",
    "--sort",
    "--dedupe-by",
    "--limit",
    "--count-only",
];

/// Snapshot the capability matrix for publication.
fn capability_matrix() -> Vec<SurfaceCapability> {
    crate::output::envelope_ops::SURFACES
        .iter()
        .map(|shape| {
            let mut supports: Vec<&'static str> = ALWAYS_SUPPORTED.to_vec();
            if shape.truncatable {
                supports.extend_from_slice(CONTENT_SUPPORTED);
            }
            if shape.rows.is_some() {
                supports.extend_from_slice(ROW_SUPPORTED);
            }
            SurfaceCapability {
                surface: shape.surface,
                discriminator_key: shape.discriminator,
                rows: shape.rows,
                identity: shape.identity,
                supports,
            }
        })
        .collect()
}

/// Emits a JSON tree of all clap commands for LLM/agent discovery.
pub fn execute_commands(_args: CommandsArgs) -> i32 {
    let cmd = RootArgs::command();
    let report = CommandsReport {
        kind: crate::types::CommandsKind::Commands,
        version: env!("CARGO_PKG_VERSION"),
        binary: env!("CARGO_PKG_NAME"),
        root: walk_command(&cmd),
        agent_ops: capability_matrix(),
        error_contract: ERROR_CONTRACT.to_vec(),
        flag_position: FLAG_POSITION,
    };
    let payload = match serde_json::to_value(&report) {
        Ok(v) => v,
        Err(err) => {
            output::emit_stderr(crate::i18n::error_msg(
                crate::i18n::Message::CommandsTreeSerializeFailed,
                &err,
            ));
            return exit_codes::GENERIC_ERROR;
        }
    };
    let shape = crate::output::envelope_ops::shape_for("commands")
        .copied()
        .unwrap_or_else(|| crate::output::envelope_ops::EnvelopeShape::rowless("commands", "type"));
    output::emit_envelope_or_refuse(payload, &shape, true, output::KeyPolicy::EnglishOnly)
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
        assert!(
            names.contains(&"deep-research"),
            "missing deep-research: {names:?}"
        );
    }
}
