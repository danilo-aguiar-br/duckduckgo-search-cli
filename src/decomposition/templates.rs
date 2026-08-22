// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (string transformations, no I/O).
//! Domain types and canonical template tables for query decomposition.

use serde::{Deserialize, Serialize};

/// A single sub-query produced by decomposition (parse-don't-validate).
///
/// `text` is a [`crate::security::ValidatedQuery`] so internal fan-out never
/// sees an unvalidated string (GAP-TYPE-002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubQuery {
    /// The sub-query text to be sent to `DuckDuckGo` (NFC + charset validated).
    pub text: crate::security::ValidatedQuery,
    /// Origin label — `heuristic:<template>`, `manual`, or `heuristic:refine`.
    pub origin: SubQueryOrigin,
}

impl SubQuery {
    /// Returns a short, stable label for logs and JSON output.
    pub fn strategy_label(&self) -> String {
        match &self.origin {
            SubQueryOrigin::Heuristic { template } => format!("heuristic:{}", template.as_str()),
            SubQueryOrigin::Manual => "manual".to_string(),
            SubQueryOrigin::HeuristicRefine => "heuristic:refine".to_string(),
        }
    }
}

/// Origin of a sub-query, used for observability and JSON reporting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubQueryOrigin {
    /// Produced by a named heuristic template.
    Heuristic {
        /// The template that produced this sub-query.
        template: HeuristicTemplate,
    },
    /// Loaded from a manual file/stdin list.
    Manual,
    /// Generated as a refinement of the original query when the templates did
    /// not fill the `max_sub_queries` budget.
    HeuristicRefine,
}

/// Canonical heuristic templates for query fan-out (v1.0.2: +News for dual).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeuristicTemplate {
    /// Focused aspect of the topic (e.g. `"<q> key components"`).
    Aspect,
    /// Comparison framing (e.g. `"<q> vs alternatives"`).
    Comparison,
    /// Timeline framing (e.g. `"<q> history timeline"`).
    Timeline,
    /// Opinion framing (e.g. `"<q> reviews opinions"`).
    Opinion,
    /// Causal framing (e.g. `"<q> causes effects"`).
    Cause,
    /// News/recency framing for dual web+news deep-research (GAP-AUD-DR-003).
    News,
}

impl HeuristicTemplate {
    /// Returns the kebab-case name of the template (used in labels and logs).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aspect => "aspect",
            Self::Comparison => "comparison",
            Self::Timeline => "timeline",
            Self::Opinion => "opinion",
            Self::Cause => "cause",
            Self::News => "news",
        }
    }

    /// Returns the suffix appended to the original query for this template.
    ///
    /// # Examples
    ///
    /// ```
    /// use duckduckgo_search_cli::decomposition::HeuristicTemplate;
    ///
    /// assert_eq!(HeuristicTemplate::Aspect.suffix(), "main aspects components");
    /// assert_eq!(HeuristicTemplate::Comparison.suffix(), "vs alternatives comparison");
    /// assert_eq!(HeuristicTemplate::Cause.suffix(), "causes effects consequences");
    /// assert_eq!(HeuristicTemplate::News.suffix(), "latest news recent press");
    /// assert_eq!(HeuristicTemplate::all().len(), 6);
    /// ```
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Aspect => "main aspects components",
            Self::Comparison => "vs alternatives comparison",
            Self::Timeline => "history timeline evolution",
            Self::Opinion => "reviews opinions expert",
            Self::Cause => "causes effects consequences",
            Self::News => "latest news recent press",
        }
    }

    /// Returns the list of all templates in canonical order (web first, news last).
    pub fn all() -> [Self; 6] {
        [
            Self::Aspect,
            Self::Comparison,
            Self::Timeline,
            Self::Opinion,
            Self::Cause,
            Self::News,
        ]
    }
}
