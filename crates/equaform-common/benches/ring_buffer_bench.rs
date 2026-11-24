use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use equaform_common::{
    EqParameters, OverflowPolicy, SpscRingBuffer, create_audio_ring_buffer, create_triple_buffer,
};

fn bench_spsc_ring_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_ring_buffer");

    group.bench_function("enqueue_dequeue_single", |b| {
        let mut ring = SpscRingBuffer::<u64>::new(1024, OverflowPolicy::DropNew);
        let mut seq = 0u64;
        b.iter(|| {
            seq = seq.wrapping_add(1);
            black_box(ring.enqueue(black_box(seq)));
            black_box(ring.dequeue());
        });
    });

    group.throughput(Throughput::Elements(64));
    group.bench_function("enqueue_dequeue_burst_64", |b| {
        let mut ring = SpscRingBuffer::<u64>::new(1024, OverflowPolicy::DropNew);
        b.iter(|| {
            for i in 0..64u64 {
                black_box(ring.enqueue(black_box(i)));
            }
            for _ in 0..64 {
                black_box(ring.dequeue());
            }
        });
    });

    group.finish();
}

fn bench_audio_ring_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_ring_buffer");

    for block_frames in [64usize, 256, 512] {
        let interleaved: Vec<f32> = (0..block_frames * 2)
            .map(|i| ((i as f32) * 0.011).sin())
            .collect();
        let mut left = vec![0.0f32; block_frames];
        let mut right = vec![0.0f32; block_frames];
        let (producer, consumer) = create_audio_ring_buffer(4096);

        group.throughput(Throughput::Elements(block_frames as u64));
        group.bench_with_input(
            BenchmarkId::new("push_pop_stereo", block_frames),
            &block_frames,
            |b, _| {
                b.iter(|| {
                    let pushed = producer.push_interleaved(black_box(&interleaved), 2);
                    let popped = consumer.pop_slice(black_box(&mut left), black_box(&mut right));
                    black_box((pushed, popped));
                });
            },
        );
    }
    group.finish();
}

fn bench_triple_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("triple_buffer_eq_params");
    let (mut producer, mut consumer) = create_triple_buffer(EqParameters::default());
    let mut params = EqParameters::default();

    group.bench_function("publish_and_read_latest", |b| {
        b.iter(|| {
            params.global.output_gain_db = if params.global.output_gain_db == 0.0 {
                1.5
            } else {
                0.0
            };
            producer.publish(black_box(params));
            black_box(consumer.read_latest_copy());
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_spsc_ring_buffer,
    bench_audio_ring_buffer,
    bench_triple_buffer
);
criterion_main!(benches);
