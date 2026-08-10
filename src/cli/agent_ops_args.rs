// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (clap argument group). No I/O, no fan-out.
//! The agent-native reduction flags, declared ONCE and shared by flatten.
//!
//! # The duplication this removes
//!
//! These nine flags were written out twice: on `CliArgs` for the root/`buscar`
//! path, and again on `DeepResearchArgs` because clap gives each subcommand its
//! own argument namespace and the root declarations are not `global`. The two
//! copies were then reconciled by hand in `merge_deep_search_defaults`, with
//! eight `if let Some(x) = deep.x { out.x = Some(x) }` blocks, and a third time
//! in `runtime::build` when the flags were copied into [`crate::types::Config`].
//!
//! Three copies of one fact is three places for it to drift, and the drift is
//! invisible until someone passes a flag on the subcommand that the root spelled
//! differently. A `#[command(flatten)]` group is the mechanism clap provides for
//! exactly this, and the crate already uses it — `RootArgs` flattens `CliArgs`.

use clap::{ArgAction, Args};

/// Help section these flags appear under, matching the root declarations.
const HEADING_OUTPUT: &str = "OUTPUT";

/// Reduction flags shared by the root path and `deep-research`.
///
/// Flattened rather than made `global = true`: a global argument is accepted
/// anywhere, including on subcommands that cannot honour it, which would
/// recreate the accepted-and-ignored problem at the parser level.
#[derive(Debug, Clone, Default, Args, PartialEq, Eq)]
pub struct AgentOpsArgs {
    /// Project each result row to the listed wire fields (comma-separated).
    #[arg(long = "fields", value_name = "LIST", help_heading = HEADING_OUTPUT)]
    pub fields: Option<String>,

    /// Alias of `--fields` (agent-native / ETL-familiar name).
    #[arg(
        long = "select",
        value_name = "LIST",
        conflicts_with = "fields",
        help_heading = HEADING_OUTPUT
    )]
    pub select: Option<String>,

    /// Filter result rows after SERP extract (agent-native, no jq).
    #[arg(long = "filter", value_name = "EXPR", help_heading = HEADING_OUTPUT)]
    pub result_filter: Option<String>,

    /// Cap result rows **after** SERP extract + `--filter` (agent-native, no jq).
    #[arg(
        long = "limit",
        value_name = "N",
        value_parser = clap::value_parser!(u32).range(1..),
        help_heading = HEADING_OUTPUT
    )]
    pub result_limit: Option<u32>,

    /// Sort result rows after filter (agent-native; no jq).
    #[arg(long = "sort", value_name = "KEY[:DIR]", help_heading = HEADING_OUTPUT)]
    pub sort: Option<String>,

    /// Deduplicate rows by canonical URL after sort (agent-native; no jq).
    #[arg(long = "dedupe-by", value_name = "FIELD", help_heading = HEADING_OUTPUT)]
    pub dedupe_by: Option<String>,

    /// Emit only compact EN counts (`{"count":N,"web":W,"news":N}`) — no result rows.
    #[arg(long = "count-only", action = ArgAction::SetTrue, help_heading = HEADING_OUTPUT)]
    pub count_only: bool,

    /// Truncate each row `content` to N Unicode scalars (explicit anti-token).
    #[arg(
        long = "truncate-content",
        value_name = "N",
        value_parser = clap::value_parser!(u32).range(1..),
        help_heading = HEADING_OUTPUT
    )]
    pub truncate_content: Option<u32>,

    /// Fail-closed if the formatted stdout payload exceeds N bytes.
    #[arg(
        long = "max-output-bytes",
        value_name = "N",
        value_parser = clap::value_parser!(u64).range(1..),
        help_heading = HEADING_OUTPUT
    )]
    pub max_output_bytes: Option<u64>,
}

impl AgentOpsArgs {
    /// Let values set on the subcommand win over values set before it.
    ///
    /// Replaces eight hand-written `if let` blocks in
    /// `merge_deep_search_defaults`. Each one was individually trivial and
    /// collectively a place to forget a field: a knob added to one struct and
    /// not to the merge would parse, be stored, and never reach the pipeline.
    ///
    /// `count_only` is a flag rather than an option, so "unset" is `false` and
    /// only a `true` on the subcommand overrides — a subcommand cannot turn
    /// off a count the caller asked for before it.
    pub fn overlay(base: &mut Self, deep: &Self) {
        if deep.fields.is_some() {
            base.fields.clone_from(&deep.fields);
        }
        if deep.select.is_some() {
            base.select.clone_from(&deep.select);
        }
        if deep.result_filter.is_some() {
            base.result_filter.clone_from(&deep.result_filter);
        }
        if deep.result_limit.is_some() {
            base.result_limit = deep.result_limit;
        }
        if deep.sort.is_some() {
            base.sort.clone_from(&deep.sort);
        }
        if deep.dedupe_by.is_some() {
            base.dedupe_by.clone_from(&deep.dedupe_by);
        }
        if deep.count_only {
            base.count_only = true;
        }
        if deep.truncate_content.is_some() {
            base.truncate_content = deep.truncate_content;
        }
        if deep.max_output_bytes.is_some() {
            base.max_output_bytes = deep.max_output_bytes;
        }
    }

    /// The `--fields` value, honouring the `--select` alias.
    ///
    /// clap already refuses both at once, so taking the first `Some` is total.
    #[must_use]
    pub fn fields_or_select(&self) -> Option<String> {
        self.fields.clone().or_else(|| self.select.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_lets_the_subcommand_win_only_when_it_set_something() {
        let mut base = AgentOpsArgs {
            fields: Some("a".into()),
            result_limit: Some(5),
            ..Default::default()
        };
        let deep = AgentOpsArgs {
            result_limit: Some(9),
            ..Default::default()
        };
        AgentOpsArgs::overlay(&mut base, &deep);
        assert_eq!(
            base.fields.as_deref(),
            Some("a"),
            "unset field was clobbered"
        );
        assert_eq!(base.result_limit, Some(9), "set field did not win");
    }

    #[test]
    fn overlay_never_turns_count_only_back_off() {
        let mut base = AgentOpsArgs {
            count_only: true,
            ..Default::default()
        };
        AgentOpsArgs::overlay(&mut base, &AgentOpsArgs::default());
        assert!(
            base.count_only,
            "a default subcommand undid an explicit flag"
        );
    }

    #[test]
    fn select_is_read_when_fields_is_absent() {
        let args = AgentOpsArgs {
            select: Some("url".into()),
            ..Default::default()
        };
        assert_eq!(args.fields_or_select().as_deref(), Some("url"));
    }
}
