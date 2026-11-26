//! Equaform CLAP Audio Plugin
//!
//! Bridges the CLAP ABI (`clack-plugin` v0.1.x) with `EngineLifecycle`,
//! lock-free parameter automation, state persistence, audio ports, and an
//! embedded macOS Cocoa (`NSView`) `egui` workspace UI.

pub mod editor;
pub mod ext_gui;
pub mod ext_params;
pub mod ext_ports;
pub mod params_map;
pub mod processor;
pub mod ui_cocoa;

use std::ffi::CStr;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use clack_common::extensions::{
    Extension, ExtensionImplementation, PluginExtensionSide, RawExtension,
    RawExtensionImplementation,
};
use clack_common::stream::{InputStream, OutputStream};
use clack_plugin::extensions::wrapper::PluginWrapper;
use clack_plugin::prelude::*;
use clap_sys::events::{
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_PARAM_GESTURE_BEGIN, CLAP_EVENT_PARAM_GESTURE_END,
    CLAP_EVENT_PARAM_VALUE, clap_event_header, clap_event_param_gesture, clap_event_param_value,
    clap_output_events,
};
use clap_sys::ext::gui::clap_window;
use clap_sys::ext::params::{CLAP_EXT_PARAMS, CLAP_PARAM_RESCAN_VALUES, clap_host_params};
use clap_sys::ext::state::{CLAP_EXT_STATE, clap_plugin_state};
use clap_sys::id::clap_id;
use clap_sys::plugin::clap_plugin;
use clap_sys::stream::{clap_istream, clap_ostream};

use equaform_common::{
    AudioRingConsumer, AudioRingProducer, EqParameters, create_audio_ring_buffer,
};

use ext_gui::{EquaformGui, EquaformGuiImplementation};
use ext_params::{EquaformParams, EquaformParamsImplementation};
use ext_ports::EquaformAudioPorts;
use params_map::{
    PARAM_COUNT_USIZE, apply_param_to_eq, default_eq_parameters, get_param_from_eq,
    normalize_param_value, param_id_to_slot, slot_to_param_id,
};
pub use processor::EquaformAudioProcessor;

// ---------------------------------------------------------------------------
// Plugin descriptor & state envelope
// ---------------------------------------------------------------------------

/// Stable descriptor shared across all plugin instances.
fn get_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("com.equaform.audio.equalizer", "Equaform-8")
        .with_vendor("Equaform Audio")
        .with_version("1.1.1")
        .with_description("High-Precision 8-Band Parametric Equalizer")
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PluginStateEnvelope {
    schema_version: u32,
    eq: EqParameters,
}

const STATE_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Lock-free shared state between AudioProcessor, MainThread, and Cocoa UI
// ---------------------------------------------------------------------------

/// Lock-free atomic state shared across the audio thread, CLAP main thread,
/// and the embedded Cocoa UI timer callback.
pub struct EquaformSharedInner {
    param_bits: [AtomicU32; PARAM_COUNT_USIZE],
    dirty_to_host: [AtomicBool; PARAM_COUNT_USIZE],
    gesture_begin_pending: [AtomicBool; PARAM_COUNT_USIZE],
    gesture_active: [AtomicBool; PARAM_COUNT_USIZE],
    gesture_end_pending: [AtomicBool; PARAM_COUNT_USIZE],
    version: AtomicU64,
    solo_mask: std::sync::atomic::AtomicU8,
    peak_l_bits: AtomicU32,
    peak_r_bits: AtomicU32,
    auto_gain_db_bits: AtomicU32,
    sample_rate_bits: AtomicU32,
    ring_producer: AudioRingProducer,
    ring_consumer: AudioRingConsumer,
}

impl EquaformSharedInner {
    /// Creates a new lock-free shared state initialized with default 8-band EQ parameters.
    pub fn new() -> Self {
        let defaults = default_eq_parameters();
        let param_bits = std::array::from_fn(|slot| {
            let id = slot_to_param_id(slot);
            let val = get_param_from_eq(&defaults, id).unwrap_or(0.0) as f32;
            AtomicU32::new(val.to_bits())
        });
        let dirty_to_host = std::array::from_fn(|_| AtomicBool::new(false));
        let gesture_begin_pending = std::array::from_fn(|_| AtomicBool::new(false));
        let gesture_active = std::array::from_fn(|_| AtomicBool::new(false));
        let gesture_end_pending = std::array::from_fn(|_| AtomicBool::new(false));
        let (ring_producer, ring_consumer) = create_audio_ring_buffer(16384);

        Self {
            param_bits,
            dirty_to_host,
            gesture_begin_pending,
            gesture_active,
            gesture_end_pending,
            version: AtomicU64::new(1),
            solo_mask: std::sync::atomic::AtomicU8::new(0),
            peak_l_bits: AtomicU32::new(f32::NEG_INFINITY.to_bits()),
            peak_r_bits: AtomicU32::new(f32::NEG_INFINITY.to_bits()),
            auto_gain_db_bits: AtomicU32::new(0.0f32.to_bits()),
            sample_rate_bits: AtomicU32::new(48000.0f32.to_bits()),
            ring_producer,
            ring_consumer,
        }
    }

    #[inline]
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    #[inline]
    pub fn get_param_value(&self, id: clap_id) -> Option<f64> {
        let slot = param_id_to_slot(id)?;
        Some(f32::from_bits(self.param_bits[slot].load(Ordering::Acquire)) as f64)
    }

    /// Updates a parameter from the host (automation / flush / load).
    /// Does NOT mark `dirty_to_host`.
    #[inline]
    pub fn set_param_from_host(&self, id: clap_id, value: f64) -> bool {
        let Some(slot) = param_id_to_slot(id) else {
            return false;
        };
        let norm = normalize_param_value(id, value);
        self.param_bits[slot].store(norm.to_bits(), Ordering::Release);
        self.version.fetch_add(1, Ordering::Release);
        true
    }

    /// Updates a parameter from the UI and tracks touch/latch automation gestures.
    #[inline]
    pub fn set_param_from_ui(&self, id: clap_id, value: f64, pointer_down: bool) -> bool {
        // Intercept monitor-only band solo (not exposed to DAW automation or presets).
        if id >= 100 && (id - 100) % 100 == equaform_common::parameter::band::SOLO {
            let band_idx = ((id - 100) / 100) as usize;
            if band_idx < equaform_common::MAX_BANDS {
                let bit = 1u8 << band_idx;
                if value > 0.5 {
                    self.solo_mask.fetch_or(bit, Ordering::AcqRel);
                } else {
                    self.solo_mask.fetch_and(!bit, Ordering::AcqRel);
                }
                self.version.fetch_add(1, Ordering::Release);
            }
            return false;
        }
        let Some(slot) = param_id_to_slot(id) else {
            return false;
        };
        let norm = normalize_param_value(id, value);
        let prev = f32::from_bits(self.param_bits[slot].load(Ordering::Acquire));
        if (prev - norm).abs() > 1e-6 {
            if !self.gesture_active[slot].load(Ordering::Acquire) {
                self.gesture_begin_pending[slot].store(true, Ordering::Release);
                if pointer_down {
                    self.gesture_active[slot].store(true, Ordering::Release);
                } else {
                    self.gesture_end_pending[slot].store(true, Ordering::Release);
                }
            }
            self.param_bits[slot].store(norm.to_bits(), Ordering::Release);
            self.dirty_to_host[slot].store(true, Ordering::Release);
            self.version.fetch_add(1, Ordering::Release);
        }
        true
    }

    /// Ends all open touch gestures when mouse buttons are released.
    /// Returns `true` if any active gesture was transitioned to ended.
    pub fn end_active_gestures(&self) -> bool {
        let mut any_ended = false;
        for slot in 0..PARAM_COUNT_USIZE {
            if self.gesture_active[slot].swap(false, Ordering::AcqRel) {
                self.gesture_end_pending[slot].store(true, Ordering::Release);
                any_ended = true;
            }
        }
        any_ended
    }

    /// Snapshots all 51 parameters lock-free into `EqParameters`.
    pub fn read_eq_parameters(&self) -> EqParameters {
        let mut eq = default_eq_parameters();
        for slot in 0..PARAM_COUNT_USIZE {
            let id = slot_to_param_id(slot);
            let val = f32::from_bits(self.param_bits[slot].load(Ordering::Acquire)) as f64;
            let _ = apply_param_to_eq(&mut eq, id, val);
        }
        eq.global.solo_mask = self.solo_mask.load(Ordering::Acquire);
        eq
    }

    /// Overwrites all 51 parameters from `EqParameters` without notifying host.
    pub fn write_eq_parameters_from_host(&self, eq: &EqParameters) {
        for slot in 0..PARAM_COUNT_USIZE {
            let id = slot_to_param_id(slot);
            let val = get_param_from_eq(eq, id).unwrap_or(0.0);
            let norm = normalize_param_value(id, val);
            self.param_bits[slot].store(norm.to_bits(), Ordering::Release);
        }
        self.solo_mask.store(eq.global.solo_mask, Ordering::Release);
        self.version.fetch_add(1, Ordering::Release);
    }

    /// Overwrites all 51 parameters from UI (e.g. preset load) and marks all changed slots dirty.
    pub fn write_eq_parameters_from_ui(&self, eq: &EqParameters) {
        for slot in 0..PARAM_COUNT_USIZE {
            let id = slot_to_param_id(slot);
            let val = get_param_from_eq(eq, id).unwrap_or(0.0);
            let norm = normalize_param_value(id, val);
            let prev = f32::from_bits(self.param_bits[slot].load(Ordering::Acquire));
            if (prev - norm).abs() > 1e-6 {
                self.gesture_begin_pending[slot].store(true, Ordering::Release);
                self.param_bits[slot].store(norm.to_bits(), Ordering::Release);
                self.dirty_to_host[slot].store(true, Ordering::Release);
                self.gesture_end_pending[slot].store(true, Ordering::Release);
            }
        }
        self.version.fetch_add(1, Ordering::Release);
    }

    #[inline]
    pub fn take_dirty_slot(&self, slot: usize) -> Option<(clap_id, f64)> {
        if slot >= PARAM_COUNT_USIZE {
            return None;
        }
        if self.dirty_to_host[slot].swap(false, Ordering::AcqRel) {
            let id = slot_to_param_id(slot);
            let val = f32::from_bits(self.param_bits[slot].load(Ordering::Acquire)) as f64;
            Some((id, val))
        } else {
            None
        }
    }

    #[inline]
    pub fn mark_dirty_slot(&self, slot: usize) {
        if slot < PARAM_COUNT_USIZE {
            self.dirty_to_host[slot].store(true, Ordering::Release);
        }
    }

    #[inline]
    pub fn take_gesture_begin(&self, slot: usize) -> Option<clap_id> {
        if slot < PARAM_COUNT_USIZE
            && self.gesture_begin_pending[slot].swap(false, Ordering::AcqRel)
        {
            Some(slot_to_param_id(slot))
        } else {
            None
        }
    }

    #[inline]
    pub fn mark_gesture_begin(&self, slot: usize) {
        if slot < PARAM_COUNT_USIZE {
            self.gesture_begin_pending[slot].store(true, Ordering::Release);
        }
    }

    #[inline]
    pub fn take_gesture_end(&self, slot: usize) -> Option<clap_id> {
        if slot < PARAM_COUNT_USIZE && self.gesture_end_pending[slot].swap(false, Ordering::AcqRel)
        {
            Some(slot_to_param_id(slot))
        } else {
            None
        }
    }

    #[inline]
    pub fn mark_gesture_end(&self, slot: usize) {
        if slot < PARAM_COUNT_USIZE {
            self.gesture_end_pending[slot].store(true, Ordering::Release);
        }
    }

    #[inline]
    pub fn set_sample_rate(&self, sr: f32) {
        self.sample_rate_bits.store(sr.to_bits(), Ordering::Release);
    }

    #[inline]
    pub fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate_bits.load(Ordering::Acquire))
    }

    #[inline]
    pub fn set_telemetry(&self, peak_l_db: f32, peak_r_db: f32, auto_gain_db: f32) {
        self.peak_l_bits
            .store(peak_l_db.to_bits(), Ordering::Relaxed);
        self.peak_r_bits
            .store(peak_r_db.to_bits(), Ordering::Relaxed);
        self.auto_gain_db_bits
            .store(auto_gain_db.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    pub fn peak_levels_db(&self) -> (f32, f32) {
        (
            f32::from_bits(self.peak_l_bits.load(Ordering::Relaxed)),
            f32::from_bits(self.peak_r_bits.load(Ordering::Relaxed)),
        )
    }

    #[inline]
    pub fn auto_gain_db(&self) -> f32 {
        f32::from_bits(self.auto_gain_db_bits.load(Ordering::Relaxed))
    }

    #[inline]
    pub fn ring_consumer(&self) -> &AudioRingConsumer {
        &self.ring_consumer
    }
}

impl Default for EquaformSharedInner {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe CLAP shared handle wrapping `Arc<EquaformSharedInner>`.
#[derive(Clone, Default)]
pub struct EquaformShared {
    inner: Arc<EquaformSharedInner>,
}

impl EquaformShared {
    pub fn inner(&self) -> &Arc<EquaformSharedInner> {
        &self.inner
    }
}

impl<'a> PluginShared<'a> for EquaformShared {}

impl EquaformParamsImplementation for EquaformShared {
    fn get_value(&self, id: clap_id) -> Option<f64> {
        self.inner.get_param_value(id)
    }

    fn set_param_from_host_flush(&self, id: clap_id, val: f64) {
        let _ = self.inner.set_param_from_host(id, val);
    }

    unsafe fn flush_pending_shared(&self, out: *const clap_output_events) {
        if out.is_null() {
            return;
        }
        // SAFETY: Caller guarantees `out` is a valid `clap_output_events` pointer from the CLAP host.
        unsafe {
            let Some(try_push) = (*out).try_push else {
                return;
            };
            for slot in 0..PARAM_COUNT_USIZE {
                if let Some(id) = self.inner.take_gesture_begin(slot) {
                    let ev = clap_event_param_gesture {
                        header: clap_event_header {
                            size: size_of::<clap_event_param_gesture>() as u32,
                            time: 0,
                            space_id: CLAP_CORE_EVENT_SPACE_ID,
                            type_: CLAP_EVENT_PARAM_GESTURE_BEGIN,
                            flags: 0,
                        },
                        param_id: id,
                    };
                    if !try_push(out, &ev.header) {
                        self.inner.mark_gesture_begin(slot);
                        continue;
                    }
                }

                if let Some((id, val)) = self.inner.take_dirty_slot(slot) {
                    let ev = clap_event_param_value {
                        header: clap_event_header {
                            size: size_of::<clap_event_param_value>() as u32,
                            time: 0,
                            space_id: CLAP_CORE_EVENT_SPACE_ID,
                            type_: CLAP_EVENT_PARAM_VALUE,
                            flags: 0,
                        },
                        param_id: id,
                        cookie: core::ptr::null_mut(),
                        note_id: -1,
                        port_index: -1,
                        channel: -1,
                        key: -1,
                        value: val,
                    };
                    if !try_push(out, &ev.header) {
                        self.inner.mark_dirty_slot(slot);
                        continue;
                    }
                }

                if let Some(id) = self.inner.take_gesture_end(slot) {
                    let ev = clap_event_param_gesture {
                        header: clap_event_header {
                            size: size_of::<clap_event_param_gesture>() as u32,
                            time: 0,
                            space_id: CLAP_CORE_EVENT_SPACE_ID,
                            type_: CLAP_EVENT_PARAM_GESTURE_END,
                            flags: 0,
                        },
                        param_id: id,
                    };
                    if !try_push(out, &ev.header) {
                        self.inner.mark_gesture_end(slot);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main thread
// ---------------------------------------------------------------------------

/// Main-thread plugin handle managing parameters, state save/load, and GUI.
pub struct EquaformMainThread<'a> {
    shared: Arc<EquaformSharedInner>,
    host: Option<HostMainThreadHandle<'a>>,
    editor: Box<dyn editor::PluginEditor>,
}

impl Default for EquaformMainThread<'_> {
    fn default() -> Self {
        let shared = Arc::new(EquaformSharedInner::new());
        let editor = editor::create_editor(Arc::clone(&shared), None);
        Self {
            shared,
            host: None,
            editor,
        }
    }
}

impl<'a> EquaformMainThread<'a> {
    fn new(shared: Arc<EquaformSharedInner>, host: HostMainThreadHandle<'a>) -> Self {
        let host_ptr = host.as_raw() as *const clap_sys::host::clap_host;
        let editor = editor::create_editor(Arc::clone(&shared), Some(host_ptr));
        Self {
            shared,
            host: Some(host),
            editor,
        }
    }

    fn request_host_params_rescan(&self) {
        if let Some(host) = &self.host {
            let raw = host.as_raw();
            if let Some(get_ext) = raw.get_extension {
                // SAFETY: `raw` is a valid `clap_host` handle; `CLAP_EXT_PARAMS` is NUL-terminated;
                // `ptr` is checked non-null before dereferencing its `rescan` callback.
                unsafe {
                    let ptr = get_ext(raw, CLAP_EXT_PARAMS.as_ptr()) as *const clap_host_params;
                    if !ptr.is_null() {
                        if let Some(rescan) = (*ptr).rescan {
                            rescan(raw, CLAP_PARAM_RESCAN_VALUES);
                        }
                    }
                }
            }
        }
    }
}

impl<'a> PluginMainThread<'a, EquaformShared> for EquaformMainThread<'a> {}

impl EquaformParamsImplementation for EquaformMainThread<'_> {
    fn get_value(&self, id: clap_id) -> Option<f64> {
        self.shared.get_param_value(id)
    }

    fn set_param_from_host_flush(&self, id: clap_id, val: f64) {
        let _ = self.shared.set_param_from_host(id, val);
    }

    unsafe fn flush_pending_shared(&self, out: *const clap_output_events) {
        let shared_wrapper = EquaformShared {
            inner: Arc::clone(&self.shared),
        };
        // SAFETY: Caller guarantees `out` is a valid `clap_output_events` pointer from the CLAP host.
        unsafe { shared_wrapper.flush_pending_shared(out) };
    }
}

impl EquaformGuiImplementation for EquaformMainThread<'_> {
    fn create_ui(&mut self, is_floating: bool) -> bool {
        self.editor.create(is_floating)
    }

    fn destroy_ui(&mut self) {
        self.editor.destroy();
    }

    unsafe fn set_parent(&mut self, window: *const clap_window) -> bool {
        // SAFETY: Caller guarantees `window` is a valid `clap_window` pointer from the CLAP host.
        unsafe { self.editor.set_parent(window) }
    }

    fn show_ui(&mut self) -> bool {
        self.editor.show()
    }

    fn hide_ui(&mut self) -> bool {
        self.editor.hide()
    }
}

// ---------------------------------------------------------------------------
// Audio processor — lock-free real-time DSP
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Plugin entry & extensions registration
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct EquaformPlugin;

impl Plugin for EquaformPlugin {
    type Shared<'a> = EquaformShared;
    type MainThread<'a> = EquaformMainThread<'a>;
    type AudioProcessor<'a> = EquaformAudioProcessor;

    fn declare_extensions(
        builder: &mut PluginExtensions<Self>,
        _shared: Option<&Self::Shared<'_>>,
    ) {
        builder
            .register::<EquaformState>()
            .register::<EquaformAudioPorts>()
            .register::<EquaformParams>()
            .register::<EquaformGui>();
    }
}

impl DefaultPluginFactory for EquaformPlugin {
    fn get_descriptor() -> PluginDescriptor {
        get_descriptor()
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(EquaformShared::default())
    }

    fn new_main_thread<'a>(
        host: HostMainThreadHandle<'a>,
        shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(EquaformMainThread::new(Arc::clone(shared.inner()), host))
    }
}

// ---------------------------------------------------------------------------
// CLAP State extension (versioned JSON envelope)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub struct EquaformState(RawExtension<PluginExtensionSide, clap_plugin_state>);

// SAFETY: `CLAP_EXT_STATE` matches `clap_plugin_state` on the plugin side.
unsafe impl Extension for EquaformState {
    const IDENTIFIERS: &[&CStr] = &[CLAP_EXT_STATE];
    type ExtensionSide = PluginExtensionSide;

    #[inline]
    unsafe fn from_raw(raw: RawExtension<Self::ExtensionSide>) -> Self {
        // SAFETY: Caller guarantees `raw` points to a valid `clap_plugin_state` struct.
        Self(unsafe { raw.cast() })
    }
}

pub trait EquaformStateImplementation {
    fn save(&mut self, save: &mut OutputStream) -> Result<(), PluginError>;
    fn load(&mut self, load: &mut InputStream) -> Result<(), PluginError>;
}

// SAFETY: `IMPLEMENTATION` provides a static `clap_plugin_state` vtable matching the CLAP ABI.
unsafe impl<P: Plugin> ExtensionImplementation<P> for EquaformState
where
    for<'a> P::MainThread<'a>: EquaformStateImplementation,
{
    const IMPLEMENTATION: RawExtensionImplementation =
        RawExtensionImplementation::new(&clap_plugin_state {
            save: Some(save::<P>),
            load: Some(load::<P>),
        });
}

unsafe extern "C" fn save<P: Plugin>(
    plugin: *const clap_plugin,
    ostream: *const clap_ostream,
) -> bool
where
    for<'a> P::MainThread<'a>: EquaformStateImplementation,
{
    if ostream.is_null() {
        return false;
    }
    // SAFETY: `plugin` and non-null `ostream` are valid CLAP host pointers on the main thread.
    unsafe {
        PluginWrapper::<P>::handle(plugin, |p| {
            let output = OutputStream::from_raw_mut(&mut *(ostream as *mut _));
            p.main_thread().as_mut().save(output)?;
            Ok(())
        })
        .is_some()
    }
}

unsafe extern "C" fn load<P: Plugin>(
    plugin: *const clap_plugin,
    istream: *const clap_istream,
) -> bool
where
    for<'a> P::MainThread<'a>: EquaformStateImplementation,
{
    if istream.is_null() {
        return false;
    }
    // SAFETY: `plugin` and non-null `istream` are valid CLAP host pointers on the main thread.
    unsafe {
        PluginWrapper::<P>::handle(plugin, |p| {
            let input = InputStream::from_raw_mut(&mut *(istream as *mut _));
            p.main_thread().as_mut().load(input)?;
            Ok(())
        })
        .is_some()
    }
}

impl EquaformStateImplementation for EquaformMainThread<'_> {
    fn save(&mut self, save: &mut OutputStream) -> Result<(), PluginError> {
        let envelope = PluginStateEnvelope {
            schema_version: STATE_SCHEMA_VERSION,
            eq: self.shared.read_eq_parameters(),
        };
        let data = serde_json::to_vec(&envelope)
            .map_err(|_| PluginError::Message("Failed to serialize state"))?;

        let mut written = 0;
        while written < data.len() {
            let n = save
                .write(&data[written..])
                .map_err(|_| PluginError::Message("Stream write error"))?;
            if n == 0 {
                return Err(PluginError::Message("Stream write returned 0"));
            }
            written += n;
        }
        Ok(())
    }

    fn load(&mut self, load: &mut InputStream) -> Result<(), PluginError> {
        let mut data = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            let n = load
                .read(&mut buf)
                .map_err(|_| PluginError::Message("Stream read error"))?;
            if n == 0 {
                break;
            }
            if data.len() + n > 1024 * 1024 {
                return Err(PluginError::Message("State payload exceeds limit"));
            }
            data.extend_from_slice(&buf[..n]);
        }

        let eq = if let Ok(env) = serde_json::from_slice::<PluginStateEnvelope>(&data) {
            if env.schema_version > STATE_SCHEMA_VERSION {
                return Err(PluginError::Message(
                    "Unsupported future state schema version",
                ));
            }
            env.eq
        } else {
            serde_json::from_slice::<EqParameters>(&data)
                .map_err(|_| PluginError::Message("Failed to deserialize state"))?
        };

        self.shared.write_eq_parameters_from_host(&eq);
        self.request_host_params_rescan();
        Ok(())
    }
}

clack_export_entry!(SinglePluginEntry::<EquaformPlugin>);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use equaform_common::MAX_BLOCK_SIZE;
    use equaform_common::{FilterType, Slope};
    use params_map::{PARAM_COUNT, get_param_meta};

    #[test]
    fn test_all_59_params_round_trip_mapping() {
        assert_eq!(PARAM_COUNT, 60);
        let shared = EquaformSharedInner::new();

        for idx in 0..PARAM_COUNT {
            let meta = get_param_meta(idx).expect("missing param meta");
            let slot = param_id_to_slot(meta.id).expect("missing slot");
            assert_eq!(slot, idx as usize);
            assert_eq!(slot_to_param_id(slot), meta.id);

            let stored = shared.get_param_value(meta.id).expect("missing value");
            assert!(
                (stored - meta.default_value).abs() < 1e-4,
                "param {} default mismatch: {} vs {}",
                meta.id,
                stored,
                meta.default_value
            );
        }
    }

    #[test]
    fn test_ui_param_sets_dirty_to_host_and_gestures() {
        let shared = EquaformSharedInner::new();
        assert!(shared.set_param_from_ui(103, 6.5, true));
        let slot = param_id_to_slot(103).unwrap();
        assert_eq!(shared.take_gesture_begin(slot), Some(103));
        let (id, val) = shared.take_dirty_slot(slot).unwrap();
        assert_eq!(id, 103);
        assert!((val - 6.5).abs() < 1e-4);
        assert!(shared.take_gesture_end(slot).is_none());

        shared.end_active_gestures();
        assert_eq!(shared.take_gesture_end(slot), Some(103));
        assert!(shared.take_dirty_slot(slot).is_none());
    }

    #[test]
    fn test_audio_processor_block_sizes_including_oversize() {
        for block_size in [1usize, 64, 256, 512, 1024, 10_000] {
            let shared = Arc::new(EquaformSharedInner::new());
            shared.set_param_from_host(503, 6.0);
            shared.set_param_from_host(501, FilterType::Bell.to_raw() as f64);
            shared.set_param_from_host(505, Slope::Db12.to_raw() as f64);

            let mut proc = EquaformAudioProcessor::new(Arc::clone(&shared));
            assert!(proc.prepare(48000.0, block_size.min(MAX_BLOCK_SIZE), 2));

            let in_l: Vec<f32> = (0..block_size)
                .map(|i| {
                    (2.0 * std::f32::consts::PI * 1000.0 * (i + 1) as f32 / 48000.0).sin() * 0.25
                })
                .collect();
            let in_r = in_l.clone();
            let mut out_l = vec![0.0f32; block_size];
            let mut out_r = vec![0.0f32; block_size];

            assert!(proc.process_planar(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
            // Verify tail beyond MAX_BLOCK_SIZE is also processed (non-zero)
            if block_size > MAX_BLOCK_SIZE {
                assert!(out_l[MAX_BLOCK_SIZE..].iter().any(|&s| s.abs() > 1e-4));
            }
            let (pl, pr) = shared.peak_levels_db();
            assert!(pl.is_finite() && pr.is_finite());
        }
    }

    #[test]
    fn test_state_envelope_serde_round_trip() {
        let shared = Arc::new(EquaformSharedInner::new());
        shared.set_param_from_host(2, -3.5);
        shared.set_param_from_host(103, 4.25);

        let envelope = PluginStateEnvelope {
            schema_version: STATE_SCHEMA_VERSION,
            eq: shared.read_eq_parameters(),
        };
        let json = serde_json::to_vec(&envelope).unwrap();
        let decoded: PluginStateEnvelope = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.schema_version, 1);
        assert!((decoded.eq.global.output_gain_db - (-3.5)).abs() < 1e-4);
        assert!((decoded.eq.bands[0].gain_db - 4.25).abs() < 1e-4);
    }
}
