// SPDX-License-Identifier: MIT OR Apache-2.0
//! Validate XDG keys/values at `config set` time (fail-closed).

use crate::error::CliError;

use super::keys::ALLOWED_KEYS;

/// Fail-closed allowlist check for `config set/get/unset` keys.
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when the operation fails.
pub fn ensure_allowed_key(key: &str) -> Result<(), CliError> {
    if ALLOWED_KEYS.contains(&key) {
        Ok(())
    } else {
        Err(CliError::InvalidConfig {
            message: format!(
                "unknown config key `{key}`; allowed: {}",
                ALLOWED_KEYS.join(", ")
            ),
        })
    }
}

/// Validate value shape for typed XDG keys at `config set` time.
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when the value is not valid for `key`.
pub fn validate_set_value(key: &str, value: &str) -> Result<(), CliError> {
    let v = value.trim();
    match key {
        "default_parallelism" => {
            let n: u32 = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!(
                    "default_parallelism must be an integer 1..={} (got {value:?})",
                    crate::types::bounded::MAX_PARALLELISM
                ),
            })?;
            if !(1..=crate::types::bounded::MAX_PARALLELISM).contains(&n) {
                return Err(CliError::InvalidConfig {
                    message: format!(
                        "default_parallelism must be in 1..={} (got {n})",
                        crate::types::bounded::MAX_PARALLELISM
                    ),
                });
            }
            Ok(())
        }
        "chrome_session_retries" => {
            let n: u32 = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!(
                    "chrome_session_retries must be an integer 0..={} (got {value:?})",
                    crate::error::MAX_CHROME_SESSION_RETRIES
                ),
            })?;
            if n > crate::error::MAX_CHROME_SESSION_RETRIES {
                return Err(CliError::InvalidConfig {
                    message: format!(
                        "chrome_session_retries must be in 0..={} (got {n})",
                        crate::error::MAX_CHROME_SESSION_RETRIES
                    ),
                });
            }
            Ok(())
        }
        "budget_profile" => {
            if crate::budget::is_known_budget_profile(v) {
                Ok(())
            } else {
                Err(CliError::InvalidConfig {
                    message: format!(
                        "budget_profile must be one of lab|desktop_contended|thin (got {value:?})"
                    ),
                })
            }
        }
        "default_sort" => {
            crate::output::SortSpec::parse(v)?;
            Ok(())
        }
        "default_dedupe_by" => {
            crate::output::DedupeBy::parse(v)?;
            Ok(())
        }
        "allow_no_warmup" | "linux_cgroup_enabled" => {
            match v.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" | "0" | "false" | "no" | "off" => Ok(()),
                _ => Err(CliError::InvalidConfig {
                    message: format!("{key} must be true|false (got {value:?})"),
                }),
            }
        }
        "linux_cgroup_memory_max_mb" => {
            let n: u64 = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!("linux_cgroup_memory_max_mb must be integer MB (got {value:?})"),
            })?;
            if n == 0 {
                return Err(CliError::InvalidConfig {
                    message: "linux_cgroup_memory_max_mb must be >= 1 when set".into(),
                });
            }
            Ok(())
        }
        "max_output_bytes" | "default_content_truncate" => {
            let n: u64 = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!("{key} must be a positive integer (got {value:?})"),
            })?;
            if n == 0 {
                return Err(CliError::InvalidConfig {
                    message: format!("{key} must be >= 1 (got 0)"),
                });
            }
            Ok(())
        }
        "default_timeout" => {
            let n: u64 = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!("default_timeout must be integer 1..={} (got {value:?})", crate::types::bounded::MAX_TIMEOUT_SECONDS),
            })?;
            if !(1..=crate::types::bounded::MAX_TIMEOUT_SECONDS).contains(&n) {
                return Err(CliError::InvalidConfig {
                    message: format!("default_timeout must be in 1..={} (got {n})", crate::types::bounded::MAX_TIMEOUT_SECONDS),
                });
            }
            Ok(())
        }
        "default_retries" => {
            let n: u32 = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!("default_retries must be integer 0..={} (got {value:?})", crate::types::bounded::MAX_RETRIES),
            })?;
            if n > crate::types::bounded::MAX_RETRIES {
                return Err(CliError::InvalidConfig {
                    message: format!("default_retries must be in 0..={} (got {n})", crate::types::bounded::MAX_RETRIES),
                });
            }
            Ok(())
        }
        "default_pages" => {
            let n: u32 = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!("default_pages must be integer 1..={} (got {value:?})", crate::types::bounded::MAX_PAGES),
            })?;
            if !(1..=crate::types::bounded::MAX_PAGES).contains(&n) {
                return Err(CliError::InvalidConfig {
                    message: format!("default_pages must be in 1..={} (got {n})", crate::types::bounded::MAX_PAGES),
                });
            }
            Ok(())
        }
        "default_num_results" => {
            let n: u32 = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!("default_num_results must be integer 1..={} (got {value:?})", crate::types::bounded::MAX_RESULT_COUNT),
            })?;
            if !(1..=crate::types::bounded::MAX_RESULT_COUNT).contains(&n) {
                return Err(CliError::InvalidConfig {
                    message: format!("default_num_results must be in 1..={} (got {n})", crate::types::bounded::MAX_RESULT_COUNT),
                });
            }
            Ok(())
        }
        "default_max_content_length" => {
            let n: usize = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!("default_max_content_length must be integer 1..={} (got {value:?})", crate::types::bounded::MAX_CONTENT_LENGTH),
            })?;
            if !(1..=crate::types::bounded::MAX_CONTENT_LENGTH).contains(&n) {
                return Err(CliError::InvalidConfig {
                    message: format!("default_max_content_length must be in 1..={} (got {n})", crate::types::bounded::MAX_CONTENT_LENGTH),
                });
            }
            Ok(())
        }
        "default_per_host_limit" => {
            let n: u32 = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!("default_per_host_limit must be integer 1..={} (got {value:?})", crate::types::bounded::MAX_PER_HOST_LIMIT),
            })?;
            if !(1..=crate::types::bounded::MAX_PER_HOST_LIMIT).contains(&n) {
                return Err(CliError::InvalidConfig {
                    message: format!("default_per_host_limit must be in 1..={} (got {n})", crate::types::bounded::MAX_PER_HOST_LIMIT),
                });
            }
            Ok(())
        }
        "default_cancel_grace_secs" => {
            let n: u64 = v.parse().map_err(|_| CliError::InvalidConfig {
                message: format!("default_cancel_grace_secs must be integer 1..=60 (got {value:?})"),
            })?;
            if !(1..=60).contains(&n) {
                return Err(CliError::InvalidConfig {
                    message: format!("default_cancel_grace_secs must be in 1..=60 (got {n})"),
                });
            }
            Ok(())
        }
        "wire_keys" => {
            if crate::output::WireKeys::parse(v).is_some() {
                Ok(())
            } else {
                Err(CliError::InvalidConfig {
                    message: format!("wire_keys must be en|pt (got {value:?})"),
                })
            }
        }
        _ => Ok(()),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_keys_include_default_lang_country() {
        assert!(ALLOWED_KEYS.contains(&"default_lang"));
        assert!(ALLOWED_KEYS.contains(&"default_country"));
    }

    #[test]
    fn allowed_keys_include_default_parallelism() {
        assert!(ALLOWED_KEYS.contains(&"default_parallelism"));
    }

    #[test]
    fn validate_set_default_parallelism_range() {
        assert!(validate_set_value("default_parallelism", "5").is_ok());
        assert!(validate_set_value("default_parallelism", "0").is_err());
        assert!(validate_set_value("default_parallelism", "99").is_err());
        assert!(validate_set_value("default_parallelism", "nope").is_err());
    }
}
