// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light format/serde + stdout I/O.
// GAP-PAR-040: serde/format of large multi-query + fetch-content payloads runs
// via `concurrency::run_cpu_bound` (`emit_*_async`) so the Tokio worker is free.
// Sync `emit_result` / `emit_ndjson` remain for tests and pure-sync handlers.
//! Formatting and emission of the final result to stdout or a file.
//!
//! **INVIOLABLE RULE (MP-06)**:
//! - This is the ONLY module authorized to use `println!` / `write!` /
//!   `writeln!` against **stdout** (or the `--output` file).
//! - Human-facing **stderr** (operator tips, cancel notices, install hints)
//!   must go through [`emit_stderr`] — never raw `eprintln!` in other modules.
//! - Diagnostic logs use `tracing::*` (subscriber writes stderr separately).
//!
//! Supported formats:
//! - `json` (default in pipe / whenever LLM consumes): JSON pretty-print.
//! - `text` (default in TTY): compact format optimized for LLM tokens and
//!   human reading — `[N] title / URL / snippet`.
//! - `markdown`: Markdown rendering (ideal for `.md` files / GitHub).
//! - `auto`: TTY detection — `text` in interactive terminal, `json` in pipe.
//!
//! Output routing:
//! - Without `--output PATH`: writes to `stdout`.
//! - With `--output PATH`: creates parent directories if needed, writes to
//!   the file with 0o644 permissions on Unix.
//!
//! # Module layout
//!
//! | Submodule | Responsibility |
//! |-----------|----------------|
//! | [`emit`] | stdout/stderr/file sinks, NDJSON & stream emit, broken-pipe helpers |
//! | [`format`] | JSON / text / Markdown / TSV formatters + display sanitization |

mod emit;
mod format;

pub use emit::{
    emit, emit_multi, emit_ndjson, emit_ndjson_async, emit_payload, emit_payload_async,
    emit_result, emit_result_async, emit_stderr, emit_stream_markdown,
    emit_stream_markdown_async, emit_stream_text, emit_stream_text_async, print_line_stdout,
    serialize_json_async,
};
pub(crate) use emit::is_broken_pipe;

#[cfg(test)]
mod tests {
    use super::emit::{
        emit_ndjson, emit_ndjson_async, emit_payload, emit_result, emit_result_async,
        emit_stderr, emit_stream_markdown, emit_stream_text, is_broken_pipe, map_serde_write,
        serialize_json_async, write_to_file,
    };
    use super::format::{
        escape_markdown, escape_markdown_url, format_multi_markdown, format_multi_text,
        format_result_text, format_single_markdown, format_single_text, resolve_auto_format,
        sanitize_untrusted_display,
    };
    use crate::error::CliError;
    use crate::types::{MultiSearchOutput, OutputFormat, SearchMetadata, SearchOutput, SearchResult};
    use std::collections::BTreeMap;
    use std::fs;
    use std::io;
    use std::path::Path;

    fn test_output() -> SearchOutput {
        SearchOutput {
            query: "teste".to_string(),
            engine: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: crate::types::test_timestamp_offset(),
            region: "br-pt".to_string(),
            result_count: 1,
            results: vec![SearchResult {
                position: 1,
                title: "Title with [brackets]".to_string(),
                url: crate::types::HttpUrl::for_test("https://example.com"),
                display_url: Some("exemplo.com".to_string()),
                snippet: Some("Description with *asterisks* and `backticks`".to_string()),
                original_title: None,
                content: None,
                content_size: None,
                content_extraction_method: None,
            }],
            pages_fetched: 1,
            news: None,
            news_count: None,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 100,
                selectors_hash: "abc1234567890def".to_string(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                chrome_attempted: false,
                user_agent: "Mozilla/5.0".to_string(),
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

    #[test]
    fn resolve_auto_format_for_file_always_json() {
        let path = Path::new("/tmp/teste.json");
        assert_eq!(
            resolve_auto_format(OutputFormat::Auto, Some(path)),
            OutputFormat::Json
        );
    }

    #[test]
    fn resolver_formato_auto_preserva_formatos_concretos() {
        assert_eq!(
            resolve_auto_format(OutputFormat::Json, None),
            OutputFormat::Json
        );
        assert_eq!(
            resolve_auto_format(OutputFormat::Text, None),
            OutputFormat::Text
        );
        assert_eq!(
            resolve_auto_format(OutputFormat::Markdown, None),
            OutputFormat::Markdown
        );
    }

    #[test]
    fn format_single_text_includes_query_and_results() {
        let output = test_output();
        let text = format_single_text(&output);
        assert!(text.contains("Query: teste"));
        assert!(text.contains("Engine: duckduckgo"));
        assert!(text.contains("Endpoint: html"));
        assert!(text.contains("Results: 1"));
        assert!(text.contains("[1] Title with [brackets]"));
        assert!(text.contains("https://example.com"));
        assert!(text.contains("Description with *asterisks*"));
    }

    #[test]
    fn format_single_text_handles_zero_results() {
        let mut output = test_output();
        output.result_count = 0;
        output.results = vec![];
        let text = format_single_text(&output);
        assert!(text.contains("Results: 0"));
        assert!(text.contains("(no results)"));
    }

    #[test]
    fn format_single_markdown_includes_h1_and_links() {
        let output = test_output();
        let md = format_single_markdown(&output);
        assert!(md.starts_with("# Results: teste\n\n"));
        assert!(md.contains("**Engine:** duckduckgo"));
        assert!(md.contains("**Total:** 1"));
        // Title with brackets must be escaped.
        // `url` crate normalizes empty path to trailing `/`.
        assert!(md.contains("[Title with \\[brackets\\]](https://example.com/)"));
        // Snippet with asterisks and backticks must be escaped.
        assert!(md.contains("Description with \\*asterisks\\* and \\`backticks\\`"));
        // display_url must appear between backticks.
        assert!(md.contains("`exemplo.com`"));
    }

    #[test]
    fn format_single_markdown_no_results_emits_warning() {
        let mut output = test_output();
        output.result_count = 0;
        output.results = vec![];
        let md = format_single_markdown(&output);
        assert!(md.contains("# Results: teste"));
        assert!(md.contains("_No results found._"));
    }

    #[test]
    fn format_result_with_original_title_shows_annotation_text() {
        // "Official site" heuristic: titulo was replaced by url_exibicao,
        // titulo_original preserves the literal text. Both must appear in text.
        let mut output = test_output();
        output.results = vec![SearchResult {
            position: 1,
            title: "saofidelis.rj.gov.br".to_string(),
            url: crate::types::HttpUrl::for_test("https://saofidelis.rj.gov.br"),
            display_url: Some("saofidelis.rj.gov.br".to_string()),
            snippet: Some("Prefeitura de São Fidélis".to_string()),
            original_title: Some("Official site".to_string()),
            content: None,
            content_size: None,
            content_extraction_method: None,
        }];
        let text = format_single_text(&output);
        assert!(text.contains("[1] saofidelis.rj.gov.br"));
        assert!(
            text.contains("(original: Official site)"),
            "text must show titulo_original when present"
        );
    }

    #[test]
    fn format_result_with_original_title_shows_annotation_markdown() {
        let mut output = test_output();
        output.results = vec![SearchResult {
            position: 1,
            title: "saofidelis.rj.gov.br".to_string(),
            url: crate::types::HttpUrl::for_test("https://saofidelis.rj.gov.br"),
            display_url: Some("saofidelis.rj.gov.br".to_string()),
            snippet: Some("Prefeitura".to_string()),
            original_title: Some("Official site".to_string()),
            content: None,
            content_size: None,
            content_extraction_method: None,
        }];
        let md = format_single_markdown(&output);
        assert!(md.contains("[saofidelis.rj.gov.br](https://saofidelis.rj.gov.br/)"));
        assert!(
            md.contains("_Original title: Official site_"),
            "markdown must show titulo_original in italics when present"
        );
    }

    #[test]
    fn format_result_without_original_title_no_annotation() {
        // titulo_original = None → no noise in output.
        let output = test_output();
        let text = format_single_text(&output);
        let md = format_single_markdown(&output);
        assert!(!text.contains("(original:"));
        assert!(!md.contains("_Original title:"));
    }

    #[test]
    fn json_omits_original_title_when_absent() {
        // skip_serializing_if = "Option::is_none" ensures the field does not
        // appear in JSON when None — preserves minimal compatibility.
        let output = test_output();
        let json = serde_json::to_string(&output).expect("serialize");
        assert!(
            !json.contains("titulo_original"),
            "JSON must not expose titulo_original when it is None"
        );
    }

    #[test]
    fn json_includes_original_title_when_present() {
        let mut output = test_output();
        output.results[0].original_title = Some("Official site".to_string());
        let json = serde_json::to_string(&output).expect("serialize");
        assert!(json.contains("\"titulo_original\":\"Official site\""));
    }

    #[test]
    fn json_no_longer_contains_related_searches_field() {
        // Regression v0.3.0: schema dropped `buscas_relacionadas` (BREAKING).
        let output = test_output();
        let json = serde_json::to_string(&output).expect("serialize");
        assert!(
            !json.contains("buscas_relacionadas"),
            "v0.3.0 removeu buscas_relacionadas do schema JSON"
        );
    }

    #[test]
    fn format_multi_text_includes_separators_per_query() {
        let output = MultiSearchOutput {
            query_count: 2,
            timestamp: crate::types::test_timestamp_offset(),
            parallelism: 3,
            searches: vec![test_output(), test_output()],
            causa_zero_histogram: BTreeMap::new(),
        };
        let text = format_multi_text(&output);
        assert!(text.contains("Queries: 2"));
        assert!(text.contains("Parallel: 3"));
        assert!(text.contains("========== Query #1 =========="));
        assert!(text.contains("========== Query #2 =========="));
    }

    #[test]
    fn format_multi_markdown_includes_overall_h1() {
        let output = MultiSearchOutput {
            query_count: 2,
            timestamp: crate::types::test_timestamp_offset(),
            parallelism: 3,
            searches: vec![test_output(), test_output()],
            causa_zero_histogram: BTreeMap::new(),
        };
        let md = format_multi_markdown(&output);
        assert!(md.starts_with("# Multiple Searches (2 queries)"));
        assert!(md.contains("**Parallelism:** 3"));
        // Each inner search must appear with its own H1.
        assert_eq!(md.matches("# Results: teste").count(), 2);
    }

    #[test]
    fn escape_markdown_protects_problematic_characters() {
        assert_eq!(escape_markdown("a*b"), "a\\*b");
        assert_eq!(escape_markdown("a[b]"), "a\\[b\\]");
        assert_eq!(escape_markdown("a`b"), "a\\`b");
        assert_eq!(escape_markdown("texto normal"), "texto normal");
        assert_eq!(escape_markdown("a\nb"), "ab");
    }

    #[test]
    fn escape_markdown_url_neutralizes_paren_and_space() {
        assert_eq!(
            escape_markdown_url("https://ex.com/x)y"),
            "https://ex.com/x%29y"
        );
        assert_eq!(
            escape_markdown_url("https://ex.com/a b"),
            "https://ex.com/a%20b"
        );
    }

    #[test]
    fn sanitize_untrusted_display_strips_ansi_and_controls() {
        assert_eq!(
            sanitize_untrusted_display("hi\x1b[31mred\x1b[0m"),
            "hired"
        );
        assert_eq!(sanitize_untrusted_display("a\0b\nc"), "abc");
        assert_eq!(
            sanitize_untrusted_display("safe\u{202E}evil"),
            "safeevil"
        );
    }

    #[test]
    fn text_format_strips_ansi_from_title() {
        let r = SearchResult {
            position: 1,
            title: "hi\x1b[31mRED".into(),
            url: crate::types::HttpUrl::for_test("https://ex.com"),
            display_url: None,
            snippet: Some("sn\x1b[31mip".into()),
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        };
        let block = format_result_text(&r);
        assert!(
            !block.contains('\x1b'),
            "ANSI must not reach text format: {block}"
        );
        assert!(block.contains("hiRED"));
        assert!(block.contains("snip"));
    }

    #[test]
    fn write_to_file_creates_parent_dirs() {
        let temp = std::env::temp_dir().join(format!("ddgcli-output-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let file = temp.join("sub").join("nested").join("saida.txt");
        write_to_file(&file, "conteudo de teste\nlinha 2\n")
            .expect("should write file with parent directories");
        let lido = fs::read_to_string(&file).expect("file should exist");
        assert_eq!(lido, "conteudo de teste\nlinha 2\n");
        fs::remove_dir_all(&temp).ok();
    }

    #[cfg(unix)]
    #[test]
    fn write_to_file_applies_644_permissions_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let file =
            std::env::temp_dir().join(format!("ddgcli-perms-test-{}.txt", std::process::id()));
        let _ = fs::remove_file(&file);
        write_to_file(&file, "x").expect("should write");
        let metadata = fs::metadata(&file).expect("should get metadata");
        let modo = metadata.permissions().mode() & 0o777;
        assert_eq!(modo, 0o644, "permissions must be 0o644 (was {modo:o})");
        fs::remove_file(&file).ok();
    }

    #[test]
    fn emitir_json_single_via_serde_continua_estavel() {
        // Regression guarantee: JSON serialization of the struct does not change.
        let output = test_output();
        let json = serde_json::to_string_pretty(&output).expect("serialization should work");
        assert!(json.contains("\"query\": \"teste\""));
        assert!(json.contains("\"quantidade_resultados\": 1"));
        assert!(json.contains("\"motor\": \"duckduckgo\""));
    }

    // -----------------------------------------------------------------------
    // Cobertura dos caminhos de streaming/arquivo
    // -----------------------------------------------------------------------

    #[test]
    fn emit_ndjson_to_file_writes_single_parseable_line() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("ndjson.log");
        let output = test_output();
        emit_ndjson(&output, Some(&file)).expect("ndjson should write");
        let content = fs::read_to_string(&file).expect("read file");
        // Trailing LF required (or empty file); never leave last record without terminator.
        assert!(
            content.ends_with('\n'),
            "NDJSON file must end with LF after the last record"
        );
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 1, "NDJSON = 1 line per call");
        // Compact JSON only — no pretty-print (pretty would inject LF between fields).
        assert!(
            !lines[0].contains('\n'),
            "NDJSON record must be a single line (compact JSON)"
        );
        let pretty = serde_json::to_string_pretty(&output).expect("pretty for contrast");
        assert_ne!(
            lines[0],
            pretty.trim(),
            "NDJSON must use compact form, not pretty-print"
        );
        let parsed: SearchOutput =
            serde_json::from_str(lines[0]).expect("NDJSON line should round-trip to SearchOutput");
        assert_eq!(parsed.query, output.query);
        assert_eq!(parsed.result_count, output.result_count);
    }

    #[test]
    fn map_serde_write_preserves_broken_pipe() {
        // Synthetic: build an Error classified as Io BrokenPipe via a closed writer.
        use std::io::Write;
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let err = serde_json::to_writer(Broken, &serde_json::json!({"a": 1}))
            .expect_err("write must fail");
        let mapped = map_serde_write(&err, "test");
        assert!(
            matches!(mapped, CliError::BrokenPipe),
            "serde_json Io BrokenPipe must map to CliError::BrokenPipe, got {mapped:?}"
        );
    }

    #[test]
    fn emit_ndjson_two_calls_append_without_truncating() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("ndjson.log");
        let output = test_output();
        emit_ndjson(&output, Some(&file)).expect("1st write");
        emit_ndjson(&output, Some(&file)).expect("2nd write (append)");
        let content = fs::read_to_string(&file).expect("read");
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 2, "modo append: 2 chamadas = 2 linhas");
    }

    #[test]
    fn emit_ndjson_creates_parent_dirs_when_missing() {
        let dir = tempfile::tempdir().expect("create tempdir");
        // Path with 2 non-existent levels.
        let file = dir.path().join("sub/outro/out.ndjson");
        assert!(!file.parent().unwrap().exists());
        emit_ndjson(&test_output(), Some(&file)).expect("should create parents");
        assert!(file.exists(), "arquivo criado");
        assert!(file.parent().unwrap().exists(), "parent directory created");
    }

    #[test]
    fn emit_stream_text_to_file_includes_query_header() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("stream.txt");
        emit_stream_text(0, &test_output(), Some(&file)).expect("stream text");
        emit_stream_text(1, &test_output(), Some(&file)).expect("stream text 2");
        let content = fs::read_to_string(&file).expect("read");
        assert!(content.contains("========== Query #1 =========="));
        assert!(content.contains("========== Query #2 =========="));
        assert!(content.contains("Query: teste"));
    }

    #[test]
    fn emit_stream_markdown_separates_queries_with_divider_from_second() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("stream.md");
        emit_stream_markdown(0, &test_output(), Some(&file)).expect("1st");
        emit_stream_markdown(1, &test_output(), Some(&file)).expect("2nd");
        let content = fs::read_to_string(&file).expect("read");
        // Separator "\n---\n" must appear ONLY between blocks (once for 2 queries).
        let ocorrencias = content.matches("\n---\n").count();
        assert_eq!(
            ocorrencias, 1,
            "divisor apenas entre queries (1 para 2 blocos)"
        );
        assert!(content.contains("# Results: teste"));
    }

    #[test]
    fn emit_result_stream_is_noop_and_does_not_create_file() {
        use crate::parallel::StreamStats;
        use crate::pipeline::PipelineResult;
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("nao-cria.json");
        let stream_stats = StreamStats {
            total: 3,
            successes: 3,
            errors: 0,
            start_timestamp: crate::types::test_timestamp(),
            parallelism: 2,
        };
        let res = PipelineResult::Stream(stream_stats);
        emit_result(&res, OutputFormat::Json, Some(&file)).expect("no-op OK");
        assert!(
            !file.exists(),
            "Stream must not escrever nada em emit_result"
        );
    }

    #[test]
    fn emit_result_single_to_file_writes_formatted_json() {
        use crate::pipeline::PipelineResult;
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("saida.json");
        let res = PipelineResult::Single(Box::new(test_output()));
        emit_result(&res, OutputFormat::Json, Some(&file)).expect("emit");
        let content = fs::read_to_string(&file).expect("read");
        let _: serde_json::Value =
            serde_json::from_str(&content).expect("content should be valid JSON");
        assert!(content.contains("\"query\": \"teste\""));
    }

    /// GAP-PAR-040a: async emit formats via `run_cpu_bound` and writes the same JSON.
    #[tokio::test]
    async fn emit_result_async_single_to_file_writes_formatted_json() {
        use crate::pipeline::PipelineResult;
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("saida-async.json");
        let res = PipelineResult::Single(Box::new(test_output()));
        emit_result_async(&res, OutputFormat::Json, Some(&file))
            .await
            .expect("emit async");
        let content = fs::read_to_string(&file).expect("read");
        let _: serde_json::Value =
            serde_json::from_str(&content).expect("content should be valid JSON");
        assert!(content.contains("\"query\": \"teste\""));
    }

    /// GAP-PAR-040c: serialize_json_async produces parseable compact JSON.
    #[tokio::test]
    async fn serialize_json_async_round_trips_search_output() {
        let output = test_output();
        let json = serialize_json_async(output.clone())
            .await
            .expect("serialize");
        let parsed: SearchOutput = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed.query, output.query);
        assert_eq!(parsed.result_count, output.result_count);
    }

    /// GAP-PAR-040b: NDJSON async writes one compact line.
    #[tokio::test]
    async fn emit_ndjson_async_writes_one_compact_line() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("stream.ndjson");
        emit_ndjson_async(test_output(), Some(file.clone()))
            .await
            .expect("ndjson async");
        let content = fs::read_to_string(&file).expect("read");
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].contains('\n'));
        let _: SearchOutput = serde_json::from_str(lines[0]).expect("round-trip");
    }

    #[test]
    fn emit_result_multi_text_to_file_contains_both_queries() {
        use crate::pipeline::PipelineResult;
        use crate::types::MultiSearchOutput;
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("multi.txt");
        let mut output1 = test_output();
        output1.query = "alpha".into();
        let mut output2 = test_output();
        output2.query = "beta".into();
        let multi = MultiSearchOutput {
            query_count: 2,
            timestamp: crate::types::test_timestamp(),
            parallelism: 2,
            searches: vec![output1, output2],
            causa_zero_histogram: BTreeMap::new(),
        };
        let res = PipelineResult::Multi(Box::new(multi));
        emit_result(&res, OutputFormat::Text, Some(&file)).expect("emit");
        let content = fs::read_to_string(&file).expect("read");
        assert!(content.contains("Query: alpha"));
        assert!(content.contains("Query: beta"));
    }

    #[test]
    fn emit_result_auto_to_file_writes_json() {
        // Auto + file → JSON (deterministic, does not depend on TTY).
        use crate::pipeline::PipelineResult;
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("auto.out");
        let res = PipelineResult::Single(Box::new(test_output()));
        emit_result(&res, OutputFormat::Auto, Some(&file)).expect("emit");
        let content = fs::read_to_string(&file).expect("read");
        // JSON starts with `{` and has "query".
        assert!(content.trim_start().starts_with('{'));
        assert!(content.contains("\"query\""));
    }

    #[test]
    fn is_broken_pipe_detects_broken_pipe() {
        assert!(is_broken_pipe(&CliError::BrokenPipe));
    }

    #[test]
    fn is_broken_pipe_rejects_other_errors() {
        let err = CliError::PathError {
            message: "not found".into(),
        };
        assert!(!is_broken_pipe(&err));
    }

    #[test]
    fn is_broken_pipe_rejects_network_error() {
        let err = CliError::NetworkError {
            message: "timeout".into(),
        };
        assert!(!is_broken_pipe(&err));
    }

    /// GAP-MACRO-002: `emit_stderr` accepts `format_args!` / any `Display`
    /// without forcing an intermediate `String` (no format!+print double alloc).
    #[test]
    fn emit_stderr_accepts_format_args_and_plain_str() {
        emit_stderr("macro-audit plain");
        emit_stderr(format_args!("macro-audit fmt code={}", 42));
        let displayable = 7u8;
        emit_stderr(displayable);
    }

    /// GAP-E2E-48-006 / CM-02: unified emit route for deep-research `-o`.
    #[test]
    fn emit_payload_writes_file_atomically() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("dr.json");
        let body = r#"{"erro":"timeout","comando":"deep-research"}"#;
        emit_payload(body, Some(&file)).expect("emit_payload");
        let content = fs::read_to_string(&file).expect("read");
        assert_eq!(content.trim(), body);
        assert!(file.exists());
    }
}
