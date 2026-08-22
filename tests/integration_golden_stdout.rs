// SPDX-License-Identifier: MIT OR Apache-2.0
//! Golden snapshots of the SHAPE of every offline stdout envelope.
//!
//! # Why shape and not bytes
//!
//! A byte-for-byte golden of `doctor` would change on every host: it carries
//! absolute paths, a live count of chrome-like processes, a git SHA and a
//! crate version. The plan that asked for this layer said it plainly —
//! "without normalisation, noise becomes an alert and the snapshot is approved
//! without being read". A snapshot nobody reads is worse than none, because it
//! looks like coverage.
//!
//! So these snapshots record the STRUCTURE: every JSON path in the envelope
//! with the type of its leaf, sorted. Values appear only where they are part of
//! the contract — the discriminator, which is a `const` in the published
//! schema. That is stable on any host and still catches the whole class this
//! repository keeps finding: a key added, removed, renamed, or changed type.
//!
//! # What this catches that conformance does not
//!
//! `assert_conforms` proves the envelope satisfies the schema. It cannot see a
//! key the schema never declared and the envelope stopped emitting, nor an
//! optional key that quietly disappeared — `deep-research`'s three Portuguese
//! field names survived two audits exactly there. The shape snapshot has no
//! optional fields: whatever the binary emitted last time is in the file.
//!
//! # Updating
//!
//! `cargo insta review`, or delete the `.snap` and re-run. Read the diff.

use serde_json::Value;
use std::collections::BTreeSet;

/// Run the compiled binary and parse stdout as JSON.
fn run_json(args: &[&str]) -> Value {
    let out = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
        .expect("compiled binary exists")
        .args(args)
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "stdout of {args:?} is not JSON: {e}\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

/// The type name this snapshot records for a JSON leaf.
fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Arrays whose POPULATION is a property of the host, with the reason.
///
/// # Why an exemption exists at all
///
/// This module's own header promises a snapshot that is "stable on any host",
/// and it normalises the obvious offenders: absolute paths, live process
/// counts, the git SHA, the crate version. It missed a subtler one. An array's
/// recorded shape here is derived from its ELEMENTS, so an array that is empty
/// on a healthy machine and populated on a sick one produces two different
/// snapshots for one unchanged binary.
///
/// `doctor.failed_checks` is exactly that: it is the list of check ids that did
/// not pass, so its emptiness IS the health of the machine running the test.
/// The committed snapshot recorded a host where something was failing, and the
/// test then failed on every healthy host — an alert that carries no
/// information about the code, which is the failure mode the header warns
/// about.
///
/// An entry here is a claim that the array's PRESENCE and TYPE are contract
/// while its CONTENT is not. The tuple requires the reason, so the next entry
/// cannot be added silently.
const HOST_VARIABLE_ARRAYS: &[(&str, &str)] = &[(
    "failed_checks[]",
    "ids of doctor checks that did not pass; population is the health of the \
     host running the test, never a property of the binary",
)];

/// The reason `path` is host-variable, if it is.
fn host_variable_reason(path: &str) -> Option<&'static str> {
    HOST_VARIABLE_ARRAYS
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, reason)| *reason)
}

/// Scalar leaves whose JSON type legitimately differs by PLATFORM.
///
/// # Why this exists (GAP-REL-005)
///
/// `shape` pins a leaf as `string` or `null`, which is exactly right for a
/// field that is always one or the other. It is wrong for a field that is a
/// string on one OS and absent on another: the snapshot then encodes the OS
/// that generated it, and the test fails on every other platform for a reason
/// that has nothing to do with the binary.
///
/// Measured: `golden_shape_doctor` pinned `paths.runtime_dir: string`, which
/// only holds on Linux. `dirs::runtime_dir()` returns `None` on macOS and
/// Windows because `XDG_RUNTIME_DIR` is a Linux concept, and
/// `platform::runtime_directory` documents exactly that. The snapshot had
/// never run outside Linux, so a Linux-only assumption shipped inside the
/// layer whose whole job is catching drift — the same class of defect as the
/// `E0432` that shipped in v1.0.2.
///
/// An entry is a claim that the field's PRESENCE is contract while its type
/// varies by host OS. The tuple requires the reason, so nothing is added
/// silently, and `platform_optional_normalises_across_os` proves the
/// normaliser actually collapses both states.
const PLATFORM_OPTIONAL_SCALARS: &[(&str, &str)] = &[(
    "paths.runtime_dir",
    "XDG_RUNTIME_DIR is a Linux concept; dirs::runtime_dir() is None on macOS \
     and Windows, so the type is string there and null here",
)];

/// The reason `path` is platform-optional, if it is.
fn platform_optional_reason(path: &str) -> Option<&'static str> {
    PLATFORM_OPTIONAL_SCALARS
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, reason)| *reason)
}

/// Collect every path in `value` as `path: type`, arrays collapsed to `[]`.
///
/// Array elements collapse to a single `[]` step and their shapes are UNIONED,
/// so a heterogeneous array shows every variant it contains rather than only
/// the shape of its first element.
fn shape_into(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                out.insert(format!("{prefix}: object(empty)"));
            }
            for (k, v) in map {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                // The discriminator is a `const` in the published schema, so
                // its VALUE is contract and belongs in the snapshot.
                if k == "type" || k == "kind" {
                    if let Value::String(s) = v {
                        out.insert(format!("{path}: string = {s:?}"));
                        continue;
                    }
                }
                match v {
                    Value::Object(_) | Value::Array(_) => shape_into(v, &path, out),
                    leaf => {
                        // A platform-optional scalar collapses to one stable
                        // token, so presence stays contract while the OS-driven
                        // string/null split stops churning the snapshot.
                        if platform_optional_reason(&path).is_some() {
                            out.insert(format!("{path}: platform-optional"));
                        } else {
                            out.insert(format!("{path}: {}", type_name(leaf)));
                        }
                    }
                }
            }
        }
        Value::Array(items) => {
            let path = format!("{prefix}[]");
            if let Some(reason) = host_variable_reason(&path) {
                let _ = reason;
                out.insert(format!("{path}: (host-variable)"));
                return;
            }
            if items.is_empty() {
                out.insert(format!("{path}: (empty)"));
            }
            for item in items {
                match item {
                    Value::Object(_) | Value::Array(_) => shape_into(item, &path, out),
                    leaf => {
                        out.insert(format!("{path}: {}", type_name(leaf)));
                    }
                }
            }
        }
        leaf => {
            out.insert(format!("{prefix}: {}", type_name(leaf)));
        }
    }
}

/// Rendered shape of an envelope, one path per line, sorted.
fn shape(value: &Value) -> String {
    let mut set = BTreeSet::new();
    shape_into(value, "", &mut set);
    set.into_iter().collect::<Vec<_>>().join("\n")
}

/// An isolated config home, so `config` envelopes do not read the developer's.
fn isolated_home() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("ddg-golden-")
        .tempdir()
        .expect("temp config home")
}

#[test]
fn golden_shape_commands() {
    insta::assert_snapshot!(shape(&run_json(&["-q", "-f", "json", "commands"])));
}

#[test]
fn golden_shape_schema_catalog() {
    insta::assert_snapshot!(shape(&run_json(&["-q", "-f", "json", "schema"])));
}

#[test]
fn golden_shape_locale() {
    insta::assert_snapshot!(shape(&run_json(&[
        "-q",
        "-f",
        "json",
        "--ui-lang",
        "en",
        "locale",
    ])));
}

#[test]
fn golden_shape_doctor() {
    insta::assert_snapshot!(shape(&run_json(&["-q", "-f", "json", "doctor"])));
}

#[test]
fn golden_shape_print_budget() {
    insta::assert_snapshot!(shape(&run_json(&[
        "-q",
        "-f",
        "json",
        "deep-research",
        "x",
        "--print-budget",
    ])));
}

#[test]
fn golden_shape_config_family() {
    let home = isolated_home();
    let dir = home.path().to_str().expect("utf-8 temp path");
    let mut rendered = Vec::new();
    for (label, args) in [
        ("config path", vec!["config", "path"]),
        ("config list", vec!["config", "list"]),
        ("config get", vec!["config", "get", "wire_keys"]),
        ("config effective", vec!["config", "effective"]),
        ("config set", vec!["config", "set", "wire_keys", "en"]),
        ("config unset", vec!["config", "unset", "wire_keys"]),
    ] {
        let mut argv = vec!["-q", "-f", "json", "--config-home", dir];
        argv.extend(args);
        rendered.push(format!("== {label} ==\n{}", shape(&run_json(&argv))));
    }
    insta::assert_snapshot!(rendered.join("\n\n"));
}

#[test]
fn golden_shape_init_config_dry_run() {
    let home = isolated_home();
    let dir = home.path().to_str().expect("utf-8 temp path");
    insta::assert_snapshot!(shape(&run_json(&[
        "-q",
        "-f",
        "json",
        "--config-home",
        dir,
        "init-config",
        "--dry-run",
    ])));
}

/// The shape function itself must distinguish the changes it claims to catch.
///
/// A golden layer whose renderer collapses two different documents to the same
/// text is worse than no layer: it reports success for a regression. This pins
/// the four drift kinds the repository has actually hit.
#[test]
fn shape_distinguishes_the_drift_it_claims_to_catch() {
    let base = serde_json::json!({"type": "x", "a": 1, "rows": [{"k": "v"}]});
    let renamed = serde_json::json!({"type": "x", "a": 1, "rows": [{"kk": "v"}]});
    let retyped = serde_json::json!({"type": "x", "a": "1", "rows": [{"k": "v"}]});
    let added = serde_json::json!({"type": "x", "a": 1, "b": 2, "rows": [{"k": "v"}]});
    let rediscriminated = serde_json::json!({"type": "y", "a": 1, "rows": [{"k": "v"}]});

    let b = shape(&base);
    assert_ne!(b, shape(&renamed), "a renamed row column must show");
    assert_ne!(b, shape(&retyped), "a changed leaf type must show");
    assert_ne!(b, shape(&added), "an added key must show");
    assert_ne!(
        b,
        shape(&rediscriminated),
        "a changed discriminator must show"
    );

    // Values that are NOT contract must not churn the snapshot.
    let other_value = serde_json::json!({"type": "x", "a": 2, "rows": [{"k": "w"}]});
    assert_eq!(b, shape(&other_value), "non-discriminator values are noise");
}

/// A platform-optional scalar must shape the same on every OS.
///
/// # Why this is the ruler and not the exemption list
///
/// `PLATFORM_OPTIONAL_SCALARS` is a claim. An entry that does not actually
/// collapse both states is worse than no entry, because it reads as coverage
/// while the snapshot still encodes the OS that produced it. This feeds the
/// normaliser the string form and the null form of the same path and fails if
/// they diverge — so the exemption cannot be present and inert.
#[test]
fn platform_optional_normalises_across_os() {
    for (path, reason) in PLATFORM_OPTIONAL_SCALARS {
        let (parent, leaf) = path
            .rsplit_once('.')
            .unwrap_or_else(|| panic!("{path}: entry must be a nested path"));
        assert!(
            !reason.trim().is_empty(),
            "{path}: an entry without a reason is an unreviewable claim"
        );

        let present = serde_json::json!({ parent: { leaf: "/run/user/1000/x" } });
        let absent = serde_json::json!({ parent: { leaf: Value::Null } });

        assert_eq!(
            shape(&present),
            shape(&absent),
            "{path} is declared platform-optional ({reason}) but the normaliser \
             still distinguishes the string form from the null form, so the \
             snapshot would keep failing on whichever OS did not generate it"
        );
    }
}

/// The doctor snapshot must not depend on the OS that generated it.
///
/// Guards the specific regression of GAP-REL-005: `paths.runtime_dir` was
/// pinned as `string`, which is only true on Linux, so the whole golden layer
/// failed on macOS for a reason unrelated to any drift.
#[test]
fn doctor_paths_shape_is_os_independent() {
    let linux = serde_json::json!({
        "paths": { "config_dir": "/c", "runtime_dir": "/run/user/1000/ddg" }
    });
    let macos = serde_json::json!({
        "paths": { "config_dir": "/c", "runtime_dir": Value::Null }
    });
    assert_eq!(
        shape(&linux),
        shape(&macos),
        "doctor `paths` must shape identically on Linux and macOS"
    );
}

/// A host-variable array must shape the same whether it is empty or full.
///
/// # Why this is the ruler and not the exemption list
///
/// `HOST_VARIABLE_ARRAYS` is a claim, and a claim without a measurement is how
/// `doctor.failed_checks` shipped a snapshot that only passed on an unhealthy
/// machine. This runs the normaliser against both states of the same array and
/// fails if they diverge, so the exemption cannot be present and inert.
#[test]
fn host_variable_arrays_shape_identically_empty_and_populated() {
    let healthy = serde_json::json!({"type": "doctor", "failed_checks": []});
    let sick = serde_json::json!({"type": "doctor", "failed_checks": ["chrome", "xvfb"]});
    assert_eq!(
        shape(&healthy),
        shape(&sick),
        "the snapshot must not change with the health of the machine running it"
    );

    // And the exemption must be narrow: a normal array still distinguishes.
    let empty_rows = serde_json::json!({"type": "doctor", "rows": []});
    let full_rows = serde_json::json!({"type": "doctor", "rows": ["a"]});
    assert_ne!(
        shape(&empty_rows),
        shape(&full_rows),
        "only arrays named in HOST_VARIABLE_ARRAYS may collapse; a blanket \
         collapse would hide the drift this whole layer exists to catch"
    );
}

/// Every host-variable exemption must carry a reason a reader can weigh.
#[test]
fn host_variable_exemptions_are_documented() {
    for (path, reason) in HOST_VARIABLE_ARRAYS {
        assert!(
            path.ends_with("[]"),
            "{path} is not an array path; the exemption only applies to arrays"
        );
        assert!(
            reason.len() > 40,
            "{path} carries the reason {reason:?}, which is too short to tell \
             the next reader why the content is not contract"
        );
    }
}
