//! Headless / fallback stub editor for non-GUI platforms or headless tests.

use clap_sys::ext::gui::clap_window;
use clap_sys::host::clap_host;
use std::sync::Arc;

use super::PluginEditor;
use crate::EquaformSharedInner;

pub struct DummyEditor;

impl DummyEditor {
    pub fn new(_shared: Arc<EquaformSharedInner>, _host: Option<*const clap_host>) -> Self {
        Self
    }
}

impl PluginEditor for DummyEditor {
    fn create(&mut self, _is_floating: bool) -> bool {
        false
    }

    unsafe fn set_parent(&mut self, _window: *const clap_window) -> bool {
        false
    }

    fn show(&mut self) -> bool {
        false
    }

    fn hide(&mut self) -> bool {
        false
    }

    fn destroy(&mut self) {}
}
