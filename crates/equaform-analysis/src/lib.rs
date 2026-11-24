#![forbid(unsafe_code)]

//! Analysis data path for Equaform.
//!
//! Provides a bounded, allocation-free handoff from the audio thread to an
//! analysis consumer (GUI, metering, spectrum). The design guarantees that
//! the audio thread never performs FFT or heavy computation.
//!
//! ## Architecture
//!
//! ```text
//! [audio thread] ──> capture() ──> AnalysisBuffer (ring) ──> consume() ──> [GUI thread]
//!                    O(1) copy         bounded SPSC            O(1) drain
//! ```
//!
//! - **AnalysisFrame**: per-block analysis data (RMS, peak, channel energy).
//!   Computed by decimating the raw buffer, not by FFT.
//! - **AnalysisBuffer**: bounded SPSC ring buffer with drop-new overflow policy.
//!   Pre-allocated at construction, no allocation after.
//! - **AnalysisEngine**: consumer-side API that drains frames and provides
//!   the latest analysis snapshot.

mod engine;
pub mod fft;
mod meters;
mod ring_buffer;

pub use engine::{AnalysisConfig, AnalysisEngine};
pub use fft::{LogBinMode, SpectrumConfig, SpectrumFrame, SpectrumProcessor, WindowType};
pub use meters::*;
pub use ring_buffer::{AnalysisBuffer, AnalysisFrame};
