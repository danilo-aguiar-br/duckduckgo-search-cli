// SPDX-License-Identifier: MIT OR Apache-2.0
//! Guards the stdout boundary: no unreduced emission path may appear silently.
//!
//! # The defect this file exists to prevent
//!
//! v1.0.4 set out to abolish "flag accepted and ignored". It converted the six
//! surfaces the plan happened to NAME — `doctor`, `commands`, `schema`,
//! `locale`, `config`, `init-config` — and declared the class closed. It was
//! not. `--probe` and `--probe-deep` emitted through their own helper, which
//! never reached the projector, and every operator stayed a no-op there:
//! `--count-only`, `--limit 1`, `--fields status` and `--truncate-content 5`
//! each returned 633 bytes against a 633-byte baseline, at exit 0.
//!
//! The failure was not the missed surface. It was the METHOD: a class was
//! closed by enumerating targets, and an enumeration is only as complete as
//! the person writing it. A list cannot report what is missing from itself.
//!
//! This file is the ruler that replaces the list. It sweeps EVERY stdout
//! emission in `src/` and requires each one outside the output module to be
//! declared with a reason. A new bypass fails the build; a stale exemption
//! fails the build too, so the allowlist cannot rot into decoration.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Call syntaxes that put bytes on stdout.
///
/// `println!` and `print!` are included even though the output module forbids
/// them elsewhere: the point of a ruler is to check, not to trust the rule.
const EMITTERS: &[&str] = &[
    "print_line_stdout(",
    "write_to_stdout(",
    "println!(",
    "print!(",
];

/// A stdout write outside `src/output/` that is allowed, and why.
///
/// Adding a row here is a deliberate act and the reason is read by whoever
/// audits this next. "It was already there" is not a reason.
struct Exemption {
    /// Path relative to the crate root.
    file: &'static str,
    /// Substring identifying the statement, so the row cannot drift to another call.
    needle: &'static str,
    /// Why this write does not belong behind the agent-native projector.
    reason: &'static str,
}

const EXEMPT: &[Exemption] = &[
    Exemption {
        file: "src/commands/schema_cmd.rs",
        needle: "output::print_line_stdout(body.trim_end())",
        reason: "`schema --name` emits a JSON Schema DOCUMENT, not an envelope. \
                 Projecting or truncating it would hand the caller a schema that \
                 no longer validates anything, which is worse than a large one. \
                 `--max-output-bytes` still applies at the stdout choke point.",
    },
    Exemption {
        file: "src/commands/schema_cmd.rs",
        needle: "match output::print_line_stdout(&json)",
        reason: "`print_json` emits the unknown-schema refusal, which is already \
                 a published routable contract (`classified-error-output`) and \
                 already exits 2. It carries no rows and no content field, so \
                 every reduction would be a no-op on it by construction.",
    },
];

/// Every `.rs` file under `src/`, excluding the output module itself.
fn source_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Relative path with forward slashes, so the allowlist reads the same everywhere.
fn relative(path: &Path) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Whether a line is prose rather than code.
///
/// Doc comments in this crate quote emitter names constantly — the module
/// header above does it a dozen times — and a ruler that cannot tell a mention
/// from a call would force those explanations to be deleted to stay green.
fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with("*") || t.starts_with("/*")
}

/// Whether `line` calls `needle` as its own identifier.
///
/// A plain `contains` is wrong here and the first run proved it: `eprintln!(`
/// contains `println!(`, so the ruler reported four STDERR writes in `main.rs`
/// and `gen_man.rs` as unreduced stdout. A guard that cries wolf gets an
/// allowlist entry written to silence it, and that entry is a lie that outlives
/// everyone who remembers why it is there. Requiring the preceding character
/// to be a non-identifier character makes the match mean what it says.
fn calls(line: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(idx) = line[from..].find(needle) {
        let at = from + idx;
        let boundary = at == 0
            || !line[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
        if boundary {
            return true;
        }
        from = at + needle.len();
    }
    false
}

/// One stdout write found in the source.
struct Found {
    file: String,
    line: usize,
    text: String,
}

fn scan() -> Vec<Found> {
    let mut found = Vec::new();
    for path in source_files() {
        let rel = relative(&path);
        // The output module IS the boundary; it is allowed to write.
        if rel.starts_with("src/output/") {
            continue;
        }
        let body = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for (i, line) in body.lines().enumerate() {
            if is_comment(line) {
                continue;
            }
            if EMITTERS.iter().any(|e| calls(line, e)) {
                found.push(Found {
                    file: rel.clone(),
                    line: i + 1,
                    text: line.trim().to_string(),
                });
            }
        }
    }
    found
}

#[test]
fn every_stdout_write_outside_the_output_module_is_declared() {
    let found = scan();
    let mut undeclared = Vec::new();
    for f in &found {
        let covered = EXEMPT
            .iter()
            .any(|e| e.file == f.file && f.text.contains(e.needle));
        if !covered {
            undeclared.push(format!("{}:{} — {}", f.file, f.line, f.text));
        }
    }
    assert!(
        undeclared.is_empty(),
        "stdout is written outside `src/output/` by code that is not declared:\n  {}\n\n\
         Every byte on stdout must pass the agent-native projector, or the flags \
         the caller set are accepted and ignored — the exact defect that survived \
         v1.0.4 on `--probe`. Either route this through \
         `output::emit_envelope_or_refuse`, or add an `Exemption` to \
         tests/integration_stdout_boundary.rs stating why this write is not an \
         envelope.",
        undeclared.join("\n  ")
    );
}

#[test]
fn no_exemption_outlives_the_code_it_excuses() {
    let found = scan();
    let mut stale = Vec::new();
    for e in EXEMPT {
        let still_there = found
            .iter()
            .any(|f| f.file == e.file && f.text.contains(e.needle));
        if !still_there {
            stale.push(format!("{} — {}", e.file, e.needle));
        }
    }
    assert!(
        stale.is_empty(),
        "these exemptions no longer match any code:\n  {}\n\n\
         An allowlist that keeps rows for deleted code stops being a record of \
         decisions and becomes decoration — and decoration is what let a whole \
         surface hide in v1.0.4. Delete the row.",
        stale.join("\n  ")
    );
}

#[test]
fn every_exemption_states_a_reason() {
    for e in EXEMPT {
        assert!(
            e.reason.len() > 40,
            "exemption for {} has no usable reason: {:?}",
            e.file,
            e.reason
        );
    }
}

/// The scan must be able to see something, or every assertion above is vacuous.
///
/// A ruler that silently matches nothing passes forever. This pins the scan to
/// a fact that is true today and would break loudly if the traversal, the
/// extension filter or the comment skip ever stopped working.
#[test]
fn the_scan_is_not_vacuous() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/output");
    assert!(
        root.is_dir(),
        "src/output must exist for the exclusion to mean anything"
    );
    assert!(
        source_files().len() > 40,
        "the source walk found too few files to be working"
    );

    // The output module itself must contain emitters the scan is excluding.
    let emit = std::fs::read_to_string(root.join("emit.rs")).expect("emit.rs");
    assert!(
        emit.lines().any(|l| EMITTERS.iter().any(|e| calls(l, e))),
        "src/output/emit.rs no longer contains any known emitter — the EMITTERS \
         list is stale and the scan is looking for syntax that does not exist"
    );

    // The boundary matcher must actually discriminate, or it degrades to
    // `contains` and starts reporting every `eprintln!` as a stdout write.
    assert!(calls("    println!(\"x\");", "println!("));
    assert!(!calls("    eprintln!(\"x\");", "println!("));
    assert!(!calls("    eprint!(\"x\");", "print!("));
    assert!(calls(
        "    output::print_line_stdout(&s)",
        "print_line_stdout("
    ));

    // And the exempt set must be a strict subset of what the scan can see.
    let files: BTreeSet<String> = scan().into_iter().map(|f| f.file).collect();
    for e in EXEMPT {
        assert!(
            files.contains(e.file),
            "exemption names {}, which the scan never visits",
            e.file
        );
    }
}
