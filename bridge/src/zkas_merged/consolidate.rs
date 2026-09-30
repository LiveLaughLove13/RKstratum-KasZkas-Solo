//! Background auto-consolidate via embedded wallet-api HTTP (never on the mining path).

use super::config::ZkasMergedConfig;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Spawn the consolidate loop when config is ready. No-op otherwise.
pub fn start_consolidate_loop(cfg: ZkasMergedConfig) {
    if !cfg.consolidate_ready() {
        if cfg.consolidate_enabled {
            warn!(
                "zkas_merged: consolidate_enabled but not ready \
                 (need seed_path file, consolidate_address, threshold>0)"
            );
        }
        return;
    }

    let interval = Duration::from_millis(cfg.consolidate_interval_ms.max(5_000));
    let busy = Arc::new(AtomicBool::new(false));
    info!(
        "zkas_merged: auto-consolidate loop started interval={}s threshold={} wallet_api={} \
         max_spends_per_tx={} (wallet-api applies its own mass cap)",
        interval.as_secs(),
        cfg.consolidate_note_threshold,
        cfg.wallet_api,
        cfg.max_spends_per_tx
    );

    tokio::spawn(async move {
        // Import once before the tick loop (blocking HTTP off the runtime).
        let cfg_import = cfg.clone();
        match tokio::task::spawn_blocking(move || import_operator_wallet(&cfg_import)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                warn!("zkas_merged: wallet import failed — consolidate loop idle: {e:#}");
                return;
            }
            Err(e) => {
                warn!("zkas_merged: wallet import task failed: {e}");
                return;
            }
        }

        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if busy
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                debug!("zkas_merged: consolidate tick skipped (fold in progress)");
                continue;
            }
            let cfg_tick = cfg.clone();
            let busy_flag = Arc::clone(&busy);
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || consolidate_tick(&cfg_tick)).await;
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => warn!("zkas_merged: consolidate tick error: {e:#}"),
                    Err(e) => warn!("zkas_merged: consolidate tick join error: {e}"),
                }
                busy_flag.store(false, Ordering::SeqCst);
            });
        }
    });
}

fn wallet_base_url(cfg: &ZkasMergedConfig) -> String {
    let raw = cfg.wallet_api.trim();
    if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.trim_end_matches('/').to_string()
    } else {
        format!("http://{}", raw.trim_end_matches('/'))
    }
}

fn read_seed_hex(path: &str) -> anyhow::Result<String> {
    let s =
        std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("read seed_path {path}: {e}"))?;
    let hex: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!(
            "seed_path must be 64 hex chars (32 bytes), got len={}",
            hex.len()
        );
    }
    Ok(hex)
}

fn import_operator_wallet(cfg: &ZkasMergedConfig) -> anyhow::Result<()> {
    let base = wallet_base_url(cfg);
    let seed_hex = read_seed_hex(cfg.seed_path.trim())?;
    let expect = cfg.consolidate_address.trim();

    let body = ureq::json!({ "seed_hex": seed_hex });
    let resp = ureq::post(&format!("{base}/api/wallet/import"))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| anyhow::anyhow!("POST /api/wallet/import: {e}"))?;

    let status = resp.status();
    let parsed: serde_json::Value = resp
        .into_json()
        .map_err(|e| anyhow::anyhow!("import response json: {e}"))?;
    if !(200..300).contains(&status) {
        anyhow::bail!("import HTTP {status}: {parsed}");
    }

    let got = parsed
        .get("address")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if got != expect {
        anyhow::bail!("imported address {got:?} does not match consolidate_address {expect:?}");
    }
    info!("zkas_merged: operator wallet imported address={got}");
    Ok(())
}

fn tip_note_count(cfg: &ZkasMergedConfig) -> anyhow::Result<usize> {
    let base = wallet_base_url(cfg);
    let resp = ureq::get(&format!("{base}/api/wallet/balance"))
        .call()
        .map_err(|e| anyhow::anyhow!("GET /api/wallet/balance: {e}"))?;
    let status = resp.status();
    let parsed: serde_json::Value = resp
        .into_json()
        .map_err(|e| anyhow::anyhow!("balance response json: {e}"))?;
    if !(200..300).contains(&status) {
        anyhow::bail!("balance HTTP {status}: {parsed}");
    }
    let notes = parsed
        .get("notes")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    Ok(notes)
}

fn post_consolidate(cfg: &ZkasMergedConfig) -> anyhow::Result<()> {
    let base = wallet_base_url(cfg);
    let body = ureq::json!({ "fee": cfg.consolidate_fee_sompi });
    let resp = ureq::post(&format!("{base}/api/wallet/consolidate"))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| anyhow::anyhow!("POST /api/wallet/consolidate: {e}"))?;
    let status = resp.status();
    let parsed: serde_json::Value = resp
        .into_json()
        .map_err(|e| anyhow::anyhow!("consolidate response json: {e}"))?;
    if !(200..300).contains(&status) {
        // 409 conflict = nothing to fold / still syncing — not fatal.
        if status == 409 {
            debug!("zkas_merged: consolidate skipped: {parsed}");
            return Ok(());
        }
        anyhow::bail!("consolidate HTTP {status}: {parsed}");
    }
    info!(
        "zkas_merged: consolidate ok txid={} consolidated={} notes_remaining={}",
        parsed.get("txid").and_then(|v| v.as_str()).unwrap_or("?"),
        parsed
            .get("consolidated")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        parsed
            .get("notes_remaining")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    );
    Ok(())
}

fn consolidate_tick(cfg: &ZkasMergedConfig) -> anyhow::Result<()> {
    let notes = tip_note_count(cfg)?;
    let thresh = cfg.consolidate_note_threshold as usize;
    if notes <= thresh {
        debug!("zkas_merged: tip notes={notes} ≤ threshold={thresh} — no fold");
        return Ok(());
    }
    info!("zkas_merged: tip notes={notes} > threshold={thresh} — consolidating");
    post_consolidate(cfg)
}
