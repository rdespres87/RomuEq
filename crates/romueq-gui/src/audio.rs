//! Real-time audio backend using CPAL.
//!
//! This module provides a real-time audio output stream (and optional input capture)
//! that processes audio through the `romueq_engine::EngineLifecycle`.
//!
//! It supports two operational modes:
//! 1. `TestSignal` — Generates a musical chord + harmonic overtone spectrum so users
//!    can immediately audition EQ cuts and boosts on headphones or speakers.
//! 2. `PassThrough` — Captures live audio from an input device (microphone, loopback,
//!    eqMac Export, or BlackHole) and plays it back processed through the EQ.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use romueq_common::EqParameters;
use romueq_engine::EngineLifecycle;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Source of audio input for the equalizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSourceMode {
    /// Internal harmonic audio generator (ideal for auditioning EQ on any PC without cables).
    TestSignal,
    /// Live input pass-through (captures from system loopback, BlackHole, or microphone).
    PassThrough,
}

struct AudioInternal {
    engine: Box<EngineLifecycle>,
    params: EqParameters,
    source_mode: AudioSourceMode,
    sample_rate: f32,
    block_size: usize,
    phase: f64,
    /// Stereo input queue: pairs of (left, right)
    input_queue: VecDeque<(f32, f32)>,
    latest_output: Vec<f32>,
    peak_l: f32,
    peak_r: f32,
    is_running: bool,
}

fn create_boxed_engine() -> Box<EngineLifecycle> {
    std::thread::Builder::new()
        .name("romueq-engine-init".to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || Box::new(EngineLifecycle::new()))
        .expect("failed to spawn engine init thread")
        .join()
        .expect("engine init panicked")
}

impl AudioInternal {
    fn new(sample_rate: f32, block_size: usize) -> Self {
        let mut engine = create_boxed_engine();
        let _ = engine.prepare(sample_rate as f64, block_size, 2);
        Self {
            engine,
            params: EqParameters::default(),
            source_mode: AudioSourceMode::TestSignal,
            sample_rate,
            block_size,
            phase: 0.0,
            input_queue: VecDeque::with_capacity(32768),
            latest_output: Vec::with_capacity(2048),
            peak_l: 0.0,
            peak_r: 0.0,
            is_running: false,
        }
    }
}

/// Real-time audio stream wrapper using CPAL.
pub struct AudioStream {
    internal: Arc<Mutex<AudioInternal>>,
    _output_stream: Option<Stream>,
    _input_stream: Option<Stream>,
    is_active: bool,
    sample_rate: f32,
    selected_input_device: Option<String>,
    selected_output_device: Option<String>,
}

impl AudioStream {
    /// Create a new audio stream. Defaults to stopped state.
    pub fn new() -> Self {
        let sample_rate = 48000.0;
        let block_size = 512;
        let internal = Arc::new(Mutex::new(AudioInternal::new(sample_rate, block_size)));

        Self {
            internal,
            _output_stream: None,
            _input_stream: None,
            is_active: false,
            sample_rate,
            selected_input_device: None,
            selected_output_device: None,
        }
    }

    /// Returns the list of all available input device names on the system.
    pub fn available_input_devices() -> Vec<String> {
        let host = cpal::default_host();
        host.input_devices()
            .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default()
    }

    /// Returns the list of all available output device names on the system.
    pub fn available_output_devices() -> Vec<String> {
        let host = cpal::default_host();
        host.output_devices()
            .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default()
    }

    /// Returns whether the audio stream is currently active and playing.
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Returns the currently configured input device name, if any.
    pub fn selected_input_device(&self) -> Option<&str> {
        self.selected_input_device.as_deref()
    }

    /// Sets the target input device name. If active, call restart or stop/start.
    pub fn set_input_device(&mut self, name: Option<String>) {
        self.selected_input_device = name;
    }

    /// Returns the currently configured output device name, if any.
    pub fn selected_output_device(&self) -> Option<&str> {
        self.selected_output_device.as_deref()
    }

    /// Sets the target output device name. If active, call restart or stop/start.
    pub fn set_output_device(&mut self, name: Option<String>) {
        self.selected_output_device = name;
    }

    /// Returns the current audio source mode.
    pub fn source_mode(&self) -> AudioSourceMode {
        self.internal
            .lock()
            .map(|g| g.source_mode)
            .unwrap_or(AudioSourceMode::TestSignal)
    }

    /// Sets the audio source mode (TestSignal or PassThrough).
    pub fn set_source_mode(&mut self, mode: AudioSourceMode) {
        if let Ok(mut g) = self.internal.lock() {
            g.source_mode = mode;
        }
    }

    /// Returns the current sample rate.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Prepare engine settings.
    pub fn prepare(&mut self, sample_rate: f32, block_size: usize, channels: usize) {
        self.sample_rate = sample_rate;
        if let Ok(mut g) = self.internal.lock() {
            g.sample_rate = sample_rate;
            g.block_size = block_size;
            let _ = g.engine.prepare(sample_rate as f64, block_size, channels);
        }
    }

    /// Update EQ parameters in real time.
    pub fn set_parameters(&mut self, params: EqParameters) -> Result<(), String> {
        if let Ok(mut g) = self.internal.lock() {
            g.params = params;
            g.engine.set_parameters(params);
        }
        Ok(())
    }

    /// Start audio processing and playback through configured or default audio devices.
    pub fn start(&mut self) -> Result<(), String> {
        if self.is_active {
            return Ok(());
        }

        let host = cpal::default_host();

        // 1. Locate Output Device
        let output_device = if let Some(ref name) = self.selected_output_device {
            host.output_devices()
                .ok()
                .and_then(|mut devs| devs.find(|d| d.name().map(|n| n == *name).unwrap_or(false)))
                .or_else(|| host.default_output_device())
        } else {
            host.default_output_device()
        }
        .ok_or_else(|| "No audio output device found on system".to_string())?;

        let supported_config = output_device
            .default_output_config()
            .map_err(|e| format!("Failed to get output config: {e}"))?;

        let channels = supported_config.channels() as usize;
        let sample_rate = supported_config.sample_rate().0 as f32;
        self.sample_rate = sample_rate;

        let block_size = 4096;
        {
            let mut g = self.internal.lock().unwrap();
            g.sample_rate = sample_rate;
            g.block_size = block_size;
            g.is_running = true;
            let _ = g.engine.prepare(sample_rate as f64, block_size, 2);
            let p = g.params;
            g.engine.set_parameters(p);
        }

        let stream_config: StreamConfig = supported_config.clone().into();
        let internal_out = Arc::clone(&self.internal);

        let mut left_in = vec![0.0f32; 2048];
        let mut right_in = vec![0.0f32; 2048];
        let mut left_out = vec![0.0f32; 2048];
        let mut right_out = vec![0.0f32; 2048];

        let err_fn = |err| eprintln!("Audio output stream error: {err}");

        let output_stream = match supported_config.sample_format() {
            SampleFormat::F32 => output_device
                .build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        process_output_f32(
                            data,
                            channels,
                            sample_rate,
                            block_size,
                            &internal_out,
                            &mut left_in,
                            &mut right_in,
                            &mut left_out,
                            &mut right_out,
                        );
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to build F32 output stream: {e}"))?,
            SampleFormat::I16 => {
                let mut float_scratch = vec![0.0f32; 4096];
                output_device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            if float_scratch.len() < data.len() {
                                float_scratch.resize(data.len(), 0.0);
                            }
                            let scratch_slice = &mut float_scratch[..data.len()];
                            process_output_f32(
                                scratch_slice,
                                channels,
                                sample_rate,
                                block_size,
                                &internal_out,
                                &mut left_in,
                                &mut right_in,
                                &mut left_out,
                                &mut right_out,
                            );
                            for (out, &sample) in data.iter_mut().zip(scratch_slice.iter()) {
                                *out = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                            }
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("Failed to build I16 output stream: {e}"))?
            }
            sample_fmt => {
                return Err(format!("Unsupported audio sample format: {sample_fmt:?}"));
            }
        };

        // 2. Locate Input Device (for pass-through loopback / microphone)
        let mut input_stream = None;
        let input_device_opt = if let Some(ref name) = self.selected_input_device {
            host.input_devices()
                .ok()
                .and_then(|mut devs| devs.find(|d| d.name().map(|n| n == *name).unwrap_or(false)))
                .or_else(|| host.default_input_device())
        } else {
            host.default_input_device()
        };

        if let Some(input_device) = input_device_opt {
            if let Ok(in_config) = input_device.default_input_config() {
                let in_channels = in_config.channels() as usize;
                let in_sample_format = in_config.sample_format();
                let in_stream_config: StreamConfig = in_config.into();
                let internal_in = Arc::clone(&self.internal);

                let in_res = match in_sample_format {
                    SampleFormat::F32 => input_device.build_input_stream(
                        &in_stream_config,
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            if let Ok(mut g) = internal_in.try_lock() {
                                if g.source_mode == AudioSourceMode::PassThrough {
                                    for frame in data.chunks(in_channels) {
                                        let l = frame.first().copied().unwrap_or(0.0);
                                        let r = if in_channels > 1 {
                                            frame.get(1).copied().unwrap_or(l)
                                        } else {
                                            l
                                        };
                                        if g.input_queue.len() < 16384 {
                                            g.input_queue.push_back((l, r));
                                        }
                                    }
                                }
                            }
                        },
                        |err| eprintln!("Audio input stream error: {err}"),
                        None,
                    ),
                    SampleFormat::I16 => input_device.build_input_stream(
                        &in_stream_config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            if let Ok(mut g) = internal_in.try_lock() {
                                if g.source_mode == AudioSourceMode::PassThrough {
                                    for frame in data.chunks(in_channels) {
                                        let l =
                                            frame.first().copied().unwrap_or(0) as f32 / 32768.0;
                                        let r = if in_channels > 1 {
                                            frame.get(1).copied().unwrap_or(0) as f32 / 32768.0
                                        } else {
                                            l
                                        };
                                        if g.input_queue.len() < 16384 {
                                            g.input_queue.push_back((l, r));
                                        }
                                    }
                                }
                            }
                        },
                        |err| eprintln!("Audio input stream error: {err}"),
                        None,
                    ),
                    _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
                };

                if let Ok(stream) = in_res {
                    let _ = stream.play();
                    input_stream = Some(stream);
                }
            }
        }

        output_stream
            .play()
            .map_err(|e| format!("Failed to play output stream: {e}"))?;

        self._output_stream = Some(output_stream);
        self._input_stream = input_stream;
        self.is_active = true;
        Ok(())
    }

    /// Stop audio processing and release hardware streams.
    pub fn stop(&mut self) -> Result<(), String> {
        if let Ok(mut g) = self.internal.lock() {
            g.is_running = false;
        }
        if let Some(ref s) = self._output_stream {
            let _ = s.pause();
        }
        if let Some(ref s) = self._input_stream {
            let _ = s.pause();
        }
        self._output_stream = None;
        self._input_stream = None;
        self.is_active = false;
        Ok(())
    }

    /// Stop and clean up resources.
    pub fn stop_and_cleanup(&mut self) {
        let _ = self.stop();
    }

    /// Pull the latest audio samples for FFT visualization.
    pub fn get_latest_output_samples(&self) -> Vec<f32> {
        if let Ok(g) = self.internal.lock() {
            g.latest_output.clone()
        } else {
            Vec::new()
        }
    }

    /// Returns the current peak levels in dBFS for Left and Right channels.
    pub fn peak_levels_db(&self) -> (f32, f32) {
        if let Ok(g) = self.internal.lock() {
            let to_db = |val: f32| {
                if val <= 1e-5 {
                    -100.0
                } else {
                    20.0 * val.log10()
                }
            };
            (to_db(g.peak_l), to_db(g.peak_r))
        } else {
            (-100.0, -100.0)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_output_f32(
    data: &mut [f32],
    channels: usize,
    sample_rate: f32,
    block_size: usize,
    internal: &Arc<Mutex<AudioInternal>>,
    left_in: &mut Vec<f32>,
    right_in: &mut Vec<f32>,
    left_out: &mut Vec<f32>,
    right_out: &mut Vec<f32>,
) {
    let frames = data.len() / channels;
    if frames == 0 {
        return;
    }

    if left_in.len() < frames {
        left_in.resize(frames, 0.0);
        right_in.resize(frames, 0.0);
        left_out.resize(frames, 0.0);
        right_out.resize(frames, 0.0);
    }

    let Ok(mut g) = internal.try_lock() else {
        data.fill(0.0);
        return;
    };

    if !g.is_running {
        data.fill(0.0);
        return;
    }

    // 1. Generate or fetch input samples
    match g.source_mode {
        AudioSourceMode::TestSignal => {
            let pi2 = std::f64::consts::PI * 2.0;
            let sr = sample_rate as f64;
            for i in 0..frames {
                let t = g.phase / sr;
                // Warm, mellow musical chord: A220 + C#277 + E330 + A440 (pure, harmonious, no harsh noise)
                let s = 0.14 * (t * 220.00 * pi2).sin()
                    + 0.10 * (t * 277.18 * pi2).sin()
                    + 0.08 * (t * 329.63 * pi2).sin()
                    + 0.06 * (t * 440.00 * pi2).sin();

                let sample_f32 = s as f32;
                left_in[i] = sample_f32;
                right_in[i] = sample_f32;
                g.phase = (g.phase + 1.0) % (sr * 60.0);
            }
        }
        AudioSourceMode::PassThrough => {
            // Latency drift control: keep buffer between 256 and 1024 frames (~5 to 20 ms)
            if g.input_queue.len() > 2048 {
                let excess = g.input_queue.len() - 1024;
                g.input_queue.drain(0..excess);
            }

            for i in 0..frames {
                if let Some((l, r)) = g.input_queue.pop_front() {
                    left_in[i] = l;
                    right_in[i] = r;
                } else {
                    left_in[i] = 0.0;
                    right_in[i] = 0.0;
                }
            }
        }
    }

    // 2. Process audio through the engine in blocks
    let mut offset = 0;
    while offset < frames {
        let chunk = (frames - offset).min(block_size);
        let in_slices: [&[f32]; 2] = [
            &left_in[offset..offset + chunk],
            &right_in[offset..offset + chunk],
        ];
        let mut out_slices: [&mut [f32]; 2] = [
            &mut left_out[offset..offset + chunk],
            &mut right_out[offset..offset + chunk],
        ];

        let _ = g.engine.process(&in_slices, &mut out_slices);
        offset += chunk;
    }

    // 3. Interleave output into CPAL buffer & compute peaks
    let mut max_l = 0.0f32;
    let mut max_r = 0.0f32;

    for i in 0..frames {
        let l = left_out[i];
        let r = right_out[i];
        max_l = max_l.max(l.abs());
        max_r = max_r.max(r.abs());

        let out_idx = i * channels;
        data[out_idx] = l;
        if channels > 1 {
            data[out_idx + 1] = r;
        }
        for c in 2..channels {
            data[out_idx + c] = 0.0;
        }
    }

    g.peak_l = max_l;
    g.peak_r = max_r;

    // 4. Save latest samples for GUI spectrum analysis
    g.latest_output.clear();
    let capture_count = frames.min(1024);
    g.latest_output
        .extend_from_slice(&left_out[frames - capture_count..frames]);
}

impl Default for AudioStream {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        self.stop_and_cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_stream_start_diagnostic() {
        let host = cpal::default_host();
        if let Some(out) = host.default_output_device() {
            println!(
                "Default output: {:?}, config: {:?}",
                out.name(),
                out.default_output_config()
            );
        }
        if let Some(inp) = host.default_input_device() {
            println!(
                "Default input: {:?}, config: {:?}",
                inp.name(),
                inp.default_input_config()
            );
        }
        println!(
            "Available outputs: {:?}",
            AudioStream::available_output_devices()
        );
        println!(
            "Available inputs: {:?}",
            AudioStream::available_input_devices()
        );
        let mut stream = AudioStream::new();
        stream.set_output_device(Some("Komplete Audio 6 MK2".to_string()));
        match stream.start() {
            Ok(()) => {
                println!(
                    "Audio stream to Komplete Audio 6 MK2 started successfully! Active: {}",
                    stream.is_active()
                );
                std::thread::sleep(std::time::Duration::from_millis(200));
                let samples = stream.get_latest_output_samples();
                println!("Captured {} output samples", samples.len());
                let (pl, pr) = stream.peak_levels_db();
                println!("Peaks: L={pl} dB, R={pr} dB");
                stream.stop().unwrap();
            }
            Err(e) => {
                println!("Audio stream to Komplete Audio 6 MK2 failed to start: {e}");
            }
        }
    }
}
