// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure CPU (no I/O) — contention factor math.
//! Host Chrome contention factor for deep-research wall-clock budget (CLI-BUDGET-01).

use crate::types::bounded::{
    BUDGET_CONTENTION_FACTOR_HIGH_PERCENT, BUDGET_CONTENTION_FACTOR_MID_PERCENT,
    BUDGET_CONTENTION_HIGH, BUDGET_CONTENTION_LOW,
};

/// Thresholds and factor percents for contention scaling (XDG-overridable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentionParams {
    /// chrome_n below this → factor 1.0 (100%).
    pub low: u64,
    /// chrome_n at/above this → high factor.
    pub high: u64,
    /// Mid-band factor as percent (200 = 2.0×).
    pub mid_percent: u64,
    /// High-band factor as percent (250 = 2.5×).
    pub high_percent: u64,
}

impl Default for ContentionParams {
    fn default() -> Self {
        Self {
            low: BUDGET_CONTENTION_LOW,
            high: BUDGET_CONTENTION_HIGH,
            mid_percent: BUDGET_CONTENTION_FACTOR_MID_PERCENT,
            high_percent: BUDGET_CONTENTION_FACTOR_HIGH_PERCENT,
        }
    }
}

/// Contention factor as integer percent of unit cost (100 = 1.0×, 250 = 2.5×).
///
/// Pure function — no I/O. Inject `chrome_n` from [`crate::process_count`].
#[must_use]
pub fn contention_factor_percent(chrome_n: u64, params: ContentionParams) -> u64 {
    let low = params.low;
    let high = params.high.max(low.saturating_add(1));
    if chrome_n < low {
        100
    } else if chrome_n < high {
        params.mid_percent.max(100)
    } else {
        params.high_percent.max(params.mid_percent).max(100)
    }
}

/// Scale `seconds` by factor percent with ceiling: `ceil(seconds * percent / 100)`.
#[must_use]
pub fn scale_seconds_by_percent(seconds: u64, percent: u64) -> u64 {
    if seconds == 0 {
        return 0;
    }
    let p = percent.max(100);
    seconds.saturating_mul(p).div_ceil(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factor_bands() {
        let p = ContentionParams::default();
        assert_eq!(contention_factor_percent(0, p), 100);
        assert_eq!(contention_factor_percent(19, p), 100);
        assert_eq!(contention_factor_percent(20, p), 200);
        assert_eq!(contention_factor_percent(39, p), 200);
        assert_eq!(contention_factor_percent(40, p), 250);
        assert_eq!(contention_factor_percent(100, p), 250);
    }

    #[test]
    fn scale_ceil() {
        assert_eq!(scale_seconds_by_percent(10, 100), 10);
        assert_eq!(scale_seconds_by_percent(10, 250), 25);
        assert_eq!(scale_seconds_by_percent(8, 250), 20); // 20.0
        assert_eq!(scale_seconds_by_percent(1, 250), 3); // ceil 2.5
    }
}
