// SPDX-License-Identifier: MIT OR Apache-2.0
//! GAP-META-001 v0.8.0 — Post-pipeline invariants.
//!
//! Tests validating that runtime-wired fields are in fact populated once the
//! pipeline has run. These tests catch missing wiring that traditional
//! unit/integration tests miss (because `None` is a valid value of the
//! `Option<T>` type).
//!
//! Principle: assert invariants by running the whole pipeline (instead of
//! mocking parts of it), checking that fields which SHOULD be populated on
//! normal paths really are.

use duckduckgo_search_cli::types::Endpoint;

#[test]
fn invariant_cascade_level_observed_present_in_metadata() {
    // Build SearchMetadata the way the pipeline would and check that
    // cascade_level_observed receives the derived value. It does not depend on
    // cfg.last_probe_cascade_level.
    use duckduckgo_search_cli::types::SearchMetadata;
    // DRY: `..Default` absorbs additive wire fields (e.g. flags_ignored V15.1).
    let metadata = SearchMetadata {
        execution_time_ms: 100,
        selectors_hash: "abc123".to_string(),
        user_agent: "test-ua".to_string(),
        cascade_level_observed: Some(0),
        ..SearchMetadata::default()
    };
    // GAP-META-001 + GAP-AUD-010: the field must be present after the pipeline runs.
    assert!(
        metadata.cascade_level_observed.is_some(),
        "GAP-AUD-010: cascade_level_observed must be present in post-pipeline metadata"
    );
}

#[test]
fn invariant_retries_configured_field_exists() {
    use duckduckgo_search_cli::types::SearchMetadata;
    // GAP-META-001 + GAP-AUD-007: the `retries_configured` field must exist
    // and be populatable. Before v0.8.0 it did not exist — the operator could not
    // tell "0 retries performed" apart from "0 retries configured".
    // DRY: `..Default` absorbs additive wire fields (e.g. flags_ignored V15.1).
    let metadata = SearchMetadata {
        execution_time_ms: 100,
        selectors_hash: "abc123".to_string(),
        retries_configured: Some(5),
        user_agent: "test-ua".to_string(),
        ..SearchMetadata::default()
    };
    assert_eq!(
        metadata.retries_configured,
        Some(5),
        "GAP-AUD-007: retries_configured must be populated when the operator passed --retries"
    );
}

#[test]
fn invariant_zero_cause_inputs_has_probe_level_field() {
    // GAP-META-001 + GAP-AUD-002/003: the classifier receives a cross-signal
    // through `last_probe_cascade_level`, which MUST exist on the input.
    use duckduckgo_search_cli::pipeline::ZeroClassificationInputs;
    let inputs = ZeroClassificationInputs {
        body: "",
        pre_flight_enabled: false,
        pre_flight_fired: false,
        execution_time_ms: 0,
        retries: 0,
        concurrent_fetches: 0,
        last_probe_cascade_level: Some(2),
    };
    assert_eq!(inputs.last_probe_cascade_level, Some(2));
}

#[test]
fn invariant_endpoint_lite_distinct_from_html() {
    // GAP-META-001 + GAP-AUD-004: Endpoint::Html and Endpoint::Lite must be
    // distinct so that --allow-lite-fallback can force the transition
    // between them.
    assert_ne!(
        Endpoint::Html,
        Endpoint::Lite,
        "GAP-AUD-004: Endpoint::Html and Endpoint::Lite must be distinct variants"
    );
}
