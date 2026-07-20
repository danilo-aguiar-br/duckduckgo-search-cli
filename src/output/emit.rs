// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: stdout/stderr I/O + atomic file write + streaming emit.
//! Emission sinks: stdout, stderr, `--output` file, NDJSON / stream blocks.
//!
//! **INVIOLABLE RULE (MP-06)**: this submodule (with the rest of `output`) is
//! the only place authorized to write the operator payload to stdout / the
//! `--output` file, and human messages to stderr via [`emit_stderr`].

use super::format::{
    format_multi, format_single, format_single_markdown, format_single_text, resolve_auto_format,
};
use crate::error::CliError;
use crate::pipeline::PipelineResult;
use crate::types::{MultiSearchOutput, OutputFormat, SearchOutput};
use std::fmt::{self, Write as FmtWrite};
use std::io::{self, Write};
use std::path::Path;

/// Prints the search result in the specified format and destination.
///
/// `output_path = None` → stdout. `Some(path)` → file (with creation of
/// parent directories if absent).
///
/// # Errors
///
/// Returns an error if writing to stdout or the output file fails, or if
/// JSON serialization of the result fails.
pub fn emit_result(
    result: &PipelineResult,
    format: OutputFormat,
    output_path: Option<&Path>,
) -> Result<(), CliError> {
    // Stream already emitted incrementally — nothing to do here.
    if matches!(result, PipelineResult::Stream(_)) {
        tracing::info!("PipelineResult::Stream — output already emitted via streaming");
        return Ok(());
    }

    let resolved_format = resolve_auto_format(format, output_path);
    let text = match result {
        PipelineResult::Single(output) => format_single(output.as_ref(), resolved_format)?,
        PipelineResult::Multi(output) => format_multi(output.as_ref(), resolved_format)?,
        PipelineResult::Stream(_) => {
            // GAP-OPS-008 (v0.8.0): unreachable!() replaced with proper Err propagation.
            // Stream variant should be consumed by the streaming consumer BEFORE emit_result
            // is called. If we reach this branch, it is a programming error (invariant violation),
            // not a condition to abort the process. Returning Err preserves cleanup paths and
            // gives the caller a structured error to log/report instead of a panic stack trace.
            return Err(CliError::InvalidConfig {
                message: "PipelineResult::Stream reached emit_result; stream variants must be consumed by the streaming consumer before non-streaming emit".to_string(),
            });
        }
    };

    match output_path {
        Some(path) => write_to_file(path, &text),
        None => write_to_stdout(&text),
    }
}

/// Async emit: format/serde off the Tokio worker (GAP-PAR-040a).
///
/// Builds the full payload via [`crate::concurrency::run_cpu_bound`], then
/// writes to stdout/file on the async task (short I/O). Prefer this from
/// `async fn` work paths (`lib::run`, deep-research). Sync [`emit_result`]
/// remains for unit tests and pure-sync utility handlers.
///
/// # Errors
///
/// Same as [`emit_result`], plus CPU-gate / join failures from `run_cpu_bound`.
pub async fn emit_result_async(
    result: &PipelineResult,
    format: OutputFormat,
    output_path: Option<&Path>,
) -> Result<(), CliError> {
    if matches!(result, PipelineResult::Stream(_)) {
        tracing::info!("PipelineResult::Stream — output already emitted via streaming");
        return Ok(());
    }

    let resolved_format = resolve_auto_format(format, output_path);
    let text = match result {
        PipelineResult::Single(output) => {
            let owned = output.as_ref().clone();
            crate::concurrency::run_cpu_bound(move || format_single(&owned, resolved_format))
                .await?
        }
        PipelineResult::Multi(output) => {
            let owned = output.as_ref().clone();
            crate::concurrency::run_cpu_bound(move || format_multi(&owned, resolved_format))
                .await?
        }
        PipelineResult::Stream(_) => {
            return Err(CliError::InvalidConfig {
                message: "PipelineResult::Stream reached emit_result_async; stream variants must be consumed by the streaming consumer before non-streaming emit".to_string(),
            });
        }
    }?;

    match output_path {
        Some(path) => write_to_file(path, &text),
        None => write_to_stdout(&text),
    }
}

/// Emit a pre-formatted payload to stdout or `--output` file (GAP-E2E-48-006 / DRY).
///
/// Single route shared by `buscar`, `deep-research` success, and structured
/// error envelopes (timeout JSON). With `Some(path)`, writes atomically and
/// leaves stdout empty; with `None`, writes one line (or block) to stdout.
///
/// # Errors
///
/// Path validation / atomic write failures, or stdout broken pipe.
pub fn emit_payload(content: &str, output_path: Option<&Path>) -> Result<(), CliError> {
    match output_path {
        Some(path) => write_to_file(path, content),
        None => write_to_stdout(content),
    }
}

/// Async wrapper for [`emit_payload`] (CPU-light; write stays on the async task).
///
/// # Errors
///
/// Same as [`emit_payload`].
pub async fn emit_payload_async(
    content: String,
    output_path: Option<&Path>,
) -> Result<(), CliError> {
    emit_payload(&content, output_path)
}

/// Serialize `value` to compact JSON off the Tokio worker (GAP-PAR-040c).
///
/// # Errors
///
/// CPU-gate failures or serde errors mapped to [`CliError::InvalidConfig`].
pub async fn serialize_json_async<T>(value: T) -> Result<String, CliError>
where
    T: serde::Serialize + Send + 'static,
{
    crate::concurrency::run_cpu_bound(move || {
        serde_json::to_string(&value).map_err(|e| CliError::InvalidConfig {
            message: format!("failed to serialize JSON: {e}"),
        })
    })
    .await?
}

/// Backwards-compatible wrapper for callers that still use only (result, format).
/// Kept to reduce churn in existing tests; new call-sites should use
/// `emit_result` with an explicit `output_path`.
///
/// # Errors
///
/// Returns an error if writing to stdout fails or if JSON serialization fails.
pub fn emit(output: &SearchOutput, format: OutputFormat) -> Result<(), CliError> {
    let resolved_format = resolve_auto_format(format, None);
    let text = format_single(output, resolved_format)?;
    write_to_stdout(&text)
}

/// Backwards-compatible wrapper for multi-query.
///
/// # Errors
///
/// Returns an error if writing to stdout fails or if JSON serialization fails.
pub fn emit_multi(output: &MultiSearchOutput, format: OutputFormat) -> Result<(), CliError> {
    let resolved_format = resolve_auto_format(format, None);
    let text = format_multi(output, resolved_format)?;
    write_to_stdout(&text)
}

/// Emits a human-facing message to stderr (MP-06).
///
/// Prefer `format_args!(...)` over `format!(...)` so no intermediate `String`
/// is allocated (rules-rust macros: avoid `format!` + print double allocation):
///
/// ```ignore
/// output::emit_stderr(format_args!("Error: {err:#}"));
/// output::emit_stderr("plain message");
/// ```
///
/// Other modules must not call `eprintln!` directly — use this sink instead.
#[inline]
pub fn emit_stderr(msg: impl fmt::Display) {
    let _ = writeln!(std::io::stderr(), "{msg}");
}

pub(super) fn write_to_stdout(content: &str) -> Result<(), CliError> {
    let stdout = io::stdout();
    let lock = stdout.lock();
    let mut writer = io::BufWriter::new(lock);
    writeln!(writer, "{content}").map_err(|e| map_io(&e, "failed to write to stdout"))?;
    writer
        .flush()
        .map_err(|e| map_io(&e, "failed to flush stdout"))?;
    Ok(())
}

#[cold]
fn map_io(e: &io::Error, ctx: &str) -> CliError {
    if e.kind() == io::ErrorKind::BrokenPipe {
        CliError::BrokenPipe
    } else {
        CliError::PathError {
            message: format!("{ctx}: {e}"),
        }
    }
}

/// Maps `serde_json` write errors, preserving `BrokenPipe` (exit 141) when the
/// underlying I/O failed because the pipe consumer closed early.
///
/// `serde_json::to_writer` surfaces I/O failures as `serde_json::Error`; mapping
/// them to `InvalidConfig` would mis-report SIGPIPE as a config problem.
#[cold]
pub(super) fn map_serde_write(e: &serde_json::Error, ctx: &str) -> CliError {
    if e.io_error_kind() == Some(io::ErrorKind::BrokenPipe) {
        CliError::BrokenPipe
    } else {
        CliError::InvalidConfig {
            message: format!("{ctx}: {e}"),
        }
    }
}

/// Checks whether a `CliError` is a `BrokenPipe` variant. Broken pipe indicates
/// the pipe reader closed (e.g. `| jaq`, `| head`). Callers MUST map this to
/// exit code **141** (`exit_codes::BROKEN_PIPE`), not 0 — rules-rust-cli-stdin-stdout.
// Trivial match — `#[inline]` (not always): hot path on every emit; body is one match.
#[inline]
pub(crate) fn is_broken_pipe(error: &CliError) -> bool {
    matches!(error, CliError::BrokenPipe)
}

/// Public: prints ONE line terminated with `\n` to stdout, with immediate flush.
/// Used by auxiliary subcommands (e.g. `init-config`) that need to emit JSON.
///
/// # Errors
///
/// Returns an error if writing to stdout fails or if the pipe is broken.
pub fn print_line_stdout(content: &str) -> Result<(), CliError> {
    write_to_stdout(content)
}

/// Public: emits a `SearchOutput` as ONE NDJSON line (compact JSON + `\n`).
///
/// If `output_file = Some`, opens the file in append mode and writes — used by
/// the `--stream` multi-query consumer to write streaming without holding everything in memory.
/// If `None`, writes to stdout with immediate flush (for real-time pipes).
///
/// # Errors
///
/// Returns an error if JSON serialization fails, if creating or appending to the
/// output file fails, or if writing to stdout fails.
pub fn emit_ndjson(
    output: &crate::types::SearchOutput,
    output_file: Option<&Path>,
) -> Result<(), CliError> {
    // NDJSON contract: one compact (non-pretty) JSON object per line + LF.
    // Pretty-print is forbidden — multi-line objects break line-oriented consumers.
    match output_file {
        Some(path) => {
            let line = serde_json::to_string(output)
                .map_err(|e| map_serde_write(&e, "failed to serialize search output as NDJSON"))?;
            append_line_to_file(path, &line)
        }
        None => {
            let stdout = io::stdout();
            let lock = stdout.lock();
            let mut writer = io::BufWriter::new(lock);
            serde_json::to_writer(&mut writer, output)
                .map_err(|e| map_serde_write(&e, "failed to serialize NDJSON"))?;
            writeln!(writer).map_err(|e| map_io(&e, "failed to write NDJSON newline"))?;
            writer
                .flush()
                .map_err(|e| map_io(&e, "failed to flush stdout"))?;
            Ok(())
        }
    }
}

/// Async NDJSON emit: serde off the Tokio worker (GAP-PAR-040b).
///
/// # Errors
///
/// CPU-gate failures, serde errors, or write failures.
pub async fn emit_ndjson_async(
    output: SearchOutput,
    output_file: Option<std::path::PathBuf>,
) -> Result<(), CliError> {
    let line = crate::concurrency::run_cpu_bound(move || {
        serde_json::to_string(&output)
            .map_err(|e| map_serde_write(&e, "failed to serialize search output as NDJSON"))
    })
    .await??;
    match output_file {
        Some(path) => append_line_to_file(&path, &line),
        None => {
            let mut block = line;
            block.push('\n');
            write_to_stdout(&block)
        }
    }
}

/// Emits a text block (`text` format) in streaming mode, representing ONE query.
///
/// # Errors
///
/// Returns an error if writing to the output file or stdout fails.
pub fn emit_stream_text(
    index: usize,
    output: &crate::types::SearchOutput,
    output_file: Option<&Path>,
) -> Result<(), CliError> {
    let mut block = String::with_capacity(900);
    let _ = writeln!(block, "========== Query #{} ==========", index + 1);
    block.push_str(&format_single_text(output));
    emit_block_stream(&block, output_file)
}

/// Async stream text emit (GAP-PAR-040b): format off worker, then write.
///
/// # Errors
///
/// CPU-gate failures or write failures.
pub async fn emit_stream_text_async(
    index: usize,
    output: SearchOutput,
    output_file: Option<std::path::PathBuf>,
) -> Result<(), CliError> {
    let block = crate::concurrency::run_cpu_bound(move || {
        let mut block = String::with_capacity(900);
        let _ = writeln!(block, "========== Query #{} ==========", index + 1);
        block.push_str(&format_single_text(&output));
        block
    })
    .await?;
    emit_block_stream(&block, output_file.as_deref())
}

/// Emits a Markdown block in streaming mode, representing ONE query.
///
/// # Errors
///
/// Returns an error if writing to the output file or stdout fails.
pub fn emit_stream_markdown(
    index: usize,
    output: &crate::types::SearchOutput,
    output_file: Option<&Path>,
) -> Result<(), CliError> {
    let mut block = String::with_capacity(1200);
    if index > 0 {
        block.push_str("\n---\n\n");
    }
    block.push_str(&format_single_markdown(output));
    emit_block_stream(&block, output_file)
}

/// Async stream Markdown emit (GAP-PAR-040b).
///
/// # Errors
///
/// CPU-gate failures or write failures.
pub async fn emit_stream_markdown_async(
    index: usize,
    output: SearchOutput,
    output_file: Option<std::path::PathBuf>,
) -> Result<(), CliError> {
    let block = crate::concurrency::run_cpu_bound(move || {
        let mut block = String::with_capacity(1200);
        if index > 0 {
            block.push_str("\n---\n\n");
        }
        block.push_str(&format_single_markdown(&output));
        block
    })
    .await?;
    emit_block_stream(&block, output_file.as_deref())
}

/// Emits `block` to stdout or appends to the indicated file. Used by text/md streams.
fn emit_block_stream(block: &str, output_file: Option<&Path>) -> Result<(), CliError> {
    match output_file {
        Some(path) => append_line_to_file(path, block),
        None => {
            let stdout = io::stdout();
            let lock = stdout.lock();
            let mut writer = io::BufWriter::new(lock);
            write!(writer, "{block}")
                .map_err(|e| map_io(&e, "failed to write streaming block to stdout"))?;
            writer
                .flush()
                .map_err(|e| map_io(&e, "failed to flush stdout"))?;
            Ok(())
        }
    }
}

/// Appends ONE line to a file (append + create mode), applying 0o644 on Unix on
/// first creation. Creates parent directories if needed.
fn append_line_to_file(path: &Path, line: &str) -> Result<(), CliError> {
    use std::fs::OpenOptions;
    crate::paths::validate_output_path(path)?;
    crate::paths::create_parent_dirs(path)?;
    let needed_create = !path.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| CliError::PathError {
            message: format!("failed to open (append) {}: {e}", path.display()),
        })?;
    writeln!(file, "{line}").map_err(|e| CliError::PathError {
        message: format!("failed to write to {}: {e}", path.display()),
    })?;
    file.flush().map_err(|e| CliError::PathError {
        message: format!("failed to flush {}: {e}", path.display()),
    })?;
    drop(file);

    #[cfg(unix)]
    if needed_create {
        crate::paths::apply_permissions_644(path)?;
    }
    #[cfg(not(unix))]
    let _ = needed_create;

    Ok(())
}

/// Writes `content` to `path`, creating parent directories if needed.
/// Uses atomic write (tempfile + rename) per rules-rust atomwrite (L-10).
/// Applies 0o644 permissions on Unix (owner writes, everyone reads).
pub(super) fn write_to_file(path: &Path, content: &str) -> Result<(), CliError> {
    crate::paths::validate_output_path(path)?;
    crate::paths::atomic_write(path, content.as_bytes())?;
    crate::paths::apply_permissions_644(path)?;

    tracing::info!(path = %path.display(), bytes = content.len(), "output written to file (atomic)");
    Ok(())
}
