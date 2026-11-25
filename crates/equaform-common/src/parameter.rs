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

use std::fmt;

/// Maximum number of EQ bands.
pub const MAX_BANDS: usize = 8;

// ===========================================================================
// ID constants
// ===========================================================================

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

// ===========================================================================
// ParamId
// ===========================================================================

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

// ===========================================================================
// ParamCategory
// ===========================================================================

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

// ===========================================================================
// ParameterType
// ===========================================================================

/// Logical parameter type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterType {
    /// Boolean / stepped parameter (0 or 1).
    Bool,
    /// Continuous parameter with a numeric range.
    Continuous,
    /// Enumerated parameter with a fixed number of values.
    Enum { count: u32 },
}

// ===========================================================================
// ParameterRange
// ===========================================================================

/// Logical parameter range with min, max, and default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParameterRange {
    /// Minimum allowed value.
    pub min: f32,
    /// Maximum allowed value.
    pub max: f32,
    /// Default value.
    pub default: f32,
}

impl ParameterRange {
    /// Clamp a value to this range.
    pub fn clamp(&self, value: f32) -> f32 {
        value.clamp(self.min, self.max)
    }

    /// Validate that a value is within range.
    pub fn validate(&self, value: f32) -> bool {
        value >= self.min && value <= self.max
    }
}

// ===========================================================================
// ParameterInfo
// ===========================================================================

/// Complete information about a single parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParameterInfo {
    /// Stable parameter ID.
    pub id: u32,
    /// Human-readable name.
    pub name: &'static str,
    /// Logical parameter type.
    pub param_type: ParameterType,
    /// Valid range and default.
    pub range: ParameterRange,
    /// Unit of measurement (empty string if none).
    pub unit: &'static str,
}

/// Returns the `ParameterInfo` for a given ID, or `None` if the ID is not
/// a known parameter.
pub fn parameter_info(id: u32) -> Option<ParameterInfo> {
    match id {
        // Global parameters
        1 => Some(ParameterInfo {
            id: 1,
            name: "global.bypass",
            param_type: ParameterType::Bool,
            range: ParameterRange {
                min: 0.0,
                max: 1.0,
                default: 0.0,
            },
            unit: "",
        }),
        2 => Some(ParameterInfo {
            id: 2,
            name: "global.output_gain_db",
            param_type: ParameterType::Continuous,
            range: ParameterRange {
                min: -24.0,
                max: 24.0,
                default: 0.0,
            },
            unit: "dB",
        }),
        3 => Some(ParameterInfo {
            id: 3,
            name: "global.auto_gain",
            param_type: ParameterType::Bool,
            range: ParameterRange {
                min: 0.0,
                max: 1.0,
                default: 0.0,
            },
            unit: "",
        }),
        4 => Some(ParameterInfo {
            id: 4,
            name: "global.processing_mode",
            param_type: ParameterType::Enum { count: 1 },
            range: ParameterRange {
                min: 0.0,
                max: 0.0,
                default: 0.0,
            },
            unit: "",
        }),
        5 => Some(ParameterInfo {
            id: 5,
            name: "global.stereo_mode",
            param_type: ParameterType::Enum { count: 1 },
            range: ParameterRange {
                min: 0.0,
                max: 0.0,
                default: 0.0,
            },
            unit: "",
        }),
        7 => Some(ParameterInfo {
            id: 7,
            name: "global.auto_gain_mode",
            param_type: ParameterType::Enum { count: 2 },
            range: ParameterRange {
                min: 0.0,
                max: 1.0,
                default: 0.0,
            },
            unit: "",
        }),
        _ => band_parameter_info(id),
    }
}

/// Resolves `ParameterInfo` for band parameters (IDs 100..899) via offset mapping.
fn band_parameter_info(id: u32) -> Option<ParameterInfo> {
    let band_idx = (id / 100).checked_sub(1)?;
    if band_idx >= 8 {
        return None;
    }
    let sub = id % 100;
    static NAMES: [[&str; 6]; 8] = [
        [
            "band[0].enabled",
            "band[0].filter_type",
            "band[0].frequency_hz",
            "band[0].gain_db",
            "band[0].q",
            "band[0].slope",
        ],
        [
            "band[1].enabled",
            "band[1].filter_type",
            "band[1].frequency_hz",
            "band[1].gain_db",
            "band[1].q",
            "band[1].slope",
        ],
        [
            "band[2].enabled",
            "band[2].filter_type",
            "band[2].frequency_hz",
            "band[2].gain_db",
            "band[2].q",
            "band[2].slope",
        ],
        [
            "band[3].enabled",
            "band[3].filter_type",
            "band[3].frequency_hz",
            "band[3].gain_db",
            "band[3].q",
            "band[3].slope",
        ],
        [
            "band[4].enabled",
            "band[4].filter_type",
            "band[4].frequency_hz",
            "band[4].gain_db",
            "band[4].q",
            "band[4].slope",
        ],
        [
            "band[5].enabled",
            "band[5].filter_type",
            "band[5].frequency_hz",
            "band[5].gain_db",
            "band[5].q",
            "band[5].slope",
        ],
        [
            "band[6].enabled",
            "band[6].filter_type",
            "band[6].frequency_hz",
            "band[6].gain_db",
            "band[6].q",
            "band[6].slope",
        ],
        [
            "band[7].enabled",
            "band[7].filter_type",
            "band[7].frequency_hz",
            "band[7].gain_db",
            "band[7].q",
            "band[7].slope",
        ],
    ];

    match sub {
        0 => Some(ParameterInfo {
            id,
            name: NAMES[band_idx as usize][0],
            param_type: ParameterType::Bool,
            range: ParameterRange {
                min: 0.0,
                max: 1.0,
                default: 1.0,
            },
            unit: "",
        }),
        1 => Some(ParameterInfo {
            id,
            name: NAMES[band_idx as usize][1],
            param_type: ParameterType::Enum { count: 5 },
            range: ParameterRange {
                min: 0.0,
                max: 4.0,
                default: 0.0,
            },
            unit: "",
        }),
        2 => Some(ParameterInfo {
            id,
            name: NAMES[band_idx as usize][2],
            param_type: ParameterType::Continuous,
            range: ParameterRange {
                min: 20.0,
                max: 20000.0,
                default: 1000.0,
            },
            unit: "Hz",
        }),
        3 => Some(ParameterInfo {
            id,
            name: NAMES[band_idx as usize][3],
            param_type: ParameterType::Continuous,
            range: ParameterRange {
                min: -24.0,
                max: 24.0,
                default: 0.0,
            },
            unit: "dB",
        }),
        4 => Some(ParameterInfo {
            id,
            name: NAMES[band_idx as usize][4],
            param_type: ParameterType::Continuous,
            range: ParameterRange {
                min: 0.1,
                max: 20.0,
                default: std::f32::consts::FRAC_1_SQRT_2,
            },
            unit: "Q",
        }),
        5 => Some(ParameterInfo {
            id,
            name: NAMES[band_idx as usize][5],
            param_type: ParameterType::Enum { count: 4 },
            range: ParameterRange {
                min: 0.0,
                max: 3.0,
                default: 1.0,
            },
            unit: "dB/oct",
        }),
        7 => {
            static ROUTING_NAMES: [&str; 8] = [
                "band[0].routing",
                "band[1].routing",
                "band[2].routing",
                "band[3].routing",
                "band[4].routing",
                "band[5].routing",
                "band[6].routing",
                "band[7].routing",
            ];
            Some(ParameterInfo {
                id,
                name: ROUTING_NAMES[band_idx as usize],
                param_type: ParameterType::Enum { count: 5 },
                range: ParameterRange {
                    min: 0.0,
                    max: 4.0,
                    default: 0.0,
                },
                unit: "",
            })
        }
        _ => None,
    }
}

// ===========================================================================
// ParameterError
// ===========================================================================

/// Error returned by parameter validation.
#[derive(Clone, Debug, PartialEq)]
pub enum ParameterError {
    /// Value is below the minimum.
    OutOfRangeBelow { id: u32, value: f32, min: f32 },
    /// Value is above the maximum.
    OutOfRangeAbove { id: u32, value: f32, max: f32 },
    /// Invalid parameter ID.
    InvalidId(u32),
}

impl fmt::Display for ParameterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParameterError::OutOfRangeBelow { id, value, min } => {
                write!(f, "Parameter({id}) value {value} below minimum {min}")
            }
            ParameterError::OutOfRangeAbove { id, value, max } => {
                write!(f, "Parameter({id}) value {value} above maximum {max}")
            }
            ParameterError::InvalidId(id) => {
                write!(f, "Unknown parameter ID {id}")
            }
        }
    }
}

impl std::error::Error for ParameterError {}

// ===========================================================================
// Validation functions
// ===========================================================================

/// Validate a parameter value against its contract range.
///
/// Returns `Ok(value)` if the value is within range, or an error describing
/// the violation. Invalid IDs are reported as `InvalidId`.
pub fn validate_value(id: u32, value: f32) -> Result<f32, ParameterError> {
    let info = parameter_info(id).ok_or(ParameterError::InvalidId(id))?;
    if value < info.range.min {
        return Err(ParameterError::OutOfRangeBelow {
            id,
            value,
            min: info.range.min,
        });
    }
    if value > info.range.max {
        return Err(ParameterError::OutOfRangeAbove {
            id,
            value,
            max: info.range.max,
        });
    }
    Ok(value)
}

/// Clamp a parameter value to its contract range.
///
/// For unknown IDs, returns the value unchanged (no clamping).
pub fn clamp_value(id: u32, value: f32) -> f32 {
    parameter_info(id)
        .map(|i| i.range.clamp(value))
        .unwrap_or(value)
}

// ===========================================================================
// FilterType
// ===========================================================================

/// Filter type for EQ bands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FilterType {
    Bell,
    LowShelf,
    HighShelf,
    HighPass,
    LowPass,
}

impl FilterType {
    /// Try to create from a raw value (0–4).
    pub const fn from_raw(value: u32) -> Option<Self> {
        match value {
            0 => Some(FilterType::Bell),
            1 => Some(FilterType::LowShelf),
            2 => Some(FilterType::HighShelf),
            3 => Some(FilterType::HighPass),
            4 => Some(FilterType::LowPass),
            _ => None,
        }
    }

    /// Convert to raw value.
    pub const fn to_raw(self) -> u32 {
        match self {
            FilterType::Bell => 0,
            FilterType::LowShelf => 1,
            FilterType::HighShelf => 2,
            FilterType::HighPass => 3,
            FilterType::LowPass => 4,
        }
    }
}

// ===========================================================================
// Slope
// ===========================================================================

/// Slope steepness for shelf and pass filters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Slope {
    Db6,
    Db12,
    Db18,
    Db24,
}

impl Slope {
    /// Try to create from a raw value (0–3).
    pub const fn from_raw(value: u32) -> Option<Self> {
        match value {
            0 => Some(Slope::Db6),
            1 => Some(Slope::Db12),
            2 => Some(Slope::Db18),
            3 => Some(Slope::Db24),
            _ => None,
        }
    }

    /// Convert to raw value.
    pub const fn to_raw(self) -> u32 {
        match self {
            Slope::Db6 => 0,
            Slope::Db12 => 1,
            Slope::Db18 => 2,
            Slope::Db24 => 3,
        }
    }
}

// ===========================================================================
// StereoMode
// ===========================================================================

/// Stereo processing mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StereoMode {
    Stereo,
    MidSide,
}

impl StereoMode {
    pub const fn from_raw(value: u32) -> Option<Self> {
        match value {
            0 => Some(StereoMode::Stereo),
            1 => Some(StereoMode::MidSide),
            _ => None,
        }
    }

    pub const fn to_raw(self) -> u32 {
        match self {
            StereoMode::Stereo => 0,
            StereoMode::MidSide => 1,
        }
    }
}

/// Per-band stereo channel routing placement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BandRouting {
    /// Process both Left and Right channels identically.
    #[default]
    Stereo = 0,
    /// Process Left channel only.
    Left = 1,
    /// Process Right channel only.
    Right = 2,
    /// Process Mid (sum) component only.
    Mid = 3,
    /// Process Side (difference) component only.
    Side = 4,
}

impl BandRouting {
    /// Try to create from a raw enum value (0–4).
    pub const fn from_raw(value: u32) -> Option<Self> {
        match value {
            0 => Some(BandRouting::Stereo),
            1 => Some(BandRouting::Left),
            2 => Some(BandRouting::Right),
            3 => Some(BandRouting::Mid),
            4 => Some(BandRouting::Side),
            _ => None,
        }
    }

    /// Convert to raw enum value (0–4).
    pub const fn to_raw(self) -> u32 {
        match self {
            BandRouting::Stereo => 0,
            BandRouting::Left => 1,
            BandRouting::Right => 2,
            BandRouting::Mid => 3,
            BandRouting::Side => 4,
        }
    }

    /// Compact label for band strip UI buttons.
    pub const fn short_label(self) -> &'static str {
        match self {
            BandRouting::Stereo => "ST",
            BandRouting::Left => "L",
            BandRouting::Right => "R",
            BandRouting::Mid => "M",
            BandRouting::Side => "S",
        }
    }

    /// Human-readable English name for DAW parameter display.
    pub const fn display_name(self) -> &'static str {
        match self {
            BandRouting::Stereo => "Stereo",
            BandRouting::Left => "Left",
            BandRouting::Right => "Right",
            BandRouting::Mid => "Mid",
            BandRouting::Side => "Side",
        }
    }

    /// Cycle to the next routing placement (`Stereo -> Left -> Right -> Mid -> Side -> Stereo`).
    pub const fn next(self) -> Self {
        match self {
            BandRouting::Stereo => BandRouting::Left,
            BandRouting::Left => BandRouting::Right,
            BandRouting::Right => BandRouting::Mid,
            BandRouting::Mid => BandRouting::Side,
            BandRouting::Side => BandRouting::Stereo,
        }
    }
}

// ===========================================================================
// ProcessingMode
// ===========================================================================

/// Processing mode for the EQ.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProcessingMode {
    MinimumPhase,
}

impl ProcessingMode {
    pub const fn from_raw(_value: u32) -> Option<Self> {
        Some(ProcessingMode::MinimumPhase)
    }

    pub const fn to_raw(self) -> u32 {
        0
    }
}

// ===========================================================================
// BandParameters
// ===========================================================================

/// Parameters for a single EQ band.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BandParameters {
    /// Whether this band is active.
    pub enabled: bool,
    /// Filter type.
    pub filter_type: FilterType,
    /// Center frequency in Hz.
    pub frequency_hz: f32,
    /// Gain in dB.
    pub gain_db: f32,
    /// Q factor.
    pub q: f32,
    /// Slope steepness.
    pub slope: Slope,
}

impl BandParameters {
    /// Returns the parameter ID for this band's gain value.
    pub fn gain_param_id(&self) -> ParamId {
        let band_idx = self.band_index();
        ParamId(BAND_ID_MIN + (band_idx as u32 * 100) + 3)
    }

    /// Returns the parameter ID for this band's frequency value.
    pub fn frequency_param_id(&self) -> ParamId {
        let band_idx = self.band_index();
        ParamId(BAND_ID_MIN + (band_idx as u32 * 100) + 2)
    }

    /// Returns the band index (0–9) for this parameter set.
    pub fn band_index(&self) -> usize {
        0 // Default; must be set by caller using band position.
    }

    /// Returns the minimum gain range for automation mapping.
    pub const fn gain_db_min() -> f32 {
        -24.0
    }

    /// Returns the maximum gain range for automation mapping.
    pub const fn gain_db_max() -> f32 {
        24.0
    }

    /// Returns the minimum frequency for automation mapping.
    pub const fn freq_min() -> f32 {
        20.0
    }

    /// Returns the maximum frequency for automation mapping.
    pub const fn freq_max() -> f32 {
        20_000.0
    }
}

impl Default for BandParameters {
    fn default() -> Self {
        BandParameters {
            enabled: true,
            filter_type: FilterType::Bell,
            frequency_hz: 1000.0,
            gain_db: 0.0,
            q: std::f32::consts::FRAC_1_SQRT_2,
            slope: Slope::Db12,
        }
    }
}

// ===========================================================================
// GlobalParameters
// ===========================================================================

/// Auto-gain compensation strategy.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum AutoGainMode {
    /// Matches perceived loudness across bypass (ideal for DAW mixing/mastering).
    #[default]
    Perceptual = 0,
    /// Caps output gain so EQ boosts never push peaks above 0 dBFS relative to input.
    HeadroomSafe = 1,
}

impl AutoGainMode {
    /// Convert to its stable serialized integer value.
    #[inline]
    pub const fn to_raw(self) -> u32 {
        self as u32
    }

    /// Parse from a serialized integer value.
    #[inline]
    pub const fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::Perceptual),
            1 => Some(Self::HeadroomSafe),
            _ => None,
        }
    }
}

/// Global EQ parameters.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GlobalParameters {
    /// Bypass the entire EQ.
    pub bypass: bool,
    /// Output gain in dB.
    pub output_gain_db: f32,
    /// Enable auto-gain compensation.
    pub auto_gain: bool,
    /// Auto-gain compensation mode (perceptual loudness vs anti-clip headroom protection).
    #[serde(default)]
    pub auto_gain_mode: AutoGainMode,
    /// Processing mode.
    pub processing_mode: ProcessingMode,
    /// Stereo mode.
    pub stereo_mode: StereoMode,
    /// Monitor-only bitmask of soloed bands (bit `i` = band `i`, excluded from persistence).
    #[serde(skip)]
    pub solo_mask: u8,
    /// Per-band stereo routing placement (defaults to `Stereo` for backward compatibility).
    #[serde(default)]
    pub band_routing: [BandRouting; MAX_BANDS],
}

impl Default for GlobalParameters {
    fn default() -> Self {
        GlobalParameters {
            bypass: false,
            output_gain_db: 0.0,
            auto_gain: false,
            auto_gain_mode: AutoGainMode::Perceptual,
            processing_mode: ProcessingMode::MinimumPhase,
            stereo_mode: StereoMode::Stereo,
            solo_mask: 0,
            band_routing: [BandRouting::Stereo; MAX_BANDS],
        }
    }
}

// ===========================================================================
// EqParameters
// ===========================================================================

/// Complete EQ parameter set.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EqParameters {
    /// Global parameters.
    pub global: GlobalParameters,
    /// Per-band parameters (8 bands).
    pub bands: [BandParameters; MAX_BANDS],
}

impl EqParameters {
    /// Returns `true` if the band at `band_idx` is currently soloed.
    #[inline]
    pub const fn is_band_solo(&self, band_idx: usize) -> bool {
        band_idx < MAX_BANDS && (self.global.solo_mask & (1u8 << band_idx)) != 0
    }

    /// Sets or clears the monitor-only solo state for `band_idx`.
    #[inline]
    pub fn set_band_solo(&mut self, band_idx: usize, solo: bool) {
        if band_idx < MAX_BANDS {
            let bit = 1u8 << band_idx;
            if solo {
                self.global.solo_mask |= bit;
            } else {
                self.global.solo_mask &= !bit;
            }
        }
    }

    /// Returns `true` if at least one band is currently soloed.
    #[inline]
    pub const fn any_band_solo(&self) -> bool {
        self.global.solo_mask != 0
    }

    /// Clears solo state across all bands.
    #[inline]
    pub fn clear_solo(&mut self) {
        self.global.solo_mask = 0;
    }

    /// Returns the stereo routing placement for `band_idx` (defaults to `Stereo` if out of range).
    #[inline]
    pub const fn band_routing(&self, band_idx: usize) -> BandRouting {
        if band_idx < MAX_BANDS {
            self.global.band_routing[band_idx]
        } else {
            BandRouting::Stereo
        }
    }

    /// Sets the stereo routing placement for `band_idx`.
    #[inline]
    pub fn set_band_routing(&mut self, band_idx: usize, routing: BandRouting) {
        if band_idx < MAX_BANDS {
            self.global.band_routing[band_idx] = routing;
        }
    }
}

impl Default for EqParameters {
    fn default() -> Self {
        EqParameters {
            global: GlobalParameters::default(),
            bands: [BandParameters::default(); MAX_BANDS],
        }
    }
}

// ===========================================================================
// ParameterCommand
// ===========================================================================

/// Parameter command from producer to consumer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParameterCommand {
    /// Set a single parameter value.
    SetParameter { id: ParamId, value: f32 },
    /// Set all parameters at once.
    SetAll(EqParameters),
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // ParamId category tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_param_id_category_global() {
        assert_eq!(ParamId(1).category(), ParamCategory::Global);
        assert_eq!(ParamId(50).category(), ParamCategory::Global);
        assert_eq!(ParamId(99).category(), ParamCategory::Global);
    }

    #[test]
    fn test_param_id_category_band() {
        assert_eq!(ParamId(100).category(), ParamCategory::Band);
        assert_eq!(ParamId(150).category(), ParamCategory::Band);
        assert_eq!(ParamId(899).category(), ParamCategory::Band);
    }

    #[test]
    fn test_param_id_category_reserved() {
        assert_eq!(ParamId(900).category(), ParamCategory::Reserved);
        assert_eq!(ParamId(950).category(), ParamCategory::Reserved);
        assert_eq!(ParamId(999).category(), ParamCategory::Reserved);
    }

    #[test]
    fn test_param_id_category_future() {
        assert_eq!(ParamId(1000).category(), ParamCategory::Future);
        assert_eq!(ParamId(2000).category(), ParamCategory::Future);
        assert_eq!(ParamId(u32::MAX).category(), ParamCategory::Future);
    }

    #[test]
    fn test_param_id_category_invalid() {
        assert_eq!(ParamId(0).category(), ParamCategory::Invalid);
    }

    // -----------------------------------------------------------------------
    // ParamId band_index / band_offset tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_param_id_band_index() {
        assert_eq!(ParamId(100).band_index(), Some(0));
        assert_eq!(ParamId(199).band_index(), Some(0));
        assert_eq!(ParamId(200).band_index(), Some(1));
        assert_eq!(ParamId(800).band_index(), Some(7));
        assert_eq!(ParamId(899).band_index(), Some(7));
        assert_eq!(ParamId(1).band_index(), None);
        assert_eq!(ParamId(0).band_index(), None);
    }

    #[test]
    fn test_param_id_band_offset() {
        assert_eq!(ParamId(100).band_offset(), Some(0));
        assert_eq!(ParamId(101).band_offset(), Some(1));
        assert_eq!(ParamId(105).band_offset(), Some(5));
        assert_eq!(ParamId(199).band_offset(), Some(99));
        assert_eq!(ParamId(200).band_offset(), Some(0));
        assert_eq!(ParamId(1).band_offset(), None);
    }

    // -----------------------------------------------------------------------
    // ParameterRange tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_range_clamp_in_range() {
        let range = ParameterRange {
            min: 0.0,
            max: 1.0,
            default: 0.5,
        };
        assert_eq!(range.clamp(0.5), 0.5);
        assert_eq!(range.clamp(0.0), 0.0);
        assert_eq!(range.clamp(1.0), 1.0);
    }

    #[test]
    fn test_range_clamp_below_min() {
        let range = ParameterRange {
            min: 0.0,
            max: 1.0,
            default: 0.5,
        };
        assert_eq!(range.clamp(-1.0), 0.0);
        assert_eq!(range.clamp(f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn test_range_clamp_above_max() {
        let range = ParameterRange {
            min: 0.0,
            max: 1.0,
            default: 0.5,
        };
        assert_eq!(range.clamp(2.0), 1.0);
        assert_eq!(range.clamp(f32::INFINITY), 1.0);
    }

    #[test]
    fn test_range_validate() {
        let range = ParameterRange {
            min: 0.0,
            max: 1.0,
            default: 0.5,
        };
        assert!(range.validate(0.0));
        assert!(range.validate(0.5));
        assert!(range.validate(1.0));
        assert!(!range.validate(-0.001));
        assert!(!range.validate(1.001));
    }

    // -----------------------------------------------------------------------
    // parameter_info tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parameter_info_global() {
        let info = parameter_info(1).unwrap();
        assert_eq!(info.id, 1);
        assert_eq!(info.name, "global.bypass");
        assert_eq!(info.range.min, 0.0);
        assert_eq!(info.range.max, 1.0);
        assert_eq!(info.range.default, 0.0);

        let info = parameter_info(2).unwrap();
        assert_eq!(info.id, 2);
        assert_eq!(info.name, "global.output_gain_db");
        assert_eq!(info.range.min, -24.0);
        assert_eq!(info.range.max, 24.0);
        assert_eq!(info.range.default, 0.0);
        assert_eq!(info.unit, "dB");
    }

    #[test]
    fn test_parameter_info_band() {
        let info = parameter_info(100).unwrap();
        assert_eq!(info.id, 100);
        assert_eq!(info.name, "band[0].enabled");
        assert_eq!(info.range.default, 1.0);

        let info = parameter_info(805).unwrap();
        assert_eq!(info.id, 805);
        assert_eq!(info.name, "band[7].slope");
        assert_eq!(info.range.min, 0.0);
        assert_eq!(info.range.max, 3.0);
    }

    #[test]
    fn test_parameter_info_reserved() {
        assert!(parameter_info(900).is_none());
        assert!(parameter_info(999).is_none());
    }

    #[test]
    fn test_parameter_info_future() {
        assert!(parameter_info(1000).is_none());
        assert!(parameter_info(u32::MAX).is_none());
    }

    #[test]
    fn test_parameter_info_invalid() {
        assert!(parameter_info(0).is_none());
    }

    #[test]
    fn test_parameter_info_all_known_ids() {
        // Verify all 53 known IDs return Some
        for id in 1..=5 {
            assert!(
                parameter_info(id).is_some(),
                "Global ID {id} should be known"
            );
        }
        for band in 0..8 {
            for offset in 0..6 {
                let id = 100 + band * 100 + offset;
                assert!(parameter_info(id).is_some(), "Band ID {id} should be known");
            }
        }
    }

    #[test]
    fn test_parameter_info_all_unknown_ids() {
        // Verify known unknown IDs return None
        assert!(parameter_info(0).is_none());
        assert!(parameter_info(900).is_none());
        assert!(parameter_info(999).is_none());
        assert!(parameter_info(1000).is_none());
        // Verify a few random IDs in the band range that are reserved
        assert!(parameter_info(106).is_none()); // band[0] reserved
        assert!(parameter_info(199).is_none()); // band[0] reserved
        assert!(parameter_info(250).is_none()); // band[1] reserved
    }

    // -----------------------------------------------------------------------
    // validate_value tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_value_valid() {
        assert!(validate_value(1, 0.0).is_ok());
        assert!(validate_value(1, 1.0).is_ok());
        assert!(validate_value(2, 0.0).is_ok());
        assert!(validate_value(2, 24.0).is_ok());
        assert!(validate_value(2, -24.0).is_ok());
    }

    #[test]
    fn test_validate_value_below_min() {
        let err = validate_value(1, -0.001).unwrap_err();
        match err {
            ParameterError::OutOfRangeBelow { id, value, min } => {
                assert_eq!(id, 1);
                assert_eq!(min, 0.0);
                assert!(value < 0.0);
            }
            _ => panic!("expected OutOfRangeBelow"),
        }
    }

    #[test]
    fn test_validate_value_above_max() {
        let err = validate_value(2, 24.001).unwrap_err();
        match err {
            ParameterError::OutOfRangeAbove { id, value, max } => {
                assert_eq!(id, 2);
                assert_eq!(max, 24.0);
                assert!(value > 24.0);
            }
            _ => panic!("expected OutOfRangeAbove"),
        }
    }

    #[test]
    fn test_validate_value_invalid_id() {
        let err = validate_value(900, 0.5).unwrap_err();
        match err {
            ParameterError::InvalidId(id) => assert_eq!(id, 900),
            _ => panic!("expected InvalidId"),
        }
    }

    #[test]
    fn test_validate_value_boundary_global() {
        // Global bypass: 0..1
        assert!(validate_value(1, 0.0).is_ok());
        assert!(validate_value(1, 1.0).is_ok());
        assert!(validate_value(1, -0.0001).is_err());
        assert!(validate_value(1, 1.0001).is_err());

        // Global output_gain_db: -24..24
        assert!(validate_value(2, -24.0).is_ok());
        assert!(validate_value(2, 24.0).is_ok());
        assert!(validate_value(2, -24.001).is_err());
        assert!(validate_value(2, 24.001).is_err());
    }

    #[test]
    fn test_validate_value_boundary_band() {
        // Band enabled: 0..1
        assert!(validate_value(100, 0.0).is_ok());
        assert!(validate_value(100, 1.0).is_ok());
        assert!(validate_value(100, -0.0001).is_err());

        // Band frequency_hz: 20..20000
        assert!(validate_value(102, 20.0).is_ok());
        assert!(validate_value(102, 20000.0).is_ok());
        assert!(validate_value(102, 19.999).is_err());
        assert!(validate_value(102, 20000.001).is_err());

        // Band q: 0.1..20
        assert!(validate_value(104, 0.1).is_ok());
        assert!(validate_value(104, 20.0).is_ok());
        assert!(validate_value(104, 0.099).is_err());
        assert!(validate_value(104, 20.001).is_err());

        // Band gain_db: -24..24
        assert!(validate_value(103, -24.0).is_ok());
        assert!(validate_value(103, 24.0).is_ok());
        assert!(validate_value(103, -24.001).is_err());
    }

    // -----------------------------------------------------------------------
    // clamp_value tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_clamp_value_in_range() {
        assert_eq!(clamp_value(1, 0.5), 0.5);
        assert_eq!(clamp_value(2, 12.0), 12.0);
    }

    #[test]
    fn test_clamp_value_below_min() {
        assert_eq!(clamp_value(1, -5.0), 0.0);
        assert_eq!(clamp_value(2, -100.0), -24.0);
        assert_eq!(clamp_value(102, 0.0), 20.0);
    }

    #[test]
    fn test_clamp_value_above_max() {
        assert_eq!(clamp_value(1, 5.0), 1.0);
        assert_eq!(clamp_value(2, 100.0), 24.0);
        assert_eq!(clamp_value(102, 50000.0), 20000.0);
    }

    #[test]
    fn test_clamp_value_unknown_id() {
        // Unknown IDs should return the value unchanged
        assert_eq!(clamp_value(900, 0.5), 0.5);
        assert_eq!(clamp_value(1000, -999.0), -999.0);
    }

    // -----------------------------------------------------------------------
    // FilterType / Slope enum tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_filter_type_roundtrip() {
        for raw in 0..=4 {
            let ft = FilterType::from_raw(raw).unwrap();
            assert_eq!(ft.to_raw(), raw);
        }
        assert!(FilterType::from_raw(5).is_none());
        assert!(FilterType::from_raw(u32::MAX).is_none());
    }

    #[test]
    fn test_slope_roundtrip() {
        for raw in 0..=3 {
            let s = Slope::from_raw(raw).unwrap();
            assert_eq!(s.to_raw(), raw);
        }
        assert!(Slope::from_raw(4).is_none());
    }

    #[test]
    fn test_processing_mode_roundtrip() {
        let pm = ProcessingMode::from_raw(0).unwrap();
        assert_eq!(pm.to_raw(), 0);
    }

    #[test]
    fn test_stereo_mode_roundtrip() {
        let sm = StereoMode::from_raw(0).unwrap();
        assert_eq!(sm.to_raw(), 0);
    }

    // -----------------------------------------------------------------------
    // BandParameters / GlobalParameters / EqParameters tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_band_parameters_default() {
        let bp = BandParameters::default();
        assert!(bp.enabled);
        assert_eq!(bp.filter_type, FilterType::Bell);
        assert_eq!(bp.frequency_hz, 1000.0);
        assert_eq!(bp.gain_db, 0.0);
        assert_eq!(bp.q, std::f32::consts::FRAC_1_SQRT_2);
        assert_eq!(bp.slope, Slope::Db12);
    }

    #[test]
    fn test_global_parameters_default() {
        let gp = GlobalParameters::default();
        assert!(!gp.bypass);
        assert_eq!(gp.output_gain_db, 0.0);
        assert!(!gp.auto_gain);
        assert_eq!(gp.processing_mode, ProcessingMode::MinimumPhase);
        assert_eq!(gp.stereo_mode, StereoMode::Stereo);
    }

    #[test]
    fn test_eq_parameters_default() {
        let eq = EqParameters::default();
        assert!(!eq.global.bypass);
        assert_eq!(eq.bands.len(), MAX_BANDS);
        for band in &eq.bands {
            assert!(band.enabled);
        }
    }

    #[test]
    fn test_eq_parameters_copy() {
        let eq1 = EqParameters::default();
        let eq2 = eq1;
        assert_eq!(eq1, eq2);
    }

    // -----------------------------------------------------------------------
    // ParamId Display test
    // -----------------------------------------------------------------------

    #[test]
    fn test_param_id_display() {
        assert_eq!(format!("{}", ParamId(42)), "ParamId(42)");
    }

    // -----------------------------------------------------------------------
    // ParameterError Display test
    // -----------------------------------------------------------------------

    #[test]
    fn test_parameter_error_display() {
        let err = validate_value(1, -0.001).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Parameter(1)"));
        assert!(msg.contains("below minimum"));
    }

    // -----------------------------------------------------------------------
    // Exhaustive band parameter boundary tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_band_parameter_boundaries_all_bands() {
        // Test each band's enabled parameter (0..1)
        for band in 0..8 {
            let id = 100 + band * 100;
            assert!(validate_value(id, 0.0).is_ok());
            assert!(validate_value(id, 1.0).is_ok());
            assert!(validate_value(id, -0.001).is_err());
            assert!(validate_value(id, 1.001).is_err());
        }

        // Test each band's frequency_hz parameter (20..20000)
        for band in 0..8 {
            let id = 100 + band * 100 + 2;
            assert!(validate_value(id, 20.0).is_ok());
            assert!(validate_value(id, 20000.0).is_ok());
            assert!(validate_value(id, 19.999).is_err());
            assert!(validate_value(id, 20000.001).is_err());
        }

        // Test each band's gain_db parameter (-24..24)
        for band in 0..8 {
            let id = 100 + band * 100 + 3;
            assert!(validate_value(id, -24.0).is_ok());
            assert!(validate_value(id, 24.0).is_ok());
            assert!(validate_value(id, -24.001).is_err());
            assert!(validate_value(id, 24.001).is_err());
        }

        // Test each band's q parameter (0.1..20)
        for band in 0..8 {
            let id = 100 + band * 100 + 4;
            assert!(validate_value(id, 0.1).is_ok());
            assert!(validate_value(id, 20.0).is_ok());
            assert!(validate_value(id, 0.099).is_err());
            assert!(validate_value(id, 20.001).is_err());
        }

        // Test each band's filter_type parameter (0..4)
        for band in 0..8 {
            let id = 100 + band * 100 + 1;
            assert!(validate_value(id, 0.0).is_ok());
            assert!(validate_value(id, 4.0).is_ok());
            assert!(validate_value(id, -0.001).is_err());
            assert!(validate_value(id, 4.001).is_err());
        }

        // Test each band's slope parameter (0..3)
        for band in 0..8 {
            let id = 100 + band * 100 + 5;
            assert!(validate_value(id, 0.0).is_ok());
            assert!(validate_value(id, 3.0).is_ok());
            assert!(validate_value(id, -0.001).is_err());
            assert!(validate_value(id, 3.001).is_err());
        }
    }

    #[test]
    fn test_global_parameter_boundaries() {
        // bypass: 0..1
        assert!(validate_value(1, 0.0).is_ok());
        assert!(validate_value(1, 1.0).is_ok());
        assert!(validate_value(1, -0.001).is_err());
        assert!(validate_value(1, 1.001).is_err());

        // output_gain_db: -24..24
        assert!(validate_value(2, -24.0).is_ok());
        assert!(validate_value(2, 24.0).is_ok());
        assert!(validate_value(2, -24.001).is_err());
        assert!(validate_value(2, 24.001).is_err());

        // auto_gain: 0..1
        assert!(validate_value(3, 0.0).is_ok());
        assert!(validate_value(3, 1.0).is_ok());
        assert!(validate_value(3, -0.001).is_err());

        // processing_mode: 0..0 (single value)
        assert!(validate_value(4, 0.0).is_ok());
        assert!(validate_value(4, 0.001).is_err());

        // stereo_mode: 0..0 (single value)
        assert!(validate_value(5, 0.0).is_ok());
        assert!(validate_value(5, 0.001).is_err());
    }

    #[test]
    fn test_invalid_id_validation() {
        // ID 0
        assert!(matches!(
            validate_value(0, 0.0),
            Err(ParameterError::InvalidId(0))
        ));
        // Reserved IDs
        assert!(matches!(
            validate_value(900, 0.0),
            Err(ParameterError::InvalidId(900))
        ));
        assert!(matches!(
            validate_value(999, 0.0),
            Err(ParameterError::InvalidId(999))
        ));
        // Future IDs
        assert!(matches!(
            validate_value(1000, 0.0),
            Err(ParameterError::InvalidId(1000))
        ));
        assert!(matches!(
            validate_value(u32::MAX, 0.0),
            Err(ParameterError::InvalidId(u32::MAX))
        ));
    }

    #[test]
    fn test_clamp_preserves_valid_values() {
        // Values within range should be unchanged
        assert_eq!(clamp_value(1, 0.0), 0.0);
        assert_eq!(clamp_value(1, 1.0), 1.0);
        assert_eq!(clamp_value(2, 0.0), 0.0);
        assert_eq!(clamp_value(2, 12.0), 12.0);
        assert_eq!(clamp_value(2, -12.0), -12.0);
        assert_eq!(clamp_value(102, 1000.0), 1000.0);
        assert_eq!(
            clamp_value(104, std::f32::consts::FRAC_1_SQRT_2),
            std::f32::consts::FRAC_1_SQRT_2
        );
    }
}
