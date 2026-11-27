//! Parameter ID constants, namespaces, and ID category inspection.

use std::fmt;

use super::info::parameter_info;

/// Maximum number of EQ bands.
pub const MAX_BANDS: usize = 8;

/// Global parameter ID range: 1–99.
pub const GLOBAL_ID_MIN: u32 = 1;
pub const GLOBAL_ID_MAX: u32 = 99;

/// Band parameter ID range: 100–899.
pub const BAND_ID_MIN: u32 = 100;
pub const BAND_ID_MAX: u32 = 899;

/// Reserved for future MVP-compatible additions: 900–999.
pub const RESERVED_ID_MIN: u32 = 900;
pub const RESERVED_ID_MAX: u32 = 999;

/// Reserved for future product-family features: >= 1000.
pub const FUTURE_ID_MIN: u32 = 1000;

/// Global parameter IDs.
pub mod global {
    pub const BYPASS: u32 = 1;
    pub const OUTPUT_GAIN_DB: u32 = 2;
    pub const AUTO_GAIN: u32 = 3;
    pub const PROCESSING_MODE: u32 = 4;
    pub const STEREO_MODE: u32 = 5;
    pub const AUTO_GAIN_COMPENSATION_DB: u32 = 6;
    pub const AUTO_GAIN_MODE: u32 = 7;
}

/// Band parameter ID offsets (relative to band base).
pub mod band {
    pub const ENABLED: u32 = 0;
    pub const FILTER_TYPE: u32 = 1;
    pub const FREQUENCY_HZ: u32 = 2;
    pub const GAIN_DB: u32 = 3;
    pub const Q: u32 = 4;
    pub const SLOPE: u32 = 5;
    pub const SOLO: u32 = 6;
    pub const ROUTING: u32 = 7;
    pub const RESERVED_END: u32 = 99;
}

/// Stable parameter identifier.
///
/// IDs are decimal `u32` values allocated according to the frozen
/// PARAMETER_STATE_CONTRACT.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParamId(pub u32);

impl ParamId {
    /// Create a new parameter ID.
    pub const fn new(id: u32) -> Self {
        ParamId(id)
    }

    /// Returns the category of this parameter ID.
    pub fn category(&self) -> ParamCategory {
        let id = self.0;
        if (GLOBAL_ID_MIN..=GLOBAL_ID_MAX).contains(&id) {
            ParamCategory::Global
        } else if (BAND_ID_MIN..=BAND_ID_MAX).contains(&id) {
            ParamCategory::Band
        } else if (RESERVED_ID_MIN..=RESERVED_ID_MAX).contains(&id) {
            ParamCategory::Reserved
        } else if id >= FUTURE_ID_MIN {
            ParamCategory::Future
        } else {
            ParamCategory::Invalid
        }
    }

    /// Returns the band index for a band parameter, or `None` if not a band ID.
    pub fn band_index(&self) -> Option<usize> {
        if self.category() != ParamCategory::Band {
            return None;
        }
        let offset = self.0 - BAND_ID_MIN;
        Some((offset / 100) as usize)
    }

    /// Returns the band offset for a band parameter (0–99), or `None`.
    pub fn band_offset(&self) -> Option<u32> {
        if self.category() != ParamCategory::Band {
            return None;
        }
        Some(self.0 - BAND_ID_MIN - ((self.0 - BAND_ID_MIN) / 100) * 100)
    }

    /// Returns the human-readable name for this parameter, or `"?"` if unknown.
    pub fn display_name(&self) -> &'static str {
        parameter_info(self.0).map(|info| info.name).unwrap_or("?")
    }
}

impl fmt::Display for ParamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ParamId({})", self.0)
    }
}

/// Category of a parameter ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamCategory {
    /// Global parameter (ID 1–99).
    Global,
    /// Band parameter (ID 100–899).
    Band,
    /// Reserved for future MVP-compatible additions (ID 900–999).
    Reserved,
    /// Reserved for future product-family features (ID >= 1000).
    Future,
    /// Invalid ID (ID 0).
    Invalid,
}
