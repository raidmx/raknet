/// Send queue for tracking unacknowledged packets.
///
/// This queue maintains packets that have been sent but not yet acknowledged.
/// It supports retransmission of lost packets based on RTT calculations.

use bytes::Bytes;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Entry in the send queue.
#[derive(Debug, Clone)]
struct QueueEntry {
    /// The packet data.
    packet: Bytes,

    /// When this packet was last sent.
    last_send_time: Instant,

    /// Number of times this packet has been retransmitted.
    retry_count: u8,
}

/// Send queue for unacknowledged packets.
///
/// Uses BTreeMap for efficient sequential access and range operations.
/// Tracks packets by their sequence number.
pub struct SendQueue {
    /// Map of sequence number to queue entry.
    packets: BTreeMap<u32, QueueEntry>,

    /// Maximum window size (default: 2048).
    /// Prevents unbounded memory growth.
    max_size: usize,

    /// Total number of packets sent.
    total_sent: u64,

    /// Total number of packets acknowledged.
    total_acked: u64,

    /// Total number of packets retransmitted.
    total_retransmitted: u64,
}

impl SendQueue {
    /// Creates a new send queue with the default max size (2048).
    pub fn new() -> Self {
        Self::with_capacity(2048)
    }

    /// Creates a new send queue with the specified max size.
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            packets: BTreeMap::new(),
            max_size,
            total_sent: 0,
            total_acked: 0,
            total_retransmitted: 0,
        }
    }

    /// Inserts a packet into the queue.
    ///
    /// Returns `true` if the packet was inserted, `false` if the queue is full.
    pub fn insert(&mut self, seq: u32, packet: Bytes) -> bool {
        if self.packets.len() >= self.max_size {
            return false; // Backpressure
        }

        self.packets.insert(seq, QueueEntry {
            packet,
            last_send_time: Instant::now(),
            retry_count: 0,
        });

        self.total_sent += 1;
        true
    }

    /// Acknowledges a packet by sequence number.
    ///
    /// Returns the packet if it was in the queue, `None` otherwise.
    pub fn acknowledge(&mut self, seq: u32) -> Option<Bytes> {
        let entry = self.packets.remove(&seq)?;
        self.total_acked += 1;
        Some(entry.packet)
    }

    /// Acknowledges multiple packets by sequence number range.
    ///
    /// Returns the number of packets acknowledged.
    pub fn acknowledge_range(&mut self, start: u32, end: u32) -> usize {
        let mut count = 0;

        // Handle wraparound by acknowledging in two parts if necessary
        if start <= end {
            // Normal range
            let to_remove: Vec<u32> = self.packets
                .range(start..=end)
                .map(|(&seq, _)| seq)
                .collect();

            for seq in to_remove {
                self.packets.remove(&seq);
                self.total_acked += 1;
                count += 1;
            }
        } else {
            // Wraparound range: start..MAX and 0..=end
            let to_remove_high: Vec<u32> = self.packets
                .range(start..)
                .map(|(&seq, _)| seq)
                .collect();

            let to_remove_low: Vec<u32> = self.packets
                .range(..=end)
                .map(|(&seq, _)| seq)
                .collect();

            for seq in to_remove_high.into_iter().chain(to_remove_low) {
                self.packets.remove(&seq);
                self.total_acked += 1;
                count += 1;
            }
        }

        count
    }

    /// Gets packets that need retransmission.
    ///
    /// Returns a vector of (sequence, packet) tuples for packets that haven't
    /// been acknowledged within the timeout period.
    ///
    /// # Arguments
    ///
    /// * `rtt` - Current round-trip time
    /// * `timeout_multiplier` - Multiplier for timeout calculation (default: 3)
    pub fn get_expired(&mut self, rtt: Duration, timeout_multiplier: u32) -> Vec<(u32, Bytes)> {
        let now = Instant::now();
        let timeout = rtt * timeout_multiplier;

        let mut expired = Vec::new();

        for (&seq, entry) in self.packets.iter_mut() {
            if now.duration_since(entry.last_send_time) > timeout {
                // Mark as retransmitted
                entry.last_send_time = now;
                entry.retry_count = entry.retry_count.saturating_add(1);
                self.total_retransmitted += 1;

                expired.push((seq, entry.packet.clone()));
            }
        }

        expired
    }

    /// Checks if a packet is in the queue.
    pub fn contains(&self, seq: u32) -> bool {
        self.packets.contains_key(&seq)
    }

    /// Returns the number of packets in the queue.
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// Returns true if the queue is full.
    pub fn is_full(&self) -> bool {
        self.packets.len() >= self.max_size
    }

    /// Returns the current window size (number of unacknowledged packets).
    pub fn window_size(&self) -> usize {
        self.packets.len()
    }

    /// Returns statistics about the send queue.
    pub fn stats(&self) -> SendQueueStats {
        SendQueueStats {
            total_sent: self.total_sent,
            total_acked: self.total_acked,
            total_retransmitted: self.total_retransmitted,
            current_queue_size: self.packets.len(),
            max_size: self.max_size,
        }
    }

    /// Clears all packets from the queue.
    pub fn clear(&mut self) {
        self.packets.clear();
    }
}

impl Default for SendQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the send queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendQueueStats {
    /// Total number of packets sent.
    pub total_sent: u64,

    /// Total number of packets acknowledged.
    pub total_acked: u64,

    /// Total number of packets retransmitted.
    pub total_retransmitted: u64,

    /// Current queue size.
    pub current_queue_size: usize,

    /// Maximum queue size.
    pub max_size: usize,
}

impl SendQueueStats {
    /// Returns the packet loss rate (0.0 to 1.0).
    pub fn loss_rate(&self) -> f64 {
        if self.total_sent == 0 {
            return 0.0;
        }
        self.total_retransmitted as f64 / self.total_sent as f64
    }

    /// Returns the queue utilization (0.0 to 1.0).
    pub fn utilization(&self) -> f64 {
        self.current_queue_size as f64 / self.max_size as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_queue_insert_acknowledge() {
        let mut queue = SendQueue::new();

        let packet = Bytes::from("test");
        assert!(queue.insert(0, packet.clone()));
        assert_eq!(queue.len(), 1);
        assert!(queue.contains(0));

        let acked = queue.acknowledge(0).unwrap();
        assert_eq!(acked, packet);
        assert_eq!(queue.len(), 0);
        assert!(!queue.contains(0));
    }

    #[test]
    fn test_send_queue_acknowledge_range() {
        let mut queue = SendQueue::new();

        for i in 0..10 {
            queue.insert(i, Bytes::from(vec![i as u8]));
        }

        assert_eq!(queue.len(), 10);

        // Acknowledge range 2-5
        let count = queue.acknowledge_range(2, 5);
        assert_eq!(count, 4); // 2, 3, 4, 5
        assert_eq!(queue.len(), 6);

        // Remaining: 0, 1, 6, 7, 8, 9
        assert!(queue.contains(0));
        assert!(queue.contains(1));
        assert!(!queue.contains(2));
        assert!(!queue.contains(5));
        assert!(queue.contains(6));
    }

    #[test]
    fn test_send_queue_expired() {
        let mut queue = SendQueue::new();

        let packet = Bytes::from("test");
        queue.insert(0, packet.clone());

        // Immediately check - should not be expired
        let expired = queue.get_expired(Duration::from_millis(100), 3);
        assert_eq!(expired.len(), 0);

        // Wait a bit
        std::thread::sleep(Duration::from_millis(50));

        // Still not expired (RTT * 3 = 300ms)
        let expired = queue.get_expired(Duration::from_millis(100), 3);
        assert_eq!(expired.len(), 0);

        // Use very low RTT to trigger expiration
        let expired = queue.get_expired(Duration::from_millis(1), 3);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].0, 0);
        assert_eq!(expired[0].1, packet);
    }

    #[test]
    fn test_send_queue_full() {
        let mut queue = SendQueue::with_capacity(5);

        for i in 0..5 {
            assert!(queue.insert(i, Bytes::from(vec![i as u8])));
        }

        assert!(queue.is_full());
        assert!(!queue.insert(5, Bytes::from(vec![5])));
    }

    #[test]
    fn test_send_queue_stats() {
        let mut queue = SendQueue::new();

        queue.insert(0, Bytes::from("test"));
        queue.insert(1, Bytes::from("test"));
        queue.acknowledge(0);

        let stats = queue.stats();
        assert_eq!(stats.total_sent, 2);
        assert_eq!(stats.total_acked, 1);
        assert_eq!(stats.current_queue_size, 1);
    }

    #[test]
    fn test_send_queue_wraparound_range() {
        let mut queue = SendQueue::new();

        // Insert packets near wraparound boundary
        let max_u24 = 0x00FFFFFF;
        queue.insert(max_u24 - 2, Bytes::from("test1"));
        queue.insert(max_u24 - 1, Bytes::from("test2"));
        queue.insert(max_u24, Bytes::from("test3"));
        queue.insert(0, Bytes::from("test4"));
        queue.insert(1, Bytes::from("test5"));
        queue.insert(2, Bytes::from("test6"));

        // Acknowledge wraparound range: (max_u24 - 1) to 1
        let count = queue.acknowledge_range(max_u24 - 1, 1);

        // Should acknowledge: max_u24-1, max_u24, 0, 1 (4 packets)
        assert_eq!(count, 4);

        // Remaining: max_u24-2, 2
        assert!(queue.contains(max_u24 - 2));
        assert!(!queue.contains(max_u24 - 1));
        assert!(!queue.contains(max_u24));
        assert!(!queue.contains(0));
        assert!(!queue.contains(1));
        assert!(queue.contains(2));
    }
}
