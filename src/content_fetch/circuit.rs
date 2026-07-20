// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure coordination (no I/O). Per-host circuit breaker for content fetch.
//! Per-host circuit breaker shared across parallel content-fetch tasks (WS-12).
//!
//! After [`CB_FAILURE_THRESHOLD`] consecutive failures against a host, the breaker
//! is OPEN for [`CB_COOLDOWN`]; during cooldown, requests are rejected without a
//! network round-trip. A single success resets the counter.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Number of consecutive failures before opening the circuit.
pub(super) const CB_FAILURE_THRESHOLD: u32 = 3;
/// How long the breaker stays OPEN before allowing a probe request.
pub(super) const CB_COOLDOWN: Duration = Duration::from_secs(30);

/// Internal state for one host in the circuit breaker.
#[derive(Debug, Clone, Copy, Default)]
pub enum BreakerState {
    /// No recent failures — requests flow normally.
    #[default]
    Closed,
    /// Cooldown window — all requests are short-circuited until the
    /// `Instant` recorded in `until` elapses.
    Open {
        /// Absolute time at which the cooldown elapses.
        until: Instant,
    },
}

/// Per-host circuit breaker entry. The `failure_count` is reset to zero on
/// every success; reaching the threshold flips the state to `Open`.
#[derive(Debug, Clone)]
pub struct BreakerEntry {
    state: BreakerState,
    failure_count: u32,
}

impl Default for BreakerEntry {
    fn default() -> Self {
        Self {
            state: BreakerState::Closed,
            failure_count: 0,
        }
    }
}

/// Map `host → BreakerEntry` shared across parallel fetches.
///
/// Wrapped in a newtype (rather than a type alias) so we can define inherent
/// `impl` methods — Rust's orphan rule forbids inherent impls on
/// `Arc<std::sync::Mutex<...>>` because both types are foreign. The wrapped
/// `std::sync::Mutex` is held only for short critical sections (state lookup
/// and update), never across `.await` points — this avoids the `Send`
/// constraint of `tokio::sync::Mutex` and is sufficient because the lock is
/// uncontended in the common path.
#[derive(Clone, Debug)]
pub struct CircuitBreakerMap(Arc<std::sync::Mutex<HashMap<String, BreakerEntry>>>);

/// Outcome of a `check_and_record_*` call on the breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerDecision {
    /// Breaker is Closed (or cooldown elapsed) — proceed with the request.
    Allow,
    /// Breaker is Open — short-circuit the request to avoid hammering a
    /// known-failing host.
    Reject,
}

impl CircuitBreakerMap {
    /// Creates a new empty breaker map.
    pub fn new() -> Self {
        Self(Arc::new(std::sync::Mutex::new(HashMap::new())))
    }

    /// Private lock helper — recovers from poison so a prior panic in another
    /// task cannot abort enrichment (RAII / interior-mutability rules).
    ///
    /// Encapsulated: public API is only [`Self::check`], [`Self::record_success`],
    /// and [`Self::record_failure`]. Never holds the lock across `.await`.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, BreakerEntry>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Test-only introspection without exposing the mutex guard publicly.
    /// Test-only host presence check (GAP-SCRAPE-R-001 split).
    #[cfg(test)]
    pub(super) fn contains_host(&self, host: &str) -> bool {
        self.lock().contains_key(host)
    }

    /// Returns `Allow` if the host may receive a request, `Reject` otherwise.
    ///
    /// Side effect: if the breaker is `Open` and the cooldown window has
    /// elapsed, the entry is reset to `Closed` (half-open probe).
    pub fn check(&self, host: &str) -> BreakerDecision {
        let mut map = self.lock();
        let Some(entry) = map.get_mut(host) else {
            return BreakerDecision::Allow;
        };
        match entry.state {
            BreakerState::Closed => BreakerDecision::Allow,
            BreakerState::Open { until } => {
                if Instant::now() >= until {
                    // Half-open: reset to Closed and let one probe through.
                    entry.state = BreakerState::Closed;
                    entry.failure_count = 0;
                    BreakerDecision::Allow
                } else {
                    BreakerDecision::Reject
                }
            }
        }
    }

    /// Records a successful fetch for `host` — resets the failure counter
    /// and returns the breaker to `Closed`.
    pub fn record_success(&self, host: &str) {
        let mut map = self.lock();
        if let Some(entry) = map.get_mut(host) {
            entry.state = BreakerState::Closed;
            entry.failure_count = 0;
        }
    }

    /// Records a failed fetch for `host`. After `FAILURE_THRESHOLD` consecutive
    /// failures, the breaker opens for `COOLDOWN` duration.
    pub fn record_failure(&self, host: &str) {
        let mut map = self.lock();
        let entry = map.entry(host.to_string()).or_default();
        entry.failure_count = entry.failure_count.saturating_add(1);
        if entry.failure_count >= CB_FAILURE_THRESHOLD {
            entry.state = BreakerState::Open {
                until: Instant::now() + CB_COOLDOWN,
            };
        }
    }
}

impl Default for CircuitBreakerMap {
    fn default() -> Self {
        Self::new()
    }
}
