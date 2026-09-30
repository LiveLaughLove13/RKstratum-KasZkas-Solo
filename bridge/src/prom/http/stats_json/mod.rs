//! `/api/stats` JSON: aggregate Prometheus metrics for the web dashboard.
//!
//! - [`types`] — serde DTOs for the dashboard API
//! - [`parse`] — label parsing and Prometheus helpers
//! - [`aggregate`] — gather metrics, fold workers/blocks, activity filter

mod aggregate;
mod parse;
mod types;

pub(crate) use aggregate::{get_stats_json, get_stats_json_all};
#[cfg(test)]
pub(crate) use types::BlockInfo;
pub(crate) use types::StatsResponse;
