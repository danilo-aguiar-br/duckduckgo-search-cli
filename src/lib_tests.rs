// SPDX-License-Identifier: MIT OR Apache-2.0
//! Library unit tests (SRP split from lib.rs).

use crate::cli::{CliArgs, CliEndpoint, CliSafeSearch, RootArgs};
use crate::types::OutputFormat;
use crate::zero_cause_is_non_legitimate;

fn base_args() -> CliArgs {
    CliArgs {
        queries: vec!["rust async".to_string()],
        num_results: Some(5),
        vertical: crate::cli::CliVertical::Web,
        format: crate::cli::CliOutputFormat::Json,
        output_file: None,
        timeout_seconds: 15,
        language: "pt".to_string(),
        country: "br".to_string(),
        parallelism: 5,
        shared_session_verticals: false,
        queries_file: None,
        pages: 1,
        retries: 2,
        disable_retry: false,
        base_url_html: None,
        base_url_lite: None,
        base_url_serp: None,
        endpoint: CliEndpoint::Html,
        time_filter: None,
        safe_search: CliSafeSearch::Moderate,
        probe: false,
        identity_profile: crate::cli::CliIdentityProfile::Auto,
        stream_mode: false,
        verbose: 0,
        quiet: false,
        no_input: false,
        agent: crate::cli::AgentOpsArgs::default(),
        pretty: false,
        fetch_content: false,
        no_fetch_content: true,
        fetch_content_cap: crate::cli::DEFAULT_FETCH_CONTENT_CAP,
        max_content_length: crate::cli::DEFAULT_MAX_CONTENT_LENGTH,
        proxy: None,
        no_proxy: false,
        // v0.7.10 B3 fix: `global_timeout_seconds` is no longer on
        // `CliArgs`; it lives on `RootArgs` and is hoisted in `run`.
        match_platform_ua: false,
        per_host_limit: crate::cli::DEFAULT_PER_HOST_LIMIT,
        chrome_path: None,
        chrome_visible: false,
        chrome_headless: false,
        chrome_xvfb: false,
        chrome_session_retries: crate::error::DEFAULT_CHROME_SESSION_RETRIES,
        dump_news_html: None,
        no_color: false,
        seed: None,
        config_path: None,
        no_warmup: false,
        allow_no_warmup: false,
        no_cookie_persistence: false,
        cookies_path: None,
        probe_deep: false,
        require_results: false,
    }
}

#[test]
fn build_config_with_valid_args() {
    let args = base_args();
    let cfg = crate::runtime::build_config(&args).expect("should build config");
    assert_eq!(cfg.query.as_str(), "rust async");
    assert_eq!(cfg.queries.len(), 1);
    assert_eq!(cfg.queries[0].as_str(), "rust async");
    assert_eq!(cfg.format, OutputFormat::Json);
    assert_eq!(cfg.num_results.map(|n| n.get()), Some(5));
    assert_eq!(cfg.parallelism.get(), 5);
    assert_eq!(cfg.pages.get(), 1);
    assert!(!cfg.stream_mode);
}

#[test]
fn build_config_ndjson_format_enables_stream_mode() {
    // GAP-E2E-51-005: `-f ndjson` → domain JSON + stream_mode.
    let mut args = base_args();
    args.format = crate::cli::CliOutputFormat::Ndjson;
    args.stream_mode = false;
    args.queries = vec!["a".into(), "b".into()];
    let cfg = crate::runtime::build_config(&args).expect("should build");
    assert_eq!(cfg.format, OutputFormat::Json);
    assert!(cfg.stream_mode);
}

// v0.7.10 GAP-WS-60 regression: `build_config` must propagate
// `args.identity_profile` into `Config.identity_profile` so the
// pipeline can pin to a fixed identity.
#[test]
fn build_config_propagates_identity_profile_default_auto() {
    let args = base_args();
    let cfg = crate::runtime::build_config(&args).expect("should build config");
    assert_eq!(
        cfg.identity_profile,
        crate::cli::CliIdentityProfile::Auto,
        "default identity_profile must be Auto"
    );
}

#[test]
fn build_config_propagates_identity_profile_chrome_linux() {
    let mut args = base_args();
    args.identity_profile = crate::cli::CliIdentityProfile::ChromeLinux;
    let cfg = crate::runtime::build_config(&args).expect("should build config");
    assert_eq!(
        cfg.identity_profile,
        crate::cli::CliIdentityProfile::ChromeLinux,
        "ChromeLinux flag must reach Config"
    );
}

// GAP-WS-105 v0.8.9: multi-query + --vertical news is accepted — each
// query in the batch runs its own Chrome session in the fan-out.
// v0.9.0 GAP-WS-106: only meaningful with the `chrome` feature (without it,
// build_config downgrades to Web).
#[cfg(feature = "chrome")]
#[test]
fn build_config_accepts_multi_query_with_news_vertical() {
    let mut args = base_args();
    args.vertical = crate::cli::CliVertical::News;
    args.queries = vec!["rust".to_string(), "tokio".to_string()];
    let cfg = crate::runtime::build_config(&args)
        .expect("multi-query + --vertical news must be accepted");
    assert_eq!(cfg.vertical, crate::types::VerticalMode::News);
    assert_eq!(cfg.queries.len(), 2);
}

// GAP-WS-104 v0.8.9: --vertical propagates to Config.vertical.
// v0.9.0 GAP-WS-106: only meaningful with the `chrome` feature (without it,
// build_config downgrades to Web).
#[cfg(feature = "chrome")]
#[test]
fn build_config_propagates_vertical_all() {
    let mut args = base_args();
    args.vertical = crate::cli::CliVertical::All;
    let cfg = crate::runtime::build_config(&args).expect("should build config");
    assert_eq!(cfg.vertical, crate::types::VerticalMode::All);
}

#[test]
fn build_config_rejects_all_empty_queries() {
    let mut args = base_args();
    args.queries = vec!["   ".to_string(), "".to_string()];
    let result = crate::runtime::build_config(&args);
    assert!(result.is_err());
}

#[test]
fn clap_rejects_unknown_format_at_parse_time() {
    // Invalid formats are no longer a post-parse `build_config` concern:
    // `CliOutputFormat` ValueEnum rejects them with clap exit 2.
    use clap::Parser;
    let err = RootArgs::try_parse_from(["bin", "-f", "xml", "q"]);
    assert!(err.is_err(), "unknown -f value must fail clap parse");
}

#[test]
fn build_config_rejects_zero_parallelism() {
    let mut args = base_args();
    args.parallelism = 0;
    assert!(crate::runtime::build_config(&args).is_err());
}

#[test]
fn build_config_rejects_parallelism_above_max() {
    let mut args = base_args();
    args.parallelism = 50;
    assert!(crate::runtime::build_config(&args).is_err());
}

#[test]
fn build_config_applies_default_num_15_when_omitted() {
    // v0.4.0: when `--num` is omitted (None), the effective default is 15
    // and this auto-raises `--pages` to 2 (since 15 > 10 and pages=1 is the default).
    let mut args = base_args();
    args.num_results = None;
    args.pages = 1;
    let cfg = crate::runtime::build_config(&args).expect("should build");
    assert_eq!(
        cfg.num_results.map(|n| n.get()),
        Some(15),
        "defaults to 15 when None"
    );
    assert_eq!(cfg.pages.get(), 2, "auto-raises to ceil(15/10) = 2");
}

#[test]
fn build_config_respects_explicit_pages_above_1() {
    // If the user passes `--pages 3` explicitly, do NOT override with
    // auto-pagination, even if the effective num would require fewer.
    let mut args = base_args();
    args.num_results = Some(20);
    args.pages = 3;
    let cfg = crate::runtime::build_config(&args).expect("should build");
    assert_eq!(cfg.num_results.map(|n| n.get()), Some(20));
    assert_eq!(cfg.pages.get(), 3, "honors explicit user --pages");
}

#[test]
fn build_config_auto_paginates_when_num_above_10() {
    // Boundary cases for the auto-paginator.
    let cases = [
        (11u32, 2u32), // ceil(11/10) = 2
        (15, 2),       // ceil(15/10) = 2
        (20, 2),       // ceil(20/10) = 2
        (21, 3),       // ceil(21/10) = 3
        (45, 5),       // ceil(45/10) = 5
        (60, 5),       // ceil(60/10) = 6 but clamped to 5
    ];
    for (num, expected_pages) in cases {
        let mut args = base_args();
        args.num_results = Some(num);
        args.pages = 1;
        let cfg = crate::runtime::build_config(&args)
            .unwrap_or_else(|e| panic!("should build for num={num}: {e}"));
        assert_eq!(
            cfg.pages.get(),
            expected_pages,
            "for num={num}, pages should be {expected_pages}"
        );
    }
}

#[test]
fn build_config_no_auto_paginate_when_num_10_or_less() {
    // If effective num <= 10, keep pages=1 (no auto-pagination).
    for num in [1u32, 5, 10] {
        let mut args = base_args();
        args.num_results = Some(num);
        args.pages = 1;
        let cfg = crate::runtime::build_config(&args).expect("should build");
        assert_eq!(cfg.pages.get(), 1, "num={num} should not auto-paginate");
    }
}

#[test]
fn build_config_combines_multiple_positional_queries() {
    let mut args = base_args();
    args.queries = vec![
        "alfa".to_string(),
        "beta".to_string(),
        "alfa".to_string(), // duplicate
        "gama".to_string(),
    ];
    let cfg = crate::runtime::build_config(&args).expect("should build config");
    assert_eq!(
        cfg.queries.iter().map(|q| q.as_str()).collect::<Vec<_>>(),
        vec!["alfa", "beta", "gama"]
    );
    assert_eq!(cfg.query.as_str(), "alfa");
}

// --- GAP-WS-104 v0.8.9: VerticalNoResults is a LEGITIMATE zero (exit 5) ---

#[test]
fn vertical_no_results_is_legitimate_zero() {
    assert!(!zero_cause_is_non_legitimate(Some(
        crate::types::ZeroCause::VerticalNoResults
    )));
    assert!(!zero_cause_is_non_legitimate(Some(
        crate::types::ZeroCause::Legitimate
    )));
    assert!(!zero_cause_is_non_legitimate(None));
}

#[test]
fn blocking_zero_causes_remain_non_legitimate() {
    for cause in [
        crate::types::ZeroCause::GhostBlock,
        crate::types::ZeroCause::AntiBot,
        crate::types::ZeroCause::InvalidResponse,
        crate::types::ZeroCause::SilentFilter,
        crate::types::ZeroCause::SuspiciousZeroResults,
    ] {
        assert!(zero_cause_is_non_legitimate(Some(cause)), "{cause:?}");
    }
}
