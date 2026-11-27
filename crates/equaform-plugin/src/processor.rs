//! Real-time CLAP audio processor for Equaform.
//!
//! Owns an isolated `EngineLifecycle` and pre-allocated planar scratch buffers
//! with zero heap allocations and zero locks in `process()`.

use std::sync::Arc;

use clack_common::events::Pckn;
use clack_common::events::event_types::{
    ParamGestureBeginEvent, ParamGestureEndEvent, ParamValueEvent,
};
use clack_common::events::spaces::CoreEventSpace;
use clack_common::utils::{ClapId, Cookie};
use clack_plugin::prelude::*;
use equaform_common::MAX_BLOCK_SIZE;
use equaform_common::parameter::global;
use equaform_engine::lifecycle::EngineLifecycle;

use crate::params_map::PARAM_COUNT_USIZE;
use crate::{EquaformMainThread, EquaformShared, EquaformSharedInner};

/// Real-time audio processor owning an isolated `EngineLifecycle` and
/// pre-allocated planar scratch buffers. Zero allocations and zero locks
/// in `process()`.
pub struct EquaformAudioProcessor<'a> {
    engine: Box<EngineLifecycle>,
    shared: Arc<EquaformSharedInner>,
    host: Option<HostAudioProcessorHandle<'a>>,
    last_param_version: u64,
    active_channels: usize,
    input_scratch: [Box<[f32]>; 2],
    output_scratch: [Box<[f32]>; 2],
}

impl Default for EquaformAudioProcessor<'_> {
    fn default() -> Self {
        Self::new(Arc::new(EquaformSharedInner::new()))
    }
}

impl<'a> EquaformAudioProcessor<'a> {
    /// Creates an audio processor bound to the given shared atomic state.
    pub fn new(shared: Arc<EquaformSharedInner>) -> Self {
        let mut engine = Box::new(EngineLifecycle::new());
        let initial_eq = shared.read_eq_parameters();
        engine.set_parameters(initial_eq);
        let version = shared.version();

        Self {
            engine,
            shared,
            host: None,
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

    #[inline]
    fn apply_host_param_event(&self, id: u32, value: f64, latency_dirty: &mut bool) {
        if self.shared.set_param_from_host(id, value)
            && id == global::PROCESSING_MODE
            && self.shared.is_latency_changed_pending()
        {
            *latency_dirty = true;
        }
    }

    #[inline]
    fn notify_latency_callback_if_needed(&self, latency_dirty: bool) {
        if latency_dirty {
            if let Some(host) = &self.host {
                host.request_callback();
            }
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
                out_ch[offset..offset + chunk].copy_from_slice(&self.output_scratch[ch][..chunk]);
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

    fn discover_block_geometry(audio: &mut Audio) -> Result<(usize, usize), PluginError> {
        let mut total_frames = 0usize;
        let mut ch_count = 0usize;
        for mut port_pair in audio {
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
        Ok((ch_count, total_frames))
    }

    fn apply_events_until(
        &mut self,
        input: &InputEvents,
        ev_idx: &mut u32,
        max_time: Option<usize>,
        latency_dirty: &mut bool,
    ) {
        let input_len = input.len();
        while *ev_idx < input_len {
            let Some(ev) = input.get(*ev_idx) else {
                break;
            };
            if let Some(max_t) = max_time {
                if (ev.header().time() as usize) > max_t {
                    break;
                }
            }
            if let Some(CoreEventSpace::ParamValue(pv)) = ev.as_core_event() {
                if let Some(id) = pv.param_id() {
                    self.apply_host_param_event(id.get(), pv.value(), latency_dirty);
                }
            }
            *ev_idx += 1;
        }
    }

    fn next_slice_end(
        input: &InputEvents,
        ev_idx: u32,
        offset: usize,
        total_frames: usize,
    ) -> usize {
        let mut next_stop = (offset + MAX_BLOCK_SIZE).min(total_frames);
        if ev_idx < input.len() {
            if let Some(ev) = input.get(ev_idx) {
                let ev_time = ev.header().time() as usize;
                if ev_time > offset && ev_time < next_stop {
                    next_stop = ev_time;
                }
            }
        }
        next_stop
    }

    fn copy_inputs_to_scratch(
        &mut self,
        audio: &mut Audio,
        ch_count: usize,
        offset: usize,
        next_stop: usize,
    ) -> Result<(), PluginError> {
        let slice_len = next_stop - offset;
        let mut copy_ch = 0usize;
        for mut port_pair in audio {
            let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                continue;
            };
            for channel_pair in channel_pairs {
                if copy_ch >= ch_count {
                    break;
                }
                match channel_pair {
                    ChannelPair::InPlace(buf) => self.input_scratch[copy_ch][..slice_len]
                        .copy_from_slice(&buf[offset..next_stop]),
                    ChannelPair::InputOutput(input, _) => self.input_scratch[copy_ch][..slice_len]
                        .copy_from_slice(&input[offset..next_stop]),
                    ChannelPair::InputOnly(buf) => self.input_scratch[copy_ch][..slice_len]
                        .copy_from_slice(&buf[offset..next_stop]),
                    ChannelPair::OutputOnly(_) => {
                        self.input_scratch[copy_ch][..slice_len].fill(0.0)
                    }
                }
                copy_ch += 1;
            }
            if copy_ch > 0 {
                break;
            }
        }
        if ch_count == 1 && self.active_channels.clamp(1, 2) == 2 {
            let (first, rest) = self.input_scratch.split_at_mut(1);
            rest[0][..slice_len].copy_from_slice(&first[0][..slice_len]);
        }
        Ok(())
    }

    fn copy_outputs_from_scratch(
        &self,
        audio: &mut Audio,
        ch_count: usize,
        offset: usize,
        next_stop: usize,
    ) -> Result<(), PluginError> {
        let slice_len = next_stop - offset;
        let mut write_ch = 0usize;
        for mut port_pair in audio {
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
        Ok(())
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

impl<'a> PluginAudioProcessor<'a, EquaformShared, EquaformMainThread<'a>>
    for EquaformAudioProcessor<'a>
{
    fn activate(
        host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut EquaformMainThread<'a>,
        shared: &'a EquaformShared,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        let mut processor = Self::new(Arc::clone(shared.inner()));
        processor.host = Some(host);
        let channels = 2;
        if !processor.prepare(
            audio_config.sample_rate,
            audio_config.max_frames_count as usize,
            channels,
        ) {
            return Err(PluginError::Message("Failed to prepare Equaform engine"));
        }
        Ok(processor)
    }

    fn process(
        &mut self,
        _process: Process,
        mut audio: Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        self.emit_ui_events_to_host(events.output);
        let (ch_count, total_frames) = Self::discover_block_geometry(&mut audio)?;
        let mut ev_idx = 0u32;
        let mut latency_dirty = false;

        if ch_count == 0 || total_frames == 0 {
            self.apply_events_until(events.input, &mut ev_idx, None, &mut latency_dirty);
            self.sync_params_from_shared();
            self.notify_latency_callback_if_needed(latency_dirty);
            return Ok(ProcessStatus::Continue);
        }

        let target_ch = self.active_channels.clamp(1, 2);
        let mut offset = 0usize;
        let mut max_l = 0.0f32;
        let mut max_r = 0.0f32;

        while offset < total_frames {
            self.apply_events_until(events.input, &mut ev_idx, Some(offset), &mut latency_dirty);
            self.sync_params_from_shared();
            let next_stop = Self::next_slice_end(events.input, ev_idx, offset, total_frames);
            let slice_len = next_stop - offset;
            if slice_len == 0 {
                break;
            }
            self.copy_inputs_to_scratch(&mut audio, ch_count, offset, next_stop)?;
            if !self.run_engine_chunk(slice_len) {
                self.notify_latency_callback_if_needed(latency_dirty);
                return Ok(ProcessStatus::Continue);
            }
            self.copy_outputs_from_scratch(&mut audio, ch_count, offset, next_stop)?;
            self.push_telemetry_chunk(slice_len, target_ch, &mut max_l, &mut max_r);
            offset = next_stop;
        }

        self.apply_events_until(events.input, &mut ev_idx, None, &mut latency_dirty);
        self.sync_params_from_shared();
        self.notify_latency_callback_if_needed(latency_dirty);
        self.shared.set_telemetry(
            linear_to_dbfs(max_l),
            linear_to_dbfs(max_r),
            self.engine.auto_gain_compensation_db(),
        );
        Ok(ProcessStatus::Continue)
    }

    fn deactivate(self, _main_thread: &mut EquaformMainThread<'a>) {
        let mut engine = self.engine;
        engine.destroy();
    }

    fn start_processing(&mut self) -> Result<(), PluginError> {
        self.engine.reset();
        Ok(())
    }

    fn stop_processing(&mut self) {}
}
