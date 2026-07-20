// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (locale detection + UI message catalog)
//! Runtime UI localization for human-facing stderr messages.
//!
//! ## Design (rules-rust multi-idioma + agent CLI constraints)
//!
//! - **UI language** is separate from DuckDuckGo search `--lang` / `-l`
//!   (SERP `kl` parameter). Override UI with `--ui-lang` or the XDG
//!   `ui-lang` preference file (GAP-SCRAPE-R2-014: no product env).
//! - **Machine stdout** (JSON / NDJSON / schema / doctor) stays locale-stable
//!   and is **not** translated (agent contract).
//! - **Technical logs** (`tracing`) stay English for grep/stability.
//! - MVP locales: **`en`** + **`pt-BR`** only (100% parity via exhaustive match).
//! - Optional top-20 languages are **not** compiled in by default
//!   (`N/A-I18N-*` / future Cargo features).
//!
//! ## Resolution precedence (4 layers)
//!
//! 1. `--ui-lang` flag  
//! 2. Persisted preference (`ui-lang` under XDG config dir)  
//! 3. OS locale via [`sys_locale::get_locale`]  
//! 4. Default [`Language::En`]
//!
//! Call [`initialize`] once after clap parse (and after [`crate::platform::init`]).

mod en;
mod language;
mod message;
mod pt_br;

pub use language::{Language, TextDirection};
pub use message::Message;

use std::sync::OnceLock;
// fluent-langneg 0.14 re-exports `icu_locid::LanguageIdentifier` (not unic-langid).
// We keep `unic-langid` for structured parse/display and convert at negotiate time.
use unic_langid::LanguageIdentifier as UnicLanguageIdentifier;

/// Filename under the config directory for a persisted UI language preference.
pub const UI_LANG_PREFERENCE_FILE: &str = "ui-lang";

/// Global resolved UI language for this process (immutable after init).
static RESOLVED: OnceLock<ResolvedLocale> = OnceLock::new();

/// Snapshot of locale resolution published at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocale {
    /// Negotiated UI language for human messages.
    pub language: Language,
    /// Which precedence layer won.
    pub source: LocaleSource,
    /// Raw OS locale string when layer 4 was consulted (`None` if unused or unavailable).
    pub system_raw: Option<String>,
}

/// Precedence layer that selected the UI language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LocaleSource {
    /// `--ui-lang` on the command line.
    Flag,
    /// Persisted `ui-lang` file under the config directory.
    Persisted,
    /// OS locale via `sys-locale`.
    System,
    /// Hard default when nothing else matched.
    Default,
}

impl LocaleSource {
    /// Stable wire / diagnostic label (English, machine-readable).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Persisted => "persisted",
            Self::System => "system",
            Self::Default => "default",
        }
    }
}

/// Returns the process-wide resolved locale, or English default before init.
pub fn resolved() -> ResolvedLocale {
    RESOLVED.get().cloned().unwrap_or(ResolvedLocale {
        language: Language::En,
        source: LocaleSource::Default,
        system_raw: None,
    })
}

/// Shortcut for the resolved [`Language`].
pub fn language() -> Language {
    resolved().language
}

/// Initializes the process UI locale exactly once.
///
/// Safe to call multiple times: subsequent calls are no-ops (first wins).
/// Emits a `tracing::debug` when OS detection fails or an override is invalid.
pub fn initialize(flag_ui_lang: Option<&str>) {
    let _ = RESOLVED.get_or_init(|| resolve(flag_ui_lang));
}

/// Pure resolution (no global state). Prefer [`initialize`] at process start;
/// use this in tests.
pub fn resolve(flag_ui_lang: Option<&str>) -> ResolvedLocale {
    // Layer 1 — flag
    if let Some(raw) = flag_ui_lang.map(str::trim).filter(|s| !s.is_empty()) {
        match Language::parse(raw) {
            Some(language) => {
                return ResolvedLocale {
                    language,
                    source: LocaleSource::Flag,
                    system_raw: None,
                };
            }
            None => {
                tracing::warn!(
                    value = raw,
                    "invalid --ui-lang value; falling through to next locale layer"
                );
            }
        }
    }

    // Layer 2 — persisted preference (XDG)
    if let Some(raw) = read_persisted_ui_lang() {
        match Language::parse(&raw) {
            Some(language) => {
                return ResolvedLocale {
                    language,
                    source: LocaleSource::Persisted,
                    system_raw: None,
                };
            }
            None => {
                tracing::warn!(
                    value = %raw,
                    "invalid persisted ui-lang preference; ignoring"
                );
            }
        }
    }

    // Layer 3 — OS locale via sys-locale (never read LANG / product env)
    let system_raw = sys_locale::get_locale();
    if let Some(ref raw) = system_raw {
        if let Some(language) = negotiate_system_locale(raw) {
            return ResolvedLocale {
                language,
                source: LocaleSource::System,
                system_raw,
            };
        }
        tracing::debug!(
            system_locale = %raw,
            "OS locale not in available UI bundle; using default"
        );
    } else {
        tracing::debug!("sys-locale returned None; using default UI language");
    }

    // Layer 5 — deterministic default (English: agent / log stability)
    ResolvedLocale {
        language: Language::En,
        source: LocaleSource::Default,
        system_raw,
    }
}

/// Negotiates a raw OS locale string against the MVP bundle (`en`, `pt-BR`).
fn negotiate_system_locale(raw: &str) -> Option<Language> {
    let requested_unic = parse_os_locale(raw)?;
    let requested = to_fluent_langid(&requested_unic)?;
    let available: Vec<fluent_langneg::LanguageIdentifier> = Language::AVAILABLE
        .iter()
        .filter_map(|l| to_fluent_langid(&l.language_identifier()))
        .collect();

    // fluent-langneg: pick best match among available identifiers.
    let matched = fluent_langneg::negotiate_languages(
        &[requested],
        &available,
        None,
        fluent_langneg::NegotiationStrategy::Filtering,
    );
    matched.first().and_then(|id| {
        let s = id.to_string();
        Language::parse(&s)
    })
}

/// Converts a `unic-langid` identifier into fluent-langneg's `icu_locid` type.
fn to_fluent_langid(
    id: &UnicLanguageIdentifier,
) -> Option<fluent_langneg::LanguageIdentifier> {
    id.to_string()
        .parse::<fluent_langneg::LanguageIdentifier>()
        .ok()
}

/// Parses OS locale strings such as `pt_BR.UTF-8`, `pt-BR`, `C`, `POSIX`.
fn parse_os_locale(raw: &str) -> Option<UnicLanguageIdentifier> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("C") || trimmed.eq_ignore_ascii_case("POSIX")
    {
        return None;
    }
    // Drop encoding / modifier: `pt_BR.UTF-8@euro` → `pt_BR`
    let base = trimmed
        .split(['.', '@'])
        .next()
        .unwrap_or(trimmed)
        .replace('_', "-");
    base.parse::<UnicLanguageIdentifier>().ok()
}

/// Reads the optional persisted UI language preference (best-effort).
fn read_persisted_ui_lang() -> Option<String> {
    let path = crate::platform::config_directory()?.join(UI_LANG_PREFERENCE_FILE);
    let contents = std::fs::read_to_string(path).ok()?;
    let line = contents.lines().next()?.trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_owned())
    }
}

/// Formats a [`Message`] in the process-resolved language.
pub fn t(msg: Message) -> &'static str {
    msg.text(language())
}

/// Formats a configuration error line for human stderr.
pub fn configuration_error(detail: impl std::fmt::Display) -> String {
    format!("{}: {detail:#}", t(Message::ConfigurationErrorPrefix))
}

/// Formats a generic error line for human stderr.
pub fn generic_error(detail: impl std::fmt::Display) -> String {
    format!("{}: {detail:#}", t(Message::ErrorPrefix))
}

/// Formats a global-timeout human stderr line.
pub fn global_timeout_exceeded(seconds: u64) -> String {
    Message::GlobalTimeoutExceeded
        .format(language(), &[("seconds", &seconds.to_string())])
}

/// Formats the clap global-flag placement tip.
pub fn flag_must_precede_subcommand(flag: &str) -> String {
    Message::FlagMustPrecedeSubcommand.format(language(), &[("flag", flag)])
}

/// Formats a deep-research global-timeout human stderr line.
pub fn deep_research_timeout_exceeded(seconds: u64) -> String {
    Message::DeepResearchTimeoutExceeded
        .format(language(), &[("seconds", &seconds.to_string())])
}

/// Formats `msg` with placeholders in the process-resolved language.
pub fn tf(msg: Message, pairs: &[(&str, &str)]) -> String {
    msg.format(language(), pairs)
}

/// Formats `msg` with a single `{error}` placeholder from a displayable error.
pub fn error_msg(msg: Message, err: impl std::fmt::Display) -> String {
    let detail = format!("{err:#}");
    msg.format(language(), &[("error", &detail)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_os_locale_normalizes_underscore_and_encoding() {
        let id = parse_os_locale("pt_BR.UTF-8").expect("parse");
        assert_eq!(id.language.as_str(), "pt");
        assert_eq!(id.region.as_ref().map(|r| r.as_str()), Some("BR"));
    }

    #[test]
    fn parse_os_locale_rejects_c_and_posix() {
        assert!(parse_os_locale("C").is_none());
        assert!(parse_os_locale("POSIX").is_none());
        assert!(parse_os_locale("").is_none());
    }

    #[test]
    fn negotiate_pt_br_system() {
        assert_eq!(
            negotiate_system_locale("pt_BR.UTF-8"),
            Some(Language::PtBr)
        );
        assert_eq!(negotiate_system_locale("pt-BR"), Some(Language::PtBr));
        // Bare `pt` negotiates to pt-BR via langneg filtering against available.
        assert_eq!(negotiate_system_locale("pt"), Some(Language::PtBr));
    }

    #[test]
    fn negotiate_en_variants() {
        assert_eq!(negotiate_system_locale("en_US.UTF-8"), Some(Language::En));
        assert_eq!(negotiate_system_locale("en-GB"), Some(Language::En));
        assert_eq!(negotiate_system_locale("en"), Some(Language::En));
    }

    #[test]
    fn negotiate_unsupported_falls_none() {
        assert_eq!(negotiate_system_locale("ja_JP.UTF-8"), None);
        assert_eq!(negotiate_system_locale("de-DE"), None);
    }

    #[test]
    fn resolve_flag_wins() {
        let r = resolve(Some("pt-BR"));
        assert_eq!(r.language, Language::PtBr);
        assert_eq!(r.source, LocaleSource::Flag);
    }

    #[test]
    fn resolve_invalid_flag_does_not_panic() {
        let r = resolve(Some("not-a-locale!!!"));
        // Falls through; may be system/default depending on host — must be valid Language.
        assert!(matches!(r.language, Language::En | Language::PtBr));
    }

    #[test]
    fn message_parity_en_pt_br_non_empty() {
        for msg in Message::ALL {
            let en = msg.text(Language::En);
            let pt = msg.text(Language::PtBr);
            assert!(!en.is_empty(), "empty EN for {msg:?}");
            assert!(!pt.is_empty(), "empty pt-BR for {msg:?}");
            assert_ne!(
                en, pt,
                "EN and pt-BR must differ for {msg:?} (copy-paste risk)"
            );
        }
    }

    #[test]
    fn format_substitutes_placeholders() {
        let s = Message::GlobalTimeoutExceeded.format(
            Language::En,
            &[("seconds", "42")],
        );
        assert!(s.contains("42"), "{s}");
        assert!(!s.contains("{seconds}"), "{s}");
        let pt = Message::GlobalTimeoutExceeded.format(
            Language::PtBr,
            &[("seconds", "42")],
        );
        assert!(pt.contains("42"), "{pt}");
    }

    #[test]
    fn language_parse_accepts_aliases() {
        assert_eq!(Language::parse("en"), Some(Language::En));
        assert_eq!(Language::parse("EN-us"), Some(Language::En));
        assert_eq!(Language::parse("pt-BR"), Some(Language::PtBr));
        assert_eq!(Language::parse("pt_br"), Some(Language::PtBr));
        assert_eq!(Language::parse("pt"), Some(Language::PtBr));
        assert_eq!(Language::parse("ja"), None);
    }

    #[test]
    fn available_is_mvp_bilingual() {
        assert_eq!(Language::AVAILABLE, &[Language::En, Language::PtBr]);
        assert_eq!(Language::En.as_bcp47(), "en");
        assert_eq!(Language::PtBr.as_bcp47(), "pt-BR");
        assert_eq!(Language::En.direction(), TextDirection::Ltr);
        assert_eq!(Language::PtBr.direction(), TextDirection::Ltr);
    }
}
