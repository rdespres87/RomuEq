//! macOS CoreAudio System Output Switcher and Driver Installer.
//!
//! Allows Equaform to transparently route macOS system audio into its virtual HAL driver,
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
    use std::sync::Once;
    use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

    const SIGHUP: i32 = 1;
    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;

    static SIGNAL_PIPE_WRITE_FD: AtomicI32 = AtomicI32::new(-1);
    static RESTORE_DEVICE_ID: AtomicU32 = AtomicU32::new(0);
    static SIGNAL_HANDLER_INIT: Once = Once::new();

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

    const EMBEDDED_EQUAFORM_DRIVER_PKG: &[u8] = include_bytes!("../resources/EquaformDriver.pkg");

    unsafe extern "C" {
        fn atexit(cb: extern "C" fn()) -> i32;
        fn pipe(fds: *mut i32) -> i32;
        fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
        fn close(fd: i32) -> i32;
        fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
    }

    extern "C" fn handle_termination_signal(sig: i32) {
        let fd = SIGNAL_PIPE_WRITE_FD.load(Ordering::Relaxed);
        if fd >= 0 {
            let byte = (sig & 0xFF) as u8;
            // POSIX guarantees `write` on a pipe fd is async-signal-safe.
            unsafe {
                let _ = write(fd, &byte as *const u8, 1);
            }
        }
    }

    fn register_posix_signal_handlers() {
        unsafe {
            let _ = signal(SIGINT, handle_termination_signal);
            let _ = signal(SIGTERM, handle_termination_signal);
            let _ = signal(SIGHUP, handle_termination_signal);
        }
    }

    /// Install POSIX `SIGINT`/`SIGTERM`/`SIGHUP` handlers using an async-signal-safe
    /// self-pipe so `Ctrl+C` cleanly restores macOS CoreAudio output before exiting.
    pub fn install_signal_handlers() {
        SIGNAL_HANDLER_INIT.call_once(|| {
            let mut fds = [-1i32; 2];
            if unsafe { pipe(fds.as_mut_ptr()) } == 0 {
                let read_fd = fds[0];
                let write_fd = fds[1];
                SIGNAL_PIPE_WRITE_FD.store(write_fd, Ordering::Release);
                let _ = std::thread::Builder::new()
                    .name("equaform-signal-watcher".to_string())
                    .spawn(move || {
                        loop {
                            let mut byte = 0u8;
                            let n = unsafe { read(read_fd, &mut byte as *mut u8, 1) };
                            if n > 0 {
                                unsafe {
                                    let _ = close(read_fd);
                                }
                                eprintln!(
                                    "[Equaform-8] Signal {byte} received — initiating graceful shutdown..."
                                );
                                // Trigger normal eframe close -> on_exit() -> audio.stop() -> restore output.
                                if equaform_gui::standalone::request_graceful_shutdown() {
                                    std::thread::sleep(std::time::Duration::from_millis(1500));
                                }
                                // Fallback if window was not open yet or event loop did not exit in time.
                                restore_on_process_exit();
                                let code = 128 + i32::from(byte);
                                std::process::exit(code);
                            }
                            if n == 0 {
                                unsafe {
                                    let _ = close(read_fd);
                                }
                                break;
                            }
                            // n < 0 (e.g. EINTR when signal lands on this thread): retry read.
                        }
                    });
            }
        });
        // Always (re-)register signal dispositions in case winit/eframe/AppKit overrode them.
        register_posix_signal_handlers();
    }

    fn saved_output_device_path() -> std::path::PathBuf {
        let base = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let new_dir = std::path::PathBuf::from(&base).join(".equaform");
        let _ = std::fs::create_dir_all(&new_dir);
        let new_file = new_dir.join("last_output_device.txt");
        if !new_file.exists() {
            let old_file = std::path::PathBuf::from(&base)
                .join(".romueq")
                .join("last_output_device.txt");
            if old_file.exists() {
                let _ = std::fs::copy(&old_file, &new_file);
            }
        }
        new_file
    }

    fn load_saved_output_device() -> Option<String> {
        std::fs::read_to_string(saved_output_device_path())
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn save_selected_output_device(name: &str) {
        let _ = equaform_common::state::atomic_write(saved_output_device_path(), name.as_bytes());
    }

    extern "C" fn restore_on_process_exit() {
        // Restore user's selected physical output (or cached original physical device ID).
        let target_id = find_physical_output_device()
            .map(|(id, _)| id)
            .or_else(|| {
                let cached = RESTORE_DEVICE_ID.load(Ordering::Acquire);
                (cached > 0).then_some(cached)
            });

        if let Some(phys_id) = target_id {
            let _ = set_default_output_device(phys_id);
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

    /// Check whether a device has output streams (i.e. can output audio).
    pub fn device_has_output_streams(device_id: u32) -> bool {
        let addr = AudioObjectPropertyAddress {
            mSelector: u32::from_be_bytes(*b"strm"),
            mScope: u32::from_be_bytes(*b"outp"),
            mElement: 0,
        };
        let mut size: u32 = 0;
        let status = unsafe {
            AudioObjectGetPropertyDataSize(device_id, &addr, 0, std::ptr::null(), &mut size)
        };
        status == 0 && size > 0
    }

    /// Check if a device name looks like a virtual loopback device rather than physical hardware.
    pub fn is_virtual_device_name(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("equaform")
            || lower.contains("romueq")
            || lower.contains("soundflower")
            || lower.contains("zoom")
            || lower.contains("teams")
    }

    /// Resolves a physical audio output device dynamically across all system locales.
    /// Prioritizes saved selection, then system default output if non-virtual,
    /// and falls back to the first non-virtual device with active output streams.
    pub fn find_physical_output_device() -> Option<(u32, String)> {
        // 1. Saved device from previous user selection
        if let Some(saved) = load_saved_output_device() {
            if let Some((id, name)) = find_device_by_name(&saved) {
                if !is_virtual_device_name(&name) && device_has_output_streams(id) {
                    return Some((id, name));
                }
            }
        }

        // 2. Current system default output device if non-virtual
        if let Some(cur_id) = get_default_output_device_id() {
            if let Some(name) = get_device_name(cur_id) {
                if !is_virtual_device_name(&name) && device_has_output_streams(cur_id) {
                    return Some((cur_id, name));
                }
            }
        }

        // 3. First non-virtual device with output streams
        for (id, name) in list_audio_devices() {
            if !is_virtual_device_name(&name) && device_has_output_streams(id) {
                return Some((id, name));
            }
        }

        None
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

    /// Check whether the Equaform native driver bundle is present on the filesystem and up to date (version >= 2 / 1.1.0).
    pub fn is_equaform_driver_installed_on_disk() -> bool {
        let plist_path = "/Library/Audio/Plug-Ins/HAL/Equaform.driver/Contents/Info.plist";
        if let Ok(content) = std::fs::read_to_string(plist_path) {
            content.contains("<string>1.1.0</string>") || content.contains("<string>2</string>")
        } else {
            false
        }
    }

    /// Check whether the legacy RomuEq native driver bundle is present on the filesystem.
    #[allow(dead_code)]
    pub fn is_romueq_driver_installed_on_disk() -> bool {
        let plist_path = "/Library/Audio/Plug-Ins/HAL/RomuEq.driver/Contents/Info.plist";
        if let Ok(content) = std::fs::read_to_string(plist_path) {
            content.contains("<string>1.1.0</string>") || content.contains("<string>2</string>")
        } else {
            false
        }
    }

    /// Check if either the Equaform or legacy RomuEq driver is installed on disk.
    pub fn is_driver_installed_on_disk() -> bool {
        is_equaform_driver_installed_on_disk()
    }

    /// Check if the native Equaform CoreAudio HAL driver (`EquaformDriver_Create`) is installed and recognized by CoreAudio.
    #[allow(dead_code)]
    pub fn is_equaform_driver_available() -> bool {
        let is_native_bundle =
            std::fs::read("/Library/Audio/Plug-Ins/HAL/Equaform.driver/Contents/Info.plist")
                .map(|bytes| {
                    bytes
                        .windows(b"EquaformDriver_Create".len())
                        .any(|w| w == b"EquaformDriver_Create")
                })
                .unwrap_or(false);

        is_native_bundle && find_device_by_name("Equaform-8").is_some()
    }

    #[allow(dead_code)]
    pub fn is_romueq_driver_available() -> bool {
        is_equaform_driver_available() || {
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
    }

    /// Install the native Equaform virtual audio driver via macOS administrator dialog.
    /// Cleans up legacy RomuEq.driver during installation.
    pub fn install_equaform_driver() -> Result<(), String> {
        let tmp_path = "/tmp/EquaformDriver.pkg";
        std::fs::write(tmp_path, EMBEDDED_EQUAFORM_DRIVER_PKG)
            .map_err(|e| format!("Impossible d'écrire le paquet d'installation temporaire: {e}"))?;

        let script = format!(
            "do shell script \"rm -rf /Library/Audio/Plug-Ins/HAL/RomuEq.driver /Library/Audio/Plug-Ins/HAL/Equaform.driver && installer -pkg {tmp_path} -target / && killall coreaudiod\" with administrator privileges with prompt \"Equaform-8 souhaite installer son pilote CoreAudio natif 'Equaform-8'.\""
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
    /// Restores the chosen hardware device when dropped (app quit, panic, or `SIGINT`/`SIGTERM`).
    pub struct SystemAudioRoutingGuard {
        pub original_device_id: Option<u32>,
        pub original_device_name: Option<String>,
        pub virtual_device_id: Option<u32>,
        pub virtual_device_name: Option<String>,
        pub routing_active: bool,
    }

    impl SystemAudioRoutingGuard {
        /// Detect the original physical output, install exit/signal handlers, and sync the
        /// virtual device sample rate without switching the macOS default output yet.
        ///
        /// Call [`Self::activate_routing`] once the audio pass-through stream has started
        /// to eliminate any silence gap during window/GPU initialization.
        pub fn prepare() -> Self {
            let mut guard = Self {
                original_device_id: None,
                original_device_name: None,
                virtual_device_id: None,
                virtual_device_name: None,
                routing_active: false,
            };

            unsafe {
                let _ = atexit(restore_on_process_exit);
            }
            install_signal_handlers();

            // Detect current default output in macOS before rerouting
            if let Some(cur_id) = get_default_output_device_id() {
                let name = get_device_name(cur_id).unwrap_or_else(|| "Inconnu".to_string());
                // If macOS is currently set to a physical device, preserve it and sync it with saved output
                if !is_virtual_device_name(&name) {
                    guard.original_device_id = Some(cur_id);
                    guard.original_device_name = Some(name.clone());
                    RESTORE_DEVICE_ID.store(cur_id, Ordering::Release);
                    save_selected_output_device(&name);
                } else {
                    // macOS was already on Equaform/RomuEq: resolve physical output device dynamically
                    if let Some((phys_id, phys_name)) = find_physical_output_device() {
                        guard.original_device_id = Some(phys_id);
                        guard.original_device_name = Some(phys_name);
                        RESTORE_DEVICE_ID.store(phys_id, Ordering::Release);
                    }
                }
            }

            // Look for "Equaform-8" (or legacy "RomuEq") virtual device, with a short micro-poll if HAL driver was just loaded
            let mut target_virtual_device =
                find_device_by_name("Equaform-8").or_else(|| find_device_by_name("RomuEq"));
            if target_virtual_device.is_none() && is_driver_installed_on_disk() {
                for _ in 0..5 {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    target_virtual_device =
                        find_device_by_name("Equaform-8").or_else(|| find_device_by_name("RomuEq"));
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
                            "[Equaform-8] Synchronizing {virt_name} to hardware sample rate: {target_sr} Hz"
                        );
                        let _ = set_device_sample_rate(virt_id, target_sr);
                        // Verify property take-effect in CoreAudio HAL with fast 10ms intervals
                        let mut verified = false;
                        for _ in 0..20 {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            if let Some(new_sr) = get_device_sample_rate(virt_id) {
                                if (new_sr - target_sr).abs() < 1.0 {
                                    println!(
                                        "[Equaform-8] {virt_name} sample rate verified and locked at {target_sr} Hz"
                                    );
                                    verified = true;
                                    break;
                                }
                            }
                        }
                        if !verified {
                            eprintln!(
                                "[Equaform-8] Warning: CoreAudio HAL sample rate synchronization timed out. Proceeding with routing."
                            );
                        }
                    } else {
                        println!(
                            "[Equaform-8] {virt_name} sample rate already locked to hardware: {target_sr} Hz"
                        );
                    }
                }

                guard.virtual_device_id = Some(virt_id);
                guard.virtual_device_name = Some(virt_name);
            }

            guard
        }

        /// Switch macOS system default output to the prepared virtual device.
        pub fn activate_routing(&mut self) {
            // Re-assert POSIX signal handlers after eframe/winit/cpal initialization.
            install_signal_handlers();
            if self.routing_active {
                return;
            }
            if let (Some(virt_id), Some(virt_name)) =
                (self.virtual_device_id, self.virtual_device_name.as_deref())
            {
                if let Err(e) = set_default_output_device(virt_id) {
                    eprintln!("[Equaform-8] Failed to route system output to {virt_name}: {e}");
                } else {
                    println!(
                        "[Equaform-8] macOS system audio successfully routed to {virt_name} (ID: {virt_id})"
                    );
                    self.routing_active = true;
                }
            }
        }

        /// Prepare and immediately intercept system audio if Equaform-8 is available.
        #[allow(dead_code)]
        pub fn activate() -> Self {
            let mut guard = Self::prepare();
            guard.activate_routing();
            guard
        }

        /// Restore the chosen physical output device (or original default output device).
        pub fn restore(&mut self) {
            if self.routing_active {
                // Prefer the output device currently chosen inside Equaform-8
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
                            "[Equaform-8] Échec de restauration de la sortie système ({target_name}): {e}"
                        );
                    } else {
                        println!(
                            "[Equaform-8] Sortie système macOS restaurée sur {target_name} (ID: {target_id})"
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
        let equaform = find_device_by_name("Equaform");
        println!("Equaform search: {equaform:?}");

        use coreaudio_sys::{AudioObjectGetPropertyData, AudioObjectPropertyAddress};
        for (name, dev_opt) in [("Komplete", &komplete), ("Equaform", &equaform)] {
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
        let on_disk = is_driver_installed_on_disk() || is_romueq_driver_installed_on_disk();
        println!("Driver installed on disk: {on_disk}");
        if on_disk {
            println!(
                "Driver available: {}",
                is_equaform_driver_available() || is_romueq_driver_available()
            );
        }
    }
}
