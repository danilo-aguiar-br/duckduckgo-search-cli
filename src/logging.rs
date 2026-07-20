// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: **I/O sink / coordination** (no work fan-out).
// Installs the process-wide tracing subscriber once; modules only emit events.
// Parallelism of search/fetch is elsewhere — this module must stay sequential.
//! Local stderr **logging** for the CLI binary (developer diagnostics only).
//!
//! # Product policy — **no telemetry**
//!
//! - This module is **not** product telemetry, metrics export, analytics, or OTLP.
//! - **stderr only** — stdout is reserved for JSON/agent payload (`output` module).
//! - **No remote sinks** — see `INVERSIONS.md`.
//! - **No `tracing-appender` / `WorkerGuard`** — one-shot process (BORN→EXECUTE→DIE).
//! - **No `reload::Layer` admin endpoint** — no long-lived HTTP server.
//! - **Binary installs once**; library modules only *emit* events/spans
//!   (never call `set_global_default` outside this module).
//!
//! # Verbosity contract (no product env)
//!
//! Precedence: **`-q` > `-v`/`-vv` > XDG `log_directive` > default `info`**.
//! Product configuration must not rely on `RUST_LOG` (GAP-LOG-ENV-001);
//! use CLI flags or `config set log_directive <filter>`.
//!
//! | CLI flags | Directive |
//! |-----------|-----------|
//! | `-q`      | `off`     |
//! | `-vv`+    | `trace`   |
//! | `-v`      | `debug`   |
//! | (none)    | XDG `log_directive` or `info` |
//!
//! # Console feature
//!
//! With `--features console` and `RUSTFLAGS="--cfg tokio_unstable"`, a
//! `console-subscriber` layer is composed with the fmt layer so both
//! `tokio-console` and stderr logs work. Without that cfg, fmt-only init runs.

use tracing_subscriber::fmt;
use tracing_subscriber::EnvFilter;

/// Installs the global tracing subscriber (idempotent).
///
/// Safe to call from `run` and from subcommands that may re-enter init:
/// a second install is ignored (already-set global default).
///
/// # Side effects
///
/// - Writes to **stderr** only.
/// - Installs `tracing-log` `LogTracer` (via `try_init`) so dependency crates
///   that emit via the `log` facade appear as tracing events.
/// - After a successful first install, emits a `debug` confirmation of the
///   effective filter and may warn about the `timeout-cli` PATH shadow.
pub(crate) fn initialize_logging(
    verbose: u8,
    quiet: bool,
    disable_colors: bool,
    xdg_log_directive: Option<&str>,
) {
    let filter = build_env_filter(verbose, quiet, xdg_log_directive);
    let filter_desc = filter.to_string();

    let installed = install_subscriber(filter, disable_colors);

    // Events only after the subscriber is live (or already was).
    if installed {
        tracing::debug!(
            filter = %filter_desc,
            verbose,
            quiet,
            ansi = !disable_colors,
            "tracing subscriber installed (stderr, compact)"
        );
    }

    // GAP-NEW-001: timeout-cli Rust wrapper shadows GNU coreutils and can
    // intercept -v flags. Detect after subscriber install so the warn is visible.
    if std::env::var_os("CARGO_BIN_EXE_timeout").is_some() {
        tracing::warn!(
            "timeout-cli Rust crate detected as parent process; \
             use /usr/bin/timeout GNU coreutils to avoid -v flag interception. \
             Run scripts/detect-timeout-wrapper.sh to verify."
        );
    }
}

/// Public crate entry used by subcommands that need logging without going
/// through the full `run` path twice (idempotent).
pub(crate) fn initialize_logging_for_command(verbose: u8, quiet: bool, disable_colors: bool) {
    initialize_logging(verbose, quiet, disable_colors, None);
}

/// Builds the [`EnvFilter`] for the given verbosity flags and optional XDG directive.
///
/// Exposed for unit tests so filter mapping stays regression-locked without
/// installing a global subscriber.
pub(crate) fn build_env_filter(
    verbose: u8,
    quiet: bool,
    xdg_log_directive: Option<&str>,
) -> EnvFilter {
    if quiet {
        // GAP-WS-QUIET-CONFIG-001: silence all tracing including ERROR.
        // Agent pipelines use -q so stderr stays empty for JSON parsers.
        return EnvFilter::new("off");
    }
    if verbose >= 2 {
        return EnvFilter::new("trace");
    }
    if verbose >= 1 {
        return EnvFilter::new("debug");
    }
    if let Some(directive) = xdg_log_directive {
        let trimmed = directive.trim();
        if !trimmed.is_empty() {
            return EnvFilter::new(trimmed);
        }
    }
    EnvFilter::new("info")
}

/// Returns `true` if this call installed the global default; `false` if one
/// was already present (or install failed for any other reason).
fn install_subscriber(filter: EnvFilter, disable_colors: bool) -> bool {
    #[cfg(all(feature = "console", tokio_unstable))]
    {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        let fmt_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(true)
            .with_thread_names(true)
            .with_ansi(!disable_colors)
            .compact();
        let console_layer = console_subscriber::spawn();
        return tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(console_layer)
            .try_init()
            .is_ok();
    }

    // Production / default path: fmt subscriber + LogTracer via try_init.
    #[cfg(not(all(feature = "console", tokio_unstable)))]
    {
        fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            // Targets enable `RUST_LOG=duckduckgo_search_cli::search=debug`.
            .with_target(true)
            // Multi-thread tokio: name helps correlate stderr with tasks.
            .with_thread_names(true)
            .with_ansi(!disable_colors)
            .compact()
            .try_init()
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_forces_off_even_if_verbose_set() {
        let f = build_env_filter(3, true, Some("trace"));
        assert_eq!(f.to_string(), "off");
    }

    #[test]
    fn xdg_directive_used_when_no_verbose() {
        let f = build_env_filter(0, false, Some("duckduckgo_search_cli=debug"));
        assert!(f.to_string().contains("debug"));
    }

    #[test]
    fn verbose_wins_over_xdg() {
        let f = build_env_filter(1, false, Some("info"));
        assert_eq!(f.to_string(), "debug");
    }

    #[test]
    fn default_verbosity_is_info() {
        let f = build_env_filter(0, false, None);
        assert_eq!(f.to_string(), "info");
    }

    #[test]
    fn verbose_one_is_debug() {
        let f = build_env_filter(1, false, None);
        assert_eq!(f.to_string(), "debug");
    }

    #[test]
    fn verbose_two_plus_is_trace() {
        let f = build_env_filter(2, false, None);
        assert_eq!(f.to_string(), "trace");
        let f3 = build_env_filter(5, false, None);
        assert_eq!(f3.to_string(), "trace");
    }

    #[test]
    fn initialize_logging_is_idempotent() {
        // First call may install; second must not panic.
        initialize_logging(0, false, true, None);
        initialize_logging(1, false, true, None);
        initialize_logging(0, true, true, None);
    }
}

