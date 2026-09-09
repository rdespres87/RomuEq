#![forbid(unsafe_code)]

//! Plugin boundary re-exports for RomuEq.
//!
//! This crate re-exports the plugin traits and types from `romueq-common::plugin`
//! so that plugin authors can depend only on `romueq-plugin` without needing
//! to know about `romueq-common`.
//!
//! In the future, this crate may also host SDK-specific plugin implementations
//! (JUCE, VST3, AU) that depend on host SDKs.

pub use romueq_common::plugin::*;
