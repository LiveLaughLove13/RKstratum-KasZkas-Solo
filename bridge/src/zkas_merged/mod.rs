//! Opt-in Kaspa→ZKas AuxPoW merge-mining for the solo bridge (additive; default off).
//!
//! When disabled, this module is never started and the Kaspa solo path is unchanged.
//! See `bridge/docs/ZKAS-MERGED-MINING.md`.

mod auxpow;
mod commitment;
mod config;
mod consolidate;
mod engine;
mod pending;
mod raw_rpc;

pub use commitment::{MERGE_MINE_MAGIC, append_zkmm_commitment, embed_zkmm_commitment};
pub use config::ZkasMergedConfig;
pub use consolidate::start_consolidate_loop;
pub use engine::{MergeFinder, ZkasMergedEngine, init_zkas_merged, zkas_merged_engine};
pub use pending::MergedPending;
