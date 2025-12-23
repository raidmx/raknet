/// Connection state management.
///
/// This module provides state tracking and shared state structures
/// for RakNet connections.

pub mod connection;
pub mod shared;
pub mod metrics;

// Re-export commonly used items
pub use connection::{ConnectionState, AtomicConnectionState};
pub use shared::SharedState;
pub use metrics::ConnectionMetrics;
