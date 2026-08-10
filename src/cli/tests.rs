// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests extracted from `src/cli/mod.rs`.

use super::*;
use clap::CommandFactory;

/// Helper: parses arguments via `RootArgs` and returns the full
/// `RootArgs` (used by tests that need access to the global
/// `global_timeout_seconds` field, which v0.7.10 B3 hoisted out
/// of `CliArgs`).
fn parse_root(argv: &[&str]) -> Result<RootArgs, clap::Error> {
    RootArgs::try_parse_from(argv)
}

/// Helper: parses arguments via root and extracts `CliArgs` (default flow = Buscar).
/// Replicates the convenience behavior of tests prior to the introduction of the subcommand.
fn parse_buscar(argv: &[&str]) -> Result<CliArgs, clap::Error> {
    let root = RootArgs::try_parse_from(argv)?;
    match root.subcommand {
        Some(Subcommand::Buscar(a)) => Ok(*a),
        Some(Subcommand::InitConfig(_))
        | Some(Subcommand::Completions(_))
        | Some(Subcommand::DeepResearch(_))
        | Some(Subcommand::Commands(_))
        | Some(Subcommand::Schema(_))
        | Some(Subcommand::Doctor(_))
        | Some(Subcommand::Locale(_))
        | Some(Subcommand::Man(_))
        | Some(Subcommand::Config(_)) => Err(clap::Error::raw(
            clap::error::ErrorKind::InvalidSubcommand,
            "subcomando nao-busca retornado em contexto que esperava busca",
        )),
        None => Ok(root.buscar),
    }
}

#[test]
fn cli_passes_schema_validation() {
    // clap's `debug_assert` validates the struct at call time.
    RootArgs::command().debug_assert();
}

#[test]
fn parses_simple_query() {
    let args = parse_buscar(&["bin", "rust async"]).expect("should parse");
    assert_eq!(args.queries, vec!["rust async".to_string()]);
    // Default is Auto (resolved at runtime via TTY detection).
    assert_eq!(args.format, CliOutputFormat::Auto);
    assert!(args.output_file.is_none());
    assert_eq!(args.timeout_seconds, 15);
    assert_eq!(args.language, "pt");
    assert_eq!(args.country, "br");
    assert_eq!(args.parallelism, DEFAULT_PARALLELISM);
    assert_eq!(args.pages, 1);
    assert_eq!(args.retries, 2);
    assert_eq!(args.endpoint, CliEndpoint::Html);
    assert!(args.time_filter.is_none());
    assert_eq!(args.safe_search, CliSafeSearch::Moderate);
    assert!(!args.stream_mode);
    assert!(args.queries_file.is_none());
    assert_eq!(args.verbose, 0);
    assert!(!args.quiet);
    assert!(!args.fetch_content);
    assert_eq!(args.max_content_length, DEFAULT_MAX_CONTENT_LENGTH);
    assert!(args.proxy.is_none());
    assert!(!args.no_proxy);
    // v0.7.10 B3 fix: `global_timeout_seconds` lives on `RootArgs`.
    // Verify the default via the root struct.
    let root = parse_root(&["bin", "q"]).unwrap();
    assert_eq!(root.global_timeout_seconds, DEFAULT_GLOBAL_TIMEOUT);
    assert!(!args.match_platform_ua);
}

#[test]
fn parses_fetch_content_and_max_content_length() {
    let args = parse_buscar(&[
        "bin",
        "--fetch-content",
        "--max-content-length",
        "500",
        "rust",
    ])
    .expect("should parse --fetch-content");
    assert!(args.fetch_content);
    assert_eq!(args.max_content_length, 500);
}

#[test]
fn proxy_and_no_proxy_are_mutually_exclusive() {
    let ok = parse_buscar(&[
        "bin",
        "--proxy",
        "http://user:pass@proxy.local:8080",
        "rust",
    ])
    .expect("should parse --proxy");
    assert_eq!(
        ok.proxy.as_deref(),
        Some("http://user:pass@proxy.local:8080")
    );
    assert!(!ok.no_proxy);

    let no = parse_buscar(&["bin", "--no-proxy", "rust"]).expect("should parse --no-proxy");
    assert!(no.no_proxy);
    assert!(no.proxy.is_none());

    let err = parse_buscar(&["bin", "--proxy", "http://x", "--no-proxy", "rust"]);
    assert!(err.is_err(), "--proxy + --no-proxy must conflict");
}

#[test]
fn parses_global_timeout() {
    // v0.7.10 B3 fix: `global_timeout_seconds` now lives on
    // `RootArgs` (global). Test via `parse_root` and the root
    // struct's field, not the inner `CliArgs`.
    let root = parse_root(&["bin", "--global-timeout", "30", "rust"]).unwrap();
    assert_eq!(root.global_timeout_seconds, 30);
}

#[test]
fn validate_max_content_length_range() {
    let mut args = parse_buscar(&["bin", "q"]).unwrap();
    args.max_content_length = 0;
    assert!(args.validate_max_content_length().is_err());
    args.max_content_length = MAX_CONTENT_LENGTH_LIMIT + 1;
    assert!(args.validate_max_content_length().is_err());
    args.max_content_length = 5000;
    assert!(args.validate_max_content_length().is_ok());
}

#[test]
fn validate_global_timeout_range() {
    // v0.7.10 B3 fix: `global_timeout_seconds` lives on `RootArgs`,
    // not on `CliArgs`. The clap `value_parser` rejects values
    // outside `1..=MAX_GLOBAL_TIMEOUT` at parse time (exit 2), so
    // this test only exercises the in-memory validation path for
    // an in-bounds value (sanity check).
    let root = parse_root(&["bin", "--global-timeout", "120", "q"]).unwrap();
    assert_eq!(root.global_timeout_seconds, 120);
    assert!(root.validate_global_timeout().is_ok());
    // `value_parser` rejects out-of-range at parse time, so we
    // verify that path here.
    assert!(
        parse_root(&["bin", "--global-timeout", "0", "q"]).is_err(),
        "clap value_parser must reject 0 (out of range 1..=3600)"
    );
    assert!(
        parse_root(&["bin", "--global-timeout", "99999", "q"]).is_err(),
        "clap value_parser must reject 99999 (out of range 1..=3600)"
    );
}

#[test]
fn validate_proxy_accepts_supported_schemes() {
    let mut args = parse_buscar(&["bin", "q"]).unwrap();
    for ok in [
        "http://proxy:8080",
        "https://user:pass@proxy:8443",
        "socks5://127.0.0.1:9050",
        "socks5h://host:1080",
    ] {
        args.proxy = Some(ok.to_string());
        assert!(
            args.validate_proxy().is_ok(),
            "proxy {ok:?} should be accepted"
        );
    }
    args.proxy = Some("ftp://proxy".to_string());
    assert!(args.validate_proxy().is_err());
    args.proxy = Some("nao-eh-uma-url".to_string());
    assert!(args.validate_proxy().is_err());
    args.proxy = None;
    assert!(args.validate_proxy().is_ok());
}

#[test]
fn parses_resilience_and_filter_flags() {
    let args = parse_buscar(&[
        "bin",
        "--pages",
        "3",
        "--retries",
        "5",
        "--endpoint",
        "lite",
        "--time-filter",
        "w",
        "--safe-search",
        "on",
        "rust",
    ])
    .expect("should parse resilience flags");
    assert_eq!(args.pages, 3);
    assert_eq!(args.retries, 5);
    assert_eq!(args.endpoint, CliEndpoint::Lite);
    assert_eq!(args.time_filter, Some(CliTimeFilter::W));
    assert_eq!(args.safe_search, CliSafeSearch::On);
}

#[test]
fn parses_vertical_default_and_custom() {
    let default_args = parse_buscar(&["bin", "q"]).unwrap();
    assert_eq!(default_args.vertical, CliVertical::All);

    let news_args = parse_buscar(&["bin", "--vertical", "news", "q"]).unwrap();
    assert_eq!(news_args.vertical, CliVertical::News);

    let all_args = parse_buscar(&["bin", "--vertical", "all", "q"]).unwrap();
    assert_eq!(all_args.vertical, CliVertical::All);

    assert!(
        parse_buscar(&["bin", "--vertical", "banana", "q"]).is_err(),
        "unknown --vertical value must be rejected by clap"
    );
}

#[test]
fn validate_pages_accepts_range_and_rejects_invalid() {
    let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
    for v in [1u32, 2, 5] {
        args.pages = v;
        assert!(args.validate_pages().is_ok(), "pages {v}");
    }
    args.pages = 0;
    assert!(args.validate_pages().is_err());
    args.pages = 6;
    assert!(args.validate_pages().is_err());
}

#[test]
fn validate_retries_rejects_above_max() {
    let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
    args.retries = 0;
    assert!(args.validate_retries().is_ok());
    args.retries = 10;
    assert!(args.validate_retries().is_ok());
    args.retries = 11;
    assert!(args.validate_retries().is_err());
}

#[test]
fn parses_multiple_positional_queries() {
    let args = parse_buscar(&["bin", "rust async", "tokio runtime", "async channels"])
        .expect("should parse multiple queries");
    assert_eq!(
        args.queries,
        vec![
            "rust async".to_string(),
            "tokio runtime".to_string(),
            "async channels".to_string(),
        ]
    );
}

#[test]
fn parses_custom_flags() {
    let args = parse_buscar(&[
        "bin",
        "--num",
        "10",
        "--format",
        "json",
        "--timeout",
        "30",
        "--lang",
        "en",
        "--country",
        "us",
        "--parallel",
        "8",
        "--verbose",
        "teste de busca",
    ])
    .expect("should parse with flags");
    assert_eq!(args.queries, vec!["teste de busca".to_string()]);
    assert_eq!(args.num_results, Some(10));
    assert_eq!(args.timeout_seconds, 30);
    assert_eq!(args.language, "en");
    assert_eq!(args.country, "us");
    assert_eq!(args.parallelism, 8);
    assert_eq!(args.verbose, 1);
}

#[test]
fn ui_lang_flag_is_global_and_distinct_from_serp_lang() {
    let root = parse_root(&["bin", "--ui-lang", "pt-BR", "hello"]).expect("should parse --ui-lang");
    assert_eq!(root.ui_lang.as_deref(), Some("pt-BR"));
    // SERP language stays independent (default pt).
    assert_eq!(root.buscar.language, "pt");
    // Accepted after a subcommand via global = true.
    let root2 = parse_root(&["bin", "locale", "--ui-lang", "en"]).expect("locale + ui-lang");
    assert!(matches!(root2.subcommand, Some(Subcommand::Locale(_))));
    assert_eq!(root2.ui_lang.as_deref(), Some("en"));
}

/// v1.0.3 GAP-AGENT-FMT: the packaged `SKILL.md` files teach
/// `<subcommand> -q -f json`. Before `format` was hoisted with
/// `global = true`, every one of these exited 2 — so every agent that
/// followed the shipped skill failed. `quiet` had been global since
/// v0.9.0; `format` was the lone omission.
#[test]
fn format_is_global_and_accepted_after_every_meta_subcommand() {
    for sub in ["doctor", "commands", "schema", "locale"] {
        let root = parse_root(&["bin", sub, "-q", "-f", "json"])
            .unwrap_or_else(|e| panic!("`{sub} -q -f json` must parse, error: {e}"));
        assert_eq!(
            root.buscar.format,
            CliOutputFormat::Json,
            "`{sub} -f json` must resolve to Json"
        );
        assert!(root.buscar.quiet, "`{sub} -q` must enable quiet");
    }
}

/// `deep-research` also accepts `-f` after the verb.
#[test]
fn format_is_accepted_after_deep_research() {
    let root = parse_root(&["bin", "deep-research", "pergunta", "-f", "json"])
        .expect("deep-research -f json must parse");
    assert_eq!(root.buscar.format, CliOutputFormat::Json);
}

/// `-o` is deliberately NOT global: `deep-research` declares its own `-o`.
/// Making it global would collide, so this test pins the decision.
#[test]
fn output_stays_local_because_deep_research_declares_its_own() {
    assert!(
        parse_root(&["bin", "doctor", "-o", "/tmp/x.json"]).is_err(),
        "-o after `doctor` must still be a usage error"
    );
}

#[test]
fn parses_short_and_long_output_flag() {
    let args = parse_buscar(&["bin", "-o", "/tmp/saida.json", "q"]).expect("should parse -o");
    assert_eq!(
        args.output_file.as_deref(),
        Some(std::path::Path::new("/tmp/saida.json"))
    );

    let args2 = parse_buscar(&["bin", "--output", "/tmp/x.md", "--format", "markdown", "q"])
        .expect("should parse --output");
    assert_eq!(
        args2.output_file.as_deref(),
        Some(std::path::Path::new("/tmp/x.md"))
    );
    assert_eq!(args2.format, CliOutputFormat::Markdown);

    let args_md = parse_buscar(&["bin", "-f", "md", "q"]).expect("should parse -f md alias");
    assert_eq!(args_md.format, CliOutputFormat::Markdown);

    assert!(
        parse_buscar(&["bin", "-f", "xml", "q"]).is_err(),
        "unknown -f value must be rejected by ValueEnum at parse time"
    );
}

#[test]
fn parses_queries_file_and_stream() {
    let args = parse_buscar(&["bin", "--queries-file", "queries.txt", "--stream"])
        .expect("should parse --queries-file and --stream");
    assert!(args.stream_mode);
    assert_eq!(
        args.queries_file.as_deref(),
        Some(std::path::Path::new("queries.txt"))
    );
    assert!(args.queries.is_empty());
}

#[test]
fn parse_format_ndjson_enables_stream_alias() {
    // GAP-E2E-51-005: `-f ndjson` is accepted and marks the stream alias.
    let args = parse_buscar(&["bin", "-f", "ndjson", "q1", "q2"]).expect("should parse -f ndjson");
    assert_eq!(args.format, CliOutputFormat::Ndjson);
    assert!(args.format.enables_stream_mode());
    // The clap flag itself stays false; build_config ORs the alias in.
    assert!(!args.stream_mode);
}

#[test]
fn parse_config_get_accepts_positional_and_flag_key() {
    // GAP-E2E-51-003: both `config get KEY` and `config get --key KEY`.
    let root = parse_root(&["bin", "config", "get", "ui_lang"]).expect("positional get");
    match root.subcommand {
        Some(Subcommand::Config(ConfigCmd::Get(args))) => {
            assert_eq!(args.key(), "ui_lang");
        }
        other => panic!("expected config get, got {other:?}"),
    }

    let root = parse_root(&["bin", "config", "get", "--key", "chrome_path"]).expect("flag get");
    match root.subcommand {
        Some(Subcommand::Config(ConfigCmd::Get(args))) => {
            assert_eq!(args.key(), "chrome_path");
        }
        other => panic!("expected config get --key, got {other:?}"),
    }
}

#[test]
fn parse_config_set_accepts_positional_and_flag_forms() {
    // GAP-E2E-51-003: `config set KEY VALUE` and `config set --key K --value V`.
    let root = parse_root(&["bin", "config", "set", "ui_lang", "en"]).expect("positional set");
    match root.subcommand {
        Some(Subcommand::Config(ConfigCmd::Set(args))) => {
            assert_eq!(args.key(), "ui_lang");
            assert_eq!(args.value(), "en");
        }
        other => panic!("expected config set positional, got {other:?}"),
    }

    let root = parse_root(&[
        "bin", "config", "set", "--key", "ui_lang", "--value", "pt-BR",
    ])
    .expect("flag set");
    match root.subcommand {
        Some(Subcommand::Config(ConfigCmd::Set(args))) => {
            assert_eq!(args.key(), "ui_lang");
            assert_eq!(args.value(), "pt-BR");
        }
        other => panic!("expected config set flags, got {other:?}"),
    }
}

#[test]
fn parse_config_unset_accepts_positional_and_flag_key() {
    let root = parse_root(&["bin", "config", "unset", "proxy_url"]).expect("positional unset");
    match root.subcommand {
        Some(Subcommand::Config(ConfigCmd::Unset(args))) => {
            assert_eq!(args.key(), "proxy_url");
        }
        other => panic!("expected config unset, got {other:?}"),
    }

    let root = parse_root(&["bin", "config", "unset", "--key", "proxy_url"]).expect("flag unset");
    match root.subcommand {
        Some(Subcommand::Config(ConfigCmd::Unset(args))) => {
            assert_eq!(args.key(), "proxy_url");
        }
        other => panic!("expected config unset --key, got {other:?}"),
    }
}

#[test]
fn parse_config_effective_subcommand() {
    let root = parse_root(&["bin", "config", "effective"]).expect("config effective");
    assert!(matches!(
        root.subcommand,
        Some(Subcommand::Config(ConfigCmd::Effective(_)))
    ));
}

#[test]
fn verbose_and_quiet_are_mutually_exclusive() {
    let result = parse_buscar(&["bin", "--verbose", "--quiet", "query qualquer"]);
    assert!(result.is_err(), "verbose + quiet must fail validation");
}

#[test]
fn short_verbose_accumulates_via_arg_action_count() {
    let v1 = parse_buscar(&["bin", "-v", "q"]).expect("-v must parse");
    assert_eq!(v1.verbose, 1, "a single -v must produce verbose == 1");
    let vv = parse_buscar(&["bin", "-vv", "q"]).expect("-vv must parse");
    assert_eq!(vv.verbose, 2, "-vv must produce verbose == 2");
    let vvv = parse_buscar(&["bin", "-vvv", "q"]).expect("-vvv must parse");
    assert_eq!(vvv.verbose, 3, "-vvv must produce verbose == 3");
    let long = parse_buscar(&["bin", "--verbose", "--verbose", "q"])
        .expect("repeated --verbose must parse");
    assert_eq!(long.verbose, 2, "repeated --verbose must accumulate to 2");
}

#[test]
fn max_concurrency_alias_sets_parallelism() {
    let args = parse_buscar(&["bin", "rust", "--max-concurrency", "7"])
        .expect("--max-concurrency alias must parse");
    assert_eq!(args.parallelism, 7);
    let short = parse_buscar(&["bin", "rust", "-p", "3"]).expect("-p still works");
    assert_eq!(short.parallelism, 3);
}

#[test]
fn shared_session_verticals_flag_parses() {
    let default = parse_buscar(&["bin", "rust"]).expect("default parse");
    assert!(
        !default.shared_session_verticals,
        "default must prefer dual multi-process verticals"
    );
    let shared = parse_buscar(&["bin", "rust", "--shared-session-verticals"])
        .expect("--shared-session-verticals must parse");
    assert!(shared.shared_session_verticals);
}

#[test]
fn validate_parallelism_accepts_allowed_range() {
    let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
    for value in [1u32, 5, 10, MAX_PARALLELISM] {
        args.parallelism = value;
        assert!(
            args.validate_parallelism().is_ok(),
            "--parallel {value} should be accepted"
        );
    }
}

#[test]
fn validate_parallelism_rejects_invalid_values() {
    let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
    args.parallelism = 0;
    assert!(args.validate_parallelism().is_err());
    args.parallelism = MAX_PARALLELISM + 1;
    assert!(args.validate_parallelism().is_err());
    args.parallelism = 100;
    assert!(args.validate_parallelism().is_err());
}

#[test]
fn parses_init_config_subcommand_with_flags() {
    let root = RootArgs::try_parse_from(["bin", "init-config", "--force", "--dry-run"])
        .expect("should parse init-config");
    let Some(Subcommand::InitConfig(args)) = root.subcommand else {
        panic!("expected InitConfig subcommand");
    };
    assert!(args.force);
    assert!(args.dry_run);
}

#[test]
fn parses_init_config_subcommand_without_flags() {
    let root = RootArgs::try_parse_from(["bin", "init-config"])
        .expect("should parse init-config without flags");
    let Some(Subcommand::InitConfig(args)) = root.subcommand else {
        panic!("expected InitConfig subcommand");
    };
    assert!(!args.force);
    assert!(!args.dry_run);
}

#[test]
fn parses_doctor_strict_flag() {
    let plain = RootArgs::try_parse_from(["bin", "doctor"]).expect("doctor");
    let Some(Subcommand::Doctor(args)) = plain.subcommand else {
        panic!("expected Doctor");
    };
    assert!(!args.strict);

    let strict = RootArgs::try_parse_from(["bin", "doctor", "--strict"]).expect("doctor --strict");
    let Some(Subcommand::Doctor(args)) = strict.subcommand else {
        panic!("expected Doctor");
    };
    assert!(args.strict);
}

#[test]
fn parses_explicit_buscar_subcommand() {
    let root = RootArgs::try_parse_from(["bin", "buscar", "rust"])
        .expect("should parse buscar subcommand");
    let Some(Subcommand::Buscar(args)) = root.subcommand else {
        panic!("expected Buscar subcommand");
    };
    assert_eq!(args.queries, vec!["rust".to_string()]);
}

#[test]
fn search_subcommand_stays_small_when_boxed() {
    // Regression guarantee: large CLI arg structs stay `Box`ed (clippy large_enum).
    // v2.0.0 boxes DeepResearchArgs after agent-ops flags (sort/dedupe/count/…).
    let enum_size = std::mem::size_of::<Subcommand>();
    assert!(
        enum_size <= 256,
        "Subcommand grew unexpectedly: {enum_size} bytes"
    );
}

#[test]
fn parse_without_subcommand_uses_search_flatten() {
    let root =
        RootArgs::try_parse_from(["bin", "rust async"]).expect("should parse without subcommand");
    assert!(root.subcommand.is_none());
    assert_eq!(root.buscar.queries, vec!["rust async".to_string()]);
}

#[test]
fn parses_per_host_limit() {
    let args = parse_buscar(&["bin", "--per-host-limit", "5", "q"]).unwrap();
    assert_eq!(args.per_host_limit, 5);
    let default = parse_buscar(&["bin", "q"]).unwrap();
    assert_eq!(default.per_host_limit, DEFAULT_PER_HOST_LIMIT);
}

#[test]
fn validate_per_host_limit_range() {
    let mut args = parse_buscar(&["bin", "q"]).unwrap();
    args.per_host_limit = 0;
    assert!(args.validate_per_host_limit().is_err());
    args.per_host_limit = MAX_PER_HOST_LIMIT + 1;
    assert!(args.validate_per_host_limit().is_err());
    args.per_host_limit = 2;
    assert!(args.validate_per_host_limit().is_ok());
}

#[test]
fn validate_timeout_seconds_rejects_zero() {
    let mut args = parse_buscar(&["bin", "q"]).unwrap();
    args.timeout_seconds = 0;
    assert!(args.validate_timeout_seconds().is_err());
    args.timeout_seconds = 1;
    assert!(args.validate_timeout_seconds().is_ok());
    args.timeout_seconds = 15;
    assert!(args.validate_timeout_seconds().is_ok());
}

/// GAP-E2E-V11-HELP-POLLUTION / V13: doctor help must not inherit SERP flags.
#[test]
fn doctor_help_excludes_serp_flags() {
    let mut cmd = RootArgs::command();
    let doctor = cmd
        .find_subcommand_mut("doctor")
        .expect("doctor subcommand");
    // Count declared arguments, not rendered lines. Line counts depend on
    // terminal width and on how long each description happens to be, so they
    // measure the formatter rather than the flag surface. The argument count
    // is exactly the pollution metric this test was written to guard.
    let arg_count = doctor.get_arguments().count();
    assert!(
        arg_count <= 12,
        "doctor exposes {arg_count} args; SERP pollution? \
             expected the doctor-local set plus global transport flags only"
    );
    let help = doctor.render_long_help().to_string();
    for ban in [
        "--vertical",
        "--shared-session-verticals",
        "--num ",
        " -n,",
        "--pages",
    ] {
        assert!(
            !help.contains(ban),
            "doctor help must not contain SERP flag {ban:?}:
{help}"
        );
    }
    // Note: parent globals (-q/--quiet) appear in the *binary* `doctor --help`
    // render path; `find_subcommand_mut` help is subcommand-local only.
}

#[test]
fn buscar_subcommand_hidden_from_root_help() {
    // GAP-WS-56: Buscar inflates the --help output because it flattens
    // every CliArgs flag. Hiding it removes the duplicate listing since
    // the no-subcommand invocation already exposes all those flags.
    let mut cmd = RootArgs::command();
    let help = cmd.render_long_help().to_string();
    assert!(
        !help.contains("buscar"),
        "buscar subcommand must be hidden from root --help, found: {help}"
    );

    // The subcommand must still be invokable even though hidden from --help.
    let root = RootArgs::try_parse_from(["bin", "buscar", "rust"])
        .expect("buscar subcommand must remain invokable when explicit");
    match root.subcommand {
        Some(Subcommand::Buscar(args)) => {
            assert_eq!(args.queries, vec!["rust".to_string()]);
        }
        other => panic!("expected Buscar subcommand, got {other:?}"),
    }
}

// v0.7.9 GAP-WS-59: --allow-lite-fallback and --pre-flight are
// declared on `RootArgs` with `global = true` so they are accepted
// both before AND after subcommands such as `deep-research`.
// The pre-v0.7.9 symptom was an `unexpected argument` exit 2 when
// the flag was passed after a positional subcommand.
#[test]
fn allow_lite_fallback_is_global() {
    // No-subcommand mode: the global flag is parsed and the
    // query is stored in the `buscar` flatten (no `Subcommand` set).
    let pre = RootArgs::try_parse_from(["bin", "--allow-lite-fallback", "rust"])
        .expect("--allow-lite-fallback must parse before any subcommand");
    assert!(pre.allow_lite_fallback);
    assert!(!pre.pre_flight);
    assert!(
        pre.subcommand.is_none(),
        "no subcommand expected, got {:?}",
        pre.subcommand
    );
    assert_eq!(pre.buscar.queries, vec!["rust".to_string()]);

    let post = RootArgs::try_parse_from([
        "bin",
        "deep-research",
        "--allow-lite-fallback",
        "--pre-flight",
        "rust",
    ])
    .expect("globals must be accepted after deep-research subcommand");
    assert!(post.allow_lite_fallback);
    assert!(post.pre_flight);
    match post.subcommand {
        Some(Subcommand::DeepResearch(_)) => {}
        other => panic!("expected DeepResearch subcommand, got {other:?}"),
    }

    let neither = RootArgs::try_parse_from(["bin", "rust"]).expect("baseline");
    assert!(!neither.allow_lite_fallback);
    assert!(!neither.pre_flight);
}

// v0.7.10 P4 #16: --require-results flag is parsed and defaults to
// false. Ensures that pipelines which don't pass the flag preserve
// the v0.7.0–v0.7.9 behavior of returning exit 0 even with zero
// aggregated results.
#[test]
fn deep_research_require_results_flag_parses() {
    // Default — flag absent → false.
    let pre = RootArgs::try_parse_from(["bin", "deep-research", "rust"]).expect("default");
    if let Some(Subcommand::DeepResearch(dr)) = pre.subcommand {
        assert!(!dr.require_results, "default must be false");
    } else {
        panic!("expected DeepResearch subcommand");
    }

    // Flag present → true.
    let post = RootArgs::try_parse_from(["bin", "deep-research", "--require-results", "rust"])
        .expect("flag present");
    if let Some(Subcommand::DeepResearch(dr)) = post.subcommand {
        assert!(
            dr.require_results,
            "--require-results must set bool to true"
        );
    } else {
        panic!("expected DeepResearch subcommand");
    }
}

// v0.9.0 GAP-WS-106 symptom B: `-q` is now `global = true` and may
// appear AFTER the `deep-research` subcommand (it previously aborted with
// `unexpected argument`).
#[test]
fn quiet_global_accepted_after_subcommand() {
    let r = RootArgs::try_parse_from(["bin", "deep-research", "rust", "-q"])
        .expect("-q must be accepted after deep-research (global)");
    assert!(
        r.buscar.quiet,
        "quiet must be true when -q appears after the subcommand"
    );
}

/// GAP-PRINT-BUDGET-QUERY / GAP-NO-INPUT / GAP-FIELDS-PROJECT clap surface.
#[test]
fn print_budget_without_query_and_agent_flags_parse() {
    let r = RootArgs::try_parse_from(["bin", "deep-research", "--print-budget"])
        .expect("--print-budget must not require QUERY");
    match r.subcommand {
        Some(Subcommand::DeepResearch(dr)) => {
            assert!(dr.print_budget);
            assert!(dr.query.is_empty());
        }
        other => panic!("expected DeepResearch, got {other:?}"),
    }
    let r2 = RootArgs::try_parse_from([
        "bin",
        "--no-input",
        "--fields",
        "url,titulo",
        "--filter",
        "host:example.com",
        "rust",
    ])
    .expect("agent-native flags must parse");
    assert!(r2.buscar.no_input);
    assert_eq!(r2.buscar.agent.fields.as_deref(), Some("url,titulo"));
    assert_eq!(
        r2.buscar.agent.result_filter.as_deref(),
        Some("host:example.com")
    );
}

// V13: after-subcommand knobs bind to DeepResearchArgs (not Root global),
// so doctor/locale help stay free of SERP flags (GAP-E2E-V11-HELP-POLLUTION).
#[test]
fn chrome_path_accepted_after_deep_research_global() {
    let r = RootArgs::try_parse_from([
        "bin",
        "deep-research",
        "rust",
        "--chrome-path",
        "/usr/lib64/chromium-browser/chromium-browser",
        "--no-news",
    ])
    .expect("--chrome-path must be accepted after deep-research (V13 deep knobs)");
    match r.subcommand {
        Some(Subcommand::DeepResearch(dr)) => {
            assert_eq!(
                dr.chrome_path.as_deref(),
                Some(std::path::Path::new(
                    "/usr/lib64/chromium-browser/chromium-browser"
                ))
            );
            let merged = merge_deep_search_defaults(&r.buscar, &dr);
            assert_eq!(
                merged.chrome_path.as_deref(),
                Some(std::path::Path::new(
                    "/usr/lib64/chromium-browser/chromium-browser"
                ))
            );
        }
        other => panic!("expected DeepResearch, got {other:?}"),
    }
}

#[test]
fn output_global_accepted_after_subcommand() {
    let r = RootArgs::try_parse_from(["bin", "deep-research", "rust", "-o", "/tmp/x.json"])
        .expect("-o must be accepted after deep-research (V13 deep knobs)");
    match r.subcommand {
        Some(Subcommand::DeepResearch(dr)) => {
            assert_eq!(
                dr.output_file.as_deref(),
                Some(std::path::Path::new("/tmp/x.json"))
            );
            let merged = merge_deep_search_defaults(&r.buscar, &dr);
            assert_eq!(
                merged.output_file.as_deref(),
                Some(std::path::Path::new("/tmp/x.json"))
            );
        }
        other => panic!("expected DeepResearch, got {other:?}"),
    }
}

// v0.9.0 GAP-WS-106 symptom A: `is_known_global_flag` covers the 8 hoisted
// flags + verbose + every CliArgs-local long flag.
#[test]
fn is_known_global_flag_covers_every_root_parser_flag() {
    for short in ["q", "o", "n", "f", "l", "c", "t", "p", "v"] {
        assert!(is_known_global_flag(short), "short -{short} must be known");
    }
    for long in [
        "quiet",
        "output",
        "num",
        "format",
        "lang",
        "country",
        "timeout",
        "parallel",
        "max-concurrency",
        "verbose",
        "queries-file",
        "pages",
        "retries",
        "endpoint",
        "vertical",
        "time-filter",
        "safe-search",
        "probe",
        "identity-profile",
        "stream",
        "fetch-content",
        "max-content-length",
        "proxy",
        "no-proxy",
        "match-platform-ua",
        "per-host-limit",
        "chrome-path",
        "no-color",
        "no-warmup",
        "no-cookie-persistence",
        "cookies-path",
        "probe-deep",
        "seed",
        "config",
    ] {
        assert!(is_known_global_flag(long), "long --{long} must be known");
    }
    assert!(
        !is_known_global_flag("zzz"),
        "a nonexistent flag must not match"
    );
}

/// v1.0.3 GAP-AGENT-HINT: the misplaced-flag hint rendered `--{token}`
/// verbatim, so anyone who typed `-f` was told to use `--f`, which clap
/// rejects with the same exit 2 as the original error. The hint has to point
/// at the canonical long form.
#[test]
fn canonical_long_flag_resolves_short_to_long_form() {
    for (short, long) in [
        ("f", "format"),
        ("q", "quiet"),
        ("n", "num"),
        ("o", "output"),
        ("l", "lang"),
        ("c", "country"),
        ("t", "timeout"),
        ("p", "parallel"),
        ("v", "verbose"),
    ] {
        assert_eq!(
            canonical_long_flag(short),
            Some(long),
            "-{short} must map to --{long}"
        );
        // Idempotent: the long form resolves to itself.
        assert_eq!(canonical_long_flag(long), Some(long));
    }
}

/// Long-only flags are already canonical; the caller uses `unwrap_or(raw)`.
#[test]
fn canonical_long_flag_returns_none_for_long_only_flags() {
    for long in ["ui-lang", "max-concurrency", "queries-file", "endpoint"] {
        assert_eq!(
            canonical_long_flag(long),
            None,
            "--{long} has no short form, so it is already canonical"
        );
    }
}
