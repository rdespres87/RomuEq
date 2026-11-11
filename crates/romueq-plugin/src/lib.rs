//! RomuEq CLAP Audio Plugin
//!
//! Bridges the CLAP ABI (`clack-plugin` v0.1.x) with `EngineLifecycle`,
//! lock-free parameter automation, state persistence, audio ports, and an
//! embedded macOS Cocoa (`NSView`) `egui` workspace UI.

pub mod ext_gui;
pub mod ext_params;
pub mod ext_ports;
pub mod params_map;
pub mod ui_cocoa;

use std::ffi::{CStr, c_void};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use clack_common::events::event_types::{
    ParamGestureBeginEvent, ParamGestureEndEvent, ParamValueEvent,
};
use clack_common::events::spaces::CoreEventSpace;
use clack_common::events::Pckn;
use clack_common::extensions::{
    Extension, ExtensionImplementation, PluginExtensionSide, RawExtension,
    RawExtensionImplementation,
};
use clack_common::stream::{InputStream, OutputStream};
use clack_common::utils::{ClapId, Cookie};
use clack_plugin::extensions::wrapper::PluginWrapper;
use clack_plugin::prelude::*;
use clap_sys::events::{
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_PARAM_GESTURE_BEGIN, CLAP_EVENT_PARAM_GESTURE_END,
    CLAP_EVENT_PARAM_VALUE, clap_event_header, clap_event_param_gesture, clap_event_param_value,
    clap_output_events,
};
use clap_sys::ext::gui::{CLAP_WINDOW_API_COCOA, clap_window};
use clap_sys::ext::params::{CLAP_EXT_PARAMS, CLAP_PARAM_RESCAN_VALUES, clap_host_params};
use clap_sys::ext::state::{CLAP_EXT_STATE, clap_plugin_state};
use clap_sys::id::clap_id;
use clap_sys::plugin::clap_plugin;
use clap_sys::stream::{clap_istream, clap_ostream};

use romueq_common::{
    AudioRingConsumer, AudioRingProducer, EqParameters, MAX_BLOCK_SIZE, create_audio_ring_buffer,
};
use romueq_engine::lifecycle::EngineLifecycle;

use ext_gui::{RomuEqGui, RomuEqGuiImplementation};
use ext_params::{RomuEqParams, RomuEqParamsImplementation};
use ext_ports::RomuEqAudioPorts;
use params_map::{
    PARAM_COUNT_USIZE, apply_param_to_eq, default_eq_parameters, get_param_from_eq,
    normalize_param_value, param_id_to_slot, slot_to_param_id,
};

// ---------------------------------------------------------------------------
// Plugin descriptor & state envelope
// ---------------------------------------------------------------------------

/// Stable descriptor shared across all plugin instances.
fn get_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("com.romueq.plugin", "RomuEq")
        .with_vendor("RomuEq")
        .with_version("0.1.0")
        .with_description("Real-time spectral equalizer plugin")
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
pub struct RomuEqSharedInner {
    param_bits: [AtomicU32; PARAM_COUNT_USIZE],
    dirty_to_host: [AtomicBool; PARAM_COUNT_USIZE],
    gesture_begin_pending: [AtomicBool; PARAM_COUNT_USIZE],
    gesture_active: [AtomicBool; PARAM_COUNT_USIZE],
    gesture_end_pending: [AtomicBool; PARAM_COUNT_USIZE],
    version: AtomicU64,
    peak_l_bits: AtomicU32,
    peak_r_bits: AtomicU32,
    auto_gain_db_bits: AtomicU32,
    sample_rate_bits: AtomicU32,
    ring_producer: AudioRingProducer,
    ring_consumer: AudioRingConsumer,
}

impl RomuEqSharedInner {
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
        if slot < PARAM_COUNT_USIZE && self.gesture_begin_pending[slot].swap(false, Ordering::AcqRel)
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
        if slot < PARAM_COUNT_USIZE && self.gesture_end_pending[slot].swap(false, Ordering::AcqRel) {
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

impl Default for RomuEqSharedInner {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe CLAP shared handle wrapping `Arc<RomuEqSharedInner>`.
#[derive(Clone, Default)]
pub struct RomuEqShared {
    inner: Arc<RomuEqSharedInner>,
}

impl RomuEqShared {
    pub fn inner(&self) -> &Arc<RomuEqSharedInner> {
        &self.inner
    }
}

impl<'a> PluginShared<'a> for RomuEqShared {}

impl RomuEqParamsImplementation for RomuEqShared {
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

/// Main-thread plugin handle managing parameters, state save/load, and Cocoa GUI.
pub struct RomuEqMainThread<'a> {
    shared: Arc<RomuEqSharedInner>,
    host: Option<HostMainThreadHandle<'a>>,
    editor: ui_cocoa::CocoaEditor,
}

impl Default for RomuEqMainThread<'_> {
    fn default() -> Self {
        let shared = Arc::new(RomuEqSharedInner::new());
        let editor = ui_cocoa::CocoaEditor::new(Arc::clone(&shared), None);
        Self {
            shared,
            host: None,
            editor,
        }
    }
}

impl<'a> RomuEqMainThread<'a> {
    fn new(shared: Arc<RomuEqSharedInner>, host: HostMainThreadHandle<'a>) -> Self {
        let host_ptr = host.as_raw() as *const clap_sys::host::clap_host;
        let editor = ui_cocoa::CocoaEditor::new(Arc::clone(&shared), Some(host_ptr));
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

impl<'a> PluginMainThread<'a, RomuEqShared> for RomuEqMainThread<'a> {}

impl RomuEqParamsImplementation for RomuEqMainThread<'_> {
    fn get_value(&self, id: clap_id) -> Option<f64> {
        self.shared.get_param_value(id)
    }

    fn set_param_from_host_flush(&self, id: clap_id, val: f64) {
        let _ = self.shared.set_param_from_host(id, val);
    }

    unsafe fn flush_pending_shared(&self, out: *const clap_output_events) {
        let shared_wrapper = RomuEqShared {
            inner: Arc::clone(&self.shared),
        };
        unsafe { shared_wrapper.flush_pending_shared(out) };
    }
}

impl RomuEqGuiImplementation for RomuEqMainThread<'_> {
    fn create_ui(&mut self, is_floating: bool) -> bool {
        if is_floating {
            return false;
        }
        self.editor.create()
    }

    fn destroy_ui(&mut self) {
        self.editor.destroy();
    }

    unsafe fn set_parent(&mut self, window: *const clap_window) -> bool {
        if window.is_null() {
            return false;
        }
        unsafe {
            let api = (*window).api;
            if api.is_null() {
                return false;
            }
            #[cfg(target_os = "macos")]
            {
                if CStr::from_ptr(api) != CLAP_WINDOW_API_COCOA {
                    return false;
                }
                let parent_ptr: *mut c_void = (*window).specific.cocoa;
                self.editor.set_parent(parent_ptr)
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = api;
                false
            }
        }
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

/// Real-time audio processor owning an isolated `EngineLifecycle` and
/// pre-allocated planar scratch buffers. Zero allocations and zero locks
/// in `process()`.
pub struct RomuEqAudioProcessor {
    engine: Box<EngineLifecycle>,
    shared: Arc<RomuEqSharedInner>,
    last_param_version: u64,
    active_channels: usize,
    input_scratch: [Box<[f32]>; 2],
    output_scratch: [Box<[f32]>; 2],
}

impl Default for RomuEqAudioProcessor {
    fn default() -> Self {
        Self::new(Arc::new(RomuEqSharedInner::new()))
    }
}

impl RomuEqAudioProcessor {
    /// Creates an audio processor bound to the given shared atomic state.
    pub fn new(shared: Arc<RomuEqSharedInner>) -> Self {
        let mut engine = Box::new(EngineLifecycle::new());
        let initial_eq = shared.read_eq_parameters();
        engine.set_parameters(initial_eq);
        let version = shared.version();

        Self {
            engine,
            shared,
            last_param_version: version,
            active_channels: 2,
            input_scratch: [
                vec![0.0f32; MAX_BLOCK_SIZE].into_boxed_slice(),
                vec![0.0f32; MAX_BLOCK_SIZE].into_boxed_slice(),
            ],
            output_scratch: [
                vec![0.0f32; MAX_BLOCK_SIZE].into_boxed_slice(),
                vec![0.0f32; MAX_BLOCK_SIZE].into_boxed_slice(),
            ],
        }
    }

    /// Prepares the internal DSP engine for a sample rate, max block size, and channel count.
    pub fn prepare(&mut self, sample_rate: f64, max_block_size: usize, channels: usize) -> bool {
        let ch = channels.clamp(1, 2);
        let blk = max_block_size.clamp(1, MAX_BLOCK_SIZE);
        self.active_channels = ch;
        self.shared.set_sample_rate(sample_rate as f32);
        self.engine.prepare(sample_rate, blk, ch)
    }

    #[inline]
    fn sync_params_from_shared(&mut self) {
        let current_version = self.shared.version();
        if current_version != self.last_param_version {
            let eq = self.shared.read_eq_parameters();
            self.engine.set_parameters(eq);
            self.last_param_version = current_version;
        }
    }

    /// Processes planar audio slices using pre-allocated scratch buffers (RT-safe).
    pub fn process_planar(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) -> bool {
        self.sync_params_from_shared();

        let num_ch = inputs.len().min(outputs.len()).min(2);
        if num_ch == 0 {
            return false;
        }
        let total_frames = inputs[0].len().min(outputs[0].len());
        if total_frames == 0 {
            return true;
        }

        let target_ch = self.active_channels.clamp(1, 2);
        let mut offset = 0;
        let mut max_l = 0.0f32;
        let mut max_r = 0.0f32;

        while offset < total_frames {
            let chunk = (total_frames - offset).min(MAX_BLOCK_SIZE);

            for (ch, in_ch) in inputs.iter().enumerate().take(num_ch) {
                self.input_scratch[ch][..chunk].copy_from_slice(&in_ch[offset..offset + chunk]);
            }
            if num_ch == 1 && target_ch == 2 {
                self.input_scratch[1][..chunk].copy_from_slice(&inputs[0][offset..offset + chunk]);
            }

            if !self.run_engine_chunk(chunk) {
                return false;
            }

            for (ch, out_ch) in outputs.iter_mut().enumerate().take(num_ch) {
                out_ch[offset..offset + chunk]
                    .copy_from_slice(&self.output_scratch[ch][..chunk]);
            }

            self.push_telemetry_chunk(chunk, target_ch, &mut max_l, &mut max_r);
            offset += chunk;
        }

        self.shared.set_telemetry(
            linear_to_dbfs(max_l),
            linear_to_dbfs(max_r),
            self.engine.auto_gain_compensation_db(),
        );

        true
    }

    #[inline]
    fn run_engine_chunk(&mut self, frames: usize) -> bool {
        let target_ch = self.active_channels.clamp(1, 2);
        if target_ch == 1 {
            let in_slices: [&[f32]; 1] = [&self.input_scratch[0][..frames]];
            let mut out_slices: [&mut [f32]; 1] = {
                let [c0, _] = &mut self.output_scratch;
                [&mut c0[..frames]]
            };
            self.engine.process(&in_slices, &mut out_slices)
        } else {
            let in_slices: [&[f32]; 2] = [
                &self.input_scratch[0][..frames],
                &self.input_scratch[1][..frames],
            ];
            let mut out_slices: [&mut [f32]; 2] = {
                let [c0, c1] = &mut self.output_scratch;
                [&mut c0[..frames], &mut c1[..frames]]
            };
            self.engine.process(&in_slices, &mut out_slices)
        }
    }

    #[inline]
    fn push_telemetry_chunk(
        &self,
        frames: usize,
        target_ch: usize,
        max_l: &mut f32,
        max_r: &mut f32,
    ) {
        let right_ch = if target_ch > 1 { 1 } else { 0 };
        for i in 0..frames {
            let l = self.output_scratch[0][i];
            let r = self.output_scratch[right_ch][i];
            *max_l = max_l.max(l.abs());
            *max_r = max_r.max(r.abs());
            let _ = self.shared.ring_producer.push(l, r);
        }
    }

    fn emit_ui_events_to_host(&self, output: &mut OutputEvents) {
        for slot in 0..PARAM_COUNT_USIZE {
            if let Some(id) = self.shared.take_gesture_begin(slot) {
                if let Some(clap_id) = ClapId::from_raw(id) {
                    let ev = ParamGestureBeginEvent::new(0, clap_id);
                    if output.try_push(ev).is_err() {
                        self.shared.mark_gesture_begin(slot);
                        continue;
                    }
                }
            }

            if let Some((id, val)) = self.shared.take_dirty_slot(slot) {
                if let Some(clap_id) = ClapId::from_raw(id) {
                    let ev =
                        ParamValueEvent::new(0, clap_id, Pckn::match_all(), val, Cookie::empty());
                    if output.try_push(ev).is_err() {
                        self.shared.mark_dirty_slot(slot);
                        continue;
                    }
                }
            }

            if let Some(id) = self.shared.take_gesture_end(slot) {
                if let Some(clap_id) = ClapId::from_raw(id) {
                    let ev = ParamGestureEndEvent::new(0, clap_id);
                    if output.try_push(ev).is_err() {
                        self.shared.mark_gesture_end(slot);
                    }
                }
            }
        }
    }
}

#[inline]
fn linear_to_dbfs(peak: f32) -> f32 {
    if peak <= 1e-10 {
        f32::NEG_INFINITY
    } else {
        20.0 * peak.log10()
    }
}

impl<'a> PluginAudioProcessor<'a, RomuEqShared, RomuEqMainThread<'a>> for RomuEqAudioProcessor {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut RomuEqMainThread<'a>,
        shared: &'a RomuEqShared,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        let mut processor = Self::new(Arc::clone(shared.inner()));
        let channels = 2;
        if !processor.prepare(
            audio_config.sample_rate,
            audio_config.max_frames_count as usize,
            channels,
        ) {
            return Err(PluginError::Message("Failed to prepare RomuEq engine"));
        }
        Ok(processor)
    }

    fn process(
        &mut self,
        _process: Process,
        mut audio: Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        // 1. Push any UI-initiated parameter changes + gestures to host automation output.
        self.emit_ui_events_to_host(events.output);

        // 2. Discover channel count and total block frame count from first audio port pair.
        let mut total_frames = 0usize;
        let mut ch_count = 0usize;
        for mut port_pair in &mut audio {
            let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                continue;
            };
            for channel_pair in channel_pairs {
                if ch_count >= 2 {
                    break;
                }
                let ch_len = match channel_pair {
                    ChannelPair::InPlace(buf) => buf.len(),
                    ChannelPair::InputOutput(input, output) => input.len().min(output.len()),
                    ChannelPair::InputOnly(buf) => buf.len(),
                    ChannelPair::OutputOnly(buf) => buf.len(),
                };
                total_frames = if ch_count == 0 {
                    ch_len
                } else {
                    total_frames.min(ch_len)
                };
                ch_count += 1;
            }
            if ch_count > 0 {
                break;
            }
        }

        let input_len = events.input.len();
        let mut ev_idx = 0u32;

        if ch_count == 0 || total_frames == 0 {
            while ev_idx < input_len {
                if let Some(ev) = events.input.get(ev_idx) {
                    if let Some(CoreEventSpace::ParamValue(pv)) = ev.as_core_event() {
                        if let Some(id) = pv.param_id() {
                            self.shared.set_param_from_host(id.get(), pv.value());
                        }
                    }
                }
                ev_idx += 1;
            }
            self.sync_params_from_shared();
            return Ok(ProcessStatus::Continue);
        }

        // 3. Process full block in sub-slices split on CLAP input event timestamps
        //    and bounded by MAX_BLOCK_SIZE (handles arbitrarily large buffers + sample-accurate automation).
        let target_ch = self.active_channels.clamp(1, 2);
        let mut offset = 0usize;
        let mut max_l = 0.0f32;
        let mut max_r = 0.0f32;

        while offset < total_frames {
            // Apply all input events due at or before `offset`.
            while ev_idx < input_len {
                let Some(ev) = events.input.get(ev_idx) else {
                    break;
                };
                if (ev.header().time() as usize) > offset {
                    break;
                }
                if let Some(CoreEventSpace::ParamValue(pv)) = ev.as_core_event() {
                    if let Some(id) = pv.param_id() {
                        self.shared.set_param_from_host(id.get(), pv.value());
                    }
                }
                ev_idx += 1;
            }
            self.sync_params_from_shared();

            // Determine next slice boundary: either next event timestamp or MAX_BLOCK_SIZE chunk end.
            let mut next_stop = (offset + MAX_BLOCK_SIZE).min(total_frames);
            if ev_idx < input_len {
                if let Some(ev) = events.input.get(ev_idx) {
                    let ev_time = ev.header().time() as usize;
                    if ev_time > offset && ev_time < next_stop {
                        next_stop = ev_time;
                    }
                }
            }

            let slice_len = next_stop - offset;
            if slice_len == 0 {
                break;
            }

            // Copy inputs for [offset..next_stop] into pre-allocated scratch buffers.
            let mut copy_ch = 0usize;
            for mut port_pair in &mut audio {
                let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                    continue;
                };
                for channel_pair in channel_pairs {
                    if copy_ch >= ch_count {
                        break;
                    }
                    match channel_pair {
                        ChannelPair::InPlace(buf) => {
                            self.input_scratch[copy_ch][..slice_len]
                                .copy_from_slice(&buf[offset..next_stop]);
                        }
                        ChannelPair::InputOutput(input, _) => {
                            self.input_scratch[copy_ch][..slice_len]
                                .copy_from_slice(&input[offset..next_stop]);
                        }
                        ChannelPair::InputOnly(buf) => {
                            self.input_scratch[copy_ch][..slice_len]
                                .copy_from_slice(&buf[offset..next_stop]);
                        }
                        ChannelPair::OutputOnly(_) => {
                            self.input_scratch[copy_ch][..slice_len].fill(0.0);
                        }
                    }
                    copy_ch += 1;
                }
                if copy_ch > 0 {
                    break;
                }
            }

            if ch_count == 1 && target_ch == 2 {
                let (first, rest) = self.input_scratch.split_at_mut(1);
                rest[0][..slice_len].copy_from_slice(&first[0][..slice_len]);
            }

            if !self.run_engine_chunk(slice_len) {
                return Ok(ProcessStatus::Continue);
            }

            // Copy outputs back for [offset..next_stop].
            let mut write_ch = 0usize;
            for mut port_pair in &mut audio {
                let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                    continue;
                };
                for channel_pair in channel_pairs {
                    if write_ch >= ch_count {
                        break;
                    }
                    let out_slice = &self.output_scratch[write_ch][..slice_len];
                    match channel_pair {
                        ChannelPair::InPlace(buf) => {
                            buf[offset..next_stop].copy_from_slice(out_slice);
                        }
                        ChannelPair::InputOutput(_, output) => {
                            output[offset..next_stop].copy_from_slice(out_slice);
                        }
                        ChannelPair::InputOnly(_) => {}
                        ChannelPair::OutputOnly(buf) => {
                            buf[offset..next_stop].copy_from_slice(out_slice);
                        }
                    }
                    write_ch += 1;
                }
                if write_ch > 0 {
                    break;
                }
            }

            self.push_telemetry_chunk(slice_len, target_ch, &mut max_l, &mut max_r);
            offset = next_stop;
        }

        // Apply any trailing events stamped at >= total_frames.
        while ev_idx < input_len {
            if let Some(ev) = events.input.get(ev_idx) {
                if let Some(CoreEventSpace::ParamValue(pv)) = ev.as_core_event() {
                    if let Some(id) = pv.param_id() {
                        self.shared.set_param_from_host(id.get(), pv.value());
                    }
                }
            }
            ev_idx += 1;
        }
        self.sync_params_from_shared();

        self.shared.set_telemetry(
            linear_to_dbfs(max_l),
            linear_to_dbfs(max_r),
            self.engine.auto_gain_compensation_db(),
        );

        Ok(ProcessStatus::Continue)
    }

    fn deactivate(self, _main_thread: &mut RomuEqMainThread<'a>) {
        let mut engine = self.engine;
        engine.destroy();
    }

    fn start_processing(&mut self) -> Result<(), PluginError> {
        self.engine.reset();
        Ok(())
    }

    fn stop_processing(&mut self) {}
}

// ---------------------------------------------------------------------------
// Plugin entry & extensions registration
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct RomuEqPlugin;

impl Plugin for RomuEqPlugin {
    type Shared<'a> = RomuEqShared;
    type MainThread<'a> = RomuEqMainThread<'a>;
    type AudioProcessor<'a> = RomuEqAudioProcessor;

    fn declare_extensions(
        builder: &mut PluginExtensions<Self>,
        _shared: Option<&Self::Shared<'_>>,
    ) {
        builder
            .register::<RomuEqState>()
            .register::<RomuEqAudioPorts>()
            .register::<RomuEqParams>()
            .register::<RomuEqGui>();
    }
}

impl DefaultPluginFactory for RomuEqPlugin {
    fn get_descriptor() -> PluginDescriptor {
        get_descriptor()
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(RomuEqShared::default())
    }

    fn new_main_thread<'a>(
        host: HostMainThreadHandle<'a>,
        shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(RomuEqMainThread::new(Arc::clone(shared.inner()), host))
    }
}

// ---------------------------------------------------------------------------
// CLAP State extension (versioned envelope per ADR-006)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub struct RomuEqState(RawExtension<PluginExtensionSide, clap_plugin_state>);

unsafe impl Extension for RomuEqState {
    const IDENTIFIERS: &[&CStr] = &[CLAP_EXT_STATE];
    type ExtensionSide = PluginExtensionSide;

    #[inline]
    unsafe fn from_raw(raw: RawExtension<Self::ExtensionSide>) -> Self {
        Self(unsafe { raw.cast() })
    }
}

pub trait RomuEqStateImplementation {
    fn save(&mut self, save: &mut OutputStream) -> Result<(), PluginError>;
    fn load(&mut self, load: &mut InputStream) -> Result<(), PluginError>;
}

unsafe impl<P: Plugin> ExtensionImplementation<P> for RomuEqState
where
    for<'a> P::MainThread<'a>: RomuEqStateImplementation,
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
    for<'a> P::MainThread<'a>: RomuEqStateImplementation,
{
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
    for<'a> P::MainThread<'a>: RomuEqStateImplementation,
{
    unsafe {
        PluginWrapper::<P>::handle(plugin, |p| {
            let input = InputStream::from_raw_mut(&mut *(istream as *mut _));
            p.main_thread().as_mut().load(input)?;
            Ok(())
        })
        .is_some()
    }
}

impl RomuEqStateImplementation for RomuEqMainThread<'_> {
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
                return Err(PluginError::Message("Unsupported future state schema version"));
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

clack_export_entry!(SinglePluginEntry::<RomuEqPlugin>);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use params_map::{PARAM_COUNT, get_param_meta};
    use romueq_common::{FilterType, Slope};

    #[test]
    fn test_all_51_params_round_trip_mapping() {
        assert_eq!(PARAM_COUNT, 51);
        let shared = RomuEqSharedInner::new();

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
        let shared = RomuEqSharedInner::new();
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
            let shared = Arc::new(RomuEqSharedInner::new());
            shared.set_param_from_host(503, 6.0);
            shared.set_param_from_host(501, FilterType::Bell.to_raw() as f64);
            shared.set_param_from_host(505, Slope::Db12.to_raw() as f64);

            let mut proc = RomuEqAudioProcessor::new(Arc::clone(&shared));
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
        let shared = Arc::new(RomuEqSharedInner::new());
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
