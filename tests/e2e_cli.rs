// SPDX-License-Identifier: MIT OR Apache-2.0
//! Testes E2E do binário compilado via `assert_cmd` + `predicates`.
//!
//! Estes testes exercitam o CLI do ponto de vista externo — validações de flags,
//! help, version, exit codes — SEM fazer chamadas HTTP reais. Testes que precisam
//! de HTTP já estão cobertos em `tests/integration_wiremock.rs`.
//!
//! Conforme `rules_rust.md` seção 20.2:
//! - `assert_cmd::Command::cargo_bin(<BIN_NAME>)` para testar binário compilado.
//! - `predicates` para assertions composáveis.
//! - `tempfile::NamedTempFile` e `TempDir` para isolamento.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

const BIN_NAME: &str = "duckduckgo-search-cli";

/// Help text channel: non-TTY `--help` goes to **stderr** (agent-native stdout
/// hygiene, GAP-E2E-V19-HELP-TOKEN-COST). TTY still uses stdout via clap default.
/// assert_cmd always runs non-TTY → look at stderr first, then stdout.
fn help_text(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stderr.contains("Usage:") || stderr.len() > stdout.len() {
        stderr.into_owned()
    } else {
        stdout.into_owned()
    }
}

#[test]
fn help_returns_success_and_contains_usage() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .arg("--help")
        .output()
        .expect("help");
    assert!(output.status.success());
    let text = help_text(&output);
    assert!(
        text.contains("Usage:"),
        "help must contain Usage; channel={text:?}"
    );
}

#[test]
fn version_returns_name_and_version() {
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .arg("--version")
        .assert()
        .success()
        .stdout(
            predicate::str::contains(BIN_NAME)
                .and(predicate::str::contains(env!("CARGO_PKG_VERSION"))),
        );
}

#[test]
fn init_config_help_returns_success() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["init-config", "--help"])
        .output()
        .expect("init-config help");
    assert!(output.status.success());
    let text = help_text(&output);
    assert!(
        text.contains("--force") && text.contains("--dry-run"),
        "init-config help must list --force/--dry-run; got {text}"
    );
}

#[test]
fn init_config_dry_run_returns_valid_json() {
    // Force an isolated DIR for XDG via temporary HOME (effective in dirs crate).
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["init-config", "--dry-run"])
        .env("HOME", temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join(".config"))
        .output()
        .expect("executar init-config");

    assert!(
        output.status.success(),
        "init-config --dry-run deve sucesso"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Valid JSON with a known field.
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout deve ser JSON válido");
    // GAP-E2E-48-005: init-config wire keys are English (`files`).
    assert!(
        value.get("files").is_some(),
        "must contain files key (EN wire; GAP-E2E-48-005)"
    );
}

#[test]
fn no_query_no_stdin_no_file_returns_exit_2() {
    // With empty/redirected stdin to /dev/null, no query is provided.
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .env("RUST_LOG", "error")
        .write_stdin("") // stdin vazio
        .output()
        .expect("executar sem query");

    assert_eq!(
        output.status.code(),
        Some(2),
        "sem query deve retornar exit 2; stdout={:?}, stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn invalid_parallelism_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--parallel", "50", "query"])
        .output()
        .expect("executar com --parallel 50");
    assert_eq!(
        output.status.code(),
        Some(2),
        "--parallel 50 deve retornar exit 2"
    );
}

#[test]
fn invalid_pages_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--pages", "10", "query"])
        .output()
        .expect("executar com --pages 10");
    assert_eq!(
        output.status.code(),
        Some(2),
        "--pages 10 deve retornar exit 2"
    );
}

#[test]
fn invalid_max_content_length_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--max-content-length", "999999", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn invalid_global_timeout_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--global-timeout", "99999", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn proxy_with_invalid_scheme_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--proxy", "ftp://naovalidos", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn verbose_e_quiet_conflitam_retornam_exit_2() {
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--verbose", "--quiet", "query"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn proxy_e_noproxy_conflitam_retornam_exit_2() {
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--proxy", "http://x", "--no-proxy", "query"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn nonexistent_queries_file_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args([
            "--queries-file",
            "/tmp/arquivo_que_realmente_nao_existe_xyz_12345",
        ])
        .output()
        .expect("executar");
    assert_eq!(
        output.status.code(),
        Some(2),
        "queries-file inexistente deve retornar exit 2"
    );
}

#[test]
fn valid_queries_file_is_read_correctly() {
    // Create a temporary file with 3 queries.
    let mut file = tempfile::NamedTempFile::new().expect("tempfile");
    writeln!(file, "foo bar").unwrap();
    writeln!(file).unwrap(); // linha vazia ignorada
    writeln!(file, "baz qux").unwrap();
    writeln!(file, "quux").unwrap();
    let path = file.path().to_path_buf();

    // Run with a short global-timeout to avoid the test going to the network
    // for too long; the point is to exercise file READING, not validate HTTP.
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args([
            "--queries-file",
            path.to_str().unwrap(),
            "--global-timeout",
            "1",
            "--quiet",
            "--format",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .expect("executar");

    // Expected exit: 0/1/3/4/5 (NOT 2, which is invalid config).
    // The key point: code != 2 means the configuration was accepted.
    let code = output.status.code().unwrap_or(-1);
    assert!(
        code != 2,
        "queries-file válido deve ser ACEITO (code != 2), mas veio {code}; \
         stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unknown_format_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--format", "xml", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn unknown_flag_returns_exit_2() {
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .arg("--flag-que-nao-existe-xyz-12345")
        .assert()
        .failure()
        .code(2);
}

// ---------------------------------------------------------------------------
// Pipe and exit code tests for --help (SIGPIPE regression prevention)
// ---------------------------------------------------------------------------

#[test]
fn long_help_contains_exit_codes_section() {
    // Verify that `--help` (long help) shows the EXIT CODES section added
    // via after_long_help in clap. Prevents regression if someone removes the attribute.
    // Non-TTY: help on stderr (GAP-E2E-V19-HELP-TOKEN-COST).
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .arg("--help")
        .output()
        .expect("help");
    assert!(output.status.success());
    let text = help_text(&output);
    assert!(text.contains("EXIT CODES:"), "missing EXIT CODES in {text}");
    assert!(text.contains("PIPE USAGE:"), "missing PIPE USAGE in {text}");
    assert!(
        text.contains("Zero results across all queries"),
        "missing exit 5 description in {text}"
    );
}

#[test]
fn short_help_does_not_contain_exit_codes() {
    // `-h` (short help) NÃO deve exibir after_long_help — apenas `--help` exibe.
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .arg("-h")
        .output()
        .expect("short help");
    assert!(output.status.success());
    let text = help_text(&output);
    assert!(
        !text.contains("EXIT CODES:"),
        "short help must not include after_long_help EXIT CODES"
    );
}

#[test]
fn help_channel_does_not_lose_bytes() {
    // Capture help text and validate it has a reasonable size.
    // Non-TTY: stderr (agent-native); prevents SIGPIPE truncation regressions.
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .arg("--help")
        .output()
        .expect("executar --help");
    assert!(output.status.success(), "exit code deve ser 0");
    let text = help_text(&output);
    assert!(
        text.len() > 500,
        "help text must have at least 500 bytes, got {}",
        text.len()
    );
}

#[test]
fn retries_above_max_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--retries", "99", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn per_host_limit_above_max_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--per-host-limit", "99", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn timeout_zero_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--timeout", "0", "query"])
        .output()
        .expect("executar com --timeout 0");
    assert_eq!(
        output.status.code(),
        Some(2),
        "--timeout 0 deve retornar exit 2 (configuração inválida)"
    );
}

#[test]
fn output_with_path_traversal_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--output", "/tmp/../../etc/passwd", "query"])
        .output()
        .expect("executar com --output path traversal");
    assert_eq!(
        output.status.code(),
        Some(2),
        "--output com path traversal deve retornar exit 2 (configuração inválida)"
    );
}

// =============================================================================
// Teste do handler SIGINT instalado em src/main.rs
// =============================================================================
//
// The binary installs a SIGINT/Ctrl+C handler in `tokio::spawn` (lines 22-27
// of `src/main.rs`). This handler awaits `tokio::signal::ctrl_c()`, logs a
// warning via `tracing` and calls `cancelamento.cancel()` propagating the signal
// to the in-flight pipeline.
//
// To exercise these lines we need to:
//   1. Launch the REAL binary (not the lib) with a slow load (mock HTTP that
//      takes a long time before responding).
//   2. Wait long enough for the handler to be installed AND an HTTP request
//      to be in-flight (otherwise SIGINT arrives before the handler).
//   3. Send SIGINT via `kill(pid, SIGINT)`.
//   4. Confirm the process terminates in a reasonable time (cancellation
//      propagated) with exit code != 0.
//
// Gated on `#[cfg(unix)]` — Windows has different semantics for Ctrl+C.
#[cfg(unix)]
mod sigint_handler {
    use super::BIN_NAME;
    use std::io::Read;
    use std::process::{Command as StdCommand, Stdio};
    use std::time::{Duration, Instant};

    // Use typed `libc::kill` (Pass 44) — no raw `extern "C"` ad-hoc.

    /// Waits for the `Child` to terminate up to `timeout_total`, polling every 50ms.
    /// Returns `Ok(status)` if it terminated, `Err(())` if the timeout was exceeded.
    fn wait_with_timeout(
        child: &mut std::process::Child,
        timeout_total: Duration,
    ) -> Result<std::process::ExitStatus, ()> {
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {
                    if start.elapsed() > timeout_total {
                        return Err(());
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => return Err(()),
            }
        }
    }

    /// Starts wiremock returning 200 with a 30s delay and launches the binary
    /// pointing the HTML endpoint at that mock. Then sends SIGINT and
    /// validates that the process terminates within the timeout (cancellation occurred).
    ///
    /// NOTE: this test depends on timing (the signal handler must be installed
    /// before SIGINT arrives). 600ms warm-up is comfortable on most CIs,
    /// but on EXTREMELY saturated runners it may occasionally fail — increase `WARMUP_MS`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sigint_dispara_cancelamento_e_termina_processo() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        const WARMUP_MS: u64 = 600;
        const HARD_TIMEOUT_PROCESS: Duration = Duration::from_secs(8);

        // 1. Mock HTTP that NEVER responds quickly — forces the request to stay
        // in-flight while we send SIGINT.
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&mock_server)
            .await;

        // 2. Locate the compiled binary via assert_cmd.
        let bin_path = assert_cmd::cargo::cargo_bin(BIN_NAME);
        assert!(bin_path.exists(), "binário deve existir: {bin_path:?}");

        // 3. Spawn process via std::process::Command (we need the PID).
        // GAP-PROC-004: explicit Stdio on all three streams (never inherit stdin).
        // V18: CLI `--base-url-*` installs EndpointPolicy (env BASE_URL is dead).
        // Residual HTTP still needs feature `http-test-harness` + HTTP_TEST=1;
        // without it Chrome path runs against the mock origin (still in-flight).
        let mut child = StdCommand::new(&bin_path)
            .arg("rust async")
            .arg("--global-timeout")
            .arg("60") // high enough to NOT be the reason for termination
            .arg("--retries")
            .arg("0")
            .arg("--quiet")
            .arg("--base-url-html")
            .arg(mock_server.uri())
            .arg("--base-url-lite")
            .arg(mock_server.uri())
            .env("DUCKDUCKGO_SEARCH_CLI_HTTP_TEST", "1")
            .env("RUST_LOG", "off")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn binário");

        let pid = child.id() as i32;

        // 4. Warm-up: tokio must register the handler BEFORE SIGINT.
        std::thread::sleep(Duration::from_millis(WARMUP_MS));

        // Sanity: still running? If already terminated, the test is inconclusive.
        if let Some(status) = child.try_wait().expect("try_wait") {
            panic!(
                "processo terminou ANTES do SIGINT (status={:?}); teste \
                 inválido — possivelmente o mock não foi atingido",
                status.code()
            );
        }

        // 5. Send SIGINT via typed libc (child PID from our spawn; not self).
        // SAFETY:
        // - `pid` is the positive process id of the child we just spawned.
        // - `SIGINT` is a valid signal number; no pointer ownership transfer.
        // - Test process is not the target (child.id() ≠ self).
        let kill_result = unsafe { libc::kill(pid, libc::SIGINT) };
        assert_eq!(
            kill_result, 0,
            "kill(pid={pid}, SIGINT) must return 0, got {kill_result}"
        );

        // 6. Wait for termination within a reasonable time.
        let status = match wait_with_timeout(&mut child, HARD_TIMEOUT_PROCESS) {
            Ok(status) => status,
            Err(()) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("processo NÃO terminou dentro de {HARD_TIMEOUT_PROCESS:?} após SIGINT — handler não funcionou ou cancelamento não propagou");
            }
        };

        // 7. Collect stderr for diagnosis on failure.
        let mut stderr_buf = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut stderr_buf);
        }

        // The process MUST have terminated due to our action (not with success 0).
        // Accept any exit code != 0 or termination by signal.
        // - If `tokio::signal::ctrl_c()` intercepted (normal path):
        //   `run()` returns with some error/cancel exit code.
        // - If SIGINT arrived BEFORE the handler was ready: process dies
        //   by signal and `code()` is None — also evidence of SIGINT.
        let code = status.code();
        assert!(
            code != Some(0),
            "processo terminou com SUCESSO (0) após SIGINT; esperado != 0. \
             stderr={stderr_buf:?}"
        );
    }
}

/// v1.0.2 CM-01: legacy 5×10 dual+fetch load must fail-fast exit 2 under 180s.
/// Pure local math — no network, no Chrome.
#[test]
fn deep_research_legacy_5_by_10_fail_fast_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "-f",
            "json",
            "--global-timeout",
            "180",
            "--fetch-content-cap",
            "10",
            "deep-research",
            "budget probe query",
            "--max-sub-queries",
            "5",
            "--no-auto-contention-budget",
        ])
        .output()
        .expect("run deep-research fail-fast");

    assert_eq!(
        output.status.code(),
        Some(2),
        "legacy 5x10 must exit 2; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("budget_underflow"),
        "stdout must contain budget_underflow: {stdout}"
    );
}

/// v1.0.2 --print-budget dry estimate without Chrome.
#[test]
fn deep_research_print_budget_legacy_5_by_10_budget_ok_false() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "-f",
            "json",
            "--global-timeout",
            "180",
            "--fetch-content-cap",
            "10",
            "deep-research",
            "budget probe query",
            "--max-sub-queries",
            "5",
            "--print-budget",
            "--no-auto-contention-budget",
        ])
        .output()
        .expect("run print-budget");

    assert!(
        output.status.success(),
        "print-budget must exit 0; stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("json budget");
    assert_eq!(v["type"], "deep_research_budget");
    assert_eq!(v["budget_ok"], false);
}

/// GAP-PRINT-BUDGET-QUERY: agent discovery without inventing a QUERY.
#[test]
fn deep_research_print_budget_without_query_exits_0() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["-q", "deep-research", "--print-budget"])
        .output()
        .expect("run print-budget no query");

    assert!(
        output.status.success(),
        "print-budget without QUERY must exit 0; stderr={:?} stdout={:?}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("json budget");
    assert_eq!(v["type"], "deep_research_budget");
    assert_eq!(v["max_sub_queries"], 3);
    assert_eq!(v["fetch_content_cap"], 4);
}

/// GAP-NO-INPUT: flag is accepted (no-op non-interactive contract).
#[test]
fn no_input_flag_accepted_on_print_budget() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["-q", "--no-input", "deep-research", "--print-budget"])
        .output()
        .expect("run with --no-input");
    assert!(
        output.status.success(),
        "--no-input must parse; stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// GAP-FIELDS-PROJECT: unknown field → exit 2 before Chrome.
#[test]
fn fields_unknown_exits_invalid_config() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "-f",
            "json",
            "--fields",
            "url,not_a_real_field",
            "--no-fetch-content",
            "rust",
        ])
        .output()
        .expect("run fields unknown");
    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown --fields must exit 2; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// GAP-E2E-V11-UNKNOWN-AS-QUERY / V12: hyphenated typo is not a SERP query.
#[test]
fn hyphenated_unknown_token_exits_invalid_config_not_serp() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["-q", "-f", "json", "not-a-real-subcommand"])
        .output()
        .expect("run typo token");
    assert_eq!(
        output.status.code(),
        Some(2),
        "hyphenated typo must exit 2 (not SERP 0); stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// GAP-E2E-V11-PREFLIGHT-NO-QUERY / V12: --pre-flight alone is health, not "no query".
#[test]
fn pre_flight_without_query_is_not_no_query_error() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["-q", "-f", "json", "--pre-flight"])
        .output()
        .expect("run pre-flight alone");
    let code = output.status.code();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Must not be the bare "no query provided" path. Probe path may exit 0/1/2
    // depending on Chrome host readiness — never the empty-args usage failure alone.
    assert!(
        code != Some(2) || !stdout.contains("no query") && !stderr.to_ascii_lowercase().contains("no query provided"),
        "pre-flight alone must not fail as missing QUERY; code={code:?} stdout={stdout:?} stderr={stderr:?}"
    );
}

/// V12: --chrome-session-retries is a recognized CLI flag (help / parse).
#[test]
fn chrome_session_retries_flag_in_help() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .arg("--help")
        .output()
        .expect("help");
    assert!(output.status.success());
    let text = help_text(&output);
    assert!(
        text.contains("chrome-session-retries"),
        "help must list chrome-session-retries; got {text}"
    );
}

/// GAP-E2E-V19-LIMIT-FLAG-MISSING: `--limit` is registered (not unexpected arg).
#[test]
fn limit_flag_in_help_and_not_unknown() {
    let help = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .arg("--help")
        .output()
        .expect("help");
    assert!(help.status.success());
    let text = help_text(&help);
    assert!(
        text.contains("--limit"),
        "help must document --limit post-SERP cap; got snippet {}",
        &text.chars().take(200).collect::<String>()
    );
    // Missing query still exit 2, but NOT "unexpected argument --limit".
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["-q", "-f", "json", "--limit", "2", "--no-fetch-content"])
        .output()
        .expect("limit without query");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.to_ascii_lowercase().contains("unexpected argument"),
        "--limit must be a known flag; got {combined}"
    );
}

/// GAP-E2E-V19-FILTER-SYNTAX-FOOTGUN: invalid filter → exit 2 before Chrome.
#[test]
fn filter_tilde_equals_exits_invalid_config() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "-f",
            "json",
            "--filter",
            "titulo~=Rust",
            "--no-fetch-content",
            "rust",
        ])
        .output()
        .expect("filter ~=");
    assert_eq!(
        output.status.code(),
        Some(2),
        "titulo~= must exit 2; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("field~needle")
            || combined.contains("titulo~")
            || combined.contains("invalid"),
        "error should tip field~needle; got {combined}"
    );
}

/// GAP-E2E-V19-FILTER-SYNTAX-FOOTGUN: `posicao=1` is not bare-needle.
#[test]
fn filter_equality_exits_invalid_config() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "-f",
            "json",
            "--filter",
            "posicao=1",
            "--no-fetch-content",
            "rust",
        ])
        .output()
        .expect("filter =");
    assert_eq!(
        output.status.code(),
        Some(2),
        "posicao=1 must exit 2 (not SERP exit 5); stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// GAP-E2E-V19-JSON-PRETTY-DEFAULT: `--pretty` is a known flag.
#[test]
fn pretty_flag_accepted_in_help() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .arg("--help")
        .output()
        .expect("help");
    assert!(output.status.success());
    let text = help_text(&output);
    assert!(
        text.contains("--pretty"),
        "help must document --pretty; got {}",
        &text.chars().take(200).collect::<String>()
    );
}

/// GAP-E2E-V11-HELP-POLLUTION / V13: doctor --help must not dump SERP flags.
#[test]
fn doctor_help_excludes_serp_pollution() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["doctor", "--help"])
        .output()
        .expect("doctor --help");
    assert!(output.status.success(), "doctor --help must exit 0");
    // Subcommand help still uses clap default channel; accept either.
    let help = help_text(&output);
    let lines = help.lines().count();
    // V18: clap wrap_help / terminal width can yield ~102 lines; ban-list is the
    // real pollution gate. Cap keeps doctor help from regressing to root SERP dump.
    assert!(
        lines <= 120,
        "doctor --help too long ({lines} lines); expected ≤120 after V13 un-global SERP"
    );
    for ban in [
        "--vertical",
        "--shared-session-verticals",
        "--fetch-content-cap",
        "--match-platform-ua",
    ] {
        assert!(
            !help.contains(ban),
            "doctor --help must not contain {ban}:
{help}"
        );
    }
    // Agent-true globals remain
    assert!(
        help.contains("--quiet") || help.contains("-q"),
        "doctor --help should still document -q/--quiet"
    );
}

/// GAP-E2E-V11-HELP-POLLUTION / V13: locale --help stays compact.
#[test]
fn locale_help_excludes_serp_pollution() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["locale", "--help"])
        .output()
        .expect("locale --help");
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.lines().count() <= 100,
        "locale --help too long: {} lines",
        help.lines().count()
    );
    assert!(
        !help.contains("--vertical"),
        "locale --help must not contain --vertical"
    );
}

/// DEEP-E2E-05: forced invalid Chrome path → exit 2 (not 5), chrome taxonomy.
#[test]
fn deep_research_invalid_chrome_path_exit_2_not_5() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "deep-research",
            "rust async tokio",
            "--max-sub-queries",
            "1",
            "--no-news",
            "--no-fetch-content",
            "--chrome-path",
            "/nonexistent/chrome-v13-e2e-does-not-exist",
            "--global-timeout",
            "30",
        ])
        .output()
        .expect("run deep invalid chrome");
    let code = output.status.code();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        code,
        Some(2),
        "invalid chrome-path must be exit 2 (config/chrome), not 5; stdout={stdout} stderr={stderr}"
    );
    assert_ne!(code, Some(5), "must never map chrome failure to zero-results exit 5");
}

/// DEEP-E2E-06: --fields / --filter accepted on deep-research after subcommand.
#[test]
fn deep_research_fields_filter_accepted_after_subcommand() {
    // Parse-level: print-budget avoids Chrome; flags must not be UnknownArgument.
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "deep-research",
            "--print-budget",
            "--fields",
            "url,titulo",
            "--filter",
            "host:example.com",
            "--max-sub-queries",
            "1",
            "--no-news",
            "--no-fetch-content",
        ])
        .output()
        .expect("deep fields/filter print-budget");
    assert_eq!(
        output.status.code(),
        Some(0),
        "fields/filter after deep-research must parse; stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("print-budget JSON");
    assert_eq!(v["type"], "deep_research_budget");
}

/// WIRE-PT-KEYS mitigation (V17): EN `--fields` aliases parse; wire rename stays major.
///
/// Serialize keys are English (`results`, `title`, …). Agents use
/// `--fields title,url`. Opt-in PT: `--wire-keys pt`.
#[test]
fn fields_en_aliases_accepted_on_deep_print_budget() {
    let deep = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "deep-research",
            "--print-budget",
            "--fields",
            "title,url,sources",
            "--max-sub-queries",
            "1",
            "--no-news",
            "--no-fetch-content",
        ])
        .output()
        .expect("deep EN fields print-budget");
    assert_eq!(
        deep.status.code(),
        Some(0),
        "EN --fields aliases must parse on deep-research; stderr={:?}",
        String::from_utf8_lossy(&deep.stderr)
    );
    let v: serde_json::Value =
        serde_json::from_slice(&deep.stdout).expect("print-budget JSON");
    assert_eq!(v["type"], "deep_research_budget");
    // Unknown EN alias must still fail closed (exit 2).
    let bad = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "deep-research",
            "--print-budget",
            "--fields",
            "title,not_a_wire_field",
            "--max-sub-queries",
            "1",
            "--no-news",
            "--no-fetch-content",
        ])
        .output()
        .expect("bad fields");
    assert_eq!(
        bad.status.code(),
        Some(2),
        "unknown --fields entry must exit 2; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&bad.stdout),
        String::from_utf8_lossy(&bad.stderr)
    );
}

/// DEEP-E2E-10: --require-results is accepted (parse + budget path).
#[test]
fn deep_research_require_results_flag_with_print_budget() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "deep-research",
            "--print-budget",
            "--require-results",
            "--max-sub-queries",
            "1",
            "--no-news",
            "--no-fetch-content",
        ])
        .output()
        .expect("require-results print-budget");
    assert_eq!(
        output.status.code(),
        Some(0),
        "require-results must parse with print-budget; stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// GAP-E2E-V14-UNDERSCORE-TYPO-AS-QUERY / V15.1: `deep_research` must not launch
/// Chrome as a SERP query — fail-closed exit 2 with tip for `deep-research`.
#[test]
fn underscore_deep_research_typo_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["-q", "deep_research"])
        .output()
        .expect("run deep_research typo");
    assert_eq!(
        output.status.code(),
        Some(2),
        "underscore typo must exit 2; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("invalid_config") || stdout.contains("deep-research"),
        "stdout must tip deep-research: {stdout}"
    );
    assert!(
        stdout.contains("deep_research") || stdout.contains("deep-research"),
        "stdout must mention the typo or canonical name: {stdout}"
    );
}

/// GAP-E2E-V14-PRINT-SCHEMA-ROOT-MISSING / V15.1: root `--print-schema` = catalog JSON.
#[test]
fn print_schema_root_emits_catalog() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["--print-schema", "-q"])
        .output()
        .expect("print-schema");
    assert_eq!(
        output.status.code(),
        Some(0),
        "print-schema must exit 0; stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be JSON catalog");
    assert_eq!(v["type"], "schema_catalog");
    assert!(
        v["count"].as_u64().unwrap_or(0) >= 1,
        "catalog must list schemas: {v}"
    );
    assert!(
        output.stderr.is_empty(),
        "agent-native: stderr must be empty with -q, got {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// GAP-E2E-V14-REQUIRE-RESULTS-SEARCH-MISSING / V15.1: bare search accepts flag.
#[test]
fn bare_search_require_results_flag_parses() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "--require-results",
            "--chrome-path",
            "/nonexistent/chrome-v15-require-results",
            "-n",
            "1",
            "rust",
        ])
        .output()
        .expect("require-results bare");
    // Unknown flag would be clap exit 2 with "unexpected argument"; chrome missing
    // is also exit 2 but message differs — ensure not "unexpected argument".
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains("unexpected argument '--require-results'"),
        "bare search must accept --require-results: {combined}"
    );
}

/// GAP-E2E-V14-DOCTOR-OK-MASKS-FAILED-CHECKS / V15.1: additive status + severity.
#[test]
fn doctor_report_includes_status_and_severity() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["doctor", "-q"])
        .output()
        .expect("doctor");
    assert_eq!(
        output.status.code(),
        Some(0),
        "doctor should exit 0 when Chrome present; stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("doctor stdout JSON");
    assert_eq!(v["type"], "doctor");
    let status = v["status"]
        .as_str()
        .expect("status field required (healthy|degraded|unhealthy)");
    assert!(
        matches!(status, "healthy" | "degraded" | "unhealthy"),
        "unexpected status {status}"
    );
    assert!(v["failed_checks"].is_array(), "failed_checks array required");
    let checks = v["checks"].as_array().expect("checks array");
    assert!(!checks.is_empty());
    assert!(
        checks.iter().all(|c| c.get("severity").is_some()),
        "every check needs severity: {checks:?}"
    );
}

/// GAP-E2E-V14-PROBE-DEEP-FLAG-ORDER / V15.1: `doctor --probe-deep` parses.
#[test]
fn doctor_probe_deep_flag_accepted_in_help() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["doctor", "--help"])
        .output()
        .expect("doctor --help");
    assert!(output.status.success());
    let help = help_text(&output);
    assert!(
        help.contains("--probe-deep"),
        "doctor --help must document --probe-deep: {help}"
    );
}

/// DEEP-E2E-07: multi-sub thin budget is honest offline (no Chrome).
/// Live multi-sub SERP remains residual (flakiness / serial Chrome cost).
#[test]
fn deep_research_multi_sub_thin_print_budget_ok() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "deep-research",
            "--print-budget",
            "--max-sub-queries",
            "2",
            "--no-fetch-content",
        ])
        .output()
        .expect("print-budget multi-sub thin");
    assert!(
        output.status.success(),
        "print-budget multi-sub thin must exit 0; stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("budget JSON");
    assert_eq!(v["type"], "deep_research_budget");
    assert_eq!(v["max_sub_queries"], 2);
    assert_eq!(v["fetch_content"], false);
    assert_eq!(v["budget_ok"], true);
    assert!(
        v["gated_seconds"].as_u64().unwrap_or(u64::MAX) <= 180,
        "thin multi-sub must fit default global: {v}"
    );
}

/// DEEP-E2E-09: default heavy workload (3 sub + dual + fetch) print-budget shape
/// + fail-fast when global timeout is too tight.
#[test]
fn deep_research_default_heavy_print_budget_and_underflow() {
    let heavy = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["-q", "deep-research", "--print-budget"])
        .output()
        .expect("default print-budget");
    assert!(
        heavy.status.success(),
        "default print-budget must exit 0; stderr={:?}",
        String::from_utf8_lossy(&heavy.stderr)
    );
    let v: serde_json::Value =
        serde_json::from_slice(&heavy.stdout).expect("budget JSON");
    assert_eq!(v["type"], "deep_research_budget");
    assert_eq!(v["max_sub_queries"], 3);
    assert_eq!(v["fetch_content"], true);
    assert_eq!(v["dual_vertical"], true);
    assert!(v["runtime_dual_multiproc"].as_bool().unwrap_or(false));
    assert!(v.get("suggested_global_timeout").is_some());
    assert!(v.get("shell_timeout_hint").is_some());
    assert!(v.get("contention_factor_percent").is_some());
    // With host chrome_n contention, budget_ok may be false for GT=200 default;
    // lab chrome_n=0 defaults still fit when suggested <= global.
    assert!(
        v["gated_seconds"].as_u64().unwrap_or(0) > 30,
        "default heavy gated_seconds should exceed a tight 30s fence: {v}"
    );

    let tight = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "deep-research",
            "heavy budget probe",
            "--global-timeout",
            "30",
            "--max-sub-queries",
            "3",
            "--no-auto-contention-budget",
        ])
        .output()
        .expect("heavy underflow");
    assert_eq!(
        tight.status.code(),
        Some(2),
        "default-ish heavy under global 30 must exit 2; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&tight.stdout),
        String::from_utf8_lossy(&tight.stderr)
    );
    let stdout = String::from_utf8_lossy(&tight.stdout);
    assert!(
        stdout.contains("budget_underflow"),
        "must emit budget_underflow: {stdout}"
    );
}

/// DEEP-E2E-11: multi-sub dual-fail chrome taxonomy (invalid path) → exit 2,
/// not zero-results exit 5; sub_queries carry chrome_not_found.
#[test]
fn deep_research_multi_sub_invalid_chrome_dual_fail_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "deep-research",
            "rust async",
            "--max-sub-queries",
            "2",
            "--no-news",
            "--no-fetch-content",
            "--chrome-path",
            "/nonexistent/chrome-v16-multi-sub-e2e",
            "--global-timeout",
            "30",
        ])
        .output()
        .expect("multi-sub invalid chrome");
    let code = output.status.code();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        code,
        Some(2),
        "multi-sub chrome fail must exit 2; stdout={stdout} stderr={stderr}"
    );
    assert_ne!(code, Some(5), "must never map chrome failure to exit 5");
    assert!(
        stdout.contains("chrome_not_found") || stdout.contains("chrome"),
        "stdout must surface chrome taxonomy: {stdout}"
    );
}

/// DEEP-E2E-08 offline contract: exit codes 130/143 + parse of cancel-related flags.
#[test]
fn deep_research_cancel_exit_codes_contract_documented() {
    // Unix shell convention: 128 + signal. Keep in sync with docs/AGENTS + signals.
    const EXIT_SIGINT: i32 = 130; // 128 + 2
    const EXIT_SIGTERM: i32 = 143; // 128 + 15
    assert_eq!(EXIT_SIGINT, 128 + 2);
    assert_eq!(EXIT_SIGTERM, 128 + 15);
    // Parse-level: deep-research still accepts --global-timeout / --cancel-grace-secs.
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "--cancel-grace-secs",
            "2",
            "deep-research",
            "--print-budget",
            "--global-timeout",
            "60",
        ])
        .output()
        .expect("print-budget with global-timeout");
    assert!(
        output.status.success(),
        "global-timeout must parse with print-budget; stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// DEEP-E2E-08 live: SIGTERM while deep-research is in-flight → exit **143**.
///
/// Uses real Chrome + Xvfb (anti-CF headed path). Thin load (1 sub, no-fetch,
/// no-news) so warm-up is short. `--cancel-grace-secs 2` keeps reap bounded.
///
/// Accepts 143 (force-exit after grace / SIGTERM mapping) as primary success.
/// Soft-pass if the process exits 2 before SIGTERM (Chrome unavailable on host)
/// — that is environment, not cancel-contract failure.
#[cfg(unix)]
#[test]
fn deep_research_sigterm_exit_143_live() {
    use std::io::Read;
    use std::process::{Command as StdCommand, Stdio};
    use std::time::{Duration, Instant};

    const WARMUP_MS: u64 = 1_200;
    const HARD_TIMEOUT: Duration = Duration::from_secs(20);

    let bin_path = assert_cmd::cargo::cargo_bin(BIN_NAME);
    assert!(bin_path.exists(), "binary must exist: {bin_path:?}");

    let mut child = StdCommand::new(&bin_path)
        .args([
            "-q",
            "--cancel-grace-secs",
            "2",
            "--global-timeout",
            "90",
            "deep-research",
            "rust async tokio cancel e2e",
            "--max-sub-queries",
            "1",
            "--no-news",
            "--no-fetch-content",
        ])
        .env("RUST_LOG", "off")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn deep-research");

    let pid = child.id() as i32;
    std::thread::sleep(Duration::from_millis(WARMUP_MS));

    // If already dead (Chrome missing / budget / invalid), classify without SIGTERM.
    if let Some(status) = child.try_wait().expect("try_wait") {
        let code = status.code();
        // Environment soft-pass: chrome fail-closed exit 2 before cancel window.
        assert!(
            code == Some(2) || code == Some(4) || code == Some(130) || code == Some(143),
            "pre-SIGTERM exit must be chrome/timeout/cancel taxonomy, got {code:?}"
        );
        return;
    }

    // SAFETY: pid is our positive child process id; SIGTERM is a valid signal.
    let kill_result = unsafe { libc::kill(pid, libc::SIGTERM) };
    assert_eq!(
        kill_result, 0,
        "kill(pid={pid}, SIGTERM) must return 0, got {kill_result}"
    );

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > HARD_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("deep-research did not exit within {HARD_TIMEOUT:?} after SIGTERM");
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => panic!("try_wait error: {e}"),
        }
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut stdout);
    }
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }

    let code = status.code();
    // Primary: cooperative force-exit maps SIGTERM → 143.
    // Also accept None (killed by signal without wait status code on some kernels).
    assert!(
        code == Some(143)
            || code == Some(130)
            || code.is_none()
            || (code == Some(2) && stdout.contains("chrome")),
        "DEEP-E2E-08: expected exit 143 (SIGTERM) after cancel; got {code:?}; \
         stdout={stdout:?} stderr={stderr:?}"
    );
    if code == Some(143) {
        // Cancel envelope when DeepInFlightGuard was armed (best-effort).
        if !stdout.is_empty() {
            assert!(
                stdout.contains("cancelled")
                    || stdout.contains("deep_research_error")
                    || stdout.contains("\"exit\":143"),
                "SIGTERM 143 with stdout should emit cancel envelope; got {stdout}"
            );
        }
    }
}

/// v2.0.0 G9: unknown --sort key fails closed before Chrome (exit 2).
#[test]
fn sort_unknown_exits_invalid_config() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "-f",
            "json",
            "--sort",
            "nope",
            "--no-fetch-content",
            "rust",
        ])
        .output()
        .expect("sort unknown");
    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown --sort must exit 2; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// v2.0.0 G9: unknown --dedupe-by fails closed before Chrome (exit 2).
#[test]
fn dedupe_by_unknown_exits_invalid_config() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "-f",
            "json",
            "--dedupe-by",
            "title",
            "--no-fetch-content",
            "rust",
        ])
        .output()
        .expect("dedupe unknown");
    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown --dedupe-by must exit 2; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// v2.0.0 G9/G10: agent ops flags appear in --help (stderr) so agents discover them.
#[test]
fn agent_ops_flags_in_help() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args(["--help"])
        .output()
        .expect("help");
    let help = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for needle in ["--sort", "--dedupe-by", "--count-only", "--truncate-content", "--max-output-bytes"] {
        assert!(
            help.contains(needle),
            "help missing {needle}"
        );
    }
}


/// G23: --no-warmup without allow is fail-closed (exit 2) before Chrome.
#[test]
fn no_warmup_blocked_without_allow() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("compiled binary")
        .args([
            "-q",
            "-f",
            "json",
            "--no-warmup",
            "--no-fetch-content",
            "rust",
        ])
        .output()
        .expect("no-warmup");
    assert_eq!(
        output.status.code(),
        Some(2),
        "no-warmup must exit 2; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
