// SPDX-License-Identifier: MIT OR Apache-2.0
//! Per-sub-query news diagnosis helpers (GAP-WS-105 / CM-09).

use crate::types::SearchOutput;

/// Max chars for `news_diagnosis` (anti-token / memory).
const NEWS_DIAGNOSIS_CAP: usize = 200;

/// Maps a sub-query's news scan result to the [`super::SubQueryOutcome`] fields.
///
/// - `--no-news` set: both fields are `None` (news was never expected).
/// - `news_len = Some(n)`: the scan ran — report the count, even when zero.
/// - `news_len = None` without `--no-news`: the news vertical became
///   unavailable mid-flight — flag it. GAP-WS-105 v0.8.9.
pub(crate) fn sub_query_news_fields(
    no_news: bool,
    news_len: Option<usize>,
) -> (Option<usize>, Option<bool>) {
    if no_news {
        (None, None)
    } else {
        match news_len {
            Some(n) => (Some(n), None),
            None => (None, Some(true)),
        }
    }
}

/// Per-sub-query news diagnosis bundle (CM-09).
#[derive(Debug, Clone, Default)]
pub(crate) struct SubQueryNewsDiag {
    pub(crate) news_count: Option<usize>,
    pub(crate) news_unavailable: Option<bool>,
    pub(crate) zero_cause: Option<crate::types::ZeroCause>,
    pub(crate) news_error: Option<String>,
    pub(crate) news_diagnosis: Option<String>,
}

/// Build rich per-sub-query news fields from a fan-out [`SearchOutput`] (CM-09).
pub(crate) fn sub_query_news_diagnosis(no_news: bool, output: &SearchOutput) -> SubQueryNewsDiag {
    let (news_count, news_unavailable) =
        sub_query_news_fields(no_news, output.news.as_ref().map(Vec::len));
    if no_news {
        return SubQueryNewsDiag::default();
    }
    let zero_cause = output.metadata.zero_cause;
    let mut news_error = None;
    let mut news_diagnosis = None;
    if news_unavailable == Some(true) {
        news_error = Some(
            output
                .message
                .as_deref()
                .and_then(|m| m.strip_prefix("news_vertical_unavailable:"))
                .map(str::to_string)
                .unwrap_or_else(|| "news_vertical_unavailable".to_string()),
        );
        let raw = output
            .metadata
            .next_action_suggestion
            .clone()
            .or_else(|| output.message.clone())
            .unwrap_or_else(|| {
                "News vertical unavailable mid-flight; web results may still be valid.".into()
            });
        news_diagnosis = Some(truncate_diag(&raw, NEWS_DIAGNOSIS_CAP));
    } else if news_count == Some(0) && zero_cause.is_some() {
        news_diagnosis = output
            .metadata
            .next_action_suggestion
            .as_ref()
            .map(|s| truncate_diag(s, NEWS_DIAGNOSIS_CAP));
    }
    SubQueryNewsDiag {
        news_count,
        news_unavailable,
        zero_cause,
        news_error,
        news_diagnosis,
    }
}

fn truncate_diag(s: &str, cap: usize) -> String {
    crate::text::truncate_to_chars(s, cap).to_string()
}
