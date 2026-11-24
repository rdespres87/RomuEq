//! Native macOS Cocoa (`NSView`) + Metal (`wgpu` + `egui-wgpu`) embedded editor
//! for Equaform. Renders `equaform_gui::workspace_ui::render_workspace` at fixed
//! studio dimensions `1180x485` points inside the host DAW window.

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::{Cell, RefCell};
    use std::ffi::c_void;
    use std::ptr::NonNull;
    use std::rc::{Rc, Weak};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use clap_sys::host::clap_host;
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
    use crate::editor::shared_host::{SharedEditorState, WgpuSurfaceHost};
    pub use crate::editor::{EDITOR_HEIGHT, EDITOR_WIDTH};

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
                    rt.state.pending_events.push(egui::Event::PointerMoved(pos));
                });
            }

            #[unsafe(method(mouseDragged:))]
            fn mouse_dragged(&self, event: &NSEvent) {
                let pos = self.event_pos(event);
                self.with_runtime(|rt| {
                    rt.mouse_inside = true;
                    rt.last_pos = pos;
                    rt.state.pending_events.push(egui::Event::PointerMoved(pos));
                });
            }

            #[unsafe(method(rightMouseDragged:))]
            fn right_mouse_dragged(&self, event: &NSEvent) {
                let pos = self.event_pos(event);
                self.with_runtime(|rt| {
                    rt.mouse_inside = true;
                    rt.last_pos = pos;
                    rt.state.pending_events.push(egui::Event::PointerMoved(pos));
                });
            }

            #[unsafe(method(mouseExited:))]
            fn mouse_exited(&self, _event: &NSEvent) {
                self.with_runtime(|rt| {
                    rt.mouse_inside = false;
                    if rt.pointer_down {
                        rt.pointer_down = false;
                        rt.state.ui_state.dragged_band = None;
                        let pos = rt.last_pos;
                        let modifiers = rt.state.modifiers;
                        rt.state.pending_events.push(egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers,
                        });
                    }
                    rt.state.pending_events.push(egui::Event::PointerGone);
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
                    let modifiers = rt.state.modifiers;
                    rt.state.pending_events.push(egui::Event::MouseWheel {
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
                self.with_runtime(|rt| rt.state.modifiers = mods);
            }

            #[unsafe(method(keyDown:))]
            fn key_down(&self, event: &NSEvent) {
                let mods = translate_modifiers(event.modifierFlags());
                let key_code = event.keyCode();
                let chars = event.characters().map(|s| s.to_string()).unwrap_or_default();

                self.with_runtime(|rt| {
                    rt.state.modifiers = mods;

                    if mods.command || mods.mac_cmd {
                        match chars.to_ascii_lowercase().as_str() {
                            "c" => {
                                rt.state.pending_events.push(egui::Event::Copy);
                                return;
                            }
                            "x" => {
                                rt.state.pending_events.push(egui::Event::Cut);
                                return;
                            }
                            "v" => {
                                let pb = NSPasteboard::generalPasteboard();
                                let pasted = unsafe {
                                    pb.stringForType(NSPasteboardTypeString)
                                        .map(|s| s.to_string())
                                };
                                if let Some(text) = pasted {
                                    rt.state.pending_events.push(egui::Event::Paste(text));
                                }
                                return;
                            }
                            "a" => {
                                rt.state.pending_events.push(egui::Event::Key {
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
                        rt.state.pending_events.push(egui::Event::Key {
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
                            rt.state.pending_events.push(egui::Event::Text(filtered));
                        }
                    }
                });
            }

            #[unsafe(method(keyUp:))]
            fn key_up(&self, event: &NSEvent) {
                let mods = translate_modifiers(event.modifierFlags());
                let key_code = event.keyCode();
                self.with_runtime(|rt| {
                    rt.state.modifiers = mods;
                    if let Some(key) = map_macos_keycode(key_code) {
                        rt.state.pending_events.push(egui::Event::Key {
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
                rt.state.modifiers = mods;
                rt.last_pos = pos;
                if button == egui::PointerButton::Primary {
                    rt.pointer_down = pressed;
                }
                rt.state.pending_events.push(egui::Event::PointerMoved(pos));
                rt.state.pending_events.push(egui::Event::PointerButton {
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
        alive: Arc<AtomicBool>,
        visible: Rc<Cell<bool>>,
        state: SharedEditorState,
        wgpu: WgpuSurfaceHost,
        last_pos: egui::Pos2,
        pointer_down: bool,
        mouse_inside: bool,
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

            let ns_view_ptr = NonNull::new(view as *const NSView as *mut c_void)
                .ok_or_else(|| "Null NSView pointer".to_string())?;
            let raw_window = RawWindowHandle::AppKit(AppKitWindowHandle::new(ns_view_ptr));
            let raw_display = RawDisplayHandle::AppKit(AppKitDisplayHandle::new());

            let wgpu = unsafe {
                WgpuSurfaceHost::new(
                    wgpu::Backends::METAL,
                    raw_display,
                    raw_window,
                    width_px,
                    height_px,
                    pixels_per_point,
                    false,
                    "EquaformPluginDevice",
                )?
            };

            let state = SharedEditorState::new(shared, host, "CLAP / VST3 / AU");

            Ok(Self {
                alive,
                visible,
                state,
                wgpu,
                last_pos: egui::Pos2::ZERO,
                pointer_down: false,
                mouse_inside: false,
            })
        }

        fn update_backing_scale(&mut self, view: &NSView) {
            let scale = view
                .window()
                .map(|w| w.backingScaleFactor() as f32)
                .unwrap_or_else(|| self.wgpu.pixels_per_point())
                .max(1.0);
            let width_px = ((EDITOR_WIDTH as f32) * scale).round().max(1.0) as u32;
            let height_px = ((EDITOR_HEIGHT as f32) * scale).round().max(1.0) as u32;
            self.wgpu.reconfigure_size(width_px, height_px, scale);
        }

        fn render_frame(&mut self, view: &NSView) {
            if !self.alive.load(Ordering::Acquire) || !self.visible.get() {
                self.state.mark_zero_fps();
                return;
            }

            let window = view.window();
            let is_minimized = window.as_ref().is_some_and(|w| w.isMiniaturized());
            let is_occluded = window
                .as_ref()
                .is_some_and(|w| !w.occlusionState().contains(NSWindowOcclusionState::Visible));
            let is_hidden = view.isHiddenOrHasHiddenAncestor();

            if is_hidden || is_minimized || is_occluded {
                self.state.mark_zero_fps();
                return; // 0 FPS: skip FFT and GPU draw
            }

            let is_key = window.as_ref().is_some_and(|w| w.isKeyWindow());
            let is_main = window.as_ref().is_some_and(|w| w.isMainWindow());
            let is_active =
                is_key || is_main || self.mouse_inside || self.state.has_pending_events();

            if self.state.should_skip_frame(is_active) {
                return; // 15 FPS when idle in background
            }

            self.update_backing_scale(view);

            let (sample_rate, peak_levels_db) = self.state.collect_audio_and_spectrum();
            let full_output = self.state.run_egui_frame(sample_rate, peak_levels_db);
            self.wgpu.render_egui(&self.state.egui_ctx, full_output);
        }
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

            self.alive.store(true, Ordering::Release);

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
