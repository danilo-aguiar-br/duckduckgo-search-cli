// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (SSOT validation limits + error mapping; one-shot startup)
//! Shared validation helpers for the serde → validator → domain pipeline.
//!
//! External file frontiers (`selectors.toml`, `user-agents.toml`, cookie jar JSON)
//! must: size-gate → deserialize → [`validator::Validate`] → domain use.
//! Local stderr diagnostics only (no product telemetry).

pub mod limits;

use crate::error::CliError;
use validator::ValidationErrors;

/// Log structured validation failures (local diagnostics only).
pub fn log_validation_errors(frontier: &str, errors: &ValidationErrors) {
    tracing::warn!(
        error_class = "validation",
        frontier,
        %errors,
        "declarative validation failed"
    );
}

/// Map [`ValidationErrors`] into [`CliError::InvalidConfig`] for strict loaders.
pub fn to_invalid_config(frontier: &str, errors: ValidationErrors) -> CliError {
    log_validation_errors(frontier, &errors);
    CliError::InvalidConfig {
        message: format!("{frontier} validation failed: {errors}"),
    }
}

/// Returns `true` when `instance` validates; otherwise logs and returns `false`
/// (resilient frontiers: cookie entries, individual UA rows).
pub fn validate_or_log<T: validator::Validate>(frontier: &str, instance: &T) -> bool {
    match instance.validate() {
        Ok(()) => true,
        Err(errors) => {
            log_validation_errors(frontier, &errors);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[derive(Debug, Validate)]
    struct Tiny {
        #[validate(length(min = 1, max = 4, message = "Tiny.name length out of range"))]
        name: String,
    }

    #[test]
    fn validate_or_log_accepts_valid() {
        let t = Tiny {
            name: "ab".to_string(),
        };
        assert!(validate_or_log("test", &t));
    }

    #[test]
    fn validate_or_log_rejects_invalid() {
        let t = Tiny {
            name: String::new(),
        };
        assert!(!validate_or_log("test", &t));
    }

    #[test]
    fn to_invalid_config_includes_frontier() {
        let t = Tiny {
            name: String::new(),
        };
        let err = t.validate().expect_err("empty");
        let cli = to_invalid_config("selectors", err);
        match cli {
            CliError::InvalidConfig { message } => {
                assert!(message.contains("selectors"), "{message}");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
