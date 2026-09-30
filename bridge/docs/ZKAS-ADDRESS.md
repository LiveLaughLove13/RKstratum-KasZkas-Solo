# ZKAS payout address (solo bridge)

A miner can declare a ZKAS Orchard payout address. The bridge decodes and validates it at
authorize time, stores it on the connection, and logs it. This is the same parser the PPLNS
pool build uses, so an address accepted here is accepted there.

## What this does and does not do

This is **recognition and validation only**. The solo bridge does not merge-mine ZKAS: it
holds no ZKas node, no treasury, and no payout engine, so a declared address does not by
itself earn or receive ZKAS. Its value is that a wrong address is caught at connect time
instead of after days of mining, and that the correct address is recorded and logged.

Actually merge-mining ZKAS requires a second `kaspad` for the ZKas chain running as a
separate process. That is out of scope here by design — this build stays a single
`stratum-bridge` binary with no external helper executables.

## How a miner sets it

Either form works; the worker string wins if both are present.

**Password** (preferred for ASICs, whose wallet/worker fields are too short for a ~150
character combined string):

```
user: kaspa:<your-kaspa-address>.<worker>
pass: zkas:<your-zkas-address>
```

**Third dot-segment of the worker string:**

```
user: kaspa:<your-kaspa-address>.<worker>.zkas:<your-zkas-address>
```

This composes with the existing `worker=DIFF` override, e.g.
`kaspa:<addr>.rig1=8192.zkas:<addr>`.

Passwords that are not ZKAS addresses (`x`, empty, anything else) are ignored exactly as
before, and a third segment that is not a `zkas:` address is still parsed as a Canxium
address.

## Validation rules

An address is accepted only if it is a mainnet `zkas:` ShieldedOrchard (version 9) address
with a valid checksum decoding to a 43-byte recipient. Rejected: empty or junk strings,
`zkastest:` testnet addresses, double prefixes (`zkas:zkas:…`), and any single-character
typo, since the bech32 checksum is verified rather than just the prefix and length.

A rejected address is **not fatal**. The bridge logs a warning and Kaspa authorize still
succeeds, so a ZKAS typo can never knock a rig off Kaspa mining.

## Verifying

At `INFO` level each authorize prints the outcome:

```
[HANDSHAKE] authorized 1.2.3.4:5678 worker='rig1' app='BzMiner' zkas=zkas:py82h42m…
[HANDSHAKE] authorized 1.2.3.4:5678 worker='rig2' app='IceRiver' zkas=none
```

`zkas=none` with a rejected address means the accompanying
`[AUTHORIZE] Ignoring invalid/non-mainnet zkas address` warning explains why.

## Code touchpoints

| Concern | File |
| --- | --- |
| Address decode + mainnet validation (pure `std`, no deps) | `bridge/src/zkas_address.rs` |
| Password + worker-string parsing at authorize | `bridge/src/stratum/default_client.rs` |
| Per-connection storage | `bridge/src/stratum/stratum_context/types.rs` (`ClientIdentity::zkas_addr`) |
| Read accessor | `bridge/src/stratum/stratum_context/mod.rs` (`StratumContext::zkas_address`) |
| Tests | `bridge/src/tests.rs` (`zkas_address_tests`), `bridge/src/zkas_address.rs` |
