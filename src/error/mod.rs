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
//! | [`chrome_classify`] | Chrome transport taxonomy + zero-result exit (V12) |

pub mod chrome_classify;
mod cli_error;
pub mod codes;
pub mod exit_codes;

pub use chrome_classify::{
    chrome_cdp_error, chrome_launch_error, chrome_session_retries, chrome_timeout_error,
    exit_for_zero_results_with_errors, is_chrome_or_config_wire, is_chrome_session_transient,
    next_action_suggestion_for_chrome_error, normalize_chrome_transport_error,
    set_chrome_session_retries, DEFAULT_CHROME_SESSION_RETRIES, MAX_CHROME_SESSION_RETRIES,
};
pub use cli_error::CliError;
