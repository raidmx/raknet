/// Fragmentation and reassembly for large packets.
///
/// This module will handle splitting packets that exceed the MTU
/// and reassembling them on the receiver side.
///
/// TODO: Implement in Phase 3

use bytes::Bytes;
use std::collections::{BTreeMap, HashMap};

/// Fragment queue for reassembling split packets.
///
/// Tracks incomplete fragmented packets and assembles them once all
/// fragments are received.
#[derive(Debug)]
pub struct FragmentQueue {
    /// Map of split_id -> (parts_received, total_parts, fragments)
    splits: HashMap<u16, (u32, u32, BTreeMap<u32, Bytes>)>,

    /// Maximum number of concurrent incomplete splits (default: 512)
    max_concurrent: usize,
}

impl FragmentQueue {
    /// Creates a new fragment queue with default max concurrent splits (512).
    pub fn new() -> Self {
        Self::with_capacity(512)
    }

    /// Creates a new fragment queue with the specified max concurrent splits.
    pub fn with_capacity(max_concurrent: usize) -> Self {
        Self {
            splits: HashMap::new(),
            max_concurrent,
        }
    }

    /// Inserts a fragment into the queue.
    ///
    /// Returns `Some(reassembled_data)` if all fragments have been received,
    /// `None` otherwise.
    pub fn insert(
        &mut self,
        split_id: u16,
        index: u32,
        count: u32,
        data: Bytes,
    ) -> Option<Bytes> {
        // Check if we're at capacity
        if self.splits.len() >= self.max_concurrent && !self.splits.contains_key(&split_id) {
            return None; // Drop fragment
        }

        let entry = self.splits.entry(split_id).or_insert((0, count, BTreeMap::new()));

        entry.2.insert(index, data);
        entry.0 += 1;

        // Check if complete
        if entry.0 == entry.1 {
            let (_, _, fragments) = self.splits.remove(&split_id)?;
            Some(reassemble(fragments))
        } else {
            None
        }
    }

    /// Returns the number of incomplete split packets.
    pub fn len(&self) -> usize {
        self.splits.len()
    }

    /// Returns true if there are no incomplete split packets.
    pub fn is_empty(&self) -> bool {
        self.splits.is_empty()
    }

    /// Clears all incomplete split packets.
    pub fn clear(&mut self) {
        self.splits.clear();
    }
}

impl Default for FragmentQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Reassembles fragments into a single packet.
fn reassemble(fragments: BTreeMap<u32, Bytes>) -> Bytes {
    let total_size: usize = fragments.values().map(|b| b.len()).sum();
    let mut result = Vec::with_capacity(total_size);

    for (_, fragment) in fragments {
        result.extend_from_slice(&fragment);
    }

    Bytes::from(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fragment_queue_basic() {
        let mut queue = FragmentQueue::new();

        // Insert fragments out of order
        assert!(queue.insert(1, 1, 3, Bytes::from("world")).is_none());
        assert!(queue.insert(1, 2, 3, Bytes::from("!")).is_none());

        // Last fragment completes the packet
        let result = queue.insert(1, 0, 3, Bytes::from("hello"));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Bytes::from("helloworld!"));
    }

    #[test]
    fn test_fragment_queue_multiple_splits() {
        let mut queue = FragmentQueue::new();

        queue.insert(1, 0, 2, Bytes::from("a"));
        queue.insert(2, 0, 2, Bytes::from("x"));
        queue.insert(1, 1, 2, Bytes::from("b"));

        assert_eq!(queue.len(), 1); // Split 1 complete, split 2 incomplete
    }
}
