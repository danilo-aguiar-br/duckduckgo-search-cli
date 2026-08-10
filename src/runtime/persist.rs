// SPDX-License-Identifier: MIT OR Apache-2.0
//! XDG config.toml load/save (atomic write, 0600 on Unix).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::CliError;
use crate::paths;
use crate::platform;

use super::user::UserConfig;

/// File name under the XDG config directory.
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// Load XDG user config for the process (empty if missing/unreadable).
///
/// Never fails hard — corrupt files log nothing here; `config get` surfaces errors.
#[must_use]
pub fn load_runtime_user_config() -> UserConfig {
    match config_file_path() {
        Ok(path) => load_config(&path).unwrap_or_default(),
        Err(_) => UserConfig::default(),
    }
}

/// Resolve `config.toml` under the XDG/OS config directory.
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when the operation fails.
pub fn config_file_path() -> Result<PathBuf, CliError> {
    let dir = platform::config_directory().ok_or_else(|| CliError::InvalidConfig {
        message: "could not resolve XDG/OS config directory".to_string(),
    })?;
    Ok(dir.join(CONFIG_FILE_NAME))
}

/// Load a `config.toml` file into [`UserConfig`] (empty if missing).
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when the file cannot be read or parsed.
pub fn load_config(path: &Path) -> Result<UserConfig, CliError> {
    if !path.exists() {
        return Ok(UserConfig::default());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| CliError::PathError {
        message: format!("failed to read {}: {e}", path.display()),
    })?;
    if raw.trim().is_empty() {
        return Ok(UserConfig::default());
    }
    // Prefer table of strings; also accept flat toml via toml::Value.
    let value: toml::Value = toml::from_str(&raw).map_err(|e| CliError::InvalidConfig {
        message: format!("invalid config.toml: {e}"),
    })?;
    let mut values = BTreeMap::new();
    if let toml::Value::Table(table) = value {
        for (k, v) in table {
            let s = match v {
                toml::Value::String(s) => s,
                other => other.to_string(),
            };
            values.insert(k, s);
        }
    }
    Ok(UserConfig { values })
}

/// Atomically write [`UserConfig`] to `config.toml` (mode 0600 on Unix).
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when the operation fails.
pub fn save_config(path: &Path, cfg: &UserConfig) -> Result<(), CliError> {
    paths::create_parent_dirs(path)?;
    let mut table = toml::map::Map::new();
    for (k, v) in &cfg.values {
        table.insert(k.clone(), toml::Value::String(v.clone()));
    }
    let body = toml::to_string_pretty(&toml::Value::Table(table)).map_err(|e| {
        CliError::InvalidConfig {
            message: format!("failed to serialize config.toml: {e}"),
        }
    })?;
    paths::atomic_write(path, body.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
