// SPDX-License-Identifier: MIT OR Apache-2.0
//! UI message keys — exhaustive match per language (no catch-all).

use super::en;
use super::language::Language;
use super::pt_br;

/// Human-facing UI message key.
///
/// Variants are English technical names (global code convention). Translations
/// live in `crate::i18n::en` and `crate::i18n::pt_br` with **exhaustive**
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
    /// Deep-research budget underflow fail-fast (placeholders `{timeout}`, `{gated}`, `{estimate}`).
    DeepResearchBudgetUnderflow,
    /// Under-budget override warning when `--allow-under-budget` (placeholders `{timeout}`, `{gated}`, `{estimate}`).
    DeepResearchBudgetAllowOverride,

    // -----------------------------------------------------------------------
    // Agent-native reduction refusals (v1.0.5).
    //
    // These sentences used to be raw `format!` literals inside
    // `output::envelope_ops`, which meant a binary advertising `--ui-lang`
    // answered every refusal in English. They live here so both renderings —
    // the STABLE English one an agent parses off stdout, and the operator's
    // language on stderr — come from ONE template. See
    // `crate::i18n::bilingual`.
    // -----------------------------------------------------------------------
    /// Row operation on a rowless envelope (placeholders `{op}`, `{surface}`).
    AgentOpsRowOpUnsupported,
    /// Surface emitted something that is not a JSON object (placeholder `{surface}`).
    AgentOpsEnvelopeNotObject,
    /// Declared row key absent from the envelope (placeholders `{surface}`, `{rows}`).
    AgentOpsRowsKeyMissing,
    /// Malformed `--filter` expression (placeholder `{expr}`).
    AgentOpsFilterInvalid,
    /// Unknown `--sort` direction (placeholder `{direction}`).
    AgentOpsSortDirectionInvalid,
    /// `--sort` given with no key at all.
    AgentOpsSortKeyMissing,
    /// `--sort` key absent from every row (placeholders `{key}`, `{rows}`, `{available}`).
    AgentOpsSortKeyUnknown,
    /// `--fields` given with no path at all.
    AgentOpsFieldsEmpty,
    /// `--fields` path broke at the top level (placeholders `{path}`, `{segment}`, `{surface}`, `{available}`).
    AgentOpsFieldsPathUnknownTop,
    /// `--fields` path broke below the top (placeholders `{path}`, `{segment}`, `{parent}`, `{available}`).
    AgentOpsFieldsPathUnknownNested,
    /// `--fields` path is invalid for this surface (placeholders `{path}`, `{surface}`).
    AgentOpsFieldsPathInvalid,
    /// Envelope carries only identifiers, so there is no content to shorten (placeholder `{surface}`).
    AgentOpsNothingToTruncate,

    // -----------------------------------------------------------------------
    // `CliError` bodies and labels (v1.0.5).
    //
    // Until now `localized_detail` translated the PREFIX and left the body in
    // English, so an operator running under `--ui-lang pt-BR` read
    // `Erro: rate limiting detected by duckduckgo` — half a sentence in each
    // language. The split below is deliberate and is the whole design:
    //
    // - a variant whose `Display` is a FIXED template gets a full translated
    //   body, because the product owns every word of it;
    // - a variant that embeds `{message}` from the caller gets a translated
    //   LABEL only, because the prose inside belongs to whoever threw it and
    //   translating it here would mean inventing text nobody wrote.
    //
    // `CliError::PathError` appears in neither list on purpose: its `Display`
    // is the bare caller message, with no label of its own.
    // -----------------------------------------------------------------------
    /// Fixed body — persistent HTTP 429 after retries.
    ErrorRateLimited,
    /// Fixed body — anti-bot block detected.
    ErrorBlocked,
    /// Fixed body — zero organic results across every query.
    ErrorNoResults,
    /// Fixed body — cooperative cancel via SIGINT/SIGTERM.
    ErrorCancelled,
    /// Fixed body — consumer closed the pipe.
    ErrorBrokenPipe,
    /// Fixed body — binary built without the `chrome` feature.
    ErrorChromeDisabled,
    /// Fixed body — safety cap exceeded (placeholders `{max}`, `{actual}`).
    ErrorPayloadTooLarge,
    /// Fixed body — unknown `Content-Encoding` (placeholder `{encoding}`).
    ErrorUnsupportedEncoding,
    /// Fixed body — response body is not valid UTF-8.
    ErrorInvalidUtf8,
    /// Fixed body — decompressor I/O failure (placeholder `{error}`).
    ErrorDecompressionIo,
    /// Fixed body — HTTP client failure under the test harness.
    ErrorHttpClient,
    /// Label — HTTP failure; caller prose follows.
    ErrorLabelHttp,
    /// Label — proxy failure; caller prose follows.
    ErrorLabelProxy,
    /// Label — low-level network failure; caller prose follows.
    ErrorLabelNetwork,
    /// Label — pipeline invariant violated; caller prose follows.
    ErrorLabelPipelineInvariant,
    /// Label — Chrome binary not found; caller remediation follows.
    ErrorLabelChromeNotFound,
    /// Label — Chrome transport failed; caller remediation follows.
    ErrorLabelChromeUnavailable,
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
        Self::DeepResearchBudgetUnderflow,
        Self::DeepResearchBudgetAllowOverride,
        Self::AgentOpsRowOpUnsupported,
        Self::AgentOpsEnvelopeNotObject,
        Self::AgentOpsRowsKeyMissing,
        Self::AgentOpsFilterInvalid,
        Self::AgentOpsSortDirectionInvalid,
        Self::AgentOpsSortKeyMissing,
        Self::AgentOpsSortKeyUnknown,
        Self::AgentOpsFieldsEmpty,
        Self::AgentOpsFieldsPathUnknownTop,
        Self::AgentOpsFieldsPathUnknownNested,
        Self::AgentOpsFieldsPathInvalid,
        Self::AgentOpsNothingToTruncate,
        Self::ErrorRateLimited,
        Self::ErrorBlocked,
        Self::ErrorNoResults,
        Self::ErrorCancelled,
        Self::ErrorBrokenPipe,
        Self::ErrorChromeDisabled,
        Self::ErrorPayloadTooLarge,
        Self::ErrorUnsupportedEncoding,
        Self::ErrorInvalidUtf8,
        Self::ErrorDecompressionIo,
        Self::ErrorHttpClient,
        Self::ErrorLabelHttp,
        Self::ErrorLabelProxy,
        Self::ErrorLabelNetwork,
        Self::ErrorLabelPipelineInvariant,
        Self::ErrorLabelChromeNotFound,
        Self::ErrorLabelChromeUnavailable,
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

#[cfg(test)]
mod tests {
    use super::Message;

    /// Variant names declared in the enum body, read from this file's own source.
    ///
    /// [`Message::ALL`] is hand-maintained, and the parity test iterates it —
    /// so a variant forgotten there is a variant the parity test never sees.
    /// That is the failure this repository keeps re-learning: a guard that
    /// keeps its own copy of the target measures the copy. Reading the source
    /// with [`include_str!`] gives the guard the real list instead of a second
    /// copy of it.
    ///
    /// The scan stops at the `impl` block, so the names inside `ALL` itself
    /// (written `Self::…`) are never mistaken for declarations.
    fn declared_variants() -> Vec<String> {
        let source = include_str!("message.rs");
        let body = source
            .split_once("pub enum Message {")
            .expect("enum declaration present")
            .1;
        let body = body.split_once("\nimpl Message {").map_or(body, |(a, _)| a);
        body.lines()
            .map(str::trim)
            .filter(|line| line.ends_with(','))
            .map(|line| line.trim_end_matches(',').to_owned())
            .filter(|name| {
                let mut chars = name.chars();
                chars.next().is_some_and(char::is_uppercase)
                    && chars.all(|c| c.is_alphanumeric() || c == '_')
            })
            .collect()
    }

    #[test]
    fn all_lists_every_declared_variant() {
        let declared = declared_variants();
        assert!(
            declared.len() > 40,
            "parser found only {} variants — it broke, not the enum",
            declared.len()
        );
        let listed: Vec<String> = Message::ALL.iter().map(|m| format!("{m:?}")).collect();
        let missing: Vec<&String> = declared.iter().filter(|d| !listed.contains(d)).collect();
        assert!(
            missing.is_empty(),
            "Message::ALL is missing {missing:?}; the parity test would never see them"
        );
        assert_eq!(
            listed.len(),
            declared.len(),
            "Message::ALL has {} entries for {} declared variants",
            listed.len(),
            declared.len()
        );
    }
}
