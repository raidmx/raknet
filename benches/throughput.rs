use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use raknet::{Frame, Reliability, u24};
use bytes::{Bytes, BytesMut};

fn benchmark_frame_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_throughput");

    // Test different packet sizes
    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("encode", size),
            size,
            |b, &size| {
                let frame = Frame::new(
                    Reliability::ReliableOrdered,
                    Bytes::from(vec![0u8; size])
                )
                .with_message_index(u24::new(123))
                .with_order(u24::new(45), 0);

                b.iter(|| {
                    let mut buf = BytesMut::new();
                    frame.encode(&mut buf);
                    buf
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("encode_decode", size),
            size,
            |b, &size| {
                let frame = Frame::new(
                    Reliability::ReliableOrdered,
                    Bytes::from(vec![0u8; size])
                )
                .with_message_index(u24::new(123))
                .with_order(u24::new(45), 0);

                b.iter(|| {
                    // Encode
                    let mut buf = BytesMut::new();
                    frame.encode(&mut buf);
                    let encoded = buf.freeze();

                    // Decode
                    let mut slice = &encoded[..];
                    Frame::decode(&mut slice).unwrap()
                });
            },
        );
    }

    group.finish();
}

fn benchmark_batch_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch");

    for count in [1, 10, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));

        group.bench_with_input(
            BenchmarkId::new("frames", count),
            count,
            |b, &count| {
                let frames: Vec<_> = (0..count)
                    .map(|_| {
                        Frame::new(
                            Reliability::Reliable,
                            Bytes::from(vec![0u8; 100])
                        )
                        .with_message_index(u24::new(123))
                    })
                    .collect();

                b.iter(|| {
                    for frame in &frames {
                        let mut buf = BytesMut::new();
                        frame.encode(&mut buf);
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_frame_throughput, benchmark_batch_encoding);
criterion_main!(benches);
