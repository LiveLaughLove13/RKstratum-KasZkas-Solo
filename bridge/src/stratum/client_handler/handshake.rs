//! Extranonce assignment after miner type is detected (`mining.subscribe`).

use crate::stratum_context::StratumContext;
use std::sync::atomic::{AtomicI32, Ordering};
use tracing::{debug, info, warn};

static GLOBAL_NEXT_EXTRANONCE: AtomicI32 = AtomicI32::new(0);

/// Assign extranonce to a client based on detected miner type.
/// Called from `handle_subscribe` after miner type is detected.
///
/// Kaspa nonce is 8 bytes: `en1 || en2`. Extranonce2 size = `8 - len(en1)`.
///
/// Marketplace ASICs (MRR / NiceHash / LazyPickaxe) reject `en2 < 7`, which means
/// `en1` must be at most **1 byte**. IceRiver is fine with the historical 2-byte `en1`
/// (`en2 = 6`).
pub fn assign_extranonce_for_miner(ctx: &StratumContext, remote_app: &str) {
    let remote_app_lower = remote_app.to_lowercase();
    let is_bitmain = remote_app_lower.contains("godminer")
        || remote_app_lower.contains("bitmain")
        || remote_app_lower.contains("antminer");
    let is_marketplace = crate::share_handler::is_marketplace_app(remote_app);

    let required_extranonce_size: i32 = if is_bitmain {
        0
    } else if is_marketplace {
        1 // => extranonce2_size = 7 (ASIC / LazyPickaxe minimum)
    } else {
        2 // IceRiver / BzMiner / Goldshell => en2 = 6
    };

    let extranonce = if required_extranonce_size > 0 {
        let max_extranonce = (1i32 << (8 * required_extranonce_size)) - 1;
        let raw = GLOBAL_NEXT_EXTRANONCE.fetch_add(1, Ordering::SeqCst);
        // Wrap the shared counter occasionally so it stays positive.
        if raw >= i32::MAX / 2 {
            GLOBAL_NEXT_EXTRANONCE.store(0, Ordering::SeqCst);
            warn!("reset global extranonce counter");
        }
        let masked = raw & max_extranonce;
        let extranonce_str = format!(
            "{:0width$x}",
            masked,
            width = (required_extranonce_size * 2) as usize
        );
        let en2_size = 8 - required_extranonce_size;
        if is_marketplace {
            info!(
                "[AUTO-EXTRANONCE] marketplace en1='{}' ({} byte) => en2_size={} peer={}:{} app='{}'",
                extranonce_str,
                required_extranonce_size,
                en2_size,
                ctx.remote_addr,
                ctx.remote_port,
                remote_app
            );
        } else {
            debug!(
                "[AUTO-EXTRANONCE] Assigned extranonce '{}' (value: {}, size: {} bytes, en2={}) to miner '{}'",
                extranonce_str, masked, required_extranonce_size, en2_size, remote_app
            );
        }
        extranonce_str
    } else {
        debug!(
            "[AUTO-EXTRANONCE] Assigned empty extranonce (size: 0 bytes) to Bitmain miner '{}'",
            remote_app
        );
        String::new()
    };

    *ctx.extranonce.lock() = extranonce.clone();

    debug!(
        "[AUTO-EXTRANONCE] Client {} extranonce set to '{}' (detected miner: '{}')",
        ctx.remote_addr, extranonce, remote_app
    );
}
