// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light pure transform — EN↔PT wire key remap at emit boundary.
//! Wire JSON key language for agent stdout (`--wire-keys` / XDG `wire_keys`).
//!
//! Domain types always **serialize English** keys (ADR-0027). When
//! [`WireKeys::Pt`] is active, this module remaps object keys (and the deep
//! sub-query `status` value) to Portuguese spellings once at the emit boundary.
//!
//! # Agent-native contract
//!
//! Default is English. Opt-in PT keeps legacy consumers without dual DTO
//! structs (DRY, one remap walk).

use crate::error::CliError;
use clap::ValueEnum;
use serde::Serialize;
use serde_json::{Map, Value};
use std::sync::atomic::{AtomicU8, Ordering};

/// Process-wide wire key language (`0` = EN, `1` = PT).
static WIRE_KEYS: AtomicU8 = AtomicU8::new(0);

/// Wire JSON key language for stdout serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum WireKeys {
    /// English keys (`results`, `title`, `error`, …) — v1.0.2 default.
    #[default]
    En,
    /// Portuguese keys (`resultados`, `titulo`, `erro`, …) — legacy opt-in.
    Pt,
}

impl WireKeys {
    /// Parse from XDG / CLI string (`en`|`pt`, case-insensitive).
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "en" | "english" => Some(Self::En),
            "pt" | "pt-br" | "portuguese" | "br" => Some(Self::Pt),
            _ => None,
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            Self::En => 0,
            Self::Pt => 1,
        }
    }

    fn from_u8(v: u8) -> Self {
        if v == 1 {
            Self::Pt
        } else {
            Self::En
        }
    }
}

/// Install wire key language for this process (call after clap + XDG resolve).
pub fn set_process_wire_keys(keys: WireKeys) {
    WIRE_KEYS.store(keys.as_u8(), Ordering::Relaxed);
}

/// Current process wire key language.
#[must_use]
pub fn process_wire_keys() -> WireKeys {
    WireKeys::from_u8(WIRE_KEYS.load(Ordering::Relaxed))
}

/// EN → PT key pairs for object property remapping (SSOT).
///
/// Keys already in PT or without a pair are left unchanged. Longest /
/// most-specific names first is unnecessary because we match exact keys only.
const EN_TO_PT: &[(&str, &str)] = &[
    ("results", "resultados"),
    ("result_count", "quantidade_resultados"),
    ("metadata", "metadados"),
    ("title", "titulo"),
    ("position", "posicao"),
    ("display_url", "url_exibicao"),
    ("original_title", "titulo_original"),
    ("content", "conteudo"),
    ("content_size", "tamanho_conteudo"),
    ("content_extraction_method", "metodo_extracao_conteudo"),
    ("source", "fonte"),
    ("relative_date", "data_relativa"),
    ("execution_time_ms", "tempo_execucao_ms"),
    ("selectors_hash", "hash_seletores"),
    ("retries", "retentativas_executadas"),
    ("retries_configured", "retentativas_configuradas"),
    ("used_fallback_endpoint", "usou_endpoint_fallback"),
    ("concurrent_fetches", "fetches_simultaneos"),
    ("fetch_successes", "sucessos_fetch"),
    ("fetch_failures", "falhas_fetch"),
    ("used_chrome", "usou_chrome"),
    ("chrome_attempted", "tentou_chrome"),
    ("identity_used", "identidade_usada"),
    ("cascade_level", "nivel_cascata"),
    ("cascade_reason", "cascata_motivo"),
    ("used_proxy", "usou_proxy"),
    ("pre_flight_fired", "pre_flight_disparado"),
    ("pre_flight_executed", "pre_flight_executado"),
    ("news_promo_filtered", "news_filtradas_promo"),
    ("stream_requested", "stream_solicitado"),
    ("stream_effective", "stream_efetivo"),
    ("zero_cause", "causa_zero"),
    ("zero_cause_histogram", "causa_zero_histogram"),
    ("next_action_suggestion", "sugestao_proxima_acao"),
    ("bytes_raw", "bytes_brutos"),
    ("bytes_decompressed", "bytes_descomprimidos"),
    ("cascade_level_observed", "cascata_nivel_observado"),
    ("endpoint_used", "endpoint_usado"),
    ("vertical_used", "vertical_usada"),
    ("chrome_path_resolved", "chrome_path_resolvido"),
    ("chrome_channel", "chrome_canal"),
    ("error", "erro"),
    ("message", "mensagem"),
    ("type", "tipo"),
    ("kind", "tipo"),
    ("news", "noticias"),
    ("news_count", "quantidade_noticias"),
    ("synthesis", "sintese"),
    ("original_query", "query_original"),
    ("aggregation_strategy", "estrategia_agregacao"),
    ("unique_result_count", "total_resultados_unicos"),
    ("unique_news_count", "total_noticias_unicas"),
    ("total_time_ms", "tempo_total_ms"),
    ("elapsed_ms", "tempo_ms"),
    ("strategy", "estrategia"),
    ("text", "texto"),
    ("error_message", "mensagem_erro"),
    ("news_unavailable", "news_indisponivel"),
    ("news_error", "news_erro"),
    ("news_diagnosis", "news_diagnostico"),
    ("sub_queries_error", "sub_queries_erro"),
    ("partial", "parcial"),
    ("partial_results", "resultados_parciais"),
    ("partial_truncated", "parcial_truncado"),
    ("searches", "buscas"),
    ("query_count", "quantidade_queries"),
    ("score", "score"),
    ("sources", "fontes"),
    ("snippet", "snippet"),
    ("url", "url"),
    ("query", "query"),
    ("engine", "engine"),
    ("endpoint", "endpoint"),
    ("timestamp", "timestamp"),
    ("region", "region"),
    ("status", "status"),
    ("command", "comando"),
];

fn map_key(en: &str) -> &str {
    for (from, to) in EN_TO_PT {
        if *from == en {
            return to;
        }
    }
    en
}

/// Whether `key` is a Portuguese spelling this module would ever produce.
///
/// # Why this is public
///
/// ADR-0027 says domain types serialize ENGLISH and Portuguese is applied once
/// here, at the emit boundary. Nothing enforced the first half of that
/// sentence, and three optional fields of the deep-research aggregated rows
/// kept a Portuguese `serde(rename)` right through the migration. They were
/// invisible because `skip_serializing_if` drops a `None` before any test can
/// see the key, so the English default wire quietly emitted `fonte` and
/// `data_relativa` while the published schema declared `source` and
/// `relative_date` under `additionalProperties: false`.
///
/// Exposing the right-hand column lets a test assert the invariant on a real
/// serialized value instead of trusting the ADR.
///
/// Test-only on purpose: this is a guard on OUR source, not a service a
/// consumer of the crate would ever call. Shipping it would widen the public
/// API for no caller.
#[cfg(test)]
#[must_use]
pub fn is_portuguese_wire_key(key: &str) -> bool {
    EN_TO_PT.iter().any(|(en, pt)| *pt == key && *en != key)
}

/// Remap a JSON [`Value`] tree from EN wire keys to PT (in place).
pub fn remap_value_en_to_pt(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let old = std::mem::take(map);
            let mut out = Map::with_capacity(old.len());
            for (k, mut v) in old {
                remap_value_en_to_pt(&mut v);
                // Deep sub-query status value: error → erro under PT wire.
                if k == "status" {
                    if let Value::String(s) = &mut v {
                        if s == "error" {
                            *s = "erro".into();
                        }
                    }
                }
                out.insert(map_key(&k).to_string(), v);
            }
            *map = out;
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                remap_value_en_to_pt(item);
            }
        }
        _ => {}
    }
}

/// Apply process wire-keys policy to a JSON value (no-op for EN).
pub fn apply_process_wire_keys(mut value: Value) -> Value {
    if process_wire_keys() == WireKeys::Pt {
        remap_value_en_to_pt(&mut value);
    }
    value
}

/// Serialize `value` to compact or pretty JSON honoring process wire-keys.
///
/// # Errors
///
/// Returns [`CliError::InvalidConfig`] when serde fails.
pub fn serialize_for_wire<T: Serialize>(value: &T) -> Result<String, CliError> {
    let mut v = serde_json::to_value(value).map_err(|e| CliError::InvalidConfig {
        message: format!("failed to serialize JSON value: {e}"),
    })?;
    if process_wire_keys() == WireKeys::Pt {
        remap_value_en_to_pt(&mut v);
    }
    if super::json_pretty_enabled() {
        serde_json::to_string_pretty(&v).map_err(|e| CliError::InvalidConfig {
            message: format!("failed to serialize search output as JSON: {e}"),
        })
    } else {
        serde_json::to_string(&v).map_err(|e| CliError::InvalidConfig {
            message: format!("failed to serialize search output as JSON: {e}"),
        })
    }
}

/// Compact JSON string for wire (no pretty), with process wire-keys.
///
/// # Errors
///
/// Serde failures mapped to [`CliError::InvalidConfig`].
pub fn to_wire_string<T: Serialize>(value: &T) -> Result<String, CliError> {
    let mut v = serde_json::to_value(value).map_err(|e| CliError::InvalidConfig {
        message: format!("failed to serialize JSON value: {e}"),
    })?;
    if process_wire_keys() == WireKeys::Pt {
        remap_value_en_to_pt(&mut v);
    }
    serde_json::to_string(&v).map_err(|e| CliError::InvalidConfig {
        message: format!("failed to serialize JSON: {e}"),
    })
}

/// Finalize an already-built [`Value`] (projected / envelope) for stdout.
pub fn finalize_wire_value(value: Value) -> Value {
    apply_process_wire_keys(value)
}

/// Serialize a [`Value`] after wire-keys policy.
///
/// Honors process-wide `--pretty` ([`super::json_pretty_enabled`]) so projected
/// `--fields` paths match full-envelope JSON indent (GAP-PRETTY-FIELDS).
/// NDJSON and other callers that need forced-compact JSON must use
/// [`to_wire_string`] instead.
///
/// # Errors
///
/// Serde stringification failures.
pub fn value_to_wire_string(value: Value) -> Result<String, CliError> {
    let v = finalize_wire_value(value);
    if super::json_pretty_enabled() {
        serde_json::to_string_pretty(&v).map_err(|e| CliError::InvalidConfig {
            message: format!("failed to serialize JSON: {e}"),
        })
    } else {
        serde_json::to_string(&v).map_err(|e| CliError::InvalidConfig {
            message: format!("failed to serialize JSON: {e}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    /// Process-wide wire/pretty flags must not race across parallel tests.
    static WIRE_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn remap_results_title_and_status() {
        // Pure remap — no process flag.
        let mut v = json!({
            "results": [{"title": "T", "url": "https://e.x"}],
            "metadata": {"sub_queries": [{"status": "error", "text": "q"}]}
        });
        remap_value_en_to_pt(&mut v);
        assert!(v.get("resultados").is_some());
        assert!(v.get("results").is_none());
        assert_eq!(v["resultados"][0]["titulo"], "T");
        assert_eq!(v["metadados"]["sub_queries"][0]["status"], "erro");
        assert_eq!(v["metadados"]["sub_queries"][0]["texto"], "q");
    }

    #[test]
    fn value_to_wire_string_honors_pretty_flag() {
        let _guard = WIRE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_keys = process_wire_keys();
        let prev_pretty = crate::output::json_pretty_enabled();
        set_process_wire_keys(WireKeys::En);
        crate::output::set_json_pretty(false);
        let compact = value_to_wire_string(json!({"results": [{"title": "T"}]})).expect("ser");
        assert!(
            !compact.contains(char::from_u32(0x0a).unwrap()),
            "compact path must not inject LF"
        );
        crate::output::set_json_pretty(true);
        let pretty = value_to_wire_string(json!({"results": [{"title": "T"}]})).expect("ser");
        assert!(
            pretty.contains(char::from_u32(0x0a).unwrap()),
            "pretty path must inject LF for --fields projection"
        );
        assert!(pretty.contains("title"));
        crate::output::set_json_pretty(prev_pretty);
        set_process_wire_keys(prev_keys);
    }

    #[test]
    fn en_mode_leaves_keys() {
        let _guard = WIRE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = process_wire_keys();
        set_process_wire_keys(WireKeys::En);
        let s = to_wire_string(&json!({"results": []})).expect("ser");
        assert!(s.contains("results"));
        assert!(!s.contains("resultados"));
        set_process_wire_keys(prev);
    }

    #[test]
    fn pt_mode_rewrites_keys() {
        let _guard = WIRE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = process_wire_keys();
        set_process_wire_keys(WireKeys::Pt);
        let s = to_wire_string(&json!({"results": [{"title": "x"}]})).expect("ser");
        assert!(s.contains("resultados"));
        assert!(s.contains("titulo"));
        set_process_wire_keys(prev);
    }

    #[test]
    fn parse_wire_keys() {
        assert_eq!(WireKeys::parse("en"), Some(WireKeys::En));
        assert_eq!(WireKeys::parse("PT"), Some(WireKeys::Pt));
        assert_eq!(WireKeys::parse("nope"), None);
    }

    #[test]
    fn portuguese_key_detector_ignores_identical_pairs() {
        assert!(is_portuguese_wire_key("fonte"));
        assert!(is_portuguese_wire_key("data_relativa"));
        assert!(is_portuguese_wire_key("url_exibicao"));
        assert!(!is_portuguese_wire_key("source"));
        // `status` and friends map to themselves; they are not PT spellings.
        for (en, pt) in EN_TO_PT {
            if en == pt {
                assert!(!is_portuguese_wire_key(pt), "{pt} maps to itself");
            }
        }
    }

    /// Class ruler for ADR-0027: no domain type may `rename` to Portuguese.
    ///
    /// The value test in `agent_ops` proves the invariant for the two
    /// aggregated rows. It cannot prove it for a struct nobody thought to
    /// serialize in a test, and that is precisely how three fields survived:
    /// they were `Option` with `skip_serializing_if`, so no fixture ever made
    /// the key appear. This walks the crate source instead of its values, so a
    /// field added tomorrow is caught with no fixture at all.
    ///
    /// Deliberately scans `serde` attributes only. A Portuguese `alias` is
    /// correct and required — it is how PT consumers keep deserializing.
    #[test]
    fn no_domain_type_renames_a_field_to_portuguese() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("readable src dir") {
                let path = entry.expect("readable entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                // This module owns the PT column; its own table is not a rename.
                if path.file_name().and_then(|n| n.to_str()) == Some("wire_keys.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("readable rust file");
                for (lineno, line) in text.lines().enumerate() {
                    let Some(rest) = line.split_once("rename = \"") else {
                        continue;
                    };
                    let Some((name, _)) = rest.1.split_once('"') else {
                        continue;
                    };
                    if is_portuguese_wire_key(name) {
                        offenders.push(format!("{}:{}: {name}", path.display(), lineno + 1));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "domain types must serialize ENGLISH keys (ADR-0027); Portuguese belongs \
             in an `alias` and in this module's EN_TO_PT table, never in a `rename`.\n\
             Offenders:\n{}",
            offenders.join("\n")
        );
    }
}
