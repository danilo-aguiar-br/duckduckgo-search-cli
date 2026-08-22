// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests for the query decomposition module.

use super::*;

fn tok() -> CancellationToken {
    CancellationToken::new()
}

#[tokio::test]
async fn heuristic_produces_five_when_no_cap() {
    let out = decompose(
        "rust async",
        crate::deep_research::SubQueryStrategy::Heuristic,
        None,
        5,
        &tok(),
        false,
    )
    .await
    .expect("ok");
    assert_eq!(out.len(), 5);
    for (i, sq) in out.iter().enumerate() {
        assert_eq!(
            sq.origin,
            SubQueryOrigin::Heuristic {
                template: HeuristicTemplate::all()[i]
            }
        );
    }
}

#[tokio::test]
async fn heuristic_caps_at_max() {
    let out = decompose(
        "rust",
        crate::deep_research::SubQueryStrategy::Heuristic,
        None,
        2,
        &tok(),
        false,
    )
    .await
    .expect("ok");
    assert_eq!(out.len(), 2);
}

#[tokio::test]
async fn heuristic_top_up_with_refinements() {
    let out = decompose(
        "rust",
        crate::deep_research::SubQueryStrategy::Heuristic,
        None,
        7,
        &tok(),
        false,
    )
    .await
    .expect("ok");
    assert_eq!(out.len(), 7);
    assert_eq!(out[5].origin, SubQueryOrigin::HeuristicRefine);
    assert_eq!(out[6].origin, SubQueryOrigin::HeuristicRefine);
}

#[tokio::test]
async fn empty_query_rejected() {
    let err = decompose(
        "   ",
        crate::deep_research::SubQueryStrategy::Heuristic,
        None,
        5,
        &tok(),
        false,
    )
    .await
    .expect_err("must fail");
    assert!(matches!(err, CliError::InvalidConfig { .. }));
}

#[tokio::test]
async fn manual_without_path_rejected() {
    let err = decompose(
        "x",
        crate::deep_research::SubQueryStrategy::Manual,
        None,
        5,
        &tok(),
        false,
    )
    .await
    .expect_err("must fail");
    assert!(matches!(err, CliError::InvalidConfig { .. }));
}

#[tokio::test]
async fn manual_reads_file_and_skips_comments() {
    let tmp = tempfile::NamedTempFile::new().expect("tmpfile");
    std::fs::write(
        tmp.path(),
        "# header comment\n\nalpha beta\n# another comment\ngamma delta\n",
    )
    .expect("write");
    let out = decompose(
        "ignored",
        crate::deep_research::SubQueryStrategy::Manual,
        Some(tmp.path()),
        10,
        &tok(),
        false,
    )
    .await
    .expect("ok");
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].text.as_str(), "alpha beta");
    assert_eq!(out[1].text.as_str(), "gamma delta");
    assert_eq!(out[0].origin, SubQueryOrigin::Manual);
}

#[tokio::test]
async fn cancel_aborts_decomposition() {
    let token = CancellationToken::new();
    token.cancel();
    let err = decompose(
        "x",
        crate::deep_research::SubQueryStrategy::Heuristic,
        None,
        5,
        &token,
        false,
    )
    .await
    .expect_err("must fail");
    assert!(matches!(err, CliError::Cancelled));
}

#[test]
fn template_labels_match_suffixes() {
    for t in HeuristicTemplate::all() {
        assert!(!t.as_str().is_empty());
        assert!(!t.suffix().is_empty());
    }
}

#[test]
fn composite_query_detects_comparison() {
    assert!(is_composite_query(
        "rust vs go",
        CompositeSignal::Comparison
    ));
    assert!(is_composite_query(
        "PostgreSQL versus MySQL",
        CompositeSignal::Comparison
    ));
    assert!(is_composite_query(
        "read or write",
        CompositeSignal::Comparison
    ));
    assert!(!is_composite_query(
        "rust async runtime",
        CompositeSignal::Comparison
    ));
}

#[test]
fn composite_query_detects_aspect() {
    assert!(is_composite_query(
        "cargo and clippy",
        CompositeSignal::Aspect
    ));
    assert!(is_composite_query(
        "cargo & clippy",
        CompositeSignal::Aspect
    ));
    assert!(is_composite_query(
        "cargo, clippy, rustfmt",
        CompositeSignal::Aspect
    ));
    assert!(!is_composite_query("rust async", CompositeSignal::Aspect));
}

#[test]
fn composite_query_detects_timeline() {
    assert!(is_composite_query(
        "history of rust",
        CompositeSignal::Timeline
    ));
    assert!(is_composite_query(
        "evolution of async runtimes",
        CompositeSignal::Timeline
    ));
    assert!(is_composite_query(
        "rust 2015 to 2024",
        CompositeSignal::Timeline
    ));
    assert!(!is_composite_query(
        "rust async runtime",
        CompositeSignal::Timeline
    ));
}

#[test]
fn composite_query_detects_opinion() {
    assert!(is_composite_query(
        "best rust web framework",
        CompositeSignal::Opinion
    ));
    assert!(is_composite_query(
        "actix vs axum review",
        CompositeSignal::Opinion
    ));
    assert!(!is_composite_query(
        "rust async runtime",
        CompositeSignal::Opinion
    ));
}

#[test]
fn composite_query_detects_cause() {
    assert!(is_composite_query(
        "why is rust hard",
        CompositeSignal::Cause
    ));
    assert!(is_composite_query(
        "causes of memory unsafety",
        CompositeSignal::Cause
    ));
    assert!(!is_composite_query(
        "rust async runtime",
        CompositeSignal::Cause
    ));
}

#[test]
fn composite_query_detects_topic() {
    assert!(is_composite_query(
        "rust: async runtime",
        CompositeSignal::Topic
    ));
    assert!(is_composite_query(
        "postgres - replication setup",
        CompositeSignal::Topic
    ));
    assert!(!is_composite_query(
        "rust async runtime",
        CompositeSignal::Topic
    ));
}

#[test]
fn heuristic_skips_redundant_comparison_template() {
    let subs = heuristic_decompose("rust vs go", 5, false).expect("heuristic");
    // Comparison template is suppressed; we should NOT see the literal
    // suffix "vs alternatives comparison" in any sub-query.
    for s in &subs {
        assert!(
            !s.text.as_str().contains("vs alternatives comparison"),
            "redundant template: {}",
            s.text
        );
    }
}

#[test]
fn heuristic_skips_redundant_cause_template() {
    let subs = heuristic_decompose("why is rust hard", 5, false).expect("heuristic");
    for s in &subs {
        assert!(
            !s.text.as_str().contains("causes effects consequences"),
            "redundant template: {}",
            s.text
        );
    }
}

#[tokio::test]
async fn decompose_rejects_empty_query() {
    let result = decompose(
        "   ",
        crate::deep_research::SubQueryStrategy::Heuristic,
        None,
        3,
        &tok(),
        false,
    )
    .await;
    assert!(matches!(result, Err(CliError::InvalidConfig { .. })));
}

#[tokio::test]
async fn decompose_rejects_zero_max() {
    let result = decompose(
        "rust",
        crate::deep_research::SubQueryStrategy::Heuristic,
        None,
        0,
        &tok(),
        false,
    )
    .await;
    assert!(matches!(result, Err(CliError::InvalidConfig { .. })));
}

#[tokio::test]
async fn decompose_respects_cancellation() {
    let token = CancellationToken::new();
    token.cancel();
    let result = decompose(
        "rust",
        crate::deep_research::SubQueryStrategy::Heuristic,
        None,
        3,
        &token,
        false,
    )
    .await;
    assert!(matches!(result, Err(CliError::Cancelled)));
}

#[tokio::test]
async fn heuristic_with_single_token_query() {
    let out = decompose(
        "rust",
        crate::deep_research::SubQueryStrategy::Heuristic,
        None,
        2,
        &tok(),
        false,
    )
    .await
    .expect("ok");
    assert_eq!(out.len(), 2);
    for s in &out {
        assert!(s.text.as_str().starts_with("rust "));
    }
}

#[tokio::test]
async fn manual_strategy_skips_blank_and_comment_lines() {
    let dir = std::env::temp_dir();
    let unique = format!(
        "dr-sub-queries-{}.txt",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let path = dir.join(unique);
    std::fs::write(
        &path,
        "# header comment\n\
             \n\
             # blank line above\n\
             rust async runtime\n\
             tokio vs async-std\n\
             \n\
             best rust web framework 2026\n",
    )
    .expect("write temp file");
    let result = decompose(
        "rust",
        crate::deep_research::SubQueryStrategy::Manual,
        Some(&path),
        10,
        &tok(),
        false,
    )
    .await
    .expect("ok");
    assert_eq!(result.len(), 3, "comments and blank lines must be ignored");
    assert_eq!(result[0].text.as_str(), "rust async runtime");
    assert_eq!(result[1].text.as_str(), "tokio vs async-std");
    assert_eq!(result[2].text.as_str(), "best rust web framework 2026");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn manual_strategy_rejects_file_with_only_comments() {
    let dir = std::env::temp_dir();
    let unique = format!(
        "dr-empty-{}.txt",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let path = dir.join(unique);
    std::fs::write(&path, "# only comments\n# nothing else\n").expect("write temp file");
    let result = decompose(
        "rust",
        crate::deep_research::SubQueryStrategy::Manual,
        Some(&path),
        10,
        &tok(),
        false,
    )
    .await;
    assert!(matches!(result, Err(CliError::InvalidConfig { .. })));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn manual_strategy_requires_path() {
    let result = decompose(
        "rust",
        crate::deep_research::SubQueryStrategy::Manual,
        None,
        3,
        &tok(),
        false,
    )
    .await;
    assert!(matches!(result, Err(CliError::InvalidConfig { .. })));
}
