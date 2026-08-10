// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for `parallel.rs` — multi-query with `wiremock`.
//!
//! ZERO real HTTP calls. Each test starts a `MockServer` on a random port and
//! points `DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML`/`_LITE` at it. Serialization
//! against other tests that touch env vars is done via the async `env_lock()`.
//!
//! Covers:
//! - Multi-query happy path (`execute_parallel_searches` with N successful queries).
//! - Streaming happy path (`execute_parallel_searches_streaming` consumed via mpsc).
//! - Streaming with a closed consumer → remaining tasks aborted via `abort_all`.
//! - `pages > 1` forces an isolated Client per task (paths 138-146 and 342-350).

mod common;

use duckduckgo_search_cli::parallel::{
    execute_parallel_searches, execute_parallel_searches_streaming,
};
use duckduckgo_search_cli::types::{Config, Endpoint};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Serializes tests that install process-wide endpoint policy / harness env.
fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: LazyLock<TokioMutex<()>> = LazyLock::new(|| TokioMutex::new(()));
    &LOCK
}

/// V18: mock endpoints via EndpointPolicy SSOT (`common::HarnessGuard`).
type EnvGuard = common::HarnessGuard;

/// Helper that builds a lean `Config` for parallelism tests.
/// `pages` controls the shared vs isolated Client decision (section 4.3).
fn test_config_wm(
    endpoint: Endpoint,
    pages: u32,
    queries: Vec<String>,
    parallelism: u32,
) -> Config {
    // proxy_config=Disabled in common::lean_config — no HTTP_PROXY inheritance.
    common::lean_config_queries(endpoint, pages, queries, parallelism)
}

/// HTML with 2 organic results with body above 5,000 bytes (anti-block threshold).
fn html_two_results() -> String {
    // Padding ensures the body stays above the silent-block threshold (5,000 bytes).
    let padding = "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. Este comentário é apenas preenchimento e não afeta a extração de resultados. -->".repeat(30);
    format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/a">Resultado A</a>
        <a class="result__snippet">Snippet A com texto suficiente para passar nos filtros padrão.</a>
        <span class="result__url">exemplo.com/a</span>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/b">Resultado B</a>
        <a class="result__snippet">Snippet B com texto suficiente para passar nos filtros padrão.</a>
        <span class="result__url">exemplo.com/b</span>
      </div>
    </div>
    </body></html>"#
    )
}

/// HTML with vqd/s/dc tokens for pagination — body above 5,000 bytes (anti-block threshold).
fn html_page_with_tokens(vqd: &str, s: &str, dc: &str, prefix: &str) -> String {
    // Padding ensures the body stays above the silent-block threshold (5,000 bytes).
    let padding = "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. Este comentário é apenas preenchimento e não afeta a extração de resultados. -->".repeat(30);
    format!(
        r#"<html><body>
        {padding}
        <form><input name="vqd" value="{vqd}"><input name="s" value="{s}"><input name="dc" value="{dc}"></form>
        <div id="links">
          <div class="result">
            <a class="result__a" href="//exemplo.com/{prefix}-1">{prefix} One</a>
            <a class="result__snippet">snippet of {prefix} one with enough size.</a>
          </div>
          <div class="result">
            <a class="result__a" href="//exemplo.com/{prefix}-2">{prefix} Two</a>
            <a class="result__snippet">snippet of {prefix} two with enough size.</a>
          </div>
        </div>
        </body></html>"#
    )
}

// ---------------------------------------------------------------------------
// Test 1: multi-query happy path — 3 queries, parallelism 2, all succeeding.
// Covers: spawn loop, semaphore acquire, shared client (pages=1),
// execute_query_with_cancellation happy path, drop(permit), ordered collection.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn multi_query_happy_path_3_queries_parallelism_2() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_two_results())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let queries = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
    let cfg = test_config_wm(Endpoint::Html, 1, queries.clone(), 2);
    let token = CancellationToken::new();

    let output = execute_parallel_searches(common::validated_queries_owned(&queries), cfg, token)
        .await
        .expect("multi-query should return Ok");

    assert_eq!(output.query_count, 3);
    assert_eq!(output.parallelism, 2);
    assert_eq!(output.searches.len(), 3);

    // Original order MUST be preserved despite staggered launch.
    assert_eq!(output.searches[0].query, "alpha");
    assert_eq!(output.searches[1].query, "beta");
    assert_eq!(output.searches[2].query, "gamma");

    // All queries must succeed (no error field) and have 2 results each.
    for search in &output.searches {
        assert!(
            search.error.is_none(),
            "query {:?} should succeed but failed: {:?}",
            search.query,
            search.message
        );
        assert_eq!(search.result_count, 2);
        assert_eq!(search.results.len(), 2);
        assert_eq!(search.pages_fetched, 1);
    }
}

// ---------------------------------------------------------------------------
// Test 2: pages > 1 → forces isolated Client construction per task.
// Covers lines 138-146 (branch `None => http::build_client_with_proxy`).
// Only 1 query to keep the test fast; 2 pages via vqd tokens.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn multi_query_with_pages_above_1_uses_isolated_client() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Page 1 GET — returns tokens to allow POST for page 2.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_page_with_tokens("vqd-pg1", "0", "30", "P1"))
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Page 2 POST — any POST body with vqd is accepted.
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_page_with_tokens("vqd-pg2", "30", "60", "P2"))
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let queries = vec!["query-multipagina".to_string()];
    // pages = 2 ACTIVATES the isolated Client branch (shared_client = None).
    let cfg = test_config_wm(Endpoint::Html, 2, queries.clone(), 1);
    let token = CancellationToken::new();

    let output = execute_parallel_searches(common::validated_queries_owned(&queries), cfg, token)
        .await
        .expect("multi-query with pages>1 should return Ok");

    assert_eq!(output.searches.len(), 1);
    let search = &output.searches[0];
    assert!(
        search.error.is_none(),
        "query should succeed: {:?}",
        search.message
    );
    // 2 results per page x 2 pages = 4.
    assert_eq!(search.result_count, 4);
    assert_eq!(search.pages_fetched, 2);
}

// ---------------------------------------------------------------------------
// Test 3: Streaming happy path — consumer receives all results via mpsc
// and statistics correctly reflect total/successes.
// Covers: execute_parallel_searches_streaming complete until return of
// `StreamStats`, successful send branch per channel.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn streaming_happy_path_consumer_receives_all_results() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_two_results())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let queries = vec!["s-um".to_string(), "s-dois".to_string()];
    let cfg = test_config_wm(Endpoint::Html, 1, queries.clone(), 2);
    let token = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(8);

    // Consumer: drains the channel in parallel with the producer.
    let consumer = tokio::spawn(async move {
        let mut received = Vec::new();
        while let Some((index, output)) = rx.recv().await {
            received.push((index, output));
        }
        received
    });

    let stats = execute_parallel_searches_streaming(
        common::validated_queries_owned(&queries),
        cfg,
        token,
        tx,
    )
    .await
    .expect("streaming should return Ok");

    let received = consumer.await.expect("consumer task should complete");

    assert_eq!(stats.total, 2);
    assert_eq!(stats.successes, 2);
    assert_eq!(stats.errors, 0);
    assert_eq!(stats.parallelism, 2);
    assert!(stats.start_timestamp.timestamp() > 0);

    assert_eq!(received.len(), 2, "consumer should receive both outputs");
    for (_index, output) in &received {
        assert!(output.error.is_none(), "streaming output should be clean");
        assert_eq!(output.result_count, 2);
    }
}

// ---------------------------------------------------------------------------
// Test 4: Streaming with cancellation BEFORE start — all queries
// return error output and statistics mark everything as error.
// Covers cancellation branch inside the task before `acquire_owned`.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn streaming_cancelled_before_start_marks_everything_as_error() {
    // Does not touch env — no mocks because tasks abort before any HTTP.
    let token = CancellationToken::new();
    token.cancel();

    let queries = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let cfg = test_config_wm(Endpoint::Html, 1, queries.clone(), 3);
    let (tx, mut rx) = mpsc::channel(8);

    let consumer = tokio::spawn(async move {
        let mut received = Vec::new();
        while let Some(item) = rx.recv().await {
            received.push(item);
        }
        received
    });

    let stats = execute_parallel_searches_streaming(
        common::validated_queries_owned(&queries),
        cfg,
        token,
        tx,
    )
    .await
    .expect("cancelled streaming should return Ok with stats");

    let received = consumer.await.expect("consumer task should complete");

    assert_eq!(stats.total, 3);
    assert_eq!(stats.successes, 0);
    assert_eq!(stats.errors, 3);
    assert_eq!(received.len(), 3);
    for (_, output) in &received {
        assert!(output.error.is_some(), "cancelled output should have error");
    }
}

// ---------------------------------------------------------------------------
// Test 5: Streaming with consumer closing channel early → producer detects
// failing `send`, calls `abort_all` and terminates the function without panicking.
// Covers lines 385-393 (branch `Err(send_error)` + `abort_all` + `break`).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn streaming_closed_consumer_aborts_remaining_tasks() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Slow response to ensure some tasks are still in-flight
    // when the consumer closes the channel. 200ms is sufficient because staggered
    // launch already spreads task starts (DELAY_BASE_STAGGERED_MS = 200ms).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_two_results())
                .insert_header("content-type", "text/html; charset=utf-8")
                .set_delay(Duration::from_millis(200)),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    // Many queries, low parallelism → many pending when rx is dropped.
    let queries: Vec<String> = (0..6).map(|i| format!("q-{i}")).collect();
    let cfg = test_config_wm(Endpoint::Html, 1, queries.clone(), 2);
    let token = CancellationToken::new();
    let (tx, rx) = mpsc::channel(1);

    // Immediate drop of receiver: forces `tx.send().await` to fail as soon as
    // the producer tries to emit the first result, triggering `abort_all`.
    drop(rx);

    let stats = execute_parallel_searches_streaming(
        common::validated_queries_owned(&queries),
        cfg,
        token,
        tx,
    )
    .await
    .expect("streaming should return Ok even with consumer closed");

    assert_eq!(stats.total, 6);
    // At least 1 task may have counted as success/error before abort,
    // but the total processed MUST be <= total sent. The key point is that
    // the function did NOT panic and returned consistent statistics.
    assert!(
        stats.successes + stats.errors <= 6,
        "sum of successes+errors must not exceed total"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Panic inside task — semaphore permit is recovered via RAII drop.
// Validates rule L542: "TEST panic inside a task and permit recovery"
// ---------------------------------------------------------------------------
#[tokio::test]
async fn panic_in_task_restores_semaphore_permit() {
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    let sem = Arc::new(Semaphore::new(2));
    let mut set = JoinSet::new();

    let sem1 = sem.clone();
    set.spawn(async move {
        let _permit = sem1.acquire_owned().await.unwrap();
        panic!("deliberate test panic to validate RAII permit recovery");
    });

    let sem2 = sem.clone();
    set.spawn(async move {
        let _permit = sem2.acquire_owned().await.unwrap();
        42_u32
    });

    let mut panic_count = 0_u32;
    let mut success_count = 0_u32;
    while let Some(result) = set.join_next().await {
        match result {
            Ok(_) => success_count += 1,
            Err(e) => {
                assert!(e.is_panic(), "expected panic, got cancellation");
                panic_count += 1;
            }
        }
    }

    assert_eq!(panic_count, 1);
    assert_eq!(success_count, 1);
    assert_eq!(sem.available_permits(), 2);
}

// ---------------------------------------------------------------------------
// Test 7: Cancel during blocked acquire_owned() — semaphore stays consistent.
// Validates rule L543: "TEST cancel during permit acquisition"
// ---------------------------------------------------------------------------
#[tokio::test]
async fn cancel_during_permit_acquisition_leaves_semaphore_consistent() {
    use tokio::sync::Semaphore;

    let sem = Arc::new(Semaphore::new(1));
    let token = CancellationToken::new();

    let held_permit = sem.clone().acquire_owned().await.unwrap();

    let sem2 = sem.clone();
    let token2 = token.clone();
    let blocked_task = tokio::spawn(async move {
        tokio::select! {
            biased;
            _ = token2.cancelled() => Err("cancelled"),
            result = sem2.acquire_owned() => Ok(result),
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    token.cancel();

    let result = blocked_task.await.unwrap();
    assert!(
        result.is_err(),
        "task must have been cancelled, not acquired"
    );

    drop(held_permit);
    assert_eq!(sem.available_permits(), 1);
}

// ---------------------------------------------------------------------------
// Test 8: Graceful shutdown with tasks in-flight — cancel mid-execution.
// Validates rule L544: "TEST graceful shutdown with tasks in flight"
// ---------------------------------------------------------------------------
#[tokio::test]
async fn graceful_shutdown_cancels_active_tasks_mid_flight() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_two_results())
                .insert_header("content-type", "text/html; charset=utf-8")
                .set_delay(Duration::from_millis(500)),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let queries = vec!["q1".into(), "q2".into(), "q3".into()];
    let cfg = test_config_wm(Endpoint::Html, 1, queries.clone(), 3);
    let token = CancellationToken::new();
    let token_cancel = token.clone();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        token_cancel.cancel();
    });

    let result =
        execute_parallel_searches(common::validated_queries_owned(&queries), cfg, token).await;
    let output = result.expect("should return Ok even when cancelled");

    for search in &output.searches {
        assert!(
            search.error.is_some(),
            "query {:?} should have been cancelled",
            search.query
        );
    }
}

// ---------------------------------------------------------------------------
// Test 9 (Linux-only): RSS stays bounded during parallel fan-out.
// Validates rule L537: "MEASURE RSS during the test to validate the memory limit"
// ---------------------------------------------------------------------------
#[cfg(target_os = "linux")]
#[tokio::test]
async fn rss_stays_bounded_during_parallel_fanout() {
    fn rss_kb() -> u64 {
        std::fs::read_to_string("/proc/self/status")
            .unwrap()
            .lines()
            .find(|l| l.starts_with("VmRSS:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_two_results())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let rss_before = rss_kb();

    let queries: Vec<String> = (0..20).map(|i| format!("rss-q{i}")).collect();
    let cfg = test_config_wm(Endpoint::Html, 1, queries.clone(), 10);
    let token = CancellationToken::new();

    let _ = execute_parallel_searches(common::validated_queries_owned(&queries), cfg, token).await;

    let rss_after = rss_kb();
    let delta_mb = rss_after.saturating_sub(rss_before) / 1024;
    assert!(
        delta_mb < 200,
        "RSS grew by {delta_mb} MB — expected < 200 MB"
    );
}

// ---------------------------------------------------------------------------
// Test 10 (Linux-only): No thread leak after parallel fan-out.
// Validates rule L557: "VERIFY absence of thread leak via ps -T"
// ---------------------------------------------------------------------------
#[cfg(target_os = "linux")]
#[tokio::test]
async fn no_thread_leak_after_parallel_fanout() {
    fn thread_count() -> usize {
        std::fs::read_to_string("/proc/self/status")
            .unwrap()
            .lines()
            .find(|l| l.starts_with("Threads:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_two_results())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1".into()),
    ]);

    let before = thread_count();

    let queries: Vec<String> = (0..10).map(|i| format!("thr-q{i}")).collect();
    let cfg = test_config_wm(Endpoint::Html, 1, queries.clone(), 5);
    let token = CancellationToken::new();

    let _ = execute_parallel_searches(common::validated_queries_owned(&queries), cfg, token).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let after = thread_count();
    assert!(
        after <= before + 2,
        "thread leak: before={before}, after={after}"
    );
}
