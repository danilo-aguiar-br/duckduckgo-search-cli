// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (process exit code constants)
//! Process exit codes (specification section 17.7).
//!
//! POSIX-oriented values used by agents and shells. Windows maps the same
//! integers through [`std::process::ExitCode`].

/// At least one query returned results.
pub const SUCCESS: i32 = 0;
/// Generic error (configuration failure, IO, etc.).
pub const GENERIC_ERROR: i32 = 1;
/// Invalid configuration (incompatible CLI arguments).
pub const INVALID_CONFIG: i32 = 2;
/// Rate limiting or blocking on all queries.
pub const RATE_LIMITED_OR_BLOCKED: i32 = 3;
/// Global timeout exceeded.
pub const GLOBAL_TIMEOUT: i32 = 4;
/// Zero results on all queries.
pub const ZERO_RESULTS: i32 = 5;
/// Zero results caused by suspected anti-bot block (auto-classified).
///
/// Distinguishes environment-level blocking from genuine empty results.
/// Triggered when zero-cause analysis classifies the empty SERP as a soft
/// block (ghost-block / anti-bot / invalid-response) under strict mode.
/// Legacy opt-out via CLI `--no-zero-cause-strict` preserves exit 5
/// (product env `DUCKDUCKGO_ZERO_CAUSE_STRICT` is **removed**).
///
/// Semver: additive extension of the exit-code range, **not** a replacement
/// of [`ZERO_RESULTS`]. Consumers that branch only on 5 should use the
/// documented CLI opt-out to preserve v0.7.x behavior.
pub const SUSPECTED_BLOCK: i32 = 6;
/// Cooperative cancel via SIGINT / Ctrl+C (128 + SIGINT(2) = **130** per POSIX).
///
/// Prefer this default for programmatic cancel and Ctrl+C. When the process
/// received **SIGTERM**, use [`CANCELLED_SIGTERM`] (**143**) instead — see
/// [`crate::signals::exit_code_for_error`].
pub const CANCELLED: i32 = 130;
/// Cooperative cancel via SIGTERM / Ctrl+Break (128 + SIGTERM(15) = **143**).
///
/// Used by supervisors (`timeout`, Docker stop, systemd). Distinct from
/// [`CANCELLED`] so agents can tell interactive Ctrl+C from orchestrated stop.
pub const CANCELLED_SIGTERM: i32 = 143;
/// Consumer closed stdout early (SIGPIPE / `BrokenPipe`).
///
/// POSIX convention: 128 + SIGPIPE(13) = **141**. Required by
/// rules-rust-cli-stdin-stdout (stream discipline). Agents and shells
/// that branch on pipeline termination use this code, not 0.
pub const BROKEN_PIPE: i32 = 141;
