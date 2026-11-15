//! Native Windows Win32 (`HWND`) + DirectX 12/Vulkan (`wgpu` + `egui-wgpu`) embedded editor
//! for Equaform. Renders `equaform_gui::workspace_ui::render_workspace` at fixed
//! studio dimensions `1180x485` points inside the host DAW window.

#[cfg(target_os = "windows")]
mod windows {
    use std::collections::VecDeque;
    use std::num::NonZeroIsize;
    use std::ptr;
    use std::sync::Arc;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    use std::time::Instant;

    use clap_sys::ext::gui::clap_window;
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
    use raw_window_handle::{
        RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
    };

    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    use crate::EquaformSharedInner;
    use crate::editor::{EDITOR_HEIGHT, EDITOR_WIDTH, PluginEditor};
    use crate::params_map::{
        PARAM_COUNT_USIZE, apply_param_to_eq, get_param_from_eq, slot_to_param_id,
    };

    const FFT_SIZE: usize = 1024;
    const TIMER_ID: usize = 1001;
    const TIMER_INTERVAL_MS: u32 = 16; // ~60 FPS

    const CLASS_NAME: &[u16] = &[
        'E' as u16, 'q' as u16, 'u' as u16, 'a' as u16, 'f' as u16, 'o' as u16, 'r' as u16,
        'm' as u16, 'P' as u16, 'l' as u16, 'u' as u16, 'g' as u16, 'i' as u16, 'n' as u16,
        'W' as u16, 'i' as u16, 'n' as u16, '3' as u16, '2' as u16, 'V' as u16, 'i' as u16,
        'e' as u16, 'w' as u16, '_' as u16, 'v' as u16, '1' as u16, 0,
    ];

    fn block_on_sync<F: std::future::Future>(fut: F) -> F::Output {
        fn noop_clone(_: *const ()) -> RawWaker {
            RawWaker::new(ptr::null(), &VTABLE)
        }
        fn noop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut pinned = std::pin::pin!(fut);
        loop {
            match pinned.as_mut().poll(&mut cx) {
                Poll::Ready(val) => return val,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    fn ensure_wndclass_registered() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| unsafe {
            let h_instance = GetModuleHandleW(ptr::null());
            let wndclass = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(win32_wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: h_instance,
                hIcon: ptr::null_mut(),
                hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
                hbrBackground: ptr::null_mut(),
                lpszMenuName: ptr::null(),
                lpszClassName: CLASS_NAME.as_ptr(),
                hIconSm: ptr::null_mut(),
            };
            RegisterClassExW(&wndclass);
        });
    }

    struct Win32EditorRuntime {
        hwnd: HWND,
        shared: Arc<EquaformSharedInner>,
        host: Option<*const clap_host>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
        egui_ctx: egui::Context,
        egui_renderer: egui_wgpu::Renderer,
        start_time: Instant,
        spectrum_proc: SpectrumProcessor,
        fft_ring: VecDeque<f32>,
        spectrum_frame_num: u64,
        ui_state: WorkspaceUiState,
        preset_manager: PresetManager,
        current_preset: String,
        status_message: Option<String>,
        scale_factor: f32,
        physical_width: u32,
        physical_height: u32,
        pending_events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
        mouse_in_window: bool,
        analysis: AnalysisSnapshot,
        fft_sample_buffer: Vec<f32>,
        frame_counter: u32,
        was_in_zero_fps: bool,
        silence_frames: u32,
        idle_transport_frames: u32,
    }

    impl Win32EditorRuntime {
        fn new(
            hwnd: HWND,
            shared: Arc<EquaformSharedInner>,
            host: Option<*const clap_host>,
            scale_factor: f32,
            physical_width: u32,
            physical_height: u32,
        ) -> Option<Self> {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::PRIMARY,
                ..Default::default()
            });

            let non_zero_hwnd = NonZeroIsize::new(hwnd as isize)?;
            let raw_window = RawWindowHandle::Win32(Win32WindowHandle::new(non_zero_hwnd));
            let raw_display = RawDisplayHandle::Windows(WindowsDisplayHandle::new());

            let surface = unsafe {
                instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: raw_display,
                    raw_window_handle: raw_window,
                })
            }
            .ok()?;

            let adapter = block_on_sync(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }))
            .or_else(|| {
                block_on_sync(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::None,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: true,
                }))
            })?;

            let (device, queue) = block_on_sync(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Equaform Win32 Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            ))
            .ok()?;

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
                width: physical_width.max(1),
                height: physical_height.max(1),
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

            let egui_ctx = egui::Context::default();
            egui_ctx.set_pixels_per_point(scale_factor);

            let egui_renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);

            let mut preset_manager = PresetManager::new();
            populate_factory_presets(&mut preset_manager);

            let spectrum_proc = SpectrumProcessor::new(SpectrumConfig {
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
            let fft_sample_buffer = Vec::with_capacity(FFT_SIZE);

            Some(Self {
                hwnd,
                shared,
                host,
                device,
                queue,
                surface,
                surface_config,
                egui_ctx,
                egui_renderer,
                start_time: Instant::now(),
                spectrum_proc,
                fft_ring: VecDeque::with_capacity(FFT_SIZE * 4),
                spectrum_frame_num: 0,
                ui_state: WorkspaceUiState::default(),
                preset_manager,
                current_preset: String::new(),
                status_message: None,
                scale_factor,
                physical_width,
                physical_height,
                pending_events: Vec::new(),
                modifiers: egui::Modifiers::default(),
                mouse_in_window: false,
                analysis,
                fft_sample_buffer,
                frame_counter: 0,
                was_in_zero_fps: false,
                silence_frames: 0,
                idle_transport_frames: 0,
            })
        }

        fn resize(&mut self, width: u32, height: u32) {
            if width == 0 || height == 0 {
                return;
            }
            self.physical_width = width;
            self.physical_height = height;
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
        }

        fn update_dpi(&mut self) {
            let dpi = unsafe { GetDpiForWindow(self.hwnd) };
            let new_scale = if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 };
            self.scale_factor = new_scale;
            self.egui_ctx.set_pixels_per_point(new_scale);
            let new_width = (EDITOR_WIDTH as f32 * new_scale).round() as u32;
            let new_height = (EDITOR_HEIGHT as f32 * new_scale).round() as u32;
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    ptr::null_mut(),
                    0,
                    0,
                    new_width as i32,
                    new_height as i32,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            self.resize(new_width, new_height);
        }

        fn update_modifiers(&mut self) {
            unsafe {
                let ctrl = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
                let shift = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
                let alt = (GetKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
                self.modifiers = egui::Modifiers {
                    alt,
                    ctrl,
                    shift,
                    mac_cmd: false,
                    command: ctrl,
                };
            }
        }

        fn request_host_flush(&self) {
            if let Some(host) = self.host {
                if !host.is_null() {
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

        fn apply_preset(&mut self, name: &str) -> bool {
            let p = match self.preset_manager.get(name) {
                Some(p) => p.preset.clone(),
                None => return false,
            };
            let mut eq = self.shared.read_eq_parameters();
            for slot in 0..PARAM_COUNT_USIZE {
                let id = slot_to_param_id(slot);
                if let Some(v) = p.parameter_values.get(id) {
                    apply_param_to_eq(&mut eq, id, v as f64);
                    self.shared.set_param_from_ui(id, v as f64, false);
                }
            }
            self.shared.write_eq_parameters_from_ui(&eq);
            self.current_preset = name.to_string();
            self.status_message = Some(format!("Preset '{name}' applied"));
            true
        }

        fn save_preset(&mut self, name: &str) {
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

        fn render_frame(&mut self) {
            let sample_rate = self.shared.sample_rate().max(8000.0);
            let consumer = self.shared.ring_consumer();

            // Ring buffer resynchronization upon waking up from 0 FPS or if buffer is >= 50% full
            if self.was_in_zero_fps || consumer.available_read() >= (consumer.capacity() / 2) {
                consumer.clear();
                self.fft_ring.clear();
                self.was_in_zero_fps = false;
            }

            let mut tmp_l = [0.0f32; 512];
            let mut tmp_r = [0.0f32; 512];
            let mut popped_any = false;

            while self.fft_ring.len() < FFT_SIZE * 2 {
                let n = consumer.pop_slice(&mut tmp_l, &mut tmp_r);
                if n == 0 {
                    break;
                }
                popped_any = true;
                for i in 0..n {
                    self.fft_ring.push_back(0.5 * (tmp_l[i] + tmp_r[i]));
                    if self.fft_ring.len() > FFT_SIZE {
                        self.fft_ring.pop_front();
                    }
                }
            }

            if !popped_any {
                self.idle_transport_frames = self.idle_transport_frames.saturating_add(1);
                if self.idle_transport_frames > 4 {
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
                let frame = self.spectrum_proc.process_ref(
                    &self.fft_sample_buffer,
                    sample_rate as f64,
                    self.spectrum_frame_num,
                );
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

            let eq = self.shared.read_eq_parameters();
            let auto_gain_comp_db = if eq.global.auto_gain {
                equaform_engine::calculate_auto_gain_compensation_db(&eq.bands, sample_rate)
            } else {
                0.0
            };
            let snapshot = eq_to_param_snapshot(&eq, auto_gain_comp_db);

            self.update_modifiers();
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
                host_description: "CLAP / VST3".to_string(),
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

            for action in out_actions {
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
                        self.preset_manager.remove(&name);
                        remove_preset_from_disk(&name);
                        if self.current_preset == name {
                            self.current_preset.clear();
                        }
                        self.status_message = Some(format!("Preset '{name}' deleted"));
                    }
                    _ => {}
                }
            }

            if dirty_host {
                self.request_host_flush();
            }

            let output_frame = match self.surface.get_current_texture() {
                Ok(frame) => frame,
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    self.surface.configure(&self.device, &self.surface_config);
                    return;
                }
                Err(_) => return,
            };

            let output_view = output_frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [self.surface_config.width, self.surface_config.height],
                pixels_per_point: self.scale_factor,
            };

            let paint_jobs = self
                .egui_ctx
                .tessellate(full_output.shapes, self.scale_factor);

            for (id, delta) in &full_output.textures_delta.set {
                self.egui_renderer
                    .update_texture(&self.device, &self.queue, *id, delta);
            }

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Equaform Win32 Render Encoder"),
                });

            let extra_cmds = self.egui_renderer.update_buffers(
                &self.device,
                &self.queue,
                &mut encoder,
                &paint_jobs,
                &screen_descriptor,
            );

            {
                let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Equaform Win32 Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &output_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.070,
                                g: 0.078,
                                b: 0.094,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                let mut render_pass = render_pass.forget_lifetime();
                self.egui_renderer
                    .render(&mut render_pass, &paint_jobs, &screen_descriptor);
            }

            for id in &full_output.textures_delta.free {
                self.egui_renderer.free_texture(id);
            }

            let mut cmd_bufs = extra_cmds;
            cmd_bufs.push(encoder.finish());
            self.queue.submit(cmd_bufs);
            output_frame.present();
        }

        fn on_mouse_move(&mut self, x: f32, y: f32) {
            self.update_modifiers();
            if !self.mouse_in_window {
                self.mouse_in_window = true;
                unsafe {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: self.hwnd,
                        dwHoverTime: 0,
                    };
                    TrackMouseEvent(&mut tme);
                }
            }
            let logical_pos = egui::pos2(x / self.scale_factor, y / self.scale_factor);
            self.pending_events
                .push(egui::Event::PointerMoved(logical_pos));
        }

        fn on_mouse_leave(&mut self) {
            self.mouse_in_window = false;
            self.pending_events.push(egui::Event::PointerGone);
        }

        fn on_mouse_button(&mut self, button: egui::PointerButton, pressed: bool) {
            self.update_modifiers();
            self.pending_events.push(egui::Event::PointerButton {
                pos: self
                    .pending_events
                    .iter()
                    .rev()
                    .find_map(|e| match e {
                        egui::Event::PointerMoved(p) => Some(*p),
                        _ => None,
                    })
                    .unwrap_or(egui::Pos2::ZERO),
                button,
                pressed,
                modifiers: self.modifiers,
            });
        }

        fn on_mouse_wheel(&mut self, delta: f32) {
            self.update_modifiers();
            let dy = delta / 120.0 * 20.0;
            self.pending_events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, dy),
                modifiers: self.modifiers,
            });
        }

        fn on_key(&mut self, vk: u32, pressed: bool) {
            self.update_modifiers();
            let key = match vk {
                0x25 => Some(egui::Key::ArrowLeft),
                0x26 => Some(egui::Key::ArrowUp),
                0x27 => Some(egui::Key::ArrowRight),
                0x28 => Some(egui::Key::ArrowDown),
                0x1B => Some(egui::Key::Escape),
                0x0D => Some(egui::Key::Enter),
                0x20 => Some(egui::Key::Space),
                0x08 => Some(egui::Key::Backspace),
                0x2E => Some(egui::Key::Delete),
                0x09 => Some(egui::Key::Tab),
                _ => None,
            };
            if let Some(k) = key {
                self.pending_events.push(egui::Event::Key {
                    key: k,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: self.modifiers,
                });
            }
        }

        fn on_char(&mut self, c: char) {
            self.update_modifiers();
            if !c.is_control() {
                self.pending_events.push(egui::Event::Text(c.to_string()));
            }
        }
    }

    fn eq_to_param_snapshot(eq: &EqParameters, auto_gain_comp_db: f32) -> ParamSnapshot {
        let mut vals = Vec::with_capacity(PARAM_COUNT_USIZE + 1);
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
            param_id: ParamId(global::AUTO_GAIN_COMPENSATION_DB),
            value: auto_gain_comp_db,
        });

        for i in 0..MAX_BANDS {
            let base = 100 + (i as u32) * 100;
            let b = &eq.bands[i];
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
        }

        ParamSnapshot::new(vals)
    }

    const WM_MOUSELEAVE_MSG: u32 = 0x02A3;

    unsafe extern "system" fn win32_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Win32EditorRuntime };
        if ptr.is_null() {
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        let runtime = unsafe { &mut *ptr };

        match msg {
            WM_PAINT => {
                unsafe {
                    let mut ps = std::mem::zeroed();
                    BeginPaint(hwnd, &mut ps);
                    runtime.render_frame();
                    EndPaint(hwnd, &ps);
                }
                0
            }
            WM_TIMER => {
                let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
                let is_visible = unsafe { IsWindowVisible(hwnd) != 0 };
                let is_iconic = if !root_hwnd.is_null() {
                    unsafe { IsIconic(root_hwnd) != 0 }
                } else {
                    false
                };

                if !is_visible || is_iconic {
                    runtime.was_in_zero_fps = true;
                    return 0; // 0 FPS
                }

                let fg_hwnd = unsafe { GetForegroundWindow() };
                let is_focused = fg_hwnd == root_hwnd || fg_hwnd == hwnd;
                let has_pending_events = !runtime.pending_events.is_empty();
                let is_active = is_focused || runtime.mouse_in_window || has_pending_events;

                runtime.frame_counter = runtime.frame_counter.wrapping_add(1);

                if has_pending_events {
                    runtime.frame_counter = 0; // Force immediate frame
                } else if !is_active && (runtime.frame_counter % 4 != 0) {
                    return 0; // 15 FPS: drop 3 out of 4 frames in background
                }

                runtime.render_frame();
                0
            }
            WM_ERASEBKGND => 1,
            WM_SIZE => {
                let width = (lparam & 0xFFFF) as u32;
                let height = ((lparam >> 16) & 0xFFFF) as u32;
                runtime.resize(width, height);
                0
            }
            WM_DPICHANGED => {
                runtime.update_dpi();
                0
            }
            WM_MOUSEMOVE => {
                let x = (lparam & 0xFFFF) as i16 as f32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as f32;
                runtime.on_mouse_move(x, y);
                0
            }
            WM_MOUSELEAVE_MSG => {
                runtime.on_mouse_leave();
                0
            }
            WM_LBUTTONDOWN => {
                unsafe {
                    SetFocus(hwnd);
                    SetCapture(hwnd);
                }
                runtime.on_mouse_button(egui::PointerButton::Primary, true);
                0
            }
            WM_LBUTTONUP => {
                unsafe {
                    ReleaseCapture();
                }
                runtime.on_mouse_button(egui::PointerButton::Primary, false);
                0
            }
            WM_RBUTTONDOWN => {
                unsafe {
                    SetFocus(hwnd);
                    SetCapture(hwnd);
                }
                runtime.on_mouse_button(egui::PointerButton::Secondary, true);
                0
            }
            WM_RBUTTONUP => {
                unsafe {
                    ReleaseCapture();
                }
                runtime.on_mouse_button(egui::PointerButton::Secondary, false);
                0
            }
            WM_MBUTTONDOWN => {
                runtime.on_mouse_button(egui::PointerButton::Middle, true);
                0
            }
            WM_MBUTTONUP => {
                runtime.on_mouse_button(egui::PointerButton::Middle, false);
                0
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam >> 16) & 0xFFFF) as i16 as f32;
                runtime.on_mouse_wheel(delta);
                0
            }
            WM_KEYDOWN => {
                runtime.on_key(wparam as u32, true);
                0
            }
            WM_KEYUP => {
                runtime.on_key(wparam as u32, false);
                0
            }
            WM_CHAR => {
                if let Some(c) = char::from_u32(wparam as u32) {
                    runtime.on_char(c);
                }
                0
            }
            WM_DESTROY => {
                unsafe {
                    KillTimer(hwnd, TIMER_ID);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    pub struct Win32Editor {
        shared: Arc<EquaformSharedInner>,
        host: Option<*const clap_host>,
        child_hwnd: HWND,
        runtime: Option<Box<Win32EditorRuntime>>,
        created: bool,
    }

    impl Win32Editor {
        pub fn new(shared: Arc<EquaformSharedInner>, host: Option<*const clap_host>) -> Self {
            Self {
                shared,
                host,
                child_hwnd: ptr::null_mut(),
                runtime: None,
                created: false,
            }
        }
    }

    impl PluginEditor for Win32Editor {
        fn create(&mut self, is_floating: bool) -> bool {
            if is_floating {
                return false;
            }
            ensure_wndclass_registered();
            self.created = true;
            true
        }

        unsafe fn set_parent(&mut self, window: *const clap_window) -> bool {
            if window.is_null() || !self.created {
                return false;
            }
            unsafe {
                let api = (*window).api;
                if api.is_null() {
                    return false;
                }
                if std::ffi::CStr::from_ptr(api) != clap_sys::ext::gui::CLAP_WINDOW_API_WIN32 {
                    return false;
                }

                let parent_hwnd = (*window).specific.win32 as HWND;
                if parent_hwnd.is_null() {
                    return false;
                }

                let dpi = GetDpiForWindow(parent_hwnd);
                let scale = if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 };
                let width = (EDITOR_WIDTH * scale as f64).round() as i32;
                let height = (EDITOR_HEIGHT * scale as f64).round() as i32;

                let h_instance = GetModuleHandleW(ptr::null());
                let child_hwnd = CreateWindowExW(
                    0,
                    CLASS_NAME.as_ptr(),
                    ptr::null(),
                    WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                    0,
                    0,
                    width,
                    height,
                    parent_hwnd,
                    ptr::null_mut(),
                    h_instance,
                    ptr::null_mut(),
                );

                if child_hwnd.is_null() {
                    return false;
                }

                let runtime = Win32EditorRuntime::new(
                    child_hwnd,
                    Arc::clone(&self.shared),
                    self.host,
                    scale,
                    width as u32,
                    height as u32,
                );

                if let Some(rt) = runtime {
                    let mut boxed = Box::new(rt);
                    let raw_ptr = &mut *boxed as *mut Win32EditorRuntime;
                    SetWindowLongPtrW(child_hwnd, GWLP_USERDATA, raw_ptr as isize);
                    SetTimer(child_hwnd, TIMER_ID, TIMER_INTERVAL_MS, None);
                    self.runtime = Some(boxed);
                    self.child_hwnd = child_hwnd;
                    true
                } else {
                    DestroyWindow(child_hwnd);
                    false
                }
            }
        }

        fn show(&mut self) -> bool {
            if !self.child_hwnd.is_null() {
                unsafe { ShowWindow(self.child_hwnd, SW_SHOW) };
                true
            } else {
                false
            }
        }

        fn hide(&mut self) -> bool {
            if !self.child_hwnd.is_null() {
                unsafe { ShowWindow(self.child_hwnd, SW_HIDE) };
                true
            } else {
                false
            }
        }

        fn destroy(&mut self) {
            if !self.child_hwnd.is_null() {
                unsafe {
                    KillTimer(self.child_hwnd, TIMER_ID);
                    DestroyWindow(self.child_hwnd);
                }
                self.child_hwnd = ptr::null_mut();
            }
            self.runtime = None;
            self.created = false;
        }
    }

    impl Drop for Win32Editor {
        fn drop(&mut self) {
            self.destroy();
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows::Win32Editor;

#[cfg(not(target_os = "windows"))]
mod fallback {
    use clap_sys::ext::gui::clap_window;
    use clap_sys::host::clap_host;
    use std::sync::Arc;

    use crate::EquaformSharedInner;
    use crate::editor::PluginEditor;

    pub struct Win32Editor;

    impl Win32Editor {
        pub fn new(_shared: Arc<EquaformSharedInner>, _host: Option<*const clap_host>) -> Self {
            Self
        }
    }

    impl PluginEditor for Win32Editor {
        fn create(&mut self, _is_floating: bool) -> bool {
            false
        }
        unsafe fn set_parent(&mut self, _window: *const clap_window) -> bool {
            false
        }
        fn show(&mut self) -> bool {
            false
        }
        fn hide(&mut self) -> bool {
            false
        }
        fn destroy(&mut self) {}
    }
}

#[cfg(not(target_os = "windows"))]
pub use fallback::Win32Editor;
