// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: **declarative / pure load** (no fan-out).
// Sequential by design: one TOML/selectors load per process start (overhead of
// JoinSet would dominate). Multi-file writes live in `config_init` (GAP-PAR-025).
//! Configuration loading and validation (XDG selectors / user-agents).
//!
//! Thin facade over [`crate::config_init`] so the project layout matches
//! rules-rust-cli-com-clap (`src/config.rs`) without breaking existing
//! `config_init` call sites.

pub use crate::config_init::*;
