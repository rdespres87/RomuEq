#![forbid(unsafe_code)]

//! Real-time engine for Equaform.
//!
//! This crate provides the runtime engine that coordinates parameter storage,
//! automation ingestion, lifecycle management, meters, bypass, and DSP
//! processing.

pub mod automation;
pub mod lifecycle;
pub mod parameter_store;
pub mod plugin;
pub mod typestate;

// Re-export for plugin and host crates
pub use lifecycle::{
    EngineLifecycle, calculate_auto_gain_compensation_db, calculate_auto_gain_for_params,
};
pub use typestate::{Engine, EngineConfigError, EngineState, Prepared, Running, Unconfigured};
