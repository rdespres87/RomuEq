//! CLAP Parameters extension implementation for Equaform.
//! Exposes 61 automatable parameters with two-way mapping to EqParameters.

use clack_common::extensions::{
    Extension, ExtensionImplementation, PluginExtensionSide, RawExtension,
    RawExtensionImplementation,
};
use clack_plugin::extensions::wrapper::{PluginWrapper, PluginWrapperError};
use clack_plugin::prelude::*;
use clap_sys::events::*;
use clap_sys::ext::params::*;
use clap_sys::id::clap_id;
use clap_sys::plugin::clap_plugin;
use std::ffi::{CStr, c_char};

use crate::params_map::{PARAM_COUNT, format_param_value, get_param_meta, parse_param_value};

// FFI Sanctuary: `EquaformParams` wraps the CLAP parameter extension handle (`clap_plugin_params`).
// It is queried and automated by host DAWs through the CLAP C ABI via `clack-plugin`.
// The Rust compiler flags it as unused in internal code paths because it is accessed exclusively across FFI boundaries.
#[derive(Copy, Clone)]
#[allow(dead_code)]
pub struct EquaformParams(RawExtension<PluginExtensionSide, clap_plugin_params>);

// SAFETY: `CLAP_EXT_PARAMS` matches `clap_plugin_params` on the plugin side.
unsafe impl Extension for EquaformParams {
    const IDENTIFIERS: &[&CStr] = &[CLAP_EXT_PARAMS];
    type ExtensionSide = PluginExtensionSide;

    #[inline]
    unsafe fn from_raw(raw: RawExtension<Self::ExtensionSide>) -> Self {
        // SAFETY: Caller guarantees `raw` points to a valid `clap_plugin_params` struct.
        Self(unsafe { raw.cast() })
    }
}

pub trait EquaformParamsImplementation {
    fn get_value(&self, id: clap_id) -> Option<f64>;
    fn set_param_from_host_flush(&self, id: clap_id, val: f64);
    /// # Safety
    /// `out` must be a valid `clap_output_events` pointer provided by the CLAP host.
    unsafe fn flush_pending_shared(&self, out: *const clap_output_events);
}

// SAFETY: `IMPLEMENTATION` provides a static `clap_plugin_params` vtable matching the CLAP ABI.
unsafe impl<P: Plugin> ExtensionImplementation<P> for EquaformParams
where
    for<'a> P::MainThread<'a>: EquaformParamsImplementation,
    for<'a> P::Shared<'a>: EquaformParamsImplementation,
{
    const IMPLEMENTATION: RawExtensionImplementation =
        RawExtensionImplementation::new(&clap_plugin_params {
            count: Some(count),
            get_info: Some(get_info),
            get_value: Some(get_value::<P>),
            value_to_text: Some(value_to_text),
            text_to_value: Some(text_to_value),
            flush: Some(flush::<P>),
        });
}

unsafe extern "C" fn count(_plugin: *const clap_plugin) -> u32 {
    PARAM_COUNT
}

unsafe extern "C" fn get_info(
    _plugin: *const clap_plugin,
    param_index: u32,
    param_info: *mut clap_param_info,
) -> bool {
    if param_info.is_null() {
        return false;
    }
    let Some(meta) = get_param_meta(param_index) else {
        return false;
    };

    // SAFETY: `param_info` was checked non-null and is a writable pointer from the CLAP host.
    unsafe {
        let info = &mut *param_info;
        info.id = meta.id;
        info.flags = meta.flags;
        info.cookie = core::ptr::null_mut();
        info.min_value = meta.min_value;
        info.max_value = meta.max_value;
        info.default_value = meta.default_value;

        info.name.fill(0);
        let name_bytes = meta.name.as_bytes();
        let name_len = name_bytes.len().min(info.name.len().saturating_sub(1));
        for (i, &b) in name_bytes.iter().enumerate().take(name_len) {
            info.name[i] = b as c_char;
        }

        info.module.fill(0);
        let mod_bytes = meta.module.as_bytes();
        let mod_len = mod_bytes.len().min(info.module.len().saturating_sub(1));
        for (i, &b) in mod_bytes.iter().enumerate().take(mod_len) {
            info.module[i] = b as c_char;
        }
    }

    true
}

unsafe extern "C" fn get_value<P: Plugin>(
    plugin: *const clap_plugin,
    param_id: clap_id,
    out_value: *mut f64,
) -> bool
where
    for<'a> P::MainThread<'a>: EquaformParamsImplementation,
{
    if out_value.is_null() {
        return false;
    }
    // SAFETY: `plugin` is a valid host-owned `clap_plugin` and `out_value` is a non-null writable `f64`.
    unsafe {
        PluginWrapper::<P>::handle(plugin, |p| {
            let mt = p.main_thread().as_ref();
            if let Some(val) = mt.get_value(param_id) {
                *out_value = val;
                Ok(())
            } else {
                Err(PluginWrapperError::Plugin(PluginError::Message(
                    "Unknown param id",
                )))
            }
        })
        .is_some()
    }
}

unsafe extern "C" fn value_to_text(
    _plugin: *const clap_plugin,
    param_id: clap_id,
    value: f64,
    out_buffer: *mut c_char,
    out_buffer_capacity: u32,
) -> bool {
    if out_buffer.is_null() || out_buffer_capacity == 0 {
        return false;
    }

    let text = format_param_value(param_id, value);
    let bytes = text.as_bytes();
    let len = bytes.len().min(out_buffer_capacity as usize - 1);
    // SAFETY: `out_buffer` is non-null with capacity >= `len + 1` bytes provided by the CLAP host.
    unsafe {
        for (i, &b) in bytes.iter().enumerate().take(len) {
            *out_buffer.add(i) = b as c_char;
        }
        *out_buffer.add(len) = 0;
    }

    true
}

unsafe extern "C" fn text_to_value(
    _plugin: *const clap_plugin,
    param_id: clap_id,
    param_value_text: *const c_char,
    out_value: *mut f64,
) -> bool {
    if param_value_text.is_null() || out_value.is_null() {
        return false;
    }
    // SAFETY: `param_value_text` is a non-null NUL-terminated host string and `out_value` is writable.
    unsafe {
        let cstr = CStr::from_ptr(param_value_text);
        let Ok(s) = cstr.to_str() else {
            return false;
        };
        if let Some(parsed) = parse_param_value(param_id, s) {
            *out_value = parsed;
            return true;
        }
    }
    false
}

unsafe extern "C" fn flush<P: Plugin>(
    plugin: *const clap_plugin,
    in_: *const clap_input_events,
    out: *const clap_output_events,
) where
    for<'a> P::MainThread<'a>: EquaformParamsImplementation,
    for<'a> P::Shared<'a>: EquaformParamsImplementation,
{
    // SAFETY: `plugin`, `in_`, and `out` (when non-null) are valid CLAP host pointers for this flush call.
    unsafe {
        let _ = PluginWrapper::<P>::handle(plugin, |p| {
            let shared = p.shared();
            if !in_.is_null() {
                if let (Some(size_fn), Some(get_fn)) = ((*in_).size, (*in_).get) {
                    let size = size_fn(in_);
                    for i in 0..size {
                        let hdr = get_fn(in_, i);
                        if !hdr.is_null()
                            && (*hdr).space_id == CLAP_CORE_EVENT_SPACE_ID
                            && (*hdr).type_ == CLAP_EVENT_PARAM_VALUE
                        {
                            let ev = &*(hdr as *const clap_event_param_value);
                            shared.set_param_from_host_flush(ev.param_id, ev.value);
                        }
                    }
                }
            }
            if !out.is_null() {
                shared.flush_pending_shared(out);
            }
            Ok(())
        });
    }
}
