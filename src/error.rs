use std::io;
use thiserror::Error;

/// Result type for RakNet operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in RakNet operations.
#[derive(Error, Debug)]
pub enum Error {
    /// I/O error from underlying socket operations.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Invalid address provided.
    #[error("Invalid address")]
    InvalidAddress,

    /// Connection closed by remote peer.
    #[error("Connection closed")]
    ConnectionClosed,

    /// Connection timed out.
    #[error("Connection timed out")]
    Timeout,

    /// Invalid packet format or data.
    #[error("Invalid packet: {0}")]
    InvalidPacket(String),

    /// Protocol version mismatch.
    #[error("Incompatible protocol version: expected {expected}, got {got}")]
    IncompatibleProtocol { expected: u8, got: u8 },

    /// Connection refused by server.
    #[error("Connection refused: {0}")]
    ConnectionRefused(String),

    /// Server is at capacity.
    #[error("No free incoming connections")]
    ServerFull,

    /// IP address is banned.
    #[error("Connection banned")]
    Banned,

    /// Already connected to this address.
    #[error("Already connected")]
    AlreadyConnected,

    /// Invalid magic bytes in packet.
    #[error("Invalid magic bytes")]
    InvalidMagic,

    /// Listener has been closed.
    #[error("Listener closed")]
    ListenerClosed,

    /// Send queue is full (backpressure).
    #[error("Send queue full")]
    SendQueueFull,

    /// Receive window is full.
    #[error("Receive window full")]
    RecvWindowFull,

    /// Too many concurrent connections.
    #[error("Too many connections")]
    TooManyConnections,

    /// Fragment queue overflow.
    #[error("Too many incomplete fragments")]
    FragmentQueueFull,

    /// Invalid MTU size.
    #[error("Invalid MTU: {0}")]
    InvalidMtu(u16),

    /// Channel send error.
    #[error("Channel send error")]
    ChannelSend,

    /// Channel receive error.
    #[error("Channel receive error")]
    ChannelRecv,

    /// Task join error.
    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),

    /// Socket error.
    #[error("Socket error: {0}")]
    Socket(String),

    /// Generic error with custom message.
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Creates an InvalidPacket error with a message.
    pub fn invalid_packet(msg: impl Into<String>) -> Self {
        Error::InvalidPacket(msg.into())
    }

    /// Creates a Socket error with a message.
    pub fn socket(msg: impl Into<String>) -> Self {
        Error::Socket(msg.into())
    }

    /// Creates an Other error with a message.
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}

impl From<std::sync::mpsc::RecvError> for Error {
    fn from(_: std::sync::mpsc::RecvError) -> Self {
        Error::ChannelRecv
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for Error {
    fn from(_: tokio::sync::mpsc::error::SendError<T>) -> Self {
        Error::ChannelSend
    }
}
