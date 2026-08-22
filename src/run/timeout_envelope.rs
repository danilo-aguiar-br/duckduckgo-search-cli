// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure construction (global timeout → wire envelope)
//! Builds the envelope emitted when the global timeout fires.
//!
//! The caller still gets a well-formed `SearchOutput`, because an agent that
//! parses JSON on success must not have to parse prose on timeout.
//!
//! This used to be a thirty-field struct literal in the middle of [`super::run`],
//! spelling out every metadata field including the twenty that are simply
//! empty. It now names only the fields the timeout actually knows something
//! about and defers the rest to [`SearchMetadata::default`] — so adding a
//! metadata field no longer carries a silent obligation to edit this site.

use crate::cli::CliArgs;
use crate::types::{utc_now, RunId, SearchMetadata, SearchOutput};

/// The envelope for a run that exceeded `--global-timeout`.
///
/// `secs` is the configured budget, not the measured duration: the budget is
/// what the caller can act on.
pub(super) fn timed_out_output(args: &CliArgs, pre_flight: bool, secs: u64) -> SearchOutput {
    // Stream was requested but, by definition, never became effective.
    let stream_requested = args.stream_mode || args.format.enables_stream_mode();
    SearchOutput {
        query: args
            .queries
            .first()
            .cloned()
            .unwrap_or_else(|| "(timeout)".to_string()),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: utc_now(),
        region: format!("{}-{}", args.country, args.language),
        result_count: 0,
        results: vec![],
        pages_fetched: 0,
        news: None,
        news_count: None,
        error: Some(crate::error::codes::TIMEOUT.to_string()),
        message: Some(format!("global timeout of {secs}s exceeded")),
        metadata: SearchMetadata {
            execution_time_ms: secs.saturating_mul(1000),
            chrome_attempted: true,
            used_proxy: args.proxy.is_some(),
            pre_flight_executed: pre_flight,
            pre_flight_status: pre_flight.then(|| "skipped".to_string()),
            stream_requested: stream_requested.then_some(true),
            stream_effective: stream_requested.then_some(false),
            next_action_suggestion: Some(
                "Raise --global-timeout (default 180s since v0.9.9) or use \
                 --vertical web --no-fetch-content for a thinner path."
                    .into(),
            ),
            result_count_compat: Some(0),
            endpoint_used_compat: Some("html".into()),
            run_id: Some(RunId::generate()),
            ..SearchMetadata::default()
        },
    }
}
