//! Equaform CLI — offline audio renderer library.
//!
//! This crate provides an offline WAV renderer that applies EQ presets to
//! audio files. It is used both as a binary (`equaform-cli`) and by
//! integration tests.

use equaform_common::EqParameters;
use equaform_engine::EngineLifecycle;
use hound::{WavReader, WavWriter};
use std::path::PathBuf;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Preset definitions
// ---------------------------------------------------------------------------

/// Flat EQ preset (no processing).
pub fn default_preset() -> EqParameters {
    let mut params = EqParameters::default();
    for band in &mut params.bands {
        band.enabled = false;
    }
    params
}

/// +6 dB at 100 Hz (low-shelf).
pub fn bass_boost_preset() -> EqParameters {
    let mut params = EqParameters::default();
    for band in &mut params.bands {
        band.enabled = false;
    }
    if let Some(band) = params.bands.get_mut(0) {
        band.enabled = true;
        band.gain_db = 6.0;
        band.frequency_hz = 100.0;
        band.q = 0.707;
    }
    params
}

/// +4 dB at 1 kHz, -3 dB at 200 Hz.
pub fn vocal_preset() -> EqParameters {
    let mut params = EqParameters::default();
    for band in &mut params.bands {
        band.enabled = false;
    }
    if let Some(band) = params.bands.get_mut(0) {
        band.enabled = true;
        band.gain_db = -3.0;
        band.frequency_hz = 200.0;
        band.q = 0.707;
    }
    if let Some(band) = params.bands.get_mut(1) {
        band.enabled = true;
        band.gain_db = 4.0;
        band.frequency_hz = 1_000.0;
        band.q = 1.0;
    }
    params
}

/// -6 dB at 5 kHz (de-ess).
pub fn de_ess_preset() -> EqParameters {
    let mut params = EqParameters::default();
    for band in &mut params.bands {
        band.enabled = false;
    }
    if let Some(band) = params.bands.get_mut(0) {
        band.enabled = true;
        band.gain_db = -6.0;
        band.frequency_hz = 5_000.0;
        band.q = 2.0;
    }
    params
}

/// Known built-in presets by name.
pub fn get_preset(name: &str) -> Option<EqParameters> {
    match name {
        "default" => Some(default_preset()),
        "bass_boost" => Some(bass_boost_preset()),
        "vocal" => Some(vocal_preset()),
        "de_ess" => Some(de_ess_preset()),
        _ => None,
    }
}

/// List of available preset names.
pub fn list_preset_names() -> &'static [&'static str] {
    &["default", "bass_boost", "vocal", "de_ess"]
}

// ---------------------------------------------------------------------------
// Audio processing
// ---------------------------------------------------------------------------

/// Process a WAV file with the given EQ parameters.
///
/// # Errors
///
/// Returns an error if:
/// - The input file cannot be opened or is not a valid WAV.
/// - The input has zero channels.
/// - The engine fails to prepare.
/// - The output file cannot be written.
pub fn process_file(
    input_path: &PathBuf,
    output_path: &PathBuf,
    sample_rate: f64,
    eq_params: EqParameters,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = WavReader::open(input_path)?;
    let spec = reader.spec();

    if spec.channels == 0 {
        return Err("Input WAV has no channels".into());
    }

    let channels = spec.channels as usize;
    let actual_sample_rate = spec.sample_rate as f64;

    // Use the sample rate from the file (or override via CLI)
    let effective_sample_rate = if (sample_rate - 44100.0).abs() < 0.01 {
        actual_sample_rate
    } else {
        sample_rate
    };

    let total_samples = reader.len() as usize / channels;

    // Read all samples into an interleaved buffer, then transpose to planar
    let interleaved: Vec<i16> = reader.samples::<i16>().filter_map(|s| s.ok()).collect();

    let samples: Vec<Vec<f32>> = (0..channels)
        .map(|ch| {
            interleaved
                .iter()
                .skip(ch)
                .step_by(channels)
                .map(|&s| s as f32 / 32768.0)
                .collect()
        })
        .collect();

    // Create and prepare the engine
    let mut engine = EngineLifecycle::new();
    if !engine.prepare(effective_sample_rate, 512, channels) {
        return Err("Failed to prepare engine".into());
    }

    engine.set_parameters(eq_params);

    let start = Instant::now();

    // Process in blocks of 512 samples
    const BLOCK_SIZE: usize = 512;
    let mut output_buffers: Vec<Vec<f32>> =
        (0..channels).map(|_| vec![0.0; total_samples]).collect();

    for block_start in (0..total_samples).step_by(BLOCK_SIZE) {
        let block_size = std::cmp::min(BLOCK_SIZE, total_samples - block_start);

        let input_slices: Vec<&[f32]> = (0..channels)
            .map(|ch| &samples[ch][block_start..block_start + block_size])
            .collect();

        let mut block_output = vec![0.0; channels * block_size];

        let mut block_output_slices: Vec<&mut [f32]> = (0..channels)
            .map(|ch| {
                let start = ch * block_size;
                let ptr = unsafe { block_output.as_mut_ptr().add(start) };
                // Safety: ptr is valid for `block_size` elements; block_output outlives the slices
                unsafe { std::slice::from_raw_parts_mut(ptr, block_size) }
            })
            .collect();

        let success = engine.process(&input_slices, &mut block_output_slices);
        if !success {
            eprintln!("Engine process returned false at block {block_start}");
        }

        for ch in 0..channels {
            let dest = &mut output_buffers[ch][block_start..block_start + block_size];
            for i in 0..block_size {
                dest[i] = block_output[ch * block_size + i];
            }
        }
    }

    let _elapsed = start.elapsed();

    // Write output WAV (16-bit integer)
    let out_spec = hound::WavSpec {
        channels: channels as u16,
        sample_rate: spec.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(output_path, out_spec)?;

    for ch_data in output_buffers.iter() {
        for &sample in ch_data.iter().take(total_samples) {
            let clamped = sample.clamp(-1.0, 1.0);
            let i16_val = (clamped * 32767.0) as i16;
            writer.write_sample(i16_val)?;
        }
    }

    writer.finalize()?;
    Ok(())
}
