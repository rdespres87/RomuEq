//! CLAP GUI extension implementation for RomuEq.
//! Supports Cocoa NSView parenting at fixed studio dimensions 1180x560.

use clack_common::extensions::{
    Extension, ExtensionImplementation, PluginExtensionSide, RawExtension,
    RawExtensionImplementation,
};
use clack_plugin::extensions::wrapper::{PluginWrapper, PluginWrapperError};
use clack_plugin::prelude::*;
use clap_sys::ext::gui::*;
use clap_sys::plugin::clap_plugin;
use std::ffi::{c_char, CStr};

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub struct RomuEqGui(RawExtension<PluginExtensionSide, clap_plugin_gui>);

unsafe impl Extension for RomuEqGui {
    const IDENTIFIERS: &[&CStr] = &[CLAP_EXT_GUI];
    type ExtensionSide = PluginExtensionSide;

    #[inline]
    unsafe fn from_raw(raw: RawExtension<Self::ExtensionSide>) -> Self {
        Self(unsafe { raw.cast() })
    }
}

pub trait RomuEqGuiImplementation {
    fn create_ui(&mut self, is_floating: bool) -> bool;
    fn destroy_ui(&mut self);
    /// # Safety
    /// `window` must be a valid `clap_window` pointer provided by the CLAP host.
    unsafe fn set_parent(&mut self, window: *const clap_window) -> bool;
    fn show_ui(&mut self) -> bool;
    fn hide_ui(&mut self) -> bool;
}

unsafe impl<P: Plugin> ExtensionImplementation<P> for RomuEqGui
where
    for<'a> P::MainThread<'a>: RomuEqGuiImplementation,
{
    const IMPLEMENTATION: RawExtensionImplementation =
        RawExtensionImplementation::new(&clap_plugin_gui {
            is_api_supported: Some(is_api_supported),
            get_preferred_api: Some(get_preferred_api),
            create: Some(create::<P>),
            destroy: Some(destroy::<P>),
            set_scale: Some(set_scale),
            get_size: Some(get_size),
            can_resize: Some(can_resize),
            get_resize_hints: Some(get_resize_hints),
            adjust_size: Some(adjust_size),
            set_size: Some(set_size),
            set_parent: Some(set_parent::<P>),
            set_transient: Some(set_transient),
            suggest_title: Some(suggest_title),
            show: Some(show::<P>),
            hide: Some(hide::<P>),
        });
}

unsafe extern "C" fn is_api_supported(
    _plugin: *const clap_plugin,
    api: *const c_char,
    is_floating: bool,
) -> bool {
    if api.is_null() || is_floating {
        return false;
    }
    unsafe {
        let req = CStr::from_ptr(api);
        #[cfg(target_os = "macos")]
        {
            req == CLAP_WINDOW_API_COCOA
        }
        #[cfg(target_os = "windows")]
        {
            req == CLAP_WINDOW_API_WIN32
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            req == CLAP_WINDOW_API_X11
        }
    }
}

unsafe extern "C" fn get_preferred_api(
    _plugin: *const clap_plugin,
    api: *mut *const c_char,
    is_floating: *mut bool,
) -> bool {
    if api.is_null() || is_floating.is_null() {
        return false;
    }
    unsafe {
        #[cfg(target_os = "macos")]
        {
            *api = CLAP_WINDOW_API_COCOA.as_ptr();
        }
        #[cfg(target_os = "windows")]
        {
            *api = CLAP_WINDOW_API_WIN32.as_ptr();
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            *api = CLAP_WINDOW_API_X11.as_ptr();
        }
        *is_floating = false;
    }
    true
}

unsafe extern "C" fn create<P: Plugin>(
    plugin: *const clap_plugin,
    _api: *const c_char,
    is_floating: bool,
) -> bool
where
    for<'a> P::MainThread<'a>: RomuEqGuiImplementation,
{
    unsafe {
        PluginWrapper::<P>::handle(plugin, |p| {
            if p.main_thread().as_mut().create_ui(is_floating) {
                Ok(())
            } else {
                Err(PluginWrapperError::Plugin(PluginError::Message("Failed to create UI")))
            }
        })
        .is_some()
    }
}

unsafe extern "C" fn destroy<P: Plugin>(plugin: *const clap_plugin)
where
    for<'a> P::MainThread<'a>: RomuEqGuiImplementation,
{
    unsafe {
        let _ = PluginWrapper::<P>::handle(plugin, |p| {
            p.main_thread().as_mut().destroy_ui();
            Ok(())
        });
    }
}

unsafe extern "C" fn set_scale(_plugin: *const clap_plugin, _scale: f64) -> bool {
    true
}

unsafe extern "C" fn get_size(
    _plugin: *const clap_plugin,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    if width.is_null() || height.is_null() {
        return false;
    }
    unsafe {
        *width = 1180;
        *height = 560;
    }
    true
}

unsafe extern "C" fn can_resize(_plugin: *const clap_plugin) -> bool {
    false
}

unsafe extern "C" fn get_resize_hints(
    _plugin: *const clap_plugin,
    hints: *mut clap_gui_resize_hints,
) -> bool {
    if hints.is_null() {
        return false;
    }
    unsafe {
        let h = &mut *hints;
        h.can_resize_horizontally = false;
        h.can_resize_vertically = false;
        h.preserve_aspect_ratio = true;
        h.aspect_ratio_width = 1180;
        h.aspect_ratio_height = 560;
    }
    true
}

unsafe extern "C" fn adjust_size(
    _plugin: *const clap_plugin,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    if width.is_null() || height.is_null() {
        return false;
    }
    unsafe {
        *width = 1180;
        *height = 560;
    }
    true
}

unsafe extern "C" fn set_size(
    _plugin: *const clap_plugin,
    width: u32,
    height: u32,
) -> bool {
    width == 1180 && height == 560
}

unsafe extern "C" fn set_parent<P: Plugin>(
    plugin: *const clap_plugin,
    window: *const clap_window,
) -> bool
where
    for<'a> P::MainThread<'a>: RomuEqGuiImplementation,
{
    unsafe {
        PluginWrapper::<P>::handle(plugin, |p| {
            if p.main_thread().as_mut().set_parent(window) {
                Ok(())
            } else {
                Err(PluginWrapperError::Plugin(PluginError::Message("Failed to set window parent")))
            }
        })
        .is_some()
    }
}

unsafe extern "C" fn set_transient(
    _plugin: *const clap_plugin,
    _window: *const clap_window,
) -> bool {
    false
}

unsafe extern "C" fn suggest_title(
    _plugin: *const clap_plugin,
    _title: *const c_char,
) {
}

unsafe extern "C" fn show<P: Plugin>(plugin: *const clap_plugin) -> bool
where
    for<'a> P::MainThread<'a>: RomuEqGuiImplementation,
{
    unsafe {
        PluginWrapper::<P>::handle(plugin, |p| {
            if p.main_thread().as_mut().show_ui() {
                Ok(())
            } else {
                Err(PluginWrapperError::Plugin(PluginError::Message("Failed to show UI")))
            }
        })
        .is_some()
    }
}

unsafe extern "C" fn hide<P: Plugin>(plugin: *const clap_plugin) -> bool
where
    for<'a> P::MainThread<'a>: RomuEqGuiImplementation,
{
    unsafe {
        PluginWrapper::<P>::handle(plugin, |p| {
            if p.main_thread().as_mut().hide_ui() {
                Ok(())
            } else {
                Err(PluginWrapperError::Plugin(PluginError::Message("Failed to hide UI")))
            }
        })
        .is_some()
    }
}
