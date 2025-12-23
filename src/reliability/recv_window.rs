/// Receive window for duplicate detection and gap tracking.
///
/// Uses a BitVec for O(1) duplicate detection and efficient memory usage.
/// Maintains a sliding window of received sequence numbers.

use bitvec::prelude::*;
use crate::protocol::seq_less_than;

/// Receive window for tracking received datagrams.
///
/// Uses a fixed-size sliding window with a BitVec for fast duplicate detection.
/// The window slides forward as packets are received, maintaining a view of
/// the most recent packets.
pub struct RecvWindow {
    /// BitVec tracking which sequence numbers have been received.
    /// Index 0 corresponds to base_seq, index 1 to base_seq+1, etc.
    received: BitVec,

    /// The base (lowest) sequence number in the window.
    base_seq: u32,

    /// Maximum window size (default: 2048).
    max_size: usize,

    /// Highest sequence number seen so far.
    highest_seq: u32,

    /// Total number of packets received.
    total_received: u64,

    /// Total number of duplicate packets detected.
    total_duplicates: u64,

    /// Total number of out-of-order packets.
    total_out_of_order: u64,
}

impl RecvWindow {
    /// Creates a new receive window with the default max size (2048).
    pub fn new() -> Self {
        Self::with_capacity(2048)
    }

    /// Creates a new receive window with the specified max size.
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            received: bitvec![0; max_size],
            base_seq: 0,
            max_size,
            highest_seq: 0,
            total_received: 0,
            total_duplicates: 0,
            total_out_of_order: 0,
        }
    }

    /// Marks a sequence number as received.
    ///
    /// Returns:
    /// - `Some(false)` if this is a duplicate packet
    /// - `Some(true)` if this is a new packet within the window
    /// - `None` if the packet is outside the window (too old)
    pub fn mark_received(&mut self, seq: u32) -> Option<bool> {
        // Check if packet is too old (before window base)
        if seq_less_than(seq, self.base_seq) {
            return None; // Too old, outside window
        }

        // Calculate offset from base
        let offset = seq.wrapping_sub(self.base_seq) as usize;

        // Check if packet is beyond window
        if offset >= self.max_size {
            // Need to slide window forward
            self.slide_to(seq);
            // After sliding, packet should be in window
            let new_offset = seq.wrapping_sub(self.base_seq) as usize;
            if new_offset >= self.max_size {
                return None; // Still outside window (shouldn't happen)
            }
            self.received.set(new_offset, true);
            self.total_received += 1;

            // Update highest seen
            if seq_less_than(self.highest_seq, seq) {
                self.highest_seq = seq;
            }

            return Some(true);
        }

        // Check if already received (duplicate)
        if self.received[offset] {
            self.total_duplicates += 1;
            return Some(false); // Duplicate
        }

        // Mark as received
        self.received.set(offset, true);
        self.total_received += 1;

        // Check if out of order
        if seq != self.highest_seq.wrapping_add(1) && self.total_received > 1 {
            self.total_out_of_order += 1;
        }

        // Update highest seen
        if seq_less_than(self.highest_seq, seq) || self.total_received == 1 {
            self.highest_seq = seq;
        }

        Some(true)
    }

    /// Checks if a sequence number has been received.
    pub fn has_received(&self, seq: u32) -> bool {
        if seq_less_than(seq, self.base_seq) {
            return false; // Before window
        }

        let offset = seq.wrapping_sub(self.base_seq) as usize;
        if offset >= self.max_size {
            return false; // Beyond window
        }

        self.received[offset]
    }

    /// Gets missing sequence numbers in the current window.
    ///
    /// Returns a vector of sequence numbers that haven't been received yet
    /// but are within the range [base_seq, highest_seq].
    pub fn get_missing(&self) -> Vec<u32> {
        let mut missing = Vec::new();

        if self.highest_seq == self.base_seq && self.total_received == 0 {
            return missing; // No packets received yet
        }

        let end_offset = self.highest_seq.wrapping_sub(self.base_seq) as usize;
        let end_offset = end_offset.min(self.max_size - 1);

        for offset in 0..=end_offset {
            if !self.received[offset] {
                let seq = self.base_seq.wrapping_add(offset as u32);
                missing.push(seq);
            }
        }

        missing
    }

    /// Slides the window to a new base sequence number.
    ///
    /// This is called when a packet is received that's beyond the current window.
    fn slide_to(&mut self, new_seq: u32) {
        let shift = new_seq.wrapping_sub(self.base_seq) as usize;

        if shift == 0 {
            return; // No sliding needed
        }

        if shift >= self.max_size {
            // Complete reset - new window doesn't overlap old one
            self.received.fill(false);
            self.base_seq = new_seq.wrapping_sub((self.max_size / 2) as u32);
        } else {
            // Partial slide
            self.received.copy_within(shift.., 0);
            self.received[self.max_size - shift..].fill(false);
            self.base_seq = self.base_seq.wrapping_add(shift as u32);
        }
    }

    /// Advances the window base to skip over contiguous received packets.
    ///
    /// This is useful for sliding the window forward once all packets up to
    /// a certain point have been received.
    pub fn advance_base(&mut self) {
        // Find how many contiguous packets from the base have been received
        let mut shift = 0;
        for offset in 0..self.max_size {
            if !self.received[offset] {
                break;
            }
            shift = offset + 1;
        }

        if shift > 0 {
            self.slide_to(self.base_seq.wrapping_add(shift as u32));
        }
    }

    /// Returns the current base (lowest) sequence number in the window.
    pub fn base(&self) -> u32 {
        self.base_seq
    }

    /// Returns the highest sequence number seen so far.
    pub fn highest(&self) -> u32 {
        self.highest_seq
    }

    /// Returns statistics about the receive window.
    pub fn stats(&self) -> RecvWindowStats {
        RecvWindowStats {
            total_received: self.total_received,
            total_duplicates: self.total_duplicates,
            total_out_of_order: self.total_out_of_order,
            base_seq: self.base_seq,
            highest_seq: self.highest_seq,
            window_size: self.max_size,
        }
    }

    /// Clears the receive window.
    pub fn clear(&mut self) {
        self.received.fill(false);
        self.base_seq = 0;
        self.highest_seq = 0;
        self.total_received = 0;
        self.total_duplicates = 0;
        self.total_out_of_order = 0;
    }
}

impl Default for RecvWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the receive window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecvWindowStats {
    /// Total number of packets received.
    pub total_received: u64,

    /// Total number of duplicate packets detected.
    pub total_duplicates: u64,

    /// Total number of out-of-order packets.
    pub total_out_of_order: u64,

    /// Current base sequence number.
    pub base_seq: u32,

    /// Highest sequence number seen.
    pub highest_seq: u32,

    /// Window size.
    pub window_size: usize,
}

impl RecvWindowStats {
    /// Returns the duplicate rate (0.0 to 1.0).
    pub fn duplicate_rate(&self) -> f64 {
        if self.total_received == 0 {
            return 0.0;
        }
        self.total_duplicates as f64 / self.total_received as f64
    }

    /// Returns the out-of-order rate (0.0 to 1.0).
    pub fn out_of_order_rate(&self) -> f64 {
        if self.total_received == 0 {
            return 0.0;
        }
        self.total_out_of_order as f64 / self.total_received as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recv_window_basic() {
        let mut window = RecvWindow::new();

        // First packet
        assert_eq!(window.mark_received(0), Some(true));
        assert!(window.has_received(0));
        assert_eq!(window.base(), 0);
        assert_eq!(window.highest(), 0);

        // Second packet
        assert_eq!(window.mark_received(1), Some(true));
        assert!(window.has_received(1));
        assert_eq!(window.highest(), 1);

        // Duplicate
        assert_eq!(window.mark_received(1), Some(false));
        assert_eq!(window.stats().total_duplicates, 1);
    }

    #[test]
    fn test_recv_window_out_of_order() {
        let mut window = RecvWindow::new();

        assert_eq!(window.mark_received(0), Some(true));
        assert_eq!(window.mark_received(5), Some(true));
        assert_eq!(window.mark_received(3), Some(true));

        assert!(window.has_received(0));
        assert!(!window.has_received(1));
        assert!(!window.has_received(2));
        assert!(window.has_received(3));
        assert!(!window.has_received(4));
        assert!(window.has_received(5));
    }

    #[test]
    fn test_recv_window_missing() {
        let mut window = RecvWindow::new();

        window.mark_received(0);
        window.mark_received(2);
        window.mark_received(4);
        window.mark_received(5);

        let missing = window.get_missing();
        assert_eq!(missing, vec![1, 3]);
    }

    #[test]
    fn test_recv_window_advance_base() {
        let mut window = RecvWindow::new();

        // Receive packets 0-4
        for i in 0..5 {
            window.mark_received(i);
        }

        // Base should still be 0
        assert_eq!(window.base(), 0);

        // Advance base (should skip all contiguous received packets)
        window.advance_base();

        // Base should now be 5 (after all received packets)
        assert_eq!(window.base(), 5);

        // Old packets should be outside window now
        assert!(!window.has_received(0));
    }

    #[test]
    fn test_recv_window_slide() {
        let mut window = RecvWindow::with_capacity(10);

        window.mark_received(0);
        window.mark_received(1);

        // Jump far ahead (beyond window)
        assert_eq!(window.mark_received(15), Some(true));

        // Old packets should be outside window
        assert!(!window.has_received(0));
        assert!(!window.has_received(1));

        // New packet should be in window
        assert!(window.has_received(15));
    }

    #[test]
    fn test_recv_window_wraparound() {
        let mut window = RecvWindow::with_capacity(10);

        let max_u24 = 0x00FFFFFF;

        // Receive packets near wraparound
        window.mark_received(max_u24 - 2);
        window.mark_received(max_u24 - 1);
        window.mark_received(max_u24);
        window.mark_received(0);
        window.mark_received(1);

        assert!(window.has_received(max_u24 - 2));
        assert!(window.has_received(max_u24 - 1));
        assert!(window.has_received(max_u24));
        assert!(window.has_received(0));
        assert!(window.has_received(1));
    }

    #[test]
    fn test_recv_window_stats() {
        let mut window = RecvWindow::new();

        window.mark_received(0);
        window.mark_received(1);
        window.mark_received(1); // Duplicate
        window.mark_received(5); // Out of order

        let stats = window.stats();
        assert_eq!(stats.total_received, 3); // 0, 1, 5
        assert_eq!(stats.total_duplicates, 1);
        assert_eq!(stats.total_out_of_order, 1);
    }
}
