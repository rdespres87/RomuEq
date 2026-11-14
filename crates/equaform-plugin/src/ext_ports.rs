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

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub struct EquaformAudioPorts(RawExtension<PluginExtensionSide, clap_plugin_audio_ports>);

unsafe impl Extension for EquaformAudioPorts {
    const IDENTIFIERS: &[&CStr] = &[CLAP_EXT_AUDIO_PORTS];
    type ExtensionSide = PluginExtensionSide;

    #[inline]
    unsafe fn from_raw(raw: RawExtension<Self::ExtensionSide>) -> Self {
        Self(unsafe { raw.cast() })
    }
}

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
