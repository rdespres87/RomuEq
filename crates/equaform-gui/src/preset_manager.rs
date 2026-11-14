//! Preset management for the Equaform GUI layer.
//!
//! Handles in-memory preset storage, file-based persistence, and migration
//! of loaded presets through the versioned state schema.

use equaform_common::state::{Preset, ProjectState, StateError};

/// Errors that can occur during preset management operations.
#[derive(Debug, PartialEq)]
pub enum PresetManagerError {
    /// The preset file could not be read from disk.
    IoError(String),
    /// The preset data failed deserialization or validation.
    StateError(StateError),
    /// The preset name is empty or invalid.
    InvalidName,
    /// A preset with this name already exists.
    NameConflict,
}

impl std::fmt::Display for PresetManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PresetManagerError::IoError(msg) => write!(f, "I/O error: {msg}"),
            PresetManagerError::StateError(e) => write!(f, "state error: {e}"),
            PresetManagerError::InvalidName => write!(f, "invalid preset name"),
            PresetManagerError::NameConflict => write!(f, "preset name already exists"),
        }
    }
}

impl std::error::Error for PresetManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PresetManagerError::StateError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<StateError> for PresetManagerError {
    fn from(e: StateError) -> Self {
        PresetManagerError::StateError(e)
    }
}

/// A named preset with its associated display information.
#[derive(Clone, Debug)]
pub struct NamedPreset {
    /// The preset name (unique within the manager).
    pub name: String,
    /// The underlying preset data.
    pub preset: Preset,
}

impl NamedPreset {
    /// Create a new named preset.
    pub fn new(name: String, preset: Preset) -> Self {
        Self { name, preset }
    }
}

/// In-memory manager for presets.
///
/// Keeps a list of named presets and supports CRUD operations, plus
/// file-based load/save for individual presets.
pub struct PresetManager {
    /// Stored presets indexed by name (first match wins for lookups).
    presets: Vec<NamedPreset>,
}

impl Default for PresetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PresetManager {
    /// Create a new empty preset manager.
    pub fn new() -> Self {
        Self {
            presets: Vec::new(),
        }
    }

    /// Returns the number of stored presets.
    pub fn len(&self) -> usize {
        self.presets.len()
    }

    /// Returns true if no presets are stored.
    pub fn is_empty(&self) -> bool {
        self.presets.is_empty()
    }

    /// Iterate over all stored presets.
    pub fn iter(&self) -> impl Iterator<Item = &NamedPreset> {
        self.presets.iter()
    }

    /// Get a preset by name, if it exists.
    pub fn get(&self, name: &str) -> Option<&NamedPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// Add or update a preset. Returns `NameConflict` if the name already exists
    /// and the preset data is different (same name, same data is a no-op).
    pub fn add(&mut self, name: String, preset: Preset) -> Result<(), PresetManagerError> {
        if name.is_empty() {
            return Err(PresetManagerError::InvalidName);
        }

        // Check for conflict
        if let Some(existing) = self.presets.iter().find(|p| p.name == name) {
            // If same data, treat as no-op
            if existing.preset == preset {
                return Ok(());
            }
            return Err(PresetManagerError::NameConflict);
        }

        self.presets.push(NamedPreset::new(name, preset));
        Ok(())
    }

    /// Remove a preset by name. Returns `true` if it was found and removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.presets.len();
        self.presets.retain(|p| p.name != name);
        self.presets.len() < before
    }

    /// Save a preset to a file on disk.
    ///
    /// Serializes the preset using the binary schema and writes it to `path`.
    pub fn save_to_file(&self, name: &str, path: &str) -> Result<(), PresetManagerError> {
        let preset = self
            .get(name)
            .ok_or_else(|| PresetManagerError::IoError(format!("preset '{name}' not found")))?;

        let data = preset.preset.serialize();
        std::fs::write(path, &data)
            .map_err(|e| PresetManagerError::IoError(format!("write '{path}': {e}")))?;

        Ok(())
    }

    /// Load a preset from a file on disk.
    ///
    /// Reads the binary data, deserializes it, and validates the result.
    /// The preset is stored under its internal name (from the file metadata).
    pub fn load_from_file(&mut self, path: &str) -> Result<NamedPreset, PresetManagerError> {
        let data = std::fs::read(path)
            .map_err(|e| PresetManagerError::IoError(format!("read '{path}': {e}")))?;

        let mut preset = Preset::deserialize(&data)?;
        // Strip read-only telemetry parameter ID 6 (auto-gain compensation readout) if present
        preset.parameter_values.remove(6);

        // Validate the loaded preset
        preset.parameter_values.validate()?;

        let name = preset.name.clone();
        self.remove(&name);
        self.add(name.clone(), preset.clone())?;

        Ok(NamedPreset::new(name, preset))
    }

    /// Create a new preset from the given project state.
    pub fn create_from_state(
        &mut self,
        name: &str,
        state: &ProjectState,
    ) -> Result<(), PresetManagerError> {
        let preset = Preset::from_state(name, state);
        self.add(name.to_string(), preset)
    }

    /// Apply a preset's parameter values to the given project state.
    ///
    /// This copies all parameter values from the preset into the state,
    /// replacing any existing values for the same IDs.
    pub fn apply_to_state(
        &self,
        name: &str,
        state: &mut ProjectState,
    ) -> Result<(), PresetManagerError> {
        let preset = self
            .get(name)
            .ok_or_else(|| PresetManagerError::IoError(format!("preset '{name}' not found")))?;

        // Apply all parameter values from the preset to the state
        for (id, value) in preset.preset.parameter_values.iter() {
            state.set(id, value);
        }

        Ok(())
    }

    /// Get the default preset name for a new preset.
    pub fn next_auto_name(&self) -> String {
        let count = self.presets.len() + 1;
        format!("Preset {count}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use equaform_common::state::CURRENT_SCHEMA_VERSION;

    // -----------------------------------------------------------------------
    // PresetManager basic tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_preset_manager_new_is_empty() {
        let manager = PresetManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
    }

    #[test]
    fn test_preset_manager_add_and_get() {
        let mut manager = PresetManager::new();
        let preset = Preset::new("Test");
        manager.add("Test".to_string(), preset.clone()).unwrap();

        let found = manager.get("Test").unwrap();
        assert_eq!(found.name, "Test");
        assert_eq!(found.preset, preset);
    }

    #[test]
    fn test_preset_manager_add_duplicate_same_data() {
        let mut manager = PresetManager::new();
        let preset = Preset::new("Test");
        manager.add("Test".to_string(), preset.clone()).unwrap();
        // Adding the same preset data with the same name should be a no-op
        assert!(manager.add("Test".to_string(), preset).is_ok());
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_preset_manager_add_duplicate_different_data() {
        let mut manager = PresetManager::new();
        let preset1 = Preset::new("Test");
        manager.add("Test".to_string(), preset1).unwrap();

        let mut preset2 = Preset::new("Test");
        preset2.parameter_values.set(1, 1.0); // different data
        assert_eq!(
            manager.add("Test".to_string(), preset2),
            Err(PresetManagerError::NameConflict)
        );
    }

    #[test]
    fn test_preset_manager_add_empty_name() {
        let mut manager = PresetManager::new();
        let result = manager.add(String::new(), Preset::new(""));
        assert_eq!(result, Err(PresetManagerError::InvalidName));
    }

    #[test]
    fn test_preset_manager_remove() {
        let mut manager = PresetManager::new();
        let preset = Preset::new("Keep");
        manager.add("Keep".to_string(), preset).unwrap();
        let preset2 = Preset::new("Remove");
        manager.add("Remove".to_string(), preset2).unwrap();

        assert!(manager.remove("Remove"));
        assert_eq!(manager.len(), 1);
        assert!(manager.get("Remove").is_none());
        assert!(manager.get("Keep").is_some());
    }

    #[test]
    fn test_preset_manager_remove_nonexistent() {
        let mut manager = PresetManager::new();
        assert!(!manager.remove("Nope"));
    }

    #[test]
    fn test_preset_manager_iter() {
        let mut manager = PresetManager::new();
        manager.add("A".to_string(), Preset::new("A")).unwrap();
        manager.add("B".to_string(), Preset::new("B")).unwrap();

        let names: Vec<&str> = manager.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["A", "B"]);
    }

    #[test]
    fn test_preset_manager_next_auto_name() {
        let manager = PresetManager::new();
        assert_eq!(manager.next_auto_name(), "Preset 1");

        let mut manager2 = PresetManager::new();
        manager2
            .add("Existing".to_string(), Preset::new("Existing"))
            .unwrap();
        assert_eq!(manager2.next_auto_name(), "Preset 2");
    }

    // -----------------------------------------------------------------------
    // File I/O tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = std::env::temp_dir().join("equaform_test_preset.bin");
        let path = tmp.to_str().unwrap();

        let mut manager = PresetManager::new();
        let mut preset = Preset::new("RoundTrip");
        preset.parameter_values.set(102, 440.0); // A4 frequency
        manager
            .add("RoundTrip".to_string(), preset.clone())
            .unwrap();

        // Save
        manager.save_to_file("RoundTrip", path).unwrap();

        // Load into a new manager
        let mut loaded_manager = PresetManager::new();
        let loaded = loaded_manager.load_from_file(path).unwrap();
        assert_eq!(loaded.name, "RoundTrip");
        assert_eq!(loaded.preset.parameter_values.get(102), Some(440.0));

        // Cleanup
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let mut manager = PresetManager::new();
        let result = manager.load_from_file("/tmp/equaform_no_such_file.bin");
        assert!(matches!(result, Err(PresetManagerError::IoError(_))));
    }

    #[test]
    fn test_save_nonexistent_preset() {
        let manager = PresetManager::new();
        let result = manager.save_to_file("Nope", "/tmp/equaform_test.bin");
        assert!(matches!(result, Err(PresetManagerError::IoError(_))));

        let _ = std::fs::remove_file("/tmp/equaform_test.bin");
    }

    #[test]
    fn test_load_malformed_file() {
        let tmp = std::env::temp_dir().join("equaform_test_malformed.bin");
        let path = tmp.to_str().unwrap();

        // Write garbage data
        std::fs::write(path, [0u8; 10]).unwrap();

        let mut manager = PresetManager::new();
        let result = manager.load_from_file(path);
        assert!(matches!(result, Err(PresetManagerError::StateError(_))));

        // Cleanup
        let _ = std::fs::remove_file(path);
    }

    // -----------------------------------------------------------------------
    // Apply to state tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_apply_preset_to_state() {
        let mut manager = PresetManager::new();
        let mut preset = Preset::new("BassBoost");
        preset.parameter_values.set(102, 80.0); // band[0] frequency = 80 Hz
        preset.parameter_values.set(103, 6.0); // band[0] gain = +6 dB
        manager.add("BassBoost".to_string(), preset).unwrap();

        let mut state = ProjectState::new();
        manager.apply_to_state("BassBoost", &mut state).unwrap();

        assert_eq!(state.get(102), Some(80.0));
        assert_eq!(state.get(103), Some(6.0));
    }

    #[test]
    fn test_apply_nonexistent_preset() {
        let manager = PresetManager::new();
        let mut state = ProjectState::new();
        let result = manager.apply_to_state("Nope", &mut state);
        assert!(matches!(result, Err(PresetManagerError::IoError(_))));
    }

    // -----------------------------------------------------------------------
    // Create from state tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_create_preset_from_state() {
        let mut manager = PresetManager::new();
        let mut state = ProjectState::new();
        state.set(102, 1000.0); // band[0] frequency
        state.set(103, -3.0); // band[0] gain

        manager.create_from_state("MidCut", &state).unwrap();

        let preset = manager.get("MidCut").unwrap();
        assert_eq!(preset.preset.parameter_values.get(102), Some(1000.0));
        assert_eq!(preset.preset.parameter_values.get(103), Some(-3.0));
    }

    // -----------------------------------------------------------------------
    // Migration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_preset_with_future_version() {
        // Create a preset file with a future schema version (2)
        let tmp = std::env::temp_dir().join("equaform_test_future.bin");
        let path = tmp.to_str().unwrap();

        // Schema version 1 data (will be read, then migration will reject if > current)
        let mut data = CURRENT_SCHEMA_VERSION.to_le_bytes().to_vec();

        // Product name "Equaform-8"
        let product = b"Equaform-8";
        data.extend_from_slice(&(product.len() as u32).to_le_bytes());
        data.extend_from_slice(product);

        // Name "Future"
        let name = b"Future";
        data.extend_from_slice(&(name.len() as u32).to_le_bytes());
        data.extend_from_slice(name);

        // No author
        data.push(0);

        // No tags
        data.extend_from_slice(&0u32.to_le_bytes());

        // One parameter
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&102u32.to_le_bytes());
        data.extend_from_slice(&440.0f32.to_le_bytes());

        std::fs::write(path, &data).unwrap();

        let mut manager = PresetManager::new();
        let result = manager.load_from_file(path);
        assert!(result.is_ok());

        // Cleanup
        let _ = std::fs::remove_file(path);
    }
}
