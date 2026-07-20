// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-light (query file/stdin combine; one-shot)
//! Query list assembly helpers (GAP-COMP-006r).

use crate::error::CliError;
use crate::search::AggregatedSearchResult;
use crate::types::SelectorConfig;
use std::collections::HashSet;
use std::path::Path;

/// Combines queries from three sources (positional, file, stdin), deduplicates
/// preserving the ORDER of the first occurrence, and filters empty strings after trim.
///
/// Performs no I/O: expects the caller to have already collected the lines (useful for tests).
///
/// # Example
///
/// ```
/// use duckduckgo_search_cli::pipeline::combine_and_dedup_queries;
///
/// let result_vec = combine_and_dedup_queries(
///     vec!["rust".into(), "  ".into(), "tokio".into()],
///     vec!["rust".into(), "serde".into()],
///     vec!["".into(), "serde".into(), "axum".into()],
/// );
///
/// // Dedup preserves order of first occurrence; empty strings (after trim) are removed.
/// assert_eq!(result_vec, vec!["rust", "tokio", "serde", "axum"]);
/// ```
pub fn combine_and_dedup_queries(
    posicionais: Vec<String>,
    de_arquivo: Vec<String>,
    de_stdin: Vec<String>,
) -> Vec<String> {
    let capacity = posicionais.len() + de_arquivo.len() + de_stdin.len();
    let mut vistos: HashSet<String> = HashSet::with_capacity(capacity);
    let mut result_vec: Vec<String> = Vec::with_capacity(capacity);

    let todas = posicionais.into_iter().chain(de_arquivo).chain(de_stdin);

    for raw in todas {
        let clean = raw.trim();
        if clean.is_empty() {
            continue;
        }
        // Clone only for unique queries (avoid allocating on duplicates).
        if vistos.contains(clean) {
            continue;
        }
        let owned = clean.to_string();
        vistos.insert(owned.clone());
        result_vec.push(owned);
    }

    result_vec
}

/// Reads a queries file — one query per line, ignoring empty lines after trim.
///
/// Correctly handles both `\n` and `\r\n` (Windows) via `BufRead::lines`.
///
/// Hard limits (defensive security):
/// - file size ≤ [`crate::security::MAX_QUERIES_FILE_BYTES`]
/// - non-empty lines ≤ [`crate::security::MAX_QUERIES`] (early reject before full parse)
///
/// # Errors
///
/// Returns an error if the file cannot be opened, exceeds size limits, or if any
/// line cannot be read (e.g. invalid UTF-8 or an I/O error).
// std::fs is intentional: query files are small config files (<1 KB typical)
// read synchronously BEFORE fan-out begins. No async tasks are blocked.
// Migrating to tokio::fs would add complexity without measurable benefit.
pub fn read_queries_from_file(path: &Path) -> Result<Vec<String>, CliError> {
    use std::io::BufRead;

    let meta = std::fs::metadata(path).map_err(|e| CliError::PathError {
        message: format!("failed to stat query file {}: {e}", path.display()),
    })?;
    if meta.len() > crate::security::MAX_QUERIES_FILE_BYTES {
        return Err(CliError::InvalidConfig {
            message: format!(
                "queries file {} exceeds {} byte limit ({} bytes)",
                path.display(),
                crate::security::MAX_QUERIES_FILE_BYTES,
                meta.len()
            ),
        });
    }

    let file = std::fs::File::open(path).map_err(|e| CliError::PathError {
        message: format!("failed to open query file {}: {e}", path.display()),
    })?;
    let reader = std::io::BufReader::new(file);
    let mut lines_vec: Vec<String> = Vec::with_capacity(20);
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| CliError::PathError {
            message: format!(
                "failed to read line {} of {}: {e}",
                index + 1,
                path.display()
            ),
        })?;
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            if lines_vec.len() >= crate::security::MAX_QUERIES {
                return Err(CliError::InvalidConfig {
                    message: format!(
                        "queries file {} has more than {} non-empty lines",
                        path.display(),
                        crate::security::MAX_QUERIES
                    ),
                });
            }
            lines_vec.push(trimmed);
        }
    }
    Ok(lines_vec)
}

/// Reads queries from stdin — one per line — ONLY if stdin is not a TTY.
/// Returns an empty `Vec` when stdin is a TTY (i.e. the user did not pipe/redirect input).
///
/// # Errors
///
/// Returns an error if any line from stdin cannot be read (e.g. invalid UTF-8
/// or an I/O error while consuming the piped input).
pub fn read_queries_from_stdin_if_pipe() -> Result<Vec<String>, CliError> {
    use std::io::{BufRead, IsTerminal};
    if std::io::stdin().is_terminal() {
        return Ok(Vec::new());
    }
    let reader = std::io::stdin().lock();
    let mut lines_vec: Vec<String> = Vec::with_capacity(20);
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| CliError::PathError {
            message: format!("failed to read line {} from stdin: {e}", index + 1),
        })?;
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            if lines_vec.len() >= crate::security::MAX_QUERIES {
                return Err(CliError::InvalidConfig {
                    message: format!(
                        "stdin provides more than {} non-empty query lines",
                        crate::security::MAX_QUERIES
                    ),
                });
            }
            lines_vec.push(trimmed);
        }
    }
    Ok(lines_vec)
}

/// Derives the observed cascade level from aggregate signals.
///
/// GAP-AUD-002 + GAP-AUD-010 v0.8.0: quando `cfg.last_probe_cascade_level`
/// is not populated (cross-process case), we infer the cascade level
/// from the search result: 0 additional attempts, 0 fallback → level 0.
/// 1 extra attempt with lite fallback → level 1. 2+ extra attempts →
/// level 2+. Faithfully documents what happened in the pipeline.
pub(crate) fn derive_cascade_level_from_attempts(agregado: &AggregatedSearchResult) -> u32 {
    let retries = agregado.attempts.saturating_sub(1);
    if agregado.used_fallback_lite && retries >= 2 {
        2
    } else if agregado.used_fallback_lite || retries >= 1 {
        1
    } else {
        0
    }
}

/// Computa a blake3 hash (hex, first 16 chars) of the serialised selector configuration.
/// Useful for versioning changes to the `selectors.toml` file in future iterations.
pub(crate) fn calculate_selectors_hash(cfg: &SelectorConfig) -> String {
    match toml::to_string(cfg) {
        Ok(serialized) => {
            let hash = blake3::hash(serialized.as_bytes());
            hash.to_hex().chars().take(16).collect()
        }
        Err(err) => {
            tracing::warn!(?err, "failed to serialize selector config for hash");
            "unknown".to_string()
        }
    }
}
