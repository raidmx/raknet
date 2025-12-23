# RakNet Protocol Specification

This document describes the RakNet protocol version 11 as implemented in this library.

## Table of Contents

- [Overview](#overview)
- [Connection Flow](#connection-flow)
- [Packet Format](#packet-format)
- [Reliability System](#reliability-system)
- [Fragmentation](#fragmentation)
- [ACK/NACK System](#acknack-system)

## Overview

RakNet is a UDP-based reliable transport protocol designed for real-time networked applications.

**Key Features**:
- Reliable and ordered packet delivery over UDP
- Automatic fragmentation and reassembly
- MTU discovery
- Congestion control
- Connection state management

**Protocol Version**: 11 (Minecraft Bedrock Edition compatible)

## Connection Flow

### Complete Handshake Sequence

```
Client                                  Server
  │                                       │
  │  ─────  UnconnectedPing (0x01)  ───► │
  │                                       │
  │ ◄────  UnconnectedPong (0x1c)  ────  │
  │                                       │
  │  ─────  OpenConnectionReq1 (0x05) ─► │  MTU Discovery
  │         (MTU=1492)                    │
  │                                       │
  │ ◄────  OpenConnectionReply1 (0x06) ─ │
  │         (MTU=1492, ServerGUID)        │
  │                                       │
  │  ─────  OpenConnectionReq2 (0x07) ─► │
  │         (ServerAddr, MTU, ClientGUID) │
  │                                       │
  │ ◄────  OpenConnectionReply2 (0x08) ─ │  Session Socket Created
  │         (ClientAddr, MTU, ServerGUID) │  (SO_REUSEPORT on port 19132)
  │                                       │
  │  ═════  [Connected Socket]  ═════════ │
  │                                       │
  │  ─────  ConnectionRequest (0x09)  ──► │  In Datagram
  │         (ClientGUID, Timestamp)       │
  │                                       │
  │ ◄────  ConnectionRequestAccepted ───  │  In Datagram
  │         (ClientAddr, Timestamps)      │
  │                                       │
  │  ───── NewIncomingConnection (0x13) ► │  In Datagram
  │         (ServerAddr, Timestamps)      │
  │                                       │
  │ ═════  [CONNECTED]  ═════════════════ │
  │                                       │
  │ ◄═══►  Application Data  ◄═══════════►│
  │                                       │
  │  ───── DisconnectNotification (0x15)► │
  │                                       │
  │ ═════  [DISCONNECTED]  ══════════════ │
```

### Phase 1: Unconnected Handshake

#### UnconnectedPing (0x01)

```
┌──────────────────────────────────────┐
│ 0x01                                 │  Packet ID
├──────────────────────────────────────┤
│ Timestamp (8 bytes)                  │  Client timestamp (milliseconds)
├──────────────────────────────────────┤
│ Magic (16 bytes)                     │  RakNet magic bytes
├──────────────────────────────────────┤
│ Client GUID (8 bytes)                │  Unique client identifier
└──────────────────────────────────────┘
```

#### UnconnectedPong (0x1c)

```
┌──────────────────────────────────────┐
│ 0x1c                                 │  Packet ID
├──────────────────────────────────────┤
│ Timestamp (8 bytes)                  │  Echo client timestamp
├──────────────────────────────────────┤
│ Server GUID (8 bytes)                │  Unique server identifier
├──────────────────────────────────────┤
│ Magic (16 bytes)                     │  RakNet magic bytes
├──────────────────────────────────────┤
│ Pong Data Length (2 bytes)           │  Custom data length
├──────────────────────────────────────┤
│ Pong Data (variable)                 │  Server info (MOTD, etc.)
└──────────────────────────────────────┘
```

### Phase 2: MTU Discovery

#### OpenConnectionRequest1 (0x05)

```
┌──────────────────────────────────────┐
│ 0x05                                 │  Packet ID
├──────────────────────────────────────┤
│ Magic (16 bytes)                     │  RakNet magic bytes
├──────────────────────────────────────┤
│ Protocol Version (1 byte)            │  11 for this implementation
├──────────────────────────────────────┤
│ Padding (MTU - 28 bytes)             │  Zero padding to test MTU
└──────────────────────────────────────┘
```

**MTU Negotiation**:
- Client tries: 1492, 1200, 576 (with 500ms timeout)
- Server accepts largest that fits
- Range: 576-1500 bytes

#### OpenConnectionReply1 (0x06)

```
┌──────────────────────────────────────┐
│ 0x06                                 │  Packet ID
├──────────────────────────────────────┤
│ Magic (16 bytes)                     │  RakNet magic bytes
├──────────────────────────────────────┤
│ Server GUID (8 bytes)                │  Unique server identifier
├──────────────────────────────────────┤
│ Use Security (1 byte)                │  0 = no security
├──────────────────────────────────────┤
│ MTU (2 bytes)                        │  Agreed MTU size
└──────────────────────────────────────┘
```

### Phase 3: Connection Setup

#### OpenConnectionRequest2 (0x07)

```
┌──────────────────────────────────────┐
│ 0x07                                 │  Packet ID
├──────────────────────────────────────┤
│ Magic (16 bytes)                     │  RakNet magic bytes
├──────────────────────────────────────┤
│ Server Address (variable)            │  IP:Port server is on
├──────────────────────────────────────┤
│ MTU (2 bytes)                        │  Agreed MTU from phase 2
├──────────────────────────────────────┤
│ Client GUID (8 bytes)                │  Unique client identifier
└──────────────────────────────────────┘
```

#### OpenConnectionReply2 (0x08)

```
┌──────────────────────────────────────┐
│ 0x08                                 │  Packet ID
├──────────────────────────────────────┤
│ Magic (16 bytes)                     │  RakNet magic bytes
├──────────────────────────────────────┤
│ Server GUID (8 bytes)                │  Unique server identifier
├──────────────────────────────────────┤
│ Client Address (variable)            │  IP:Port client is on
├──────────────────────────────────────┤
│ MTU (2 bytes)                        │  Confirmed MTU size
├──────────────────────────────────────┤
│ Use Security (1 byte)                │  0 = no security
└──────────────────────────────────────┘
```

**Server creates session socket here** with SO_REUSEPORT on same port (19132).

### Phase 4: Connection Handshake (In Datagrams)

#### ConnectionRequest (0x09)

Sent as frame payload in datagram.

```
┌──────────────────────────────────────┐
│ 0x09                                 │  Packet ID
├──────────────────────────────────────┤
│ Client GUID (8 bytes)                │  Unique client identifier
├──────────────────────────────────────┤
│ Request Timestamp (8 bytes)          │  When request was sent
└──────────────────────────────────────┘
```

#### ConnectionRequestAccepted (0x10)

```
┌──────────────────────────────────────┐
│ 0x10                                 │  Packet ID
├──────────────────────────────────────┤
│ Client Address (variable)            │  IP:Port of client
├──────────────────────────────────────┤
│ System Index (2 bytes)               │  Always 0
├──────────────────────────────────────┤
│ Internal IDs (20 * 10 bytes)         │  System addresses
├──────────────────────────────────────┤
│ Request Timestamp (8 bytes)          │  Echo from request
├──────────────────────────────────────┤
│ Accepted Timestamp (8 bytes)         │  When accepted
└──────────────────────────────────────┘
```

#### NewIncomingConnection (0x13)

```
┌──────────────────────────────────────┐
│ 0x13                                 │  Packet ID
├──────────────────────────────────────┤
│ Server Address (variable)            │  IP:Port of server
├──────────────────────────────────────┤
│ Internal IDs (20 * 10 bytes)         │  System addresses
├──────────────────────────────────────┤
│ Request Timestamp (8 bytes)          │  Original request time
├──────────────────────────────────────┤
│ Accepted Timestamp (8 bytes)         │  When server accepted
└──────────────────────────────────────┘
```

**Connection is now CONNECTED**.

## Packet Format

### Datagram Packet (0x80-0x8f)

```
┌──────────────────────────────────────┐
│ Flags (1 byte)                       │  0x80-0x8f
│  Bit 7: Valid (1)                    │
│  Bit 6-4: 000                        │
│  Bit 3: ACK                          │
│  Bit 2: NAK                          │
│  Bit 1: PacketPair                   │
│  Bit 0: ContinuousSend               │
├──────────────────────────────────────┤
│ Sequence Number (3 bytes)            │  u24 sequence
├──────────────────────────────────────┤
│ Frames (variable)                    │  One or more frames
│   ┌──────────────────────────────┐   │
│   │ Frame 1                      │   │
│   ├──────────────────────────────┤   │
│   │ Frame 2                      │   │
│   │ ...                          │   │
│   └──────────────────────────────┘   │
└──────────────────────────────────────┘
```

**Important**: Sequence numbers are 24-bit (0 to 16,777,215), then wrap to 0.

### Frame Format

```
┌──────────────────────────────────────┐
│ Flags (1 byte)                       │
│  Bit 7-5: Reliability (3 bits)       │
│  Bit 4: Split (1 = fragmented)       │
│  Bit 3-0: Unused                     │
├──────────────────────────────────────┤
│ Length (2 bytes)                     │  In BITS, not bytes!
├──────────────────────────────────────┤
│ [Reliable Index (3 bytes)]           │  If reliability >= Reliable
├──────────────────────────────────────┤
│ [Sequenced Index (3 bytes)]          │  If reliability is Sequenced
├──────────────────────────────────────┤
│ [Order Index (3 bytes)]              │  If reliability is Ordered
│ [Order Channel (1 byte)]             │
├──────────────────────────────────────┤
│ [Split Count (4 bytes)]              │  If split flag set
│ [Split ID (2 bytes)]                 │
│ [Split Index (4 bytes)]              │
├──────────────────────────────────────┤
│ Payload (variable)                   │  Actual data
└──────────────────────────────────────┘
```

**Frame Length Calculation**:
```rust
let length_in_bits = payload.len() * 8;
// Encode as big-endian u16
```

## Reliability System

### Reliability Levels

| Value | Name | Reliable | Ordered | Sequenced |
|-------|------|----------|---------|-----------|
| 0 | Unreliable | ❌ | ❌ | ❌ |
| 1 | UnreliableSequenced | ❌ | ❌ | ✅ |
| 2 | Reliable | ✅ | ❌ | ❌ |
| 3 | ReliableOrdered | ✅ | ✅ | ✅ |
| 4 | ReliableSequenced | ✅ | ✅ (latest) | ✅ |

**Encoding**:
```
flags = (reliability as u8) << 5;
if has_split {
    flags |= 0x10;
}
```

### Sequence Handling

**24-bit Wraparound**:
```rust
// Sequence comparison with wraparound
fn seq_less_than(a: u32, b: u32) -> bool {
    const HALF: u32 = 0x800000; // 2^23
    ((a < b) && (b - a < HALF)) ||
    ((a > b) && (a - b > HALF))
}
```

**Example**:
```
seq_less_than(0, 1) = true
seq_less_than(0xFFFFFF, 0) = true  // Wraparound
seq_less_than(1, 0xFFFFFF) = false
```

## Fragmentation

### When to Fragment

```rust
let max_payload = mtu - 60; // Headers: IP(20) + UDP(8) + Datagram(4) + Frame(28)

if data.len() > max_payload {
    // Fragment into multiple frames
}
```

### Fragment Frame Structure

```
Split Packet ID: u16 (unique per fragmented packet)
Split Count: u32 (total fragments)
Split Index: u32 (0 to count-1)
```

**Example** (1400 MTU):
- Max single frame: ~1340 bytes
- 5000 byte payload → 4 fragments
  - Fragment 0: 1340 bytes (index=0, count=4, id=123)
  - Fragment 1: 1340 bytes (index=1, count=4, id=123)
  - Fragment 2: 1340 bytes (index=2, count=4, id=123)
  - Fragment 3: 980 bytes (index=3, count=4, id=123)

### Reassembly

- Fragments tracked by split_id
- Stored in order (BTreeMap)
- Assembled when all received
- Timeout after 8 seconds

## ACK/NACK System

### ACK Packet (0xc0)

```
┌──────────────────────────────────────┐
│ 0xc0                                 │  Packet ID
├──────────────────────────────────────┤
│ Count (2 bytes)                      │  Number of records
├──────────────────────────────────────┤
│ Records (variable)                   │  Sequence ranges
│   For each record:                   │
│   ┌──────────────────────────────┐   │
│   │ Single (1 byte) = 1          │   │  If single sequence
│   │ Sequence (3 bytes)           │   │
│   └──────────────────────────────┘   │
│   OR                                 │
│   ┌──────────────────────────────┐   │
│   │ Single (1 byte) = 0          │   │  If range
│   │ Start (3 bytes)              │   │
│   │ End (3 bytes)                │   │
│   └──────────────────────────────┘   │
└──────────────────────────────────────┘
```

### NACK Packet (0xa0)

Same format as ACK, but with packet ID 0xa0.
Indicates missing packets (gaps in sequence).

### Range Compression

**Example**:
```
Received: [0, 1, 2, 3, 5, 7, 8, 9, 10]

ACK ranges:
- Range(0, 3)   // 0-3 received
- Single(5)     // 5 received
- Range(7, 10)  // 7-10 received

NACK ranges:
- Single(4)     // 4 missing
- Single(6)     // 6 missing
```

**Compression Efficiency**:
- Uncompressed: 9 sequences * 3 bytes = 27 bytes
- Compressed: 2 ranges + 1 single = 16 bytes
- Savings: ~41%

**Best case** (consecutive 1000 sequences):
- Uncompressed: 3000 bytes
- Compressed: 6 bytes (one range)
- Savings: 99.8%

## Keepalive & Timeout

### ConnectedPing (0x00)

```
┌──────────────────────────────────────┐
│ 0x00                                 │  Packet ID
├──────────────────────────────────────┤
│ Ping Timestamp (8 bytes)             │  When ping was sent
└──────────────────────────────────────┘
```

Sent every 5 seconds.

### ConnectedPong (0x03)

```
┌──────────────────────────────────────┐
│ 0x03                                 │  Packet ID
├──────────────────────────────────────┤
│ Ping Timestamp (8 bytes)             │  Echo from ping
├──────────────────────────────────────┤
│ Pong Timestamp (8 bytes)             │  When pong was sent
└──────────────────────────────────────┘
```

### RTT Calculation

```rust
rtt = pong_receive_time - ping_send_time;

// Exponential weighted moving average
new_rtt = (old_rtt * 0.9) + (measured_rtt * 0.1);
```

### Timeout

Connection times out if no packets received for 10 seconds.

## Disconnect

### DisconnectNotification (0x15)

```
┌──────────────────────────────────────┐
│ 0x15                                 │  Packet ID
└──────────────────────────────────────┘
```

Sent when gracefully closing connection.

## Magic Bytes

```rust
const MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00,
    0xfe, 0xfe, 0xfe, 0xfe,
    0xfd, 0xfd, 0xfd, 0xfd,
    0x12, 0x34, 0x56, 0x78,
];
```

Used in all unconnected packets to identify RakNet protocol.

---

For implementation details, see [ARCHITECTURE.md](ARCHITECTURE.md).
For API usage, see [API.md](API.md).
