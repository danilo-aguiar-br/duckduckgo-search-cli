// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: I/O-bound (page download) + multi-process Chrome CDP.
//! Parallel fan-out for content extraction (flag `--fetch-content`).
//!
//! ## Layout (Pass 46 / GAP-SCRAPE-R-001 — SRP split)
//!
//! | Submodule | Responsibility |
//! |-----------|----------------|
//! | [`circuit`] | Per-host circuit breaker (WS-12) |
//! | [`host`] | Per-host semaphore map + URL host extraction |
//! | [`enrich`] | Chrome pool + parallel enrichment orchestration |
//!
//! For each result in a `SearchOutput`, spawns an async task bounded by a
//! `Semaphore` (same capacity as `--parallel` / `--max-concurrency`). Production
//! transport is a multi-process Chrome pool; residual HTTP harness only under
//! test. Fills `SearchResult.content`, `.content_size` and
//! `.content_extraction_method` when successful.
//!
//! Also updates the `SearchMetadata` fields:
//! - `concurrent_fetches` = total spawned tasks.
//! - `fetch_successes` = tasks that returned non-empty `content`.
//! - `fetch_failures` = tasks that returned an error or empty content.
//!
//! Extraction respects `CancellationToken` — global cancellation aborts all
//! in-flight tasks quickly.
//!
//! ## Scraping policy (Pass 45–46)
//!
//! Does **not** honor `robots.txt` (operator mandate). Each URL still passes the
//! shared SSRF gate ([`crate::content::url_is_safe_to_fetch`]) before Chrome
//! navigation or residual HTTP fetch. Concurrency is bounded by global/per-host
//! [`tokio::sync::Semaphore`]s, circuit breaker, and optional progress on stderr.

mod circuit;
mod enrich;
mod host;

pub use circuit::{BreakerDecision, BreakerEntry, BreakerState, CircuitBreakerMap};
pub use enrich::{enrich_with_content, enrich_with_content_opts, EnrichOptions};
pub use host::{extract_host, semaphore_for_host, PerHostSemaphoreMap};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SearchMetadata, SearchOutput, SearchResult};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    /// Test-only sleep between concurrent permit holders (GAP-SCRAPE-R-010).
    const TEST_CONCURRENCY_HOLD_MS: u64 = 30;
    /// Short HTTP client timeout for cancelled-enrichment unit test.
    const TEST_HTTP_TIMEOUT_MS: u64 = 100;

    fn test_config(parallelism: u32, max_tam: usize) -> crate::types::Config {
        let q = crate::security::ValidatedQuery::try_new("q").expect("q");
        let mut cfg = crate::types::Config::default();
        cfg.query = q.clone();
        cfg.queries = vec![q];
        cfg.parallelism = crate::types::ParallelismDegree::try_new(parallelism.max(1).min(20))
            .expect("parallelism");
        cfg.max_content_length = crate::types::ContentLengthLimit::try_new(max_tam.max(1))
            .expect("content");
        cfg.fetch_content = true;
        cfg.pages = crate::types::PageCount::try_new(1).expect("pages");
        cfg.retries = crate::types::RetryBudget::try_new(0).expect("retries");
        cfg
    }

    fn empty_output() -> SearchOutput {
        SearchOutput {
            query: "q".to_string(),
            engine: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: crate::types::test_timestamp(),
            region: "br-pt".to_string(),
            result_count: 0,
            results: vec![],
            pages_fetched: 1,
            news: None,
            news_count: None,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "x".to_string(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                chrome_attempted: false,
                user_agent: "ua".to_string(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
                pre_flight_fired: false,
                pre_flight_executed: false,
                pre_flight_status: None,
                news_promo_filtered: None,
                stream_requested: None,
                stream_effective: None,
                zero_cause: None,
                next_action_suggestion: None,
                bytes_raw: None,
                bytes_decompressed: None,
                cascade_level_observed: None,
                result_count_compat: None,
                endpoint_used_compat: None,
                vertical_used: None,
                chrome_path_resolved: None,
                chrome_channel: None,
                ..Default::default()
            },
        }
    }

    #[tokio::test]
    async fn enrich_with_content_no_op_when_flag_false() {
        crate::tls_bootstrap::ensure_for_tests();
        let cliente = reqwest::Client::new();
        let mut cfg = test_config(3, 1000);
        cfg.fetch_content = false;
        let mut output = empty_output();
        output.results.push(SearchResult {
            position: 1,
            title: "Um".to_string(),
            url: crate::types::HttpUrl::for_test("http://inexistente.local/a"),
            display_url: None,
            snippet: None,
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        });

        let token = CancellationToken::new();
        enrich_with_content(&mut output, Some(&cliente), &cfg, &token).await;

        assert!(output.results[0].content.is_none());
        assert_eq!(output.metadata.concurrent_fetches, 0);
    }

    #[test]
    fn extract_host_valid_url_returns_host() {
        assert_eq!(extract_host("https://www.example.com/a"), "www.example.com");
        assert_eq!(extract_host("https://API.test/x"), "api.test");
    }

    #[test]
    fn extract_host_invalid_url_returns_unknown() {
        assert_eq!(extract_host("nao-eh-url"), "unknown");
        assert_eq!(extract_host(""), "unknown");
    }

    #[test]
    fn semaphore_for_host_creates_once_per_host() {
        let mapa: PerHostSemaphoreMap = Arc::new(StdMutex::new(HashMap::new()));
        let sema_a1 = semaphore_for_host(&mapa, "a.com", 3);
        let sema_a2 = semaphore_for_host(&mapa, "a.com", 99);
        assert!(Arc::ptr_eq(&sema_a1, &sema_a2));
        assert_eq!(sema_a1.available_permits(), 3);

        let sema_b = semaphore_for_host(&mapa, "b.com", 5);
        assert!(!Arc::ptr_eq(&sema_a1, &sema_b));
        assert_eq!(sema_b.available_permits(), 5);

        let mapa_guardado = mapa.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(mapa_guardado.len(), 2);
    }

    #[tokio::test]
    async fn get_semaphore_limits_simultaneous_concurrency_on_same_host() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mapa: PerHostSemaphoreMap = Arc::new(StdMutex::new(HashMap::new()));
        let contador_simultaneo = Arc::new(AtomicUsize::new(0));
        let pico_simultaneo = Arc::new(AtomicUsize::new(0));

        let mut tarefas = Vec::with_capacity(20);
        for _ in 0..20 {
            let mapa = Arc::clone(&mapa);
            let contador = Arc::clone(&contador_simultaneo);
            let pico = Arc::clone(&pico_simultaneo);
            tarefas.push(tokio::spawn(async move {
                let sema = semaphore_for_host(&mapa, "same-host.com", 2);
                let _permit = sema
                    .acquire_owned()
                    .await
                    .expect("BUG: semaphore should not be closed");
                let atual = contador.fetch_add(1, Ordering::SeqCst) + 1;
                let mut p = pico.load(Ordering::SeqCst);
                while atual > p {
                    match pico.compare_exchange(p, atual, Ordering::SeqCst, Ordering::SeqCst) {
                        Ok(_) => break,
                        Err(novo) => p = novo,
                    }
                }
                tokio::time::sleep(Duration::from_millis(TEST_CONCURRENCY_HOLD_MS)).await;
                contador.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for t in tarefas {
            let _ = t.await;
        }
        assert!(
            pico_simultaneo.load(Ordering::SeqCst) <= 2,
            "simultaneous peak {} exceeded limit 2",
            pico_simultaneo.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn enrich_with_content_cancelled_marks_failures() {
        crate::tls_bootstrap::ensure_for_tests();
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(TEST_HTTP_TIMEOUT_MS))
            .build()
            .unwrap();
        let cfg = test_config(2, 1000);
        let mut output = empty_output();
        for i in 0..3 {
            output.results.push(SearchResult {
                position: (i + 1) as u32,
                title: format!("r{i}"),
                url: crate::types::HttpUrl::for_test(&format!("http://127.0.0.1:1/{i}")),
                display_url: None,
                snippet: None,
                original_title: None,
                content: None,
                content_size: None,
                content_extraction_method: None,
            });
        }

        let token = CancellationToken::new();
        token.cancel();
        enrich_with_content(&mut output, Some(&cliente), &cfg, &token).await;

        assert_eq!(output.metadata.fetch_successes, 0);
    }

    #[test]
    fn ws12_breaker_allows_when_closed() {
        let cb = CircuitBreakerMap::new();
        assert_eq!(cb.check("host-a.com"), BreakerDecision::Allow);
        assert_eq!(cb.check("host-a.com"), BreakerDecision::Allow);
    }

    #[test]
    fn ws12_breaker_opens_after_threshold_failures() {
        use super::circuit::CB_FAILURE_THRESHOLD;
        let cb = CircuitBreakerMap::new();
        for _ in 0..(CB_FAILURE_THRESHOLD - 1) {
            cb.record_failure("flaky.com");
            assert_eq!(
                cb.check("flaky.com"),
                BreakerDecision::Allow,
                "must remain Closed below threshold"
            );
        }
        cb.record_failure("flaky.com");
        assert_eq!(
            cb.check("flaky.com"),
            BreakerDecision::Reject,
            "must Open at threshold"
        );
        assert_eq!(cb.check("healthy.com"), BreakerDecision::Allow);
    }

    #[test]
    fn ws12_breaker_resets_on_success() {
        let cb = CircuitBreakerMap::new();
        cb.record_failure("x.com");
        cb.record_failure("x.com");
        cb.record_success("x.com");
        assert_eq!(
            cb.check("x.com"),
            BreakerDecision::Allow,
            "success must clear the failure counter"
        );
        cb.record_failure("x.com");
        cb.record_failure("x.com");
        assert_eq!(cb.check("x.com"), BreakerDecision::Allow);
    }

    #[test]
    fn ws12_breaker_half_opens_after_cooldown() {
        use super::circuit::CB_FAILURE_THRESHOLD;
        let cb = CircuitBreakerMap::new();
        for _ in 0..CB_FAILURE_THRESHOLD {
            cb.record_failure("slow.com");
        }
        assert_eq!(cb.check("slow.com"), BreakerDecision::Reject);
        assert!(cb.contains_host("slow.com"));
    }
}
