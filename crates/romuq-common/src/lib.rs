#![forbid(unsafe_code)]

pub const MAX_BANDS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType { Bell, LowShelf, HighShelf, HighPass, LowPass }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slope { Db6, Db12, Db18, Db24 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StereoMode { Stereo }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessingMode { MinimumPhase }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandParameters {
    pub enabled: bool,
    pub filter_type: FilterType,
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub slope: Slope,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalParameters {
    pub bypass: bool,
    pub output_gain_db: f32,
    pub auto_gain: bool,
    pub processing_mode: ProcessingMode,
    pub stereo_mode: StereoMode,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqParameters {
    pub global: GlobalParameters,
    pub bands: [BandParameters; MAX_BANDS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParamId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParameterCommand {
    SetParameter { id: ParamId, value: f32 },
    SetAll(EqParameters),
}
