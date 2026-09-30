#[cfg(feature = "rkstratum_cpu_miner")]
use crate::rkstratum_cpu_miner::InternalMinerMetrics;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

#[derive(Clone)]
pub struct WorkStats {
    pub blocks_found: Arc<Mutex<i64>>,
    pub shares_found: Arc<Mutex<i64>>,
    pub shares_diff: Arc<Mutex<f64>>,
    pub stale_shares: Arc<Mutex<i64>>,
    pub invalid_shares: Arc<Mutex<i64>>,
    pub worker_name: Arc<Mutex<String>>,
    pub start_time: Instant,
    pub last_share: Arc<Mutex<Instant>>,
    pub var_diff_start_time: Arc<Mutex<Option<Instant>>>,
    pub var_diff_shares_found: Arc<Mutex<i64>>,
    pub var_diff_window: Arc<Mutex<usize>>,
    pub min_diff: Arc<Mutex<f64>>,
    /// Per-worker VarDiff floor (0 = none). Marketplace seeds use this so diff
    /// cannot step below the rental's required band.
    pub diff_floor: Arc<Mutex<f64>>,
}

impl WorkStats {
    pub fn new(worker_name: String) -> Self {
        Self {
            blocks_found: Arc::new(Mutex::new(0)),
            shares_found: Arc::new(Mutex::new(0)),
            shares_diff: Arc::new(Mutex::new(0.0)),
            stale_shares: Arc::new(Mutex::new(0)),
            invalid_shares: Arc::new(Mutex::new(0)),
            worker_name: Arc::new(Mutex::new(worker_name)),
            start_time: Instant::now(),
            last_share: Arc::new(Mutex::new(Instant::now())),
            var_diff_start_time: Arc::new(Mutex::new(None)),
            var_diff_shares_found: Arc::new(Mutex::new(0)),
            var_diff_window: Arc::new(Mutex::new(0)),
            min_diff: Arc::new(Mutex::new(0.0)),
            diff_floor: Arc::new(Mutex::new(0.0)),
        }
    }
}

pub(crate) struct StatsPrinterEntry {
    pub instance_id: String,
    pub inst_short: String,
    pub target_spm: f64,
    pub start: Instant,
    pub stats: Arc<Mutex<HashMap<String, WorkStats>>>,
    pub overall: Arc<WorkStats>,
}

pub(crate) static STATS_PRINTER_REGISTRY: Lazy<Mutex<Vec<StatsPrinterEntry>>> =
    Lazy::new(|| Mutex::new(Vec::new()));
pub static STATS_PRINTER_STARTED: AtomicBool = AtomicBool::new(false);

/// Normalize instance id so `[Instance 1]` and `Instance 1` match.
pub(crate) fn normalize_instance_id(instance_id: &str) -> String {
    instance_id
        .chars()
        .filter(|c| *c != '[' && *c != ']')
        .collect::<String>()
        .trim()
        .to_string()
}

/// Separator between wallet and worker in [`worker_stats_key`] (unlikely in addresses/names).
pub(crate) const WORKER_STATS_KEY_SEP: char = '\u{1f}';

/// Map key for live `WorkStats`: same worker name on different wallets stays separate.
/// Pre-authorize (empty wallet) uses the worker name alone; after authorize it becomes
/// `{wallet}{SEP}{worker}` so site/terminal hashrates cannot double-count name collisions.
pub(crate) fn worker_stats_key(worker_name: &str, wallet: &str) -> String {
    let worker = worker_name.trim();
    let wallet = wallet.trim();
    if wallet.is_empty() {
        worker.to_string()
    } else {
        format!("{wallet}{WORKER_STATS_KEY_SEP}{worker}")
    }
}

/// Split a [`worker_stats_key`] into `(wallet, worker_name)`.
pub(crate) fn split_worker_stats_key(key: &str) -> (String, String) {
    if let Some((wallet, worker)) = key.split_once(WORKER_STATS_KEY_SEP) {
        (wallet.to_string(), worker.to_string())
    } else {
        (String::new(), key.to_string())
    }
}

/// Lookup key for `/api/stats` terminal hashrate overlay: `normalized_instance|worker|wallet`.
pub(crate) fn terminal_hashrate_key(instance_id: &str, worker_name: &str, wallet: &str) -> String {
    format!(
        "{}|{}|{}",
        normalize_instance_id(instance_id),
        worker_name.trim(),
        wallet.trim()
    )
}

/// Live terminal hashrates (GH/s), keyed by [`terminal_hashrate_key`].
/// Same formula as the CLI worker table: `shares_diff / session_elapsed`.
pub(crate) fn terminal_session_hashrates_ghs() -> HashMap<String, f64> {
    let mut out = HashMap::new();
    let registry = STATS_PRINTER_REGISTRY.lock();
    for entry in registry.iter() {
        let inst = normalize_instance_id(&entry.instance_id);
        let stats_map = entry.stats.lock();
        for (stats_key, v) in stats_map.iter() {
            let (wallet, worker_name) = split_worker_stats_key(stats_key);
            let elapsed = v.start_time.elapsed().as_secs_f64();
            let rate = if elapsed > 0.0 {
                *v.shares_diff.lock() / elapsed
            } else {
                0.0
            };
            if rate > 0.0 {
                out.insert(terminal_hashrate_key(&inst, &worker_name, &wallet), rate);
            }
        }
    }
    out
}

#[cfg(feature = "rkstratum_cpu_miner")]
pub static RKSTRATUM_CPU_MINER_METRICS: Lazy<
    parking_lot::Mutex<Option<Arc<InternalMinerMetrics>>>,
> = Lazy::new(|| parking_lot::Mutex::new(None));

#[cfg(feature = "rkstratum_cpu_miner")]
pub fn set_rkstratum_cpu_miner_metrics(metrics: Arc<InternalMinerMetrics>) {
    *RKSTRATUM_CPU_MINER_METRICS.lock() = Some(metrics);
}

pub(crate) fn format_hashrate(ghs: f64) -> String {
    if ghs < 1.0 {
        format!("{:.2}MH/s", ghs * 1000.0)
    } else if ghs < 1000.0 {
        format!("{:.2}GH/s", ghs)
    } else {
        format!("{:.2}TH/s", ghs / 1000.0)
    }
}
