// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure (Copy input struct for deep-research budget).
//! Inputs for deep-research wall-clock estimation (CLI + XDG resolved).

use crate::budget::contention::ContentionParams;
use crate::types::bounded::{
    BUDGET_FETCH_SECONDS_ESTIMATE, BUDGET_SAFETY_MARGIN_PERCENT, BUDGET_SERP_SECONDS_ESTIMATE,
};

/// Inputs for deep-research wall-clock estimation (CLI + XDG resolved).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepResearchBudgetInput {
    /// Maximum sub-queries (decomposition cap).
    pub max_sub_queries: usize,
    /// Whether nested content fetch is enabled.
    pub fetch_content: bool,
    /// Cap of URLs fetched per vertical when fetch is on.
    pub fetch_content_cap: usize,
    /// Dual web+news verticals requested (`!no_news`).
    pub dual_vertical: bool,
    /// Reflective depth rounds (`--depth`).
    pub depth: u32,
    /// SERP seconds per sub-query (SSOT or XDG override).
    pub serp_seconds: u64,
    /// Fetch seconds per URL (SSOT or XDG override).
    pub fetch_seconds: u64,
    /// Safety margin percent applied at the gate (SSOT or XDG override).
    pub margin_percent: u64,
    /// CLI/XDG parallelism (`-p` / `default_parallelism`).
    pub parallelism: u32,
    /// Observed chrome-like process count (0 = unknown / lab).
    pub chrome_n: u64,
    /// Operator forced shared-session verticals (disables dual multiproc).
    pub shared_session_verticals: bool,
    /// Contention thresholds (XDG-overridable).
    pub contention: ContentionParams,
}

impl DeepResearchBudgetInput {
    /// Build from resolved CLI values using built-in estimate constants.
    ///
    /// Defaults: `parallelism` = product default, `chrome_n` = 0, shared = false.
    #[must_use]
    pub fn from_cli(
        max_sub_queries: usize,
        fetch_content: bool,
        fetch_content_cap: usize,
        dual_vertical: bool,
        depth: u32,
    ) -> Self {
        Self {
            max_sub_queries,
            fetch_content,
            fetch_content_cap,
            dual_vertical,
            depth,
            serp_seconds: BUDGET_SERP_SECONDS_ESTIMATE,
            fetch_seconds: BUDGET_FETCH_SECONDS_ESTIMATE,
            margin_percent: BUDGET_SAFETY_MARGIN_PERCENT,
            parallelism: crate::types::bounded::DEFAULT_PARALLELISM,
            chrome_n: 0,
            shared_session_verticals: false,
            contention: ContentionParams::default(),
        }
    }

    /// Whether runtime dual multiproc web∥news is active (≠ `dual_vertical` alone).
    #[must_use]
    pub fn runtime_dual_multiproc(self) -> bool {
        self.dual_vertical
            && crate::concurrency::prefer_dual_vertical_chrome(
                self.parallelism,
                crate::concurrency::DualVerticalMode::Auto,
                self.shared_session_verticals,
            )
    }

    /// Chrome slots per sub-query under the query semaphore.
    #[must_use]
    pub fn slots_per_query(self) -> u32 {
        crate::concurrency::chrome_slots_per_query(
            true,
            self.dual_vertical,
            self.runtime_dual_multiproc(),
        )
    }
}
