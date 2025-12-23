/// Connection state management with atomic transitions.
///
/// This module provides the ConnectionState enum and atomic wrapper
/// for lock-free state transitions.

use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};

/// Connection state.
///
/// Represents the current state of a RakNet connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConnectionState {
    /// Handshake in progress.
    /// The connection is being established but not yet ready for data transfer.
    Connecting = 0,

    /// Connection is active and ready for data transfer.
    /// This is the normal operational state.
    Connected = 1,

    /// Graceful shutdown initiated.
    /// The connection is closing but waiting for final acknowledgments.
    Disconnecting = 2,

    /// Connection is closed.
    /// No further data transfer is possible.
    Disconnected = 3,
}

impl ConnectionState {
    /// Converts the state to its u8 representation.
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Creates a state from its u8 representation.
    ///
    /// Returns `None` if the value is invalid.
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Connecting),
            1 => Some(Self::Connected),
            2 => Some(Self::Disconnecting),
            3 => Some(Self::Disconnected),
            _ => None,
        }
    }

    /// Returns true if this state allows sending data.
    pub const fn can_send(self) -> bool {
        matches!(self, Self::Connected)
    }

    /// Returns true if this state allows receiving data.
    pub const fn can_receive(self) -> bool {
        matches!(self, Self::Connecting | Self::Connected)
    }

    /// Returns true if the connection is closed.
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Disconnected)
    }

    /// Returns true if the connection is active.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Connected)
    }

    /// Checks if a state transition is valid.
    pub const fn can_transition_to(self, next: ConnectionState) -> bool {
        match (self, next) {
            // Can always stay in same state
            (a, b) if a as u8 == b as u8 => true,

            // Valid transitions
            (Self::Connecting, Self::Connected) => true,
            (Self::Connecting, Self::Disconnected) => true,
            (Self::Connected, Self::Disconnecting) => true,
            (Self::Connected, Self::Disconnected) => true,
            (Self::Disconnecting, Self::Disconnected) => true,

            // Invalid transitions
            _ => false,
        }
    }
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::Connecting
    }
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connecting => write!(f, "Connecting"),
            Self::Connected => write!(f, "Connected"),
            Self::Disconnecting => write!(f, "Disconnecting"),
            Self::Disconnected => write!(f, "Disconnected"),
        }
    }
}

/// Atomic wrapper for ConnectionState.
///
/// Allows lock-free state transitions across threads.
pub struct AtomicConnectionState {
    state: AtomicU8,
}

impl AtomicConnectionState {
    /// Creates a new AtomicConnectionState.
    pub const fn new(state: ConnectionState) -> Self {
        Self {
            state: AtomicU8::new(state.to_u8()),
        }
    }

    /// Loads the current state.
    #[inline]
    pub fn load(&self, order: Ordering) -> ConnectionState {
        ConnectionState::from_u8(self.state.load(order))
            .expect("AtomicConnectionState contains invalid state")
    }

    /// Stores a new state.
    #[inline]
    pub fn store(&self, state: ConnectionState, order: Ordering) {
        self.state.store(state.to_u8(), order);
    }

    /// Swaps the state, returning the previous value.
    #[inline]
    pub fn swap(&self, state: ConnectionState, order: Ordering) -> ConnectionState {
        ConnectionState::from_u8(self.state.swap(state.to_u8(), order))
            .expect("AtomicConnectionState contained invalid state")
    }

    /// Atomically compares and exchanges the state.
    ///
    /// Returns `Ok` with the previous value if the exchange succeeded,
    /// `Err` with the current value if it failed.
    #[inline]
    pub fn compare_exchange(
        &self,
        current: ConnectionState,
        new: ConnectionState,
        success: Ordering,
        failure: Ordering,
    ) -> Result<ConnectionState, ConnectionState> {
        self.state
            .compare_exchange(current.to_u8(), new.to_u8(), success, failure)
            .map(|val| ConnectionState::from_u8(val).unwrap())
            .map_err(|val| ConnectionState::from_u8(val).unwrap())
    }

    /// Atomically compares and exchanges the state (weak version).
    ///
    /// This is the weak version that may spuriously fail.
    /// Use in a loop for better performance.
    #[inline]
    pub fn compare_exchange_weak(
        &self,
        current: ConnectionState,
        new: ConnectionState,
        success: Ordering,
        failure: Ordering,
    ) -> Result<ConnectionState, ConnectionState> {
        self.state
            .compare_exchange_weak(current.to_u8(), new.to_u8(), success, failure)
            .map(|val| ConnectionState::from_u8(val).unwrap())
            .map_err(|val| ConnectionState::from_u8(val).unwrap())
    }

    /// Attempts a validated state transition.
    ///
    /// Returns `Ok(())` if the transition was valid and successful,
    /// `Err(current_state)` if the transition is invalid or CAS failed.
    pub fn transition(
        &self,
        expected: ConnectionState,
        new: ConnectionState,
    ) -> Result<(), ConnectionState> {
        // Check if transition is valid
        if !expected.can_transition_to(new) {
            return Err(self.load(Ordering::Acquire));
        }

        // Attempt atomic transition
        self.compare_exchange(
            expected,
            new,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .map(|_| ())
    }
}

impl Default for AtomicConnectionState {
    fn default() -> Self {
        Self::new(ConnectionState::default())
    }
}

impl fmt::Debug for AtomicConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AtomicConnectionState({:?})", self.load(Ordering::Relaxed))
    }
}

/// Macro to match on connection state and execute code blocks.
#[macro_export]
macro_rules! match_state {
    ($state:expr, {
        Connecting => $connecting:expr,
        Connected => $connected:expr,
        Disconnecting => $disconnecting:expr,
        Disconnected => $disconnected:expr,
    }) => {
        match $state {
            $crate::state::ConnectionState::Connecting => $connecting,
            $crate::state::ConnectionState::Connected => $connected,
            $crate::state::ConnectionState::Disconnecting => $disconnecting,
            $crate::state::ConnectionState::Disconnected => $disconnected,
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_state_conversions() {
        assert_eq!(ConnectionState::Connecting.to_u8(), 0);
        assert_eq!(ConnectionState::Connected.to_u8(), 1);
        assert_eq!(ConnectionState::Disconnecting.to_u8(), 2);
        assert_eq!(ConnectionState::Disconnected.to_u8(), 3);

        assert_eq!(ConnectionState::from_u8(0), Some(ConnectionState::Connecting));
        assert_eq!(ConnectionState::from_u8(1), Some(ConnectionState::Connected));
        assert_eq!(ConnectionState::from_u8(2), Some(ConnectionState::Disconnecting));
        assert_eq!(ConnectionState::from_u8(3), Some(ConnectionState::Disconnected));
        assert_eq!(ConnectionState::from_u8(4), None);
    }

    #[test]
    fn test_connection_state_checks() {
        assert!(!ConnectionState::Connecting.can_send());
        assert!(ConnectionState::Connected.can_send());
        assert!(!ConnectionState::Disconnecting.can_send());
        assert!(!ConnectionState::Disconnected.can_send());

        assert!(ConnectionState::Connecting.can_receive());
        assert!(ConnectionState::Connected.can_receive());
        assert!(!ConnectionState::Disconnecting.can_receive());
        assert!(!ConnectionState::Disconnected.can_receive());

        assert!(ConnectionState::Disconnected.is_closed());
        assert!(!ConnectionState::Connected.is_closed());
    }

    #[test]
    fn test_valid_transitions() {
        // Valid transitions
        assert!(ConnectionState::Connecting.can_transition_to(ConnectionState::Connected));
        assert!(ConnectionState::Connecting.can_transition_to(ConnectionState::Disconnected));
        assert!(ConnectionState::Connected.can_transition_to(ConnectionState::Disconnecting));
        assert!(ConnectionState::Connected.can_transition_to(ConnectionState::Disconnected));
        assert!(ConnectionState::Disconnecting.can_transition_to(ConnectionState::Disconnected));

        // Invalid transitions
        assert!(!ConnectionState::Connected.can_transition_to(ConnectionState::Connecting));
        assert!(!ConnectionState::Disconnected.can_transition_to(ConnectionState::Connected));
        assert!(!ConnectionState::Disconnecting.can_transition_to(ConnectionState::Connecting));
    }

    #[test]
    fn test_atomic_connection_state() {
        let state = AtomicConnectionState::new(ConnectionState::Connecting);

        assert_eq!(state.load(Ordering::Relaxed), ConnectionState::Connecting);

        state.store(ConnectionState::Connected, Ordering::Relaxed);
        assert_eq!(state.load(Ordering::Relaxed), ConnectionState::Connected);

        let old = state.swap(ConnectionState::Disconnecting, Ordering::Relaxed);
        assert_eq!(old, ConnectionState::Connected);
        assert_eq!(state.load(Ordering::Relaxed), ConnectionState::Disconnecting);
    }

    #[test]
    fn test_atomic_compare_exchange() {
        let state = AtomicConnectionState::new(ConnectionState::Connecting);

        // Successful exchange
        let result = state.compare_exchange(
            ConnectionState::Connecting,
            ConnectionState::Connected,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        assert!(result.is_ok());
        assert_eq!(state.load(Ordering::Relaxed), ConnectionState::Connected);

        // Failed exchange (wrong current state)
        let result = state.compare_exchange(
            ConnectionState::Connecting,
            ConnectionState::Disconnected,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        assert!(result.is_err());
        assert_eq!(state.load(Ordering::Relaxed), ConnectionState::Connected);
    }

    #[test]
    fn test_validated_transition() {
        let state = AtomicConnectionState::new(ConnectionState::Connecting);

        // Valid transition
        assert!(state.transition(ConnectionState::Connecting, ConnectionState::Connected).is_ok());

        // Invalid transition
        assert!(state.transition(ConnectionState::Connected, ConnectionState::Connecting).is_err());

        // State should remain Connected
        assert_eq!(state.load(Ordering::Relaxed), ConnectionState::Connected);
    }

    #[test]
    fn test_default() {
        assert_eq!(ConnectionState::default(), ConnectionState::Connecting);
        let atomic = AtomicConnectionState::default();
        assert_eq!(atomic.load(Ordering::Relaxed), ConnectionState::Connecting);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", ConnectionState::Connecting), "Connecting");
        assert_eq!(format!("{}", ConnectionState::Connected), "Connected");
        assert_eq!(format!("{}", ConnectionState::Disconnecting), "Disconnecting");
        assert_eq!(format!("{}", ConnectionState::Disconnected), "Disconnected");
    }
}
