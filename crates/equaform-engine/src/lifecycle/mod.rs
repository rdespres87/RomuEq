#![forbid(unsafe_code)]

//! Allocation-free engine lifecycle management.
//!
//! This module provides `EngineLifecycle`, a state machine that manages the
//! prepare/process/reset/reprepare sequence for the Equaform engine.
//!
//! # Design
//!
//! The key requirement is **zero allocation during the processing path**.
//! All buffers are pre-allocated with a flat `[f32; MAX_CHANNELS * MAX_BLOCK_SIZE]`
//! array, eliminating both `Vec` indirection and per-block allocations.
//!
//! # Lifecycle
//!
//! ```text
//! EngineLifecycle::new()
//!     → prepare(sample_rate, max_block_size, channels)
//!     → set_parameters(eq_params)
//!     → process(inputs, outputs)  // can be called repeatedly
//!     → reset()                   // optional, clears internal state
//!     → prepare(new_params)       // reprepare (sample-rate/channel change)
//!     → destroy()                 // cleanup
//! ```
//!
//! # Real-time safety
//!
//! - `process()` performs **no allocations**, no syscalls, no blocking.
//! - Invalid-host behaviour is deterministic: returns `false` without
//!   modifying state (except the documented panic on `inputs.len() != outputs.len()`).
//! - All buffers are stack-allocated or use a flat pre-allocated array.
//!
//! # DSP processing
//!
//! The engine applies a cascade of biquad filters (one per band) per channel,
//! then applies output gain and optional bypass. All filter state is stored
//! in a flat `[f32; MAX_BANDS * MAX_CHANNELS * 4]` array (two delay samples
//! per biquad stage × 2 delay lines).

use crate::automation::AutomationIngestor;
use equaform_analysis::AnalysisBuffer;
use equaform_common::automation::{AutomationEvent, InterpolationMode};
use equaform_common::{
    EqParameters, MAX_BANDS, MAX_BLOCK_SIZE, MAX_CHANNELS,
};
use equaform_dsp::coefficients::{BiquadCoefficients, compute_coefficients};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of channels supported by the engine lifecycle.
pub(super) const LIFECYCLE_MAX_CHANNELS: usize = MAX_CHANNELS;

/// Maximum block size supported by the engine lifecycle.
pub(super) const LIFECYCLE_MAX_BLOCK_SIZE: usize = MAX_BLOCK_SIZE;

/// Total number of samples in the flat internal buffer:
/// `MAX_CHANNELS * MAX_BLOCK_SIZE`.
pub(super) const FLAT_BUFFER_SIZE: usize = LIFECYCLE_MAX_CHANNELS * LIFECYCLE_MAX_BLOCK_SIZE;

/// Number of biquad stages per channel (one per band).
pub(super) const BIQUAD_STAGES_PER_CHANNEL: usize = MAX_BANDS;

/// Delay line samples per biquad stage (x[n-1], x[n-2], y[n-1], y[n-2]).
pub(super) const DELAY_SAMPLES_PER_STAGE: usize = 4;

/// Total delay line storage: bands × channels × delay samples.
pub(super) const DELAY_BUFFER_SIZE: usize =
    BIQUAD_STAGES_PER_CHANNEL * LIFECYCLE_MAX_CHANNELS * DELAY_SAMPLES_PER_STAGE;

/// Crossfade duration for click-free bypass in milliseconds.
pub(super) const CROSSFADE_MS: usize = 10;

// ---------------------------------------------------------------------------
// LifecycleState
// ---------------------------------------------------------------------------

/// Current state of the engine lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleState {
    /// Engine has been created but not yet prepared.
    Created,
    /// Engine is prepared and ready to process audio.
    Ready,
}

impl Default for LifecycleState {
    fn default() -> Self {
        Self::Created
    }
}

// ---------------------------------------------------------------------------
// BypassTransition
// ---------------------------------------------------------------------------
// EngineLifecycle
// ---------------------------------------------------------------------------

/// Engine lifecycle manager.
///
/// Manages the prepare/process/reset/reprepare sequence for the engine.
/// It owns the sample rate, block size, channel configuration, a
/// **flat pre-allocated audio buffer**, and **flat pre-allocated biquad
/// delay-line state** — no `Vec` allocations during `process()`.
///
/// # Allocation-free guarantee
///
/// - `new()` allocates nothing (flat arrays are inline in the struct).
/// - `prepare()` zeroes the buffers but does **not** allocate.
/// - `process()` copies data into/out of the flat buffer and runs biquad
///   cascades — no allocation.
/// - `reset()` zeroes the buffers — no allocation.
/// - `destroy()` resets state — no allocation.
///
/// # Layout
///
/// The flat audio buffer is indexed as `data[channel * max_block_size + sample]`,
/// giving planar channel layout with a single contiguous allocation.
///
/// The delay-line buffer is indexed as
/// `delay[band * channels * 4 + channel * 4 + delay_slot]`,
/// where `delay_slot` is 0/1 for x[n-1]/x[n-2] and 2/3 for y[n-1]/y[n-2].
pub struct EngineLifecycle {
    /// Current lifecycle state.
    state: LifecycleState,
    /// Target sample rate in Hz (e.g. 44100.0, 48000.0).
    sample_rate: f64,
    /// Maximum audio block size in samples.
    max_block_size: usize,
    /// Number of channels (1 = mono, 2 = stereo).
    channels: usize,
    /// Flat pre-allocated audio buffer: `channel * max_block_size + sample`.
    /// Initialized to zeros; never reallocated.
    buffer: [f32; FLAT_BUFFER_SIZE],
    /// Flat pre-allocated biquad delay-line state:
    /// `band * channels * 4 + channel * 4 + slot`.
    /// Slot 0/1 = x[n-1]/x[n-2], slot 2/3 = y[n-1]/y[n-2].
    delay_line: [f32; DELAY_BUFFER_SIZE],
    /// Current EQ parameters.
    eq_params: EqParameters,
    /// Number of blocks processed since last reset.
    blocks_processed: u64,
    /// Current stream sample offset (monotonically increasing).
    sample_offset: u64,
    /// Automation event ingestor (initialized in process)
    automation_ingestor: Option<AutomationIngestor>,
    /// Last pushed automation event (for API)
    last_automation_event: Option<AutomationEvent>,

    // -----------------------------------------------------------------------
    // Bypass transition state (click-free crossfade)
    // -----------------------------------------------------------------------
    /// Previous bypass state (before the current transition)
    prev_bypass: bool,
    /// Current bypass state
    curr_bypass: bool,
    /// Crossfade progress in samples (0 = start, CROSSFADE_SAMPLES = done)
    crossfade_progress: usize,
    /// Whether a transition is currently in progress
    transitioning: bool,
    /// Buffer to store original input for crossfade bypass
    bypass_buffer: [f32; FLAT_BUFFER_SIZE],

    // -----------------------------------------------------------------------
    // Analysis handoff (M5-T001)
    // -----------------------------------------------------------------------
    /// Bounded ring buffer for analysis frame handoff to the GUI thread.
    /// Audio thread pushes frames; GUI thread consumes them.
    analysis_buffer: AnalysisBuffer,

    // -----------------------------------------------------------------------
    // Coefficient caching (zero per-block recalculation)
    // -----------------------------------------------------------------------
    /// Cached biquad filter coefficients for each band.
    cached_coeffs: [BiquadCoefficients; MAX_BANDS],
    /// Dirty flag indicating whether coefficients need recalculation.
    coeffs_dirty: bool,
    /// Cached perceptual loudness auto-gain compensation in dB.
    cached_auto_gain_db: f32,
}

mod auto_gain;
mod processor;

#[cfg(test)]
mod tests;

pub use auto_gain::calculate_auto_gain_compensation_db;

impl EngineLifecycle {
    /// Create a new engine lifecycle in the `Created` state.
    ///
    /// No allocation occurs — all arrays are inline in the struct.
    pub fn new() -> Self {
        EngineLifecycle {
            state: LifecycleState::Created,
            sample_rate: 44100.0,
            max_block_size: 512,
            channels: 2,
            buffer: [0.0; FLAT_BUFFER_SIZE],
            delay_line: [0.0; DELAY_BUFFER_SIZE],
            eq_params: EqParameters::default(),
            blocks_processed: 0,
            sample_offset: 0,
            automation_ingestor: None,
            last_automation_event: None,
            // Bypass transition state (click-free crossfade)
            prev_bypass: false,
            curr_bypass: false,
            crossfade_progress: 0,
            transitioning: false,
            bypass_buffer: [0.0; FLAT_BUFFER_SIZE],
            // Analysis handoff (M5-T001) — pre-allocated ring buffer
            analysis_buffer: AnalysisBuffer::new(),
            // Cached filter coefficients
            cached_coeffs: [BiquadCoefficients::default(); MAX_BANDS],
            coeffs_dirty: true,
            cached_auto_gain_db: 0.0,
        }
    }

    /// Prepare the engine with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` — target sample rate in Hz (e.g. 44100.0, 48000.0).
    /// * `max_block_size` — maximum audio block size in samples.
    /// * `channels` — number of channels (1 = mono, 2 = stereo).
    ///
    /// # Returns
    ///
    /// `true` if preparation succeeded.
    ///
    /// # Panics
    ///
    /// Panics if `sample_rate` is zero or negative, if `max_block_size` is
    /// zero, or if `channels` is zero or exceeds `MAX_CHANNELS`.
    ///
    /// # Allocation
    ///
    /// **No allocation.** The flat buffers are pre-sized at construction time.
    /// This method only updates metadata and zeroes the used portion.
    pub fn prepare(&mut self, sample_rate: f64, max_block_size: usize, channels: usize) -> bool {
        assert!(sample_rate > 0.0, "sample_rate must be > 0");
        assert!(max_block_size > 0, "max_block_size must be > 0");
        assert!(
            channels > 0 && channels <= LIFECYCLE_MAX_CHANNELS,
            "channels must be in 1..={LIFECYCLE_MAX_CHANNELS}"
        );
        assert!(
            max_block_size <= LIFECYCLE_MAX_BLOCK_SIZE,
            "max_block_size must be <= {LIFECYCLE_MAX_BLOCK_SIZE}"
        );

        self.sample_rate = sample_rate;
        self.max_block_size = max_block_size;
        self.channels = channels;

        // Zero the entire flat buffer on prepare (reprepare clears stale data).
        self.buffer.iter_mut().for_each(|s| *s = 0.0);
        self.bypass_buffer.iter_mut().for_each(|s| *s = 0.0);
        self.delay_line.iter_mut().for_each(|s| *s = 0.0);

        // Reset bypass transition state on prepare.
        self.prev_bypass = false;
        self.curr_bypass = false;
        self.crossfade_progress = 0;
        self.transitioning = false;

        // Initialize the automation ingestor on prepare (not process) so it is
        // allocation-free in the hot path.
        self.automation_ingestor = Some(AutomationIngestor::new(InterpolationMode::Linear));

        self.recompute_all_coefficients();

        self.state = LifecycleState::Ready;
        true
    }

    /// Recompute cached biquad filter coefficients for all bands.
    fn recompute_all_coefficients(&mut self) {
        let sample_rate = self.sample_rate as f32;
        for band_idx in 0..MAX_BANDS {
            let band_params = &self.eq_params.bands[band_idx];
            self.cached_coeffs[band_idx] = compute_coefficients(
                band_params.filter_type,
                band_params.frequency_hz,
                band_params.gain_db,
                band_params.q,
                band_params.slope,
                sample_rate,
            );
        }
        self.coeffs_dirty = false;

        self.update_auto_gain_db();
    }

    /// Recompute perceptual RMS power loudness-weighted auto-gain compensation in dB.
    fn update_auto_gain_db(&mut self) {
        self.cached_auto_gain_db =
            calculate_auto_gain_compensation_db(&self.eq_params.bands, self.sample_rate as f32);
    }

    /// Return the active auto-gain compensation in dB.
    pub fn auto_gain_compensation_db(&self) -> f32 {
        if self.eq_params.global.auto_gain {
            self.cached_auto_gain_db
        } else {
            0.0
        }
    }

    /// Set the EQ parameters.
    ///
    /// Parameters are applied on the next `process()` call.
    ///
    /// # Allocation
    ///
    /// **No allocation.** The `EqParameters` struct is `Copy` and stored inline.
    pub fn set_parameters(&mut self, params: EqParameters) {
        if self.eq_params != params {
            self.eq_params = params;
            self.coeffs_dirty = true;
            self.update_auto_gain_db();
        }
    }

    /// M5-T001: Get a mutable reference to the analysis buffer for GUI thread consumption.
    ///
    /// The GUI thread calls `consume_all()` on the returned buffer to drain
    /// analysis frames produced by the audio thread.
    #[allow(dead_code)]
    pub fn analysis_buffer(&mut self) -> &mut AnalysisBuffer {
        &mut self.analysis_buffer
    }

    /// Push an automation event into the ingestor.
    ///
    /// The event is queued and will be batched when `process()` is called.
    /// This method is non-blocking and allocation-free (the ingestor uses
    /// pre-allocated internal storage).
    pub fn push_automation_event(&mut self, event: AutomationEvent) {
        if let Some(ingestor) = &mut self.automation_ingestor {
            ingestor.push_event(event);
        }
        self.last_automation_event = Some(event);
    }

    /// Reset internal state (delays, filters, meters).
    ///
    /// Does NOT change sample rate, block size, or channel configuration.
    /// The engine remains in the `Ready` state.
    ///
    /// # Allocation
    ///
    /// **No allocation.** Only zeroes the flat buffers and resets the counter.
    pub fn reset(&mut self) {
        if self.state == LifecycleState::Ready {
            self.buffer.iter_mut().for_each(|s| *s = 0.0);
            self.bypass_buffer.iter_mut().for_each(|s| *s = 0.0);
            self.delay_line.iter_mut().for_each(|s| *s = 0.0);
            self.blocks_processed = 0;
            self.sample_offset = 0;
            if let Some(ref mut ingestor) = self.automation_ingestor {
                ingestor.reset();
            }
            // Reset bypass transition state.
            self.prev_bypass = false;
            self.curr_bypass = false;
            self.crossfade_progress = 0;
            self.transitioning = false;
        }
    }

    /// Destroy the engine lifecycle.
    ///
    /// Transitions to the `Created` state. After this, `prepare` must be
    /// called again before processing.
    ///
    /// # Allocation
    ///
    /// **No allocation.** Simply resets state.
    pub fn destroy(&mut self) {
        self.state = LifecycleState::Created;
        self.buffer.iter_mut().for_each(|s| *s = 0.0);
        self.delay_line.iter_mut().for_each(|s| *s = 0.0);
        self.blocks_processed = 0;
        self.sample_offset = 0;
        self.automation_ingestor = None;
    }

    /// Return the current lifecycle state.
    pub fn state(&self) -> LifecycleState {
        self.state
    }

    /// Return the current sample rate.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Return the current max block size.
    pub fn max_block_size(&self) -> usize {
        self.max_block_size
    }

    /// Return the current channel count.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Return the number of blocks processed since last reset.
    pub fn blocks_processed(&self) -> u64 {
        self.blocks_processed
    }

    /// Return a reference to the flat buffer data.
    ///
    /// The buffer is laid out as planar channels:
    /// `data[channel * max_block_size + sample]`.
    pub fn buffer(&self) -> &[f32] {
        &self.buffer
    }

    /// Return a mutable reference to the flat buffer data.
    pub fn buffer_mut(&mut self) -> &mut [f32] {
        &mut self.buffer
    }

    /// Return the current EQ parameters.
    pub fn eq_parameters(&self) -> &EqParameters {
        &self.eq_params
    }

    /// Return a mutable reference to the current EQ parameters.
    pub fn eq_parameters_mut(&mut self) -> &mut EqParameters {
        &mut self.eq_params
    }

    /// Serialize the current EQ parameters to a bincode byte vector.
    pub fn save_state(&self) -> Vec<u8> {
        bincode::serialize(&self.eq_params).unwrap_or_default()
    }

    /// Deserialize EQ parameters from a bincode byte slice.
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        if let Ok(params) = bincode::deserialize::<EqParameters>(data) {
            self.eq_params = params;
            self.coeffs_dirty = true;
            true
        } else {
            false
        }
    }
}

impl Default for EngineLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

