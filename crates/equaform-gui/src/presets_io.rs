//! Preset and state disk persistence for RomuEq.
//!
//! Stores user-created presets in `~/.romueq/presets/*.req`, the last active preset
//! name in `~/.romueq/last_preset.txt`, and complete app state in `~/.romueq/app_state.bin`.

use crate::default_8_bands;
use crate::preset_manager::PresetManager;
use equaform_common::EqParameters;
use equaform_common::state::Preset;

/// Returns the configuration directory for Equaform (`%APPDATA%\Equaform` on Windows).
#[cfg(target_os = "windows")]
pub fn user_config_dir() -> std::path::PathBuf {
    let base = std::env::var("APPDATA")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let dir = std::path::PathBuf::from(&base).join("Equaform");
    let _ = std::fs::create_dir_all(dir.join("presets"));
    dir
}

/// Returns the configuration directory for Equaform (`~/.equaform`), migrating from legacy `~/.romueq` if present.
#[cfg(not(target_os = "windows"))]
pub fn user_config_dir() -> std::path::PathBuf {
    let base = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let new_dir = std::path::PathBuf::from(&base).join(".equaform");
    let old_dir = std::path::PathBuf::from(&base).join(".romueq");
    if !new_dir.exists() && old_dir.exists() {
        let _ = std::fs::create_dir_all(&new_dir);
        let _ = std::fs::create_dir_all(new_dir.join("presets"));
        if let Ok(entries) = std::fs::read_dir(old_dir.join("presets")) {
            for entry in entries.flatten() {
                let target = new_dir.join("presets").join(entry.file_name());
                let _ = std::fs::copy(entry.path(), target);
            }
        }
        for file in ["app_state.bin", "last_preset.txt", "last_output_device.txt"] {
            let src = old_dir.join(file);
            let dst = new_dir.join(file);
            if src.exists() && !dst.exists() {
                let _ = std::fs::copy(src, dst);
            }
        }
    }
    let _ = std::fs::create_dir_all(new_dir.join("presets"));
    new_dir
}

/// Sanitizes a preset name into a safe `.req` filename.
pub fn sanitize_preset_filename(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric()
                || c == '-'
                || c == '_'
                || c == ' '
                || c == '+'
                || c == '('
                || c == ')'
            {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.req", s.trim())
}

/// Saves a named preset from the preset manager to `~/.romueq/presets/<name>.req`.
pub fn persist_preset_to_disk(
    preset_manager: &mut PresetManager,
    name: &str,
) -> Result<(), String> {
    let path = user_config_dir()
        .join("presets")
        .join(sanitize_preset_filename(name));
    let path_str = path
        .to_str()
        .ok_or_else(|| "Invalid UTF-8 path".to_string())?;
    preset_manager
        .save_to_file(name, path_str)
        .map_err(|e| e.to_string())
}

/// Removes a preset file `~/.romueq/presets/<name>.req` from disk.
pub fn remove_preset_from_disk(name: &str) {
    let path = user_config_dir()
        .join("presets")
        .join(sanitize_preset_filename(name));
    let _ = std::fs::remove_file(path);
}

/// Saves the last active preset name atomically to `~/.equaform/last_preset.txt`.
pub fn save_last_active_preset_name(name: &str) {
    let path = user_config_dir().join("last_preset.txt");
    let _ = equaform_common::state::atomic_write(path, name.as_bytes());
}

/// Loads the last active preset name from `~/.romueq/last_preset.txt`.
pub fn load_last_active_preset_name() -> Option<String> {
    let path = user_config_dir().join("last_preset.txt");
    equaform_common::state::read_to_string_bounded(
        path,
        equaform_common::state::MAX_TEXT_FILE_BYTES,
    )
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

/// Standard factory preset names (including localized aliases).
pub const FACTORY_PRESETS: &[&str] = &[
    "Flat",
    "Bass Boost (+6 dB)",
    "Electronic / EDM",
    "Rock / Metal",
    "Hip-Hop / R&B",
    "Jazz / Warmth",
    "Modern Pop",
    "Acoustic Clarity",
    "Vocal Air",
    "Podcast / Voice",
    "Cinema / Dialog",
    "Gaming / FPS",
    "Audiophile (Harman)",
    "Night Listening",
    "Loudness",
    "HS7 Monitor Correction",
    // Legacy French aliases for backward compatibility with existing states
    "Électro / EDM",
    "Pop Moderne",
    "Podcast / Voix",
    "Cinéma / Dialogues",
    "Casque Audiophile (Harman)",
    "Écoute Nocturne (Night Mode)",
];

/// Returns `true` if `name` corresponds to a built-in factory preset.
pub fn is_factory_preset(name: &str) -> bool {
    FACTORY_PRESETS.contains(&name)
}

/// Populates the preset manager with the 16 standard factory presets.
pub fn populate_factory_presets(preset_manager: &mut PresetManager) {
    let mut add = |name: &str, configure: fn(&mut EqParameters)| {
        let mut params = EqParameters {
            bands: default_8_bands(),
            ..Default::default()
        };
        configure(&mut params);

        let mut preset = Preset::new(name);
        preset
            .parameter_values
            .set(1, if params.global.bypass { 1.0 } else { 0.0 });
        preset.parameter_values.set(2, params.global.output_gain_db);
        preset
            .parameter_values
            .set(3, if params.global.auto_gain { 1.0 } else { 0.0 });

        for band_idx in 0..equaform_common::MAX_BANDS {
            let base = 100u32 + band_idx as u32 * 100;
            let band = &params.bands[band_idx];
            preset
                .parameter_values
                .set(base, if band.enabled { 1.0 } else { 0.0 });
            preset
                .parameter_values
                .set(base + 1, band.filter_type.to_raw() as f32);
            preset.parameter_values.set(base + 2, band.frequency_hz);
            preset.parameter_values.set(base + 3, band.gain_db);
            preset.parameter_values.set(base + 4, band.q);
            preset
                .parameter_values
                .set(base + 5, band.slope.to_raw() as f32);
            preset
                .parameter_values
                .set(base + 7, params.band_routing(band_idx).to_raw() as f32);
        }

        preset_manager.remove(name);
        let _ = preset_manager.add(name.to_string(), preset);
    };

    add("Flat", |_| {});
    add("Bass Boost (+6 dB)", |p| {
        p.bands[0].gain_db = 6.0;
        p.bands[1].gain_db = 3.5;
    });
    add("Electronic / EDM", |p| {
        p.bands[0].gain_db = 5.5;
        p.bands[1].gain_db = 4.0;
        p.bands[3].gain_db = -2.5;
        p.bands[5].gain_db = 1.5;
        p.bands[7].gain_db = 4.5;
    });
    add("Rock / Metal", |p| {
        p.bands[0].gain_db = 2.0;
        p.bands[1].gain_db = 4.5;
        p.bands[2].gain_db = -1.5;
        p.bands[3].gain_db = -3.0;
        p.bands[5].gain_db = 3.5;
        p.bands[6].gain_db = 2.5;
        p.bands[7].gain_db = 2.0;
    });
    add("Hip-Hop / R&B", |p| {
        p.bands[0].gain_db = 7.0;
        p.bands[1].gain_db = 4.5;
        p.bands[2].gain_db = 1.0;
        p.bands[3].gain_db = -2.0;
        p.bands[5].gain_db = 1.5;
        p.bands[7].gain_db = 3.0;
    });
    add("Jazz / Warmth", |p| {
        p.bands[0].gain_db = 1.5;
        p.bands[1].gain_db = 2.5;
        p.bands[2].gain_db = 3.0;
        p.bands[3].gain_db = 1.5;
        p.bands[5].gain_db = -1.0;
        p.bands[6].gain_db = -2.0;
        p.bands[7].gain_db = -2.5;
    });
    add("Modern Pop", |p| {
        p.bands[0].gain_db = 4.0;
        p.bands[1].gain_db = 3.0;
        p.bands[3].gain_db = -2.0;
        p.bands[4].gain_db = 1.0;
        p.bands[5].gain_db = 2.5;
        p.bands[6].gain_db = 3.0;
        p.bands[7].gain_db = 4.0;
    });
    add("Acoustic Clarity", |p| {
        p.bands[1].gain_db = -2.0;
        p.bands[4].gain_db = 2.5;
        p.bands[5].gain_db = 3.5;
        p.bands[7].gain_db = 4.0;
    });
    add("Vocal Air", |p| {
        p.bands[0].gain_db = -4.0;
        p.bands[1].gain_db = -2.5;
        p.bands[4].gain_db = 2.0;
        p.bands[5].gain_db = 3.5;
        p.bands[7].gain_db = 5.0;
    });
    add("Podcast / Voice", |p| {
        p.bands[0].gain_db = -8.0;
        p.bands[1].gain_db = -4.0;
        p.bands[2].gain_db = -2.0;
        p.bands[4].gain_db = 2.5;
        p.bands[5].gain_db = 4.0;
        p.bands[7].gain_db = 1.5;
    });
    add("Cinema / Dialog", |p| {
        p.bands[0].gain_db = 5.0;
        p.bands[1].gain_db = 2.0;
        p.bands[3].gain_db = -2.5;
        p.bands[4].gain_db = 2.0;
        p.bands[5].gain_db = 3.5;
        p.bands[7].gain_db = 2.0;
    });
    add("Gaming / FPS", |p| {
        p.bands[0].gain_db = -6.0;
        p.bands[1].gain_db = -3.0;
        p.bands[2].gain_db = -1.5;
        p.bands[4].gain_db = 3.0;
        p.bands[5].gain_db = 5.0;
        p.bands[6].gain_db = 4.0;
        p.bands[7].gain_db = -2.0;
    });
    add("Audiophile (Harman)", |p| {
        p.bands[0].gain_db = 4.5;
        p.bands[1].gain_db = 2.5;
        p.bands[2].gain_db = -0.5;
        p.bands[5].gain_db = 2.0;
        p.bands[6].gain_db = 1.0;
        p.bands[7].gain_db = 1.5;
    });
    add("Night Listening", |p| {
        p.bands[0].gain_db = -9.0;
        p.bands[1].gain_db = -5.0;
        p.bands[2].gain_db = -2.0;
        p.bands[4].gain_db = 2.0;
        p.bands[5].gain_db = 2.5;
        p.bands[6].gain_db = -2.0;
        p.bands[7].gain_db = -4.0;
    });
    add("Loudness", |p| {
        p.bands[0].gain_db = 5.0;
        p.bands[1].gain_db = 3.0;
        p.bands[3].gain_db = -2.5;
        p.bands[7].gain_db = 4.0;
    });
    // HS7 Monitor Correction: compensates measured HS7 response deviations
    // based on ASR/spinorama data. Compensates low-mid buildup (125 Hz-1 kHz)
    // and high-frequency dip (6-10 kHz).
    add("HS7 Monitor Correction", |p| {
        p.bands[0].gain_db = 0.5; // 32 Hz LowShelf: slight sub-bass
        p.bands[1].gain_db = -1.0; // 75 Hz: reduce wall-coupling bass
        p.bands[2].gain_db = 1.5; // 180 Hz: ASR peak correction
        p.bands[3].gain_db = -3.5; // 420 Hz: ASR dip -4.13 dB @ 896 Hz
        p.bands[4].gain_db = -2.0; // 1 kHz: low-mid buildup reduction
        p.bands[5].gain_db = 1.5; // 2.4 kHz: ASR peak @ 1712 Hz
        p.bands[6].gain_db = -2.5; // 5.6 kHz: ASR dip -3.04 dB @ 3942 Hz
        p.bands[7].gain_db = -3.0; // 13.5 kHz HighShelf: ASR dip -3.6 dB @ 12184 Hz
    });

    // Load any user-saved presets from presets directory
    let presets_dir = user_config_dir().join("presets");
    if let Ok(entries) = std::fs::read_dir(&presets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("req") {
                if let Some(p_str) = path.to_str() {
                    let _ = preset_manager.load_from_file(p_str);
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Also load from %ProgramData%\Equaform\presets if present
        if let Ok(progdata) = std::env::var("ProgramData") {
            let common_presets = std::path::PathBuf::from(progdata)
                .join("Equaform")
                .join("presets");
            if common_presets.exists() && common_presets != presets_dir {
                if let Ok(entries) = std::fs::read_dir(common_presets) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("req") {
                            if let Some(p_str) = path.to_str() {
                                let _ = preset_manager.load_from_file(p_str);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Case-insensitive comparator for preset names with exact-case tie-breaking.
fn cmp_preset_name(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

/// Comparator for factory preset names where `"Flat"` always sorts first,
/// followed by case-insensitive alphabetical order.
fn cmp_factory_preset_name(a: &str, b: &str) -> std::cmp::Ordering {
    (a != "Flat")
        .cmp(&(b != "Flat"))
        .then_with(|| cmp_preset_name(a, b))
}

/// Partitions preset names into sorted `(user_presets, factory_presets)`.
///
/// - User presets are sorted case-insensitively A-Z (with exact-case tie-break).
/// - Factory presets place `"Flat"` first, followed by remaining factory presets
///   sorted case-insensitively A-Z (with exact-case tie-break).
pub fn partition_and_sort_presets<'a>(
    names: impl IntoIterator<Item = &'a str>,
) -> (Vec<String>, Vec<String>) {
    let mut user = Vec::new();
    let mut factory = Vec::new();

    for name in names {
        if is_factory_preset(name) {
            factory.push(name.to_string());
        } else {
            user.push(name.to_string());
        }
    }

    user.sort_by(|a, b| cmp_preset_name(a, b));
    factory.sort_by(|a, b| cmp_factory_preset_name(a, b));

    (user, factory)
}

/// Returns all preset names in display order: user presets (A-Z) followed by
/// factory presets (`"Flat"` first, then A-Z).
pub fn sorted_preset_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let (mut user, factory) = partition_and_sort_presets(names);
    user.extend(factory);
    user
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_user_and_factory_presets_sorted_correctly() {
        let names = [
            "Gamma",
            "Vocal Air",
            "alpha",
            "Flat",
            "Beta",
            "Acoustic Clarity",
            "Rock / Metal",
        ];

        let (user, factory) = partition_and_sort_presets(names);

        assert_eq!(user, vec!["alpha", "Beta", "Gamma"]);
        assert_eq!(
            factory,
            vec!["Flat", "Acoustic Clarity", "Rock / Metal", "Vocal Air"]
        );
    }

    #[test]
    fn case_insensitive_tie_breaking() {
        let names = ["beta", "Alpha", "alpha", "BETA"];
        let (user, factory) = partition_and_sort_presets(names);

        assert_eq!(user, vec!["Alpha", "alpha", "BETA", "beta"]);
        assert!(factory.is_empty());
    }

    #[test]
    fn empty_user_only_and_factory_only_inputs() {
        let (u0, f0) = partition_and_sort_presets(std::iter::empty());
        assert!(u0.is_empty());
        assert!(f0.is_empty());

        let (u1, f1) = partition_and_sort_presets(["Zeta", "alpha", "Mid"]);
        assert_eq!(u1, vec!["alpha", "Mid", "Zeta"]);
        assert!(f1.is_empty());

        let (u2, f2) = partition_and_sort_presets(["Vocal Air", "Flat", "Loudness"]);
        assert!(u2.is_empty());
        assert_eq!(f2, vec!["Flat", "Loudness", "Vocal Air"]);
    }

    #[test]
    fn sorted_preset_names_concatenates_user_then_factory() {
        let names = ["perso 2", "Vocal Air", "perso", "Flat", "Acoustic Clarity"];
        let result = sorted_preset_names(names);

        assert_eq!(
            result,
            vec!["perso", "perso 2", "Flat", "Acoustic Clarity", "Vocal Air"]
        );
    }
}
