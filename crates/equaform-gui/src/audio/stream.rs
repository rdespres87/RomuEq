//! CPAL audio stream lifecycle, device enumeration, and lock-free parameter transport.

#[cfg(target_os = "macos")]
use super::macos_virtual_rate_sync;
use super::render::process_output_block;
use super::{
    AudioRenderContext, AudioScratchBuffers, AudioSourceMode, AudioTelemetry,
    ContinuousPiDriftController, MAX_SCRATCH_FRAMES, StreamFormatSpec, soft_limit,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use equaform_common::EqParameters;
use equaform_common::audio_ring::{AudioRingConsumer, create_audio_ring_buffer};
use equaform_common::triple_buffer::{
    TripleBufferConsumer, TripleBufferProducer, create_triple_buffer,
};
use equaform_engine::EngineLifecycle;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicUsize, Ordering};

/// Pre-allocated state moved into the CPAL output callback closure (zero RT allocations).
struct OutputCallbackState {
    engine: Box<EngineLifecycle>,
    param_consumer: TripleBufferConsumer<EqParameters>,
    audio_consumer: AudioRingConsumer,
    drift: ContinuousPiDriftController,
    is_running: Arc<AtomicBool>,
    is_muted: Arc<AtomicBool>,
    source_mode: Arc<AtomicU8>,
    peak_l: Arc<AtomicU32>,
    peak_r: Arc<AtomicU32>,
    spectrum_samples: Arc<Vec<AtomicU32>>,
    phase: f64,
    mute_gain: f32,
    left_in: Vec<f32>,
    right_in: Vec<f32>,
    left_out: Vec<f32>,
    right_out: Vec<f32>,
}

impl OutputCallbackState {
    fn process_f32(&mut self, data: &mut [f32], format: StreamFormatSpec) {
        let mut ctx = AudioRenderContext {
            format,
            engine: &mut self.engine,
            param_consumer: &mut self.param_consumer,
            audio_consumer: &self.audio_consumer,
            drift: &mut self.drift,
            telemetry: AudioTelemetry {
                is_running: &self.is_running,
                is_muted: &self.is_muted,
                source_mode: &self.source_mode,
                peak_l: &self.peak_l,
                peak_r: &self.peak_r,
                spectrum_samples: &self.spectrum_samples,
            },
            phase: &mut self.phase,
            mute_gain: &mut self.mute_gain,
            scratch: AudioScratchBuffers {
                left_in: &mut self.left_in,
                right_in: &mut self.right_in,
                left_out: &mut self.left_out,
                right_out: &mut self.right_out,
            },
        };
        process_output_block(data, &mut ctx);
    }
}

pub struct AudioStream {
    _output_stream: Option<Stream>,
    _input_stream: Option<Stream>,
    is_active: bool,
    is_running: Arc<AtomicBool>,
    is_muted: Arc<AtomicBool>,
    source_mode: Arc<AtomicU8>,
    selected_output_device: Option<String>,
    selected_input_device: Option<String>,
    param_producer: Option<TripleBufferProducer<EqParameters>>,
    current_params: EqParameters,
    delivered_params: Option<EqParameters>,
    peak_l: Arc<AtomicU32>,
    peak_r: Arc<AtomicU32>,
    spectrum_samples: Arc<Vec<AtomicU32>>,
    stream_errors: Arc<AtomicUsize>,
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
            is_muted: Arc::new(AtomicBool::new(false)),
            source_mode: Arc::new(AtomicU8::new(AudioSourceMode::PassThrough as u8)),
            sample_rate,
            selected_input_device: None,
            selected_output_device: None,
            param_producer: None,
            current_params,
            delivered_params: None,
            peak_l: Arc::new(AtomicU32::new(0)),
            peak_r: Arc::new(AtomicU32::new(0)),
            spectrum_samples: Arc::new((0..2048).map(|_| AtomicU32::new(0)).collect()),
            stream_errors: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Number of asynchronous audio stream errors reported by CPAL error callbacks.
    pub fn stream_error_count(&self) -> usize {
        self.stream_errors.load(Ordering::Relaxed)
    }

    /// Shared error counter for external monitoring or telemetry.
    pub fn stream_errors(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.stream_errors)
    }

    pub fn available_output_devices() -> Vec<String> {
        let host = cpal::default_host();
        let mut out = Vec::new();
        if let Ok(devices) = host.devices() {
            for d in devices {
                if let Ok(name) = d.name() {
                    let lower = name.to_lowercase();
                    if lower.contains("equaform") || lower == "romueq" || lower.contains("romueq ")
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

    /// Mute or unmute output smoothly on the audio thread (5 ms ramp, zero stream teardown).
    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::Release);
    }

    /// Returns `true` if output is currently muted.
    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::Acquire)
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
        if let Some(ref mut producer) = self.param_producer {
            producer.publish(params);
            self.delivered_params = Some(params);
        } else {
            self.delivered_params = None;
        }
        Ok(())
    }

    /// Ensures the audio thread has received the latest `params`.
    pub fn ensure_parameters_synced(&mut self, params: EqParameters) {
        self.current_params = params;
        if self.delivered_params != Some(params) {
            if let Some(ref mut producer) = self.param_producer {
                producer.publish(params);
                self.delivered_params = Some(params);
            }
        }
    }

    fn resolve_output_device(&self, host: &cpal::Host) -> Result<cpal::Device, String> {
        let device = if let Some(ref name) = self.selected_output_device {
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
        };
        device.ok_or_else(|| "No audio output device found on system".to_string())
    }

    fn resolve_output_format(&self, output_device: &cpal::Device) -> (usize, f32, SampleFormat) {
        if let Ok(cfg) = output_device.default_output_config() {
            return (
                (cfg.channels() as usize).max(1),
                cfg.sample_rate().0 as f32,
                cfg.sample_format(),
            );
        }
        #[cfg(target_os = "macos")]
        let hw_sr = {
            let dev_name = self
                .selected_output_device
                .clone()
                .or_else(|| output_device.name().ok());
            dev_name
                .as_deref()
                .and_then(macos_virtual_rate_sync::get_device_sample_rate_by_name)
                .or_else(macos_virtual_rate_sync::get_default_output_sample_rate)
                .unwrap_or(44100.0) as f32
        };
        #[cfg(not(target_os = "macos"))]
        let hw_sr = 44100.0_f32;
        (2, hw_sr, SampleFormat::F32)
    }

    fn resolve_input_device(&self, host: &cpal::Host) -> Option<cpal::Device> {
        if let Some(ref name) = self.selected_input_device {
            host.input_devices().ok().and_then(|mut devs| {
                devs.find(|d| {
                    d.name()
                        .map(|n| n.trim() == name.trim() || n.contains(name))
                        .unwrap_or(false)
                })
            })
        } else {
            host.input_devices().ok().and_then(|devs| {
                let dev_list: Vec<_> = devs.collect();
                dev_list
                    .iter()
                    .find(|d| {
                        d.name()
                            .map(|n| n.contains("Equaform") || n.contains("RomuEq"))
                            .unwrap_or(false)
                    })
                    .cloned()
            })
        }
    }

    fn compute_rate_ratio(input_device: Option<&cpal::Device>, output_sr: f32) -> f64 {
        let Some(in_dev) = input_device else {
            return 1.0;
        };
        if let Ok(in_cfg) = in_dev.default_input_config() {
            let in_sr = f64::from(in_cfg.sample_rate().0);
            let out_sr = f64::from(output_sr);
            let ratio = in_sr / out_sr;
            println!(
                "[AudioStream] Audio chain rate sync: Input={in_sr:.0} Hz, Output={out_sr:.0} Hz, RateRatio={ratio:.6}"
            );
            ratio
        } else {
            1.0
        }
    }

    fn build_output_stream(
        &self,
        output_device: &cpal::Device,
        format: StreamFormatSpec,
        sample_format: SampleFormat,
        mut state: OutputCallbackState,
    ) -> Result<cpal::Stream, String> {
        let stream_config = StreamConfig {
            channels: format.channels as u16,
            sample_rate: cpal::SampleRate(format.sample_rate as u32),
            buffer_size: cpal::BufferSize::Default,
        };
        let stream_errors_out = Arc::clone(&self.stream_errors);
        let err_fn = move |_err| {
            stream_errors_out.fetch_add(1, Ordering::Relaxed);
        };

        match sample_format {
            SampleFormat::F32 => output_device
                .build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        state.process_f32(data, format);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Failed to build F32 output stream: {e}")),
            SampleFormat::I16 => {
                let mut float_scratch = vec![0.0f32; MAX_SCRATCH_FRAMES * format.channels.max(2)];
                output_device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            let frames = (data.len() / format.channels).min(MAX_SCRATCH_FRAMES);
                            let active_len = frames * format.channels;
                            let scratch_slice = &mut float_scratch[..active_len];
                            state.process_f32(scratch_slice, format);
                            for (out, &sample) in
                                data[..active_len].iter_mut().zip(scratch_slice.iter())
                            {
                                *out = (soft_limit(sample) * 32767.0) as i16;
                            }
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("Failed to build I16 output stream: {e}"))
            }
            sample_fmt => Err(format!("Unsupported audio sample format: {sample_fmt:?}")),
        }
    }

    fn start_input_stream(
        &self,
        input_device: cpal::Device,
        producer: equaform_common::audio_ring::AudioRingProducer,
    ) -> Result<Option<cpal::Stream>, String> {
        let in_config = input_device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {e}"))?;
        let in_channels = in_config.channels() as usize;
        let in_stream_config = StreamConfig {
            channels: in_channels as u16,
            sample_rate: in_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };
        let err_counter = Arc::clone(&self.stream_errors);
        let in_res = match in_config.sample_format() {
            SampleFormat::F32 => input_device.build_input_stream(
                &in_stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    producer.push_interleaved(data, in_channels);
                },
                move |_err| {
                    err_counter.fetch_add(1, Ordering::Relaxed);
                },
                None,
            ),
            SampleFormat::I16 => {
                let mut conv_scratch = vec![0.0f32; MAX_SCRATCH_FRAMES * in_channels.max(2)];
                input_device.build_input_stream(
                    &in_stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let frames = (data.len() / in_channels).min(MAX_SCRATCH_FRAMES);
                        let active_len = frames * in_channels;
                        for (dst, &src) in conv_scratch[..active_len].iter_mut().zip(data.iter()) {
                            *dst = src as f32 / 32768.0;
                        }
                        producer.push_interleaved(&conv_scratch[..active_len], in_channels);
                    },
                    move |_err| {
                        err_counter.fetch_add(1, Ordering::Relaxed);
                    },
                    None,
                )
            }
            _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
        };

        match in_res {
            Ok(stream) => match stream.play() {
                Ok(()) => {
                    println!("[AudioStream] Input stream started successfully!");
                    Ok(Some(stream))
                }
                Err(e) => {
                    eprintln!("[AudioStream] Failed to play input stream: {e}");
                    Ok(None)
                }
            },
            Err(e) => {
                eprintln!("[AudioStream] Failed to build input stream: {e}");
                Ok(None)
            }
        }
    }

    /// Start audio processing and playback through configured or default audio devices.
    pub fn start(&mut self) -> Result<(), String> {
        if self.is_active {
            return Ok(());
        }

        let host = cpal::default_host();
        let output_device = self.resolve_output_device(&host)?;
        let (channels, sample_rate, sample_format) = self.resolve_output_format(&output_device);

        #[cfg(target_os = "macos")]
        macos_virtual_rate_sync::sync_virtual_input_rate(sample_rate as f64);

        self.sample_rate = sample_rate;
        let block_size = 512;
        let (audio_producer, audio_consumer) = create_audio_ring_buffer(16384);
        let (param_producer, param_consumer) = create_triple_buffer(self.current_params);
        self.param_producer = Some(param_producer);
        self.delivered_params = Some(self.current_params);

        self.is_running.store(true, Ordering::Release);
        let mut engine = Box::new(EngineLifecycle::new());
        let _ = engine.prepare(sample_rate as f64, block_size, 2);
        engine.set_parameters(self.current_params);

        let input_device_opt = self.resolve_input_device(&host);
        println!(
            "[AudioStream] Input device resolved: {:?}",
            input_device_opt.as_ref().and_then(|d| d.name().ok())
        );
        let rate_ratio = Self::compute_rate_ratio(input_device_opt.as_ref(), sample_rate);
        let cb_state = OutputCallbackState {
            engine,
            param_consumer,
            audio_consumer,
            drift: ContinuousPiDriftController::new(rate_ratio),
            is_running: Arc::clone(&self.is_running),
            is_muted: Arc::clone(&self.is_muted),
            source_mode: Arc::clone(&self.source_mode),
            peak_l: Arc::clone(&self.peak_l),
            peak_r: Arc::clone(&self.peak_r),
            spectrum_samples: Arc::clone(&self.spectrum_samples),
            phase: 0.0,
            mute_gain: if self.is_muted.load(Ordering::Acquire) {
                0.0
            } else {
                1.0
            },
            left_in: vec![0.0f32; MAX_SCRATCH_FRAMES],
            right_in: vec![0.0f32; MAX_SCRATCH_FRAMES],
            left_out: vec![0.0f32; MAX_SCRATCH_FRAMES],
            right_out: vec![0.0f32; MAX_SCRATCH_FRAMES],
        };

        let format = StreamFormatSpec {
            channels,
            sample_rate,
            block_size,
        };
        let output_stream =
            self.build_output_stream(&output_device, format, sample_format, cb_state)?;
        let input_stream = match input_device_opt {
            Some(in_dev) => self.start_input_stream(in_dev, audio_producer)?,
            None => None,
        };

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

    /// Copies the latest output samples into a pre-allocated vector for zero-allocation GUI analysis.
    pub fn copy_latest_output_samples(&self, dest: &mut Vec<f32>) {
        dest.clear();
        dest.extend(
            self.spectrum_samples
                .iter()
                .map(|s| f32::from_bits(s.load(Ordering::Relaxed))),
        );
    }

    /// Resets the spectrum samples buffer to zero to prevent stale telemetry display on wake-up.
    pub fn clear_spectrum_buffer(&self) {
        for s in self.spectrum_samples.iter() {
            s.store(0, Ordering::Relaxed);
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
