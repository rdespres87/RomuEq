//! Stable parameter definitions and ID mapping for RomuEq CLAP plugin.
//!
//! Provides 51 stable automatable parameters aligned with `romueq_common::parameter`:
//! - ID 1: Bypass (Bool)
//! - ID 2: Output Gain dB (-24.0 .. +24.0)
//! - ID 3: Auto-Gain (Bool)
//! - ID 100 + i*100: Band i Enabled (Bool)
//! - ID 101 + i*100: Band i FilterType (Enum: 0=Bell, 1=LowShelf, 2=HighShelf, 3=HighPass, 4=LowPass)
//! - ID 102 + i*100: Band i Frequency Hz (20.0 .. 20000.0)
//! - ID 103 + i*100: Band i Gain dB (-24.0 .. +24.0)
//! - ID 104 + i*100: Band i Q (0.1 .. 20.0)
//! - ID 105 + i*100: Band i Slope (Enum: 0=6dB/o, 1=12dB/o, 2=18dB/o, 3=24dB/o)

use clap_sys::ext::params::*;
use clap_sys::id::clap_id;
use romueq_common::{EqParameters, FilterType, GlobalParameters, MAX_BANDS, Slope};

/// Total count of exposed automatable parameters (3 global + 8 * 6 band params = 51).
pub const PARAM_COUNT: u32 = 3 + (MAX_BANDS as u32 * 6);
pub const PARAM_COUNT_USIZE: usize = PARAM_COUNT as usize;

#[derive(Clone, Copy, Debug)]
pub struct ParamMeta {
    pub id: clap_id,
    pub name: &'static str,
    pub module: &'static str,
    pub min_value: f64,
    pub max_value: f64,
    pub default_value: f64,
    pub flags: clap_param_info_flags,
}

/// Maps a CLAP parameter ID to its flat atomic slot index `[0..PARAM_COUNT_USIZE)`.
#[inline]
pub const fn param_id_to_slot(id: clap_id) -> Option<usize> {
    match id {
        1 => Some(0),
        2 => Some(1),
        3 => Some(2),
        100..=899 => {
            let offset = id - 100;
            let band = offset / 100;
            let field = offset % 100;
            if (band as usize) < MAX_BANDS && field < 6 {
                Some(3 + (band as usize) * 6 + (field as usize))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Maps a flat atomic slot index `[0..PARAM_COUNT_USIZE)` back to its CLAP parameter ID.
#[inline]
pub const fn slot_to_param_id(slot: usize) -> clap_id {
    match slot {
        0 => 1,
        1 => 2,
        2 => 3,
        s if s < PARAM_COUNT_USIZE => {
            let rem = s - 3;
            let band = rem / 6;
            let field = rem % 6;
            100 + (band as clap_id) * 100 + (field as clap_id)
        }
        _ => 0,
    }
}

/// Returns the default 8-band `EqParameters` matching the standalone app.
pub fn default_eq_parameters() -> EqParameters {
    EqParameters {
        global: GlobalParameters::default(),
        bands: romueq_gui::default_8_bands(),
    }
}

/// Reads a single parameter value from `EqParameters` in CLAP native range.
pub fn get_param_from_eq(eq: &EqParameters, id: clap_id) -> Option<f64> {
    match id {
        1 => Some(if eq.global.bypass { 1.0 } else { 0.0 }),
        2 => Some(eq.global.output_gain_db as f64),
        3 => Some(if eq.global.auto_gain { 1.0 } else { 0.0 }),
        100..=899 => {
            let offset = id - 100;
            let band_idx = (offset / 100) as usize;
            let field = offset % 100;
            if band_idx >= MAX_BANDS {
                return None;
            }
            let b = &eq.bands[band_idx];
            match field {
                0 => Some(if b.enabled { 1.0 } else { 0.0 }),
                1 => Some(b.filter_type.to_raw() as f64),
                2 => Some(b.frequency_hz as f64),
                3 => Some(b.gain_db as f64),
                4 => Some(b.q as f64),
                5 => Some(b.slope.to_raw() as f64),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Writes a single CLAP parameter value into `EqParameters`, clamping/quantizing safely.
pub fn apply_param_to_eq(eq: &mut EqParameters, id: clap_id, val: f64) -> bool {
    let val = if val.is_finite() { val } else { 0.0 };
    match id {
        1 => {
            eq.global.bypass = val >= 0.5;
            true
        }
        2 => {
            eq.global.output_gain_db = (val as f32).clamp(-24.0, 24.0);
            true
        }
        3 => {
            eq.global.auto_gain = val >= 0.5;
            true
        }
        100..=899 => {
            let offset = id - 100;
            let band_idx = (offset / 100) as usize;
            let field = offset % 100;
            if band_idx >= MAX_BANDS {
                return false;
            }
            let b = &mut eq.bands[band_idx];
            match field {
                0 => {
                    b.enabled = val >= 0.5;
                    true
                }
                1 => {
                    let raw = val.round().clamp(0.0, 4.0) as u32;
                    if let Some(ft) = FilterType::from_raw(raw) {
                        b.filter_type = ft;
                        true
                    } else {
                        false
                    }
                }
                2 => {
                    b.frequency_hz = (val as f32).clamp(20.0, 20000.0);
                    true
                }
                3 => {
                    b.gain_db = (val as f32).clamp(-24.0, 24.0);
                    true
                }
                4 => {
                    b.q = (val as f32).clamp(0.1, 20.0);
                    true
                }
                5 => {
                    let raw = val.round().clamp(0.0, 3.0) as u32;
                    if let Some(s) = Slope::from_raw(raw) {
                        b.slope = s;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// Clamps/quantizes a raw `f64` value according to parameter `id` metadata.
pub fn normalize_param_value(id: clap_id, val: f64) -> f32 {
    let val = if val.is_finite() { val } else { 0.0 };
    match id {
        1 | 3 => {
            if val >= 0.5 {
                1.0
            } else {
                0.0
            }
        }
        2 => (val as f32).clamp(-24.0, 24.0),
        100..=899 => match (id - 100) % 100 {
            0 => {
                if val >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            1 => val.round().clamp(0.0, 4.0) as f32,
            2 => (val as f32).clamp(20.0, 20000.0),
            3 => (val as f32).clamp(-24.0, 24.0),
            4 => (val as f32).clamp(0.1, 20.0),
            5 => val.round().clamp(0.0, 3.0) as f32,
            _ => val as f32,
        },
        _ => val as f32,
    }
}

pub fn get_param_meta(index: u32) -> Option<ParamMeta> {
    if index == 0 {
        return Some(ParamMeta {
            id: 1,
            name: "Bypass",
            module: "Master",
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.0,
            flags: CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_BYPASS | CLAP_PARAM_IS_AUTOMATABLE,
        });
    }
    if index == 1 {
        return Some(ParamMeta {
            id: 2,
            name: "Gain Master",
            module: "Master",
            min_value: -24.0,
            max_value: 24.0,
            default_value: 0.0,
            flags: CLAP_PARAM_IS_AUTOMATABLE,
        });
    }
    if index == 2 {
        return Some(ParamMeta {
            id: 3,
            name: "Auto Gain",
            module: "Master",
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.0,
            flags: CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_AUTOMATABLE,
        });
    }

    let band_param_idx = index - 3;
    let band_idx = (band_param_idx / 6) as usize;
    let field_idx = band_param_idx % 6;

    if band_idx >= MAX_BANDS {
        return None;
    }

    let base_id = 100 + (band_idx as clap_id * 100);

    match field_idx {
        0 => Some(ParamMeta {
            id: base_id,
            name: match band_idx {
                0 => "Bande 1 Active",
                1 => "Bande 2 Active",
                2 => "Bande 3 Active",
                3 => "Bande 4 Active",
                4 => "Bande 5 Active",
                5 => "Bande 6 Active",
                6 => "Bande 7 Active",
                _ => "Bande 8 Active",
            },
            module: "Bandes",
            min_value: 0.0,
            max_value: 1.0,
            default_value: 1.0,
            flags: CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_AUTOMATABLE,
        }),
        1 => Some(ParamMeta {
            id: base_id + 1,
            name: match band_idx {
                0 => "Bande 1 Type",
                1 => "Bande 2 Type",
                2 => "Bande 3 Type",
                3 => "Bande 4 Type",
                4 => "Bande 5 Type",
                5 => "Bande 6 Type",
                6 => "Bande 7 Type",
                _ => "Bande 8 Type",
            },
            module: "Bandes",
            min_value: 0.0,
            max_value: 4.0,
            default_value: if band_idx == 0 {
                FilterType::LowShelf.to_raw() as f64
            } else if band_idx == 7 {
                FilterType::HighShelf.to_raw() as f64
            } else {
                FilterType::Bell.to_raw() as f64
            },
            flags: CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
        }),
        2 => Some(ParamMeta {
            id: base_id + 2,
            name: match band_idx {
                0 => "Bande 1 Fréquence",
                1 => "Bande 2 Fréquence",
                2 => "Bande 3 Fréquence",
                3 => "Bande 4 Fréquence",
                4 => "Bande 5 Fréquence",
                5 => "Bande 6 Fréquence",
                6 => "Bande 7 Fréquence",
                _ => "Bande 8 Fréquence",
            },
            module: "Bandes",
            min_value: 20.0,
            max_value: 20000.0,
            default_value: match band_idx {
                0 => 32.0,
                1 => 75.0,
                2 => 180.0,
                3 => 420.0,
                4 => 1000.0,
                5 => 2400.0,
                6 => 5600.0,
                _ => 13500.0,
            },
            flags: CLAP_PARAM_IS_AUTOMATABLE,
        }),
        3 => Some(ParamMeta {
            id: base_id + 3,
            name: match band_idx {
                0 => "Bande 1 Gain",
                1 => "Bande 2 Gain",
                2 => "Bande 3 Gain",
                3 => "Bande 4 Gain",
                4 => "Bande 5 Gain",
                5 => "Bande 6 Gain",
                6 => "Bande 7 Gain",
                _ => "Bande 8 Gain",
            },
            module: "Bandes",
            min_value: -24.0,
            max_value: 24.0,
            default_value: 0.0,
            flags: CLAP_PARAM_IS_AUTOMATABLE,
        }),
        4 => Some(ParamMeta {
            id: base_id + 4,
            name: match band_idx {
                0 => "Bande 1 Facteur Q",
                1 => "Bande 2 Facteur Q",
                2 => "Bande 3 Facteur Q",
                3 => "Bande 4 Facteur Q",
                4 => "Bande 5 Facteur Q",
                5 => "Bande 6 Facteur Q",
                6 => "Bande 7 Facteur Q",
                _ => "Bande 8 Facteur Q",
            },
            module: "Bandes",
            min_value: 0.1,
            max_value: 20.0,
            default_value: if band_idx == 0 || band_idx == 7 {
                0.707
            } else {
                1.0
            },
            flags: CLAP_PARAM_IS_AUTOMATABLE,
        }),
        5 => Some(ParamMeta {
            id: base_id + 5,
            name: match band_idx {
                0 => "Bande 1 Pente",
                1 => "Bande 2 Pente",
                2 => "Bande 3 Pente",
                3 => "Bande 4 Pente",
                4 => "Bande 5 Pente",
                5 => "Bande 6 Pente",
                6 => "Bande 7 Pente",
                _ => "Bande 8 Pente",
            },
            module: "Bandes",
            min_value: 0.0,
            max_value: 3.0,
            default_value: Slope::Db12.to_raw() as f64,
            flags: CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
        }),
        _ => None,
    }
}
