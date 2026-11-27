//! Native Windows Win32 (`HWND`) + DirectX 12/Vulkan (`wgpu` + `egui-wgpu`) embedded editor
//! for Equaform. Renders `equaform_gui::workspace_ui::render_workspace` at fixed
//! studio dimensions `1180x485` points inside the host DAW window.

#[cfg(target_os = "windows")]
mod windows {
    use std::num::NonZeroIsize;
    use std::ptr;
    use std::sync::Arc;

    use clap_sys::ext::gui::clap_window;
    use clap_sys::host::clap_host;
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
    use crate::editor::shared_host::{SharedEditorState, WgpuSurfaceConfig, WgpuSurfaceHost};
    use crate::editor::{EDITOR_HEIGHT, EDITOR_WIDTH, PluginEditor};

    const TIMER_ID: usize = 1001;
    const TIMER_INTERVAL_MS: u32 = 16; // ~60 FPS

    const CLASS_NAME: &[u16] = &[
        'E' as u16, 'q' as u16, 'u' as u16, 'a' as u16, 'f' as u16, 'o' as u16, 'r' as u16,
        'm' as u16, 'P' as u16, 'l' as u16, 'u' as u16, 'g' as u16, 'i' as u16, 'n' as u16,
        'W' as u16, 'i' as u16, 'n' as u16, '3' as u16, '2' as u16, 'V' as u16, 'i' as u16,
        'e' as u16, 'w' as u16, '_' as u16, 'v' as u16, '1' as u16, 0,
    ];

    fn ensure_wndclass_registered() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        // SAFETY: Called once via `std::sync::Once`; `CLASS_NAME` is static NUL-terminated UTF-16
        // and `win32_wndproc` is a valid `extern "system"` window procedure.
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

    struct Win32RuntimeConfig {
        hwnd: HWND,
        shared: Arc<EquaformSharedInner>,
        host: Option<*const clap_host>,
        scale_factor: f32,
        physical_width: u32,
        physical_height: u32,
    }

    struct Win32EditorRuntime {
        hwnd: HWND,
        state: SharedEditorState,
        wgpu: WgpuSurfaceHost,
        mouse_in_window: bool,
    }

    impl Win32EditorRuntime {
        fn new(config: Win32RuntimeConfig) -> Option<Self> {
            let non_zero_hwnd = NonZeroIsize::new(config.hwnd as isize)?;
            let raw_window = RawWindowHandle::Win32(Win32WindowHandle::new(non_zero_hwnd));
            let raw_display = RawDisplayHandle::Windows(WindowsDisplayHandle::new());

            // SAFETY: `config.hwnd` is a valid Win32 child window owned by `Win32Editor`, which
            // destroys it and clears `GWLP_USERDATA` in `destroy()` before dropping `Win32EditorRuntime`.
            let wgpu = unsafe {
                WgpuSurfaceHost::new(WgpuSurfaceConfig {
                    backends: wgpu::Backends::PRIMARY,
                    raw_display_handle: raw_display,
                    raw_window_handle: raw_window,
                    width_px: config.physical_width,
                    height_px: config.physical_height,
                    pixels_per_point: config.scale_factor,
                    allow_fallback_adapter: true,
                    device_label: "Equaform Win32 Device",
                })
            }
            .ok()?;

            let state = SharedEditorState::new(config.shared, config.host, "CLAP / VST3");
            state.egui_ctx.set_pixels_per_point(config.scale_factor);

            Some(Self {
                hwnd: config.hwnd,
                state,
                wgpu,
                mouse_in_window: false,
            })
        }

        fn resize(&mut self, width: u32, height: u32) {
            self.wgpu
                .reconfigure_size(width, height, self.wgpu.pixels_per_point());
        }

        fn update_dpi(&mut self) {
            // SAFETY: `self.hwnd` is a live Win32 window handle owned by `Win32Editor`.
            let dpi = unsafe { GetDpiForWindow(self.hwnd) };
            let new_scale = if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 };
            self.state.egui_ctx.set_pixels_per_point(new_scale);
            let new_width = (EDITOR_WIDTH as f32 * new_scale).round() as u32;
            let new_height = (EDITOR_HEIGHT as f32 * new_scale).round() as u32;
            // SAFETY: `self.hwnd` is a valid Win32 window handle; `SWP_NOMOVE | SWP_NOZORDER` ignores position/insert-after.
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
            self.wgpu.reconfigure_size(new_width, new_height, new_scale);
        }

        fn update_modifiers(&mut self) {
            // SAFETY: `GetKeyState` is a stateless Win32 query on virtual key codes.
            unsafe {
                let ctrl = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
                let shift = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
                let alt = (GetKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
                self.state.modifiers = egui::Modifiers {
                    alt,
                    ctrl,
                    shift,
                    mac_cmd: false,
                    command: ctrl,
                };
            }
        }

        fn render_frame(&mut self) {
            self.update_modifiers();
            let (sample_rate, peak_levels_db) = self.state.collect_audio_and_spectrum();
            let full_output = self.state.run_egui_frame(sample_rate, peak_levels_db);
            self.wgpu.render_egui(&self.state.egui_ctx, full_output);
        }

        fn on_mouse_move(&mut self, x: f32, y: f32) {
            self.update_modifiers();
            if !self.mouse_in_window {
                self.mouse_in_window = true;
                // SAFETY: `tme` is a properly sized `TRACKMOUSEEVENT` referencing live `self.hwnd`.
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
            let scale = self.wgpu.pixels_per_point();
            let logical_pos = egui::pos2(x / scale, y / scale);
            self.state
                .pending_events
                .push(egui::Event::PointerMoved(logical_pos));
        }

        fn on_mouse_leave(&mut self) {
            self.mouse_in_window = false;
            self.state.pending_events.push(egui::Event::PointerGone);
        }

        fn on_mouse_button(&mut self, button: egui::PointerButton, pressed: bool) {
            self.update_modifiers();
            let pos = self
                .state
                .pending_events
                .iter()
                .rev()
                .find_map(|e| match e {
                    egui::Event::PointerMoved(p) => Some(*p),
                    _ => None,
                })
                .unwrap_or(egui::Pos2::ZERO);
            let modifiers = self.state.modifiers;
            self.state.pending_events.push(egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            });
        }

        fn on_mouse_wheel(&mut self, delta: f32) {
            self.update_modifiers();
            let dy = delta / 120.0 * 20.0;
            let modifiers = self.state.modifiers;
            self.state.pending_events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, dy),
                modifiers,
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
                let modifiers = self.state.modifiers;
                self.state.pending_events.push(egui::Event::Key {
                    key: k,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers,
                });
            }
        }

        fn on_char(&mut self, c: char) {
            self.update_modifiers();
            if !c.is_control() {
                self.state
                    .pending_events
                    .push(egui::Event::Text(c.to_string()));
            }
        }
    }

    const WM_MOUSELEAVE_MSG: u32 = 0x02A3;

    fn handle_paint(runtime: &mut Win32EditorRuntime, hwnd: HWND) -> LRESULT {
        // SAFETY: `ps` is a POD `PAINTSTRUCT` initialized by `BeginPaint` and paired with `EndPaint`.
        unsafe {
            let mut ps = std::mem::zeroed();
            BeginPaint(hwnd, &mut ps);
            runtime.render_frame();
            EndPaint(hwnd, &ps);
        }
        0
    }

    fn handle_timer(runtime: &mut Win32EditorRuntime, hwnd: HWND) -> LRESULT {
        // SAFETY: `hwnd` is a valid Win32 window handle; `root_hwnd` is checked non-null before `IsIconic`.
        let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
        // SAFETY: `hwnd` is a valid Win32 window handle.
        let is_visible = unsafe { IsWindowVisible(hwnd) != 0 };
        let is_iconic = if !root_hwnd.is_null() {
            // SAFETY: `root_hwnd` was checked non-null above.
            unsafe { IsIconic(root_hwnd) != 0 }
        } else {
            false
        };

        if !is_visible || is_iconic {
            runtime.state.mark_zero_fps();
            return 0;
        }

        // SAFETY: `GetForegroundWindow` takes no arguments and returns a handle or null.
        let fg_hwnd = unsafe { GetForegroundWindow() };
        let is_focused = fg_hwnd == root_hwnd || fg_hwnd == hwnd;
        let is_active = is_focused || runtime.mouse_in_window || runtime.state.has_pending_events();

        if runtime.state.should_skip_frame(is_active) {
            return 0;
        }

        runtime.render_frame();
        0
    }

    fn handle_mouse_button(
        runtime: &mut Win32EditorRuntime,
        hwnd: HWND,
        button: egui::PointerButton,
        pressed: bool,
        capture: bool,
    ) -> LRESULT {
        if capture {
            if pressed {
                // SAFETY: `hwnd` is a valid Win32 window handle on the UI thread.
                unsafe {
                    SetFocus(hwnd);
                    SetCapture(hwnd);
                }
            } else {
                // SAFETY: Releases mouse capture acquired on button press on the UI thread.
                unsafe {
                    ReleaseCapture();
                }
            }
        }
        runtime.on_mouse_button(button, pressed);
        0
    }

    fn handle_mouse_message(
        runtime: &mut Win32EditorRuntime,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
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
                handle_mouse_button(runtime, hwnd, egui::PointerButton::Primary, true, true)
            }
            WM_LBUTTONUP => {
                handle_mouse_button(runtime, hwnd, egui::PointerButton::Primary, false, true)
            }
            WM_RBUTTONDOWN => {
                handle_mouse_button(runtime, hwnd, egui::PointerButton::Secondary, true, true)
            }
            WM_RBUTTONUP => {
                handle_mouse_button(runtime, hwnd, egui::PointerButton::Secondary, false, true)
            }
            WM_MBUTTONDOWN => {
                handle_mouse_button(runtime, hwnd, egui::PointerButton::Middle, true, false)
            }
            WM_MBUTTONUP => {
                handle_mouse_button(runtime, hwnd, egui::PointerButton::Middle, false, false)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam >> 16) & 0xFFFF) as i16 as f32;
                runtime.on_mouse_wheel(delta);
                0
            }
            _ => 0,
        }
    }

    fn handle_key_message(runtime: &mut Win32EditorRuntime, msg: u32, wparam: WPARAM) -> LRESULT {
        match msg {
            WM_KEYDOWN => runtime.on_key(wparam as u32, true),
            WM_KEYUP => runtime.on_key(wparam as u32, false),
            WM_CHAR => {
                if let Some(c) = char::from_u32(wparam as u32) {
                    runtime.on_char(c);
                }
            }
            _ => {}
        }
        0
    }

    fn handle_destroy(hwnd: HWND) -> LRESULT {
        // SAFETY: `hwnd` is being destroyed on the UI thread; stopping the timer and clearing
        // `GWLP_USERDATA` prevents any subsequent message from dereferencing `runtime`.
        unsafe {
            KillTimer(hwnd, TIMER_ID);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
        0
    }

    unsafe extern "system" fn win32_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // SAFETY: `hwnd` is the window receiving this Win32 message; `GWLP_USERDATA` holds either 0
        // or the live `Box<Win32EditorRuntime>` pointer stored by `Win32Editor::set_parent`.
        let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Win32EditorRuntime };
        if ptr.is_null() {
            // SAFETY: Standard Win32 default message dispatch for valid `(hwnd, msg, wparam, lparam)`.
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        // SAFETY: `ptr` is non-null, points to `Win32Editor::runtime` which is exclusively accessed
        // on the UI thread, and is cleared to 0 in `WM_DESTROY` before the box is dropped.
        let runtime = unsafe { &mut *ptr };

        match msg {
            WM_PAINT => handle_paint(runtime, hwnd),
            WM_TIMER => handle_timer(runtime, hwnd),
            WM_ERASEBKGND => 1,
            WM_SIZE => {
                runtime.resize((lparam & 0xFFFF) as u32, ((lparam >> 16) & 0xFFFF) as u32);
                0
            }
            WM_DPICHANGED => {
                runtime.update_dpi();
                0
            }
            WM_MOUSEMOVE | WM_MOUSELEAVE_MSG | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN
            | WM_RBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_MOUSEWHEEL => {
                handle_mouse_message(runtime, hwnd, msg, wparam, lparam)
            }
            WM_KEYDOWN | WM_KEYUP | WM_CHAR => handle_key_message(runtime, msg, wparam),
            WM_DESTROY => handle_destroy(hwnd),
            // SAFETY: Standard Win32 default message dispatch for unhandled messages.
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
            // SAFETY: Caller guarantees `window` is a valid CLAP host `clap_window` pointer;
            // `api` and `parent_hwnd` are checked non-null before Win32 child window creation.
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

                let runtime = Win32EditorRuntime::new(Win32RuntimeConfig {
                    hwnd: child_hwnd,
                    shared: Arc::clone(&self.shared),
                    host: self.host,
                    scale_factor: scale,
                    physical_width: width as u32,
                    physical_height: height as u32,
                });

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
                // SAFETY: `self.child_hwnd` is a live child window owned by `self`.
                unsafe { ShowWindow(self.child_hwnd, SW_SHOW) };
                true
            } else {
                false
            }
        }

        fn hide(&mut self) -> bool {
            if !self.child_hwnd.is_null() {
                // SAFETY: `self.child_hwnd` is a live child window owned by `self`.
                unsafe { ShowWindow(self.child_hwnd, SW_HIDE) };
                true
            } else {
                false
            }
        }

        fn destroy(&mut self) {
            if !self.child_hwnd.is_null() {
                // SAFETY: `self.child_hwnd` is a live child window owned by `self`; `DestroyWindow`
                // triggers `WM_DESTROY` which clears `GWLP_USERDATA` before `self.runtime` is dropped.
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
