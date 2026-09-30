//! ZKas full node embedded in the bridge process, mirroring `inprocess_node.rs`.
//!
//! Merged mining needs a ZKas node to build block templates and validate submissions.
//! Linking zkas-rusty here keeps the solo build to a single executable instead of
//! requiring an operator-run node or a spawned child.
//!
//! The bridge still reaches this node over localhost gRPC, exactly as it reaches the
//! in-process Kaspa node — embedding changes who owns the process, not the transport.

use std::sync::Arc;

use zkas_kaspa_core::signals::Shutdown;
use zkas_kaspa_utils::fd_budget;
use zkas_kaspad::{args as zkas_args, daemon as zkas_daemon};

pub(crate) struct InProcessZkasNode {
    core: Arc<zkas_kaspa_core::core::Core>,
    workers: Vec<std::thread::JoinHandle<()>>,
}

impl InProcessZkasNode {
    pub(crate) fn start_from_args(args: zkas_args::Args) -> Result<Self, anyhow::Error> {
        let _ = fd_budget::try_set_fd_limit(zkas_daemon::DESIRED_DAEMON_SOFT_FD_LIMIT);

        let runtime = zkas_daemon::Runtime::from_args(&args);
        let fd_total_budget = fd_budget::limit()
            - args.rpc_max_clients as i32
            - args.inbound_limit as i32
            - args.outbound_target as i32;
        let (core, _) = zkas_daemon::create_core_with_runtime(&runtime, &args, fd_total_budget);
        let workers = core.start();
        Ok(Self { core, workers })
    }

    fn shutdown(self) {
        self.core.shutdown();
        self.core.join(self.workers);
    }
}

pub(crate) async fn shutdown_inprocess_zkas(node: InProcessZkasNode) {
    let _ = tokio::task::spawn_blocking(move || node.shutdown()).await;
}
