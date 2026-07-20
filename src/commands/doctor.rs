// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: **diagnostic / sequential by design**.
// No fan-out: chrome detect + path probes are microsecond-to-ms local I/O;
// coordination overhead of JoinSet/Semaphore would dominate. Runs under the
// process-wide multi_thread Tokio runtime but does not spawn parallel tasks.
// Parallelism modus operandi applies to search / fetch / deep-research paths.
//! Handler for the `doctor` subcommand — environment diagnostics as JSON.
//!
//! # `--strict` (OPP-DOCTOR-STRICT)
//!
//! Chrome major detection is available via
//! [`crate::browser::detect_chrome_major_version`]. With `--strict`:
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
    major
        > CHROMIUMOXIDE_PDL_BASELINE_MAJOR.saturating_add(CHROME_MAJOR_WILDLY_AHEAD_DELTA)
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    #[serde(rename = "type")]
    kind: &'static str,
    version: &'static str,
    git_sha: &'static str,
    ok: bool,
    /// Whether `--strict` was requested (additive; agents may ignore).
    strict: bool,
    platform: PlatformInfo,
    environment: EnvironmentInfo,
    chrome: ChromeInfo,
    features: FeaturesInfo,
    paths: PathsInfo,
    checks: Vec<Check>,
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
    detail: String,
}

/// Runs environment diagnostics and prints a single JSON report on stdout.
pub fn execute_doctor(args: DoctorArgs) -> i32 {
    let mut checks = Vec::new();

    // GAP-SCRAPE-R2-013: product env NO_CHROME is not read; always report clean.
    let no_chrome_env = false;

    let runtime_env = platform::detect_runtime_environment();

    #[cfg(feature = "chrome")]
    let (chrome, major_for_strict) = {
        match crate::browser::detect_chrome_resolved(None) {
            Ok(resolved) => {
                let major = crate::browser::detect_chrome_major_version(&resolved.path);
                checks.push(Check {
                    name: "chrome_detect",
                    ok: true,
                    detail: format!(
                        "found {} (channel={})",
                        resolved.path.display(),
                        resolved.channel.as_str()
                    ),
                });
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
                checks.push(Check {
                    name: "chrome_detect",
                    ok: false,
                    detail: e.to_string(),
                });
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
        checks.push(Check {
            name: "chrome_detect",
            ok: false,
            detail: "crate built without feature `chrome`".to_string(),
        });
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

    checks.push(Check {
        name: "no_chrome_env",
        ok: true,
        detail: "product env NO_CHROME removed (GAP-SCRAPE-R2-013); Chrome required via feature"
            .into(),
    });
    let _ = no_chrome_env;

    // PDL / major compatibility check (always reported; only fails `ok` under --strict).
    let pdl_check_ok = match major_for_strict {
        Some(major) if chrome_major_wildly_ahead_of_pdl(major) => {
            checks.push(Check {
                name: "chrome_pdl_compat",
                ok: false,
                detail: format!(
                    "Chrome major {major} is wildly ahead of chromiumoxide PDL baseline \
                     {CHROMIUMOXIDE_PDL_BASELINE_MAJOR} (slack +{CHROME_MAJOR_WILDLY_AHEAD_DELTA}); \
                     CDP InvalidMessage noise / domain drift likely (GAP-E2E-48-011)"
                ),
            });
            false
        }
        Some(major) => {
            checks.push(Check {
                name: "chrome_pdl_compat",
                ok: true,
                detail: format!(
                    "Chrome major {major} within slack of PDL baseline \
                     {CHROMIUMOXIDE_PDL_BASELINE_MAJOR} (+{CHROME_MAJOR_WILDLY_AHEAD_DELTA})"
                ),
            });
            true
        }
        None if chrome.detected => {
            checks.push(Check {
                name: "chrome_pdl_compat",
                ok: true,
                detail: "Chrome detected but major version probe unavailable — \
                         cannot assert PDL drift (not a strict failure)"
                    .into(),
            });
            true
        }
        None => {
            checks.push(Check {
                name: "chrome_pdl_compat",
                ok: true,
                detail: "skipped — Chrome not detected (chrome_detect owns the failure)".into(),
            });
            true
        }
    };

    // Sandbox awareness for operators (informational — does not fail doctor alone).
    if runtime_env.flatpak || runtime_env.snap {
        checks.push(Check {
            name: "process_sandbox",
            ok: true,
            detail: format!(
                "CLI process sandbox markers: {:?} — Chrome automation may require host install",
                runtime_env.labels()
            ),
        });
    } else {
        checks.push(Check {
            name: "process_sandbox",
            ok: true,
            detail: "no Flatpak/Snap process sandbox markers".into(),
        });
    }

    if runtime_env.container {
        checks.push(Check {
            name: "container",
            ok: true,
            detail: "container markers detected — Chrome will use --no-sandbox when needed".into(),
        });
    }

    let config_dir = platform::config_directory().map(|p| p.display().to_string());
    let cache_dir = platform::cache_directory().map(|p| p.display().to_string());
    let data_dir = platform::data_directory().map(|p| p.display().to_string());
    let state_dir = platform::state_directory().map(|p| p.display().to_string());
    let runtime_dir = platform::runtime_directory().map(|p| p.display().to_string());

    checks.push(Check {
        name: "config_dir",
        ok: config_dir.is_some(),
        detail: config_dir
            .clone()
            .unwrap_or_else(|| "unavailable".into()),
    });

    // GAP-TLS-015 / ADR-0022: local stack description only (no network / no telemetry).
    checks.push(Check {
        name: "tls_stack",
        ok: true,
        detail: "production=native Chrome TLS (no synthetic fingerprint spoof; ADR-0016/0022); residual HTTP=rustls+aws-lc-rs harness (ADR-0021); no native-tls"
            .into(),
    });

    let base_ok = checks
        .iter()
        .filter(|c| c.name == "chrome_detect" || c.name == "no_chrome_env" || c.name == "config_dir")
        .all(|c| c.ok)
        && chrome.detected
        && chrome.feature_enabled
        && !no_chrome_env;

    // OPP-DOCTOR-STRICT: --strict also fails when major ≫ PDL, or (redundantly)
    // when Chrome is missing (already reflected in base_ok).
    let ok = if args.strict {
        base_ok && pdl_check_ok && chrome.detected
    } else {
        base_ok
    };

    let report = DoctorReport {
        kind: "doctor",
        version: env!("CARGO_PKG_VERSION"),
        git_sha: env!("GIT_SHA"),
        ok,
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
        paths: PathsInfo {
            config_dir,
            cache_dir,
            data_dir,
            state_dir,
            runtime_dir,
        },
        checks,
    };

    match serde_json::to_string_pretty(&report) {
        Ok(json) => match output::print_line_stdout(&json) {
            Ok(()) => {
                if ok {
                    exit_codes::SUCCESS
                } else {
                    // Environment not ready for production search — not a usage error.
                    exit_codes::GENERIC_ERROR
                }
            }
            Err(err) if output::is_broken_pipe(&err) => exit_codes::BROKEN_PIPE,
            Err(err) => {
                output::emit_stderr(crate::i18n::error_msg(
                    crate::i18n::Message::DoctorEmitFailed,
                    &err,
                ));
                exit_codes::GENERIC_ERROR
            }
        },
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
        assert!(!chrome_major_wildly_ahead_of_pdl(CHROMIUMOXIDE_PDL_BASELINE_MAJOR));
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
