// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fail-closed detection of mistyped subcommand tokens used as SERP queries.

/// True when a bare positional looks like a mistyped subcommand rather than a SERP query.
///
/// GAP-E2E-V11-UNKNOWN-AS-QUERY: tokens with hyphens (and no spaces) that are not
/// in the known subcommand set fail closed with exit 2 instead of launching Chrome.
///
/// GAP-E2E-V14-UNDERSCORE-TYPO-AS-QUERY: underscore aliases of known commands
/// (`deep_research`, `init_config`) also fail closed — they previously became
/// SERP queries and burned `--global-timeout` (exit 4).
pub fn looks_like_unknown_subcommand_token(token: &str) -> bool {
    const KNOWN: &[&str] = &[
        "buscar",
        "deep-research",
        "doctor",
        "config",
        "schema",
        "locale",
        "man",
        "completions",
        "commands",
        "init-config",
        "help",
    ];
    let t = token.trim();
    if t.is_empty() || t.starts_with('-') || t.contains(char::is_whitespace) {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    // Underscore form of a known subcommand (agent footgun: deep_research).
    // Only trap aliases / prefix typos of multi-part commands — plain queries
    // like `some_api_name` must remain valid SERP tokens.
    if lower.contains('_') {
        let normalized = lower.replace('_', "-");
        if KNOWN.contains(&normalized.as_str()) {
            return true;
        }
        // Prefix traps for multi-part known commands (deep-*, init-*).
        const UNDERSCORE_PREFIX_TRAPS: &[&str] = &["deep_", "init_"];
        if UNDERSCORE_PREFIX_TRAPS
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        {
            return true;
        }
        return false;
    }
    // Require a hyphen so plain queries like `openai` / `rustc` stay SERP.
    if !lower.contains('-') {
        return false;
    }
    !KNOWN.contains(&lower.as_str())
}

#[cfg(test)]
mod unknown_subcommand_guard_tests {
    use super::looks_like_unknown_subcommand_token;

    #[test]
    fn rejects_hyphenated_typos() {
        assert!(looks_like_unknown_subcommand_token("not-a-real-subcommand"));
        assert!(looks_like_unknown_subcommand_token("deep-researh"));
        assert!(looks_like_unknown_subcommand_token("init-cofig"));
    }

    #[test]
    fn rejects_underscore_aliases_of_known_commands() {
        // GAP-E2E-V14-UNDERSCORE-TYPO-AS-QUERY
        assert!(looks_like_unknown_subcommand_token("deep_research"));
        assert!(looks_like_unknown_subcommand_token("init_config"));
        assert!(looks_like_unknown_subcommand_token("Deep_Research"));
        assert!(looks_like_unknown_subcommand_token("deep_researh"));
    }

    #[test]
    fn allows_plain_and_known() {
        assert!(!looks_like_unknown_subcommand_token("openai"));
        assert!(!looks_like_unknown_subcommand_token("rust async"));
        assert!(!looks_like_unknown_subcommand_token("deep-research"));
        assert!(!looks_like_unknown_subcommand_token("--help"));
        // Underscored plain queries that are NOT command aliases stay SERP.
        assert!(!looks_like_unknown_subcommand_token("some_api_name"));
        // Hyphenated research queries must use: buscar "site-specific-term"
        assert!(looks_like_unknown_subcommand_token("site-specific-term"));
    }
}
