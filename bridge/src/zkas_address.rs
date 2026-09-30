//! ZKAS Orchard address decode / mainnet validation.
//!
//! Self-contained on purpose: pure `std`, no crate dependencies and no ZKas node.
//! Used by stratum authorize so a miner-supplied `zkas:` payout address is validated
//! at connect time instead of silently accepting junk or a testnet string.

/// Decode a `zkas:` / `zkastest:` Orchard address → 43-byte coinbase script body.
pub fn orchard_script_bytes_from_zkas_address(addr: &str) -> Option<Vec<u8>> {
    let addr = addr.trim();
    let (hrp, data) = addr.split_once(':')?;
    if hrp != "zkas" && hrp != "zkastest" {
        return None;
    }
    let payload = decode_kaspa_bech32_payload(hrp, data)?;
    // payload = [version = 9 = ShieldedOrchard] || 43-byte recipient
    if payload.len() != 44 || payload[0] != 9 {
        return None;
    }
    Some(payload[1..].to_vec())
}

/// True when `addr` is a mainnet `zkas:` Orchard payment address.
///
/// Rejects empty/junk, `zkastest:`, double-prefix (`zkas:zkas:…`), and any string
/// that does not decode to a ShieldedOrchard (version 9) 43-byte recipient.
pub fn is_valid_mainnet_zkas_payout_address(addr: &str) -> bool {
    let addr = addr.trim();
    if addr.is_empty() {
        return false;
    }
    let Some((hrp, data)) = addr.split_once(':') else {
        return false;
    };
    if hrp != "zkas" || data.is_empty() || data.contains(':') {
        return false;
    }
    orchard_script_bytes_from_zkas_address(addr).is_some()
}

// Kaspa-style bech32 (matches the kaspa-addresses / zkas address codec).

const REV_CHARSET: [u8; 128] = {
    let mut t = [100u8; 128];
    let charset = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    let mut i = 0;
    while i < 32 {
        t[charset[i] as usize] = i as u8;
        i += 1;
    }
    t
};

fn polymod(values: impl Iterator<Item = u8>) -> u64 {
    let mut c = 1u64;
    for d in values {
        let c0 = c >> 35;
        c = ((c & 0x07ffffffff) << 5) ^ (d as u64);
        if c0 & 0x01 != 0 {
            c ^= 0x98f2bc8e61;
        }
        if c0 & 0x02 != 0 {
            c ^= 0x79b76d99e2;
        }
        if c0 & 0x04 != 0 {
            c ^= 0xf33e5fb3c4;
        }
        if c0 & 0x08 != 0 {
            c ^= 0xae2eabe2a8;
        }
        if c0 & 0x10 != 0 {
            c ^= 0x1e4f43e470;
        }
    }
    c ^ 1
}

fn checksum(payload_u5: &[u8], prefix: impl Iterator<Item = u8>) -> u64 {
    polymod(
        prefix
            .chain(std::iter::once(0u8))
            .chain(payload_u5.iter().copied())
            .chain(std::iter::repeat_n(0u8, 8)),
    )
}

fn conv5to8(payload: &[u8]) -> Vec<u8> {
    let mut eight_bit = vec![0u8; payload.len() * 5 / 8];
    let mut current_idx = 0usize;
    let mut buff = 0u16;
    let mut bits = 0u32;
    for c in payload {
        buff = (buff << 5) | (*c as u16);
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            eight_bit[current_idx] = (buff >> bits) as u8;
            buff &= (1 << bits) - 1;
            current_idx += 1;
        }
    }
    eight_bit
}

fn decode_kaspa_bech32_payload(hrp: &str, data: &str) -> Option<Vec<u8>> {
    if data.len() < 8 {
        return None;
    }
    let mut address_u5 = Vec::with_capacity(data.len());
    for b in data.bytes() {
        let idx = *REV_CHARSET.get(b as usize)?;
        if idx == 100 {
            return None;
        }
        address_u5.push(idx);
    }
    let (payload_u5, checksum_u5) = address_u5.split_at(address_u5.len() - 8);
    let fivebit_prefix = hrp.bytes().map(|c| c & 0x1f);
    let checksum_bytes = conv5to8(checksum_u5);
    if checksum_bytes.len() != 5 {
        return None;
    }
    let mut be = [0u8; 8];
    be[3..].copy_from_slice(&checksum_bytes);
    let checksum_num = u64::from_be_bytes(be);
    if checksum(payload_u5, fivebit_prefix) != checksum_num {
        return None;
    }
    Some(conv5to8(payload_u5))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known-valid mainnet Orchard address (the public ZKas protocol dev-fee recipient),
    /// used here only as a decode vector.
    const KNOWN_VALID_ADDR: &str =
        "zkas:py82h42m9qjff0knpcmllzq3c7qhurje5auh4tq2ceagf69wjpf23djwwmqr26zhsua8rrglrwdltsh";

    #[test]
    fn decode_rejects_non_zkas_prefix() {
        assert!(orchard_script_bytes_from_zkas_address("kaspa:qqqq").is_none());
    }

    #[test]
    fn decode_known_valid_address() {
        let script = orchard_script_bytes_from_zkas_address(KNOWN_VALID_ADDR).expect("decode");
        assert_eq!(script.len(), 43);
    }

    #[test]
    fn mainnet_payout_accepts_valid_zkas() {
        assert!(is_valid_mainnet_zkas_payout_address(KNOWN_VALID_ADDR));
        assert!(is_valid_mainnet_zkas_payout_address(&format!(
            "  {KNOWN_VALID_ADDR}  "
        )));
    }

    #[test]
    fn mainnet_payout_rejects_junk_and_testnet() {
        assert!(!is_valid_mainnet_zkas_payout_address(""));
        assert!(!is_valid_mainnet_zkas_payout_address("zkas:x"));
        assert!(!is_valid_mainnet_zkas_payout_address("zkas:aaa"));
        assert!(!is_valid_mainnet_zkas_payout_address("zkas:zkas:pabc"));
        assert!(!is_valid_mainnet_zkas_payout_address("kaspa:qqqq"));
        // Testnet HRP is never accepted for mainnet payouts (even if payload-shaped).
        let testnetish = KNOWN_VALID_ADDR.replacen("zkas:", "zkastest:", 1);
        assert!(!is_valid_mainnet_zkas_payout_address(&testnetish));
        assert!(!is_valid_mainnet_zkas_payout_address("zkastest:x"));
    }

    #[test]
    fn a_single_flipped_character_is_rejected() {
        // Checksum must actually be verified, not just the prefix and length.
        let mut chars: Vec<char> = KNOWN_VALID_ADDR.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'h' { 'q' } else { 'h' };
        let typo: String = chars.into_iter().collect();
        assert_ne!(typo, KNOWN_VALID_ADDR);
        assert!(
            !is_valid_mainnet_zkas_payout_address(&typo),
            "a typo'd address must not validate"
        );
    }
}
