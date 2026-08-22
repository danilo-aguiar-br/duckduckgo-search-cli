// SPDX-License-Identifier: MIT OR Apache-2.0
//! Post-publish verification gate (GAP-REL-001 / ADR-0032).
//!
//! # The question no other gate asks
//!
//! Every gate in `NO_CI.md` answers "does the TREE compile?". `cargo publish
//! --dry-run` answers "is the PACKAGE well-formed?". None of them answers the
//! only question a user experiences: **does the version the registry actually
//! serves compile?**
//!
//! That gap is not theoretical. v1.0.1 and v1.0.2 shipped an ungated `use` of a
//! `#[cfg(target_os = "linux")]` item, so `cargo install` failed with `E0432`
//! on macOS and Windows. The fix landed in the tree and was published — and
//! then the FIXED version was yanked, which silently promoted the broken v1.0.2
//! back to `max_stable_version`. Yank is a registry mutation that reorders
//! version resolution, and it passed through no gate at all, so the regression
//! produced no signal anywhere.
//!
//! This binary closes that loop. Run it after every `cargo publish` and after
//! every `cargo yank`.
//!
//! # Usage
//!
//! ```text
//! cargo run --bin verify_published --features release-gate
//! ```
//!
//! # Exit codes
//!
//! Reuses the crate's taxonomy so a wrapper can branch the same way it does on
//! the main binary:
//!
//! - `0` — the registry serves this crate's version and no lower version is live
//! - `1` — divergence: the registry serves something other than this version
//! - `2` — the response could not be parsed into the expected shape
//! - `3` — blocked by the registry (HTTP 403), e.g. a non-identifying User-Agent
//! - `4` — the request timed out
//!
//! # stdout is contract
//!
//! One compact JSON envelope on stdout, diagnostics on stderr — the same
//! agent-native discipline as the main binary.

use duckduckgo_search_cli::error::exit_codes;
use std::time::Duration;

/// crates.io rejects requests without an identifying User-Agent.
///
/// Measured 2026-08-21: no UA → HTTP 403; `curl/8.0` → HTTP 403; a UA naming
/// the tool and a contact → HTTP 200. A gate that sent a generic UA would read
/// its own 403 as "crate not found" and report a false all-clear, which is the
/// precise failure mode this gate exists to prevent.
const USER_AGENT: &str = concat!(
    "duckduckgo-search-cli/",
    env!("CARGO_PKG_VERSION"),
    " (release-gate; https://github.com/daniloaguiarbr/duckduckgo-search-cli)"
);

/// This crate's version, as compiled.
const LOCAL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Published versions MEASURED not to compile outside Linux, with the evidence.
///
/// # Why a list and not a heuristic
///
/// Old live versions are normal in any registry, so "something older is live"
/// is noise, not a finding. What is actionable is a version known to be broken
/// that a fresh `cargo install` can still resolve to. Each entry here was
/// measured by running `scripts/portability-lint.sh` against the published
/// source of that exact version, so the claim is reproducible rather than
/// remembered.
///
/// An entry leaves this list only when the version is yanked — never because it
/// looks old.
const KNOWN_BROKEN_VERSIONS: &[(&str, &str)] = &[
    (
        "1.0.1",
        "ungated `use` of platform-only items in src/browser/session.rs:7 — \
         2 violations, E0432 on macOS and Windows",
    ),
    (
        "1.0.2",
        "same class in src/browser/session/mod.rs:20 — 2 violations, E0432 on \
         macOS and Windows; this is the version users currently receive",
    ),
];

const API_URL: &str = "https://crates.io/api/v1/crates/duckduckgo-search-cli";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A semantic version reduced to the triple used for ordering.
///
/// Avoids a `semver` dependency for what is a three-integer comparison. Any
/// component that does not parse sorts as `0`, which is deliberately
/// conservative: an unparseable version can never outrank a real one and be
/// mistaken for a newer release.
fn version_triple(v: &str) -> (u64, u64, u64) {
    let core = v.split(['-', '+']).next().unwrap_or(v);
    let mut it = core.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

fn main() -> std::process::ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("verify_published: failed to build tokio runtime: {e}");
            return std::process::ExitCode::from(exit_codes::GENERIC_ERROR as u8);
        }
    };
    let code = runtime.block_on(run());
    std::process::ExitCode::from(code as u8)
}

#[allow(clippy::too_many_lines)]
async fn run() -> i32 {
    // rustls panics with "No provider set" on the first handshake unless a
    // `CryptoProvider` is installed for the process. The main binary does this
    // too; reusing the crate's bootstrap keeps one TLS setup path.
    if let Err(e) = duckduckgo_search_cli::tls_bootstrap::install_rustls_crypto_provider() {
        eprintln!("verify_published: failed to install rustls crypto provider: {e}");
        return exit_codes::GENERIC_ERROR;
    }

    let client = match reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("verify_published: failed to build HTTP client: {e}");
            return exit_codes::GENERIC_ERROR;
        }
    };

    let response = match client.get(API_URL).send().await {
        Ok(r) => r,
        Err(e) if e.is_timeout() => {
            eprintln!("verify_published: crates.io timed out after {REQUEST_TIMEOUT:?}");
            return exit_codes::GLOBAL_TIMEOUT;
        }
        Err(e) => {
            eprintln!("verify_published: request to crates.io failed: {e}");
            return exit_codes::GENERIC_ERROR;
        }
    };

    let status = response.status();
    // 403 and 404 mean opposite things and must never be conflated: 403 is the
    // registry refusing US (bad UA, rate limit), 404 would mean the crate does
    // not exist. Reporting a 403 as "not found" is exactly how a release gate
    // turns into a rubber stamp.
    if status.as_u16() == 403 {
        eprintln!(
            "verify_published: crates.io returned 403. The API refuses requests \
             without an identifying User-Agent; this gate sends {USER_AGENT:?}. \
             This is NOT evidence that the crate is missing."
        );
        return exit_codes::RATE_LIMITED_OR_BLOCKED;
    }
    if !status.is_success() {
        eprintln!("verify_published: crates.io returned HTTP {status}");
        return exit_codes::GENERIC_ERROR;
    }

    // `text()` + `serde_json` rather than `response.json()`: the latter needs
    // reqwest's `json` feature, and this gate deliberately adds no feature that
    // the harness does not already carry.
    let raw = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("verify_published: failed to read crates.io response body: {e}");
            return exit_codes::GENERIC_ERROR;
        }
    };
    let body: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("verify_published: crates.io response was not valid JSON: {e}");
            return exit_codes::INVALID_CONFIG;
        }
    };

    let Some(max_stable) = body
        .get("crate")
        .and_then(|c| c.get("max_stable_version"))
        .and_then(serde_json::Value::as_str)
    else {
        eprintln!("verify_published: response has no crate.max_stable_version");
        return exit_codes::INVALID_CONFIG;
    };

    let Some(versions) = body.get("versions").and_then(serde_json::Value::as_array) else {
        eprintln!("verify_published: response has no versions array");
        return exit_codes::INVALID_CONFIG;
    };

    let local = version_triple(LOCAL_VERSION);

    // Every live (non-yanked) version, and specifically the live ones that are
    // OLDER than this build. Those are what a fresh `cargo install` can still
    // resolve to, and they are the population that must be empty.
    let mut live: Vec<String> = Vec::new();
    let mut local_is_published = false;
    let mut local_is_yanked = false;
    for v in versions {
        let (Some(num), Some(yanked)) = (
            v.get("num").and_then(serde_json::Value::as_str),
            v.get("yanked").and_then(serde_json::Value::as_bool),
        ) else {
            continue;
        };
        if num == LOCAL_VERSION {
            local_is_published = true;
            local_is_yanked = yanked;
        }
        if !yanked {
            live.push(num.to_string());
        }
    }
    live.sort_by_key(|v| version_triple(v));

    // Old live versions are normal; a KNOWN-BROKEN live version is not.
    let broken_live: Vec<serde_json::Value> = KNOWN_BROKEN_VERSIONS
        .iter()
        .filter(|(num, _)| live.iter().any(|l| l == num))
        .map(|(num, evidence)| serde_json::json!({ "version": num, "evidence": evidence }))
        .collect();

    let serves_local = max_stable == LOCAL_VERSION;
    let ok = serves_local && broken_live.is_empty();
    let _ = local;

    // Keyed `gate`, deliberately NOT `type`.
    //
    // `type` is the product envelope discriminator, and
    // `every_emitted_discriminator_has_a_published_schema` requires every one of
    // them to ship a JSON Schema in the catalog. This binary is maintainer
    // tooling behind `required-features`; a user of the CLI never sees it, so
    // adding it to the published catalog would describe a surface the product
    // does not have. Using a different key keeps that ruler intact and narrow
    // instead of carving an exemption into it.
    let envelope = serde_json::json!({
        "gate": "verify_published",
        "ok": ok,
        "local_version": LOCAL_VERSION,
        "max_stable_version": max_stable,
        "serves_local_version": serves_local,
        "local_is_published": local_is_published,
        "local_is_yanked": local_is_yanked,
        "live_version_count": live.len(),
        "known_broken_live": broken_live,
    });
    println!(
        "{}",
        serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string())
    );

    if ok {
        return exit_codes::SUCCESS;
    }

    // Name the specific shape of the divergence, because the remedy differs.
    if local_is_published && local_is_yanked {
        eprintln!(
            "verify_published: {LOCAL_VERSION} IS published but YANKED, so the \
             registry fell back to {max_stable}. This is the v1.0.5 incident: \
             yanking the fixed version promotes a broken one. Either \
             `cargo yank --undo --version {LOCAL_VERSION}` or publish a higher version."
        );
    } else if !local_is_published {
        eprintln!(
            "verify_published: {LOCAL_VERSION} is NOT on crates.io; the registry \
             serves {max_stable}. Publish before treating this release as done."
        );
    } else if !serves_local {
        eprintln!("verify_published: registry serves {max_stable}, not {LOCAL_VERSION}.");
    }
    for entry in &broken_live {
        let version = entry.get("version").and_then(serde_json::Value::as_str);
        let evidence = entry.get("evidence").and_then(serde_json::Value::as_str);
        eprintln!(
            "verify_published: {} is live and installable but does not compile: {}",
            version.unwrap_or("?"),
            evidence.unwrap_or("?")
        );
    }
    if !broken_live.is_empty() {
        eprintln!(
            "verify_published: yank those ONLY after a working version is \
             published — the Cargo book requires that order so dependents are \
             never left without a compatible release."
        );
    }
    exit_codes::GENERIC_ERROR
}
