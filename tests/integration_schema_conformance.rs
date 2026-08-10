// SPDX-License-Identifier: MIT OR Apache-2.0
//! Conformance of emitted envelopes against the schemas the CLI publishes.
//!
//! # Why this file exists
//!
//! `docs/schemas/*.json` is the contract agents code against, but until v1.0.3
//! nothing verified that the binary actually honoured it. The gap was not
//! theoretical: `--probe` and `--probe-deep` built their payloads from
//! hand-written `serde_json::json!` literals that emitted Portuguese keys
//! (`usou_chrome`, `tentou_chrome`, `cascata_motivo`) straight to stdout,
//! bypassing the `wire_keys` remap layer. `probe-deep-output.schema.json` sets
//! `additionalProperties: false`, so a strict validator rejected the whole
//! document — yet every gate stayed green.
//!
//! # Scope
//!
//! Schemas describe the **English** wire (ADR-0027). Portuguese output is a
//! post-processing remap applied at the emit boundary, so PT envelopes are
//! deliberately *not* validated against these schemas; they are covered by the
//! key-mapping assertions instead.

use chrono::{DateTime, Utc};
use duckduckgo_search_cli::deep_research::{
    DeepResearchMetadata, DeepResearchOutput, SubQueryOutcome,
};
use duckduckgo_search_cli::output::{serialize_for_wire, set_process_wire_keys, WireKeys};
use duckduckgo_search_cli::types::{
    HttpUrl, MultiSearchOutput, NewsResult, ProbeDeepReport, ProbeDeepStatus, ProbeReport,
    ProbeStatus, SearchMetadata, SearchOutput, SearchResult, ThinErrorResponse,
};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;

/// `set_process_wire_keys` writes process-wide state; tests in this binary run
/// on parallel threads and would otherwise observe each other's language.
static WIRE_LOCK: Mutex<()> = Mutex::new(());

fn schema_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/schemas")
}

fn load_schema(name: &str) -> Value {
    let path = schema_dir().join(name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read schema {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("schema {} is not valid JSON: {e}", path.display()))
}

/// Every published schema, keyed by its own `$id`.
///
/// `ndjson-event.schema.json` `$ref`s its siblings by absolute GitHub URL. The
/// validator would try to fetch those over HTTP, which needs the `resolve-http`
/// feature and, worse, would make the test suite depend on the network. Feeding
/// the local copies into the registry keeps resolution entirely offline and
/// means a `$ref` to a schema we no longer ship fails loudly here.
fn local_registry() -> jsonschema::Registry<'static> {
    let mut builder = jsonschema::Registry::new();
    let entries = std::fs::read_dir(schema_dir()).expect("docs/schemas must exist");
    for entry in entries {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).expect("readable schema");
        let Ok(doc) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if let Some(id) = doc.get("$id").and_then(Value::as_str) {
            let id = id.to_string();
            builder = builder
                .add(id.clone(), doc)
                .unwrap_or_else(|e| panic!("cannot register {id} in the registry: {e}"));
        }
    }
    builder.prepare().expect("registry must prepare offline")
}

/// Validate `instance` against `name`, panicking with every violation listed.
fn assert_conforms(name: &str, instance: &Value) {
    let schema = load_schema(name);
    let registry = local_registry();
    let validator = jsonschema::options()
        .with_registry(&registry)
        .build(&schema)
        .unwrap_or_else(|e| panic!("{name} is not a valid JSON Schema document: {e}"));
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("  at {}: {e}", e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "envelope violates {name}:\n{}\ninstance:\n{}",
        errors.join("\n"),
        serde_json::to_string_pretty(instance).unwrap_or_default()
    );
}

/// Serialize under an explicit wire language, restoring the previous setting.
fn wire_json<T: serde::Serialize>(value: &T, keys: WireKeys) -> Value {
    let _guard = WIRE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_process_wire_keys(keys);
    let rendered = serialize_for_wire(value).expect("serialize for wire");
    set_process_wire_keys(WireKeys::En);
    serde_json::from_str(&rendered).expect("wire output must be valid JSON")
}

/// The exact envelope `--probe` / `--probe-deep` write on stdout.
///
/// `wire_json` serializes the struct and stops there. The binary does one more
/// step: `probe::probe_payload_value` attaches `deep_research_budget` after
/// serialization. Validating the struct therefore validated a document the
/// product never emits, which is how an undeclared key survived a release
/// behind `additionalProperties: true`. Probe conformance now goes through the
/// real constructor; no Chrome session is needed because the attach is pure.
fn emitted_probe_json<T: serde::Serialize>(report: &T, keys: WireKeys) -> Value {
    let _guard = WIRE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let value = duckduckgo_search_cli::probe::probe_payload_value(report)
        .expect("probe payload must serialize");
    set_process_wire_keys(keys);
    let rendered = duckduckgo_search_cli::output::value_to_wire_string(value).expect("wire render");
    set_process_wire_keys(WireKeys::En);
    serde_json::from_str(&rendered).expect("probe envelope must be valid JSON")
}

// ---------------------------------------------------------------------------
// Every published schema must itself be a compilable JSON Schema document.
// Catches schema rot independently of any envelope.
// ---------------------------------------------------------------------------

#[test]
fn every_published_schema_compiles() {
    let dir = schema_dir();
    let registry = local_registry();
    let entries = std::fs::read_dir(&dir).expect("docs/schemas must exist");
    let mut checked = 0_usize;
    for entry in entries {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).expect("readable schema");
        let doc: Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()));
        // Uses the offline registry so cross-schema `$ref`s resolve locally.
        jsonschema::options()
            .with_registry(&registry)
            .build(&doc)
            .unwrap_or_else(|e| panic!("{} is not a valid JSON Schema: {e}", path.display()));
        checked += 1;
    }
    assert!(
        checked >= 24,
        "expected at least 24 published schemas, compiled {checked}"
    );
}

// ---------------------------------------------------------------------------
// --probe
// ---------------------------------------------------------------------------

#[test]
fn probe_verdict_conforms_to_published_schema() {
    let report = ProbeReport::verdict(
        "html",
        8_941,
        "https://html.duckduckgo.com/html/?q=probe".to_string(),
        true,
        41_233,
        true,
    );
    assert_conforms(
        "probe-output.schema.json",
        &emitted_probe_json(&report, WireKeys::En),
    );
}

#[test]
fn probe_blocked_verdict_conforms_to_published_schema() {
    let report = ProbeReport::verdict(
        "html",
        1_204,
        "https://html.duckduckgo.com/html/?q=probe".to_string(),
        false,
        900,
        false,
    );
    let v = emitted_probe_json(&report, WireKeys::En);
    assert_eq!(v["status"], "blocked");
    assert_conforms("probe-output.schema.json", &v);
}

#[test]
fn probe_failure_conforms_to_published_schema() {
    // Pre-1.0.3 this path emitted `"status": 0` — an integer where the schema
    // demands the ok/blocked/error enum — and omitted the required `healthy`.
    let report = ProbeReport::failure(
        "html",
        0,
        None,
        false,
        "chrome not found".to_string(),
        Some("CHROME_UNAVAILABLE".to_string()),
    );
    let v = emitted_probe_json(&report, WireKeys::En);
    assert!(v["status"].is_string(), "status must be the verdict enum");
    assert_eq!(v["healthy"], false, "healthy is required by the schema");
    assert_conforms("probe-output.schema.json", &v);
}

#[test]
fn probe_english_wire_never_emits_portuguese_chrome_keys() {
    // Direct regression guard for the v1.0.3 defect.
    let report = ProbeReport::verdict("html", 10, "https://e.x".to_string(), true, 5_000, true);
    let v = wire_json(&report, WireKeys::En);
    assert!(v.get("used_chrome").is_some(), "EN wire needs used_chrome");
    assert!(
        v.get("chrome_attempted").is_some(),
        "EN wire needs chrome_attempted"
    );
    assert!(
        v.get("usou_chrome").is_none(),
        "EN wire must not leak the Portuguese usou_chrome"
    );
    assert!(
        v.get("tentou_chrome").is_none(),
        "EN wire must not leak the Portuguese tentou_chrome"
    );
}

#[test]
fn probe_portuguese_wire_still_emits_legacy_keys() {
    let report = ProbeReport::verdict("html", 10, "https://e.x".to_string(), true, 5_000, true);
    let v = wire_json(&report, WireKeys::Pt);
    assert!(
        v.get("usou_chrome").is_some(),
        "--wire-keys pt must keep the legacy spelling"
    );
    assert!(v.get("tentou_chrome").is_some());
    assert!(v.get("used_chrome").is_none());
}

// ---------------------------------------------------------------------------
// --probe-deep  (schema sets additionalProperties: false — strict)
// ---------------------------------------------------------------------------

#[test]
fn probe_deep_captcha_conforms_to_published_schema() {
    let report = ProbeDeepReport {
        kind: duckduckgo_search_cli::types::ProbeDeepKind::ProbeDeep,
        status: ProbeDeepStatus::Captcha,
        endpoint: "html".to_string(),
        http_status: Some(403),
        latency_ms: Some(2_500),
        cascade_level: Some(1),
        cascade_reason: Some("cloudflare_anomaly_modal".to_string()),
        mitigation_suggestion: Some("rotate proxy and warm up cookies".to_string()),
        url: Some("https://html.duckduckgo.com/html/".to_string()),
        used_chrome: Some(true),
        chrome_attempted: Some(true),
        body_len: Some(12_000),
        error: None,
        error_code: None,
    };
    assert_conforms(
        "probe-deep-output.schema.json",
        &emitted_probe_json(&report, WireKeys::En),
    );
}

#[test]
fn probe_deep_failure_conforms_to_published_schema() {
    let report = ProbeDeepReport::failure(
        "html",
        Some(120),
        Some(false),
        "chrome launch failed".to_string(),
        Some("CHROME_LAUNCH".to_string()),
    );
    assert_conforms(
        "probe-deep-output.schema.json",
        &emitted_probe_json(&report, WireKeys::En),
    );
}

#[test]
fn probe_deep_english_wire_uses_cascade_reason() {
    // `cascata_motivo` is undeclared and the schema forbids extra properties,
    // so the old spelling invalidated the entire document.
    let report = ProbeDeepReport {
        kind: duckduckgo_search_cli::types::ProbeDeepKind::ProbeDeep,
        status: ProbeDeepStatus::Ok,
        endpoint: "html".to_string(),
        http_status: Some(200),
        latency_ms: Some(900),
        cascade_level: Some(0),
        cascade_reason: Some("none".to_string()),
        mitigation_suggestion: None,
        url: Some("https://html.duckduckgo.com/html/".to_string()),
        used_chrome: Some(true),
        chrome_attempted: Some(true),
        body_len: Some(30_000),
        error: None,
        error_code: None,
    };
    let en = wire_json(&report, WireKeys::En);
    assert_eq!(en["cascade_reason"], "none");
    assert!(en.get("cascata_motivo").is_none());

    let pt = wire_json(&report, WireKeys::Pt);
    assert_eq!(pt["cascata_motivo"], "none");
    assert!(pt.get("cascade_reason").is_none());
}

#[test]
fn probe_status_enums_serialize_lowercase() {
    // The schemas enumerate lowercase values; a derive change that flipped the
    // casing would silently break every consumer branching on `status`.
    assert_eq!(
        serde_json::to_value(ProbeStatus::Blocked).expect("ser"),
        Value::from("blocked")
    );
    assert_eq!(
        serde_json::to_value(ProbeStatus::Error).expect("ser"),
        Value::from("error")
    );
    assert_eq!(
        serde_json::to_value(ProbeDeepStatus::Captcha).expect("ser"),
        Value::from("captcha")
    );
}

// ---------------------------------------------------------------------------
// error-response  (thin failure envelope)
//
// The v1.0.3 audit found this schema uncovered, and three defects living in
// exactly that blind spot: eleven emit sites bypassed the wire mapper so
// `--wire-keys pt` kept English keys; `result_count` / `results` /
// `next_action_suggestion` were undeclared under `additionalProperties: false`;
// and `metadata.retentativas` was the sole Portuguese spelling in an English
// schema, so it never matched the default wire.
// ---------------------------------------------------------------------------

#[test]
fn thin_error_minimal_conforms_to_published_schema() {
    let e = ThinErrorResponse::new("invalid_config", "bad --filter syntax");
    assert_conforms("error-response.schema.json", &wire_json(&e, WireKeys::En));
}

#[test]
fn thin_error_with_search_shape_conforms_to_published_schema() {
    let e = ThinErrorResponse::new("invalid_config", "unknown bare token").with_search_shape();
    let v = wire_json(&e, WireKeys::En);
    assert_eq!(v["result_count"], 0);
    assert!(v["results"].as_array().expect("results array").is_empty());
    assert_conforms("error-response.schema.json", &v);
}

#[test]
fn thin_error_with_suggestion_conforms_to_published_schema() {
    let e = ThinErrorResponse::new("empty_query", "query is empty")
        .with_suggestion("Provide a non-empty query.");
    let v = wire_json(&e, WireKeys::En);
    assert_eq!(v["next_action_suggestion"], "Provide a non-empty query.");
    assert_conforms("error-response.schema.json", &v);
}

#[test]
fn thin_error_portuguese_wire_translates_every_key() {
    // Regression guard for the v1.0.3 defect: `print_line_stdout(&json.to_string())`
    // emitted identical bytes under `--wire-keys pt` and the EN default.
    let e = ThinErrorResponse::new("invalid_config", "boom")
        .with_suggestion("do x")
        .with_search_shape();
    let pt = wire_json(&e, WireKeys::Pt);
    assert_eq!(pt["erro"], "invalid_config");
    assert_eq!(pt["mensagem"], "boom");
    assert_eq!(pt["sugestao_proxima_acao"], "do x");
    assert_eq!(pt["quantidade_resultados"], 0);
    assert!(pt.get("resultados").is_some());
    for en in [
        "error",
        "message",
        "next_action_suggestion",
        "result_count",
        "results",
    ] {
        assert!(
            pt.get(en).is_none(),
            "PT wire must not keep the EN key `{en}`"
        );
    }
}

// ---------------------------------------------------------------------------
// search-output + search-metadata
// ---------------------------------------------------------------------------

fn sample_search_output() -> SearchOutput {
    let ts: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-06-07T00:00:00Z")
        .expect("rfc3339")
        .with_timezone(&Utc);
    SearchOutput {
        query: "rust".to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp: ts,
        region: "us-en".to_string(),
        result_count: 1,
        results: vec![SearchResult {
            position: 1,
            title: "The Rust Programming Language".to_string(),
            url: HttpUrl::try_new("https://www.rust-lang.org/").expect("valid url"),
            display_url: Some("rust-lang.org".to_string()),
            snippet: Some("A language empowering everyone.".to_string()),
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        }],
        pages_fetched: 1,
        news: Some(vec![NewsResult {
            position: 1,
            title: "Rust 2026 edition lands".to_string(),
            url: HttpUrl::try_new("https://blog.rust-lang.org/").expect("valid url"),
            source: Some("Rust Blog".to_string()),
            relative_date: Some("2 hours ago".to_string()),
            thumbnail: Some("https://blog.rust-lang.org/t.png".to_string()),
            content: None,
            content_size: None,
            content_extraction_method: None,
        }]),
        news_count: Some(1),
        error: None,
        message: None,
        metadata: maximal_metadata(),
    }
}

/// Every optional metadata field populated.
///
/// A sparse fixture is a weak guard against `additionalProperties: false`: an
/// undeclared property only shows up once something actually emits it. The
/// v1.0.3 audit hit exactly that — `flags_ignored`, `retries_configured` and
/// three stale Portuguese spellings were invisible until the fixture forced
/// every field onto the wire at once.
fn maximal_metadata() -> SearchMetadata {
    SearchMetadata {
        execution_time_ms: 1_234,
        selectors_hash: "deadbeef".to_string(),
        retries: 1,
        retries_configured: Some(2),
        used_fallback_endpoint: false,
        concurrent_fetches: 2,
        fetch_successes: 2,
        fetch_failures: 0,
        used_chrome: true,
        chrome_attempted: true,
        user_agent: "test-ua".to_string(),
        identity_used: Some("chrome-linux".to_string()),
        cascade_level: Some(0),
        used_proxy: false,
        pre_flight_fired: false,
        pre_flight_executed: false,
        pre_flight_status: Some("ok".to_string()),
        news_promo_filtered: Some(0),
        stream_requested: Some(false),
        stream_effective: Some(false),
        zero_cause: None,
        next_action_suggestion: Some("none".to_string()),
        bytes_raw: Some(40_000),
        bytes_decompressed: Some(120_000),
        cascade_level_observed: Some(0),
        result_count_compat: Some(1),
        endpoint_used_compat: Some("html".to_string()),
        vertical_used: Some("web".to_string()),
        chrome_path_resolved: Some("/usr/bin/chromium-browser".to_string()),
        chrome_channel: Some("host".to_string()),
        run_id: None,
        flags_ignored: Some(vec!["--endpoint".to_string()]),
    }
}

#[test]
fn search_output_conforms_to_published_schema() {
    let mut out = sample_search_output();
    out.fill_compat_fields();
    assert_conforms("search-output.schema.json", &wire_json(&out, WireKeys::En));
}

#[test]
fn search_metadata_conforms_to_published_schema() {
    let mut out = sample_search_output();
    out.fill_compat_fields();
    let v = wire_json(&out, WireKeys::En);
    assert_conforms("search-metadata.schema.json", &v["metadata"]);
}

// ---------------------------------------------------------------------------
// deep-research-output
// ---------------------------------------------------------------------------

/// One deep-research run with a single successful sub-query and no rows.
///
/// Shared by the output-schema test and the timeout-envelope test, which needs
/// a real `DeepResearchOutput` to exercise the partial-harvest branch.
fn sample_deep_research_output() -> DeepResearchOutput {
    DeepResearchOutput {
        kind: duckduckgo_search_cli::types::DeepResearchKind::DeepResearch,
        query: "rust async".to_string(),
        metadata: DeepResearchMetadata {
            original_query: "rust async".to_string(),
            sub_queries: vec![SubQueryOutcome {
                text: "rust async runtime".to_string(),
                strategy: "heuristic".to_string(),
                status: "ok".to_string(),
                elapsed_ms: 120,
                error: None,
                news_count: None,
                news_unavailable: None,
                zero_cause: None,
                news_error: None,
                news_diagnosis: None,
            }],
            aggregation_strategy: "rrf".to_string(),
            unique_result_count: 0,
            unique_news_count: 0,
            total_elapsed_ms: 900,
            cascade_level: None,
            used_chrome: true,
            chrome_path_resolved: Some("/usr/bin/chromium-browser".into()),
            chrome_channel: Some("host".into()),
            sub_queries_total: 1,
            sub_queries_ok: 1,
            sub_queries_error: 0,
            partial: false,
            chrome_contention_advisory: false,
        },
        results: Vec::new(),
        news: Vec::new(),
        news_count: 0,
        synth: None,
    }
}

#[test]
fn deep_research_output_conforms_to_published_schema() {
    let out = sample_deep_research_output();
    assert_conforms(
        "deep-research-output.schema.json",
        &wire_json(&out, WireKeys::En),
    );
}

// ---------------------------------------------------------------------------
// multi-search-output  (root of a multi-query run)
// ---------------------------------------------------------------------------

#[test]
fn multi_search_output_conforms_to_published_schema() {
    let mut inner = sample_search_output();
    inner.fill_compat_fields();
    let ts: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-06-07T00:00:00Z")
        .expect("rfc3339")
        .with_timezone(&Utc);
    let multi = MultiSearchOutput {
        query_count: 1,
        timestamp: ts,
        parallelism: 5,
        searches: vec![inner],
        zero_cause_histogram: std::collections::BTreeMap::new(),
    };
    assert_conforms(
        "multi-search-output.schema.json",
        &wire_json(&multi, WireKeys::En),
    );
}

// ---------------------------------------------------------------------------
// ndjson-event  (one `--stream` line)
// ---------------------------------------------------------------------------

#[test]
fn ndjson_stream_line_conforms_to_published_schema() {
    // The schema is a bare `$ref` to search-output, so this test earns its keep
    // only by (a) proving the cross-schema `$ref` resolves offline and (b)
    // pinning the two stream flags the streaming contract promises.
    let mut out = sample_search_output();
    out.metadata.stream_requested = Some(true);
    out.metadata.stream_effective = Some(true);
    out.fill_compat_fields();
    let v = wire_json(&out, WireKeys::En);
    assert_eq!(v["metadata"]["stream_requested"], true);
    assert_eq!(v["metadata"]["stream_effective"], true);
    assert_conforms("ndjson-event.schema.json", &v);
}

// ---------------------------------------------------------------------------
// deep-research-budget  (--print-budget and the underflow refusal)
// ---------------------------------------------------------------------------

/// A budget input with every knob set, so no optional key stays invisible.
fn sample_budget_input() -> duckduckgo_search_cli::budget::DeepResearchBudgetInput {
    let mut input =
        duckduckgo_search_cli::budget::DeepResearchBudgetInput::from_cli(5, true, 4, true, 2);
    input.parallelism = 5;
    // Pinned rather than sampled: `count_chrome_like_processes` reads host state,
    // which would make the fixture depend on what else is running right now.
    input.chrome_n = 12;
    input
}

#[test]
fn print_budget_payload_conforms_to_published_schema() {
    let payload = duckduckgo_search_cli::budget::print_budget_payload(
        sample_budget_input(),
        180,
        false,
        true,
        462,
    );
    assert_eq!(payload["type"], "deep_research_budget");
    assert_conforms("deep-research-budget.schema.json", &payload);
}

// ---------------------------------------------------------------------------
// deep_research_error — four disjoint shapes behind ONE discriminator
//
// `type` alone cannot route these, so `deep-research-error.schema.json` keys
// each `oneOf` branch on the PAIR (`type`, `error`). Every branch is validated
// below against a real payload: the first attempt at this contract published
// only the `budget_underflow` shape and therefore REJECTED the other three,
// which is how a published schema became worse than an absent one
// (GAP-SCHEMA-OVERLOADED-DISCRIMINATOR-001).
// ---------------------------------------------------------------------------

#[test]
fn budget_underflow_conforms_to_deep_research_error_schema() {
    let payload = duckduckgo_search_cli::budget::budget_underflow_payload(
        420,
        462,
        180,
        sample_budget_input(),
    );
    assert_eq!(payload["error"], "budget_underflow");
    assert_eq!(payload["type"], "deep_research_error");
    assert_conforms("deep-research-error.schema.json", &payload);
}

#[test]
fn sub_queries_incomplete_conforms_to_deep_research_error_schema() {
    let payload = duckduckgo_search_cli::output::sub_queries_incomplete_payload(5, 3, 2);
    assert_eq!(payload["error"], "sub_queries_incomplete");
    assert_eq!(payload["type"], "deep_research_error");
    assert_conforms("deep-research-error.schema.json", &payload);
}

/// The cancel envelope, validated as the bytes the product actually WRITES.
///
/// Arming the in-flight guard with a temp path and then driving the signal
/// handler is what the real SIGTERM path does, so the file read back here went
/// through the same wire mapper and serializer as production output. Building
/// the payload by hand in the test would validate the test's idea of the
/// envelope instead of the product's.
///
/// # Why this holds `WIRE_LOCK`
///
/// These three envelopes travel through `serialize_for_wire`, which reads a
/// PROCESS-WIDE wire-key setting. A sibling test flipping that setting to
/// Portuguese mid-run renames `error` to `erro`, and this test then reads
/// `value["error"]` as `Null`. It happened: one full-suite run failed here with
/// `left: Null, right: "cancelled"` while two other cargo processes loaded the
/// machine, and the test passed in isolation every time afterwards. The lock
/// already existed for exactly this hazard; these three were simply never
/// brought under it.
#[test]
fn cancel_envelope_conforms_to_deep_research_error_schema() {
    let _wire = WIRE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_process_wire_keys(WireKeys::En);
    let file = tempfile::NamedTempFile::new().expect("temp output file");
    let path = file.path().to_path_buf();
    let _guard = duckduckgo_search_cli::output::DeepInFlightGuard::arm(Some(&path));
    let exit = duckduckgo_search_cli::output::emit_cancel_if_deep_in_flight(
        duckduckgo_search_cli::signals::ShutdownReason::Terminate,
    );
    assert!(
        exit == 130 || exit == 143,
        "cancel must exit on a signal code, got {exit}"
    );
    let raw = std::fs::read_to_string(&path).expect("cancel envelope written to -o path");
    let value: Value = serde_json::from_str(&raw).expect("cancel envelope is valid JSON");
    assert_eq!(value["error"], "cancelled");
    assert_conforms("deep-research-error.schema.json", &value);
}

/// Drive one future to completion on a private current-thread runtime.
///
/// These envelope tests must hold `WIRE_LOCK` across the emit, because the emit
/// reads a process-wide wire-key setting. Holding a `std::sync::MutexGuard`
/// across an `.await` is a real hazard and clippy rejects it, so the async work
/// runs inside `block_on` instead: from the guard's point of view the emit is
/// one synchronous call, and no await point crosses the lock.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread runtime")
        .block_on(fut)
}

#[test]
fn timeout_envelope_without_partial_conforms_to_deep_research_error_schema() {
    let _wire = WIRE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_process_wire_keys(WireKeys::En);
    let file = tempfile::NamedTempFile::new().expect("temp output file");
    let exit = block_on(duckduckgo_search_cli::output::emit_timeout_envelope(
        180,
        None,
        Some(file.path()),
        None,
    ));
    assert_eq!(exit, 4, "global timeout exits 4");
    let raw = std::fs::read_to_string(file.path()).expect("timeout envelope written");
    let value: Value = serde_json::from_str(&raw).expect("timeout envelope is valid JSON");
    assert_eq!(value["partial"], false);
    assert_conforms("deep-research-error.schema.json", &value);
}

/// The partial branch carries nine extra keys the empty branch must not have.
#[test]
fn timeout_envelope_with_partial_conforms_to_deep_research_error_schema() {
    let _wire = WIRE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_process_wire_keys(WireKeys::En);
    let file = tempfile::NamedTempFile::new().expect("temp output file");
    let partial = sample_deep_research_output();
    let exit = block_on(duckduckgo_search_cli::output::emit_timeout_envelope(
        180,
        Some(&partial),
        Some(file.path()),
        None,
    ));
    assert_eq!(exit, 4);
    let raw = std::fs::read_to_string(file.path()).expect("timeout envelope written");
    let value: Value = serde_json::from_str(&raw).expect("timeout envelope is valid JSON");
    assert_eq!(value["partial"], true);
    assert!(value.get("partial_results").is_some());
    assert_conforms("deep-research-error.schema.json", &value);
}

/// The four error shapes do NOT share one wire policy, and that is measured.
///
/// `budget_underflow` is serialized with `payload.to_string()`, bypassing the
/// wire-key mapper, so it stays English under any `--wire-keys`. `cancelled` and
/// `timeout` go through `value_to_wire_string`, which applies the mapper, and
/// the PT map does contain `type`, `error`, `message`, `command` and `partial`.
///
/// This test exists because the schema descriptions CLAIM that asymmetry. The
/// claim was first written from reading the code, and reading the code is what
/// produced the defect these schemas were published to fix — so it is asserted
/// here instead of trusted. If a future change unifies the two policies, this
/// test fails and the schema wording must follow.
#[test]
fn error_envelope_wire_policy_differs_between_budget_and_signal_paths() {
    let refusal = duckduckgo_search_cli::budget::budget_underflow_payload(
        420,
        462,
        180,
        sample_budget_input(),
    );
    let _guard = WIRE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_process_wire_keys(WireKeys::Pt);
    let budget_bytes = refusal.to_string();
    let mapped = duckduckgo_search_cli::output::value_to_wire_string(refusal.clone())
        .expect("wire serialization");
    set_process_wire_keys(WireKeys::En);

    assert!(
        budget_bytes.contains("\"type\""),
        "the budget path bypasses the mapper, so it must stay English: {budget_bytes}"
    );
    assert!(
        mapped.contains("\"tipo\""),
        "the mapper path DOES rename `type` under pt; if this stops being true, \
         the wording in deep-research-error.schema.json is stale: {mapped}"
    );
}

/// A `budget_underflow` envelope must NOT validate against the budget schema.
///
/// This is the regression guard for the defect itself. Before the split, the
/// two shapes shared `deep-research-budget.schema.json` behind a `oneOf`, so
/// the file claimed a discriminator it only partly covered. Asserting the
/// separation here means a future merge back into one file fails loudly rather
/// than quietly re-teaching agents the wrong routing rule.
#[test]
fn budget_schema_no_longer_claims_the_error_discriminator() {
    let refusal = duckduckgo_search_cli::budget::budget_underflow_payload(
        420,
        462,
        180,
        sample_budget_input(),
    );
    let schema = load_schema("deep-research-budget.schema.json");
    let registry = local_registry();
    let validator = jsonschema::options()
        .with_registry(&registry)
        .build(&schema)
        .expect("budget schema compiles");
    assert!(
        !validator.is_valid(&refusal),
        "deep-research-budget.schema.json must describe ONE discriminator \
         (deep_research_budget); the refusal belongs to deep-research-error.schema.json"
    );
}

/// The budget envelopes are English-only, and that is a decision, not an oversight.
///
/// Their keys are budget diagnostics (`gated_seconds`, `chrome_n`), not
/// search-result fields, so `wire_keys` carries no Portuguese mapping for them
/// and `--wire-keys pt` leaves them alone. Asserting it here stops a future
/// reader from filing the sameness as the wire bug of GAP-WIRE-THIN-ERROR-001.
#[test]
fn budget_envelope_keys_are_english_in_both_wire_modes() {
    let input = sample_budget_input();
    let en = duckduckgo_search_cli::budget::print_budget_payload(input, 180, false, true, 462);
    let mut keys: Vec<&str> = en
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert!(keys.contains(&"gated_seconds"));
    assert!(keys.contains(&"parallelism"));
    assert!(
        !keys
            .iter()
            .any(|k| k.starts_with("paralelismo") || k.contains("segundos")),
        "budget envelope must not carry Portuguese keys: {keys:?}"
    );
}

// ---------------------------------------------------------------------------
// init-config  (the report the subcommand prints, and the files it writes)
// ---------------------------------------------------------------------------

/// Run `init-config` into a throwaway config home; return the dir and its report.
///
/// The dir is returned so the caller keeps the `TempDir` alive — dropping it
/// deletes the very files the next assertion wants to read.
fn run_init_config(extra: &[&str]) -> (tempfile::TempDir, Value) {
    let dir = tempfile::tempdir().expect("temp config home");
    let mut cmd = assert_cmd::Command::cargo_bin("duckduckgo-search-cli").expect("compiled binary");
    cmd.arg("--config-home").arg(dir.path()).arg("init-config");
    for a in extra {
        cmd.arg(a);
    }
    let out = cmd.output().expect("init-config runs");
    assert!(
        out.status.success(),
        "init-config failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = serde_json::from_slice::<Value>(&out.stdout)
        .expect("init-config must print one JSON object on stdout");
    (dir, report)
}

#[test]
fn init_config_report_conforms_to_published_schema() {
    let (_dir, report) = run_init_config(&[]);
    assert_eq!(report["dry_run"], false);
    assert_eq!(report["files"][0]["action"], "created");
    assert_conforms("init-config-output.schema.json", &report);
}

/// `--dry-run` and `--force` change the action enum, so both are validated too.
///
/// A fixture that only ever sees `created` would let a rename of `would_create`
/// or `overwritten` ship undetected — the same sparse-fixture blind spot that
/// hid `flags_ignored` and `retries_configured` in the v1.0.3 second-pass audit.
#[test]
fn init_config_dry_run_and_force_reports_conform_to_published_schema() {
    let (dir, dry) = run_init_config(&["--dry-run"]);
    assert_eq!(dry["dry_run"], true);
    assert_eq!(dry["files"][0]["action"], "would_create");
    assert_conforms("init-config-output.schema.json", &dry);
    drop(dir);

    // Second pass over a populated dir: once without --force, once with.
    let dir = tempfile::tempdir().expect("temp config home");
    let run = |args: &[&str]| -> Value {
        let mut cmd =
            assert_cmd::Command::cargo_bin("duckduckgo-search-cli").expect("compiled binary");
        cmd.arg("--config-home").arg(dir.path()).arg("init-config");
        for a in args {
            cmd.arg(a);
        }
        let out = cmd.output().expect("init-config runs");
        assert!(out.status.success());
        serde_json::from_slice::<Value>(&out.stdout).expect("json report")
    };
    let _created = run(&[]);
    let skipped = run(&[]);
    assert_eq!(skipped["files"][0]["action"], "skipped");
    assert_conforms("init-config-output.schema.json", &skipped);

    let overwritten = run(&["--force"]);
    assert_eq!(overwritten["files"][0]["action"], "overwritten");
    assert_conforms("init-config-output.schema.json", &overwritten);
}

// ---------------------------------------------------------------------------
// config  (the two TOML files `init-config` writes)
// ---------------------------------------------------------------------------

/// Runs the real binary's `init-config` into a throwaway config home and parses
/// the two TOML files it wrote.
///
/// Deliberately spawns the binary instead of reading `DEFAULT_SELECTORS_TOML` /
/// `DEFAULT_USER_AGENTS_TOML`: the point is to validate what a user's disk ends
/// up holding, including the write path. `--config-home` is honoured verbatim by
/// `platform::config_directory`, so the files land directly in `dir`.
fn init_config_artifacts() -> (tempfile::TempDir, Value, Value) {
    let (dir, _report) = run_init_config(&[]);

    let parse = |file: &str| -> Value {
        let path = dir.path().join(file);
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("init-config did not write {}: {e}", path.display()));
        toml::from_str::<Value>(&raw)
            .unwrap_or_else(|e| panic!("{} is not valid TOML: {e}", path.display()))
    };
    let selectors = parse("selectors.toml");
    let user_agents = parse("user-agents.toml");
    (dir, selectors, user_agents)
}

#[test]
fn init_config_artifacts_conform_to_published_schema() {
    let (_dir, selectors, user_agents) = init_config_artifacts();
    let instance = serde_json::json!({
        "selectors": selectors,
        "user_agents": user_agents,
    });
    assert_conforms("config.schema.json", &instance);
}

/// Pins the shape the previous schema got wrong, so the fiction cannot return.
///
/// Until v1.0.3 `config.schema.json` declared `user_agents` as an array of
/// strings. The real file is a table whose `agents` key holds `{ua, platform}`
/// rows, and no version of the product ever wrote the declared shape. The
/// schema was exempt from conformance testing, so nothing ever compared the two.
#[test]
fn user_agents_file_is_a_table_of_rows_not_an_array_of_strings() {
    let (_dir, _selectors, user_agents) = init_config_artifacts();
    let agents = user_agents["agents"]
        .as_array()
        .expect("user-agents.toml root key `agents` must be an array");
    assert!(!agents.is_empty(), "shipped UA pool must not be empty");
    for row in agents {
        assert!(
            row.get("ua")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty()),
            "every agent row needs a non-empty `ua`: {row}"
        );
    }

    // The pre-1.0.3 declaration, fed to the corrected schema, must be rejected.
    let legacy = serde_json::json!({
        "selectors": {},
        "user_agents": ["Mozilla/5.0 …"],
    });
    let schema = load_schema("config.schema.json");
    let registry = local_registry();
    let validator = jsonschema::options()
        .with_registry(&registry)
        .build(&schema)
        .expect("config.schema.json compiles");
    assert!(
        !validator.is_valid(&legacy),
        "the corrected schema still accepts the array-of-strings shape it replaced"
    );
}

/// A partial `selectors.toml` is a supported hotfix, so the schema must allow it.
///
/// `SelectorConfig` carries `#[serde(default)]` on every table: dropping in a
/// file with only `[html_endpoint]` patches one selector group and inherits the
/// rest from the binary. A schema that marked the tables `required` would
/// declare that workflow invalid.
#[test]
fn partial_selectors_file_conforms_to_published_schema() {
    let (_dir, _selectors, user_agents) = init_config_artifacts();
    let instance = serde_json::json!({
        "selectors": { "html_endpoint": { "snippet": ".result__snippet" } },
        "user_agents": user_agents,
    });
    assert_conforms("config.schema.json", &instance);
}

// ---------------------------------------------------------------------------
// Introspection surfaces
//
// Five commands wrote structured JSON with no published contract until v1.0.3
// (GAP-SCHEMA-INTROSPECTION-UNDECLARED-001). They were invisible to the drift
// test because that test compares two sets of FILES, and an envelope with no
// schema has no file to enumerate. Each test below runs the COMPILED binary and
// validates its real stdout, so a schema can never describe a document the
// product does not write.
// ---------------------------------------------------------------------------

/// Run the compiled binary and parse its stdout as JSON.
fn run_cli_json(args: &[&str]) -> Value {
    let out = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
        .expect("compiled binary")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("running {args:?} failed: {e}"));
    let stdout = String::from_utf8(out.stdout).expect("stdout is utf-8");
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "{args:?} did not print JSON on stdout: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

#[test]
fn commands_tree_conforms_to_published_schema() {
    let value = run_cli_json(&["-q", "-f", "json", "commands"]);
    assert_eq!(value["type"], "commands");
    assert_conforms("commands-output.schema.json", &value);
}

/// The classified `type: "error"` envelope, validated against the real stdout.
///
/// Until v1.0.4 the routing table sent `type: "error"` to
/// `error-response.schema.json`, where `error` is a plain string. This envelope
/// nests an object there, so an agent that followed the published route saw the
/// document rejected on its first keyword. The shape now has its own contract
/// and the table points at it.
#[test]
fn classified_error_envelope_conforms_to_published_schema() {
    let value = run_cli_json(&["-q", "-f", "json", "schema", "--name", "no-such-schema"]);
    assert_eq!(value["type"], "error");
    assert_eq!(value["error"]["code"], "unknown_schema");
    assert!(
        value["error"]["known"].is_array(),
        "the refusal must list the ids the catalog ships, or the agent has to guess"
    );
    assert_conforms("classified-error-output.schema.json", &value);
}

#[test]
fn schema_catalog_conforms_to_published_schema() {
    let value = run_cli_json(&["-q", "-f", "json", "schema"]);
    assert_eq!(value["type"], "schema_catalog");
    assert_conforms("schema-catalog.schema.json", &value);
}

/// The catalog's own `count` must equal the array it reports.
///
/// A truncated read is otherwise indistinguishable from a short catalog, and
/// the catalog is the entry point an agent uses to find every other contract.
#[test]
fn schema_catalog_count_matches_its_own_array() {
    let value = run_cli_json(&["-q", "-f", "json", "schema"]);
    let count = value["count"].as_u64().expect("count is an integer");
    let listed = value["schemas"]
        .as_array()
        .expect("schemas is an array")
        .len();
    assert_eq!(
        count as usize, listed,
        "catalog count disagrees with its array"
    );
}

/// The catalog must let an agent route by `type`, and route UNAMBIGUOUSLY.
///
/// Two schemas claiming one discriminator is the original defect restated: an
/// agent picks whichever it finds first and validates a valid envelope against
/// the wrong contract. Uniqueness is therefore asserted, not assumed, and every
/// discriminator the product actually emits must be present.
#[test]
fn schema_catalog_routes_every_discriminator_unambiguously() {
    let value = run_cli_json(&["-q", "-f", "json", "schema"]);
    let entries = value["schemas"].as_array().expect("schemas is an array");
    let declared: Vec<&str> = entries
        .iter()
        .filter_map(|e| e.get("discriminator").and_then(Value::as_str))
        .collect();

    for expected in [
        "deep_research_budget",
        "deep_research_error",
        "schema_catalog",
        "commands",
        "doctor",
        "probe",
        // v1.0.4: this list said `probe-deep` with a hyphen, mirroring the wrong
        // value in `DISCRIMINATOR_SCHEMAS` instead of the `probe_deep` the
        // struct emits. A test that copies the table cannot catch the table
        // being wrong; the authority is now `properties.type.const` in the
        // published schema, pinned by `discriminator_table_matches_schema_const`.
        "probe_deep",
        "deep_research",
        "error",
    ] {
        assert!(
            declared.contains(&expected),
            "catalog declares no schema for envelopes with type {expected:?}: {declared:?}"
        );
    }

    let mut sorted = declared.clone();
    sorted.sort_unstable();
    let with_duplicates = sorted.len();
    sorted.dedup();
    assert_eq!(
        with_duplicates,
        sorted.len(),
        "one discriminator maps to more than one schema, so routing is ambiguous: {declared:?}"
    );
}

#[test]
fn locale_report_conforms_to_published_schema() {
    let value = run_cli_json(&["-q", "-f", "json", "locale"]);
    assert_conforms("locale-output.schema.json", &value);
}

/// `doctor` is the envelope an agent most often parses before deciding to run.
///
/// Its exit code tracks host readiness, so it is deliberately not asserted:
/// a degraded host is a valid host, and the contract under test is the SHAPE.
#[test]
fn doctor_report_conforms_to_published_schema() {
    let value = run_cli_json(&["-q", "-f", "json", "doctor"]);
    assert_eq!(value["type"], "doctor");
    assert_conforms("doctor-output.schema.json", &value);
}

#[test]
fn config_list_conforms_to_published_schema_when_empty_and_when_set() {
    let dir = tempfile::tempdir().expect("temp config home");
    let home = dir.path().to_str().expect("utf-8 temp path");

    let empty = run_cli_json(&["-q", "--config-home", home, "config", "list"]);
    assert_conforms("config-list-output.schema.json", &empty);

    let set = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
        .expect("compiled binary")
        .args([
            "--config-home",
            home,
            "config",
            "set",
            "default_num_results",
            "7",
        ])
        .output()
        .expect("config set runs");
    assert!(
        set.status.success(),
        "config set failed: {}",
        String::from_utf8_lossy(&set.stderr)
    );

    let populated = run_cli_json(&["-q", "--config-home", home, "config", "list"]);
    assert!(
        populated["values"].get("default_num_results").is_some(),
        "config list must report the value just written: {populated}"
    );
    assert_conforms("config-list-output.schema.json", &populated);
}

/// The whole `config` family, not just the subcommand that was in hand.
///
/// Closing `config list` alone would have repeated the mistake this audit
/// exists to correct: `path`, `get`, `set`, `unset` and `effective` each write
/// their own JSON, and sweeping the family is what found the other four.
#[test]
fn every_config_subcommand_envelope_conforms_to_its_published_schema() {
    let dir = tempfile::tempdir().expect("temp config home");
    let home = dir.path().to_str().expect("utf-8 temp path");

    let path = run_cli_json(&["-q", "--config-home", home, "config", "path"]);
    assert_conforms("config-path-output.schema.json", &path);

    // `get` before the key exists: `present` false, `value` null.
    let absent = run_cli_json(&["-q", "--config-home", home, "config", "get", "proxy_url"]);
    assert_eq!(absent["present"], false);
    assert_conforms("config-get-output.schema.json", &absent);

    let set = run_cli_json(&[
        "-q",
        "--config-home",
        home,
        "config",
        "set",
        "default_num_results",
        "9",
    ]);
    assert_eq!(set["action"], "set");
    assert_conforms("config-mutation-output.schema.json", &set);

    // `get` after the write: the other branch of the same schema.
    let present = run_cli_json(&[
        "-q",
        "--config-home",
        home,
        "config",
        "get",
        "default_num_results",
    ]);
    assert_eq!(present["present"], true);
    assert_conforms("config-get-output.schema.json", &present);

    let effective = run_cli_json(&["-q", "--config-home", home, "config", "effective"]);
    assert_eq!(
        effective["values"]["default_num_results"]["source"], "xdg",
        "a stored value must resolve from the xdg layer"
    );
    assert_conforms("config-effective-output.schema.json", &effective);

    let unset = run_cli_json(&[
        "-q",
        "--config-home",
        home,
        "config",
        "unset",
        "default_num_results",
    ]);
    assert_eq!(unset["action"], "unset");
    assert_eq!(unset["removed"], true);
    assert_conforms("config-mutation-output.schema.json", &unset);

    // Unsetting an already-absent key is not an error; `removed` is how a
    // caller tells the two outcomes apart, since the exit code cannot.
    let again = run_cli_json(&[
        "-q",
        "--config-home",
        home,
        "config",
        "unset",
        "default_num_results",
    ]);
    assert_eq!(again["removed"], false);
    assert_conforms("config-mutation-output.schema.json", &again);
}

// ---------------------------------------------------------------------------
// Coverage ledger
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Bidirectional schema drift detection.
//
// `assert_conforms` answers one question: is this document legal? It cannot
// answer the two that actually caught the v1.0.4 defects.
//
//   UNDOCUMENTED_FIELD — the envelope carries a key the schema never declares.
//     Legal whenever the object is open, and `probe-output.schema.json` was the
//     one open object in the whole set. `deep_research_budget` rode through it
//     for a release.
//
//   MISSING_FIELD — the schema declares a property no envelope ever carries.
//     Always legal, because declaring a property does not require it. That is
//     how `identity_used` sat in `probe-deep-output.schema.json` describing a
//     field `ProbeDeepReport` does not have.
//
// Both are set differences over the top-level keys, and neither is expressible
// as a JSON Schema keyword — which is exactly why they need their own ruler.
// ---------------------------------------------------------------------------

/// Top-level property names a schema declares.
fn declared_properties(schema_name: &str) -> std::collections::BTreeSet<String> {
    load_schema(schema_name)
        .get("properties")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// Assert both drift directions for one schema against a corpus of envelopes.
///
/// The corpus must cover every optional property at least once. That is the
/// point: a property no fixture can produce is either dead in the product or
/// fiction in the schema, and both deserve a failing test rather than silence.
fn assert_no_schema_drift(schema_name: &str, corpus: &[Value]) {
    assert!(
        !corpus.is_empty(),
        "{schema_name}: empty corpus — the ruler would pass without measuring anything"
    );
    let declared = declared_properties(schema_name);
    assert!(
        !declared.is_empty(),
        "{schema_name} declares no properties; drift detection is meaningless here"
    );

    let mut emitted = std::collections::BTreeSet::new();
    let mut undocumented = std::collections::BTreeSet::new();
    for instance in corpus {
        let obj = instance
            .as_object()
            .unwrap_or_else(|| panic!("{schema_name}: corpus entry is not a JSON object"));
        for key in obj.keys() {
            emitted.insert(key.clone());
            if !declared.contains(key) {
                undocumented.insert(key.clone());
            }
        }
    }
    let missing: Vec<&String> = declared.difference(&emitted).collect();

    assert!(
        undocumented.is_empty(),
        "UNDOCUMENTED_FIELD in {schema_name}: emitted but never declared: {undocumented:?}\n\
         Declare them, or stop emitting them. An open object is not a contract."
    );
    assert!(
        missing.is_empty(),
        "MISSING_FIELD in {schema_name}: declared but never emitted: {missing:?}\n\
         Add an envelope to the corpus that carries them, or delete them from the schema."
    );
}

#[test]
fn probe_envelope_has_no_schema_drift_in_either_direction() {
    let healthy = ProbeReport::verdict(
        "html",
        8_941,
        "https://html.duckduckgo.com/html/?q=probe".to_string(),
        true,
        41_233,
        true,
    );
    let failed = ProbeReport::failure(
        "html",
        0,
        Some("https://html.duckduckgo.com/html/?q=probe".to_string()),
        false,
        "chrome not found".to_string(),
        Some("CHROME_UNAVAILABLE".to_string()),
    );
    assert_no_schema_drift(
        "probe-output.schema.json",
        &[
            emitted_probe_json(&healthy, WireKeys::En),
            emitted_probe_json(&failed, WireKeys::En),
        ],
    );
}

#[test]
fn probe_deep_envelope_has_no_schema_drift_in_either_direction() {
    let captcha = ProbeDeepReport {
        kind: duckduckgo_search_cli::types::ProbeDeepKind::ProbeDeep,
        status: ProbeDeepStatus::Captcha,
        endpoint: "html".to_string(),
        http_status: Some(403),
        latency_ms: Some(2_500),
        cascade_level: Some(1),
        cascade_reason: Some("cloudflare_anomaly_modal".to_string()),
        mitigation_suggestion: Some("rotate proxy and retry".to_string()),
        url: Some("https://duckduckgo.com/?q=probe".to_string()),
        used_chrome: Some(true),
        chrome_attempted: Some(true),
        body_len: Some(12_004),
        error: Some("interstitial detected".to_string()),
        error_code: Some("ANTI_BOT".to_string()),
    };
    assert_no_schema_drift(
        "probe-deep-output.schema.json",
        &[emitted_probe_json(&captcha, WireKeys::En)],
    );
}

#[test]
fn classified_error_envelope_has_no_schema_drift_in_either_direction() {
    let value = run_cli_json(&["-q", "-f", "json", "schema", "--name", "no-such-schema"]);
    assert_no_schema_drift("classified-error-output.schema.json", &[value]);
}

/// `--max-output-bytes` must bite on the introspection surfaces too.
///
/// The cap was enforced inside `emit_payload`, which only search and
/// deep-research travel. Every other surface reached stdout through
/// `print_line_stdout` / `emit_wire_line`, so the flag parsed, validated and
/// then did nothing — the caller believed a budget was in force while the full
/// envelope went out. Enforcement now sits at `write_to_stdout`, the single
/// choke point every byte passes through.
#[test]
fn max_output_bytes_is_enforced_on_introspection_surfaces() {
    for args in [
        vec!["-q", "-f", "json", "--max-output-bytes", "32", "commands"],
        vec!["-q", "-f", "json", "--max-output-bytes", "32", "schema"],
        vec!["-q", "-f", "json", "--max-output-bytes", "32", "locale"],
    ] {
        let out = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
            .expect("compiled binary")
            .args(&args)
            .output()
            .unwrap_or_else(|e| panic!("running {args:?} failed: {e}"));
        assert!(
            !out.status.success(),
            "{args:?} must refuse rather than print past the cap; stdout was {}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(
            out.stdout.is_empty(),
            "{args:?} printed {} bytes despite a 32-byte cap",
            out.stdout.len()
        );
    }
}

/// The English-only wire of the introspection surfaces is a DECISION.
///
/// `emit_wire_line`'s doc states that `config`, `schema`, `commands` and
/// `locale` deliberately stay English and keep using `print_line_stdout`. That
/// decision was written in a doc comment and asserted nowhere, so the next
/// reader had no way to tell it apart from the bypass bug that
/// GAP-WIRE-THIN-ERROR-001 fixed in eleven other places.
///
/// The consequence is real and is pinned here: under `--wire-keys pt` a probe
/// envelope routes on `tipo` while a doctor envelope routes on `type`. Anyone
/// unifying the two has to delete this test on purpose.
#[test]
fn introspection_surfaces_stay_english_under_portuguese_wire_keys() {
    let doctor = run_cli_json(&["-q", "-f", "json", "--wire-keys", "pt", "doctor"]);
    assert_eq!(
        doctor["type"], "doctor",
        "doctor keeps the English discriminator key under --wire-keys pt"
    );
    assert!(
        doctor.get("tipo").is_none(),
        "doctor must not translate its discriminator; it bypasses the mapper by design"
    );

    let commands = run_cli_json(&["-q", "-f", "json", "--wire-keys", "pt", "commands"]);
    assert_eq!(commands["type"], "commands");

    // The other half of the asymmetry: probe DOES translate, because it travels
    // the mapper. Asserted on the struct so no Chrome session is needed.
    let report = ProbeReport::verdict("html", 10, "https://e.x".to_string(), true, 5_000, true);
    let pt = wire_json(&report, WireKeys::Pt);
    assert_eq!(
        pt["tipo"], "probe",
        "probe translates the discriminator key; that is the asymmetry"
    );
}

/// Schemas validated only through a `$ref` from a parent envelope.
///
/// These have no `assert_conforms` call of their own and never will: nothing
/// emits them alone. Each entry must name the parent that reaches them, so the
/// claim is checkable by a reader instead of taken on trust.
const TRANSITIVELY_COVERED: &[(&str, &str)] = &[
    (
        "search-result.schema.json",
        "reached through `$ref` from search-output; the fixture populates one result row",
    ),
    (
        "news-result.schema.json",
        "reached through `$ref` from search-output; the fixture populates one news row",
    ),
];

/// Every `assert_conforms("<file>", …)` written anywhere under `tests/`.
///
/// Reading the call sites instead of a hand-maintained list is the whole point:
/// see [`every_published_schema_is_covered`]. Scans the whole directory rather
/// than this one file, so splitting the suite cannot silently shrink coverage.
fn harvest_asserted_schemas() -> std::collections::BTreeSet<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut found = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(&dir).expect("tests dir is readable") {
        let path = entry.expect("readable entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("readable test file");
        let mut rest = text.as_str();
        while let Some(idx) = rest.find("assert_conforms(") {
            rest = &rest[idx + "assert_conforms(".len()..];
            // The schema file name is the next string literal, on this line or
            // the next — rustfmt breaks the call when the arguments are long.
            let Some(open) = rest.find('"') else { break };
            let after = &rest[open + 1..];
            let Some(close) = after.find('"') else { break };
            let name = &after[..close];
            if name.ends_with(".schema.json") {
                found.insert(name.to_string());
            }
        }
    }
    found
}

/// Fails when a published schema is not actually validated by a live test.
///
/// # Why the hand-written ledger was replaced
///
/// The v1.0.3 ledger was a `const COVERED: &[&str]` compared against the
/// contents of `docs/schemas`. It answered "is this file named in a list?",
/// never "does a test actually run against it". Deleting a conformance test
/// left the ledger green, because the name stayed in the list — the ruler
/// meant to prove coverage could not detect the absence of the very thing it
/// was proving. It also blocked splitting this file, on the stated grounds
/// that test binaries do not share a `mod`; that reason never applied, because
/// the ledger observed no test at all.
///
/// The list is now HARVESTED from the `assert_conforms` call sites across the
/// whole `tests/` directory. Deleting a test removes its call, the schema
/// becomes uncovered, and the build goes red. Splitting the suite is safe for
/// the same reason: the scan follows the calls wherever they move.
#[test]
fn every_published_schema_is_covered() {
    let asserted = harvest_asserted_schemas();
    let transitive: std::collections::BTreeSet<String> = TRANSITIVELY_COVERED
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();

    let overlap: Vec<&String> = asserted.intersection(&transitive).collect();
    assert!(
        overlap.is_empty(),
        "these have a direct `assert_conforms` call, so they are not transitive-only \
         — remove them from TRANSITIVELY_COVERED: {overlap:?}"
    );

    let mut published = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(schema_dir()).expect("docs/schemas must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            published.insert(
                path.file_name()
                    .and_then(|n| n.to_str())
                    .expect("utf-8 file name")
                    .to_string(),
            );
        }
    }

    let uncovered: Vec<&String> = published
        .iter()
        .filter(|n| !asserted.contains(*n) && !transitive.contains(*n))
        .collect();
    assert!(
        uncovered.is_empty(),
        "published schemas that no test validates: {uncovered:?}\n\
         Write an `assert_conforms(\"<file>\", &envelope)` test, or add the file to \
         TRANSITIVELY_COVERED naming the parent that `$ref`s it."
    );

    let phantom: Vec<&String> = asserted
        .iter()
        .filter(|n| !published.contains(*n))
        .collect();
    assert!(
        phantom.is_empty(),
        "tests validate against schema files that are not published: {phantom:?}"
    );

    for (name, reason) in TRANSITIVELY_COVERED {
        assert!(
            published.contains(*name),
            "TRANSITIVELY_COVERED names an unpublished schema: {name}"
        );
        assert!(
            reason.contains("$ref"),
            "{name}: state which parent reaches it through `$ref`"
        );
    }
}

/// Retired: the hand-written ledger, kept only as a compile-time reminder.
#[test]
#[ignore = "superseded by every_published_schema_is_covered, which measures instead of claiming"]
fn every_published_schema_is_covered_or_explicitly_excluded() {
    // Schemas with a test above that feeds a real envelope through the wire.
    const COVERED: &[&str] = &[
        "probe-output.schema.json",
        "probe-deep-output.schema.json",
        "error-response.schema.json",
        // The classified `type: "error"` shape, distinct from the thin one above
        // and validated against the binary's real stdout.
        "classified-error-output.schema.json",
        "search-output.schema.json",
        "search-metadata.schema.json",
        "multi-search-output.schema.json",
        "deep-research-output.schema.json",
        "ndjson-event.schema.json",
        // Reached transitively through `$ref` from search-output; the fixture
        // populates one result row and one news row so both really validate.
        "search-result.schema.json",
        "news-result.schema.json",
        // Not a stdout envelope: the two TOML files `init-config` writes to
        // disk, parsed back and validated against the aggregate contract.
        "config.schema.json",
        // Single discriminator since v1.0.3: only `--print-budget`. A test also
        // asserts the refusal does NOT validate here, so the split cannot regress.
        "deep-research-budget.schema.json",
        // All four `deep_research_error` shapes: budget underflow, signal cancel,
        // timeout with and without partials, and sub-queries incomplete.
        "deep-research-error.schema.json",
        // Introspection surfaces, each validated against the compiled binary's
        // real stdout rather than a fixture.
        "commands-output.schema.json",
        "schema-catalog.schema.json",
        "locale-output.schema.json",
        "doctor-output.schema.json",
        "config-list-output.schema.json",
        // The rest of the `config` family, swept after `list` rather than left
        // for a later audit to find.
        "config-path-output.schema.json",
        "config-get-output.schema.json",
        "config-mutation-output.schema.json",
        "config-effective-output.schema.json",
        // Four of the six action variants are exercised: created, would_create,
        // skipped, overwritten.
        "init-config-output.schema.json",
    ];
    // Exemptions must state the reason, not merely the name.
    //
    // Empty since v1.0.3. `config.schema.json` was the last entry, exempt on the
    // grounds that no emitted artifact had the `{selectors, user_agents}` shape.
    // That was true and the conclusion was still wrong: the two files exist and
    // each has a checkable shape, and going to look revealed the schema had been
    // describing a document the product never wrote. An exemption is a place
    // defects hide, so this list stays empty unless there is no artifact at all.
    const EXCLUDED: &[(&str, &str)] = &[];

    let mut uncovered = Vec::new();
    for entry in std::fs::read_dir(schema_dir()).expect("docs/schemas must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8 file name")
            .to_string();
        let known =
            COVERED.contains(&name.as_str()) || EXCLUDED.iter().any(|(n, _)| *n == name.as_str());
        if !known {
            uncovered.push(name);
        }
    }
    assert!(
        uncovered.is_empty(),
        "published schemas with no conformance test and no written exemption: {uncovered:?}\n\
         Add an `assert_conforms` test, or add an entry to EXCLUDED explaining why \
         no envelope can be fed to it."
    );
}
