//! macOS CoreAudio HAL sample-rate synchronization for the virtual loopback device.

use std::ffi::{CStr, c_char, c_void};

#[repr(C)]
struct AudioObjectPropertyAddress {
    m_selector: u32,
    m_scope: u32,
    m_element: u32,
}

const SYSTEM_OBJECT: u32 = 1;

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyDataSize(
        in_object_id: u32,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        out_data_size: *mut u32,
    ) -> i32;

    fn AudioObjectGetPropertyData(
        in_object_id: u32,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        io_data_size: *mut u32,
        out_data: *mut c_void,
    ) -> i32;

    fn AudioObjectSetPropertyData(
        in_object_id: u32,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const c_void,
        in_data_size: u32,
        in_data: *const c_void,
    ) -> i32;
}

fn get_device_name(device_id: u32) -> Option<String> {
    let addr = AudioObjectPropertyAddress {
        m_selector: u32::from_be_bytes(*b"name"),
        m_scope: u32::from_be_bytes(*b"glob"),
        m_element: 0,
    };
    let mut buf = [0 as c_char; 256];
    let mut size = buf.len() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            buf.as_mut_ptr() as *mut _,
        )
    };
    if status == 0 {
        let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
        cstr.to_str().ok().map(|s| s.trim().to_string())
    } else {
        None
    }
}

fn list_devices() -> Vec<(u32, String)> {
    let addr = AudioObjectPropertyAddress {
        m_selector: u32::from_be_bytes(*b"dev#"),
        m_scope: u32::from_be_bytes(*b"glob"),
        m_element: 0,
    };
    let mut size: u32 = 0;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(SYSTEM_OBJECT, &addr, 0, std::ptr::null(), &mut size)
    };
    if status != 0 || size == 0 {
        return Vec::new();
    }
    let count = size as usize / std::mem::size_of::<u32>();
    let mut ids = vec![0u32; count];
    let status = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM_OBJECT,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            ids.as_mut_ptr() as *mut _,
        )
    };
    if status != 0 {
        return Vec::new();
    }
    ids.into_iter()
        .filter_map(|id| get_device_name(id).map(|n| (id, n)))
        .collect()
}

pub fn get_device_sample_rate(device_id: u32) -> Option<f64> {
    let addr = AudioObjectPropertyAddress {
        m_selector: u32::from_be_bytes(*b"nsrt"),
        m_scope: u32::from_be_bytes(*b"glob"),
        m_element: 0,
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

pub fn get_device_sample_rate_by_name(query: &str) -> Option<f64> {
    let q = query.to_lowercase();
    list_devices()
        .into_iter()
        .find(|(_, n)| n.to_lowercase().contains(&q))
        .and_then(|(id, _)| get_device_sample_rate(id))
}

pub fn get_default_output_sample_rate() -> Option<f64> {
    let addr = AudioObjectPropertyAddress {
        m_selector: u32::from_be_bytes(*b"dOut"),
        m_scope: u32::from_be_bytes(*b"glob"),
        m_element: 0,
    };
    let mut dev_id: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM_OBJECT,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut dev_id as *mut _ as *mut _,
        )
    };
    if status == 0 && dev_id > 0 {
        get_device_sample_rate(dev_id)
    } else {
        None
    }
}

pub fn sync_virtual_input_rate(target_sr: f64) {
    if target_sr <= 0.0 {
        return;
    }
    let addr = AudioObjectPropertyAddress {
        m_selector: u32::from_be_bytes(*b"nsrt"),
        m_scope: u32::from_be_bytes(*b"glob"),
        m_element: 0,
    };
    let size = std::mem::size_of::<f64>() as u32;
    for (id, name) in list_devices() {
        let l = name.to_lowercase();
        if l.contains("equaform") || l == "romueq" {
            if let Some(cur) = get_device_sample_rate(id) {
                if (cur - target_sr).abs() > 1.0 {
                    let _ = unsafe {
                        AudioObjectSetPropertyData(
                            id,
                            &addr,
                            0,
                            std::ptr::null(),
                            size,
                            &target_sr as *const _ as *const _,
                        )
                    };
                    for _ in 0..10 {
                        if let Some(now) = get_device_sample_rate(id) {
                            if (now - target_sr).abs() < 1.0 {
                                break;
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(25));
                    }
                }
            }
        }
    }
}

pub fn restore_macos_default_output(target_name: &str) {
    let q = target_name.to_lowercase();
    if q.contains("equaform") || q.contains("romueq") {
        return;
    }
    let devices = list_devices();
    let found = devices
        .iter()
        .find(|(_, n)| n == target_name)
        .or_else(|| devices.iter().find(|(_, n)| n.to_lowercase().contains(&q)));
    if let Some((dev_id, _)) = found {
        let addr = AudioObjectPropertyAddress {
            m_selector: u32::from_be_bytes(*b"dOut"),
            m_scope: u32::from_be_bytes(*b"glob"),
            m_element: 0,
        };
        let size = std::mem::size_of::<u32>() as u32;
        let _ = unsafe {
            AudioObjectSetPropertyData(
                SYSTEM_OBJECT,
                &addr,
                0,
                std::ptr::null(),
                size,
                dev_id as *const _ as *const _,
            )
        };
    }
}
