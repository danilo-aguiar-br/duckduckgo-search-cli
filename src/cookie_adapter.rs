// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (cookie jar wrapper for reqwest + JSON persistence)
//! v0.7.3 PR2 / v0.8.6 — Bridge between `reqwest::cookie::Jar` (the in-memory
//! cookie store used by `reqwest::Client::cookie_provider`) and the JSON file
//! format produced by [`crate::session_warmup::default_cookies_path`].
//!
//! `reqwest::cookie::Jar` implements `reqwest::cookie::CookieStore` natively.
//! However, `Jar` does not expose iteration over stored cookies. We persist
//! cookies by extracting the `Cookie` header via `CookieStore::cookies()` for
//! the `DuckDuckGo` domain and rebuild the jar from the file on each invocation
//! using `Jar::add_cookie_str()`.
//!
//! # Threat model
//!
//! - On-disk jar holds **session credentials** → mode `0o600`, atomic write.
//! - File content is untrusted → size cap + fail-open empty jar on parse error.
//! - Custom `--cookies-path` is validated like output paths (no `..` / system dirs).

use crate::error::CliError;
use crate::validation::{self, limits};
use serde::{Deserialize, Serialize};
use std::path::Path;
use validator::Validate;

/// One cookie row in the on-disk JSON projection (typed DTO — not `Value`).
///
/// Wire shape is stable for session warm-up: `name`, `value`, `domain`, and
/// optional `secure` (default `true`). Unknown fields are ignored (Must-Ignore).
/// After deserialize, [`Validate`] enforces length caps (GAP-SERDE-002).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Validate)]
struct CookieEntry {
    #[validate(length(
        min = 1,
        max = 256,
        message = "CookieEntry.name length out of range (1..=256)"
    ))]
    name: String,
    #[validate(length(
        max = 4096,
        message = "CookieEntry.value exceeds maximum length (4096)"
    ))]
    value: String,
    #[serde(default = "default_cookie_domain")]
    #[validate(length(
        min = 1,
        max = 253,
        message = "CookieEntry.domain length out of range (1..=253)"
    ))]
    domain: String,
    #[serde(default = "default_cookie_secure")]
    secure: bool,
}

fn default_cookie_domain() -> String {
    crate::endpoints::HOST_DDG.to_string()
}

const fn default_cookie_secure() -> bool {
    true
}

/// A `reqwest::cookie::Jar` paired with a backing JSON file path.
///
/// Constructed once per CLI invocation. The jar is the active cookie
/// store passed to `reqwest::Client::cookie_provider`; the file path is
/// the on-disk projection used by [`SessionWarmup`] to read and write
/// the persistent jar.
///
/// [`SessionWarmup`]: crate::session_warmup::default_cookies_path
#[derive(Clone, Debug)]
pub struct PersistentJar {
    /// The active in-memory jar shared with `reqwest::Client`.
    pub jar: std::sync::Arc<reqwest::cookie::Jar>,
    /// Path to the JSON file on disk. `None` disables persistence.
    pub path: Option<std::path::PathBuf>,
}

impl PersistentJar {
    /// Creates a new empty persistent jar at the given path.
    pub fn empty(path: Option<std::path::PathBuf>) -> Self {
        Self {
            jar: std::sync::Arc::new(reqwest::cookie::Jar::default()),
            path,
        }
    }

    /// Loads the JSON projection from disk into a fresh jar.
    ///
    /// Returns an empty jar if persistence is disabled, the file is
    /// missing, or the file is malformed. Malformed files are logged
    /// and treated as empty (so a corrupt jar does not break the CLI).
    pub fn load(path: Option<std::path::PathBuf>) -> Self {
        let jar = match path.as_ref() {
            Some(p) if p.exists() => {
                // Defensive size gate before allocating the full file in memory.
                match std::fs::metadata(p) {
                    Ok(meta) if meta.len() > crate::security::MAX_COOKIE_JAR_BYTES => {
                        tracing::warn!(
                            path = %p.display(),
                            size = meta.len(),
                            limit = crate::security::MAX_COOKIE_JAR_BYTES,
                            "cookie jar exceeds size limit — starting with empty jar"
                        );
                        reqwest::cookie::Jar::default()
                    }
                    Ok(_) => match std::fs::read(p) {
                        Ok(bytes) => match Self::parse_json_bytes(&bytes) {
                            Ok(jar) => jar,
                            Err(e) => {
                                tracing::warn!(
                                    error_class = "validation",
                                    frontier = "cookie",
                                    error = %e,
                                    path = %p.display(),
                                    "cookie jar is malformed — starting with empty jar"
                                );
                                reqwest::cookie::Jar::default()
                            }
                        },
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                path = %p.display(),
                                "failed to read cookie jar — starting with empty jar"
                            );
                            reqwest::cookie::Jar::default()
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            path = %p.display(),
                            "failed to stat cookie jar — starting with empty jar"
                        );
                        reqwest::cookie::Jar::default()
                    }
                }
            }
            _ => reqwest::cookie::Jar::default(),
        };
        Self {
            jar: std::sync::Arc::new(jar),
            path,
        }
    }

    /// Persists the current jar to disk in JSON format.
    ///
    /// On Unix, the file is written with mode `0o600` (owner read+write
    /// only) because it contains session credentials. Errors are logged
    /// but never fatal — a failed save does not break the current
    /// invocation.
    pub fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    tracing::warn!(
                        error = %e,
                        path = %parent.display(),
                        "failed to create cookie jar parent dir"
                    );
                    return;
                }
            }
        }
        let json = match Self::to_json(&self.jar) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize cookie jar");
                return;
            }
        };
        // GAP-WS-LIFECYCLE-001 L-10: atomic write for session credentials.
        if let Err(e) = crate::paths::atomic_write(path, json.as_bytes()) {
            tracing::warn!(
                error = %e,
                path = %path.display(),
                "failed to persist cookie jar to disk"
            );
        } else {
            // On Unix, tighten mode after write (session credentials).
            // Nested under `else` so Windows does not hit a needless trailing `return`.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o600);
                if let Err(e) = std::fs::set_permissions(path, perms) {
                    tracing::warn!(
                        error = %e,
                        "failed to set 0o600 permissions on cookie jar"
                    );
                }
            }
        }
    }

    /// Returns a shared reference suitable for `reqwest::Client::cookie_provider`.
    pub fn as_provider(&self) -> std::sync::Arc<reqwest::cookie::Jar> {
        self.jar.clone()
    }

    /// Performs a `GET <url>` warm-up request to populate session cookies.
    ///
    /// Returns silently on any error — the warm-up is best-effort and
    /// failure here must not break the real query.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the warm-up HTTP request itself fails (network
    /// error, TLS error, etc.). The pipeline logs and continues.
    pub async fn warm_up(&self, client: &reqwest::Client) -> Result<(), CliError> {
        let warmup_url = crate::endpoints::serp_base_url();
        client
            .get(&warmup_url)
            .send()
            .await
            .map_err(|e| CliError::http_with_source("warm-up request failed", e))?;
        Ok(())
    }

    /// Serializes the jar to a stable JSON projection.
    ///
    /// Since `reqwest::cookie::Jar` does not expose iteration, we extract
    /// cookies via `CookieStore::cookies()` for the `DuckDuckGo` domain and
    /// parse the combined `Cookie` header into individual `name=value` pairs.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `serde_json::to_string` fails.
    pub fn to_json(jar: &reqwest::cookie::Jar) -> serde_json::Result<String> {
        use reqwest::cookie::CookieStore;
        // Prefer effective SERP base (wiremock / env); fall back to compile-time origin.
        // Never panic: unparseable override → empty projection rather than abort.
        let ddg_url = match url::Url::parse(&crate::endpoints::serp_base_url())
            .or_else(|_| url::Url::parse(crate::endpoints::URL_DDG_ORIGIN))
        {
            Ok(u) => u,
            Err(_) => return serde_json::to_string(&Vec::<CookieEntry>::new()),
        };
        let cookies: Vec<CookieEntry> = match jar.cookies(&ddg_url) {
            Some(header_value) => {
                let header_str = header_value.to_str().unwrap_or("");
                header_str
                    .split("; ")
                    .filter_map(|pair| {
                        let mut parts = pair.splitn(2, '=');
                        let name = parts.next()?.trim();
                        let value = parts.next().unwrap_or("").trim();
                        if name.is_empty() {
                            return None;
                        }
                        Some(CookieEntry {
                            name: name.to_string(),
                            value: value.to_string(),
                            domain: default_cookie_domain(),
                            secure: default_cookie_secure(),
                        })
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        serde_json::to_string(&cookies)
    }

    /// Parses a JSON projection (as written by `to_json`) into a fresh
    /// `reqwest::cookie::Jar`. Each entry is converted to a cookie string
    /// and added via `Jar::add_cookie_str()`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the JSON is malformed (not an array of objects with
    /// string `name`/`value`). Entries with empty `name` are skipped.
    /// Parses cookie jar JSON from a UTF-8 string (strips BOM).
    ///
    /// Prefer [`Self::parse_json_bytes`] when the source is a file on disk.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the JSON is malformed. Entries that fail declarative
    /// [`Validate`] are skipped (fail-open, same policy as a corrupt jar).
    pub fn parse_json(content: &str) -> serde_json::Result<reqwest::cookie::Jar> {
        let content = content.strip_prefix('\u{feff}').unwrap_or(content);
        Self::parse_json_bytes(content.as_bytes())
    }

    /// Parses cookie jar JSON from raw bytes (GAP-SERDE-006: `from_slice`).
    ///
    /// # Errors
    ///
    /// Returns `Err` if the JSON is malformed.
    pub fn parse_json_bytes(bytes: &[u8]) -> serde_json::Result<reqwest::cookie::Jar> {
        // Strip UTF-8 BOM if a human-edited export introduced one.
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        let entries: Vec<CookieEntry> = serde_json::from_slice(bytes)?;
        let jar = reqwest::cookie::Jar::default();
        let _ = (
            limits::MAX_COOKIE_NAME_CHARS,
            limits::MAX_COOKIE_VALUE_CHARS,
            limits::MAX_COOKIE_DOMAIN_CHARS,
        );
        for entry in entries {
            if !validation::validate_or_log("cookie", &entry) {
                continue;
            }
            let scheme = if entry.secure { "https" } else { "http" };
            let url_str = format!("{scheme}://{}/", entry.domain);
            if let Ok(url) = url_str.parse::<url::Url>() {
                let cookie_str = format!("{}={}", entry.name, entry.value);
                jar.add_cookie_str(&cookie_str, &url);
            }
        }
        Ok(jar)
    }
}

/// Computes the default cookie jar file path under the XDG config directory.
///
/// Uses [`crate::platform::config_directory`] (XDG / Apple / APPDATA) — never
/// a hard-coded home layout such as `/home/user`.
///
/// # Errors
///
/// Returns `Err` if the platform config directory cannot be resolved.
pub fn default_cookies_path() -> Result<std::path::PathBuf, CliError> {
    let base = crate::platform::config_directory().ok_or_else(|| CliError::PathError {
        message: "could not determine user config directory for cookie jar".into(),
    })?;
    Ok(base.join(crate::session_warmup::DEFAULT_COOKIES_FILENAME))
}

/// Returns the XDG-relative cookie path for use with `Path::new`.
pub fn default_cookies_path_for(path: &Path) -> &Path {
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn empty_jar_serializes_to_empty_array() {
        let jar = reqwest::cookie::Jar::default();
        let json = PersistentJar::to_json(&jar).expect("serialize");
        assert_eq!(json, "[]");
    }

    #[test]
    fn malformed_json_yields_parse_error() {
        let result = PersistentJar::parse_json("not json {{{{");
        assert!(result.is_err(), "expected parse error, got {result:?}");
    }

    #[test]
    fn parse_json_strips_utf8_bom() {
        let with_bom = "\u{feff}[{\"name\":\"kl\",\"value\":\"br-pt\",\"domain\":\"duckduckgo.com\"}]";
        let jar = PersistentJar::parse_json(with_bom).expect("BOM-prefixed JSON must parse");
        let json = PersistentJar::to_json(&jar).expect("serialize");
        assert!(json.contains("kl"), "cookie name preserved after BOM strip");
        assert!(json.contains("br-pt"));
    }

    #[test]
    fn parse_json_skips_invalid_length_entries() {
        let raw = r#"[{"name":"","value":"x","domain":"duckduckgo.com"},{"name":"ok","value":"v","domain":"duckduckgo.com"}]"#;
        let _jar = PersistentJar::parse_json(raw).expect("parse");
        let oversize_name = "n".repeat(300);
        let raw2 = format!(
            r#"[{{"name":"{oversize_name}","value":"v","domain":"duckduckgo.com"}}]"#
        );
        let _jar2 = PersistentJar::parse_json(&raw2).expect("parse oversize");
    }

    #[test]
    fn parse_json_bytes_matches_str_path() {
        let raw = br#"[{"name":"a","value":"b","domain":"duckduckgo.com"}]"#;
        let _jar = PersistentJar::parse_json_bytes(raw).expect("bytes");
    }

    #[test]
        fn parse_json_uses_typed_cookie_entry_not_value_map() {
        // Typed DTO: extra fields are Must-Ignore; missing secure defaults to true.
        let raw = r#"[{"name":"vqd","value":"1","domain":"duckduckgo.com","extra_future":true}]"#;
        let jar = PersistentJar::parse_json(raw).expect("Must-Ignore unknown fields");
        let json = PersistentJar::to_json(&jar).expect("serialize");
        assert!(json.contains("vqd"));
        assert!(!json.contains("extra_future"));
    }

    #[test]
    fn round_trip_preserves_cookie() {
        let jar = reqwest::cookie::Jar::default();
        let url: url::Url = crate::endpoints::serp_base_url().parse().unwrap();
        jar.add_cookie_str("kl=br-pt", &url);
        let json = PersistentJar::to_json(&jar).expect("serialize");
        assert!(json.contains("kl"));
        assert!(json.contains("br-pt"));
    }

    #[test]
    fn persistent_jar_empty_creates_empty_jar() {
        let pj = PersistentJar::empty(None);
        assert!(
            pj.path.is_none(),
            "path should be None when constructed with None"
        );
        let json = PersistentJar::to_json(&pj.jar).expect("serialize empty");
        assert_eq!(json, "[]", "empty jar serializes to empty JSON array");
    }

    #[test]
    fn persistent_jar_load_missing_file_returns_empty() {
        let pj = PersistentJar::load(Some(PathBuf::from("/nonexistent/path/cookies.json")));
        let json = PersistentJar::to_json(&pj.jar).expect("serialize empty fallback");
        assert_eq!(json, "[]", "missing file should fallback to empty jar");
    }

    #[test]
    fn persistent_jar_save_writes_to_disk() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("cookies.json");
        let pj = PersistentJar::empty(Some(path.clone()));
        pj.save();
        assert!(path.exists(), "save() must create the cookies file");
        let content = std::fs::read_to_string(&path).expect("read file");
        assert_eq!(content, "[]", "empty jar written to disk as empty array");
    }

    #[test]
    fn default_cookies_path_returns_path_under_config_dir() {
        let path = default_cookies_path().expect("default path should resolve");
        let s = path.to_string_lossy();
        assert!(
            s.contains("duckduckgo-search-cli") && s.ends_with("cookies.json"),
            "default path must be <config>/duckduckgo-search-cli/cookies.json, got {s}"
        );
    }
}
