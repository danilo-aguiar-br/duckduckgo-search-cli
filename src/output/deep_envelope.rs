// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light JSON emit + one-shot shutdown contract (no Chrome I/O).
//! Deep-research agent envelopes: timeout, cancel, and in-flight registration.
//!
//! # One-shot contract (CM-05)
//!
//! - Internal timeout fence: emit envelope **before** Chrome reap.
//! - External SIGTERM/SIGINT force-exit: emit a **minimal** cancel envelope
//!   before [`crate::process_lifecycle::ensure_oneshot_cleanup`].
//! - Partial harvest is capped to bound memory and agent tokens.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{exit_codes, CliError};
use crate::output::{self, emit_payload};
use crate::signals::ShutdownReason;
use crate::types::bounded::DEEP_RESEARCH_PARTIAL_RESULT_CAP;

/// In-flight registration: deep-research is running.
#[derive(Debug, Clone)]
struct InFlight {
    /// Optional `-o` path; `None` means stdout.
    output: Option<PathBuf>,
}

static DEEP_STATE: Mutex<Option<InFlight>> = Mutex::new(None);

/// RAII guard: registers deep-research as in-flight for SIGTERM envelope emit.
///
/// Clears the registry on drop (normal completion or early return).
#[derive(Debug)]
pub struct DeepInFlightGuard {
    _private: (),
}

impl DeepInFlightGuard {
    /// Register deep-research as active (after budget gate, before fan-out).
    #[must_use]
    pub fn arm(output_path: Option<&Path>) -> Self {
        if let Ok(mut slot) = DEEP_STATE.lock() {
            *slot = Some(InFlight {
                output: output_path.map(Path::to_path_buf),
            });
        }
        Self { _private: () }
    }
}

impl Drop for DeepInFlightGuard {
    fn drop(&mut self) {
        clear_deep_research_in_flight();
    }
}

/// Clear the in-flight marker.
pub fn clear_deep_research_in_flight() {
    if let Ok(mut slot) = DEEP_STATE.lock() {
        *slot = None;
    }
}

/// Emit cancel envelope if deep-research is armed; clear state; return exit code.
///
/// When not in-flight, returns the signal exit code without writing JSON.
#[must_use]
pub fn emit_cancel_if_deep_in_flight(reason: ShutdownReason) -> i32 {
    let output = match DEEP_STATE.lock() {
        Ok(mut slot) => match slot.take() {
            Some(InFlight { output }) => output,
            None => return reason.exit_code(),
        },
        Err(_) => return reason.exit_code(),
    };
    emit_cancel_envelope(reason, output.as_deref())
}

/// Emit the minimal cancel JSON payload.
fn emit_cancel_envelope(reason: ShutdownReason, output_path: Option<&Path>) -> i32 {
    let exit = reason.exit_code();
    let payload = serde_json::json!({
        "error": "cancelled",
        "type": "deep_research_error",
        "command": "deep-research",
        "signal": reason.as_str(),
        "exit": exit,
        "partial": false,
    });
    let body = match output::value_to_wire_string(payload) {
        Ok(s) => s,
        Err(err) => {
            output::emit_stderr(format!("failed to serialize cancel envelope: {err}"));
            return exit;
        }
    };
    match emit_payload(&body, output_path) {
        Ok(()) => exit,
        Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
        Err(err) => {
            output::emit_stderr(format!("failed to emit cancel envelope: {err}"));
            exit
        }
    }
}

/// Agent-stable timeout envelope (GAP-E2E-48-007 / CM-05). Exit remains 4.
///
/// Truncates partial result lists to [`DEEP_RESEARCH_PARTIAL_RESULT_CAP`].
/// When `fields` is set, partial rows are projected (GAP-DEEP-PROJECT-FILTER).
pub async fn emit_timeout_envelope(
    seconds: u64,
    partial: Option<&crate::deep_research::DeepResearchOutput>,
    output_path: Option<&Path>,
    fields: Option<&crate::output::FieldSet>,
) -> i32 {
    let mut payload = serde_json::json!({
        "error": "timeout",
        "message": format!("global timeout of {seconds}s exceeded (deep-research)"),
        "seconds": seconds,
        "command": "deep-research",
        "type": "deep_research_error",
    });
    if let Some(out) = partial {
        let mut truncated = out.clone();
        if truncated.results.len() > DEEP_RESEARCH_PARTIAL_RESULT_CAP {
            truncated.results.truncate(DEEP_RESEARCH_PARTIAL_RESULT_CAP);
        }
        if truncated.news.len() > DEEP_RESEARCH_PARTIAL_RESULT_CAP {
            truncated.news.truncate(DEEP_RESEARCH_PARTIAL_RESULT_CAP);
        }
        // Prefer projected compact Value when agent requested --fields.
        let partial_json = if let Some(fs) = fields {
            match serde_json::to_value(&truncated) {
                Ok(mut v) => {
                    super::project::project_envelope_value(&mut v, fs);
                    Some(v)
                }
                Err(_) => None,
            }
        } else {
            serde_json::to_value(&truncated).ok()
        };
        if let Some(partial_json) = partial_json {
            payload["partial_results"] = partial_json;
            payload["partial"] = serde_json::json!(true);
            payload["partial_truncated"] = serde_json::json!(
                out.results.len() > DEEP_RESEARCH_PARTIAL_RESULT_CAP
                    || out.news.len() > DEEP_RESEARCH_PARTIAL_RESULT_CAP
            );
            // CLI-OBS-01 / CLI-TIMEOUT-01: agent-facing counters (not phone-home telemetry).
            payload["sub_queries_total"] = serde_json::json!(out.metadata.sub_queries_total);
            payload["sub_queries_ok"] = serde_json::json!(out.metadata.sub_queries_ok);
            payload["sub_queries_error"] = serde_json::json!(out.metadata.sub_queries_error);
            payload["subs_started"] = serde_json::json!(out.metadata.sub_queries_total);
            payload["subs_finished"] = serde_json::json!(
                out.metadata.sub_queries_ok + out.metadata.sub_queries_error
            );
            payload["dual_used"] = serde_json::json!(!out.news.is_empty() || out.news_count > 0);
            payload["chrome_contention_advisory"] =
                serde_json::json!(out.metadata.chrome_contention_advisory);
        }
    } else {
        payload["partial"] = serde_json::json!(false);
    }
    // Always surface host chrome_n at timeout for RCA (CLI-OBS-01).
    let chrome_n = crate::process_count::count_chrome_like_processes() as u64;
    payload["chrome_n"] = serde_json::json!(chrome_n);
    payload["next_action_suggestion"] = serde_json::json!(
        "Raise --global-timeout to print-budget suggested_global_timeout (or enable \
--auto-contention-budget); keep --news and -p>=2 for dual multiproc; only then try \
--no-fetch-content / --no-news as last resort."
    );
    let body = match output::value_to_wire_string(payload) {
        Ok(s) => s,
        Err(err) => {
            output::emit_stderr(format!("failed to serialize timeout envelope: {err}"));
            return exit_codes::GLOBAL_TIMEOUT;
        }
    };
    match emit_payload(&body, output_path) {
        Ok(()) => exit_codes::GLOBAL_TIMEOUT,
        Err(CliError::BrokenPipe) => exit_codes::BROKEN_PIPE,
        Err(err) => {
            output::emit_stderr(format!("failed to emit timeout envelope: {err}"));
            exit_codes::GLOBAL_TIMEOUT
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn cancel_envelope_writes_json_to_output_file() {
        let tmp = NamedTempFile::new().expect("tempfile");
        let path = tmp.path().to_path_buf();
        let _guard = DeepInFlightGuard::arm(Some(&path));
        // Disarm drop before emit by forgetting guard after manual take — emit takes state.
        // Keep guard alive so state remains until emit_cancel takes it.
        let code = emit_cancel_if_deep_in_flight(ShutdownReason::Terminate);
        assert_eq!(code, exit_codes::CANCELLED_SIGTERM);
        let body = fs::read_to_string(&path).expect("read envelope");
        let v: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(v["type"], "deep_research_error");
        assert_eq!(v["error"], "cancelled");
        assert_eq!(v["exit"], 143);
        assert_eq!(v["partial"], false);
        // Guard drop is harmless after state already cleared.
        drop(_guard);
    }

    #[test]
    fn cancel_without_in_flight_skips_write() {
        clear_deep_research_in_flight();
        let code = emit_cancel_if_deep_in_flight(ShutdownReason::Interrupt);
        assert_eq!(code, exit_codes::CANCELLED);
    }
}
