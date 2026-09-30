//! ZKas AuxPoW commitment in a Kaspa parent coinbase payload.
//! Layout: `ZKMM` (4 ASCII bytes) || lowercase-hex(`H_fc`) (64 chars).
//! See firecash/zkas-rusty `consensus/core/src/auxpow.rs`.

pub const MERGE_MINE_MAGIC: [u8; 4] = *b"ZKMM";
pub const COMMITMENT_HEX_LEN: usize = 64;

/// Build `prefix || ZKMM || hex(h_fc) || suffix` (hex is lowercase ASCII / UTF-8 safe).
pub fn embed_zkmm_commitment(prefix: &[u8], h_fc: &[u8; 32], suffix: &[u8]) -> Vec<u8> {
    let hex = encode_hex32(h_fc);
    let mut out =
        Vec::with_capacity(prefix.len() + MERGE_MINE_MAGIC.len() + hex.len() + suffix.len());
    out.extend_from_slice(prefix);
    out.extend_from_slice(&MERGE_MINE_MAGIC);
    out.extend_from_slice(&hex);
    out.extend_from_slice(suffix);
    out
}

/// Append a ZKMM commitment to an existing Kaspa `extraData` / coinbase tag.
/// Returns `None` if `tag` already contains `ZKMM` (anti-ambiguity).
pub fn append_zkmm_commitment(tag: &[u8], h_fc: &[u8; 32]) -> Option<Vec<u8>> {
    if find_magic(tag).is_some() {
        return None;
    }
    Some(embed_zkmm_commitment(tag, h_fc, &[]))
}

fn find_magic(payload: &[u8]) -> Option<usize> {
    payload
        .windows(MERGE_MINE_MAGIC.len())
        .position(|w| w == MERGE_MINE_MAGIC)
}

fn encode_hex32(bytes: &[u8; 32]) -> [u8; COMMITMENT_HEX_LEN] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; COMMITMENT_HEX_LEN];
    for (i, &b) in bytes.iter().enumerate() {
        out[2 * i] = HEX[(b >> 4) as usize];
        out[2 * i + 1] = HEX[(b & 0x0f) as usize];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_contains_magic_and_64_hex() {
        let h = [0xabu8; 32];
        let payload = embed_zkmm_commitment(b"RK-Stratum", &h, &[]);
        assert!(payload.windows(4).any(|w| w == b"ZKMM"));
        assert_eq!(payload.len(), b"RK-Stratum".len() + 4 + 64);
        let start = b"RK-Stratum".len() + 4;
        assert!(payload[start..].iter().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn append_rejects_duplicate_magic() {
        let h = [1u8; 32];
        let once = append_zkmm_commitment(b"tag", &h).expect("first");
        assert!(append_zkmm_commitment(&once, &h).is_none());
    }
}
