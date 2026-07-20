// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential utility — XDG config.toml get/set (no fan-out).
//! `config` subcommand — persistent product settings via XDG (no product env).
//!
//! # Runtime apply (GAP-XDG-RUNTIME-001)
//!
//! [`load_runtime_user_config`] is called from `lib::run` so `config set`
//! actually changes search / deep-research / logging defaults. Precedence:
//! **CLI flag (non-default) > XDG `config.toml` > built-in defaults**.

use crate::cli::{
    CliArgs, CliVertical, ConfigCmd, RootArgs, DEFAULT_GLOBAL_TIMEOUT, DEFAULT_SERP_COUNTRY,
    DEFAULT_SERP_LANG,
};
use crate::error::{exit_codes, CliError};
use crate::output;
use crate::paths;
use crate::platform;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CONFIG_FILE_NAME: &str = "config.toml";

/// Allowed keys for `config set/get/unset` (SSOT; extend carefully).
pub const ALLOWED_KEYS: &[&str] = &[
    "ui_lang",
    "chrome_path",
    "proxy_url",
    "default_global_timeout",
    "default_vertical",
    "fetch_content_default",
    "log_directive",
    "default_lang",
    "default_country",
];

/// In-memory view of XDG `config.toml` for runtime apply.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    /// Flat string key/value map (allowed keys in [`ALLOWED_KEYS`]).
    #[serde(default, flatten)]
    pub values: BTreeMap<String, String>,
}

impl UserConfig {
    /// Lookup a raw string value.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Parse `default_global_timeout` when present and valid (`1..=3600`).
    #[must_use]
    pub fn default_global_timeout(&self) -> Option<u64> {
        let raw = self.get("default_global_timeout")?;
        let n: u64 = raw.trim().parse().ok()?;
        if (1..=crate::types::bounded::MAX_GLOBAL_TIMEOUT_SECONDS).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Parse `default_vertical` (`web`|`news`|`all`).
    #[must_use]
    pub fn default_vertical(&self) -> Option<CliVertical> {
        match self.get("default_vertical")?.trim().to_ascii_lowercase().as_str() {
            "web" => Some(CliVertical::Web),
            "news" => Some(CliVertical::News),
            "all" => Some(CliVertical::All),
            _ => None,
        }
    }

    /// Parse `fetch_content_default` (`true`/`false`/`1`/`0`/`on`/`off`).
    #[must_use]
    pub fn fetch_content_default(&self) -> Option<bool> {
        match self.get("fetch_content_default")?.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    /// Optional Chrome binary path from XDG.
    #[must_use]
    pub fn chrome_path(&self) -> Option<PathBuf> {
        let raw = self.get("chrome_path")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(PathBuf::from(raw))
        }
    }

    /// Optional proxy URL from XDG.
    #[must_use]
    pub fn proxy_url(&self) -> Option<String> {
        let raw = self.get("proxy_url")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw.to_string())
        }
    }

    /// Optional tracing filter directive (replaces product `RUST_LOG`).
    #[must_use]
    pub fn log_directive(&self) -> Option<&str> {
        let raw = self.get("log_directive")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    }

    /// Optional UI language override when CLI `--ui-lang` omitted.
    #[must_use]
    pub fn ui_lang(&self) -> Option<&str> {
        let raw = self.get("ui_lang")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    }

    /// Optional SERP language default (`-l` / `--lang`) from XDG.
    #[must_use]
    pub fn default_lang(&self) -> Option<&str> {
        let raw = self.get("default_lang")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    }

    /// Optional SERP country default (`-c` / `--country`) from XDG.
    #[must_use]
    pub fn default_country(&self) -> Option<&str> {
        let raw = self.get("default_country")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    }
}

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

/// Apply XDG defaults onto parsed CLI when the user left clap defaults.
///
/// Precedence: explicit CLI > XDG > built-in. Detection of “explicit CLI” uses
/// equality against known clap defaults (standard for global flags with
/// `default_value_t`).
pub fn apply_user_config_to_root(root: &mut RootArgs, xdg: &UserConfig) {
    // global_timeout: only replace when still at built-in default.
    if root.global_timeout_seconds == DEFAULT_GLOBAL_TIMEOUT {
        if let Some(n) = xdg.default_global_timeout() {
            root.global_timeout_seconds = n;
        }
    }

    // ui_lang: CLI Option None → XDG.
    if root.ui_lang.is_none() {
        if let Some(lang) = xdg.ui_lang() {
            root.ui_lang = Some(lang.to_string());
        }
    }

    apply_user_config_to_cli_args(&mut root.buscar, xdg);
}

/// Apply XDG defaults onto search CLI args (buscar / deep-research defaults).
pub fn apply_user_config_to_cli_args(args: &mut CliArgs, xdg: &UserConfig) {
    if args.chrome_path.is_none() {
        if let Some(p) = xdg.chrome_path() {
            args.chrome_path = Some(p);
        }
    }

    // Proxy: only when neither --proxy nor --no-proxy was set.
    if args.proxy.is_none() && !args.no_proxy {
        if let Some(url) = xdg.proxy_url() {
            args.proxy = Some(url);
        }
    }

    // Vertical default is All.
    if matches!(args.vertical, CliVertical::All) {
        if let Some(v) = xdg.default_vertical() {
            args.vertical = v;
        }
    }

    // Fetch default ON: only apply XDG false when user did not pass
    // --fetch-content / --no-fetch-content.
    if !args.fetch_content && !args.no_fetch_content {
        if let Some(want_fetch) = xdg.fetch_content_default() {
            if want_fetch {
                args.fetch_content = true;
            } else {
                args.no_fetch_content = true;
            }
        }
    }

    // SERP language/country: only replace when still at built-in clap defaults
    // (GAP-E2E-51-013). Explicit CLI `-l`/`-c` always wins.
    if args.language == DEFAULT_SERP_LANG {
        if let Some(lang) = xdg.default_lang() {
            args.language = lang.to_string();
        }
    }
    if args.country == DEFAULT_SERP_COUNTRY {
        if let Some(cc) = xdg.default_country() {
            args.country = cc.to_string();
        }
    }
}

fn config_file_path() -> Result<PathBuf, CliError> {
    let dir = platform::config_directory().ok_or_else(|| CliError::InvalidConfig {
        message: "could not resolve XDG/OS config directory".to_string(),
    })?;
    Ok(dir.join(CONFIG_FILE_NAME))
}

fn load_config(path: &Path) -> Result<UserConfig, CliError> {
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

fn save_config(path: &Path, cfg: &UserConfig) -> Result<(), CliError> {
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

fn ensure_allowed_key(key: &str) -> Result<(), CliError> {
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

/// Dispatch `config` subcommands; returns process exit code.
pub fn execute_config(cmd: ConfigCmd) -> i32 {
    match run(cmd) {
        Ok(()) => exit_codes::SUCCESS,
        Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
        Err(e) => {
            let payload = serde_json::json!({
                "erro": e.error_code(),
                "mensagem": format!("{e}"),
            });
            let _ = output::print_line_stdout(&payload.to_string());
            e.exit_code()
        }
    }
}

fn run(cmd: ConfigCmd) -> Result<(), CliError> {
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
            output::print_line_stdout(&payload.to_string())?;
            Ok(())
        }
        ConfigCmd::List(_) => {
            let path = config_file_path()?;
            let cfg = load_config(&path)?;
            let payload = serde_json::json!({
                "config_file": path.display().to_string(),
                "values": cfg.values,
                "allowed_keys": ALLOWED_KEYS,
            });
            output::print_line_stdout(&payload.to_string())?;
            Ok(())
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
            output::print_line_stdout(&payload.to_string())?;
            Ok(())
        }
        ConfigCmd::Set(args) => {
            let key = args.key();
            let value = args.value();
            ensure_allowed_key(key)?;
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
            output::print_line_stdout(&payload.to_string())?;
            Ok(())
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
            output::print_line_stdout(&payload.to_string())?;
            Ok(())
        }
        ConfigCmd::Effective(_) => {
            // CLI flags are not present on this subcommand; report XDG vs built-in.
            let path = config_file_path()?;
            let cfg = load_config(&path)?;
            let payload = build_effective_payload(&path, &cfg);
            output::print_line_stdout(&payload.to_string())?;
            Ok(())
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
    use crate::cli::{CliVertical, RootArgs};
    use clap::Parser;

    #[test]
    fn apply_timeout_only_when_cli_at_default() {
        let mut root = RootArgs::try_parse_from(["ddg", "q"]).expect("parse");
        assert_eq!(root.global_timeout_seconds, DEFAULT_GLOBAL_TIMEOUT);
        let mut xdg = UserConfig::default();
        xdg.values
            .insert("default_global_timeout".into(), "90".into());
        apply_user_config_to_root(&mut root, &xdg);
        assert_eq!(root.global_timeout_seconds, 90);

        root.global_timeout_seconds = 120;
        apply_user_config_to_root(&mut root, &xdg);
        assert_eq!(root.global_timeout_seconds, 120);
    }

    #[test]
    fn apply_proxy_and_vertical_and_fetch() {
        let mut root = RootArgs::try_parse_from(["ddg", "q"]).expect("parse");
        let mut xdg = UserConfig::default();
        xdg.values
            .insert("proxy_url".into(), "http://127.0.0.1:9".into());
        xdg.values.insert("default_vertical".into(), "web".into());
        xdg.values
            .insert("fetch_content_default".into(), "false".into());
        apply_user_config_to_cli_args(&mut root.buscar, &xdg);
        assert_eq!(root.buscar.proxy.as_deref(), Some("http://127.0.0.1:9"));
        assert!(matches!(root.buscar.vertical, CliVertical::Web));
        assert!(root.buscar.no_fetch_content);
    }

    #[test]
    fn cli_proxy_wins_over_xdg() {
        let mut root = RootArgs::try_parse_from([
            "ddg",
            "--proxy",
            "http://cli:1",
            "q",
        ])
        .expect("parse");
        let mut xdg = UserConfig::default();
        xdg.values
            .insert("proxy_url".into(), "http://xdg:1".into());
        apply_user_config_to_cli_args(&mut root.buscar, &xdg);
        assert_eq!(root.buscar.proxy.as_deref(), Some("http://cli:1"));
    }

    #[test]
    fn apply_default_lang_country_only_when_cli_at_builtin() {
        let mut root = RootArgs::try_parse_from(["ddg", "q"]).expect("parse");
        assert_eq!(root.buscar.language, DEFAULT_SERP_LANG);
        assert_eq!(root.buscar.country, DEFAULT_SERP_COUNTRY);
        let mut xdg = UserConfig::default();
        xdg.values.insert("default_lang".into(), "en".into());
        xdg.values.insert("default_country".into(), "us".into());
        apply_user_config_to_cli_args(&mut root.buscar, &xdg);
        assert_eq!(root.buscar.language, "en");
        assert_eq!(root.buscar.country, "us");

        // Explicit CLI wins.
        let mut root2 = RootArgs::try_parse_from(["ddg", "--lang", "es", "--country", "es", "q"])
            .expect("parse");
        apply_user_config_to_cli_args(&mut root2.buscar, &xdg);
        assert_eq!(root2.buscar.language, "es");
        assert_eq!(root2.buscar.country, "es");
    }

    #[test]
    fn allowed_keys_include_default_lang_country() {
        assert!(ALLOWED_KEYS.contains(&"default_lang"));
        assert!(ALLOWED_KEYS.contains(&"default_country"));
    }

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
        let timeout = values
            .get("default_global_timeout")
            .expect("timeout key");
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
