//! MiningPoolStats overlay for solo dual-external (listing only).
//!
//! When `mps_crawler.enabled`:
//! - **Default** `/api/stats` stays **site/dashboard-safe**: native `blocks`/`workers`
//!   arrays plus InsScan identity (`miningMode`, `soloFlavor`, `coins`, `fee`). A
//!   `blocksRecent` mirror is included; no nested MPS `stats`/`nodes`/`scheme`.
//! - **`?mps=1`**: full MiningPoolStats shape (`apiVersion`, H/s `hashrate`, object
//!   `blocks` map, `scheme: SOLO`, etc.).
//!
//! `/api/blocks` is also served. Does not change coinbase / solo_fee rolls.

use super::stats_json::{StatsResponse, get_stats_json, get_stats_json_all};
use crate::app_config::MpsCrawlerConfig;
use crate::solo_fee;
use once_cell::sync::OnceCell;
use serde::Serialize;
use std::collections::BTreeMap;

const GH_TO_HS: f64 = 1_000_000_000.0;
const MPS_BLOCK_MAP_LIMIT: usize = 100;

static MPS_CFG: OnceCell<MpsCrawlerConfig> = OnceCell::new();

pub fn install_mps_crawler(cfg: MpsCrawlerConfig) {
    let _ = MPS_CFG.set(cfg);
}

pub fn mps_crawler_ready() -> bool {
    MPS_CFG.get().is_some_and(|c| c.enabled)
}

fn listing() -> &'static MpsCrawlerConfig {
    MPS_CFG.get().expect("mps_crawler not installed")
}

fn hashrate_hs(stats: &StatsResponse) -> u64 {
    let total_gh: f64 = stats.workers.iter().map(|w| w.hashrate).sum();
    (total_gh * GH_TO_HS).round().max(0.0) as u64
}

fn fee_percent() -> f64 {
    solo_fee::solo_fee_config()
        .map(|c| c.fee_percent)
        .filter(|p| *p > 0.0)
        .unwrap_or(1.0)
}

fn ms_to_unix_sec(ts: i64) -> i64 {
    if ts > 1_000_000_000_000 {
        ts / 1000
    } else {
        ts
    }
}

fn blocks_map(stats: &StatsResponse) -> (BTreeMap<String, String>, Option<u64>, Option<i64>) {
    let mut map = BTreeMap::new();
    let mut best_h = 0u64;
    let mut best_t = 0i64;
    for b in stats.blocks.iter().take(MPS_BLOCK_MAP_LIMIT * 4) {
        let height = b.bluescore.trim().parse::<u64>().unwrap_or(0);
        if height == 0 || b.hash.trim().is_empty() {
            continue;
        }
        let time_sec = ms_to_unix_sec(b.timestamp.trim().parse::<i64>().unwrap_or(0));
        if time_sec <= 0 {
            continue;
        }
        map.entry(height.to_string())
            .or_insert_with(|| time_sec.to_string());
        if height >= best_h {
            best_h = height;
            best_t = time_sec;
        }
        if map.len() >= MPS_BLOCK_MAP_LIMIT {
            break;
        }
    }
    let lastblock = (best_h > 0).then_some(best_h);
    let lastblocktime = (best_t > 0).then_some(best_t);
    (map, lastblock, lastblocktime)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MpsNodeRow {
    name: String,
    height: String,
    last_beat: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MpsStatsInner {
    last_block_found: i64,
    lastblock: Option<u64>,
    lastblocktime: i64,
    blocks_nr: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TwoMinersBlockRow {
    height: u64,
    timestamp: i64,
    hash: String,
    orphan: bool,
    reward: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MpsBlocksResponse {
    api_version: u32,
    scheme: String,
    candidates_total: usize,
    candidates: Vec<TwoMinersBlockRow>,
    immature: Vec<TwoMinersBlockRow>,
    immature_total: u32,
    matured_total: u64,
    matured: Vec<TwoMinersBlockRow>,
    luck: u32,
    min_payout: f64,
    lastblock: Option<u64>,
    lastblocktime: i64,
    blocks_nr: u64,
    stats: MpsStatsInner,
}

fn two_miners_rows(stats: &StatsResponse, limit: usize) -> Vec<TwoMinersBlockRow> {
    stats
        .blocks
        .iter()
        .filter_map(|b| {
            let height = b.bluescore.trim().parse::<u64>().ok().filter(|&h| h > 0)?;
            let hash = b.hash.trim();
            if hash.is_empty() {
                return None;
            }
            let timestamp = ms_to_unix_sec(b.timestamp.trim().parse::<i64>().unwrap_or(0));
            if timestamp <= 0 {
                return None;
            }
            Some(TwoMinersBlockRow {
                height,
                timestamp,
                hash: hash.to_string(),
                orphan: false,
                reward: 0,
            })
        })
        .take(limit)
        .collect()
}

fn build_combined_stats_json(stats: &StatsResponse, mps_object_blocks: bool) -> String {
    let cfg = listing();
    let fee = fee_percent();

    let mut dash = serde_json::to_value(stats).unwrap_or_else(|_| serde_json::json!({}));
    let Some(d) = dash.as_object_mut() else {
        return serde_json::to_string(&dash).unwrap_or_else(|_| "{}".to_string());
    };

    // InsScan / rkstratum.site identity (always).
    d.insert("miningMode".into(), serde_json::json!("solo"));
    d.insert("soloFlavor".into(), serde_json::json!("multicoin"));
    d.insert("fee".into(), serde_json::json!(fee));
    d.insert("feePercent".into(), serde_json::json!(fee));
    if d.get("coins").and_then(|c| c.as_array()).is_none() {
        d.insert(
            "coins".into(),
            serde_json::json!(["KAS".to_string(), "ZKAS".to_string()]),
        );
    }
    if let Some(arr) = d.get("blocks").cloned() {
        d.insert("blocksRecent".to_string(), arr);
    }
    if !cfg.website.is_empty() {
        d.insert("website".into(), serde_json::json!(cfg.website));
    }
    if !cfg.stratum_url.is_empty() {
        d.insert("url".into(), serde_json::json!(cfg.stratum_url));
    }
    if !cfg.pool_name.is_empty() {
        d.insert("pool".into(), serde_json::json!(cfg.pool_name));
    }

    // Full MPS crawler shape only when explicitly requested (?mps=1).
    if mps_object_blocks {
        let (blocks_by_height, lastblock, lastblocktime) = blocks_map(stats);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let now_sec = now_ms / 1000;
        let last_found = lastblocktime.unwrap_or(now_sec);
        let blocks_nr = blocks_by_height.len() as u64;
        let workers = stats.activeWorkers.max(stats.workers.len());
        let tip = lastblock
            .map(|h| h.to_string())
            .or_else(|| (stats.networkBlockCount > 0).then(|| stats.networkBlockCount.to_string()));

        d.insert(
            "blocksByHeight".into(),
            serde_json::json!(blocks_by_height.clone()),
        );
        d.insert("blocks".into(), serde_json::json!(blocks_by_height));
        d.insert("apiVersion".into(), serde_json::json!(200));
        d.insert("hashrate".into(), serde_json::json!(hashrate_hs(stats)));
        d.insert("minersTotal".into(), serde_json::json!(workers));
        d.insert("workersTotal".into(), serde_json::json!(workers));
        d.insert("minPayout".into(), serde_json::json!(0.0));
        d.insert("minpay".into(), serde_json::json!(0.0));
        d.insert("paymentThreshold".into(), serde_json::json!(0.0));
        d.insert("luck".into(), serde_json::json!(100));
        d.insert(
            "maturedTotal".into(),
            serde_json::json!(stats.totalBlocksAcceptedByNode.max(stats.totalBlocks)),
        );
        d.insert("now".into(), serde_json::json!(now_ms));
        d.insert("lastblock".into(), serde_json::json!(lastblock));
        d.insert("lastblocktime".into(), serde_json::json!(last_found));
        d.insert("lastBlockFound".into(), serde_json::json!(last_found));
        d.insert("blocksNr".into(), serde_json::json!(blocks_nr));
        d.insert("blocksCount".into(), serde_json::json!(blocks_nr));
        d.insert("scheme".into(), serde_json::json!("SOLO"));
        if let Some(name) = tip.clone() {
            d.insert("blockHeight".into(), serde_json::json!(name.clone()));
            d.insert("height".into(), serde_json::json!(name));
        }
        d.insert(
            "stats".into(),
            serde_json::json!({
                "lastBlockFound": last_found,
                "lastblock": lastblock,
                "lastblocktime": last_found,
                "blocksNr": blocks_nr,
            }),
        );
        let node_name = if cfg.pool_name.trim().is_empty() {
            "RKStratum Solo Multicoin".to_string()
        } else {
            cfg.pool_name.clone()
        };
        if let Some(h) = tip {
            d.insert(
                "nodes".into(),
                serde_json::json!([MpsNodeRow {
                    name: node_name,
                    height: h,
                    last_beat: now_sec.to_string(),
                }]),
            );
        }
    }

    serde_json::to_string(&dash).unwrap_or_else(|_| "{}".to_string())
}

fn build_blocks_json(stats: &StatsResponse) -> String {
    let rows = two_miners_rows(stats, 200);
    let (map, lastblock, lastblocktime) = blocks_map(stats);
    let now_sec = chrono::Utc::now().timestamp();
    let last_found = lastblocktime.unwrap_or(now_sec);
    let blocks_nr = map.len() as u64;
    let body = MpsBlocksResponse {
        api_version: 200,
        scheme: "SOLO".to_string(),
        candidates_total: rows.len().min(20),
        candidates: rows.iter().take(20).cloned().collect(),
        immature: rows.iter().take(50).cloned().collect(),
        immature_total: rows.len().min(50) as u32,
        matured_total: stats.totalBlocksAcceptedByNode.max(stats.totalBlocks),
        matured: rows,
        luck: 0,
        min_payout: 0.0,
        lastblock,
        lastblocktime: last_found,
        blocks_nr,
        stats: MpsStatsInner {
            last_block_found: last_found,
            lastblock,
            lastblocktime: last_found,
            blocks_nr,
        },
    };
    serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_string())
}

pub async fn get_solo_mps_stats_json(instance_id: Option<&str>, mps_object_blocks: bool) -> String {
    let stats = match instance_id {
        Some(id) => get_stats_json(id).await,
        None => get_stats_json_all().await,
    };
    build_combined_stats_json(&stats, mps_object_blocks)
}

pub async fn get_solo_mps_blocks_json(instance_id: Option<&str>) -> String {
    let stats = match instance_id {
        Some(id) => get_stats_json(id).await,
        None => get_stats_json_all().await,
    };
    build_blocks_json(&stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_config::MpsCrawlerConfig;

    fn sample_stats() -> StatsResponse {
        StatsResponse {
            totalBlocks: 2,
            totalBlocksAcceptedByNode: 2,
            totalBlocksNotConfirmedBlue: 0,
            totalShares: 10,
            totalHashrate: 0.0,
            totalWorkers: 0,
            networkHashrate: 0,
            networkDifficulty: 0.0,
            networkBlockCount: 100,
            activeWorkers: 1,
            internalCpu: None,
            blocks: vec![super::super::stats_json::BlockInfo {
                instance: String::new(),
                worker: "w".into(),
                wallet: String::new(),
                timestamp: "1780679039".into(),
                hash: "abcd".into(),
                nonce: String::new(),
                bluescore: "522156794".into(),
            }],
            workers: vec![],
            bridgeUptime: None,
            miningMode: Some("solo".into()),
            soloFlavor: Some("multicoin".into()),
            coins: None,
            fee: Some(1.0),
            fee_percent: Some(1.0),
        }
    }

    #[test]
    fn solo_mps_overlay_default_keeps_blocks_array() {
        install_mps_crawler(MpsCrawlerConfig {
            enabled: true,
            pool_name: "Test Solo".into(),
            website: "https://example.com".into(),
            stratum_url: "stratum+tcp://example.com:6666".into(),
        });
        let json = build_combined_stats_json(&sample_stats(), false);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["miningMode"], "solo");
        assert_eq!(v["soloFlavor"], "multicoin");
        assert!(v["blocks"].is_array());
        assert!(v["blocksRecent"].is_array());
        assert!(v.get("workers").is_some());
        // MPS-only fields stay off the default (site-safe) path.
        assert!(v.get("apiVersion").is_none());
        assert!(v.get("scheme").is_none());
        assert!(v.get("blocksByHeight").is_none());
        assert!(v.get("nodes").is_none());
    }

    #[test]
    fn solo_mps_overlay_mps_query_uses_object_blocks() {
        install_mps_crawler(MpsCrawlerConfig {
            enabled: true,
            pool_name: "Test Solo".into(),
            website: "https://example.com".into(),
            stratum_url: "stratum+tcp://example.com:6666".into(),
        });
        let json = build_combined_stats_json(&sample_stats(), true);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["apiVersion"], 200);
        assert_eq!(v["scheme"], "SOLO");
        assert!(v["blocks"].is_object());
        assert!(v["blocksRecent"].is_array());
        assert!(v["blocksByHeight"].is_object());
    }
}
