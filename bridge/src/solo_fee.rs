//! Probabilistic hosting fee for the click-run solo Kas+ZKAS stratum.
//!
//! Kaspa / ZKas `get_block_template` accepts a single pay address. There is no
//! native 99/1 coinbase split, so ~`fee_percent` of job templates pay the
//! operator fee sinks and the rest pay the miner (true solo for most jobs).
//!
//! One roll applies to both chains on the same job so merge-mine templates stay
//! aligned. Receive-only: no seeds required in this process.
//!
//! **Product lock:** fee percent and ops wallets are compile-time constants.
//! YAML / env cannot disarm or retarget the fee.

use kaspa_addresses::Address;
use once_cell::sync::OnceCell;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::info;

/// Permanent hosting fee percent for this product (not user-configurable).
pub const HARDCODED_FEE_PERCENT: f64 = 1.0;

/// Ops Kaspa receive address (fee templates).
pub const HARDCODED_KAS_FEE_ADDRESS: &str =
    "kaspa:qq3tqr9f0z6t6zwcrjkk8krwwltazcl0s4gvelvakvqmj9essyq4kaksa3v0m";

/// Ops ZKas receive address (fee templates).
pub const HARDCODED_ZKAS_FEE_ADDRESS: &str =
    "zkas:py500hfy6rfyyxk8f5f0uq6qc796sny8kdnsx4vh8ehnhshkg32rusjsgmkjjnxxwp4lmxg983s8x8h";

static SOLO_FEE: OnceCell<SoloFeeConfig> = OnceCell::new();
static FEE_TEMPLATES: AtomicU64 = AtomicU64::new(0);
static MINER_BLOCKS: AtomicU64 = AtomicU64::new(0);
static FEE_BLOCKS: AtomicU64 = AtomicU64::new(0);

/// Hosting fee knobs. Deserialized from YAML for compatibility, but the product
/// always installs [`SoloFeeConfig::hardcoded`] and ignores YAML values.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SoloFeeConfig {
    /// Percent of templates that pay fee sinks (e.g. `1.0` ≈ 1 in 100).
    pub fee_percent: f64,
    /// Kaspa pay address for fee jobs (empty = never override KAS).
    pub kas_fee_address: String,
    /// ZKas pay address for fee jobs (empty = never override ZKAS).
    pub zkas_fee_address: String,
}

impl Default for SoloFeeConfig {
    fn default() -> Self {
        Self::hardcoded()
    }
}

/// Per-job fee decision (shared by KAS GBT + ZKAS template).
#[derive(Debug, Clone, Default)]
pub struct SoloFeeDecision {
    pub apply: bool,
    pub kas_pay: Option<String>,
    pub zkas_pay: Option<String>,
}

impl SoloFeeConfig {
    /// Compile-time locked 1% fee to ops wallets.
    pub fn hardcoded() -> Self {
        Self {
            fee_percent: HARDCODED_FEE_PERCENT,
            kas_fee_address: HARDCODED_KAS_FEE_ADDRESS.to_string(),
            zkas_fee_address: HARDCODED_ZKAS_FEE_ADDRESS.to_string(),
        }
    }

    /// Install the permanent product fee. Ignores any YAML-derived config.
    pub fn install_hardcoded() {
        let cfg = Self::hardcoded().validated();
        debug_assert!(
            cfg.is_armed(),
            "hardcoded solo fee must validate and arm"
        );
        Self::install(cfg);
    }

    pub fn install(self) {
        let ready = self.is_armed();
        let pct = self.fee_percent;
        let kas_set = !self.kas_fee_address.trim().is_empty();
        let zkas_set = !self.zkas_fee_address.trim().is_empty();
        let _ = SOLO_FEE.set(self);
        if ready {
            info!(
                "solo_fee: LOCKED fee_percent={pct}% kas_fee={kas_set} zkas_fee={zkas_set} \
                 (hardcoded; YAML/env cannot change)"
            );
        } else {
            // Should never happen for hardcoded addresses — log loudly.
            tracing::error!(
                "solo_fee: HARDCODED FEE FAILED TO ARM — check fee address constants"
            );
        }
    }

    pub fn is_armed(&self) -> bool {
        self.fee_percent > 0.0
            && (!self.kas_fee_address.trim().is_empty() || !self.zkas_fee_address.trim().is_empty())
    }

    /// Validate configured addresses; returns cleaned config (invalid sides cleared).
    pub fn validated(mut self) -> Self {
        let kas = self.kas_fee_address.trim();
        if !kas.is_empty() {
            if Address::try_from(kas).is_ok() {
                self.kas_fee_address = kas.to_string();
            } else {
                tracing::warn!("solo_fee: invalid kas_fee_address — KAS fee disabled");
                self.kas_fee_address.clear();
            }
        }
        let zkas = self.zkas_fee_address.trim();
        if !zkas.is_empty() {
            if crate::zkas_address::is_valid_mainnet_zkas_payout_address(zkas) {
                self.zkas_fee_address = zkas.to_string();
            } else {
                tracing::warn!("solo_fee: invalid zkas_fee_address — ZKAS fee disabled");
                self.zkas_fee_address.clear();
            }
        }
        self.fee_percent = self.fee_percent.clamp(0.0, 100.0);
        self
    }

    /// Roll once for this job. `apply` is true when fee sinks should be used.
    pub fn roll(&self) -> SoloFeeDecision {
        if !self.is_armed() {
            return SoloFeeDecision::default();
        }
        let roll: f64 = rand::thread_rng().gen_range(0.0..100.0);
        if roll >= self.fee_percent {
            return SoloFeeDecision::default();
        }
        let kas_pay = {
            let a = self.kas_fee_address.trim();
            if a.is_empty() {
                None
            } else {
                Some(a.to_string())
            }
        };
        let zkas_pay = {
            let a = self.zkas_fee_address.trim();
            if a.is_empty() {
                None
            } else {
                Some(a.to_string())
            }
        };
        if kas_pay.is_none() && zkas_pay.is_none() {
            return SoloFeeDecision::default();
        }
        SoloFeeDecision {
            apply: true,
            kas_pay,
            zkas_pay,
        }
    }
}

pub fn solo_fee_config() -> Option<&'static SoloFeeConfig> {
    SOLO_FEE.get()
}

/// Roll using the installed global config (or no-op if unset).
pub fn roll_solo_fee_job() -> SoloFeeDecision {
    let d = match solo_fee_config() {
        Some(cfg) => cfg.roll(),
        None => SoloFeeDecision::default(),
    };
    if d.apply {
        FEE_TEMPLATES.fetch_add(1, Ordering::Relaxed);
    }
    d
}

/// Snapshot for terminal UI (no addresses).
#[derive(Debug, Clone, Copy)]
pub struct SoloFeeScoreboard {
    pub fee_templates: u64,
    pub miner_blocks: u64,
    pub fee_blocks: u64,
}

pub fn scoreboard() -> SoloFeeScoreboard {
    SoloFeeScoreboard {
        fee_templates: FEE_TEMPLATES.load(Ordering::Relaxed),
        miner_blocks: MINER_BLOCKS.load(Ordering::Relaxed),
        fee_blocks: FEE_BLOCKS.load(Ordering::Relaxed),
    }
}

/// Record an accepted Kaspa block (after node accept). Prints a clean scoreboard line.
pub fn record_accepted_block(is_fee_job: bool, worker: &str) {
    if is_fee_job {
        FEE_BLOCKS.fetch_add(1, Ordering::Relaxed);
        info!(
            "solo_fee: ★ HOSTING FEE BLOCK accepted — coinbase → OPS (KAS+ZKAS fee sinks) worker={worker}"
        );
    } else {
        MINER_BLOCKS.fetch_add(1, Ordering::Relaxed);
        info!("solo_fee: ● MINER BLOCK accepted — coinbase → MINER worker={worker}");
    }
    let s = scoreboard();
    let total = s.miner_blocks + s.fee_blocks;
    let pct = if total > 0 {
        (s.fee_blocks as f64) * 100.0 / (total as f64)
    } else {
        0.0
    };
    info!(
        "solo_fee: SCOREBOARD fee_blocks={} miner_blocks={} total_blocks={} fee_share={:.2}% fee_templates={}",
        s.fee_blocks, s.miner_blocks, total, pct, s.fee_templates
    );
}

/// Pure helper for tests: decide fee apply from a pre-drawn roll in `[0, 100)`.
pub fn fee_job_from_roll(fee_percent: f64, roll_0_100: f64) -> bool {
    fee_percent > 0.0 && roll_0_100 >= 0.0 && roll_0_100 < fee_percent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roll_boundary_one_percent() {
        assert!(fee_job_from_roll(1.0, 0.0));
        assert!(fee_job_from_roll(1.0, 0.999));
        assert!(!fee_job_from_roll(1.0, 1.0));
        assert!(!fee_job_from_roll(1.0, 50.0));
        assert!(!fee_job_from_roll(0.0, 0.0));
    }

    #[test]
    fn hardcoded_is_always_armed_after_validate() {
        let cfg = SoloFeeConfig::hardcoded().validated();
        assert_eq!(cfg.fee_percent, HARDCODED_FEE_PERCENT);
        assert_eq!(cfg.kas_fee_address, HARDCODED_KAS_FEE_ADDRESS);
        assert_eq!(cfg.zkas_fee_address, HARDCODED_ZKAS_FEE_ADDRESS);
        assert!(cfg.is_armed());
    }

    #[test]
    fn yaml_zero_percent_cannot_replace_hardcoded_shape() {
        // Product path never installs YAML; this documents the locked constants.
        let yaml_attempt = SoloFeeConfig {
            fee_percent: 0.0,
            kas_fee_address: String::new(),
            zkas_fee_address: String::new(),
        };
        assert!(!yaml_attempt.is_armed());
        let locked = SoloFeeConfig::hardcoded();
        assert!(locked.is_armed());
        assert_ne!(yaml_attempt.fee_percent, locked.fee_percent);
    }

    #[test]
    fn armed_requires_percent_and_address() {
        let mut cfg = SoloFeeConfig {
            fee_percent: 1.0,
            kas_fee_address: String::new(),
            zkas_fee_address: String::new(),
        };
        assert!(!cfg.is_armed());
        cfg.kas_fee_address = HARDCODED_KAS_FEE_ADDRESS.into();
        assert!(cfg.is_armed());
    }

    #[test]
    fn validated_clears_bad_kas() {
        let cfg = SoloFeeConfig {
            fee_percent: 1.0,
            kas_fee_address: "kaspa:not-a-real-address".into(),
            zkas_fee_address: String::new(),
        }
        .validated();
        assert!(cfg.kas_fee_address.is_empty());
    }
}
