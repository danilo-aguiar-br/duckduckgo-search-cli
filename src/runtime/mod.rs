// SPDX-License-Identifier: MIT OR Apache-2.0
//! Runtime configuration SSOT — resolve effective settings for one-shot runs.
//!
//! # Precedence
//! **CLI (explicit) > XDG `config.toml` > OS locale (lang/country) > FACTORY seed**
//!
//! # Layout
//! - [`keys`] — allowlist of XDG keys
//! - [`user`] — typed getters over the flat map
//! - [`persist`] — load/save `config.toml` under XDG
//! - [`apply`] — overlay XDG onto clap defaults
//! - [`build`] — `CliArgs` → pipeline [`crate::types::Config`]
//! - [`factory`] — documented factory seeds (no magic at call sites)
//! - [`validate`] — fail-closed `config set` validation
//!
//! Product code must not read process environment for knobs; use CLI flags and
//! `config set` only (anti-env / XDG rules).

pub mod apply;
pub mod build;
pub mod factory;
pub mod keys;
pub mod persist;
pub mod user;
pub mod validate;

pub use apply::{apply_user_config_to_cli_args, apply_user_config_to_root};
pub use build::build_config;
pub use factory::{
    FACTORY_DEFAULT_NUM_RESULTS, FACTORY_MAX_AUTO_PAGES, FACTORY_SERP_PAGE_SIZE,
};
pub use keys::ALLOWED_KEYS;
pub use persist::{
    config_file_path, load_config, load_runtime_user_config, save_config, CONFIG_FILE_NAME,
};
pub use user::UserConfig;
pub use validate::{ensure_allowed_key, validate_set_value};
