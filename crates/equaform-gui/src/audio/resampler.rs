//! Fractional-phase stereo linear interpolation and PI clock-drift compensation.

use super::MAX_SCRATCH_FRAMES;
use equaform_common::audio_ring::AudioRingConsumer;

/// Parameters for [`DriftCompensatingReader::interpolate_block`].
#[derive(Clone, Copy)]
struct InterpolateBlockParams {
    count: usize,
    step: f64,
    scratch_idx: usize,
    popped: usize,
}

/// Fractional-phase stereo linear interpolation state for [`ContinuousPiDriftController`].
#[derive(Clone, Copy, Debug, Default)]
struct ResamplerState {
    phase: f64,
    prev_l: f32,
    prev_r: f32,
    curr_l: f32,
    curr_r: f32,
    has_samples: bool,
}

impl ResamplerState {
    fn reset(&mut self) {
        self.phase = 0.0;
        self.prev_l = 0.0;
        self.prev_r = 0.0;
        self.curr_l = 0.0;
        self.curr_r = 0.0;
        self.has_samples = false;
    }
}

/// Inline buffer holding unconsumed stereo frames across block boundaries.
struct LeftoverFrames {
    left: [f32; 16],
    right: [f32; 16],
    len: usize,
}

impl Default for LeftoverFrames {
    fn default() -> Self {
        Self {
            left: [0.0f32; 16],
            right: [0.0f32; 16],
            len: 0,
        }
    }
}

impl LeftoverFrames {
    fn clear(&mut self) {
        self.len = 0;
    }
}

/// Transparent lock-free clock drift controller.
///
/// Dynamically compensates for crystal clock frequency differences between independent
/// input (e.g. Equaform-8) and output (e.g. Komplete Audio 6 MK2) hardware devices
/// without audible pitch modulation, buffer hunting, or chopping.
pub struct ContinuousPiDriftController {
    scratch_l: Vec<f32>,
    scratch_r: Vec<f32>,
    leftover: LeftoverFrames,
    resampler: ResamplerState,
    smoothed_available: f64,
    is_primed: bool,
    rate_ratio: f64,
    pub underrun_count: usize,
    pub unprimed_cycles: usize,
}

impl Default for ContinuousPiDriftController {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl ContinuousPiDriftController {
    /// Safe pre-buffering headroom for external USB audio interfaces (~46 ms at 44.1 kHz).
    pub const TARGET_FRAMES: usize = 2048;

    pub fn new(rate_ratio: f64) -> Self {
        Self {
            scratch_l: vec![0.0f32; MAX_SCRATCH_FRAMES],
            scratch_r: vec![0.0f32; MAX_SCRATCH_FRAMES],
            leftover: LeftoverFrames::default(),
            resampler: ResamplerState::default(),
            smoothed_available: Self::TARGET_FRAMES as f64,
            is_primed: false,
            rate_ratio: if rate_ratio > 0.0 { rate_ratio } else { 1.0 },
            underrun_count: 0,
            unprimed_cycles: 0,
        }
    }

    pub fn set_rate_ratio(&mut self, ratio: f64) {
        if ratio > 0.0 {
            self.rate_ratio = ratio;
        }
    }

    pub fn reset(&mut self) {
        self.is_primed = false;
        self.resampler.reset();
        self.leftover.clear();
        self.smoothed_available = Self::TARGET_FRAMES as f64;
    }

    pub fn is_primed(&self) -> bool {
        self.is_primed
    }

    /// Read `count` frames from `consumer` into `left` and `right` destination slices,
    /// applying transparent lock-free transfer, sample-rate adaptation, and click-free drift compensation.
    pub fn read_from(
        &mut self,
        consumer: &AudioRingConsumer,
        left: &mut [f32],
        right: &mut [f32],
        count: usize,
    ) {
        let available = consumer.available_read();
        if available > 12000 {
            let _ = consumer.discard(available.saturating_sub(Self::TARGET_FRAMES));
        }
        if !self.ensure_primed_and_available(available, left, right, count) {
            return;
        }

        self.smoothed_available = 0.98 * self.smoothed_available + 0.02 * (available as f64);
        let error = self.smoothed_available - (Self::TARGET_FRAMES as f64);
        let in_deadband = error.abs() < 512.0;
        let is_1to1 = (self.rate_ratio - 1.0).abs() < 1e-6;

        if is_1to1
            && in_deadband
            && self.leftover.len == 0
            && available >= count
            && self.try_fast_path_pop(consumer, left, right, count)
        {
            return;
        }

        let step = if in_deadband && is_1to1 {
            1.0
        } else {
            let drift_ppm = ((error / (Self::TARGET_FRAMES as f64)) * 0.001).clamp(-0.003, 0.003);
            self.rate_ratio * (1.0 + drift_ppm)
        };

        let popped = self.populate_scratch(consumer, count, step);
        let Some(scratch_idx) = self.seed_resampler(popped) else {
            left[..count].fill(0.0);
            right[..count].fill(0.0);
            return;
        };
        let consumed = self.interpolate_block(
            left,
            right,
            InterpolateBlockParams {
                count,
                step,
                scratch_idx,
                popped,
            },
        );
        self.save_leftover(consumed, popped);
    }

    fn ensure_primed_and_available(
        &mut self,
        available: usize,
        left: &mut [f32],
        right: &mut [f32],
        count: usize,
    ) -> bool {
        if !self.is_primed {
            self.unprimed_cycles += 1;
            if available >= Self::TARGET_FRAMES {
                self.is_primed = true;
                self.smoothed_available = available as f64;
            } else {
                left[..count].fill(0.0);
                right[..count].fill(0.0);
                return false;
            }
        }
        if available == 0 && self.leftover.len == 0 {
            self.underrun_count += 1;
            let mut l = self.resampler.curr_l;
            let mut r = self.resampler.curr_r;
            for (dl, dr) in left[..count].iter_mut().zip(right[..count].iter_mut()) {
                l *= 0.985;
                r *= 0.985;
                *dl = l;
                *dr = r;
            }
            self.resampler.prev_l = l;
            self.resampler.prev_r = r;
            self.resampler.curr_l = l;
            self.resampler.curr_r = r;
            self.resampler.has_samples = true;
            self.resampler.phase = 0.0;
            return false;
        }
        true
    }

    fn try_fast_path_pop(
        &mut self,
        consumer: &AudioRingConsumer,
        left: &mut [f32],
        right: &mut [f32],
        count: usize,
    ) -> bool {
        if consumer.pop_slice(&mut left[..count], &mut right[..count]) != count {
            return false;
        }
        self.resampler.prev_l = left[count - 1];
        self.resampler.prev_r = right[count - 1];
        self.resampler.curr_l = self.resampler.prev_l;
        self.resampler.curr_r = self.resampler.prev_r;
        self.resampler.has_samples = true;
        self.resampler.phase = 0.0;
        true
    }

    fn populate_scratch(&mut self, consumer: &AudioRingConsumer, count: usize, step: f64) -> usize {
        let initial_need = if self.resampler.has_samples { 0 } else { 2 };
        let extra_margin = usize::from((step - 1.0).abs() > 1e-12 || self.resampler.phase > 0.0);
        let step_advances =
            (self.resampler.phase + (count as f64) * step).floor() as usize + extra_margin;
        let total_needed = (initial_need + step_advances).min(MAX_SCRATCH_FRAMES);

        for k in 0..self.leftover.len {
            self.scratch_l[k] = self.leftover.left[k];
            self.scratch_r[k] = self.leftover.right[k];
        }
        let from_consumer = total_needed.saturating_sub(self.leftover.len);
        let newly_popped = if from_consumer > 0 {
            consumer.pop_slice(
                &mut self.scratch_l[self.leftover.len..self.leftover.len + from_consumer],
                &mut self.scratch_r[self.leftover.len..self.leftover.len + from_consumer],
            )
        } else {
            0
        };
        let popped = self.leftover.len + newly_popped;
        self.leftover.clear();
        popped
    }

    fn seed_resampler(&mut self, popped: usize) -> Option<usize> {
        if self.resampler.has_samples {
            return Some(0);
        }
        if popped >= 2 {
            self.resampler.prev_l = self.scratch_l[0];
            self.resampler.prev_r = self.scratch_r[0];
            self.resampler.curr_l = self.scratch_l[1];
            self.resampler.curr_r = self.scratch_r[1];
            self.resampler.has_samples = true;
            Some(2)
        } else if popped == 1 {
            self.resampler.prev_l = self.scratch_l[0];
            self.resampler.prev_r = self.scratch_r[0];
            self.resampler.curr_l = self.resampler.prev_l;
            self.resampler.curr_r = self.resampler.prev_r;
            self.resampler.has_samples = true;
            Some(1)
        } else {
            None
        }
    }

    fn interpolate_block(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        params: InterpolateBlockParams,
    ) -> usize {
        let mut scratch_idx = params.scratch_idx;
        for i in 0..params.count {
            let frac = self.resampler.phase as f32;
            left[i] =
                self.resampler.prev_l + frac * (self.resampler.curr_l - self.resampler.prev_l);
            right[i] =
                self.resampler.prev_r + frac * (self.resampler.curr_r - self.resampler.prev_r);
            self.resampler.phase += params.step;
            while self.resampler.phase >= 1.0 {
                self.resampler.phase -= 1.0;
                self.resampler.prev_l = self.resampler.curr_l;
                self.resampler.prev_r = self.resampler.curr_r;
                if scratch_idx < params.popped {
                    self.resampler.curr_l = self.scratch_l[scratch_idx];
                    self.resampler.curr_r = self.scratch_r[scratch_idx];
                    scratch_idx += 1;
                } else {
                    self.underrun_count += 1;
                    self.resampler.curr_l *= 0.98;
                    self.resampler.curr_r *= 0.98;
                }
            }
        }
        scratch_idx
    }

    fn save_leftover(&mut self, scratch_idx: usize, popped: usize) {
        if scratch_idx < popped {
            let rem = (popped - scratch_idx).min(16);
            for k in 0..rem {
                self.leftover.left[k] = self.scratch_l[scratch_idx + k];
                self.leftover.right[k] = self.scratch_r[scratch_idx + k];
            }
            self.leftover.len = rem;
        }
    }
}
