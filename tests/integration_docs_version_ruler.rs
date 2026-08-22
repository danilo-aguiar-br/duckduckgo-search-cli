// SPDX-License-Identifier: MIT OR Apache-2.0
//! Guards the version label: no document may declare a stale CURRENT version.
//!
//! # The defect this file exists to prevent
//!
//! `tests/integration_docs_drift.rs` has 20 tests and covers 22 documents. The
//! repository has 44. Measured on 2026-08-21, ELEVEN documents still announced
//! `1.0.5` as the current line while the tree was at `1.0.6` — and the list of
//! liars was almost exactly the list of files the drift gate never opens.
//!
//! `SECURITY.md` promised support for a version that was no longer the latest.
//! `docs/CROSS_PLATFORM.md` said `v1.0.3` while its Portuguese twin said
//! `v1.0.2`. `docs/INSTALL-WINDOWS.md` told Windows users to install, without
//! mentioning that the version crates.io served did not compile for them.
//!
//! This is the SAME class as GAP-REL-001. There, a configuration existed that
//! no gate compiled. Here, a document exists that no test reads. In both cases
//! everything was green while the user received something broken. A file under
//! a ruler cannot rot, because the build breaks when it lies; a file outside
//! one rots in silence until a human happens to read it.
//!
//! # What this ruler deliberately does NOT do
//!
//! It does not police historical references. `since v0.9.8`, `ADR-0027 in
//! v1.0.2` and a `## [1.0.4]` changelog heading are all CORRECT and must stay
//! frozen — rewriting them would destroy the only value those lines have. Only
//! a phrase that CLAIMS to state the current version is measured.
//!
//! # Relationship to `integration_root_docs.rs`
//!
//! That file already has `no_document_claims_a_current_version_that_is_not_shipped`,
//! and this one was written without noticing it. Keeping both is deliberate,
//! because they measure different failures — but nobody should delete one as a
//! duplicate of the other without reading this paragraph first.
//!
//! The older guard matches a closed list of line OPENERS (`Current version:`,
//! `Versão atual:`) across 29 documents. It is stricter about shape and looser
//! about reach. Its own comment records the same lesson twice over: the list
//! grew to four entries only after three documents invented a third wording it
//! had never been taught.
//!
//! This guard drops the opener requirement, so it sees a claim buried
//! mid-sentence. That is not hypothetical — `CONTRIBUTING.md:95` read
//! `still current in v1.0.5` inside a bullet about lifecycle E2E, and both the
//! older guard and the first draft of THIS one walked straight past it. It also
//! walks `skills/`, which neither the older guard nor the drift gate opens, and
//! it adds the bilingual cross-check, because a pair can be internally
//! consistent and still contradict its own translation.

use std::path::{Path, PathBuf};

/// Phrases that announce "this is the version you are running", in both languages.
///
/// Each entry is matched case-insensitively. A version literal appearing on the
/// same line AFTER one of these is treated as a claim about the present.
const CURRENT_VERSION_MARKERS: &[&str] = &[
    "current version",
    "current release",
    "current line",
    "current product line",
    "current line documented here",
    // Measured 2026-08-21: `CONTRIBUTING.md:95` read "still current in v1.0.5"
    // and the first version of this ruler walked straight past it, because the
    // claim hides mid-sentence instead of opening the line. A marker list is
    // only as good as the phrasings someone thought to write down.
    "still current in",
    // Measured 2026-08-21: `docs/generated/flags_en.md:3` declared
    // "generated from ... on binary **v1.0.5**" while the tree was at 1.0.6, and
    // NONE of the markers above saw it. That file is the worst possible place to
    // rot: it calls itself an SSOT generated from the binary, so a reader trusts
    // it more than prose — and no generator actually rewrites it, so the label is
    // maintained by hand while wearing the costume of automation.
    //
    // The marker had to be the ACT of generating, not the word "binary". The
    // first attempt matched the word for "binary" in both languages and
    // immediately failed on `BENCHMARKS.pt-BR.md:66`, a line that states the
    // table was measured on v0.7.10 and has NOT been re-measured since — an
    // honest disclosure of a stale benchmark, turned into a build failure by a
    // marker that was too broad. A ruler that punishes transparency teaches
    // people to delete the disclosure, which is the opposite of what it is for.
    "generated from",
    "gerado a partir de",
    "versão atual",
    "release atual",
    "linha atual",
    "linha atual documentada aqui",
    "ainda vigentes na",
    "ainda vigente na",
];

/// Files that record the past on purpose, so a stale label is their content.
///
/// `CHANGELOG` and `MIGRATION` are ledgers of previous releases. `decisions/`
/// holds ADRs, which are immutable records of a decision at a point in time.
/// `gaps.md` narrates an audit. Freezing them is the whole point.
const HISTORICAL_BY_DESIGN: &[&str] = &[
    "CHANGELOG.md",
    "CHANGELOG.pt-BR.md",
    "docs/MIGRATION.md",
    "docs/MIGRATION.pt-BR.md",
    "gaps.md",
];

/// Every documentation file that claims to describe the present.
fn documentation_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf(), root.join("docs"), root.join("skills")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = relative(&path);
            if path.is_dir() {
                // ADRs are immutable records; target/ is build output.
                if rel.ends_with("decisions") || rel.ends_with("target") || rel.contains("/.") {
                    continue;
                }
                stack.push(path);
                continue;
            }
            let is_doc = path.extension().is_some_and(|e| e == "md" || e == "txt");
            if is_doc && !HISTORICAL_BY_DESIGN.contains(&rel.as_str()) {
                out.push(path);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn relative(path: &Path) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The first `X.Y.Z` appearing in `rest`, ignoring a leading `v` and markup.
///
/// Markdown wraps these in `**`, backticks and brackets, so the scan walks
/// characters rather than trusting a token split.
fn first_version(rest: &str) -> Option<String> {
    let bytes: Vec<char> = rest.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut dots = 0;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == '.') {
                if bytes[i] == '.' {
                    dots += 1;
                }
                i += 1;
            }
            let literal: String = bytes[start..i].iter().collect();
            // Require a full three-part version, and reject a trailing dot
            // from a sentence end such as "1.0.6." at the close of a clause.
            let trimmed = literal.trim_end_matches('.');
            if dots >= 2 && trimmed.split('.').count() == 3 {
                return Some(trimmed.to_string());
            }
            continue;
        }
        i += 1;
    }
    None
}

/// A claim that a document makes about the current version.
struct Claim {
    file: String,
    line: usize,
    claimed: String,
    text: String,
}

fn claims() -> Vec<Claim> {
    let mut found = Vec::new();
    for path in documentation_files() {
        let rel = relative(&path);
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (i, line) in body.lines().enumerate() {
            let lower = line.to_lowercase();
            for marker in CURRENT_VERSION_MARKERS {
                let Some(at) = lower.find(marker) else {
                    continue;
                };
                let rest = &line[at + marker.len()..];
                // Stop at a sentence boundary so a later historical mention on
                // the same line is not read as part of this claim.
                let clause = rest.split(['.', ';'].as_ref()).next().unwrap_or(rest);
                let scan = if first_version(clause).is_some() {
                    clause
                } else {
                    rest
                };
                if let Some(v) = first_version(scan) {
                    found.push(Claim {
                        file: rel.clone(),
                        line: i + 1,
                        claimed: v,
                        text: line.trim().chars().take(160).collect(),
                    });
                }
                break;
            }
        }
    }
    found
}

#[test]
fn no_document_declares_a_stale_current_version() {
    let expected = env!("CARGO_PKG_VERSION");
    let stale: Vec<String> = claims()
        .into_iter()
        .filter(|c| c.claimed != expected)
        .map(|c| {
            format!(
                "{}:{} — claims {}, tree is {}\n      {}",
                c.file, c.line, c.claimed, expected, c.text
            )
        })
        .collect();

    assert!(
        stale.is_empty(),
        "these documents announce a version the tree does not have:\n  {}\n\n\
         A document that names the wrong current version sends users to install \
         it. v1.0.2 was served by crates.io unable to compile on macOS or \
         Windows, and the install guide kept telling people to run \
         `cargo install` with no version pin.\n\n\
         Either update the label, or — if this line records HISTORY rather than \
         the present — rephrase it so it does not read as a claim about now \
         (`since v0.9.8`, `introduced in v1.0.2`), because this ruler only \
         measures phrases that announce the CURRENT version.",
        stale.join("\n  ")
    );
}

/// The pair `X.md` / `X.pt-BR.md` must agree about what version is current.
///
/// Measured on 2026-08-21: `docs/CROSS_PLATFORM.md` said `v1.0.3` while
/// `docs/CROSS_PLATFORM.pt-BR.md` said `v1.0.2`. Neither was right, and a
/// reader of either one had no way to know the other disagreed.
#[test]
fn bilingual_pairs_agree_on_the_current_version() {
    let all = claims();
    let mut mismatches = Vec::new();
    for c in &all {
        let Some(stem) = c.file.strip_suffix(".pt-BR.md") else {
            continue;
        };
        let english = format!("{stem}.md");
        for e in all.iter().filter(|e| e.file == english) {
            if e.claimed != c.claimed {
                mismatches.push(format!(
                    "{}:{} says {} but {}:{} says {}",
                    c.file, c.line, c.claimed, e.file, e.line, e.claimed
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "bilingual pairs disagree about the current version:\n  {}\n\n\
         A translated document is not a separate product. When the two halves \
         name different versions, at least one of them is lying to whoever \
         reads only that language.",
        mismatches.join("\n  ")
    );
}

/// The scan must see something, or both assertions above are vacuous.
///
/// A ruler that silently matches nothing passes forever. This pins the scan to
/// facts that are true today and would break loudly if the traversal, the
/// extension filter or the marker list ever stopped working.
#[test]
fn the_version_scan_is_not_vacuous() {
    let files = documentation_files();
    assert!(
        files.len() > 25,
        "the documentation walk found only {} files, which is too few to be working",
        files.len()
    );

    let found = claims();
    assert!(
        !found.is_empty(),
        "no document states a current version at all — the marker list is stale \
         and this ruler is measuring nothing"
    );

    // The version extractor must handle the markup the documents actually use.
    assert_eq!(first_version("**v1.0.6**").as_deref(), Some("1.0.6"));
    assert_eq!(first_version(": `1.0.6` (agent)").as_deref(), Some("1.0.6"));
    assert_eq!(first_version("v1.0.6.").as_deref(), Some("1.0.6"));
    assert_eq!(first_version("no version here"), None);
    // A two-part number is not a release label and must not be captured.
    assert_eq!(first_version("glibc 2.17"), None);

    // The historical exclusions must name files that exist, or they are lies.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for f in HISTORICAL_BY_DESIGN {
        assert!(
            root.join(f).is_file(),
            "HISTORICAL_BY_DESIGN names {f}, which does not exist"
        );
    }
}
