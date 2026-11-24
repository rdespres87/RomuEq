//! Fractional-phase stereo linear interpolation and PI clock-drift compensation.

use super::MAX_SCRATCH_FRAMES;
use equaform_common::audio_ring::AudioRingConsumer;

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

        // Severe backlog (> 12000 frames, ~270 ms): discard excess down to TARGET_FRAMES
        if available > 12000 {
            let excess = available.saturating_sub(Self::TARGET_FRAMES);
            let _ = consumer.discard(excess);
        }

        // 1. Initial pre-buffering: wait until buffer reaches TARGET_FRAMES
        if !self.is_primed {
            self.unprimed_cycles += 1;
            if available >= Self::TARGET_FRAMES {
                self.is_primed = true;
                self.smoothed_available = available as f64;
            } else {
                left[..count].fill(0.0);
                right[..count].fill(0.0);
                return;
            }
        }

        // 2. Buffer starvation protection:
        // If available frames are 0 or drop below a full block `count`, deliver silence for this cycle
        // without consuming partial blocks, avoiding audible chopping and exponential decay stutter.
        if available == 0 {
            self.underrun_count += 1;
            self.resampler.has_samples = false;
            self.resampler.phase = 0.0;
            self.leftover.clear();
            left[..count].fill(0.0);
            right[..count].fill(0.0);
            return;
        }
        if available < count && self.leftover.len == 0 {
            self.underrun_count += 1;
            self.resampler.has_samples = false;
            self.resampler.phase = 0.0;
            left[..count].fill(0.0);
            right[..count].fill(0.0);
            return;
        }

        // 3. Smooth EMA tracking of buffer level
        self.smoothed_available = 0.98 * self.smoothed_available + 0.02 * (available as f64);

        // 4. Smooth PI / proportional step calculation with generous ±512 frames deadband
        let error = self.smoothed_available - (Self::TARGET_FRAMES as f64);
        let in_deadband = error.abs() < 512.0;
        let is_1to1 = (self.rate_ratio - 1.0).abs() < 1e-6;

        // Bit-exact fast-path: direct slice transfer without fractional resampling interpolation
        if is_1to1 && in_deadband && self.leftover.len == 0 && available >= count {
            let popped = consumer.pop_slice(&mut left[..count], &mut right[..count]);
            if popped == count {
                self.resampler.prev_l = left[count - 1];
                self.resampler.prev_r = right[count - 1];
                self.resampler.curr_l = self.resampler.prev_l;
                self.resampler.curr_r = self.resampler.prev_r;
                self.resampler.has_samples = true;
                self.resampler.phase = 0.0;
                return;
            }
        }

        let step = if in_deadband && is_1to1 {
            1.0
        } else {
            let drift_ppm = ((error / (Self::TARGET_FRAMES as f64)) * 0.001).clamp(-0.003, 0.003);
            self.rate_ratio * (1.0 + drift_ppm)
        };

        // 5. Determine input frames needed (with +1 safety margin when fractional to guard against IEEE-754 repeated addition rounding)
        let initial_need = if self.resampler.has_samples { 0 } else { 2 };
        let extra_margin = usize::from((step - 1.0).abs() > 1e-12 || self.resampler.phase > 0.0);
        let step_advances =
            (self.resampler.phase + (count as f64) * step).floor() as usize + extra_margin;
        let total_needed = (initial_need + step_advances).min(MAX_SCRATCH_FRAMES);

        // Copy any unconsumed leftover frames from previous block to beginning of scratch
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

        let mut scratch_idx = 0;
        if !self.resampler.has_samples {
            if popped >= 2 {
                self.resampler.prev_l = self.scratch_l[0];
                self.resampler.prev_r = self.scratch_r[0];
                self.resampler.curr_l = self.scratch_l[1];
                self.resampler.curr_r = self.scratch_r[1];
                scratch_idx = 2;
                self.resampler.has_samples = true;
            } else if popped == 1 {
                self.resampler.prev_l = self.scratch_l[0];
                self.resampler.prev_r = self.scratch_r[0];
                self.resampler.curr_l = self.resampler.prev_l;
                self.resampler.curr_r = self.resampler.prev_r;
                scratch_idx = 1;
                self.resampler.has_samples = true;
            } else {
                left[..count].fill(0.0);
                right[..count].fill(0.0);
                return;
            }
        }

        // 6. Phase-continuous sample-by-sample linear interpolation
        for i in 0..count {
            let frac = self.resampler.phase as f32;
            left[i] =
                self.resampler.prev_l + frac * (self.resampler.curr_l - self.resampler.prev_l);
            right[i] =
                self.resampler.prev_r + frac * (self.resampler.curr_r - self.resampler.prev_r);
            self.resampler.phase += step;
            while self.resampler.phase >= 1.0 {
                self.resampler.phase -= 1.0;
                self.resampler.prev_l = self.resampler.curr_l;
                self.resampler.prev_r = self.resampler.curr_r;
                if scratch_idx < popped {
                    self.resampler.curr_l = self.scratch_l[scratch_idx];
                    self.resampler.curr_r = self.scratch_r[scratch_idx];
                    scratch_idx += 1;
                } else {
                    // Starvation underrun: soft exponential decay to silence without clicking
                    self.underrun_count += 1;
                    self.resampler.curr_l *= 0.98;
                    self.resampler.curr_r *= 0.98;
                }
            }
        }

        // 7. Save any unconsumed frames for the next block
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
