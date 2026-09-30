# Solo ZKAS private finds (dual-external)

Miners who know their full `zkas:` can list **their own** AuxPoW-accepted ZKAS finds without linking to a public `kaspa:` identity.

## API

`POST /api/solo/zkas/finds`

```json
{
  "zkas_address": "zkas:…",
  "limit": 50,
  "before_ms": 1710000000000,
  "before_hash": "optional_tiebreak_hex"
}
```

- Body only — query strings rejected
- `limit` optional (default **100**, clamp 1..=100)
- `before_ms` / `before_hash` optional keyset cursor — return rows **older** than that pair (`ORDER BY accepted_at_ms DESC, zkas_block_hash DESC`)
- Hit → (no address in body):

```json
{
  "finds": [{ "zkas_block_hash", "accepted_at_ms", "role" }],
  "count": 4503,
  "finds_last_24h": 307,
  "finds_last_7d": 2100,
  "returned": 50,
  "has_more": true,
  "buckets_hourly_24h": [{ "t": 1710000000000, "n": 12 }],
  "buckets_daily_7d": [{ "t": 1709942400000, "n": 400 }]
}
```

| Field | Meaning |
|-------|---------|
| `count` | **Total** ledger rows for that address (not page-capped) |
| `finds_last_24h` | Rows with `accepted_at_ms` in the last 24h (authoritative) |
| `finds_last_7d` | Rows with `accepted_at_ms` in the last 7 days (authoritative) |
| `returned` | Length of this page |
| `has_more` | Another older page exists |
| `buckets_*` | Zero-filled chart buckets (counts only; no hashes) |

- Explorer link (UI): `https://explorer.zkas.info/blocks/{zkas_block_hash}` — block hash only, never kaspa:/worker
- Miss → uniform `404`
- Rate limit: `RKSTRATUM_HTTP_SOLO_ZKAS_FINDS_RATE_PER_MIN` (default 30)
- `Cache-Control: no-store`
- Logs `zkas#…` tag only

DB path (Windows default): `E:\kaspa-stratum-bridge\solo_zkas_finds.sqlite`  
Override: `RKSTRATUM_SOLO_ZKAS_FINDS_DB` (full file path).  
Non-Windows fallback: `~/.kaspa-stratum-bridge/solo_zkas_finds.sqlite`.

Schema stores `zkas_address` + `zkas_block_hash` + time + `role` (`miner`|`fee`) — **never** kaspa/worker/KAS parent.

HTTP note: the dashboard listener reads the full `Content-Length` body (same fix as Cap verify). A single socket read is not enough behind Cloudflare.

## Deploy

Rebuild and restart dual-external only. Finds before this deploy are not backfilled.
