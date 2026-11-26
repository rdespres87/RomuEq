//! Shared platform-independent editor host for native plugin backends (Cocoa & Win32).
//!
//! Centralizes:
//! - Synchronous future polling for immediate `wgpu` adapter/device creation
//! - `wgpu` surface/device/renderer lifecycle and `egui-wgpu` frame presentation (`WgpuSurfaceHost`)
//! - Lock-free audio telemetry & FFT spectrum collection, adaptive 60/15/0 FPS throttling,
//!   `ParamSnapshot` construction, preset CRUD, and CLAP host flush (`SharedEditorState`)

use std::collections::VecDeque;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::time::Instant;

use clap_sys::ext::params::{CLAP_EXT_PARAMS, clap_host_params};
use clap_sys::host::clap_host;
use equaform_analysis::{SpectrumConfig, SpectrumProcessor};
use equaform_common::parameter::{band, global};
use equaform_common::state::ProjectState;
use equaform_common::{EqParameters, MAX_BANDS, ParamId};
use equaform_gui::preset_manager::PresetManager;
use equaform_gui::presets_io::{
    persist_preset_to_disk, populate_factory_presets, remove_preset_from_disk,
};
use equaform_gui::workspace_ui::{
    HostEnvironment, WorkspaceAction, WorkspaceUiContext, WorkspaceUiState, render_workspace,
};
use equaform_gui::{AnalysisSnapshot, GuiCommandBatch, ParamSnapshot, ParamValue};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::EquaformSharedInner;
use crate::editor::{EDITOR_HEIGHT, EDITOR_WIDTH};
use crate::params_map::{
    PARAM_COUNT_USIZE, apply_param_to_eq, get_param_from_eq, slot_to_param_id,
};

pub const FFT_SIZE: usize = 1024;

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.070,
    g: 0.078,
    b: 0.094,
    a: 1.0,
};

/// Minimal synchronous poll helper for `wgpu` adapter/device futures.
pub fn block_on_sync<F: std::future::Future>(fut: F) -> F::Output {
    fn noop_clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    fn noop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);
    // SAFETY: `VTABLE` functions are pure no-ops that never dereference the null data pointer.
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut pinned = std::pin::pin!(fut);
    loop {
        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// Converts a lock-free `EqParameters` snapshot into a `ParamSnapshot` for the workspace UI.
pub fn eq_to_param_snapshot(eq: &EqParameters, auto_gain_comp_db: f32) -> ParamSnapshot {
    let mut vals = Vec::with_capacity(PARAM_COUNT_USIZE + 1 + MAX_BANDS);
    vals.push(ParamValue {
        param_id: ParamId(global::BYPASS),
        value: if eq.global.bypass { 1.0 } else { 0.0 },
    });
    vals.push(ParamValue {
        param_id: ParamId(global::OUTPUT_GAIN_DB),
        value: eq.global.output_gain_db,
    });
    vals.push(ParamValue {
        param_id: ParamId(global::AUTO_GAIN),
        value: if eq.global.auto_gain { 1.0 } else { 0.0 },
    });
    vals.push(ParamValue {
        param_id: ParamId(global::PROCESSING_MODE),
        value: eq.global.processing_mode.to_raw() as f32,
    });
    vals.push(ParamValue {
        param_id: ParamId(global::AUTO_GAIN_COMPENSATION_DB),
        value: auto_gain_comp_db,
    });
    vals.push(ParamValue {
        param_id: ParamId(global::AUTO_GAIN_MODE),
        value: eq.global.auto_gain_mode.to_raw() as f32,
    });

    for (i, b) in eq.bands.iter().enumerate().take(MAX_BANDS) {
        let base = 100 + (i as u32) * 100;
        vals.push(ParamValue {
            param_id: ParamId(base + band::ENABLED),
            value: if b.enabled { 1.0 } else { 0.0 },
        });
        vals.push(ParamValue {
            param_id: ParamId(base + band::FILTER_TYPE),
            value: b.filter_type.to_raw() as f32,
        });
        vals.push(ParamValue {
            param_id: ParamId(base + band::FREQUENCY_HZ),
            value: b.frequency_hz,
        });
        vals.push(ParamValue {
            param_id: ParamId(base + band::GAIN_DB),
            value: b.gain_db,
        });
        vals.push(ParamValue {
            param_id: ParamId(base + band::Q),
            value: b.q,
        });
        vals.push(ParamValue {
            param_id: ParamId(base + band::SLOPE),
            value: b.slope.to_raw() as f32,
        });
        vals.push(ParamValue {
            param_id: ParamId(base + band::SOLO),
            value: if eq.is_band_solo(i) { 1.0 } else { 0.0 },
        });
        vals.push(ParamValue {
            param_id: ParamId(base + band::ROUTING),
            value: eq.band_routing(i).to_raw() as f32,
        });
    }

    ParamSnapshot::new(vals)
}

/// Configuration for creating a [`WgpuSurfaceHost`].
#[derive(Debug, Clone, Copy)]
pub struct WgpuSurfaceConfig {
    pub backends: wgpu::Backends,
    pub raw_display_handle: RawDisplayHandle,
    pub raw_window_handle: RawWindowHandle,
    pub width_px: u32,
    pub height_px: u32,
    pub pixels_per_point: f32,
    pub allow_fallback_adapter: bool,
    pub device_label: &'static str,
}

/// Shared `wgpu` surface, device, queue, and `egui-wgpu` renderer state.
pub struct WgpuSurfaceHost {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    renderer: egui_wgpu::Renderer,
    pixels_per_point: f32,
}

impl WgpuSurfaceHost {
    /// Initializes a `wgpu` surface and `egui_wgpu::Renderer` attached to a native window handle.
    ///
    /// # Safety
    /// `config.raw_display_handle` and `config.raw_window_handle` must remain valid for the entire
    /// lifetime of the returned `WgpuSurfaceHost`.
    pub unsafe fn new(config: WgpuSurfaceConfig) -> Result<Self, String> {
        let WgpuSurfaceConfig {
            backends,
            raw_display_handle,
            raw_window_handle,
            width_px,
            height_px,
            pixels_per_point,
            allow_fallback_adapter,
            device_label,
        } = config;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        // SAFETY: Caller guarantees `raw_display_handle` and `raw_window_handle` remain valid
        // for the entire lifetime of the returned `WgpuSurfaceHost`.
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle,
                    raw_window_handle,
                })
                .map_err(|e| format!("create_surface_unsafe: {e}"))?
        };

        let mut adapter = block_on_sync(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }));

        if adapter.is_none() && allow_fallback_adapter {
            adapter = block_on_sync(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: Some(&surface),
                force_fallback_adapter: true,
            }));
        }

        let adapter = adapter.ok_or_else(|| "No compatible wgpu adapter found".to_string())?;

        let (device, queue) = block_on_sync(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some(device_label),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| format!("request_device: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                !f.is_srgb()
                    && matches!(
                        f,
                        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                    )
            })
            .or_else(|| caps.formats.first().copied())
            .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width_px.max(1),
            height: height_px.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Opaque),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);

        Ok(Self {
            _instance: instance,
            surface,
            device,
            queue,
            surface_config,
            renderer,
            pixels_per_point: pixels_per_point.max(1.0),
        })
    }

    #[inline]
    pub fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    /// Updates the swapchain dimensions and DPI scale factor if changed.
    pub fn reconfigure_size(&mut self, width_px: u32, height_px: u32, pixels_per_point: f32) {
        if width_px == 0 || height_px == 0 {
            return;
        }
        self.pixels_per_point = pixels_per_point.max(1.0);
        if width_px != self.surface_config.width || height_px != self.surface_config.height {
            self.surface_config.width = width_px;
            self.surface_config.height = height_px;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    /// Tessellates and presents an `egui::FullOutput` frame to the `wgpu` surface.
    pub fn render_egui(&mut self, egui_ctx: &egui::Context, full_output: egui::FullOutput) {
        let output_tex = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                return;
            }
            Err(_) => return,
        };

        let tex_view = output_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: self.pixels_per_point,
        };

        let paint_jobs = egui_ctx.tessellate(full_output.shapes, self.pixels_per_point);

        for (id, delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("EquaformPluginEncoder"),
            });

        let extra_cmds = self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("EquaformPluginRenderPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &tex_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = render_pass.forget_lifetime();
            self.renderer
                .render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }

        let mut cmd_bufs = extra_cmds;
        cmd_bufs.push(encoder.finish());
        self.queue.submit(cmd_bufs);
        output_tex.present();
    }
}

/// Platform-independent, GPU-free editor state shared by Cocoa and Win32 backends.
pub struct SharedEditorState {
    shared: Arc<EquaformSharedInner>,
    host: Option<*const clap_host>,
    host_description: &'static str,
    pub egui_ctx: egui::Context,
    start_time: Instant,
    pub pending_events: Vec<egui::Event>,
    pub modifiers: egui::Modifiers,
    pub ui_state: WorkspaceUiState,
    preset_manager: PresetManager,
    current_preset: String,
    status_message: Option<String>,
    spectrum_processor: SpectrumProcessor,
    spectrum_frame_num: u64,
    last_spectrum_sr: f32,
    fft_ring: VecDeque<f32>,
    analysis: AnalysisSnapshot,
    fft_sample_buffer: Vec<f32>,
    frame_counter: u32,
    was_in_zero_fps: bool,
    silence_frames: u32,
    idle_transport_frames: u32,
}

impl SharedEditorState {
    /// Creates a new shared editor state with factory presets and pre-allocated FFT buffers.
    pub fn new(
        shared: Arc<EquaformSharedInner>,
        host: Option<*const clap_host>,
        host_description: &'static str,
    ) -> Self {
        let mut preset_manager = PresetManager::new();
        populate_factory_presets(&mut preset_manager);

        let sr = shared.sample_rate().max(8000.0);
        let spectrum_processor = SpectrumProcessor::new(SpectrumConfig {
            fft_size: FFT_SIZE,
            smoothing_factor: 0.4,
            window_type: equaform_analysis::WindowType::Hann,
            log_bin_mode: equaform_analysis::LogBinMode::None,
            min_db: -120.0,
        });

        let analysis = AnalysisSnapshot {
            spectrum_data: Vec::with_capacity(513),
            ..Default::default()
        };

        Self {
            shared,
            host,
            host_description,
            egui_ctx: egui::Context::default(),
            start_time: Instant::now(),
            pending_events: Vec::new(),
            modifiers: egui::Modifiers::default(),
            ui_state: WorkspaceUiState {
                language: equaform_gui::Language::load_saved(),
                ..Default::default()
            },
            preset_manager,
            current_preset: String::new(),
            status_message: None,
            spectrum_processor,
            spectrum_frame_num: 0,
            last_spectrum_sr: sr,
            fft_ring: VecDeque::with_capacity(FFT_SIZE * 2),
            analysis,
            fft_sample_buffer: Vec::with_capacity(FFT_SIZE),
            frame_counter: 0,
            was_in_zero_fps: false,
            silence_frames: 0,
            idle_transport_frames: 0,
        }
    }

    /// Marks the editor as having entered 0 FPS dormancy so the ring buffer resyncs on wake-up.
    #[inline]
    pub fn mark_zero_fps(&mut self) {
        self.was_in_zero_fps = true;
    }

    #[inline]
    pub fn has_pending_events(&self) -> bool {
        !self.pending_events.is_empty()
    }

    /// Adaptive frame throttling: 60 FPS when active or events are pending, 15 FPS when idle.
    /// Returns `true` if the current tick should be skipped.
    pub fn should_skip_frame(&mut self, is_active: bool) -> bool {
        let has_pending_events = !self.pending_events.is_empty();
        self.frame_counter = self.frame_counter.wrapping_add(1);

        if has_pending_events {
            self.frame_counter = 0; // Force immediate frame
            false
        } else {
            !is_active && (self.frame_counter % 4 != 0)
        }
    }

    /// Drains audio samples from the lock-free ring buffer and updates FFT spectrum & meters.
    /// Returns `(sample_rate, peak_levels_db)`.
    pub fn collect_audio_and_spectrum(&mut self) -> (f32, (f32, f32)) {
        let consumer = self.shared.ring_consumer();
        if self.was_in_zero_fps || consumer.available_read() >= (consumer.capacity() / 2) {
            consumer.clear();
            self.fft_ring.clear();
            self.was_in_zero_fps = false;
        }

        let sample_rate = self.shared.sample_rate().max(8000.0);
        if (sample_rate - self.last_spectrum_sr).abs() > 1.0 {
            self.spectrum_processor.reconfigure(SpectrumConfig {
                fft_size: FFT_SIZE,
                smoothing_factor: 0.4,
                window_type: equaform_analysis::WindowType::Hann,
                log_bin_mode: equaform_analysis::LogBinMode::None,
                min_db: -120.0,
            });
            self.last_spectrum_sr = sample_rate;
        }

        let mut tmp_l = [0.0f32; 512];
        let mut tmp_r = [0.0f32; 512];
        let mut popped_any = false;
        loop {
            let n = consumer.pop_slice(&mut tmp_l, &mut tmp_r);
            if n == 0 {
                break;
            }
            popped_any = true;
            for (&l, &r) in tmp_l[..n].iter().zip(&tmp_r[..n]) {
                self.fft_ring.push_back(0.5 * (l + r));
                if self.fft_ring.len() > FFT_SIZE {
                    self.fft_ring.pop_front();
                }
            }
        }

        if !popped_any {
            self.idle_transport_frames = self.idle_transport_frames.saturating_add(1);
            if self.idle_transport_frames > 4 {
                // Host playback is paused or stopped: decay ring samples towards silence
                for s in self.fft_ring.iter_mut() {
                    *s *= 0.85;
                }
            }
        } else {
            self.idle_transport_frames = 0;
        }

        let peak_levels_db = self.shared.peak_levels_db();
        self.analysis.peak[0] = peak_levels_db.0;
        self.analysis.peak[1] = peak_levels_db.1;
        self.analysis.rms[0] = peak_levels_db.0;
        self.analysis.rms[1] = peak_levels_db.1;

        let max_peak = peak_levels_db.0.max(peak_levels_db.1);
        let is_audio_silent = max_peak <= -90.0;
        let is_idle = (self.idle_transport_frames >= 15) || is_audio_silent;

        if is_idle {
            self.silence_frames = self.silence_frames.saturating_add(1);
        } else if max_peak > -84.0 {
            self.silence_frames = 0;
        }

        if self.fft_ring.len() >= FFT_SIZE && self.silence_frames < 15 {
            self.fft_sample_buffer.clear();
            self.fft_sample_buffer.extend(self.fft_ring.iter().copied());
            self.spectrum_frame_num += 1;
            let frame = self.spectrum_processor.process_ref(
                &self.fft_sample_buffer,
                sample_rate as f64,
                self.spectrum_frame_num,
            );
            // Subtract unnormalized DFT gain 20*log10(N/2) (~54.18 dB for N=1024) so 1.0 full-scale = 0 dBFS
            let dft_norm_db = 20.0 * ((frame.fft_size as f64) * 0.5).log10();
            self.analysis.spectrum_data.clear();
            self.analysis
                .spectrum_data
                .extend((0..frame.num_bins).map(|b| {
                    [
                        frame.bin_frequency(b),
                        (frame.magnitudes_db[b] as f64) - dft_norm_db,
                    ]
                }));
        } else {
            self.analysis.spectrum_data.clear();
        }

        (sample_rate, peak_levels_db)
    }

    /// Runs a single `egui` frame, syncs parameter changes & workspace actions back to the host,
    /// and returns the resulting `egui::FullOutput` for presentation.
    pub fn run_egui_frame(
        &mut self,
        sample_rate: f32,
        peak_levels_db: (f32, f32),
    ) -> egui::FullOutput {
        let eq = self.shared.read_eq_parameters();
        let auto_gain_comp_db = if eq.global.auto_gain {
            equaform_engine::calculate_auto_gain_for_params(&eq, sample_rate)
        } else {
            0.0
        };
        let snapshot = eq_to_param_snapshot(&eq, auto_gain_comp_db);

        let events = std::mem::take(&mut self.pending_events);
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(EDITOR_WIDTH as f32, EDITOR_HEIGHT as f32),
            )),
            time: Some(self.start_time.elapsed().as_secs_f64()),
            modifiers: self.modifiers,
            events,
            ..Default::default()
        };

        let mut out_batch = GuiCommandBatch::new();
        let mut out_actions: Vec<WorkspaceAction> = Vec::new();
        let env = HostEnvironment::Plugin {
            host_description: self.host_description.to_string(),
            sample_rate,
            peak_levels_db,
        };

        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::central_panel(&ctx.style())
                        .fill(egui::Color32::from_rgb(18, 20, 24))
                        .inner_margin(8.0),
                )
                .show(ctx, |ui| {
                    let ws_ctx = WorkspaceUiContext {
                        env: &env,
                        band_params: &eq.bands,
                        sample_rate,
                        snapshot: &snapshot,
                        analysis: &self.analysis,
                        preset_manager: &self.preset_manager,
                        current_preset: &self.current_preset,
                        status_message: self.status_message.as_deref(),
                    };
                    let (b, a) = render_workspace(ui, &mut self.ui_state, &ws_ctx);
                    out_batch = b;
                    out_actions = a;
                });
        });

        let pointer_down = self.egui_ctx.input(|i| i.pointer.any_down());
        let mut dirty_host = false;
        for cmd in out_batch.commands() {
            if self
                .shared
                .set_param_from_ui(cmd.param_id.0, cmd.value as f64, pointer_down)
            {
                dirty_host = true;
            }
        }
        if !pointer_down && self.shared.end_active_gestures() {
            dirty_host = true;
        }

        if self.apply_workspace_actions(out_actions) {
            dirty_host = true;
        }

        if dirty_host {
            self.request_host_flush();
        }
        if self.shared.is_latency_changed_pending() {
            self.request_host_callback();
        }

        full_output
    }

    /// Processes workspace actions emitted by the UI. Returns `true` if parameters changed
    /// and a CLAP host flush should be requested.
    pub fn apply_workspace_actions(&mut self, actions: Vec<WorkspaceAction>) -> bool {
        let mut dirty_host = false;
        for action in actions {
            match action {
                WorkspaceAction::ApplyPreset(name) => {
                    if self.apply_preset(&name) {
                        dirty_host = true;
                    }
                }
                WorkspaceAction::SavePreset(name) => {
                    self.save_preset(&name);
                }
                WorkspaceAction::DeletePreset(name) => {
                    self.delete_preset(&name);
                }
                WorkspaceAction::SetLanguage(lang) => {
                    lang.save();
                }
                _ => {}
            }
        }
        dirty_host
    }

    pub fn apply_preset(&mut self, name: &str) -> bool {
        let Some(named) = self.preset_manager.get(name) else {
            return false;
        };
        let mut eq = self.shared.read_eq_parameters();
        for (id, val) in named.preset.parameter_values.iter() {
            let _ = apply_param_to_eq(&mut eq, id, val as f64);
        }
        self.shared.write_eq_parameters_from_ui(&eq);
        self.current_preset = name.to_string();
        self.status_message = Some(format!("Preset '{name}' applied"));
        true
    }

    pub fn save_preset(&mut self, name: &str) {
        let eq = self.shared.read_eq_parameters();
        let mut state = ProjectState::new();
        for slot in 0..PARAM_COUNT_USIZE {
            let id = slot_to_param_id(slot);
            if let Some(val) = get_param_from_eq(&eq, id) {
                state.set(id, val as f32);
            }
        }
        match self.preset_manager.create_from_state(name, &state) {
            Ok(()) => {
                if let Err(err) = persist_preset_to_disk(&mut self.preset_manager, name) {
                    self.status_message = Some(format!("Save error: {err}"));
                } else {
                    self.current_preset = name.to_string();
                    self.status_message = Some(format!("Preset '{name}' saved"));
                }
            }
            Err(err) => {
                self.status_message = Some(format!("Error: {err}"));
            }
        }
    }

    pub fn delete_preset(&mut self, name: &str) {
        self.preset_manager.remove(name);
        remove_preset_from_disk(name);
        if self.current_preset == name {
            self.current_preset.clear();
        }
        self.status_message = Some(format!("Preset '{name}' deleted"));
    }

    pub fn request_host_flush(&self) {
        if let Some(host) = self.host {
            if !host.is_null() {
                // SAFETY: `host` is a valid CLAP host pointer for the plugin's lifetime.
                unsafe {
                    if let Some(get_ext) = (*host).get_extension {
                        let ptr =
                            get_ext(host, CLAP_EXT_PARAMS.as_ptr()) as *const clap_host_params;
                        if !ptr.is_null() {
                            if let Some(req) = (*ptr).request_flush {
                                req(host);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Requests a main-thread callback from the CLAP host (thread-safe).
    pub fn request_host_callback(&self) {
        if let Some(host) = self.host {
            if !host.is_null() {
                // SAFETY: `host` is a valid CLAP host pointer for the plugin's lifetime,
                // and `clap_host::request_callback` is thread-safe per the CLAP specification.
                unsafe {
                    if let Some(req) = (*host).request_callback {
                        req(host);
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub fn current_preset(&self) -> &str {
        &self.current_preset
    }

    #[cfg(test)]
    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    #[cfg(test)]
    pub fn analysis(&self) -> &AnalysisSnapshot {
        &self.analysis
    }

    #[cfg(test)]
    pub fn preset_manager(&self) -> &PresetManager {
        &self.preset_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params_map::default_eq_parameters;
    use equaform_common::{FilterType, Slope};

    #[test]
    fn test_eq_to_param_snapshot_maps_global_and_all_bands() {
        let mut eq = default_eq_parameters();
        eq.global.bypass = true;
        eq.global.output_gain_db = -3.5;
        eq.global.auto_gain = true;
        eq.global.processing_mode = equaform_common::ProcessingMode::LinearPhaseHigh;
        eq.bands[2].enabled = true;
        eq.bands[2].filter_type = FilterType::HighShelf;
        eq.bands[2].frequency_hz = 4200.0;
        eq.bands[2].gain_db = 5.5;
        eq.bands[2].q = 1.4;
        eq.bands[2].slope = Slope::Db24;

        let snap = eq_to_param_snapshot(&eq, -1.25);

        assert_eq!(snap.get(ParamId(global::BYPASS)), Some(1.0));
        assert_eq!(snap.get(ParamId(global::OUTPUT_GAIN_DB)), Some(-3.5));
        assert_eq!(snap.get(ParamId(global::AUTO_GAIN)), Some(1.0));
        assert_eq!(snap.get(ParamId(global::PROCESSING_MODE)), Some(2.0));
        assert_eq!(
            snap.get(ParamId(global::AUTO_GAIN_COMPENSATION_DB)),
            Some(-1.25)
        );

        let base = 100 + 2 * 100;
        assert_eq!(snap.get(ParamId(base + band::ENABLED)), Some(1.0));
        assert_eq!(
            snap.get(ParamId(base + band::FILTER_TYPE)),
            Some(FilterType::HighShelf.to_raw() as f32)
        );
        assert_eq!(snap.get(ParamId(base + band::FREQUENCY_HZ)), Some(4200.0));
        assert_eq!(snap.get(ParamId(base + band::GAIN_DB)), Some(5.5));
        assert_eq!(snap.get(ParamId(base + band::Q)), Some(1.4));
        assert_eq!(
            snap.get(ParamId(base + band::SLOPE)),
            Some(Slope::Db24.to_raw() as f32)
        );
    }

    #[test]
    fn test_should_skip_frame_throttling_and_event_wakeup() {
        let shared = Arc::new(EquaformSharedInner::new());
        let mut state = SharedEditorState::new(shared, None, "Test Host");

        // Active window never skips
        for _ in 0..8 {
            assert!(!state.should_skip_frame(true));
        }

        // Inactive window runs at 15 FPS (1 out of every 4 frames rendered)
        state.frame_counter = 0;
        assert!(state.should_skip_frame(false)); // counter = 1 -> skip
        assert!(state.should_skip_frame(false)); // counter = 2 -> skip
        assert!(state.should_skip_frame(false)); // counter = 3 -> skip
        assert!(!state.should_skip_frame(false)); // counter = 4 -> render

        // Pending events force immediate wakeup even when inactive
        state
            .pending_events
            .push(egui::Event::PointerMoved(egui::pos2(10.0, 20.0)));
        assert!(!state.should_skip_frame(false));
        assert_eq!(state.frame_counter, 0);
    }

    #[test]
    fn test_apply_preset_updates_shared_params_and_status() {
        let shared = Arc::new(EquaformSharedInner::new());
        let mut state = SharedEditorState::new(Arc::clone(&shared), None, "Test Host");

        assert!(!state.apply_preset("NonExistentPreset_XYZ"));

        let first_preset_name = state
            .preset_manager()
            .iter()
            .next()
            .map(|p| p.name.clone())
            .expect("factory presets should be populated");

        let version_before = shared.version();
        assert!(state.apply_preset(&first_preset_name));
        assert_eq!(state.current_preset(), first_preset_name);
        assert!(
            state
                .status_message()
                .is_some_and(|m| m.contains(&first_preset_name))
        );
        assert!(shared.version() > version_before);
    }

    #[test]
    fn test_collect_audio_and_spectrum_handles_silence_and_zero_fps_resync() {
        let shared = Arc::new(EquaformSharedInner::new());
        shared.set_sample_rate(48000.0);
        shared.set_telemetry(-120.0, -120.0, 0.0);

        let mut state = SharedEditorState::new(Arc::clone(&shared), None, "Test Host");
        state.mark_zero_fps();

        let (sr, peaks) = state.collect_audio_and_spectrum();
        assert!((sr - 48000.0).abs() < 1e-3);
        assert_eq!(peaks, (-120.0, -120.0));
        assert!(!state.was_in_zero_fps);
        assert!(state.analysis().spectrum_data.is_empty());
    }

    #[test]
    fn test_run_egui_frame_executes_headless_without_panic() {
        let shared = Arc::new(EquaformSharedInner::new());
        shared.set_sample_rate(48000.0);
        let mut state = SharedEditorState::new(shared, None, "Test Host");

        let output = state.run_egui_frame(48000.0, (-18.0, -18.0));
        assert!(!output.shapes.is_empty());
        assert!(!state.has_pending_events());
    }
}
