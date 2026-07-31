// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (CLI parsing via clap derive, zero runtime).
//! XDG `config` subcommand clap types (GAP-CLI-MOD-SPLIT / GAP-V101-XDG-001).

use clap::{Args, Subcommand as ClapSubcommand};

/// `config` subcommand — XDG persistence (GAP-V101-XDG-001).
#[derive(Debug, Clone, ClapSubcommand)]
pub enum ConfigCmd {
    /// Print the resolved XDG config directory path as JSON.
    Path(ConfigPathArgs),
    /// List all keys in `config.toml` as JSON.
    List(ConfigListArgs),
    /// Get one key (JSON object `{ "key", "value" }`).
    Get(ConfigGetArgs),
    /// Set one key (creates `config.toml` with mode 0600 when needed).
    Set(ConfigSetArgs),
    /// Unset (remove) one key from `config.toml`.
    Unset(ConfigUnsetArgs),
    /// Show merged effective values (CLI > XDG > defaults) for allowed keys.
    Effective(ConfigEffectiveArgs),
}

/// Arguments for `config path`.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigPathArgs {}

/// Arguments for `config list`.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigListArgs {}

/// Arguments for `config effective`.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigEffectiveArgs {}

/// Arguments for `config get` (GAP-E2E-51-003: positional **or** `--key`).
///
/// Accepted forms:
/// - `config get KEY`
/// - `config get --key KEY`
#[derive(Debug, Clone, Args)]
pub struct ConfigGetArgs {
    /// Configuration key as a positional argument (`config get KEY`).
    #[arg(value_name = "KEY", required_unless_present = "key_flag")]
    pub key_positional: Option<String>,

    /// Configuration key via flag (`config get --key KEY`).
    #[arg(
        long = "key",
        value_name = "KEY",
        required_unless_present = "key_positional"
    )]
    pub key_flag: Option<String>,
}

impl ConfigGetArgs {
    /// Resolved key from positional or `--key` (clap guarantees one is present).
    ///
    /// # Panics
    ///
    /// Panics if neither positional `KEY` nor `--key` is set (clap forbids that).
    #[must_use]
    pub fn key(&self) -> &str {
        self.key_flag
            .as_deref()
            .or(self.key_positional.as_deref())
            .expect("clap requires positional KEY or --key")
    }
}

/// Arguments for `config set` (GAP-E2E-51-003: positional **or** flags).
///
/// Accepted forms:
/// - `config set KEY VALUE`
/// - `config set --key KEY --value VALUE`
/// - mixed (`config set KEY --value VALUE`, `config set --key KEY VALUE`)
#[derive(Debug, Clone, Args)]
pub struct ConfigSetArgs {
    /// Configuration key as a positional argument.
    #[arg(value_name = "KEY", required_unless_present = "key_flag")]
    pub key_positional: Option<String>,

    /// Value as a positional argument.
    #[arg(value_name = "VALUE", required_unless_present = "value_flag")]
    pub value_positional: Option<String>,

    /// Configuration key via flag (`--key`).
    #[arg(
        long = "key",
        value_name = "KEY",
        required_unless_present = "key_positional"
    )]
    pub key_flag: Option<String>,

    /// Value via flag (`--value`).
    #[arg(
        long = "value",
        value_name = "VALUE",
        required_unless_present = "value_positional"
    )]
    pub value_flag: Option<String>,
}

impl ConfigSetArgs {
    /// Resolved key from positional or `--key`.
    ///
    /// # Panics
    ///
    /// Panics if neither positional `KEY` nor `--key` is set (clap forbids that).
    #[must_use]
    pub fn key(&self) -> &str {
        self.key_flag
            .as_deref()
            .or(self.key_positional.as_deref())
            .expect("clap requires positional KEY or --key")
    }

    /// Resolved value from positional or `--value`.
    ///
    /// # Panics
    ///
    /// Panics if neither positional `VALUE` nor `--value` is set (clap forbids that).
    #[must_use]
    pub fn value(&self) -> &str {
        self.value_flag
            .as_deref()
            .or(self.value_positional.as_deref())
            .expect("clap requires positional VALUE or --value")
    }
}

/// Arguments for `config unset` (positional **or** `--key`).
///
/// Accepted forms:
/// - `config unset KEY`
/// - `config unset --key KEY`
#[derive(Debug, Clone, Args)]
pub struct ConfigUnsetArgs {
    /// Configuration key as a positional argument.
    #[arg(value_name = "KEY", required_unless_present = "key_flag")]
    pub key_positional: Option<String>,

    /// Configuration key via flag (`--key`).
    #[arg(
        long = "key",
        value_name = "KEY",
        required_unless_present = "key_positional"
    )]
    pub key_flag: Option<String>,
}

impl ConfigUnsetArgs {
    /// Resolved key from positional or `--key`.
    ///
    /// # Panics
    ///
    /// Panics if neither positional `KEY` nor `--key` is set (clap forbids that).
    #[must_use]
    pub fn key(&self) -> &str {
        self.key_flag
            .as_deref()
            .or(self.key_positional.as_deref())
            .expect("clap requires positional KEY or --key")
    }
}
