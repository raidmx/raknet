/// Ordered channel implementation for in-order packet delivery.
///
/// RakNet supports 16 independent ordered channels, each maintaining
/// its own ordering sequence.
///
/// TODO: Implement fully in Phase 3

use bytes::Bytes;
use std::collections::BTreeMap;

/// Ordered channel for maintaining packet order.
///
/// Packets are delivered in order based on their order index.
/// Out-of-order packets are queued until gaps are filled.
#[derive(Debug)]
pub struct OrderedChannel {
    /// Next expected order index.
    next_index: u32,

    /// Queue of packets waiting for gaps to be filled.
    /// Maps order_index -> packet_data
    queue: BTreeMap<u32, Bytes>,

    /// Maximum queue size (default: 256)
    max_queue_size: usize,
}

impl OrderedChannel {
    /// Creates a new ordered channel.
    pub fn new() -> Self {
        Self {
            next_index: 0,
            queue: BTreeMap::new(),
            max_queue_size: 256,
        }
    }

    /// Inserts a packet into the channel.
    ///
    /// Returns a vector of packets that are now ready for delivery (in order).
    /// An empty vector means the packet was queued or dropped.
    pub fn insert(&mut self, order_index: u32, data: Bytes) -> Vec<Bytes> {
        let mut ready = Vec::new();

        if order_index == self.next_index {
            // This is the next expected packet
            ready.push(data);
            self.next_index = self.next_index.wrapping_add(1);

            // Check if queued packets can now be delivered
            while let Some(data) = self.queue.remove(&self.next_index) {
                ready.push(data);
                self.next_index = self.next_index.wrapping_add(1);
            }
        } else if order_index > self.next_index {
            // Future packet - queue it
            if self.queue.len() < self.max_queue_size {
                self.queue.insert(order_index, data);
            }
            // If queue is full, drop the packet
        }
        // If order_index < next_index, it's a duplicate or old packet - ignore

        ready
    }

    /// Returns the next expected order index.
    pub fn next_index(&self) -> u32 {
        self.next_index
    }

    /// Returns the number of queued packets.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Clears the channel.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.next_index = 0;
    }
}

impl Default for OrderedChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordered_channel_in_order() {
        let mut channel = OrderedChannel::new();

        let ready = channel.insert(0, Bytes::from("packet0"));
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], Bytes::from("packet0"));

        let ready = channel.insert(1, Bytes::from("packet1"));
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn test_ordered_channel_out_of_order() {
        let mut channel = OrderedChannel::new();

        // Receive packet 2 first
        let ready = channel.insert(2, Bytes::from("packet2"));
        assert_eq!(ready.len(), 0); // Queued

        // Receive packet 0
        let ready = channel.insert(0, Bytes::from("packet0"));
        assert_eq!(ready.len(), 1); // Only packet 0 ready

        // Receive packet 1 - should release 1 and 2
        let ready = channel.insert(1, Bytes::from("packet1"));
        assert_eq!(ready.len(), 2); // packet1 and packet2
        assert_eq!(ready[0], Bytes::from("packet1"));
        assert_eq!(ready[1], Bytes::from("packet2"));
    }

    #[test]
    fn test_ordered_channel_duplicate() {
        let mut channel = OrderedChannel::new();

        channel.insert(0, Bytes::from("packet0"));

        // Insert duplicate
        let ready = channel.insert(0, Bytes::from("packet0_dup"));
        assert_eq!(ready.len(), 0); // Ignored
    }
}
