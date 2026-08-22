// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (CSS selector compilation)
//! Compiled CSS selectors for the html and lite SERP endpoints.

use crate::types::SelectorConfig;
use scraper::Selector;
use std::sync::LazyLock;

pub(super) fn sel_tr() -> &'static Selector {
    static C: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("tr").expect("static CSS selector 'tr' is valid"));
    &C
}

pub(super) fn sel_strategy2_links() -> &'static Selector {
    static C: LazyLock<Selector> = LazyLock::new(|| {
        Selector::parse("#links a[href], .result a[href]")
            .expect("static CSS selector for result links is valid")
    });
    &C
}

pub(super) struct CompiledSelectors {
    pub result_item: Selector,
    pub ad_class: Option<Selector>,
    pub title_sel: Option<Selector>,
    pub snippet: Option<Selector>,
    pub display_url_sel: Option<Selector>,
    pub ad_classes_raw: Vec<String>,
    pub ad_attributes: Vec<(String, String)>,
    pub url_patterns: Vec<String>,
}

impl CompiledSelectors {
    pub fn compile(cfg: &SelectorConfig) -> Option<Self> {
        let result_item = match Selector::parse(&cfg.html_endpoint.result_item) {
            Ok(s) => s,
            Err(error) => {
                tracing::error!(
                    ?error,
                    selector = %cfg.html_endpoint.result_item,
                    "Result selector invalid — cannot extract"
                );
                return None;
            }
        };
        let join_ad = cfg.html_endpoint.ads_filter.ad_classes.join(", ");
        let ad_class = if join_ad.is_empty() {
            None
        } else {
            Selector::parse(&join_ad).ok()
        };
        let title_sel = Selector::parse(&cfg.html_endpoint.title_and_url).ok();
        let snippet = Selector::parse(&cfg.html_endpoint.snippet).ok();
        let display_url_sel = Selector::parse(&cfg.html_endpoint.display_url).ok();
        let ad_classes_raw = cfg
            .html_endpoint
            .ads_filter
            .ad_classes
            .iter()
            .map(|c| c.trim_start_matches('.').to_string())
            .collect();
        let ad_attributes = cfg
            .html_endpoint
            .ads_filter
            .ad_attributes
            .iter()
            .filter_map(|e| {
                let mut parts = e.splitn(2, '=');
                let key = parts.next()?.trim().to_string();
                let value = parts.next()?.trim().to_string();
                Some((key, value))
            })
            .collect();
        let url_patterns = cfg.html_endpoint.ads_filter.ad_url_patterns.clone();
        Some(Self {
            result_item,
            ad_class,
            title_sel,
            snippet,
            display_url_sel,
            ad_classes_raw,
            ad_attributes,
            url_patterns,
        })
    }
}

pub(super) struct CompiledLiteSelectors {
    pub link: Selector,
    pub snippet_td: Selector,
}

impl CompiledLiteSelectors {
    pub fn compile(cfg: &SelectorConfig) -> Option<Self> {
        let link = Selector::parse(&cfg.lite_endpoint.result_link)
            .or_else(|_| Selector::parse("a.result-link, a"))
            .ok()?;
        let snippet_td = Selector::parse(&cfg.lite_endpoint.result_snippet)
            .or_else(|_| Selector::parse("td.result-snippet, td"))
            .ok()?;
        Some(Self { link, snippet_td })
    }
}
