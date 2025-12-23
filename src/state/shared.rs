/// Shared connection state.
///
/// This structure contains all the state for a RakNet connection,
/// wrapped in Arc for sharing across tasks. Uses atomic operations
/// where possible for lock-free access.

use crate::protocol::{AtomicU24, u24};
use crate::reliability::{
    SendQueue, RecvWindow, FragmentQueue, OrderedChannel, AckRangeList,
};
use crate::state::{AtomicConnectionState, ConnectionState, ConnectionMetrics};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

/// Shared state for a RakNet connection.
///
/// This structure is wrapped in Arc and shared between the receive task,
/// tick task, and the application-facing stream.
///
/// Design philosophy:
/// - Atomics for simple counters and flags (lock-free)
/// - Mutex for complex data structures (minimal lock time)
/// - Separate mutexes to reduce contention
pub struct SharedState {
    // === Atomic state (lock-free) ===

    /// Connection state.
    pub state: AtomicConnectionState,

    /// Next datagram sequence number to send.
    pub send_seq: AtomicU24,

    /// Current split packet ID counter.
    pub split_id: AtomicU16,

    /// Current MTU size.
    pub mtu: AtomicU16,

    /// Connection metrics (RTT, bandwidth, etc.).
    pub metrics: ConnectionMetrics,

    // === Mutexed state (requires locking) ===

    /// Send queue for tracking unacknowledged packets.
    pub send_queue: Mutex<SendQueue>,

    /// Receive window for duplicate detection.
    pub recv_window: Mutex<RecvWindow>,

    /// Fragment queue for reassembling split packets.
    pub fragment_queue: Mutex<FragmentQueue>,

    /// Ordered channels (16 channels, 0-15).
    pub ordered_channels: [Mutex<OrderedChannel>; 16],

    /// Pending ACKs to send.
    pub pending_acks: Mutex<AckRangeList>,

    /// Pending NACKs to send.
    pub pending_nacks: Mutex<AckRangeList>,

    // === Index counters (atomic) ===

    /// Next message index for reliable packets.
    pub message_index: AtomicU24,

    /// Next order index per channel (one atomic per channel).
    pub order_indices: [AtomicU24; 16],

    /// Next sequence index for sequenced packets.
    pub sequence_index: AtomicU24,
}

impl SharedState {
    /// Creates a new shared state with the given MTU.
    pub fn new(mtu: u16) -> Arc<Self> {
        Arc::new(Self {
            // Atomic state
            state: AtomicConnectionState::new(ConnectionState::Connecting),
            send_seq: AtomicU24::new(0),
            split_id: AtomicU16::new(0),
            mtu: AtomicU16::new(mtu),
            metrics: ConnectionMetrics::new(),

            // Mutexed structures
            send_queue: Mutex::new(SendQueue::new()),
            recv_window: Mutex::new(RecvWindow::new()),
            fragment_queue: Mutex::new(FragmentQueue::new()),
            ordered_channels: array_init::array_init(|_| Mutex::new(OrderedChannel::new())),
            pending_acks: Mutex::new(AckRangeList::new()),
            pending_nacks: Mutex::new(AckRangeList::new()),

            // Index counters
            message_index: AtomicU24::new(0),
            order_indices: array_init::array_init(|_| AtomicU24::new(0)),
            sequence_index: AtomicU24::new(0),
        })
    }

    /// Allocates the next datagram sequence number.
    pub fn next_send_seq(&self) -> u24 {
        self.send_seq.fetch_add(1, Ordering::AcqRel)
    }

    /// Allocates the next message index (for reliable packets).
    pub fn next_message_index(&self) -> u24 {
        self.message_index.fetch_add(1, Ordering::AcqRel)
    }

    /// Allocates the next order index for a specific channel.
    pub fn next_order_index(&self, channel: u8) -> u24 {
        debug_assert!(channel < 16, "Order channel must be 0-15");
        self.order_indices[channel as usize].fetch_add(1, Ordering::AcqRel)
    }

    /// Allocates the next sequence index (for sequenced packets).
    pub fn next_sequence_index(&self) -> u24 {
        self.sequence_index.fetch_add(1, Ordering::AcqRel)
    }

    /// Allocates the next split packet ID (for fragmentation).
    pub fn next_split_id(&self) -> u16 {
        self.split_id.fetch_add(1, Ordering::AcqRel)
    }

    /// Gets the current MTU.
    pub fn mtu(&self) -> u16 {
        self.mtu.load(Ordering::Relaxed)
    }

    /// Sets the MTU.
    pub fn set_mtu(&self, mtu: u16) {
        self.mtu.store(mtu, Ordering::Relaxed);
    }

    /// Gets the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        self.state.load(Ordering::Acquire)
    }

    /// Checks if the connection is active.
    pub fn is_connected(&self) -> bool {
        self.connection_state() == ConnectionState::Connected
    }

    /// Checks if the connection is closed.
    pub fn is_closed(&self) -> bool {
        self.connection_state().is_closed()
    }

    /// Transitions to connected state.
    pub fn mark_connected(&self) -> bool {
        self.state.transition(ConnectionState::Connecting, ConnectionState::Connected).is_ok()
    }

    /// Initiates graceful shutdown.
    pub fn mark_disconnecting(&self) -> bool {
        self.state.transition(ConnectionState::Connected, ConnectionState::Disconnecting).is_ok()
    }

    /// Marks the connection as disconnected.
    pub fn mark_disconnected(&self) {
        self.state.store(ConnectionState::Disconnected, Ordering::Release);
    }

    /// Checks if the connection has timed out.
    pub fn is_timed_out(&self) -> bool {
        self.metrics.is_timed_out()
    }

    /// Returns statistics about the send queue.
    pub fn send_queue_stats(&self) -> crate::reliability::send_queue::SendQueueStats {
        self.send_queue.lock().stats()
    }

    /// Returns statistics about the receive window.
    pub fn recv_window_stats(&self) -> crate::reliability::recv_window::RecvWindowStats {
        self.recv_window.lock().stats()
    }

    /// Returns a snapshot of connection metrics.
    pub fn metrics_snapshot(&self) -> crate::state::metrics::MetricsSnapshot {
        self.metrics.snapshot()
    }
}

impl std::fmt::Debug for SharedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedState")
            .field("state", &self.connection_state())
            .field("send_seq", &self.send_seq.load(Ordering::Relaxed))
            .field("mtu", &self.mtu())
            .field("metrics", &self.metrics_snapshot())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_state_creation() {
        let state = SharedState::new(1400);

        assert_eq!(state.connection_state(), ConnectionState::Connecting);
        assert_eq!(state.mtu(), 1400);
        assert!(!state.is_connected());
        assert!(!state.is_closed());
    }

    #[test]
    fn test_sequence_allocation() {
        let state = SharedState::new(1400);

        assert_eq!(state.next_send_seq(), u24::new(0));
        assert_eq!(state.next_send_seq(), u24::new(1));
        assert_eq!(state.next_send_seq(), u24::new(2));
    }

    #[test]
    fn test_message_index_allocation() {
        let state = SharedState::new(1400);

        assert_eq!(state.next_message_index(), u24::new(0));
        assert_eq!(state.next_message_index(), u24::new(1));
    }

    #[test]
    fn test_order_index_allocation() {
        let state = SharedState::new(1400);

        // Different channels have independent indices
        assert_eq!(state.next_order_index(0), u24::new(0));
        assert_eq!(state.next_order_index(1), u24::new(0));
        assert_eq!(state.next_order_index(0), u24::new(1));
        assert_eq!(state.next_order_index(1), u24::new(1));
    }

    #[test]
    fn test_state_transitions() {
        let state = SharedState::new(1400);

        assert!(state.mark_connected());
        assert!(state.is_connected());

        assert!(state.mark_disconnecting());
        assert_eq!(state.connection_state(), ConnectionState::Disconnecting);

        state.mark_disconnected();
        assert!(state.is_closed());
    }

    #[test]
    fn test_invalid_state_transition() {
        let state = SharedState::new(1400);

        state.mark_connected();

        // Cannot go back to connecting
        let result = state.state.transition(
            ConnectionState::Connected,
            ConnectionState::Connecting
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_mtu() {
        let state = SharedState::new(1400);

        assert_eq!(state.mtu(), 1400);

        state.set_mtu(1200);
        assert_eq!(state.mtu(), 1200);
    }

    #[test]
    fn test_split_id_allocation() {
        let state = SharedState::new(1400);

        assert_eq!(state.next_split_id(), 0);
        assert_eq!(state.next_split_id(), 1);
        assert_eq!(state.next_split_id(), 2);
    }
}
