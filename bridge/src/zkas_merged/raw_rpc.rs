//! Minimal ZKas gRPC stream client.
//!
//! - `get_block_template` sends `payAddress` as a raw string (`zkas:…`) — no stock Address parse.
//! - `submit_block_with_aux_pow` encodes header protobuf field 16 (`auxPow` hex) on the wire
//!   without round-tripping through stock prost types (which would drop unknown field 16).
//! - Maintains **at most one** MessageStream and reconnects after disconnect (no reconnect storm).

use anyhow::{Context, Result, anyhow};
use async_channel::{Receiver, Sender, bounded};
use bytes::{BufMut, BytesMut};
use futures_util::FutureExt;
use futures_util::StreamExt;
use http::uri::PathAndQuery;
use kaspa_consensus_core::block::Block;
use kaspa_grpc_core::{
    RPC_MAX_MESSAGE_SIZE,
    protowire::{
        GetBlockTemplateRequestMessage, GetInfoRequestMessage, KaspadRequest, KaspadResponse,
        kaspad_response,
    },
};
use kaspa_rpc_core::GetBlockTemplateResponse;
use kaspa_rpc_core::error::RpcResult;
use prost::Message;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tonic::Request;
use tonic::client::Grpc;
use tonic::codec::CompressionEncoding;
use tonic_prost::ProstCodec;
use tracing::{debug, info, warn};

const REQ_TIMEOUT: Duration = Duration::from_secs(30);
const MESSAGE_STREAM_PATH: &str = "/protowire.RPC/MessageStream";
const RECONNECT_MIN_MS: u64 = 1_000;
const RECONNECT_MAX_MS: u64 = 10_000;
const INITIAL_CONNECT_ATTEMPTS: u32 = 12;

enum Outbound {
    Typed(Box<KaspadRequest>),
    /// Pre-encoded full `KaspadRequest` bytes (preserves `auxPow` field 16).
    Raw(Vec<u8>),
}

/// Opaque already-encoded `KaspadRequest` (encode-only).
#[derive(Clone, Default)]
struct RawKaspadBytes(Vec<u8>);

impl Message for RawKaspadBytes {
    fn encode_raw(&self, buf: &mut impl BufMut)
    where
        Self: Sized,
    {
        buf.put_slice(&self.0);
    }

    fn merge_field(
        &mut self,
        _tag: u32,
        _wire_type: prost::encoding::WireType,
        _buf: &mut impl bytes::Buf,
        _ctx: prost::encoding::DecodeContext,
    ) -> Result<(), prost::DecodeError>
    where
        Self: Sized,
    {
        // Encode-only type — never decoded.
        #[allow(deprecated)]
        Err(prost::DecodeError::new("RawKaspadBytes is encode-only"))
    }

    fn encoded_len(&self) -> usize {
        self.0.len()
    }

    fn clear(&mut self) {
        self.0.clear();
    }
}

pub struct ZkasRawRpc {
    tx: Sender<(Outbound, oneshot::Sender<Result<KaspadResponse>>)>,
}

impl ZkasRawRpc {
    /// Connect and keep a single MessageStream alive (reconnects on disconnect).
    pub async fn connect(grpc_host_port: &str) -> Result<Arc<Self>> {
        let url = normalize_url(grpc_host_port);
        let (tx, rx) = bounded(64);
        let (ready_tx, ready_rx) = oneshot::channel::<Result<()>>();

        tokio::spawn(async move {
            run_supervisor(url, rx, ready_tx).await;
        });

        ready_rx
            .await
            .map_err(|_| anyhow!("zkas raw RPC connect task dropped"))?
            .context("zkas raw RPC connect")?;

        Ok(Arc::new(Self { tx }))
    }

    async fn call_outbound(&self, outbound: Outbound) -> Result<KaspadResponse> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send((outbound, resp_tx))
            .await
            .map_err(|_| anyhow!("zkas raw RPC channel closed"))?;
        tokio::time::timeout(REQ_TIMEOUT, resp_rx)
            .await
            .map_err(|_| anyhow!("zkas raw RPC timeout"))?
            .map_err(|_| anyhow!("zkas raw RPC response dropped"))?
    }

    /// `pay_address` is sent verbatim (e.g. `zkas:…`).
    pub async fn get_block_template(&self, pay_address: &str) -> Result<Block> {
        let proto = GetBlockTemplateRequestMessage {
            pay_address: pay_address.to_string(),
            extra_data: String::new(),
        };
        let mut request: KaspadRequest = proto.into();
        request.id = u64::from_le_bytes(rand::random::<[u8; 8]>());
        let response = self
            .call_outbound(Outbound::Typed(Box::new(request)))
            .await?;

        let Some(kaspad_response::Payload::GetBlockTemplateResponse(msg)) = response.payload else {
            return Err(anyhow!(
                "unexpected payload for getBlockTemplate: {:?}",
                response.payload
            ));
        };
        if let Some(err) = msg.error {
            return Err(anyhow!("ZKas getBlockTemplate RPC error: {}", err.message));
        }
        let core: RpcResult<GetBlockTemplateResponse> = (&msg).try_into();
        let core = core.map_err(|e| anyhow!("decode GetBlockTemplateResponse: {e}"))?;
        Block::try_from(core.block).context("Block::try_from ZKas template")
    }

    /// Submit a ZKas template block with AuxPoW hex on header field 16.
    pub async fn submit_block_with_aux_pow(&self, block: &Block, aux_pow_hex: &str) -> Result<()> {
        let rpc_block: kaspa_rpc_core::RpcBlock = block.into();
        let proto_block: kaspa_grpc_core::protowire::RpcBlock = (&rpc_block).into();
        let id = u64::from_le_bytes(rand::random::<[u8; 8]>());
        let raw = encode_kaspad_submit_request(id, &proto_block, aux_pow_hex)?;
        let response = self.call_outbound(Outbound::Raw(raw)).await?;

        let Some(kaspad_response::Payload::SubmitBlockResponse(msg)) = response.payload else {
            return Err(anyhow!(
                "unexpected payload for submitBlock: {:?}",
                response.payload
            ));
        };
        if let Some(err) = msg.error {
            return Err(anyhow!("ZKas submitBlock RPC error: {}", err.message));
        }
        if msg.reject_reason != 0 {
            return Err(anyhow!(
                "ZKas submitBlock rejected: reason={}",
                msg.reject_reason
            ));
        }
        Ok(())
    }

    /// Node sync / version info for dashboards.
    pub async fn get_info(&self) -> Result<ZkasNodeInfo> {
        let mut request: KaspadRequest = GetInfoRequestMessage {}.into();
        request.id = u64::from_le_bytes(rand::random::<[u8; 8]>());
        let response = self
            .call_outbound(Outbound::Typed(Box::new(request)))
            .await?;
        let Some(kaspad_response::Payload::GetInfoResponse(msg)) = response.payload else {
            return Err(anyhow!(
                "unexpected payload for getInfo: {:?}",
                response.payload
            ));
        };
        if let Some(err) = msg.error {
            return Err(anyhow!("ZKas getInfo RPC error: {}", err.message));
        }
        Ok(ZkasNodeInfo {
            is_synced: msg.is_synced,
            server_version: msg.server_version,
            mempool_size: msg.mempool_size,
            is_utxo_indexed: msg.is_utxo_indexed,
            p2p_id: msg.p2p_id,
        })
    }

    /// Server info including virtual DAA score.
    pub async fn get_server_info(&self) -> Result<ZkasServerInfo> {
        use kaspa_grpc_core::protowire::GetServerInfoRequestMessage;
        let mut request: KaspadRequest = GetServerInfoRequestMessage {}.into();
        request.id = u64::from_le_bytes(rand::random::<[u8; 8]>());
        let response = self
            .call_outbound(Outbound::Typed(Box::new(request)))
            .await?;
        let Some(kaspad_response::Payload::GetServerInfoResponse(msg)) = response.payload else {
            return Err(anyhow!(
                "unexpected payload for getServerInfo: {:?}",
                response.payload
            ));
        };
        if let Some(err) = msg.error {
            return Err(anyhow!("ZKas getServerInfo RPC error: {}", err.message));
        }
        Ok(ZkasServerInfo {
            is_synced: msg.is_synced,
            server_version: msg.server_version,
            network_id: msg.network_id,
            virtual_daa_score: msg.virtual_daa_score,
            has_utxo_index: msg.has_utxo_index,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ZkasNodeInfo {
    pub is_synced: bool,
    pub server_version: String,
    pub mempool_size: u64,
    pub is_utxo_indexed: bool,
    pub p2p_id: String,
}

#[derive(Debug, Clone)]
pub struct ZkasServerInfo {
    pub is_synced: bool,
    pub server_version: String,
    pub network_id: String,
    pub virtual_daa_score: u64,
    pub has_utxo_index: bool,
}

fn normalize_url(grpc_host_port: &str) -> String {
    if grpc_host_port.starts_with("http://") || grpc_host_port.starts_with("https://") {
        grpc_host_port.to_string()
    } else if grpc_host_port.starts_with("grpc://") {
        grpc_host_port.replacen("grpc://", "http://", 1)
    } else {
        format!("http://{grpc_host_port}")
    }
}

fn encode_kaspad_submit_request(
    id: u64,
    stock_block: &kaspa_grpc_core::protowire::RpcBlock,
    aux_pow_hex: &str,
) -> Result<Vec<u8>> {
    let header = stock_block
        .header
        .as_ref()
        .ok_or_else(|| anyhow!("missing header"))?;
    let mut header_bytes = header.encode_to_vec();
    prost::encoding::string::encode(16, &aux_pow_hex.to_string(), &mut header_bytes);

    let mut block_bytes = Vec::new();
    prost::encoding::bytes::encode(1, &header_bytes, &mut block_bytes);
    for tx in &stock_block.transactions {
        prost::encoding::message::encode(2, tx, &mut block_bytes);
    }
    if let Some(ref verbose) = stock_block.verbose_data {
        prost::encoding::message::encode(3, verbose, &mut block_bytes);
    }

    let mut submit_bytes = Vec::new();
    prost::encoding::bytes::encode(2, &block_bytes, &mut submit_bytes);

    let mut req = BytesMut::new();
    prost::encoding::uint64::encode(101, &id, &mut req);
    prost::encoding::bytes::encode(1003, &submit_bytes, &mut req);
    Ok(req.to_vec())
}

/// Owns `pending_rx` for the process lifetime. Opens **one** stream at a time.
async fn run_supervisor(
    url: String,
    pending_rx: Receiver<(Outbound, oneshot::Sender<Result<KaspadResponse>>)>,
    ready_tx: oneshot::Sender<Result<()>>,
) {
    let mut backoff_ms = RECONNECT_MIN_MS;
    let mut ready_tx = Some(ready_tx);
    let mut initial_attempts = 0u32;
    let mut ever_connected = false;

    loop {
        if pending_rx.is_closed() {
            if let Some(ready) = ready_tx.take() {
                let _ = ready.send(Err(anyhow!("zkas raw RPC channel closed before connect")));
            }
            return;
        }

        let (out_tx, mut inbound) = match open_message_stream(&url).await {
            Ok(v) => v,
            Err(e) => {
                if ready_tx.is_some() {
                    initial_attempts += 1;
                    if initial_attempts >= INITIAL_CONNECT_ATTEMPTS {
                        if let Some(ready) = ready_tx.take() {
                            let _ = ready.send(Err(e));
                        }
                        warn!(
                            "zkas raw RPC initial connect exhausted; will keep retrying in background"
                        );
                    } else {
                        warn!(
                            "zkas raw RPC initial connect attempt {initial_attempts}/{INITIAL_CONNECT_ATTEMPTS} failed: {e:#}; retrying in {backoff_ms}ms"
                        );
                    }
                } else {
                    warn!("zkas raw RPC reconnect failed: {e:#}; retrying in {backoff_ms}ms");
                }
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms.saturating_mul(2)).min(RECONNECT_MAX_MS);
                continue;
            }
        };

        // Signal readiness as soon as GetInfo succeeds — do not wait for session end.
        if let Some(ready) = ready_tx.take() {
            let _ = ready.send(Ok(()));
        }
        if ever_connected {
            info!("zkas raw RPC: reconnected to {url}");
        } else {
            info!("zkas raw RPC: connected to {url}");
            ever_connected = true;
        }
        backoff_ms = RECONNECT_MIN_MS;

        match drive_session(&pending_rx, out_tx, &mut inbound).await {
            SessionEnd::CallerGone => return,
            SessionEnd::StreamDead(reason) => {
                warn!("zkas raw RPC stream ended ({reason}); reconnecting in {backoff_ms}ms");
            }
        }

        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms.saturating_mul(2)).min(RECONNECT_MAX_MS);
    }
}

enum SessionEnd {
    StreamDead(String),
    CallerGone,
}

async fn drive_session(
    pending_rx: &Receiver<(Outbound, oneshot::Sender<Result<KaspadResponse>>)>,
    out_tx: Sender<Outbound>,
    inbound: &mut tonic::Streaming<KaspadResponse>,
) -> SessionEnd {
    let mut waiters: std::collections::HashMap<u64, oneshot::Sender<Result<KaspadResponse>>> =
        std::collections::HashMap::new();

    let end = loop {
        tokio::select! {
            msg = inbound.next() => {
                match msg {
                    Some(Ok(resp)) => {
                        if let Some(waiter) = waiters.remove(&resp.id) {
                            let _ = waiter.send(Ok(resp));
                        }
                    }
                    Some(Err(e)) => {
                        break SessionEnd::StreamDead(format!("stream error: {e}"));
                    }
                    None => {
                        break SessionEnd::StreamDead("stream closed".into());
                    }
                }
            }
            pending = pending_rx.recv() => {
                match pending {
                    Ok((out, waiter)) => {
                        let id = match &out {
                            Outbound::Typed(r) => r.id,
                            Outbound::Raw(bytes) => peek_request_id(bytes).unwrap_or(0),
                        };
                        waiters.insert(id, waiter);
                        if out_tx.send(out).await.is_err() {
                            if let Some(w) = waiters.remove(&id) {
                                let _ = w.send(Err(anyhow!("request forward closed")));
                            }
                            break SessionEnd::StreamDead("request forward closed".into());
                        }
                    }
                    Err(_) => break SessionEnd::CallerGone,
                }
            }
        }
    };

    for (_, w) in waiters.drain() {
        let _ = w.send(Err(anyhow!("zkas raw RPC stream reconnecting")));
    }
    end
}

async fn open_message_stream(
    url: &str,
) -> Result<(Sender<Outbound>, tonic::Streaming<KaspadResponse>)> {
    let channel = tonic::transport::Channel::from_shared(url.to_string())
        .map_err(|e| anyhow!("bad ZKas URI {url}: {e}"))?
        .connect_timeout(Duration::from_secs(15))
        .timeout(REQ_TIMEOUT)
        .connect()
        .await
        .with_context(|| format!("connect ZKas gRPC {url}"))?;

    let mut grpc = Grpc::new(channel)
        .send_compressed(CompressionEncoding::Gzip)
        .accept_compressed(CompressionEncoding::Gzip)
        .max_decoding_message_size(RPC_MAX_MESSAGE_SIZE);

    grpc.ready()
        .await
        .map_err(|e| anyhow!("ZKas gRPC not ready: {e}"))?;

    let (out_tx, out_rx) = bounded::<Outbound>(64);
    let outbound = async_stream::stream! {
        while let Ok(item) = out_rx.recv().await {
            let bytes = match item {
                Outbound::Typed(req) => req.encode_to_vec(),
                Outbound::Raw(bytes) => bytes,
            };
            yield RawKaspadBytes(bytes);
        }
    };

    let path = PathAndQuery::from_static(MESSAGE_STREAM_PATH);
    let codec = ProstCodec::<RawKaspadBytes, KaspadResponse>::default();
    let streaming =
        AssertUnwindSafe(grpc.streaming(Request::new(outbound), path, codec)).catch_unwind();
    let mut inbound = match streaming.await {
        Ok(Ok(resp)) => resp.into_inner(),
        Ok(Err(e)) => return Err(anyhow!("ZKas MessageStream: {e}")),
        Err(_) => return Err(anyhow!("ZKas MessageStream panicked (gRPC Ready race)")),
    };

    let mut get_info: KaspadRequest = GetInfoRequestMessage {}.into();
    get_info.id = 1;
    out_tx
        .send(Outbound::Typed(Box::new(get_info)))
        .await
        .map_err(|_| anyhow!("send GetInfo"))?;
    match inbound.next().await {
        Some(Ok(msg)) => {
            debug!("zkas raw RPC GetInfo ok id={}", msg.id);
        }
        Some(Err(e)) => return Err(anyhow!("GetInfo stream error: {e}")),
        None => return Err(anyhow!("GetInfo: stream closed")),
    }

    Ok((out_tx, inbound))
}

fn peek_request_id(bytes: &[u8]) -> Option<u64> {
    use bytes::Buf;
    let mut buf = bytes;
    while buf.has_remaining() {
        let (tag, wire) = prost::encoding::decode_key(&mut buf).ok()?;
        if tag == 101 && wire == prost::encoding::WireType::Varint {
            return prost::encoding::decode_varint(&mut buf).ok();
        }
        prost::encoding::skip_field(
            wire,
            tag,
            &mut buf,
            prost::encoding::DecodeContext::default(),
        )
        .ok()?;
    }
    None
}
