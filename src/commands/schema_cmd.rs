// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential utility (emit static JSON Schema). No fan-out — justified.
//! Handler for the `schema` subcommand — emit JSON Schema catalog or body.

use crate::cli::SchemaArgs;
use crate::error::exit_codes;
use crate::output;

/// Compile-time catalog of public JSON Schemas shipped under `docs/schemas/`.
const SCHEMAS: &[(&str, &str)] = &[
    (
        "search-output",
        include_str!("../../docs/schemas/search-output.schema.json"),
    ),
    (
        "search-result",
        include_str!("../../docs/schemas/search-result.schema.json"),
    ),
    (
        "search-metadata",
        include_str!("../../docs/schemas/search-metadata.schema.json"),
    ),
    (
        "multi-search-output",
        include_str!("../../docs/schemas/multi-search-output.schema.json"),
    ),
    (
        "news-result",
        include_str!("../../docs/schemas/news-result.schema.json"),
    ),
    (
        "ndjson-event",
        include_str!("../../docs/schemas/ndjson-event.schema.json"),
    ),
    (
        "error-response",
        include_str!("../../docs/schemas/error-response.schema.json"),
    ),
    (
        "deep-research-output",
        include_str!("../../docs/schemas/deep-research-output.schema.json"),
    ),
    (
        "probe-output",
        include_str!("../../docs/schemas/probe-output.schema.json"),
    ),
    (
        "probe-deep-output",
        include_str!("../../docs/schemas/probe-deep-output.schema.json"),
    ),
    (
        "config",
        include_str!("../../docs/schemas/config.schema.json"),
    ),
];

/// Emits schema catalog (list) or a single named schema body on stdout.
pub fn execute_schema(args: &SchemaArgs) -> i32 {
    match args.name.as_deref() {
        None => emit_catalog(),
        Some(name) => emit_named(name),
    }
}

fn emit_catalog() -> i32 {
    let schemas: Vec<_> = SCHEMAS
        .iter()
        .map(|(id, _)| {
            serde_json::json!({
                "id": id,
                "invoke": format!("duckduckgo-search-cli schema --name {id}"),
            })
        })
        .collect();
    let payload = serde_json::json!({
        "type": "schema_catalog",
        "version": env!("CARGO_PKG_VERSION"),
        "count": schemas.len(),
        "schemas": schemas,
    });
    print_json(&payload)
}

fn emit_named(name: &str) -> i32 {
    let key = name
        .trim()
        .trim_end_matches(".schema.json")
        .trim_end_matches(".json");
    match SCHEMAS.iter().find(|(id, _)| *id == key) {
        Some((id, body)) => {
            // Body is already JSON Schema text; validate it parses, then emit raw.
            if serde_json::from_str::<serde_json::Value>(body).is_err() {
                output::emit_stderr(crate::i18n::tf(
                    crate::i18n::Message::SchemaInvalidJson,
                    &[("id", id)],
                ));
                return exit_codes::GENERIC_ERROR;
            }
            match output::print_line_stdout(body.trim_end()) {
                Ok(()) => exit_codes::SUCCESS,
                Err(err) if output::is_broken_pipe(&err) => exit_codes::BROKEN_PIPE,
                Err(err) => {
                    let err_s = format!("{err:#}");
                    output::emit_stderr(crate::i18n::tf(
                        crate::i18n::Message::SchemaEmitFailed,
                        &[("id", id), ("error", &err_s)],
                    ));
                    exit_codes::GENERIC_ERROR
                }
            }
        }
        None => {
            let known: Vec<_> = SCHEMAS.iter().map(|(id, _)| *id).collect();
            let payload = serde_json::json!({
                "type": "error",
                "error": {
                    "category": "validation",
                    "code": "unknown_schema",
                    "message": format!("unknown schema id {name:?}"),
                    "known": known,
                }
            });
            // Unknown schema is a user/data error — structured on stdout for agents
            // (catalog is also stdout). Exit 2 (invalid config / usage).
            let _ = print_json(&payload);
            exit_codes::INVALID_CONFIG
        }
    }
}

fn print_json(value: &serde_json::Value) -> i32 {
    match serde_json::to_string_pretty(value) {
        Ok(json) => match output::print_line_stdout(&json) {
            Ok(()) => exit_codes::SUCCESS,
            Err(err) if output::is_broken_pipe(&err) => exit_codes::BROKEN_PIPE,
            Err(err) => {
                output::emit_stderr(crate::i18n::error_msg(
                    crate::i18n::Message::SchemaJsonEmitFailed,
                    &err,
                ));
                exit_codes::GENERIC_ERROR
            }
        },
        Err(err) => {
            output::emit_stderr(crate::i18n::error_msg(
                crate::i18n::Message::SchemaSerializeFailed,
                &err,
            ));
            exit_codes::GENERIC_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_non_empty_and_valid_json() {
        for (id, body) in SCHEMAS {
            assert!(!id.is_empty());
            serde_json::from_str::<serde_json::Value>(body)
                .unwrap_or_else(|e| panic!("schema {id} invalid JSON: {e}"));
        }
        assert!(SCHEMAS.len() >= 8);
    }

    #[test]
    fn search_output_schema_present() {
        assert!(SCHEMAS.iter().any(|(id, _)| *id == "search-output"));
    }
}
