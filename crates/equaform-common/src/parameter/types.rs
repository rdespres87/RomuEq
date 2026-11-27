//! Enumerated parameter value types for EQ bands and global processing.

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

/// Processing mode for the EQ.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum ProcessingMode {
    /// Zero-latency minimum-phase IIR biquad cascade (0 samples latency).
    #[default]
    MinimumPhase,
    /// Linear-phase 2048-tap FIR equalizer (1024 samples latency).
    LinearPhaseLow,
    /// Linear-phase 4096-tap FIR equalizer (2048 samples latency).
    LinearPhaseHigh,
}

impl ProcessingMode {
    /// Converts a raw `u32` value to a `ProcessingMode`, returning `None` for unknown values.
    #[inline]
    pub const fn from_raw(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::MinimumPhase),
            1 => Some(Self::LinearPhaseLow),
            2 => Some(Self::LinearPhaseHigh),
            _ => None,
        }
    }

    /// Returns the raw `u32` representation of this mode.
    #[inline]
    pub const fn to_raw(self) -> u32 {
        match self {
            Self::MinimumPhase => 0,
            Self::LinearPhaseLow => 1,
            Self::LinearPhaseHigh => 2,
        }
    }

    /// Returns the algorithmic latency in samples.
    #[inline]
    pub const fn latency_samples(self) -> u32 {
        match self {
            Self::MinimumPhase => 0,
            Self::LinearPhaseLow => 1024,
            Self::LinearPhaseHigh => 2048,
        }
    }

    /// Returns the FIR filter length in taps (0 for minimum-phase IIR).
    #[inline]
    pub const fn fir_length(self) -> usize {
        match self {
            Self::MinimumPhase => 0,
            Self::LinearPhaseLow => 2048,
            Self::LinearPhaseHigh => 4096,
        }
    }

    /// Returns `true` when linear-phase FIR processing is active.
    #[inline]
    pub const fn is_linear_phase(self) -> bool {
        !matches!(self, Self::MinimumPhase)
    }

    /// Returns a human-readable display label for DAW hosts and UI tooltips.
    #[inline]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::MinimumPhase => "Zero Latency",
            Self::LinearPhaseLow => "Linear Low",
            Self::LinearPhaseHigh => "Linear High",
        }
    }
}

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
