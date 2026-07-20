// SPDX-License-Identifier: MIT OR Apache-2.0
//! English UI strings — exhaustive match (no catch-all).

use super::message::Message;

/// Translates `msg` to English. Must list every [`Message`] variant.
pub fn translate(msg: Message) -> &'static str {
    match msg {
        Message::ConfigurationErrorPrefix => "Configuration error",
        Message::ErrorPrefix => "Error",
        Message::GlobalTimeoutExceeded => "Error: global timeout of {seconds}s exceeded",
        Message::DeepResearchTimeoutExceeded => {
            "Error: global timeout of {seconds}s exceeded (deep-research)"
        }
        Message::FlagMustPrecedeSubcommand => {
            "\n\nTip: the `-{flag}` flag exists but must appear BEFORE the \
             subcommand (e.g. `duckduckgo-search-cli -{flag} deep-research \"query\"`)."
        }
        Message::XvfbAutoInstallAttempt => {
            "\x1b[33m[duckduckgo-search-cli]\x1b[0m Xvfb not found — \
             attempting automatic install via passwordless sudo..."
        }
        Message::XvfbInstalledOk => {
            "\x1b[32m[duckduckgo-search-cli]\x1b[0m Xvfb installed successfully."
        }
        Message::XvfbAutoInstallFailed => {
            "\x1b[31m[duckduckgo-search-cli]\x1b[0m Auto-install failed \
             (passwordless sudo not available)."
        }
        Message::XvfbImmutableDistro => {
            "\x1b[33m[duckduckgo-search-cli]\x1b[0m Immutable distro detected ({distro}) — \
             Xvfb auto-install is not possible."
        }
        Message::XvfbUnknownDistro => {
            "\x1b[33m[duckduckgo-search-cli]\x1b[0m Unrecognized distro ({distro}) — \
             Xvfb auto-install is not available."
        }
        Message::XvfbPackageManagerFailed => {
            "\x1b[31m[duckduckgo-search-cli]\x1b[0m Failed to run package manager: {error}"
        }
        Message::XvfbInstallManually => "\x1b[33m  Install manually:\x1b[0m",
        Message::XvfbInstallManuallyFull => "\x1b[33m  Install Xvfb manually:\x1b[0m\n",
        Message::XvfbUnavailableHeadlessFallback => {
            "\x1b[33m[duckduckgo-search-cli]\x1b[0m Xvfb unavailable — \
             Chrome will run headless (weaker anti-bot evasion)."
        }
        Message::CancelCooperativeStarted => {
            "duckduckgo-search-cli: {signal} — cooperative cancel started; \
             force exit {exit} in {grace}s (second signal exits immediately)"
        }
        Message::CancelSecondSignalForceExit => {
            "duckduckgo-search-cli: second signal during grace — immediate force exit {exit} (one-shot)"
        }
        Message::CancelGraceExpiredForceExit => {
            "duckduckgo-search-cli: cancel grace period ({grace}s) expired — force exit {exit} (one-shot)"
        }
        Message::DeepResearchZeroResultsRequire => {
            "deep-research produced zero results for query {query}; \
             --require-results set → exiting non-zero"
        }
        Message::StdoutWriteFailed => "stdout write failed: {error}",
        Message::DeepResearchSerializeFailed => {
            "Error serializing deep-research output: {error}"
        }
        Message::DeepResearchFailed => "deep-research failed: {error}",
        Message::CommandsTreeEmitFailed => "failed to emit commands tree: {error}",
        Message::CommandsTreeSerializeFailed => "failed to serialize commands tree: {error}",
        Message::DoctorEmitFailed => "failed to emit doctor report: {error}",
        Message::DoctorSerializeFailed => "failed to serialize doctor report: {error}",
        Message::SchemaInvalidJson => "embedded schema {id} is not valid JSON",
        Message::SchemaEmitFailed => "failed to emit schema {id}: {error}",
        Message::SchemaJsonEmitFailed => "failed to emit schema JSON: {error}",
        Message::SchemaSerializeFailed => "failed to serialize schema JSON: {error}",
        Message::LocaleEmitFailed => "failed to emit locale report: {error}",
        Message::LocaleSerializeFailed => "failed to serialize locale report: {error}",
        Message::SynthesisRecentNewsHeading => "### Recent news\n\n",
        Message::SynthesisRecentNewsLabel => "Recent news:\n\n",
        Message::NoResultsPlaceholder => "\n(no results)\n",
        Message::MarkdownResultsHeading => "# Results: {query}\n\n",
        Message::MarkdownMetaLine => {
            "**Engine:** {engine} | **Endpoint:** {endpoint} | **Total:** {total}\n\n"
        }
    }
}
