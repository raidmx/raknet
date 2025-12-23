# Troubleshooting Guide

Common issues and solutions when using RakNet.

## Table of Contents

- [Connection Issues](#connection-issues)
- [Performance Problems](#performance-problems)
- [Protocol Errors](#protocol-errors)
- [Memory Issues](#memory-issues)
- [Platform-Specific](#platform-specific)

## Connection Issues

### Cannot Connect to Server

**Symptoms**:
- Client connection times out
- No UnconnectedPong received

**Possible Causes**:

1. **Firewall Blocking UDP**
   ```bash
   # Check if port is accessible
   nc -vuz 127.0.0.1 19132

   # Allow port in firewall (Linux)
   sudo ufw allow 19132/udp

   # Allow port in firewall (macOS)
   sudo pfctl -e
   ```

2. **Wrong Address/Port**
   ```rust
   // ❌ Wrong
   client.connect("localhost:19132".parse()?).await?;

   // ✅ Correct
   client.connect("127.0.0.1:19132".parse()?).await?;
   ```

3. **Server Not Running**
   ```bash
   # Check if server is listening
   netstat -an | grep 19132
   lsof -i UDP:19132
   ```

4. **MTU Too Large**
   ```
   Error: Timeout during MTU discovery

   Solution: Network doesn't support large MTU.
   Client automatically falls back to smaller sizes (1492 → 1200 → 576).
   Wait longer for timeout (currently 500ms per attempt).
   ```

### Connection Drops Randomly

**Symptoms**:
- Connection works for a while then closes
- No error messages

**Possible Causes**:

1. **Connection Timeout**
   ```rust
   // Default timeout: 10 seconds of no packets

   // Debug: Check if packets are being sent
   let stats = stream.metrics_snapshot();
   println!("Last recv: {:?}", stats.time_since_last_recv);

   // Solution: Ensure keepalive pings are being sent (every 5s by default)
   ```

2. **Network Congestion**
   ```rust
   // Check send queue
   let stats = stream.send_queue_stats();
   if stats.unacked_count > 1500 {
       eprintln!("Warning: High unacked count");
   }

   // Solution: Slow down sending rate
   ```

3. **Packet Loss**
   ```bash
   # Simulate packet loss for testing (Linux)
   sudo tc qdisc add dev lo root netem loss 10%

   # Remove
   sudo tc qdisc del dev lo root
   ```

### "Connection Refused" Error

**Symptoms**:
```
Error: ConnectionRefused("Server is full")
Error: ConnectionRefused("Already connected")
```

**Solutions**:

1. **Server Full**
   ```rust
   // Server has reached connection limit
   // Wait and retry, or increase server capacity
   ```

2. **Already Connected**
   ```rust
   // Server thinks you're already connected
   // Wait 15 seconds for old connection to timeout
   // Or use different client GUID
   ```

## Performance Problems

### Low Throughput

**Symptoms**:
- Can only send a few packets per second
- High latency

**Diagnostics**:
```rust
// Check send queue
let stats = stream.send_queue_stats();
println!("Unacked: {}", stats.unacked_count);
println!("Retries: {}", stats.total_retries);

// Check RTT
let metrics = stream.metrics_snapshot();
println!("RTT: {:?}", metrics.rtt);
```

**Solutions**:

1. **Send Queue Full**
   ```rust
   // Don't send faster than network can handle
   if stats.unacked_count > 1000 {
       tokio::time::sleep(Duration::from_millis(10)).await;
   }
   ```

2. **High Packet Loss**
   ```rust
   // Use less aggressive reliability
   // Unreliable instead of ReliableOrdered for non-critical data
   ```

3. **Small MTU**
   ```rust
   println!("MTU: {}", stream.mtu());
   // If MTU is 576, network is very limited
   // Consider compression or reduce packet sizes
   ```

### High CPU Usage

**Symptoms**:
- 100% CPU usage
- Slow packet processing

**Diagnostics**:
```bash
# Profile with perf (Linux)
perf record -g ./target/release/myapp
perf report

# Profile with instruments (macOS)
instruments -t "Time Profiler" ./target/release/myapp
```

**Solutions**:

1. **Too Many Connections**
   ```rust
   // Limit concurrent connections
   let max_connections = 1000;
   ```

2. **Tight Loop**
   ```rust
   // ❌ Bad: Busy loop
   while let Some(data) = stream.recv().await {
       process(data);
   }

   // ✅ Good: Yield occasionally
   while let Some(data) = stream.recv().await {
       process(data);
       tokio::task::yield_now().await; // Every N iterations
   }
   ```

3. **Heavy Processing**
   ```rust
   // Offload heavy work to blocking threadpool
   let result = tokio::task::spawn_blocking(|| {
       expensive_computation()
   }).await?;
   ```

### High Memory Usage

**Symptoms**:
- Memory continuously grows
- Out of memory crashes

**Diagnostics**:
```rust
// Check connection count
println!("Connections: {}", connection_count);

// Check queue sizes
let stats = stream.send_queue_stats();
println!("Send queue: {}", stats.unacked_count);
```

**Solutions**:

1. **Memory Leak**
   ```rust
   // Ensure connections are properly dropped
   drop(stream);

   // Don't hold references in closures
   tokio::spawn(async move {
       // stream moved into task, will be dropped
   });
   ```

2. **Too Many Fragments**
   ```rust
   // Fragment queue growing unbounded
   // Solution: Cleanup happens every 2 seconds automatically
   // Or manually: fragment_queue.cleanup_expired()
   ```

## Protocol Errors

### "Invalid Packet" Errors

**Symptoms**:
```
Error: InvalidPacket("Expected UnconnectedPong")
Error: InvalidPacket("Invalid magic bytes")
```

**Causes**:

1. **Wrong Protocol Version**
   ```rust
   // This implementation uses RakNet v11
   // Not compatible with other versions
   ```

2. **Corrupted Packets**
   ```bash
   # Debug with tcpdump
   sudo tcpdump -i any -X port 19132

   # Look for malformed packets
   ```

3. **Man-in-the-Middle**
   ```
   // RakNet has no encryption by default
   // Packets can be intercepted/modified
   // Solution: Use TLS tunnel or custom encryption
   ```

### "Incompatible Protocol Version" Error

**Symptoms**:
```
Error: IncompatibleProtocol { expected: 11, got: 9 }
```

**Solution**:
```rust
// Server and client must use same protocol version
// This implementation only supports version 11
// Update all clients to v11
```

### Fragmentation Issues

**Symptoms**:
- Large packets never arrive
- "Fragment timeout" messages

**Diagnostics**:
```rust
let stats = stream.fragment_queue_stats();
println!("Incomplete fragments: {}", stats.incomplete_count);
```

**Solutions**:

1. **Packet Loss**
   ```
   If any fragment is lost, entire packet is dropped after 8s
   Solution: Use Reliable reliability for fragmented packets
   ```

2. **MTU Mismatch**
   ```
   Sender thinks MTU is 1400, but packet gets dropped at 1200
   Solution: Proper MTU discovery during handshake
   ```

## Memory Issues

### "Send Queue Full" Error

**Symptoms**:
```
Error: SendQueueFull
```

**Cause**:
- Sending faster than network can handle
- Too many unacknowledged packets (>2048)

**Solution**:
```rust
match stream.send(data, reliability).await {
    Err(Error::SendQueueFull) => {
        // Wait for ACKs
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Retry send
    },
    Err(e) => return Err(e),
    Ok(()) => {}
}
```

### "Fragment Queue Full" Error

**Symptoms**:
- Large packets being dropped
- No error returned (silently dropped)

**Cause**:
- More than 512 concurrent incomplete fragmented packets

**Solution**:
```rust
// Reduce fragment timeout to cleanup faster
fragment_queue.set_timeout(Duration::from_secs(5));

// Or send smaller packets
// Or wait before sending next large packet
```

## Platform-Specific

### Linux

**SO_REUSEPORT Not Available**:
```
Error: Socket error: Protocol not available
```

**Solution**:
```bash
# Requires Linux 3.9+
uname -r

# Upgrade kernel if necessary
```

**Permission Denied on Port < 1024**:
```
Error: Permission denied (os error 13)
```

**Solution**:
```bash
# Use sudo
sudo ./target/release/server

# Or use port >= 1024
# Or grant CAP_NET_BIND_SERVICE
sudo setcap 'cap_net_bind_service=+ep' ./target/release/server
```

### macOS

**SO_REUSEPORT Issues**:
```
Works on macOS 10.9+, but behavior slightly different from Linux
```

**Firewall Blocking**:
```bash
# Check firewall
/usr/libexec/ApplicationFirewall/socketfilterfw --getglobalstate

# Allow application
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add /path/to/app
```

### Windows

**SO_REUSEPORT Not Supported**:
```
Windows doesn't support SO_REUSEPORT the same way
This implementation may not work correctly on Windows
```

**Workaround**:
```rust
// Use single socket with manual demultiplexing
// Not implemented in current version
// TODO: Windows support
```

## Debugging Tips

### Enable Debug Logging

```rust
// Add to Cargo.toml
[dependencies]
env_logger = "0.10"

// In code
env_logger::init();

// Run with
RUST_LOG=debug cargo run
```

### Packet Capture

```bash
# Capture packets
sudo tcpdump -i any -w raknet.pcap port 19132

# Analyze with wireshark
wireshark raknet.pcap
```

### Print Connection State

```rust
println!("{:?}", stream);  // Uses Debug impl

// Or detailed
let metrics = stream.metrics_snapshot();
println!("RTT: {:?}", metrics.rtt);
println!("Sent: {} bytes", metrics.total_sent);
println!("Recv: {} bytes", metrics.total_received);
println!("Last activity: {:?}", metrics.time_since_last_recv);
```

### Test with Packet Loss

```bash
# Linux: Add 10% packet loss
sudo tc qdisc add dev lo root netem loss 10%

# Linux: Add 50ms latency
sudo tc qdisc add dev lo root netem delay 50ms

# Remove
sudo tc qdisc del dev lo root
```

### Stress Testing

```rust
// Spawn many connections
for i in 0..1000 {
    tokio::spawn(async move {
        let mut client = RakNetClient::new().await?;
        let stream = client.connect(server_addr).await?;
        // ...
    });
}
```

## Common Mistakes

### 1. Not Calling `run()` on Listener

```rust
// ❌ Wrong
let listener = Arc::new(RakNetListener::bind("0.0.0.0:19132").await?);
listener.accept().await?; // Hangs forever

// ✅ Correct
let listener = Arc::new(RakNetListener::bind("0.0.0.0:19132").await?);
listener.clone().run(); // Start background task
listener.accept().await?; // Works
```

### 2. Holding Stream Across Await

```rust
// ❌ Wrong: Stream not Send
tokio::spawn(async move {
    let stream = get_stream();
    stream.send(data).await?; // Compile error
});

// ✅ Correct: Move stream into task
tokio::spawn(async move {
    let mut stream = get_stream();
    stream.send(data).await?;
});
```

### 3. Ignoring Errors

```rust
// ❌ Wrong
stream.send(data, reliability).await;

// ✅ Correct
stream.send(data, reliability).await?;

// Or handle specifically
match stream.send(data, reliability).await {
    Ok(()) => {},
    Err(Error::ConnectionClosed) => {
        // Handle disconnect
    },
    Err(e) => {
        eprintln!("Send error: {}", e);
    }
}
```

### 4. Not Checking Connection State

```rust
// ❌ Wrong: Send on closed connection
stream.send(data, reliability).await?;

// ✅ Correct: Check first
if stream.is_connected() {
    stream.send(data, reliability).await?;
}
```

## Getting Help

If you encounter issues not covered here:

1. **Check the examples** in `examples/` directory
2. **Read the API documentation** in [API.md](API.md)
3. **Enable debug logging** to see what's happening
4. **Capture packets** with tcpdump to verify protocol
5. **Open an issue** on GitHub with:
   - Rust version (`rustc --version`)
   - OS and version
   - Code snippet
   - Error message
   - Packet capture (if applicable)

---

For architecture details, see [ARCHITECTURE.md](ARCHITECTURE.md).
For performance tuning, see [PERFORMANCE.md](PERFORMANCE.md).
