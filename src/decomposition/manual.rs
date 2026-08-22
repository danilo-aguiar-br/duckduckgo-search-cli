// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (reads the manual sub-query list from disk).
//! Manual decomposition strategy: sub-queries read from a file list.

use super::templates::{SubQuery, SubQueryOrigin};
use crate::error::CliError;
use std::path::Path;

pub(super) async fn load_manual(
    path: Option<&Path>,
    max_sub_queries: usize,
) -> Result<Vec<SubQuery>, CliError> {
    let path = path.ok_or_else(|| CliError::InvalidConfig {
        message: "manual sub-query strategy requires --sub-queries-file".to_string(),
    })?;
    let contents = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| CliError::InvalidConfig {
            message: format!("read sub-queries file: {e}"),
        })?;
    let mut out: Vec<SubQuery> = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        out.push(SubQuery {
            text: crate::security::ValidatedQuery::try_new(trimmed).map_err(|e| match e {
                CliError::InvalidConfig { message } => CliError::InvalidConfig {
                    message: format!("manual sub-query: {message}"),
                },
                other => other,
            })?,
            origin: SubQueryOrigin::Manual,
        });
        if out.len() >= max_sub_queries {
            break;
        }
    }
    if out.is_empty() {
        return Err(CliError::InvalidConfig {
            message: format!(
                "no sub-queries found in {} (file empty or only comments)",
                path.display()
            ),
        });
    }
    Ok(out)
}
