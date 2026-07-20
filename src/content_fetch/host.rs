// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure coordination (short std mutex critical sections).
//! Per-host semaphore map and URL host extraction for content-fetch fan-out.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Semaphore;

/// Map `host → Semaphore` for per-host rate-limiting shared across tasks.
///
/// Uses [`std::sync::Mutex`] (not `tokio::sync::Mutex`) because the critical
/// section never crosses `.await` — only HashMap lookup/insert. Interior-
/// mutability rules: tokio Mutex is reserved for locks that must span await
/// (Chrome CDP exclusivity below).
pub type PerHostSemaphoreMap = Arc<StdMutex<HashMap<String, Arc<Semaphore>>>>;

/// Gets (or creates under lock) the semaphore for the given `host` with capacity `limit`.
///
/// Synchronous short critical section on a [`std::sync::Mutex`]: lookup/insert only.
/// The returned `Arc<Semaphore>` is cloned so callers can `.acquire_owned().await`
/// **after** this function returns (lock is never held across await).
///
/// Poison is recovered so a prior panic cannot permanently disable per-host limiting.
#[must_use]
pub fn semaphore_for_host(
    mapa: &PerHostSemaphoreMap,
    host: &str,
    limit: usize,
) -> Arc<Semaphore> {
    let mut guard = mapa
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .entry(host.to_string())
        .or_insert_with(|| Arc::new(Semaphore::new(limit.max(1))))
        .clone()
}

/// Extracts the host from a URL. Returns `"unknown"` when the URL is malformed —
/// all malformed URLs share the same slot (this is a safe fallback).
///
/// Hosts are normalised to lowercase so that `Exemplo.COM` and `exemplo.com`
/// share the same per-host `Semaphore`.
///
/// # Example
///
/// ```
/// use duckduckgo_search_cli::content_fetch::extract_host;
///
/// assert_eq!(extract_host("https://www.example.com/path?q=1"), "www.example.com");
/// assert_eq!(extract_host("https://API.test/x"), "api.test"); // lowercased
/// assert_eq!(extract_host("not-a-url"), "unknown");             // malformed
/// assert_eq!(extract_host(""), "unknown");                      // empty
/// ```
// Thin URL host extractor used on every content-fetch fan-out; `#[inline]` not `always`.
#[inline]
pub fn extract_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_lowercase()))
        .unwrap_or_else(|| "unknown".to_string())
}
