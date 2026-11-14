//! Unified audio plugin editor abstraction for Equaform.
//!
//! Provides the `PluginEditor` lifecycle trait and factory method `create_editor`
//! routing to native backends:
//! - macOS: Cocoa (`NSView` + Metal via `wgpu`)
//! - Windows: Win32 (`HWND` + DirectX 12/Vulkan via `wgpu`)
//! - Headless / fallback: Dummy stub

use clap_sys::ext::gui::clap_window;
use clap_sys::host::clap_host;
use std::sync::Arc;

use crate::EquaformSharedInner;

pub const EDITOR_WIDTH: f64 = 1180.0;
pub const EDITOR_HEIGHT: f64 = 485.0;

/// Unified lifecycle interface for plugin GUI windowing backends.
pub trait PluginEditor: 'static {
    /// Creates the editor resources (allocates contexts, starts render loops).
    fn create(&mut self, is_floating: bool) -> bool;

    /// Attaches the editor UI as a child of the host's native window.
    ///
    /// # Safety
    /// `window` must point to a valid, host-provided `clap_window` struct.
    unsafe fn set_parent(&mut self, window: *const clap_window) -> bool;

    /// Shows the plugin UI window.
    fn show(&mut self) -> bool;

    /// Hides the plugin UI window.
    fn hide(&mut self) -> bool;

    /// Destroys all window handles, surfaces, and resources.
    fn destroy(&mut self);
}

#[cfg(target_os = "macos")]
pub mod cocoa;

#[cfg(target_os = "windows")]
pub mod win32;

pub mod dummy;

/// Factory function creating the appropriate platform editor instance.
pub fn create_editor(
    shared: Arc<EquaformSharedInner>,
    host: Option<*const clap_host>,
) -> Box<dyn PluginEditor> {
    #[cfg(target_os = "macos")]
    {
        Box::new(cocoa::CocoaEditor::new(shared, host))
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(win32::Win32Editor::new(shared, host))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Box::new(dummy::DummyEditor::new(shared, host))
    }
}
