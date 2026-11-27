//! CLAP Latency extension implementation for Equaform.
//! Reports algorithmic plugin latency in samples for automatic DAW Plugin Delay Compensation.

use std::ffi::CStr;

use clack_common::extensions::{
    Extension, ExtensionImplementation, PluginExtensionSide, RawExtension,
    RawExtensionImplementation,
};
use clack_plugin::extensions::wrapper::PluginWrapper;
use clack_plugin::prelude::Plugin;
use clap_sys::ext::latency::{CLAP_EXT_LATENCY, clap_plugin_latency};
use clap_sys::plugin::clap_plugin;

// FFI Sanctuary: `EquaformLatency` wraps the CLAP latency extension handle (`clap_plugin_latency`).
// It is queried by host DAWs through the CLAP C ABI via `clack-plugin`.
// The Rust compiler flags it as unused in internal code paths because it is accessed exclusively across FFI boundaries.
#[derive(Copy, Clone)]
#[expect(
    dead_code,
    reason = "CLAP extension handle exposed across FFI via clack-plugin"
)]
pub struct EquaformLatency(RawExtension<PluginExtensionSide, clap_plugin_latency>);

// SAFETY: `CLAP_EXT_LATENCY` matches `clap_plugin_latency` on the plugin side.
unsafe impl Extension for EquaformLatency {
    const IDENTIFIERS: &[&CStr] = &[CLAP_EXT_LATENCY];
    type ExtensionSide = PluginExtensionSide;

    #[inline]
    unsafe fn from_raw(raw: RawExtension<Self::ExtensionSide>) -> Self {
        // SAFETY: Caller guarantees `raw` points to a valid `clap_plugin_latency` struct.
        Self(unsafe { raw.cast() })
    }
}

/// Trait implemented by plugin state that provides the current algorithmic latency in samples.
pub trait EquaformLatencyImplementation {
    fn latency_samples(&self) -> u32;
}

// SAFETY: `IMPLEMENTATION` provides a static `clap_plugin_latency` vtable matching the CLAP ABI.
unsafe impl<P: Plugin> ExtensionImplementation<P> for EquaformLatency
where
    for<'a> P::Shared<'a>: EquaformLatencyImplementation,
{
    const IMPLEMENTATION: RawExtensionImplementation =
        RawExtensionImplementation::new(&clap_plugin_latency {
            get: Some(get::<P>),
        });
}

unsafe extern "C" fn get<P: Plugin>(plugin: *const clap_plugin) -> u32
where
    for<'a> P::Shared<'a>: EquaformLatencyImplementation,
{
    // SAFETY: `plugin` is a valid host-owned `clap_plugin` pointer and `p.shared()` is `Sync`.
    unsafe {
        PluginWrapper::<P>::handle(plugin, |p| Ok(p.shared().latency_samples())).unwrap_or_default()
    }
}
