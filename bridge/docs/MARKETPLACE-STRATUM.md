# Marketplace Stratum wire (NiceHash / MRR / LazyPickaxe)

**Do not regress this.** Matching a known-good NiceHash Kaspa pool (`kas.2miners.com:2020`) is what makes MRR/NiceHash ASICs submit shares instead of authorizing and disconnecting with zero shares. IceRiver and Bitmain must stay on their own paths.

This is the solo bridge: a rented rig authorizes with its own `kaspa:` address and mines templates paying that address. Marketplace support is purely a wire/difficulty concern and does not change the solo payout model.

## Miner classes (UA → behavior)

| Class | UA detection (`diff_policy.rs`) | Job format | Extranonce | Notify `id` |
|-------|----------------------------------|------------|------------|-------------|
| **Marketplace** | `nicehash`, `miningrigrentals`, `lazypickaxe` | Kaspa-common `[job_id, [u64×4], timestamp]` | 1-byte en1 → en2_size **7** | JSON `null` |
| **IceRiver** | `iceriver`, `icemining`, `icm` | 80-char hex string | 2-byte en1, params `[en1]` only | **omit** `id` |
| **Bitmain** | `godminer`, `bitmain`, `antminer` | Legacy `[job_id, [u64×4], timestamp]` | empty en1 in subscribe result | full JSON-RPC |

Detection helpers live in `bridge/src/share_handler/diff_policy.rs`:

- `is_marketplace_app`
- `uses_eth_stratum_hex_job` — **IceRiver only** (marketplace must **not** use hex jobs)

## Marketplace handshake (canonical — matches 2Miners)

```text
C→S  mining.subscribe  ["NiceHash/1.0.0"]
S→C  {"id":1,"result":[true,"EthereumStratum/1.0.0"],"error":null}
S→C  {"id":null,"method":"set_extranonce","params":["ec",7]}   ← bare set_extranonce, NOT mining.set_extranonce
C→S  mining.authorize  ["kaspa:….WORKER=DIFF","x"]
S→C  {"id":2,"result":true,"error":null}
S→C  {"id":null,"method":"mining.set_difficulty","params":[16384]}
S→C  {"id":null,"method":"mining.notify","params":["1",[u64,u64,u64,u64],timestamp_ms]}
```

Critical rules:

1. Method name is **`set_extranonce`** (no `mining.` prefix).
2. Params are **`[extranonce1_hex, extranonce2_size]`** with size **7** (1-byte en1).
3. Send extranonce **immediately after subscribe**, before authorize is processed.
4. Difficulty and notify use **`"id": null`** — not omitted, not numeric.
5. `mining.set_difficulty` is **awaited** before the first `mining.notify`.
6. Jobs are **array + timestamp**, never IceRiver hex, even if the UA also matches the big-job regex.

Code touchpoints (keep in sync):

| Concern | File |
|---------|------|
| UA detection, seed diff, VarDiff floor, `worker=DIFF` | `bridge/src/share_handler/diff_policy.rs` |
| en1 size = 1 byte for marketplace | `bridge/src/stratum/client_handler/handshake.rs` |
| `set_extranonce` after subscribe, Eth-style `login` | `bridge/src/stratum/default_client.rs` |
| `id: null` notifications | `bridge/src/stratum/stratum_context/outbound.rs` (`send_notification_null_id`) |
| Awaited difficulty, array jobs | `bridge/src/stratum/client_handler/job_dispatch/{difficulty,immediate_job,new_block_job}.rs` |
| Floor survives VarDiff step-down | `bridge/src/share_handler/lifecycle.rs`, `work_stats.rs`, `bridge/src/mining/mining_state.rs` |

## Difficulty policy

Rentals need a high starting difficulty and must never be stepped below the band they were rented at.

```yaml
# bridge/config.yaml
nicehash_min_share_diff: 16384   # 0 = feature off
```

- **Off by default.** With `0`, marketplace UAs behave exactly like any other miner.
- When set, marketplace UAs start at `max(instance min_share_diff, nicehash_min_share_diff)` and that value becomes a VarDiff floor.
- `pow2_clamp: true` also clamps this value to a power of two at startup.
- Per-worker override: `kaspa:ADDR.RK=16384` or `kaspa:ADDR.rig/d=8192` raises both the start diff and the floor, capped at 65536 and floored to a power of two.
- Non-marketplace miners are never affected by any of the above.

## Reference capture (no guesswork)

Reproduce the golden handshake against a known-good pool:

```bash
python bridge/scripts/stratum_ref_probe.py \
  --host kas.2miners.com --port 2020 \
  --agent "NiceHash/1.0.0" \
  --user "kaspa:YOUR_ADDR.WORKER" --pass x \
  --mode subscribe --wait 5
```

Compare to this bridge:

```bash
python bridge/scripts/stratum_ref_probe.py \
  --host 127.0.0.1 --port 5555 \
  --agent "NiceHash/1.0.0" \
  --user "kaspa:YOUR_ADDR.RK1=32768" --pass x \
  --mode subscribe --wait 3
```

The two captures must agree on method names, `id` presence, and param shapes.

## Known status

| Agent | Status | Notes |
|-------|--------|-------|
| NiceHash / MRR subscribe path | **Working** | The proven marketplace path; MRR is detected by the `miningrigrentals` UA |
| LazyPickaxe `login` | Partial | `login` with object params is accepted and normalized, but this client has been observed dropping ~0.6s after the first job. `kas.2miners.com` answers `Unknown method` for `login`, so there is no reference wire to match yet — do not guess a result shape |
| IceRiver | Working | Must keep hex + omit-`id`; marketplace rules must not leak into this path |
| Bitmain | Working | Unchanged by marketplace support |

## Regression checklist (before merging stratum changes)

- [ ] Marketplace detected by `is_marketplace_app`; **not** by `uses_eth_stratum_hex_job`
- [ ] Marketplace en1 length is **1 byte** (`en2_size = 7`)
- [ ] Wire uses **`set_extranonce`** with `[en1, 7]` and `"id":null` right after subscribe
- [ ] Marketplace notify is **3 params**: job id, 4×u64 array, timestamp
- [ ] IceRiver still gets hex notify with **no** `id` field
- [ ] `nicehash_min_share_diff: 0` leaves every miner class byte-identical to before
- [ ] `cargo test` + `cargo clippy --all-targets -- -D warnings`
- [ ] Optional: re-run `stratum_ref_probe.py` against 2Miners and localhost; shapes must match

## History (why this exists)

An earlier attempt served NiceHash the IceRiver hex job with numeric notification ids. Clients authenticated and accepted difficulty but submitted **0 shares** and EOF'd immediately. A wire capture against **2Miners** showed the mismatches listed above; aligning marketplace-only to that wire produced shares without changing IceRiver or Bitmain behavior.
