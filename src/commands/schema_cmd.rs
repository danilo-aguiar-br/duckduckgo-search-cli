// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: sequential utility (emit static JSON Schema). No fan-out — justified.
//! Handler for the `schema` subcommand — emit JSON Schema catalog or body.

use crate::cli::SchemaArgs;
use crate::error::exit_codes;
use crate::output;
use crate::output::envelope_ops::EnvelopeShape;

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
        "classified-error-output",
        include_str!("../../docs/schemas/classified-error-output.schema.json"),
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
    (
        "init-config-output",
        include_str!("../../docs/schemas/init-config-output.schema.json"),
    ),
    (
        "deep-research-budget",
        include_str!("../../docs/schemas/deep-research-budget.schema.json"),
    ),
    (
        "deep-research-error",
        include_str!("../../docs/schemas/deep-research-error.schema.json"),
    ),
    (
        "commands-output",
        include_str!("../../docs/schemas/commands-output.schema.json"),
    ),
    (
        "schema-catalog",
        include_str!("../../docs/schemas/schema-catalog.schema.json"),
    ),
    (
        "locale-output",
        include_str!("../../docs/schemas/locale-output.schema.json"),
    ),
    (
        "doctor-output",
        include_str!("../../docs/schemas/doctor-output.schema.json"),
    ),
    (
        "config-list-output",
        include_str!("../../docs/schemas/config-list-output.schema.json"),
    ),
    (
        "config-path-output",
        include_str!("../../docs/schemas/config-path-output.schema.json"),
    ),
    (
        "config-get-output",
        include_str!("../../docs/schemas/config-get-output.schema.json"),
    ),
    (
        "config-mutation-output",
        include_str!("../../docs/schemas/config-mutation-output.schema.json"),
    ),
    (
        "config-effective-output",
        include_str!("../../docs/schemas/config-effective-output.schema.json"),
    ),
];

/// Every `type` discriminator the product emits, mapped to its published schema.
///
/// # Why a hand-written table and not a scan alone
///
/// The scan in [`tests::every_emitted_discriminator_has_a_published_schema`]
/// only sees `"type": "…"` written as a literal inside a `json!` macro. Three
/// envelopes — `doctor`, `probe`, `probe-deep` — carry their discriminator on a
/// `#[serde(rename = "type")]` struct field instead, so no literal exists to
/// find. Listing them here is what keeps the scan's blind spot from becoming
/// the product's blind spot; the test asserts the scan is a SUBSET of this
/// table, never that the scan is complete.
const DISCRIMINATOR_SCHEMAS: &[(&str, &str)] = &[
    // Emitted as a `json!` literal — the scan finds these.
    ("deep_research_budget", "deep-research-budget"),
    ("deep_research_error", "deep-research-error"),
    ("schema_catalog", "schema-catalog"),
    ("commands", "commands-output"),
    // v1.0.4: was mapped to `error-response`, which is a DIFFERENT envelope —
    // there `error` is a plain string and there is no `type` at all. The
    // classified shape (`error` as an object with category/code/message) had no
    // contract, so routing by `type: "error"` sent an agent to a schema that
    // rejects the document on its first keyword. `error-response` is routed by
    // shape, not by discriminator, so it is deliberately absent from this table.
    ("error", "classified-error-output"),
    // Emitted through serde on a renamed struct field — invisible to the scan.
    ("doctor", "doctor-output"),
    ("probe", "probe-output"),
    // v1.0.4: was `probe-deep` with a hyphen while `ProbeDeepReport` emits the
    // underscored `probe_deep`. Nothing confronted the hand-written value with
    // the value the struct actually writes, so an agent routing by the
    // published table found no schema for the envelope in its hand. The value
    // is now pinned by `discriminator_table_matches_schema_const`, which reads
    // `properties.type.const` out of the published file.
    ("probe_deep", "probe-deep-output"),
    // Deliberate exception: this envelope carries its discriminator on `kind`,
    // not `type` (`#[serde(rename = "kind")]` in `deep_research::types`). It was
    // absent from this table for that reason, which left the largest envelope in
    // the product unroutable by the published catalog. Renaming the wire key
    // would break every consumer already reading `kind`, so the exception is
    // declared here and asserted by `discriminator_key_is_kind_only_for_deep_research`.
    ("deep_research", "deep-research-output"),
    // v1.0.4: seven envelopes carried NO discriminator at all, so no amount of
    // checking this table against the schemas could have found them — there
    // was nothing on either side to compare. An agent holding a `config get`
    // envelope had no key to route on and had to recognise the shape. All
    // seven now emit `type` from a compiler-checked enum, and
    // `every_published_schema_is_routable` fails the build if an eighth
    // appears without one.
    ("config_path", "config-path-output"),
    ("config_list", "config-list-output"),
    ("config_get", "config-get-output"),
    ("config_mutation", "config-mutation-output"),
    ("config_effective", "config-effective-output"),
    ("locale", "locale-output"),
    ("init_config", "init-config-output"),
];

/// Schemas that are legitimately NOT routed by a discriminator.
///
/// The completeness ruler needs a total partition of the published set, and a
/// total partition needs somewhere to put the envelopes that genuinely have no
/// discriminator. Two kinds live here, and each entry must say which:
///
/// - FRAGMENT: never emitted alone, reached only through `$ref` from a parent.
/// - SHAPE-ROUTED: emitted alone, identified by its required keys instead.
///
/// This list is the ONLY escape hatch, and adding to it is a deliberate act.
/// Why a published schema carries no discriminator.
///
/// # Why this is a type and not a sentence
///
/// v1.0.4 wrote these reasons as free prose in a `&str`, all three kinds mixed
/// in one column: `"fragment: `$ref`d by search-output"` sat next to
/// `"shape-routed: identified by `searches`"`. A human could tell them apart;
/// nothing else could. The partition the completeness ruler depends on was
/// therefore asserted in English and checked by nobody, and the facts inside
/// the prose — WHICH parent `$ref`s the fragment, WHICH keys identify the
/// shape — were unverifiable claims.
///
/// As a type they are checkable, and the tests do check them: a declared
/// parent must really `$ref` the child, and declared shape keys must really
/// appear in the schema's `required`. A wrong claim now fails the build
/// instead of misleading a reader.
#[derive(Debug, Clone, Copy)]
enum RoutingKind {
    /// Never emitted alone; reached only through `$ref` from `parent`.
    Fragment {
        /// Schema id that references this one.
        parent: &'static str,
    },
    /// Emitted alone and identified by these required keys instead of a `type`.
    ShapeRouted {
        /// Keys that must all be present for a consumer to recognise it.
        keys: &'static [&'static str],
    },
    /// Not an envelope at all: describes something other than stdout output.
    NotAnEnvelope {
        /// What the document actually describes.
        reason: &'static str,
    },
}

impl RoutingKind {
    /// Machine-readable tag published in the catalog.
    const fn tag(self) -> &'static str {
        match self {
            Self::Fragment { .. } => "fragment",
            Self::ShapeRouted { .. } => "shape_routed",
            Self::NotAnEnvelope { .. } => "not_an_envelope",
        }
    }
}

/// Schemas that are legitimately NOT routed by a discriminator, and why.
///
/// This list is the ONLY escape hatch from [`DISCRIMINATOR_SCHEMAS`], and
/// adding to it is a deliberate act.
const NON_DISCRIMINATED_SCHEMAS: &[(&str, RoutingKind)] = &[
    (
        "search-output",
        RoutingKind::ShapeRouted {
            keys: &["query", "results"],
        },
    ),
    (
        "multi-search-output",
        RoutingKind::ShapeRouted {
            keys: &["searches"],
        },
    ),
    (
        "ndjson-event",
        RoutingKind::ShapeRouted {
            keys: &["query", "results"],
        },
    ),
    (
        "search-metadata",
        RoutingKind::Fragment {
            parent: "search-output",
        },
    ),
    (
        "search-result",
        RoutingKind::Fragment {
            parent: "search-output",
        },
    ),
    (
        "news-result",
        RoutingKind::Fragment {
            parent: "search-output",
        },
    ),
    (
        "config",
        RoutingKind::NotAnEnvelope {
            reason: "describes the TOML files on disk, not stdout",
        },
    ),
    (
        // The thin error has no `type`, and routing `type: "error"` here is
        // exactly the v1.0.4 defect that sent an agent to the wrong contract:
        // `classified-error-output` owns that value.
        "error-response",
        RoutingKind::ShapeRouted {
            keys: &["error", "message"],
        },
    ),
];

/// Schemas whose discriminator lives on `kind` instead of `type`.
///
/// One entry today. Kept as a list so the asymmetry is enumerable by a test
/// rather than remembered, and so adding a second one is a deliberate act.
const KIND_DISCRIMINATOR_SCHEMAS: &[&str] = &["deep-research-output"];

/// Emits schema catalog (list) or a single named schema body on stdout.
pub fn execute_schema(args: &SchemaArgs) -> i32 {
    match args.name.as_deref() {
        None => emit_catalog(),
        Some(name) => emit_named(name),
    }
}

/// The `type` value an envelope described by `schema_id` carries, if any.
///
/// Published in the catalog so an agent can route mechanically: read `type` off
/// an envelope, find the entry that declares it, fetch that schema. Without it
/// the mapping lives only in the agent's head, which is how a consumer ends up
/// validating a `cancelled` envelope against the budget contract.
fn discriminator_for(schema_id: &str) -> Option<&'static str> {
    DISCRIMINATOR_SCHEMAS
        .iter()
        .find(|(_, id)| *id == schema_id)
        .map(|(discriminator, _)| *discriminator)
}

/// The KEY an agent must read to find the discriminator of `schema_id`.
///
/// Publishing the value without the key was half a routing table. Almost every
/// envelope carries it on `type`, but `deep-research-output` carries it on
/// `kind`, so an agent reading `type` on the largest envelope in the product
/// found nothing and had no way to learn why. Emitting the key alongside the
/// value makes the exception machine-readable instead of folklore, and keeps
/// [`KIND_DISCRIMINATOR_SCHEMAS`] load-bearing rather than a test-only constant
/// that `dead_code` would ask us to silence.
/// Why `schema_id` carries no discriminator, when that is deliberate.
///
/// Keeps [`NON_DISCRIMINATED_SCHEMAS`] load-bearing instead of a test-only
/// constant, and gives an agent the one thing absence cannot express: whether
/// the envelope has no routing key by design or by oversight.
fn routing_note_for(schema_id: &str) -> Option<RoutingKind> {
    NON_DISCRIMINATED_SCHEMAS
        .iter()
        .find(|(id, _)| *id == schema_id)
        .map(|(_, kind)| *kind)
}

/// Render a [`RoutingKind`] as the catalog publishes it.
///
/// Structured rather than a sentence, so a consumer can branch on `kind`
/// instead of pattern-matching English.
fn routing_value(kind: RoutingKind) -> serde_json::Value {
    let mut out = serde_json::json!({ "kind": kind.tag() });
    match kind {
        RoutingKind::Fragment { parent } => {
            out["parent"] = serde_json::json!(parent);
        }
        RoutingKind::ShapeRouted { keys } => {
            out["identified_by"] = serde_json::json!(keys);
        }
        RoutingKind::NotAnEnvelope { reason } => {
            out["reason"] = serde_json::json!(reason);
        }
    }
    out
}

fn discriminator_key_for(schema_id: &str) -> &'static str {
    if KIND_DISCRIMINATOR_SCHEMAS.contains(&schema_id) {
        "kind"
    } else {
        "type"
    }
}

fn emit_catalog() -> i32 {
    let schemas: Vec<_> = SCHEMAS
        .iter()
        .map(|(id, _)| {
            let mut entry = serde_json::json!({
                "id": id,
                "invoke": format!("duckduckgo-search-cli schema --name {id}"),
            });
            if let Some(discriminator) = discriminator_for(id) {
                entry["discriminator"] = serde_json::json!(discriminator);
                entry["discriminator_key"] = serde_json::json!(discriminator_key_for(id));
            } else if let Some(kind) = routing_note_for(id) {
                // Published rather than kept as a test-only constant, for the
                // same reason `discriminator_key` is published: a consumer that
                // finds no discriminator needs to learn whether the envelope
                // has none BY DESIGN or whether the catalog simply forgot one.
                // Silence cannot tell those apart; this field can.
                entry["routing"] = routing_value(kind);
            }
            entry
        })
        .collect();
    let payload = serde_json::json!({
        "type": crate::types::SchemaCatalogKind::SchemaCatalog,
        "version": env!("CARGO_PKG_VERSION"),
        "count": schemas.len(),
        "schemas": schemas,
    });
    // Rows are the catalog entries, so the whole reduction set applies here.
    // The shape is LOOKED UP rather than rebuilt: a local copy would carry an
    // empty identity list, and `--truncate-content 12` would go straight back
    // to turning `invoke` into `duckduckgo-s`.
    let shape = crate::output::envelope_ops::shape_for("schema")
        .copied()
        .unwrap_or_else(|| {
            crate::output::envelope_ops::EnvelopeShape::with_rows("schema", "schemas", "type")
        });
    emit_reduced(payload, &shape)
}

/// Emit an introspection envelope through the reduction boundary.
///
/// Introspection stays English by design, so it cannot use `emit_wire_line`;
/// before v1.0.4 it therefore used `print_line_stdout`, which skipped every
/// agent-native reduction the caller asked for.
fn emit_reduced(payload: serde_json::Value, shape: &EnvelopeShape) -> i32 {
    output::emit_envelope_or_refuse(payload, shape, true, output::KeyPolicy::EnglishOnly)
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

    /// The compiled catalog and `docs/schemas/` must be the SAME set.
    ///
    /// # Why this test exists
    ///
    /// [`SCHEMAS`] is hand-maintained: adding a file under `docs/schemas/` does
    /// **not** add it to `duckduckgo-search-cli schema`, and deleting one leaves
    /// an `include_str!` that fails the build only if you happen to rebuild.
    /// Until v1.0.3 nothing compared the two lists, so the catalog an agent
    /// queries could drift away from the contract the repository publishes —
    /// the same shape of silence that let a published schema describe a document
    /// the product never wrote (ADR-0030).
    ///
    /// Drift in either direction is a failure here, with the offending ids named.
    #[test]
    fn catalog_matches_published_schema_files() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/schemas");
        let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
            .expect("docs/schemas must exist")
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                let name = path.file_name()?.to_str()?;
                name.strip_suffix(".schema.json").map(str::to_string)
            })
            .collect();
        on_disk.sort_unstable();

        let mut compiled: Vec<String> = SCHEMAS.iter().map(|(id, _)| (*id).to_string()).collect();
        compiled.sort_unstable();

        let missing_from_catalog: Vec<_> =
            on_disk.iter().filter(|id| !compiled.contains(id)).collect();
        let missing_from_disk: Vec<_> =
            compiled.iter().filter(|id| !on_disk.contains(id)).collect();

        assert!(
            missing_from_catalog.is_empty() && missing_from_disk.is_empty(),
            "schema catalog drifted from docs/schemas/\n\
             published but not in SCHEMAS: {missing_from_catalog:?}\n\
             in SCHEMAS but not published: {missing_from_disk:?}"
        );
    }

    /// Harvest every `"type": "<literal>"` discriminator written in `text`.
    ///
    /// Comment lines are skipped: prose that quotes a discriminator — including
    /// the doc comment you are reading — is not an emission, and treating it as
    /// one made the first run of this test demand a schema for `"…"`.
    fn harvest_discriminators(text: &str, out: &mut std::collections::BTreeSet<String>) {
        const NEEDLE: &str = "\"type\"";
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("*") {
                continue;
            }
            let mut idx = 0usize;
            while let Some(found) = line[idx..].find(NEEDLE) {
                idx = idx + found + NEEDLE.len();
                // Only a `"type": "literal"` pair counts. `("type", "tipo")` in
                // the wire map and `v["type"]` in assertions carry no colon, so
                // they fall out here rather than being filtered later by name.
                let Some(rest) = line[idx..].trim_start().strip_prefix(':') else {
                    continue;
                };
                let Some(rest) = rest.trim_start().strip_prefix('"') else {
                    continue;
                };
                if let Some(end) = rest.find('"') {
                    out.insert(rest[..end].to_string());
                }
            }
        }
    }

    /// Walk `dir` for `.rs` files, harvesting discriminators from each.
    fn walk_rust_sources(dir: &std::path::Path, out: &mut std::collections::BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_rust_sources(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    harvest_discriminators(&text, out);
                }
            }
        }
    }

    /// Every discriminator the product emits must have a published schema.
    ///
    /// # Why this test exists, when a drift test already did
    ///
    /// [`catalog_matches_published_schema_files`] compares two sets of FILES:
    /// what sits under `docs/schemas/` against what [`SCHEMAS`] compiles in.
    /// That is real coverage, and it is structurally blind to the opposite
    /// failure — an envelope the product WRITES with no schema anywhere. There
    /// is no file for a file-to-file comparison to enumerate, so the absence is
    /// invisible by construction.
    ///
    /// v1.0.3 paid for that blindness twice. `deep_research_error` shipped with
    /// four disjoint shapes behind one discriminator and a schema covering one
    /// of them, and five introspection surfaces shipped with none at all. Both
    /// were found by reading, not by any gate, because nothing compared the set
    /// of things EMITTED against the set of things DESCRIBED.
    ///
    /// This test walks the crate source and asserts every `"type"` literal it
    /// finds is declared in [`DISCRIMINATOR_SCHEMAS`], and that each entry
    /// there names a schema the catalog actually ships. Adding a new envelope
    /// without a contract now fails the build.
    #[test]
    fn every_emitted_discriminator_has_a_published_schema() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut emitted = std::collections::BTreeSet::new();
        walk_rust_sources(&src, &mut emitted);
        assert!(
            !emitted.is_empty(),
            "the source scan found no discriminators at all — it is broken, \
             and a broken scan silently passes every assertion below"
        );

        let undeclared: Vec<&String> = emitted
            .iter()
            .filter(|d| {
                !DISCRIMINATOR_SCHEMAS
                    .iter()
                    .any(|(name, _)| *name == d.as_str())
            })
            .collect();
        assert!(
            undeclared.is_empty(),
            "envelope emitted with no published schema: {undeclared:?}\n\
             Publish one under docs/schemas/, add it to SCHEMAS, and map it in \
             DISCRIMINATOR_SCHEMAS."
        );

        let dangling: Vec<&str> = DISCRIMINATOR_SCHEMAS
            .iter()
            .filter(|(_, id)| !SCHEMAS.iter().any(|(known, _)| known == id))
            .map(|(name, _)| *name)
            .collect();
        assert!(
            dangling.is_empty(),
            "discriminators mapped to a schema the catalog does not ship: {dangling:?}"
        );
    }

    /// The discriminator a schema DECLARES and the one the table PUBLISHES
    /// must be the same string, in both directions.
    ///
    /// # The hole this closes
    ///
    /// [`every_emitted_discriminator_has_a_published_schema`] only sees
    /// `"type": "literal"` written inside a `json!` macro. Envelopes that carry
    /// the discriminator on a `#[serde(rename = "type")]` struct field have no
    /// literal to find, so they were listed in [`DISCRIMINATOR_SCHEMAS`] by
    /// hand — and a hand-written list is a claim, not a check. It drifted:
    /// `ProbeDeepReport` emitted `probe_deep` while this table advertised
    /// `probe-deep`, and no gate could see the difference.
    ///
    /// Making the published schema the single source of truth removes the
    /// hand-written value from the loop entirely. The chain is now closed at
    /// both ends: the schema declares `const`, this test pins the table to it,
    /// and the conformance suite validates the real envelope against the same
    /// `const` — so a struct that emits the wrong string fails there.
    ///
    /// A source scan for `kind: "literal"` was considered and rejected: the
    /// field name `kind` is also used for values that are NOT wire
    /// discriminators (`LocaleReport.kind` serializes as `strategy`), so the
    /// scan would report false positives and teach the next reader to silence
    /// it.
    /// Every published schema is routable, or is declared as deliberately not.
    ///
    /// # Why a partition and not another list
    ///
    /// The existing rulers compare the routing table against the schemas that
    /// DO declare a discriminator. Both are blind in the same direction: a
    /// schema with no discriminator anywhere is absent from both sides of every
    /// comparison, so nothing notices. That is how five `config` envelopes,
    /// `locale` and `init-config` stayed unroutable through two audits — seven
    /// of twenty-four, and the ones an agent parses before deciding whether a
    /// run is worth attempting.
    ///
    /// A total, disjoint partition has no such blind spot. Every published file
    /// belongs to exactly one of two sets, and a new file that belongs to
    /// neither fails the build with the question spelled out.
    #[test]
    fn every_published_schema_is_routable() {
        let routed: std::collections::HashSet<&str> =
            DISCRIMINATOR_SCHEMAS.iter().map(|(_, id)| *id).collect();
        let excused: std::collections::HashSet<&str> = NON_DISCRIMINATED_SCHEMAS
            .iter()
            .map(|(id, _)| *id)
            .collect();

        let overlap: Vec<&str> = routed.intersection(&excused).copied().collect();
        assert!(
            overlap.is_empty(),
            "a schema cannot be both routed and excused from routing: {overlap:?}"
        );

        let mut unclassified = Vec::new();
        for (id, _) in SCHEMAS {
            if !routed.contains(id) && !excused.contains(id) {
                unclassified.push(*id);
            }
        }
        assert!(
            unclassified.is_empty(),
            "published schemas that are neither routable nor declared unroutable: \
             {unclassified:?}\n\
             Either give the envelope a `type` const and add it to \
             DISCRIMINATOR_SCHEMAS, or add it to NON_DISCRIMINATED_SCHEMAS with \
             the reason it has no discriminator."
        );

        let known: std::collections::HashSet<&str> = SCHEMAS.iter().map(|(id, _)| *id).collect();
        for (id, _) in NON_DISCRIMINATED_SCHEMAS {
            assert!(known.contains(id), "excused schema `{id}` is not published");
        }
    }

    /// Keys a document MUST carry, following `allOf` and `$ref` composition.
    ///
    /// JSON Schema lets a document inherit its obligations, so reading
    /// `required` off the top level answers a narrower question than "what
    /// must be present". `depth` bounds the walk so a cyclic `$ref` between
    /// two published schemas fails as a missing key rather than a hung test.
    fn required_transitive(
        id: &str,
        load: &dyn Fn(&str) -> serde_json::Value,
        depth: usize,
    ) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        if depth > 4 {
            return out;
        }
        let doc = load(id);
        if let Some(list) = doc.get("required").and_then(|r| r.as_array()) {
            out.extend(list.iter().filter_map(|v| v.as_str()).map(str::to_string));
        }
        if let Some(all_of) = doc.get("allOf").and_then(|a| a.as_array()) {
            for member in all_of {
                if let Some(list) = member.get("required").and_then(|r| r.as_array()) {
                    out.extend(list.iter().filter_map(|v| v.as_str()).map(str::to_string));
                }
                if let Some(reference) = member.get("$ref").and_then(|r| r.as_str()) {
                    if let Some(parent) = reference.strip_suffix(".schema.json") {
                        out.extend(required_transitive(parent, load, depth + 1));
                    }
                }
            }
        }
        out
    }

    /// Every routing claim must be true of the published document.
    ///
    /// # What this catches that the partition test cannot
    ///
    /// `every_published_schema_is_routable` proves the partition is total and
    /// disjoint — that each schema is classified exactly once. It says nothing
    /// about whether the classification is CORRECT, because until v1.0.5 the
    /// reason was English prose and there was nothing in it a test could
    /// check. A fragment could name a parent that never referenced it, and a
    /// shape-routed entry could name keys the schema does not require, and
    /// both would pass every gate while telling an agent something false.
    ///
    /// Now `Fragment` must name a parent that really `$ref`s it, and
    /// `ShapeRouted` must name keys that are really in `required`.
    #[test]
    fn routing_claims_match_the_published_schemas() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/schemas");
        let load = |id: &str| -> serde_json::Value {
            let path = dir.join(format!("{id}.schema.json"));
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            serde_json::from_str(&raw)
                .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
        };

        for (id, kind) in NON_DISCRIMINATED_SCHEMAS {
            match kind {
                RoutingKind::Fragment { parent } => {
                    let doc = load(parent);
                    let needle = format!("{id}.schema.json");
                    assert!(
                        doc.to_string().contains(&needle),
                        "`{id}` is declared a fragment of `{parent}`, but `{parent}` \
                         never `$ref`s `{needle}`. Either fix the parent or \
                         reclassify `{id}`."
                    );
                }
                RoutingKind::ShapeRouted { keys } => {
                    // Resolved TRANSITIVELY through `allOf` and `$ref`, not read
                    // off the top level. `ndjson-event` is `allOf: [$ref
                    // search-output]` and declares no `required` of its own, yet
                    // every line it describes carries `query` and `results`
                    // because the parent requires them. A shallow read called
                    // that claim false; what was shallow was the ruler.
                    let required = required_transitive(id, &load, 0);
                    for key in *keys {
                        assert!(
                            required.contains(*key),
                            "`{id}` claims to be identified by `{key}`, but `{key}` \
                             is not required by it or by anything it composes \
                             ({required:?}). A key a document may omit cannot \
                             identify it."
                        );
                    }
                }
                RoutingKind::NotAnEnvelope { reason } => {
                    assert!(
                        !reason.trim().is_empty(),
                        "`{id}` must say what it describes instead of an envelope"
                    );
                }
            }
        }
    }

    #[test]
    fn discriminator_table_matches_schema_const() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/schemas");
        let mut declared: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();

        for (id, _) in SCHEMAS {
            let path = dir.join(format!("{id}.schema.json"));
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            let doc: serde_json::Value = serde_json::from_str(&raw)
                .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()));
            // `kind` is the documented exception; everything else uses `type`.
            let key = if KIND_DISCRIMINATOR_SCHEMAS.contains(id) {
                "kind"
            } else {
                "type"
            };
            if let Some(value) = doc
                .pointer(&format!("/properties/{key}/const"))
                .and_then(serde_json::Value::as_str)
            {
                declared.insert((*id).to_string(), value.to_string());
                continue;
            }
            // A `oneOf` schema declares the const once per branch under `$defs`.
            // All branches must agree, otherwise the file claims more than one
            // discriminator — the GAP-SCHEMA-OVERLOADED-DISCRIMINATOR-001 shape.
            let branch_consts: std::collections::BTreeSet<String> = doc
                .get("$defs")
                .and_then(serde_json::Value::as_object)
                .map(|defs| {
                    defs.values()
                        .filter_map(|branch| {
                            branch
                                .pointer(&format!("/properties/{key}/const"))
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_string)
                        })
                        .collect()
                })
                .unwrap_or_default();
            assert!(
                branch_consts.len() <= 1,
                "{id} declares more than one discriminator across its branches: {branch_consts:?}"
            );
            if let Some(value) = branch_consts.into_iter().next() {
                declared.insert((*id).to_string(), value);
            }
        }

        assert!(
            !declared.is_empty(),
            "no schema declares a discriminator const — this test is broken, \
             and a broken test passes every assertion below"
        );

        let mut mismatched = Vec::new();
        for (schema_id, schema_const) in &declared {
            match DISCRIMINATOR_SCHEMAS.iter().find(|(_, id)| id == schema_id) {
                None => mismatched.push(format!(
                    "{schema_id} declares const `{schema_const}` but is absent from \
                     DISCRIMINATOR_SCHEMAS"
                )),
                Some((table_value, _)) if table_value != schema_const => mismatched.push(format!(
                    "{schema_id}: schema says `{schema_const}`, table says `{table_value}`"
                )),
                Some(_) => {}
            }
        }
        for (table_value, schema_id) in DISCRIMINATOR_SCHEMAS {
            if !declared.contains_key(*schema_id) {
                mismatched.push(format!(
                    "{schema_id} is mapped to `{table_value}` in the table but declares no \
                     discriminator const — add one so the value is checkable"
                ));
            }
        }

        assert!(
            mismatched.is_empty(),
            "discriminator drift between the published schemas and the routing table:\n  {}",
            mismatched.join("\n  ")
        );
    }

    /// The `kind` exception is exactly one schema, and it is the deep-research one.
    ///
    /// Pins the asymmetry so a second exception cannot appear quietly, and so
    /// anyone unifying the wire on `type` has to delete this test on purpose.
    #[test]
    fn discriminator_key_is_kind_only_for_deep_research() {
        assert_eq!(
            KIND_DISCRIMINATOR_SCHEMAS,
            ["deep-research-output"],
            "a second `kind` discriminator appeared; unify on `type` or justify it here"
        );
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/schemas/deep-research-output.schema.json");
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("readable schema"))
                .expect("valid JSON");
        assert!(
            doc.pointer("/properties/type").is_none(),
            "deep-research-output must not grow a `type` property while `kind` is the wire key"
        );
    }
}
