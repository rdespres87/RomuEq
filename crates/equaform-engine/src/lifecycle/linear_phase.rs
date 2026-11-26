//! Linear-phase bypass dry delay line and click-free mode transition state.

use super::{FLAT_BUFFER_SIZE, LIFECYCLE_MAX_CHANNELS};
use equaform_common::ProcessingMode;

/// Capacity of the pre-allocated per-channel dry delay ring buffer in samples.
///
/// Matches the maximum linear-phase algorithmic latency (`2048` samples in
/// [`ProcessingMode::LinearPhaseHigh`]).
pub(super) const DRY_DELAY_CAPACITY: usize = 2048;
const DRY_DELAY_MASK: usize = DRY_DELAY_CAPACITY - 1;

const _: () = assert!(DRY_DELAY_CAPACITY.is_power_of_two());

/// Pre-allocated per-channel circular delay line for latency-aligned dry bypass.
///
/// Heap-allocated once at construction with zero allocations during `reset()` or
/// audio processing.
pub(super) struct DryDelayLine {
    buffer: Box<[f32]>,
    write_pos: [usize; LIFECYCLE_MAX_CHANNELS],
}

impl DryDelayLine {
    /// Allocates the per-channel delay storage once.
    pub(super) fn new() -> Self {
        Self {
            buffer: vec![0.0_f32; DRY_DELAY_CAPACITY * LIFECYCLE_MAX_CHANNELS].into_boxed_slice(),
            write_pos: [0; LIFECYCLE_MAX_CHANNELS],
        }
    }

    /// Zeroes all delayed samples and resets write cursors without allocating.
    pub(super) fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos.fill(0);
    }

    /// Pushes `input` into channel `ch`'s ring buffer and writes `delay_samples`-delayed
    /// audio into `output`.
    ///
    /// Safe for arbitrary block sizes (including `input.len() > DRY_DELAY_CAPACITY`)
    /// because each sample is read before its slot is overwritten.
    pub(super) fn process_channel(
        &mut self,
        ch: usize,
        input: &[f32],
        output: &mut [f32],
        delay_samples: usize,
    ) {
        if ch >= LIFECYCLE_MAX_CHANNELS {
            return;
        }

        let count = input.len().min(output.len());
        let delay = delay_samples.min(DRY_DELAY_CAPACITY);
        if delay == 0 {
            output[..count].copy_from_slice(&input[..count]);
            return;
        }

        let base = ch * DRY_DELAY_CAPACITY;
        let ring = &mut self.buffer[base..base + DRY_DELAY_CAPACITY];
        let mut wp = self.write_pos[ch];

        for (&src, dst) in input[..count].iter().zip(output[..count].iter_mut()) {
            let rp = (wp + DRY_DELAY_CAPACITY - delay) & DRY_DELAY_MASK;
            *dst = ring[rp];
            ring[wp] = src;
            wp = (wp + 1) & DRY_DELAY_MASK;
        }

        self.write_pos[ch] = wp;
    }
}

/// Click-free crossfade state and planar scratch buffer for live `ProcessingMode` switching.
///
/// Tracks `mix` in `[0.0, 1.0]` where `0.0` is steady-state `active_mode` and `1.0`
/// is 100% outgoing `previous_mode`, ramping smoothly down to `0.0` over `CROSSFADE_MS`.
pub(super) struct ModeTransitionState {
    pub(super) active_mode: ProcessingMode,
    pub(super) previous_mode: ProcessingMode,
    pub(super) mix: f32,
    pub(super) input_fade_gain: f32,
    pub(super) use_tail_fade: bool,
    pub(super) prev_output: [f32; LIFECYCLE_MAX_CHANNELS],
    pub(super) last_rendered: [f32; LIFECYCLE_MAX_CHANNELS],
    pub(super) scratch_buffer: Box<[f32]>,
}

impl ModeTransitionState {
    /// Allocates the planar scratch buffer once for outgoing-mode rendering.
    pub(super) fn new() -> Self {
        Self {
            active_mode: ProcessingMode::MinimumPhase,
            previous_mode: ProcessingMode::MinimumPhase,
            mix: 0.0,
            input_fade_gain: 1.0,
            use_tail_fade: false,
            prev_output: [0.0; LIFECYCLE_MAX_CHANNELS],
            last_rendered: [0.0; LIFECYCLE_MAX_CHANNELS],
            scratch_buffer: vec![0.0_f32; FLAT_BUFFER_SIZE].into_boxed_slice(),
        }
    }

    /// Resets transition state to `mode` with no active crossfade.
    pub(super) fn reset(&mut self, mode: ProcessingMode) {
        self.active_mode = mode;
        self.previous_mode = mode;
        self.mix = 0.0;
        self.input_fade_gain = 1.0;
        self.use_tail_fade = false;
        self.prev_output.fill(0.0);
        self.last_rendered.fill(0.0);
        self.scratch_buffer.fill(0.0);
    }

    /// Evaluates `target_mode` and initiates a crossfade when switching modes during playback.
    ///
    /// On the first block (`is_first_block == true`), snaps immediately without ramping.
    /// Returns `true` when a runtime mode switch was initiated.
    pub(super) fn update(
        &mut self,
        target_mode: ProcessingMode,
        is_first_block: bool,
        force_tail_fade: bool,
    ) -> bool {
        if is_first_block {
            self.active_mode = target_mode;
            self.previous_mode = target_mode;
            self.mix = 0.0;
            self.input_fade_gain = 1.0;
            self.use_tail_fade = false;
            self.prev_output.fill(0.0);
            self.last_rendered.fill(0.0);
            return false;
        }

        if target_mode != self.active_mode {
            let already_ramping = self.mix > f32::EPSILON;
            let both_linear = self.active_mode.is_linear_phase() && target_mode.is_linear_phase();
            self.use_tail_fade = force_tail_fade || already_ramping || both_linear;
            self.prev_output = self.last_rendered;
            self.previous_mode = self.active_mode;
            self.active_mode = target_mode;
            self.mix = 1.0;
            self.input_fade_gain = if target_mode.is_linear_phase() {
                0.0
            } else {
                1.0
            };
            return true;
        }

        false
    }

    /// Applies a smooth `0.0 -> 1.0` fade-in envelope to freshly reset linear-phase input
    /// after a runtime mode switch so the pipeline delay emerges click-free.
    pub(super) fn apply_input_fade_in(
        &mut self,
        buffer: &mut [f32],
        max_block: usize,
        channels: usize,
        block_size: usize,
        step: f32,
    ) {
        if self.input_fade_gain >= 1.0 - f32::EPSILON {
            return;
        }

        let start_gain = self.input_fade_gain;
        let mut end_gain = start_gain;
        for ch in 0..channels {
            let offset = ch * max_block;
            let mut gain = start_gain;
            for sample in &mut buffer[offset..offset + block_size] {
                *sample *= gain;
                gain = (gain + step).min(1.0);
            }
            end_gain = gain;
        }
        self.input_fade_gain = if end_gain >= 1.0 - f32::EPSILON {
            1.0
        } else {
            end_gain
        };
    }

    /// Returns `true` while a mode crossfade ramp is in progress.
    #[inline]
    pub(super) fn is_ramping(&self) -> bool {
        self.mix > f32::EPSILON
    }

    /// Advances the mode crossfade toward `0.0` and returns `true` when finished.
    #[inline]
    pub(super) fn advance(&mut self, step: f32, block_size: usize) -> bool {
        self.mix = (self.mix - step * block_size as f32).max(0.0);
        if self.mix <= f32::EPSILON {
            self.mix = 0.0;
            self.use_tail_fade = false;
            return true;
        }
        false
    }

    /// Captures the final output sample of each active channel for smooth tail fades.
    pub(super) fn record_last_rendered(
        &mut self,
        buffer: &[f32],
        max_block: usize,
        channels: usize,
        block_size: usize,
    ) {
        if block_size == 0 {
            return;
        }
        let last_idx = block_size - 1;
        let ch_count = channels.min(LIFECYCLE_MAX_CHANNELS);
        for (ch, slot) in self.last_rendered[..ch_count].iter_mut().enumerate() {
            if let Some(&sample) = buffer.get(ch * max_block + last_idx) {
                *slot = sample;
            }
        }
    }
}
