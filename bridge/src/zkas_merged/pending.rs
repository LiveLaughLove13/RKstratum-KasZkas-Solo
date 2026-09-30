use kaspa_consensus_core::block::Block;
use kaspa_hashes::Hash;
use std::collections::{HashMap, VecDeque};

/// Bounded map from ZKas block hash `H_fc` → full ZKas template awaiting a solved parent.
pub struct MergedPending {
    map: HashMap<Hash, Block>,
    order: VecDeque<Hash>,
    cap: usize,
}

impl MergedPending {
    pub fn new(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    pub fn insert(&mut self, h_fc: Hash, fc_block: Block) {
        if self.map.insert(h_fc, fc_block).is_none() {
            self.order.push_back(h_fc);
            while self.order.len() > self.cap {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
    }

    pub fn get(&self, h_fc: &Hash) -> Option<Block> {
        self.map.get(h_fc).cloned()
    }
}
