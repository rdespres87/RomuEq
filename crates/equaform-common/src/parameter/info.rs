//! Parameter metadata lookup, range definitions, and validation helpers.

use std::fmt;

use super::ids::MAX_BANDS;

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

struct BandParamSpec {
    sub: u32,
    param_type: ParameterType,
    range: ParameterRange,
    unit: &'static str,
    names: [&'static str; MAX_BANDS],
}

const GLOBAL_INFOS: [ParameterInfo; 6] = [
    ParameterInfo {
        id: 1,
        name: "global.bypass",
        param_type: ParameterType::Bool,
        range: ParameterRange {
            min: 0.0,
            max: 1.0,
            default: 0.0,
        },
        unit: "",
    },
    ParameterInfo {
        id: 2,
        name: "global.output_gain_db",
        param_type: ParameterType::Continuous,
        range: ParameterRange {
            min: -24.0,
            max: 24.0,
            default: 0.0,
        },
        unit: "dB",
    },
    ParameterInfo {
        id: 3,
        name: "global.auto_gain",
        param_type: ParameterType::Bool,
        range: ParameterRange {
            min: 0.0,
            max: 1.0,
            default: 0.0,
        },
        unit: "",
    },
    ParameterInfo {
        id: 4,
        name: "global.processing_mode",
        param_type: ParameterType::Enum { count: 3 },
        range: ParameterRange {
            min: 0.0,
            max: 2.0,
            default: 0.0,
        },
        unit: "",
    },
    ParameterInfo {
        id: 5,
        name: "global.stereo_mode",
        param_type: ParameterType::Enum { count: 1 },
        range: ParameterRange {
            min: 0.0,
            max: 0.0,
            default: 0.0,
        },
        unit: "",
    },
    ParameterInfo {
        id: 7,
        name: "global.auto_gain_mode",
        param_type: ParameterType::Enum { count: 2 },
        range: ParameterRange {
            min: 0.0,
            max: 1.0,
            default: 0.0,
        },
        unit: "",
    },
];

const BAND_SPECS: [BandParamSpec; 7] = [
    BandParamSpec {
        sub: 0,
        param_type: ParameterType::Bool,
        range: ParameterRange {
            min: 0.0,
            max: 1.0,
            default: 1.0,
        },
        unit: "",
        names: [
            "band[0].enabled",
            "band[1].enabled",
            "band[2].enabled",
            "band[3].enabled",
            "band[4].enabled",
            "band[5].enabled",
            "band[6].enabled",
            "band[7].enabled",
        ],
    },
    BandParamSpec {
        sub: 1,
        param_type: ParameterType::Enum { count: 5 },
        range: ParameterRange {
            min: 0.0,
            max: 4.0,
            default: 0.0,
        },
        unit: "",
        names: [
            "band[0].filter_type",
            "band[1].filter_type",
            "band[2].filter_type",
            "band[3].filter_type",
            "band[4].filter_type",
            "band[5].filter_type",
            "band[6].filter_type",
            "band[7].filter_type",
        ],
    },
    BandParamSpec {
        sub: 2,
        param_type: ParameterType::Continuous,
        range: ParameterRange {
            min: 20.0,
            max: 20000.0,
            default: 1000.0,
        },
        unit: "Hz",
        names: [
            "band[0].frequency_hz",
            "band[1].frequency_hz",
            "band[2].frequency_hz",
            "band[3].frequency_hz",
            "band[4].frequency_hz",
            "band[5].frequency_hz",
            "band[6].frequency_hz",
            "band[7].frequency_hz",
        ],
    },
    BandParamSpec {
        sub: 3,
        param_type: ParameterType::Continuous,
        range: ParameterRange {
            min: -24.0,
            max: 24.0,
            default: 0.0,
        },
        unit: "dB",
        names: [
            "band[0].gain_db",
            "band[1].gain_db",
            "band[2].gain_db",
            "band[3].gain_db",
            "band[4].gain_db",
            "band[5].gain_db",
            "band[6].gain_db",
            "band[7].gain_db",
        ],
    },
    BandParamSpec {
        sub: 4,
        param_type: ParameterType::Continuous,
        range: ParameterRange {
            min: 0.1,
            max: 20.0,
            default: std::f32::consts::FRAC_1_SQRT_2,
        },
        unit: "Q",
        names: [
            "band[0].q",
            "band[1].q",
            "band[2].q",
            "band[3].q",
            "band[4].q",
            "band[5].q",
            "band[6].q",
            "band[7].q",
        ],
    },
    BandParamSpec {
        sub: 5,
        param_type: ParameterType::Enum { count: 4 },
        range: ParameterRange {
            min: 0.0,
            max: 3.0,
            default: 1.0,
        },
        unit: "dB/oct",
        names: [
            "band[0].slope",
            "band[1].slope",
            "band[2].slope",
            "band[3].slope",
            "band[4].slope",
            "band[5].slope",
            "band[6].slope",
            "band[7].slope",
        ],
    },
    BandParamSpec {
        sub: 7,
        param_type: ParameterType::Enum { count: 5 },
        range: ParameterRange {
            min: 0.0,
            max: 4.0,
            default: 0.0,
        },
        unit: "",
        names: [
            "band[0].routing",
            "band[1].routing",
            "band[2].routing",
            "band[3].routing",
            "band[4].routing",
            "band[5].routing",
            "band[6].routing",
            "band[7].routing",
        ],
    },
];

/// Returns the `ParameterInfo` for a given ID, or `None` if the ID is not
/// a known parameter.
pub fn parameter_info(id: u32) -> Option<ParameterInfo> {
    GLOBAL_INFOS
        .iter()
        .copied()
        .find(|info| info.id == id)
        .or_else(|| band_parameter_info(id))
}

/// Resolves `ParameterInfo` for band parameters (IDs 100..899) via offset mapping.
fn band_parameter_info(id: u32) -> Option<ParameterInfo> {
    let band_idx = (id / 100).checked_sub(1)? as usize;
    if band_idx >= MAX_BANDS {
        return None;
    }
    let sub = id % 100;
    let spec = BAND_SPECS.iter().find(|s| s.sub == sub)?;
    Some(ParameterInfo {
        id,
        name: spec.names[band_idx],
        param_type: spec.param_type,
        range: spec.range,
        unit: spec.unit,
    })
}

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
