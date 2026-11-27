//! CLAP Audio Ports extension implementation for Equaform.
//! Exposes 1 stereo input and 1 stereo output port.

use clack_common::extensions::{
    Extension, ExtensionImplementation, PluginExtensionSide, RawExtension,
    RawExtensionImplementation,
};
use clack_plugin::prelude::Plugin;
use clap_sys::ext::audio_ports::*;
use clap_sys::plugin::clap_plugin;
use std::ffi::{CStr, c_char};

// FFI Sanctuary: `EquaformAudioPorts` wraps the CLAP audio ports extension handle (`clap_plugin_audio_ports`).
// It is queried and enumerated by host DAWs through the CLAP C ABI via `clack-plugin`.
// The Rust compiler flags it as unused in internal code paths because it is accessed exclusively across FFI boundaries.
#[derive(Copy, Clone)]
#[expect(
    dead_code,
    reason = "CLAP extension handle exposed across FFI via clack-plugin"
)]
pub struct EquaformAudioPorts(RawExtension<PluginExtensionSide, clap_plugin_audio_ports>);

// SAFETY: `CLAP_EXT_AUDIO_PORTS` matches `clap_plugin_audio_ports` on the plugin side.
unsafe impl Extension for EquaformAudioPorts {
    const IDENTIFIERS: &[&CStr] = &[CLAP_EXT_AUDIO_PORTS];
    type ExtensionSide = PluginExtensionSide;

    #[inline]
    unsafe fn from_raw(raw: RawExtension<Self::ExtensionSide>) -> Self {
        // SAFETY: Caller guarantees `raw` points to a valid `clap_plugin_audio_ports` struct.
        Self(unsafe { raw.cast() })
    }
}

// SAFETY: `IMPLEMENTATION` provides a static `clap_plugin_audio_ports` vtable matching the CLAP ABI.
unsafe impl<P: Plugin> ExtensionImplementation<P> for EquaformAudioPorts {
    const IMPLEMENTATION: RawExtensionImplementation =
        RawExtensionImplementation::new(&clap_plugin_audio_ports {
            count: Some(count),
            get: Some(get),
        });
}

unsafe extern "C" fn count(_plugin: *const clap_plugin, _is_input: bool) -> u32 {
    1 // 1 stereo port for input and 1 stereo port for output
}

unsafe extern "C" fn get(
    _plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_audio_port_info,
) -> bool {
    if index != 0 || info.is_null() {
        return false;
    }

    // SAFETY: `info` was checked non-null above and is a valid writable pointer provided by the CLAP host.
    unsafe {
        let inf = &mut *info;
        inf.id = if is_input { 0 } else { 1 };
        inf.flags = CLAP_AUDIO_PORT_IS_MAIN;
        inf.channel_count = 2; // Stereo
        inf.port_type = CLAP_PORT_STEREO.as_ptr();
        inf.in_place_pair = if is_input { 1 } else { 0 };

        let name_bytes: &[u8] = if is_input {
            b"Stereo In\0"
        } else {
            b"Stereo Out\0"
        };
        for (i, &b) in name_bytes.iter().enumerate() {
            if i < inf.name.len() {
                inf.name[i] = b as c_char;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::mem::MaybeUninit;

    #[test]
    fn test_audio_ports_count_and_get() {
        let p: *const clap_plugin = std::ptr::null();
        // SAFETY: `count` ignores the plugin pointer and returns the static port count.
        unsafe {
            assert_eq!(count(p, true), 1);
            assert_eq!(count(p, false), 1);
            assert!(!get(p, 0, true, std::ptr::null_mut()));
            assert!(!get(p, 1, true, std::ptr::null_mut()));
        }

        let mut info = MaybeUninit::<clap_audio_port_info>::zeroed();
        let ptr = info.as_mut_ptr();
        // SAFETY: `ptr` points to a valid zeroed `clap_audio_port_info` and index 0 is valid.
        unsafe {
            assert!(get(p, 0, true, ptr));
            let i = &*ptr;
            assert_eq!((i.id, i.in_place_pair, i.channel_count), (0, 1, 2));
            assert_eq!(CStr::from_ptr(i.name.as_ptr()).to_bytes(), b"Stereo In");
        }

        let mut info_out = MaybeUninit::<clap_audio_port_info>::zeroed();
        let ptr_out = info_out.as_mut_ptr();
        // SAFETY: `ptr_out` points to a valid zeroed `clap_audio_port_info` and index 0 is valid.
        unsafe {
            assert!(get(p, 0, false, ptr_out));
            let i = &*ptr_out;
            assert_eq!((i.id, i.in_place_pair, i.channel_count), (1, 0, 2));
            assert_eq!(CStr::from_ptr(i.name.as_ptr()).to_bytes(), b"Stereo Out");
        }
    }
}
