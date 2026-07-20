// SPDX-License-Identifier: MIT OR Apache-2.0
//! UI message keys — exhaustive match per language (no catch-all).

use super::en;
use super::language::Language;
use super::pt_br;

/// Human-facing UI message key.
///
/// Variants are English technical names (global code convention). Translations
/// live in [`crate::i18n::en`] and [`crate::i18n::pt_br`] with **exhaustive**
/// `match` (no `_` arm). Machine-oriented JSON field names and `tracing` text
/// are **not** represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Message {
    /// Prefix before a configuration/`clap` error detail (`Configuration error`).
    ConfigurationErrorPrefix,
    /// Prefix before a generic runtime error detail (`Error`).
    ErrorPrefix,
    /// Global wall-clock timeout exceeded (placeholder `{seconds}`).
    GlobalTimeoutExceeded,
    /// Deep-research path hit the global timeout (placeholder `{seconds}`).
    DeepResearchTimeoutExceeded,
    /// Clap unknown-arg tip when a known flag was placed after a subcommand
    /// (placeholder `{flag}`).
    FlagMustPrecedeSubcommand,
    /// Xvfb auto-install start banner.
    XvfbAutoInstallAttempt,
    /// Xvfb installed successfully.
    XvfbInstalledOk,
    /// Xvfb auto-install failed (no passwordless sudo).
    XvfbAutoInstallFailed,
    /// Immutable distro: cannot auto-install Xvfb (placeholder `{distro}`).
    XvfbImmutableDistro,
    /// Unrecognized distro for Xvfb auto-install (placeholder `{distro}`).
    XvfbUnknownDistro,
    /// Failed to spawn package manager (placeholder `{error}`).
    XvfbPackageManagerFailed,
    /// Manual install hint prefix (short).
    XvfbInstallManually,
    /// Manual install hint prefix for full instruction block.
    XvfbInstallManuallyFull,
    /// Xvfb unavailable; Chrome will run headless.
    XvfbUnavailableHeadlessFallback,
    /// Cancel cooperative start (placeholders `{signal}`, `{exit}`, `{grace}`).
    CancelCooperativeStarted,
    /// Second signal during grace period (placeholder `{exit}`).
    CancelSecondSignalForceExit,
    /// Grace period expired (placeholders `{grace}`, `{exit}`).
    CancelGraceExpiredForceExit,
    /// Deep-research zero results with `--require-results` (placeholder `{query}`).
    DeepResearchZeroResultsRequire,
    /// Stdout write failure (placeholder `{error}`).
    StdoutWriteFailed,
    /// Deep-research JSON serialize failure (placeholder `{error}`).
    DeepResearchSerializeFailed,
    /// Deep-research pipeline failure (placeholder `{error}`).
    DeepResearchFailed,
    /// Commands tree emit failure (placeholder `{error}`).
    CommandsTreeEmitFailed,
    /// Commands tree serialize failure (placeholder `{error}`).
    CommandsTreeSerializeFailed,
    /// Doctor report emit failure (placeholder `{error}`).
    DoctorEmitFailed,
    /// Doctor report serialize failure (placeholder `{error}`).
    DoctorSerializeFailed,
    /// Embedded schema is not valid JSON (placeholder `{id}`).
    SchemaInvalidJson,
    /// Schema emit failure (placeholders `{id}`, `{error}`).
    SchemaEmitFailed,
    /// Schema JSON emit failure (placeholder `{error}`).
    SchemaJsonEmitFailed,
    /// Schema serialize failure (placeholder `{error}`).
    SchemaSerializeFailed,
    /// Locale report emit failure (placeholder `{error}`).
    LocaleEmitFailed,
    /// Locale report serialize failure (placeholder `{error}`).
    LocaleSerializeFailed,
    /// Markdown / text synthesis section header for recent news.
    SynthesisRecentNewsHeading,
    /// Plain-text synthesis label for recent news.
    SynthesisRecentNewsLabel,
    /// Empty-results placeholder for human text format.
    NoResultsPlaceholder,
    /// Markdown H1 for a search report (placeholder `{query}`).
    MarkdownResultsHeading,
    /// Markdown meta line (placeholders `{engine}`, `{endpoint}`, `{total}`).
    MarkdownMetaLine,
}

impl Message {
    /// Every message key — used by parity tests (must stay in sync with match arms).
    pub const ALL: &'static [Message] = &[
        Self::ConfigurationErrorPrefix,
        Self::ErrorPrefix,
        Self::GlobalTimeoutExceeded,
        Self::DeepResearchTimeoutExceeded,
        Self::FlagMustPrecedeSubcommand,
        Self::XvfbAutoInstallAttempt,
        Self::XvfbInstalledOk,
        Self::XvfbAutoInstallFailed,
        Self::XvfbImmutableDistro,
        Self::XvfbUnknownDistro,
        Self::XvfbPackageManagerFailed,
        Self::XvfbInstallManually,
        Self::XvfbInstallManuallyFull,
        Self::XvfbUnavailableHeadlessFallback,
        Self::CancelCooperativeStarted,
        Self::CancelSecondSignalForceExit,
        Self::CancelGraceExpiredForceExit,
        Self::DeepResearchZeroResultsRequire,
        Self::StdoutWriteFailed,
        Self::DeepResearchSerializeFailed,
        Self::DeepResearchFailed,
        Self::CommandsTreeEmitFailed,
        Self::CommandsTreeSerializeFailed,
        Self::DoctorEmitFailed,
        Self::DoctorSerializeFailed,
        Self::SchemaInvalidJson,
        Self::SchemaEmitFailed,
        Self::SchemaJsonEmitFailed,
        Self::SchemaSerializeFailed,
        Self::LocaleEmitFailed,
        Self::LocaleSerializeFailed,
        Self::SynthesisRecentNewsHeading,
        Self::SynthesisRecentNewsLabel,
        Self::NoResultsPlaceholder,
        Self::MarkdownResultsHeading,
        Self::MarkdownMetaLine,
    ];

    /// Returns the static template for `lang` (may contain `{name}` placeholders).
    pub fn text(self, lang: Language) -> &'static str {
        match lang {
            Language::En => en::translate(self),
            Language::PtBr => pt_br::translate(self),
        }
    }

    /// Substitutes `{key}` placeholders from `pairs` into the template.
    ///
    /// Unknown placeholders are left unchanged. Values are inserted as-is
    /// (no nested formatting).
    pub fn format(self, lang: Language, pairs: &[(&str, &str)]) -> String {
        let mut out = self.text(lang).to_owned();
        for (key, value) in pairs {
            let needle = format!("{{{key}}}");
            out = out.replace(&needle, value);
        }
        out
    }
}
