// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (string transformations, no I/O).
//! Heuristic decomposition strategy and composite-query detection.

use super::templates::{HeuristicTemplate, SubQuery, SubQueryOrigin};
use crate::error::CliError;
use regex::Regex;

/// Compile a static pattern with hard size limits (ReDoS defence-in-depth).
///
/// Patterns are compile-time constants and non-nested; limits still bound
/// worst-case DFA memory if a future edit introduces a pathological pattern.
fn static_regex(pattern: &'static str) -> Regex {
    use regex::RegexBuilder;
    // 1 MiB compile / DFA caps — far above needs for these short patterns.
    RegexBuilder::new(pattern)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 20)
        .build()
        .expect("static regex pattern is valid and must compile")
}

/// Heuristic signals that a query is already a compound (multi-concept) one.
/// When matched, the corresponding [`HeuristicTemplate`] is suppressed from
/// the fan-out to avoid emitting redundant or self-contradictory sub-queries
/// like `rust vs go vs alternatives comparison`.
///
/// The detection is conservative: a single match on the high-signal
/// patterns below is enough to mark the query as composite for that
/// dimension. False positives are tolerable (the worst case is one
/// fewer sub-query); false negatives are not (the worst case is a
/// nonsensical `"X vs Y vs alternatives comparison"` sub-query).
///
/// # Panics
///
/// Panics only if one of the static regexes fails to compile, which
/// cannot happen with the current literals — size-capped `RegexBuilder`
/// is a defence-in-depth marker for future edits.
pub fn is_composite_query(query: &str, signal: CompositeSignal) -> bool {
    use std::sync::LazyLock;
    static RE_VS: LazyLock<Regex> = LazyLock::new(|| {
        // "X vs Y", "X versus Y", "X or Y" — case-insensitive.
        static_regex(r"(?i)\b(vs\.?|versus|or)\b")
    });
    static RE_AND: LazyLock<Regex> = LazyLock::new(|| {
        // "X and Y", "X & Y", "X, Y" — case-insensitive.
        static_regex(r"(?i)\b(and)\b|\s&\s|,\s")
    });
    static RE_COLON: LazyLock<Regex> = LazyLock::new(|| {
        // "topic: subtopic" — explicit hierarchical decomposition.
        static_regex(r":\s|\s-\s")
    });
    static RE_TIMELINE: LazyLock<Regex> = LazyLock::new(|| {
        // "history of", "evolution of", "from ... to ...", year ranges.
        static_regex(r"(?i)\b(history|evolution|timeline|chronolog)|(\b\d{4}\b.*\b\d{4}\b)")
    });
    static RE_OPINION: LazyLock<Regex> = LazyLock::new(|| {
        // "reviews", "opinions", "best", "worst", "rating".
        static_regex(r"(?i)\b(review|opinion|rating|best|worst)\b")
    });
    static RE_CAUSE: LazyLock<Regex> = LazyLock::new(|| {
        // "causes", "effects", "consequences", "why", "impact".
        static_regex(r"(?i)(causes?|effects?|consequences?|why|impact)\b")
    });

    let re: &Regex = match signal {
        CompositeSignal::Comparison => &RE_VS,
        CompositeSignal::Aspect => &RE_AND,
        CompositeSignal::Timeline => &RE_TIMELINE,
        CompositeSignal::Opinion => &RE_OPINION,
        CompositeSignal::Cause => &RE_CAUSE,
        CompositeSignal::Topic => &RE_COLON,
    };
    re.is_match(query)
}

/// High-level decomposition signal a query might already encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeSignal {
    /// Query is already a comparison (`X vs Y`).
    Comparison,
    /// Query is already a multi-aspect listing (`X and Y`).
    Aspect,
    /// Query is already timeline-shaped (`history of X`).
    Timeline,
    /// Query is already opinion-shaped (`best X`).
    Opinion,
    /// Query is already cause-shaped (`why X`).
    Cause,
    /// Query is already topic-decomposed (`topic: subtopic`).
    Topic,
}

pub(super) fn heuristic_decompose(
    query: &str,
    max_sub_queries: usize,
    news_aware: bool,
) -> Result<Vec<SubQuery>, CliError> {
    let mut out: Vec<SubQuery> = Vec::with_capacity(max_sub_queries);

    // CM-08: when dual news is enabled, pin a recency template first so news
    // SERP is not starved by evergreen web suffixes (aspects/components…).
    if news_aware && max_sub_queries > 0 {
        let t = HeuristicTemplate::News;
        let raw = format!("{} {}", query, t.suffix());
        out.push(SubQuery {
            text: crate::security::ValidatedQuery::try_new(&raw)?,
            origin: SubQueryOrigin::Heuristic { template: t },
        });
    }

    let template_for_signal = [
        (CompositeSignal::Aspect, HeuristicTemplate::Aspect),
        (CompositeSignal::Comparison, HeuristicTemplate::Comparison),
        (CompositeSignal::Timeline, HeuristicTemplate::Timeline),
        (CompositeSignal::Opinion, HeuristicTemplate::Opinion),
        (CompositeSignal::Cause, HeuristicTemplate::Cause),
    ];

    for (_, t) in template_for_signal
        .into_iter()
        .filter(|(sig, _)| !is_composite_query(query, *sig))
    {
        if out.len() >= max_sub_queries {
            break;
        }
        let raw = format!("{} {}", query, t.suffix());
        out.push(SubQuery {
            text: crate::security::ValidatedQuery::try_new(&raw)?,
            origin: SubQueryOrigin::Heuristic { template: t },
        });
    }

    // If the user requested more sub-queries than templates, top up with
    // language refinements of the original query.
    let mut refine_index: usize = 0;
    let refinements = if news_aware {
        [
            "breaking news today",
            "tutorial guide",
            "examples use cases",
            "best practices",
        ]
    } else {
        [
            "tutorial guide",
            "examples use cases",
            "best practices",
            "overview summary",
        ]
    };
    while out.len() < max_sub_queries {
        let suffix = refinements[refine_index % refinements.len()];
        let raw = format!("{query} {suffix}");
        out.push(SubQuery {
            text: crate::security::ValidatedQuery::try_new(&raw)?,
            origin: SubQueryOrigin::HeuristicRefine,
        });
        refine_index += 1;
        if refine_index >= refinements.len() * 4 {
            // Defensive: never loop forever even if a caller passes a huge value.
            break;
        }
    }
    Ok(out)
}
