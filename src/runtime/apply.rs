// SPDX-License-Identifier: MIT OR Apache-2.0
//! Apply XDG defaults onto clap-parsed args (CLI > XDG > FACTORY).
//!
//! Detection of "explicit CLI" uses equality against known clap defaults.

use crate::cli::{
    CliArgs, CliVertical, RootArgs, DEFAULT_GLOBAL_TIMEOUT, DEFAULT_SERP_COUNTRY,
    DEFAULT_SERP_LANG,
};
use super::user::UserConfig;

/// Apply XDG defaults onto parsed CLI when the user left clap defaults.
///
/// Precedence: explicit CLI > XDG > built-in. Detection of “explicit CLI” uses
/// equality against known clap defaults (standard for global flags with
/// `default_value_t`).
pub fn apply_user_config_to_root(root: &mut RootArgs, xdg: &UserConfig) {
    // global_timeout: only replace when still at built-in default.
    if root.global_timeout_seconds == DEFAULT_GLOBAL_TIMEOUT {
        if let Some(n) = xdg.default_global_timeout() {
            root.global_timeout_seconds = n;
        }
    }

    // ui_lang: CLI Option None → XDG.
    if root.ui_lang.is_none() {
        if let Some(lang) = xdg.ui_lang() {
            root.ui_lang = Some(lang.to_string());
        }
    }

    // cancel grace: only when still at built-in default.
    if root.cancel_grace_secs == crate::types::bounded::DEFAULT_CANCEL_GRACE_SECS {
        if let Some(raw) = xdg.get("default_cancel_grace_secs") {
            if let Ok(n) = raw.trim().parse::<u64>() {
                if (1..=60).contains(&n) {
                    root.cancel_grace_secs = n;
                }
            }
        }
    }

    // wire_keys: only when still at default EN (CLI flag wins when pt).
    if root.wire_keys == crate::output::WireKeys::En {
        if let Some(raw) = xdg.get("wire_keys") {
            if let Some(wk) = crate::output::WireKeys::parse(raw) {
                root.wire_keys = wk;
            }
        }
    }

    apply_user_config_to_cli_args(&mut root.buscar, xdg);
}

/// Map OS locale to a DuckDuckGo SERP language code when XDG has no override.
///
/// Returns `None` when the locale is unknown so the built-in const remains.
fn serp_lang_from_os_locale() -> Option<String> {
    let raw = sys_locale::get_locale()?;
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("pt") {
        Some("pt".into())
    } else if lower.starts_with("en") {
        Some("en".into())
    } else if lower.starts_with("es") {
        Some("es".into())
    } else if lower.starts_with("fr") {
        Some("fr".into())
    } else if lower.starts_with("de") {
        Some("de".into())
    } else if lower.starts_with("it") {
        Some("it".into())
    } else if lower.starts_with("ja") {
        Some("jp".into())
    } else if lower.starts_with("zh") {
        Some("zh-cn".into())
    } else {
        None
    }
}

/// Map OS locale to a DuckDuckGo country / region code (`kl` country half).
fn serp_country_from_os_locale() -> Option<String> {
    let raw = sys_locale::get_locale()?;
    // Accept `pt_BR.UTF-8`, `en-US`, `en_US`, etc.
    let normalized = raw.replace('_', "-");
    let parts: Vec<&str> = normalized.split(['-', '.']).collect();
    if parts.len() >= 2 {
        let region = parts[1].trim();
        if region.len() == 2 && region.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some(region.to_ascii_lowercase());
        }
    }
    let lang = parts.first().copied().unwrap_or("").to_ascii_lowercase();
    match lang.as_str() {
        "pt" => Some("br".into()),
        "en" => Some("us".into()),
        "es" => Some("es".into()),
        "fr" => Some("fr".into()),
        "de" => Some("de".into()),
        "it" => Some("it".into()),
        "ja" => Some("jp".into()),
        "zh" => Some("cn".into()),
        _ => None,
    }
}

/// Apply XDG defaults onto search CLI args (buscar / deep-research defaults).
pub fn apply_user_config_to_cli_args(args: &mut CliArgs, xdg: &UserConfig) {
    if args.chrome_path.is_none() {
        if let Some(p) = xdg.chrome_path() {
            args.chrome_path = Some(p);
        }
    }

    // Proxy: only when neither --proxy nor --no-proxy was set.
    if args.proxy.is_none() && !args.no_proxy {
        if let Some(url) = xdg.proxy_url() {
            args.proxy = Some(url);
        }
    }

    // Vertical default is All.
    if matches!(args.vertical, CliVertical::All) {
        if let Some(v) = xdg.default_vertical() {
            args.vertical = v;
        }
    }

    // Fetch default ON: only apply XDG false when user did not pass
    // --fetch-content / --no-fetch-content.
    if !args.fetch_content && !args.no_fetch_content {
        if let Some(want_fetch) = xdg.fetch_content_default() {
            if want_fetch {
                args.fetch_content = true;
            } else {
                args.no_fetch_content = true;
            }
        }
    }

    // SERP language/country precedence (GAP-E2E-V19-DEFAULT-SERP-BR-HARDCODE):
    // 1. Explicit CLI `-l`/`-c` (not at built-in default) always wins.
    // 2. XDG `default_lang` / `default_country` when set.
    // 3. Derive from OS locale (sys-locale) when still at built-in defaults.
    // 4. Built-in const fallback remains only when locale is unknown.
    if args.language == DEFAULT_SERP_LANG {
        if let Some(lang) = xdg.default_lang() {
            args.language = lang.to_string();
        } else if let Some(lang) = serp_lang_from_os_locale() {
            args.language = lang;
        }
    }
    if args.country == DEFAULT_SERP_COUNTRY {
        if let Some(cc) = xdg.default_country() {
            args.country = cc.to_string();
        } else if let Some(cc) = serp_country_from_os_locale() {
            args.country = cc;
        }
    }

    // fetch-content-cap: only when still at built-in default (v1.0.2 CM-12).
    if args.fetch_content_cap == crate::cli::DEFAULT_FETCH_CONTENT_CAP {
        if let Some(cap) = xdg.default_fetch_content_cap() {
            args.fetch_content_cap = cap;
        }
    }

    // Parallelism / max-concurrency: only when still at clap default (GAP-XDG-DEFAULT-PARALLEL).
    if args.parallelism == crate::cli::DEFAULT_PARALLELISM {
        if let Some(n) = xdg.default_parallelism() {
            args.parallelism = n;
        }
    }

    // Chrome session retries: only when still at built-in default (V12).
    if args.chrome_session_retries == crate::error::DEFAULT_CHROME_SESSION_RETRIES {
        if let Some(n) = xdg.chrome_session_retries() {
            args.chrome_session_retries = n;
        }
    }

    // Agent ops defaults (CLI wins when already set).
    if args.sort.is_none() {
        if let Some(s) = xdg.get("default_sort") {
            args.sort = Some(s.to_string());
        }
    }
    if args.dedupe_by.is_none() {
        if let Some(s) = xdg.get("default_dedupe_by") {
            args.dedupe_by = Some(s.to_string());
        }
    }
    if args.max_output_bytes.is_none() {
        if let Some(raw) = xdg.get("max_output_bytes") {
            if let Ok(n) = raw.trim().parse::<u64>() {
                if n >= 1 {
                    args.max_output_bytes = Some(n);
                }
            }
        }
    }
    if args.truncate_content.is_none() {
        if let Some(raw) = xdg.get("default_content_truncate") {
            if let Ok(n) = raw.trim().parse::<u32>() {
                if n >= 1 {
                    args.truncate_content = Some(n);
                }
            }
        }
    }
    if !args.allow_no_warmup {
        if let Some(raw) = xdg.get("allow_no_warmup") {
            match raw.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => args.allow_no_warmup = true,
                _ => {}
            }
        }
    }

    // Operational defaults (V30): only when CLI still at FACTORY.
    if args.timeout_seconds == crate::types::bounded::DEFAULT_TIMEOUT_SECONDS {
        if let Some(raw) = xdg.get("default_timeout") {
            if let Ok(n) = raw.trim().parse::<u64>() {
                if (1..=crate::types::bounded::MAX_TIMEOUT_SECONDS).contains(&n) {
                    args.timeout_seconds = n;
                }
            }
        }
    }
    if args.retries == crate::types::bounded::DEFAULT_RETRIES {
        if let Some(raw) = xdg.get("default_retries") {
            if let Ok(n) = raw.trim().parse::<u32>() {
                if n <= crate::types::bounded::MAX_RETRIES {
                    args.retries = n;
                }
            }
        }
    }
    if args.pages == crate::types::bounded::DEFAULT_PAGES {
        if let Some(raw) = xdg.get("default_pages") {
            if let Ok(n) = raw.trim().parse::<u32>() {
                if (1..=crate::types::bounded::MAX_PAGES).contains(&n) {
                    args.pages = n;
                }
            }
        }
    }
    if args.num_results.is_none() {
        if let Some(raw) = xdg.get("default_num_results") {
            if let Ok(n) = raw.trim().parse::<u32>() {
                if (1..=crate::types::bounded::MAX_RESULT_COUNT).contains(&n) {
                    args.num_results = Some(n);
                }
            }
        }
    }
    if args.max_content_length == crate::types::bounded::DEFAULT_CONTENT_LENGTH {
        if let Some(raw) = xdg.get("default_max_content_length") {
            if let Ok(n) = raw.trim().parse::<usize>() {
                if (1..=crate::types::bounded::MAX_CONTENT_LENGTH).contains(&n) {
                    args.max_content_length = n;
                }
            }
        }
    }
    if args.per_host_limit == crate::types::bounded::DEFAULT_PER_HOST_LIMIT {
        if let Some(raw) = xdg.get("default_per_host_limit") {
            if let Ok(n) = raw.trim().parse::<u32>() {
                if (1..=crate::types::bounded::MAX_PER_HOST_LIMIT).contains(&n) {
                    args.per_host_limit = n;
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn apply_timeout_only_when_cli_at_default() {
        let mut root = RootArgs::try_parse_from(["ddg", "q"]).expect("parse");
        assert_eq!(root.global_timeout_seconds, DEFAULT_GLOBAL_TIMEOUT);
        let mut xdg = UserConfig::default();
        xdg.values
            .insert("default_global_timeout".into(), "90".into());
        apply_user_config_to_root(&mut root, &xdg);
        assert_eq!(root.global_timeout_seconds, 90);

        root.global_timeout_seconds = 120;
        apply_user_config_to_root(&mut root, &xdg);
        assert_eq!(root.global_timeout_seconds, 120);
    }

    #[test]
    fn apply_proxy_and_vertical_and_fetch() {
        let mut root = RootArgs::try_parse_from(["ddg", "q"]).expect("parse");
        let mut xdg = UserConfig::default();
        xdg.values
            .insert("proxy_url".into(), "http://127.0.0.1:9".into());
        xdg.values.insert("default_vertical".into(), "web".into());
        xdg.values
            .insert("fetch_content_default".into(), "false".into());
        apply_user_config_to_cli_args(&mut root.buscar, &xdg);
        assert_eq!(root.buscar.proxy.as_deref(), Some("http://127.0.0.1:9"));
        assert!(matches!(root.buscar.vertical, CliVertical::Web));
        assert!(root.buscar.no_fetch_content);
    }

    #[test]
    fn cli_proxy_wins_over_xdg() {
        let mut root = RootArgs::try_parse_from([
            "ddg",
            "--proxy",
            "http://cli:1",
            "q",
        ])
        .expect("parse");
        let mut xdg = UserConfig::default();
        xdg.values
            .insert("proxy_url".into(), "http://xdg:1".into());
        apply_user_config_to_cli_args(&mut root.buscar, &xdg);
        assert_eq!(root.buscar.proxy.as_deref(), Some("http://cli:1"));
    }

    #[test]
    fn apply_default_lang_country_only_when_cli_at_builtin() {
        let mut root = RootArgs::try_parse_from(["ddg", "q"]).expect("parse");
        assert_eq!(root.buscar.language, DEFAULT_SERP_LANG);
        assert_eq!(root.buscar.country, DEFAULT_SERP_COUNTRY);
        let mut xdg = UserConfig::default();
        xdg.values.insert("default_lang".into(), "en".into());
        xdg.values.insert("default_country".into(), "us".into());
        apply_user_config_to_cli_args(&mut root.buscar, &xdg);
        assert_eq!(root.buscar.language, "en");
        assert_eq!(root.buscar.country, "us");

        let mut root2 = RootArgs::try_parse_from(["ddg", "--lang", "es", "--country", "es", "q"])
            .expect("parse");
        apply_user_config_to_cli_args(&mut root2.buscar, &xdg);
        assert_eq!(root2.buscar.language, "es");
        assert_eq!(root2.buscar.country, "es");
    }

    #[test]
    fn apply_default_parallelism_only_when_cli_at_builtin() {
        let mut root = RootArgs::try_parse_from(["ddg", "q"]).expect("parse");
        assert_eq!(root.buscar.parallelism, crate::cli::DEFAULT_PARALLELISM);
        let mut xdg = UserConfig::default();
        xdg.values
            .insert("default_parallelism".into(), "3".into());
        apply_user_config_to_cli_args(&mut root.buscar, &xdg);
        assert_eq!(root.buscar.parallelism, 3);

        let mut root2 = RootArgs::try_parse_from(["ddg", "-p", "7", "q"]).expect("parse");
        apply_user_config_to_cli_args(&mut root2.buscar, &xdg);
        assert_eq!(root2.buscar.parallelism, 7);
    }
}
