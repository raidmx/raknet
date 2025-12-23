# API Reference

Complete API documentation for the RakNet Rust implementation.

## Table of Contents

- [Core Types](#core-types)
- [Server API](#server-api)
- [Client API](#client-api)
- [Connection API](#connection-api)
- [Protocol Types](#protocol-types)
- [Error Types](#error-types)

## Core Types

### Reliability

```rust
pub enum Reliability {
    Unreliable,
    UnreliableSequenced,
    Reliable,
    ReliableOrdered,
    ReliableSequenced,
}
```

Defines how packets are delivered.

**Variants**:

- `Unreliable` - No guarantees, fire-and-forget
- `UnreliableSequenced` - Drops old packets, no delivery guarantee
- `Reliable` - Guaranteed delivery, may reorder
- `ReliableOrdered` - Guaranteed delivery in order (most common)
- `ReliableSequenced` - Guaranteed delivery, only latest in sequence

**Methods**:
```rust
impl Reliability {
    pub fn is_reliable(&self) -> bool
    pub fn is_ordered(&self) -> bool
    pub fn is_sequenced(&self) -> bool
    pub fn header_size(&self) -> usize
}
```

**Example**:
```rust
use raknet::Reliability;

let rel = Reliability::ReliableOrdered;
assert!(rel.is_reliable());
assert!(rel.is_ordered());
```

### ConnectionState

```rust
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
}
```

Represents the current state of a connection.

**Methods**:
```rust
impl ConnectionState {
    pub fn is_connected(&self) -> bool
    pub fn is_closed(&self) -> bool
}
```

## Server API

### RakNetListener

```rust
pub struct RakNetListener { /* private fields */ }
```

Listens for incoming RakNet connections.

#### Methods

##### `bind`

```rust
pub async fn bind(addr: impl ToSocketAddrs) -> Result<Self>
```

Creates a new listener bound to the specified address.

**Parameters**:
- `addr` - Address to bind to (e.g., "0.0.0.0:19132")

**Returns**: `Result<RakNetListener>`

**Example**:
```rust
let listener = RakNetListener::bind("0.0.0.0:19132").await?;
```

##### `run`

```rust
pub fn run(self: Arc<Self>)
```

Starts the listener's background task to accept connections.
Must be called on an `Arc<RakNetListener>`.

**Example**:
```rust
let listener = Arc::new(RakNetListener::bind("0.0.0.0:19132").await?);
listener.clone().run();
```

##### `accept`

```rust
pub async fn accept(&mut self) -> Result<RakNetStream>
```

Accepts a new incoming connection.

**Returns**: `Result<RakNetStream>` - Stream for the new connection

**Blocks**: Until a new connection is established

**Example**:
```rust
while let Ok(stream) = listener.accept().await {
    tokio::spawn(async move {
        handle_connection(stream).await;
    });
}
```

##### `local_addr`

```rust
pub fn local_addr(&self) -> Result<SocketAddr>
```

Returns the local address this listener is bound to.

##### `server_guid`

```rust
pub fn server_guid(&self) -> i64
```

Returns the server's unique GUID.

##### `set_pong_data`

```rust
pub fn set_pong_data(&self, data: Bytes)
```

Sets custom data to include in UnconnectedPong responses.

**Parameters**:
- `data` - Custom data (typically server MOTD/info)

**Example**:
```rust
listener.set_pong_data(Bytes::from("My RakNet Server v1.0"));
```

## Client API

### RakNetClient

```rust
pub struct RakNetClient { /* private fields */ }
```

Client for connecting to RakNet servers.

#### Methods

##### `new`

```rust
pub async fn new() -> Result<Self>
```

Creates a new RakNet client with a random GUID.

**Returns**: `Result<RakNetClient>`

**Example**:
```rust
let client = RakNetClient::new().await?;
```

##### `connect`

```rust
pub async fn connect(&mut self, server_addr: SocketAddr) -> Result<RakNetStream>
```

Connects to a RakNet server.

**Parameters**:
- `server_addr` - Server address to connect to

**Returns**: `Result<RakNetStream>` - Stream for bidirectional communication

**Performs**:
1. UnconnectedPing/Pong exchange
2. MTU discovery (tries 1492, 1200, 576)
3. OpenConnectionRequest2/Reply2
4. Connection handshake (ConnectionRequest, NewIncomingConnection)

**Example**:
```rust
let mut client = RakNetClient::new().await?;
let stream = client.connect("127.0.0.1:19132".parse()?).await?;
```

##### `local_addr`

```rust
pub fn local_addr(&self) -> Result<SocketAddr>
```

Returns the local address of the client socket.

##### `client_guid`

```rust
pub fn client_guid(&self) -> i64
```

Returns the client's unique GUID.

## Connection API

### RakNetStream

```rust
pub struct RakNetStream { /* private fields */ }
```

Represents an established RakNet connection.
Provides bidirectional communication with reliability guarantees.

#### Methods

##### `send`

```rust
pub async fn send(&self, data: Bytes, reliability: Reliability) -> Result<()>
```

Sends data to the remote peer with the specified reliability.

**Parameters**:
- `data` - Data to send (will be fragmented if exceeds MTU)
- `reliability` - How to deliver the data

**Returns**: `Result<()>`

**Errors**:
- `Error::ConnectionClosed` - Connection is closed

**Example**:
```rust
use bytes::Bytes;
use raknet::Reliability;

stream.send(
    Bytes::from("Hello, World!"),
    Reliability::ReliableOrdered
).await?;
```

##### `recv`

```rust
pub async fn recv(&mut self) -> Option<Bytes>
```

Receives data from the remote peer.

**Returns**: `Option<Bytes>` - Data if available, `None` if connection closed

**Blocks**: Until data is available or connection closes

**Example**:
```rust
while let Some(data) = stream.recv().await {
    println!("Received: {:?}", data);
}
```

##### `close`

```rust
pub async fn close(&self) -> Result<()>
```

Gracefully closes the connection.

Sends a DisconnectNotification to the remote peer.

**Returns**: `Result<()>`

**Example**:
```rust
stream.close().await?;
```

##### `local_addr`

```rust
pub fn local_addr(&self) -> Result<SocketAddr>
```

Returns the local address of this connection.

##### `remote_addr`

```rust
pub fn remote_addr(&self) -> SocketAddr
```

Returns the remote peer's address.

##### `mtu`

```rust
pub fn mtu(&self) -> u16
```

Returns the negotiated MTU size for this connection.

##### `is_connected`

```rust
pub fn is_connected(&self) -> bool
```

Checks if the connection is still active.

**Returns**: `true` if connection is in Connected state

**Example**:
```rust
if stream.is_connected() {
    stream.send(data, Reliability::Reliable).await?;
}
```

## Protocol Types

### u24

```rust
pub struct u24(u32);
```

24-bit unsigned integer (0 to 16,777,215).
Used for sequence numbers to match RakNet protocol.

#### Methods

##### `new`

```rust
pub const fn new(value: u32) -> Self
```

Creates a new u24, masking to 24 bits.

**Example**:
```rust
let seq = u24::new(0x123456);
assert_eq!(seq.get(), 0x123456);

let wrapped = u24::new(0x1000000); // Wraps around
assert_eq!(wrapped.get(), 0);
```

##### `get`

```rust
pub const fn get(self) -> u32
```

Returns the value as a u32.

##### `wrapping_add`

```rust
pub const fn wrapping_add(self, rhs: u32) -> Self
```

Adds with wrapping at 24-bit boundary.

### Frame

```rust
pub struct Frame {
    pub reliability: Reliability,
    pub payload: Bytes,
    pub message_index: Option<u24>,
    pub sequence_index: Option<u24>,
    pub order_index: Option<u24>,
    pub order_channel: u8,
    pub split: Option<SplitInfo>,
}
```

Represents a single frame within a datagram.

#### Methods

##### `new`

```rust
pub fn new(reliability: Reliability, payload: Bytes) -> Self
```

Creates a new frame with the given reliability and payload.

##### `with_message_index`

```rust
pub fn with_message_index(self, index: u24) -> Self
```

Sets the message index (for reliable frames).

##### `with_order`

```rust
pub fn with_order(self, index: u24, channel: u8) -> Self
```

Sets the order index and channel (for ordered frames).

##### `with_split`

```rust
pub fn with_split(self, split_info: SplitInfo) -> Self
```

Marks the frame as split/fragmented.

##### `encode`

```rust
pub fn encode(&self, buf: &mut BytesMut)
```

Encodes the frame into a buffer.

##### `decode`

```rust
pub fn decode(buf: &mut &[u8]) -> Result<Self>
```

Decodes a frame from a buffer.

### SplitInfo

```rust
pub struct SplitInfo {
    pub count: u32,  // Total number of fragments
    pub id: u16,     // Unique split packet ID
    pub index: u32,  // Index of this fragment
}
```

Information about a fragmented packet.

## Error Types

### Error

```rust
pub enum Error {
    Io(io::Error),
    InvalidAddress,
    ConnectionClosed,
    Timeout,
    InvalidPacket(String),
    IncompatibleProtocol { expected: u8, got: u8 },
    ConnectionRefused(String),
    ServerFull,
    Banned,
    AlreadyConnected,
    InvalidMagic,
    ListenerClosed,
    SendQueueFull,
    RecvWindowFull,
    TooManyConnections,
    FragmentQueueFull,
    InvalidMtu(u16),
    ChannelSend,
    ChannelRecv,
    TaskJoin(JoinError),
    Socket(String),
    Other(String),
}
```

Error types that can occur during RakNet operations.

#### Helper Methods

```rust
impl Error {
    pub fn invalid_packet(msg: impl Into<String>) -> Self
    pub fn socket(msg: impl Into<String>) -> Self
    pub fn other(msg: impl Into<String>) -> Self
}
```

**Example**:
```rust
match stream.recv().await {
    Some(data) => process(data),
    None => {
        // Connection closed
        eprintln!("Connection closed");
    }
}

match stream.send(data, Reliability::Reliable).await {
    Ok(()) => {},
    Err(Error::ConnectionClosed) => {
        eprintln!("Cannot send: connection closed");
    },
    Err(Error::SendQueueFull) => {
        eprintln!("Send queue full, backpressure");
    },
    Err(e) => {
        eprintln!("Send error: {}", e);
    }
}
```

## Advanced Usage

### Custom Reliability Strategies

```rust
// For real-time data (position updates)
stream.send(position_data, Reliability::UnreliableSequenced).await?;

// For important events (player joined)
stream.send(event_data, Reliability::ReliableOrdered).await?;

// For fire-and-forget (particles, effects)
stream.send(effect_data, Reliability::Unreliable).await?;
```

### Handling Large Payloads

```rust
// Automatic fragmentation for payloads > MTU
let large_data = vec![0u8; 10000];
stream.send(
    Bytes::from(large_data),
    Reliability::ReliableOrdered
).await?;
// Automatically split into ~7 fragments (with MTU=1400)
```

### Connection Management

```rust
// Server-side connection handling
let listener = Arc::new(RakNetListener::bind("0.0.0.0:19132").await?);
listener.clone().run();

while let Ok(mut stream) = listener.accept().await {
    let addr = stream.remote_addr();
    println!("Connection from {}", addr);

    tokio::spawn(async move {
        while let Some(data) = stream.recv().await {
            // Handle data
        }
        println!("Connection closed: {}", addr);
    });
}
```

### Graceful Shutdown

```rust
// Server shutdown
drop(listener); // Stops accepting new connections

// Client disconnect
stream.close().await?; // Sends disconnect notification
```

## Constants

```rust
pub const PROTOCOL_VERSION: u8 = 11;
pub const MAGIC: [u8; 16] = [0x00, 0xff, 0xff, 0x00, ...];
```

**Protocol Version**: RakNet protocol version 11 (Minecraft Bedrock compatible)

**Magic Bytes**: RakNet offline message magic bytes for handshake packets

## Type Aliases

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

Convenience type alias for Results with RakNet errors.

---

For usage examples, see [EXAMPLES.md](EXAMPLES.md).
For protocol details, see [PROTOCOL.md](PROTOCOL.md).
