mod diff_policy;
mod duplicate_submit;
mod kaspa_api_trait;
mod lifecycle;
mod submit;
mod vardiff;
pub(crate) mod work_stats;

pub use diff_policy::{
    apply_worker_requested_diff, diff_floor_for_app, is_marketplace_app, is_nicehash_app,
    parse_worker_diff_override, start_diff_for_app, uses_eth_stratum_hex_job,
};
pub use kaspa_api_trait::KaspaApiTrait;
pub use lifecycle::average_worker_spm;
pub use submit::{SubmitError, SubmitRunError};
#[cfg(feature = "rkstratum_cpu_miner")]
pub use work_stats::{RKSTRATUM_CPU_MINER_METRICS, set_rkstratum_cpu_miner_metrics};
pub use work_stats::{STATS_PRINTER_STARTED, WorkStats};
pub(crate) use work_stats::{terminal_hashrate_key, terminal_session_hashrates_ghs};

use duplicate_submit::DuplicateSubmitGuard;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ShareHandler {
    #[allow(dead_code)]
    tip_blue_score: Arc<Mutex<u64>>,
    stats: Arc<Mutex<HashMap<String, WorkStats>>>,
    overall: Arc<WorkStats>,
    instance_id: String, // Instance identifier for logging
    duplicate_submit_guard: Arc<Mutex<DuplicateSubmitGuard>>,
}

impl ShareHandler {
    pub fn log_prefix(&self) -> String {
        format!("[{}]", self.instance_id)
    }
}

#[cfg(test)]
impl ShareHandler {
    pub(crate) fn test_stats_len(&self) -> usize {
        self.stats.lock().len()
    }

    pub(crate) fn test_clear_stats(&self) {
        self.stats.lock().clear();
    }
}
