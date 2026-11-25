//! Stable parameter definitions and ID mapping for Equaform CLAP plugin.
//!
//! Provides 51 stable automatable parameters aligned with `equaform_common::parameter`:
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
use equaform_common::{
    AutoGainMode, BandRouting, EqParameters, FilterType, GlobalParameters, MAX_BANDS, Slope,
};

/// Total count of exposed automatable parameters (4 global + 8 * 7 band params = 60).
pub const PARAM_COUNT: u32 = 4 + (MAX_BANDS as u32 * 7);
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
        7 => Some(3),
        100..=899 => {
            let offset = id - 100;
            let band = offset / 100;
            let field = offset % 100;
            if (band as usize) < MAX_BANDS {
                if field < 6 {
                    Some(4 + (band as usize) * 7 + (field as usize))
                } else if field == 7 {
                    Some(4 + (band as usize) * 7 + 6)
                } else {
                    None
                }
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
        3 => 7,
        s if s < PARAM_COUNT_USIZE => {
            let rem = s - 4;
            let band = rem / 7;
            let field_slot = rem % 7;
            let field_id = if field_slot == 6 { 7 } else { field_slot };
            100 + (band as clap_id) * 100 + (field_id as clap_id)
        }
        _ => 0,
    }
}

/// Returns the default 8-band `EqParameters` matching the standalone app.
pub fn default_eq_parameters() -> EqParameters {
    EqParameters {
        global: GlobalParameters::default(),
        bands: equaform_gui::default_8_bands(),
    }
}

/// Reads a single parameter value from `EqParameters` in CLAP native range.
pub fn get_param_from_eq(eq: &EqParameters, id: clap_id) -> Option<f64> {
    match id {
        1 => Some(if eq.global.bypass { 1.0 } else { 0.0 }),
        2 => Some(eq.global.output_gain_db as f64),
        3 => Some(if eq.global.auto_gain { 1.0 } else { 0.0 }),
        7 => Some(eq.global.auto_gain_mode.to_raw() as f64),
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
                7 => Some(eq.band_routing(band_idx).to_raw() as f64),
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
        7 => {
            let raw = val.round().clamp(0.0, 1.0) as u32;
            if let Some(mode) = AutoGainMode::from_raw(raw) {
                eq.global.auto_gain_mode = mode;
                true
            } else {
                false
            }
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
                7 => {
                    let raw = val.round().clamp(0.0, 4.0) as u32;
                    if let Some(r) = BandRouting::from_raw(raw) {
                        eq.set_band_routing(band_idx, r);
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
        7 => val.round().clamp(0.0, 1.0) as f32,
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
            7 => val.round().clamp(0.0, 4.0) as f32,
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
            name: "Master Gain",
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
    if index == 3 {
        return Some(ParamMeta {
            id: 7,
            name: "Auto Gain Mode",
            module: "Master",
            min_value: 0.0,
            max_value: 1.0,
            default_value: AutoGainMode::Perceptual.to_raw() as f64,
            flags: CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
        });
    }

    let band_param_idx = index - 4;
    let band_idx = (band_param_idx / 7) as usize;
    let field_idx = band_param_idx % 7;

    if band_idx >= MAX_BANDS {
        return None;
    }

    let base_id = 100 + (band_idx as clap_id * 100);

    match field_idx {
        0 => Some(ParamMeta {
            id: base_id,
            name: match band_idx {
                0 => "Band 1 Active",
                1 => "Band 2 Active",
                2 => "Band 3 Active",
                3 => "Band 4 Active",
                4 => "Band 5 Active",
                5 => "Band 6 Active",
                6 => "Band 7 Active",
                _ => "Band 8 Active",
            },
            module: "Bands",
            min_value: 0.0,
            max_value: 1.0,
            default_value: 1.0,
            flags: CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_AUTOMATABLE,
        }),
        1 => Some(ParamMeta {
            id: base_id + 1,
            name: match band_idx {
                0 => "Band 1 Type",
                1 => "Band 2 Type",
                2 => "Band 3 Type",
                3 => "Band 4 Type",
                4 => "Band 5 Type",
                5 => "Band 6 Type",
                6 => "Band 7 Type",
                _ => "Band 8 Type",
            },
            module: "Bands",
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
                0 => "Band 1 Frequency",
                1 => "Band 2 Frequency",
                2 => "Band 3 Frequency",
                3 => "Band 4 Frequency",
                4 => "Band 5 Frequency",
                5 => "Band 6 Frequency",
                6 => "Band 7 Frequency",
                _ => "Band 8 Frequency",
            },
            module: "Bands",
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
                0 => "Band 1 Gain",
                1 => "Band 2 Gain",
                2 => "Band 3 Gain",
                3 => "Band 4 Gain",
                4 => "Band 5 Gain",
                5 => "Band 6 Gain",
                6 => "Band 7 Gain",
                _ => "Band 8 Gain",
            },
            module: "Bands",
            min_value: -24.0,
            max_value: 24.0,
            default_value: 0.0,
            flags: CLAP_PARAM_IS_AUTOMATABLE,
        }),
        4 => Some(ParamMeta {
            id: base_id + 4,
            name: match band_idx {
                0 => "Band 1 Q Factor",
                1 => "Band 2 Q Factor",
                2 => "Band 3 Q Factor",
                3 => "Band 4 Q Factor",
                4 => "Band 5 Q Factor",
                5 => "Band 6 Q Factor",
                6 => "Band 7 Q Factor",
                _ => "Band 8 Q Factor",
            },
            module: "Bands",
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
                0 => "Band 1 Slope",
                1 => "Band 2 Slope",
                2 => "Band 3 Slope",
                3 => "Band 4 Slope",
                4 => "Band 5 Slope",
                5 => "Band 6 Slope",
                6 => "Band 7 Slope",
                _ => "Band 8 Slope",
            },
            module: "Bands",
            min_value: 0.0,
            max_value: 3.0,
            default_value: Slope::Db12.to_raw() as f64,
            flags: CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
        }),
        6 => Some(ParamMeta {
            id: base_id + 7,
            name: match band_idx {
                0 => "Band 1 Routing",
                1 => "Band 2 Routing",
                2 => "Band 3 Routing",
                3 => "Band 4 Routing",
                4 => "Band 5 Routing",
                5 => "Band 6 Routing",
                6 => "Band 7 Routing",
                _ => "Band 8 Routing",
            },
            module: "Bands",
            min_value: 0.0,
            max_value: 4.0,
            default_value: BandRouting::Stereo.to_raw() as f64,
            flags: CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_60_param_metadata_english() {
        assert_eq!(PARAM_COUNT, 60);
        let mut eq = default_eq_parameters();
        for idx in 0..PARAM_COUNT {
            let meta = get_param_meta(idx)
                .unwrap_or_else(|| panic!("Missing metadata for param index {idx}"));
            assert!(!meta.name.is_empty());
            assert!(!meta.module.is_empty());
            // Verify names and modules are pure ASCII (English, no accents)
            assert!(
                meta.name.is_ascii(),
                "Param {idx} name '{}' must be ASCII English",
                meta.name
            );
            assert!(
                meta.module.is_ascii(),
                "Param {idx} module '{}' must be ASCII English",
                meta.module
            );
            // Verify get_param_from_eq and apply_param_to_eq work for all 51 IDs
            let val = get_param_from_eq(&eq, meta.id)
                .unwrap_or_else(|| panic!("get_param_from_eq failed for id {}", meta.id));
            assert!(apply_param_to_eq(&mut eq, meta.id, val));
            // Verify normalize_param_value produces value within min..=max range
            let norm = normalize_param_value(meta.id, val);
            let min_val = meta.min_value as f32;
            let max_val = meta.max_value as f32;
            let param_id = meta.id;
            assert!(
                (min_val..=max_val).contains(&norm),
                "Normalized value {norm} out of range [{min_val}, {max_val}] for id {param_id}"
            );
        }
        assert!(get_param_meta(PARAM_COUNT).is_none());
        assert!(get_param_from_eq(&eq, 99999).is_none());
        assert!(!apply_param_to_eq(&mut eq, 99999, 0.0));
    }
}
