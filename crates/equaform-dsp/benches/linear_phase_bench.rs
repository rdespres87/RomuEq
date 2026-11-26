use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use equaform_common::{
    BandParameters, BandRouting, EqParameters, FilterType, ProcessingMode, Slope,
};
use equaform_dsp::linear_phase::{KernelSynthesizer, PartitionBank};
use equaform_dsp::{LinearPhaseConvolver, UPOLS_BLOCK_SIZE};

const SAMPLE_RATE: f32 = 48_000.0;

fn multi_band_ms_params() -> EqParameters {
    let mut params = EqParameters::default();
    let specs = [
        (FilterType::HighPass, 35.0, 0.0, 0.707, BandRouting::Stereo),
        (FilterType::LowShelf, 110.0, 3.0, 0.707, BandRouting::Mid),
        (FilterType::Bell, 320.0, -2.5, 1.4, BandRouting::Stereo),
        (FilterType::Bell, 1000.0, 4.5, 1.1, BandRouting::Mid),
        (FilterType::Bell, 3200.0, 2.0, 1.8, BandRouting::Side),
        (FilterType::HighShelf, 9000.0, 3.5, 0.707, BandRouting::Side),
    ];
    for (idx, &(filter_type, frequency_hz, gain_db, q, routing)) in specs.iter().enumerate() {
        params.bands[idx] = BandParameters {
            enabled: true,
            filter_type,
            frequency_hz,
            gain_db,
            q,
            slope: Slope::Db12,
        };
        params.set_band_routing(idx, routing);
    }
    params
}

fn bench_upols_process_stereo_256(c: &mut Criterion) {
    let mut group = c.benchmark_group("linear_phase_upols_stereo_256");
    group.throughput(Throughput::Elements(UPOLS_BLOCK_SIZE as u64));

    let params = multi_band_ms_params();
    for (name, mode) in [
        (
            "upols_process_stereo_low_256",
            ProcessingMode::LinearPhaseLow,
        ),
        (
            "upols_process_stereo_high_256",
            ProcessingMode::LinearPhaseHigh,
        ),
    ] {
        let mut convolver = LinearPhaseConvolver::new(UPOLS_BLOCK_SIZE);
        convolver.update_kernel(&params, SAMPLE_RATE, mode);
        convolver.reset();

        let mut left: Vec<f32> = (0..UPOLS_BLOCK_SIZE)
            .map(|i| ((i as f32) * 0.017).sin() * 0.25)
            .collect();
        let mut right: Vec<f32> = (0..UPOLS_BLOCK_SIZE)
            .map(|i| ((i as f32) * 0.023).cos() * 0.25)
            .collect();

        group.bench_function(name, |b| {
            b.iter(|| {
                convolver.process_stereo(black_box(&mut left), black_box(&mut right));
            });
        });
    }
    group.finish();
}

fn bench_kernel_synthesize(c: &mut Criterion) {
    let mut group = c.benchmark_group("linear_phase_kernel_synthesize");
    let params = multi_band_ms_params();
    let mut synthesizer = KernelSynthesizer::new();
    let mut bank = PartitionBank::new();

    for (name, mode) in [
        ("kernel_synthesize_low", ProcessingMode::LinearPhaseLow),
        ("kernel_synthesize_high", ProcessingMode::LinearPhaseHigh),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                synthesizer.synthesize(
                    black_box(&params),
                    black_box(SAMPLE_RATE),
                    black_box(mode),
                    black_box(&mut bank),
                );
            });
        });
    }
    group.finish();
}

fn bench_async_request_and_poll_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("linear_phase_async_overhead");
    let params = multi_band_ms_params();
    let mut convolver = LinearPhaseConvolver::new(UPOLS_BLOCK_SIZE);

    group.bench_function("async_request_and_poll_overhead", |b| {
        b.iter(|| {
            convolver.request_kernel_update(
                black_box(&params),
                black_box(SAMPLE_RATE),
                black_box(ProcessingMode::LinearPhaseLow),
            );
            black_box(convolver.poll_async_kernel());
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_upols_process_stereo_256,
    bench_kernel_synthesize,
    bench_async_request_and_poll_overhead
);
criterion_main!(benches);
