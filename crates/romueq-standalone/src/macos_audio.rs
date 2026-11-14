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

    const EMBEDDED_ROMUEQ_DRIVER_PKG: &[u8] = include_bytes!("../resources/RomuEqDriver.pkg");

    unsafe extern "C" {
        fn atexit(cb: extern "C" fn()) -> i32;
    }

    fn saved_output_device_path() -> std::path::PathBuf {
        let base = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let dir = std::path::PathBuf::from(base).join(".romueq");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("last_output_device.txt")
    }

    fn load_saved_output_device() -> Option<String> {
        std::fs::read_to_string(saved_output_device_path())
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn save_selected_output_device(name: &str) {
        let _ = std::fs::write(saved_output_device_path(), name.as_bytes());
    }

    extern "C" fn restore_on_process_exit() {
        // If current macOS default output is RomuEq/BlackHole, restore to user's selected physical output
        if let Some(cur_id) = get_default_output_device_id() {
            let cur_name = get_device_name(cur_id).unwrap_or_default().to_lowercase();
            if cur_name.contains("romueq") || cur_name.contains("blackhole") {
                if let Some(saved) = load_saved_output_device() {
                    if let Some((phys_id, _)) = find_device_by_name(&saved) {
                        let _ = set_default_output_device(phys_id);
                        return;
                    }
                }
                if let Some((phys_id, _)) = find_device_by_name("Komplete Audio")
                    .or_else(|| find_device_by_name("Haut-parleurs"))
                    .or_else(|| find_device_by_name("MacBook"))
                {
                    let _ = set_default_output_device(phys_id);
                }
            }
        }
    }

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

    /// Set the nominal sample rate of an AudioDeviceID.
    pub fn set_device_sample_rate(device_id: u32, sample_rate: f64) -> Result<(), String> {
        let addr = AudioObjectPropertyAddress {
            mSelector: u32::from_be_bytes(*b"nsrt"),
            mScope: u32::from_be_bytes(*b"glob"),
            mElement: 0,
        };
        let size = std::mem::size_of::<f64>() as u32;
        let status = unsafe {
            AudioObjectSetPropertyData(
                device_id,
                &addr,
                0,
                std::ptr::null(),
                size,
                &sample_rate as *const _ as *const _,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!("Failed to set sample rate: {status}"))
        }
    }

    /// Get the nominal sample rate of an AudioDeviceID.
    pub fn get_device_sample_rate(device_id: u32) -> Option<f64> {
        let addr = AudioObjectPropertyAddress {
            mSelector: u32::from_be_bytes(*b"nsrt"),
            mScope: u32::from_be_bytes(*b"glob"),
            mElement: 0,
        };
        let mut sr: f64 = 0.0;
        let mut size = std::mem::size_of::<f64>() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(
                device_id,
                &addr,
                0,
                std::ptr::null(),
                &mut size,
                &mut sr as *mut _ as *mut _,
            )
        };
        if status == 0 && sr > 0.0 {
            Some(sr)
        } else {
            None
        }
    }

    /// Check whether the RomuEq native driver bundle is present on the filesystem.
    pub fn is_romueq_driver_installed_on_disk() -> bool {
        std::path::Path::new("/Library/Audio/Plug-Ins/HAL/RomuEq.driver/Contents/Info.plist")
            .exists()
    }

    /// Check if the native RomuEq CoreAudio HAL driver (`RomuEqDriver_Create`) is installed and recognized by CoreAudio.
    #[allow(dead_code)]
    pub fn is_romueq_driver_available() -> bool {
        let is_native_bundle =
            std::fs::read("/Library/Audio/Plug-Ins/HAL/RomuEq.driver/Contents/Info.plist")
                .map(|bytes| {
                    bytes
                        .windows(b"RomuEqDriver_Create".len())
                        .any(|w| w == b"RomuEqDriver_Create")
                })
                .unwrap_or(false);

        is_native_bundle && find_device_by_name("RomuEq").is_some()
    }

    /// Install the native RomuEq virtual audio driver via macOS administrator dialog.
    pub fn install_romueq_driver() -> Result<(), String> {
        let tmp_path = "/tmp/RomuEqDriver.pkg";
        std::fs::write(tmp_path, EMBEDDED_ROMUEQ_DRIVER_PKG)
            .map_err(|e| format!("Impossible d'écrire le paquet d'installation temporaire: {e}"))?;

        let script = format!(
            "do shell script \"rm -rf /Library/Audio/Plug-Ins/HAL/RomuEq.driver && installer -pkg {tmp_path} -target / && killall coreaudiod\" with administrator privileges with prompt \"RomuEq souhaite installer son pilote CoreAudio natif 'RomuEq'.\""
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
            Err(format!(
                "Autorisation refusée ou installation échouée: {stderr}"
            ))
        }
    }

    /// Guard struct that saves original physical output and switches system output to virtual device.
    /// Restores the chosen hardware device when dropped (app quit or panic).
    pub struct SystemAudioRoutingGuard {
        pub original_device_id: Option<u32>,
        pub original_device_name: Option<String>,
        pub routing_active: bool,
    }

    impl SystemAudioRoutingGuard {
        /// Create guard and intercept system audio if RomuEq is available.
        pub fn activate() -> Self {
            let mut guard = Self {
                original_device_id: None,
                original_device_name: None,
                routing_active: false,
            };

            unsafe {
                let _ = atexit(restore_on_process_exit);
            }

            // Detect current default output in macOS before rerouting
            if let Some(cur_id) = get_default_output_device_id() {
                let name = get_device_name(cur_id).unwrap_or_else(|| "Inconnu".to_string());
                let lower = name.to_lowercase();
                // If macOS is currently set to a physical device, preserve it and sync it with RomuEq's saved output
                if !lower.contains("romueq")
                    && !lower.contains("blackhole")
                    && !lower.contains("eqmac")
                {
                    guard.original_device_id = Some(cur_id);
                    guard.original_device_name = Some(name.clone());
                    save_selected_output_device(&name);
                } else {
                    // macOS was already on RomuEq (e.g. after a restart): check user's last selected physical output first
                    let saved_phys =
                        load_saved_output_device().and_then(|saved| find_device_by_name(&saved));
                    if let Some((phys_id, phys_name)) = saved_phys
                        .or_else(|| find_device_by_name("Komplete Audio"))
                        .or_else(|| find_device_by_name("Haut-parleurs"))
                        .or_else(|| find_device_by_name("MacBook"))
                    {
                        guard.original_device_id = Some(phys_id);
                        guard.original_device_name = Some(phys_name);
                    }
                }
            }

            // Look for "RomuEq" virtual device, with a short micro-poll if HAL driver was just loaded
            let mut target_virtual_device = find_device_by_name("RomuEq");
            if target_virtual_device.is_none() && is_romueq_driver_installed_on_disk() {
                for _ in 0..5 {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    target_virtual_device = find_device_by_name("RomuEq");
                    if target_virtual_device.is_some() {
                        break;
                    }
                }
            }

            if let Some((virt_id, virt_name)) = target_virtual_device {
                if let Some(target_sr) = guard.original_device_id.and_then(get_device_sample_rate) {
                    let current_sr = get_device_sample_rate(virt_id);
                    let needs_sync = match current_sr {
                        Some(sr) => (sr - target_sr).abs() >= 1.0,
                        None => true,
                    };

                    if needs_sync {
                        println!(
                            "[RomuEq] Synchronizing {virt_name} to hardware sample rate: {target_sr} Hz"
                        );
                        let _ = set_device_sample_rate(virt_id, target_sr);
                        // Verify property take-effect in CoreAudio HAL with fast 10ms intervals
                        let mut verified = false;
                        for _ in 0..20 {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            if let Some(new_sr) = get_device_sample_rate(virt_id) {
                                if (new_sr - target_sr).abs() < 1.0 {
                                    println!(
                                        "[RomuEq] {virt_name} sample rate verified and locked at {target_sr} Hz"
                                    );
                                    verified = true;
                                    break;
                                }
                            }
                        }
                        if !verified {
                            eprintln!(
                                "[RomuEq] Warning: CoreAudio HAL sample rate synchronization timed out. Proceeding with routing."
                            );
                        }
                    } else {
                        println!(
                            "[RomuEq] {virt_name} sample rate already locked to hardware: {target_sr} Hz"
                        );
                    }
                }

                if let Err(e) = set_default_output_device(virt_id) {
                    eprintln!("[RomuEq] Failed to route system output to {virt_name}: {e}");
                } else {
                    println!(
                        "[RomuEq] macOS system audio successfully routed to {virt_name} (ID: {virt_id})"
                    );
                    guard.routing_active = true;
                }
            }

            guard
        }

        /// Restore the chosen physical output device (or original default output device).
        pub fn restore(&mut self) {
            if self.routing_active {
                // Prefer the output device currently chosen inside RomuEq
                let target = load_saved_output_device()
                    .and_then(|saved| find_device_by_name(&saved))
                    .or_else(|| {
                        self.original_device_id.map(|id| {
                            (
                                id,
                                self.original_device_name
                                    .clone()
                                    .unwrap_or_else(|| "Périphérique d'origine".to_string()),
                            )
                        })
                    });

                if let Some((target_id, target_name)) = target {
                    if let Err(e) = set_default_output_device(target_id) {
                        eprintln!(
                            "[RomuEq] Échec de restauration de la sortie système ({target_name}): {e}"
                        );
                    } else {
                        println!(
                            "[RomuEq] Sortie système macOS restaurée sur {target_name} (ID: {target_id})"
                        );
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

        use coreaudio_sys::{AudioObjectGetPropertyData, AudioObjectPropertyAddress};
        for (name, dev_opt) in [("Komplete", &komplete), ("BlackHole", &blackhole)] {
            if let Some((id, _)) = dev_opt {
                let addr = AudioObjectPropertyAddress {
                    mSelector: u32::from_be_bytes(*b"nsrt"),
                    mScope: u32::from_be_bytes(*b"glob"),
                    mElement: 0,
                };
                let mut sr: f64 = 0.0;
                let mut size = std::mem::size_of::<f64>() as u32;
                let status = unsafe {
                    AudioObjectGetPropertyData(
                        *id,
                        &addr,
                        0,
                        std::ptr::null(),
                        &mut size,
                        &mut sr as *mut _ as *mut _,
                    )
                };
                println!(
                    "Device '{name}' (ID {id}) nominal sample rate: {sr} Hz (status: {status})"
                );
            }
        }
    }

    #[test]
    fn test_driver_installed_on_disk() {
        let on_disk = is_romueq_driver_installed_on_disk();
        println!("RomuEq driver installed on disk: {on_disk}");
        if on_disk {
            println!("Driver available: {}", is_romueq_driver_available());
        }
    }
}
