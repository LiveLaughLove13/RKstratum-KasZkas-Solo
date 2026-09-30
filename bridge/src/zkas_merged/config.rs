use serde::{Deserialize, Serialize};
use std::path::Path;

/// Global ZKas AuxPoW merge-mining settings for the solo bridge (default: disabled).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ZkasMergedConfig {
    /// Master switch. When `false`, no in-process ZKas node and no coinbase ZKMM tags.
    pub enabled: bool,
    /// When `true`, assemble AuxPoW and log only — do not submit to ZKas.
    pub dry_run: bool,
    /// In-process ZKas gRPC listen (`host:port`) for templates / AuxPoW submit.
    pub zkas_rpc: String,
    /// In-process ZKas P2P listen (`host:port`).
    pub zkas_p2p: String,
    /// Data directory for the embedded ZKas node. Empty → beside Kaspa appdir default.
    pub zkas_appdir: String,
    /// Embedded wallet-api listen (`host:port`) for auto-consolidate.
    pub wallet_api: String,
    /// Used when a miner has no valid `zkas:` password address.
    pub fallback_zkas_address: String,
    /// Max pending ZKas templates keyed by `H_fc` (FIFO eviction).
    pub pending_cap: usize,
    /// Background fold of operator wallet notes via wallet-api.
    pub consolidate_enabled: bool,
    /// When tip note count exceeds this, POST `/api/wallet/consolidate` (0 = off).
    pub consolidate_note_threshold: u32,
    /// How often the consolidate loop ticks.
    pub consolidate_interval_ms: u64,
    /// Base fee (sompi) for consolidate; wallet-api may raise for mass.
    pub consolidate_fee_sompi: u64,
    /// Documented spend cap (wallet-api uses its own mass-derived max).
    pub max_spends_per_tx: u32,
    /// Path to 32-byte seed hex file (64 chars) for the operator wallet.
    pub seed_path: String,
    /// Must match the address derived from `seed_path` after import.
    pub consolidate_address: String,
}

impl Default for ZkasMergedConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dry_run: true,
            zkas_rpc: "127.0.0.1:16810".to_string(),
            zkas_p2p: "127.0.0.1:16811".to_string(),
            zkas_appdir: String::new(),
            wallet_api: "127.0.0.1:18501".to_string(),
            fallback_zkas_address: String::new(),
            pending_cap: 64,
            consolidate_enabled: false,
            consolidate_note_threshold: 40,
            consolidate_interval_ms: 600_000,
            consolidate_fee_sompi: 10_000_000,
            max_spends_per_tx: 20,
            seed_path: String::new(),
            consolidate_address: String::new(),
        }
    }
}

impl ZkasMergedConfig {
    /// Merge mining can start when enabled and RPC listen is configured.
    /// Per-miner or fallback `zkas:` address is resolved at template time.
    pub fn is_ready(&self) -> bool {
        self.enabled && !self.zkas_rpc.trim().is_empty()
    }

    /// Auto-consolidate needs merge ready, consolidate on, seed file, and address.
    pub fn consolidate_ready(&self) -> bool {
        if !self.is_ready() || !self.consolidate_enabled {
            return false;
        }
        if self.consolidate_note_threshold == 0 {
            return false;
        }
        let seed = self.seed_path.trim();
        let addr = self.consolidate_address.trim();
        if seed.is_empty() || addr.is_empty() {
            return false;
        }
        if !addr.starts_with("zkas:") && !addr.starts_with("zkastest:") {
            return false;
        }
        Path::new(seed).is_file()
    }

    pub fn resolved_zkas_appdir(&self) -> std::path::PathBuf {
        let trimmed = self.zkas_appdir.trim();
        if !trimmed.is_empty() {
            return std::path::PathBuf::from(trimmed);
        }
        crate::app_dirs::default_inprocess_zkas_kaspad_appdir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_disabled_dry_run() {
        let cfg = ZkasMergedConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.dry_run);
        assert!(!cfg.is_ready());
        assert!(!cfg.consolidate_ready());
    }

    #[test]
    fn ready_when_enabled_with_rpc() {
        let cfg = ZkasMergedConfig {
            enabled: true,
            ..Default::default()
        };
        assert!(cfg.is_ready());
    }

    #[test]
    fn yaml_parses_solo_fields() {
        let cfg: ZkasMergedConfig = serde_yaml::from_str(
            r#"
enabled: true
dry_run: true
zkas_rpc: "127.0.0.1:16810"
fallback_zkas_address: "zkas:abc"
consolidate_enabled: false
"#,
        )
        .expect("yaml");
        assert!(cfg.enabled);
        assert_eq!(cfg.fallback_zkas_address, "zkas:abc");
    }
}
