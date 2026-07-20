// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (error module root — reexports)
//! Structured error codes as defined in specification section 14.3.
//!
//! The typed [`CliError`] enum maps each failure mode to a specific exit
//! code and JSON error code. Library consumers should match on the enum
//! variants; binary callers can use [`CliError::exit_code`] directly.
//!
//! # Module layout (Pass 43 SRP)
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`codes`] | Stable wire/agent string codes |
//! | [`exit_codes`] | Process exit integers (POSIX-oriented) |
//! | [`cli_error`] | `thiserror` enum + constructors + classification |

mod cli_error;
pub mod codes;
pub mod exit_codes;

pub use cli_error::CliError;
