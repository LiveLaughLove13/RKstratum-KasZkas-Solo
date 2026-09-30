# ZKas merge-mining (solo bridge)

Opt-in Kaspa→ZKas AuxPoW merge-mining in the **solo** `stratum-bridge` binary.
Default is **off** — existing solo operators are unchanged until they enable it.

## One binary

- Stock Kaspa embeds via `--node-mode inprocess` (unchanged).
- When `zkas_merged.enabled=true`, the same process also starts an **in-process ZKas**
  node (linked from firecash/zkas-rusty) with localhost gRPC + optional embedded
  wallet-api for auto-consolidate.
- No second `kaspad` executable, no `shielded-pay.exe`, no PPLNS / treasury payouts.

## Miner password: `zkas:`

At stratum authorize, a miner may supply a mainnet `zkas:…` address (password or
user field). That address becomes the ZKas template `payAddress` for their jobs.

If the miner has no valid `zkas:`, the bridge uses `fallback_zkas_address` from
config. If neither is set, the job is **Kaspa-only** (no `ZKMM` tag).

## How merge works

1. Fetch a ZKas template for the resolved pay address (1.5s cache per address).
2. Stage `ZKMM` + hex(`H_fc`) on the Kaspa coinbase tag.
3. ASIC mines the Kaspa job as usual.
4. On Kaspa accept → build AuxPoW from the parent’s committed `H_fc` → submit to ZKas
   (or log only when `dry_run: true`).

## Auto-consolidate (operator wallet)

Solo model: the **operator** holds the seed for the wallet being folded (usually
your `fallback_zkas_address` / your own mining `zkas:`).

When enabled, a background loop talks HTTP to the embedded wallet-api
(`127.0.0.1:<wallet_api>`, insecure). If tip note count exceeds the threshold it
POSTs `/api/wallet/consolidate`. Halo2 proving runs inside walletd
(`spawn_blocking` there); mining / AuxPoW never wait on prove.

Requires:

- `consolidate_enabled: true`
- `seed_path` → 64-hex seed file
- `consolidate_address` must match the address derived from that seed
- `consolidate_note_threshold` > 0

## Config

```yaml
zkas_merged:
  enabled: false
  dry_run: true
  zkas_rpc: "127.0.0.1:16810"
  zkas_p2p: "127.0.0.1:16811"
  zkas_appdir: ""              # default beside Kaspa appdir
  wallet_api: "127.0.0.1:18501"
  fallback_zkas_address: ""
  consolidate_enabled: false
  consolidate_note_threshold: 40
  consolidate_interval_ms: 600000
  consolidate_fee_sompi: 10000000
  max_spends_per_tx: 20        # documented; wallet-api applies its mass cap
  seed_path: ""
  consolidate_address: ""
```

Recommended first enable: `enabled: true`, `dry_run: true`, set
`fallback_zkas_address`, leave consolidate off until AuxPoW submits look correct,
then set `dry_run: false`.

## Build note

ZKas crates come from [`firecash/zkas-rusty`](https://github.com/firecash/zkas-rusty)
(`tag = "zkas-v1.0.4"` in `bridge/Cargo.toml`). No local `_vendor` checkout is required.
Full bump checklist for Kaspa **and** ZKas: [`UPSTREAM-UPDATES.md`](./UPSTREAM-UPDATES.md).
Enabling `wallet-api` pulls in Halo2 / walletd and lengthens compile time.
