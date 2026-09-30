//! Build AuxPoW witness bytes (borsh-hex) for a solved Kaspa parent bound to ZKas `H_fc`.
//!
//! Layout matches firecash/zkas-rusty `consensus/core/src/auxpow.rs` `AuxPow`:
//! `parent_header` (ZKas `Header` = stock fields + `Option::None` for `aux_pow`)
//! + `parent_coinbase` + `coinbase_merkle_branch`.

use anyhow::{Result, anyhow};
use borsh::BorshSerialize;
use kaspa_consensus_core::{block::Block, hashing, header::Header, tx::Transaction};
use kaspa_hashes::{Hash, ZERO_HASH};

/// Build the coinbase (leaf 0) Merkle inclusion branch for a multi-tx parent.
/// Matches [`kaspa_consensus_core::merkle::calc_hash_merkle_root`] / zkas-pool `merged.rs`.
pub fn coinbase_merkle_branch(txs: &[Transaction]) -> Vec<Hash> {
    if txs.len() <= 1 {
        return vec![];
    }
    let mut level: Vec<Option<Hash>> = txs.iter().map(|t| Some(hashing::tx::hash(t))).collect();
    level.resize(level.len().next_power_of_two(), None);

    let mut branch = Vec::new();
    let mut idx = 0usize;
    while level.len() > 1 {
        branch.push(level[idx ^ 1].unwrap_or(ZERO_HASH));
        let mut next = Vec::with_capacity(level.len() / 2);
        for pair in level.chunks(2) {
            let combined = pair[0].map(|l| {
                kaspa_merkle::merkle_hash(l, pair.get(1).copied().flatten().unwrap_or(ZERO_HASH))
            });
            next.push(combined);
        }
        idx /= 2;
        level = next;
    }
    branch
}

/// Encode AuxPoW as lowercase hex for ZKas `RpcBlockHeader.auxPow` / `RpcRawHeader.aux_pow`.
pub fn encode_aux_pow_hex(parent: &Block, _fc_block: &Block) -> Result<String> {
    encode_aux_pow_hex_from_parts(parent.header.as_ref(), &parent.transactions)
}

fn encode_aux_pow_hex_from_parts(header: &Header, txs: &[Transaction]) -> Result<String> {
    let coinbase = txs
        .first()
        .ok_or_else(|| anyhow!("parent missing coinbase"))?
        .clone();
    let branch = coinbase_merkle_branch(txs);

    let mut out = Vec::new();
    // Stock Header borsh, then `0` = `Option::None` for ZKas `Header.aux_pow`.
    let mut hdr = borsh::to_vec(header).map_err(|e| anyhow!("borsh parent header: {e}"))?;
    hdr.push(0u8);
    out.extend_from_slice(&hdr);
    coinbase
        .serialize(&mut out)
        .map_err(|e| anyhow!("borsh coinbase: {e}"))?;
    branch
        .serialize(&mut out)
        .map_err(|e| anyhow!("borsh branch: {e}"))?;

    Ok(hex::encode(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaspa_consensus_core::merkle::calc_hash_merkle_root;
    use kaspa_consensus_core::subnets::{SUBNETWORK_ID_COINBASE, SUBNETWORK_ID_NATIVE};
    use kaspa_consensus_core::tx::Transaction;

    fn coinbase_tx(tag: u8) -> Transaction {
        Transaction::new(
            0,
            vec![],
            vec![],
            0,
            SUBNETWORK_ID_COINBASE,
            0,
            vec![tag; 8],
        )
    }

    fn other_tx(tag: u8) -> Transaction {
        Transaction::new(0, vec![], vec![], 0, SUBNETWORK_ID_NATIVE, 0, vec![tag; 16])
    }

    #[test]
    fn coinbase_branch_folds_to_merkle_root() {
        for n in 1..=9usize {
            let mut txs = vec![coinbase_tx(1)];
            for i in 1..n {
                txs.push(other_tx(i as u8));
            }
            let root = calc_hash_merkle_root(txs.iter());
            let branch = coinbase_merkle_branch(&txs);
            let mut acc = hashing::tx::hash(&txs[0]);
            for sibling in &branch {
                acc = kaspa_merkle::merkle_hash(acc, *sibling);
            }
            assert_eq!(acc, root, "branch must reproduce merkle root for n={n}");
        }
    }

    #[test]
    fn aux_pow_hex_is_nonempty_lowercase() {
        let txs = vec![coinbase_tx(9)];
        let root = calc_hash_merkle_root(txs.iter());
        let mut header = Header::from_precomputed_hash(Hash::default(), vec![]);
        header.hash_merkle_root = root;
        let hex = encode_aux_pow_hex_from_parts(&header, &txs).expect("encode");
        assert!(!hex.is_empty());
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hex, hex.to_lowercase());
    }
}
