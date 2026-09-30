//! Per-miner difficulty seeding / floors.
//!
//! Global `min_share_diff` remains the default seed for ASICs. NiceHash / MRR
//! proxies that identify as marketplace UAs often require a higher starting
//! difficulty (and must not VarDiff below that band). Other miners are unchanged.
//!
//! Optional HeroMiners-style worker override: `kaspa:ADDR.worker=8192` seeds and
//! floors VarDiff at the requested power-of-two difficulty (password still zkas).

/// Upper bound for `worker=DIFF` overrides (covers MRR high-end rental band).
pub const MAX_WORKER_REQUESTED_DIFF: f64 = 65536.0;

/// True when the stratum `mining.subscribe` app string is a NiceHash proxy.
pub fn is_nicehash_app(remote_app: &str) -> bool {
    remote_app.to_ascii_lowercase().contains("nicehash")
}

/// True for NiceHash / MiningRigRentals / LazyPickaxe marketplace proxies.
pub fn is_marketplace_app(remote_app: &str) -> bool {
    let a = remote_app.to_ascii_lowercase();
    a.contains("nicehash") || a.contains("miningrigrentals") || a.contains("lazypickaxe")
}

/// EthereumStratum-style clients that need an 80-char hex `mining.notify`.
/// IceRiver only — marketplace (NiceHash / LazyPickaxe / MRR) uses Kaspa-common
/// `[job_id, [u64;4], timestamp]` like 2Miners (`kas.2miners.com`).
pub fn uses_eth_stratum_hex_job(remote_app: &str) -> bool {
    let a = remote_app.to_ascii_lowercase();
    a.contains("iceriver") || a.contains("icemining") || a.contains("ethereumstratum")
}

/// Starting difficulty for a client: marketplace UAs use `max(base, nicehash_min)`
/// when `nicehash_min > 0`; everyone else keeps `base`.
pub fn start_diff_for_app(base_min: f64, nicehash_min: f64, remote_app: &str) -> f64 {
    if nicehash_min > 0.0 && is_marketplace_app(remote_app) {
        base_min.max(nicehash_min)
    } else {
        base_min
    }
}

/// VarDiff floor for a client (0 = no extra floor). Matches the marketplace seed
/// so high-end MRR/NH rentals are not stepped back into a "too low" band.
pub fn diff_floor_for_app(nicehash_min: f64, remote_app: &str) -> f64 {
    if nicehash_min > 0.0 && is_marketplace_app(remote_app) {
        nicehash_min
    } else {
        0.0
    }
}

/// Normalize a worker-requested difficulty: finite, ≥ 1, ≤ max, floored to pow2.
pub fn normalize_worker_requested_diff(value: f64) -> Option<f64> {
    if !value.is_finite() || value < 1.0 {
        return None;
    }
    let capped = value.min(MAX_WORKER_REQUESTED_DIFF);
    let floored = 2_f64.powi(capped.log2().floor() as i32);
    if floored < 1.0 { None } else { Some(floored) }
}

/// Parse optional `worker=DIFF` or `worker/d=DIFF` from the worker segment.
/// Returns `(worker_name_without_suffix, Some(normalized_diff))` when valid.
/// Invalid / non-numeric suffix leaves the worker string unchanged and returns `None`.
pub fn parse_worker_diff_override(worker: &str) -> (String, Option<f64>) {
    let worker = worker.trim();
    if worker.is_empty() {
        return (String::new(), None);
    }

    // Prefer `/d=DIGITS` (HeroMiners alternate), then trailing `=DIGITS`.
    if let Some(idx) = worker.rfind("/d=") {
        let name = worker[..idx].trim_end();
        let digits = &worker[idx + 3..];
        if let Some(diff) = parse_diff_digits(digits) {
            let name = if name.is_empty() {
                "worker".to_string()
            } else {
                name.to_string()
            };
            return (name, Some(diff));
        }
        return (worker.to_string(), None);
    }

    if let Some(idx) = worker.rfind('=') {
        let name = worker[..idx].trim_end();
        let digits = &worker[idx + 1..];
        if let Some(diff) = parse_diff_digits(digits) {
            let name = if name.is_empty() {
                "worker".to_string()
            } else {
                name.to_string()
            };
            return (name, Some(diff));
        }
        return (worker.to_string(), None);
    }

    (worker.to_string(), None)
}

fn parse_diff_digits(digits: &str) -> Option<f64> {
    let digits = digits.trim();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let value: f64 = digits.parse().ok()?;
    normalize_worker_requested_diff(value)
}

/// Apply worker-requested start+floor on top of base / marketplace policy.
/// `requested == 0` is a no-op.
pub fn apply_worker_requested_diff(min_diff: f64, diff_floor: f64, requested: f64) -> (f64, f64) {
    if requested > 0.0 {
        (min_diff.max(requested), diff_floor.max(requested))
    } else {
        (min_diff, diff_floor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_nicehash_ua() {
        assert!(is_nicehash_app("NiceHash/1.0.0"));
        assert!(is_nicehash_app("nicehash"));
        assert!(!is_nicehash_app("IceRiverMiner-v1.1"));
        assert!(!is_nicehash_app("GodMiner"));
        assert!(!is_nicehash_app(""));
        assert!(!is_nicehash_app("MiningRigRentals/Test/1.0"));
    }

    #[test]
    fn detects_marketplace_ua() {
        assert!(is_marketplace_app("NiceHash/1.0.0"));
        assert!(is_marketplace_app("MiningRigRentals/Test/1.0"));
        assert!(is_marketplace_app("miningrigrentals"));
        assert!(is_marketplace_app("lazypickaxe.com"));
        assert!(!is_marketplace_app("IceRiverMiner-v1.1"));
        assert!(!is_marketplace_app("GodMiner"));
    }

    #[test]
    fn eth_stratum_hex_job_uas() {
        assert!(uses_eth_stratum_hex_job("IceRiverMiner-v1.1"));
        assert!(uses_eth_stratum_hex_job("ethereumstratum"));
        // Marketplace uses Kaspa-common array jobs (2Miners), not IceRiver hex.
        assert!(!uses_eth_stratum_hex_job("NiceHash/1.0.0"));
        assert!(!uses_eth_stratum_hex_job("lazypickaxe.com"));
        assert!(!uses_eth_stratum_hex_job("MiningRigRentals/Test/1.0"));
        assert!(!uses_eth_stratum_hex_job("GodMiner"));
        assert!(!uses_eth_stratum_hex_job("BzMiner"));
        assert!(!uses_eth_stratum_hex_job(""));
    }

    #[test]
    fn nicehash_seed_raises_only_when_configured() {
        assert_eq!(start_diff_for_app(512.0, 8192.0, "NiceHash/1.0.0"), 8192.0);
        assert_eq!(
            start_diff_for_app(16384.0, 8192.0, "NiceHash/1.0.0"),
            16384.0
        );
        assert_eq!(start_diff_for_app(512.0, 0.0, "NiceHash/1.0.0"), 512.0);
        assert_eq!(
            start_diff_for_app(512.0, 8192.0, "IceRiverMiner-v1.1"),
            512.0
        );
        assert_eq!(
            start_diff_for_app(512.0, 8192.0, "MiningRigRentals/Test/1.0"),
            8192.0
        );
    }

    #[test]
    fn marketplace_floor_for_nh_and_mrr() {
        assert_eq!(diff_floor_for_app(8192.0, "NiceHash/1.0.0"), 8192.0);
        assert_eq!(
            diff_floor_for_app(8192.0, "MiningRigRentals/Test/1.0"),
            8192.0
        );
        assert_eq!(diff_floor_for_app(8192.0, "IceRiverMiner-v1.1"), 0.0);
        assert_eq!(diff_floor_for_app(0.0, "NiceHash/1.0.0"), 0.0);
    }

    #[test]
    fn parse_worker_equals_diff() {
        let (name, diff) = parse_worker_diff_override("rig=8192");
        assert_eq!(name, "rig");
        assert_eq!(diff, Some(8192.0));

        let (name, diff) = parse_worker_diff_override("asic-215=16384");
        assert_eq!(name, "asic-215");
        assert_eq!(diff, Some(16384.0));

        let (name, diff) = parse_worker_diff_override("rig=32768");
        assert_eq!(name, "rig");
        assert_eq!(diff, Some(32768.0));

        // MRR / LazyPickaxe style worker tag
        let (name, diff) = parse_worker_diff_override("RK=16384");
        assert_eq!(name, "RK");
        assert_eq!(diff, Some(16384.0));
    }

    #[test]
    fn parse_worker_slash_d_diff() {
        let (name, diff) = parse_worker_diff_override("rig/d=8192");
        assert_eq!(name, "rig");
        assert_eq!(diff, Some(8192.0));
    }

    #[test]
    fn parse_worker_no_override() {
        let (name, diff) = parse_worker_diff_override("rig");
        assert_eq!(name, "rig");
        assert_eq!(diff, None);

        let (name, diff) = parse_worker_diff_override("rig=abc");
        assert_eq!(name, "rig=abc");
        assert_eq!(diff, None);

        let (name, diff) = parse_worker_diff_override("rig=");
        assert_eq!(name, "rig=");
        assert_eq!(diff, None);
    }

    #[test]
    fn normalize_floors_to_pow2_and_caps() {
        assert_eq!(normalize_worker_requested_diff(9000.0), Some(8192.0));
        assert_eq!(normalize_worker_requested_diff(100_000.0), Some(65536.0));
        assert_eq!(normalize_worker_requested_diff(0.5), None);
        assert_eq!(normalize_worker_requested_diff(f64::NAN), None);
    }

    #[test]
    fn worker_requested_raises_seed_and_floor() {
        let (min, floor) = apply_worker_requested_diff(512.0, 0.0, 8192.0);
        assert_eq!(min, 8192.0);
        assert_eq!(floor, 8192.0);

        // Worker override wins over lower marketplace floor.
        let (min, floor) = apply_worker_requested_diff(8192.0, 8192.0, 16384.0);
        assert_eq!(min, 16384.0);
        assert_eq!(floor, 16384.0);

        // Marketplace floor stays if request is lower.
        let (min, floor) = apply_worker_requested_diff(8192.0, 8192.0, 4096.0);
        assert_eq!(min, 8192.0);
        assert_eq!(floor, 8192.0);

        let (min, floor) = apply_worker_requested_diff(512.0, 0.0, 0.0);
        assert_eq!(min, 512.0);
        assert_eq!(floor, 0.0);
    }
}
