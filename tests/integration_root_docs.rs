// SPDX-License-Identifier: MIT OR Apache-2.0
//! Inventory guard for the documentation that ships in the repository root.
//!
//! # The class this closes
//!
//! `integration_docs_drift.rs` already confronts the binary with the READMEs,
//! but it confronts them about FLAGS only. Three things it never looked at
//! drifted at once, and drifted quietly:
//!
//! - `BENCHMARKS.md` and `NO_CI.md` shipped with no `.pt-BR` sibling, in a
//!   project whose stated rule is that no public document may omit its pair.
//!   Half the audience could not read either file.
//! - The `Commands` table in both READMEs was stamped `(v1.0.2)` while the
//!   binary shipped `1.0.5`. Three releases of contract change — the whole
//!   agent-native reduction surface — were invisible to anyone reading the
//!   documentation instead of the changelog.
//! - No ruler required a SUBCOMMAND to be documented at all. A flag added to
//!   `--help` failed the build; a whole subcommand did not.
//!
//! The shape of the failure is the one v1.0.5 was written to abolish: a guard
//! that names its targets cannot report the target nobody named. So this file
//! measures the inventory instead of listing it. A new root document lands in
//! exactly one bucket — translated, or exempt WITH A REASON — and anything
//! else fails the build.
//!
//! # Why the version rule is scoped and not universal
//!
//! Requiring every root document to name the shipped version would be a lie
//! dressed as rigour: `CODE_OF_CONDUCT.md` has no version, and demanding one
//! would train the next author to paste a number nobody maintains. The rule
//! applies to documents that DESCRIBE THE PRODUCT SURFACE, because those are
//! the ones a reader mistakes for current. Everything else must say why it is
//! out of scope, in code, next to the name.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[path = "common/language.rs"]
mod language;

/// Repository root, resolved from the manifest rather than the cwd.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Crate version reported by the binary under test.
fn live_version() -> String {
    let out = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
        .expect("compiled binary exists")
        .arg("--version")
        .output()
        .expect("version runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string()
}

/// The `commands` envelope, which is the published command surface.
fn commands_envelope() -> serde_json::Value {
    let out = assert_cmd::Command::cargo_bin("duckduckgo-search-cli")
        .expect("compiled binary exists")
        .args(["commands", "-q", "-f", "json"])
        .output()
        .expect("commands runs");
    serde_json::from_slice(&out.stdout).expect("commands emits a JSON envelope")
}

/// Every visible command path the binary publishes, qualified for nested ones.
///
/// Hidden commands are excluded on purpose: `buscar` is an undocumented alias
/// kept for compatibility, and requiring prose for something deliberately
/// absent from `--help` would push the documentation to advertise it.
fn live_command_paths() -> BTreeSet<String> {
    fn walk(node: &serde_json::Value, prefix: &str, out: &mut BTreeSet<String>) {
        let Some(subs) = node.get("subcommands").and_then(|s| s.as_array()) else {
            return;
        };
        for sub in subs {
            let Some(name) = sub.get("name").and_then(|n| n.as_str()) else {
                continue;
            };
            let hidden = sub.get("hidden").and_then(|h| h.as_bool()).unwrap_or(false);
            let path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix} {name}")
            };
            if !hidden {
                out.insert(path.clone());
            }
            walk(sub, &path, out);
        }
    }
    let mut out = BTreeSet::new();
    walk(&commands_envelope()["root"], "", &mut out);
    out
}

/// Root markdown that is PUBLISHED documentation for a reader of the product.
///
/// Two root files are markdown and are not that:
///
/// - `gaps.md` is a gitignored engineering log whose value is that past entries
///   are never rewritten.
/// - `CLAUDE.md` is instruction for an agent working ON this repository, not
///   description of the CLI. Translating it would serve nobody, and stamping it
///   with a release version would be meaningless.
///
/// Excluding them once here is honest. Exempting them rule by rule would spread
/// the same claim across three places and let those places disagree.
fn root_markdown() -> Vec<PathBuf> {
    const NOT_PRODUCT_DOCUMENTATION: &[&str] = &["gaps.md", "CLAUDE.md"];
    let mut out: Vec<PathBuf> = std::fs::read_dir(repo_root())
        .expect("root is readable")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "md"))
        .filter(|p| !NOT_PRODUCT_DOCUMENTATION.contains(&name_of(p).as_str()))
        .collect();
    out.sort();
    out
}

/// File name of a path, as a `String`, for readable assertion messages.
fn name_of(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Root documents that describe the product surface and must stay current.
///
/// A document lands here when a reader would reasonably take it as a statement
/// about the CLI as it ships today. `gaps.md` is deliberately absent: it is an
/// engineering record whose whole value is that past entries are NOT rewritten.
const SURFACE_DOCUMENTS: &[&str] = &[
    "README.md",
    "README.pt-BR.md",
    "CHANGELOG.md",
    "CHANGELOG.pt-BR.md",
    "INTEGRATIONS.md",
    "INTEGRATIONS.pt-BR.md",
];

/// Root documents exempt from the version rule, each with the reason.
///
/// An entry here is a claim that the document says nothing a reader could
/// mistake for a statement about the current release. Adding a name without a
/// reason is not possible: the tuple requires one.
const VERSION_EXEMPT: &[(&str, &str)] = &[
    (
        "CODE_OF_CONDUCT.md",
        "Contributor Covenant 2.1 text; versions the covenant, not the CLI",
    ),
    ("CODE_OF_CONDUCT.pt-BR.md", "mirror of the covenant text"),
    (
        "CONTRIBUTING.md",
        "describes the contribution workflow, which is release-independent",
    ),
    (
        "CONTRIBUTING.pt-BR.md",
        "mirror of the contribution workflow",
    ),
    (
        "SECURITY.md",
        "supported-version policy is asserted by its own table, not by prose",
    ),
    ("SECURITY.pt-BR.md", "mirror of the security policy"),
    (
        "INVERSIONS.md",
        "records design inversions by date, not by shipped version",
    ),
    ("INVERSIONS.pt-BR.md", "mirror of the inversions record"),
    (
        "NO_CI.md",
        "states why no CI exists; the answer does not move with releases",
    ),
    ("NO_CI.pt-BR.md", "mirror of the no-CI rationale"),
    (
        "BENCHMARKS.md",
        "reports measurements stamped with the version they were taken on",
    ),
    ("BENCHMARKS.pt-BR.md", "mirror of the benchmark report"),
];

/// Every public root document must ship with its Portuguese counterpart.
///
/// The two files that broke this rule did so for months without any guard
/// noticing, because every guard in the repository named the documents it
/// checked. This one enumerates the directory instead.
#[test]
fn every_root_document_has_a_translation() {
    let mut missing = Vec::new();
    for path in root_markdown() {
        let name = name_of(&path);
        if name.contains(".pt-BR.") {
            let english = name.replace(".pt-BR.", ".");
            if !repo_root().join(&english).exists() {
                missing.push(format!("{name} has no English original ({english})"));
            }
            continue;
        }
        let translated = name.replace(".md", ".pt-BR.md");
        if !repo_root().join(&translated).exists() {
            missing.push(format!("{name} has no translation ({translated})"));
        }
    }
    assert!(
        missing.is_empty(),
        "root documents missing their pair:\n  {}\n\n\
         The project rule is that no public document omits its `.pt-BR` sibling. \
         Either write the translation or, if the file is not public documentation, \
         say so here with a reason.",
        missing.join("\n  ")
    );
}

/// Documents under `docs/` that carry both languages inside a single file.
///
/// Each entry is a claim that the file serves a Portuguese reader without a
/// `.pt-BR` sibling, and the reason has to be written down.
const DOCS_SINGLE_FILE_BILINGUAL: &[(&str, &str)] = &[
    (
        "AGENT_RULES.md",
        "every rule is stated twice inside the file, EN line then PT line",
    ),
    (
        "PROMPT_RULES_ANTI_CLOUDFLARE.pt-BR.md",
        "a pt-BR prompt to paste into an agent; an English copy would be a different artifact, not a translation",
    ),
];

/// Every public document under `docs/` must ship with its Portuguese pair.
///
/// # The scope this widens
///
/// [`every_root_document_has_a_translation`] reads ONE directory. It closed the
/// class for the repository root and left twenty documents under `docs/`
/// unmeasured — the same shape as the guard it was written to replace. The
/// 2026-08-10 audit found the consequence was small (two unpaired files, both
/// defensible) and the exposure was not: nothing would have said so.
///
/// `docs/decisions/` and `docs/schemas/` are deliberately outside. An ADR is an
/// engineering record written once and never rewritten, and the schema index is
/// bilingual in one file. Neither is prose a reader picks a language for.
#[test]
fn every_docs_document_has_a_translation_or_a_declared_reason() {
    let exempt: BTreeSet<&str> = DOCS_SINGLE_FILE_BILINGUAL
        .iter()
        .map(|(name, _)| *name)
        .collect();

    let dir = repo_root().join("docs");
    let mut present: Vec<String> = std::fs::read_dir(&dir)
        .expect("docs is readable")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "md"))
        .map(|p| name_of(&p))
        .collect();
    present.sort();
    assert!(
        present.len() >= 10,
        "docs/ yielded only {} markdown files, so this ruler measured almost \
         nothing — the directory moved or the filter broke",
        present.len()
    );

    let mut missing = Vec::new();
    for name in &present {
        if exempt.contains(name.as_str()) {
            continue;
        }
        if name.contains(".pt-BR.") {
            let english = name.replace(".pt-BR.", ".");
            if !dir.join(&english).exists() {
                missing.push(format!("{name} has no English original ({english})"));
            }
            continue;
        }
        let translated = name.replace(".md", ".pt-BR.md");
        if !dir.join(&translated).exists() {
            missing.push(format!("{name} has no translation ({translated})"));
        }
    }
    assert!(
        missing.is_empty(),
        "documents under docs/ missing their pair:\n  {}\n\n\
         Write the translation, or add the file to DOCS_SINGLE_FILE_BILINGUAL \
         with the reason one file serves both readers.",
        missing.join("\n  ")
    );

    let stale: Vec<&&str> = DOCS_SINGLE_FILE_BILINGUAL
        .iter()
        .map(|(name, _)| name)
        .filter(|name| !present.contains(&(*name).to_string()))
        .collect();
    assert!(
        stale.is_empty(),
        "these docs/ exemptions name files that no longer exist: {stale:?}"
    );
}

/// Documents that must name every command the binary publishes.
///
/// `llms.txt` is deliberately absent. Its contract is to stay SHORT — it is the
/// discovery stub, and `llms-full.txt` is the expanded artifact. Forcing the
/// full command tree into the stub would pit one rule against another, and the
/// rule that loses would be the one nobody wrote a test for.
const COMMAND_REFERENCES: &[&str] = &[
    "README.md",
    "README.pt-BR.md",
    "llms-full.txt",
    "llms.pt-BR.txt",
    "docs/AGENTS.md",
    "docs/AGENTS.pt-BR.md",
];

/// Every visible subcommand must be documented in each command reference.
///
/// The flag ruler in `integration_docs_drift.rs` covers `--flags` only, which
/// is why a `Commands` table could sit three releases out of date while every
/// documentation test passed. A flag added to `--help` failed the build; a
/// whole subcommand did not.
#[test]
fn every_live_subcommand_is_documented() {
    let commands = live_command_paths();
    assert!(
        commands.len() >= 9,
        "expected the published command tree to be non-trivial, got {commands:?}"
    );

    for readme in COMMAND_REFERENCES {
        let text = std::fs::read_to_string(repo_root().join(readme))
            .unwrap_or_else(|e| panic!("{readme} is readable: {e}"));
        let undocumented: Vec<&String> = commands.iter().filter(|c| !text.contains(*c)).collect();
        assert!(
            undocumented.is_empty(),
            "{readme} does not document these subcommands: {undocumented:?}\n\n\
             Every command the binary publishes must appear in both READMEs. \
             Run `duckduckgo-search-cli commands` to see the live tree."
        );
    }
}

/// Documents that describe the product surface must name the shipped version.
#[test]
fn surface_documentation_names_the_shipped_version() {
    let version = live_version();
    assert!(!version.is_empty(), "--version reported nothing");

    let mut stale = Vec::new();
    for name in SURFACE_DOCUMENTS {
        let text = std::fs::read_to_string(repo_root().join(name))
            .unwrap_or_else(|e| panic!("{name} is readable: {e}"));
        if !text.contains(&version) {
            stale.push(*name);
        }
    }
    assert!(
        stale.is_empty(),
        "these documents describe the product surface but never mention {version}: {stale:?}\n\n\
         A reader takes them as current. If one of them genuinely has nothing to say \
         about this release, move it to VERSION_EXEMPT with the reason."
    );
}

/// A document that states a current version must state THIS one.
///
/// # Why this ruler is phrased as a claim check and not as a stamp check
///
/// [`surface_documentation_names_the_shipped_version`] governs six named files
/// in the repository root. Everything under `docs/` was outside it, because
/// `root_markdown` reads one directory and does not recurse. The 2026-08-10
/// audit measured the consequence: `docs/HOW_TO_USE.md` and its pt-BR mirror
/// opened with `**Current version: 1.0.3**` while the crate shipped 1.0.5.
///
/// The obvious repair — add every file under `docs/` to `SURFACE_DOCUMENTS` —
/// would demand a version mention in twenty documents, and most of them have
/// nothing to say about a given release. Forcing a stamp there buys a green
/// gate with decorative edits, which is how a stamp stops meaning anything.
///
/// So this measures the assertion instead of the presence. A document is free
/// to never mention a version. The moment it writes "Current version: X" it has
/// made a claim a reader will act on, and the claim must be true.
#[test]
fn no_document_claims_a_current_version_that_is_not_shipped() {
    /// Openers that assert "this is the release you are reading about".
    ///
    /// `Version:` and `Versão:` joined in v1.0.5. The list had two entries and
    /// three documents made the claim: `docs/AGENTS.md`, its mirror, and
    /// `docs/AGENT_RULES.md` all opened with `Version: **v1.0.3**` while the
    /// crate shipped 1.0.5, and the ruler read past them because they had
    /// invented a third wording. A closed list of openers is exactly the shape
    /// this file keeps finding: the guard measures what it remembered to name.
    const CLAIM_PREFIXES: &[&str] = &["Current version:", "Versão atual:", "Version:", "Versão:"];

    let version = live_version();
    assert!(!version.is_empty(), "--version reported nothing");

    let mut markdown: Vec<PathBuf> = root_markdown();
    if let Ok(entries) = std::fs::read_dir(repo_root().join("docs")) {
        markdown.extend(
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "md")),
        );
    }

    let mut wrong = Vec::new();
    let mut claims = 0usize;
    for path in &markdown {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for (idx, line) in text.lines().enumerate() {
            let stripped = line.trim_start_matches(['*', '#', ' ', '-']);
            let Some(prefix) = CLAIM_PREFIXES.iter().find(|p| stripped.starts_with(**p)) else {
                continue;
            };
            claims += 1;
            let claimed = stripped[prefix.len()..]
                .trim_start_matches(['*', ' '])
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_end_matches('*');
            // `Version: **v1.0.5**` and `Current version: 1.0.5` are the same
            // claim; `--version` reports the number without the `v`.
            let claimed = claimed.strip_prefix('v').unwrap_or(claimed);
            if claimed != version {
                wrong.push(format!(
                    "{}:{}: claims {claimed:?}, binary ships {version:?}",
                    name_of(path),
                    idx + 1
                ));
            }
        }
    }

    // Non-vacuity: if nobody makes the claim, the loop above proves nothing and
    // a future rename of the opener would retire this gate in silence.
    assert!(
        claims > 0,
        "no document states a current version, so this ruler measured nothing. \
         Either the opener was reworded — update CLAIM_PREFIXES — or the claim \
         was dropped everywhere, and then this test should be deleted rather \
         than left passing on an empty set."
    );
    assert!(
        wrong.is_empty(),
        "these documents state a current version that is not the one this \
         binary ships:\n{}\n\n\
         A stale version line is worse than no version line: the reader stops \
         checking. Update the claim or remove it.",
        wrong.join("\n")
    );
}

/// The two buckets must together cover every root document.
///
/// This is what makes the pair above a ruler rather than two lists: a new root
/// document cannot be silently ungoverned, because it belongs to neither bucket
/// and the build says so.
#[test]
fn the_version_rule_covers_every_root_document() {
    let governed: BTreeSet<&str> = SURFACE_DOCUMENTS
        .iter()
        .copied()
        .chain(VERSION_EXEMPT.iter().map(|(name, _)| *name))
        .collect();

    let present: BTreeSet<String> = root_markdown().iter().map(|p| name_of(p)).collect();

    let ungoverned: Vec<&String> = present
        .iter()
        .filter(|n| !governed.contains(n.as_str()))
        .collect();
    assert!(
        ungoverned.is_empty(),
        "these root documents belong to neither bucket: {ungoverned:?}\n\n\
         Add each to SURFACE_DOCUMENTS if a reader would take it as current, \
         or to VERSION_EXEMPT with the reason it is not."
    );

    let stale: Vec<&&str> = governed.iter().filter(|n| !present.contains(**n)).collect();
    assert!(
        stale.is_empty(),
        "these names are governed but no longer exist: {stale:?}\n\n\
         A rule about a deleted file is decoration. Remove the entry."
    );
}

/// Every relative link in root documentation must resolve to a real file.
///
/// The `llms.txt` contract forbids linking documentation that does not exist,
/// and a broken link in a README is the same defect with a different audience.
#[test]
fn every_relative_link_in_root_documentation_resolves() {
    let mut broken = Vec::new();
    let mut checked = 0usize;

    let mut files: Vec<PathBuf> = root_markdown();
    for txt in ["llms.txt", "llms-full.txt", "llms.pt-BR.txt"] {
        let path = repo_root().join(txt);
        if path.exists() {
            files.push(path);
        }
    }

    for path in &files {
        let name = name_of(path);
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for target in markdown_link_targets(&text) {
            if target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
                || target.starts_with('#')
                || target.is_empty()
            {
                continue;
            }
            // Strip any in-page anchor before resolving the file itself.
            let file_part = target.split('#').next().unwrap_or(&target);
            if file_part.is_empty() {
                continue;
            }
            checked += 1;
            if !repo_root().join(file_part).exists() {
                broken.push(format!("{name} -> {target}"));
            }
        }
    }

    assert!(
        checked > 20,
        "link extractor found only {checked} links; it is probably broken"
    );
    assert!(
        broken.is_empty(),
        "root documentation links to files that do not exist:\n  {}",
        broken.join("\n  ")
    );
}

/// `text` with fenced blocks and inline code spans blanked out.
///
/// Without this the extractor reports `](URL)` from the line that DOCUMENTS the
/// markdown output format: `` `- [Title](URL)\n  > snippet` ``. That is prose
/// about a link, not a link. Four such false positives appeared on the first
/// run, and the tempting fix was to exempt the four files — which would have
/// silenced the guard on those files for every real break afterwards.
fn strip_code(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            out.push('\n');
            continue;
        }
        if in_fence {
            out.push('\n');
            continue;
        }
        let mut in_span = false;
        for c in line.chars() {
            if c == '`' {
                in_span = !in_span;
                out.push(' ');
            } else {
                out.push(if in_span { ' ' } else { c });
            }
        }
        out.push('\n');
    }
    out
}

/// Link targets of `[text](target)` occurrences, in source order.
fn markdown_link_targets(text: &str) -> Vec<String> {
    let text = strip_code(text);
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ']' && i + 1 < chars.len() && chars[i + 1] == '(' {
            let mut j = i + 2;
            let mut target = String::new();
            let mut depth = 1;
            while j < chars.len() {
                match chars[j] {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                target.push(chars[j]);
                j += 1;
            }
            // A newline inside means this was not a link; skip it.
            if !target.contains('\n') {
                // Markdown allows a title after the target: `(path "title")`.
                let target = target.split_whitespace().next().unwrap_or("").to_string();
                out.push(target);
            }
            i = j + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// The link extractor must actually find links, or the guard is decoration.
#[test]
fn the_link_extractor_is_not_vacuous() {
    let found =
        markdown_link_targets("see [a](README.md) and [b](https://x.test) and [c](#anchor)");
    assert_eq!(found, vec!["README.md", "https://x.test", "#anchor"]);
    assert!(markdown_link_targets("no links here at all").is_empty());

    // Prose ABOUT a link is not a link.
    assert!(
        markdown_link_targets("- `markdown`: `- [Title](URL)\\n  > snippet`.").is_empty(),
        "an inline code span must not yield a link target"
    );
    assert!(
        markdown_link_targets("```\n[a](nope.md)\n```\n[b](real.md)") == vec!["real.md"],
        "a fenced block must not yield a link target"
    );
}

/// Whether a line of prose reads as Portuguese.
///
/// # Why the word list moved out of this file
///
/// This file used to declare its own `PORTUGUESE_MARKERS` — seventeen entries,
/// every one carrying a diacritic — while `integration_docs_drift.rs` declared
/// a different table of thirty-two plain function words. Two tables was the
/// right call: markdown prose and Rust comments really are different corpora,
/// and a marker like `que` that is safe inside code would fire on any English
/// sentence quoting Portuguese in a document.
///
/// What was NOT right is that each file also wrote its own matcher, and the two
/// disagreed. One split the line on non-alphabetic characters and compared
/// whole words, which silently cannot match a marker containing a space —
/// `"sobre o"`, `"para o"` and `"com o"` were dead entries in this table's
/// sibling. The shared matcher in [`language::contains_marker`] is the
/// substring-with-edges version, which handles both shapes.
///
/// Wire keys and identifiers need no exclusion list here: [`strip_code`] blanks
/// out backticked spans first, and this project writes identifiers in backticks.
fn looks_portuguese(line: &str) -> bool {
    language::looks_portuguese(line, language::PROSE_MARKERS)
}

/// English root documents must be written in English.
///
/// # The class this closes
///
/// `integration_docs_drift.rs` already proves that COMMENTS in `src/`, `tests/`
/// and `benches/` are English. Nothing made the same demand of the published
/// documents, and the leak was larger there: whole historical `CHANGELOG.md`
/// entries — the canonical English file — were written in Portuguese, and both
/// `CONTRIBUTING.md` and `BENCHMARKS.md` carried Portuguese descriptions.
///
/// The mirror direction is not checked. A Portuguese document quoting an
/// English sentence is normal; the canonical file being half-translated is not.
#[test]
fn english_root_documents_are_written_in_english() {
    let mut offenders = Vec::new();
    for path in root_markdown() {
        let name = name_of(&path);
        if name.contains(".pt-BR.") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (n, line) in strip_code(&text).lines().enumerate() {
            if looks_portuguese(line) {
                offenders.push(format!("{name}:{}", n + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "Portuguese prose in {} line(s) of English documentation:\n  {}\n\n\
         English is the canonical language of the public documents. \
         The Portuguese text belongs in the `.pt-BR.md` sibling.",
        offenders.len(),
        offenders.join("\n  ")
    );
}

/// The Portuguese detector must fire on Portuguese and stay quiet on English.
#[test]
fn the_portuguese_detector_is_not_vacuous() {
    assert!(looks_portuguese(
        "Versão do UA Chrome alinhada à versão real instalada"
    ));
    assert!(looks_portuguese("WebRTC não vaza IP real"));
    assert!(looks_portuguese("versão em português"));
    assert!(!looks_portuguese(
        "Every flag the binary advertises must be documented."
    ));
    assert!(!looks_portuguese(
        "The projector reduces the envelope before serialization."
    ));
    // A marker must not fire when it is merely a substring of an English word,
    // which is why the detector requires a non-alphanumeric edge on both sides.
    assert!(!looks_portuguese("for that reason the season ended"));
}

/// How far into a document the cross-language link may appear.
///
/// The rule says "first useful line", which in practice means after a badge
/// cluster and a tagline. Fifteen lines is generous enough to allow that and
/// tight enough that a link buried in the footer does not satisfy it: a reader
/// who cannot see it above the fold cannot use it.
const CROSS_LINK_WINDOW: usize = 15;

/// Every published root document links its counterpart near the top.
///
/// Three documents failed this silently. `INTEGRATIONS.md` and its mirror
/// pointed at each other nowhere at all, so a Portuguese reader landing on the
/// English integration catalogue had no way to discover the translation that
/// existed all along.
#[test]
fn every_root_document_links_its_counterpart() {
    let mut missing = Vec::new();
    for path in root_markdown() {
        let name = name_of(&path);
        let counterpart = if name.contains(".pt-BR.") {
            name.replace(".pt-BR.", ".")
        } else {
            name.replace(".md", ".pt-BR.md")
        };
        if !repo_root().join(&counterpart).exists() {
            continue; // the translation rule above already reports this
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let head: String = text
            .lines()
            .take(CROSS_LINK_WINDOW)
            .collect::<Vec<_>>()
            .join("\n");
        if !head.contains(&counterpart) {
            missing.push(format!(
                "{name} does not link {counterpart} in its first {CROSS_LINK_WINDOW} lines"
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "cross-language links missing:\n  {}\n\n\
         Every public document opens with a link to the opposite language, so a \
         reader who lands on the wrong one can leave immediately.",
        missing.join("\n  ")
    );
}

/// Heading levels present in a document, ignoring fenced code.
///
/// Bash comments inside a fence start with `#` and would otherwise be counted
/// as H1 headings — a trap that made the first hand measurement of these files
/// report five H1s in a README that has none.
fn heading_levels(text: &str) -> Vec<usize> {
    strip_code(text)
        .lines()
        .filter_map(|line| {
            let hashes = line.chars().take_while(|c| *c == '#').count();
            let rest = &line[hashes..];
            (hashes > 0 && rest.starts_with(' ')).then_some(hashes)
        })
        .collect()
}

/// Every published root document opens with exactly one H1.
///
/// `README.md` had NONE. It began with a badge cluster and went straight to an
/// `## English` wrapper left over from when the file carried both languages, so
/// GitHub, crates.io and docs.rs all rendered the project's landing page with
/// no title and the primary keyword absent from the document outline. The
/// Portuguese mirror had its H1 all along, which is exactly why a rule checked
/// per language and never across them missed it.
#[test]
fn every_root_document_has_exactly_one_h1() {
    let mut offenders = Vec::new();
    for path in root_markdown() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let count = heading_levels(&text).iter().filter(|l| **l == 1).count();
        if count != 1 {
            offenders.push(format!("{}: {count} H1", name_of(&path)));
        }
    }
    assert!(
        offenders.is_empty(),
        "documents without exactly one top-level heading:\n  {}\n\n\
         A document with no H1 renders untitled everywhere it is published; \
         one with several has no single subject.",
        offenders.join("\n  ")
    );
}

/// The heading parser must not count shell comments inside fences.
#[test]
fn the_heading_parser_ignores_fenced_code() {
    assert_eq!(heading_levels("# Title\n## Section"), vec![1, 2]);
    assert_eq!(
        heading_levels("# Title\n```bash\n# a shell comment\n```\n## Section"),
        vec![1, 2],
        "a comment inside a fence must not read as a heading"
    );
    // `#hashtag` is not a heading; ATX headings require a space.
    assert!(heading_levels("#nothing").is_empty());
}

/// Root documentation carries no emoji.
///
/// The project's documentation rules forbid them outright. One survived in both
/// READMEs (`### 📚 Documentation`) precisely because no ruler looked.
#[test]
fn root_documentation_has_no_emoji() {
    let mut offenders = Vec::new();
    for path in root_markdown() {
        let name = name_of(&path);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (n, line) in text.lines().enumerate() {
            if let Some(ch) = line.chars().find(|c| is_emoji(*c)) {
                offenders.push(format!("{name}:{} contains {ch:?}", n + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "emoji in root documentation:\n  {}",
        offenders.join("\n  ")
    );
}

/// Whether a character sits in one of the pictographic Unicode blocks.
///
/// Deliberately narrow. Accented Latin, box drawing and typographic dashes are
/// all legitimate here, and a greedy matcher would flag Portuguese prose.
fn is_emoji(c: char) -> bool {
    matches!(c as u32,
        0x1F300..=0x1FAFF   // pictographs, emoticons, symbols, supplements
        | 0x1F000..=0x1F2FF // tiles, enclosed alphanumeric supplement
        | 0x2600..=0x27BF   // misc symbols and dingbats
        | 0xFE0F            // variation selector-16
        | 0x2B00..=0x2BFF   // misc symbols and arrows
    )
}

/// The emoji detector must fire on emoji and stay quiet on Portuguese.
#[test]
fn the_emoji_detector_is_not_vacuous() {
    assert!("📚".chars().any(is_emoji), "detector missed a pictograph");
    assert!("✅".chars().any(is_emoji), "detector missed a dingbat");
    assert!(
        !"configuração não é decoração — três hífens"
            .chars()
            .any(is_emoji),
        "detector fired on Portuguese prose"
    );
    assert!(!"README.md `--fields`".chars().any(is_emoji));
}

/// Every root document that ships must ship with its Portuguese pair.
///
/// # The class this closes
///
/// `every_root_document_has_a_translation` enumerates the DIRECTORY, which is
/// the right question for a contributor and the wrong one for a reader. The
/// reader gets the tarball, and `Cargo.toml` `include` is an explicit allowlist:
/// a file absent from it is silently absent from the artifact, with no warning
/// from `cargo package`.
///
/// Measured before this ruler existed: `BENCHMARKS.pt-BR.md` and
/// `NO_CI.pt-BR.md` were in the tree, were green under the directory rule, and
/// were NOT in the published crate. A Portuguese reader installing from
/// crates.io received English-only for both.
///
/// This is the same family the repository keeps meeting — a guard measuring
/// something adjacent to the thing that matters — one step further out: the
/// repository is not the artifact.
#[test]
fn published_documentation_ships_every_translation() {
    let out = std::process::Command::new(env!("CARGO"))
        .args(["package", "--list", "--allow-dirty"])
        .current_dir(repo_root())
        .output()
        .expect("cargo package --list runs");
    assert!(
        out.status.success(),
        "cargo package --list failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let listed: BTreeSet<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();

    // Root markdown only: a nested path has its own directory conventions.
    let root_docs: Vec<&String> = listed
        .iter()
        .filter(|p| !p.contains('/') && p.ends_with(".md"))
        .collect();
    assert!(
        root_docs.len() >= 10,
        "expected the packaged root documentation to be non-trivial, got {root_docs:?}"
    );

    let mut missing = Vec::new();
    for doc in &root_docs {
        if doc.contains(".pt-BR.") {
            continue;
        }
        let translated = doc.replace(".md", ".pt-BR.md");
        if !listed.contains(&translated) {
            missing.push(format!(
                "{doc} is published without {translated}, which exists in the tree: \
                 add it to `include` in Cargo.toml"
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "the published crate ships documentation without its translation:\n  {}\n\n\
         `include` is an allowlist and `cargo package` does not warn about a file \
         you forgot to list. The tree passing is not the artifact passing.",
        missing.join("\n  ")
    );
}
