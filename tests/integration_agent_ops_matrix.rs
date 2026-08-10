// SPDX-License-Identifier: MIT OR Apache-2.0
//! Every agent-native operator, on every offline surface, must DO or REFUSE.
//!
//! # Why byte counts and not behaviour assertions
//!
//! "Accepted and ignored" is invisible to a behavioural test: the envelope is
//! valid, the exit code is 0, every field a test would check is exactly where
//! it should be. The only observable difference between "honoured" and
//! "silently dropped" is the SIZE of what came back. That is why the v1.0.3
//! and v1.0.4 audits both had to measure bytes by hand to find it, and why the
//! measurement now lives here instead of in someone's shell history.
//!
//! # The measurement trap this file avoids
//!
//! The v1.0.4 audit briefly reported a one-byte reduction on five surfaces
//! that turned out to be an artefact: the baseline was measured through a pipe
//! (trailing newline counted) and the variants through `$(...)` (trailing
//! newline eaten). Every number below comes from the same `Command::output()`
//! call shape, so a difference is a difference in the product.

use std::process::Command;

/// Path to the binary under test, provided by cargo.
const BIN: &str = env!("CARGO_BIN_EXE_duckduckgo-search-cli");

/// Exit code for a refused operation.
const EXIT_REFUSED: i32 = 2;

/// A surface reachable without Chrome or the network.
struct Surface {
    /// Human name used in assertion messages.
    name: &'static str,
    /// Argv after the shared prefix.
    args: &'static [&'static str],
    /// Whether the envelope carries a row array (row operators must work).
    has_rows: bool,
    /// Whether the envelope carries prose `--truncate-content` can shorten.
    ///
    /// `false` means every string is an identifier fed back to a program, so
    /// the surface must REFUSE rather than return byte-identical output — the
    /// second way a no-op can hide, found by this very file.
    has_content: bool,
}

const SURFACES: &[Surface] = &[
    Surface {
        name: "doctor",
        args: &["doctor"],
        has_rows: true,
        has_content: true,
    },
    Surface {
        name: "schema catalog",
        args: &["schema"],
        has_rows: true,
        has_content: true,
    },
    Surface {
        name: "locale",
        args: &["locale"],
        has_rows: true,
        has_content: false,
    },
    Surface {
        name: "config list",
        args: &["config", "list"],
        has_rows: true,
        has_content: false,
    },
    Surface {
        name: "config effective",
        args: &["config", "effective"],
        has_rows: true,
        has_content: false,
    },
    Surface {
        name: "commands",
        args: &["commands"],
        has_rows: false,
        has_content: true,
    },
    Surface {
        name: "config path",
        args: &["config", "path"],
        has_rows: false,
        has_content: false,
    },
    // Added after `every_published_surface_is_exercised_or_excused` reported
    // both as published-but-unmeasured.
    Surface {
        name: "init-config",
        args: &["init-config", "--dry-run"],
        has_rows: true,
        has_content: true,
    },
    Surface {
        name: "config get",
        args: &["config", "get", "ui_lang"],
        has_rows: false,
        has_content: false,
    },
];

/// Operators that need a row array and must refuse without one.
///
/// Derived from the published matrix, not guessed: an operator that every
/// ROWED surface supports and no ROWLESS one does IS a row operator, and
/// `every_published_row_operator_is_exercised` fails if this list falls behind.
/// It did — `--filter` and `--sort` were missing, so a rowless surface that
/// started accepting them instead of refusing would have broken nothing here.
const ROW_OPS: &[&[&str]] = &[
    &["--limit", "1"],
    &["--count-only"],
    &["--dedupe-by", "type"],
    &["--filter", "type=doctor"],
    &["--sort", "type"],
];

/// Run the binary with an isolated config home and return (exit, stdout, stderr).
///
/// `ops` go BEFORE the subcommand. The agent-native operators bind to the root
/// argument set, not to each subcommand, so `doctor --count-only` is a clap
/// usage error (exit 2, empty stdout) and would make every assertion below
/// pass for the wrong reason — it looks exactly like a refusal.
fn run(home: &std::path::Path, ops: &[&str], surface: &[&str]) -> (i32, Vec<u8>, String) {
    let mut cmd = Command::new(BIN);
    cmd.arg("--config-home").arg(home).arg("--no-color");
    cmd.args(ops);
    cmd.args(surface);
    let out = cmd.output().expect("failed to run binary");
    (
        out.status.code().unwrap_or(-1),
        out.stdout,
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn home() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn no_surface_accepts_an_operator_and_ignores_it() {
    let dir = home();
    for s in SURFACES {
        let (base_code, base_out, _) = run(dir.path(), &[], s.args);
        assert!(
            !base_out.is_empty(),
            "{} produced no stdout at all (exit {base_code}); the matrix below \
             would then be comparing nothing against nothing",
            s.name
        );

        // `--fields` applies to any object envelope; `--truncate-content` to
        // any envelope that carries prose.
        let mut ops = vec![vec!["--fields", "type"]];
        if s.has_content {
            ops.push(vec!["--truncate-content", "4"]);
        }
        for op in ops {
            let (code, out, _) = run(dir.path(), &op, s.args);
            if code == EXIT_REFUSED {
                continue; // A loud refusal is an acceptable outcome.
            }
            assert!(
                out.len() < base_out.len(),
                "{} with {:?} exited {code} and returned {} bytes against a \
                 {} byte baseline — the operator was accepted and ignored, \
                 which is the defect this whole layer exists to prevent",
                s.name,
                op,
                out.len(),
                base_out.len()
            );
        }
    }
}

#[test]
fn rowless_surfaces_refuse_row_operators_loudly() {
    let dir = home();
    for s in SURFACES.iter().filter(|s| !s.has_rows) {
        for op in ROW_OPS {
            let (code, out, err) = run(dir.path(), op, s.args);
            assert_eq!(
                code, EXIT_REFUSED,
                "{} has no rows, so {:?} must refuse with exit {EXIT_REFUSED}, \
                 got {code}",
                s.name, op
            );
            let body = String::from_utf8_lossy(&out);
            assert!(
                body.contains("\"error\""),
                "{} refused {:?} but stdout carries no routable envelope: {body:?}",
                s.name,
                op
            );
            assert!(
                !err.trim().is_empty(),
                "{} refused {:?} with nothing on stderr for the operator to read",
                s.name,
                op
            );
        }
    }
}

/// A surface made entirely of identifiers must refuse, not return the same bytes.
///
/// # The second hiding place
///
/// Exempting identity keys from `--truncate-content` was the fix for `invoke`
/// and `id` being mutilated. On `config list` it went further than intended:
/// every string there is an identifier, so once they were all protected the
/// flag returned 1138 bytes against a 1138-byte baseline at exit 0. That is
/// byte-for-byte the signature of the original defect, and the fact that this
/// instance was CORRECT is invisible to the caller. A no-op that is right for
/// a good reason still teaches an agent that the flag works.
#[test]
fn contentless_surfaces_refuse_truncate() {
    let dir = home();
    for s in SURFACES.iter().filter(|s| !s.has_content) {
        let (code, out, err) = run(dir.path(), &["--truncate-content", "4"], s.args);
        assert_eq!(
            code, EXIT_REFUSED,
            "{} carries only identifiers, so --truncate-content must refuse \
             rather than return the input unchanged; got exit {code}",
            s.name
        );
        assert!(
            String::from_utf8_lossy(&out).contains("\"error\""),
            "{} refused --truncate-content without a routable envelope",
            s.name
        );
        assert!(
            !err.trim().is_empty(),
            "{} refused --truncate-content with nothing on stderr",
            s.name
        );
    }
}

#[test]
fn row_surfaces_honour_row_operators() {
    let dir = home();
    for s in SURFACES.iter().filter(|s| s.has_rows) {
        let (_, base_out, _) = run(dir.path(), &[], s.args);
        let (code, out, _) = run(dir.path(), &["--count-only"], s.args);
        assert_eq!(code, 0, "{} --count-only should succeed", s.name);
        assert!(
            out.len() < base_out.len(),
            "{} --count-only returned {} bytes against {} — a count that is not \
             smaller than the thing counted was not applied",
            s.name,
            out.len(),
            base_out.len()
        );
        assert!(
            String::from_utf8_lossy(&out).contains("\"count\""),
            "{} --count-only produced no `count` key",
            s.name
        );
    }
}

/// Refusal must look the SAME everywhere, which is what v1.0.4 got wrong.
///
/// `doctor`, `locale`, `commands`, `schema` and `init-config` wrote prose to
/// stderr and left stdout empty; `config` wrote a JSON envelope to stdout and
/// nothing to stderr. Same flag, same failure, two shapes — so an agent that
/// parsed stdout could route a refusal from one family and saw silence from
/// the other, with no way to tell it apart from a crash.
#[test]
fn refusal_has_one_shape_across_families() {
    let dir = home();
    let mut shapes = Vec::new();
    for s in SURFACES.iter().filter(|s| !s.has_rows) {
        let (code, out, err) = run(dir.path(), &["--limit", "1"], s.args);
        let body: serde_json::Value = serde_json::from_slice(&out).unwrap_or_else(|e| {
            panic!(
                "{} refusal is not JSON on stdout: {e}; got {:?}",
                s.name,
                String::from_utf8_lossy(&out)
            )
        });
        let mut keys: Vec<String> = body
            .as_object()
            .expect("refusal must be a JSON object")
            .keys()
            .cloned()
            .collect();
        keys.sort();
        shapes.push((s.name, code, keys, !err.trim().is_empty()));
    }

    let first = shapes
        .first()
        .expect("at least one rowless surface")
        .clone();
    for (name, code, keys, has_stderr) in &shapes {
        assert_eq!(
            (*code, keys.clone(), *has_stderr),
            (first.1, first.2.clone(), first.3),
            "{name} refuses with a different contract than {}: one family \
             cannot answer in a shape another does not",
            first.0
        );
    }
}

/// `--truncate-content` must not mutilate a value the agent feeds back.
///
/// `schema --truncate-content 12` used to turn `invoke` into `duckduckgo-s`
/// and `id` into `searc`: a command line that no longer runs and a name
/// `schema --name` rejects. Output that looks valid and is not is strictly
/// worse than output that is obviously wrong.
#[test]
fn truncate_preserves_contract_identity_on_the_schema_catalog() {
    let dir = home();
    let (code, out, _) = run(dir.path(), &["--truncate-content", "12"], &["schema"]);
    assert_eq!(code, 0, "schema --truncate-content should succeed");
    let doc: serde_json::Value = serde_json::from_slice(&out).expect("catalog JSON");
    let rows = doc["schemas"].as_array().expect("schemas array");
    assert!(!rows.is_empty(), "catalog is empty");

    for row in rows {
        let id = row["id"].as_str().expect("id");
        let invoke = row["invoke"].as_str().expect("invoke");
        assert!(
            invoke.starts_with("duckduckgo-search-cli schema --name "),
            "invoke was truncated into something unrunnable: {invoke:?}"
        );
        assert!(
            invoke.ends_with(id),
            "invoke {invoke:?} does not end with its own id {id:?}"
        );
        assert!(
            id.len() > 5,
            "id {id:?} looks truncated; ids are fed back to `schema --name`"
        );
    }
}

/// The exemption must not silently become total: content still shrinks.
#[test]
fn truncate_still_shortens_actual_content() {
    let dir = home();
    let (_, base, _) = run(dir.path(), &[], &["schema"]);
    let (code, small, _) = run(dir.path(), &["--truncate-content", "4"], &["schema"]);
    assert_eq!(code, 0);
    assert!(
        small.len() < base.len(),
        "truncate returned {} bytes against a {} byte baseline — exempting \
         identity keys must not turn the flag back into the no-op it was",
        small.len(),
        base.len()
    );
}

/// Every surface the binary PUBLISHES must be exercised or explicitly excused.
///
/// # The class this closes
///
/// `SURFACES` above is a hand-written copy of the capability matrix. The
/// original in `src/output/envelope_ops.rs` has thirteen rows; this copy had
/// seven. The six that were never exercised are `init-config`, `config get`,
/// `config set`, `config unset`, `--probe` and `--probe-deep` — which is to say
/// the matrix test was measuring a bit over half of the contract while its name
/// promised the whole of it.
///
/// This is the same failure as the language ruler that only read comments, and
/// it has the same shape: a guard that keeps its own copy of the target
/// measures the copy. The fix in both cases is to stop copying. The list of
/// argv still has to live here, because the published matrix names surfaces and
/// not command lines, but every published surface must now appear in it or in
/// an exclusion that states its reason.
#[test]
fn every_published_surface_is_exercised_or_excused() {
    /// Published surfaces this harness cannot drive, and why.
    const NOT_EXERCISABLE: &[(&str, &str)] = &[
        (
            "--probe",
            "reaches the live DuckDuckGo endpoint through Chrome; an offline \
             matrix test would assert on the host's network, not on the code",
        ),
        (
            "--probe-deep",
            "same live transport as --probe, plus the anti-bot cascade",
        ),
        (
            "config set",
            "mutates the config file; the row operators it publishes are the \
             two universal ones, already covered by `config get`",
        ),
        (
            "config unset",
            "mutates the config file; identical published capability to \
             `config set`",
        ),
        (
            "buscar",
            "every envelope comes from a live SERP over Chrome, so driving it \
             here would assert on DuckDuckGo's availability rather than on \
             this binary; the operators themselves are unit-tested against \
             `SearchOutput` fixtures in `src/output/agent_ops.rs`",
        ),
        (
            "deep-research",
            "same live transport, multiplied by the sub-query fan-out; the \
             one operator whose deep behaviour was actually wrong, \
             `--truncate-content` over `synthesis.body`, is pinned by \
             `deep_truncate_shortens_the_synthesised_report`",
        ),
    ];

    let dir = home();
    let (code, stdout, stderr) = run(dir.path(), &[], &["commands"]);
    assert_eq!(code, 0, "commands must succeed; stderr: {stderr}");
    let doc: serde_json::Value =
        serde_json::from_slice(&stdout).expect("commands emits a JSON envelope");
    let published = doc["agent_ops"]
        .as_array()
        .expect("commands publishes agent_ops as an array of surfaces");
    assert!(
        published.len() >= 13,
        "the published matrix shrank to {} surfaces; if a surface was removed \
         on purpose, remove its row here too",
        published.len()
    );

    let exercised: Vec<&str> = SURFACES.iter().map(|s| s.name).collect();
    let mut unmeasured = Vec::new();
    for surface in published {
        let name = surface["surface"].as_str().unwrap_or_default();
        // The local table names `schema` as `schema catalog` for readability.
        let covered = exercised
            .iter()
            .any(|e| *e == name || e.starts_with(&format!("{name} ")));
        let excused = NOT_EXERCISABLE.iter().any(|(s, _)| *s == name);
        if !covered && !excused {
            unmeasured.push(name.to_string());
        }
    }

    assert!(
        unmeasured.is_empty(),
        "these surfaces are published in `agent_ops` and exercised by nothing \
         in this file: {unmeasured:?}\n\n\
         A capability matrix that lists a surface is a promise to the agent \
         reading it. Add a row to SURFACES, or add the surface to \
         NOT_EXERCISABLE with the reason it cannot be driven here."
    );

    // The exclusion list must not outlive the surfaces it names.
    for (name, reason) in NOT_EXERCISABLE {
        assert!(
            published
                .iter()
                .any(|s| s["surface"].as_str() == Some(*name)),
            "NOT_EXERCISABLE names {name:?}, which the binary no longer \
             publishes — remove the row rather than leaving a claim about a \
             surface that is gone"
        );
        assert!(
            reason.len() > 30,
            "{name} carries the reason {reason:?}, too short to weigh"
        );
    }
}

/// Every row operator the matrix publishes must be exercised by ROW_OPS.
///
/// `ROW_OPS` above listed three of the five the product publishes: `--filter`
/// and `--sort` were absent, so a rowless surface that started ACCEPTING them
/// instead of refusing would not have failed anything.
#[test]
fn every_published_row_operator_is_exercised() {
    let dir = home();
    let (code, stdout, _) = run(dir.path(), &[], &["commands"]);
    assert_eq!(code, 0);
    let doc: serde_json::Value = serde_json::from_slice(&stdout).expect("JSON envelope");
    let published = doc["agent_ops"].as_array().expect("agent_ops array");

    // Operators a ROWED surface supports but a ROWLESS one does not are, by
    // definition, the row operators.
    let rowed: std::collections::BTreeSet<&str> = published
        .iter()
        .filter(|s| !s["rows"].is_null())
        .flat_map(|s| s["supports"].as_array().into_iter().flatten())
        .filter_map(serde_json::Value::as_str)
        .collect();
    let rowless: std::collections::BTreeSet<&str> = published
        .iter()
        .filter(|s| s["rows"].is_null())
        .flat_map(|s| s["supports"].as_array().into_iter().flatten())
        .filter_map(serde_json::Value::as_str)
        .collect();
    let row_only: Vec<&&str> = rowed.difference(&rowless).collect();

    let exercised: Vec<&str> = ROW_OPS
        .iter()
        .filter_map(|op| op.first().copied())
        .collect();
    let missing: Vec<&&&str> = row_only
        .iter()
        .filter(|op| !exercised.contains(**op))
        .collect();
    assert!(
        missing.is_empty(),
        "these operators need a row array and are exercised by nothing: \
         {missing:?}\nPublished row-only operators: {row_only:?}\nROW_OPS \
         covers: {exercised:?}"
    );
}
