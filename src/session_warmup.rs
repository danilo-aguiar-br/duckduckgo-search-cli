// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (cookie jar path resolution and warm-up orchestration)
//! v0.7.3 PR2 — Session warm-up orchestration.
//!
//! Thin module that owns the CLI flags and default XDG paths for the
//! cookie jar. The actual JSON <-> `reqwest::cookie::Jar` conversion lives
//! in [`crate::cookie_adapter::PersistentJar`]; this module just
//! decides where the file lives, whether to persist at all, and
//! whether to perform a warm-up request.

use crate::error::CliError;
use std::path::PathBuf;

/// Default filename for the cookie jar inside the XDG config directory.
pub const DEFAULT_COOKIES_FILENAME: &str = "cookies.json";

/// Resolves the default cookie jar file path under the XDG config directory.
///
/// Uses [`crate::platform::config_directory`] (XDG / Apple / APPDATA) — never
/// a hard-coded home layout. Typical results:
/// - Unix: `$XDG_CONFIG_HOME/duckduckgo-search-cli/cookies.json`
/// - Windows: `%APPDATA%\duckduckgo-search-cli\cookies.json`
///
/// # Errors
///
/// Returns `Err` if the platform config directory cannot be resolved.
pub fn default_cookies_path() -> Result<PathBuf, CliError> {
    let base = crate::platform::config_directory().ok_or_else(|| CliError::PathError {
        message: "could not determine user config directory for cookie jar".into(),
    })?;
    Ok(base.join(DEFAULT_COOKIES_FILENAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cookies_path_lives_under_config_dir() {
        let path = default_cookies_path().expect("path");
        assert!(path.ends_with("duckduckgo-search-cli/cookies.json"));
    }
}
