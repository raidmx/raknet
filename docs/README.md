# RakNet Documentation

Welcome to the RakNet Rust implementation documentation. This is a high-performance, async implementation of the RakNet protocol (version 11) built with Tokio.

## Table of Contents

- [Quick Start](#quick-start)
- [Documentation Files](#documentation-files)
- [Features](#features)
- [Requirements](#requirements)

## Quick Start

### Adding to Your Project

```toml
[dependencies]
raknet = { path = "../raknet" }
tokio = { version = "1", features = ["full"] }
bytes = "1"
```

### Server Example

```rust
use raknet::{RakNetListener, Reliability};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create and start listener
    let listener = Arc::new(RakNetListener::bind("0.0.0.0:19132").await?);
    listener.clone().run();

    println!("Server listening on 0.0.0.0:19132");

    // Accept connections
    while let Ok(mut stream) = listener.accept().await {
        tokio::spawn(async move {
            println!("New connection from {}", stream.remote_addr());

            // Echo server
            while let Some(data) = stream.recv().await {
                stream.send(data, Reliability::ReliableOrdered).await?;
            }

            Ok::<_, raknet::Error>(())
        });
    }

    Ok(())
}
```

### Client Example

```rust
use raknet::{RakNetClient, Reliability};
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client and connect
    let mut client = RakNetClient::new().await?;
    let mut stream = client.connect("127.0.0.1:19132".parse()?).await?;

    println!("Connected to server!");

    // Send message
    stream.send(Bytes::from("Hello, RakNet!"), Reliability::ReliableOrdered).await?;

    // Receive response
    if let Some(response) = stream.recv().await {
        println!("Received: {:?}", response);
    }

    Ok(())
}
```

## Documentation Files

- **[ARCHITECTURE.md](ARCHITECTURE.md)** - System architecture and design decisions
- **[API.md](API.md)** - Complete API reference
- **[PROTOCOL.md](PROTOCOL.md)** - RakNet protocol specification details
- **[EXAMPLES.md](EXAMPLES.md)** - Comprehensive usage examples
- **[PERFORMANCE.md](PERFORMANCE.md)** - Performance characteristics and tuning
- **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)** - Common issues and solutions

## Features

### Core Protocol Features

- ✅ **Full RakNet Protocol v11** - Compatible with Minecraft Bedrock Edition
- ✅ **5 Reliability Levels**:
  - Unreliable
  - UnreliableSequenced
  - Reliable
  - ReliableOrdered
  - ReliableSequenced
- ✅ **Automatic Fragmentation** - Split large packets exceeding MTU
- ✅ **Fragment Reassembly** - Reconstruct split packets with timeout handling
- ✅ **MTU Discovery** - Automatic MTU negotiation (576-1500 bytes)
- ✅ **Connection Handshake** - Full connection establishment flow
- ✅ **Keepalive & Timeout** - Automatic connection health monitoring

### Performance Features

- ✅ **Zero-Copy Architecture** - Uses `bytes::Bytes` for efficient memory usage
- ✅ **Lock-Free Atomics** - Custom `AtomicU24` for sequence numbers
- ✅ **SO_REUSEPORT** - Kernel-level packet demultiplexing
- ✅ **Async/Await** - Built on Tokio for high concurrency
- ✅ **Concurrent Tasks** - Separate receive, send, and tick tasks per connection

### Advanced Features

- ✅ **RTT Calculation** - Exponential weighted moving average
- ✅ **Adaptive Retransmission** - Timeout based on measured RTT (3x RTT)
- ✅ **ACK/NACK Range Compression** - Up to 99.8% size reduction
- ✅ **Duplicate Detection** - O(1) BitVec-based deduplication
- ✅ **Ordered Delivery** - 16 independent ordered channels
- ✅ **Send Queue Management** - Configurable backpressure and retry limits

## Requirements

- **Rust**: 1.70 or later
- **Tokio**: Async runtime
- **Dependencies**:
  - `bytes` - Zero-copy byte buffers
  - `tokio` - Async I/O and networking
  - `thiserror` - Error handling
  - `parking_lot` - High-performance mutexes
  - `bit-vec` - Bit vector for duplicate detection
  - `array-init` - Array initialization

## Project Structure

```
raknet/
├── src/
│   ├── lib.rs              # Public API exports
│   ├── error.rs            # Error types
│   ├── listener.rs         # Server listener
│   ├── client.rs           # Client implementation
│   ├── connection.rs       # Connection stream (RakNetStream)
│   ├── socket.rs           # Socket utilities (SO_REUSEPORT)
│   ├── protocol/           # Protocol layer
│   │   ├── mod.rs
│   │   ├── packet.rs       # Packet IDs and constants
│   │   ├── codec.rs        # Encode/decode functions
│   │   ├── frame.rs        # Frame structure
│   │   ├── uint24.rs       # 24-bit integer type
│   │   └── magic.rs        # Magic bytes
│   ├── reliability/        # Reliability layer
│   │   ├── mod.rs
│   │   ├── levels.rs       # Reliability levels enum
│   │   ├── send_queue.rs   # Unacknowledged packet tracking
│   │   ├── recv_window.rs  # Duplicate detection
│   │   ├── ack.rs          # ACK/NACK handling
│   │   ├── fragment.rs     # Fragment reassembly
│   │   └── ordered.rs      # Ordered channel
│   └── state/              # Connection state
│       ├── mod.rs
│       ├── connection.rs   # Connection state machine
│       ├── shared.rs       # Shared state (Arc)
│       └── metrics.rs      # Connection metrics
├── examples/               # Usage examples
├── docs/                   # This documentation
└── tests/                  # Integration tests
```

## Key Concepts

### Connection Lifecycle

1. **Handshake** - Client pings server, performs MTU discovery
2. **Connection Setup** - OpenConnectionRequest/Reply exchange
3. **Session Establishment** - ConnectionRequest/Accepted/NewIncomingConnection
4. **Active** - Bidirectional communication with reliability
5. **Disconnect** - Graceful or timeout-based disconnection

### Reliability Guarantees

| Level | Guaranteed Delivery | Ordering | Sequencing |
|-------|-------------------|----------|------------|
| Unreliable | ❌ | ❌ | ❌ |
| UnreliableSequenced | ❌ | ❌ | ✅ (drops old) |
| Reliable | ✅ | ❌ | ❌ |
| ReliableOrdered | ✅ | ✅ | ✅ |
| ReliableSequenced | ✅ | ✅ (latest) | ✅ |

### Memory Usage

Per connection (approximate):
- RakNetStream overhead: ~400 bytes
- SharedState: ~200 bytes
- SendQueue (2048 packets): ~16 KB
- RecvWindow (BitVec): ~320 bytes
- OrderedChannels (16): ~4 KB
- **Total**: ~21 KB per connection

For 10,000 connections: ~210 MB

## Performance Targets

- **Throughput**: >100k packets/second per connection
- **Latency**: <1ms processing overhead
- **Connections**: 10k+ concurrent connections
- **Memory**: <50KB per connection
- **CPU**: O(1) duplicate detection, O(log k) ACK processing

## License

See LICENSE file in the root directory.

## Contributing

Contributions are welcome! Please ensure:
- All tests pass (`cargo test`)
- Code is formatted (`cargo fmt`)
- No clippy warnings (`cargo clippy`)

## References

- [Original Go Implementation](https://github.com/sandertv/go-raknet)
- [RakNet Protocol Documentation](http://www.jenkinssoftware.com/)
- [Minecraft Bedrock Protocol](https://wiki.vg/Bedrock_Protocol)

---

For detailed information on specific topics, see the documentation files listed above.
