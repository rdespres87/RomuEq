//! Stable parameter types, IDs, ranges, and validation.
//!
//! Implements the frozen PARAMETER_STATE_CONTRACT for Equaform MVP.
//!
//! # Parameter ID Allocation
//!
//! | ID range | Meaning |
//! |---:|---|
//! | 1–99 | Global parameters |
//! | 100–899 | Eight band parameter groups, 100–199 per band |
//! | 900–999 | Reserved for future MVP-compatible additions |
//! | >=1000 | Reserved for future product-family features |

mod eq;
mod ids;
mod info;
mod types;

#[cfg(test)]
mod tests;

pub use eq::{BandParameters, EqParameters, GlobalParameters, ParameterCommand};
pub use ids::{
    BAND_ID_MAX, BAND_ID_MIN, FUTURE_ID_MIN, GLOBAL_ID_MAX, GLOBAL_ID_MIN, MAX_BANDS,
    ParamCategory, ParamId, RESERVED_ID_MAX, RESERVED_ID_MIN, band, global,
};
pub use info::{
    ParameterError, ParameterInfo, ParameterRange, ParameterType, clamp_value, parameter_info,
    validate_value,
};
pub use types::{AutoGainMode, BandRouting, FilterType, ProcessingMode, Slope, StereoMode};
