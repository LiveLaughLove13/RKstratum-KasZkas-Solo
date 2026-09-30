//! Slim solo merge engine: per-miner ZKas pay address, AuxPoW submit only (no PPLNS credit).

use super::auxpow::encode_aux_pow_hex;
use super::commitment::append_zkmm_commitment;
use super::config::ZkasMergedConfig;
use super::pending::MergedPending;
use super::raw_rpc::ZkasRawRpc;
use anyhow::{Context, Result};
use kaspa_consensus_core::block::Block;
use kaspa_consensus_core::hashing;
use kaspa_hashes::Hash;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

static ENGINE: OnceCell<Arc<ZkasMergedEngine>> = OnceCell::new();

/// Reuse one ZKas template per pay-address across concurrent job dispatches.
const TEMPLATE_CACHE_TTL: Duration = Duration::from_millis(1_500);
const TEMPLATE_WARN_INTERVAL: Duration = Duration::from_secs(30);

pub fn zkas_merged_engine() -> Option<Arc<ZkasMergedEngine>> {
    ENGINE.get().cloned()
}

/// Start the engine when config is ready. No-op / Ok(None) when disabled.
pub async fn init_zkas_merged(
    cfg: ZkasMergedConfig,
    base_coinbase_tag: Vec<u8>,
) -> Result<Option<Arc<ZkasMergedEngine>>> {
    if !cfg.enabled {
        info!("zkas_merged: disabled (Kaspa-only mode)");
        return Ok(None);
    }
    if !cfg.is_ready() {
        warn!("zkas_merged.enabled=true but zkas_rpc missing — merge mining not started");
        return Ok(None);
    }

    let engine = ZkasMergedEngine::connect(cfg, base_coinbase_tag).await?;
    let engine = Arc::new(engine);
    let _ = ENGINE.set(Arc::clone(&engine));
    info!(
        "zkas_merged: connected to {} (dry_run={}, fallback={})",
        engine.cfg.zkas_rpc,
        engine.cfg.dry_run,
        if engine.cfg.fallback_zkas_address.trim().is_empty() {
            "(none)"
        } else {
            engine.cfg.fallback_zkas_address.trim()
        }
    );
    Ok(Some(engine))
}

struct CachedTemplate {
    at: Instant,
    h_fc: Hash,
    block: Block,
    extra: Vec<u8>,
}

pub struct ZkasMergedEngine {
    cfg: ZkasMergedConfig,
    zkas: Arc<ZkasRawRpc>,
    pending: Mutex<MergedPending>,
    enabled_instances: Mutex<HashSet<String>>,
    next_extra: Mutex<Option<Vec<u8>>>,
    /// Singleflight + short TTL per pay-address string.
    template_fetch: tokio::sync::Mutex<()>,
    template_cache: Mutex<HashMap<String, CachedTemplate>>,
    last_template_warn: Mutex<Option<Instant>>,
    base_coinbase_tag: Vec<u8>,
}

/// Finder identity logged after AuxPoW (no ledger credit in solo).
#[derive(Clone, Debug, Default)]
pub struct MergeFinder {
    pub kaspa_addr: String,
    pub zkas_addr: String,
    pub worker: String,
}

impl ZkasMergedEngine {
    pub fn rpc(&self) -> &Arc<ZkasRawRpc> {
        &self.zkas
    }

    async fn connect(cfg: ZkasMergedConfig, base_coinbase_tag: Vec<u8>) -> Result<Self> {
        let zkas = ZkasRawRpc::connect(&cfg.zkas_rpc)
            .await
            .with_context(|| format!("zkas_merged: connect to ZKas node at {}", cfg.zkas_rpc))?;

        Ok(Self {
            pending: Mutex::new(MergedPending::new(cfg.pending_cap)),
            cfg,
            zkas,
            enabled_instances: Mutex::new(HashSet::new()),
            next_extra: Mutex::new(None),
            template_fetch: tokio::sync::Mutex::new(()),
            template_cache: Mutex::new(HashMap::new()),
            last_template_warn: Mutex::new(None),
            base_coinbase_tag,
        })
    }

    pub fn config(&self) -> &ZkasMergedConfig {
        &self.cfg
    }

    pub fn enable_instance(&self, instance_id: &str) {
        self.enabled_instances
            .lock()
            .insert(instance_id.to_string());
        info!("zkas_merged: instance {instance_id} opted in");
    }

    pub fn instance_wants_merge(&self, instance_id: &str) -> bool {
        let set = self.enabled_instances.lock();
        set.is_empty() || set.contains(instance_id)
    }

    /// Resolve pay address: miner `zkas:` if valid, else operator fallback.
    pub fn resolve_pay_address(&self, miner_zkas: Option<&str>) -> Option<String> {
        if let Some(a) = miner_zkas.map(str::trim).filter(|s| !s.is_empty())
            && crate::zkas_address::is_valid_mainnet_zkas_payout_address(a)
        {
            return Some(a.to_string());
        }
        let fb = self.cfg.fallback_zkas_address.trim();
        if fb.is_empty() {
            return None;
        }
        if fb.starts_with("zkastest:") {
            if crate::zkas_address::orchard_script_bytes_from_zkas_address(fb).is_some() {
                return Some(fb.to_string());
            }
            return None;
        }
        if crate::zkas_address::is_valid_mainnet_zkas_payout_address(fb) {
            return Some(fb.to_string());
        }
        None
    }

    /// Stage ZKMM coinbase extra for the next `get_block_template` on this task.
    ///
    /// `pay_address` is the miner `zkas:` when set; falls back to config.
    /// Returns `None` (Kaspa-only job) when neither is usable.
    pub async fn prepare_commitment_for_client(
        &self,
        instance_id: &str,
        pay_address: Option<&str>,
    ) -> Option<Vec<u8>> {
        if !self.instance_wants_merge(instance_id) {
            *self.next_extra.lock() = None;
            return None;
        }

        let Some(pay) = self.resolve_pay_address(pay_address) else {
            debug!(
                "zkas_merged: no miner zkas: and no fallback — Kaspa-only job (instance {instance_id})"
            );
            *self.next_extra.lock() = None;
            return None;
        };

        match self.fetch_zkas_template_coalesced(&pay).await {
            Ok((h_fc, block, extra)) => {
                self.pending.lock().insert(h_fc, block);
                *self.next_extra.lock() = Some(extra.clone());
                debug!("zkas_merged: staged H_fc={h_fc} pay={pay} for instance {instance_id}");
                Some(extra)
            }
            Err(e) => {
                self.warn_template_failure(&e);
                *self.next_extra.lock() = None;
                None
            }
        }
    }

    pub fn take_staged_extra(&self) -> Option<Vec<u8>> {
        self.next_extra.lock().take()
    }

    fn warn_template_failure(&self, e: &anyhow::Error) {
        let mut last = self.last_template_warn.lock();
        let now = Instant::now();
        let should = last
            .map(|t| now.duration_since(t) >= TEMPLATE_WARN_INTERVAL)
            .unwrap_or(true);
        if should {
            warn!("zkas_merged: ZKas template failed ({e:#}) — Kaspa job without merge");
            *last = Some(now);
        } else {
            debug!("zkas_merged: ZKas template failed ({e:#}) — Kaspa job without merge");
        }
    }

    async fn fetch_zkas_template_coalesced(
        &self,
        pay_address: &str,
    ) -> Result<(Hash, Block, Vec<u8>)> {
        if let Some(cached) = self.template_cache.lock().get(pay_address)
            && cached.at.elapsed() < TEMPLATE_CACHE_TTL
        {
            return Ok((cached.h_fc, cached.block.clone(), cached.extra.clone()));
        }

        let _fetch_guard = self.template_fetch.lock().await;

        if let Some(cached) = self.template_cache.lock().get(pay_address)
            && cached.at.elapsed() < TEMPLATE_CACHE_TTL
        {
            return Ok((cached.h_fc, cached.block.clone(), cached.extra.clone()));
        }

        let (h_fc, block) = self.fetch_zkas_template(pay_address).await?;
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(h_fc.as_ref());
        let Some(extra) = append_zkmm_commitment(&self.base_coinbase_tag, &bytes) else {
            return Err(anyhow::anyhow!(
                "base coinbase tag already contains ZKMM — skipping"
            ));
        };

        self.template_cache.lock().insert(
            pay_address.to_string(),
            CachedTemplate {
                at: Instant::now(),
                h_fc,
                block: block.clone(),
                extra: extra.clone(),
            },
        );
        Ok((h_fc, block, extra))
    }

    async fn fetch_zkas_template(&self, pay_address: &str) -> Result<(Hash, Block)> {
        if !pay_address.starts_with("zkas:") && !pay_address.starts_with("zkastest:") {
            return Err(anyhow::anyhow!(
                "pay_address must be a zkas: (or zkastest:) address, got {pay_address}"
            ));
        }

        let block = self
            .zkas
            .get_block_template(pay_address)
            .await
            .context("ZKas get_block_template")?;
        let h_fc = hashing::header::hash(&block.header);
        Ok((h_fc, block))
    }

    pub async fn on_kaspa_block_accepted(
        &self,
        parent: &Block,
        instance_id: &str,
        finder: &MergeFinder,
    ) {
        if !self.instance_wants_merge(instance_id) {
            return;
        }

        let Some(h_fc) = extract_h_fc_from_parent(parent) else {
            debug!("zkas_merged: accepted Kaspa block has no ZKMM H_fc commitment");
            return;
        };

        let Some(fc_block) = self.pending.lock().get(&h_fc) else {
            warn!(
                "zkas_merged: pending ZKas template missing for H_fc={h_fc} \
                 (parent committed this hash; template may have been evicted)"
            );
            return;
        };

        let aux_hex = match encode_aux_pow_hex(parent, &fc_block) {
            Ok(h) => h,
            Err(e) => {
                warn!("zkas_merged: AuxPoW encode failed for H_fc={h_fc}: {e:#}");
                return;
            }
        };

        if self.cfg.dry_run {
            let zkas_tag = crate::solo_zkas_finds::zkas_addr_log_tag(&finder.zkas_addr);
            info!(
                "zkas_merged[dry_run]: would AuxPoW-submit ZKas H_fc={h_fc} \
                 aux_pow_hex_len={} instance={instance_id} {zkas_tag} (addresses redacted)",
                aux_hex.len(),
            );
            return;
        }

        match self
            .zkas
            .submit_block_with_aux_pow(&fc_block, &aux_hex)
            .await
        {
            Ok(()) => {
                let zkas_tag = crate::solo_zkas_finds::zkas_addr_log_tag(&finder.zkas_addr);
                let kas_tag = {
                    use blake2::{Blake2b512, Digest};
                    let mut h = Blake2b512::new();
                    h.update(finder.kaspa_addr.trim().to_lowercase().as_bytes());
                    format!("kas#{}", hex::encode(&h.finalize()[..5]))
                };
                info!(
                    "zkas_merged: AuxPoW ZKas block accepted H_fc={h_fc} instance={instance_id} \
                     {kas_tag} {zkas_tag} (addresses redacted)"
                );
                // Persist ZKAS find only — never store kaspa:/worker for unlinkability.
                crate::solo_zkas_finds::record_find(&finder.zkas_addr, &h_fc.to_string());
            }
            Err(e) => {
                warn!(
                    "zkas_merged: AuxPoW submit failed H_fc={h_fc} parent={}: {e:#}",
                    parent.header.hash
                );
            }
        }
    }
}

fn extract_h_fc_from_parent(parent: &Block) -> Option<Hash> {
    use super::commitment::{COMMITMENT_HEX_LEN, MERGE_MINE_MAGIC};
    let payload = parent.transactions.first()?.payload.as_slice();
    let mut i = 0usize;
    while i + MERGE_MINE_MAGIC.len() + COMMITMENT_HEX_LEN <= payload.len() {
        if payload[i..i + MERGE_MINE_MAGIC.len()] == MERGE_MINE_MAGIC {
            let hex = &payload
                [i + MERGE_MINE_MAGIC.len()..i + MERGE_MINE_MAGIC.len() + COMMITMENT_HEX_LEN];
            let mut bytes = [0u8; 32];
            for (o, pair) in bytes.iter_mut().zip(hex.chunks_exact(2)) {
                *o = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
            }
            return Some(Hash::from_bytes(bytes));
        }
        i += 1;
    }
    None
}

fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}
