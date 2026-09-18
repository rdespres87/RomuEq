#![forbid(unsafe_code)]

//! Real-time engine for RomuEq.
//!
//! This crate provides the runtime engine that coordinates parameter storage,
//! automation ingestion, lifecycle management, meters, bypass, and DSP
//! processing.

pub mod automation;
pub mod lifecycle;
pub mod parameter_store;
pub mod plugin;

// Re-export for plugin crate
pub use lifecycle::EngineLifecycle;
