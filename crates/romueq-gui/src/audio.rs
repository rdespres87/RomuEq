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

/// Transparent, smooth soft-knee limiter and anti-clipping transfer function.
///
/// - Perfectly bit-linear for |x| <= 0.85 (-1.4 dBFS), guaranteeing 0.00% distortion for normal signals.
/// - Smooth C^1 continuous hyperbolic saturation curve for |x| > 0.85.
/// - Asymptotically bounded strictly within (-1.0, 1.0) so audio can NEVER clip, fold, or wrap around.
#[inline]
pub fn soft_limit(x: f32) -> f32 {
    const K: f32 = 0.85;
    const REMAIN: f32 = 1.0 - K; // 0.15
    if x.abs() <= K {
        x
    } else if x > K {
        K + REMAIN * ((x - K) / REMAIN).tanh()
    } else {
        -K - REMAIN * ((-x - K) / REMAIN).tanh()
    }
}

/// High-fidelity jitter buffer with adaptive fractional drift compensation.
///
/// Decouples independent audio clocks between input capture (e.g. BlackHole)
/// and hardware output (e.g. Komplete Audio 6 MK2), completely eliminating
/// buffer underrun chop, buffer overflow frame-drops, and clock-drift pitch wobbles.
pub struct AudioJitterBuffer {
    buffer: VecDeque<(f32, f32)>,
    is_primed: bool,
    read_frac: f64,
}

impl Default for AudioJitterBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioJitterBuffer {
    pub const TARGET_FRAMES: usize = 2048; // ~46 ms at 44.1 kHz
    pub const MIN_FRAMES: usize = 1536;    // ~35 ms
    pub const MAX_FRAMES: usize = 2560;    // ~58 ms
    pub const MAX_CAPACITY: usize = 8192;  // ~185 ms

    pub fn new() -> Self {
        Self {
            buffer: VecDeque::with_capacity(Self::MAX_CAPACITY),
            is_primed: false,
            read_frac: 0.0,
        }
    }

    pub fn push(&mut self, l: f32, r: f32) {
        if self.buffer.len() >= Self::MAX_CAPACITY {
            let excess = self.buffer.len() - Self::TARGET_FRAMES;
            self.buffer.drain(0..excess);
        }
        self.buffer.push_back((l, r));

        if !self.is_primed && self.buffer.len() >= Self::TARGET_FRAMES {
            self.is_primed = true;
            self.read_frac = 0.0;
        }
    }

    pub fn pop_into(&mut self, left: &mut [f32], right: &mut [f32], count: usize) {
        if !self.is_primed {
            left[..count].fill(0.0);
            right[..count].fill(0.0);
            return;
        }

        let cur_len = self.buffer.len();

        // If buffer drops dangerously low, enter re-priming mode cleanly
        if cur_len < count + 2 {
            self.is_primed = false;
            left[..count].fill(0.0);
            right[..count].fill(0.0);
            return;
        }

        // Proportional drift compensation:
        // Nominal: rate = 1.000 (100% bit-exact 1:1 pass-through)
        // High buffer: rate slightly > 1.0 (consumes ~2-5 extra frames per block smoothly)
        // Low buffer: rate slightly < 1.0 (consumes ~2-5 fewer frames per block smoothly)
        let rate = if cur_len > 4096 {
            1.02
        } else if cur_len > Self::MAX_FRAMES {
            1.005
        } else if cur_len < 1024 {
            0.98
        } else if cur_len < Self::MIN_FRAMES {
            0.995
        } else {
            1.000
        };

        let mut pos = self.read_frac;
        for i in 0..count {
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;

            if idx + 1 < self.buffer.len() {
                let (l0, r0) = self.buffer[idx];
                let (l1, r1) = self.buffer[idx + 1];
                left[i] = l0 + frac * (l1 - l0);
                right[i] = r0 + frac * (r1 - r0);
            } else if idx < self.buffer.len() {
                let (l0, r0) = self.buffer[idx];
                left[i] = l0;
                right[i] = r0;
            } else {
                left[i] = 0.0;
                right[i] = 0.0;
            }

            pos += rate;
        }

        let consumed = pos as usize;
        let consumed = consumed.min(self.buffer.len());
        self.buffer.drain(0..consumed);
        self.read_frac = pos - consumed as f64;
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.is_primed = false;
        self.read_frac = 0.0;
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

struct AudioInternal {
    engine: Box<EngineLifecycle>,
    params: EqParameters,
    source_mode: AudioSourceMode,
    sample_rate: f32,
    block_size: usize,
    phase: f64,
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
    jitter_buffer: Arc<Mutex<AudioJitterBuffer>>,
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
        let jitter_buffer = Arc::new(Mutex::new(AudioJitterBuffer::new()));

        Self {
            internal,
            jitter_buffer,
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
        if let Ok(mut q) = self.jitter_buffer.lock() {
            q.clear();
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
        let jitter_out = Arc::clone(&self.jitter_buffer);

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
                            &jitter_out,
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
                let jitter_out_i16 = Arc::clone(&self.jitter_buffer);
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
                                &jitter_out_i16,
                                &mut left_in,
                                &mut right_in,
                                &mut left_out,
                                &mut right_out,
                            );
                            for (out, &sample) in data.iter_mut().zip(scratch_slice.iter()) {
                                *out = (soft_limit(sample) * 32767.0) as i16;
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

        // 2. Locate Input Device strictly (prevent accidental laptop mic feedback!)
        let mut input_stream = None;
        let input_device_opt = if let Some(ref name) = self.selected_input_device {
            host.input_devices().ok().and_then(|mut devs| {
                devs.find(|d| {
                    d.name()
                        .map(|n| n.trim() == name.trim() || n.contains(name))
                        .unwrap_or(false)
                })
            })
        } else {
            // Auto-detect virtual loopback bridge (BlackHole or eqMac Export)
            host.input_devices().ok().and_then(|mut devs| {
                devs.find(|d| {
                    d.name()
                        .map(|n| n.contains("BlackHole") || n.contains("eqMac Export"))
                        .unwrap_or(false)
                })
            })
        };

        println!(
            "[AudioStream] Input device resolved: {:?}",
            input_device_opt.as_ref().and_then(|d| d.name().ok())
        );

        if let Some(input_device) = input_device_opt {
            match input_device.default_input_config() {
                Ok(in_config) => {
                    println!("[AudioStream] Input config: {in_config:?}");
                    let in_channels = in_config.channels() as usize;
                    let in_sample_format = in_config.sample_format();
                    // Lock input sample rate to match the output hardware sample rate (e.g. 44100 Hz)
                    let in_stream_config = StreamConfig {
                        channels: in_channels as u16,
                        sample_rate: cpal::SampleRate(sample_rate as u32),
                        buffer_size: cpal::BufferSize::Default,
                    };
                    let jitter_in = Arc::clone(&self.jitter_buffer);

                    let in_res = match in_sample_format {
                        SampleFormat::F32 => input_device.build_input_stream(
                            &in_stream_config,
                            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                                if let Ok(mut q) = jitter_in.lock() {
                                    for frame in data.chunks(in_channels) {
                                        let l = frame.first().copied().unwrap_or(0.0);
                                        let r = if in_channels > 1 {
                                            frame.get(1).copied().unwrap_or(l)
                                        } else {
                                            l
                                        };
                                        q.push(l, r);
                                    }
                                }
                            },
                            |err| eprintln!("Audio input stream error: {err}"),
                            None,
                        ),
                        SampleFormat::I16 => input_device.build_input_stream(
                            &in_stream_config,
                            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                                if let Ok(mut q) = jitter_in.lock() {
                                    for frame in data.chunks(in_channels) {
                                        let l =
                                            frame.first().copied().unwrap_or(0) as f32 / 32768.0;
                                        let r = if in_channels > 1 {
                                            frame.get(1).copied().unwrap_or(0) as f32 / 32768.0
                                        } else {
                                            l
                                        };
                                        q.push(l, r);
                                    }
                                }
                            },
                            |err| eprintln!("Audio input stream error: {err}"),
                            None,
                        ),
                        _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
                    };

                    match in_res {
                        Ok(stream) => {
                            if let Err(e) = stream.play() {
                                eprintln!("[AudioStream] Failed to play input stream: {e}");
                            } else {
                                println!("[AudioStream] Input stream started successfully!");
                                input_stream = Some(stream);
                            }
                        }
                        Err(e) => {
                            eprintln!("[AudioStream] Failed to build input stream: {e}");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[AudioStream] Failed to get default input config: {e}");
                }
            }
        } else {
            println!("[AudioStream] Aucun périphérique d'entrée loopback spécifié ou détecté.");
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
        if let Ok(mut q) = self.jitter_buffer.lock() {
            q.clear();
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
    jitter_buffer: &Arc<Mutex<AudioJitterBuffer>>,
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

    let mode = {
        if let Ok(g) = internal.try_lock() {
            if !g.is_running {
                data.fill(0.0);
                return;
            }
            g.source_mode
        } else {
            AudioSourceMode::PassThrough
        }
    };

    // 1. Fetch input samples or generate test signal
    match mode {
        AudioSourceMode::TestSignal => {
            if let Ok(mut g) = internal.lock() {
                let pi2 = std::f64::consts::PI * 2.0;
                let sr = sample_rate as f64;
                for i in 0..frames {
                    let t = g.phase / sr;
                    // Warm, mellow musical chord: A220 + C#277 + E330 + A440
                    let s = 0.14 * (t * 220.00 * pi2).sin()
                        + 0.10 * (t * 277.18 * pi2).sin()
                        + 0.08 * (t * 329.63 * pi2).sin()
                        + 0.06 * (t * 440.00 * pi2).sin();

                    let sample_f32 = s as f32;
                    left_in[i] = sample_f32;
                    right_in[i] = sample_f32;
                    g.phase = (g.phase + 1.0) % (sr * 60.0);
                }
            } else {
                left_in[..frames].fill(0.0);
                right_in[..frames].fill(0.0);
            }
        }
        AudioSourceMode::PassThrough => {
            if let Ok(mut q) = jitter_buffer.lock() {
                q.pop_into(left_in, right_in, frames);
            } else {
                left_in[..frames].fill(0.0);
                right_in[..frames].fill(0.0);
            }
        }
    }

    // 2. Process audio through the engine in blocks
    let mut offset = 0;
    if let Ok(mut g) = internal.lock() {
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

        // 3. Interleave output into CPAL buffer & compute peaks with soft saturation protection
        let mut max_l = 0.0f32;
        let mut max_r = 0.0f32;

        for i in 0..frames {
            // Transparent soft limiter: perfectly linear below 0.85 (-1.4 dBFS),
            // smooth saturation above, strictly bounded in (-1.0, 1.0) without square clipping.
            let l = soft_limit(left_out[i]);
            let r = soft_limit(right_out[i]);
            max_l = max_l.max(l.abs());
            max_r = max_r.max(r.abs());

            let out_idx = i * channels;
            data[out_idx] = l;
            if channels > 1 {
                data[out_idx + 1] = r;
            }
            // Mirror to channels 3/4 for Komplete Audio 6 headphone 2 / line out 3-4
            if channels >= 4 {
                data[out_idx + 2] = l;
                data[out_idx + 3] = r;
            }
            for c in 4..channels {
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
    } else {
        data.fill(0.0);
    }
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
    fn test_soft_limit_characteristics() {
        // Linear region: |x| <= 0.85
        assert_eq!(soft_limit(0.0), 0.0);
        assert_eq!(soft_limit(0.5), 0.5);
        assert_eq!(soft_limit(-0.5), -0.5);
        assert_eq!(soft_limit(0.85), 0.85);
        assert_eq!(soft_limit(-0.85), -0.85);

        // Saturation region: strictly bounded <= 1.0, monotonically non-decreasing
        let at_1 = soft_limit(1.0);
        let at_2 = soft_limit(2.0);
        let at_10 = soft_limit(10.0);
        assert!(at_1 > 0.85 && at_1 < 1.0);
        assert!(at_2 >= at_1 && at_2 <= 1.0);
        assert!(at_10 >= at_2 && at_10 <= 1.0);
        assert_eq!(soft_limit(-1.0), -at_1);
    }

    #[test]
    fn test_jitter_buffer_steady_state() {
        let mut jb = AudioJitterBuffer::new();
        let mut left_out = vec![0.0f32; 512];
        let mut right_out = vec![0.0f32; 512];

        // Fill to target (2048 frames)
        for i in 0..AudioJitterBuffer::TARGET_FRAMES {
            let val = (i as f32) / 1000.0;
            jb.push(val, val);
        }
        assert!(jb.is_primed);
        assert_eq!(jb.len(), AudioJitterBuffer::TARGET_FRAMES);

        // Read 1 block (512 frames) — nominal rate = 1.000 bit-exact
        jb.pop_into(&mut left_out, &mut right_out, 512);
        for (i, &sample) in left_out.iter().enumerate().take(512) {
            let expected = (i as f32) / 1000.0;
            assert!((sample - expected).abs() < 1e-6);
        }
        assert_eq!(jb.len(), AudioJitterBuffer::TARGET_FRAMES - 512);

        // Push another block
        for _ in 0..512 {
            jb.push(0.5, 0.5);
        }
        assert_eq!(jb.len(), AudioJitterBuffer::TARGET_FRAMES);
    }

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
