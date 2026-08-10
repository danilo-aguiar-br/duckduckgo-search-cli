// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential utility — XDG config.toml get/set (no fan-out).
//! `config` subcommand — persistent product settings via XDG (no product env).
//!
//! # Runtime apply (GAP-XDG-RUNTIME-001 / V28 SSOT)
//!
//! Resolution lives in [`crate::runtime`]: **CLI > XDG > FACTORY**.
//! This module is the CRUD surface (`config get/set/list/...`) plus
//! re-exports for backward-compatible call sites.

use crate::cli::{ConfigCmd, DEFAULT_GLOBAL_TIMEOUT, DEFAULT_SERP_COUNTRY, DEFAULT_SERP_LANG};
use crate::error::{exit_codes, CliError};
use crate::output;
use crate::platform;
use crate::runtime::{
    config_file_path, ensure_allowed_key, load_config, save_config, validate_set_value,
    CONFIG_FILE_NAME,
};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::path::Path;

pub use crate::runtime::{
    apply_user_config_to_cli_args, apply_user_config_to_root, load_runtime_user_config, UserConfig,
    ALLOWED_KEYS,
};

/// Dispatch `config` subcommands; returns process exit code.
pub fn execute_config(cmd: ConfigCmd) -> i32 {
    match run(cmd) {
        Ok(()) => exit_codes::SUCCESS,
        Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
        // v1.0.5: this arm used to hand-roll the refusal envelope and write it
        // with `print_line_stdout`, which is why `config` was the only family
        // whose failures were routable while `doctor`, `locale`, `commands`,
        // `schema` and `init-config` wrote prose to stderr and left stdout
        // empty. Same flag, same failure, two shapes. `output::refuse` is now
        // the single emitter for every surface, and it writes BOTH halves.
        Err(e) => output::refuse(&e),
    }
}

/// Emit a `config` envelope through the agent-native reduction boundary.
///
/// The five `config` envelopes had NO discriminator until v1.0.4, so the
/// published catalog could not route them and an agent had to recognise each
/// shape by hand. They also went out through `print_line_stdout`, which skipped
/// every reduction the caller asked for.
fn emit_config(
    mut payload: JsonValue,
    surface: &'static str,
    discriminator: crate::types::ConfigKind,
    rows: Option<&'static str>,
) -> Result<(), CliError> {
    if let Some(map) = payload.as_object_mut() {
        map.insert(
            "type".to_string(),
            serde_json::to_value(discriminator).map_err(|e| CliError::InvalidConfig {
                message: format!("failed to serialize discriminator: {e}"),
            })?,
        );
    }
    // The row key and the identity keys are declared once, in `SURFACES`,
    // which is also what `commands` publishes. Rebuilding the shape here left
    // two copies of the same fact free to drift, and only the operator who hit
    // the stale one would ever find out.
    let shape = crate::output::envelope_ops::shape_for(surface)
        .copied()
        .unwrap_or_else(|| match rows {
            Some(key) => {
                crate::output::envelope_ops::EnvelopeShape::with_rows(surface, key, "type")
            }
            None => crate::output::envelope_ops::EnvelopeShape::rowless(surface, "type"),
        });
    output::emit_envelope(payload, &shape, false)
}

fn run(cmd: ConfigCmd) -> Result<(), CliError> {
    use crate::types::ConfigKind;
    match cmd {
        ConfigCmd::Path(_) => {
            let dir = platform::config_directory().ok_or_else(|| CliError::InvalidConfig {
                message: "could not resolve XDG/OS config directory".to_string(),
            })?;
            let file = dir.join(CONFIG_FILE_NAME);
            let payload = serde_json::json!({
                "config_directory": dir.display().to_string(),
                "config_file": file.display().to_string(),
            });
            emit_config(payload, "config path", ConfigKind::ConfigPath, None)
        }
        ConfigCmd::List(_) => {
            let path = config_file_path()?;
            let cfg = load_config(&path)?;
            let payload = serde_json::json!({
                "config_file": path.display().to_string(),
                "values": cfg.values,
                "allowed_keys": ALLOWED_KEYS,
            });
            emit_config(
                payload,
                "config list",
                ConfigKind::ConfigList,
                Some("allowed_keys"),
            )
        }
        ConfigCmd::Get(args) => {
            let key = args.key();
            ensure_allowed_key(key)?;
            let path = config_file_path()?;
            let cfg = load_config(&path)?;
            let value = cfg.values.get(key).cloned();
            let payload = serde_json::json!({
                "key": key,
                "value": value,
                "present": value.is_some(),
            });
            emit_config(payload, "config get", ConfigKind::ConfigGet, None)
        }
        ConfigCmd::Set(args) => {
            let key = args.key();
            let value = args.value();
            ensure_allowed_key(key)?;
            validate_set_value(key, value)?;
            let path = config_file_path()?;
            let mut cfg = load_config(&path)?;
            cfg.values.insert(key.to_string(), value.to_string());
            save_config(&path, &cfg)?;
            let payload = serde_json::json!({
                "action": "set",
                "key": key,
                "value": value,
                "config_file": path.display().to_string(),
            });
            emit_config(payload, "config set", ConfigKind::ConfigMutation, None)
        }
        ConfigCmd::Unset(args) => {
            let key = args.key();
            ensure_allowed_key(key)?;
            let path = config_file_path()?;
            let mut cfg = load_config(&path)?;
            let removed = cfg.values.remove(key).is_some();
            if removed {
                save_config(&path, &cfg)?;
            }
            let payload = serde_json::json!({
                "action": "unset",
                "key": key,
                "removed": removed,
                "config_file": path.display().to_string(),
            });
            emit_config(payload, "config unset", ConfigKind::ConfigMutation, None)
        }
        ConfigCmd::Effective(_) => {
            // CLI flags are not present on this subcommand; report XDG vs built-in.
            let path = config_file_path()?;
            let cfg = load_config(&path)?;
            let payload = build_effective_payload(&path, &cfg);
            emit_config(
                payload,
                "config effective",
                ConfigKind::ConfigEffective,
                Some("allowed_keys"),
            )
        }
    }
}

/// Built-in default for an allowed key when no XDG value is set.
///
/// Returns `None` when the key has no static product default (e.g. optional
/// paths, UI lang negotiated from OS locale at runtime).
fn builtin_default_for_key(key: &str) -> Option<String> {
    match key {
        "default_global_timeout" => Some(DEFAULT_GLOBAL_TIMEOUT.to_string()),
        "default_vertical" => Some("all".to_string()),
        // Fetch is ON by default (`!no_fetch_content` in build_config).
        "fetch_content_default" => Some("true".to_string()),
        "default_lang" => Some(DEFAULT_SERP_LANG.to_string()),
        "default_country" => Some(DEFAULT_SERP_COUNTRY.to_string()),
        "default_max_sub_queries" => {
            Some(crate::deep_research::DEFAULT_MAX_SUB_QUERIES.to_string())
        }
        "default_fetch_content_cap" => Some(crate::cli::DEFAULT_FETCH_CONTENT_CAP.to_string()),
        "deep_research_allow_under_budget" => Some("false".to_string()),
        // v1.0.5 probe ceilings. Reporting `default: null` here while the
        // binary in fact falls back to a compiled number would make
        // `config effective` lie about the precedence it exists to explain.
        "probe_launch_timeout_seconds" => {
            Some(crate::types::bounded::PROBE_LAUNCH_TIMEOUT_SECONDS.to_string())
        }
        "probe_extract_timeout_seconds" => {
            Some(crate::types::bounded::PROBE_EXTRACT_TIMEOUT_SECONDS.to_string())
        }
        "probe_deep_launch_timeout_seconds" => {
            Some(crate::types::bounded::PROBE_DEEP_LAUNCH_TIMEOUT_SECONDS.to_string())
        }
        "probe_deep_extract_timeout_seconds" => {
            Some(crate::types::bounded::PROBE_DEEP_EXTRACT_TIMEOUT_SECONDS.to_string())
        }
        "budget_serp_seconds" => {
            Some(crate::types::bounded::BUDGET_SERP_SECONDS_ESTIMATE.to_string())
        }
        "budget_fetch_seconds" => {
            Some(crate::types::bounded::BUDGET_FETCH_SECONDS_ESTIMATE.to_string())
        }
        "budget_safety_margin_percent" => {
            Some(crate::types::bounded::BUDGET_SAFETY_MARGIN_PERCENT.to_string())
        }
        "budget_contention_low" => Some(crate::types::bounded::BUDGET_CONTENTION_LOW.to_string()),
        "budget_contention_high" => Some(crate::types::bounded::BUDGET_CONTENTION_HIGH.to_string()),
        "budget_contention_factor_mid_percent" => {
            Some(crate::types::bounded::BUDGET_CONTENTION_FACTOR_MID_PERCENT.to_string())
        }
        "budget_contention_factor_high_percent" => {
            Some(crate::types::bounded::BUDGET_CONTENTION_FACTOR_HIGH_PERCENT.to_string())
        }
        "deep_research_auto_contention_budget" => Some("true".to_string()),
        "deep_research_timeout_grace_seconds" => {
            Some(crate::types::bounded::DEEP_RESEARCH_TIMEOUT_GRACE_SECONDS.to_string())
        }
        "budget_profile" => Some("lab".to_string()),
        "default_parallelism" => Some(crate::cli::DEFAULT_PARALLELISM.to_string()),
        "chrome_session_retries" => Some(crate::error::DEFAULT_CHROME_SESSION_RETRIES.to_string()),
        "ui_lang" | "chrome_path" | "proxy_url" | "log_directive" => None,
        _ => None,
    }
}

/// Build the `config effective` JSON payload (GAP-E2E-51-018).
///
/// Precedence documented in the payload: **CLI flag > XDG > built-in default**.
/// This subcommand has no search CLI context, so `cli` is always null and
/// `effective` = XDG when present else built-in default (or null).
fn build_effective_payload(path: &Path, cfg: &UserConfig) -> JsonValue {
    let mut values = JsonMap::new();
    for &key in ALLOWED_KEYS {
        let default = builtin_default_for_key(key);
        let xdg = cfg
            .get(key)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let (effective, source) = match &xdg {
            Some(v) => (Some(v.clone()), "xdg"),
            None => match &default {
                Some(v) => (Some(v.clone()), "default"),
                None => (None, "unset"),
            },
        };
        values.insert(
            key.to_string(),
            serde_json::json!({
                "default": default,
                "xdg": xdg,
                "cli": null,
                "effective": effective,
                "source": source,
            }),
        );
    }
    serde_json::json!({
        "config_file": path.display().to_string(),
        "precedence": ["cli", "xdg", "default"],
        "note": "cli is null in `config effective` (no search flags); runtime search applies CLI > XDG > default",
        "allowed_keys": ALLOWED_KEYS,
        "values": values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn effective_payload_merges_xdg_over_defaults() {
        let mut cfg = UserConfig::default();
        cfg.values
            .insert("default_global_timeout".into(), "200".into());
        cfg.values.insert("default_lang".into(), "en".into());
        let path = PathBuf::from("/tmp/config.toml");
        let payload = build_effective_payload(&path, &cfg);
        let values = payload
            .get("values")
            .and_then(|v| v.as_object())
            .expect("values object");
        let timeout = values.get("default_global_timeout").expect("timeout key");
        assert_eq!(timeout.get("source").and_then(|v| v.as_str()), Some("xdg"));
        assert_eq!(
            timeout.get("effective").and_then(|v| v.as_str()),
            Some("200")
        );
        let lang = values.get("default_lang").expect("lang key");
        assert_eq!(lang.get("source").and_then(|v| v.as_str()), Some("xdg"));
        assert_eq!(lang.get("effective").and_then(|v| v.as_str()), Some("en"));
        let country = values.get("default_country").expect("country key");
        assert_eq!(
            country.get("source").and_then(|v| v.as_str()),
            Some("default")
        );
        assert_eq!(
            country.get("effective").and_then(|v| v.as_str()),
            Some(DEFAULT_SERP_COUNTRY)
        );
    }
}
