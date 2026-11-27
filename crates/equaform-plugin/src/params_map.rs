//! Stable parameter definitions and ID mapping for Equaform CLAP plugin.
//!
//! Provides 61 stable automatable parameters aligned with `equaform_common::parameter`:
//! - ID 1: Bypass (Bool)
//! - ID 2: Output Gain dB (-24.0 .. +24.0)
//! - ID 3: Auto-Gain (Bool)
//! - ID 4: Processing Mode (Enum: 0=Zero Latency, 1=Linear Low, 2=Linear High)
//! - ID 7: Auto-Gain Mode (Enum: 0=Perceptual, 1=Anti-Clip)
//! - ID 100 + i*100: Band i Enabled (Bool)
//! - ID 101 + i*100: Band i FilterType (Enum: 0=Bell, 1=LowShelf, 2=HighShelf, 3=HighPass, 4=LowPass)
//! - ID 102 + i*100: Band i Frequency Hz (20.0 .. 20000.0)
//! - ID 103 + i*100: Band i Gain dB (-24.0 .. +24.0)
//! - ID 104 + i*100: Band i Q (0.1 .. 20.0)
//! - ID 105 + i*100: Band i Slope (Enum: 0=6dB/o, 1=12dB/o, 2=18dB/o, 3=24dB/o)
//! - ID 107 + i*100: Band i Routing (Enum: 0=Stereo, 1=Left, 2=Right, 3=Mid, 4=Side)

use clap_sys::ext::params::*;
use clap_sys::id::clap_id;
use equaform_common::{
    AutoGainMode, BandRouting, EqParameters, FilterType, GlobalParameters, MAX_BANDS,
    ProcessingMode, Slope,
};

/// Total count of exposed automatable parameters (5 global + 8 * 7 band params = 61).
pub const PARAM_COUNT: u32 = 5 + (MAX_BANDS as u32 * 7);
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
        4 => Some(4),
        100..=899 => {
            let offset = id - 100;
            let band = offset / 100;
            let field = offset % 100;
            if (band as usize) < MAX_BANDS {
                if field < 6 {
                    Some(5 + (band as usize) * 7 + (field as usize))
                } else if field == 7 {
                    Some(5 + (band as usize) * 7 + 6)
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
        4 => 4,
        s if s < PARAM_COUNT_USIZE => {
            let rem = s - 5;
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
        4 => Some(eq.global.processing_mode.to_raw() as f64),
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
        4 => {
            let raw = val.round().clamp(0.0, 2.0) as u32;
            if let Some(mode) = ProcessingMode::from_raw(raw) {
                eq.global.processing_mode = mode;
                true
            } else {
                false
            }
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
        4 => val.round().clamp(0.0, 2.0) as f32,
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

const GLOBAL_PARAM_SPECS: [(clap_id, &str, &str, f64, f64, clap_param_info_flags); 5] = [
    (
        1,
        "Bypass",
        "Master",
        0.0,
        1.0,
        CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_BYPASS | CLAP_PARAM_IS_AUTOMATABLE,
    ),
    (
        2,
        "Master Gain",
        "Master",
        -24.0,
        24.0,
        CLAP_PARAM_IS_AUTOMATABLE,
    ),
    (
        3,
        "Auto Gain",
        "Master",
        0.0,
        1.0,
        CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_AUTOMATABLE,
    ),
    (
        7,
        "Auto Gain Mode",
        "Master",
        0.0,
        1.0,
        CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
    ),
    (
        4,
        "Processing Mode",
        "Global",
        0.0,
        2.0,
        CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_IS_STEPPED,
    ),
];

const BAND_FIELD_SPECS: [(clap_id, f64, f64, clap_param_info_flags); 7] = [
    (
        0,
        0.0,
        1.0,
        CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_AUTOMATABLE,
    ),
    (
        1,
        0.0,
        4.0,
        CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
    ),
    (2, 20.0, 20000.0, CLAP_PARAM_IS_AUTOMATABLE),
    (3, -24.0, 24.0, CLAP_PARAM_IS_AUTOMATABLE),
    (4, 0.1, 20.0, CLAP_PARAM_IS_AUTOMATABLE),
    (
        5,
        0.0,
        3.0,
        CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
    ),
    (
        7,
        0.0,
        4.0,
        CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
    ),
];

const BAND_PARAM_NAMES: [[&str; 7]; 8] = [
    [
        "Band 1 Active",
        "Band 1 Type",
        "Band 1 Frequency",
        "Band 1 Gain",
        "Band 1 Q Factor",
        "Band 1 Slope",
        "Band 1 Routing",
    ],
    [
        "Band 2 Active",
        "Band 2 Type",
        "Band 2 Frequency",
        "Band 2 Gain",
        "Band 2 Q Factor",
        "Band 2 Slope",
        "Band 2 Routing",
    ],
    [
        "Band 3 Active",
        "Band 3 Type",
        "Band 3 Frequency",
        "Band 3 Gain",
        "Band 3 Q Factor",
        "Band 3 Slope",
        "Band 3 Routing",
    ],
    [
        "Band 4 Active",
        "Band 4 Type",
        "Band 4 Frequency",
        "Band 4 Gain",
        "Band 4 Q Factor",
        "Band 4 Slope",
        "Band 4 Routing",
    ],
    [
        "Band 5 Active",
        "Band 5 Type",
        "Band 5 Frequency",
        "Band 5 Gain",
        "Band 5 Q Factor",
        "Band 5 Slope",
        "Band 5 Routing",
    ],
    [
        "Band 6 Active",
        "Band 6 Type",
        "Band 6 Frequency",
        "Band 6 Gain",
        "Band 6 Q Factor",
        "Band 6 Slope",
        "Band 6 Routing",
    ],
    [
        "Band 7 Active",
        "Band 7 Type",
        "Band 7 Frequency",
        "Band 7 Gain",
        "Band 7 Q Factor",
        "Band 7 Slope",
        "Band 7 Routing",
    ],
    [
        "Band 8 Active",
        "Band 8 Type",
        "Band 8 Frequency",
        "Band 8 Gain",
        "Band 8 Q Factor",
        "Band 8 Slope",
        "Band 8 Routing",
    ],
];

const BAND_DEFAULT_FREQS: [f64; 8] = [32.0, 75.0, 180.0, 420.0, 1000.0, 2400.0, 5600.0, 13500.0];

fn global_default(index: usize) -> f64 {
    match index {
        3 => AutoGainMode::Perceptual.to_raw() as f64,
        4 => ProcessingMode::MinimumPhase.to_raw() as f64,
        _ => 0.0,
    }
}

fn band_default(band_idx: usize, field_idx: usize) -> f64 {
    match field_idx {
        0 => 1.0,
        1 => match band_idx {
            0 => FilterType::LowShelf.to_raw() as f64,
            7 => FilterType::HighShelf.to_raw() as f64,
            _ => FilterType::Bell.to_raw() as f64,
        },
        2 => BAND_DEFAULT_FREQS[band_idx],
        4 => {
            if band_idx == 0 || band_idx == 7 {
                0.707
            } else {
                1.0
            }
        }
        5 => Slope::Db12.to_raw() as f64,
        6 => BandRouting::Stereo.to_raw() as f64,
        _ => 0.0,
    }
}

/// Returns CLAP metadata for a parameter index (`0..PARAM_COUNT`).
pub fn get_param_meta(index: u32) -> Option<ParamMeta> {
    if index < 5 {
        let idx = index as usize;
        let (id, name, module, min_value, max_value, flags) = GLOBAL_PARAM_SPECS[idx];
        return Some(ParamMeta {
            id,
            name,
            module,
            min_value,
            max_value,
            default_value: global_default(idx),
            flags,
        });
    }

    let band_param_idx = index - 5;
    let band_idx = (band_param_idx / 7) as usize;
    let field_idx = (band_param_idx % 7) as usize;
    if band_idx >= MAX_BANDS {
        return None;
    }

    let base_id = 100 + (band_idx as clap_id * 100);
    let (id_offset, min_value, max_value, flags) = BAND_FIELD_SPECS[field_idx];
    Some(ParamMeta {
        id: base_id + id_offset,
        name: BAND_PARAM_NAMES[band_idx][field_idx],
        module: "Bands",
        min_value,
        max_value,
        default_value: band_default(band_idx, field_idx),
        flags,
    })
}

/// Formats a CLAP parameter value into a human-readable display string for DAW automation lanes.
pub fn format_param_value(param_id: clap_id, value: f64) -> String {
    if param_id == 1 {
        if value >= 0.5 {
            "Bypass".to_string()
        } else {
            "On".to_string()
        }
    } else if param_id == 3 {
        if value >= 0.5 {
            "On".to_string()
        } else {
            "Off".to_string()
        }
    } else if param_id == 4 {
        let raw = if value.is_finite() {
            value.round().clamp(0.0, 2.0) as u32
        } else {
            0
        };
        ProcessingMode::from_raw(raw)
            .unwrap_or_default()
            .display_name()
            .to_string()
    } else if param_id == 7 {
        if value >= 0.5 {
            "Anti-Clip".to_string()
        } else {
            "Perceptual".to_string()
        }
    } else if param_id == 2 || (param_id >= 100 && param_id % 100 == 3) {
        format!("{value:+.1} dB")
    } else if param_id >= 100 && param_id % 100 == 0 {
        if value >= 0.5 {
            "On".to_string()
        } else {
            "Off".to_string()
        }
    } else if param_id >= 100 && param_id % 100 == 1 {
        match FilterType::from_raw(value.round().clamp(0.0, 4.0) as u32) {
            Some(FilterType::Bell) => "Bell".to_string(),
            Some(FilterType::LowShelf) => "Low Shelf".to_string(),
            Some(FilterType::HighShelf) => "High Shelf".to_string(),
            Some(FilterType::HighPass) => "High Pass".to_string(),
            Some(FilterType::LowPass) => "Low Pass".to_string(),
            None => "Bell".to_string(),
        }
    } else if param_id >= 100 && param_id % 100 == 2 {
        if value >= 1000.0 {
            format!("{:.2} kHz", value / 1000.0)
        } else {
            format!("{value:.0} Hz")
        }
    } else if param_id >= 100 && param_id % 100 == 4 {
        format!("Q {value:.2}")
    } else if param_id >= 100 && param_id % 100 == 5 {
        match Slope::from_raw(value.round().clamp(0.0, 3.0) as u32) {
            Some(Slope::Db6) => "6 dB/o".to_string(),
            Some(Slope::Db12) => "12 dB/o".to_string(),
            Some(Slope::Db18) => "18 dB/o".to_string(),
            Some(Slope::Db24) => "24 dB/o".to_string(),
            None => "12 dB/o".to_string(),
        }
    } else if param_id >= 100 && param_id % 100 == 7 {
        BandRouting::from_raw(value.round().clamp(0.0, 4.0) as u32)
            .unwrap_or_default()
            .display_name()
            .to_string()
    } else {
        format!("{value:.2}")
    }
}

/// Parses a user-entered parameter text string into its CLAP native numeric value.
pub fn parse_param_value(param_id: clap_id, text: &str) -> Option<f64> {
    let lower = text.trim().to_ascii_lowercase().replace(',', ".");

    if param_id == 1 {
        if lower == "bypass" || lower == "off" || lower == "1" || lower == "true" {
            return Some(1.0);
        }
        if lower == "on" || lower == "0" || lower == "false" || lower == "active" {
            return Some(0.0);
        }
    }
    if param_id == 3 || (param_id >= 100 && param_id % 100 == 0) {
        if lower == "on" || lower == "1" || lower == "true" {
            return Some(1.0);
        }
        if lower == "off" || lower == "0" || lower == "false" {
            return Some(0.0);
        }
    }
    if param_id == 4 {
        return match lower.as_str() {
            "zero" | "zero latency" | "minimum" | "minimum phase" | "min" | "0" => {
                Some(ProcessingMode::MinimumPhase.to_raw() as f64)
            }
            "linear low" | "low" | "linear phase low" | "1" => {
                Some(ProcessingMode::LinearPhaseLow.to_raw() as f64)
            }
            "linear high" | "high" | "linear phase high" | "2" => {
                Some(ProcessingMode::LinearPhaseHigh.to_raw() as f64)
            }
            _ => lower
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && (0.0..=2.0).contains(v))
                .and_then(|v| ProcessingMode::from_raw(v.round() as u32))
                .map(|m| m.to_raw() as f64),
        };
    }
    if param_id >= 100 && param_id % 100 == 1 {
        let parsed = match lower.as_str() {
            "bell" | "peak" => Some(FilterType::Bell.to_raw() as f64),
            "low shelf" | "lowshelf" | "ls" => Some(FilterType::LowShelf.to_raw() as f64),
            "high shelf" | "highshelf" | "hishelf" | "hs" => {
                Some(FilterType::HighShelf.to_raw() as f64)
            }
            "high pass" | "highpass" | "hipass" | "hp" | "hpf" => {
                Some(FilterType::HighPass.to_raw() as f64)
            }
            "low pass" | "lowpass" | "lp" | "lpf" => Some(FilterType::LowPass.to_raw() as f64),
            _ => None,
        };
        if parsed.is_some() {
            return parsed;
        }
    }
    if param_id >= 100 && param_id % 100 == 5 {
        let parsed = match lower.as_str() {
            "6 db/o" | "6 db/oct" | "6" => Some(Slope::Db6.to_raw() as f64),
            "12 db/o" | "12 db/oct" | "12" => Some(Slope::Db12.to_raw() as f64),
            "18 db/o" | "18 db/oct" | "18" => Some(Slope::Db18.to_raw() as f64),
            "24 db/o" | "24 db/oct" | "24" => Some(Slope::Db24.to_raw() as f64),
            _ => None,
        };
        if parsed.is_some() {
            return parsed;
        }
    }
    if param_id >= 100 && param_id % 100 == 7 {
        let parsed = match lower.as_str() {
            "stereo" | "st" => Some(BandRouting::Stereo.to_raw() as f64),
            "left" | "l" => Some(BandRouting::Left.to_raw() as f64),
            "right" | "r" => Some(BandRouting::Right.to_raw() as f64),
            "mid" | "m" => Some(BandRouting::Mid.to_raw() as f64),
            "side" | "s" => Some(BandRouting::Side.to_raw() as f64),
            _ => None,
        };
        if parsed.is_some() {
            return parsed;
        }
    }

    let is_khz = lower.ends_with("khz") || (lower.ends_with('k') && !lower.ends_with("ok"));
    let cleaned = lower
        .trim_end_matches("db/o")
        .trim_end_matches("db/oct")
        .trim_end_matches("db")
        .trim_end_matches("khz")
        .trim_end_matches("hz")
        .trim_end_matches('k')
        .trim_start_matches('q')
        .trim();
    if let Ok(mut v) = cleaned.parse::<f64>() {
        if is_khz && param_id >= 100 && param_id % 100 == 2 {
            v *= 1000.0;
        }
        return Some(v);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_61_param_metadata_english() {
        assert_eq!(PARAM_COUNT, 61);
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
            // Verify slot <-> id round-trip
            assert_eq!(param_id_to_slot(meta.id), Some(idx as usize));
            assert_eq!(slot_to_param_id(idx as usize), meta.id);
            // Verify get_param_from_eq and apply_param_to_eq work for all 61 IDs
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

    #[test]
    fn test_processing_mode_param_round_trip_format_and_parse() {
        assert_eq!(param_id_to_slot(4), Some(4));
        assert_eq!(slot_to_param_id(4), 4);

        let meta = get_param_meta(4).expect("slot 4 metadata must exist");
        assert_eq!(meta.id, 4);
        assert_eq!(meta.name, "Processing Mode");
        assert_eq!(meta.module, "Global");
        assert_eq!(meta.min_value, 0.0);
        assert_eq!(meta.max_value, 2.0);
        assert_eq!(meta.default_value, 0.0);
        assert_eq!(
            meta.flags,
            CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_IS_STEPPED
        );

        let mut eq = default_eq_parameters();
        assert_eq!(get_param_from_eq(&eq, 4), Some(0.0));

        for (raw, expected_mode, label) in [
            (0.0, ProcessingMode::MinimumPhase, "Zero Latency"),
            (1.0, ProcessingMode::LinearPhaseLow, "Linear Low"),
            (2.0, ProcessingMode::LinearPhaseHigh, "Linear High"),
        ] {
            assert!(apply_param_to_eq(&mut eq, 4, raw));
            assert_eq!(eq.global.processing_mode, expected_mode);
            assert_eq!(get_param_from_eq(&eq, 4), Some(raw));
            assert_eq!(format_param_value(4, raw), label);
            assert_eq!(parse_param_value(4, label), Some(raw));
        }

        // Case-insensitive and alias parsing
        assert_eq!(parse_param_value(4, "ZERO"), Some(0.0));
        assert_eq!(parse_param_value(4, "Minimum"), Some(0.0));
        assert_eq!(parse_param_value(4, "0"), Some(0.0));
        assert_eq!(parse_param_value(4, "  LiNeAr LoW "), Some(1.0));
        assert_eq!(parse_param_value(4, "low"), Some(1.0));
        assert_eq!(parse_param_value(4, "1"), Some(1.0));
        assert_eq!(parse_param_value(4, "Linear High"), Some(2.0));
        assert_eq!(parse_param_value(4, "HIGH"), Some(2.0));
        assert_eq!(parse_param_value(4, "2"), Some(2.0));

        // Invalid or out-of-range inputs rejected
        assert_eq!(parse_param_value(4, "3"), None);
        assert_eq!(parse_param_value(4, "-1"), None);
        assert_eq!(parse_param_value(4, "unknown"), None);

        // Normalization clamps and rounds NaN / out-of-range
        assert_eq!(normalize_param_value(4, f64::NAN), 0.0);
        assert_eq!(normalize_param_value(4, 99.0), 2.0);
        assert_eq!(normalize_param_value(4, 1.4), 1.0);
        assert_eq!(normalize_param_value(4, 1.6), 2.0);
    }

    #[test]
    fn test_all_61_param_metadata_snapshot() {
        let globals: [(clap_id, &str, &str, f64, f64, f64, clap_param_info_flags); 5] = [
            (
                1,
                "Bypass",
                "Master",
                0.0,
                1.0,
                0.0,
                CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_BYPASS | CLAP_PARAM_IS_AUTOMATABLE,
            ),
            (
                2,
                "Master Gain",
                "Master",
                -24.0,
                24.0,
                0.0,
                CLAP_PARAM_IS_AUTOMATABLE,
            ),
            (
                3,
                "Auto Gain",
                "Master",
                0.0,
                1.0,
                0.0,
                CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_AUTOMATABLE,
            ),
            (
                7,
                "Auto Gain Mode",
                "Master",
                0.0,
                1.0,
                AutoGainMode::Perceptual.to_raw() as f64,
                CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
            ),
            (
                4,
                "Processing Mode",
                "Global",
                0.0,
                2.0,
                ProcessingMode::MinimumPhase.to_raw() as f64,
                CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_IS_STEPPED,
            ),
        ];
        for (idx, exp) in globals.iter().enumerate() {
            let m = get_param_meta(idx as u32).expect("global param meta");
            assert_eq!(
                (
                    m.id,
                    m.name,
                    m.module,
                    m.min_value,
                    m.max_value,
                    m.default_value,
                    m.flags
                ),
                *exp
            );
        }

        let freqs = [32.0, 75.0, 180.0, 420.0, 1000.0, 2400.0, 5600.0, 13500.0];
        for (b, &freq) in freqs.iter().enumerate() {
            let base_id = 100 + (b as clap_id * 100);
            let band_type = match b {
                0 => FilterType::LowShelf,
                7 => FilterType::HighShelf,
                _ => FilterType::Bell,
            };
            let q_default = if b == 0 || b == 7 { 0.707 } else { 1.0 };
            let expected = [
                (
                    base_id,
                    format!("Band {} Active", b + 1),
                    "Bands",
                    0.0,
                    1.0,
                    1.0,
                    CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_AUTOMATABLE,
                ),
                (
                    base_id + 1,
                    format!("Band {} Type", b + 1),
                    "Bands",
                    0.0,
                    4.0,
                    band_type.to_raw() as f64,
                    CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
                ),
                (
                    base_id + 2,
                    format!("Band {} Frequency", b + 1),
                    "Bands",
                    20.0,
                    20000.0,
                    freq,
                    CLAP_PARAM_IS_AUTOMATABLE,
                ),
                (
                    base_id + 3,
                    format!("Band {} Gain", b + 1),
                    "Bands",
                    -24.0,
                    24.0,
                    0.0,
                    CLAP_PARAM_IS_AUTOMATABLE,
                ),
                (
                    base_id + 4,
                    format!("Band {} Q Factor", b + 1),
                    "Bands",
                    0.1,
                    20.0,
                    q_default,
                    CLAP_PARAM_IS_AUTOMATABLE,
                ),
                (
                    base_id + 5,
                    format!("Band {} Slope", b + 1),
                    "Bands",
                    0.0,
                    3.0,
                    Slope::Db12.to_raw() as f64,
                    CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
                ),
                (
                    base_id + 7,
                    format!("Band {} Routing", b + 1),
                    "Bands",
                    0.0,
                    4.0,
                    BandRouting::Stereo.to_raw() as f64,
                    CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_ENUM | CLAP_PARAM_IS_AUTOMATABLE,
                ),
            ];
            for (offset, exp) in expected.iter().enumerate() {
                let idx = 5 + b * 7 + offset;
                let m = get_param_meta(idx as u32).expect("band param meta");
                assert_eq!(
                    (
                        m.id,
                        m.name,
                        m.module,
                        m.min_value,
                        m.max_value,
                        m.default_value,
                        m.flags
                    ),
                    (exp.0, exp.1.as_str(), exp.2, exp.3, exp.4, exp.5, exp.6)
                );
            }
        }
    }
}
