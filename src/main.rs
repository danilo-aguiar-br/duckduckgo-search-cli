// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: **runtime orchestrator** (not a fan-out site).
// - Builds Tokio multi_thread runtime (workers + max_blocking_threads).
// - Fan-out admission lives in parallel/content_fetch/pipeline (Semaphore).
// - CPU-bound work admits via concurrency::run_cpu_bound (GAP-PAR-030).
// - Process reap on exit: process_lifecycle (GAP-PAR-031 multi-session parallel).
//! Entry point for the `duckduckgo-search-cli` binary.
//!
//! **One-shot lifecycle** (rules-rust-cli-one-shot + graceful-shutdown):
//! BORN → EXECUTE → DIE in a single invocation. No daemon, no listen sockets,
//! no persistent queues. Shutdown is **minimal CLI** (detect → cancel → drain
//! → reap → exit), not a multi-subsystem server coordinator.
//!
//! This function does ONLY the minimum:
//! 1. Restores SIGPIPE via [`duckduckgo_search_cli::signals`].
//! 2. Installs one-shot Chrome orphan reap guard (feature `chrome`).
//! 3. Creates the root `CancellationToken` and installs the cancel handler:
//!    first SIGINT/SIGTERM (or Ctrl+C/Ctrl+Break) cancels cooperatively;
//!    second signal or grace expiry force-exits with **130** / **143**;
//!    grace defaults to 5s (override with CLI `--cancel-grace-secs`).
//! 4. Delegates to [`duckduckgo_search_cli::run()`] in `lib.rs`.
//! 5. Propagates the returned exit code to the operating system.
//!
//! ALL business logic lives in `lib.rs` and its submodules.

use std::process::ExitCode;
use tokio_util::sync::CancellationToken;

// Allocator: system (libc) by design — not mimalloc/jemalloc.
// Justification (resource-economy + latency rules — measure before adopting):
// - One-shot process: BORN → EXECUTE → DIE; no long-lived heap fragmentation.
// - Dominant RSS and wall-clock are Chrome + network RTT, not Rust heap churn.
// - Alternative allocators add dep weight and musl/cross-compile surface
//   without measured P99 win on this workload (see BENCHMARKS.md / NO_CI gates).
// - HFT-style mlockall / huge pages / isolcpus / PGO+BOLT are N/A for a
//   short-lived search CLI (see gaps.md N/A-LAT-*).
//
// Runtime (rules-rust parallel):
// - flavor = multi_thread (NOT current_thread): multi-query JoinSet + Chrome CDP
//   need concurrent polling across cores.
// - worker_threads = max(2, available_parallelism) via concurrency policy.
// - max_blocking_threads = clamp(workers*4, 16..=128) so spawn_blocking
//   (gzip/readability/SERP extract/synthesis) cannot grow to Tokio default 512.
// - Fan-out admission is still gated by Arc<Semaphore> in parallel/content_fetch;
//   CPU admits via blocking_cpu_semaphore / run_cpu_bound (GAP-PAR-030).
// - the runtime sizes the *scheduler*, the semaphore sizes *work admission*.

fn main() -> ExitCode {
    // Friendly panic report in release (human-panic). profile.release uses
    // panic="abort"; the hook still runs once. Installed before the tracing
    // subscriber because clap parse / early failures may panic before
    // `logging::initialize_logging`. process_lifecycle chains a panic reap
    // hook after Chrome starts. Service-style tracing-panic + WorkerGuard
    // flush are N/A for this one-shot binary (gaps.md N/A-LOG-*).
    human_panic::setup_panic!();

    // GAP-TLS-002 / ADR-0021: sole rustls CryptoProvider (aws-lc-rs) before any
    // Tokio worker or residual reqwest TLS (no-provider feature).
    if let Err(err) = duckduckgo_search_cli::tls_bootstrap::install_rustls_crypto_provider() {
        eprintln!("failed to install rustls crypto provider: {err}");
        return ExitCode::from(1);
    }

    duckduckgo_search_cli::signals::restore_sigpipe();

    // tokio-console: composed inside `logging::initialize_logging` when
    // built with `--features console` + `RUSTFLAGS=--cfg tokio_unstable`.
    // Do NOT call `console_subscriber::init()` here — it would race the
    // fmt subscriber and leave -v/-q ineffective.

    // GAP-WS-TMP-PROFILE-ORPHAN-001:
    // - new(): sweep orphan ddg-chrome-* from prior SIGKILL/OOM (never .tmp* /
    //   org.chromium.Chromium.*)
    // - Drop: reap registered sessions (process + disk)
    #[cfg(feature = "chrome")]
    let _exit_reap = duckduckgo_search_cli::process_lifecycle::ExitReapGuard::new();

    let workers = duckduckgo_search_cli::concurrency::runtime_worker_threads();
    let max_blocking = duckduckgo_search_cli::concurrency::max_blocking_threads();

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .max_blocking_threads(max_blocking)
        .enable_all()
        .thread_name("ddg-cli-worker")
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            // Last-resort: stderr before logging; process cannot run async work.
            eprintln!("failed to build tokio multi_thread runtime: {err}");
            return ExitCode::from(1);
        }
    };

    runtime.block_on(async_main())
}

async fn async_main() -> ExitCode {
    let cancellation = CancellationToken::new();
    duckduckgo_search_cli::signals::install_cancellation_handler(cancellation.clone());

    let exit_code = duckduckgo_search_cli::run(cancellation).await;
    ExitCode::from(exit_code as u8)
}
