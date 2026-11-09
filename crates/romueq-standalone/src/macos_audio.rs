//! macOS CoreAudio System Output Switcher and Driver Installer.
//!
//! Allows RomuEq to transparently route macOS system audio into BlackHole 2ch,
//! apply real-time 8-band parametric EQ, and restore the original physical
//! output (e.g. Komplete Audio 6 MK2) upon exit.

#[cfg(target_os = "macos")]
pub mod macos_impl {
    use coreaudio_sys::{
        AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectPropertyAddress,
        AudioObjectSetPropertyData,
    };
    use std::ffi::CStr;
    use std::os::raw::c_char;

    const SYSTEM_OBJECT: u32 = 1;

    const PROPERTY_DEFAULT_OUTPUT: AudioObjectPropertyAddress = AudioObjectPropertyAddress {
        mSelector: u32::from_be_bytes(*b"dOut"),
        mScope: u32::from_be_bytes(*b"glob"),
        mElement: 0,
    };

    const PROPERTY_DEVICES: AudioObjectPropertyAddress = AudioObjectPropertyAddress {
        mSelector: u32::from_be_bytes(*b"dev#"),
        mScope: u32::from_be_bytes(*b"glob"),
        mElement: 0,
    };

    const EMBEDDED_BLACKHOLE_PKG: &[u8] = include_bytes!("../resources/BlackHole2ch.pkg");

    /// Get the current macOS system default output device ID.
    pub fn get_default_output_device_id() -> Option<u32> {
        let mut device_id: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;

        let status = unsafe {
            AudioObjectGetPropertyData(
                SYSTEM_OBJECT,
                &PROPERTY_DEFAULT_OUTPUT,
                0,
                std::ptr::null(),
                &mut size,
                &mut device_id as *mut _ as *mut _,
            )
        };

        if status == 0 && device_id > 0 {
            Some(device_id)
        } else {
            None
        }
    }

    /// Set the macOS system default output device ID.
    pub fn set_default_output_device(device_id: u32) -> Result<(), String> {
        let size = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            AudioObjectSetPropertyData(
                SYSTEM_OBJECT,
                &PROPERTY_DEFAULT_OUTPUT,
                0,
                std::ptr::null(),
                size,
                &device_id as *const _ as *const _,
            )
        };

        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "Échec de configuration du périphérique de sortie {device_id}: OSStatus {status}"
            ))
        }
    }

    /// Get the human-readable name of an AudioDeviceID.
    pub fn get_device_name(device_id: u32) -> Option<String> {
        let address = AudioObjectPropertyAddress {
            mSelector: u32::from_be_bytes(*b"name"),
            mScope: u32::from_be_bytes(*b"glob"),
            mElement: 0,
        };

        let mut name_buffer = [0 as c_char; 256];
        let mut size = name_buffer.len() as u32;

        let status = unsafe {
            AudioObjectGetPropertyData(
                device_id,
                &address,
                0,
                std::ptr::null(),
                &mut size,
                name_buffer.as_mut_ptr() as *mut _,
            )
        };

        if status == 0 {
            let cstr = unsafe { CStr::from_ptr(name_buffer.as_ptr()) };
            cstr.to_str().ok().map(|s| s.trim().to_string())
        } else {
            None
        }
    }

    /// List all audio devices available in the system as (id, name).
    pub fn list_audio_devices() -> Vec<(u32, String)> {
        let mut size: u32 = 0;
        let status = unsafe {
            AudioObjectGetPropertyDataSize(
                SYSTEM_OBJECT,
                &PROPERTY_DEVICES,
                0,
                std::ptr::null(),
                &mut size,
            )
        };

        if status != 0 || size == 0 {
            return Vec::new();
        }

        let count = size as usize / std::mem::size_of::<u32>();
        let mut device_ids = vec![0u32; count];

        let status = unsafe {
            AudioObjectGetPropertyData(
                SYSTEM_OBJECT,
                &PROPERTY_DEVICES,
                0,
                std::ptr::null(),
                &mut size,
                device_ids.as_mut_ptr() as *mut _,
            )
        };

        if status != 0 {
            return Vec::new();
        }

        let mut result = Vec::new();
        for id in device_ids {
            if let Some(name) = get_device_name(id) {
                result.push((id, name));
            }
        }
        result
    }

    /// Find an audio device whose name contains the given query (case-insensitive).
    pub fn find_device_by_name(query: &str) -> Option<(u32, String)> {
        let query_lower = query.to_lowercase();
        list_audio_devices()
            .into_iter()
            .find(|(_, name)| name.to_lowercase().contains(&query_lower))
    }

    /// Check if BlackHole 2ch driver is present and recognized by CoreAudio.
    pub fn is_blackhole_available() -> bool {
        find_device_by_name("BlackHole 2ch").is_some()
    }

    /// Install BlackHole 2ch driver automatically via native macOS administrator dialog.
    pub fn install_blackhole() -> Result<(), String> {
        let tmp_path = "/tmp/RomuEq_BlackHole2ch.pkg";
        std::fs::write(tmp_path, EMBEDDED_BLACKHOLE_PKG)
            .map_err(|e| format!("Impossible d'écrire le paquet d'installation temporaire: {e}"))?;

        let script = format!(
            "do shell script \"installer -pkg {tmp_path} -target / && killall -9 coreaudiod\" with administrator privileges with prompt \"RomuEq a besoin d'installer le composant audio BlackHole 2ch pour capturer le son du système macOS.\""
        );

        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("Erreur lors de l'appel système d'autorisation: {e}"))?;

        let _ = std::fs::remove_file(tmp_path);

        if output.status.success() {
            // Allow coreaudiod daemon 1.5 seconds to restart and register new devices
            std::thread::sleep(std::time::Duration::from_millis(1500));
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Autorisation refusée ou installation échouée: {stderr}"))
        }
    }

    /// Guard struct that saves original physical output and switches system output to BlackHole.
    /// Restores the original hardware device when dropped (app quit or panic).
    pub struct SystemAudioRoutingGuard {
        pub original_device_id: Option<u32>,
        pub original_device_name: Option<String>,
        pub routing_active: bool,
    }

    impl SystemAudioRoutingGuard {
        /// Create guard and intercept system audio if BlackHole is available.
        pub fn activate() -> Self {
            let mut guard = Self {
                original_device_id: None,
                original_device_name: None,
                routing_active: false,
            };

            // Detect current default output
            if let Some(cur_id) = get_default_output_device_id() {
                let name = get_device_name(cur_id).unwrap_or_else(|| "Inconnu".to_string());
                // Avoid saving BlackHole or eqMac as original physical device
                if !name.to_lowercase().contains("blackhole") && !name.to_lowercase().contains("eqmac") {
                    guard.original_device_id = Some(cur_id);
                    guard.original_device_name = Some(name.clone());
                } else {
                    // Search for a physical device (Komplete Audio or built-in speakers)
                    if let Some((phys_id, phys_name)) = find_device_by_name("Komplete Audio")
                        .or_else(|| find_device_by_name("Haut-parleurs"))
                    {
                        guard.original_device_id = Some(phys_id);
                        guard.original_device_name = Some(phys_name);
                    }
                }
            }

            // If BlackHole 2ch exists, switch macOS default output to it
            if let Some((bh_id, _)) = find_device_by_name("BlackHole 2ch") {
                if let Err(e) = set_default_output_device(bh_id) {
                    eprintln!("[RomuEq] Impossible de basculer la sortie système vers BlackHole: {e}");
                } else {
                    println!("[RomuEq] Sortie système macOS routée avec succès vers BlackHole 2ch (ID: {bh_id})");
                    guard.routing_active = true;
                }
            }

            guard
        }

        /// Restore the original default output device.
        pub fn restore(&mut self) {
            if self.routing_active {
                if let Some(orig_id) = self.original_device_id {
                    let orig_name = self.original_device_name.as_deref().unwrap_or("Périphérique d'origine");
                    if let Err(e) = set_default_output_device(orig_id) {
                        eprintln!("[RomuEq] Échec de restauration de la sortie système ({orig_name}): {e}");
                    } else {
                        println!("[RomuEq] Sortie système macOS restaurée sur {orig_name} (ID: {orig_id})");
                    }
                }
                self.routing_active = false;
            }
        }
    }

    impl Drop for SystemAudioRoutingGuard {
        fn drop(&mut self) {
            self.restore();
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::*;

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::macos_impl::*;

    #[test]
    fn test_list_and_default_device() {
        if let Some(def_id) = get_default_output_device_id() {
            let name = get_device_name(def_id).unwrap_or_else(|| "Unknown".to_string());
            println!("Default Output Device ID: {def_id} ({name})");
            assert!(def_id > 0);
        }
        let devices = list_audio_devices();
        println!("All system devices ({}):", devices.len());
        for (id, name) in &devices {
            println!("  [{id}] {name}");
        }
        assert!(!devices.is_empty());
    }

    #[test]
    fn test_find_device() {
        let komplete = find_device_by_name("Komplete");
        println!("Komplete search: {komplete:?}");
        let blackhole = find_device_by_name("BlackHole");
        println!("BlackHole search: {blackhole:?}");
    }
}
