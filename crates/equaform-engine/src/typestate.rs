//! Compile-time typestate wrapper around [`EngineLifecycle`].
//!
//! Enforces the `Unconfigured -> Prepared -> Running` state machine at compile
//! time with zero runtime or memory overhead (`PhantomData` marker only).

use crate::lifecycle::EngineLifecycle;
use equaform_analysis::AnalysisBuffer;
use equaform_common::EqParameters;
use equaform_common::audio_buffer::{MAX_BLOCK_SIZE, MAX_CHANNELS};
use equaform_common::automation::AutomationEvent;
use std::fmt;
use std::marker::PhantomData;

mod private {
    pub trait Sealed {}
}

/// Sealed marker trait implemented by all valid [`Engine`] lifecycle states.
pub trait EngineState: private::Sealed {}

/// Initial state before audio topology has been validated and prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unconfigured;

/// Configured state ready to start processing or be re-prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prepared;

/// Active processing state in which real-time audio blocks may be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Running;

impl private::Sealed for Unconfigured {}
impl private::Sealed for Prepared {}
impl private::Sealed for Running {}
impl EngineState for Unconfigured {}
impl EngineState for Prepared {}
impl EngineState for Running {}

/// Configuration validation errors returned by [`Engine::prepare`] and [`Engine::reprepare`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EngineConfigError {
    /// Sample rate was non-positive or non-finite (`NaN`, `±inf`).
    InvalidSampleRate(f64),
    /// Block size was zero or exceeded the static buffer capacity.
    InvalidBlockSize { requested: usize, max: usize },
    /// Channel count was zero or exceeded the static channel capacity.
    InvalidChannelCount { requested: usize, max: usize },
}

impl fmt::Display for EngineConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleRate(sr) => write!(f, "invalid sample rate: {sr}"),
            Self::InvalidBlockSize { requested, max } => {
                write!(f, "invalid block size: {requested} (max {max})")
            }
            Self::InvalidChannelCount { requested, max } => {
                write!(f, "invalid channel count: {requested} (max {max})")
            }
        }
    }
}

impl std::error::Error for EngineConfigError {}

fn validate_config(
    sample_rate: f64,
    max_block_size: usize,
    channels: usize,
) -> Result<(), EngineConfigError> {
    if !sample_rate.is_finite() || sample_rate <= 0.0 {
        return Err(EngineConfigError::InvalidSampleRate(sample_rate));
    }
    if max_block_size == 0 || max_block_size > MAX_BLOCK_SIZE {
        return Err(EngineConfigError::InvalidBlockSize {
            requested: max_block_size,
            max: MAX_BLOCK_SIZE,
        });
    }
    if channels == 0 || channels > MAX_CHANNELS {
        return Err(EngineConfigError::InvalidChannelCount {
            requested: channels,
            max: MAX_CHANNELS,
        });
    }
    Ok(())
}

/// Zero-overhead typestate engine enforcing `Unconfigured -> Prepared -> Running`.
///
/// Calling `process` before `prepare` and `start` is rejected at compile time:
///
/// ```compile_fail
/// use equaform_engine::Engine;
///
/// let mut engine = Engine::new();
/// let input = [0.0f32; 64];
/// let mut output = [0.0f32; 64];
/// engine.process(&[&input], &mut [&mut output]);
/// ```
///
/// ```compile_fail
/// use equaform_engine::Engine;
///
/// let mut engine = Engine::new().prepare(48_000.0, 64, 1).ok().unwrap();
/// let input = [0.0f32; 64];
/// let mut output = [0.0f32; 64];
/// engine.process(&[&input], &mut [&mut output]);
/// ```
pub struct Engine<S: EngineState> {
    inner: Box<EngineLifecycle>,
    _state: PhantomData<S>,
}

impl Engine<Unconfigured> {
    /// Create a new unconfigured engine, heap-allocating the ~526 KiB inline DSP buffers once
    /// so all subsequent state transitions move only a pointer and perform zero allocations.
    pub fn new() -> Self {
        Self {
            inner: Box::new(EngineLifecycle::new()),
            _state: PhantomData,
        }
    }

    /// Validate topology and transition to [`Prepared`].
    ///
    /// On invalid parameters, returns the original `Engine<Unconfigured>` alongside the error.
    pub fn prepare(
        mut self,
        sample_rate: f64,
        max_block_size: usize,
        channels: usize,
    ) -> Result<Engine<Prepared>, (Self, EngineConfigError)> {
        if let Err(err) = validate_config(sample_rate, max_block_size, channels) {
            return Err((self, err));
        }
        self.inner.prepare(sample_rate, max_block_size, channels);
        Ok(Engine {
            inner: self.inner,
            _state: PhantomData,
        })
    }
}

impl Default for Engine<Unconfigured> {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine<Prepared> {
    /// Transition to [`Running`] so real-time audio blocks can be processed.
    pub fn start(self) -> Engine<Running> {
        Engine {
            inner: self.inner,
            _state: PhantomData,
        }
    }

    /// Re-validate and apply a new audio topology while staying [`Prepared`].
    ///
    /// On invalid parameters, preserves the previous prepared topology and returns it.
    pub fn reprepare(
        mut self,
        sample_rate: f64,
        max_block_size: usize,
        channels: usize,
    ) -> Result<Engine<Prepared>, (Self, EngineConfigError)> {
        if let Err(err) = validate_config(sample_rate, max_block_size, channels) {
            return Err((self, err));
        }
        self.inner.prepare(sample_rate, max_block_size, channels);
        Ok(Engine {
            inner: self.inner,
            _state: PhantomData,
        })
    }

    /// Tear down the prepared state and return to [`Unconfigured`].
    pub fn unconfigure(mut self) -> Engine<Unconfigured> {
        self.inner.destroy();
        Engine {
            inner: self.inner,
            _state: PhantomData,
        }
    }
}

impl Engine<Running> {
    /// Process a single audio block (zero allocation, wait-free).
    pub fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) -> bool {
        self.inner.process(inputs, outputs)
    }

    /// Enqueue a sample-accurate automation event for the next processed block.
    pub fn push_automation_event(&mut self, event: AutomationEvent) {
        self.inner.push_automation_event(event);
    }

    /// Clear delay lines and internal meters without leaving [`Running`].
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Stop processing, reset DSP state, and return to [`Prepared`].
    pub fn stop(mut self) -> Engine<Prepared> {
        self.inner.reset();
        Engine {
            inner: self.inner,
            _state: PhantomData,
        }
    }

    /// Destroy active state and return to [`Unconfigured`].
    pub fn unconfigure(mut self) -> Engine<Unconfigured> {
        self.inner.destroy();
        Engine {
            inner: self.inner,
            _state: PhantomData,
        }
    }
}

impl<S: EngineState> Engine<S> {
    /// Update EQ parameters (applied on next audio block).
    pub fn set_parameters(&mut self, params: EqParameters) {
        self.inner.set_parameters(params);
    }

    /// Read the active EQ parameters.
    pub fn eq_parameters(&self) -> &EqParameters {
        self.inner.eq_parameters()
    }

    /// Return the active auto-gain compensation in dB.
    pub fn auto_gain_compensation_db(&self) -> f32 {
        self.inner.auto_gain_compensation_db()
    }

    /// Return the algorithmic latency in samples for the active processing mode.
    #[inline]
    pub fn latency_samples(&self) -> u32 {
        self.inner.latency_samples()
    }

    /// Serialize EQ parameters to a binary payload.
    pub fn save_state(&self) -> Vec<u8> {
        self.inner.save_state()
    }

    /// Deserialize EQ parameters from a binary payload.
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        self.inner.load_state(data)
    }

    /// Return the configured sample rate in Hz.
    pub fn sample_rate(&self) -> f64 {
        self.inner.sample_rate()
    }

    /// Return the configured maximum block size in samples.
    pub fn max_block_size(&self) -> usize {
        self.inner.max_block_size()
    }

    /// Return the configured channel count.
    pub fn channels(&self) -> usize {
        self.inner.channels()
    }

    /// Return the number of audio blocks processed since the last reset.
    pub fn blocks_processed(&self) -> u64 {
        self.inner.blocks_processed()
    }

    /// Borrow the lock-free analysis buffer for GUI consumption.
    pub fn analysis_buffer(&mut self) -> &mut AnalysisBuffer {
        self.inner.analysis_buffer()
    }

    /// Borrow the underlying [`EngineLifecycle`] for read-only inspection.
    pub fn as_lifecycle(&self) -> &EngineLifecycle {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::EngineLifecycle;
    use equaform_common::EqParameters;
    use equaform_common::audio_buffer::{MAX_BLOCK_SIZE, MAX_CHANNELS};
    use std::mem::size_of;

    #[test]
    fn test_typestate_zero_size_overhead() {
        let base = size_of::<Box<EngineLifecycle>>();
        assert_eq!(size_of::<Engine<Unconfigured>>(), base);
        assert_eq!(size_of::<Engine<Prepared>>(), base);
        assert_eq!(size_of::<Engine<Running>>(), base);
    }

    #[test]
    fn test_typestate_full_lifecycle_transitions() -> Result<(), String> {
        let engine = Engine::new();
        let prepared = engine
            .prepare(48_000.0, 256, 2)
            .map_err(|(_, e)| format!("prepare failed: {e}"))?;
        assert_eq!(prepared.sample_rate(), 48_000.0);
        assert_eq!(prepared.max_block_size(), 256);
        assert_eq!(prepared.channels(), 2);

        let mut running = prepared.start();
        assert_eq!(running.blocks_processed(), 0);

        let block = 256;
        let in_l = vec![0.0f32; block];
        let in_r = vec![0.0f32; block];
        let mut out_l = vec![0.0f32; block];
        let mut out_r = vec![0.0f32; block];

        let ok = running.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]);
        assert!(ok);
        assert_eq!(running.blocks_processed(), 1);

        let prepared2 = running.stop();
        assert_eq!(prepared2.sample_rate(), 48_000.0);
        assert_eq!(prepared2.blocks_processed(), 0);

        let prepared3 = prepared2
            .reprepare(96_000.0, 128, 2)
            .map_err(|(_, e)| format!("reprepare failed: {e}"))?;
        assert_eq!(prepared3.sample_rate(), 96_000.0);
        assert_eq!(prepared3.max_block_size(), 128);

        let running2 = prepared3.start();
        let _unconfigured = running2.unconfigure();
        Ok(())
    }

    #[test]
    fn test_typestate_prepare_rejects_invalid_configs() -> Result<(), String> {
        let valid_sr = 48_000.0;
        let valid_bs = 256usize;
        let valid_ch = 2usize;

        for bad_sr in [0.0_f64, -48_000.0, f64::NAN, f64::INFINITY] {
            let engine = Engine::new();
            match engine.prepare(bad_sr, valid_bs, valid_ch) {
                Err((returned_engine, EngineConfigError::InvalidSampleRate(v))) => {
                    assert!(
                        (bad_sr.is_nan() && v.is_nan())
                            || (bad_sr.is_infinite() && v.is_infinite())
                            || (v - bad_sr).abs() < f64::EPSILON
                    );
                    let ok = returned_engine
                        .prepare(valid_sr, valid_bs, valid_ch)
                        .map_err(|(_, e)| format!("re-prepare after bad sr: {e}"))?;
                    assert_eq!(ok.sample_rate(), valid_sr);
                }
                Err((_, e)) => return Err(format!("expected InvalidSampleRate, got {e}")),
                Ok(_) => return Err(format!("expected error for sample_rate={bad_sr}")),
            }
        }

        for bad_bs in [0usize, MAX_BLOCK_SIZE + 1] {
            let engine = Engine::new();
            match engine.prepare(valid_sr, bad_bs, valid_ch) {
                Err((returned_engine, EngineConfigError::InvalidBlockSize { requested, max })) => {
                    assert_eq!(requested, bad_bs);
                    assert_eq!(max, MAX_BLOCK_SIZE);
                    let ok = returned_engine
                        .prepare(valid_sr, valid_bs, valid_ch)
                        .map_err(|(_, e)| format!("re-prepare after bad bs: {e}"))?;
                    assert_eq!(ok.max_block_size(), valid_bs);
                }
                Err((_, e)) => return Err(format!("expected InvalidBlockSize, got {e}")),
                Ok(_) => return Err(format!("expected error for block_size={bad_bs}")),
            }
        }

        for bad_ch in [0usize, MAX_CHANNELS + 1] {
            let engine = Engine::new();
            match engine.prepare(valid_sr, valid_bs, bad_ch) {
                Err((
                    returned_engine,
                    EngineConfigError::InvalidChannelCount { requested, max },
                )) => {
                    assert_eq!(requested, bad_ch);
                    assert_eq!(max, MAX_CHANNELS);
                    let ok = returned_engine
                        .prepare(valid_sr, valid_bs, valid_ch)
                        .map_err(|(_, e)| format!("re-prepare after bad ch: {e}"))?;
                    assert_eq!(ok.channels(), valid_ch);
                }
                Err((_, e)) => return Err(format!("expected InvalidChannelCount, got {e}")),
                Ok(_) => return Err(format!("expected error for channels={bad_ch}")),
            }
        }

        Ok(())
    }

    #[test]
    fn test_typestate_reprepare_error_preserves_prepared_state() -> Result<(), String> {
        let prepared = Engine::new()
            .prepare(48_000.0, 256, 2)
            .map_err(|(_, e)| format!("initial prepare: {e}"))?;

        match prepared.reprepare(0.0, 256, 2) {
            Err((preserved, EngineConfigError::InvalidSampleRate(_))) => {
                assert_eq!(preserved.sample_rate(), 48_000.0);
                assert_eq!(preserved.max_block_size(), 256);
                assert_eq!(preserved.channels(), 2);
                Ok(())
            }
            Err((_, e)) => Err(format!("expected InvalidSampleRate, got {e}")),
            Ok(_) => Err("expected reprepare to reject sample_rate=0.0".to_string()),
        }
    }

    #[test]
    fn test_typestate_parameters_and_state_roundtrip() -> Result<(), String> {
        let mut running = Engine::new()
            .prepare(48_000.0, 256, 2)
            .map_err(|(_, e)| format!("prepare: {e}"))?
            .start();

        let mut custom = EqParameters::default();
        custom.bands[0].gain_db = 4.5;
        running.set_parameters(custom);
        assert_eq!(running.eq_parameters(), &custom);

        let saved = running.save_state();
        running.set_parameters(EqParameters::default());
        assert_ne!(running.eq_parameters(), &custom);

        assert!(running.load_state(&saved));
        assert_eq!(running.eq_parameters(), &custom);
        assert_eq!(running.latency_samples(), 0);

        custom.global.processing_mode = equaform_common::ProcessingMode::LinearPhaseLow;
        running.set_parameters(custom);
        assert_eq!(running.latency_samples(), 1024);

        let prepared_again = running.stop();
        assert_eq!(prepared_again.latency_samples(), 1024);

        Ok(())
    }
}
