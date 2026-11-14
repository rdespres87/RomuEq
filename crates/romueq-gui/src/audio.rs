//! Real-time audio backend using CPAL.
//!
//! This module provides a real-time, lock-free audio output stream and input capture
//! that processes audio through `romueq_engine::EngineLifecycle`.
//!
//! Conforming to ADR-007, the audio callback performs zero allocations, zero mutex locks,
//! zero syscalls, and provides deterministic fallback behavior.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use romueq_common::EqParameters;
use romueq_common::audio_ring::{AudioRingConsumer, create_audio_ring_buffer};
use romueq_engine::EngineLifecycle;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

/// Maximum number of scratch frames allocated per block (pre-allocated up-front, never resized).
pub const MAX_SCRATCH_FRAMES: usize = 8192;

/// Source of audio input for the equalizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSourceMode {
    /// Internal harmonic audio generator (ideal for auditioning EQ on any PC without cables).
    TestSignal = 0,
    /// Live input pass-through (captures from system loopback, BlackHole, or microphone).
    PassThrough = 1,
}

impl From<u8> for AudioSourceMode {
    fn from(val: u8) -> Self {
        match val {
            0 => AudioSourceMode::TestSignal,
            _ => AudioSourceMode::PassThrough,
        }
    }
}

/// Transparent lock-free clock drift controller.
///
/// Dynamically compensates for crystal clock frequency differences between independent
/// input (e.g. BlackHole) and output (e.g. Komplete Audio 6 MK2) hardware devices
/// without audible pitch modulation, buffer hunting, or chopping.
pub struct ContinuousPiDriftController {
    scratch_l: Vec<f32>,
    scratch_r: Vec<f32>,
    leftover_l: [f32; 16],
    leftover_r: [f32; 16],
    leftover_len: usize,
    phase: f64,
    prev_l: f32,
    prev_r: f32,
    curr_l: f32,
    curr_r: f32,
    has_samples: bool,
    smoothed_available: f64,
    is_primed: bool,
    rate_ratio: f64,
    pub underrun_count: usize,
    pub unprimed_cycles: usize,
}

impl Default for ContinuousPiDriftController {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl ContinuousPiDriftController {
    /// Safe pre-buffering headroom for external USB audio interfaces (~46 ms at 44.1 kHz).
    pub const TARGET_FRAMES: usize = 2048;

    pub fn new(rate_ratio: f64) -> Self {
        Self {
            scratch_l: vec![0.0f32; MAX_SCRATCH_FRAMES],
            scratch_r: vec![0.0f32; MAX_SCRATCH_FRAMES],
            leftover_l: [0.0f32; 16],
            leftover_r: [0.0f32; 16],
            leftover_len: 0,
            phase: 0.0,
            prev_l: 0.0,
            prev_r: 0.0,
            curr_l: 0.0,
            curr_r: 0.0,
            has_samples: false,
            smoothed_available: Self::TARGET_FRAMES as f64,
            is_primed: false,
            rate_ratio: if rate_ratio > 0.0 { rate_ratio } else { 1.0 },
            underrun_count: 0,
            unprimed_cycles: 0,
        }
    }

    pub fn set_rate_ratio(&mut self, ratio: f64) {
        if ratio > 0.0 {
            self.rate_ratio = ratio;
        }
    }

    pub fn reset(&mut self) {
        self.is_primed = false;
        self.has_samples = false;
        self.phase = 0.0;
        self.prev_l = 0.0;
        self.prev_r = 0.0;
        self.curr_l = 0.0;
        self.curr_r = 0.0;
        self.leftover_len = 0;
        self.smoothed_available = Self::TARGET_FRAMES as f64;
    }

    pub fn is_primed(&self) -> bool {
        self.is_primed
    }

    /// Read `count` frames from `consumer` into `left` and `right` destination slices,
    /// applying transparent lock-free transfer, sample-rate adaptation, and click-free drift compensation.
    pub fn read_from(
        &mut self,
        consumer: &AudioRingConsumer,
        left: &mut [f32],
        right: &mut [f32],
        count: usize,
    ) {
        let available = consumer.available_read();

        // Severe backlog (> 12000 frames, ~270 ms): discard excess down to TARGET_FRAMES
        if available > 12000 {
            let excess = available.saturating_sub(Self::TARGET_FRAMES);
            let _ = consumer.discard(excess);
        }

        // 1. Initial pre-buffering: wait until buffer reaches TARGET_FRAMES
        if !self.is_primed {
            self.unprimed_cycles += 1;
            if available >= Self::TARGET_FRAMES {
                self.is_primed = true;
                self.smoothed_available = available as f64;
            } else {
                left[..count].fill(0.0);
                right[..count].fill(0.0);
                return;
            }
        }

        // 2. Buffer starvation protection:
        // If available frames are 0 or drop below a full block `count`, deliver silence for this cycle
        // without consuming partial blocks, avoiding audible chopping and exponential decay stutter.
        if available == 0 {
            self.underrun_count += 1;
            self.has_samples = false;
            self.phase = 0.0;
            self.leftover_len = 0;
            left[..count].fill(0.0);
            right[..count].fill(0.0);
            return;
        }
        if available < count && self.leftover_len == 0 {
            self.underrun_count += 1;
            self.has_samples = false;
            self.phase = 0.0;
            left[..count].fill(0.0);
            right[..count].fill(0.0);
            return;
        }

        // 3. Smooth EMA tracking of buffer level
        self.smoothed_available = 0.98 * self.smoothed_available + 0.02 * (available as f64);

        // 4. Smooth PI / proportional step calculation with generous ±512 frames deadband
        let error = self.smoothed_available - (Self::TARGET_FRAMES as f64);
        let in_deadband = error.abs() < 512.0;
        let is_1to1 = (self.rate_ratio - 1.0).abs() < 1e-6;

        // Bit-exact fast-path: direct slice transfer without fractional resampling interpolation
        if is_1to1 && in_deadband && self.leftover_len == 0 && available >= count {
            let popped = consumer.pop_slice(&mut left[..count], &mut right[..count]);
            if popped == count {
                self.prev_l = left[count - 1];
                self.prev_r = right[count - 1];
                self.curr_l = self.prev_l;
                self.curr_r = self.prev_r;
                self.has_samples = true;
                self.phase = 0.0;
                return;
            }
        }

        let step = if in_deadband && is_1to1 {
            1.0
        } else {
            let drift_ppm = ((error / (Self::TARGET_FRAMES as f64)) * 0.001).clamp(-0.003, 0.003);
            self.rate_ratio * (1.0 + drift_ppm)
        };

        // 5. Determine input frames needed (with +1 safety margin when fractional to guard against IEEE-754 repeated addition rounding)
        let initial_need = if self.has_samples { 0 } else { 2 };
        let extra_margin = usize::from((step - 1.0).abs() > 1e-12 || self.phase > 0.0);
        let step_advances = (self.phase + (count as f64) * step).floor() as usize + extra_margin;
        let total_needed = (initial_need + step_advances).min(MAX_SCRATCH_FRAMES);

        // Copy any unconsumed leftover frames from previous block to beginning of scratch
        for k in 0..self.leftover_len {
            self.scratch_l[k] = self.leftover_l[k];
            self.scratch_r[k] = self.leftover_r[k];
        }

        let from_consumer = total_needed.saturating_sub(self.leftover_len);
        let newly_popped = if from_consumer > 0 {
            consumer.pop_slice(
                &mut self.scratch_l[self.leftover_len..self.leftover_len + from_consumer],
                &mut self.scratch_r[self.leftover_len..self.leftover_len + from_consumer],
            )
        } else {
            0
        };

        let popped = self.leftover_len + newly_popped;
        self.leftover_len = 0;

        let mut scratch_idx = 0;
        if !self.has_samples {
            if popped >= 2 {
                self.prev_l = self.scratch_l[0];
                self.prev_r = self.scratch_r[0];
                self.curr_l = self.scratch_l[1];
                self.curr_r = self.scratch_r[1];
                scratch_idx = 2;
                self.has_samples = true;
            } else if popped == 1 {
                self.prev_l = self.scratch_l[0];
                self.prev_r = self.scratch_r[0];
                self.curr_l = self.prev_l;
                self.curr_r = self.prev_r;
                scratch_idx = 1;
                self.has_samples = true;
            } else {
                left[..count].fill(0.0);
                right[..count].fill(0.0);
                return;
            }
        }

        // 6. Phase-continuous sample-by-sample linear interpolation
        for i in 0..count {
            let frac = self.phase as f32;
            left[i] = self.prev_l + frac * (self.curr_l - self.prev_l);
            right[i] = self.prev_r + frac * (self.curr_r - self.prev_r);
            self.phase += step;
            while self.phase >= 1.0 {
                self.phase -= 1.0;
                self.prev_l = self.curr_l;
                self.prev_r = self.curr_r;
                if scratch_idx < popped {
                    self.curr_l = self.scratch_l[scratch_idx];
                    self.curr_r = self.scratch_r[scratch_idx];
                    scratch_idx += 1;
                } else {
                    // Starvation underrun: soft exponential decay to silence without clicking
                    self.underrun_count += 1;
                    self.curr_l *= 0.98;
                    self.curr_r *= 0.98;
                }
            }
        }

        // 7. Save any unconsumed frames for the next block
        if scratch_idx < popped {
            let rem = (popped - scratch_idx).min(16);
            for k in 0..rem {
                self.leftover_l[k] = self.scratch_l[scratch_idx + k];
                self.leftover_r[k] = self.scratch_r[scratch_idx + k];
            }
            self.leftover_len = rem;
        }
    }
}
pub struct AudioStream {
    _output_stream: Option<Stream>,
    _input_stream: Option<Stream>,
    is_active: bool,
    is_running: Arc<AtomicBool>,
    source_mode: Arc<AtomicU8>,
    selected_output_device: Option<String>,
    selected_input_device: Option<String>,
    param_sender: Option<SyncSender<EqParameters>>,
    current_params: EqParameters,
    delivered_params: Option<EqParameters>,
    peak_l: Arc<AtomicU32>,
    peak_r: Arc<AtomicU32>,
    spectrum_samples: Arc<Vec<AtomicU32>>,
    sample_rate: f32,
}

impl Default for AudioStream {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioStream {
    /// Create a new audio stream. Defaults to stopped state.
    pub fn new() -> Self {
        let sample_rate = 44100.0;
        let mut current_params = EqParameters::default();
        current_params.global.bypass = true;

        Self {
            _output_stream: None,
            _input_stream: None,
            is_active: false,
            is_running: Arc::new(AtomicBool::new(false)),
            source_mode: Arc::new(AtomicU8::new(AudioSourceMode::PassThrough as u8)),
            sample_rate,
            selected_input_device: None,
            selected_output_device: None,
            param_sender: None,
            current_params,
            delivered_params: None,
            peak_l: Arc::new(AtomicU32::new(0)),
            peak_r: Arc::new(AtomicU32::new(0)),
            spectrum_samples: Arc::new((0..2048).map(|_| AtomicU32::new(0)).collect()),
        }
    }

    pub fn available_output_devices() -> Vec<String> {
        let host = cpal::default_host();
        let mut out = Vec::new();
        if let Ok(devices) = host.devices() {
            for d in devices {
                if let Ok(name) = d.name() {
                    let lower = name.to_lowercase();
                    if lower.contains("equaform")
                        || lower == "romueq"
                        || lower.contains("romueq ")
                        || lower.contains("blackhole")
                        || lower.contains("eqmac export")
                    {
                        continue;
                    }
                    if out.contains(&name) {
                        continue;
                    }

                    // Check default or supported configs first (avoids instantiating dummy AUHAL AudioUnits)
                    let is_supported = d.default_output_config().is_ok()
                        || d.supported_output_configs()
                            .map(|mut c| c.next().is_some())
                            .unwrap_or(false);

                    if is_supported {
                        out.push(name);
                    } else {
                        // Fallback only if default/supported query failed (e.g. CPAL 0.15.3 macOS input bug on built-in output)
                        let test_config = StreamConfig {
                            channels: 2,
                            sample_rate: cpal::SampleRate(44100),
                            buffer_size: cpal::BufferSize::Default,
                        };
                        if d.build_output_stream(&test_config, |_: &mut [f32], _| {}, |_| {}, None)
                            .is_ok()
                        {
                            out.push(name);
                        }
                    }
                }
            }
        }
        out
    }

    pub fn available_input_devices() -> Vec<String> {
        let host = cpal::default_host();
        host.input_devices()
            .map(|devs| devs.filter_map(|d| d.name().ok()).collect())
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
        AudioSourceMode::from(self.source_mode.load(Ordering::Relaxed))
    }

    /// Sets the audio source mode (TestSignal or PassThrough).
    pub fn set_source_mode(&mut self, mode: AudioSourceMode) {
        self.source_mode.store(mode as u8, Ordering::Relaxed);
    }

    /// Returns the current sample rate.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Prepare engine settings.
    pub fn prepare(&mut self, sample_rate: f32, _block_size: usize, _channels: usize) {
        self.sample_rate = sample_rate;
    }

    /// Update EQ parameters in real time without locking the audio thread.
    pub fn set_parameters(&mut self, params: EqParameters) -> Result<(), String> {
        self.current_params = params;
        if let Some(ref sender) = self.param_sender {
            if sender.try_send(params).is_ok() {
                self.delivered_params = Some(params);
            } else {
                self.delivered_params = None;
            }
        } else {
            self.delivered_params = None;
        }
        Ok(())
    }

    /// Ensures the audio thread has received the latest `params` (retries if the channel was full on a prior frame).
    pub fn ensure_parameters_synced(&mut self, params: EqParameters) {
        self.current_params = params;
        if self.delivered_params != Some(params) {
            if let Some(ref sender) = self.param_sender {
                if sender.try_send(params).is_ok() {
                    self.delivered_params = Some(params);
                }
            }
        }
    }

    /// Start audio processing and playback through configured or default audio devices.
    pub fn start(&mut self) -> Result<(), String> {
        if self.is_active {
            return Ok(());
        }

        let host = cpal::default_host();

        // 1. Locate Output Device (preferring output-capable subdevice when duplicate names exist)
        let output_device = if let Some(ref name) = self.selected_output_device {
            host.devices()
                .ok()
                .and_then(|devs| {
                    let candidates: Vec<_> = devs
                        .filter(|d| d.name().map(|n| n == *name).unwrap_or(false))
                        .collect();
                    candidates
                        .iter()
                        .find(|d| {
                            d.default_output_config().is_ok()
                                || d.supported_output_configs()
                                    .map(|mut c| c.next().is_some())
                                    .unwrap_or(false)
                        })
                        .cloned()
                        .or_else(|| candidates.into_iter().next())
                })
                .or_else(|| host.default_output_device())
        } else {
            host.default_output_device()
        }
        .ok_or_else(|| "No audio output device found on system".to_string())?;

        let (channels, sample_rate, sample_format) =
            if let Ok(cfg) = output_device.default_output_config() {
                (
                    (cfg.channels() as usize).max(1),
                    cfg.sample_rate().0 as f32,
                    cfg.sample_format(),
                )
            } else {
                // Fallback for macOS output-only devices (such as Haut-parleurs MacBook Pro)
                // where CPAL's default_output_config() fails due to a known CoreAudio bug in CPAL.
                #[cfg(target_os = "macos")]
                let hw_sr = macos_virtual_rate_sync::get_device_sample_rate_by_name(
                    self.selected_output_device
                        .as_deref()
                        .unwrap_or("Haut-parleurs"),
                )
                .unwrap_or(44100.0) as f32;
                #[cfg(not(target_os = "macos"))]
                let hw_sr = 44100.0_f32;
                (2, hw_sr, SampleFormat::F32)
            };

        // On macOS, synchronize virtual loopback device (RomuEq / BlackHole 2ch) sample rate
        // to match the selected hardware output device sample rate whenever starting/switching output.
        #[cfg(target_os = "macos")]
        macos_virtual_rate_sync::sync_virtual_input_rate(sample_rate as f64);

        self.sample_rate = sample_rate;

        let block_size = 512;
        let stream_config = StreamConfig {
            channels: channels as u16,
            sample_rate: cpal::SampleRate(sample_rate as u32),
            buffer_size: cpal::BufferSize::Default,
        };

        // Create lock-free audio ring buffer between input and output threads
        let (audio_producer, audio_consumer) = create_audio_ring_buffer(16384);

        // Create lock-free parameter channel
        let (param_tx, param_rx) = sync_channel::<EqParameters>(256);
        let _ = param_tx.try_send(self.current_params);
        self.param_sender = Some(param_tx);
        self.delivered_params = Some(self.current_params);

        let is_running = Arc::clone(&self.is_running);
        is_running.store(true, Ordering::Release);
        let source_mode = Arc::clone(&self.source_mode);
        let peak_l = Arc::clone(&self.peak_l);
        let peak_r = Arc::clone(&self.peak_r);
        let spectrum_samples = Arc::clone(&self.spectrum_samples);

        let mut engine = Box::new(EngineLifecycle::new());
        let _ = engine.prepare(sample_rate as f64, block_size, 2);
        engine.set_parameters(self.current_params);

        // Pre-resolve input device and dynamic sample rate ratio before stream creation
        let input_device_opt = if let Some(ref name) = self.selected_input_device {
            host.input_devices().ok().and_then(|mut devs| {
                devs.find(|d| {
                    d.name()
                        .map(|n| n.trim() == name.trim() || n.contains(name))
                        .unwrap_or(false)
                })
            })
        } else {
            // Auto-detect virtual loopback bridge (Equaform prioritized, then RomuEq, BlackHole or eqMac Export)
            host.input_devices().ok().and_then(|devs| {
                let dev_list: Vec<_> = devs.collect();
                dev_list
                    .iter()
                    .find(|d| {
                        d.name()
                            .map(|n| n.contains("Equaform") || n.contains("RomuEq"))
                            .unwrap_or(false)
                    })
                    .or_else(|| {
                        dev_list
                            .iter()
                            .find(|d| d.name().map(|n| n.contains("BlackHole")).unwrap_or(false))
                    })
                    .or_else(|| {
                        dev_list.iter().find(|d| {
                            d.name()
                                .map(|n| n.contains("eqMac Export"))
                                .unwrap_or(false)
                        })
                    })
                    .cloned()
            })
        };

        println!(
            "[AudioStream] Input device resolved: {:?}",
            input_device_opt.as_ref().and_then(|d| d.name().ok())
        );

        let rate_ratio = if let Some(ref in_dev) = input_device_opt {
            if let Ok(in_cfg) = in_dev.default_input_config() {
                let in_sr = in_cfg.sample_rate().0 as f64;
                let out_sr = sample_rate as f64;
                let ratio = in_sr / out_sr;
                println!(
                    "[AudioStream] Audio chain rate sync: Input={in_sr:.0} Hz, Output={out_sr:.0} Hz, RateRatio={ratio:.6}"
                );
                ratio
            } else {
                1.0
            }
        } else {
            1.0
        };

        let mut drift = ContinuousPiDriftController::new(rate_ratio);
        let mut phase = 0.0f64;

        // Pre-allocate scratch buffers strictly to MAX_SCRATCH_FRAMES (ADR-007: zero allocation in callbacks)
        let mut left_in = vec![0.0f32; MAX_SCRATCH_FRAMES];
        let mut right_in = vec![0.0f32; MAX_SCRATCH_FRAMES];
        let mut left_out = vec![0.0f32; MAX_SCRATCH_FRAMES];
        let mut right_out = vec![0.0f32; MAX_SCRATCH_FRAMES];

        let err_fn = |err| eprintln!("Audio output stream error: {err}");

        let output_stream = match sample_format {
            SampleFormat::F32 => output_device
                .build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        process_output_block(
                            data,
                            channels,
                            sample_rate,
                            block_size,
                            &mut engine,
                            &param_rx,
                            &audio_consumer,
                            &mut drift,
                            &is_running,
                            &source_mode,
                            &peak_l,
                            &peak_r,
                            &spectrum_samples,
                            &mut phase,
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
                let mut float_scratch = vec![0.0f32; MAX_SCRATCH_FRAMES * channels.max(2)];
                output_device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            let frames = (data.len() / channels).min(MAX_SCRATCH_FRAMES);
                            let active_len = frames * channels;
                            let scratch_slice = &mut float_scratch[..active_len];
                            process_output_block(
                                scratch_slice,
                                channels,
                                sample_rate,
                                block_size,
                                &mut engine,
                                &param_rx,
                                &audio_consumer,
                                &mut drift,
                                &is_running,
                                &source_mode,
                                &peak_l,
                                &peak_r,
                                &spectrum_samples,
                                &mut phase,
                                &mut left_in,
                                &mut right_in,
                                &mut left_out,
                                &mut right_out,
                            );
                            for (out, &sample) in
                                data[..active_len].iter_mut().zip(scratch_slice.iter())
                            {
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

        // 2. Build Input Stream using its native hardware sample rate (auto-resampled to output by ContinuousPiDriftController)
        let mut input_stream = None;
        if let Some(input_device) = input_device_opt {
            let in_config = input_device
                .default_input_config()
                .map_err(|e| format!("Failed to get default input config: {e}"))?;

            let in_channels = in_config.channels() as usize;
            let in_sample_format = in_config.sample_format();
            let in_sr = in_config.sample_rate().0;

            let in_stream_config = StreamConfig {
                channels: in_channels as u16,
                sample_rate: cpal::SampleRate(in_sr),
                buffer_size: cpal::BufferSize::Default,
            };
            let producer = audio_producer;

            let in_res = match in_sample_format {
                SampleFormat::F32 => input_device.build_input_stream(
                    &in_stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        producer.push_interleaved(data, in_channels);
                    },
                    |err| eprintln!("Audio input stream error: {err}"),
                    None,
                ),
                SampleFormat::I16 => {
                    let mut conv_scratch = vec![0.0f32; MAX_SCRATCH_FRAMES * in_channels.max(2)];
                    input_device.build_input_stream(
                        &in_stream_config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            let frames = (data.len() / in_channels).min(MAX_SCRATCH_FRAMES);
                            let active_len = frames * in_channels;
                            for (dst, &src) in
                                conv_scratch[..active_len].iter_mut().zip(data.iter())
                            {
                                *dst = src as f32 / 32768.0;
                            }
                            producer.push_interleaved(&conv_scratch[..active_len], in_channels);
                        },
                        |err| eprintln!("Audio input stream error: {err}"),
                        None,
                    )
                }
                _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
            };

            match in_res {
                Ok(stream) => {
                    if let Err(e) = stream.play() {
                        eprintln!("[AudioStream] Failed to play input stream: {e}");
                    } else {
                        input_stream = Some(stream);
                        println!("[AudioStream] Input stream started successfully!");
                    }
                }
                Err(e) => {
                    eprintln!("[AudioStream] Failed to build input stream: {e}");
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

    /// Stops audio playback and resets engine buffers.
    pub fn stop(&mut self) -> Result<(), String> {
        self.is_running.store(false, Ordering::Release);
        if let Some(ref s) = self._output_stream {
            let _ = s.pause();
        }
        if let Some(ref s) = self._input_stream {
            let _ = s.pause();
        }
        self._output_stream = None;
        self._input_stream = None;
        self.peak_l.store(0, Ordering::Relaxed);
        self.peak_r.store(0, Ordering::Relaxed);
        for s in self.spectrum_samples.iter() {
            s.store(0, Ordering::Relaxed);
        }
        self.is_active = false;
        Ok(())
    }

    /// Restores the macOS system default output device (`dOut`) to the currently selected
    /// physical output device so closing the application never changes the user's chosen output.
    pub fn restore_system_output_to_selected(&self) {
        #[cfg(target_os = "macos")]
        if let Some(ref out_name) = self.selected_output_device {
            macos_virtual_rate_sync::restore_macos_default_output(out_name);
        }
    }

    /// Returns a copy of the latest output samples for GUI spectrum analysis.
    pub fn get_latest_output_samples(&self) -> Vec<f32> {
        self.spectrum_samples
            .iter()
            .map(|s| f32::from_bits(s.load(Ordering::Relaxed)))
            .collect()
    }

    /// Returns the current peak levels in dB for left and right channels.
    pub fn peak_levels_db(&self) -> (f32, f32) {
        let to_db = |lin: f32| {
            if lin <= 1e-5 {
                -100.0
            } else {
                20.0 * lin.log10()
            }
        };
        let l = f32::from_bits(self.peak_l.load(Ordering::Relaxed));
        let r = f32::from_bits(self.peak_r.load(Ordering::Relaxed));
        (to_db(l), to_db(r))
    }
}

#[inline]
pub fn soft_limit(x: f32) -> f32 {
    let abs_x = x.abs();
    if abs_x <= 0.85 {
        x
    } else {
        let sign = x.signum();
        sign * (0.85 + 0.15 * ((abs_x - 0.85) / 0.15).tanh())
    }
}

/// Real-time audio block processor: wait-free, zero allocation, zero mutex locks.
#[allow(clippy::too_many_arguments)]
fn process_output_block(
    data: &mut [f32],
    channels: usize,
    sample_rate: f32,
    block_size: usize,
    engine: &mut EngineLifecycle,
    param_rx: &Receiver<EqParameters>,
    audio_consumer: &AudioRingConsumer,
    drift: &mut ContinuousPiDriftController,
    is_running: &AtomicBool,
    source_mode: &AtomicU8,
    peak_l: &AtomicU32,
    peak_r: &AtomicU32,
    spectrum_samples: &[AtomicU32],
    phase: &mut f64,
    left_in: &mut [f32],
    right_in: &mut [f32],
    left_out: &mut [f32],
    right_out: &mut [f32],
) {
    let frames = (data.len() / channels).min(MAX_SCRATCH_FRAMES);
    if frames == 0 {
        return;
    }

    if !is_running.load(Ordering::Acquire) {
        data.fill(0.0);
        return;
    }

    // 1. Drain pending parameter updates non-blocking
    while let Ok(new_params) = param_rx.try_recv() {
        engine.set_parameters(new_params);
    }

    let mode = AudioSourceMode::from(source_mode.load(Ordering::Relaxed));

    // 2. Fetch input samples or generate musical test signal
    match mode {
        AudioSourceMode::TestSignal => {
            let pi2 = std::f64::consts::PI * 2.0;
            let sr = sample_rate as f64;
            for i in 0..frames {
                let t = *phase / sr;
                // Warm musical chord: A220 + C#277 + E330 + A440
                let s = 0.14 * (t * 220.00 * pi2).sin()
                    + 0.10 * (t * 277.18 * pi2).sin()
                    + 0.08 * (t * 329.63 * pi2).sin()
                    + 0.06 * (t * 440.00 * pi2).sin();

                let sample_f32 = s as f32;
                left_in[i] = sample_f32;
                right_in[i] = sample_f32;
                *phase = (*phase + 1.0) % (sr * 60.0);
            }
        }
        AudioSourceMode::PassThrough => {
            drift.read_from(audio_consumer, left_in, right_in, frames);
        }
    }

    // 3. Process audio through the engine in blocks (or direct bit-exact bypass)
    let is_bypassed = engine.eq_parameters().global.bypass;
    if is_bypassed {
        left_out[..frames].copy_from_slice(&left_in[..frames]);
        right_out[..frames].copy_from_slice(&right_in[..frames]);
    } else {
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

            let success = engine.process(&in_slices, &mut out_slices);
            if !success {
                // Strict deterministic fallback on processing error
                out_slices[0].copy_from_slice(in_slices[0]);
                out_slices[1].copy_from_slice(in_slices[1]);
            }
            offset += chunk;
        }
    }

    // 4. Multi-channel hardware interleaving, soft saturation, and zeroing unused channels
    let mut max_l = 0.0f32;
    let mut max_r = 0.0f32;

    for i in 0..frames {
        let l = if is_bypassed {
            left_out[i]
        } else {
            soft_limit(left_out[i])
        };
        let r = if is_bypassed {
            right_out[i]
        } else {
            soft_limit(right_out[i])
        };
        max_l = max_l.max(l.abs());
        max_r = max_r.max(r.abs());

        let out_idx = i * channels;
        if channels == 1 {
            // Mono hardware output: combine Left and Right channels cleanly
            data[out_idx] = (l + r) * 0.5;
        } else {
            // Main hardware output: Left and Right (e.g. MacBook Pro speakers, headphones)
            data[out_idx] = l;
            data[out_idx + 1] = r;

            // Multi-channel interface: mirror stereo pair to channels 3/4 (headphone 2 / secondary out)
            if channels >= 4 {
                data[out_idx + 2] = l;
                data[out_idx + 3] = r;
            } else if channels == 3 {
                data[out_idx + 2] = 0.0;
            }
            // Explicitly zero-fill all remaining hardware channels (e.g. 5, 6 on 6-channel interfaces)
            for c in 4..channels {
                data[out_idx + c] = 0.0;
            }
        }
    }

    // 5. Update lock-free telemetry
    peak_l.store(max_l.to_bits(), Ordering::Relaxed);
    peak_r.store(max_r.to_bits(), Ordering::Relaxed);

    let spec_len = spectrum_samples.len();
    let capture_count = frames.min(spec_len);
    let keep = spec_len - capture_count;
    for i in 0..keep {
        let prev = spectrum_samples[i + capture_count].load(Ordering::Relaxed);
        spectrum_samples[i].store(prev, Ordering::Relaxed);
    }
    for i in 0..capture_count {
        let idx = frames - capture_count + i;
        let val = (left_out[idx] + right_out[idx]) * 0.5;
        spectrum_samples[keep + i].store(val.to_bits(), Ordering::Relaxed);
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos_virtual_rate_sync {
    use std::ffi::{CStr, c_char, c_void};

    #[repr(C)]
    struct AudioObjectPropertyAddress {
        m_selector: u32,
        m_scope: u32,
        m_element: u32,
    }

    const SYSTEM_OBJECT: u32 = 1;

    #[link(name = "CoreAudio", kind = "framework")]
    unsafe extern "C" {
        fn AudioObjectGetPropertyDataSize(
            in_object_id: u32,
            in_address: *const AudioObjectPropertyAddress,
            in_qualifier_data_size: u32,
            in_qualifier_data: *const c_void,
            out_data_size: *mut u32,
        ) -> i32;

        fn AudioObjectGetPropertyData(
            in_object_id: u32,
            in_address: *const AudioObjectPropertyAddress,
            in_qualifier_data_size: u32,
            in_qualifier_data: *const c_void,
            io_data_size: *mut u32,
            out_data: *mut c_void,
        ) -> i32;

        fn AudioObjectSetPropertyData(
            in_object_id: u32,
            in_address: *const AudioObjectPropertyAddress,
            in_qualifier_data_size: u32,
            in_qualifier_data: *const c_void,
            in_data_size: u32,
            in_data: *const c_void,
        ) -> i32;
    }

    fn get_device_name(device_id: u32) -> Option<String> {
        let addr = AudioObjectPropertyAddress {
            m_selector: u32::from_be_bytes(*b"name"),
            m_scope: u32::from_be_bytes(*b"glob"),
            m_element: 0,
        };
        let mut buf = [0 as c_char; 256];
        let mut size = buf.len() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(
                device_id,
                &addr,
                0,
                std::ptr::null(),
                &mut size,
                buf.as_mut_ptr() as *mut _,
            )
        };
        if status == 0 {
            let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
            cstr.to_str().ok().map(|s| s.trim().to_string())
        } else {
            None
        }
    }

    fn list_devices() -> Vec<(u32, String)> {
        let addr = AudioObjectPropertyAddress {
            m_selector: u32::from_be_bytes(*b"dev#"),
            m_scope: u32::from_be_bytes(*b"glob"),
            m_element: 0,
        };
        let mut size: u32 = 0;
        let status = unsafe {
            AudioObjectGetPropertyDataSize(SYSTEM_OBJECT, &addr, 0, std::ptr::null(), &mut size)
        };
        if status != 0 || size == 0 {
            return Vec::new();
        }
        let count = size as usize / std::mem::size_of::<u32>();
        let mut ids = vec![0u32; count];
        let status = unsafe {
            AudioObjectGetPropertyData(
                SYSTEM_OBJECT,
                &addr,
                0,
                std::ptr::null(),
                &mut size,
                ids.as_mut_ptr() as *mut _,
            )
        };
        if status != 0 {
            return Vec::new();
        }
        ids.into_iter()
            .filter_map(|id| get_device_name(id).map(|n| (id, n)))
            .collect()
    }

    pub fn get_device_sample_rate(device_id: u32) -> Option<f64> {
        let addr = AudioObjectPropertyAddress {
            m_selector: u32::from_be_bytes(*b"nsrt"),
            m_scope: u32::from_be_bytes(*b"glob"),
            m_element: 0,
        };
        let mut sr: f64 = 0.0;
        let mut size = std::mem::size_of::<f64>() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(
                device_id,
                &addr,
                0,
                std::ptr::null(),
                &mut size,
                &mut sr as *mut _ as *mut _,
            )
        };
        if status == 0 && sr > 0.0 {
            Some(sr)
        } else {
            None
        }
    }

    pub fn get_device_sample_rate_by_name(query: &str) -> Option<f64> {
        let q = query.to_lowercase();
        list_devices()
            .into_iter()
            .find(|(_, n)| n.to_lowercase().contains(&q))
            .and_then(|(id, _)| get_device_sample_rate(id))
    }

    pub fn sync_virtual_input_rate(target_sr: f64) {
        if target_sr <= 0.0 {
            return;
        }
        let addr = AudioObjectPropertyAddress {
            m_selector: u32::from_be_bytes(*b"nsrt"),
            m_scope: u32::from_be_bytes(*b"glob"),
            m_element: 0,
        };
        let size = std::mem::size_of::<f64>() as u32;
        for (id, name) in list_devices() {
            let l = name.to_lowercase();
            if l.contains("equaform") || l == "romueq" || l.contains("blackhole") {
                if let Some(cur) = get_device_sample_rate(id) {
                    if (cur - target_sr).abs() > 1.0 {
                        let _ = unsafe {
                            AudioObjectSetPropertyData(
                                id,
                                &addr,
                                0,
                                std::ptr::null(),
                                size,
                                &target_sr as *const _ as *const _,
                            )
                        };
                        for _ in 0..10 {
                            if let Some(now) = get_device_sample_rate(id) {
                                if (now - target_sr).abs() < 1.0 {
                                    break;
                                }
                            }
                            std::thread::sleep(std::time::Duration::from_millis(25));
                        }
                    }
                }
            }
        }
    }

    pub fn restore_macos_default_output(target_name: &str) {
        let q = target_name.to_lowercase();
        if q.contains("equaform")
            || q.contains("romueq")
            || q.contains("blackhole")
            || q.contains("eqmac")
        {
            return;
        }
        let devices = list_devices();
        let found = devices
            .iter()
            .find(|(_, n)| n == target_name)
            .or_else(|| devices.iter().find(|(_, n)| n.to_lowercase().contains(&q)));
        if let Some((dev_id, _)) = found {
            let addr = AudioObjectPropertyAddress {
                m_selector: u32::from_be_bytes(*b"dOut"),
                m_scope: u32::from_be_bytes(*b"glob"),
                m_element: 0,
            };
            let size = std::mem::size_of::<u32>() as u32;
            let _ = unsafe {
                AudioObjectSetPropertyData(
                    SYSTEM_OBJECT,
                    &addr,
                    0,
                    std::ptr::null(),
                    size,
                    dev_id as *const _ as *const _,
                )
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires physical macOS audio hardware (BlackHole 2ch / Komplete Audio)"]
    fn test_live_streaming_diagnostic() {
        let mut stream = AudioStream::new();
        stream.set_source_mode(AudioSourceMode::PassThrough);
        stream.set_input_device(Some("BlackHole 2ch".to_string()));
        stream.set_output_device(Some("Komplete Audio 6 MK2".to_string()));

        println!("Starting diagnostic live stream on Komplete Audio...");
        match stream.start() {
            Ok(()) => {
                println!(
                    "Stream started successfully! Target SR={}",
                    stream.sample_rate()
                );
                for sec in 1..=3 {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let (pl, pr) = stream.peak_levels_db();
                    println!("Second {sec}: peaks L={pl:.1} dB, R={pr:.1} dB");
                }
                stream.stop().unwrap();
                println!("Stream stopped.");
            }
            Err(e) => {
                println!("Stream start failed: {e}");
            }
        }
    }

    #[test]
    fn test_soft_limit_characteristics() {
        assert_eq!(soft_limit(0.0), 0.0);
        assert_eq!(soft_limit(0.5), 0.5);
        assert_eq!(soft_limit(-0.5), -0.5);
        assert_eq!(soft_limit(0.85), 0.85);
        assert_eq!(soft_limit(-0.85), -0.85);

        let at_1 = soft_limit(1.0);
        let at_2 = soft_limit(2.0);
        let at_10 = soft_limit(10.0);
        assert!(at_1 > 0.85 && at_1 < 1.0);
        assert!(at_2 >= at_1 && at_2 <= 1.0);
        assert!(at_10 >= at_2 && at_10 <= 1.0);
        assert_eq!(soft_limit(-1.0), -at_1);
    }

    #[test]
    fn test_realtime_threaded_streaming() {
        let (producer, consumer) = create_audio_ring_buffer(16384);
        let running = Arc::new(AtomicBool::new(true));
        let r_in = Arc::clone(&running);

        let input_thread = std::thread::spawn(move || {
            let chunk = vec![0.5f32; 1024]; // 512 frames * 2 channels
            while r_in.load(Ordering::Relaxed) {
                producer.push_interleaved(&chunk, 2);
                std::thread::sleep(std::time::Duration::from_micros(11610)); // 512 / 44100 = 11.61 ms
            }
        });

        let mut left_out = vec![0.0f32; 512];
        let mut right_out = vec![0.0f32; 512];
        let mut is_primed = false;
        let mut last_l = 0.0f32;
        let mut last_r = 0.0f32;

        for _sec in 0..100 {
            // 100 iterations of 512 frames ~ 1.16 seconds
            let available = consumer.available_read();
            if !is_primed {
                if available >= 2048 {
                    is_primed = true;
                } else {
                    left_out.fill(0.0);
                    right_out.fill(0.0);
                }
            }

            if is_primed {
                let frames_to_pop = if available > 3072 {
                    513
                } else if (511..1024).contains(&available) {
                    511
                } else {
                    512
                };

                let popped = consumer.pop_slice(
                    &mut left_out[..frames_to_pop],
                    &mut right_out[..frames_to_pop],
                );
                if frames_to_pop == 513 && popped == 513 {
                    left_out[511] = (left_out[511] + left_out[512]) * 0.5;
                    right_out[511] = (right_out[511] + right_out[512]) * 0.5;
                } else if frames_to_pop == 511 && popped == 511 {
                    left_out[511] = (left_out[510] + last_l) * 0.5;
                    right_out[511] = (right_out[510] + last_r) * 0.5;
                } else {
                    left_out[511] = 0.5;
                    right_out[511] = 0.5;
                }
                last_l = left_out[511];
                last_r = right_out[511];
            }
        }
        running.store(false, Ordering::Relaxed);
        let _ = input_thread.join();
    }

    #[test]
    fn test_drift_controller_underrun_recovery_without_stall() {
        let (producer, consumer) = create_audio_ring_buffer(16384);
        let mut drift = ContinuousPiDriftController::new(1.0);
        let mut left = vec![0.0f32; 512];
        let mut right = vec![0.0f32; 512];

        // 1. Prime the controller with 4096 frames
        for _ in 0..8 {
            let chunk = vec![0.33f32; 1024];
            producer.push_interleaved(&chunk, 2);
        }
        drift.read_from(&consumer, &mut left, &mut right, 512);
        assert!(drift.is_primed());
        assert_eq!(left[0], 0.33);

        // 2. Simulate severe USB starvation: drain the buffer down to 0
        while consumer.available_read() > 0 {
            let _ = consumer.pop();
        }
        assert_eq!(consumer.available_read(), 0);

        // 3. Output callback runs while buffer is starved
        drift.read_from(&consumer, &mut left, &mut right, 512);
        // It outputs silence for this starvation block
        assert_eq!(left[0], 0.0);
        // CRITICAL: Controller must REMAIN PRIMED to avoid the 46 ms reprime blackout!
        assert!(
            drift.is_primed(),
            "Controller must remain primed on starvation!"
        );

        // 4. Input packet arrives immediately on the next cycle (just 512 frames)
        let chunk = vec![0.88f32; 1024];
        producer.push_interleaved(&chunk, 2);

        // 5. Output callback runs immediately on the next block
        drift.read_from(&consumer, &mut left, &mut right, 512);
        // Audio playback resumes immediately without waiting for 4096 frames!
        assert!(drift.is_primed());
        assert!(
            (left[0] - 0.88).abs() < 1e-4,
            "Playback resumed instantly without stall!"
        );
    }

    #[test]
    fn test_streaming_simulation_100_blocks() {
        let (producer, consumer) = create_audio_ring_buffer(16384);
        let mut drift = ContinuousPiDriftController::default();
        let mut left_out = vec![0.0f32; 512];
        let mut right_out = vec![0.0f32; 512];

        // Simulate 100 blocks
        for _ in 0..100 {
            let chunk = vec![0.1f32; 1024];
            producer.push_interleaved(&chunk, 2);
            drift.read_from(&consumer, &mut left_out, &mut right_out, 512);
        }
    }

    #[test]
    fn test_drift_controller_rate_conversions() {
        // 1. Nominal 44.1 kHz -> 44.1 kHz (ratio = 1.0)
        let (producer, consumer) = create_audio_ring_buffer(16384);
        let mut drift_1to1 = ContinuousPiDriftController::new(1.0);
        let mut left = vec![0.0f32; 512];
        let mut right = vec![0.0f32; 512];

        // Push initial pre-buffer (at least TARGET_FRAMES = 4096)
        for _ in 0..8 {
            let chunk = vec![0.5f32; 1024];
            producer.push_interleaved(&chunk, 2);
        }
        assert!(consumer.available_read() >= ContinuousPiDriftController::TARGET_FRAMES);

        for _ in 0..50 {
            let chunk = vec![0.5f32; 1024];
            producer.push_interleaved(&chunk, 2);
            drift_1to1.read_from(&consumer, &mut left, &mut right, 512);
            assert_eq!(left[0], 0.5);
            assert_eq!(right[0], 0.5);
        }
        assert!(drift_1to1.is_primed());

        // 2. 48.0 kHz input -> 44.1 kHz output (ratio = 48000.0 / 44100.0 = 1.088435)
        let (p_48to44, c_48to44) = create_audio_ring_buffer(16384);
        let mut drift_48to44 = ContinuousPiDriftController::new(48000.0 / 44100.0);
        // Pre-fill to TARGET_FRAMES
        for _ in 0..8 {
            let in_chunk = vec![0.25f32; 557 * 2];
            p_48to44.push_interleaved(&in_chunk, 2);
        }
        for _ in 0..50 {
            let in_chunk = vec![0.25f32; 557 * 2]; // ~557 input frames per 512 output frames
            p_48to44.push_interleaved(&in_chunk, 2);
            drift_48to44.read_from(&c_48to44, &mut left, &mut right, 512);
        }
        assert!(drift_48to44.is_primed());
        // Sound must be non-zero and stable around 0.25
        assert!((left[256] - 0.25).abs() < 1e-4);

        // 3. 44.1 kHz input -> 48.0 kHz output (ratio = 44100.0 / 48000.0 = 0.91875)
        let (p_44to48, c_44to48) = create_audio_ring_buffer(16384);
        let mut drift_44to48 = ContinuousPiDriftController::new(44100.0 / 48000.0);
        for _ in 0..9 {
            let in_chunk = vec![0.75f32; 470 * 2];
            p_44to48.push_interleaved(&in_chunk, 2);
        }
        for _ in 0..50 {
            let in_chunk = vec![0.75f32; 470 * 2]; // ~470 input frames per 512 output frames
            p_44to48.push_interleaved(&in_chunk, 2);
            drift_44to48.read_from(&c_44to48, &mut left, &mut right, 512);
        }
        assert!(drift_44to48.is_primed());
        assert!((left[256] - 0.75).abs() < 1e-4);
    }

    #[test]
    fn test_drift_controller_sine_phase_continuity_across_blocks() {
        let (producer, consumer) = create_audio_ring_buffer(16384);
        // Simulate asynchronous USB crystal drift (+350 ppm)
        let mut drift = ContinuousPiDriftController::new(1.00035);
        let mut left = vec![0.0f32; 512];
        let mut right = vec![0.0f32; 512];

        let sr = 44100.0f64;
        let freq = 440.0f64;
        let max_theoretical_step = (2.0 * std::f64::consts::PI * freq / sr) as f32 * 1.05;
        let mut sample_idx = 0usize;

        // Pre-fill 6000 frames of continuous 440 Hz sine wave
        for _ in 0..6000 {
            let s = (2.0 * std::f64::consts::PI * freq * (sample_idx as f64) / sr).sin() as f32;
            assert!(producer.push(s, -s));
            sample_idx += 1;
        }

        let mut prev_out_l: Option<f32> = None;
        for _block in 0..100 {
            // Replenish ~512 frames per block
            for _ in 0..512 {
                let s = (2.0 * std::f64::consts::PI * freq * (sample_idx as f64) / sr).sin() as f32;
                let _ = producer.push(s, -s);
                sample_idx += 1;
            }
            drift.read_from(&consumer, &mut left, &mut right, 512);
            for &sample in &left {
                if let Some(prev) = prev_out_l {
                    let delta = (sample - prev).abs();
                    assert!(
                        delta <= max_theoretical_step,
                        "Phase discontinuity click detected across block boundary! delta={delta}, max={max_theoretical_step}"
                    );
                }
                prev_out_l = Some(sample);
            }
        }
        assert_eq!(drift.underrun_count, 0);
    }

    #[test]
    #[ignore = "Requires physical macOS 'Haut-parleurs' audio device"]
    fn test_audio_stream_start_diagnostic() {
        let host = cpal::default_host();
        let mac_speakers = host
            .devices()
            .unwrap()
            .find(|d| {
                d.name()
                    .map(|n| n.contains("Haut-parleurs"))
                    .unwrap_or(false)
            })
            .expect("Haut-parleurs not found");
        println!("Found Mac speakers: {:?}", mac_speakers.name());
        let config = StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(44100),
            buffer_size: cpal::BufferSize::Default,
        };
        let stream = mac_speakers.build_output_stream(
            &config,
            |data: &mut [f32], _| {
                data.fill(0.0);
            },
            |e| eprintln!("err: {e}"),
            None,
        );
        println!(
            "Building output stream on Mac speakers: {:?}",
            stream.is_ok()
        );
        assert!(stream.is_ok());
        let mac_speakers = host
            .devices()
            .unwrap()
            .find(|d| {
                d.name()
                    .map(|n| n.contains("Haut-parleurs"))
                    .unwrap_or(false)
            })
            .expect("Haut-parleurs not found");
        println!("Found Mac speakers: {:?}", mac_speakers.name());
        let config = StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(44100),
            buffer_size: cpal::BufferSize::Default,
        };
        let stream = mac_speakers.build_output_stream(
            &config,
            |data: &mut [f32], _| {
                data.fill(0.0);
            },
            |e| eprintln!("err: {e}"),
            None,
        );
        println!(
            "Building output stream on Mac speakers: {:?}",
            stream.is_ok()
        );
        assert!(stream.is_ok());
        if let Some(out) = host.default_output_device() {
            println!(
                "Default output: {:?}, config: {:?}",
                out.name(),
                out.default_output_config()
            );
            if let Ok(configs) = out.supported_output_configs() {
                for c in configs {
                    println!("  Supported config: {c:?}");
                }
            }
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
        stream.set_output_device(Some("Haut-parleurs MacBook Pro".to_string()));
        match stream.start() {
            Ok(()) => {
                println!(
                    "Audio stream to Haut-parleurs MacBook Pro started successfully! Active: {}",
                    stream.is_active()
                );
                std::thread::sleep(std::time::Duration::from_millis(100));
                stream.stop().unwrap();
            }
            Err(e) => {
                println!("Audio stream to Haut-parleurs MacBook Pro failed to start: {e}");
            }
        }
    }
}
