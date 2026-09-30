use crate::{
    jsonrpc_event::JsonRpcEvent, mining_state::MiningState, prom::*,
    stratum_context::StratumContext,
};
use tracing::{debug, error};

/// Send `mining.set_difficulty` to a client (spawned). Prefer [`send_client_diff_await`]
/// before the first `mining.notify` so marketplace proxies see difficulty first.
pub fn send_client_diff(
    instance_id: &str,
    client: &StratumContext,
    state: &MiningState,
    diff: f64,
) {
    let instance_id = instance_id.to_string();
    let client_clone = client.clone();
    let _ = state; // retained for call-site compatibility
    tokio::spawn(async move {
        let _ = send_client_diff_await(&instance_id, &client_clone, diff).await;
    });
}

/// Awaited difficulty send — must complete before the first job for NiceHash / MRR,
/// which drop a `mining.notify` that arrives before any difficulty.
pub async fn send_client_diff_await(
    instance_id: &str,
    client: &StratumContext,
    diff: f64,
) -> Result<(), ()> {
    debug!(
        "[DIFFICULTY] Sending mining.set_difficulty={} to {}:{}",
        diff, client.remote_addr, client.remote_port
    );

    let remote_app = client.identity.lock().remote_app.clone();
    let is_marketplace = crate::share_handler::is_marketplace_app(&remote_app);

    let send_result = if is_marketplace {
        // Kaspa common stratum uses integer powers of two; keep whole numbers as ints.
        let diff_value =
            if diff.fract() == 0.0 && diff.is_finite() && diff >= 0.0 && diff <= u64::MAX as f64 {
                serde_json::Value::Number(serde_json::Number::from(diff as u64))
            } else {
                serde_json::Value::Number(
                    serde_json::Number::from_f64(diff)
                        .unwrap_or_else(|| serde_json::Number::from(diff as u64)),
                )
            };
        // 2Miners NiceHash Kaspa: {"id":null,"method":"mining.set_difficulty",...}
        client
            .send_notification_null_id("mining.set_difficulty", vec![diff_value])
            .await
    } else {
        let diff_value = serde_json::Value::Number(
            serde_json::Number::from_f64(diff)
                .unwrap_or_else(|| serde_json::Number::from(diff as u64)),
        );
        let diff_event = JsonRpcEvent {
            jsonrpc: "2.0".to_string(),
            method: "mining.set_difficulty".to_string(),
            id: None,
            params: vec![diff_value],
        };
        client.send(diff_event).await
    };

    if let Err(e) = send_result {
        let wallet_addr = client.identity.lock().wallet_addr.clone();
        record_worker_error(
            instance_id,
            &wallet_addr,
            crate::errors::ErrorShortCode::FailedSetDiff.as_str(),
        );
        error!("[DIFFICULTY] ERROR: Failed sending difficulty: {}", e);
        return Err(());
    }

    if is_marketplace {
        debug!(
            "[DIFFICULTY] set_difficulty={} peer={}:{} app='{}'",
            diff, client.remote_addr, client.remote_port, remote_app
        );
    } else {
        debug!(
            "[DIFFICULTY] Successfully sent difficulty {} to {}",
            diff, client.remote_addr
        );
    }
    Ok(())
}
