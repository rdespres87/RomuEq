//! Native macOS Cocoa (`NSView`) + Metal (`wgpu` + `egui-wgpu`) embedded editor
//! for Equaform. Renders `equaform_gui::workspace_ui::render_workspace` at fixed
//! studio dimensions `1180x560` points inside the host DAW window.

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::ffi::c_void;
    use std::ptr::NonNull;
    use std::rc::{Rc, Weak};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
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
    use objc2::rc::{Retained, Weak as ObjCWeak};
    use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send};
    use objc2_app_kit::{
        NSEvent, NSEventModifierFlags, NSPasteboard, NSPasteboardTypeString, NSResponder,
        NSTrackingArea, NSTrackingAreaOptions, NSView, NSWindowOcclusionState,
    };
    use objc2_foundation::{
        MainThreadMarker, NSObject, NSPoint, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize,
        NSTimer,
    };
    use raw_window_handle::{
        AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
    };

    use crate::EquaformSharedInner;
    use crate::params_map::{
        PARAM_COUNT_USIZE, apply_param_to_eq, get_param_from_eq, slot_to_param_id,
    };

    pub const EDITOR_WIDTH: f64 = 1180.0;
    pub const EDITOR_HEIGHT: f64 = 485.0;
    const FFT_SIZE: usize = 1024;

    /// Minimal synchronous poll helper for Metal adapter/device futures (resolve immediately on macOS).
    fn block_on_sync<F: std::future::Future>(fut: F) -> F::Output {
        fn noop_clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        fn noop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);
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

    struct ViewIvars {
        runtime: Weak<RefCell<Option<EditorRuntime>>>,
    }

    define_class!(
        #[unsafe(super(NSView, NSResponder, NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "EquaformPluginCocoaView_v1"]
        #[ivars = ViewIvars]
        struct EquaformCocoaView;

        impl EquaformCocoaView {
            #[unsafe(method(isFlipped))]
            fn is_flipped(&self) -> bool {
                true
            }

            #[unsafe(method(acceptsFirstResponder))]
            fn accepts_first_responder(&self) -> bool {
                true
            }

            #[unsafe(method(updateTrackingAreas))]
            fn update_tracking_areas(&self) {
                let existing = self.trackingAreas();
                for area in existing {
                    self.removeTrackingArea(&area);
                }
                self.install_tracking_area();
            }

            #[unsafe(method(mouseDown:))]
            fn mouse_down(&self, event: &NSEvent) {
                if let Some(win) = self.window() {
                    win.makeFirstResponder(Some(self));
                }
                self.push_pointer_button(event, egui::PointerButton::Primary, true);
            }

            #[unsafe(method(mouseUp:))]
            fn mouse_up(&self, event: &NSEvent) {
                self.push_pointer_button(event, egui::PointerButton::Primary, false);
            }

            #[unsafe(method(rightMouseDown:))]
            fn right_mouse_down(&self, event: &NSEvent) {
                self.push_pointer_button(event, egui::PointerButton::Secondary, true);
            }

            #[unsafe(method(rightMouseUp:))]
            fn right_mouse_up(&self, event: &NSEvent) {
                self.push_pointer_button(event, egui::PointerButton::Secondary, false);
            }

            #[unsafe(method(otherMouseDown:))]
            fn other_mouse_down(&self, event: &NSEvent) {
                self.push_pointer_button(event, egui::PointerButton::Middle, true);
            }

            #[unsafe(method(otherMouseUp:))]
            fn other_mouse_up(&self, event: &NSEvent) {
                self.push_pointer_button(event, egui::PointerButton::Middle, false);
            }

            #[unsafe(method(mouseEntered:))]
            fn mouse_entered(&self, _event: &NSEvent) {
                self.with_runtime(|rt| {
                    rt.mouse_inside = true;
                });
            }

            #[unsafe(method(mouseMoved:))]
            fn mouse_moved(&self, event: &NSEvent) {
                let pos = self.event_pos(event);
                self.with_runtime(|rt| {
                    rt.mouse_inside = true;
                    rt.last_pos = pos;
                    rt.pending_events.push(egui::Event::PointerMoved(pos));
                });
            }

            #[unsafe(method(mouseDragged:))]
            fn mouse_dragged(&self, event: &NSEvent) {
                let pos = self.event_pos(event);
                self.with_runtime(|rt| {
                    rt.mouse_inside = true;
                    rt.last_pos = pos;
                    rt.pending_events.push(egui::Event::PointerMoved(pos));
                });
            }

            #[unsafe(method(rightMouseDragged:))]
            fn right_mouse_dragged(&self, event: &NSEvent) {
                let pos = self.event_pos(event);
                self.with_runtime(|rt| {
                    rt.mouse_inside = true;
                    rt.last_pos = pos;
                    rt.pending_events.push(egui::Event::PointerMoved(pos));
                });
            }

            #[unsafe(method(mouseExited:))]
            fn mouse_exited(&self, _event: &NSEvent) {
                self.with_runtime(|rt| {
                    rt.mouse_inside = false;
                    if rt.pointer_down {
                        rt.pointer_down = false;
                        rt.ui_state.dragged_band = None;
                        let pos = rt.last_pos;
                        let modifiers = rt.modifiers;
                        rt.pending_events.push(egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers,
                        });
                    }
                    rt.pending_events.push(egui::Event::PointerGone);
                });
            }

            #[unsafe(method(scrollWheel:))]
            fn scroll_wheel(&self, event: &NSEvent) {
                let precise = event.hasPreciseScrollingDeltas();
                let dx = event.scrollingDeltaX() as f32;
                let dy = event.scrollingDeltaY() as f32;
                let unit = if precise {
                    egui::MouseWheelUnit::Point
                } else {
                    egui::MouseWheelUnit::Line
                };
                self.with_runtime(|rt| {
                    let modifiers = rt.modifiers;
                    rt.pending_events.push(egui::Event::MouseWheel {
                        unit,
                        delta: egui::vec2(dx, dy),
                        modifiers,
                    });
                });
            }

            #[unsafe(method(flagsChanged:))]
            fn flags_changed(&self, event: &NSEvent) {
                let flags = event.modifierFlags();
                let mods = translate_modifiers(flags);
                self.with_runtime(|rt| rt.modifiers = mods);
            }

            #[unsafe(method(keyDown:))]
            fn key_down(&self, event: &NSEvent) {
                let mods = translate_modifiers(event.modifierFlags());
                let key_code = event.keyCode();
                let chars = event.characters().map(|s| s.to_string()).unwrap_or_default();

                self.with_runtime(|rt| {
                    rt.modifiers = mods;

                    if mods.command || mods.mac_cmd {
                        match chars.to_ascii_lowercase().as_str() {
                            "c" => {
                                rt.pending_events.push(egui::Event::Copy);
                                return;
                            }
                            "x" => {
                                rt.pending_events.push(egui::Event::Cut);
                                return;
                            }
                            "v" => {
                                let pb = NSPasteboard::generalPasteboard();
                                let pasted = unsafe {
                                    pb.stringForType(NSPasteboardTypeString)
                                        .map(|s| s.to_string())
                                };
                                if let Some(text) = pasted {
                                    rt.pending_events.push(egui::Event::Paste(text));
                                }
                                return;
                            }
                            "a" => {
                                rt.pending_events.push(egui::Event::Key {
                                    key: egui::Key::A,
                                    physical_key: None,
                                    pressed: true,
                                    repeat: false,
                                    modifiers: mods,
                                });
                                return;
                            }
                            _ => {}
                        }
                    }

                    if let Some(key) = map_macos_keycode(key_code) {
                        rt.pending_events.push(egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: event.isARepeat(),
                            modifiers: mods,
                        });
                    } else if !chars.is_empty() && !mods.ctrl && !mods.command && !mods.mac_cmd {
                        let filtered: String =
                            chars.chars().filter(|c| !c.is_control()).collect();
                        if !filtered.is_empty() {
                            rt.pending_events.push(egui::Event::Text(filtered));
                        }
                    }
                });
            }

            #[unsafe(method(keyUp:))]
            fn key_up(&self, event: &NSEvent) {
                let mods = translate_modifiers(event.modifierFlags());
                let key_code = event.keyCode();
                self.with_runtime(|rt| {
                    rt.modifiers = mods;
                    if let Some(key) = map_macos_keycode(key_code) {
                        rt.pending_events.push(egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: false,
                            repeat: false,
                            modifiers: mods,
                        });
                    }
                });
            }
        }
    );

    impl EquaformCocoaView {
        fn new_attached(
            mtm: MainThreadMarker,
            parent: &NSView,
            runtime: Weak<RefCell<Option<EditorRuntime>>>,
        ) -> Retained<Self> {
            let rect = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(EDITOR_WIDTH, EDITOR_HEIGHT),
            );
            let this = mtm.alloc::<Self>();
            let this = this.set_ivars(ViewIvars { runtime });
            let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: rect] };
            view.setWantsLayer(true);
            parent.addSubview(&view);
            view
        }

        fn install_tracking_area(&self) {
            let options = NSTrackingAreaOptions::MouseEnteredAndExited
                | NSTrackingAreaOptions::MouseMoved
                | NSTrackingAreaOptions::ActiveAlways
                | NSTrackingAreaOptions::InVisibleRect;
            let rect = self.bounds();
            let area = unsafe {
                NSTrackingArea::initWithRect_options_owner_userInfo(
                    NSTrackingArea::alloc(),
                    rect,
                    options,
                    Some(self),
                    None,
                )
            };
            self.addTrackingArea(&area);
        }

        fn event_pos(&self, event: &NSEvent) -> egui::Pos2 {
            let loc = event.locationInWindow();
            let pt = self.convertPoint_fromView(loc, None);
            egui::pos2(pt.x as f32, pt.y as f32)
        }

        fn with_runtime(&self, f: impl FnOnce(&mut EditorRuntime)) {
            if let Some(rc) = self.ivars().runtime.upgrade() {
                if let Ok(mut guard) = rc.try_borrow_mut() {
                    if let Some(rt) = guard.as_mut() {
                        if rt.alive.load(Ordering::Acquire) {
                            f(rt);
                        }
                    }
                }
            }
        }

        fn push_pointer_button(&self, event: &NSEvent, button: egui::PointerButton, pressed: bool) {
            let pos = self.event_pos(event);
            let mods = translate_modifiers(event.modifierFlags());
            self.with_runtime(|rt| {
                rt.modifiers = mods;
                rt.last_pos = pos;
                if button == egui::PointerButton::Primary {
                    rt.pointer_down = pressed;
                }
                rt.pending_events.push(egui::Event::PointerMoved(pos));
                rt.pending_events.push(egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: mods,
                });
            });
        }
    }

    fn translate_modifiers(flags: NSEventModifierFlags) -> egui::Modifiers {
        let shift = flags.contains(NSEventModifierFlags::Shift);
        let ctrl = flags.contains(NSEventModifierFlags::Control);
        let alt = flags.contains(NSEventModifierFlags::Option);
        let mac_cmd = flags.contains(NSEventModifierFlags::Command);
        egui::Modifiers {
            alt,
            ctrl,
            shift,
            mac_cmd,
            command: mac_cmd,
        }
    }

    fn map_macos_keycode(key_code: u16) -> Option<egui::Key> {
        match key_code {
            36 | 76 => Some(egui::Key::Enter),
            48 => Some(egui::Key::Tab),
            51 => Some(egui::Key::Backspace),
            53 => Some(egui::Key::Escape),
            117 => Some(egui::Key::Delete),
            123 => Some(egui::Key::ArrowLeft),
            124 => Some(egui::Key::ArrowRight),
            125 => Some(egui::Key::ArrowDown),
            126 => Some(egui::Key::ArrowUp),
            115 => Some(egui::Key::Home),
            119 => Some(egui::Key::End),
            _ => None,
        }
    }

    struct EditorRuntime {
        shared: Arc<EquaformSharedInner>,
        host: Option<*const clap_host>,
        alive: Arc<AtomicBool>,
        visible: Rc<Cell<bool>>,
        egui_ctx: egui::Context,
        start_time: Instant,
        pending_events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
        last_pos: egui::Pos2,
        pointer_down: bool,

        // wgpu Metal surface + egui renderer
        _instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface_config: wgpu::SurfaceConfiguration,
        renderer: egui_wgpu::Renderer,
        pixels_per_point: f32,

        // Equaform UI & analysis state
        ui_state: WorkspaceUiState,
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
        mouse_inside: bool,
        was_in_zero_fps: bool,
        silence_frames: u32,
        idle_transport_frames: u32,
    }

    impl EditorRuntime {
        fn new(
            shared: Arc<EquaformSharedInner>,
            host: Option<*const clap_host>,
            view: &NSView,
            visible: Rc<Cell<bool>>,
            alive: Arc<AtomicBool>,
        ) -> Result<Self, String> {
            let pixels_per_point = view
                .window()
                .map(|w| w.backingScaleFactor() as f32)
                .unwrap_or(2.0)
                .max(1.0);

            let width_px = ((EDITOR_WIDTH as f32) * pixels_per_point).round().max(1.0) as u32;
            let height_px = ((EDITOR_HEIGHT as f32) * pixels_per_point).round().max(1.0) as u32;

            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::METAL,
                ..Default::default()
            });

            let ns_view_ptr = NonNull::new(view as *const NSView as *mut c_void)
                .ok_or_else(|| "Null NSView pointer".to_string())?;
            let raw_window = RawWindowHandle::AppKit(AppKitWindowHandle::new(ns_view_ptr));
            let raw_display = RawDisplayHandle::AppKit(AppKitDisplayHandle::new());

            let surface = unsafe {
                instance
                    .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: raw_display,
                        raw_window_handle: raw_window,
                    })
                    .map_err(|e| format!("create_surface_unsafe: {e}"))?
            };

            let adapter = block_on_sync(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }))
            .ok_or_else(|| "No Metal adapter found".to_string())?;

            let (device, queue) = block_on_sync(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("EquaformPluginDevice"),
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
                width: width_px,
                height: height_px,
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
            let egui_ctx = egui::Context::default();

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
            let fft_sample_buffer = Vec::with_capacity(FFT_SIZE);

            Ok(Self {
                shared,
                host,
                alive,
                visible,
                egui_ctx,
                start_time: Instant::now(),
                pending_events: Vec::new(),
                modifiers: egui::Modifiers::default(),
                last_pos: egui::Pos2::ZERO,
                pointer_down: false,
                _instance: instance,
                surface,
                device,
                queue,
                surface_config,
                renderer,
                pixels_per_point,
                ui_state: WorkspaceUiState::default(),
                preset_manager,
                current_preset: String::new(),
                status_message: None,
                spectrum_processor,
                spectrum_frame_num: 0,
                last_spectrum_sr: sr,
                fft_ring: VecDeque::with_capacity(FFT_SIZE * 2),
                analysis,
                fft_sample_buffer,
                frame_counter: 0,
                mouse_inside: false,
                was_in_zero_fps: false,
                silence_frames: 0,
                idle_transport_frames: 0,
            })
        }

        fn update_backing_scale(&mut self, view: &NSView) {
            let scale = view
                .window()
                .map(|w| w.backingScaleFactor() as f32)
                .unwrap_or(self.pixels_per_point)
                .max(1.0);
            let width_px = ((EDITOR_WIDTH as f32) * scale).round().max(1.0) as u32;
            let height_px = ((EDITOR_HEIGHT as f32) * scale).round().max(1.0) as u32;
            if width_px != self.surface_config.width || height_px != self.surface_config.height {
                self.pixels_per_point = scale;
                self.surface_config.width = width_px;
                self.surface_config.height = height_px;
                self.surface.configure(&self.device, &self.surface_config);
            }
        }

        fn render_frame(&mut self, view: &NSView) {
            if !self.alive.load(Ordering::Acquire) || !self.visible.get() {
                self.was_in_zero_fps = true;
                return;
            }

            let window = view.window();
            let is_minimized = window.as_ref().is_some_and(|w| w.isMiniaturized());
            let is_occluded = window
                .as_ref()
                .is_some_and(|w| !w.occlusionState().contains(NSWindowOcclusionState::Visible));
            let is_hidden = view.isHiddenOrHasHiddenAncestor();

            if is_hidden || is_minimized || is_occluded {
                self.was_in_zero_fps = true;
                return; // 0 FPS : pas de FFT, pas de GPU draw
            }

            let is_key = window.as_ref().is_some_and(|w| w.isKeyWindow());
            let is_main = window.as_ref().is_some_and(|w| w.isMainWindow());
            let has_pending_events = !self.pending_events.is_empty();
            let is_active = is_key || is_main || self.mouse_inside || has_pending_events;

            self.frame_counter = self.frame_counter.wrapping_add(1);

            if has_pending_events {
                self.frame_counter = 0; // Force immediate frame
            } else if !is_active && (self.frame_counter % 4 != 0) {
                return; // 15 FPS: drop 3 out of 4 frames when idle in background
            }

            self.update_backing_scale(view);

            // Ring buffer resynchronization upon waking up from 0 FPS or if buffer is >= 50% full
            let consumer = self.shared.ring_consumer();
            if self.was_in_zero_fps || consumer.available_read() >= (consumer.capacity() / 2) {
                consumer.clear();
                self.fft_ring.clear();
                self.was_in_zero_fps = false;
            }

            // 1. Collect audio & telemetry from lock-free shared state
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

            let eq = self.shared.read_eq_parameters();
            let auto_gain_comp_db = if eq.global.auto_gain {
                equaform_engine::calculate_auto_gain_compensation_db(&eq.bands, sample_rate)
            } else {
                0.0
            };
            let snapshot = eq_to_param_snapshot(&eq, auto_gain_comp_db);

            // 2. Run egui frame with shared Workspace UI
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
                host_description: "CLAP / VST3 / AU".to_string(),
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

            // 3. Apply parameter commands & workspace actions back to lock-free host state
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

            // 4. Render with wgpu Metal
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

            let paint_jobs = self
                .egui_ctx
                .tessellate(full_output.shapes, self.pixels_per_point);

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

        fn apply_preset(&mut self, name: &str) -> bool {
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

    /// Cocoa plugin editor handle owned by `EquaformMainThread`.
    pub struct CocoaEditor {
        shared: Arc<EquaformSharedInner>,
        host: Option<*const clap_host>,
        created: bool,
        visible: Rc<Cell<bool>>,
        alive: Arc<AtomicBool>,
        runtime: Option<Rc<RefCell<Option<EditorRuntime>>>>,
        view: Option<Retained<EquaformCocoaView>>,
        timer: Option<Retained<NSTimer>>,
    }

    impl CocoaEditor {
        pub fn new(shared: Arc<EquaformSharedInner>, host: Option<*const clap_host>) -> Self {
            Self {
                shared,
                host,
                created: false,
                visible: Rc::new(Cell::new(true)),
                alive: Arc::new(AtomicBool::new(false)),
                runtime: None,
                view: None,
                timer: None,
            }
        }

        pub fn create(&mut self) -> bool {
            self.created = true;
            true
        }

        pub fn set_parent(&mut self, parent_nsview: *mut c_void) -> bool {
            if parent_nsview.is_null() {
                return false;
            }
            let Some(mtm) = MainThreadMarker::new() else {
                return false;
            };

            // Tear down any previous view/timer if re-parented
            self.teardown_view();

            let parent_view: &NSView = unsafe { &*(parent_nsview as *const NSView) };

            let runtime_holder: Rc<RefCell<Option<EditorRuntime>>> = Rc::new(RefCell::new(None));
            let _ = runtime_holder;

            // Create runtime after NSView exists so wgpu attaches CAMetalLayer cleanly
            self.alive.store(true, Ordering::Release);

            // Allocate view first with empty weak ref? Instead use Rc<RefCell<EditorRuntime>> once created:
            // Actually `EditorRuntime::new` only borrows `view: &NSView`, so create view after? No,
            // `EquaformCocoaView::new_attached` needs Weak<RefCell<EditorRuntime>>. Solve cleanly with
            // an indirection or Rc::new_cyclic!
            let shared = Arc::clone(&self.shared);
            let host = self.host;
            let visible = Rc::clone(&self.visible);
            let alive = Arc::clone(&self.alive);

            let mut view_out: Option<Retained<EquaformCocoaView>> = None;
            let mut init_ok = false;

            let rc = Rc::new_cyclic(|weak| {
                let view = EquaformCocoaView::new_attached(mtm, parent_view, weak.clone());
                let res = EditorRuntime::new(shared, host, &view, visible, alive);
                view_out = Some(view);
                match res {
                    Ok(rt) => {
                        init_ok = true;
                        RefCell::new(Some(rt))
                    }
                    Err(_) => RefCell::new(None),
                }
            });

            if !init_ok {
                if let Some(view) = view_out {
                    view.removeFromSuperview();
                }
                self.alive.store(false, Ordering::Release);
                return false;
            }

            let Some(view) = view_out else {
                return false;
            };

            let weak_rt = Rc::downgrade(&rc);
            let weak_view = ObjCWeak::from_retained(&view);
            let alive_timer = Arc::clone(&self.alive);

            let block = block2::RcBlock::new(move |_timer: NonNull<NSTimer>| {
                if !alive_timer.load(Ordering::Acquire) {
                    return;
                }
                if let (Some(rc), Some(view_ref)) = (weak_rt.upgrade(), weak_view.load()) {
                    if let Ok(mut guard) = rc.try_borrow_mut() {
                        if let Some(rt) = guard.as_mut() {
                            rt.render_frame(&view_ref);
                        }
                    }
                }
            });

            let timer =
                unsafe { NSTimer::timerWithTimeInterval_repeats_block(1.0 / 60.0, true, &block) };
            unsafe {
                let run_loop = NSRunLoop::currentRunLoop();
                run_loop.addTimer_forMode(&timer, NSRunLoopCommonModes);
            }

            self.runtime = Some(rc);
            self.view = Some(view);
            self.timer = Some(timer);
            true
        }

        pub fn show(&mut self) -> bool {
            self.visible.set(true);
            if let Some(ref view) = self.view {
                view.setHidden(false);
            }
            true
        }

        pub fn hide(&mut self) -> bool {
            self.visible.set(false);
            if let Some(ref view) = self.view {
                view.setHidden(true);
            }
            true
        }

        fn teardown_view(&mut self) {
            self.alive.store(false, Ordering::Release);
            if let Some(timer) = self.timer.take() {
                timer.invalidate();
            }
            if let Some(view) = self.view.take() {
                view.removeFromSuperview();
            }
            self.runtime = None;
        }

        pub fn destroy(&mut self) {
            self.teardown_view();
            self.created = false;
        }
    }

    impl Drop for CocoaEditor {
        fn drop(&mut self) {
            self.destroy();
        }
    }

    impl crate::editor::PluginEditor for CocoaEditor {
        fn create(&mut self, is_floating: bool) -> bool {
            if is_floating {
                return false;
            }
            self.create()
        }

        unsafe fn set_parent(&mut self, window: *const clap_sys::ext::gui::clap_window) -> bool {
            if window.is_null() {
                return false;
            }
            unsafe {
                let api = (*window).api;
                if api.is_null() {
                    return false;
                }
                if std::ffi::CStr::from_ptr(api) != clap_sys::ext::gui::CLAP_WINDOW_API_COCOA {
                    return false;
                }
                let parent_ptr = (*window).specific.cocoa;
                self.set_parent(parent_ptr)
            }
        }

        fn show(&mut self) -> bool {
            self.show()
        }

        fn hide(&mut self) -> bool {
            self.hide()
        }

        fn destroy(&mut self) {
            self.destroy();
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::{CocoaEditor, EDITOR_HEIGHT, EDITOR_WIDTH};

#[cfg(not(target_os = "macos"))]
mod fallback {
    use crate::EquaformSharedInner;
    use clap_sys::host::clap_host;
    use std::ffi::c_void;
    use std::sync::Arc;

    pub struct CocoaEditor;
    impl CocoaEditor {
        pub fn new(_shared: Arc<EquaformSharedInner>, _host: Option<*const clap_host>) -> Self {
            Self
        }
        pub fn create(&mut self) -> bool {
            false
        }
        pub fn set_parent(&mut self, _parent_nsview: *mut c_void) -> bool {
            false
        }
        pub fn show(&mut self) -> bool {
            false
        }
        pub fn hide(&mut self) -> bool {
            false
        }
        pub fn destroy(&mut self) {}
    }

    impl crate::editor::PluginEditor for CocoaEditor {
        fn create(&mut self, _is_floating: bool) -> bool {
            false
        }
        unsafe fn set_parent(&mut self, _window: *const clap_sys::ext::gui::clap_window) -> bool {
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

#[cfg(not(target_os = "macos"))]
pub use fallback::CocoaEditor;
