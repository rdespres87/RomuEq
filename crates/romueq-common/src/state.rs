//! Versioned logical state and preset serialization for RomuEq.
//!
//! Implements the state contract defined in ADR-006 and PARAMETER_STATE_CONTRACT.
//!
//! # Schema
//!
//! Schema version 1 envelope:
//!
//! ```text
//! schema_version: u32
//! product:        u32 length + UTF-8 bytes
//! param_count:    u32
//! params:         param_count × (id: u32, value: f32)
//! ```
//!
//! State save/load is a main-thread operation; audio processing never performs
//! persistence I/O. DSP coefficients and filter history are **not** authoritative
//! persisted state.

use crate::parameter::{MAX_BANDS, ParameterCommand, band, global};
use std::collections::BTreeMap;

// ===========================================================================
// Error types
// ===========================================================================

/// Errors that can occur during state or preset operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    /// The schema version is newer than what this implementation supports.
    UnsupportedFutureVersion { version: u32 },
    /// The serialized data is malformed or truncated.
    MalformedInput,
    /// A parameter value is outside its valid range.
    InvalidParameterValue { id: u32 },
    /// The product name does not match the expected value.
    UnknownProduct,
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::UnsupportedFutureVersion { version } => {
                write!(f, "unsupported future state version: {version}")
            }
            StateError::MalformedInput => write!(f, "malformed or truncated input"),
            StateError::InvalidParameterValue { id } => {
                write!(f, "invalid parameter value for ID {id}")
            }
            StateError::UnknownProduct => write!(f, "unknown product name"),
        }
    }
}

impl std::error::Error for StateError {}

/// Result type for state operations.
pub type StateResult<T> = Result<T, StateError>;

// ===========================================================================
// Schema version
// ===========================================================================

/// The current (and only) persisted state schema version.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// The product name that identifies Equaform-8 state files.
pub const PRODUCT_NAME: &str = "Equaform-8";

// ===========================================================================
// Parameter values map
// ===========================================================================

/// A map of stable parameter IDs to their logical values.
///
/// Uses `BTreeMap` for deterministic serialization order (sorted by ID).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParameterValues {
    map: BTreeMap<u32, f32>,
}

impl ParameterValues {
    /// Create a new empty parameter values map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the value for a parameter ID, or `None` if not set.
    pub fn get(&self, id: u32) -> Option<f32> {
        self.map.get(&id).copied()
    }

    /// Set a parameter value.
    pub fn set(&mut self, id: u32, value: f32) {
        self.map.insert(id, value);
    }

    /// Remove a parameter value.
    pub fn remove(&mut self, id: u32) -> Option<f32> {
        self.map.remove(&id)
    }

    /// Returns `true` if the map contains the given ID.
    pub fn contains(&self, id: u32) -> bool {
        self.map.contains_key(&id)
    }

    /// Number of parameters in the map.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate over all (id, value) pairs in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, f32)> + '_ {
        self.map.iter().map(|(&id, &v)| (id, v))
    }

    /// Fill the map with all default values for the complete state
    /// (all globals + all 8 bands × 6 parameters).
    pub fn fill_defaults(&mut self) {
        // Global parameters
        self.set(global::BYPASS, 0.0);
        self.set(global::OUTPUT_GAIN_DB, 0.0);
        self.set(global::AUTO_GAIN, 0.0);
        self.set(global::PROCESSING_MODE, 0.0);
        self.set(global::STEREO_MODE, 0.0);

        // Band parameters (6 per band × 8 bands)
        for band_idx in 0..MAX_BANDS {
            let base: u32 = 100 + band_idx as u32 * 100;
            self.set(base + band::ENABLED, 1.0);
            self.set(base + band::FILTER_TYPE, 0.0);
            self.set(base + band::FREQUENCY_HZ, 1000.0);
            self.set(base + band::GAIN_DB, 0.0);
            self.set(base + band::Q, std::f32::consts::FRAC_1_SQRT_2);
            self.set(base + band::SLOPE, 1.0);
        }
    }

    /// Validate all values against their known ranges.
    /// Returns the first invalid parameter ID, or `Ok(())` if all valid.
    pub fn validate(&self) -> StateResult<()> {
        for (&id, &value) in &self.map {
            if let Err(_e) = crate::parameter::validate_value(id, value) {
                return Err(StateError::InvalidParameterValue { id });
            }
        }
        Ok(())
    }
}

// ===========================================================================
// Project state
// ===========================================================================

/// Complete persisted state for a RomuEq instance.
///
/// Contains the full parameter set (globals + all bands). DSP coefficients
/// and filter history are **not** persisted — they are recomputed from
/// parameter values on load.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectState {
    /// Schema version of this state.
    pub schema_version: u32,
    /// Logical parameter values.
    pub parameter_values: ParameterValues,
}

impl ProjectState {
    /// Create a new state with the current schema version and all defaults.
    pub fn new() -> Self {
        let mut state = Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            parameter_values: ParameterValues::new(),
        };
        state.parameter_values.fill_defaults();
        state
    }

    /// Get a parameter value by ID.
    pub fn get(&self, id: u32) -> Option<f32> {
        self.parameter_values.get(id)
    }

    /// Set a parameter value.
    pub fn set(&mut self, id: u32, value: f32) {
        self.parameter_values.set(id, value);
    }

    /// Apply a batch of parameter commands to this state.
    pub fn apply_commands(&mut self, commands: &[ParameterCommand]) {
        for cmd in commands {
            match cmd {
                ParameterCommand::SetParameter { id, value } => {
                    self.set(id.0, *value);
                }
                ParameterCommand::SetAll(_) => {
                    // SetAll is handled by the engine; skip for state persistence.
                }
            }
        }
    }

    /// Serialize this state to a binary buffer.
    ///
    /// Format:
    /// - `schema_version` (u32 LE)
    /// - `product` length (u32 LE) + UTF-8 bytes
    /// - `param_count` (u32 LE)
    /// - for each param: `id` (u32 LE) + `value` (f32 LE)
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Schema version
        buf.extend_from_slice(&self.schema_version.to_le_bytes());

        // Product name
        let product_bytes = PRODUCT_NAME.as_bytes();
        buf.extend_from_slice(&(product_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(product_bytes);

        // Parameter values (sorted by ID via BTreeMap)
        let param_count = self.parameter_values.len() as u32;
        buf.extend_from_slice(&param_count.to_le_bytes());

        for (id, value) in self.parameter_values.iter() {
            buf.extend_from_slice(&id.to_le_bytes());
            buf.extend_from_slice(&value.to_le_bytes());
        }

        buf
    }

    /// Deserialize a state from a binary buffer.
    ///
    /// Returns `StateError::UnsupportedFutureVersion` if the schema version
    /// is newer than `CURRENT_SCHEMA_VERSION`.
    /// Returns `StateError::MalformedInput` if the data is truncated.
    pub fn deserialize(data: &[u8]) -> StateResult<Self> {
        let mut offset = 0;

        // Read schema version
        let schema_version = u32::from_le_bytes(
            data[offset..offset + 4]
                .try_into()
                .map_err(|_| StateError::MalformedInput)?,
        );
        offset += 4;

        // Reject unsupported future versions
        if schema_version > CURRENT_SCHEMA_VERSION {
            return Err(StateError::UnsupportedFutureVersion {
                version: schema_version,
            });
        }

        // Read product name
        let product_len = u32::from_le_bytes(
            data[offset..offset + 4]
                .try_into()
                .map_err(|_| StateError::MalformedInput)?,
        ) as usize;
        offset += 4;

        if offset + product_len > data.len() {
            return Err(StateError::MalformedInput);
        }

        let product = std::str::from_utf8(&data[offset..offset + product_len])
            .map_err(|_| StateError::MalformedInput)?;
        offset += product_len;

        if product != PRODUCT_NAME && product != "RomuEq" {
            return Err(StateError::UnknownProduct);
        }

        // Read parameter count
        let param_count = u32::from_le_bytes(
            data[offset..offset + 4]
                .try_into()
                .map_err(|_| StateError::MalformedInput)?,
        ) as usize;
        offset += 4;

        // Read parameters
        let mut parameter_values = ParameterValues::new();
        for _ in 0..param_count {
            if offset + 8 > data.len() {
                return Err(StateError::MalformedInput);
            }

            let id = u32::from_le_bytes(
                data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| StateError::MalformedInput)?,
            );
            offset += 4;

            let value = f32::from_le_bytes(
                data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| StateError::MalformedInput)?,
            );
            offset += 4;

            parameter_values.set(id, value);
        }

        Ok(Self {
            schema_version,
            parameter_values,
        })
    }

    /// Check if this state is compatible with the current schema.
    pub fn is_compatible(&self) -> bool {
        self.schema_version <= CURRENT_SCHEMA_VERSION
    }
}

// ===========================================================================
// Migration
// ===========================================================================

/// Migration function type: transforms state from version `from` to version `to`.
pub type MigrationFn = fn(ProjectState) -> StateResult<ProjectState>;

/// Registry of available migrations.
///
/// For MVP (version 1), there are no migrations. This registry is structured
/// for future version bumps (e.g., 1 → 2, 2 → 3).
pub struct MigrationRegistry {
    migrations: Vec<(u32, u32, MigrationFn)>,
}

impl MigrationRegistry {
    /// Create a new migration registry with all registered migrations.
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    /// Register a migration from version `from` to version `to`.
    pub fn add_migration(&mut self, from: u32, to: u32, func: MigrationFn) {
        self.migrations.push((from, to, func));
    }

    /// Migrate a state from its current version to the current schema version.
    ///
    /// If the state is already at the current version, returns it unchanged.
    /// If the state is from a future version, returns
    /// `StateError::UnsupportedFutureVersion`.
    ///
    /// A failed migration leaves the original state unchanged.
    pub fn migrate(&self, mut state: ProjectState) -> StateResult<ProjectState> {
        let original_version = state.schema_version;

        if original_version > CURRENT_SCHEMA_VERSION {
            return Err(StateError::UnsupportedFutureVersion {
                version: original_version,
            });
        }

        if original_version == CURRENT_SCHEMA_VERSION {
            return Ok(state);
        }

        // Apply migrations sequentially: original → original+1 → ... → CURRENT
        let mut current = original_version;
        while current < CURRENT_SCHEMA_VERSION {
            let next = current + 1;
            let migration = self
                .migrations
                .iter()
                .find(|(from, to, _)| *from == current && *to == next)
                .ok_or(StateError::MalformedInput)?;

            let next_state = (migration.2)(state)?;
            state = next_state;
            current = next;
        }

        Ok(state)
    }
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Preset
// ===========================================================================

/// A user-facing named snapshot of the logical parameter schema.
///
/// Preset metadata (name, author, tags, version) is **not** an audio parameter
/// and must not become host automation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preset {
    /// Human-readable preset name.
    pub name: String,
    /// Optional author identifier.
    pub author: Option<String>,
    /// Optional tags for categorization.
    pub tags: Vec<String>,
    /// The parameter values that define this preset.
    pub parameter_values: ParameterValues,
}

impl Preset {
    /// Create a new preset with the given name and default parameter values.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            author: None,
            tags: Vec::new(),
            parameter_values: ParameterValues::new(),
        }
    }

    /// Create a preset from a project state, using the state's parameter values.
    pub fn from_state(name: impl Into<String>, state: &ProjectState) -> Self {
        Self {
            name: name.into(),
            author: None,
            tags: Vec::new(),
            parameter_values: state.parameter_values.clone(),
        }
    }

    /// Serialize this preset to a binary buffer.
    ///
    /// Format:
    /// - `schema_version` (u32 LE)
    /// - `product` length (u32 LE) + UTF-8 bytes
    /// - `name` length (u32 LE) + UTF-8 bytes
    /// - `author` present (u8) + if present: length (u32 LE) + UTF-8 bytes
    /// - `tag_count` (u32 LE)
    /// - for each tag: length (u32 LE) + UTF-8 bytes
    /// - `param_count` (u32 LE)
    /// - for each param: `id` (u32 LE) + `value` (f32 LE)
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Schema version
        buf.extend_from_slice(&CURRENT_SCHEMA_VERSION.to_le_bytes());

        // Product name
        let product_bytes = PRODUCT_NAME.as_bytes();
        buf.extend_from_slice(&(product_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(product_bytes);

        // Preset name
        let name_bytes = self.name.as_bytes();
        buf.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(name_bytes);

        // Author (optional)
        match &self.author {
            Some(author) => {
                buf.push(1);
                let author_bytes = author.as_bytes();
                buf.extend_from_slice(&(author_bytes.len() as u32).to_le_bytes());
                buf.extend_from_slice(author_bytes);
            }
            None => buf.push(0),
        }

        // Tags
        let tag_count = self.tags.len() as u32;
        buf.extend_from_slice(&tag_count.to_le_bytes());
        for tag in &self.tags {
            let tag_bytes = tag.as_bytes();
            buf.extend_from_slice(&(tag_bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(tag_bytes);
        }

        // Parameter values
        let param_count = self.parameter_values.len() as u32;
        buf.extend_from_slice(&param_count.to_le_bytes());
        for (id, value) in self.parameter_values.iter() {
            buf.extend_from_slice(&id.to_le_bytes());
            buf.extend_from_slice(&value.to_le_bytes());
        }

        buf
    }

    /// Deserialize a preset from a binary buffer.
    pub fn deserialize(data: &[u8]) -> StateResult<Self> {
        let mut offset = 0;

        // Schema version (must be supported)
        let schema_version = u32::from_le_bytes(
            data[offset..offset + 4]
                .try_into()
                .map_err(|_| StateError::MalformedInput)?,
        );
        offset += 4;

        if schema_version > CURRENT_SCHEMA_VERSION {
            return Err(StateError::UnsupportedFutureVersion {
                version: schema_version,
            });
        }

        // Product name
        let product_len = u32::from_le_bytes(
            data[offset..offset + 4]
                .try_into()
                .map_err(|_| StateError::MalformedInput)?,
        ) as usize;
        offset += 4;

        if offset + product_len > data.len() {
            return Err(StateError::MalformedInput);
        }

        let product = std::str::from_utf8(&data[offset..offset + product_len])
            .map_err(|_| StateError::MalformedInput)?;
        offset += product_len;

        if product != PRODUCT_NAME && product != "RomuEq" {
            return Err(StateError::UnknownProduct);
        }

        // Preset name
        let name_len = u32::from_le_bytes(
            data[offset..offset + 4]
                .try_into()
                .map_err(|_| StateError::MalformedInput)?,
        ) as usize;
        offset += 4;

        if offset + name_len > data.len() {
            return Err(StateError::MalformedInput);
        }

        let name = std::str::from_utf8(&data[offset..offset + name_len])
            .map_err(|_| StateError::MalformedInput)?;
        offset += name_len;

        // Author (optional)
        let has_author = data[offset];
        offset += 1;

        let author = if has_author != 0 {
            if offset + 4 > data.len() {
                return Err(StateError::MalformedInput);
            }
            let author_len = u32::from_le_bytes(
                data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| StateError::MalformedInput)?,
            ) as usize;
            offset += 4;

            if offset + author_len > data.len() {
                return Err(StateError::MalformedInput);
            }

            let author = std::str::from_utf8(&data[offset..offset + author_len])
                .map_err(|_| StateError::MalformedInput)?;
            offset += author_len;
            Some(author.to_string())
        } else {
            None
        };

        // Tags
        let tag_count = u32::from_le_bytes(
            data[offset..offset + 4]
                .try_into()
                .map_err(|_| StateError::MalformedInput)?,
        ) as usize;
        offset += 4;

        let mut tags = Vec::with_capacity(tag_count);
        for _ in 0..tag_count {
            if offset + 4 > data.len() {
                return Err(StateError::MalformedInput);
            }
            let tag_len = u32::from_le_bytes(
                data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| StateError::MalformedInput)?,
            ) as usize;
            offset += 4;

            if offset + tag_len > data.len() {
                return Err(StateError::MalformedInput);
            }

            let tag = std::str::from_utf8(&data[offset..offset + tag_len])
                .map_err(|_| StateError::MalformedInput)?;
            offset += tag_len;
            tags.push(tag.to_string());
        }

        // Parameter values
        let param_count = u32::from_le_bytes(
            data[offset..offset + 4]
                .try_into()
                .map_err(|_| StateError::MalformedInput)?,
        ) as usize;
        offset += 4;

        let mut parameter_values = ParameterValues::new();
        for _ in 0..param_count {
            if offset + 8 > data.len() {
                return Err(StateError::MalformedInput);
            }

            let id = u32::from_le_bytes(
                data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| StateError::MalformedInput)?,
            );
            offset += 4;

            let value = f32::from_le_bytes(
                data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| StateError::MalformedInput)?,
            );
            offset += 4;

            parameter_values.set(id, value);
        }

        Ok(Self {
            name: name.to_string(),
            author,
            tags,
            parameter_values,
        })
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParamId;

    // -----------------------------------------------------------------------
    // ParameterValues tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parameter_values_new_is_empty() {
        let pv = ParameterValues::new();
        assert!(pv.is_empty());
        assert_eq!(pv.len(), 0);
        assert!(pv.get(1).is_none());
    }

    #[test]
    fn test_parameter_values_set_and_get() {
        let mut pv = ParameterValues::new();
        pv.set(1, 0.5);
        assert_eq!(pv.get(1), Some(0.5));
        assert_eq!(pv.len(), 1);
    }

    #[test]
    fn test_parameter_values_remove() {
        let mut pv = ParameterValues::new();
        pv.set(1, 0.5);
        let removed = pv.remove(1);
        assert_eq!(removed, Some(0.5));
        assert!(pv.get(1).is_none());
        assert_eq!(pv.len(), 0);
    }

    #[test]
    fn test_parameter_values_fill_defaults() {
        let mut pv = ParameterValues::new();
        pv.fill_defaults();

        // Should have 5 globals + 8 bands × 6 params = 53 parameters
        assert_eq!(pv.len(), 53);

        // Check a few defaults
        assert_eq!(pv.get(global::BYPASS), Some(0.0));
        assert_eq!(pv.get(global::OUTPUT_GAIN_DB), Some(0.0));
        assert_eq!(pv.get(100), Some(1.0)); // band[0].enabled
        assert_eq!(pv.get(102), Some(1000.0)); // band[0].frequency_hz
    }

    #[test]
    fn test_parameter_values_iter_sorted() {
        let mut pv = ParameterValues::new();
        pv.set(5, 5.0);
        pv.set(1, 1.0);
        pv.set(3, 3.0);

        let ids: Vec<u32> = pv.iter().map(|(id, _)| id).collect();
        assert_eq!(ids, vec![1, 3, 5]);
    }

    #[test]
    fn test_parameter_values_validate_ok() {
        let mut pv = ParameterValues::new();
        pv.set(1, 0.5); // global.bypass: valid
        assert!(pv.validate().is_ok());
    }

    #[test]
    fn test_parameter_values_validate_invalid() {
        let mut pv = ParameterValues::new();
        pv.set(1, 2.0); // global.bypass max is 1.0
        assert_eq!(
            pv.validate(),
            Err(StateError::InvalidParameterValue { id: 1 })
        );
    }

    // -----------------------------------------------------------------------
    // ProjectState tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_project_state_new() {
        let state = ProjectState::new();
        assert_eq!(state.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(state.parameter_values.len(), 53);
    }

    #[test]
    fn test_project_state_serialize_deserialize_roundtrip() {
        let original = ProjectState::new();
        let serialized = original.serialize();
        let deserialized = ProjectState::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(deserialized.parameter_values.len(), 53);
        assert_eq!(deserialized, original);
    }

    #[test]
    fn test_project_state_serialize_deserialize_lossless() {
        let mut state = ProjectState::new();
        state.set(1, 1.0); // bypass on
        state.set(102, 440.0); // band[0].frequency_hz = A4

        let serialized = state.serialize();
        let deserialized = ProjectState::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.get(1), Some(1.0));
        assert_eq!(deserialized.get(102), Some(440.0));
        assert_eq!(deserialized, state);
    }

    #[test]
    fn test_project_state_apply_commands() {
        let mut state = ProjectState::new();
        let commands = vec![
            ParameterCommand::SetParameter {
                id: ParamId(1),
                value: 1.0,
            },
            ParameterCommand::SetParameter {
                id: ParamId(102),
                value: 1000.0,
            },
        ];
        state.apply_commands(&commands);

        assert_eq!(state.get(1), Some(1.0));
        assert_eq!(state.get(102), Some(1000.0));
    }

    #[test]
    fn test_project_state_unsupported_future_version() {
        let data = vec![
            2, 0, 0, 0, // schema_version = 2 (future)
            5, 0, 0, 0, // product length = 5
            b'R', b'o', b'm', b'u', b'E', // "RomuEq"
            0, 0, 0, 0, // param_count = 0
        ];
        let result = ProjectState::deserialize(&data);
        assert_eq!(
            result,
            Err(StateError::UnsupportedFutureVersion { version: 2 })
        );
    }

    #[test]
    fn test_project_state_malformed_input_truncated() {
        let data = vec![1, 0, 0, 0, 5, 0, 0, 0, b'R']; // truncated product name
        let result = ProjectState::deserialize(&data);
        assert_eq!(result, Err(StateError::MalformedInput));
    }

    #[test]
    fn test_project_state_unknown_product() {
        let data = vec![
            1, 0, 0, 0, // schema_version = 1
            5, 0, 0, 0, // product length = 5
            b'X', b'x', b'x', b'x', b'x', // unknown product
            0, 0, 0, 0, // param_count = 0
        ];
        let result = ProjectState::deserialize(&data);
        assert_eq!(result, Err(StateError::UnknownProduct));
    }

    #[test]
    fn test_project_state_is_compatible() {
        let state = ProjectState::new();
        assert!(state.is_compatible());
    }

    // -----------------------------------------------------------------------
    // Migration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_migration_noop_current_version() {
        let registry = MigrationRegistry::new();
        let state = ProjectState::new();
        let migrated = registry.migrate(state.clone()).unwrap();
        assert_eq!(migrated, state);
    }

    #[test]
    fn test_migration_rejects_future_version() {
        let registry = MigrationRegistry::new();
        let mut state = ProjectState::new();
        state.schema_version = 99;

        let result = registry.migrate(state);
        assert_eq!(
            result,
            Err(StateError::UnsupportedFutureVersion { version: 99 })
        );
    }

    // -----------------------------------------------------------------------
    // Preset tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_preset_new() {
        let preset = Preset::new("Test Preset");
        assert_eq!(preset.name, "Test Preset");
        assert!(preset.author.is_none());
        assert!(preset.tags.is_empty());
        assert!(preset.parameter_values.is_empty());
    }

    #[test]
    fn test_preset_from_state() {
        let state = ProjectState::new();
        let preset = Preset::from_state("From State", &state);
        assert_eq!(preset.name, "From State");
        assert_eq!(preset.parameter_values.len(), 53);
    }

    #[test]
    fn test_preset_serialize_deserialize_roundtrip() {
        let mut preset = Preset::new("My Preset");
        preset.author = Some("Author".to_string());
        preset.tags = vec!["bass".to_string(), "treble".to_string()];
        preset.parameter_values.set(1, 1.0);
        preset.parameter_values.set(102, 440.0);

        let serialized = preset.serialize();
        let deserialized = Preset::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.name, "My Preset");
        assert_eq!(deserialized.author, Some("Author".to_string()));
        assert_eq!(
            deserialized.tags,
            vec!["bass".to_string(), "treble".to_string()]
        );
        assert_eq!(deserialized.parameter_values.get(1), Some(1.0));
        assert_eq!(deserialized.parameter_values.get(102), Some(440.0));
    }

    #[test]
    fn test_preset_serialize_deserialize_no_author() {
        let preset = Preset::new("Simple");
        let serialized = preset.serialize();
        let deserialized = Preset::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.name, "Simple");
        assert_eq!(deserialized.author, None);
    }

    #[test]
    fn test_preset_unsupported_future_version() {
        let data = vec![
            99, 0, 0, 0, // schema_version = 99 (future)
            5, 0, 0, 0, // product length = 5
            b'R', b'o', b'm', b'u', b'E', // "RomuEq"
            0, 0, 0, 0, // name length = 0
            0, // no author
            0, 0, 0, 0, // tag_count = 0
            0, 0, 0, 0, // param_count = 0
        ];
        let result = Preset::deserialize(&data);
        assert_eq!(
            result,
            Err(StateError::UnsupportedFutureVersion { version: 99 })
        );
    }

    #[test]
    fn test_preset_malformed_input() {
        let data = vec![1, 0, 0, 0, 5, 0, 0, 0, b'R']; // truncated
        let result = Preset::deserialize(&data);
        assert_eq!(result, Err(StateError::MalformedInput));
    }

    #[test]
    fn test_preset_unknown_product() {
        let data = vec![
            1, 0, 0, 0, // schema_version = 1
            5, 0, 0, 0, // product length = 5
            b'X', b'x', b'x', b'x', b'x', // unknown product
            0, 0, 0, 0, // name length = 0
            0, // no author
            0, 0, 0, 0, // tag_count = 0
            0, 0, 0, 0, // param_count = 0
        ];
        let result = Preset::deserialize(&data);
        assert_eq!(result, Err(StateError::UnknownProduct));
    }

    // -----------------------------------------------------------------------
    // Golden migration fixtures (empty chain for v1)
    // -----------------------------------------------------------------------

    #[test]
    fn test_golden_state_v1_roundtrip() {
        // Create a known state and verify lossless round-trip
        let mut state = ProjectState::new();
        state.set(1, 1.0);
        state.set(102, 200.0);
        state.set(103, -6.0);
        state.set(202, 4000.0);
        state.set(203, 12.0);

        let serialized = state.serialize();
        let deserialized = ProjectState::deserialize(&serialized).unwrap();
        assert_eq!(deserialized, state);
    }

    #[test]
    fn test_golden_preset_roundtrip() {
        let mut preset = Preset::new("Golden Test");
        preset.author = Some("Test".to_string());
        preset.tags = vec!["test".to_string()];
        preset.parameter_values.set(1, 0.0);
        preset.parameter_values.set(100, 1.0);
        preset.parameter_values.set(101, 3.0); // HighPass
        preset.parameter_values.set(102, 80.0);
        preset.parameter_values.set(103, -12.0);
        preset.parameter_values.set(104, 0.707);
        preset.parameter_values.set(105, 3.0); // 24 dB/oct

        let serialized = preset.serialize();
        let deserialized = Preset::deserialize(&serialized).unwrap();
        assert_eq!(deserialized, preset);
    }
}
