// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (HTML parse + text extract via scraper; not Send)
//! Simplified readability over UTF-8 HTML (scraper / html5ever).

use crate::validation::limits::MIN_LINE_LENGTH;
use scraper::{Html, Selector};
use std::sync::LazyLock;

/// Threshold below which we consider content "insufficient" (Chrome fallback candidate).
pub(crate) const MIN_CONTENT_THRESHOLD: usize = 200;

/// Applies simplified readability in 5 steps over UTF-8 HTML.
///
/// Returns clean text truncated at `max_size` characters (respecting word boundaries).
/// Called from within `spawn_blocking` because `scraper::Html` is not `Send`.
pub(crate) fn apply_readability(html: &str, max_size: usize) -> String {
    let document = Html::parse_document(html);

    // Step 2: identify main container.
    let mut container_ref = None;
    for sel in cached_sel_containers() {
        if let Some(first_match) = document.select(sel).next() {
            container_ref = Some(first_match);
            break;
        }
    }

    // Fallback: full body.
    let container = match container_ref {
        Some(c) => c,
        None => match document.select(cached_sel_body()).next() {
            Some(b) => b,
            None => return String::new(),
        },
    };

    // Step 3: extract text from relevant blocks within the container.
    let blocks = cached_sel_blocks();

    let excluded_tags: &[&str] = &[
        "nav", "header", "footer", "aside", "script", "style", "noscript", "iframe", "svg", "form",
    ];
    let excluded_classes: &[&str] = &[
        "sidebar",
        "nav",
        "menu",
        "footer",
        "header",
        "ad",
        "advertisement",
        "social-share",
    ];
    let excluded_roles: &[&str] = &["navigation", "banner", "contentinfo"];

    let mut lines_vec: Vec<String> = Vec::with_capacity(64);
    for block in container.select(blocks) {
        if has_excluded_ancestor(block, excluded_tags, excluded_classes, excluded_roles) {
            continue;
        }
        let mut text = String::with_capacity(256);
        let mut needs_space = false;
        for fragment in block.text() {
            for word in fragment.split_whitespace() {
                if needs_space {
                    text.push(' ');
                }
                text.push_str(word);
                needs_space = true;
            }
        }
        if !text.is_empty() {
            lines_vec.push(text);
        }
    }

    // Step 4: cleanup — short lines discarded, normalize whitespace between lines.
    let mut content = String::with_capacity(lines_vec.len() * 100);
    let mut is_first = true;
    for l in lines_vec {
        if l.chars().count() >= MIN_LINE_LENGTH {
            if !is_first {
                content.push('\n');
            }
            content.push_str(&l);
            is_first = false;
        }
    }

    // Step 5: truncate at max_size characters respecting word boundaries.
    truncate_at_word(&content, max_size)
}

/// Checks whether an element (or any ancestor) belongs to the "chrome" categories.
fn has_excluded_ancestor(
    element: scraper::ElementRef<'_>,
    tags: &[&str],
    classes: &[&str],
    roles: &[&str],
) -> bool {
    let mut current_node = element.parent();
    while let Some(node) = current_node {
        if let Some(el) = scraper::ElementRef::wrap(node) {
            let name = el.value().name();
            if tags.iter().any(|t| t.eq_ignore_ascii_case(name)) {
                return true;
            }
            if let Some(class_attr) = el.value().attr("class") {
                for c in class_attr.split_ascii_whitespace() {
                    if classes
                        .iter()
                        .any(|excluded| c.eq_ignore_ascii_case(excluded))
                    {
                        return true;
                    }
                }
            }
            if let Some(role) = el.value().attr("role") {
                if roles.iter().any(|r| r.eq_ignore_ascii_case(role)) {
                    return true;
                }
            }
        }
        current_node = node.parent();
    }
    false
}

/// Truncates `text` at `max_size` characters respecting word boundaries.
pub(crate) fn truncate_at_word(text: &str, max_size: usize) -> String {
    if max_size == 0 {
        return String::new();
    }
    let byte_pos = text.char_indices().nth(max_size).map(|(i, _)| i);
    let Some(cut) = byte_pos else {
        return text.to_string();
    };
    let prefix = &text[..cut];
    if let Some(pos) = prefix.rfind(char::is_whitespace) {
        return prefix[..pos].trim_end().to_string();
    }
    prefix.to_string()
}

fn cached_sel_containers() -> &'static [Selector] {
    static C: LazyLock<Vec<Selector>> = LazyLock::new(|| {
        [
            "article",
            "main",
            "[role=\"main\"]",
            ".post-content",
            ".article-body",
            ".entry-content",
            "#content",
            ".content",
        ]
        .iter()
        .filter_map(|s| Selector::parse(s).ok())
        .collect()
    });
    &C
}

fn cached_sel_body() -> &'static Selector {
    static C: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("body").expect("static CSS selector 'body' is valid"));
    &C
}

fn cached_sel_blocks() -> &'static Selector {
    static C: LazyLock<Selector> = LazyLock::new(|| {
        Selector::parse("p, h1, h2, h3, h4, h5, h6, li, blockquote, pre, td, th")
            .expect("static CSS selector for content blocks is valid")
    });
    &C
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_at_word_preserves_boundary() {
        let text = "uma frase qualquer with várias palavras";
        let t = truncate_at_word(text, 10);
        assert!(t.len() <= 10);
        assert!(!t.ends_with(' '));
        assert!(
            text.starts_with(&t),
            "truncated ({t:?}) must be a prefix of the original"
        );
    }

    #[test]
    fn truncate_at_word_short_text_returns_original() {
        assert_eq!(truncate_at_word("oi", 100), "oi");
        assert_eq!(truncate_at_word("", 100), "");
    }

    #[test]
    fn truncate_at_word_no_whitespace_cuts_hard() {
        let t = truncate_at_word("palavraSemEspacoNenhum", 10);
        assert_eq!(t.chars().count(), 10);
    }

    #[test]
    fn readability_extracts_simple_article() {
        let html = r#"<html><body>
            <nav><a href="/">Menu</a></nav>
            <article>
              <h1>Article Title</h1>
              <p>This is the first paragraph of the article with at least twenty characters of substance.</p>
              <p>Second paragraph also with enough content to pass the minimum line length threshold.</p>
            </article>
            <footer>Copyright</footer>
            </body></html>"#;
        let text = apply_readability(html, 1000);
        assert!(text.contains("first paragraph"));
        assert!(text.contains("Second paragraph"));
        assert!(!text.contains("Menu"));
        assert!(!text.contains("Copyright"));
    }

    #[test]
    fn readability_uses_main_when_no_article() {
        let html = r#"<html><body>
            <header>Irrelevant header text here for noise</header>
            <main>
              <p>Main content via main tag, with more than twenty characters of useful text here.</p>
              <p>Another relevant paragraph with enough content so it is not discarded by the filter.</p>
            </main>
            </body></html>"#;
        let text = apply_readability(html, 1000);
        assert!(text.contains("Main content"));
        assert!(text.contains("Another relevant"));
        assert!(!text.contains("Irrelevant header"));
    }

    #[test]
    fn readability_removes_script_style_nav() {
        let html = r#"<html><body>
            <nav><p>This paragraph inside nav must be discarded because it is chrome navigation.</p></nav>
            <article>
              <script>var x = 1;</script>
              <style>.a { color: red; }</style>
              <p>Legitimate paragraph inside article with enough content to pass the line threshold.</p>
            </article>
            </body></html>"#;
        let text = apply_readability(html, 1000);
        assert!(text.contains("Legitimate paragraph"));
        assert!(!text.contains("inside nav"));
        assert!(!text.contains("var x = 1"));
        assert!(!text.contains("color: red"));
    }

    #[test]
    fn readability_truncates_at_max_size() {
        let long_content = "Paragraph one with at least twenty characters here.\n".repeat(100);
        let html = format!("<html><body><article><p>{long_content}</p></article></body></html>");
        let text = apply_readability(&html, 200);
        assert!(text.chars().count() <= 200);
    }

    #[test]
    fn readability_returns_empty_without_enough_content() {
        let html = r#"<html><body>
            <nav>Short menu</nav>
            <footer>Short footer.</footer>
            </body></html>"#;
        let text = apply_readability(html, 1000);
        assert!(
            text.len() < MIN_CONTENT_THRESHOLD,
            "without substantive content expected, got: {text:?}"
        );
    }
}
