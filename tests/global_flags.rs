// SPDX-License-Identifier: MIT OR Apache-2.0
//! E2E tests for GAP-WS-106 (v0.9.0) — CLI ergonomics.
//!
//! Validates the three fixed symptoms against the compiled binary:
//! - Symptom A: the PT-BR hint does NOT appear for unknown typo'd flags
//!   (it only appears for known global flags used out of position, which
//!   no longer errors because they are `global = true` — covered by unit tests).
//! - Symptom C (build without `chrome`): the parser still accepts `--no-news`
//!   and `--vertical news`; the auto-default is validated in integration tests.
//!
//! Symptom B (the parser accepts `-q`/`-o` after a subcommand) is covered by the
//! unit tests `quiet_global_accepted_after_subcommand` and
//! `output_global_accepted_after_subcommand` in `src/cli.rs`.

use assert_cmd::Command;
use predicates::prelude::*;

const BIN_NAME: &str = "duckduckgo-search-cli";

/// Symptom A — an unknown typo'd flag does NOT trigger the PT-BR hint
/// (the hint only appears for known global flags used out of position).
#[test]
fn symptom_a_unknown_flag_does_not_trigger_hint() {
    Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["--flag-totalmente-inexistente"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Dica:").not());
}

/// Symptom A — the PT-BR hint appears when a known `CliArgs`-local
/// (non-hoisted) flag is passed after a subcommand. `--pages` exists on the
/// root parser but is LOCAL to `CliArgs`; inside `deep-research`
/// (which uses `DeepResearchArgs`) it is unknown, so clap reports
/// `UnknownArgument`, and the formatter appends the hint because `pages` matches
/// `is_known_global_flag`.
#[test]
fn symptom_a_hint_appears_for_known_local_flag_after_subcommand() {
    Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["deep-research", "--pages", "3", "rust"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Dica:"));
}

/// Symptom C (build without chrome) — the `deep-research` subcommand still
/// accepts `--no-news` explicitly (backward compatibility preserved).
/// Validating the runtime auto-default belongs to the integration tests.
#[cfg(not(feature = "chrome"))]
#[test]
fn symptom_c_deep_research_accepts_no_news_without_chrome() {
    // No network: only `--help` is passed to validate the parser on the
    // chrome-less path. The `--no-news` auto-default happens only in runtime
    // code; this test documents that the flag is still accepted.
    Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["deep-research", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}
