// SPDX-License-Identifier: MIT OR Apache-2.0
// GAP-COMP-004: module directory for web/news/url SRP (facade in mod.rs).
// Workload: CPU-bound (HTML parsing and text extraction via scraper).
// Parallelism:
// - Per document: sequential (one Html tree; strategies 1→2 are cascade, not fan-out).
// - Across queries: I/O fan-out in parallel.rs / dual Chrome JoinSet.
// - Async call sites MUST use `*_async` helpers (GAP-PAR-030) → `run_cpu_bound`
//   (spawn_blocking + blocking_cpu_semaphore). Sync APIs remain for tests/benches.
// - scraper/html5ever uses Rc (not Send); never hold Html across .await.
// - Rayon not used: N docs small vs RTT; blocking pool already sized for CPU.
//! Extraction of search results from `DuckDuckGo` HTML.
//!
//! In the MVP implements ONLY Strategy 1 (stable class selectors):
//! - Container: `#links`.
//! - Items: `.result` (multiple alternative selectors).
//! - Title + URL: `.result__a`.
//! - Snippet: `.result__snippet`.
//! - Display URL: `.result__url`.
//!
//! Ad filtering:
//! - Removes elements with class `.result--ad` or `.badge--ad`.
//! - Removes elements with attribute `data-nrn="ad"`.
//! - Removes results whose URL contains `duckduckgo.com/y.js`.
//!
//! URL resolution:
//! - Protocol-relative URLs (`//example.com`) are prefixed with `https:`.
//! - URLs containing a `DuckDuckGo` internal redirect (`/l/?uddg=...&rut=...`) are
//!   unwrapped via URL-decoding of the `uddg` parameter.
//! - URLs on the `duckduckgo.com` domain itself are filtered out.

use crate::error::CliError;
use crate::types::{NewsResult, SearchResult, SelectorConfig};

/// Async SERP web extract off the Tokio worker (GAP-PAR-030 / 038).
///
/// Owns `raw_html` and a clone of `cfg` so the blocking task is `'static`.
/// Prefer over calling [`extract_results_with_strategies_cfg`] from `async fn`.
///
/// # Errors
///
/// Returns [`CliError`] when the CPU-bound extractor or its worker join fails.
#[tracing::instrument(level = "debug", skip_all, fields(html_len = raw_html.len()))]
pub async fn extract_results_with_strategies_cfg_async(
    raw_html: String,
    cfg: SelectorConfig,
) -> Result<Vec<SearchResult>, CliError> {
    crate::concurrency::run_cpu_bound(move || extract_results_with_strategies_cfg(&raw_html, &cfg))
        .await
}

/// Async Lite SERP extract (GAP-PAR-030).
///
/// # Errors
///
/// Returns [`CliError`] when the CPU-bound extractor or its worker join fails.
#[tracing::instrument(level = "debug", skip_all, fields(html_len = raw_html.len()))]
pub async fn extract_results_lite_with_cfg_async(
    raw_html: String,
    cfg: SelectorConfig,
) -> Result<Vec<SearchResult>, CliError> {
    crate::concurrency::run_cpu_bound(move || extract_results_lite_with_cfg(&raw_html, &cfg)).await
}

/// Async news SERP extract with promo stats (GAP-PAR-030 / 038).
///
/// # Errors
///
/// Returns [`CliError`] when the CPU-bound extractor or its worker join fails.
#[tracing::instrument(level = "debug", skip_all, fields(html_len = raw_html.len()))]
pub async fn extract_news_results_with_stats_async(
    raw_html: String,
    cfg: SelectorConfig,
) -> Result<(Vec<NewsResult>, u32), CliError> {
    crate::concurrency::run_cpu_bound(move || extract_news_results_with_stats(&raw_html, &cfg))
        .await
}


pub mod news;
pub mod url;
pub mod web;

pub use news::{
    extract_news_results_with_cfg, extract_news_results_with_stats, filter_news_results,
    is_ddg_promo_url,
};
pub use url::resolve_url;
pub use web::{
    extract_results, extract_results_lite, extract_results_lite_with_cfg,
    extract_results_with_cfg, extract_results_with_strategies,
    extract_results_with_strategies_cfg, extract_results_with_strategies_on_document,
};

// Test/helper reexports (pub(crate)).
#[allow(unused_imports)]
pub(crate) use news::looks_like_relative_date;
#[allow(unused_imports)]
pub(crate) use web::normalize_text;

#[cfg(test)]
mod tests {
    use super::*;
    use super::news::news_meta_from_ancestors;
    use crate::types::NewsSelectors;
    use scraper::{Html, Selector};

    #[test]
    fn resolver_url_prefixa_protocol_relative() {
        assert_eq!(
            resolve_url("//exemplo.com/caminho").as_ref().map(|u| u.as_str()),
            Some("https://exemplo.com/caminho")
        );
    }

    #[test]
    fn resolver_url_desencapsula_redirect_uddg() {
        let href = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexemplo.com%2Fnoticia&rut=abc123";
        let resolvida = resolve_url(href).expect("should decode uddg");
        assert_eq!(resolvida, "https://exemplo.com/noticia");
    }

    #[test]
    fn resolve_url_unwraps_uddg_with_absolute_path() {
        let href = "/l/?uddg=https%3A%2F%2Fexemplo.com%2Farticle";
        let resolvida = resolve_url(href).expect("should decode uddg");
        assert_eq!(resolvida, "https://exemplo.com/article");
    }

    #[test]
    fn resolve_url_filters_duckduckgo_without_uddg() {
        assert_eq!(resolve_url("https://duckduckgo.com/settings"), None);
        assert_eq!(resolve_url("//html.duckduckgo.com/html/?q=teste"), None);
    }

    #[test]
    fn resolver_url_mantem_absolutas_externas() {
        assert_eq!(
            resolve_url("https://example.com.br/noticia")
                .as_ref()
                .map(|u| u.as_str()),
            Some("https://example.com.br/noticia")
        );
    }

    #[test]
    fn resolve_url_returns_none_for_empty_string() {
        assert_eq!(resolve_url(""), None);
        assert_eq!(resolve_url("   "), None);
    }

    #[test]
    fn normalize_text_colapsa_whitespace() {
        assert_eq!(
            normalize_text("  olá   mundo\n\n\ttexto  ", 100),
            "olá mundo texto"
        );
    }

    #[test]
    fn normalize_text_trunca_respeitando_char_boundary() {
        let long_text = "á".repeat(300);
        let truncated = normalize_text(&long_text, 200);
        assert_eq!(truncated.chars().count(), 200);
    }

    #[test]
    fn extract_results_works_with_minimal_html() {
        let html = r#"
            <html><body>
            <div id="links">
              <div class="result">
                <a class="result__a" href="//exemplo.com/pagina">Título Exemplo</a>
                <a class="result__snippet">Esta é uma descrição de exemplo.</a>
                <span class="result__url">exemplo.com</span>
              </div>
              <div class="result result--ad">
                <a class="result__a" href="//anuncio.com">Anúncio Pago</a>
              </div>
              <div class="result">
                <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwikipedia.org%2Fwiki%2FRust">Rust</a>
                <a class="result__snippet">Linguagem de programação Rust.</a>
              </div>
            </div>
            </body></html>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 2, "deve filtrar o anúncio");
        assert_eq!(results[0].position, 1);
        assert_eq!(results[0].title, "Título Exemplo");
        assert_eq!(results[0].url, "https://exemplo.com/pagina");
        assert_eq!(
            results[0].snippet.as_deref(),
            Some("Esta é uma descrição de exemplo.")
        );
        assert_eq!(results[1].position, 2);
        assert_eq!(results[1].title, "Rust");
        assert_eq!(results[1].url, "https://wikipedia.org/wiki/Rust");
    }

    #[test]
    fn extract_results_filters_js_urls() {
        let html = r#"
            <div id="links">
              <div class="result">
                <a class="result__a" href="//duckduckgo.com/y.js?ad=1">Tracker</a>
              </div>
              <div class="result">
                <a class="result__a" href="//site-valido.com/pagina">Válido</a>
              </div>
            </div>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Válido");
    }

    #[test]
    fn extract_results_respects_data_nrn_ad_attribute() {
        let html = r#"
            <div id="links">
              <div class="result" data-nrn="ad">
                <a class="result__a" href="//anuncio.com">Patrocinado</a>
              </div>
              <div class="result" data-nrn="organic">
                <a class="result__a" href="//organico.com">Orgânico</a>
              </div>
            </div>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 1);
        // `url` crate normalizes empty path to `/`.
        assert_eq!(results[0].url.as_str(), "https://organico.com/");
    }

    #[test]
    fn extract_results_empty_returns_empty_vec() {
        let html = "<html><body>Sem results</body></html>";
        let results = extract_results(html);
        assert!(results.is_empty());
    }

    #[test]
    fn strategy_2_recovers_when_classes_absent() {
        let html = r#"
            <html><body>
            <div id="links">
              <div>
                <a href="//exemplo.com/artigo">Título do Artigo de Exemplo</a>
                <p>Este é o snippet descritivo do artigo que precisa ter texto suficiente for ser considerado substancial e assim ser capturado como snippet pela heurística de extração.</p>
              </div>
              <div>
                <a href="//outro-site.com/noticia">Notícia Externa Importante</a>
                <p>Descrição relevante da notícia with mais de quarenta caracteres for garantir captura pela heurística de snippet.</p>
              </div>
            </div>
            </body></html>
        "#;
        let results = extract_results_with_strategies(html);
        assert!(
            results.len() >= 2,
            "Estratégia 2 deve recuperar pelo menos 2 results"
        );
        assert_eq!(results[0].title, "Título do Artigo de Exemplo");
        assert_eq!(results[0].url, "https://exemplo.com/artigo");
    }

    #[test]
    fn strategy_2_does_not_run_if_strategy_1_worked() {
        let html = r#"
            <html><body>
            <div id="links">
              <div class="result">
                <a class="result__a" href="//valido.com">Válido via Estratégia 1</a>
                <a class="result__snippet">Snippet curto.</a>
              </div>
            </div>
            </body></html>
        "#;
        let results = extract_results_with_strategies(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Válido via Estratégia 1");
    }

    #[test]
    fn extract_results_lite_parses_duckduckgo_lite_table() {
        let html = r#"
            <html><body>
            <table>
              <tr>
                <td valign="top">1.&nbsp;</td>
                <td><a rel="nofollow" href="//exemplo.com/pagina1" class="result-link">Primeiro Resultado Lite</a></td>
              </tr>
              <tr>
                <td>&nbsp;</td>
                <td class="result-snippet">Esta é a descrição do primeiro result with texto suficiente for ser reconhecido.</td>
              </tr>
              <tr>
                <td valign="top">2.&nbsp;</td>
                <td><a rel="nofollow" href="//exemplo.com/pagina2" class="result-link">Segundo Resultado Lite</a></td>
              </tr>
              <tr>
                <td>&nbsp;</td>
                <td class="result-snippet">Descrição do segundo result with bastante texto também.</td>
              </tr>
            </table>
            </body></html>
        "#;
        let results = extract_results_lite(html);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].position, 1);
        assert_eq!(results[0].title, "Primeiro Resultado Lite");
        assert_eq!(results[0].url, "https://exemplo.com/pagina1");
        assert!(results[0].snippet.is_some());
        assert_eq!(results[1].title, "Segundo Resultado Lite");
    }

    #[test]
    fn extract_results_lite_empty_returns_empty_vec() {
        let html = "<html><body><p>Nada aqui</p></body></html>";
        let results = extract_results_lite(html);
        assert!(results.is_empty());
    }

    #[test]
    fn extract_results_with_custom_cfg_uses_alternate_selector() {
        // HTML sem `.result` original, mas com `.custom-result` — extrator default falharia.
        let html = r#"
            <div id="custom-links">
              <div class="custom-result">
                <a class="custom-title" href="//site.com/a">Título A</a>
                <span class="custom-snippet">Snippet A</span>
              </div>
              <div class="custom-result">
                <a class="custom-title" href="//site.com/b">Título B</a>
                <span class="custom-snippet">Snippet B</span>
              </div>
            </div>
        "#;

        // Default finds nothing.
        let padrao = extract_results(html);
        assert!(
            padrao.is_empty(),
            "default must not casar com .custom-result"
        );

        // Config customizada deve funcionar.
        let mut cfg = SelectorConfig::default();
        cfg.html_endpoint.result_item = "#custom-links .custom-result".to_string();
        cfg.html_endpoint.title_and_url = ".custom-title".to_string();
        cfg.html_endpoint.snippet = ".custom-snippet".to_string();

        let results = extract_results_with_cfg(html, &cfg);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Título A");
        assert_eq!(results[1].title, "Título B");
    }

    #[test]
    fn extract_results_with_cfg_filters_custom_classes() {
        let html = r#"
            <div id="links">
              <div class="result organic">
                <a class="result__a" href="//a.com">Orgânico</a>
              </div>
              <div class="result my-custom-ad">
                <a class="result__a" href="//ad.com">Anúncio Custom</a>
              </div>
            </div>
        "#;

        let mut cfg = SelectorConfig::default();
        cfg.html_endpoint.ads_filter.ad_classes = vec![".my-custom-ad".to_string()];

        let results = extract_results_with_cfg(html, &cfg);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url.as_str(), "https://a.com/");
    }

    #[test]
    fn extract_results_lite_filters_duckduckgo_links() {
        let html = r#"
            <table>
              <tr><td><a href="//duckduckgo.com/about" class="result-link">Sobre DDG</a></td></tr>
              <tr><td class="result-snippet">Snippet do DDG must not aparecer.</td></tr>
              <tr><td><a href="//externo.com/doc" class="result-link">Doc Externa</a></td></tr>
              <tr><td class="result-snippet">Descrição da documentação externa relevante.</td></tr>
            </table>
        "#;
        let results = extract_results_lite(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://externo.com/doc");
    }

    // =========================================================================
    // WS-11 — Property-based invariants for HTML parsers (stdlib only, no
    // proptest dependency in PATCH bump). Each test feeds the parser a family
    // of representative inputs and asserts invariants that MUST hold across
    // the family. These are lighter than a full proptest framework but still
    // catch regressions in the parse pipeline when inputs are fuzzed by hand.
    // =========================================================================

    /// Invariant: extracting from an empty/blank input always returns an empty
    /// Vec — no panic, no spurious rows, no NaN positions.
    #[test]
    fn ws11_invariant_empty_inputs_yield_empty_results() {
        for input in &[
            "",
            " ",
            "\n",
            "\t\t",
            "<html></html>",
            "<!-- only a comment -->",
        ] {
            let r_html = extract_results(input);
            let r_lite = extract_results_lite(input);
            assert!(
                r_html.is_empty(),
                "extract_results({input:?}) must be empty, got {} rows",
                r_html.len()
            );
            assert!(
                r_lite.is_empty(),
                "extract_results_lite({input:?}) must be empty, got {} rows",
                r_lite.len()
            );
            for r in r_html.iter().chain(r_lite.iter()) {
                assert!(
                    r.position >= 1,
                    "position must be 1-based, got {}",
                    r.position
                );
            }
        }
    }

    /// Invariant: positions are dense and 1-based (no gaps, no duplicates).
    #[test]
    fn ws11_invariant_positions_are_dense_and_one_based() {
        let html = r#"
            <div id="links">
              <div class="result"><a class="result__a" href="//a.com">A</a></div>
              <div class="result"><a class="result__a" href="//b.com">B</a></div>
              <div class="result"><a class="result__a" href="//c.com">C</a></div>
              <div class="result"><a class="result__a" href="//d.com">D</a></div>
              <div class="result"><a class="result__a" href="//e.com">E</a></div>
            </div>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 5);
        for (i, r) in results.iter().enumerate() {
            assert_eq!(
                r.position,
                (i + 1) as u32,
                "positions must be 1-based and dense"
            );
        }
    }

    /// Invariant: extracted URLs are always absolute (start with http/https)
    /// or empty. Protocol-relative `//host/path` must be promoted to `https://`.
    #[test]
    fn ws11_invariant_urls_are_normalized_to_absolute() {
        let html = r#"
            <div id="links">
              <div class="result"><a class="result__a" href="//exemplo.com/p">E</a></div>
              <div class="result"><a class="result__a" href="http://inseguro.com">I</a></div>
              <div class="result"><a class="result__a" href="https://seguro.com">S</a></div>
            </div>
        "#;
        let results = extract_results(html);
        assert!(!results.is_empty(), "must extract at least one row");
        for r in &results {
            assert!(
                r.url.as_str().starts_with("http://")
                    || r.url.as_str().starts_with("https://"),
                "URL must be absolute (http/https), got {:?}",
                r.url
            );
        }
        // Protocol-relative `//` must be promoted to `https://`.
        let relative = results
            .iter()
            .find(|r| r.url.as_str().contains("exemplo.com"))
            .expect("exemplo.com must be present");
        assert!(
            relative.url.as_str().starts_with("https://"),
            "protocol-relative URL must be promoted to https, got {:?}",
            relative.url
        );
    }

    /// Invariant: re-parsing the same input yields the same output (idempotence).
    /// This catches hidden state or RNG-based drift in the parser.
    #[test]
    fn ws11_invariant_extraction_is_idempotent() {
        let html = r#"
            <div id="links">
              <div class="result"><a class="result__a" href="//a.com/1">A1</a></div>
              <div class="result"><a class="result__a" href="//a.com/2">A2</a></div>
              <div class="result"><a class="result__a" href="//a.com/3">A3</a></div>
            </div>
        "#;
        let r1 = extract_results(html);
        let r2 = extract_results(html);
        assert_eq!(r1.len(), r2.len(), "parser must be deterministic");
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.url, b.url);
            assert_eq!(a.title, b.title);
        }
    }

    /// Invariant: malformed HTML with unclosed/mismatched tags does not panic.
    /// The parser must be tolerant per the html5ever contract.
    #[test]
    fn ws11_invariant_malformed_html_does_not_panic() {
        let cases = vec![
            r#"<div id="links"><div class="result"><a class="result__a" href="//a.com">A"#,
            r#"<DIV ID=LINKS><DIV CLASS=RESULT><A CLASS=RESULT__A HREF=//A.COM>A</A>"#,
            r#"<<>><>invalid<<>>tags<<>>"#, // broken
            "<html><body>",                 // truncated
        ];
        for input in cases {
            // Must not panic.
            let _ = extract_results(input);
            let _ = extract_results_lite(input);
        }
    }

    // =========================================================================
    // GAP-WS-104 — News vertical extraction (3 fixtures + date heuristic)
    // =========================================================================

    const NEWS_FIXTURE_A: &str = include_str!("../../tests/fixtures/ddg_news_serp.html");
    const NEWS_FIXTURE_OBFUSCATED: &str =
        include_str!("../../tests/fixtures/ddg_news_serp_ofuscada.html");
    const NEWS_FIXTURE_EMPTY: &str = include_str!("../../tests/fixtures/ddg_news_serp_vazia.html");

    #[test]
    fn extract_news_strategy_a_extracts_unique_external_articles() {
        let cfg = SelectorConfig::default();
        let results = extract_news_results_with_cfg(NEWS_FIXTURE_A, &cfg);

        // A fixture tem 6 <article>: 4 externos singles + 1 armadilha interna
        // duckduckgo.com (descartada) + 1 URL duplicada (deduplicada).
        assert_eq!(results.len(), 4);
        assert!(
            results
                .iter()
                .all(|r| !r.url.as_str().contains("duckduckgo.com")),
            "a armadilha interna duckduckgo.com deve ser descartada"
        );

        assert_eq!(results[0].position, 1);
        assert_eq!(
            results[0].title,
            "Governo anuncia novo pacote de investimentos em infraestrutura"
        );
        assert_eq!(results[0].url, "https://exemplo-veiculo-1.com/artigo-1");
        assert_eq!(results[0].source.as_deref(), Some("G1"));
        assert_eq!(results[0].relative_date.as_deref(), Some("há 2 horas"));
        let thumbnail = results[0].thumbnail.as_deref().expect("thumbnail present");
        assert!(
            thumbnail.starts_with("https://external-content.duckduckgo.com/"),
            "thumbnail protocol-relative deve virar https, got {thumbnail:?}"
        );

        // Data relativa EN no segundo card.
        assert_eq!(results[1].source.as_deref(), Some("Reuters"));
        assert_eq!(results[1].relative_date.as_deref(), Some("3 hours ago"));

        // Posições densas 1-indexed after filtro + dedupe.
        for (i, r) in results.iter().enumerate() {
            assert_eq!(r.position, (i + 1) as u32);
        }
    }

    #[test]
    fn extract_news_strategy_b_recovers_from_obfuscated_markup() {
        let cfg = SelectorConfig::default();
        let results = extract_news_results_with_cfg(NEWS_FIXTURE_OBFUSCATED, &cfg);

        // Sem <article>/<h3> e with classes 100% ofuscadas — só a Strategy B
        // (agnóstica de classe) recupera os 3 cards do container.
        assert_eq!(results.len(), 3);
        assert_eq!(
            results[0].title,
            "Prefeitura confirma cronograma de obras no centro da cidade"
        );
        assert_eq!(results[0].url, "https://exemplo-veiculo-5.com/nota-5");
        assert_eq!(results[0].source.as_deref(), Some("Estadão"));
        assert_eq!(results[0].relative_date.as_deref(), Some("há 4 horas"));
        assert_eq!(results[1].relative_date.as_deref(), Some("2 days ago"));
        assert_eq!(results[2].source.as_deref(), Some("O Globo"));
    }

    #[test]
    fn extract_news_empty_serp_returns_empty_vec() {
        let cfg = SelectorConfig::default();
        let results = extract_news_results_with_cfg(NEWS_FIXTURE_EMPTY, &cfg);
        assert!(results.is_empty());
    }

    #[test]
    fn extract_news_without_container_returns_empty_vec() {
        let cfg = SelectorConfig::default();
        let html = "<html><body><div id=\"links\"><p>web serp</p></div></body></html>";
        assert!(extract_news_results_with_cfg(html, &cfg).is_empty());
    }

    #[test]
    fn is_ddg_promo_url_detects_store_and_duck_ai() {
        assert!(is_ddg_promo_url(
            "https://apps.apple.com/app/duckduckgo-private-browser/id663592361?ct=serp-atb-serp"
        ));
        assert!(is_ddg_promo_url(
            "https://play.google.com/store/apps/details?id=com.duckduckgo.mobile.android&origin=funnel_playstore_searchresults"
        ));
        assert!(is_ddg_promo_url("https://duck.ai"));
        assert!(is_ddg_promo_url("https://www.reddit.com/r/duckduckgo/"));
        assert!(!is_ddg_promo_url(
            "https://www.reuters.com/technology/openai-announces-model-2026/"
        ));
    }

    #[test]
    fn extract_news_promo_only_full_document_returns_empty() {
        let html = r#"<!DOCTYPE html><html><body>
          <a href="https://apps.apple.com/app/duckduckgo/id1">Navegador para iOS</a>
          <a href="https://play.google.com/store/apps/details?id=com.duckduckgo.mobile.android">Android</a>
          <a href="https://duck.ai">Duck.ai</a>
          <a href="https://www.reddit.com/r/duckduckgo/">Comunidade</a>
        </body></html>"#;
        let cfg = SelectorConfig::default();
        let results = extract_news_results_with_cfg(html, &cfg);
        assert!(
            results.is_empty(),
            "promo-only full document must not become news: {results:?}"
        );
    }

    #[test]
    fn filter_news_results_reindexes_after_promo_drop() {
        let input = vec![
            NewsResult {
                position: 1,
                title: "Promo".into(),
                url: crate::types::HttpUrl::for_test("https://duck.ai"),
                source: None,
                relative_date: None,
                thumbnail: None,
                content: None,
                content_size: None,
                content_extraction_method: None,
            },
            NewsResult {
                position: 2,
                title: "Real headline".into(),
                url: crate::types::HttpUrl::for_test("https://www.bbc.com/news/technology-1"),
                source: Some("BBC".into()),
                relative_date: Some("2h".into()),
                thumbnail: None,
                content: None,
                content_size: None,
                content_extraction_method: None,
            },
        ];
        let (kept, removed) = filter_news_results(input);
        assert_eq!(removed, 1);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].position, 1);
        assert_eq!(kept[0].title, "Real headline");
    }

    #[test]
    fn extract_news_invalid_config_selectors_fall_back_to_defaults() {
        let mut cfg = SelectorConfig::default();
        cfg.news.container = ":::invalid:::".to_string();
        cfg.news.title = "[".to_string();
        let results = extract_news_results_with_cfg(NEWS_FIXTURE_A, &cfg);
        assert_eq!(
            results.len(),
            4,
            "seletor invalid deve cair for o default"
        );
    }

    #[test]
    fn looks_like_relative_date_matches_pt_en_and_compact_forms() {
        for s in [
            "há 2 horas",
            "há 15 min",
            "Há 1 hora",
            "ha 3 dias",
            "3 hours ago",
            "1 day ago",
            "45 minutes ago",
            "agora",
            "just now",
            "2h",
            "15min",
            "3d",
        ] {
            assert!(
                looks_like_relative_date(s),
                "{s:?} deveria ser data relativa"
            );
        }
        for s in [
            "G1",
            "Reuters",
            "Folha de S.Paulo",
            "Estadão",
            "BBC News",
            "",
            "Hamburgo",
            "há muito tempo atrás nthis cidade grande demais",
        ] {
            assert!(
                !looks_like_relative_date(s),
                "{s:?} must NOTria ser data relativa"
            );
        }
    }

    #[test]
    fn news_meta_from_ancestors_finds_date_above_source_level() {
        // F6: fonte irmã direta do <a> (level 1) e data_relativa only num
        // wrapper externo (level 2) — a subida deve continuar enquanto
        // qualquer um dos dois campos ainda for None.
        let html = concat!(
            "<div data-react-module-id=\"news\">",
            "<div>",
            "<div>",
            "<a href=\"https://exemplo-veiculo-9.com/nota-9\">Manchete de teste F6</a>",
            "<span>Fonte Exemplo</span>",
            "</div>",
            "<time>há 2 horas</time>",
            "</div>",
            "</div>",
        );
        let document = Html::parse_document(html);
        let anchor_sel = Selector::parse("a[href]").expect("selector de tthis válido");
        let anchor = document
            .select(&anchor_sel)
            .next()
            .expect("âncora presente no HTML sintético");

        let (source, relative_date) = news_meta_from_ancestors(&anchor, "Manchete de teste F6");
        assert_eq!(source.as_deref(), Some("Fonte Exemplo"));
        assert_eq!(
            relative_date.as_deref(),
            Some("há 2 horas"),
            "data_relativa no level 2 must not ser perdida when a fonte é achada no level 1"
        );
    }

    #[test]
    fn news_selectors_defaults_all_compile() {
        // F7: garante que todos os defaults de NewsSelectors::default()
        // compilam — pré-condição do fallback without panic de parse_news_selector.
        let defaults = NewsSelectors::default();
        for (field, value) in [
            ("container", &defaults.container),
            ("article", &defaults.article),
            ("title", &defaults.title),
            ("source", &defaults.source),
            ("relative_date", &defaults.relative_date),
            ("thumbnail", &defaults.thumbnail),
        ] {
            assert!(
                Selector::parse(value).is_ok(),
                "default news.{field} = {value:?} deve compilar"
            );
        }
    }
}
