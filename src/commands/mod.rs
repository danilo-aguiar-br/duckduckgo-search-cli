// SPDX-License-Identifier: MIT OR Apache-2.0
//! Subcommand handlers (rules-rust-cli-com-clap: one module per subcommand).
//!
//! Dispatch lives in [`crate::run`]; each module owns the handler for its
//! clap [`crate::cli::Subcommand`] variant (plus man-page generation).
//!
//! ## Parallelism matrix (GAP-PAR-024 / Pass 28)
//!
//! | Subcommand | Fan-out | Bound | CPU offload | Notes |
//! |------------|---------|-------|-------------|-------|
//! | `buscar` (via root) | `parallel` / `content_fetch` / dual Chrome | `--parallel` | SERP + emit `run_cpu_bound` | work path |
//! | `buscar --stream` | JoinSet producer + consumer | channel + CPU sem | NDJSON/format async | GAP-PAR-040b |
//! | `deep-research` | multi-query JoinSet + dual aggregate + synth | `--parallel` + CPU sem | aggregate + synth + JSON | work path |
//! | `doctor` | none | n/a | n/a | local µs probes (N/A) |
//! | `init-config` | 2 file writes (`thread::scope`) | 2 | n/a | GAP-PAR-025 |
//! | `schema` / `completions` / `man` / `locale` / `commands` | none | n/a | n/a | pure emit (N/A) |

pub mod commands_tree;
pub mod completions;
pub mod config_cmd;
pub mod deep_research;
pub mod doctor;
pub mod init_config;
pub mod locale_cmd;
pub mod man;
pub mod schema_cmd;

pub use commands_tree::execute_commands;
pub use completions::execute_completions;
pub use config_cmd::execute_config;
pub use deep_research::execute_deep_research;
pub use doctor::execute_doctor;
pub use init_config::execute_init_config;
pub use locale_cmd::execute_locale;
pub use man::{execute_man, render_man_page, write_man_page};
pub use schema_cmd::execute_schema;
