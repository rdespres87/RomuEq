//! Zero-allocation native internationalization (`i18n`) for Equaform GUI.
//!
//! Provides a statically typed translation catalog (`&'static str`) with zero
//! heap allocations on lookup, supporting English, French, German, and Spanish.

mod catalog;

use std::path::Path;

/// Supported UI languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    French,
    German,
    Spanish,
}

impl Language {
    /// All supported languages in canonical display order.
    pub const ALL: [Language; 4] = [
        Language::English,
        Language::French,
        Language::German,
        Language::Spanish,
    ];

    /// ISO 639-1 lowercase two-letter code (`"en"`, `"fr"`, `"de"`, `"es"`).
    pub const fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::French => "fr",
            Language::German => "de",
            Language::Spanish => "es",
        }
    }

    /// Uppercase two-letter code for compact toolbar display.
    pub const fn short_code(self) -> &'static str {
        match self {
            Language::English => "EN",
            Language::French => "FR",
            Language::German => "DE",
            Language::Spanish => "ES",
        }
    }

    /// Native display name of the language.
    pub const fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::French => "Français",
            Language::German => "Deutsch",
            Language::Spanish => "Español",
        }
    }

    /// Parse a language from a code or name (trimmed, case-insensitive).
    pub fn from_code(raw: &str) -> Option<Self> {
        let normalized = raw.trim().to_lowercase();
        match normalized.as_str() {
            "en" | "english" => Some(Language::English),
            "fr" | "french" | "français" | "francais" => Some(Language::French),
            "de" | "german" | "deutsch" => Some(Language::German),
            "es" | "spanish" | "español" | "espanol" => Some(Language::Spanish),
            _ => None,
        }
    }

    /// Load a language preference from the given file path, defaulting to English.
    pub fn load_from_path(path: &Path) -> Self {
        equaform_common::state::read_to_string_bounded(
            path,
            equaform_common::state::MAX_TEXT_FILE_BYTES,
        )
        .ok()
        .and_then(|content| Self::from_code(&content))
        .unwrap_or_default()
    }

    /// Save the language preference atomically to the given file path, creating parent directories if needed.
    pub fn save_to_path(self, path: &Path) -> std::io::Result<()> {
        equaform_common::state::atomic_write(path, self.code().as_bytes())
    }

    /// Load the saved language preference from the user config directory.
    pub fn load_saved() -> Self {
        let path = crate::presets_io::user_config_dir().join("language.txt");
        Self::load_from_path(&path)
    }

    /// Persist the language preference to the user config directory.
    pub fn save(self) {
        let path = crate::presets_io::user_config_dir().join("language.txt");
        let _ = self.save_to_path(&path);
    }
}

/// Zero-allocation static translation catalog for all GUI strings.
#[derive(Clone, Copy, Debug)]
pub struct Translations {
    pub window_title: &'static str,
    pub bypass_on: &'static str,
    pub bypass_off: &'static str,
    pub tooltip_enable_eq: &'static str,
    pub tooltip_bypass_eq: &'static str,
    pub mute: &'static str,
    pub unmute: &'static str,
    pub output_label: &'static str,
    pub tooltip_refresh_devices: &'static str,
    pub stereo_label: &'static str,
    pub tooltip_language: &'static str,
    pub eq_response_title: &'static str,
    pub eq_response_hint: &'static str,
    pub spectrum_title: &'static str,
    pub eq_response_series: &'static str,
    pub eq_curve_series: &'static str,
    pub spectrum_series: &'static str,
    pub band_prefix: &'static str,
    pub axis_freq: &'static str,
    pub axis_gain: &'static str,
    pub tooltip_freq_short: &'static str,
    pub tooltip_gain_short: &'static str,
    pub tooltip_level_short: &'static str,
    pub master_label: &'static str,
    pub output_slider: &'static str,
    pub auto_gain: &'static str,
    pub tooltip_auto_gain: &'static str,
    pub auto_gain_mode_perceptual: &'static str,
    pub auto_gain_mode_safe: &'static str,
    pub tooltip_auto_gain_perceptual: &'static str,
    pub tooltip_auto_gain_safe: &'static str,
    pub preset_label: &'static str,
    pub select_preset: &'static str,
    pub preset_name_hint: &'static str,
    pub save_button: &'static str,
    pub delete_button: &'static str,
    pub tooltip_factory_preset_locked: &'static str,
    pub tooltip_delete_preset: &'static str,
    pub band_names: [&'static str; 8],
    pub band_fallback: &'static str,
    pub filter_types: [&'static str; 5],
    pub tooltip_reset_gain: &'static str,
    pub tooltip_center_freq: &'static str,
    pub tooltip_q_factor: &'static str,
    pub tooltip_compare_slot_a: &'static str,
    pub tooltip_compare_slot_b: &'static str,
    pub tooltip_copy_ab_settings: &'static str,
    pub tooltip_undo: &'static str,
    pub tooltip_redo: &'static str,
    pub tooltip_band_solo: &'static str,
    pub tooltip_channel_routing_prefix: &'static str,
    pub tooltip_channel_routing_hint: &'static str,
    pub system_default_output: &'static str,
    pub status_eq_active: &'static str,
    pub status_eq_bypassed: &'static str,
    pub status_audio_muted: &'static str,
    pub status_devices_refreshed: &'static str,
    pub status_selected_output: &'static str,
    pub status_stream_error: &'static str,
    pub status_audio_error: &'static str,
    pub status_output_error: &'static str,
    pub status_preset_applied: &'static str,
    pub status_preset_saved: &'static str,
    pub status_preset_deleted: &'static str,
    pub status_preset_error: &'static str,
}

impl Translations {
    /// Returns the static translation catalog for the requested language.
    pub const fn for_language(lang: Language) -> &'static Self {
        match lang {
            Language::English => &catalog::EN,
            Language::French => &catalog::FR,
            Language::German => &catalog::DE,
            Language::Spanish => &catalog::ES,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_languages_have_non_empty_translations() {
        for lang in Language::ALL {
            let t = Translations::for_language(lang);
            let scalar_fields = [
                t.bypass_on,
                t.bypass_off,
                t.tooltip_enable_eq,
                t.tooltip_bypass_eq,
                t.mute,
                t.unmute,
                t.output_label,
                t.tooltip_refresh_devices,
                t.stereo_label,
                t.tooltip_language,
                t.eq_response_title,
                t.eq_response_hint,
                t.spectrum_title,
                t.eq_response_series,
                t.eq_curve_series,
                t.spectrum_series,
                t.band_prefix,
                t.axis_freq,
                t.axis_gain,
                t.tooltip_freq_short,
                t.tooltip_gain_short,
                t.tooltip_level_short,
                t.master_label,
                t.output_slider,
                t.auto_gain,
                t.tooltip_auto_gain,
                t.auto_gain_mode_perceptual,
                t.auto_gain_mode_safe,
                t.tooltip_auto_gain_perceptual,
                t.tooltip_auto_gain_safe,
                t.preset_label,
                t.select_preset,
                t.preset_name_hint,
                t.save_button,
                t.delete_button,
                t.tooltip_factory_preset_locked,
                t.tooltip_delete_preset,
                t.band_fallback,
                t.tooltip_reset_gain,
                t.tooltip_center_freq,
                t.tooltip_q_factor,
                t.tooltip_compare_slot_a,
                t.tooltip_compare_slot_b,
                t.tooltip_copy_ab_settings,
                t.tooltip_undo,
                t.tooltip_redo,
                t.tooltip_band_solo,
                t.tooltip_channel_routing_prefix,
                t.tooltip_channel_routing_hint,
                t.system_default_output,
                t.status_eq_active,
                t.status_eq_bypassed,
                t.status_audio_muted,
                t.status_devices_refreshed,
                t.status_selected_output,
                t.status_stream_error,
                t.status_audio_error,
                t.status_output_error,
                t.status_preset_applied,
                t.status_preset_saved,
                t.status_preset_deleted,
                t.status_preset_error,
            ];

            for field in scalar_fields {
                assert!(!field.trim().is_empty(), "Empty translation for {lang:?}");
            }
            for name in t.band_names {
                assert!(!name.trim().is_empty(), "Empty band name for {lang:?}");
            }
            for ft in t.filter_types {
                assert!(!ft.trim().is_empty(), "Empty filter type for {lang:?}");
            }
        }
    }

    #[test]
    fn test_new_tooltips_are_localized_outside_english() {
        let en = Translations::for_language(Language::English);
        for lang in [Language::French, Language::German, Language::Spanish] {
            let t = Translations::for_language(lang);
            assert_ne!(t.tooltip_compare_slot_a, en.tooltip_compare_slot_a);
            assert_ne!(t.tooltip_compare_slot_b, en.tooltip_compare_slot_b);
            assert_ne!(t.tooltip_copy_ab_settings, en.tooltip_copy_ab_settings);
            assert_ne!(t.tooltip_undo, en.tooltip_undo);
            assert_ne!(t.tooltip_redo, en.tooltip_redo);
            assert_ne!(t.tooltip_band_solo, en.tooltip_band_solo);
            assert_ne!(
                t.tooltip_channel_routing_prefix,
                en.tooltip_channel_routing_prefix
            );
            assert_ne!(
                t.tooltip_channel_routing_hint,
                en.tooltip_channel_routing_hint
            );
        }
    }

    #[test]
    fn test_language_code_roundtrip() {
        for lang in Language::ALL {
            assert_eq!(Language::from_code(lang.code()), Some(lang));
            assert_eq!(Language::from_code(lang.short_code()), Some(lang));
            assert_eq!(Language::from_code(lang.native_name()), Some(lang));
        }
        assert_eq!(Language::from_code("  FR \n"), Some(Language::French));
        assert_eq!(Language::from_code("unknown"), None);
        assert_eq!(Language::from_code(""), None);
    }

    #[test]
    fn test_language_persistence_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!("equaform_i18n_test_{}", std::process::id()));
        let path = dir.join("language.txt");

        // Missing file falls back to English.
        let _ = std::fs::remove_file(&path);
        assert_eq!(Language::load_from_path(&path), Language::English);

        for lang in Language::ALL {
            lang.save_to_path(&path)?;
            assert_eq!(Language::load_from_path(&path), lang);
        }

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
