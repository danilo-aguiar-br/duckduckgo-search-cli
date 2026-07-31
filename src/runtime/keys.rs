// SPDX-License-Identifier: MIT OR Apache-2.0
//! XDG `config.toml` key allowlist (SSOT).
//!
//! All product knobs that may be persisted under the XDG config home must
//! appear here. CRUD (`config set/get/unset`) and runtime apply share this list.

/// Allowed keys for `config set/get/unset` (SSOT; extend carefully).
pub const ALLOWED_KEYS: &[&str] = &[
    "ui_lang",
    "chrome_path",
    "proxy_url",
    "default_global_timeout",
    "default_vertical",
    "fetch_content_default",
    "log_directive",
    "default_lang",
    "default_country",
    // v1.0.2 deep-research budget contract (GAP-AUD-DR-006 / CM-12)
    "default_max_sub_queries",
    "default_fetch_content_cap",
    "deep_research_allow_under_budget",
    "budget_serp_seconds",
    "budget_fetch_seconds",
    "budget_safety_margin_percent",
    "budget_contention_low",
    "budget_contention_high",
    "budget_contention_factor_mid_percent",
    "budget_contention_factor_high_percent",
    "deep_research_auto_contention_budget",
    "deep_research_timeout_grace_seconds",
    "budget_profile",
    // v1.0.2 GAP-XDG-DEFAULT-PARALLEL: persist `-p` / `--max-concurrency` default
    "default_parallelism",
    // v1.0.2 GAP-E2E-V11-CHROME-FLAKY: cold-start session retry budget
    "chrome_session_retries",
    // v2.0.0 agent-native post-SERP ops defaults (G9/G10/G13)
    "default_sort",
    "default_dedupe_by",
    "max_output_bytes",
    "default_content_truncate",
    "allow_no_warmup",
    "linux_cgroup_enabled",
    "linux_cgroup_memory_max_mb",
    // v2.0.0 V30 operational defaults (CLI > XDG > FACTORY)
    "default_timeout",
    "default_retries",
    "default_pages",
    "default_num_results",
    "default_max_content_length",
    "default_per_host_limit",
    "default_cancel_grace_secs",
    "wire_keys",
];
