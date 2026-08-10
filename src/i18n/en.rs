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
            "\n\nTip: the `--{flag}` flag exists but must appear BEFORE the \
             subcommand (e.g. `duckduckgo-search-cli --{flag}` or \
             `duckduckgo-search-cli --{flag} doctor`). Some flags are also \
             accepted on the subcommand itself (see --help)."
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
        Message::DeepResearchBudgetUnderflow => {
            "Error: --global-timeout {timeout}s is below gated deep-research estimate \
{gated}s (raw ~{estimate}s). Raise timeout, reduce load, or pass --allow-under-budget."
        }
        Message::DeepResearchBudgetAllowOverride => {
            "Warning: --global-timeout {timeout}s is below gated estimate {gated}s \
(raw ~{estimate}s); continuing because --allow-under-budget is set."
        }
        // Agent-native reduction refusals (v1.0.5). These English strings are
        // the STABLE wire text an agent reads from stdout: keep them precise
        // and do not reword them casually.
        // Do NOT list the supported flags here. An earlier wording did, and it
        // was wrong on every contentless surface: `config path --limit 1`
        // advertised `--truncate-content`, which that same surface refuses.
        // A hardcoded list in a shared message cannot know the surface it is
        // talking about; `commands` publishes the per-surface truth.
        Message::AgentOpsRowOpUnsupported => {
            "{op} is not supported by `{surface}`: this envelope has no row array. \
             Run `duckduckgo-search-cli commands` and read `agent_ops` for what \
             this surface accepts."
        }
        Message::AgentOpsEnvelopeNotObject => "`{surface}` did not emit a JSON object",
        Message::AgentOpsRowsKeyMissing => {
            "`{surface}` declares its rows under `{rows}`, which is missing from the \
             envelope — this is a product defect, please report it"
        }
        Message::AgentOpsFilterInvalid => {
            "invalid --filter `{expr}`: expected `key=value`, `key!=value` or `key~substring`"
        }
        Message::AgentOpsSortDirectionInvalid => {
            "invalid --sort direction `{direction}`: expected `asc` or `desc`"
        }
        Message::AgentOpsSortKeyMissing => "invalid --sort: missing key",
        Message::AgentOpsSortKeyUnknown => {
            "invalid --sort key `{key}`: no row under `{rows}` has it. Available: {available}"
        }
        Message::AgentOpsFieldsEmpty => "invalid --fields: no path given",
        Message::AgentOpsFieldsPathUnknownTop => {
            "invalid --fields path `{path}`: `{segment}` is not a key of the top level \
             of `{surface}`. Available there: {available}"
        }
        Message::AgentOpsFieldsPathUnknownNested => {
            "invalid --fields path `{path}`: `{segment}` is not a key of `{parent}`. \
             Available there: {available}"
        }
        Message::AgentOpsFieldsPathInvalid => "invalid --fields path `{path}` for `{surface}`",
        Message::AgentOpsNothingToTruncate => {
            "--truncate-content is not supported by `{surface}`: every string in \
             this envelope is an identifier fed back to a program, so shortening \
             one would produce output that looks valid and is not. Run \
             `duckduckgo-search-cli commands` and read `agent_ops` for what this \
             surface accepts."
        }
        // `CliError` bodies (v1.0.5). These duplicate the `Display` text of the
        // matching variant by design: `Display` is the STABLE English an agent
        // reads off stdout and must not move, while this is the human half that
        // may be reworded per language. Keeping them equal in English is what
        // makes the pt-BR rendering a translation rather than a second product.
        Message::ErrorRateLimited => "rate limiting detected by duckduckgo",
        Message::ErrorBlocked => "anti-bot blocking detected (http 202 anomaly)",
        Message::ErrorNoResults => "zero results across all queries",
        Message::ErrorCancelled => "operation cancelled via sigint/sigterm",
        Message::ErrorBrokenPipe => "pipe closed by consumer (broken pipe)",
        Message::ErrorChromeDisabled => {
            "chrome transport unavailable (rebuild with --features chrome)"
        }
        Message::ErrorPayloadTooLarge => "payload exceeds {max} bytes (got {actual})",
        Message::ErrorUnsupportedEncoding => "unsupported content-encoding: {encoding}",
        Message::ErrorInvalidUtf8 => "response body is not valid utf-8",
        Message::ErrorDecompressionIo => "decompression i/o error: {error}",
        Message::ErrorHttpClient => "http client error",
        // `CliError` labels (v1.0.5). The caller's prose follows each of these
        // and is never translated — the product does not rewrite text it did
        // not write.
        Message::ErrorLabelHttp => "http error",
        Message::ErrorLabelProxy => "proxy error",
        Message::ErrorLabelNetwork => "network error",
        Message::ErrorLabelPipelineInvariant => "pipeline invariant violation",
        Message::ErrorLabelChromeNotFound => "chrome not found",
        Message::ErrorLabelChromeUnavailable => "chrome unavailable",
    }
}
