// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (CLI parsing via clap derive, zero runtime).
//! Utility subcommand clap argument structs (GAP-CLI-MOD-SPLIT).

use clap::{ArgAction, Args};

use super::CompletionShell;

/// Arguments for the `man` subcommand.
#[derive(Debug, Clone, Args, Default)]
pub struct ManArgs {
    /// Optional path to write the man page (atomic). When omitted, writes roff to stdout.
    /// Uses `--file` (not `-o`) to avoid clashing with the global `--output` flag.
    #[arg(long = "file", value_name = "PATH")]
    pub file: Option<std::path::PathBuf>,
}

/// Arguments for the `locale` subcommand (UI language diagnostics).
#[derive(Debug, Clone, Args, Default)]
pub struct LocaleArgs {}

/// Arguments for the `commands` subcommand (agent-ready command tree).
#[derive(Debug, Clone, Args, Default)]
pub struct CommandsArgs {}

/// Arguments for the `schema` subcommand.
#[derive(Debug, Clone, Args, Default)]
pub struct SchemaArgs {
    /// Schema id to emit (e.g. `search-output`). When omitted, lists all ids.
    #[arg(long = "name", value_name = "ID")]
    pub name: Option<String>,
}

/// Arguments for the `doctor` subcommand.
#[derive(Debug, Clone, Args, Default)]
pub struct DoctorArgs {
    /// Exit non-zero when Chrome is missing, or when the detected Chrome major
    /// is wildly ahead of the chromiumoxide PDL baseline (GAP / OPP-DOCTOR-STRICT).
    ///
    /// JSON stdout shape stays agent-stable (additive fields only). Without
    /// this flag, doctor still reports `ok=false` when Chrome is missing, but
    /// does **not** fail solely for a far-ahead Chrome major.
    #[arg(long = "strict")]
    pub strict: bool,

    /// Run CAPTCHA/interstitial probe-deep calibration instead of the local
    /// environment report (GAP-E2E-V14-PROBE-DEEP-FLAG-ORDER).
    ///
    /// Equivalent to root `--probe-deep`. Accepted here so
    /// `doctor --probe-deep` parses without flag-order footguns.
    #[arg(long = "probe-deep", action = clap::ArgAction::SetTrue)]
    pub probe_deep: bool,
}

/// Arguments for the `completions` subcommand (MP-04).
#[derive(Debug, Clone, Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for (bash, zsh, fish, powershell, elvish).
    #[arg(value_enum)]
    pub shell: CompletionShell,
}

/// Arguments specific to the `init-config` subcommand.
#[derive(Debug, Clone, Args)]
pub struct InitConfigArgs {
    /// Overwrites existing files. Without this flag, files already present
    /// are kept intact.
    #[arg(long = "force", action = ArgAction::SetTrue)]
    pub force: bool,

    /// Simulates execution without writing any file to disk. Reports the actions
    /// that would be taken.
    #[arg(long = "dry-run", action = ArgAction::SetTrue)]
    pub dry_run: bool,
}
