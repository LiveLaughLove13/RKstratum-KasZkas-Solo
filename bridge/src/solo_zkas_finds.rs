//! Private solo ZKAS find ledger (dual-external merge-mine).
//!
//! Stores **zkas payee + ZKAS block hash only** — never kaspa:/worker/KAS parent —
//! so a DB leak or private API cannot link identities to public `/api/stats` blocks.

use blake2::{Blake2b512, Digest};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

static DB: OnceCell<Mutex<Connection>> = OnceCell::new();

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS solo_zkas_finds (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  zkas_address TEXT NOT NULL,
  zkas_block_hash TEXT NOT NULL,
  accepted_at_ms INTEGER NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('miner','fee')),
  UNIQUE(zkas_address, zkas_block_hash)
);
CREATE INDEX IF NOT EXISTS idx_solo_zkas_finds_addr_time
  ON solo_zkas_finds(zkas_address, accepted_at_ms DESC);
"#;

const MS_HOUR: i64 = 60 * 60 * 1000;
const MS_DAY: i64 = 24 * MS_HOUR;
const DEFAULT_PAGE_LIMIT: usize = 100;
const MAX_PAGE_LIMIT: usize = 100;

/// Safe public fields for private verify (no kaspa / worker / KAS hash).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoloZkasFindApi {
    pub zkas_block_hash: String,
    pub accepted_at_ms: i64,
    pub role: String,
}

/// Chart bucket — start timestamp + count (no address / hash).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoloZkasFindBucket {
    pub t: i64,
    pub n: i64,
}

#[derive(Debug, Deserialize)]
struct SoloZkasFindsRequest {
    zkas_address: String,
    #[serde(default)]
    limit: Option<u32>,
    /// Keyset cursor: return rows older than this `(accepted_at_ms, zkas_block_hash)`.
    #[serde(default)]
    before_ms: Option<i64>,
    #[serde(default)]
    before_hash: Option<String>,
}

fn default_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("RKSTRATUM_SOLO_ZKAS_FINDS_DB") {
        let t = p.trim();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    // Prefer durable E: volume on the dual-external host so finds survive
    // LOCALAPPDATA / profile wipes. Override with RKSTRATUM_SOLO_ZKAS_FINDS_DB.
    #[cfg(windows)]
    {
        PathBuf::from(r"E:\kaspa-stratum-bridge\solo_zkas_finds.sqlite")
    }
    #[cfg(not(windows))]
    {
        crate::app_dirs::get_bridge_app_dir().join("solo_zkas_finds.sqlite")
    }
}

/// Open (or create) the finds DB. Safe to call multiple times.
pub fn init() -> anyhow::Result<()> {
    init_at_path(&default_db_path())
}

pub fn init_at_path(path: &Path) -> anyhow::Result<()> {
    if DB.get().is_some() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    DB.set(Mutex::new(conn))
        .map_err(|_| anyhow::anyhow!("solo_zkas_finds: DB already initialized"))?;
    info!(
        target: "solo_zkas_finds",
        path = %path.display(),
        "solo ZKAS finds ledger ready"
    );
    Ok(())
}

/// Short log tag — never log the full shielded address.
pub fn zkas_addr_log_tag(addr: &str) -> String {
    let mut hasher = Blake2b512::new();
    hasher.update(addr.trim().to_lowercase().as_bytes());
    let digest = hasher.finalize();
    format!("zkas#{}", hex::encode(&digest[..5]))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn role_for_payee(zkas_addr: &str) -> &'static str {
    let addr = zkas_addr.trim();
    if let Some(cfg) = crate::solo_fee::solo_fee_config() {
        let fee = cfg.zkas_fee_address.trim();
        if !fee.is_empty() && fee.eq_ignore_ascii_case(addr) {
            return "fee";
        }
    }
    "miner"
}

fn clamp_page_limit(limit: Option<u32>) -> usize {
    match limit {
        Some(n) if n > 0 => (n as usize).clamp(1, MAX_PAGE_LIMIT),
        _ => DEFAULT_PAGE_LIMIT,
    }
}

fn insert_find(
    conn: &Connection,
    addr: &str,
    hash: &str,
    accepted_at_ms: i64,
    role: &str,
) -> anyhow::Result<usize> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO solo_zkas_finds
         (zkas_address, zkas_block_hash, accepted_at_ms, role)
         VALUES (?1, ?2, ?3, ?4)",
        params![addr, hash, accepted_at_ms, role],
    )?;
    Ok(n)
}

/// Record an AuxPoW-accepted ZKAS find. Ignores invalid / empty addresses.
pub fn record_find(zkas_addr: &str, zkas_block_hash: &str) {
    let addr = zkas_addr.trim();
    let hash = zkas_block_hash.trim().to_lowercase();
    if addr.is_empty() || hash.is_empty() {
        return;
    }
    if !crate::zkas_address::is_valid_mainnet_zkas_payout_address(addr) {
        warn!(
            target: "solo_zkas_finds",
            tag = %zkas_addr_log_tag(addr),
            "skip record: invalid zkas payee"
        );
        return;
    }
    let Some(db) = DB.get() else {
        warn!(target: "solo_zkas_finds", "skip record: DB not initialized");
        return;
    };
    let role = role_for_payee(addr);
    let accepted_at_ms = now_ms();
    let tag = zkas_addr_log_tag(addr);
    let conn = db.lock();
    match insert_find(&conn, addr, &hash, accepted_at_ms, role) {
        Ok(n) if n > 0 => {
            info!(
                target: "solo_zkas_finds",
                %tag,
                role,
                "recorded solo ZKAS find"
            );
        }
        Ok(_) => {}
        Err(e) => {
            warn!(target: "solo_zkas_finds", %tag, error = %e, "record failed");
        }
    }
}

fn finds_page_conn(
    conn: &Connection,
    zkas_addr: &str,
    limit: usize,
    before_ms: Option<i64>,
    before_hash: Option<&str>,
) -> anyhow::Result<(Vec<SoloZkasFindApi>, bool)> {
    let addr = zkas_addr.trim();
    let lim = limit.clamp(1, MAX_PAGE_LIMIT);
    // Fetch one extra row to know if another page exists.
    let fetch = (lim + 1) as i64;

    let mut out: Vec<SoloZkasFindApi> = Vec::new();
    if let Some(before) = before_ms {
        let hash = before_hash.unwrap_or("").trim().to_lowercase();
        let mut stmt = conn.prepare(
            "SELECT zkas_block_hash, accepted_at_ms, role
             FROM solo_zkas_finds
             WHERE zkas_address = ?1
               AND (
                 accepted_at_ms < ?2
                 OR (accepted_at_ms = ?2 AND zkas_block_hash < ?3)
               )
             ORDER BY accepted_at_ms DESC, zkas_block_hash DESC
             LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![addr, before, hash, fetch], |row| {
            Ok(SoloZkasFindApi {
                zkas_block_hash: row.get(0)?,
                accepted_at_ms: row.get(1)?,
                role: row.get(2)?,
            })
        })?;
        for r in rows {
            out.push(r?);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT zkas_block_hash, accepted_at_ms, role
             FROM solo_zkas_finds
             WHERE zkas_address = ?1
             ORDER BY accepted_at_ms DESC, zkas_block_hash DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![addr, fetch], |row| {
            Ok(SoloZkasFindApi {
                zkas_block_hash: row.get(0)?,
                accepted_at_ms: row.get(1)?,
                role: row.get(2)?,
            })
        })?;
        for r in rows {
            out.push(r?);
        }
    }

    let has_more = out.len() > lim;
    if has_more {
        out.truncate(lim);
    }
    Ok((out, has_more))
}

/// Private lookup — returns finds for this address only (no address in rows).
pub fn finds_for_address(zkas_addr: &str, limit: usize) -> anyhow::Result<Vec<SoloZkasFindApi>> {
    let Some(db) = DB.get() else {
        anyhow::bail!("unavailable");
    };
    let conn = db.lock();
    let (finds, _) = finds_page_conn(&conn, zkas_addr, limit, None, None)?;
    Ok(finds)
}

/// Keyset page for private API.
pub fn finds_page_for_address(
    zkas_addr: &str,
    limit: usize,
    before_ms: Option<i64>,
    before_hash: Option<&str>,
) -> anyhow::Result<(Vec<SoloZkasFindApi>, bool)> {
    let Some(db) = DB.get() else {
        anyhow::bail!("unavailable");
    };
    let conn = db.lock();
    finds_page_conn(&conn, zkas_addr, limit, before_ms, before_hash)
}

pub fn address_has_finds(zkas_addr: &str) -> anyhow::Result<bool> {
    Ok(count_for_address(zkas_addr)? > 0)
}

fn count_conn(conn: &Connection, zkas_addr: &str) -> anyhow::Result<i64> {
    let addr = zkas_addr.trim();
    let n: i64 = conn.query_row(
        "SELECT COUNT(1) FROM solo_zkas_finds WHERE zkas_address = ?1",
        params![addr],
        |row| row.get(0),
    )?;
    Ok(n)
}

pub fn count_for_address(zkas_addr: &str) -> anyhow::Result<i64> {
    let Some(db) = DB.get() else {
        anyhow::bail!("unavailable");
    };
    let conn = db.lock();
    count_conn(&conn, zkas_addr)
}

fn count_since_conn(conn: &Connection, zkas_addr: &str, since_ms: i64) -> anyhow::Result<i64> {
    let addr = zkas_addr.trim();
    let n: i64 = conn.query_row(
        "SELECT COUNT(1) FROM solo_zkas_finds
         WHERE zkas_address = ?1 AND accepted_at_ms >= ?2",
        params![addr, since_ms],
        |row| row.get(0),
    )?;
    Ok(n)
}

/// Count finds with `accepted_at_ms` in the last 24 hours (wall clock).
pub fn count_last_24h_for_address(zkas_addr: &str) -> anyhow::Result<i64> {
    let Some(db) = DB.get() else {
        anyhow::bail!("unavailable");
    };
    let since_ms = now_ms().saturating_sub(MS_DAY);
    let conn = db.lock();
    count_since_conn(&conn, zkas_addr, since_ms)
}

/// Count finds with `accepted_at_ms` in the last 7 days (wall clock).
pub fn count_last_7d_for_address(zkas_addr: &str) -> anyhow::Result<i64> {
    let Some(db) = DB.get() else {
        anyhow::bail!("unavailable");
    };
    let since_ms = now_ms().saturating_sub(7 * MS_DAY);
    let conn = db.lock();
    count_since_conn(&conn, zkas_addr, since_ms)
}

fn floor_to_hour(ms: i64) -> i64 {
    if ms <= 0 {
        return 0;
    }
    (ms / MS_HOUR) * MS_HOUR
}

fn floor_to_utc_day(ms: i64) -> i64 {
    if ms <= 0 {
        return 0;
    }
    (ms / MS_DAY) * MS_DAY
}

fn buckets_conn(
    conn: &Connection,
    zkas_addr: &str,
    since_ms: i64,
    bucket_ms: i64,
    now: i64,
) -> anyhow::Result<Vec<SoloZkasFindBucket>> {
    let addr = zkas_addr.trim();
    let mut stmt = conn.prepare(
        "SELECT ((accepted_at_ms / ?3) * ?3) AS bucket_start, COUNT(1)
         FROM solo_zkas_finds
         WHERE zkas_address = ?1 AND accepted_at_ms >= ?2
         GROUP BY bucket_start
         ORDER BY bucket_start ASC",
    )?;
    let rows = stmt.query_map(params![addr, since_ms, bucket_ms], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut map: HashMap<i64, i64> = HashMap::new();
    for r in rows {
        let (t, n) = r?;
        map.insert(t, n);
    }

    let first = if bucket_ms == MS_HOUR {
        floor_to_hour(since_ms)
    } else {
        floor_to_utc_day(since_ms)
    };
    let last = if bucket_ms == MS_HOUR {
        floor_to_hour(now)
    } else {
        floor_to_utc_day(now)
    };

    let mut out = Vec::new();
    let mut t = first;
    while t <= last {
        out.push(SoloZkasFindBucket {
            t,
            n: *map.get(&t).unwrap_or(&0),
        });
        t = t.saturating_add(bucket_ms);
        if out.len() > 400 {
            break;
        }
    }
    Ok(out)
}

fn analytics_for_address(
    conn: &Connection,
    zkas_addr: &str,
    now: i64,
) -> anyhow::Result<(i64, i64, i64, Vec<SoloZkasFindBucket>, Vec<SoloZkasFindBucket>)> {
    let total = count_conn(conn, zkas_addr)?;
    let last_24h = count_since_conn(conn, zkas_addr, now.saturating_sub(MS_DAY))?;
    let last_7d = count_since_conn(conn, zkas_addr, now.saturating_sub(7 * MS_DAY))?;
    let hourly = buckets_conn(conn, zkas_addr, now.saturating_sub(MS_DAY), MS_HOUR, now)?;
    let daily = buckets_conn(conn, zkas_addr, now.saturating_sub(7 * MS_DAY), MS_DAY, now)?;
    Ok((total, last_24h, last_7d, hourly, daily))
}

/// `POST /api/solo/zkas/finds` — returns `(status_code, json_body)`.
/// Never includes kaspa:/worker/KAS hash or the raw zkas address in the success body.
pub fn post_solo_zkas_finds_json(body: &str) -> (u16, String) {
    let req: SoloZkasFindsRequest = match serde_json::from_str(body.trim()) {
        Ok(r) => r,
        Err(_) => {
            let empty = body.trim().is_empty();
            return (
                400,
                if empty {
                    r#"{"error":"invalid_json","message":"Empty body. Send JSON {\"zkas_address\":\"zkas:…\"}."}"#
                        .into()
                } else {
                    r#"{"error":"invalid_json","message":"Body must be JSON {\"zkas_address\":\"zkas:…\"}."}"#
                        .into()
                },
            );
        }
    };
    let addr = req.zkas_address.trim().to_string();
    if !crate::zkas_address::is_valid_mainnet_zkas_payout_address(&addr) {
        return (
            400,
            r#"{"error":"invalid_address","message":"Enter a valid mainnet zkas: payout address."}"#
                .into(),
        );
    }

    let limit = clamp_page_limit(req.limit);
    let before_ms = req.before_ms.filter(|ms| *ms > 0);
    let before_hash = req
        .before_hash
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase());
    if before_ms.is_some() && before_hash.is_none() {
        // Allow time-only cursor (hash defaults to "" in SQL — still deterministic).
    }

    let tag = zkas_addr_log_tag(&addr);
    let Some(db) = DB.get() else {
        return (
            503,
            r#"{"error":"unavailable","message":"Solo ZKAS finds ledger unavailable."}"#.into(),
        );
    };
    let conn = db.lock();
    let now = now_ms();

    let (total, last_24h, last_7d, buckets_hourly, buckets_daily) =
        match analytics_for_address(&conn, &addr, now) {
            Ok(v) => v,
            Err(e) => {
                warn!(target: "solo_zkas_finds", %tag, error = %e, "private finds analytics error");
                return (
                    500,
                    r#"{"error":"error","message":"Lookup failed."}"#.into(),
                );
            }
        };

    if total <= 0 {
        info!(target: "solo_zkas_finds", %tag, "private finds miss");
        return (
            404,
            r#"{"error":"not_found","message":"No solo ZKAS finds for that address."}"#.into(),
        );
    }

    match finds_page_conn(
        &conn,
        &addr,
        limit,
        before_ms,
        before_hash.as_deref(),
    ) {
        Ok((finds, has_more)) => {
            if finds.is_empty() && before_ms.is_none() {
                info!(target: "solo_zkas_finds", %tag, "private finds miss");
                return (
                    404,
                    r#"{"error":"not_found","message":"No solo ZKAS finds for that address."}"#
                        .into(),
                );
            }
            info!(
                target: "solo_zkas_finds",
                %tag,
                count = total,
                last_24h,
                last_7d,
                returned = finds.len(),
                has_more,
                "private finds ok"
            );
            let payload = serde_json::json!({
                "finds": finds,
                "count": total,
                "finds_last_24h": last_24h,
                "finds_last_7d": last_7d,
                "returned": finds.len(),
                "has_more": has_more,
                "buckets_hourly_24h": buckets_hourly,
                "buckets_daily_7d": buckets_daily,
            });
            (
                200,
                serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into()),
            )
        }
        Err(e) => {
            warn!(target: "solo_zkas_finds", %tag, error = %e, "private finds db error");
            (
                500,
                r#"{"error":"error","message":"Lookup failed."}"#.into(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    static TEST_LOCK: StdMutex<()> = StdMutex::new(());

    const DEV_FEE_ADDR: &str =
        "zkas:py82h42m9qjff0knpcmllzq3c7qhurje5auh4tq2ceagf69wjpf23djwwmqr26zhsua8rrglrwdltsh";

    fn open_temp_conn() -> (Connection, PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "solo_zkas_finds_page_{}_{}.sqlite",
            std::process::id(),
            nanos
        ));
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        (conn, path)
    }

    fn hash_n(n: u8) -> String {
        format!("{:064x}", n as u128)
    }

    #[test]
    fn log_tag_never_contains_address() {
        let tag = zkas_addr_log_tag(DEV_FEE_ADDR);
        assert!(tag.starts_with("zkas#"));
        assert!(!tag.contains("py82"));
        assert!(!tag.contains(DEV_FEE_ADDR));
    }

    #[test]
    fn keyset_pagination_and_analytics_on_conn() {
        let (conn, path) = open_temp_conn();
        let now = 1_700_000_000_000i64; // fixed for buckets
        // 5 finds spaced by 1 hour; oldest first for insert order.
        for i in 0..5u8 {
            insert_find(
                &conn,
                DEV_FEE_ADDR,
                &hash_n(i),
                now - (4 - i as i64) * MS_HOUR,
                "miner",
            )
            .unwrap();
        }

        let (page1, more1) = finds_page_conn(&conn, DEV_FEE_ADDR, 2, None, None).unwrap();
        assert_eq!(page1.len(), 2);
        assert!(more1);
        assert_eq!(page1[0].zkas_block_hash, hash_n(4));
        assert_eq!(page1[1].zkas_block_hash, hash_n(3));

        let (page2, more2) = finds_page_conn(
            &conn,
            DEV_FEE_ADDR,
            2,
            Some(page1[1].accepted_at_ms),
            Some(&page1[1].zkas_block_hash),
        )
        .unwrap();
        assert_eq!(page2.len(), 2);
        assert!(more2);
        assert_eq!(page2[0].zkas_block_hash, hash_n(2));
        assert_eq!(page2[1].zkas_block_hash, hash_n(1));

        let (page3, more3) = finds_page_conn(
            &conn,
            DEV_FEE_ADDR,
            2,
            Some(page2[1].accepted_at_ms),
            Some(&page2[1].zkas_block_hash),
        )
        .unwrap();
        assert_eq!(page3.len(), 1);
        assert!(!more3);
        assert_eq!(page3[0].zkas_block_hash, hash_n(0));

        let (total, last_24h, last_7d, hourly, daily) =
            analytics_for_address(&conn, DEV_FEE_ADDR, now).unwrap();
        assert_eq!(total, 5);
        assert_eq!(last_24h, 5);
        assert_eq!(last_7d, 5);
        assert!(!hourly.is_empty());
        assert_eq!(hourly.iter().map(|b| b.n).sum::<i64>(), 5);
        assert!(!daily.is_empty());
        assert_eq!(daily.iter().map(|b| b.n).sum::<i64>(), 5);

        // Older than 7d should not count in 7d / 24h.
        insert_find(
            &conn,
            DEV_FEE_ADDR,
            &hash_n(9),
            now - 10 * MS_DAY,
            "miner",
        )
        .unwrap();
        let (total2, last_24h2, last_7d2, _, _) =
            analytics_for_address(&conn, DEV_FEE_ADDR, now).unwrap();
        assert_eq!(total2, 6);
        assert_eq!(last_24h2, 5);
        assert_eq!(last_7d2, 5);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn record_and_private_json_omit_linkable_fields() {
        let _g = TEST_LOCK.lock().unwrap();
        if DB.get().is_some() {
            return;
        }
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "solo_zkas_finds_test_{}_{}.sqlite",
            std::process::id(),
            nanos
        ));
        let _ = std::fs::remove_file(&path);
        init_at_path(&path).unwrap();

        record_find(
            DEV_FEE_ADDR,
            "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899",
        );
        let (status, json) =
            post_solo_zkas_finds_json(&format!(r#"{{"zkas_address":"{DEV_FEE_ADDR}"}}"#,));
        assert_eq!(status, 200, "{json}");
        assert!(!json.contains("kaspa:"));
        assert!(!json.contains("zkas_address"));
        assert!(!json.contains("worker"));
        assert!(json.contains("zkas_block_hash"));
        assert!(json.contains("accepted_at_ms"));
        assert!(json.contains("finds_last_24h"));
        assert!(json.contains("finds_last_7d"));
        assert!(json.contains("has_more"));
        assert!(json.contains("buckets_hourly_24h"));
        assert!(json.contains("buckets_daily_7d"));

        let (miss_status, _) = post_solo_zkas_finds_json(
            r#"{"zkas_address":"zkas:qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"}"#,
        );
        // Invalid or empty → 400 or 404; never 200 with kaspa leak.
        assert!(miss_status == 400 || miss_status == 404, "{miss_status}");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_json_400() {
        let (status, json) = post_solo_zkas_finds_json("not-json");
        assert_eq!(status, 400);
        assert!(json.contains("invalid_json"));
    }

    #[test]
    fn invalid_address_400() {
        let (status, json) = post_solo_zkas_finds_json(r#"{"zkas_address":"kaspa:qqqq"}"#);
        assert_eq!(status, 400);
        assert!(json.contains("invalid_address"));
    }
}
