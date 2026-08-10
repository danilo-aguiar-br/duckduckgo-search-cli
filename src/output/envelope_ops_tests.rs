// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit tests for the generic envelope reduction (`envelope_ops`).

use super::*;
use serde_json::json;

/// Shape of `doctor`: rows under `checks`, discriminator on `type`.
fn doctor_shape() -> EnvelopeShape {
    EnvelopeShape::with_rows("doctor", "checks", "type")
}

/// Shape of `commands`: no row array at all.
fn commands_shape() -> EnvelopeShape {
    EnvelopeShape::rowless("commands", "type")
}

fn doctor_envelope() -> Value {
    json!({
        "type": "doctor",
        "version": "1.0.4",
        "ok": true,
        "failed_checks": [],
        "checks": [
            {"id": "chrome_detect", "ok": true,  "severity": "hard", "detail": "found"},
            {"id": "config_dir",    "ok": false, "severity": "soft", "detail": "absent"},
            {"id": "feature",       "ok": true,  "severity": "hard", "detail": "chrome"},
        ],
    })
}

fn ops() -> AgentOps {
    AgentOps::default()
}

#[test]
fn no_ops_leaves_the_envelope_untouched() {
    let mut v = doctor_envelope();
    let before = v.clone();
    apply_generic(&mut v, &ops(), &doctor_shape()).expect("no-op succeeds");
    assert_eq!(v, before);
}

#[test]
fn fields_projects_top_level_and_keeps_the_discriminator() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        fields: Some("ok".into()),
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("projection succeeds");
    let obj = v.as_object().expect("object");
    assert_eq!(obj.len(), 2, "discriminator plus one path: {obj:?}");
    assert_eq!(obj["type"], "doctor");
    assert_eq!(obj["ok"], true);
}

#[test]
fn fields_projects_row_columns_through_a_dotted_path() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        fields: Some("checks.id".into()),
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("projection succeeds");
    assert_eq!(
        v,
        json!({
            "type": "doctor",
            "checks": [{"id": "chrome_detect"}, {"id": "config_dir"}, {"id": "feature"}],
        })
    );
}

#[test]
fn fields_refuses_a_path_that_matches_nothing() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        fields: Some("nope".into()),
        ..ops()
    };
    let err = apply_generic(&mut v, &o, &doctor_shape()).expect_err("unknown path must refuse");
    let msg = err.to_string();
    assert!(msg.contains("nope"), "names the bad path: {msg}");
    assert!(msg.contains("version"), "lists what does exist: {msg}");
}

/// A dotted miss must point at the level where the path broke, not the root.
#[test]
fn fields_error_reports_the_level_that_failed() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        fields: Some("checks.nope".into()),
        ..ops()
    };
    let err = apply_generic(&mut v, &o, &doctor_shape()).expect_err("must refuse");
    let msg = err.to_string();
    assert!(msg.contains("`checks`"), "names the level: {msg}");
    assert!(msg.contains("severity"), "lists the ROW keys: {msg}");
    assert!(
        !msg.contains("version"),
        "must not list top-level keys for a row miss: {msg}"
    );
}

#[test]
fn filter_keeps_only_matching_rows() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        filter: Some("ok=false".into()),
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("filter succeeds");
    let rows = v["checks"].as_array().expect("array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], "config_dir");
}

#[test]
fn filter_substring_and_negation() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        filter: Some("id~config".into()),
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("filter succeeds");
    assert_eq!(v["checks"].as_array().expect("array").len(), 1);

    let mut v = doctor_envelope();
    let o = AgentOps {
        filter: Some("severity!=hard".into()),
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("filter succeeds");
    assert_eq!(v["checks"].as_array().expect("array").len(), 1);
}

#[test]
fn filter_rejects_a_malformed_expression() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        filter: Some("garbage".into()),
        ..ops()
    };
    assert!(apply_generic(&mut v, &o, &doctor_shape()).is_err());
}

#[test]
fn sort_orders_rows_and_desc_reverses() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        sort: Some("id".into()),
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("sort succeeds");
    let ids: Vec<&str> = v["checks"]
        .as_array()
        .expect("array")
        .iter()
        .map(|r| r["id"].as_str().expect("str"))
        .collect();
    assert_eq!(ids, ["chrome_detect", "config_dir", "feature"]);

    let mut v = doctor_envelope();
    let o = AgentOps {
        sort: Some("id:desc".into()),
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("sort succeeds");
    let ids: Vec<&str> = v["checks"]
        .as_array()
        .expect("array")
        .iter()
        .map(|r| r["id"].as_str().expect("str"))
        .collect();
    assert_eq!(ids, ["feature", "config_dir", "chrome_detect"]);
}

#[test]
fn sort_refuses_a_key_no_row_carries() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        sort: Some("nope".into()),
        ..ops()
    };
    let err = apply_generic(&mut v, &o, &doctor_shape()).expect_err("unknown key must refuse");
    assert!(err.to_string().contains("severity"), "lists real keys");
}

#[test]
fn dedupe_drops_repeats_and_keeps_rows_without_the_key() {
    let mut v = json!({
        "type": "doctor",
        "checks": [{"k": "a"}, {"k": "a"}, {"k": "b"}, {"other": 1}],
    });
    let o = AgentOps {
        dedupe_by: Some("k".into()),
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("dedupe succeeds");
    assert_eq!(v["checks"].as_array().expect("array").len(), 3);
}

#[test]
fn limit_applies_after_filter() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        filter: Some("ok=true".into()),
        limit: Some(1),
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("filter then limit succeeds");
    let rows = v["checks"].as_array().expect("array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], "chrome_detect");
}

#[test]
fn count_only_replaces_the_payload_and_keeps_routing() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        count_only: true,
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("count succeeds");
    assert_eq!(v, json!({"type": "doctor", "count": 3}));
}

#[test]
fn count_only_counts_what_survived_the_filter() {
    let mut v = doctor_envelope();
    let o = AgentOps {
        filter: Some("ok=true".into()),
        count_only: true,
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("count succeeds");
    assert_eq!(v["count"], 2);
}

#[test]
fn truncate_content_caps_every_string_by_scalars() {
    let mut v = json!({"type": "doctor", "detail": "áéíóú-long-tail"});
    let o = AgentOps {
        truncate_content: Some(5),
        ..ops()
    };
    apply_generic(&mut v, &o, &commands_shape()).expect("truncate succeeds");
    assert_eq!(v["detail"], "áéíóú", "counts scalars, never bytes");
}

/// Truncation must never mangle the discriminator.
///
/// `--truncate-content 8` turned `"config_path"` into `"config_p"`, a value no
/// published schema declares. The envelope stopped matching its own `const`
/// and became unroutable — the caller asked to shorten content, not to break
/// the contract.
#[test]
fn truncate_content_never_touches_the_discriminator() {
    let mut v = json!({"type": "config_path", "config_file": "/very/long/path/config.toml"});
    let o = AgentOps {
        truncate_content: Some(8),
        ..ops()
    };
    let shape = EnvelopeShape::rowless("config path", "type");
    apply_generic(&mut v, &o, &shape).expect("truncate succeeds");
    assert_eq!(v["type"], "config_path", "discriminator survives intact");
    assert_eq!(v["config_file"], "/very/lo", "everything else is shortened");
}

/// The heart of the fix: a rowless envelope REFUSES every row operation.
#[test]
fn rowless_envelope_refuses_every_row_operation() {
    let cases: [(&str, AgentOps); 5] = [
        (
            "--filter",
            AgentOps {
                filter: Some("a=b".into()),
                ..ops()
            },
        ),
        (
            "--sort",
            AgentOps {
                sort: Some("a".into()),
                ..ops()
            },
        ),
        (
            "--dedupe-by",
            AgentOps {
                dedupe_by: Some("a".into()),
                ..ops()
            },
        ),
        (
            "--limit",
            AgentOps {
                limit: Some(1),
                ..ops()
            },
        ),
        (
            "--count-only",
            AgentOps {
                count_only: true,
                ..ops()
            },
        ),
    ];
    for (flag, o) in cases {
        let mut v = json!({"type": "commands", "root": {"name": "x"}});
        let err = apply_generic(&mut v, &o, &commands_shape())
            .expect_err("row op must be refused on a rowless envelope");
        let msg = err.to_string();
        assert!(msg.contains(flag), "{flag}: message names the flag: {msg}");
        assert!(
            msg.contains("commands"),
            "{flag}: message names the surface: {msg}"
        );
        // v1.0.5: this used to require the message to LIST the supported
        // flags. The list was hardcoded in one shared sentence and was wrong
        // on every contentless surface — `config path --limit 1` advertised
        // `--truncate-content`, which that same surface refuses. A shared
        // message cannot know the surface it is describing, so it now points
        // at the matrix that does.
        assert!(
            msg.contains("commands") && msg.contains("agent_ops"),
            "{flag}: message must point at the published capability matrix \
             instead of repeating a list that can be wrong here: {msg}"
        );
    }
}

/// A rowless envelope still honours the operations that do have meaning.
#[test]
fn rowless_envelope_still_projects_and_truncates() {
    let mut v = json!({"type": "commands", "version": "1.0.4", "binary": "ddg"});
    let o = AgentOps {
        fields: Some("version".into()),
        ..ops()
    };
    apply_generic(&mut v, &o, &commands_shape()).expect("projection is always available");
    assert_eq!(v, json!({"type": "commands", "version": "1.0.4"}));
}

/// A filter that removes every row must not turn projection into an error.
///
/// `--filter ok=false --fields checks.id` on a healthy host leaves `checks`
/// empty. Reading that as "no such key" reported a typo the operator did not
/// make, and exit 2 for a perfectly ordinary empty result.
#[test]
fn projecting_an_empty_row_array_yields_an_empty_array() {
    let mut v = json!({"type": "doctor", "checks": []});
    let o = AgentOps {
        fields: Some("checks.id".into()),
        ..ops()
    };
    apply_generic(&mut v, &o, &doctor_shape()).expect("empty rows project to empty");
    assert_eq!(v, json!({"type": "doctor", "checks": []}));
}

/// The typo case still fails, so the relaxation above did not blind the check.
#[test]
fn projecting_a_missing_row_column_still_refuses() {
    let mut v = json!({"type": "doctor", "checks": [{"id": "a"}]});
    let o = AgentOps {
        fields: Some("checks.nope".into()),
        ..ops()
    };
    assert!(apply_generic(&mut v, &o, &doctor_shape()).is_err());
}

#[test]
fn published_surfaces_have_unique_names() {
    let mut names: Vec<&str> = SURFACES.iter().map(|s| s.surface).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "duplicate surface in SURFACES");
}
