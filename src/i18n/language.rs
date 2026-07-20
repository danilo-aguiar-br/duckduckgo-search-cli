// SPDX-License-Identifier: MIT OR Apache-2.0
//! Supported UI languages (MVP: `en` + `pt-BR`).

use unic_langid::LanguageIdentifier;

/// Text direction for terminal layout (MVP locales are all LTR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextDirection {
    /// Left-to-right (Latin, CJK when added, etc.).
    Ltr,
    /// Right-to-left (Arabic / Hebrew — only under future `i18n-rtl` feature).
    Rtl,
}

/// UI language supported by this binary build.
///
/// English identifiers (rules-rust code-in-English). Wire / BCP-47 tags use
/// `as_bcp47()`. Marked `#[non_exhaustive]` so optional top-20 locales can be
/// added behind Cargo features without a major SemVer break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Language {
    /// English (neutral `en` — not `en-US` only).
    En,
    /// Brazilian Portuguese (`pt-BR`). Bare `pt` maps here in the MVP.
    PtBr,
}

impl Language {
    /// Locales compiled into the default binary (MVP bilingual only).
    pub const AVAILABLE: &'static [Language] = &[Self::En, Self::PtBr];

    /// BCP-47 tag used in diagnostics and preference files.
    pub const fn as_bcp47(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::PtBr => "pt-BR",
        }
    }

    /// ISO 15924 script subtag for this language.
    pub const fn script(self) -> &'static str {
        match self {
            Self::En | Self::PtBr => "Latn",
        }
    }

    /// Reading direction for terminal UI.
    pub const fn direction(self) -> TextDirection {
        match self {
            Self::En | Self::PtBr => TextDirection::Ltr,
        }
    }

    /// Fallback language when a regional variant is missing a string.
    ///
    /// MVP has full parity; fallback is always English for agent stability.
    pub const fn fallback(self) -> Language {
        match self {
            Self::En => Self::En,
            Self::PtBr => Self::En,
        }
    }

    /// Structured BCP-47 identifier for negotiation.
    ///
    /// # Panics
    ///
    /// Panics if a compile-time BCP-47 tag fails to parse (static tags are valid).
    pub fn language_identifier(self) -> LanguageIdentifier {
        self.as_bcp47()
            .parse()
            .expect("static BCP-47 tags are valid")
    }

    /// Maps a negotiated [`LanguageIdentifier`] onto a compiled [`Language`].
    pub fn from_language_identifier(id: &LanguageIdentifier) -> Option<Self> {
        let lang = id.language.as_str();
        match lang {
            "en" => Some(Self::En),
            "pt" => {
                // MVP: bare `pt` and `pt-BR` → PtBr. `pt-PT` is not a separate
                // compiled locale (would need feature `i18n-pt-pt` later).
                // Prefer PtBr for any Portuguese tag in the default binary.
                Some(Self::PtBr)
            }
            _ => None,
        }
    }

    /// Parses a user-supplied tag (`en`, `en-US`, `pt-BR`, `pt_br`, `pt`).
    pub fn parse(raw: &str) -> Option<Self> {
        let normalized = raw.trim().replace('_', "-");
        if normalized.is_empty() {
            return None;
        }
        let id: LanguageIdentifier = normalized.parse().ok()?;
        Self::from_language_identifier(&id)
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_bcp47())
    }
}
