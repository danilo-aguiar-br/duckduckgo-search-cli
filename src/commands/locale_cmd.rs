// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential utility (locale print). No I/O fan-out — justified.
//! Handler for the `locale` subcommand — UI language diagnostics as JSON.

use crate::cli::LocaleArgs;
use crate::error::exit_codes;
use crate::i18n::{self, Language};
use crate::output;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct LocaleReport {
    #[serde(rename = "type")]
    kind: &'static str,
    /// Negotiated UI BCP-47 tag (`en` or `pt-BR`).
    resolved: String,
    /// Precedence layer that won (`flag` / `persisted` / `system` / `default`).
    source: &'static str,
    /// Raw OS locale when available.
    system_raw: Option<String>,
    /// Script subtag (e.g. `Latn`).
    script: &'static str,
    /// `ltr` or `rtl`.
    direction: &'static str,
    /// Locales compiled into this binary.
    available: Vec<&'static str>,
    /// CLI flag for UI language override (no product env — GAP-SCRAPE-R2-014).
    ui_lang_flag: &'static str,
    /// Note: DuckDuckGo search `-l/--lang` is a separate SERP parameter.
    search_lang_flag: &'static str,
}

/// Emits a single JSON object describing the resolved UI locale on stdout.
pub fn execute_locale(_args: LocaleArgs) -> i32 {
    // Ensure resolution ran (normally done in `run` before dispatch).
    let snap = i18n::resolved();
    let lang = snap.language;
    let report = LocaleReport {
        kind: "locale",
        resolved: lang.as_bcp47().to_owned(),
        source: snap.source.as_str(),
        // Move the owned Option out of the snapshot (partial move).
        system_raw: snap.system_raw,
        script: lang.script(),
        direction: match lang.direction() {
            i18n::TextDirection::Ltr => "ltr",
            i18n::TextDirection::Rtl => "rtl",
        },
        available: Language::AVAILABLE.iter().map(|l| l.as_bcp47()).collect(),
        ui_lang_flag: "--ui-lang",
        search_lang_flag: "-l/--lang (DuckDuckGo SERP kl; not UI)",
    };

    match serde_json::to_string(&report) {
        Ok(json) => {
            if let Err(err) = output::print_line_stdout(&json) {
                output::emit_stderr(i18n::error_msg(i18n::Message::LocaleEmitFailed, err));
                return exit_codes::GENERIC_ERROR;
            }
            exit_codes::SUCCESS
        }
        Err(err) => {
            output::emit_stderr(i18n::error_msg(i18n::Message::LocaleSerializeFailed, err));
            exit_codes::GENERIC_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_report_serializes_type() {
        i18n::initialize(Some("en"));
        let code = execute_locale(LocaleArgs {});
        assert_eq!(code, exit_codes::SUCCESS);
    }
}
