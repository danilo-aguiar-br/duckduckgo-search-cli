// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared Criterion configuration for **latency-oriented** microbenches.
//!
//! Product context: wall-clock search is dominated by Chrome + network RTT.
//! These benches measure pure-CPU tails on hot helpers. Methodology:
//!
//! 1. Prefer **median (≈P50)** and Criterion's confidence interval over mean.
//! 2. Treat jitter / outliers as signal — do not discard without investigation.
//! 3. Always run under `[profile.bench]` (inherits fat LTO release).
//! 4. After a run, read estimates (local only — no CI thresholds):
//!    ```text
//!    jq '{median: .median.point_estimate, mean: .mean.point_estimate,
//!         std_dev: .std_dev.point_estimate}' \
//!      target/criterion/<bench_id>/new/estimates.json
//!    ```
//! 5. Mean alone is **not** the primary latency metric for this project.

use criterion::Criterion;
use std::time::Duration;

/// Criterion tuned for pure-CPU latency helpers (pre-flight, extract, decode).
///
/// - Larger sample size improves median stability.
/// - Short warm-up / measurement keep local runs practical (NO CI).
/// - `noise_threshold(0.05)` matches the project regression policy (~5%).
pub fn latency_criterion() -> Criterion {
    Criterion::default()
        .sample_size(200)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(3))
        .noise_threshold(0.05)
        .confidence_level(0.95)
        .significance_level(0.05)
}
