//! Backward-compatibility re-export for CocoaEditor.
//! The canonical implementation lives in `crate::editor::cocoa`.

#[cfg(target_os = "macos")]
pub use crate::editor::cocoa::CocoaEditor;

#[cfg(not(target_os = "macos"))]
pub use crate::editor::dummy::DummyEditor as CocoaEditor;

pub use crate::editor::{EDITOR_HEIGHT, EDITOR_WIDTH};
