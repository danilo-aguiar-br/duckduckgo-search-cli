// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light format/serde (no stdout I/O).
//! Formatters for search output: JSON, text, Markdown, TSV + display sanitization.

use crate::error::CliError;
use crate::types::{MultiSearchOutput, OutputFormat, SearchOutput, SearchResult};
use std::fmt::Write as FmtWrite;
use std::path::Path;

/// Resolves `OutputFormat::Auto` to the concrete format based on TTY detection.
///
/// - Outputting to file (`output_path = Some`) → JSON (stable and parseable).
/// - Auto + stdout TTY → Text (ergonomic for humans).
/// - Auto + stdout pipe → JSON (programmatic consumption).
pub(super) fn resolve_auto_format(
    format: OutputFormat,
    output_path: Option<&Path>,
) -> OutputFormat {
    match format {
        OutputFormat::Auto => {
            if output_path.is_some() {
                OutputFormat::Json
            } else if crate::platform::stdout_is_tty() {
                OutputFormat::Text
            } else {
                OutputFormat::Json
            }
        }
        other => other,
    }
}

pub(super) fn format_single(
    output: &SearchOutput,
    format: OutputFormat,
) -> Result<String, CliError> {
    match format {
        OutputFormat::Json | OutputFormat::Auto => {
            serde_json::to_string_pretty(output).map_err(|e| CliError::InvalidConfig {
                message: format!("failed to serialize search output as JSON: {e}"),
            })
        }
        OutputFormat::Text => Ok(format_single_text(output)),
        OutputFormat::Markdown => Ok(format_single_markdown(output)),
        OutputFormat::Tsv => Ok(format_single_tsv(output)),
    }
}

pub(super) fn format_multi(
    output: &MultiSearchOutput,
    format: OutputFormat,
) -> Result<String, CliError> {
    match format {
        OutputFormat::Json | OutputFormat::Auto => {
            serde_json::to_string_pretty(output).map_err(|e| CliError::InvalidConfig {
                message: format!("failed to serialize multi-search output as JSON: {e}"),
            })
        }
        OutputFormat::Text => Ok(format_multi_text(output)),
        OutputFormat::Markdown => Ok(format_multi_markdown(output)),
        OutputFormat::Tsv => Ok(format_multi_tsv(output)),
    }
}

/// TSV escape: replace tabs/newlines; wrap only when needed is unnecessary for agents
/// that treat every field as raw after unescape of `\t`/`\n`.
fn tsv_field(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Single-query TSV: header + one row per result.
/// Columns: `rank`, `title`, `url`, `snippet`, `query`.
fn format_single_tsv(output: &SearchOutput) -> String {
    let mut buffer = String::with_capacity(64 + output.results.len().saturating_mul(200));
    buffer.push_str("rank\ttitle\turl\tsnippet\tquery\n");
    let q = tsv_field(&output.query);
    for r in &output.results {
        let _ = writeln!(
            buffer,
            "{}\t{}\t{}\t{}\t{}",
            r.position,
            tsv_field(&r.title),
            tsv_field(r.url.as_str()),
            tsv_field(r.snippet.as_deref().unwrap_or("")),
            q
        );
    }
    buffer
}

fn format_multi_tsv(output: &MultiSearchOutput) -> String {
    let mut buffer = String::with_capacity(64 + output.searches.len().saturating_mul(400));
    buffer.push_str("query_index\trank\ttitle\turl\tsnippet\tquery\n");
    for (qi, search) in output.searches.iter().enumerate() {
        let q = tsv_field(&search.query);
        for r in &search.results {
            let _ = writeln!(
                buffer,
                "{}\t{}\t{}\t{}\t{}\t{}",
                qi + 1,
                r.position,
                tsv_field(&r.title),
                tsv_field(r.url.as_str()),
                tsv_field(r.snippet.as_deref().unwrap_or("")),
                q
            );
        }
    }
    buffer
}

/// `text` format for single-query — compact, optimized for LLM tokens.
///
/// ```text
/// Query: <query> | Engine: duckduckgo | Endpoint: html | Results: N
///
/// [1] <title>
///     <url>
///     <snippet>
///
/// [2] ...
/// ```
pub(super) fn format_single_text(output: &SearchOutput) -> String {
    let cap = 100usize.saturating_add(output.results.len().saturating_mul(200));
    let mut buffer = String::with_capacity(cap);
    buffer.push_str(&format_header_text(output));
    if output.results.is_empty() {
        buffer.push_str(crate::i18n::t(crate::i18n::Message::NoResultsPlaceholder));
        return buffer;
    }
    for result_item in &output.results {
        buffer.push('\n');
        buffer.push_str(&format_result_text(result_item));
    }
    buffer
}

pub(super) fn format_multi_text(output: &MultiSearchOutput) -> String {
    let cap = 100usize.saturating_add(output.searches.len().saturating_mul(800));
    let mut buffer = String::with_capacity(cap);
    let _ = writeln!(
        buffer,
        "Queries: {} | Parallel: {} | Timestamp: {}",
        output.query_count, output.parallelism, output.timestamp
    );
    for (i, search) in output.searches.iter().enumerate() {
        let _ = write!(buffer, "\n========== Query #{} ==========\n", i + 1);
        buffer.push_str(&format_single_text(search));
    }
    buffer
}

fn format_header_text(output: &SearchOutput) -> String {
    format!(
        "Query: {} | Engine: {} | Endpoint: {} | Results: {}\n",
        sanitize_untrusted_display(&output.query),
        output.engine,
        output.endpoint,
        output.result_count
    )
}

pub(super) fn format_result_text(r: &SearchResult) -> String {
    let mut block = String::with_capacity(300);
    let _ = writeln!(
        block,
        "[{}] {}",
        r.position,
        sanitize_untrusted_display(&r.title)
    );
    if let Some(original) = &r.original_title {
        if !original.is_empty() {
            let _ = writeln!(
                block,
                "    (original: {})",
                sanitize_untrusted_display(original)
            );
        }
    }
    let _ = writeln!(
        block,
        "    {}",
        sanitize_untrusted_display(r.url.as_str())
    );
    if let Some(snippet) = &r.snippet {
        if !snippet.is_empty() {
            let _ = writeln!(block, "    {}", sanitize_untrusted_display(snippet));
        }
    }
    block
}

/// `markdown` format for single-query — ideal for `.md` files and GitHub.
///
/// ```markdown
/// # Results: <query>
///
/// **Engine:** duckduckgo | **Endpoint:** html | **Total:** N
///
/// ## 1. [<title>](<url>)
///
/// <snippet>
///
/// ---
///
/// ## 2. ...
/// ```
///
/// Human labels follow the resolved UI locale (`en` / `pt-BR`).
pub(super) fn format_single_markdown(output: &SearchOutput) -> String {
    let cap = 200usize.saturating_add(output.results.len().saturating_mul(300));
    let mut buffer = String::with_capacity(cap);
    let total = output.result_count.to_string();
    buffer.push_str(&crate::i18n::tf(
        crate::i18n::Message::MarkdownResultsHeading,
        &[("query", &output.query)],
    ));
    buffer.push_str(&crate::i18n::tf(
        crate::i18n::Message::MarkdownMetaLine,
        &[
            ("engine", &output.engine),
            ("endpoint", &output.endpoint),
            ("total", &total),
        ],
    ));
    if output.results.is_empty() {
        buffer.push_str("_No results found._\n");
        return buffer;
    }
    for (i, r) in output.results.iter().enumerate() {
        if i > 0 {
            buffer.push_str("---\n\n");
        }
        let _ = write!(
            buffer,
            "## {}. [{}]({})\n\n",
            r.position,
            escape_markdown(&r.title),
            escape_markdown_url(r.url.as_str())
        );
        if let Some(original) = &r.original_title {
            if !original.is_empty() {
                let _ = write!(
                    buffer,
                    "_Original title: {}_\n\n",
                    escape_markdown(original)
                );
            }
        }
        if let Some(snippet) = &r.snippet {
            if !snippet.is_empty() {
                let _ = write!(buffer, "{}\n\n", escape_markdown(snippet));
            }
        }
        if let Some(url_exibicao) = &r.display_url {
            if !url_exibicao.is_empty() {
                let _ = write!(buffer, "`{url_exibicao}`\n\n");
            }
        }
    }
    buffer
}

pub(super) fn format_multi_markdown(output: &MultiSearchOutput) -> String {
    let cap = 200usize.saturating_add(output.searches.len().saturating_mul(1200));
    let mut buffer = String::with_capacity(cap);
    let _ = write!(
        buffer,
        "# Multiple Searches ({} queries)\n\n",
        output.query_count
    );
    let _ = write!(
        buffer,
        "**Parallelism:** {} | **Timestamp:** {}\n\n",
        output.parallelism, output.timestamp
    );
    for (i, search) in output.searches.iter().enumerate() {
        if i > 0 {
            buffer.push_str("\n---\n\n");
        }
        buffer.push_str(&format_single_markdown(search));
    }
    buffer
}

/// Strips hostile control / ANSI / bidi sequences from untrusted display text
/// (SERP titles, snippets, URLs written to a TTY).
///
/// Network HTML is attacker-controlled; without this, CSI sequences in titles
/// can spoof terminal chrome or inject log lines when redirected.
pub(super) fn sanitize_untrusted_display(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI: ESC [ ... final byte in @..~
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&n) {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                // OSC: ESC ] ... BEL or ST (ESC \)
                chars.next();
                while let Some(n) = chars.next() {
                    if n == '\u{07}' {
                        break;
                    }
                    if n == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            } else {
                // Other ESC sequences: drop the next char if present.
                let _ = chars.next();
            }
            continue;
        }
        if c == '\t' {
            out.push(c);
            continue;
        }
        if c.is_control() {
            continue;
        }
        // Bidi / zero-width spoofing in display channels.
        if matches!(
            c,
            '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        ) {
            continue;
        }
        out.push(c);
    }
    out
}

/// Escapes Markdown characters that could break rendering in titles
/// or snippets. Conservative: escapes `\`, `*`, `[`, `]`, backticks, and
/// strips ASCII control / ANSI / bidi (via [`sanitize_untrusted_display`]).
pub(super) fn escape_markdown(text: &str) -> String {
    let cleaned = sanitize_untrusted_display(text);
    let mut out = String::with_capacity(cleaned.len() + cleaned.len() / 8);
    for ch in cleaned.chars() {
        match ch {
            '\\' | '*' | '[' | ']' | '`' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Escapes a URL for use inside a Markdown link destination `](url)`.
///
/// Prevents `)` / whitespace / control characters from terminating the link
/// early (content injection from adversarial SERP titles/URLs).
pub(super) fn escape_markdown_url(url: &str) -> String {
    let cleaned = sanitize_untrusted_display(url);
    let mut out = String::with_capacity(cleaned.len() + 8);
    for ch in cleaned.chars() {
        match ch {
            ')' => out.push_str("%29"),
            '(' => out.push_str("%28"),
            ' ' => out.push_str("%20"),
            '\t' => out.push_str("%09"),
            _ => out.push(ch),
        }
    }
    out
}
