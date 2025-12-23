use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use raknet::{Frame, Reliability, SplitInfo, u24};
use raknet::protocol::*;
use bytes::{Bytes, BytesMut};

fn benchmark_frame_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_encoding");

    // Test different frame types
    let frames = vec![
        ("unreliable", Frame::new(Reliability::Unreliable, Bytes::from(vec![0u8; 100]))),
        (
            "reliable",
            Frame::new(Reliability::Reliable, Bytes::from(vec![0u8; 100]))
                .with_message_index(u24::new(123)),
        ),
        (
            "reliable_ordered",
            Frame::new(Reliability::ReliableOrdered, Bytes::from(vec![0u8; 100]))
                .with_message_index(u24::new(123))
                .with_order(u24::new(45), 0),
        ),
        (
            "fragmented",
            Frame::new(Reliability::ReliableOrdered, Bytes::from(vec![0u8; 100]))
                .with_message_index(u24::new(123))
                .with_order(u24::new(45), 0)
                .with_split(SplitInfo { count: 5, id: 1, index: 2 }),
        ),
    ];

    for (name, frame) in frames {
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut buf = BytesMut::new();
                black_box(&frame).encode(&mut buf);
                black_box(buf);
            });
        });
    }

    group.finish();
}

fn benchmark_frame_decoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_decoding");

    // Pre-encode frames
    let frames = vec![
        ("unreliable", {
            let frame = Frame::new(Reliability::Unreliable, Bytes::from(vec![0u8; 100]));
            let mut buf = BytesMut::new();
            frame.encode(&mut buf);
            buf.freeze()
        }),
        ("reliable_ordered", {
            let frame = Frame::new(Reliability::ReliableOrdered, Bytes::from(vec![0u8; 100]))
                .with_message_index(u24::new(123))
                .with_order(u24::new(45), 0);
            let mut buf = BytesMut::new();
            frame.encode(&mut buf);
            buf.freeze()
        }),
        ("fragmented", {
            let frame = Frame::new(Reliability::ReliableOrdered, Bytes::from(vec![0u8; 100]))
                .with_message_index(u24::new(123))
                .with_order(u24::new(45), 0)
                .with_split(SplitInfo { count: 5, id: 1, index: 2 });
            let mut buf = BytesMut::new();
            frame.encode(&mut buf);
            buf.freeze()
        }),
    ];

    for (name, encoded) in frames {
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut slice = black_box(&encoded[..]);
                let frame = Frame::decode(&mut slice).unwrap();
                black_box(frame);
            });
        });
    }

    group.finish();
}

fn benchmark_datagram_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("datagram");

    // Test with different numbers of frames
    for frame_count in [1, 5, 10].iter() {
        let frames: Vec<Bytes> = (0..*frame_count)
            .map(|_| {
                let frame = Frame::new(Reliability::Reliable, Bytes::from(vec![0u8; 100]))
                    .with_message_index(u24::new(123));
                let mut buf = BytesMut::new();
                frame.encode(&mut buf);
                buf.freeze()
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("encode", frame_count),
            &frames,
            |b, frames| {
                b.iter(|| {
                    let datagram = encode_datagram(black_box(u24::new(456)), black_box(&frames));
                    black_box(datagram);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_ack_encoding(c: &mut Criterion) {
    use raknet::reliability::{AckRangeList, encode_ack};

    let mut group = c.benchmark_group("ack_encoding");

    // Test different ACK patterns
    let patterns = vec![
        ("single_ack", {
            let mut list = AckRangeList::new();
            list.insert(u24::new(100));
            list
        }),
        ("consecutive", {
            let mut list = AckRangeList::new();
            for i in 0..1000 {
                list.insert(u24::new(i));
            }
            list
        }),
        ("sparse", {
            let mut list = AckRangeList::new();
            for i in (0..1000).step_by(2) {
                list.insert(u24::new(i));
            }
            list
        }),
    ];

    for (name, list) in patterns {
        group.bench_function(name, |b| {
            b.iter(|| {
                let encoded = encode_ack(black_box(&list));
                black_box(encoded);
            });
        });
    }

    group.finish();
}

fn benchmark_u24_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("u24");

    group.bench_function("creation", |b| {
        b.iter(|| {
            let val = u24::new(black_box(0x123456));
            black_box(val);
        });
    });

    group.bench_function("get", |b| {
        let val = u24::new(0x123456);
        b.iter(|| {
            let n = black_box(val).get();
            black_box(n);
        });
    });

    group.bench_function("wrapping_add", |b| {
        let val = u24::new(0xFFFFFF);
        b.iter(|| {
            let result = black_box(val).wrapping_add(black_box(1));
            black_box(result);
        });
    });

    group.bench_function("seq_less_than", |b| {
        b.iter(|| {
            let result = seq_less_than(black_box(100), black_box(200));
            black_box(result);
        });
    });

    group.finish();
}

fn benchmark_packet_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("packets");

    group.bench_function("unconnected_ping", |b| {
        b.iter(|| {
            let packet = encode_unconnected_ping(black_box(12345), black_box(67890));
            black_box(packet);
        });
    });

    group.bench_function("connection_request", |b| {
        b.iter(|| {
            let packet = encode_connection_request(black_box(12345), black_box(67890));
            black_box(packet);
        });
    });

    group.bench_function("connected_ping", |b| {
        b.iter(|| {
            let packet = encode_connected_ping(black_box(12345));
            black_box(packet);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_frame_encoding,
    benchmark_frame_decoding,
    benchmark_datagram_encoding,
    benchmark_ack_encoding,
    benchmark_u24_operations,
    benchmark_packet_encoding
);
criterion_main!(benches);
