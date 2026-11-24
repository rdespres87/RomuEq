use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use equaform_common::{FilterType, Slope};
use equaform_dsp::{BiquadFilter, compute_coefficients, process_stereo_planar_in_place};

const SAMPLE_RATE: f32 = 48_000.0;

fn bench_compute_coefficients(c: &mut Criterion) {
    let mut group = c.benchmark_group("biquad_compute_coefficients");
    let cases = [
        ("bell", FilterType::Bell, 1000.0, 6.0, 1.4),
        ("low_shelf", FilterType::LowShelf, 120.0, 4.5, 0.707),
        ("high_shelf", FilterType::HighShelf, 8000.0, -3.0, 0.707),
        ("low_pass", FilterType::LowPass, 4000.0, 0.0, 0.707),
        ("high_pass", FilterType::HighPass, 80.0, 0.0, 0.707),
    ];

    for (name, filter_type, freq, gain, q) in cases {
        group.bench_with_input(BenchmarkId::new("48khz", name), &name, |b, _| {
            b.iter(|| {
                compute_coefficients(
                    black_box(filter_type),
                    black_box(freq),
                    black_box(gain),
                    black_box(q),
                    black_box(Slope::Db12),
                    black_box(SAMPLE_RATE),
                )
            });
        });
    }
    group.finish();
}

fn bench_biquad_process_block(c: &mut Criterion) {
    let mut group = c.benchmark_group("biquad_stereo_block");
    let coeffs = compute_coefficients(FilterType::Bell, 1000.0, 4.0, 1.2, Slope::Db12, SAMPLE_RATE);

    for block_frames in [64usize, 256, 512, 1024] {
        let len = block_frames * 2;
        let input: Vec<f32> = (0..len).map(|i| ((i as f32) * 0.017).sin() * 0.5).collect();
        let mut output = vec![0.0f32; len];
        let mut filter = BiquadFilter::new(2, coeffs);

        group.throughput(Throughput::Elements(block_frames as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(block_frames),
            &block_frames,
            |b, _| {
                b.iter(|| {
                    filter.process(black_box(&input), black_box(&mut output));
                });
            },
        );
    }
    group.finish();
}

fn bench_eight_band_cascade(c: &mut Criterion) {
    let mut group = c.benchmark_group("biquad_8band_stereo_cascade");
    let block_frames = 256usize;
    let len = block_frames * 2;
    let input: Vec<f32> = (0..len).map(|i| ((i as f32) * 0.013).sin() * 0.5).collect();
    let mut buf_a = vec![0.0f32; len];
    let mut buf_b = vec![0.0f32; len];

    let specs = [
        (FilterType::HighPass, 40.0, 0.0, 0.707),
        (FilterType::LowShelf, 100.0, 2.5, 0.707),
        (FilterType::Bell, 250.0, -2.0, 1.5),
        (FilterType::Bell, 800.0, 1.5, 1.2),
        (FilterType::Bell, 2500.0, 3.0, 1.8),
        (FilterType::Bell, 5500.0, -1.5, 2.0),
        (FilterType::HighShelf, 10000.0, 2.0, 0.707),
        (FilterType::LowPass, 18000.0, 0.0, 0.707),
    ];

    let mut filters: Vec<BiquadFilter> = specs
        .iter()
        .map(|&(ft, freq, gain, q)| {
            let coeffs = compute_coefficients(ft, freq, gain, q, Slope::Db12, SAMPLE_RATE);
            BiquadFilter::new(2, coeffs)
        })
        .collect();

    group.throughput(Throughput::Elements(block_frames as u64));
    group.bench_function("256_frames_8_bands", |b| {
        b.iter(|| {
            filters[0].process(black_box(&input), &mut buf_a);
            for (idx, filter) in filters.iter_mut().enumerate().skip(1) {
                if idx % 2 == 1 {
                    filter.process(&buf_a, &mut buf_b);
                } else {
                    filter.process(&buf_b, &mut buf_a);
                }
            }
            black_box(&buf_b);
        });
    });
    group.finish();
}

fn bench_stereo_planar_block(c: &mut Criterion) {
    let mut group = c.benchmark_group("biquad_stereo_planar_block");
    let coeffs = compute_coefficients(FilterType::Bell, 1000.0, 4.0, 1.2, Slope::Db12, SAMPLE_RATE);

    for block_frames in [64usize, 256, 512, 1024] {
        let mut left: Vec<f32> = (0..block_frames)
            .map(|i| ((i as f32) * 0.017).sin() * 0.5)
            .collect();
        let mut right: Vec<f32> = (0..block_frames)
            .map(|i| ((i as f32) * 0.023).cos() * 0.5)
            .collect();
        let mut delay_l = [0.0f32; 4];
        let mut delay_r = [0.0f32; 4];

        group.throughput(Throughput::Elements(block_frames as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(block_frames),
            &block_frames,
            |b, _| {
                b.iter(|| {
                    process_stereo_planar_in_place(
                        black_box(&mut left),
                        black_box(&mut right),
                        black_box(&coeffs),
                        &mut delay_l,
                        &mut delay_r,
                    );
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_compute_coefficients,
    bench_biquad_process_block,
    bench_eight_band_cascade,
    bench_stereo_planar_block
);
criterion_main!(benches);
