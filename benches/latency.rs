use criterion::{criterion_group, criterion_main, Criterion};
use raknet::{Frame, Reliability, u24};
use raknet::protocol::*;
use raknet::reliability::{SendQueue, RecvWindow, OrderedChannel};
use bytes::Bytes;

fn benchmark_duplicate_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("duplicate_detection");

    group.bench_function("mark_received", |b| {
        let mut window = RecvWindow::new();
        let mut seq = 0u32;

        b.iter(|| {
            let result = window.mark_received(seq);
            seq = seq.wrapping_add(1) & 0xFFFFFF;
            result
        });
    });

    group.bench_function("out_of_order", |b| {
        let mut window = RecvWindow::new();

        b.iter(|| {
            // Simulate out-of-order packets
            window.mark_received(100);
            window.mark_received(102);
            window.mark_received(101);
        });
    });

    group.finish();
}

fn benchmark_send_queue_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("send_queue");

    group.bench_function("insert", |b| {
        let mut queue = SendQueue::new();
        let data = Bytes::from(vec![0u8; 100]);
        let mut seq = 0u32;

        b.iter(|| {
            queue.insert(seq, data.clone());
            seq = seq.wrapping_add(1) & 0xFFFFFF;
        });
    });

    group.bench_function("acknowledge", |b| {
        // Pre-fill queue
        let mut queue = SendQueue::new();
        let data = Bytes::from(vec![0u8; 100]);
        for i in 0..1000 {
            queue.insert(i, data.clone());
        }

        let mut seq = 0u32;
        b.iter(|| {
            queue.acknowledge(seq);
            seq = seq.wrapping_add(1);
        });
    });

    group.finish();
}

fn benchmark_ordered_channel(c: &mut Criterion) {
    let mut group = c.benchmark_group("ordered_channel");

    group.bench_function("in_order", |b| {
        let mut channel = OrderedChannel::new();
        let data = Bytes::from(vec![0u8; 100]);
        let mut index = 0u32;

        b.iter(|| {
            channel.insert(index, data.clone());
            index = index.wrapping_add(1);
        });
    });

    group.bench_function("out_of_order", |b| {
        let mut channel = OrderedChannel::new();
        let data = Bytes::from(vec![0u8; 100]);

        b.iter(|| {
            // Receive packets 0, 2, 1 - should queue 2 until 1 arrives
            channel.insert(0, data.clone());
            channel.insert(2, data.clone());
            channel.insert(1, data.clone());
        });
    });

    group.finish();
}

fn benchmark_reliability_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("reliability_overhead");

    let reliabilities = vec![
        ("unreliable", Reliability::Unreliable),
        ("unreliable_seq", Reliability::UnreliableSequenced),
        ("reliable", Reliability::Reliable),
        ("reliable_ord", Reliability::ReliableOrdered),
        ("reliable_seq", Reliability::ReliableSequenced),
    ];

    for (name, reliability) in reliabilities {
        group.bench_function(name, |b| {
            let data = Bytes::from(vec![0u8; 100]);
            let mut frame = Frame::new(reliability, data);

            if reliability.is_reliable() {
                frame = frame.with_message_index(u24::new(123));
            }
            if reliability.is_ordered() {
                frame = frame.with_order(u24::new(45), 0);
            }

            b.iter(|| {
                let mut buf = bytes::BytesMut::new();
                frame.encode(&mut buf);
                buf
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_duplicate_detection,
    benchmark_send_queue_ops,
    benchmark_ordered_channel,
    benchmark_reliability_overhead
);
criterion_main!(benches);
