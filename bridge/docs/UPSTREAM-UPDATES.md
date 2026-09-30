# Upstream update strategy (Kaspa + ZKas)

This solo bridge embeds **two** node stacks in one `stratum-bridge` binary:

| Stack | Source | How it is pinned today | Role |
| --- | --- | --- | --- |
| Kaspa | [`LiveLaughLove13/rusty-kaspa`](https://github.com/LiveLaughLove13/rusty-kaspa) | Workspace `Cargo.toml`: `branch = "master"` (resolved rev in `Cargo.lock`) | In-process Kaspa node + stratum mining |
| ZKas | [`firecash/zkas-rusty`](https://github.com/firecash/zkas-rusty) | `bridge/Cargo.toml`: **release tag** (currently `zkas-v1.0.4`) | In-process ZKas node + AuxPoW merge + wallet-api consolidate |

Never point ZKas at a local `_vendor` tree. Always use the firecash GitHub repo so team releases are a tag bump.

---

## Rules that keep updates from breaking production

1. **One upstream at a time.** Do not bump Kaspa and ZKas in the same change. Dual-stack link failures are hard to attribute if both move together.
2. **Tag / rev, not floating tip, for releases you ship.** ZKas is already tag-pinned. For Kaspa, after a successful bump, record the `Cargo.lock` rev (and prefer switching workspace deps from `branch = "master"` to `rev = "<sha>"` before tagging a release binary).
3. **Never edit `Cargo.lock` by hand.** Change `Cargo.toml`, then let Cargo rewrite the lockfile.
4. **Keep the dual-allocator escape hatch.** ZKas `kaspad` must keep `default-features = false, features = ["heap", "wallet-api"]`. `heap` disables ZKas’s `#[global_allocator]` so Rusty-Kaspa’s mimalloc remains the only one.
5. **Gate live merge/consolidate behind config.** Default `zkas_merged.enabled: false`. After a bump, smoke with `dry_run: true` before paying real AuxPoW / folds.
6. **Stop if consensus / genesis changes.** A ZKas genesis or AuxPoW-format change is not a drop-in bump — treat it as a migration (see below).

---

## ZKas bump procedure (`firecash/zkas-rusty`)

### When to bump

- A new **`zkas-v*`** release on [firecash/zkas-rusty releases](https://github.com/firecash/zkas-rusty/releases) that you need (wallet sync fixes, merge-mining fixes, security).
- Prefer the latest **stable** `zkas-vX.Y.Z` tag. Do not track `main` for shipped binaries unless you intentionally want tip churn.

### Steps

1. **Read the release notes** for: consensus / genesis changes, AuxPoW / `ZKMM` changes, wallet-api / consolidate API changes, new required CLI flags (`--addpeer`, bootstrap snapshot, `--archival`).
2. **API preflight** (cheap, before full compile):
   - Confirm `kaspad` still exposes `features = ["heap", "wallet-api"]`.
   - Confirm `kaspad_lib::{args, daemon}` still has `Runtime::from_args`, `create_core_with_runtime`, `DESIRED_DAEMON_SOFT_FD_LIMIT`.
   - Confirm `--wallet-api` / `--wallet-api-insecure` still exist if consolidate is enabled.
3. **Bump the three deps in `bridge/Cargo.toml`** to the same tag:
   ```toml
   zkas_kaspad = { package = "kaspad", git = "https://github.com/firecash/zkas-rusty.git", tag = "zkas-vX.Y.Z", default-features = false, features = ["heap", "wallet-api"] }
   zkas_kaspa_core = { package = "kaspa-core", git = "https://github.com/firecash/zkas-rusty.git", tag = "zkas-vX.Y.Z" }
   zkas_kaspa_utils = { package = "kaspa-utils", git = "https://github.com/firecash/zkas-rusty.git", tag = "zkas-vX.Y.Z" }
   ```
4. **Resolve + verify**
   ```bash
   cargo tree -p kaspa-stratum-bridge --depth 1   # both kaspad lines: LiveLaughLove13 + firecash
   cargo fmt --all
   cargo clippy -p kaspa-stratum-bridge --all-targets --all-features -- -D warnings
   cargo test -p kaspa-stratum-bridge --all-features
   cargo build -p kaspa-stratum-bridge --release --bin stratum-bridge
   ```
5. **Smoke (optional but recommended before mining)**
   - Start with `zkas_merged.enabled: true`, `dry_run: true`.
   - Confirm in-process ZKas binds `zkas_rpc` / `zkas_p2p`, templates stage `ZKMM`, authorize still accepts `zkas:` password.
   - If using consolidate: import still works against wallet-api; a fold tick does not stall Kaspa jobs.
   - Then `dry_run: false` only after AuxPoW accepts look healthy.
6. **Commit** `Cargo.toml` + `Cargo.lock` + any bridge glue fixes together. Update the “current pin” line in this doc and in `ZKAS-MERGED-MINING.md`.

### ZKas red flags (stop and plan a migration)

- Genesis hash / network id change
- AuxPoW borsh layout or `ZKMM` commitment format change
- Removal of `heap` / `wallet-api` features, or rename of `kaspad_lib` entry points
- Wallet-api consolidate route / auth change that breaks `bridge/src/zkas_merged/consolidate.rs`

### Operational note (not a compile break)

Upstream may still require a **bootstrap datadir** and `--addpeer` for a fresh ZKas node to join mainnet. That is runtime ops (`zkas_appdir` / peers), not a Cargo pin problem. Follow the release notes on [firecash/zkas-rusty releases](https://github.com/firecash/zkas-rusty/releases).

---

## Kaspa bump procedure (`LiveLaughLove13/rusty-kaspa`)

### Why this is riskier than ZKas

Workspace deps currently use `branch = "master"`. `Cargo.lock` freezes the last resolved rev, but `cargo update` can jump tip without a tag ceremony. Treat Kaspa bumps as deliberate.

### Steps

1. **Record the current rev** from `Cargo.lock` (search `LiveLaughLove13/rusty-kaspa`) so you can revert.
2. **Fetch changelog / commits** on that fork (and upstream `kaspanet/rusty-kaspa` if the fork tracks it): consensus, RPC, gRPC Ready races, coinbase / template APIs, allocator changes.
3. **Bump intentionally**
   - Preferred for a release: set every `kaspa-*` / `kaspad` workspace dep to the same `rev = "<sha>"` (drop `branch = "master"`).
   - Dev exploration only: keep `branch = "master"` and run `cargo update -p kaspad` (updates the lockfile tip).
4. **Verify**
   ```bash
   cargo fmt --all
   cargo clippy -p kaspa-stratum-bridge --all-targets --all-features -- -D warnings
   cargo test -p kaspa-stratum-bridge --all-features
   cargo build -p kaspa-stratum-bridge --release --bin stratum-bridge
   ```
5. **Smoke**
   - `--node-mode inprocess` still starts; gRPC connect does not panic on Ready race.
   - Templates + submits still work for IceRiver / marketplace / plain miners.
   - With ZKas **still on the previous pin**, confirm merge hooks still compile and `dry_run` AuxPoW path still runs (proves the Kaspa side of the dual stack alone).
6. **Commit** workspace `Cargo.toml` (if rev-pinned) + `Cargo.lock`.

### Kaspa red flags (stop and plan)

- `kaspa-alloc` / global allocator changes that fight ZKas `heap`
- Breaking `GetBlockTemplate` / `SubmitBlock` or notification mode changes
- gRPC / tonic version jumps that break `zkas_merged/raw_rpc.rs` (it uses the **Kaspa** `kaspa-grpc-core` types on the wire)
- Consensus hard forks your operators are not ready for

---

## Dual-stack link checklist (after either bump)

Always confirm both stacks remain distinct in the graph:

```bash
cargo tree -p kaspa-stratum-bridge --depth 1 | findstr /i "kaspad kaspa-core"
```

You should see **two** `kaspad` lines (and two `kaspa-core` / `kaspa-utils`): one firecash tag, one LiveLaughLove13 rev. A single line means one stack was dropped — do not ship.

Also confirm there is still exactly one binary:

```bash
# bridge/Cargo.toml should have a single [[bin]] name = "stratum-bridge"
```

---

## Rollback

1. Revert the `Cargo.toml` pin(s) and restore the previous `Cargo.lock` from git.
2. `cargo clean -p kaspa-stratum-bridge` if the dual link misbehaves after a half-applied update.
3. Rebuild release and redeploy the known-good exe.

---

## Current pins (update this table when you bump)

| Stack | Pin | Resolved rev (see `Cargo.lock`) | Verified |
| --- | --- | --- | --- |
| ZKas | tag `zkas-v1.0.5` | see `Cargo.lock` | unchanged this bump (one upstream at a time) |
| Kaspa | rev `01b532e8b553523216471682649693af92f0fd16` (v2.1.0) | same | aligned with Cap KAS-zkas; stage via `Promote-DualExternal.ps1 -BuildOnly` |
