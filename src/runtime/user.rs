// SPDX-License-Identifier: MIT OR Apache-2.0
//! Typed view of XDG `config.toml` values (SSOT getters).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::cli::CliVertical;

/// In-memory view of XDG `config.toml` for runtime apply.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    /// Flat string key/value map (allowed keys in `ALLOWED_KEYS`).
    #[serde(default, flatten)]
    pub values: BTreeMap<String, String>,
}

impl UserConfig {
    /// Lookup a raw string value.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Parse `default_global_timeout` when present and valid (`1..=3600`).
    #[must_use]
    pub fn default_global_timeout(&self) -> Option<u64> {
        let raw = self.get("default_global_timeout")?;
        let n: u64 = raw.trim().parse().ok()?;
        if (1..=crate::types::bounded::MAX_GLOBAL_TIMEOUT_SECONDS).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Parse `default_vertical` (`web`|`news`|`all`).
    #[must_use]
    pub fn default_vertical(&self) -> Option<CliVertical> {
        match self
            .get("default_vertical")?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "web" => Some(CliVertical::Web),
            "news" => Some(CliVertical::News),
            "all" => Some(CliVertical::All),
            _ => None,
        }
    }

    /// Parse `fetch_content_default` (`true`/`false`/`1`/`0`/`on`/`off`).
    #[must_use]
    pub fn fetch_content_default(&self) -> Option<bool> {
        match self
            .get("fetch_content_default")?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    /// Optional Chrome binary path from XDG.
    #[must_use]
    pub fn chrome_path(&self) -> Option<PathBuf> {
        let raw = self.get("chrome_path")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(PathBuf::from(raw))
        }
    }

    /// Optional proxy URL from XDG.
    #[must_use]
    pub fn proxy_url(&self) -> Option<String> {
        let raw = self.get("proxy_url")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw.to_string())
        }
    }

    /// Optional tracing filter directive (replaces product `RUST_LOG`).
    #[must_use]
    pub fn log_directive(&self) -> Option<&str> {
        let raw = self.get("log_directive")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    }

    /// Optional UI language override when CLI `--ui-lang` omitted.
    #[must_use]
    pub fn ui_lang(&self) -> Option<&str> {
        let raw = self.get("ui_lang")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    }

    /// Optional SERP language default (`-l` / `--lang`) from XDG.
    #[must_use]
    pub fn default_lang(&self) -> Option<&str> {
        let raw = self.get("default_lang")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    }

    /// Optional SERP country default (`-c` / `--country`) from XDG.
    #[must_use]
    pub fn default_country(&self) -> Option<&str> {
        let raw = self.get("default_country")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    }

    /// Optional default `--max-sub-queries` for deep-research (1..=12).
    #[must_use]
    pub fn default_max_sub_queries(&self) -> Option<usize> {
        let n: usize = self.get("default_max_sub_queries")?.trim().parse().ok()?;
        if (1..=crate::deep_research::MAX_SUB_QUERIES).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Optional default `--fetch-content-cap` (1..=50).
    #[must_use]
    pub fn default_fetch_content_cap(&self) -> Option<usize> {
        let n: usize = self.get("default_fetch_content_cap")?.trim().parse().ok()?;
        if (1..=50).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Optional default `-p` / `--max-concurrency` (`1..=MAX_PARALLELISM`).
    ///
    /// Applied only when the CLI flag is still at the built-in default
    /// ([`crate::cli::DEFAULT_PARALLELISM`]) — explicit `-p N` always wins
    /// (GAP-XDG-DEFAULT-PARALLEL).
    #[must_use]
    pub fn default_parallelism(&self) -> Option<u32> {
        let n: u32 = self.get("default_parallelism")?.trim().parse().ok()?;
        if (1..=crate::types::bounded::MAX_PARALLELISM).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Optional Chrome session launch retry budget (`0..=MAX_CHROME_SESSION_RETRIES`).
    ///
    /// Applied only when CLI `--chrome-session-retries` is still at the built-in
    /// default (GAP-E2E-V11-CHROME-FLAKY / V12).
    #[must_use]
    pub fn chrome_session_retries(&self) -> Option<u32> {
        let n: u32 = self.get("chrome_session_retries")?.trim().parse().ok()?;
        if n <= crate::error::MAX_CHROME_SESSION_RETRIES {
            Some(n)
        } else {
            None
        }
    }

    /// Optional deep-research under-budget escape hatch.
    #[must_use]
    pub fn deep_research_allow_under_budget(&self) -> Option<bool> {
        match self
            .get("deep_research_allow_under_budget")?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    /// Probe ceiling override, in seconds, validated against `1..=600`.
    ///
    /// The four probe ceilings were inline literals until v1.0.5. They are
    /// policy, and every other timeout policy in this product is tunable from
    /// XDG without a rebuild; these are now too. Out-of-range or unparseable
    /// values return `None` so the compiled default wins rather than a
    /// nonsense one.
    #[must_use]
    fn probe_seconds(&self, key: &str) -> Option<u64> {
        let n: u64 = self.get(key)?.trim().parse().ok()?;
        (1..=600).contains(&n).then_some(n)
    }

    /// Optional Chrome launch ceiling for `--probe` (seconds).
    #[must_use]
    pub fn probe_launch_timeout(&self) -> Option<u64> {
        self.probe_seconds("probe_launch_timeout_seconds")
    }

    /// Optional DOM extraction ceiling for `--probe` (seconds).
    #[must_use]
    pub fn probe_extract_timeout(&self) -> Option<u64> {
        self.probe_seconds("probe_extract_timeout_seconds")
    }

    /// Optional Chrome launch ceiling for `--probe-deep` (seconds).
    #[must_use]
    pub fn probe_deep_launch_timeout(&self) -> Option<u64> {
        self.probe_seconds("probe_deep_launch_timeout_seconds")
    }

    /// Optional DOM extraction ceiling for `--probe-deep` (seconds).
    #[must_use]
    pub fn probe_deep_extract_timeout(&self) -> Option<u64> {
        self.probe_seconds("probe_deep_extract_timeout_seconds")
    }

    /// Optional SERP seconds estimate override for budget gate.
    #[must_use]
    pub fn budget_serp_seconds(&self) -> Option<u64> {
        let n: u64 = self.get("budget_serp_seconds")?.trim().parse().ok()?;
        if (1..=600).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Optional fetch seconds estimate override for budget gate.
    #[must_use]
    pub fn budget_fetch_seconds(&self) -> Option<u64> {
        let n: u64 = self.get("budget_fetch_seconds")?.trim().parse().ok()?;
        if (1..=600).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Optional budget safety margin percent (0..=100).
    #[must_use]
    pub fn budget_safety_margin_percent(&self) -> Option<u64> {
        let n: u64 = self
            .get("budget_safety_margin_percent")?
            .trim()
            .parse()
            .ok()?;
        if n <= 100 {
            Some(n)
        } else {
            None
        }
    }

    /// Chrome_n low threshold for contention factor 1.0.
    #[must_use]
    pub fn budget_contention_low(&self) -> Option<u64> {
        let n: u64 = self.get("budget_contention_low")?.trim().parse().ok()?;
        if (1..=10_000).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Chrome_n high threshold for high contention factor.
    #[must_use]
    pub fn budget_contention_high(&self) -> Option<u64> {
        let n: u64 = self.get("budget_contention_high")?.trim().parse().ok()?;
        if (1..=10_000).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Mid-band factor percent (e.g. 200 = 2.0×).
    #[must_use]
    pub fn budget_contention_factor_mid_percent(&self) -> Option<u64> {
        let n: u64 = self
            .get("budget_contention_factor_mid_percent")?
            .trim()
            .parse()
            .ok()?;
        if (100..=1000).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// High-band factor percent (e.g. 250 = 2.5×).
    #[must_use]
    pub fn budget_contention_factor_high_percent(&self) -> Option<u64> {
        let n: u64 = self
            .get("budget_contention_factor_high_percent")?
            .trim()
            .parse()
            .ok()?;
        if (100..=1000).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Auto-raise global timeout under contention (CLI-AUTO-01).
    #[must_use]
    pub fn deep_research_auto_contention_budget(&self) -> Option<bool> {
        match self
            .get("deep_research_auto_contention_budget")?
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    /// Timeout partial harvest grace seconds.
    #[must_use]
    pub fn deep_research_timeout_grace_seconds(&self) -> Option<u64> {
        let n: u64 = self
            .get("deep_research_timeout_grace_seconds")?
            .trim()
            .parse()
            .ok()?;
        if (1..=300).contains(&n) {
            Some(n)
        } else {
            None
        }
    }

    /// Budget profile name (`lab` | `desktop_contended`).
    #[must_use]
    pub fn budget_profile(&self) -> Option<&str> {
        let raw = self.get("budget_profile")?.trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    }
}
