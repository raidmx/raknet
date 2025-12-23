/// Connection metrics and RTT tracking.
///
/// This module provides structures for tracking connection performance
/// metrics including RTT, bandwidth, and packet statistics.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Connection metrics tracked atomically.
///
/// All fields are atomic for lock-free updates from multiple tasks.
#[derive(Debug)]
pub struct ConnectionMetrics {
    /// Current round-trip time in microseconds.
    rtt: AtomicU64,

    /// RTT variance in microseconds.
    rtt_var: AtomicU64,

    /// Last time a packet was received (Unix timestamp in microseconds).
    last_recv_time: AtomicU64,

    /// Last time a packet was sent (Unix timestamp in microseconds).
    last_send_time: AtomicU64,

    /// Total bytes sent.
    bytes_sent: AtomicU64,

    /// Total bytes received.
    bytes_recv: AtomicU64,

    /// Total packets sent.
    packets_sent: AtomicU64,

    /// Total packets received.
    packets_recv: AtomicU64,
}

impl ConnectionMetrics {
    /// Creates new connection metrics with default values.
    pub fn new() -> Self {
        let now = now_micros();

        Self {
            rtt: AtomicU64::new(300_000), // 300ms initial RTT
            rtt_var: AtomicU64::new(0),
            last_recv_time: AtomicU64::new(now),
            last_send_time: AtomicU64::new(now),
            bytes_sent: AtomicU64::new(0),
            bytes_recv: AtomicU64::new(0),
            packets_sent: AtomicU64::new(0),
            packets_recv: AtomicU64::new(0),
        }
    }

    /// Gets the current RTT as a Duration.
    pub fn rtt(&self) -> Duration {
        Duration::from_micros(self.rtt.load(Ordering::Relaxed))
    }

    /// Gets the RTT variance as a Duration.
    pub fn rtt_var(&self) -> Duration {
        Duration::from_micros(self.rtt_var.load(Ordering::Relaxed))
    }

    /// Updates the RTT based on a new sample.
    ///
    /// Uses exponential weighted moving average:
    /// RTT = (7/8) * RTT + (1/8) * sample
    pub fn update_rtt(&self, sample: Duration) {
        let sample_micros = sample.as_micros() as u64;
        let old_rtt = self.rtt.load(Ordering::Relaxed);

        // EWMA: new = 0.875 * old + 0.125 * sample
        let new_rtt = (old_rtt * 7 + sample_micros) / 8;
        self.rtt.store(new_rtt, Ordering::Relaxed);

        // Update variance
        let diff = if sample_micros > old_rtt {
            sample_micros - old_rtt
        } else {
            old_rtt - sample_micros
        };

        let old_var = self.rtt_var.load(Ordering::Relaxed);
        let new_var = (old_var * 3 + diff) / 4;
        self.rtt_var.store(new_var, Ordering::Relaxed);
    }

    /// Records that a packet was sent.
    pub fn record_send(&self, bytes: usize) {
        self.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        self.last_send_time.store(now_micros(), Ordering::Release);
    }

    /// Records that a packet was received.
    pub fn record_recv(&self, bytes: usize) {
        self.bytes_recv.fetch_add(bytes as u64, Ordering::Relaxed);
        self.packets_recv.fetch_add(1, Ordering::Relaxed);
        self.last_recv_time.store(now_micros(), Ordering::Release);
    }

    /// Gets the time since the last packet was received.
    pub fn time_since_last_recv(&self) -> Duration {
        let last = self.last_recv_time.load(Ordering::Acquire);
        let now = now_micros();
        Duration::from_micros(now.saturating_sub(last))
    }

    /// Gets the time since the last packet was sent.
    pub fn time_since_last_send(&self) -> Duration {
        let last = self.last_send_time.load(Ordering::Acquire);
        let now = now_micros();
        Duration::from_micros(now.saturating_sub(last))
    }

    /// Returns a snapshot of the current metrics.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            rtt: self.rtt(),
            rtt_var: self.rtt_var(),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_recv: self.bytes_recv.load(Ordering::Relaxed),
            packets_sent: self.packets_sent.load(Ordering::Relaxed),
            packets_recv: self.packets_recv.load(Ordering::Relaxed),
        }
    }

    /// Checks if the connection has timed out.
    ///
    /// Timeout = 5 seconds + 2 * RTT
    pub fn is_timed_out(&self) -> bool {
        let timeout = Duration::from_secs(5) + self.rtt() * 2;
        self.time_since_last_recv() > timeout
    }
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of connection metrics at a point in time.
#[derive(Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    /// Current RTT.
    pub rtt: Duration,

    /// RTT variance.
    pub rtt_var: Duration,

    /// Total bytes sent.
    pub bytes_sent: u64,

    /// Total bytes received.
    pub bytes_recv: u64,

    /// Total packets sent.
    pub packets_sent: u64,

    /// Total packets received.
    pub packets_recv: u64,
}

impl MetricsSnapshot {
    /// Calculates the average packet size sent.
    pub fn avg_packet_size_sent(&self) -> f64 {
        if self.packets_sent == 0 {
            return 0.0;
        }
        self.bytes_sent as f64 / self.packets_sent as f64
    }

    /// Calculates the average packet size received.
    pub fn avg_packet_size_recv(&self) -> f64 {
        if self.packets_recv == 0 {
            return 0.0;
        }
        self.bytes_recv as f64 / self.packets_recv as f64
    }

    /// Returns the total bandwidth in bytes/sec (assumes 1 second duration).
    pub fn bandwidth_bps(&self) -> f64 {
        (self.bytes_sent + self.bytes_recv) as f64
    }
}

/// Returns the current time in microseconds since UNIX epoch.
#[inline]
pub fn now_micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_metrics_creation() {
        let metrics = ConnectionMetrics::new();
        assert_eq!(metrics.rtt(), Duration::from_millis(300));
        assert_eq!(metrics.bytes_sent.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_update_rtt() {
        let metrics = ConnectionMetrics::new();

        // Update with lower RTT
        metrics.update_rtt(Duration::from_millis(100));
        let rtt = metrics.rtt();
        assert!(rtt < Duration::from_millis(300));
        assert!(rtt > Duration::from_millis(100));
    }

    #[test]
    fn test_record_send_recv() {
        let metrics = ConnectionMetrics::new();

        metrics.record_send(100);
        assert_eq!(metrics.bytes_sent.load(Ordering::Relaxed), 100);
        assert_eq!(metrics.packets_sent.load(Ordering::Relaxed), 1);

        metrics.record_recv(200);
        assert_eq!(metrics.bytes_recv.load(Ordering::Relaxed), 200);
        assert_eq!(metrics.packets_recv.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_time_since_last() {
        let metrics = ConnectionMetrics::new();

        thread::sleep(Duration::from_millis(10));

        let since_recv = metrics.time_since_last_recv();
        assert!(since_recv >= Duration::from_millis(10));
    }

    #[test]
    fn test_snapshot() {
        let metrics = ConnectionMetrics::new();

        metrics.record_send(100);
        metrics.record_recv(200);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.bytes_sent, 100);
        assert_eq!(snapshot.bytes_recv, 200);
        assert_eq!(snapshot.packets_sent, 1);
        assert_eq!(snapshot.packets_recv, 1);
    }

    #[test]
    fn test_timeout() {
        let metrics = ConnectionMetrics::new();

        // Should not be timed out initially
        assert!(!metrics.is_timed_out());

        // Manually set last_recv_time to old value
        metrics.last_recv_time.store(0, Ordering::Release);

        // Should now be timed out
        assert!(metrics.is_timed_out());
    }
}
