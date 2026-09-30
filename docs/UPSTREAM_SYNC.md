# Upstream sync guide

This repository is a **standalone** Stratum bridge workspace (`bridge/`, `bridge-tauri/`, root `Cargo.toml`). It is **not** a fork of the full [LiveLaughLove13/rusty-kaspa](https://github.com/LiveLaughLove13/rusty-kaspa) monorepo, but it must stay aligned with:

- The **`bridge/`** crate in that tree (behavior, security fixes, config semantics).
- **Kaspa library APIs** from the same repo (consensus, RPC, in-process `kaspad`), including hard-fork lines such as **Toccata**.

Use this document whenever you pull new work from upstream `master` (or a tagged pre-release such as `1.3.0-toc.*`).

## Repositories

| Remote | URL | Role |
| --- | --- | --- |
| `origin` | `https://github.com/LiveLaughLove13/rusty-kaspa-stratum.git` | This standalone repo (releases, Tauri GUI, CI). |
| `upstream` | `https://github.com/LiveLaughLove13/rusty-kaspa.git` | Full node + `bridge/` source of truth for bridge logic. |

If `upstream` is missing:

```bash
git remote add upstream https://github.com/LiveLaughLove13/rusty-kaspa.git
git fetch upstream master
```

On PowerShell, use the same commands (no `head`; use `Select-Object -First N` if you paginate log output).

## What to sync vs what to keep

### Do **not** replace wholesale

Upstream `bridge/` on `master` uses a **flat** layout (`kaspaapi.rs`, `share_handler.rs`, `prom.rs`, `stratum_listener.rs`, …). This repo uses a **modular** layout under `bridge/src/` (see [bridge/docs/CONTRIBUTOR_MAP.md](../bridge/docs/CONTRIBUTOR_MAP.md)).

Never copy the entire upstream `bridge/` tree over `bridge/` here—you would lose:

- Module boundaries (`stratum/`, `share_handler/submit/`, `prom/http/`, …).
- `bridge-tauri/` desktop shell and duplicated dashboard assets under `bridge-tauri/ui/`.
- Standalone workspace `Cargo.toml`, Docker, and release workflows.

### **Do** sync on every upstream pass

1. **Workspace Kaspa dependencies** — root [Cargo.toml](../Cargo.toml) `[workspace.dependencies]` git URLs and version label (e.g. `1.3.0-toc.5`), then refresh the lockfile:

   ```bash
   cargo update -p kaspa-consensus-core -p kaspa-hashes -p kaspa-pow -p kaspa-addresses \
     -p kaspa-rpc-core -p kaspa-rpc-service -p kaspa-grpc-client -p kaspa-notify \
     -p kaspa-core -p kaspa-utils -p kaspad -p kaspa-alloc
   ```

2. **`rust-version`** in root `Cargo.toml` and [bridge-tauri/src-tauri/Cargo.toml](../bridge-tauri/src-tauri/Cargo.toml) to match upstream workspace (check upstream root `Cargo.toml`).

3. **Bridge logic** — port commits that touch `bridge/` in upstream (see [Finding upstream bridge changes](#finding-upstream-bridge-changes)).

4. **Static dashboard** — if upstream changes `bridge/static/js/dashboard.js` (or related HTML/CSS), apply the same edits to:

   - `bridge/static/…`
   - `bridge-tauri/ui/dashboard/…` (keep both in sync).

5. **Docs** — upstream `bridge/docs/README.md` may gain new CLI or config notes; merge into [bridge/docs/README.md](../bridge/docs/README.md) and [CONFIGURATION.md](CONFIGURATION.md) as needed.

### Optional / release-only upstream paths

These live only in the monorepo; mirror behavior here only when you need parity:

- `docker/Dockerfile.stratum-bridge` — compare with root [Dockerfile](../Dockerfile).
- `.github/workflows/` — compare deploy/AppImage steps with [.github/workflows/rust.yml](../.github/workflows/rust.yml).
- `bridge/appimage/` — documented in [PACKAGING.md](PACKAGING.md).

## Finding upstream bridge changes

After fetching upstream:

```bash
git fetch upstream master
git log --oneline <last-sync-upstream-sha>..upstream/master -- bridge/
```

Record `<last-sync-upstream-sha>` in your PR or commit message (e.g. `Synced bridge from upstream d5205cc7`).

Inspect a specific fix:

```bash
git show <commit> -- bridge/
```

Compare file-level diff **without** merging trees (standalone HEAD vs upstream bridge):

```bash
git diff HEAD upstream/master -- bridge/
```

Large diffs are expected: layout differs. Use them to spot **new files**, **config.yaml** changes, and **static/** updates; port logic into the modular paths below.

## Upstream flat file → standalone module map

When reading upstream `bridge/src/*.rs`, use this map to know where to apply changes:

| Upstream (flat) | Standalone (modular) |
| --- | --- |
| `app_config.rs` | `bridge/src/config/app_config.rs` |
| `client_handler.rs` | `bridge/src/stratum/client_handler/` (+ `job_dispatch/`, `handshake.rs`) |
| `default_client.rs` | `bridge/src/stratum/default_client.rs` |
| `errors.rs` | `bridge/src/util/errors.rs` |
| `hasher.rs` | `bridge/src/mining/hasher.rs` |
| `jsonrpc_event.rs` | `bridge/src/jsonrpc/jsonrpc_event.rs` |
| `kaspaapi.rs` | `bridge/src/kaspa/kaspaapi/` |
| `mining_state.rs` | `bridge/src/mining/mining_state.rs` |
| `pow_diagnostic.rs` | `bridge/src/mining/pow_diagnostic.rs` |
| `prom.rs` | `bridge/src/prom/metrics.rs`, `prom/http/`, `prom/mod.rs` |
| `share_handler.rs` | `bridge/src/share_handler/` (`lifecycle.rs`, `submit/`, `vardiff.rs`, …) |
| `stratum_context.rs` | `bridge/src/stratum/stratum_context/` |
| `stratum_listener.rs` | `bridge/src/stratum/stratum_listener/` + `stratum_line_codec.rs` |
| `stratum_server.rs` | `bridge/src/stratum/stratum_server.rs` |
| `main.rs` / `cli.rs` | `bridge/src/main.rs`, `bridge/src/runner.rs`, `bridge/src/cli.rs` (standalone splits orchestration into `runner.rs`) |
| `rkstratum_cpu_miner.rs` | `bridge/src/cpu_miner/rkstratum_cpu_miner.rs` |
| `tests.rs` | `bridge/src/tests.rs` |

Full per-file notes: [bridge/docs/CONTRIBUTOR_MAP.md](../bridge/docs/CONTRIBUTOR_MAP.md).

## Recommended sync workflow

### 1. Prepare

```bash
git fetch upstream master
git checkout main
git pull origin main
```

Note upstream tip, e.g. `git rev-parse upstream/master`.

### 2. Bump Kaspa stack

1. Open upstream `Cargo.toml` on GitHub (or `git show upstream/master:Cargo.toml`) and copy:
   - `workspace.package.version` (e.g. `1.3.0-toc.5`)
   - `rust-version`
2. Update root [Cargo.toml](../Cargo.toml):
   - `workspace.package.version` (bridge crate version label)
   - `rust-version`
   - All `kaspa-*` / `kaspad` git deps → `https://github.com/LiveLaughLove13/rusty-kaspa.git`, branch `master` (or a specific tag for reproducible releases).
3. Run `cargo update` for Kaspa packages (commands above).
4. `cargo build -p kaspa-stratum-bridge` and fix **compile errors** first (RPC/consensus API breaks are common across hard forks).

### 3. Port bridge commits

For each upstream commit touching `bridge/`:

1. Read `git show <commit> -- bridge/`.
2. Apply equivalent logic in the modular path (table above).
3. If the change adds tests in upstream `bridge/src/tests.rs`, add or adjust tests in `bridge/src/tests.rs` here.

**Examples already ported (Toccata era)—use as patterns:**

| Upstream PR / topic | Standalone location |
| --- | --- |
| Stratum line size cap (#1023) | `stratum/stratum_line_codec.rs`, `stratum_listener/client_io/read_loop.rs` |
| Worker label `asic-{id}` (#1014) | `stratum/stratum_context/mod.rs`, `prom/metrics.rs` (`worker_context`), `default_client.rs` |
| 0-share worker UI retention (#1016) | `share_handler/lifecycle.rs` (`get_stats_if_exists`), `prom/metrics.rs` (`update_worker_difficulty`) |
| TOTAL SPM average (#1033) | `share_handler/lifecycle.rs` (`average_worker_spm`) |
| Legacy IP in dashboard | `bridge/static/js/dashboard.js`, `bridge-tauri/ui/dashboard/js/dashboard.js` |

### 4. Config and packaging

- Diff `bridge/config.yaml` and [CONFIGURATION.md](CONFIGURATION.md).
- Skim upstream `bridge/Cargo.toml` for new dependencies or feature flags; mirror in [bridge/Cargo.toml](../bridge/Cargo.toml).

### 5. Verify

```bash
cargo fmt --all
cargo clippy -p kaspa-stratum-bridge -- -D warnings
cargo test -p kaspa-stratum-bridge --lib
cargo build -p kaspa-stratum-bridge --release
```

**Runtime smoke test** (external Toccata-capable node):

```bash
# Terminal A: kaspad from LiveLaughLove13/rusty-kaspa (matching network / fork)
kaspad --utxoindex --rpclisten=127.0.0.1:16110 --rpclisten-borsh=127.0.0.1:17110

# Terminal B: bridge from this repo
cargo run -p kaspa-stratum-bridge --release --bin stratum-bridge -- \
  --config bridge/config.yaml --node-mode external
```

Confirm dashboard, authorize, jobs, and submit against your target network (mainnet / testnet / Toccata testnet).

Tag releases only after `v*` CI (see [README.md](../README.md)) if you need desktop + musl artifacts.

## Toccata and hard forks

When upstream merges a **hard fork** (e.g. Toccata on testnet-10 / future networks):

1. Sync **all** workspace Kaspa git deps to the fork branch or tag that contains the activation logic.
2. Rebuild and run integration tests; PoW, block template, and RPC types may change.
3. Run the bridge against a node built from the **same** upstream revision (or official pre-release binary for that network).
4. Document the upstream SHA and network in your sync commit message.

Pinned lockfile revision example (after `cargo update`):

```text
kaspa-consensus-core v1.3.0-toc.5 (https://github.com/LiveLaughLove13/rusty-kaspa.git?branch=master#d5205cc7)
```

For reproducible releases, consider noting that git SHA in release notes.

## When kaspanet/rusty-kaspa matters

[LiveLaughLove13/rusty-kaspa](https://github.com/LiveLaughLove13/rusty-kaspa) is forked from [kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa). Bridge fixes often land on **kaspanet** first, then appear on LiveLaughLove13 `master`. If something is missing on LiveLaughLove13, check kaspanet PRs labeled `[Bridge]` or `bridge/` paths before reimplementing locally.

This standalone repo should track **LiveLaughLove13** unless you explicitly decide to follow kaspanet for a given release.

## Checklist (copy into PR description)

- [ ] `git fetch upstream` and recorded upstream SHA
- [ ] `Cargo.toml` / `Cargo.lock` Kaspa deps and `rust-version` aligned
- [ ] All relevant `git log upstream/master -- bridge/` commits reviewed and ported or N/A
- [ ] `bridge/static` and `bridge-tauri/ui/dashboard` JS/CSS/HTML synced if UI changed
- [ ] `bridge/config.yaml` + [CONFIGURATION.md](CONFIGURATION.md) updated if needed
- [ ] `cargo build`, `cargo test -p kaspa-stratum-bridge --lib`, clippy clean
- [ ] Smoke test vs matching `kaspad` on target network (in-process or external)

## Related docs

- [docs/README.md](README.md) — documentation index  
- [bridge/docs/CONTRIBUTOR_MAP.md](../bridge/docs/CONTRIBUTOR_MAP.md) — module file map  
- [bridge/docs/README.md](../bridge/docs/README.md) — operator-facing bridge guide  
- [PACKAGING.md](PACKAGING.md) — AppImage, Tauri, release layout  
