//! Composite parameter structures and parameter command transport types.

use super::ids::{BAND_ID_MIN, MAX_BANDS, ParamId};
use super::types::{AutoGainMode, BandRouting, FilterType, ProcessingMode, Slope, StereoMode};

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

/// Parameter command from producer to consumer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParameterCommand {
    /// Set a single parameter value.
    SetParameter { id: ParamId, value: f32 },
    /// Set all parameters at once.
    SetAll(EqParameters),
}
