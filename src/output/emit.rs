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
use super::project::{
    format_multi_json_projected, format_search_json_projected, format_search_tsv_projected,
    FieldSet,
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
/// When `fields` is `Some`, JSON/TSV project result rows to the allowlisted
/// keys (GAP-FIELDS-PROJECT). Caller should already have applied filter +
/// struct-level strip via [`super::project::apply_to_search_output`].
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
    emit_result_with_fields(result, format, output_path, None)
}

/// Like [`emit_result`] with optional field projection.
///
/// # Errors
///
/// Same as [`emit_result`]: write failures, broken pipe, or serialization errors.
pub fn emit_result_with_fields(
    result: &PipelineResult,
    format: OutputFormat,
    output_path: Option<&Path>,
    fields: Option<&FieldSet>,
) -> Result<(), CliError> {
    // Stream already emitted incrementally — nothing to do here.
    if matches!(result, PipelineResult::Stream(_)) {
        tracing::info!("PipelineResult::Stream — output already emitted via streaming");
        return Ok(());
    }

    let resolved_format = resolve_auto_format(format, output_path);
    let text = match result {
        PipelineResult::Single(output) => {
            format_single_projected(output.as_ref(), resolved_format, fields)?
        }
        PipelineResult::Multi(output) => {
            format_multi_projected(output.as_ref(), resolved_format, fields)?
        }
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
    emit_result_with_fields_async(result, format, output_path, None).await
}

/// Async emit with optional field projection (GAP-FIELDS-PROJECT).
///
/// # Errors
///
/// Same as [`emit_result_async`]: write failures, broken pipe, serde, or CPU-gate join errors.
pub async fn emit_result_with_fields_async(
    result: &PipelineResult,
    format: OutputFormat,
    output_path: Option<&Path>,
    fields: Option<FieldSet>,
) -> Result<(), CliError> {
    if matches!(result, PipelineResult::Stream(_)) {
        tracing::info!("PipelineResult::Stream — output already emitted via streaming");
        return Ok(());
    }

    let resolved_format = resolve_auto_format(format, output_path);
    let text = match result {
        PipelineResult::Single(output) => {
            let owned = output.as_ref().clone();
            let fields = fields.clone();
            crate::concurrency::run_cpu_bound(move || {
                format_single_projected(&owned, resolved_format, fields.as_ref())
            })
            .await?
        }
        PipelineResult::Multi(output) => {
            let owned = output.as_ref().clone();
            let fields = fields.clone();
            crate::concurrency::run_cpu_bound(move || {
                format_multi_projected(&owned, resolved_format, fields.as_ref())
            })
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

fn format_single_projected(
    output: &SearchOutput,
    format: OutputFormat,
    fields: Option<&FieldSet>,
) -> Result<String, CliError> {
    match (format, fields) {
        (OutputFormat::Json | OutputFormat::Auto, Some(fs)) => {
            format_search_json_projected(output, fs)
        }
        (OutputFormat::Tsv, Some(fs)) => Ok(format_search_tsv_projected(output, fs)),
        _ => format_single(output, format),
    }
}

fn format_multi_projected(
    output: &MultiSearchOutput,
    format: OutputFormat,
    fields: Option<&FieldSet>,
) -> Result<String, CliError> {
    match (format, fields) {
        (OutputFormat::Json | OutputFormat::Auto, Some(fs)) => {
            format_multi_json_projected(output, fs)
        }
        _ => format_multi(output, format),
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
    // Agent-native anti-token: honor --max-output-bytes / XDG (0 = unlimited).
    super::agent_ops::enforce_max_output_bytes(
        content,
        super::agent_ops::process_max_output_bytes(),
    )?;
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
    crate::concurrency::run_cpu_bound(move || crate::output::to_wire_string(&value)).await?
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
    // Agent-native anti-token cap, enforced at the ONE place every stdout byte
    // passes through. It used to live only in `emit_payload`, which search and
    // deep-research use — so `--max-output-bytes` was accepted and silently
    // ignored by `--probe`, `doctor`, `commands`, `schema`, `config` and
    // `locale`, the surfaces whose envelopes an agent is most likely to want
    // capped. A flag that parses and does nothing is worse than a rejected
    // flag: the caller believes the budget is being honoured.
    super::agent_ops::enforce_max_output_bytes(
        content,
        super::agent_ops::process_max_output_bytes(),
    )?;
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
#[allow(dead_code)] // retained for map_serde_write error mapping in stream paths / tests
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

/// Serializes `value` under the active wire-keys policy and prints it as one line.
///
/// Prefer this over `print_line_stdout(&payload.to_string())` for anything that
/// carries wire fields. The raw form skips
/// [`crate::output::serialize_for_wire`], so `--wire-keys pt` silently keeps the
/// English spellings — that is how eleven thin-error sites drifted before
/// v1.0.3. Introspection surfaces (`config`, `schema`, `commands`, `locale`)
/// deliberately stay English and must keep using `print_line_stdout` directly.
///
/// # Errors
///
/// Returns an error if serialization fails or if writing to stdout fails.
pub fn emit_wire_line<T: serde::Serialize>(value: &T) -> Result<(), CliError> {
    let rendered = crate::output::serialize_for_wire(value)?;
    write_to_stdout(&rendered)
}

/// Emits an introspection envelope, applying agent-native reduction first.
///
/// # Why introspection needs its own emit
///
/// [`emit_wire_line`] does two things: it remaps keys to Portuguese when
/// `--wire-keys pt` is active, and it writes. Introspection surfaces must NOT
/// do the first — `doctor`, `commands`, `schema`, `locale` and `config` stay
/// English by design, and a test asserts it. So they used
/// [`print_line_stdout`] and, in doing so, skipped every reduction the caller
/// asked for. `--fields`, `--filter`, `--sort`, `--dedupe-by`, `--limit` and
/// `--count-only` parsed, validated and vanished.
///
/// This is the third emit path: reduce, keep English, write. Which reductions
/// an envelope can honour is declared in
/// [`crate::output::envelope_ops::SURFACES`]; anything it cannot honour is
/// refused here rather than ignored.
///
/// # Errors
///
/// [`CliError::InvalidConfig`] when a requested reduction has no meaning for
/// this envelope, plus any serialization or stdout failure.
pub fn emit_envelope(
    value: serde_json::Value,
    shape: &crate::output::envelope_ops::EnvelopeShape,
    pretty: bool,
) -> Result<(), CliError> {
    emit_envelope_with(value, shape, pretty, KeyPolicy::EnglishOnly)
}

/// Which key spelling an envelope leaves the process with.
///
/// The reduction boundary is shared, the key policy is not. Introspection
/// (`doctor`, `commands`, `schema`, `locale`, `config`) is deliberately
/// English on both wire settings, and a test asserts it. The probe family is a
/// WIRE surface: it has carried Portuguese spellings under `--wire-keys pt`
/// since before the projector existed, and consumers read them.
///
/// Making the policy an argument is what let the probe join the projector
/// without regressing either contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPolicy {
    /// Keys stay English regardless of `--wire-keys` (introspection).
    EnglishOnly,
    /// Keys follow the process `--wire-keys` policy (wire surfaces).
    ProcessWire,
}

/// Reduce an envelope, apply `keys`, and write it to stdout.
///
/// # Why the reduction runs BEFORE the key mapping
///
/// `--fields status` has to mean the same thing under `--wire-keys en` and
/// `--wire-keys pt`. Reducing first means paths are always matched against the
/// English document, so the operator writes one path and gets the same
/// projection either way; the Portuguese spelling is applied to whatever
/// survived. Mapping first would silently make every documented `--fields`
/// path wrong for half the users.
///
/// # Errors
///
/// [`CliError::InvalidConfig`] when a requested reduction has no meaning for
/// this envelope, plus any serialization or stdout failure.
pub fn emit_envelope_with(
    mut value: serde_json::Value,
    shape: &crate::output::envelope_ops::EnvelopeShape,
    pretty: bool,
    keys: KeyPolicy,
) -> Result<(), CliError> {
    let ops = crate::output::envelope_ops::process_agent_ops();
    crate::output::envelope_ops::apply_generic(&mut value, ops, shape)?;
    let value = match keys {
        KeyPolicy::EnglishOnly => value,
        KeyPolicy::ProcessWire => super::wire_keys::finalize_wire_value(value),
    };
    let rendered = if pretty {
        serde_json::to_string_pretty(&value)
    } else {
        serde_json::to_string(&value)
    }
    .map_err(|e| CliError::InvalidConfig {
        message: format!("failed to serialize envelope: {e}"),
    })?;
    write_to_stdout(&rendered)
}

/// Emit an envelope through the reduction boundary, or refuse routably.
///
/// # The defect this closes
///
/// Refusal had two contracts. `doctor`, `locale`, `commands`, `schema` and
/// `init-config` each hand-rolled the same `match`: exit 2, prose on stderr,
/// stdout EMPTY. `config` hand-rolled a different one: exit 2, an
/// `error-response` envelope on stdout, nothing on stderr. Same flag, same
/// failure, two shapes, and nothing declaring which was intended — so an agent
/// parsing stdout got a routable refusal from one family and silence from the
/// other, with no way to tell a refusal from a crash.
///
/// There is now one contract for every surface, including the probe:
///
/// - stdout carries `{"error": code, "message": …}`, the published
///   `error-response` shape, so the refusal is as machine-readable as the
///   success it replaces.
/// - stderr carries the localized human sentence, so `--ui-lang` still works.
/// - the exit code is whatever the error already earned, normally `2`.
///
/// The stdout `message` stays English on purpose. It is the machine half of
/// the contract and shares the reasoning that keeps introspection keys
/// English: a consumer must not have to negotiate locale to parse a failure.
pub fn emit_envelope_or_refuse(
    value: serde_json::Value,
    shape: &crate::output::envelope_ops::EnvelopeShape,
    pretty: bool,
    keys: KeyPolicy,
) -> i32 {
    match emit_envelope_with(value, shape, pretty, keys) {
        Ok(()) => crate::error::exit_codes::SUCCESS,
        Err(err) if is_broken_pipe(&err) => crate::error::exit_codes::BROKEN_PIPE,
        Err(err) => refuse(&err),
    }
}

/// Write one refusal to both streams and return its exit code.
///
/// Broken pipe on the stdout half is swallowed deliberately: the caller closed
/// the pipe, the refusal still happened, and the exit code it earned does not
/// become 141 because nobody was listening.
pub fn refuse(err: &CliError) -> i32 {
    let payload = serde_json::json!({
        "error": err.error_code(),
        "message": err.to_string(),
    });
    let _ = write_to_stdout(&payload.to_string());
    emit_stderr(err.localized_detail());
    err.exit_code()
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
            let line = crate::output::to_wire_string(output)?;
            append_line_to_file(path, &line)
        }
        None => {
            let line = crate::output::to_wire_string(output)?;
            let mut block = line;
            block.push('\n');
            write_to_stdout(&block)
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
    let line =
        crate::concurrency::run_cpu_bound(move || crate::output::to_wire_string(&output)).await??;
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
