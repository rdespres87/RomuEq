#![forbid(unsafe_code)]

//! Parameter storage for the engine.
//!
//! This module provides a simple parameter store that tracks the current
//! value of each parameter and supports sample-offset-based updates.

use equaform_common::ParamId;
use std::collections::HashMap;

/// Storage for parameter values keyed by `ParamId`.
///
/// Each parameter has a current value and an optional list of
/// (sample_offset, value) automation points.
#[derive(Debug)]
pub struct ParameterStore {
    /// Current values for each parameter.
    values: HashMap<ParamId, f32>,
    /// Automation points per parameter: param_id -> [(sample_offset, value)].
    automation_points: HashMap<ParamId, Vec<(u32, f32)>>,
}

impl ParameterStore {
    /// Create a new empty parameter store.
    pub fn new() -> Self {
        ParameterStore {
            values: HashMap::new(),
            automation_points: HashMap::new(),
        }
    }

    /// Set the current value of a parameter.
    pub fn set_value(&mut self, param_id: ParamId, value: f32, sample_offset: u32) {
        self.values.insert(param_id, value);
        self.automation_points
            .entry(param_id)
            .or_default()
            .push((sample_offset, value));
    }

    /// Get the current value of a parameter.
    pub fn get_value(&self, param_id: ParamId) -> Option<f32> {
        self.values.get(&param_id).copied()
    }

    /// Get all automation points for a parameter.
    pub fn get_automation_points(&self, param_id: ParamId) -> &[(u32, f32)] {
        self.automation_points
            .get(&param_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Reset all values to their defaults.
    pub fn reset(&mut self) {
        self.values.clear();
        self.automation_points.clear();
    }

    /// Returns the number of parameters currently stored.
    pub fn param_count(&self) -> usize {
        self.values.len()
    }
}

impl Default for ParameterStore {
    fn default() -> Self {
        Self::new()
    }
}
