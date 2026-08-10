// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for the news vertical (GAP-WS-104 v0.8.9).
//!
//! They cover end-to-end observable behaviour without network access:
//! - extraction from the 3 news SERP fixtures (`tests/fixtures/ddg_news_serp*.html`)
//! - the JSON envelope contract (`noticias[]`, `quantidade_noticias`,
//!   `vertical_usada`) and byte-identical compatibility of `web` mode
//! - vertical URL building (`ia=news&iar=news`) with an EndpointPolicy override
//! - configuration guards exercised through the real binary (exit 2)
//! - serde round-trip of `ZeroCause::VerticalNoResults` in kebab-case
//!
//! That `VerticalNoResults` belongs to the LEGITIMATE zero list
//! (exit 5, never 6) is covered by the unit test
//! `lib::tests::vertical_no_results_is_legitimate_zero` — the
//! `zero_cause_is_non_legitimate` function is private by design.

mod common;
use chrono::{TimeZone, Utc};

use duckduckgo_search_cli::extraction::extract_news_results_with_cfg;
use duckduckgo_search_cli::search::build_news_search_url;
use duckduckgo_search_cli::types::{
    NewsResult, SafeSearch, SearchMetadata, SearchOutput, SelectorConfig, ZeroCause,
};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;

/// `build_news_search_url` reads `serp_base_url()` (EndpointPolicy SSOT).
/// Serialize tests that install process-wide endpoint overrides.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Poison-safe policy lock (interior-mutability rules: never panic on poison).
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn load_fixture(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fixtures");
    path.push(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_duckduckgo-search-cli")
}

fn metadata_stub() -> SearchMetadata {
    let mut m = common::sample_metadata();
    m.user_agent = "Mozilla/5.0".to_string();
    m
}

fn output_stub() -> SearchOutput {
    SearchOutput {
        query: "noticias brasil".to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp: Utc.with_ymd_and_hms(2026, 7, 6, 0, 0, 0).unwrap(),
        region: "br-pt".to_string(),
        result_count: 0,
        results: vec![],
        pages_fetched: 1,
        error: None,
        message: None,
        metadata: metadata_stub(),
        news: None,
        news_count: None,
    }
}

// ---------------------------------------------------------------------------
// 1. Fixture extraction
// ---------------------------------------------------------------------------

#[test]
fn fixture_strategy_a_extracts_unique_external_cards_with_all_fields() {
    let cfg = SelectorConfig::default();
    let html = load_fixture("ddg_news_serp.html");
    let results = extract_news_results_with_cfg(&html, &cfg);

    // The fixture has 6 <article> nodes: 4 unique external ones + 1 internal
    // duckduckgo.com decoy (filtered out) + 1 duplicate URL (deduplicated).
    assert_eq!(
        results.len(),
        4,
        "expected 4 unique external results, got {}",
        results.len()
    );
    assert!(
        results
            .iter()
            .all(|r| !r.url.as_str().contains("duckduckgo.com")),
        "the internal duckduckgo.com decoy must be discarded"
    );

    // Complete fields on the first card (Portuguese relative date).
    let first = &results[0];
    assert_eq!(first.position, 1);
    assert_eq!(
        first.title,
        "Governo anuncia novo pacote de investimentos em infraestrutura"
    );
    assert_eq!(first.url, "https://exemplo-veiculo-1.com/artigo-1");
    assert_eq!(first.source.as_deref(), Some("G1"));
    assert_eq!(first.relative_date.as_deref(), Some("há 2 horas"));
    let thumb = first.thumbnail.as_deref().expect("thumbnail present");
    assert!(
        thumb.starts_with("https://"),
        "protocol-relative thumbnail must resolve to https, got {thumb:?}"
    );

    // English relative date on the second card.
    assert_eq!(results[1].source.as_deref(), Some("Reuters"));
    assert_eq!(results[1].relative_date.as_deref(), Some("3 hours ago"));

    // Dense 1-indexed positions after filtering and dedupe.
    for (i, r) in results.iter().enumerate() {
        assert_eq!(
            r.position,
            u32::try_from(i + 1).expect("position fits in u32")
        );
        assert!(
            !r.title.is_empty(),
            "empty title at position {}",
            r.position
        );
        assert!(
            r.url.as_str().starts_with("https://") || r.url.as_str().starts_with("http://"),
            "non-absolute URL at position {}: {}",
            r.position,
            r.url
        );
    }
}

#[test]
fn obfuscated_fixture_falls_back_to_strategy_b_and_extracts_title_and_url() {
    let cfg = SelectorConfig::default();
    let html = load_fixture("ddg_news_serp_ofuscada.html");
    let results = extract_news_results_with_cfg(&html, &cfg);

    // With no <article>/<h3> and fully obfuscated classes, only
    // Strategy B (class-agnostic) recovers the cards.
    assert!(
        !results.is_empty(),
        "Strategy B must extract from the obfuscated fixture"
    );
    assert_eq!(results.len(), 3);
    assert_eq!(
        results[0].title,
        "Prefeitura confirma cronograma de obras no centro da cidade"
    );
    assert_eq!(results[0].url, "https://exemplo-veiculo-5.com/nota-5");
    for (i, r) in results.iter().enumerate() {
        assert_eq!(
            r.position,
            u32::try_from(i + 1).expect("position fits in u32")
        );
        assert!(!r.title.is_empty());
        assert!(!r.url.as_str().is_empty());
    }
}

#[test]
fn empty_fixture_returns_empty_vec() {
    let cfg = SelectorConfig::default();
    let html = load_fixture("ddg_news_serp_vazia.html");
    let results = extract_news_results_with_cfg(&html, &cfg);
    assert!(
        results.is_empty(),
        "a container with no articles must produce an empty vec"
    );
}

// ---------------------------------------------------------------------------
// 2. JSON envelope contract
// ---------------------------------------------------------------------------

/// ADR-0027: `serde` serializes the **English** wire. Portuguese keys are
/// produced only by the `output::wire_keys` post-processing layer under
/// `--wire-keys pt`.
///
/// Before v1.0.3 this test asserted Portuguese keys straight out of `serde`,
/// which had been the pre-ADR-0027 contract — it failed deterministically.
/// It now pins BOTH contracts so neither can regress silently.
#[test]
fn news_envelope_serializes_en_wire_by_default() {
    let mut output = output_stub();
    output.news = Some(vec![NewsResult {
        position: 1,
        title: "Manchete".to_string(),
        url: common::http_url("https://veiculo.com/artigo"),
        source: Some("G1".to_string()),
        relative_date: Some("há 2 horas".to_string()),
        thumbnail: Some("https://img.example/t.jpg".to_string()),
        content: None,
        content_size: None,
        content_extraction_method: None,
    }]);
    output.news_count = Some(1);
    output.metadata.vertical_used = Some("news".to_string());

    let json = serde_json::to_string(&output).expect("serialization must work");

    // English wire (the default since ADR-0027).
    assert!(json.contains("\"news\""), "json={json}");
    assert!(json.contains("\"news_count\":1"), "json={json}");
    assert!(json.contains("\"vertical_used\":\"news\""), "json={json}");
    assert!(json.contains("\"position\":1"), "json={json}");
    assert!(json.contains("\"title\":\"Manchete\""), "json={json}");
    assert!(json.contains("\"url\":\"https://veiculo.com/artigo\""));
    assert!(json.contains("\"source\":\"G1\""), "json={json}");
    assert!(
        json.contains("\"relative_date\":\"há 2 horas\""),
        "json={json}"
    );
    assert!(json.contains("\"thumbnail\":\"https://img.example/t.jpg\""));

    // The Portuguese keys must NOT come out of serde.
    assert!(!json.contains("\"noticias\""), "json={json}");
    assert!(!json.contains("\"quantidade_noticias\""), "json={json}");
    assert!(!json.contains("\"vertical_usada\""), "json={json}");
}

/// Companion to the test above: the `wire_keys` layer still delivers the
/// Portuguese wire, which is the `--wire-keys pt` contract.
///
/// `set_process_wire_keys` is process-GLOBAL state, so this test takes the
/// file-level `env_lock` BEFORE mutating and restores `En` at the end. Mutating
/// global state outside the critical section is exactly the race v1.0.3 fixed in
/// `tests/integration_wiremock.rs`.
#[test]
fn news_envelope_remaps_to_pt_wire_on_demand() {
    let _guard = env_lock();
    let mut output = output_stub();
    output.news = Some(vec![NewsResult {
        position: 1,
        title: "Manchete".to_string(),
        url: common::http_url("https://veiculo.com/artigo"),
        source: Some("G1".to_string()),
        relative_date: Some("há 2 horas".to_string()),
        thumbnail: None,
        content: None,
        content_size: None,
        content_extraction_method: None,
    }]);
    output.news_count = Some(1);
    output.metadata.vertical_used = Some("news".to_string());

    use duckduckgo_search_cli::output::{set_process_wire_keys, to_wire_string, WireKeys};

    set_process_wire_keys(WireKeys::Pt);
    let json = to_wire_string(&output).expect("PT serialization must work");
    set_process_wire_keys(WireKeys::En);

    assert!(json.contains("\"noticias\""), "json={json}");
    assert!(json.contains("\"quantidade_noticias\":1"), "json={json}");
    assert!(json.contains("\"vertical_usada\":\"news\""), "json={json}");
    assert!(json.contains("\"posicao\":1"), "json={json}");
    assert!(json.contains("\"titulo\":\"Manchete\""), "json={json}");
    assert!(json.contains("\"fonte\":\"G1\""), "json={json}");
    assert!(
        json.contains("\"data_relativa\":\"há 2 horas\""),
        "json={json}"
    );
}

#[test]
fn news_envelope_omits_absent_optional_fields() {
    let mut output = output_stub();
    output.news = Some(vec![NewsResult {
        position: 1,
        title: "Manchete".to_string(),
        url: common::http_url("https://veiculo.com/artigo"),
        source: None,
        relative_date: None,
        thumbnail: None,
        content: None,
        content_size: None,
        content_extraction_method: None,
    }]);
    output.news_count = Some(1);

    let json = serde_json::to_string(&output).expect("serialization must work");
    assert!(!json.contains("\"fonte\""));
    assert!(!json.contains("\"data_relativa\""));
    assert!(!json.contains("\"thumbnail\""));
}

#[test]
fn web_mode_envelope_stays_byte_compatible_with_v088() {
    let output = output_stub();
    let json = serde_json::to_string(&output).expect("serialization must work");

    // Default web mode: NO new field may appear (byte-identical contract
    // with v0.8.8 for existing jaq consumers).
    assert!(!json.contains("\"noticias\":"));
    assert!(!json.contains("\"quantidade_noticias\":"));
    assert!(!json.contains("\"vertical_usada\":"));

    // Round-trip: the old envelope is still deserializable.
    let parsed: SearchOutput = serde_json::from_str(&json).expect("round-trip");
    assert!(parsed.news.is_none());
    assert!(parsed.news_count.is_none());
    assert!(parsed.metadata.vertical_used.is_none());
}

// ---------------------------------------------------------------------------
// 3. News vertical URL builder
// ---------------------------------------------------------------------------

#[test]
fn build_news_search_url_includes_ia_and_iar_news() {
    let _guard = env_lock();
    // Ensure defaults (no leaked EndpointPolicy from other tests).
    let _ep = common::EndpointPolicyGuard::install(None, None, None);

    let url = build_news_search_url("rust programming", "pt", "br", None, SafeSearch::Moderate);
    assert!(
        url.starts_with("https://duckduckgo.com/"),
        "news must use the main SERP, got {url}"
    );
    assert!(url.contains("ia=news&iar=news"), "missing ia/iar in {url}");
    assert!(url.contains("kl=br-pt"), "missing kl in {url}");
}

#[test]
fn build_news_search_url_encodes_the_query() {
    let _guard = env_lock();
    let _ep = common::EndpointPolicyGuard::install(None, None, None);

    let url = build_news_search_url(
        "eleições 2026 & economia",
        "pt",
        "br",
        None,
        SafeSearch::Moderate,
    );
    assert!(
        url.contains("q=elei%C3%A7%C3%B5es%202026%20%26%20economia"),
        "query must be URL-encoded, got {url}"
    );
    assert!(
        !url.contains("eleições"),
        "the raw query leaked into the URL: {url}"
    );
}

#[test]
fn build_news_search_url_respects_endpoint_policy_override() {
    let _guard = env_lock();
    // V18: EndpointPolicy SSOT (not product env BASE_URL_SERP).
    let _ep = common::EndpointPolicyGuard::serp_only("http://127.0.0.1:9/serp");
    let url = build_news_search_url("rust", "pt", "br", None, SafeSearch::Moderate);

    assert!(
        url.starts_with("http://127.0.0.1:9/serp?q=rust"),
        "EndpointPolicy serp override must apply, got {url}"
    );
    assert!(url.contains("ia=news&iar=news"));
}

// ---------------------------------------------------------------------------
// 4. Configuration guards (through the real binary — observable behaviour)
// ---------------------------------------------------------------------------

// GAP-WS-113 / V18: product env NO_CHROME is not read (GAP-SCRAPE-R2-013).
// Fail-closed via CLI `--chrome-path` to a missing binary (exit 2). Multi-query OK.
#[test]
fn binary_news_multi_query_no_chrome_fail_closed() {
    let output = Command::new(bin_path())
        .args([
            "--vertical",
            "news",
            "-q",
            "-f",
            "json",
            "--chrome-path",
            "/nonexistent/chrome-news-multi-v18",
            "rust",
            "tokio",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("binary must run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stderr.contains("aceita apenas UMA query"),
        "the multi-query guard must stay removed (GAP-WS-105); stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "GAP-WS-113: invalid --chrome-path must fail exit 2; stdout={stdout} stderr={stderr}"
    );
}

// GAP-WS-113: deep-research without usable Chrome fails closed (no auto --no-news).
#[test]
fn binary_deep_research_no_chrome_fail_closed() {
    let output = Command::new(bin_path())
        .args([
            "-q",
            "deep-research",
            "rust async",
            "--max-sub-queries",
            "1",
            "--no-fetch-content",
            "--chrome-path",
            "/nonexistent/chrome-deep-v18",
            "--global-timeout",
            "30",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("binary must run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(2),
        "GAP-WS-113: deep-research invalid chrome must exit 2; stdout={stdout}"
    );
}

// GAP-WS-113: --vertical news + invalid chrome => exit 2 (no silent web downgrade).
#[test]
fn binary_vertical_news_no_chrome_fail_closed() {
    let output = Command::new(bin_path())
        .args([
            "--vertical",
            "news",
            "-q",
            "-f",
            "json",
            "--chrome-path",
            "/nonexistent/chrome-news-v18",
            "rust",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("binary must run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(2),
        "GAP-WS-113: --vertical news invalid chrome must exit 2; stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// 5. ZeroCause::VerticalNoResults
// ---------------------------------------------------------------------------

/// ADR-0027: the serialized value is English. The Portuguese value is still
/// accepted on DESERIALIZE via `alias`, which is the compatibility contract.
#[test]
fn zero_cause_vertical_no_results_round_trip_kebab_case() {
    let json =
        serde_json::to_string(&ZeroCause::VerticalNoResults).expect("serialization must work");
    assert_eq!(json, "\"vertical-no-results\"");

    let parsed: ZeroCause = serde_json::from_str(&json).expect("round-trip EN");
    assert_eq!(parsed, ZeroCause::VerticalNoResults);

    // The historical Portuguese alias still deserializes.
    let legacy: ZeroCause =
        serde_json::from_str("\"vertical-sem-resultados\"").expect("PT alias must deserialize");
    assert_eq!(legacy, ZeroCause::VerticalNoResults);
}

#[test]
fn zero_cause_vertical_no_results_serializes_in_envelope() {
    let mut output = output_stub();
    output.metadata.zero_cause = Some(ZeroCause::VerticalNoResults);
    let json = serde_json::to_string(&output).expect("serialization must work");
    assert!(
        json.contains("\"zero_cause\":\"vertical-no-results\""),
        "json={json}"
    );
}

/// v1.0.3: `ghost_block` and `anti_bot` were the last two snake_case values in
/// an enum whose other siblings were already kebab-case. Unifying them is
/// BREAKING on serialize, so the old spellings remain as deserialization
/// aliases — without them, payloads captured by agents before 1.0.3 would stop
/// parsing.
#[test]
fn zero_cause_ghost_block_and_anti_bot_emit_kebab_case() {
    assert_eq!(
        serde_json::to_string(&ZeroCause::GhostBlock).expect("serializes"),
        "\"ghost-block\""
    );
    assert_eq!(
        serde_json::to_string(&ZeroCause::AntiBot).expect("serializes"),
        "\"anti-bot\""
    );
}

#[test]
fn zero_cause_accepts_pre_1_0_3_snake_case_aliases() {
    let ghost: ZeroCause =
        serde_json::from_str("\"ghost_block\"").expect("snake alias must deserialize");
    assert_eq!(ghost, ZeroCause::GhostBlock);

    let anti: ZeroCause =
        serde_json::from_str("\"anti_bot\"").expect("snake alias must deserialize");
    assert_eq!(anti, ZeroCause::AntiBot);
}

/// The published schema is embedded in the binary via `include_str!`, so it IS
/// the contract the agent reads. If it diverges from what serde emits, the CLI
/// advertises a value it never produces.
#[test]
fn published_schema_lists_the_values_actually_emitted() {
    for name in [
        "docs/schemas/search-output.schema.json",
        "docs/schemas/search-metadata.schema.json",
    ] {
        let raw = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name))
            .unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
        assert!(
            raw.contains("\"ghost-block\"") && raw.contains("\"anti-bot\""),
            "{name} must list the kebab-case values serde emits"
        );
        assert!(
            !raw.contains("\"ghost_block\"") && !raw.contains("\"anti_bot\""),
            "{name} still lists the snake_case values the binary no longer emits"
        );
    }
}
