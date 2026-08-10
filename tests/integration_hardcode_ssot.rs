// SPDX-License-Identifier: MIT OR Apache-2.0
//! Rulers for the hardcode prohibition, measured instead of asserted.
//!
//! # Why this file exists
//!
//! `src/endpoints.rs` opens by declaring itself the single source of truth for
//! product network identity, and it says call sites "must use these symbols
//! instead of raw string literals". That sentence had been true for years and
//! guarded by nothing, so five production call sites drifted back to raw
//! literals without any build failing:
//!
//! - `extraction/web.rs` matched `"duckduckgo.com/y.js"` twice by hand
//! - `extraction/web.rs` compared against a bare `"duckduckgo.com"`
//! - `types/selectors.rs` seeded the default ad filter with its own copy
//! - `zero_cause.rs` carried its own copy of the stealth-shell fragment
//!
//! Every one of those is a product fact with exactly one correct value. The
//! failure mode of a duplicated product fact is not a compile error, it is a
//! silent behaviour split: rename the path upstream and four filters stop
//! filtering while the fifth keeps working.
//!
//! A rule written in a doc comment is a wish. This is the measurement.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Repository root, resolved from the manifest rather than the cwd.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Production source files, excluding inline `#[cfg(test)]` bodies.
///
/// # Why test bodies are excluded rather than the files that hold them
///
/// This crate keeps unit tests inline, so excluding a whole file because it
/// contains tests would blind the ruler to that file's production code — which
/// is where two of the five offenders actually lived. Fixture HTML, mock cookie
/// payloads and assertion prose legitimately name the host, and all of them sit
/// inside `mod tests`.
fn production_lines(path: &Path) -> Vec<(usize, String)> {
    // A whole file can be a test module: this crate declares several as
    // `#[cfg(test)] mod tests;` in a sibling `tests.rs` or `*_tests.rs`.
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name == "tests.rs" || name.ends_with("_tests.rs") {
            return Vec::new();
        }
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut test_depth: Option<i32> = None;
    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if test_depth.is_none() && trimmed.starts_with("mod tests") {
            test_depth = Some(0);
        }
        if let Some(depth) = test_depth.as_mut() {
            *depth += i32::try_from(line.matches('{').count()).unwrap_or(0);
            *depth -= i32::try_from(line.matches('}').count()).unwrap_or(0);
            if *depth <= 0 && line.contains('}') {
                test_depth = None;
            }
            continue;
        }
        // Doc and line comments may name the host in prose.
        if trimmed.starts_with("//") {
            continue;
        }
        out.push((idx + 1, line.to_string()));
    }
    out
}

/// Whether a line uses the host as NETWORK IDENTITY rather than as prose.
///
/// # Why the distinction is the whole rule
///
/// Two operator messages name the host inside advice — "warm up on
/// duckduckgo.com first". Rewriting those to interpolate a constant would make
/// the sentence harder to read and would protect nothing, because nobody
/// matches a URL against a hint. What the SSOT governs is the value the code
/// COMPARES or CONSTRUCTS: a `contains`, a prefix test, or a literal URL. Those
/// are the ones that silently stop working when the upstream path changes.
///
/// Erring toward prose is deliberate. A false negative here costs one duplicate
/// sentence; a false positive teaches the next reader that the ruler cries wolf,
/// and a ruler nobody believes is worse than none.
fn is_network_identity_use(line: &str) -> bool {
    const MATCHERS: &[&str] = &[
        "contains(",
        "starts_with(",
        "ends_with(",
        "strip_prefix(",
        "https://",
        "http://",
    ];
    MATCHERS.iter().any(|m| line.contains(m))
}

/// Every `.rs` file under `src/`.
fn source_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![repo_root().join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Production code must reach DuckDuckGo hosts through `endpoints`, not literals.
#[test]
fn no_production_code_hardcodes_a_duckduckgo_host() {
    let root = repo_root();
    let endpoints = root.join("src/endpoints.rs");
    let mut offenders: BTreeSet<String> = BTreeSet::new();

    for path in source_files() {
        // The SSOT is allowed — and required — to hold the literals.
        if path == endpoints {
            continue;
        }
        for (lineno, line) in production_lines(&path) {
            if !line.contains("duckduckgo.com") || !is_network_identity_use(&line) {
                continue;
            }
            offenders.insert(format!(
                "{}:{lineno}: {}",
                path.strip_prefix(&root).unwrap_or(&path).display(),
                line.trim()
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "production code must name DuckDuckGo through `crate::endpoints`, whose \
         module header already says so. Each literal below is a second copy of a \
         product fact that has exactly one correct value:\n{}\n\n\
         Add the missing constant to src/endpoints.rs and use it here.",
        offenders.into_iter().collect::<Vec<_>>().join("\n")
    );
}

/// The ruler must be able to fail, or it is decoration.
#[test]
fn host_literal_detector_is_not_vacuous() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("sample.rs");
    std::fs::write(
        &file,
        concat!(
            "// duckduckgo.com in a comment is prose, not code\n",
            "fn live() { let _ = \"duckduckgo.com/y.js\"; }\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    const FIXTURE: &str = \"duckduckgo.com\";\n",
            "}\n",
        ),
    )
    .expect("write sample");

    let lines = production_lines(&file);
    let hits: Vec<&(usize, String)> = lines
        .iter()
        .filter(|(_, l)| l.contains("duckduckgo.com"))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "the detector must see the production literal and neither the comment \
         nor the fixture inside `mod tests`; it saw {hits:?}"
    );
    assert_eq!(hits[0].0, 2, "the offending line is the function body");
}

/// Every constant this ruler protects must actually be referenced somewhere.
///
/// A constant nobody uses is the same defect in the other direction: the SSOT
/// grows a symbol, the call sites keep their literals, and the module looks
/// authoritative while governing nothing.
#[test]
fn endpoint_host_constants_have_at_least_one_consumer() {
    let root = repo_root();
    let endpoints_src =
        std::fs::read_to_string(root.join("src/endpoints.rs")).expect("endpoints.rs is readable");

    let names: Vec<String> = endpoints_src
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("pub const ")?;
            rest.split(':').next().map(str::trim).map(str::to_string)
        })
        .collect();
    assert!(
        names.len() >= 8,
        "expected the endpoint SSOT to declare its host and URL constants, found {names:?}"
    );

    // A constant consumed only by this module's own accessor functions is
    // correctly placed, not orphaned: `html_base_url()` returning the default
    // IS the call site, and forcing an external one would push the literal back
    // out into the code this ruler exists to keep clean. So the search covers
    // every source file INCLUDING the SSOT, and the declaration line itself is
    // what gets discounted.
    let mut orphans = Vec::new();
    for name in &names {
        let mut uses = 0usize;
        for path in source_files() {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for line in text.lines() {
                if !line.contains(name.as_str()) {
                    continue;
                }
                // Skip the declaration itself; count every other mention.
                if line.trim().starts_with("pub const ") {
                    continue;
                }
                uses += 1;
            }
        }
        if uses == 0 {
            orphans.push(name.clone());
        }
    }
    assert!(
        orphans.is_empty(),
        "these endpoint constants are declared and consumed by nothing: {orphans:?}. \
         Either wire the call site that should use them, or delete them — an \
         unused entry in a single source of truth is a claim about code that \
         does not exist."
    );
}
