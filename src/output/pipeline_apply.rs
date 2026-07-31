// SPDX-License-Identifier: MIT OR Apache-2.0
//! Post-SERP project/filter/limit helpers used by the main emit path.

use crate::pipeline::PipelineResult;

use super::{FieldSet, ResultFilter};

/// Parse optional `--fields` / `--select` into a [`FieldSet`].
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when the field list is invalid.
pub fn config_fields_parse(
    raw: Option<&str>,
) -> Result<Option<FieldSet>, crate::error::CliError> {
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(FieldSet::parse(s)?)),
    }
}

/// Parse optional `--filter` into a [`ResultFilter`].
///
/// # Errors
///
/// Returns [`crate::error::CliError`] when the filter expression is invalid.
pub fn config_filter_parse(
    raw: Option<&str>,
) -> Result<Option<ResultFilter>, crate::error::CliError> {
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(ResultFilter::parse(s)?)),
    }
}

/// Apply field projection and/or result filter; returns hit count after filter.
pub fn apply_project_filter(
    output: &mut PipelineResult,
    fields: Option<&FieldSet>,
    filter: Option<&ResultFilter>,
) -> u32 {
    if fields.is_none() && filter.is_none() {
        return match output {
            PipelineResult::Single(s) => {
                (s.results.len() as u32)
                    .saturating_add(s.news.as_ref().map_or(0, |n| n.len() as u32))
            }
            PipelineResult::Multi(m) => m
                .searches
                .iter()
                .map(|s| {
                    (s.results.len() as u32)
                        .saturating_add(s.news.as_ref().map_or(0, |n| n.len() as u32))
                })
                .fold(0u32, u32::saturating_add),
            PipelineResult::Stream(_) => 0,
        };
    }
    match output {
        PipelineResult::Single(s) => {
            super::project::apply_to_search_output(s, fields, filter)
        }
        PipelineResult::Multi(m) => {
            super::project::apply_to_multi_output(m, fields, filter)
        }
        PipelineResult::Stream(_) => 0,
    }
}

/// Cap result lists with agent-native `--limit` (post filter/sort/dedupe).
pub fn apply_result_limit(output: &mut PipelineResult, limit: Option<u32>) {
    if limit.is_none() {
        return;
    }
    match output {
        PipelineResult::Single(s) => {
            super::project::apply_result_limit_search(s, limit);
        }
        PipelineResult::Multi(m) => {
            super::project::apply_result_limit_multi(m, limit);
        }
        PipelineResult::Stream(_) => {}
    }
}

/// Mark empty post-filter search envelopes with `error=filter_empty`.
pub fn mark_filter_empty(output: &mut PipelineResult) {
    match output {
        PipelineResult::Single(s) => {
            super::project::mark_filter_empty_search(s);
        }
        PipelineResult::Multi(m) => {
            for s in &mut m.searches {
                if s.results.is_empty()
                    && s.news.as_ref().is_none_or(|n| n.is_empty())
                    && s.error.is_none()
                {
                    super::project::mark_filter_empty_search(s);
                }
            }
        }
        PipelineResult::Stream(_) => {}
    }
}
