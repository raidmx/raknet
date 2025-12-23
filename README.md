# RakNet - High-Performance Rust Implementation

A high-performance RakNet protocol implementation in Rust for Minecraft Bedrock Edition compatibility, featuring:

- **SO_REUSEPORT architecture** for kernel-level packet demultiplexing
- **Tokio-based async I/O** for maximum concurrency
- **Per-session connected UDP sockets** for optimal performance
- **Full Minecraft Bedrock protocol compatibility**

## Architecture Highlights

### SO_REUSEPORT Design

This implementation uses a unique architecture that eliminates the traditional single-socket bottleneck:

1. **Listener socket**: Handles unconnected packets (ping, handshake)
2. **Session sockets**: Each connection gets its own connected UDP socket on the same port
3. **Kernel demultiplexing**: The OS automatically routes packets by 4-tuple (src_ip, src_port, dst_ip, dst_port) to the correct socket
4. **Zero contention**: Parallel packet processing across connections without lock contention

### Performance Features

- **Lock-free atomics** for counters and simple state
- **Zero-copy packet handling** with `bytes::Bytes`
- **Minimal allocations** through buffer pooling (planned)
- **Parallel processing** of packets across sessions

## Project Status

### ✅ Phase 1: Foundation (COMPLETED)

- [x] uint24 type with atomic support
- [x] Packet IDs and magic constant
- [x] Basic packet codec (ping/pong, handshake)
- [x] Error types with thiserror
- [x] SO_REUSEPORT socket setup
- [x] Basic RakNetListener for ping/pong
- [x] Example server and client

**Milestone Achieved**: Server responds to Minecraft client pings and appears in server list

### 🚧 Phase 2: Reliability (TODO)

- [ ] Frame format encoding/decoding
- [ ] Send queue with sequence numbers
- [ ] Receive window with duplicate detection
- [ ] ACK/NACK range compression
- [ ] Tick task for ACK flush and retransmit
- [ ] Unreliable and Reliable packet levels

**Target Milestone**: Maintain active connections and exchange reliable packets

### 📋 Phase 3: Ordering & Fragmentation (TODO)

- [ ] Ordered channel implementation (16 channels)
- [ ] Reliable Ordered reliability
- [ ] Unreliable/Reliable Sequenced
- [ ] Packet fragmentation for >MTU packets
- [ ] Fragment reassembly
- [ ] Fragment cleanup

**Target Milestone**: Handle large packets and maintain ordering

### 📋 Phase 4: Security & Performance (TODO)

- [ ] MTU discovery (1492, 1200, 576)
- [ ] Cookie validation
- [ ] IP blocking (10s timeout)
- [ ] Protocol version checks
- [ ] RTT calculation (sliding 5s window)
- [ ] Buffer pooling

**Target Milestone**: Minecraft Bedrock client can join and play

### 📋 Phase 5: Optimization (TODO)

- [ ] Lock-free data structures
- [ ] Batch packet processing
- [ ] Memory profiling
- [ ] Benchmarks vs go-raknet

**Target Milestone**: >10k concurrent connections

## Prerequisites

### macOS

You need Xcode Command Line Tools installed:

```bash
xcode-select --install
```

### Other Platforms

Ensure you have:
- Rust 1.75 or later
- Standard C/C++ toolchain (gcc/clang)

## Building

```bash
# Check compilation
cargo check

# Build release version
cargo build --release

# Run tests
cargo test

# Run examples
cargo run --example server
cargo run --example ping_client
```

## Usage

### Server Example

```rust
use raknet::{RakNetListener, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Bind to Minecraft Bedrock port
    let mut listener = RakNetListener::bind("0.0.0.0:19132").await?;

    // Set server information (MOTD)
    listener.set_pong_data(
        "MCPE;My Server;568;1.20.0;0;10;13253860892328930865;World;Survival;1;19132;19133;"
    );

    println!("Listening on {}", listener.local_addr()?);

    // Run the listener
    listener.run().await?;

    Ok(())
}
```

### Testing with Minecraft

1. Run the server: `cargo run --example server`
2. Open Minecraft Bedrock Edition
3. Go to the "Friends" tab
4. Look for your server in the LAN games section

The server will appear with the name "Rust RakNet Server" (or your custom name).

### Ping Client Example

```rust
use raknet::protocol::*;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> raknet::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect("127.0.0.1:19132").await?;

    // Send ping
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let ping = encode_unconnected_ping(timestamp, 12345);
    socket.send(&ping).await?;

    // Receive pong
    let mut buf = vec![0u8; 2048];
    let len = socket.recv(&mut buf).await?;

    let (_, server_guid, pong_data) = decode_unconnected_pong(&buf[..len])?;

    println!("Server GUID: {}", server_guid);
    println!("Server info: {}", String::from_utf8_lossy(&pong_data));

    Ok(())
}
```

## Module Structure

```
src/
├── lib.rs              # Public API exports
├── error.rs            # Error types
├── listener.rs         # RakNetListener implementation
├── socket.rs           # SO_REUSEPORT socket utilities
│
└── protocol/
    ├── mod.rs          # Protocol exports
    ├── uint24.rs       # 24-bit integer type
    ├── packet.rs       # Packet ID constants
    ├── magic.rs        # RakNet magic constant
    └── codec.rs        # Packet encoding/decoding
```

## Protocol Details

### RakNet Protocol Version

This implementation supports RakNet protocol version **11**, which is compatible with Minecraft Bedrock Edition.

### Packet Types Implemented

#### Phase 1 (Current)
- ✅ Unconnected Ping (0x01)
- ✅ Unconnected Pong (0x1c)
- ✅ Open Connection Request 1 (0x05)
- ✅ Open Connection Reply 1 (0x06)

#### Coming Soon
- Open Connection Request 2 (0x07)
- Open Connection Reply 2 (0x08)
- Connection Request (0x09)
- Connection Request Accepted (0x10)
- Frame Set (0x80-0x8f) - Datagram packets
- ACK (0xc0)
- NACK (0xa0)

### uint24 Type

RakNet uses 24-bit sequence numbers. This implementation provides a custom `u24` type:

```rust
use raknet::u24;

let seq = u24::new(0);
let next = seq.wrapping_add(1);

// Atomic variant for concurrent access
use raknet::protocol::AtomicU24;
use std::sync::atomic::Ordering;

let atomic_seq = AtomicU24::new(0);
let old = atomic_seq.fetch_add(1, Ordering::Relaxed);
```

## Design Philosophy

1. **Performance First**: Every design decision prioritizes throughput and latency
2. **Correct Protocol**: Full Minecraft Bedrock compatibility, no shortcuts
3. **Idiomatic Rust**: Uses standard patterns (Result, async/await, std::net-like API)
4. **Incremental Implementation**: Build and test in phases, each with clear milestones
5. **Zero Dependencies** (where possible): Minimize external dependencies for security and build speed

## Performance Goals

- **Throughput**: >100k packets/sec per core
- **Latency**: <1ms additional latency over raw UDP
- **Connections**: >10k concurrent connections
- **Memory**: <10KB per connection average

## Testing

```bash
# Run unit tests
cargo test

# Run integration tests
cargo test --test integration

# Run benchmarks
cargo bench

# Run with logging
RUST_LOG=debug cargo run --example server
```

## Contributing

Contributions are welcome! This project is being built incrementally following the roadmap above.

### Current Focus

We're currently in **Phase 1** (Foundation). The next priorities are:

1. Complete handshake flow (OpenConnectionRequest2/Reply2)
2. Implement session socket handoff
3. Create RakNetStream for connection handling

## References

- [go-raknet](https://github.com/sandertv/go-raknet) - Reference implementation
- [RakNet Protocol Wiki](https://wiki.bedrock.dev/servers/raknet)
- [Bedrock Protocol Documentation](https://bedrock-crustaceans.github.io/protocol-wiki/)

## License

This project is being developed for educational and server software purposes.

## Acknowledgments

- [go-raknet](https://github.com/sandertv/go-raknet) by sandertv for the excellent reference implementation
- The Minecraft Bedrock community for protocol documentation
