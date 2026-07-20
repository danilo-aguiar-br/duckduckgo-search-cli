// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (SIGPIPE/SIGINT/SIGTERM handler installation)
//! Cross-platform signal handlers for the CLI binary.
//!
//! Centralizes SIGPIPE (Unix) and SIGINT/SIGTERM (cross-platform) handling for a
//! **one-shot** process (rules-rust graceful-shutdown + rules-rust-cli-one-shot):
//!
//! 1. **Detect** — first SIGINT / SIGTERM (Unix) or Ctrl+C / Ctrl+Break (Windows)
//! 2. **Signal** — cancel the root [`CancellationToken`]
//! 3. **Await** — cooperative drain for [`cancel_force_exit_secs`], then hard exit
//!
//! - [`restore_sigpipe`]: keeps `SIG_IGN` so `| head` / `| jaq` yield EPIPE →
//!   exit **141** via BrokenPipe mapping **and** one-shot Chrome reap still runs
//!   (SIG_DFL would kill before Drop — GAP-E2E-51-001).
//! - [`install_cancellation_handler`]: first signal cancels; second signal or
//!   grace expiry force-exits with **130** (SIGINT) or **143** (SIGTERM).
//!
//! Daemon-only signals (SIGHUP reload, SIGUSR1/2 ops, readiness probes) are
//! intentionally **not** handled — this binary is BORN → EXECUTE → DIE.

use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::error::exit_codes;

/// Default cooperative-cancel grace before hard process exit (seconds).
///
/// rules-rust graceful-shutdown: 5–10s typical for interactive CLIs.
/// Override with CLI `--cancel-grace-secs` (clamped 1..=60) — GAP-SCRAPE-R2-011.
pub const CANCEL_FORCE_EXIT_SECS: u64 = 5;

/// Process-wide cancel grace (seconds). `0` means “use default”.
static CANCEL_GRACE_SECS: AtomicU64 = AtomicU64::new(0);

/// Last cancel exit code recorded by the signal handler (130 or 143).
///
/// Defaults to SIGINT convention so cooperative `CliError::Cancelled` stays
/// 130 when no signal was recorded (tests / programmatic cancel).
///
/// Atomic ordering: all loads/stores use **`Ordering::SeqCst`** (documented per
/// interior-mutability rules). Single writer (signal path) + readers on the
/// cooperative cancel / exit path; `SeqCst` is the conservative total-order
/// default so `last_cancel_exit_code()` always observes `record_cancel_exit`
/// before hard `process::exit`. `Release`/`Acquire` would also suffice for
/// this single-location flag; `SeqCst` is intentional.
static LAST_CANCEL_EXIT: AtomicI32 = AtomicI32::new(exit_codes::CANCELLED);

/// Why the process is shutting down (maps to Unix `128 + signal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    /// SIGINT / Ctrl+C → exit **130** (`128 + 2`).
    Interrupt,
    /// SIGTERM / Ctrl+Break → exit **143** (`128 + 15`).
    Terminate,
}

impl ShutdownReason {
    /// POSIX-style exit code for this signal.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Interrupt => exit_codes::CANCELLED,
            Self::Terminate => exit_codes::CANCELLED_SIGTERM,
        }
    }

    /// Human-readable label for stderr / tracing (not a wire protocol).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Interrupt => "SIGINT/Ctrl+C",
            Self::Terminate => "SIGTERM/Ctrl+Break",
        }
    }
}

/// Exit code of the last recorded cancel signal (130 default, 143 after SIGTERM).
#[must_use]
pub fn last_cancel_exit_code() -> i32 {
    // SeqCst: see `LAST_CANCEL_EXIT` module docs (total order vs signal store).
    LAST_CANCEL_EXIT.load(Ordering::SeqCst)
}

/// Resolve process exit code for a pipeline/deep-research error.
///
/// Signal-aware: [`CliError::Cancelled`] uses the recorded SIGINT/SIGTERM code
/// so Docker/`timeout`/supervisors that send SIGTERM get **143**, not 130.
#[must_use]
pub fn exit_code_for_error(err: &crate::error::CliError) -> i32 {
    match err {
        crate::error::CliError::Cancelled => last_cancel_exit_code(),
        other => other.exit_code(),
    }
}

fn record_cancel_exit(code: i32) {
    // SeqCst: pairs with `last_cancel_exit_code` load (see static docs).
    LAST_CANCEL_EXIT.store(code, Ordering::SeqCst);
}

/// Install process-wide cancel grace (CLI `--cancel-grace-secs`, clamped 1..=60).
pub fn set_cancel_grace_secs(secs: u64) {
    let clamped = secs.clamp(1, 60);
    CANCEL_GRACE_SECS.store(clamped, Ordering::SeqCst);
}

/// Effective cancel grace period (default 5s, optional CLI override 1..=60).
#[must_use]
pub fn cancel_force_exit_secs() -> u64 {
    let v = CANCEL_GRACE_SECS.load(Ordering::SeqCst);
    if (1..=60).contains(&v) {
        v
    } else {
        CANCEL_FORCE_EXIT_SECS
    }
}

/// Configures SIGPIPE handling for one-shot agent pipes (GAP-E2E-51-001 / 51-007).
///
/// **Deliberately keeps** the Rust default `SIG_IGN` for SIGPIPE on Unix.
///
/// Restoring `SIG_DFL` (classic Unix tools) kills the process on the first write
/// to a closed pipe **before** `Drop` / [`crate::process_lifecycle::ExitReapGuard`]
/// can reap Chrome `ddg-chrome-*` sessions — leaving process/disk orphans.
///
/// With `SIG_IGN`, writes return `EPIPE` → [`crate::error::CliError::BrokenPipe`]
/// → exit **141**, and cleanup paths still run. Call sites must map BrokenPipe
/// to 141 (already done in `lib::run` / emit helpers).
///
/// The function name is retained for call-site BC (`main` still calls it once).
#[cfg(unix)]
pub fn restore_sigpipe() {
    // Keep SIG_IGN (Rust runtime default). Explicit no-op documents the policy:
    // one-shot reap > SIG_DFL process death on pipe close.
    // If a future platform ever resets SIGPIPE, re-install SIG_IGN here:
    // unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN); }
}

/// No-op on Windows — SIGPIPE does not exist.
#[cfg(not(unix))]
pub fn restore_sigpipe() {}

/// Spawns an async task that awaits the first cancel signal and drives one-shot shutdown.
///
/// # Platforms
/// - **Unix:** `ctrl_c` (SIGINT) + `SignalKind::terminate` (SIGTERM)
/// - **Windows:** `ctrl_c` + `ctrl_break`
/// - **Other:** `ctrl_c` only
///
/// # Phases (detect → signal → await)
/// 1. First signal → record exit code (130/143), `cancellation.cancel()`, stderr notice
/// 2. Grace window ([`cancel_force_exit_secs`]) — pipeline may return cooperatively
/// 3. Second signal **or** grace expiry → `reap_all_registered` + `process::exit(code)`
pub fn install_cancellation_handler(cancellation: CancellationToken) {
    // JoinHandle intentionally not stored (fire-and-forget for process lifetime).
    tokio::spawn(async move {
        let Some(reason) = wait_first_shutdown_signal().await else {
            return;
        };
        cancel_then_force_exit(cancellation, reason).await;
    });
}

/// Wait for the first portable cancel signal (central detector).
async fn wait_first_shutdown_signal() -> Option<ShutdownReason> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(?err, "failed to install SIGTERM handler — Ctrl+C only");
                return wait_ctrl_c_as(ShutdownReason::Interrupt).await;
            }
        };
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(err) = result {
                    tracing::warn!(?err, "failed to install ctrl+c handler");
                    return None;
                }
                tracing::warn!(
                    signal = "SIGINT",
                    exit = ShutdownReason::Interrupt.exit_code(),
                    "SIGINT/Ctrl+C received — cancelling in-flight tasks (one-shot shutdown)"
                );
                Some(ShutdownReason::Interrupt)
            }
            _ = sigterm.recv() => {
                tracing::warn!(
                    signal = "SIGTERM",
                    exit = ShutdownReason::Terminate.exit_code(),
                    "SIGTERM received — cancelling in-flight tasks (one-shot shutdown)"
                );
                Some(ShutdownReason::Terminate)
            }
        }
    }

    #[cfg(windows)]
    {
        use tokio::signal::windows::{ctrl_break, ctrl_c};
        let mut break_stream = match ctrl_break() {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(?err, "failed to install Ctrl+Break handler — Ctrl+C only");
                return wait_ctrl_c_as(ShutdownReason::Interrupt).await;
            }
        };
        let mut ctrl_stream = match ctrl_c() {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(?err, "failed to install Ctrl+C handler");
                return None;
            }
        };
        tokio::select! {
            result = ctrl_stream.recv() => {
                if result.is_none() {
                    tracing::warn!("Ctrl+C stream closed without signal");
                    return None;
                }
                tracing::warn!(
                    signal = "Ctrl+C",
                    exit = ShutdownReason::Interrupt.exit_code(),
                    "Ctrl+C received — cancelling in-flight tasks (one-shot shutdown)"
                );
                Some(ShutdownReason::Interrupt)
            }
            result = break_stream.recv() => {
                if result.is_none() {
                    tracing::warn!("Ctrl+Break stream closed without signal");
                    return None;
                }
                tracing::warn!(
                    signal = "Ctrl+Break",
                    exit = ShutdownReason::Terminate.exit_code(),
                    "Ctrl+Break received — cancelling in-flight tasks (one-shot shutdown)"
                );
                Some(ShutdownReason::Terminate)
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        wait_ctrl_c_as(ShutdownReason::Interrupt).await
    }
}

async fn wait_ctrl_c_as(reason: ShutdownReason) -> Option<ShutdownReason> {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::warn!(?err, "failed to install ctrl+c handler");
        return None;
    }
    tracing::warn!(
        signal = reason.as_str(),
        exit = reason.exit_code(),
        "cancel signal received — cancelling in-flight tasks (one-shot shutdown)"
    );
    Some(reason)
}

/// Cancel cooperatively, then hard-exit after grace or on a second signal.
async fn cancel_then_force_exit(cancellation: CancellationToken, reason: ShutdownReason) {
    let exit_code = reason.exit_code();
    record_cancel_exit(exit_code);
    cancellation.cancel();

    let grace = cancel_force_exit_secs();
    // Prefer stderr so operators see feedback even when tracing is quiet (-q).
    // MP-06: route human stderr through output::emit_stderr (no raw eprintln!).
    let exit_s = exit_code.to_string();
    let grace_s = grace.to_string();
    crate::output::emit_stderr(crate::i18n::tf(
        crate::i18n::Message::CancelCooperativeStarted,
        &[
            ("signal", reason.as_str()),
            ("exit", &exit_s),
            ("grace", &grace_s),
        ],
    ));

    let second_signal = wait_grace_or_second_signal(grace).await;
    if second_signal {
        crate::output::emit_stderr(crate::i18n::tf(
            crate::i18n::Message::CancelSecondSignalForceExit,
            &[("exit", &exit_s)],
        ));
    } else {
        crate::output::emit_stderr(crate::i18n::tf(
            crate::i18n::Message::CancelGraceExpiredForceExit,
            &[("grace", &grace_s), ("exit", &exit_s)],
        ));
    }

    // ExitReapGuard Drop may not run on process::exit; reap Chrome/Xvfb explicitly.
    #[cfg(feature = "chrome")]
    crate::process_lifecycle::ensure_oneshot_cleanup();
    std::process::exit(exit_code);
}

/// Returns `true` if a second cancel signal arrived before the grace period elapsed.
async fn wait_grace_or_second_signal(grace_secs: u64) -> bool {
    let grace = Duration::from_secs(grace_secs);

    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).ok();
        tokio::select! {
            biased;
            result = tokio::signal::ctrl_c() => {
                if result.is_err() {
                    // Handler install failed — still wait remaining grace via sleep branch.
                    tokio::time::sleep(grace).await;
                    return false;
                }
                true
            }
            _ = async {
                match sigterm.as_mut() {
                    Some(s) => {
                        s.recv().await;
                    }
                    None => std::future::pending::<()>().await,
                }
            } => true,
            _ = tokio::time::sleep(grace) => false,
        }
    }

    #[cfg(windows)]
    {
        use tokio::signal::windows::{ctrl_break, ctrl_c};
        let mut break_stream = ctrl_break().ok();
        let mut ctrl_stream = ctrl_c().ok();
        tokio::select! {
            biased;
            result = async {
                match ctrl_stream.as_mut() {
                    Some(s) => s.recv().await.is_some(),
                    None => {
                        std::future::pending::<()>().await;
                        false
                    }
                }
            } => result,
            result = async {
                match break_stream.as_mut() {
                    Some(s) => s.recv().await.is_some(),
                    None => {
                        std::future::pending::<()>().await;
                        false
                    }
                }
            } => result,
            _ = tokio::time::sleep(grace) => false,
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        tokio::time::sleep(grace).await;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_sigpipe_does_not_panic() {
        restore_sigpipe();
    }

    #[tokio::test]
    async fn install_handler_does_not_panic() {
        let token = CancellationToken::new();
        install_cancellation_handler(token);
    }

    #[test]
    fn cancel_force_exit_grace_default_is_five_seconds() {
        // rules-rust-cli-one-shot / graceful-shutdown: 5s cleanup window.
        assert_eq!(CANCEL_FORCE_EXIT_SECS, 5);
        // Reset policy so parallel tests do not leave a non-default grace.
        set_cancel_grace_secs(CANCEL_FORCE_EXIT_SECS);
        assert_eq!(cancel_force_exit_secs(), 5);
    }

    #[test]
    fn cancel_grace_cli_policy_overrides_default() {
        set_cancel_grace_secs(12);
        assert_eq!(cancel_force_exit_secs(), 12);
        set_cancel_grace_secs(CANCEL_FORCE_EXIT_SECS);
    }

    #[test]
    fn shutdown_reason_exit_codes_follow_posix_128_plus_n() {
        assert_eq!(ShutdownReason::Interrupt.exit_code(), 130);
        assert_eq!(ShutdownReason::Terminate.exit_code(), 143);
        assert_eq!(exit_codes::CANCELLED, 130);
        assert_eq!(exit_codes::CANCELLED_SIGTERM, 143);
    }

    #[test]
    fn last_cancel_exit_defaults_to_sigint_convention() {
        // Fresh process / tests without a live signal: Cancelled → 130.
        // Note: other tests in this process may have recorded a value; only
        // assert that a valid cancel code is stored.
        let code = last_cancel_exit_code();
        assert!(
            code == exit_codes::CANCELLED || code == exit_codes::CANCELLED_SIGTERM,
            "unexpected last_cancel_exit_code={code}"
        );
    }

    #[test]
    fn record_and_read_cancel_exit_roundtrip() {
        record_cancel_exit(exit_codes::CANCELLED_SIGTERM);
        assert_eq!(last_cancel_exit_code(), 143);
        record_cancel_exit(exit_codes::CANCELLED);
        assert_eq!(last_cancel_exit_code(), 130);
    }

    #[test]
    fn exit_code_for_error_uses_recorded_signal_for_cancelled() {
        record_cancel_exit(exit_codes::CANCELLED_SIGTERM);
        assert_eq!(
            exit_code_for_error(&crate::error::CliError::Cancelled),
            143
        );
        record_cancel_exit(exit_codes::CANCELLED);
        assert_eq!(
            exit_code_for_error(&crate::error::CliError::Cancelled),
            130
        );
        assert_eq!(
            exit_code_for_error(&crate::error::CliError::BrokenPipe),
            exit_codes::BROKEN_PIPE
        );
    }

}
