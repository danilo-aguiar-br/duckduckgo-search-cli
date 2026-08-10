// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: **diagnostic / sequential by design** (I/O-bound one-shot).
// No fan-out: chrome detect + path probes are local I/O; JoinSet/Semaphore overhead
// would dominate for N=1. Runs under the process-wide multi_thread Tokio runtime
// but does not spawn parallel tasks.
//
// GAP-PAR-043: `chrome --version` probe is **blocking** (`std::process::Command` +
// `std::thread::sleep` poll). From this async handler we call
// `detect_chrome_major_version_async` → `run_cpu_bound` so Tokio workers are not
// stalled (rules-rust: never `std::thread::sleep` on the async executor).
// Parallelism modus operandi for search / fetch / deep-research is elsewhere.
//! Handler for the `doctor` subcommand — environment diagnostics as JSON.
//!
//! # `--strict` (OPP-DOCTOR-STRICT)
//!
//! Chrome major detection is available via
//! [`crate::browser::detect_chrome_major_version_async`] (offloads the blocking
//! `--version` probe; GAP-PAR-043). With `--strict`:
//!
//! - exit **non-zero** when Chrome is **not** detected (or feature disabled);
//! - exit **non-zero** when the detected major is **wildly ahead** of the
//!   chromiumoxide PDL baseline ([`CHROMIUMOXIDE_PDL_BASELINE_MAJOR`] +
//!   [`CHROME_MAJOR_WILDLY_AHEAD_DELTA`]);
//! - if Chrome is found but `--version` cannot be parsed, strict does **not**
//!   invent a failure (cannot prove “wildly ahead”).
//!
//! JSON stdout remains agent-stable: existing keys are preserved; additive
//! fields (`strict`, `chrome.major_version`, check `chrome_pdl_compat`) may
//! appear. No product environment variables are introduced.

use crate::cli::DoctorArgs;
use crate::error::exit_codes;
use crate::output;
use crate::platform::{self, RuntimeEnvironment};
use serde::Serialize;

/// Chromiumoxide 0.9.x PDL generation / identity fallback baseline (Chrome major).
///
/// Used by `doctor --strict` to judge whether the host Chrome is “wildly ahead”
/// of the protocol surface the crate was generated against. Not a hard pin of
/// supported browsers at runtime (CDP still runs; InvalidMessage noise is the
/// residual — see GAP-E2E-48-011).
pub const CHROMIUMOXIDE_PDL_BASELINE_MAJOR: u32 = 146;

/// Majors more than this many above [`CHROMIUMOXIDE_PDL_BASELINE_MAJOR`] are
/// treated as “wildly ahead” under `doctor --strict`.
pub const CHROME_MAJOR_WILDLY_AHEAD_DELTA: u32 = 20;

/// Returns `true` when `major` is more than
/// [`CHROME_MAJOR_WILDLY_AHEAD_DELTA`] above the PDL baseline.
#[must_use]
pub fn chrome_major_wildly_ahead_of_pdl(major: u32) -> bool {
    major > CHROMIUMOXIDE_PDL_BASELINE_MAJOR.saturating_add(CHROME_MAJOR_WILDLY_AHEAD_DELTA)
}

/// Agent-facing readiness status (GAP-E2E-V14-DOCTOR-OK-MASKS-FAILED-CHECKS).
///
/// - `healthy` — hard checks pass and no soft failures
/// - `degraded` — hard checks pass but soft checks failed (e.g. concurrent Chrome)
/// - `unhealthy` — hard readiness failed (Chrome missing, config dir, …)
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DoctorStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Severity of an individual doctor check.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CheckSeverity {
    /// Contributes to `ok=false` / `status=unhealthy` when failed.
    Hard,
    /// Contributes to `status=degraded` only; does not flip legacy `ok`.
    Soft,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    /// Envelope discriminator — always `doctor`, enforced by [`DoctorKind`].
    #[serde(rename = "type")]
    kind: crate::types::DoctorKind,
    version: &'static str,
    git_sha: &'static str,
    /// Legacy hard readiness (chrome_detect + config_dir + feature). Preserved for BC.
    ok: bool,
    /// Agent-first aggregate: healthy | degraded | unhealthy (additive V15.1).
    status: DoctorStatus,
    /// Names of checks with `ok=false` (hard and soft). Additive.
    failed_checks: Vec<&'static str>,
    /// Whether `--strict` was requested (additive; agents may ignore).
    strict: bool,
    platform: PlatformInfo,
    environment: EnvironmentInfo,
    chrome: ChromeInfo,
    features: FeaturesInfo,
    paths: PathsInfo,
    checks: Vec<Check>,
    /// Host chrome-like process count (CLI-DOC-02 / dual advisory).
    #[serde(skip_serializing_if = "Option::is_none")]
    chrome_n: Option<u64>,
    /// Contention factor percent for deep-research budget (100 = 1.0×).
    #[serde(skip_serializing_if = "Option::is_none")]
    contention_factor_percent: Option<u64>,
    /// Whether dual multiproc deep-research is advisable without raising GT.
    ready_for_dual_deep_research: bool,
    /// Suggested global timeout for default dual+fetch workload on this host.
    recommended_global_timeout: u64,
    /// Linux cgroup opt-in status (G6; n/a on macOS/Windows).
    linux_cgroup: crate::cgroup::CgroupStatus,
    /// Configured memory max MiB when cgroup enabled (else null).
    #[serde(skip_serializing_if = "Option::is_none")]
    linux_cgroup_memory_max_mb: Option<u64>,
}

#[derive(Debug, Serialize)]
struct PlatformInfo {
    os: &'static str,
    arch: &'static str,
    family: &'static str,
    name: &'static str,
}

/// Specialized runtime markers for agent operators (WSL, container, CI, …).
#[derive(Debug, Serialize)]
struct EnvironmentInfo {
    wsl: bool,
    container: bool,
    termux: bool,
    ci: bool,
    flatpak: bool,
    snap: bool,
    labels: Vec<&'static str>,
}

impl From<RuntimeEnvironment> for EnvironmentInfo {
    fn from(env: RuntimeEnvironment) -> Self {
        Self {
            wsl: env.wsl,
            container: env.container,
            termux: env.termux,
            ci: env.ci,
            flatpak: env.flatpak,
            snap: env.snap,
            labels: env.labels(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ChromeInfo {
    feature_enabled: bool,
    detected: bool,
    path: Option<String>,
    channel: Option<String>,
    /// Parsed major from `chrome --version` when probe succeeds (additive).
    major_version: Option<u32>,
    no_chrome_env: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct FeaturesInfo {
    chrome: bool,
}

#[derive(Debug, Serialize)]
struct PathsInfo {
    config_dir: Option<String>,
    cache_dir: Option<String>,
    data_dir: Option<String>,
    state_dir: Option<String>,
    runtime_dir: Option<String>,
}

#[derive(Debug, Serialize)]
struct Check {
    name: &'static str,
    ok: bool,
    /// hard | soft — agents that only read top-level `ok` should also read `status`.
    severity: CheckSeverity,
    detail: String,
}

impl Check {
    fn hard(name: &'static str, ok: bool, detail: impl Into<String>) -> Self {
        Self {
            name,
            ok,
            severity: CheckSeverity::Hard,
            detail: detail.into(),
        }
    }

    fn soft(name: &'static str, ok: bool, detail: impl Into<String>) -> Self {
        Self {
            name,
            ok,
            severity: CheckSeverity::Soft,
            detail: detail.into(),
        }
    }
}

/// Runs environment diagnostics and prints a single JSON report on stdout.
///
/// Async because the Chrome major probe is blocking subprocess I/O and must run
/// under [`crate::concurrency::run_cpu_bound`] (GAP-PAR-043). Cancel-safe: no
/// long-lived resources; a mid-probe cancel only drops the JoinHandle after
/// the blocking pool finishes the short version poll.
pub async fn execute_doctor(args: DoctorArgs) -> i32 {
    let mut checks = Vec::new();

    // GAP-SCRAPE-R2-013: product env NO_CHROME is not read; always report clean.
    let no_chrome_env = false;

    let runtime_env = platform::detect_runtime_environment();

    #[cfg(feature = "chrome")]
    let (chrome, major_for_strict) = {
        match crate::browser::detect_chrome_resolved(None) {
            Ok(resolved) => {
                // GAP-PAR-043: never call the sync probe on the Tokio worker.
                let major = match crate::browser::detect_chrome_major_version_async(&resolved.path)
                    .await
                {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            path = %resolved.path.display(),
                            "doctor: chrome --version probe failed after offload; reporting major=None"
                        );
                        None
                    }
                };
                checks.push(Check::hard(
                    "chrome_detect",
                    true,
                    format!(
                        "found {} (channel={})",
                        resolved.path.display(),
                        resolved.channel.as_str()
                    ),
                ));
                let info = ChromeInfo {
                    feature_enabled: true,
                    detected: true,
                    path: Some(resolved.path.display().to_string()),
                    channel: Some(resolved.channel.as_str().to_string()),
                    major_version: major,
                    no_chrome_env,
                    error: None,
                };
                (info, major)
            }
            Err(e) => {
                checks.push(Check::hard("chrome_detect", false, e.to_string()));
                let info = ChromeInfo {
                    feature_enabled: true,
                    detected: false,
                    path: None,
                    channel: None,
                    major_version: None,
                    no_chrome_env,
                    error: Some(e.to_string()),
                };
                (info, None)
            }
        }
    };

    #[cfg(not(feature = "chrome"))]
    let (chrome, major_for_strict): (ChromeInfo, Option<u32>) = {
        checks.push(Check::hard(
            "chrome_detect",
            false,
            "crate built without feature `chrome`".to_string(),
        ));
        (
            ChromeInfo {
                feature_enabled: false,
                detected: false,
                path: None,
                channel: None,
                major_version: None,
                no_chrome_env,
                error: Some("feature chrome disabled at compile time".into()),
            },
            None,
        )
    };

    checks.push(Check::hard(
        "no_chrome_env",
        true,
        "product env NO_CHROME removed (GAP-SCRAPE-R2-013); Chrome required via feature",
    ));
    let _ = no_chrome_env;

    // PDL / major compatibility check (soft by default; elevates under --strict).
    let pdl_check_ok = match major_for_strict {
        Some(major) if chrome_major_wildly_ahead_of_pdl(major) => {
            checks.push(Check::soft(
                "chrome_pdl_compat",
                false,
                format!(
                    "Chrome major {major} is wildly ahead of chromiumoxide PDL baseline \
                     {CHROMIUMOXIDE_PDL_BASELINE_MAJOR} (slack +{CHROME_MAJOR_WILDLY_AHEAD_DELTA}); \
                     CDP InvalidMessage noise / domain drift likely (GAP-E2E-48-011)"
                ),
            ));
            false
        }
        Some(major) => {
            checks.push(Check::soft(
                "chrome_pdl_compat",
                true,
                format!(
                    "Chrome major {major} within slack of PDL baseline \
                     {CHROMIUMOXIDE_PDL_BASELINE_MAJOR} (+{CHROME_MAJOR_WILDLY_AHEAD_DELTA})"
                ),
            ));
            true
        }
        None if chrome.detected => {
            checks.push(Check::soft(
                "chrome_pdl_compat",
                true,
                "Chrome detected but major version probe unavailable — \
                 cannot assert PDL drift (not a strict failure)",
            ));
            true
        }
        None => {
            checks.push(Check::soft(
                "chrome_pdl_compat",
                true,
                "skipped — Chrome not detected (chrome_detect owns the failure)",
            ));
            true
        }
    };

    // Sandbox awareness for operators (informational soft check).
    if runtime_env.flatpak || runtime_env.snap {
        checks.push(Check::soft(
            "process_sandbox",
            true,
            format!(
                "CLI process sandbox markers: {:?} — Chrome automation may require host install",
                runtime_env.labels()
            ),
        ));
    } else {
        checks.push(Check::soft(
            "process_sandbox",
            true,
            "no Flatpak/Snap process sandbox markers",
        ));
    }

    // GAP-E2E-V11-CHROME-FLAKY / V12 / V14: soft fail when many Chrome processes
    // already run — surfaces as status=degraded (not legacy ok=false).
    {
        let concurrent = crate::process_count::count_chrome_like_processes();
        let ok = concurrent < 12;
        checks.push(Check::soft(
            "chrome_concurrent_processes",
            ok,
            if concurrent == 0 {
                "no chrome/chromium processes observed".into()
            } else if ok {
                format!(
                    "{concurrent} chrome/chromium-like process(es) observed — \
                     elevated cold-start contention possible under parallel SERP"
                )
            } else {
                format!(
                    "{concurrent} chrome/chromium-like process(es) observed — \
                     high contention risk (Flatpak desktop + host); for dual web+news \
                     multiproc raise --global-timeout to print-budget suggested_global_timeout \
                     and keep -p>=2; single-flight SERP only if dual not required; \
                     close spare browsers; set chrome_session_retries via config"
                )
            },
        ));
    }

    if runtime_env.container {
        checks.push(Check::soft(
            "container",
            true,
            "container markers detected — Chrome will use --no-sandbox when needed",
        ));
    }

    let config_dir = platform::config_directory().map(|p| p.display().to_string());
    let cache_dir = platform::cache_directory().map(|p| p.display().to_string());
    let data_dir = platform::data_directory().map(|p| p.display().to_string());
    let state_dir = platform::state_directory().map(|p| p.display().to_string());
    let runtime_dir = platform::runtime_directory().map(|p| p.display().to_string());

    checks.push(Check::hard(
        "config_dir",
        config_dir.is_some(),
        config_dir.clone().unwrap_or_else(|| "unavailable".into()),
    ));

    // GAP-TLS-015 / ADR-0022: local stack description only (no network / no telemetry).
    checks.push(Check::soft(
        "tls_stack",
        true,
        "production=native Chrome TLS (no synthetic fingerprint spoof; ADR-0016/0022); residual HTTP=rustls+aws-lc-rs harness (ADR-0021); no native-tls",
    ));

    // CM-10 / GAP-AUD-DR + CLI-DOC dual readiness: budget vs timeout + host chrome_n.
    let (doctor_chrome_n, doctor_factor, ready_for_dual, recommended_gt) = {
        use crate::budget::{
            default_deep_research_budget_ok, gated_estimate, input_contention_factor_percent,
            suggested_global_timeout, DeepResearchBudgetInput,
        };
        use crate::cli::DEFAULT_FETCH_CONTENT_CAP;
        use crate::deep_research::DEFAULT_MAX_SUB_QUERIES;
        use crate::types::bounded::DEFAULT_GLOBAL_TIMEOUT_SECONDS;

        let chrome_n = crate::process_count::count_chrome_like_processes() as u64;
        let mut input = DeepResearchBudgetInput::from_cli(
            DEFAULT_MAX_SUB_QUERIES,
            true,
            DEFAULT_FETCH_CONTENT_CAP,
            true,
            0,
        );
        input.chrome_n = chrome_n;
        let gated_lab = {
            let lab = DeepResearchBudgetInput::from_cli(
                DEFAULT_MAX_SUB_QUERIES,
                true,
                DEFAULT_FETCH_CONTENT_CAP,
                true,
                0,
            );
            gated_estimate(lab)
        };
        let gated_host = gated_estimate(input);
        let suggested = suggested_global_timeout(input);
        let factor = input_contention_factor_percent(input);
        let ok_budget = default_deep_research_budget_ok(
            DEFAULT_MAX_SUB_QUERIES,
            true,
            DEFAULT_FETCH_CONTENT_CAP,
            true,
            0,
        );
        checks.push(Check::soft(
            "deep_research_budget_ok",
            ok_budget,
            format!(
                "default deep-research lab gated {gated_lab}s / host gated {gated_host}s \
(suggested {suggested}s, chrome_n={chrome_n}, factor={factor}%) vs DEFAULT_GLOBAL_TIMEOUT {DEFAULT_GLOBAL_TIMEOUT_SECONDS}s \
(max_sub={DEFAULT_MAX_SUB_QUERIES}, fetch_cap={DEFAULT_FETCH_CONTENT_CAP}, dual=true, depth=0)"
            ),
        ));
        let ready = chrome_n < crate::types::bounded::BUDGET_CONTENTION_LOW
            && suggested <= DEFAULT_GLOBAL_TIMEOUT_SECONDS;
        (Some(chrome_n), Some(factor), ready, suggested)
    };

    let hard_ok = checks
        .iter()
        .filter(|c| c.severity == CheckSeverity::Hard)
        .all(|c| c.ok)
        && chrome.detected
        && chrome.feature_enabled
        && !no_chrome_env;

    let soft_ok = checks
        .iter()
        .filter(|c| c.severity == CheckSeverity::Soft)
        .all(|c| c.ok);

    let failed_checks: Vec<&'static str> =
        checks.iter().filter(|c| !c.ok).map(|c| c.name).collect();

    // Legacy `ok` = hard readiness only (BC desktop). Under --strict, any
    // failed check (hard or soft, including concurrent Chrome / PDL) flips
    // ok=false so agents do not treat degraded hosts as ready
    // (GAP-E2E-V19-DOCTOR-STRICT-SOFT).
    let ok = if args.strict {
        hard_ok && soft_ok && pdl_check_ok && chrome.detected
    } else {
        hard_ok
    };

    let status = if !hard_ok {
        DoctorStatus::Unhealthy
    } else if !soft_ok || (args.strict && !pdl_check_ok) {
        DoctorStatus::Degraded
    } else {
        DoctorStatus::Healthy
    };

    let xdg_cfg = crate::runtime::load_runtime_user_config();
    let cgroup_policy = crate::cgroup::CgroupPolicy::from_xdg(
        xdg_cfg.get("linux_cgroup_enabled"),
        xdg_cfg.get("linux_cgroup_memory_max_mb"),
    );

    let report = DoctorReport {
        kind: crate::types::DoctorKind::Doctor,
        version: env!("CARGO_PKG_VERSION"),
        git_sha: env!("GIT_SHA"),
        ok,
        status,
        failed_checks,
        strict: args.strict,
        platform: PlatformInfo {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            family: std::env::consts::FAMILY,
            name: platform::platform_name(),
        },
        environment: EnvironmentInfo::from(runtime_env),
        chrome,
        features: FeaturesInfo {
            chrome: cfg!(feature = "chrome"),
        },
        chrome_n: doctor_chrome_n,
        contention_factor_percent: doctor_factor,
        ready_for_dual_deep_research: ready_for_dual,
        recommended_global_timeout: recommended_gt,
        linux_cgroup: cgroup_policy.status(),
        linux_cgroup_memory_max_mb: cgroup_policy.memory_max_mb,
        paths: PathsInfo {
            config_dir,
            cache_dir,
            data_dir,
            state_dir,
            runtime_dir,
        },
        checks,
    };

    // Compact JSON by default; --pretty (global) enables indent.
    let payload = serde_json::to_value(&report);
    match payload {
        Ok(value) => {
            // Rows live under `checks`, NOT `failed_checks`: the latter is a
            // derived list of names, and picking it would silently answer a
            // different question than the operator asked.
            let shape = crate::output::envelope_ops::shape_for("doctor")
                .copied()
                .unwrap_or_else(|| {
                    crate::output::envelope_ops::EnvelopeShape::with_rows(
                        "doctor", "checks", "type",
                    )
                });
            // Environment not ready for production search is not a usage
            // error, so a healthy EMIT of an unhealthy report still exits 1.
            let ok_code = if ok {
                exit_codes::SUCCESS
            } else {
                exit_codes::GENERIC_ERROR
            };
            match output::emit_envelope_or_refuse(
                value,
                &shape,
                output::json_pretty_enabled(),
                output::KeyPolicy::EnglishOnly,
            ) {
                exit_codes::SUCCESS => ok_code,
                other => other,
            }
        }
        Err(err) => {
            output::emit_stderr(crate::i18n::error_msg(
                crate::i18n::Message::DoctorSerializeFailed,
                &err,
            ));
            exit_codes::GENERIC_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdl_baseline_not_wildly_ahead() {
        assert!(!chrome_major_wildly_ahead_of_pdl(
            CHROMIUMOXIDE_PDL_BASELINE_MAJOR
        ));
        assert!(!chrome_major_wildly_ahead_of_pdl(
            CHROMIUMOXIDE_PDL_BASELINE_MAJOR + CHROME_MAJOR_WILDLY_AHEAD_DELTA
        ));
    }

    #[test]
    fn pdl_wildly_ahead_threshold() {
        let limit =
            CHROMIUMOXIDE_PDL_BASELINE_MAJOR.saturating_add(CHROME_MAJOR_WILDLY_AHEAD_DELTA);
        assert!(!chrome_major_wildly_ahead_of_pdl(limit));
        assert!(chrome_major_wildly_ahead_of_pdl(limit + 1));
        assert!(chrome_major_wildly_ahead_of_pdl(999));
    }
}
