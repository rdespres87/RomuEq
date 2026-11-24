#![forbid(unsafe_code)]

//! Equaform DSP — digital signal processing primitives.
//!
//! This crate provides biquad filter coefficient generation for the MVP
//! filter types (Bell, LowShelf, HighShelf, HighPass, LowPass) as defined
//! in the Audio EQ Cookbook (BJD, 2020).
//!
//! # Modules
//!
//! - [`coefficients`] — biquad coefficient generation for all MVP filter types
//!
//! # Example
//!
//! ```
//! use equaform_dsp::coefficients::{compute_coefficients, BiquadCoefficients};
//! use equaform_common::{FilterType, Slope};
//!
//! let coeffs = compute_coefficients(
//!     FilterType::LowPass,
//!     1000.0,   // Hz
//!     0.0,      // dB (ignored for pass filters)
//!     0.707,    // Q (Butterworth)
//!     Slope::Db12,
//!     48000.0,  // sample rate
//! );
//! assert!(coeffs.is_finite());
//! ```

pub mod coefficients;
pub mod filter;
pub mod frequency_response;
pub mod simd;

// Re-export the main API for convenience.
pub use coefficients::{
    BiquadCoefficients, FilterSpecification, compute_coefficients, compute_coefficients_from_spec,
};
pub use filter::{BiquadFilter, BiquadState};
pub use frequency_response::{ResponsePoint, biquad_response_at_frequency, compute_response};
pub use simd::{process_mono_in_place, process_stereo_interleaved, process_stereo_planar_in_place};
