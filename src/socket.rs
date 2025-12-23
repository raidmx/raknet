use crate::error::{Error, Result};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// Creates a listener socket with SO_REUSEPORT enabled.
///
/// This socket is used by the RakNet listener to receive unconnected packets
/// (ping, handshake) from clients.
///
/// SO_REUSEPORT allows multiple sockets to bind to the same port, enabling
/// kernel-level load balancing across sockets based on the 4-tuple
/// (src_ip, src_port, dst_ip, dst_port).
pub fn create_listener_socket(addr: SocketAddr) -> Result<std::net::UdpSocket> {
    let domain = if addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };

    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| Error::socket(format!("Failed to create socket: {}", e)))?;

    // Enable SO_REUSEPORT on Unix platforms
    #[cfg(unix)]
    {
        socket
            .set_reuse_port(true)
            .map_err(|e| Error::socket(format!("Failed to set SO_REUSEPORT: {}", e)))?;
    }

    // Enable SO_REUSEADDR (useful for quick restart)
    socket
        .set_reuse_address(true)
        .map_err(|e| Error::socket(format!("Failed to set SO_REUSEADDR: {}", e)))?;

    // Set non-blocking mode
    socket
        .set_nonblocking(true)
        .map_err(|e| Error::socket(format!("Failed to set non-blocking: {}", e)))?;

    // Bind to the address
    socket
        .bind(&addr.into())
        .map_err(|e| Error::socket(format!("Failed to bind to {}: {}", addr, e)))?;

    Ok(socket.into())
}

/// Creates a session socket connected to a specific remote address.
///
/// This socket is used for an individual RakNet connection after the handshake
/// is complete. It binds to the same port as the listener (using SO_REUSEPORT)
/// and then connects to the remote peer.
///
/// Once connected, the kernel will automatically route packets from this
/// specific remote address to this socket, rather than the listener socket.
/// This enables parallel packet processing across multiple connections.
///
/// # Arguments
///
/// * `listen_addr` - The local address to bind to (same as listener)
/// * `remote_addr` - The remote peer address to connect to
pub fn create_session_socket(
    listen_addr: SocketAddr,
    remote_addr: SocketAddr,
) -> Result<std::net::UdpSocket> {
    // Create socket with SO_REUSEPORT on the same port as listener
    let socket = create_listener_socket(listen_addr)?;

    // Connect to the remote address
    // This establishes the 4-tuple for kernel demultiplexing
    socket
        .connect(remote_addr)
        .map_err(|e| Error::socket(format!("Failed to connect to {}: {}", remote_addr, e)))?;

    Ok(socket)
}

/// Creates a Tokio UDP socket from a std UDP socket.
pub fn to_tokio_socket(std_socket: std::net::UdpSocket) -> Result<UdpSocket> {
    UdpSocket::from_std(std_socket).map_err(Error::from)
}

/// Creates a listener Tokio UDP socket with SO_REUSEPORT.
pub async fn create_tokio_listener(addr: SocketAddr) -> Result<UdpSocket> {
    let std_socket = create_listener_socket(addr)?;
    to_tokio_socket(std_socket)
}

/// Creates a session Tokio UDP socket connected to a remote address.
pub async fn create_tokio_session(
    listen_addr: SocketAddr,
    remote_addr: SocketAddr,
) -> Result<UdpSocket> {
    let std_socket = create_session_socket(listen_addr, remote_addr)?;
    to_tokio_socket(std_socket)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_listener_socket() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let socket = create_listener_socket(addr).unwrap();
        let local_addr = socket.local_addr().unwrap();
        assert_eq!(local_addr.ip(), addr.ip());
    }

    #[test]
    fn test_create_session_socket() {
        // Create listener
        let listen_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = create_listener_socket(listen_addr).unwrap();
        let actual_listen_addr = listener.local_addr().unwrap();

        // Create a remote socket to connect to
        let remote: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let remote_socket = create_listener_socket(remote).unwrap();
        let remote_addr = remote_socket.local_addr().unwrap();

        // Create session socket
        let session = create_session_socket(actual_listen_addr, remote_addr).unwrap();

        // Verify it's bound to the same port as listener
        assert_eq!(session.local_addr().unwrap().port(), actual_listen_addr.port());
    }

    #[cfg(unix)]
    #[test]
    fn test_so_reuseport() {
        use std::os::unix::io::AsRawFd;

        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let socket1 = create_listener_socket(addr).unwrap();
        let actual_addr = socket1.local_addr().unwrap();

        // Should be able to create another socket on the same port with SO_REUSEPORT
        let socket2 = create_listener_socket(actual_addr).unwrap();

        assert_eq!(socket1.local_addr().unwrap(), socket2.local_addr().unwrap());
        assert_ne!(socket1.as_raw_fd(), socket2.as_raw_fd());
    }

    #[tokio::test]
    async fn test_tokio_listener() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let socket = create_tokio_listener(addr).await.unwrap();
        let local_addr = socket.local_addr().unwrap();
        assert_eq!(local_addr.ip(), addr.ip());
    }

    #[tokio::test]
    async fn test_tokio_session() {
        let listen_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = create_tokio_listener(listen_addr).await.unwrap();
        let actual_listen_addr = listener.local_addr().unwrap();

        let remote_addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let session = create_tokio_session(actual_listen_addr, remote_addr).await.unwrap();

        assert_eq!(session.local_addr().unwrap().port(), actual_listen_addr.port());
    }
}
