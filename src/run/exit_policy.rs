// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure classification (pipeline result → process exit code)
//! Maps a successful pipeline result onto the process exit code.
//!
//! This lived inline in [`super::run`], interleaved with emission, so the
//! classification could only be exercised by driving a whole search. Every
//! predicate here is a total function over [`PipelineResult`], which is what
//! makes the table at the bottom of this file testable without a network.

use crate::error::exit_codes;
use crate::pipeline::PipelineResult;
use crate::zero_cause_is_non_legitimate;

/// True when any branch carries the `pre_flight_blocked` marker.
///
/// `Stream` is false by construction: stream emits incrementally, so the
/// per-sub-query histogram already carries the classification.
pub(super) fn pre_flight_blocked(result: &PipelineResult) -> bool {
    match result {
        PipelineResult::Single(s) => s.error.as_deref() == Some("pre_flight_blocked"),
        PipelineResult::Multi(m) => m
            .searches
            .iter()
            .any(|b| b.error.as_deref() == Some("pre_flight_blocked")),
        PipelineResult::Stream(_) => false,
    }
}

/// True when any branch attributes its zero-result to something other than an
/// honestly empty index.
pub(super) fn zero_cause_non_legitimate(result: &PipelineResult) -> bool {
    match result {
        PipelineResult::Single(s) => zero_cause_is_non_legitimate(s.metadata.zero_cause),
        PipelineResult::Multi(m) => m
            .searches
            .iter()
            .any(|b| zero_cause_is_non_legitimate(b.metadata.zero_cause)),
        PipelineResult::Stream(_) => false,
    }
}

/// True when any branch failed on Chrome transport or on configuration.
///
/// Such a failure must not read as an empty index, so it is classified from
/// the wire code first and only then from free text, which catches mislabels.
pub(super) fn chrome_transport_or_config_error(result: &PipelineResult) -> bool {
    let is_chrome = |wire: Option<&str>| {
        crate::error::is_chrome_or_config_wire(wire)
            || wire.is_some_and(crate::error::chrome_classify::message_implies_chrome_or_config)
    };
    match result {
        PipelineResult::Single(s) => is_chrome(s.error.as_deref()),
        PipelineResult::Multi(m) => m.searches.iter().any(|b| is_chrome(b.error.as_deref())),
        PipelineResult::Stream(_) => false,
    }
}

/// Decides the exit code, in the one order the contract fixes.
///
/// `strict` is the default; `--no-zero-cause-strict` turns it off and maps the
/// suspected-block codes back to the legacy zero-result code, so retry loops
/// written against older releases keep working.
pub(super) fn exit_code_for(result: &PipelineResult, total: u32, strict: bool) -> i32 {
    if chrome_transport_or_config_error(result) {
        tracing::warn!(
            "Chrome transport/config failure (GAP-WS-113/V12); emitting exit 2 (INVALID_CONFIG)"
        );
        return exit_codes::INVALID_CONFIG;
    }
    if pre_flight_blocked(result) {
        if strict {
            tracing::warn!("pre-flight detected anti-bot block; emitting exit 3");
            return exit_codes::RATE_LIMITED_OR_BLOCKED;
        }
        tracing::warn!(
            "pre-flight detected anti-bot block + BC opt-out; emitting exit 5 (ZERO_RESULTS)"
        );
        return exit_codes::ZERO_RESULTS;
    }
    if total == 0 {
        if strict && zero_cause_non_legitimate(result) {
            tracing::warn!(
                "Zero results with non-legitimate zero_cause; emitting exit 6 (SUSPECTED_BLOCK)"
            );
            tracing::warn!("  opt-out via --no-zero-cause-strict to restore exit 5");
            return exit_codes::SUSPECTED_BLOCK;
        }
        tracing::warn!("Zero results returned across all queries");
        return exit_codes::ZERO_RESULTS;
    }
    exit_codes::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{utc_now, SearchMetadata, SearchOutput, ZeroCause};

    fn output_with(error: Option<&str>, zero_cause: Option<ZeroCause>) -> PipelineResult {
        let out = SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: utc_now(),
            region: "br-pt".into(),
            result_count: 0,
            results: vec![],
            pages_fetched: 0,
            news: None,
            news_count: None,
            error: error.map(str::to_owned),
            message: None,
            metadata: SearchMetadata {
                zero_cause,
                ..SearchMetadata::default()
            },
        };
        PipelineResult::Single(Box::new(out))
    }

    #[test]
    fn a_healthy_search_with_results_exits_zero() {
        let result = output_with(None, None);
        assert_eq!(exit_code_for(&result, 10, true), exit_codes::SUCCESS);
    }

    #[test]
    fn chrome_failure_outranks_every_other_classification() {
        // Chrome down AND zero results: the transport error must win, or the
        // caller reads a broken browser as an empty index and stops retrying.
        let result = output_with(Some(crate::error::codes::CHROME_UNAVAILABLE), None);
        assert_eq!(exit_code_for(&result, 0, true), exit_codes::INVALID_CONFIG);
    }

    #[test]
    fn pre_flight_block_is_rate_limited_under_strict_and_zero_results_without() {
        let result = output_with(Some("pre_flight_blocked"), None);
        assert_eq!(
            exit_code_for(&result, 0, true),
            exit_codes::RATE_LIMITED_OR_BLOCKED
        );
        assert_eq!(exit_code_for(&result, 0, false), exit_codes::ZERO_RESULTS);
    }

    #[test]
    fn a_suspicious_zero_is_only_suspected_block_under_strict() {
        let result = output_with(None, Some(ZeroCause::AntiBot));
        assert_eq!(exit_code_for(&result, 0, true), exit_codes::SUSPECTED_BLOCK);
        assert_eq!(exit_code_for(&result, 0, false), exit_codes::ZERO_RESULTS);
    }

    #[test]
    fn an_honest_empty_index_is_zero_results_under_both_modes() {
        let result = output_with(None, Some(ZeroCause::Legitimate));
        assert_eq!(exit_code_for(&result, 0, true), exit_codes::ZERO_RESULTS);
        assert_eq!(exit_code_for(&result, 0, false), exit_codes::ZERO_RESULTS);
    }

    #[test]
    fn the_stream_variant_never_claims_a_block() {
        let result = PipelineResult::Stream(crate::parallel::StreamStats::default());
        assert!(!pre_flight_blocked(&result));
        assert!(!zero_cause_non_legitimate(&result));
        assert!(!chrome_transport_or_config_error(&result));
    }
}
