//! RKstratumKasZkasSolo — click-run in-process Kaspa + ZKAS solo stratum.
//! Hosting fee is permanently locked at 1% (see `solo_fee`).

use clap::Parser;
use kaspa_alloc::init_allocator_with_default_settings;
use kaspa_stratum_bridge::cli::{Cli, NodeMode};
use kaspa_stratum_bridge::product_runtime::{
    ensure_product_runtime, hold_console_on_error, print_startup_banner,
};
use kaspa_stratum_bridge::runner::run;

#[tokio::main]
async fn main() {
    init_allocator_with_default_settings();
    print_startup_banner();

    let result = async {
        let runtime_cfg = ensure_product_runtime()?;

        let mut cli = Cli::parse();
        // Product always embeds both nodes — ignore any --node-mode external from args.
        cli.node_mode = Some(NodeMode::Inprocess);
        if cli.config.is_none() {
            cli.config = Some(runtime_cfg);
        }

        run(cli).await
    }
    .await;

    if let Err(e) = result {
        eprintln!("\nFATAL: {e:#}");
        hold_console_on_error();
        std::process::exit(1);
    }
}
