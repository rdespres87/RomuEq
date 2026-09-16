#![forbid(unsafe_code)]

//! GUI framework contract for RomuEq.
//!
//! This crate defines the interface between the GUI layer and the engine layer.
//! The GUI must communicate with the engine via commands and queries — no direct
//! DSP state access.
//!
//! ## Framework Decision (ADR-003)
//!
//! **egui** (with winit backend for standalone mode) is the selected GUI framework.
//!
//! ## Lifecycle Contract
//!
//! The GUI follows a four-phase lifecycle aligned with the engine:
//!
//! 1. **create** — GUI context and window creation (not realtime-critical)
//! 2. **prepare** — engine prepared with sample rate, block size, channels
//! 3. **run** — render loop + command processing (non-blocking)
//! 4. **destroy** — clean up GUI resources, signal engine to destroy
//!
//! ## Thread Model
//!
//! - **GUI thread** (main): owns egui context, processes user input, renders UI
//! - **Audio thread**: owns engine, processes audio blocks (not directly visible to GUI)
//! - **Command queue**: GUI → engine (parameter commands, preset loads)
//! - **Query interface**: engine → GUI (current parameter values, analysis data)
//!
//! The command queue and query interface are the only cross-thread contracts.

// ---------------------------------------------------------------------------
// Command: GUI → engine direction
// ---------------------------------------------------------------------------

/// Unique parameter identifier (re-exports romueq_common for GUI convenience).
pub use romueq_common::parameter::ParamId;

/// A command from the GUI to the engine.
///
/// Each command targets exactly one parameter and carries the new value.
/// Commands are batched and processed by the engine on the next prepare/process cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct GuiCommand {
    /// The parameter this command affects.
    pub param_id: ParamId,
    /// The new value to set (clamped by the GUI before sending).
    pub value: f32,
}

/// Batch of commands from the GUI, to be applied atomically.
#[derive(Debug, Clone, Default)]
pub struct GuiCommandBatch {
    commands: Vec<GuiCommand>,
}

impl GuiCommandBatch {
    /// Create a new empty batch.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Add a command to this batch.
    pub fn push(&mut self, param_id: ParamId, value: f32) {
        // De-duplicate: if the same param_id already exists, replace it.
        for cmd in &mut self.commands {
            if cmd.param_id == param_id {
                cmd.value = value;
                return;
            }
        }
        self.commands.push(GuiCommand { param_id, value });
    }

    /// Returns the commands in this batch.
    pub fn commands(&self) -> &[GuiCommand] {
        &self.commands
    }

    /// Returns true if this batch is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Consume this batch and return its commands.
    pub fn into_commands(self) -> Vec<GuiCommand> {
        self.commands
    }
}

// ---------------------------------------------------------------------------
// Query: engine → GUI direction
// ---------------------------------------------------------------------------

/// Current parameter value as reported by the engine.
#[derive(Debug, Clone)]
pub struct ParamValue {
    /// The parameter ID.
    pub param_id: ParamId,
    /// Current value set by the engine (after any automation/automation effects).
    pub value: f32,
}

/// Snapshot of all parameter values, as reported by the engine.
///
/// The GUI uses this to keep its widgets in sync with the actual DSP state.
#[derive(Debug, Clone)]
pub struct ParamSnapshot {
    values: Vec<ParamValue>,
}

impl ParamSnapshot {
    /// Create a new snapshot from the engine's parameter values.
    pub fn new(values: Vec<ParamValue>) -> Self {
        Self { values }
    }

    /// Returns all parameter values in the snapshot.
    pub fn values(&self) -> &[ParamValue] {
        &self.values
    }

    /// Get the value for a specific parameter, if present.
    pub fn get(&self, param_id: ParamId) -> Option<f32> {
        self.values
            .iter()
            .find(|v| v.param_id == param_id)
            .map(|v| v.value)
    }

    /// Returns true if this snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the number of parameters in this snapshot.
    pub fn len(&self) -> usize {
        self.values.len()
    }
}

/// Current analysis data as reported by the engine's analysis pipeline.
#[derive(Debug, Clone)]
pub struct AnalysisSnapshot {
    /// RMS values per channel (dBFS, or -inf for silence).
    pub rms: [f32; romueq_common::ANALYSIS_MAX_CHANNELS],
    /// Peak values per channel (dBFS, or -inf for silence).
    pub peak: [f32; romueq_common::ANALYSIS_MAX_CHANNELS],
    /// Whether the last frame had dropped data.
    pub dropped: bool,
}

impl Default for AnalysisSnapshot {
    fn default() -> Self {
        Self {
            rms: [f32::NEG_INFINITY; romueq_common::ANALYSIS_MAX_CHANNELS],
            peak: [f32::NEG_INFINITY; romueq_common::ANALYSIS_MAX_CHANNELS],
            dropped: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Engine interface contract (implemented by romueq-engine)
// ---------------------------------------------------------------------------

/// The interface the GUI uses to interact with the engine.
///
/// This trait is implemented by romueq-engine and provides a clean boundary:
/// the GUI knows nothing about DSP internals, only commands and queries.
///
/// ## Thread Safety
///
/// This trait is `!Sync` by default — the GUI owns a mutable reference to
/// the engine during the run phase. For multi-threaded scenarios, the engine
/// can be wrapped in a mutex or channel-based message queue.
pub trait GuiClient {
    /// Prepare the engine for audio processing.
    ///
    /// Called once after GUI creation and whenever sample rate, block size,
    /// or channel count changes.
    fn prepare(&mut self, sample_rate: f32, block_size: usize, channels: usize);

    /// Process a single audio block.
    ///
    /// The engine applies any pending GUI commands before processing the block.
    fn process(&mut self, input: &[f32], output: &mut [f32]);

    /// Get the current parameter snapshot from the engine.
    ///
    /// The GUI calls this once per frame to keep widgets in sync.
    fn snapshot(&self) -> ParamSnapshot;

    /// Get the current analysis snapshot from the engine's analysis pipeline.
    fn analysis_snapshot(&self) -> AnalysisSnapshot;

    /// Apply a batch of commands from the GUI.
    ///
    /// Commands are applied immediately (not deferred) to keep the GUI responsive.
    fn apply_commands(&mut self, batch: GuiCommandBatch);

    /// Destroy the engine and release all resources.
    fn destroy(&mut self);

    /// Returns true if the engine is currently prepared (after prepare(), before destroy()).
    fn is_prepared(&self) -> bool;
}

// ---------------------------------------------------------------------------
// GUI Application trait (implemented by concrete egui app)
// ---------------------------------------------------------------------------

/// The GUI application interface.
///
/// Implement this trait to create a concrete egui application.
/// The framework handles window creation, rendering, and event processing.
pub trait GuiApp: Send {
    /// Create a new GUI app instance.
    fn new() -> Self;

    /// Called every frame to update and render the UI.
    ///
    /// The implementation should:
    /// 1. Process any new commands from user input
    /// 2. Query the engine for current state
    /// 3. Render the UI using egui widgets
    fn update(&mut self, client: &mut dyn GuiClient);

    /// Returns the window title for this GUI application.
    fn window_title(&self) -> &str {
        "RomuEq"
    }

    /// Returns the window size hint (width, height) in physical pixels.
    fn window_size(&self) -> (f32, f32) {
        (800.0, 600.0)
    }
}

// ---------------------------------------------------------------------------
// Standalone entry point (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "standalone")]
use eframe::App;

/// Standalone entry point for RomuEq.
///
/// Creates an egui window backed by winit and runs the GUI application loop.
#[cfg(feature = "standalone")]
pub mod standalone {
    use super::*;

    /// Run the GUI application as a standalone desktop app.
    pub fn run(app: impl GuiApp) -> eframe::Result {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_resizable(true),
            ..Default::default()
        };
        let window_title = app.window_title().to_string();
        eframe::run_native(
            &window_title,
            options,
            Box::new(move |cc| {
                let ppp = cc.egui_ctx.pixels_per_point();
                cc.egui_ctx.set_pixels_per_point(ppp);
                Ok(Box::new(StandaloneRunner::new(app)))
            }),
        )
    }

    /// A runner that bridges the GuiApp trait with eframe::App.
    struct StandaloneRunner<A: GuiApp> {
        app: A,
        client: EngineClient,
    }

    impl<A: GuiApp> StandaloneRunner<A> {
        fn new(app: A) -> Self {
            Self {
                app,
                client: EngineClient::new(),
            }
        }
    }

    impl<A: GuiApp> App for StandaloneRunner<A> {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            // Update DPI scaling.
            let _pixels_per_point = ctx.pixels_per_point();

            // Query engine state.
            let snapshot = self.client.snapshot();
            let analysis = self.client.analysis_snapshot();

            // Process any pending commands.
            let batch = self.client.take_pending_commands();
            if !batch.is_empty() {
                self.client.apply_commands(batch);
            }

            // Update GUI app with current state.
            self.app.update(&mut self.client);

            // Render the UI.
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("RomuEq — EQ Parameter Editor");

                // Band controls (placeholder — M6-T002 will implement real widgets).
                ui.separator();

                // Show current parameter values.
                for pv in snapshot.values() {
                    ui.label(format!("{}: {:.2}", pv.param_id.display_name(), pv.value));
                }

                // Show analysis data.
                ui.separator();
                ui.label("Analysis:");
                if analysis.dropped {
                    ui.colored_label(egui::Color32::RED, "Dropped frames detected");
                }
            });

            // Repaint continuously for real-time response.
            ctx.request_repaint();
        }

        fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
            // Light theme background.
            [0.95, 0.95, 0.95, 1.0]
        }
    }

    /// A minimal engine client that satisfies GuiClient for standalone mode.
    ///
    /// In production, this would be wrapped around the actual romueq-engine.
    struct EngineClient {
        snapshot: ParamSnapshot,
        analysis: AnalysisSnapshot,
        pending_commands: GuiCommandBatch,
    }

    impl EngineClient {
        fn new() -> Self {
            Self {
                snapshot: ParamSnapshot::new(Vec::new()),
                analysis: AnalysisSnapshot::default(),
                pending_commands: GuiCommandBatch::new(),
            }
        }

        fn take_pending_commands(&mut self) -> GuiCommandBatch {
            std::mem::take(&mut self.pending_commands)
        }
    }

    impl GuiClient for EngineClient {
        fn prepare(&mut self, _sample_rate: f32, _block_size: usize, _channels: usize) {}
        fn process(&mut self, _input: &[f32], _output: &mut [f32]) {}
        fn snapshot(&self) -> ParamSnapshot {
            self.snapshot.clone()
        }
        fn analysis_snapshot(&self) -> AnalysisSnapshot {
            self.analysis.clone()
        }
        fn apply_commands(&mut self, batch: GuiCommandBatch) {
            // Store commands for later processing.
            for cmd in batch.into_commands() {
                self.pending_commands.push(cmd.param_id, cmd.value);
            }
        }
        fn destroy(&mut self) {}
        fn is_prepared(&self) -> bool {
            false // Placeholder
        }
    }
}
