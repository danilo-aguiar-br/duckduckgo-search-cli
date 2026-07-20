// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (User-Agent / browser profile selection)
//! Browser family detection, [`BrowserProfile`], and User-Agent pool selection.

use crate::error::CliError;
use crate::platform;
use crate::validation::{self, limits};
use rand::seq::{IndexedRandom, IteratorRandom};
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CACHE_CONTROL,
};
use serde::Deserialize;
use validator::Validate;

/// Accept-Encoding values we can actually decode (gzip/deflate features on;
/// brotli/`br` intentionally omitted — decoder removed in v0.8.6).
pub(super) const ACCEPT_ENCODING_SUPPORTED: &str = "gzip, deflate";

/// Built-in User-Agent list embedded in the binary as fallback when `config/user-agents.toml`
/// is not available.
///
/// v0.3.0 — POOL UPDATE (2026-04-14):
/// The old text browser UAs (Lynx, w3m, Links, `ELinks`) were REMOVED.
/// Empirically they still return HTTP 200, but `DuckDuckGo` serves DEGRADED HTML
/// for those agents: the layout lacks consistent `.result__snippet` classes,
/// forcing the extractor to fall back to Strategy 2 and return empty/incorrect snippets.
///
/// Final empirical validation (2026-04-14, real requests to /html/):
///   Chrome 146 Win/Mac/Linux → 200 OK ✓
///   Edge   145 Windows       → 200 OK ✓
///   Safari 17.6 macOS        → 200 OK ✓
///   Firefox 134 Linux        → 200 OK ✓
///   Firefox 134 Windows      → 202 ANOMALY ✗ (REMOVED)
///   Firefox 134 macOS        → 202 ANOMALY ✗ (REMOVED)
///
/// `DuckDuckGo` blocks Firefox desktop Win/Mac on the `/html/` endpoint
/// (anti-bot heuristic: UA claiming full browser without JS). Linux Firefox
/// passes because it is a minority desktop — DDG's filter is less aggressive.
pub(super) const USER_AGENTS_DEFAULT: &[&str] = &[
    // Chrome desktop (Windows / macOS / Linux) — abril 2026
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
    // Edge Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.3800.97",
    // Firefox desktop (Linux only — Win/Mac return HTTP 202 on /html/)
    "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
    // Safari macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15",
];

// ---------------------------------------------------------------------------
// Browser family
// ---------------------------------------------------------------------------

/// Detected browser family from the User-Agent string.
///
/// Used to generate family-specific headers (Client Hints, Accept, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserFamily {
    /// Google Chrome or Chromium derivatives (except Edge).
    #[default]
    Chrome,
    /// Mozilla Firefox.
    Firefox,
    /// Apple Safari (no Chrome indicator in the UA).
    Safari,
    /// Microsoft Edge (Chromium-based, contains `Edg/`).
    Edge,
}

// ---------------------------------------------------------------------------
// Perfil de browser
// ---------------------------------------------------------------------------

/// Complete browser profile derived from its User-Agent.
///
/// Encapsulates family, major version, and platform to generate correct
/// Sec-Fetch and Client Hints headers per family.
#[derive(Debug, Clone, Default)]
pub struct BrowserProfile {
    /// Detected browser family.
    pub family: BrowserFamily,
    /// Full User-Agent string.
    pub user_agent: String,
    /// Browser major version (e.g. 146 for Chrome 146).
    pub major_version: u32,
    /// Platform normalized for Client Hints (e.g. `"Windows"`, `"macOS"`, `"Linux"`).
    pub ua_platform: &'static str,
}

/// Detects the browser family from a User-Agent string.
///
/// Detection priority:
/// 1. `Edg/` → Edge
/// 2. `Chrome/` → Chrome
/// 3. `Firefox/` → Firefox
/// 4. `Safari/` without `Chrome/` → Safari
/// 5. Fallback → Firefox
///
/// # Exemplos
///
/// ```
/// use duckduckgo_search_cli::http::{detect_family, BrowserFamily};
/// assert_eq!(detect_family("Mozilla/5.0 ... Chrome/146 ... Edg/145"), BrowserFamily::Edge);
/// assert_eq!(detect_family("Mozilla/5.0 ... Chrome/146 ..."), BrowserFamily::Chrome);
/// ```
pub fn detect_family(ua: &str) -> BrowserFamily {
    if ua.contains("Edg/") {
        BrowserFamily::Edge
    } else if ua.contains("Chrome/") {
        BrowserFamily::Chrome
    } else if ua.contains("Firefox/") {
        BrowserFamily::Firefox
    } else if ua.contains("Safari/") {
        BrowserFamily::Safari
    } else {
        BrowserFamily::Firefox
    }
}

/// Extracts the major version of the browser from the UA and the detected family.
///
/// Supported patterns: `Chrome/146`, `Firefox/134`, `Version/17` (Safari), `Edg/145`.
/// Returns `0` if no pattern is found.
pub(super) fn extract_major_version(ua: &str, family: BrowserFamily) -> u32 {
    let prefix = match family {
        BrowserFamily::Chrome => "Chrome/",
        BrowserFamily::Firefox => "Firefox/",
        BrowserFamily::Safari => "Version/",
        BrowserFamily::Edge => "Edg/",
    };

    if let Some(pos) = ua.find(prefix) {
        let rest = &ua[pos + prefix.len()..];
        return rest
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .fold(0u32, |acc, c| acc * 10 + c.to_digit(10).unwrap_or(0));
    }
    0
}

/// Extracts the platform from the UA and normalizes it to the Client Hints format.
///
/// Mappings:
/// - `Windows NT` → `"Windows"`
/// - `Macintosh` → `"macOS"`
/// - Fallback → `"Linux"`
fn extract_ua_platform(ua: &str) -> &'static str {
    if ua.contains("Windows NT") {
        "Windows"
    } else if ua.contains("Macintosh") {
        "macOS"
    } else {
        "Linux"
    }
}

/// Builds a complete [`BrowserProfile`] from a User-Agent string.
///
/// Combines `detect_family`, `extract_major_version`, and `extract_ua_platform`.
///
/// The resulting profile automatically emits the correct `Sec-Fetch-*` and Client Hints
/// headers for the detected family — **do not inject custom Sec-Fetch or Accept
/// headers on top of this profile** (see rule R33 in `AGENT_RULES.md`).
///
/// # Exemplos
///
/// ```
/// use duckduckgo_search_cli::http::{create_browser_profile, BrowserFamily};
///
/// // Chrome UA → Chrome family, major version extracted, Linux platform
/// let ua_chrome = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
///                  (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
/// let profile = create_browser_profile(ua_chrome);
/// assert_eq!(profile.family, BrowserFamily::Chrome);
/// assert_eq!(profile.major_version, 146);
/// assert_eq!(profile.ua_platform, "Linux");
///
/// // Edge UA → Edge family (Sec-CH-UA* headers emitted automatically)
/// let ua_edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
///                (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0";
/// let profile_edge = create_browser_profile(ua_edge);
/// assert_eq!(profile_edge.family, BrowserFamily::Edge);
/// assert_eq!(profile_edge.ua_platform, "Windows");
/// ```
pub fn create_browser_profile(ua: &str) -> BrowserProfile {
    BrowserProfile::from_user_agent(ua)
}

impl BrowserProfile {
    /// Build a profile by parsing family/version/platform from a User-Agent string.
    #[must_use]
    pub fn from_user_agent(ua: &str) -> Self {
        let family = detect_family(ua);
        let major_version = extract_major_version(ua, family);
        let ua_platform = extract_ua_platform(ua);
        Self {
            family,
            user_agent: ua.to_string(),
            major_version,
            ua_platform,
        }
    }

    /// Generates the full initial headers for the first request of the session.
    ///
    /// Includes universal headers (Accept, Accept-Language, Accept-Encoding,
    /// Upgrade-Insecure-Requests, Sec-Fetch-*) and, for Chrome/Edge, Client Hints
    /// (Sec-CH-UA, Sec-CH-UA-Mobile, Sec-CH-UA-Platform, Cache-Control).
    ///
    /// # Arguments
    /// * `language` — BCP-47 language code (e.g. `"pt"`, `"en"`).
    /// * `country` — ISO 3166-1 alpha-2 country code (e.g. `"br"`, `"us"`).
    ///
    /// # Errors
    /// Returns an error if any header value contains invalid bytes.
    pub fn initial_headers(&self, language: &str, country: &str) -> Result<HeaderMap, CliError> {
        let mut headers = HeaderMap::new();

        // Accept by browser family
        let accept_value = match self.family {
            BrowserFamily::Chrome | BrowserFamily::Edge => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"
            }
            BrowserFamily::Firefox => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"
            }
            BrowserFamily::Safari => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
            }
        };
        headers.insert(ACCEPT, HeaderValue::from_static(accept_value));

        // Accept-Language with q-values
        let language_lower = language.to_ascii_lowercase();
        let country_upper = country.to_ascii_uppercase();
        let accept_language = if language_lower == "en" {
            "en-US,en;q=0.9".to_string()
        } else {
            format!("{language_lower}-{country_upper},{language_lower};q=0.9,en-US;q=0.8,en;q=0.7")
        };
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_str(&accept_language).map_err(|e| CliError::InvalidConfig {
                message: format!("Accept-Language contains invalid characters: {e}"),
            })?,
        );

        // Accept-Encoding — only encodings this binary can decode (no br).
        headers.insert(
            ACCEPT_ENCODING,
            HeaderValue::from_static(ACCEPT_ENCODING_SUPPORTED),
        );

        // Upgrade-Insecure-Requests
        headers.insert(
            HeaderName::from_static("upgrade-insecure-requests"),
            HeaderValue::from_static("1"),
        );

        // Sec-Fetch universais
        headers.insert(
            HeaderName::from_static("sec-fetch-dest"),
            HeaderValue::from_static("document"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-mode"),
            HeaderValue::from_static("navigate"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("none"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-user"),
            HeaderValue::from_static("?1"),
        );

        // Client Hints — exclusivo Chrome e Edge
        if matches!(self.family, BrowserFamily::Chrome | BrowserFamily::Edge) {
            let sec_ch_ua = match self.family {
                BrowserFamily::Edge => format!(
                    r#""Chromium";v="{v}", "Microsoft Edge";v="{v}", "Not-A.Brand";v="99""#,
                    v = self.major_version
                ),
                _ => format!(
                    r#""Chromium";v="{v}", "Google Chrome";v="{v}", "Not-A.Brand";v="99""#,
                    v = self.major_version
                ),
            };
            headers.insert(
                HeaderName::from_static("sec-ch-ua"),
                HeaderValue::from_str(&sec_ch_ua).map_err(|e| CliError::InvalidConfig {
                    message: format!("Sec-CH-UA contains invalid characters: {e}"),
                })?,
            );
            headers.insert(
                HeaderName::from_static("sec-ch-ua-mobile"),
                HeaderValue::from_static("?0"),
            );
            let platform_quoted = format!(r#""{}""#, self.ua_platform);
            headers.insert(
                HeaderName::from_static("sec-ch-ua-platform"),
                HeaderValue::from_str(&platform_quoted).map_err(|e| CliError::InvalidConfig {
                    message: format!("Sec-CH-UA-Platform contains invalid characters: {e}"),
                })?,
            );
            headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=0"));
        }

        Ok(headers)
    }

    /// Generates headers for pagination requests (same session, site already known).
    ///
    /// Difference from `construir_headers`: `Sec-Fetch-Site` becomes
    /// `same-origin` instead of `none`.
    pub fn pagination_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("sec-fetch-dest"),
            HeaderValue::from_static("document"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-mode"),
            HeaderValue::from_static("navigate"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("same-origin"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-user"),
            HeaderValue::from_static("?1"),
        );
        headers
    }
}

// ---------------------------------------------------------------------------
// Entry TOML do arquivo user-agents.toml externo
// ---------------------------------------------------------------------------

/// TOML entry from the external `user-agents.toml` file.
#[derive(Debug, Clone, Deserialize, Validate)]
struct ExternalTomlAgent {
    #[validate(length(
        min = 1,
        max = 512,
        message = "ExternalTomlAgent.ua length out of range (1..=512)"
    ))]
    ua: String,
    #[serde(default = "platform_any")]
    #[validate(length(
        min = 1,
        max = 32,
        message = "ExternalTomlAgent.platform length out of range (1..=32)"
    ))]
    platform: String,
    /// Optional field: browser family (`"chrome"`, `"firefox"`, `"safari"`, `"edge"`).
    /// If absent, the family is detected automatically in `create_browser_profile()`.
    #[serde(default)]
    #[allow(dead_code)]
    browser: Option<String>,
}

fn platform_any() -> String {
    "any".to_string()
}

#[derive(Debug, Clone, Deserialize)]
struct UserAgentsFile {
    #[serde(default)]
    agents: Vec<ExternalTomlAgent>,
}

// ---------------------------------------------------------------------------
// User-Agent loading
// ---------------------------------------------------------------------------

/// Loads the User-Agent list combining the external file (if it exists) with defaults.
///
/// If `corresponde_plataforma` is true, filters by current platform (`linux`/`macos`/`windows`)
/// OR `any`. Always returns a non-empty list — on failure, uses `USER_AGENTS_DEFAULT`.
pub fn load_user_agents(match_platform: bool) -> Vec<String> {
    let Some(path) = platform::user_agents_toml_path() else {
        tracing::info!("no config directory — using built-in UAs");
        return default_user_agents_vec();
    };

    if path.metadata().map(|m| m.len()).unwrap_or(0) > 1_048_576 {
        tracing::warn!(
            path = %path.display(),
            "user-agents.toml exceeds 1 MB limit — using built-in UAs"
        );
        return default_user_agents_vec();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) => {
            tracing::info!(
                path = %path.display(),
                ?err,
                "user-agents.toml not found — using built-in UAs"
            );
            return default_user_agents_vec();
        }
    };

    let file_data: UserAgentsFile = match toml::from_str(&content) {
        Ok(a) => a,
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                ?err,
                "user-agents.toml invalid — using built-in UAs"
            );
            return default_user_agents_vec();
        }
    };

    let current_platform = platform::platform_name();
    let _ = (
        limits::MAX_UA_CHARS,
        limits::MAX_UA_PLATFORM_CHARS,
    );
    let filtered: Vec<String> = file_data
        .agents
        .into_iter()
        .filter(|a| {
            // Etapa 2: declarative validation (GAP-SERDE-004).
            if !validation::validate_or_log("user-agents", a) {
                return false;
            }
            if !match_platform {
                return true;
            }
            a.platform == "any" || a.platform == current_platform
        })
        .map(|a| a.ua)
        .collect();

    if filtered.is_empty() {
        tracing::warn!("user-agents.toml produced no applicable UA — using defaults");
        return default_user_agents_vec();
    }

    tracing::info!(
        path = %path.display(),
        total = filtered.len(),
        match_platform,
        "User-Agents loaded from external user-agents.toml"
    );
    filtered
}

fn default_user_agents_vec() -> Vec<String> {
    use std::sync::LazyLock;
    static CACHE: LazyLock<Vec<String>> = LazyLock::new(|| {
        USER_AGENTS_DEFAULT
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    });
    CACHE.clone()
}
// ---------------------------------------------------------------------------
// User-Agent / BrowserProfile selection
// ---------------------------------------------------------------------------

/// Selects a random User-Agent from the built-in list.
pub fn select_user_agent() -> String {
    let mut rng = rand::rng();
    USER_AGENTS_DEFAULT
        .choose(&mut rng)
        .copied()
        .unwrap_or(USER_AGENTS_DEFAULT[0])
        .to_string()
}

/// Selects a random User-Agent from the provided list (useful after `load_user_agents`).
///
/// If the list is empty, falls back to the built-in default.
pub fn select_user_agent_from_list(list: &[String]) -> String {
    let mut rng = rand::rng();
    list.choose(&mut rng)
        .cloned()
        .unwrap_or_else(select_user_agent)
}

/// Selects a random [`BrowserProfile`] from the provided list.
///
/// Each string in the list is converted into a [`BrowserProfile`] via [`create_browser_profile`].
/// If the list is empty, creates a profile from the built-in default.
///
/// # Exemplos
///
/// ```
/// use duckduckgo_search_cli::http::{select_profile_from_list, BrowserFamily};
///
/// // Single Chrome UA list → always returns Chrome profile
/// let list = vec![
///     "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
///      (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"
///         .to_string(),
/// ];
/// let profile = select_profile_from_list(&list);
/// assert_eq!(profile.family, BrowserFamily::Chrome);
///
/// // Empty list → falls back to built-in default (returns a valid profile)
/// let profile_default = select_profile_from_list(&[]);
/// // family is one of the known BrowserFamily values
/// let _ = profile_default.family;
/// ```
pub fn select_profile_from_list(list: &[String]) -> BrowserProfile {
    let ua = select_user_agent_from_list(list);
    create_browser_profile(&ua)
}

/// Selects a [`BrowserProfile`] from the provided list using a deterministic seed.
///
/// When `seed` is `Some`, uses `SmallRng::seed_from_u64` for reproducible selection.
/// When `None`, delegates to [`select_profile_from_list`] (random).
pub fn select_profile_from_list_seeded(list: &[String], seed: Option<u64>) -> BrowserProfile {
    match seed {
        Some(s) => {
            use rand::SeedableRng;
            let mut rng = rand::rngs::StdRng::seed_from_u64(s);
            let ua = if list.is_empty() {
                USER_AGENTS_DEFAULT
                    .choose(&mut rng)
                    .copied()
                    .unwrap_or(USER_AGENTS_DEFAULT[0])
                    .to_string()
            } else {
                list.choose(&mut rng)
                    .cloned()
                    .unwrap_or_else(select_user_agent)
            };
            create_browser_profile(&ua)
        }
        None => select_profile_from_list(list),
    }
}

/// Selects a random User-Agent different from the one provided in `excluding` (when possible).
///
/// Used by the retry mechanism when HTTP 403 is detected — rotating UA reduces the chance
/// of a consistent identity profile. If all UAs in the list match `excluding`
/// (or the list has a single item), returns any UA from the list.
pub fn select_random_user_agent(excluding: Option<&str>) -> String {
    let mut rng = rand::rng();
    let chosen = USER_AGENTS_DEFAULT
        .iter()
        .filter(|ua| match excluding {
            Some(excl) => **ua != excl,
            None => true,
        })
        .choose(&mut rng);

    match chosen {
        Some(ua) => ua.to_string(),
        None => select_user_agent(),
    }
}

