// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: **policy / coordination** (not a fan-out site).
//
// This module is the single source of truth for bounded concurrency in the CLI.
// Hot paths that actually spawn work live in:
// - `parallel` — multi-query I/O fan-out (JoinSet + Semaphore)
// - `content_fetch` — per-URL content enrichment (JoinSet + Semaphore + per-host)
// - `deep_research` — decomposition → parallel fan-out → dual aggregation
// - `pipeline` — single-query dual vertical multi-process Chrome (web ∥ news)
//
// Bottleneck removed by fan-out: network RTT / Chrome SERP latency (I/O-bound).
// Resource saturated: outbound connections + DuckDuckGo rate limits + Chrome RSS.
// NOT saturated by default: local CPU (SERP parse is tiny vs RTT).
//
// ## Command × fan-out matrix (GAP-PAR-024 — modus operandi inventory)
//
// | Command / path              | Fan-out site              | Bound                         | Notes |
// |-----------------------------|---------------------------|-------------------------------|-------|
// | buscar multi-query          | parallel JoinSet          | Semaphore = effective         | slots/query 1 or 2 (dual) |
// | buscar single vertical=all  | pipeline dual Chrome      | budget ≥ 2 → 2 OS processes   | GAP-PAR-021 |
// | buscar --fetch-content      | content_fetch JoinSet     | global + per-host + pool      | nested pool=1 |
// | deep-research               | parallel + dual aggregate | effective + CPU semaphore     | |
// | SERP HTML parse (web/news)  | run_cpu_bound             | blocking_cpu_semaphore        | GAP-PAR-030 |
// | synthesis (deep stage 4)    | run_cpu_bound             | blocking_cpu_semaphore        | GAP-PAR-034 |
// | gzip/deflate body decode    | run_cpu_bound             | blocking_cpu_semaphore        | GAP-PAR-039 |
// | emit/format/stream serde    | run_cpu_bound (async emit)| blocking_cpu_semaphore        | GAP-PAR-040 |
// | probe / probe-deep          | none (1 URL)              | n/a                           | one-shot |
// | doctor / locale / schema /  | none                      | n/a                           | overhead > gain (N/A-PAR-020) |
// | completions / man / commands| none                      | n/a                           | pure emit (N/A-PAR-020) |
// | init-config                 | 2 file writes (scope)     | 2 threads                     | GAP-PAR-025 |
// | reap multi-session / sweep  | thread::scope (lifecycle) | available_parallelism         | GAP-PAR-031/037/041 |
// | /proc cmdline collect       | thread::scope (n≥32)      | available_parallelism         | GAP-PAR-041 |
//
// ## Permit formula (documented for rules-rust parallel checklist)
//
// ```text
// user_n     = --parallel / --max-concurrency  (CLI, default 5, clamp 1..=20)
// cpus       = std::thread::available_parallelism().get()
// free_ram   = MemAvailable from /proc/meminfo (Linux) or None elsewhere
// ram_task   ≈ 150 MiB (Chrome CDP process) or ≈ 5 MiB (HTTP harness residual)
// advisory_n = min(cpus, free_ram_mib * 50% / ram_task)   // soft / tracing only
// effective  = clamp(user_n, 1, MAX_PARALLELISM)            // hard gate
// pool_n     = min(effective, url_count) for multi-process Chrome fetch pool
// dual       = vertical all + budget≥2 + !--shared-session-verticals
// slots/query= 2 when dual web+news, else 1
// // multi-query: acquire_many(slots) so peak Chrome OS ≤ effective
// ```
//
// Dynamic CPU/RAM sizing is **intentionally not** the hard gate:
// 1. DuckDuckGo anti-bot is the real constraint (keep default ≤ 5).
// 2. Operators need a stable, explicit knob for agents (`--parallel N`).
// 3. Chrome RSS varies by OS; free-RAM is used only for **advisory** warnings.
//
// Measured RSS ground truth (local, `/usr/bin/time -v`, see BENCHMARKS.md):
// - single HTTP SERP task ≈ 2–5 MiB process delta
// - single Chrome SERP session ≈ 120–250 MiB peak RSS
// - 5 concurrent Chrome queries can approach ~1 GiB — hence MAX_PARALLELISM=20
//   is a hard ceiling, not a recommended default.
//
// Multi-processing:
// - multi-query SERP: one chromiumoxide process per admitted task (`parallel.rs`)
// - `--vertical all` dual: **two** Chromes (web ∥ news) when budget allows
//   (GAP-PAR-021); multi-query pays `acquire_many(2)` so peak ≤ effective
// - `--fetch-content`: Chrome **pool** sized to effective concurrency
// - **Invariant (GAP-PAR-016):** peak Chrome OS processes per invocation ≤
//   `effective_concurrency`. Nested multi-query × fetch must NOT multiply
//   pools (each query task uses pool_size=1 when nested under query fan-out).
// - CPU `spawn_blocking` (gzip/readability/aggregate/SERP extract/synthesis/emit):
//   gated by `blocking_cpu_semaphore` via [`run_cpu_bound`] (docs.rs + GAP-PAR-030/039/040).
// Utility subcommands (doctor/locale/schema/completions/man/commands)
// are intentionally sequential — no fan-out opportunity larger than overhead
// (N/A-PAR-020: JoinSet would dominate µs work).
// `init-config` is the only utility with multi-file I/O (parallel writes).
//! Bounded concurrency policy for the CLI.
//!
//! See module-level workload comments for the permit formula and trade-offs.

use crate::cli::{DEFAULT_PARALLELISM, MAX_PARALLELISM};
use crate::error::CliError;
use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};
use std::thread;
use tokio::sync::Semaphore;

/// Hard ceiling mirrored from CLI (`--parallel` / `--max-concurrency`).
pub const MAX_CONCURRENCY: u32 = MAX_PARALLELISM;

/// Default concurrency when the operator does not pass a flag.
pub const DEFAULT_CONCURRENCY: u32 = DEFAULT_PARALLELISM;

/// Approximate RSS (MiB) of one Chrome SERP task — used only for advisory logs.
///
/// Grounded in local `/usr/bin/time -v` samples (BENCHMARKS.md / NO_CI).
pub const CHROME_TASK_RSS_MIB: u64 = 150;

/// Approximate RSS (MiB) of one residual HTTP SERP task (test harness).
pub const HTTP_TASK_RSS_MIB: u64 = 5;

/// Returns the host CPU count (`available_parallelism`), defaulting to 1.
#[must_use]
pub fn cpu_count() -> usize {
    thread::available_parallelism()
        .map(NonZeroUsize::get)
        .unwrap_or(1)
}

/// Clamps a user-requested concurrency into the legal range `[1, MAX_CONCURRENCY]`.
#[must_use]
pub fn clamp_concurrency(requested: u32) -> u32 {
    requested.clamp(1, MAX_CONCURRENCY)
}

/// Effective permit count for fan-out sites.
///
/// Always honors the operator flag after clamp. CPU/RAM are **not** used as a
/// silent hard cap (see module docs); call [`advisory_cap_chrome`] for logs.
#[must_use]
pub fn effective_concurrency(user_parallelism: u32) -> u32 {
    clamp_concurrency(user_parallelism.max(1))
}

/// Reads free/available RAM in MiB from the host.
///
/// - Linux: `MemAvailable` from `/proc/meminfo` (falls back to `MemFree`).
/// - Other OS: `None` (no portable zero-dep probe; operator flag remains hard gate).
///
/// Used only for **advisory** concurrency warnings — never as a silent hard cap.
#[must_use]
pub fn free_ram_mib() -> Option<u64> {
    free_ram_mib_from_proc_meminfo(std::fs::read_to_string("/proc/meminfo").ok().as_deref())
}

/// Parses `/proc/meminfo` text. Prefer `MemAvailable` (accounts for reclaimable
/// cache); fall back to `MemFree`. Values are kB in the file → MiB here.
#[must_use]
pub fn free_ram_mib_from_proc_meminfo(contents: Option<&str>) -> Option<u64> {
    let text = contents?;
    let mut available_kb: Option<u64> = None;
    let mut free_kb: Option<u64> = None;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let key = parts.next()?;
        let value = parts.next()?.parse::<u64>().ok()?;
        match key {
            "MemAvailable:" => available_kb = Some(value),
            "MemFree:" => free_kb = Some(value),
            _ => {}
        }
    }
    let kb = available_kb.or(free_kb)?;
    Some(kb / 1024)
}

/// Soft upper bound for Chrome multi-process fan-out from CPUs **and** free RAM.
///
/// Formula (rules checklist, advisory only):
/// `min(cpus, floor((free_ram_mib * 50%) / CHROME_TASK_RSS_MIB))`, clamped to
/// `[1, MAX_CONCURRENCY]`. When RAM cannot be probed, CPU count alone is used.
///
/// Does **not** replace the user flag; used for tracing warnings only.
#[must_use]
pub fn advisory_cap_chrome() -> u32 {
    let cpu_cap = u32::try_from(cpu_count()).unwrap_or(1).max(1);
    let ram_cap = free_ram_mib().map_or(MAX_CONCURRENCY, |free| {
        // 50% safety margin on free RAM, then divide by measured Chrome task RSS.
        let usable = free / 2;
        let by_ram = usable / CHROME_TASK_RSS_MIB;
        u32::try_from(by_ram.max(1)).unwrap_or(1)
    });
    clamp_concurrency(cpu_cap.min(ram_cap).max(1))
}

/// Hard ceiling on simultaneous Chrome OS processes for one CLI invocation.
///
/// Equals [`effective_concurrency`]. SERP fan-out and content-fetch pools must
/// share this budget so nested multi-query × fetch cannot reach N² processes
/// (GAP-PAR-016). Measured RSS ≈ [`CHROME_TASK_RSS_MIB`] MiB per process.
#[must_use]
pub fn chrome_process_budget(user_parallelism: u32) -> usize {
    effective_concurrency(user_parallelism) as usize
}

/// How many Chrome OS processes to launch for a content-fetch pool.
///
/// # Arguments
/// * `user_parallelism` — `--parallel` / `--max-concurrency`
/// * `url_count` — URLs to enrich
/// * `nested_in_query_fanout` — `true` when called from multi-query tasks that
///   already hold one concurrency slot each (SERP). Forces `pool_size = 1` so
///   peak Chromes = in-flight queries ≤ budget, not `budget × pool`.
///
/// Single-query (`nested_in_query_fanout = false`):
/// `min(effective, url_count)`, at least 1 when there is work.
#[must_use]
pub fn chrome_pool_size(
    user_parallelism: u32,
    url_count: usize,
    nested_in_query_fanout: bool,
) -> usize {
    if url_count == 0 {
        return 0;
    }
    if nested_in_query_fanout {
        // GAP-PAR-016: multi-query tasks already multi-process SERP (1 Chrome
        // each under the query Semaphore). Enrich with 1 browser per task so
        // concurrent enrichments stay within chrome_process_budget.
        return 1;
    }
    let effective = chrome_process_budget(user_parallelism);
    effective.min(url_count).max(1)
}

/// How web + news SERPs are scheduled for `--vertical all` (GAP-PAR-021 / 027).
///
/// - [`Auto`](DualVerticalMode::Auto) — dual multi-process when budget ≥ 2 and
///   the operator did not force a shared session.
/// - [`ForceShared`](DualVerticalMode::ForceShared) — one Chrome, web then news
///   (lower RSS / anti-bot surface; higher wall-clock).
/// - [`ForceDual`](DualVerticalMode::ForceDual) — always two Chromes when both
///   verticals are requested (tests / explicit multi-process).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DualVerticalMode {
    /// Prefer dual multi-process when resource budget allows.
    #[default]
    Auto,
    /// Force one shared Chrome session (serial web → news).
    ForceShared,
    /// Force dual Chrome processes when both verticals are active.
    ForceDual,
}

/// Whether web and news SERPs should run as **two OS Chrome processes** in
/// parallel (GAP-PAR-021).
///
/// # Arguments
/// * `user_parallelism` — `--parallel` / `--max-concurrency`
/// * `mode` — [`DualVerticalMode`] override
/// * `shared_session_verticals` — CLI `--shared-session-verticals` (forces shared)
///
/// Dual requires `chrome_process_budget ≥ 2` unless `ForceDual` (still needs
/// both verticals at the call site). `ForceShared` and the CLI flag always
/// disable dual.
#[must_use]
pub fn prefer_dual_vertical_chrome(
    user_parallelism: u32,
    mode: DualVerticalMode,
    shared_session_verticals: bool,
) -> bool {
    if shared_session_verticals {
        return false;
    }
    match mode {
        DualVerticalMode::ForceShared => false,
        DualVerticalMode::ForceDual => true,
        DualVerticalMode::Auto => chrome_process_budget(user_parallelism) >= 2,
    }
}

/// Chrome OS process slots consumed by one query task under the query Semaphore.
///
/// When dual web+news multi-process is active, the task costs **2** permits so
/// peak Chromes across the JoinSet stays ≤ `effective` (GAP-PAR-021b / 016).
/// Otherwise 1 permit = 1 Chrome (shared session or single vertical).
#[must_use]
pub fn chrome_slots_per_query(includes_web: bool, includes_news: bool, dual: bool) -> u32 {
    if dual && includes_web && includes_news {
        2
    } else {
        1
    }
}

/// Process-wide admission gate for CPU-bound `spawn_blocking` work.
///
/// Tokio docs (spawn_blocking): when running many CPU-bound jobs, use a
/// semaphore — `max_blocking_threads` only sizes the pool, it does not admit.
/// Permits = `max(2, cpu_count)` so gzip/readability cannot fork-bomb the
/// blocking pool under multi-query + fetch-content.
#[must_use]
pub fn blocking_cpu_semaphore() -> Arc<Semaphore> {
    static SEM: OnceLock<Arc<Semaphore>> = OnceLock::new();
    Arc::clone(SEM.get_or_init(|| {
        let permits = cpu_count().max(2);
        Arc::new(Semaphore::new(permits))
    }))
}

/// Run CPU-bound work off the Tokio async worker (GAP-PAR-030 / M1).
///
/// # Admission
///
/// 1. Log `cpu_permits_available` (GAP-PAR-033).
/// 2. `acquire_owned` on [`blocking_cpu_semaphore`].
/// 3. `spawn_blocking` with the permit moved into the closure (RAII recovery
///    on panic — permit drops when the blocking task ends).
/// 4. Map [`tokio::task::JoinError`]: `is_panic` / `is_cancelled` / other.
///
/// Prefer this over ad-hoc `spawn_blocking` so every CPU fan-out shares one gate.
///
/// # Errors
///
/// - Semaphore closed (runtime shutdown).
/// - Blocking task panicked or was cancelled before start.
#[tracing::instrument(level = "debug", skip_all, fields(cpu_permits_available))]
pub async fn run_cpu_bound<F, T>(f: F) -> Result<T, CliError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let sem = blocking_cpu_semaphore();
    tracing::Span::current().record("cpu_permits_available", sem.available_permits());
    tracing::debug!(
        cpu_permits_available = sem.available_permits(),
        "CPU gate acquire (GAP-PAR-033)"
    );
    let permit = sem.acquire_owned().await.map_err(|e| CliError::NetworkError {
        message: format!("blocking CPU semaphore closed: {e}"),
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        f()
    })
    .await
    .map_err(|e| {
        if e.is_panic() {
            CliError::NetworkError {
                message: "CPU-bound task panicked".into(),
            }
        } else if e.is_cancelled() {
            CliError::Cancelled
        } else {
            CliError::NetworkError {
                message: format!("CPU-bound join failed: {e}"),
            }
        }
    })
}

/// Tokio multi-thread worker count for the process-wide runtime.
///
/// Matches host parallelism so JoinSet tasks and CDP futures schedule across
/// cores. Floor 2 so a single-core report still allows concurrent I/O polling.
#[must_use]
pub fn runtime_worker_threads() -> usize {
    cpu_count().max(2)
}

/// Cap for Tokio's `spawn_blocking` pool.
///
/// Content readability / gzip decode use `spawn_blocking`. Bound scales with
/// host CPUs but stays well below the Tokio default of 512 so a runaway
/// blocking fan-out cannot fork-bomb the machine.
#[must_use]
pub fn max_blocking_threads() -> usize {
    // Formula: max(16, workers * 4), hard ceiling 128.
    runtime_worker_threads().saturating_mul(4).clamp(16, 128)
}

/// Bounded mpsc capacity for streaming producers (backpressure).
///
/// Spec: `parallelism * 2`, minimum 2.
#[must_use]
pub fn stream_channel_capacity(parallelism: u32) -> usize {
    (effective_concurrency(parallelism) as usize)
        .saturating_mul(2)
        .max(2)
}

/// Emits a debug/trace advisory when Chrome multi-process fan-out may thrash.
pub fn log_chrome_concurrency_advisory(effective: u32) {
    let advisory = advisory_cap_chrome();
    let free = free_ram_mib();
    if effective > advisory {
        tracing::warn!(
            effective,
            advisory_cap = advisory,
            cpus = cpu_count(),
            free_ram_mib = free,
            chrome_task_rss_mib = CHROME_TASK_RSS_MIB,
            "Chrome fan-out concurrency exceeds advisory CPU/RAM cap — \
             each task is a separate browser process; reduce --parallel / \
             --max-concurrency if RSS or anti-bot pressure rises"
        );
    } else {
        tracing::debug!(
            effective,
            advisory_cap = advisory,
            cpus = cpu_count(),
            free_ram_mib = free,
            workers = runtime_worker_threads(),
            max_blocking = max_blocking_threads(),
            "concurrency policy applied"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_respects_bounds() {
        assert_eq!(clamp_concurrency(0), 1);
        assert_eq!(clamp_concurrency(1), 1);
        assert_eq!(clamp_concurrency(DEFAULT_CONCURRENCY), DEFAULT_CONCURRENCY);
        assert_eq!(clamp_concurrency(MAX_CONCURRENCY), MAX_CONCURRENCY);
        assert_eq!(clamp_concurrency(MAX_CONCURRENCY + 50), MAX_CONCURRENCY);
    }

    #[test]
    fn effective_never_zero() {
        assert!(effective_concurrency(0) >= 1);
        assert!(effective_concurrency(u32::MAX) <= MAX_CONCURRENCY);
    }

    #[test]
    fn stream_capacity_scales() {
        assert_eq!(stream_channel_capacity(1), 2);
        assert_eq!(stream_channel_capacity(5), 10);
        assert_eq!(stream_channel_capacity(0), 2);
    }

    #[test]
    fn runtime_sizes_are_sane() {
        let w = runtime_worker_threads();
        let b = max_blocking_threads();
        assert!(w >= 2);
        assert!((16..=128).contains(&b));
        assert!(b >= w);
    }

    #[test]
    fn cpu_count_at_least_one() {
        assert!(cpu_count() >= 1);
    }

    #[test]
    fn advisory_cap_in_range() {
        let a = advisory_cap_chrome();
        assert!((1..=MAX_CONCURRENCY).contains(&a));
    }

    #[test]
    fn free_ram_parses_memavailable() {
        let sample = "\
MemTotal:       16384000 kB
MemFree:         2048000 kB
MemAvailable:    8192000 kB
Buffers:          100000 kB
";
        assert_eq!(free_ram_mib_from_proc_meminfo(Some(sample)), Some(8000));
    }

    #[test]
    fn free_ram_falls_back_to_memfree() {
        let sample = "\
MemTotal:       16384000 kB
MemFree:         4096000 kB
";
        assert_eq!(free_ram_mib_from_proc_meminfo(Some(sample)), Some(4000));
    }

    #[test]
    fn free_ram_none_on_missing() {
        assert_eq!(free_ram_mib_from_proc_meminfo(None), None);
        assert_eq!(free_ram_mib_from_proc_meminfo(Some("garbage")), None);
    }

    #[test]
    fn chrome_pool_size_respects_work_and_flag() {
        assert_eq!(chrome_pool_size(5, 0, false), 0);
        assert_eq!(chrome_pool_size(5, 1, false), 1);
        assert_eq!(chrome_pool_size(5, 3, false), 3);
        assert_eq!(chrome_pool_size(5, 100, false), 5);
        assert_eq!(chrome_pool_size(0, 10, false), 1); // clamp to ≥1 effective
        assert_eq!(
            chrome_pool_size(50, 10, false),
            MAX_CONCURRENCY.min(10) as usize
        );
    }

    #[test]
    fn chrome_pool_nested_under_query_fanout_is_one() {
        // GAP-PAR-016: nested enrich must not multiply pool by effective.
        assert_eq!(chrome_pool_size(5, 100, true), 1);
        assert_eq!(chrome_pool_size(20, 10, true), 1);
        assert_eq!(chrome_pool_size(1, 1, true), 1);
        assert_eq!(chrome_pool_size(5, 0, true), 0);
    }

    #[test]
    fn chrome_process_budget_matches_effective() {
        assert_eq!(
            chrome_process_budget(5),
            effective_concurrency(5) as usize
        );
        assert_eq!(
            chrome_process_budget(0),
            effective_concurrency(0) as usize
        );
    }

    #[test]
    fn blocking_cpu_semaphore_has_at_least_two_permits() {
        let sem = blocking_cpu_semaphore();
        assert!(sem.available_permits() >= 2);
    }

    /// GAP-PAR-029: dual vertical policy and chrome slots (peak ≤ effective).
    #[test]
    fn prefer_dual_respects_budget_mode_and_shared_flag() {
        assert!(prefer_dual_vertical_chrome(
            5,
            DualVerticalMode::Auto,
            false
        ));
        assert!(!prefer_dual_vertical_chrome(
            1,
            DualVerticalMode::Auto,
            false
        ));
        assert!(!prefer_dual_vertical_chrome(
            5,
            DualVerticalMode::Auto,
            true
        ));
        assert!(!prefer_dual_vertical_chrome(
            5,
            DualVerticalMode::ForceShared,
            false
        ));
        assert!(prefer_dual_vertical_chrome(
            1,
            DualVerticalMode::ForceDual,
            false
        ));
        // Shared flag wins over ForceDual (operator escape hatch).
        assert!(!prefer_dual_vertical_chrome(
            5,
            DualVerticalMode::ForceDual,
            true
        ));
    }

    #[test]
    fn chrome_slots_dual_web_news_is_two() {
        assert_eq!(chrome_slots_per_query(true, true, true), 2);
        assert_eq!(chrome_slots_per_query(true, true, false), 1);
        assert_eq!(chrome_slots_per_query(true, false, true), 1);
        assert_eq!(chrome_slots_per_query(false, true, true), 1);
        assert_eq!(chrome_slots_per_query(false, false, true), 1);
    }

    /// With dual slots=2 and Semaphore(effective), peak concurrent queries
    /// is floor(effective/2) and peak Chromes ≤ effective.
    #[tokio::test]
    async fn dual_slots_acquire_many_peak_chrome_never_exceeds_budget() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        const EFFECTIVE: usize = 4;
        const SLOTS: u32 = 2;
        let sem = Arc::new(Semaphore::new(EFFECTIVE));
        let chromes = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut set = tokio::task::JoinSet::new();
        for _ in 0..8 {
            let sem = Arc::clone(&sem);
            let chromes = Arc::clone(&chromes);
            let peak = Arc::clone(&peak);
            set.spawn(async move {
                let _permit = sem
                    .acquire_many_owned(SLOTS)
                    .await
                    .expect("sem open");
                // Simulate dual Chrome: hold SLOTS "processes".
                let now = chromes.fetch_add(SLOTS as usize, Ordering::SeqCst) + SLOTS as usize;
                peak.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                chromes.fetch_sub(SLOTS as usize, Ordering::SeqCst);
            });
        }
        while set.join_next().await.is_some() {}
        let observed = peak.load(Ordering::SeqCst);
        assert!(
            observed <= EFFECTIVE,
            "dual peak chromes {observed} exceeded budget {EFFECTIVE}"
        );
        assert_eq!(sem.available_permits(), EFFECTIVE);
    }

    /// GAP-PAR-014: peak concurrent holders of a global gate never exceed N.
    #[tokio::test]
    async fn global_semaphore_peak_never_exceeds_n() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        const N: usize = 3;
        let sem = Arc::new(Semaphore::new(N));
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut set = tokio::task::JoinSet::new();
        for _ in 0..20 {
            let sem = Arc::clone(&sem);
            let concurrent = Arc::clone(&concurrent);
            let peak = Arc::clone(&peak);
            set.spawn(async move {
                let _permit = sem.acquire_owned().await.expect("sem open");
                let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                concurrent.fetch_sub(1, Ordering::SeqCst);
            });
        }
        while set.join_next().await.is_some() {}
        let observed = peak.load(Ordering::SeqCst);
        assert!(
            observed <= N,
            "global peak {observed} exceeded admit limit {N}"
        );
        assert_eq!(sem.available_permits(), N);
    }

    /// GAP-PAR-019: panic inside a task still returns the permit via RAII Drop.
    #[tokio::test]
    async fn global_semaphore_permit_recovered_after_task_panic() {
        const N: usize = 2;
        let sem = Arc::new(Semaphore::new(N));
        let sem_task = Arc::clone(&sem);
        let handle = tokio::spawn(async move {
            let _permit = sem_task.acquire_owned().await.expect("sem open");
            panic!("boom — permit must drop on unwind");
        });
        let join_err = handle.await.expect_err("task must panic");
        assert!(join_err.is_panic());
        // Yield so Drop of the permit runs before we sample.
        tokio::task::yield_now().await;
        assert_eq!(
            sem.available_permits(),
            N,
            "permit must be recovered after panic"
        );
    }

    /// GAP-PAR-030 / M2: concurrent `run_cpu_bound` never exceeds CPU gate permits.
    #[tokio::test]
    async fn run_cpu_bound_peak_never_exceeds_cpu_gate() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        // Capacity is `cpu_count().max(2)` at process init — NOT
        // `available_permits()` at test start (other tests may hold permits,
        // so a snapshot of available can be lower than true capacity and
        // peak can then appear to "exceed" the snapshot).
        let capacity = cpu_count().max(2);
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut set = tokio::task::JoinSet::new();
        for _ in 0..(capacity * 4).max(8) {
            let concurrent = Arc::clone(&concurrent);
            let peak = Arc::clone(&peak);
            set.spawn(async move {
                run_cpu_bound(move || {
                    let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    concurrent.fetch_sub(1, Ordering::SeqCst);
                    1u32
                })
                .await
                .expect("cpu bound ok")
            });
        }
        while set.join_next().await.is_some() {}
        let observed = peak.load(Ordering::SeqCst);
        assert!(
            observed <= capacity,
            "run_cpu_bound peak {observed} exceeded CPU gate capacity {capacity}"
        );
        // GAP-PAR-037: do not assert absolute available_permits on the
        // process-wide gate (other tests may hold permits concurrently).
    }

    /// GAP-PAR-030 / M3: panic inside `run_cpu_bound` recovers the CPU permit.
    ///
    /// Does not assert absolute `available_permits` on the process-wide gate
    /// (other tests may hold permits concurrently). Instead, proves recovery
    /// by successfully admitting a follow-up `run_cpu_bound`.
    #[tokio::test]
    async fn run_cpu_bound_permit_recovered_after_panic() {
        let err = run_cpu_bound(|| -> u32 {
            panic!("cpu boom");
        })
        .await
        .expect_err("must surface panic as CliError");
        assert!(
            matches!(err, crate::error::CliError::NetworkError { .. }),
            "panic maps to NetworkError, got {err:?}"
        );
        // Follow-up work must still acquire — proves RAII recovered the permit
        // even under concurrent use of the process-wide CPU gate.
        let v = run_cpu_bound(|| 7u32)
            .await
            .expect("post-panic acquire must succeed");
        assert_eq!(v, 7);
    }
}
