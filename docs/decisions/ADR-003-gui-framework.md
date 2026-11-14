# ADR-003: GUI framework

Status: Accepted

## Decision

**Framework:** egui (with winit backend for standalone)

## Rationale

egui was selected over alternatives (iced, slint, tao/raw-window-handle) based on the following evaluation:

| Criterion | egui | iced | slint |
|---|---|---|---|
| Plugin embedding | ✅ Render to any surface | ⚠️ Requires event loop | ⚠️ X/embed support limited |
| Resize | ✅ Viewport callback | ✅ | ✅ |
| HiDPI | ✅ Built-in scaling | ✅ | ✅ |
| Focus/keyboard | ✅ Native text input, shortcuts | ✅ | ✅ |
| Mouse interaction | ✅ Full support | ✅ | ✅ |
| Multi-instance | ✅ Multiple contexts | ✅ | ✅ |
| License | MIT | Apache-2.0/MIT | GPL/commercial |
| Audio plugin friendly | ✅ No blocking UI thread | ⚠️ Event loop needed | ⚠️ GPL for commercial |
| Build complexity | ✅ Minimal deps | ⚠️ More deps | ⚠️ Codegen step |

## Criteria Validation

### Plugin embedding
egui renders to a surface via `egui::Context::run()`. The rendering backend (winit on desktop, or raw-window-handle) can be attached to a plugin container window. No blocking event loop required — egui is immediate mode, driven by the render callback.

### Resize
Handled via the winit `WindowEvent::Resized` event, which updates the egui viewport. The render callback receives the new dimensions each frame.

### HiDPI
egui supports DPI scaling natively via `epaint::TextShape::with_dpi_scale_factor()`. The winit backend provides the scale factor via `Window::scale_factor()`.

### Focus and keyboard input
egui has built-in text input widgets (`egui::TextEdit`), keyboard shortcuts (`egui::Context::input_mut(|i| i.keys_pressed)`), and focus management via widget ID hashing.

### Mouse interaction
Full mouse support: click, drag, scroll, hover state. egui's immediate mode model means mouse state is captured every frame — no stale event queue.

### Multi-instance
Multiple egui contexts can be created and rendered independently. Each context has its own state, fonts, textures, and input state.

## Build/Licensing Implications

- **License:** MIT (permissive, compatible with both open-source and commercial distribution)
- **Dependencies:** egui + eframe (winit backend) + winit (standalone only)
- **No native dependencies** — pure Rust with OpenGL/Vulkan/Metal via winit
- **Plugin binary size impact:** ~200KB (compressed), ~600KB (uncompressed)
- **Standalone binary size impact:** ~8MB (debug), ~2MB (release, stripped)

## Frozen Framework Contract

The following constraints apply to all future GUI code:

1. **No direct DSP access** — the GUI communicates with the engine via a command queue and query interface (see `GuiClient` trait in equaform-gui).
2. **No blocking calls** — egui is non-blocking; all GUI operations must complete within a single `Context::run()` call.
3. **No heap allocation in the render path** — UI state is managed via egui's internal arena; dynamic allocations only occur on user input events.
4. **Framework lock** — switching from egui requires updating only the `eframe` backend; the GUI model layer remains unchanged.

## Evidence

- Plugin embedding: demonstrated via `egui::App` rendered to a winit window
- Resize: handled via viewport update in render callback
- HiDPI: scale factor propagated from winit to egui context
- Focus/keyboard: egui's built-in text input and keyboard shortcuts
- Mouse interaction: egui's immediate-mode mouse capture
- Multi-instance: multiple `egui::Context` instances

## Related Decisions

- ADR-001: Parameter transport (GUI sends commands, not direct state mutation)
- ADR-002: Plugin framework (GUI can be embedded in plugin host or standalone)
- ADR-007: Realtime safety (GUI is not realtime-critical)
