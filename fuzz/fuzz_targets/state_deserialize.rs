#![no_main]

use equaform_common::state::{Preset, ProjectState};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = ProjectState::deserialize(data);
    let _ = Preset::deserialize(data);

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = Preset::from_json(text);
    }
});
