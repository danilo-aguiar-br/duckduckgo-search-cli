// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(dead_code)]
//! Single source of truth for the "this project is written in English" ruler.
//!
//! # Why one module and not two tables
//!
//! Two files carried their own copy of `PORTUGUESE_MARKERS` with two DIFFERENT
//! matchers: `integration_docs_drift.rs` split on non-alphabetic characters and
//! compared whole words, `integration_root_docs.rs` searched substrings and
//! checked the edges by hand. Neither was wrong for its own corpus — code
//! comments and markdown prose really do need different word lists — but
//! nothing said so, and nothing stopped the two from drifting further.
//!
//! Here the two WORD LISTS stay separate, because the corpora are, and the
//! MATCHER is shared, because the question is the same.
//!
//! # The three axes a language ruler has to cover
//!
//! The previous ruler measured one of them and its failure message claimed all
//! three. `looks_portuguese` returned `false` for any line that did not begin
//! with `//`, so it could only ever see comments. Measured on v1.0.5:
//!
//! - comments: zero survivors — the ruler had genuinely cleared its own scope
//! - string literals: 307 lines, one character outside that scope
//! - Rust identifiers: around twenty, including `precisa_sandbox_off`
//!
//! There is a fourth that no word list can catch, because a mechanical sweep
//! replaced Portuguese words with English ones inside Portuguese sentences and
//! left hybrids behind — `"must NOTria ser data relativa"`, `"deve failurer"`,
//! `"seletor invalid deve cair for o default"`. Those need their own detector.
//!
//! # The boundary this module draws
//!
//! A string that is an ARGUMENT to an assertion or a log macro is PROSE, and
//! prose is English. Any other string is DATA, and this CLI searches in
//! Brazilian Portuguese, so its fixtures must be free to be Portuguese. That
//! line is what lets the ruler stay sharp inside the very files where the
//! defects live, instead of exempting them wholesale.

use std::path::Path;

/// Portuguese function words for CODE corpora (comments, assertion prose).
///
/// Deliberately excludes the Portuguese WIRE KEYS (`resultados`, `metadados`,
/// `tipo`, `noticias`, `fonte`, `erro`, `mensagem`, …). Those are identifiers
/// of the `--wire-keys pt` contract, and code must be free to name them — a
/// comment that says "the value `error` becomes `erro` under PT wire" is
/// English prose doing its job.
///
/// A first draft of this table added `erro` and `vazio`, and the ruler
/// immediately reported seven comments that were correctly explaining the PT
/// wire contract. That is the same mistake in the other direction: a marker
/// list that names an identifier stops measuring language and starts measuring
/// vocabulary.
pub const CODE_MARKERS: &[&str] = &[
    "não", "nao", "que", "para", "quando", "porque", "deve", "devem", "também", "então", "nunca",
    "apenas", "somente", "garante", "retorna", "falha", "busca", "pela", "pelo", "isso", "ainda",
    "depois", "onde", "seja", "são", "uma", "cada", "mesmo", "arquivo", "janela", "visivel",
    "chave", "linha", "esperado", "deveria",
];

/// Portuguese markers for MARKDOWN prose corpora.
///
/// Every one carries a diacritic or is unambiguous, so the detector cannot fire
/// on an English sentence.
pub const PROSE_MARKERS: &[&str] = &[
    "não",
    "versão",
    "português",
    "configuração",
    "conteúdo",
    "também",
    "após",
    "própria",
    "está",
    "são",
    "você",
    "aqui",
    "porque",
    "quando",
    "sobre o",
    "para o",
    "com o",
];

/// Word stems that mark a Rust identifier as Portuguese.
///
/// Only stems with no English homograph, so `total_attempts` and
/// `pagination_headers` cannot trip it.
///
/// # Stems deliberately left out
///
/// `sem` is Portuguese for "without" and would have caught one survivor, but
/// this repository already spells `semaphore` as `sem_task` and `f_sem`, so the
/// stem would report English as Portuguese. `com` and `no` lose the same way.
/// A stem that fires on correct code trains the next reader to skip the gate,
/// which costs more than the one identifier it would have caught — and that
/// survivor is reached anyway through `campos` and `desserializa`.
pub const IDENTIFIER_STEMS: &[&str] = &[
    "precisa",
    "arquivo",
    "arquivos",
    "caminho",
    "conteudo",
    "cliente",
    "saida",
    "tamanho",
    "metodo",
    "extracao",
    "mensagem",
    "consulta",
    "tentativa",
    "quantidade",
    "resultados",
    "noticias",
    "unicos",
    "vazio",
    // Added 2026-08-10, from identifiers the ruler had let through.
    "binario",
    "emite",
    "campos",
    "sintese",
    "desserializa",
    "aditivos",
    "formatos",
    "tres",
    "para",
];

/// Fragments left behind by a word-for-word translation sweep.
///
/// # Why a word list cannot find these
///
/// Each one is an English word glued to Portuguese morphology, or an English
/// word dropped into a Portuguese clause. `"deve failurer"` contains `deve`,
/// which IS in [`CODE_MARKERS`] — but `"must NOTria ser data relativa"` does
/// not contain a single entry from any list, because the sweep replaced every
/// word the list knew. The residue is the grammar, not the vocabulary.
pub const CORRUPTED_FRAGMENTS: &[&str] = &[
    "failurer",
    "notria",
    "must not ser",
    "must not criar",
    "must not sobrescrever",
    "must not escrever",
    "must not casar",
    "must not aparecer",
    "must not estar",
    "must not voltar",
    "must not perdida",
    "invalid deve",
    "deve failurer",
    "without erro",
    "deve cair for",
    "not captura",
    "results legitimates",
];

/// Whether `text` contains `marker` as a standalone word.
///
/// The two previous copies of this logic differed: one split the line on
/// non-alphabetic characters, the other matched substrings and inspected the
/// edges. They agree on every real case and disagree on markers that contain a
/// space, which only the second handled. This is the second behaviour, which is
/// the strictly more capable one.
#[must_use]
pub fn contains_marker(text: &str, marker: &str) -> bool {
    let lower = text.to_lowercase();
    lower.match_indices(marker).any(|(i, _)| {
        let before = lower[..i].chars().next_back();
        let after = lower[i + marker.len()..].chars().next();
        let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
        edge(before) && edge(after)
    })
}

/// Whether `text` reads as Portuguese against `markers`.
#[must_use]
pub fn looks_portuguese(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|m| contains_marker(text, m))
}

/// Whether `text` carries a diacritic only Portuguese uses in this project.
///
/// English prose in this repository is plain ASCII. Spanish or French would
/// also trip this, and that is fine: neither belongs in the source either.
#[must_use]
pub fn has_portuguese_diacritic(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c,
            'ã' | 'õ' | 'ç' | 'á' | 'é' | 'í' | 'ó' | 'ú' | 'â' | 'ê' | 'ô' | 'à'
        )
    })
}

/// The corrupted fragment `text` contains, if any.
///
/// Matching is word-bounded on the RIGHT edge, because a plain `contains` made
/// `"must not ser"` fire on the perfectly good English `"must not serialize PT
/// resultados"`. A detector for broken translations that flags correct English
/// teaches its readers to ignore it.
#[must_use]
pub fn corrupted_fragment(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    CORRUPTED_FRAGMENTS
        .iter()
        .find(|f| {
            lower.match_indices(**f).any(|(i, _)| {
                let after = lower[i + f.len()..].chars().next();
                after.is_none_or(|c| !c.is_alphanumeric())
            })
        })
        .copied()
}

/// Calls whose arguments include prose, and where that prose STARTS.
///
/// # Why the position matters more than the call
///
/// The first version of this table listed the calls and treated every literal
/// inside one as prose. That over-reached, and the over-reach was not a rounding
/// error: `assert_eq!(parse("há 2 horas"), Some(dur))` compares a pt-BR relative
/// date, which is the exact input this CLI is built to parse. Fifty-six such
/// fixtures were reported as if a maintainer had left Portuguese behind.
///
/// The distinction is positional and exact. `assert_eq!` and `assert_ne!` take
/// two VALUES and only then a message; `assert!` takes one condition; the
/// message-only calls start at zero. Values are data and this project's data is
/// Portuguese on purpose.
///
/// `expect` is a function rather than a macro, but its single argument has the
/// same audience and the same rule.
const PROSE_CALLS: &[(&str, usize)] = &[
    ("assert_eq!", 2),
    ("assert_ne!", 2),
    ("assert!", 1),
    ("debug_assert!", 1),
    ("panic!", 0),
    ("unreachable!", 0),
    ("todo!", 0),
    ("expect(", 0),
    ("expect_err(", 0),
    ("tracing::info!", 0),
    ("tracing::warn!", 0),
    ("tracing::error!", 0),
    ("tracing::debug!", 0),
    ("tracing::trace!", 0),
    ("info!(", 0),
    ("warn!(", 0),
    ("error!(", 0),
    ("debug!(", 0),
    ("trace!(", 0),
];

/// A string literal found in prose position, with its 1-based line number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProseLiteral {
    /// 1-based line where the literal appears.
    pub line: usize,
    /// The literal's contents, without the surrounding quotes.
    pub text: String,
}

/// Every string literal that sits in the MESSAGE position of such a call.
///
/// # Why a paren-depth scanner and not a parser
///
/// A real parse would need `syn` as a dev-dependency and a full expansion of
/// `macro_rules!` bodies to be exact. The question here is coarser: is this
/// literal in the argument slot of a call whose audience is a human. Tracking
/// paren depth and counting top-level commas answers that for every shape this
/// repository actually writes, including the common one where the message sits
/// alone on its own line.
///
/// Nested calls are handled by the depth counter: a literal inside
/// `parse("há 2 horas")` sits at depth 2 of the enclosing `assert_eq!`, so it
/// belongs to the VALUE argument and never reaches the message slot.
#[must_use]
pub fn prose_literals(source: &str) -> Vec<ProseLiteral> {
    let mut out = Vec::new();
    let mut state: Option<(usize, i32, usize)> = None; // (skip_args, depth, commas)
    for (idx, raw) in source.lines().enumerate() {
        let line = strip_line_comment(raw);
        if state.is_none() {
            if let Some(skip) = PROSE_CALLS
                .iter()
                .filter(|(call, _)| line.contains(call))
                .map(|(_, skip)| *skip)
                .min()
            {
                state = Some((skip, 0, 0));
            }
        }
        let Some((skip, depth, commas)) = state.as_mut() else {
            continue;
        };
        // Walk the line, tracking depth and top-level commas, collecting
        // literals only once enough argument separators have gone by.
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        let mut in_string = false;
        let mut current = String::new();
        while i < chars.len() {
            let c = chars[i];
            match c {
                '\\' if in_string => {
                    i += 1;
                    if i < chars.len() {
                        current.push(chars[i]);
                    }
                }
                '"' => {
                    if in_string {
                        if *commas >= *skip {
                            out.push(ProseLiteral {
                                line: idx + 1,
                                text: std::mem::take(&mut current),
                            });
                        } else {
                            current.clear();
                        }
                    }
                    in_string = !in_string;
                }
                _ if in_string => current.push(c),
                '(' | '[' | '{' => *depth += 1,
                ')' | ']' | '}' => *depth -= 1,
                ',' if *depth == 1 => *commas += 1,
                _ => {}
            }
            i += 1;
        }
        if *depth <= 0 {
            state = None;
        }
    }
    out
}

/// Drop a trailing `//` comment, which is governed by the comment ruler.
fn strip_line_comment(line: &str) -> String {
    let mut in_string = false;
    let bytes: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            '\\' if in_string => i += 1,
            '"' => in_string = !in_string,
            '/' if !in_string && i + 1 < bytes.len() && bytes[i + 1] == '/' => {
                return bytes[..i].iter().collect();
            }
            _ => {}
        }
        i += 1;
    }
    line.to_string()
}

/// Net parenthesis balance of a line, ignoring parens inside string literals.
fn balance(line: &str) -> i32 {
    let mut depth = 0i32;
    let mut in_string = false;
    let bytes: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            '\\' if in_string => i += 1,
            '"' => in_string = !in_string,
            '(' if !in_string => depth += 1,
            ')' if !in_string => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    depth
}

/// Contents of every double-quoted literal on a line.
#[must_use]
pub fn string_literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let bytes: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            '\\' if in_string => {
                i += 1;
                if i < bytes.len() {
                    current.push(bytes[i]);
                }
            }
            '"' => {
                if in_string {
                    out.push(std::mem::take(&mut current));
                }
                in_string = !in_string;
            }
            c if in_string => current.push(c),
            _ => {}
        }
        i += 1;
    }
    out
}

/// Portuguese identifier declared on this line, if any.
///
/// Skips serde attributes: `#[serde(rename = "quantidade_resultados")]` names
/// the `--wire-keys pt` contract, which is required to be Portuguese.
#[must_use]
pub fn portuguese_identifier(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("//") || trimmed.starts_with("#[") || trimmed.contains("serde(") {
        return None;
    }
    // Walk the modifier prefix instead of matching a fixed first word.
    //
    // The previous version matched only `let|fn|const|static|pub` in FIRST
    // position, so `async fn` fell through to the catch-all and every async
    // declaration in the repository was invisible to this axis. Measured on
    // 2026-08-10: `async fn binario_no_news_emite_envelope_com_campos_news_
    // aditivos` sat in `tests/` while this ruler reported zero identifiers.
    // Enumerating the modifiers that may PRECEDE a declarer is total in a way
    // that enumerating first words never was.
    const MODIFIERS: &[&str] = &["async", "unsafe", "extern", "mut", "default", "move"];
    const DECLARERS: &[&str] = &["let", "fn", "const", "static"];

    let mut saw_declarer = false;
    let mut name = "";
    for word in trimmed.split_whitespace() {
        // `pub`, `pub(crate)`, `pub(super)` — one arm, no visibility list.
        if word.starts_with("pub") || MODIFIERS.contains(&word) {
            continue;
        }
        if DECLARERS.contains(&word) {
            saw_declarer = true;
            continue;
        }
        if saw_declarer {
            name = word;
        }
        break;
    }
    if !saw_declarer || name.is_empty() {
        return None;
    }
    let name = name
        .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_')
        .split(['(', ':', '<', '='])
        .next()
        .unwrap_or("");
    if name.is_empty() {
        return None;
    }
    IDENTIFIER_STEMS
        .iter()
        .any(|stem| name.split('_').any(|part| part == *stem))
        .then(|| name.to_string())
}

/// Every `.rs` file under the given roots, skipping the given file names.
#[must_use]
pub fn rust_files(roots: &[&Path], skip_names: &[&str]) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = roots.iter().map(|p| p.to_path_buf()).collect();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if skip_names.contains(&name) {
                continue;
            }
            files.push(path);
        }
    }
    files.sort();
    files
}
