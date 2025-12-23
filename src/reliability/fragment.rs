/// Fragmentation and reassembly for large packets.
///
/// This module handles splitting packets that exceed the MTU
/// and reassembling them on the receiver side.

use bytes::Bytes;
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

/// Fragment queue for reassembling split packets.
///
/// Tracks incomplete fragmented packets and assembles them once all
/// fragments are received. Includes timeout mechanism to prevent
/// memory leaks from incomplete packets.
#[derive(Debug)]
pub struct FragmentQueue {
    /// Map of split_id -> FragmentEntry
    splits: HashMap<u16, FragmentEntry>,

    /// Maximum number of concurrent incomplete splits (default: 512)
    max_concurrent: usize,

    /// Timeout for incomplete fragments (default: 8 seconds)
    timeout: Duration,
}

/// Entry for a single fragmented packet being reassembled.
#[derive(Debug)]
struct FragmentEntry {
    /// Total number of fragments expected
    total_count: u32,

    /// Fragments received so far (index -> data)
    fragments: BTreeMap<u32, Bytes>,

    /// Time when first fragment was received
    first_received: Instant,
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
            timeout: Duration::from_secs(8),
        }
    }

    /// Sets the timeout for incomplete fragments.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Inserts a fragment into the queue.
    ///
    /// Returns `Some(reassembled_data)` if all fragments have been received,
    /// `None` otherwise.
    ///
    /// # Arguments
    ///
    /// * `split_id` - Unique identifier for this fragmented packet
    /// * `index` - Index of this fragment (0 to count-1)
    /// * `count` - Total number of fragments
    /// * `data` - Fragment data
    pub fn insert(
        &mut self,
        split_id: u16,
        index: u32,
        count: u32,
        data: Bytes,
    ) -> Option<Bytes> {
        // Validate fragment index
        if index >= count {
            return None; // Invalid fragment
        }

        // Check if we're at capacity
        if self.splits.len() >= self.max_concurrent && !self.splits.contains_key(&split_id) {
            return None; // Drop fragment - queue full
        }

        // Get or create entry
        let entry = self.splits.entry(split_id).or_insert_with(|| FragmentEntry {
            total_count: count,
            fragments: BTreeMap::new(),
            first_received: Instant::now(),
        });

        // Verify total count matches (all fragments should have same count)
        if entry.total_count != count {
            // Mismatch - remove corrupted entry
            self.splits.remove(&split_id);
            return None;
        }

        // Check for duplicate fragment
        if entry.fragments.contains_key(&index) {
            return None; // Duplicate, ignore
        }

        // Insert fragment
        entry.fragments.insert(index, data);

        // Check if complete
        if entry.fragments.len() == count as usize {
            // All fragments received - reassemble
            if let Some(entry) = self.splits.remove(&split_id) {
                return Some(reassemble(entry.fragments));
            }
        }

        None
    }

    /// Cleans up expired incomplete fragments.
    ///
    /// Returns the number of fragments cleaned up.
    pub fn cleanup_expired(&mut self) -> usize {
        let now = Instant::now();
        let timeout = self.timeout;

        let expired: Vec<u16> = self
            .splits
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.first_received) > timeout)
            .map(|(id, _)| *id)
            .collect();

        let count = expired.len();
        for id in expired {
            self.splits.remove(&id);
        }

        count
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

    #[test]
    fn test_duplicate_fragments() {
        let mut queue = FragmentQueue::new();

        assert!(queue.insert(1, 0, 2, Bytes::from("hello")).is_none());
        assert!(queue.insert(1, 0, 2, Bytes::from("duplicate")).is_none()); // Duplicate ignored

        let result = queue.insert(1, 1, 2, Bytes::from("world"));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Bytes::from("helloworld"));
    }

    #[test]
    fn test_invalid_fragment_index() {
        let mut queue = FragmentQueue::new();

        // Index >= count is invalid
        assert!(queue.insert(1, 5, 3, Bytes::from("invalid")).is_none());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_count_mismatch() {
        let mut queue = FragmentQueue::new();

        queue.insert(1, 0, 3, Bytes::from("a"));
        // Different count for same split_id - should reject
        assert!(queue.insert(1, 1, 5, Bytes::from("b")).is_none());
        assert_eq!(queue.len(), 0); // Entry removed due to mismatch
    }

    #[test]
    fn test_cleanup_expired() {
        let mut queue = FragmentQueue::new();
        queue.set_timeout(Duration::from_millis(100));

        queue.insert(1, 0, 2, Bytes::from("a"));
        assert_eq!(queue.len(), 1);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));

        let cleaned = queue.cleanup_expired();
        assert_eq!(cleaned, 1);
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_capacity_limit() {
        let mut queue = FragmentQueue::with_capacity(2);

        queue.insert(1, 0, 2, Bytes::from("a"));
        queue.insert(2, 0, 2, Bytes::from("b"));

        // At capacity - new split_id should be rejected
        assert!(queue.insert(3, 0, 2, Bytes::from("c")).is_none());
        assert_eq!(queue.len(), 2);

        // But can still add to existing split_id
        assert!(queue.insert(1, 1, 2, Bytes::from("x")).is_some());
    }
}
