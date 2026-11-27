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
    for raw in 0..=2 {
        let pm = ProcessingMode::from_raw(raw).unwrap();
        assert_eq!(pm.to_raw(), raw);
    }
    assert!(ProcessingMode::from_raw(3).is_none());
    assert_eq!(ProcessingMode::default(), ProcessingMode::MinimumPhase);
    assert_eq!(ProcessingMode::MinimumPhase.latency_samples(), 0);
    assert_eq!(ProcessingMode::LinearPhaseLow.latency_samples(), 1024);
    assert_eq!(ProcessingMode::LinearPhaseHigh.latency_samples(), 2048);
    assert_eq!(ProcessingMode::MinimumPhase.fir_length(), 0);
    assert_eq!(ProcessingMode::LinearPhaseLow.fir_length(), 2048);
    assert_eq!(ProcessingMode::LinearPhaseHigh.fir_length(), 4096);
    assert!(!ProcessingMode::MinimumPhase.is_linear_phase());
    assert!(ProcessingMode::LinearPhaseLow.is_linear_phase());
    assert!(ProcessingMode::LinearPhaseHigh.is_linear_phase());
    assert_eq!(ProcessingMode::MinimumPhase.display_name(), "Zero Latency");
    assert_eq!(ProcessingMode::LinearPhaseLow.display_name(), "Linear Low");
    assert_eq!(
        ProcessingMode::LinearPhaseHigh.display_name(),
        "Linear High"
    );
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

    // processing_mode: 0..2 (Zero Latency, Linear Low, Linear High)
    assert!(validate_value(4, 0.0).is_ok());
    assert!(validate_value(4, 1.0).is_ok());
    assert!(validate_value(4, 2.0).is_ok());
    assert!(validate_value(4, -0.001).is_err());
    assert!(validate_value(4, 2.001).is_err());

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

#[test]
fn test_band_parameter_info_snapshot_all_bands() {
    let expected_specs: [(u32, &str, ParameterType, f32, f32, f32, &str); 7] = [
        (0, "enabled", ParameterType::Bool, 0.0, 1.0, 1.0, ""),
        (
            1,
            "filter_type",
            ParameterType::Enum { count: 5 },
            0.0,
            4.0,
            0.0,
            "",
        ),
        (
            2,
            "frequency_hz",
            ParameterType::Continuous,
            20.0,
            20000.0,
            1000.0,
            "Hz",
        ),
        (
            3,
            "gain_db",
            ParameterType::Continuous,
            -24.0,
            24.0,
            0.0,
            "dB",
        ),
        (
            4,
            "q",
            ParameterType::Continuous,
            0.1,
            20.0,
            std::f32::consts::FRAC_1_SQRT_2,
            "Q",
        ),
        (
            5,
            "slope",
            ParameterType::Enum { count: 4 },
            0.0,
            3.0,
            1.0,
            "dB/oct",
        ),
        (
            7,
            "routing",
            ParameterType::Enum { count: 5 },
            0.0,
            4.0,
            0.0,
            "",
        ),
    ];

    for band in 0..8u32 {
        let base = 100 + band * 100;
        for &(sub, suffix, param_type, min, max, default, unit) in &expected_specs {
            let id = base + sub;
            let info = parameter_info(id)
                .unwrap_or_else(|| panic!("expected Some(ParameterInfo) for id {id}"));
            assert_eq!(info.id, id);
            assert_eq!(info.name, format!("band[{band}].{suffix}"));
            assert_eq!(info.param_type, param_type);
            assert_eq!(info.range.min, min);
            assert_eq!(info.range.max, max);
            assert_eq!(info.range.default, default);
            assert_eq!(info.unit, unit);
        }
        assert!(parameter_info(base + 6).is_none());
        for sub in 8..100u32 {
            assert!(parameter_info(base + sub).is_none());
        }
    }
    assert!(parameter_info(99).is_none());
    assert!(parameter_info(900).is_none());
}
