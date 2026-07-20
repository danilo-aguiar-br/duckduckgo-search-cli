// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (CSS selector config DTOs + Validate)
//! External `selectors.toml` DTOs (GAP-SERDE-009).

use crate::validation::limits;
use serde::{Deserialize, Serialize};
use validator::Validate;

// Align validate length attrs with SSOT caps.
const _: () = {
    let _ = limits::MAX_CSS_SELECTOR_CHARS;
    let _ = limits::MAX_SELECTOR_LIST_ITEMS;
};

/// CSS selector configuration (loaded from selectors.toml or hardcoded defaults).
///
/// Retains the existing fields (`html_endpoint`) for backward compatibility with
/// tests and selector hashing. Starting from iteration 6, adds flat additional
/// fields for the Lite endpoint, pagination, and related searches, enabling
/// full externalization via an external TOML file.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct SelectorConfig {
    /// Legacy group — retained for compatibility with existing serialization and tests.
    #[validate(nested)]
    pub html_endpoint: HtmlSelectors,

    /// Selector group for the Lite endpoint.
    #[serde(default)]
    #[validate(nested)]
    pub lite_endpoint: LiteSelectors,

    /// Selectors used to extract pagination data (form `s`).
    #[serde(default)]
    #[validate(nested)]
    pub pagination: PaginationSelectors,

    /// Selectors used to extract "related searches".
    #[serde(default)]
    #[validate(nested)]
    pub related_searches: RelatedSelectors,

    /// Selector group for the news vertical (`--vertical news|all`).
    /// GAP-WS-104 v0.8.9.
    #[serde(default)]
    #[validate(nested)]
    pub news: NewsSelectors,
}

/// CSS selectors for the full HTML endpoint (`html.duckduckgo.com`).
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct HtmlSelectors {
    /// Outer container holding all organic results.
    #[validate(length(min = 1, max = 2048, message = "HtmlSelectors.results_container length out of range"))]
    pub results_container: String,
    /// Individual result item (excludes ads).
    #[validate(length(min = 1, max = 2048, message = "HtmlSelectors.result_item length out of range"))]
    pub result_item: String,
    /// Link element carrying the title and destination URL.
    #[validate(length(min = 1, max = 2048, message = "HtmlSelectors.title_and_url length out of range"))]
    pub title_and_url: String,
    /// Element containing the result snippet/description.
    #[validate(length(min = 1, max = 2048, message = "HtmlSelectors.snippet length out of range"))]
    pub snippet: String,
    /// Element showing the display URL below the title.
    #[validate(length(min = 1, max = 2048, message = "HtmlSelectors.display_url length out of range"))]
    pub display_url: String,
    /// Rules for filtering out sponsored/ad results.
    #[validate(nested)]
    pub ads_filter: AdFilter,
}

/// Patterns used to detect and filter out sponsored results.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct AdFilter {
    /// CSS classes that mark an element as an ad.
    #[validate(length(max = 256, message = "AdFilter.ad_classes exceeds max items"))]
    pub ad_classes: Vec<String>,
    /// HTML attributes indicating sponsored content.
    #[validate(length(max = 256, message = "AdFilter.ad_attributes exceeds max items"))]
    pub ad_attributes: Vec<String>,
    /// URL substrings found in ad-tracking redirects.
    #[validate(length(max = 256, message = "AdFilter.ad_url_patterns exceeds max items"))]
    pub ad_url_patterns: Vec<String>,
}

/// CSS selectors for the lite endpoint (`lite.duckduckgo.com`).
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct LiteSelectors {
    /// Table element wrapping all results.
    #[validate(length(min = 1, max = 2048, message = "LiteSelectors.results_table length out of range"))]
    pub results_table: String,
    /// Anchor element linking to the result page.
    #[validate(length(min = 1, max = 2048, message = "LiteSelectors.result_link length out of range"))]
    pub result_link: String,
    /// Cell containing the result snippet text.
    #[validate(length(min = 1, max = 2048, message = "LiteSelectors.result_snippet length out of range"))]
    pub result_snippet: String,
}

/// CSS selectors for extracting pagination tokens from the HTML form.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct PaginationSelectors {
    /// Hidden input carrying the `vqd` token.
    #[validate(length(min = 1, max = 2048, message = "PaginationSelectors.vqd_input length out of range"))]
    pub vqd_input: String,
    /// Hidden input carrying the `s` (start offset) value.
    #[validate(length(min = 1, max = 2048, message = "PaginationSelectors.s_input length out of range"))]
    pub s_input: String,
    /// Hidden input carrying the `dc` (document count) value.
    #[validate(length(min = 1, max = 2048, message = "PaginationSelectors.dc_input length out of range"))]
    pub dc_input: String,
    /// Form element for the "next page" action.
    #[validate(length(min = 1, max = 2048, message = "PaginationSelectors.next_form length out of range"))]
    pub next_form: String,
}

/// CSS selectors for related-searches links (currently unused; DDG HTML does not expose them).
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct RelatedSelectors {
    /// Container element for the related-searches block.
    #[validate(length(min = 1, max = 2048, message = "RelatedSelectors.container length out of range"))]
    pub container: String,
    /// Anchor elements inside the related-searches block.
    #[validate(length(min = 1, max = 2048, message = "RelatedSelectors.links length out of range"))]
    pub links: String,
}

/// CSS selectors for the news vertical (`ia=news&iar=news`, Chrome-rendered).
///
/// The DDG news module is a React component with obfuscated per-build
/// classes — `container`/`article` anchor on the semantic
/// `data-react-module-id` attribute (Strategy A). When the module markup
/// changes, `extraction::extract_news_results_with_cfg` falls back to a
/// class-agnostic strategy that ignores these selectors entirely.
/// GAP-WS-104 v0.8.9.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(default)]
pub struct NewsSelectors {
    /// Outer container holding the news module.
    #[validate(length(min = 1, max = 2048, message = "NewsSelectors.container length out of range"))]
    pub container: String,
    /// Individual news card/article element.
    #[validate(length(min = 1, max = 2048, message = "NewsSelectors.article length out of range"))]
    pub article: String,
    /// Headline element within the article.
    #[validate(length(min = 1, max = 2048, message = "NewsSelectors.title length out of range"))]
    pub title: String,
    /// Publisher/source element within the article.
    #[validate(length(min = 1, max = 2048, message = "NewsSelectors.source length out of range"))]
    pub source: String,
    /// Relative-date element within the article (disambiguated from
    /// `source` via `extraction::looks_like_relative_date`).
    #[validate(length(min = 1, max = 2048, message = "NewsSelectors.relative_date length out of range"))]
    pub relative_date: String,
    /// Thumbnail `<img>` element within the article.
    #[validate(length(min = 1, max = 2048, message = "NewsSelectors.thumbnail length out of range"))]
    pub thumbnail: String,
}

impl Default for HtmlSelectors {
    fn default() -> Self {
        Self {
            results_container: "#links".to_string(),
            result_item:
                "#links .result:not(.result--ad), #links .results_links, div.result:not(.result--ad)"
                    .to_string(),
            title_and_url: ".result__a, a.result__a, .result__title a".to_string(),
            // v0.3.0: removido `.result__body` — casava o container pai e trazia
            // titulo+url+snippet concatenados no campo snippet.
            snippet: ".result__snippet, a.result__snippet".to_string(),
            display_url: ".result__url, span.result__url".to_string(),
            ads_filter: AdFilter::default(),
        }
    }
}

impl Default for AdFilter {
    fn default() -> Self {
        Self {
            ad_classes: vec![".result--ad".to_string(), ".badge--ad".to_string()],
            ad_attributes: vec!["data-nrn=ad".to_string()],
            ad_url_patterns: vec!["duckduckgo.com/y.js".to_string()],
        }
    }
}

impl Default for LiteSelectors {
    fn default() -> Self {
        Self {
            results_table: "table, body table".to_string(),
            result_link: "a.result-link, td a[href]".to_string(),
            result_snippet: "td.result-snippet, tr.result-snippet td".to_string(),
        }
    }
}

impl Default for PaginationSelectors {
    fn default() -> Self {
        Self {
            vqd_input: "input[name='vqd'], input[type='hidden'][name='vqd']".to_string(),
            s_input: "input[name='s']".to_string(),
            dc_input: "input[name='dc']".to_string(),
            next_form: "form.result--more__btn, form[action='/html/']".to_string(),
        }
    }
}

impl Default for RelatedSelectors {
    fn default() -> Self {
        Self {
            container: ".result--more__btn, .result--sep".to_string(),
            links: "a".to_string(),
        }
    }
}

impl Default for NewsSelectors {
    fn default() -> Self {
        Self {
            container: "[data-testid=\"news-vertical\"], [data-react-module-id=\"news\"]"
                .to_string(),
            article: "article, [data-testid=\"result\"], li".to_string(),
            title: "h2, h3, h4, a[data-testid=\"result-title-a\"]".to_string(),
            source: "span, time".to_string(),
            relative_date: "span, time".to_string(),
            thumbnail: "img".to_string(),
        }
    }
}
