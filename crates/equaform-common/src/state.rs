//! Versioned logical state and preset serialization for Equaform.
//!
//! Implements the state contract defined in ADR-006 and PARAMETER_STATE_CONTRACT.
//!
//! # Schema
//!
//! Schema version 1 binary envelope:
//!
//! ```text
//! magic:          [u8; 4] = b"EQFR"
//! schema_version: u32 LE (= 1)
//! product:        u32 length + UTF-8 bytes
//! [preset meta]:  name, author, tags (Preset only)
//! param_count:    u32 LE
//! params:         param_count × (id: u32 LE, value: f32 LE)
//! crc32:          u32 LE (IEEE 802.3 CRC32 over all preceding bytes)
//! ```
//!
//! State save/load is a main-thread operation; audio processing never performs
//! persistence I/O. DSP coefficients and filter history are **not** authoritative
//! persisted state.

use crate::parameter::{MAX_BANDS, ParameterCommand, band, global};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

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
    /// The CRC32 integrity checksum did not match the payload bytes.
    ChecksumMismatch { expected: u32, actual: u32 },
    /// JSON serialization or deserialization failed.
    InvalidJson,
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
            StateError::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "CRC32 checksum mismatch (expected 0x{expected:08x}, got 0x{actual:08x})"
                )
            }
            StateError::InvalidJson => write!(f, "invalid JSON preset payload"),
        }
    }
}

impl std::error::Error for StateError {}

/// Result type for state operations.
pub type StateResult<T> = Result<T, StateError>;

// ===========================================================================
// Schema version, magic header & IEEE 802.3 CRC32
// ===========================================================================

/// Four-byte magic header identifying Equaform binary payloads.
pub const STATE_MAGIC: [u8; 4] = *b"EQFR";

/// The current persisted state schema version.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// The product name that identifies Equaform-8 state files.
pub const PRODUCT_NAME: &str = "Equaform-8";

/// Precomputed IEEE 802.3 CRC32 lookup table (reflected polynomial `0xEDB8_8320`).
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// Compute the standard IEEE 802.3 CRC32 checksum of `data`.
pub fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        let idx = ((crc ^ u32::from(byte)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
    }
    !crc
}

// ===========================================================================
// Atomic disk persistence helper
// ===========================================================================

static TMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Atomically write `data` to `path` via a sibling temporary file + `fsync` + `rename`.
///
/// Guarantees that a crash or power loss mid-write never leaves a truncated or
/// partially-written file at `path`. Cleans up the temporary file on error.
pub fn atomic_write(path: impl AsRef<Path>, data: &[u8]) -> std::io::Result<()> {
    let target = path.as_ref();
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("equaform_state");
    let pid = std::process::id();
    let seq = TMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!(".{file_name}.{pid}.{seq}.tmp");
    let tmp_path = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(tmp_name),
        _ => std::path::PathBuf::from(tmp_name),
    };

    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    if let Err(err) = std::fs::rename(&tmp_path, target) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    Ok(())
}

/// Upper bound for text files read through `read_to_string_bounded` (16 MiB).
pub const MAX_TEXT_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// Reads a UTF-8 text file into a `String`, enforcing a byte-level size cap.
///
/// The read is TOCTOU-safe: the file is opened once and bounded with
/// `std::io::Read::take`, rejecting any file exceeding `max_bytes`.
pub fn read_to_string_bounded(path: impl AsRef<Path>, max_bytes: u64) -> std::io::Result<String> {
    use std::io::Read;

    let file = std::fs::File::open(path)?;
    let mut limited = file.take(max_bytes.saturating_add(1));
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes)?;

    if bytes.len() as u64 > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("file exceeds maximum allowed size of {max_bytes} bytes"),
        ));
    }

    String::from_utf8(bytes)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

// ===========================================================================
// Bounds-checked binary cursor
// ===========================================================================

/// Bounds-checked little-endian binary reader that never panics on truncated
/// or adversarial inputs.
struct BinaryReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> BinaryReader<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    const fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }

    fn read_bytes(&mut self, len: usize) -> StateResult<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(StateError::MalformedInput)?;
        let slice = self
            .data
            .get(self.offset..end)
            .ok_or(StateError::MalformedInput)?;
        self.offset = end;
        Ok(slice)
    }

    fn read_u8(&mut self) -> StateResult<u8> {
        let slice = self.read_bytes(1)?;
        Ok(slice[0])
    }

    fn read_u32(&mut self) -> StateResult<u32> {
        let slice = self.read_bytes(4)?;
        let arr: [u8; 4] = slice.try_into().map_err(|_| StateError::MalformedInput)?;
        Ok(u32::from_le_bytes(arr))
    }

    fn read_f32(&mut self) -> StateResult<f32> {
        let slice = self.read_bytes(4)?;
        let arr: [u8; 4] = slice.try_into().map_err(|_| StateError::MalformedInput)?;
        Ok(f32::from_le_bytes(arr))
    }

    fn read_str(&mut self) -> StateResult<&'a str> {
        let len = self.read_u32()? as usize;
        let bytes = self.read_bytes(len)?;
        std::str::from_utf8(bytes).map_err(|_| StateError::MalformedInput)
    }
}

/// Unwrap a framed binary envelope (`"EQFR"` + version + payload + CRC32).
///
/// Returns `(schema_version, reader_positioned_after_version)`.
fn open_envelope(data: &[u8]) -> StateResult<(u32, BinaryReader<'_>)> {
    // Minimum framed size: 4 (magic) + 4 (version) + 4 (crc32)
    if data.len() < 12 || !data.starts_with(&STATE_MAGIC) {
        return Err(StateError::MalformedInput);
    }

    let (body, crc_slice) = data.split_at(data.len() - 4);
    let actual_crc = u32::from_le_bytes(
        crc_slice
            .try_into()
            .map_err(|_| StateError::MalformedInput)?,
    );
    let expected_crc = crc32_ieee(body);
    if expected_crc != actual_crc {
        return Err(StateError::ChecksumMismatch {
            expected: expected_crc,
            actual: actual_crc,
        });
    }

    let mut reader = BinaryReader::new(body);
    let _magic = reader.read_bytes(4)?;
    let schema_version = reader.read_u32()?;
    if schema_version == 0 {
        return Err(StateError::MalformedInput);
    }
    if schema_version > CURRENT_SCHEMA_VERSION {
        return Err(StateError::UnsupportedFutureVersion {
            version: schema_version,
        });
    }
    Ok((schema_version, reader))
}

fn read_product_checked(reader: &mut BinaryReader<'_>) -> StateResult<()> {
    let product = reader.read_str()?;
    if product != PRODUCT_NAME && product != "RomuEq" {
        return Err(StateError::UnknownProduct);
    }
    Ok(())
}

fn read_parameter_values(reader: &mut BinaryReader<'_>) -> StateResult<ParameterValues> {
    let param_count = reader.read_u32()? as usize;
    // Guard against OOM / DoS from malformed counts before iterating.
    let required_bytes = param_count
        .checked_mul(8)
        .ok_or(StateError::MalformedInput)?;
    if required_bytes > reader.remaining() {
        return Err(StateError::MalformedInput);
    }

    let mut parameter_values = ParameterValues::new();
    for _ in 0..param_count {
        let id = reader.read_u32()?;
        let value = reader.read_f32()?;
        if !value.is_finite() {
            return Err(StateError::InvalidParameterValue { id });
        }
        parameter_values.set(id, value);
    }
    Ok(parameter_values)
}

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

/// Complete persisted state for an Equaform instance.
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

    /// Serialize this state to a binary buffer with `"EQFR"` magic header
    /// and trailing IEEE 802.3 CRC32 checksum.
    ///
    /// Format:
    /// - `magic` (`b"EQFR"`, 4 bytes)
    /// - `schema_version` (u32 LE)
    /// - `product` length (u32 LE) + UTF-8 bytes
    /// - `param_count` (u32 LE)
    /// - for each param: `id` (u32 LE) + `value` (f32 LE)
    /// - `crc32` (u32 LE, over all preceding bytes)
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(32 + self.parameter_values.len() * 8);

        // Magic header + schema version
        buf.extend_from_slice(&STATE_MAGIC);
        let version = if self.schema_version == 0 {
            CURRENT_SCHEMA_VERSION
        } else {
            self.schema_version
        };
        buf.extend_from_slice(&version.to_le_bytes());

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

        // Trailing IEEE 802.3 CRC32
        let checksum = crc32_ieee(&buf);
        buf.extend_from_slice(&checksum.to_le_bytes());

        buf
    }

    /// Deserialize a state from a framed binary buffer (`"EQFR"` + CRC32).
    ///
    /// Returns `StateError::ChecksumMismatch` if CRC32 verification fails.
    /// Returns `StateError::UnsupportedFutureVersion` if the schema version
    /// is newer than `CURRENT_SCHEMA_VERSION`.
    /// Returns `StateError::MalformedInput` if the data is truncated or malformed.
    pub fn deserialize(data: &[u8]) -> StateResult<Self> {
        let (schema_version, mut reader) = open_envelope(data)?;
        read_product_checked(&mut reader)?;
        let parameter_values = read_parameter_values(&mut reader)?;

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

    /// Serialize this preset to a binary buffer with `"EQFR"` magic header
    /// and trailing IEEE 802.3 CRC32 checksum.
    ///
    /// Format:
    /// - `magic` (`b"EQFR"`, 4 bytes)
    /// - `schema_version` (u32 LE)
    /// - `product` length (u32 LE) + UTF-8 bytes
    /// - `name` length (u32 LE) + UTF-8 bytes
    /// - `author` present (u8) + if present: length (u32 LE) + UTF-8 bytes
    /// - `tag_count` (u32 LE)
    /// - for each tag: length (u32 LE) + UTF-8 bytes
    /// - `param_count` (u32 LE)
    /// - for each param: `id` (u32 LE) + `value` (f32 LE)
    /// - `crc32` (u32 LE, over all preceding bytes)
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64 + self.parameter_values.len() * 8);

        // Magic header + schema version
        buf.extend_from_slice(&STATE_MAGIC);
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

        // Trailing IEEE 802.3 CRC32
        let checksum = crc32_ieee(&buf);
        buf.extend_from_slice(&checksum.to_le_bytes());

        buf
    }

    /// Deserialize a preset from a framed binary buffer (`"EQFR"` + CRC32).
    pub fn deserialize(data: &[u8]) -> StateResult<Self> {
        let (_schema_version, mut reader) = open_envelope(data)?;
        read_product_checked(&mut reader)?;

        // Preset name
        let name = reader.read_str()?.to_string();

        // Author (optional)
        let has_author = reader.read_u8()?;
        let author = if has_author != 0 {
            Some(reader.read_str()?.to_string())
        } else {
            None
        };

        // Tags (guard against OOM / DoS before allocating)
        let tag_count = reader.read_u32()? as usize;
        let min_tag_bytes = tag_count.checked_mul(4).ok_or(StateError::MalformedInput)?;
        if min_tag_bytes > reader.remaining() {
            return Err(StateError::MalformedInput);
        }

        let mut tags = Vec::with_capacity(tag_count);
        for _ in 0..tag_count {
            tags.push(reader.read_str()?.to_string());
        }

        // Parameter values
        let parameter_values = read_parameter_values(&mut reader)?;

        Ok(Self {
            name,
            author,
            tags,
            parameter_values,
        })
    }

    /// Export this preset as a pretty-printed, versioned JSON string.
    pub fn to_json(&self) -> StateResult<String> {
        let parameters: BTreeMap<u32, f32> = self.parameter_values.iter().collect();
        let envelope = PresetJsonEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            product: Cow::Borrowed(PRODUCT_NAME),
            name: Cow::Borrowed(&self.name),
            author: self.author.as_deref().map(Cow::Borrowed),
            tags: self
                .tags
                .iter()
                .map(|t| Cow::Borrowed(t.as_str()))
                .collect(),
            parameters,
        };
        serde_json::to_string_pretty(&envelope).map_err(|_| StateError::InvalidJson)
    }

    /// Import a preset from a JSON string previously created by [`Preset::to_json`].
    pub fn from_json(json: &str) -> StateResult<Self> {
        let envelope: PresetJsonEnvelope<'_> =
            serde_json::from_str(json).map_err(|_| StateError::InvalidJson)?;

        if envelope.schema_version == 0 {
            return Err(StateError::MalformedInput);
        }
        if envelope.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(StateError::UnsupportedFutureVersion {
                version: envelope.schema_version,
            });
        }
        if envelope.product != PRODUCT_NAME && envelope.product != "RomuEq" {
            return Err(StateError::UnknownProduct);
        }

        let mut parameter_values = ParameterValues::new();
        for (id, value) in envelope.parameters {
            if !value.is_finite() {
                return Err(StateError::InvalidParameterValue { id });
            }
            parameter_values.set(id, value);
        }

        Ok(Self {
            name: envelope.name.into_owned(),
            author: envelope.author.map(Cow::into_owned),
            tags: envelope.tags.into_iter().map(Cow::into_owned).collect(),
            parameter_values,
        })
    }
}

/// Versioned JSON serialization envelope for human-readable preset export/import.
#[derive(Serialize, Deserialize)]
struct PresetJsonEnvelope<'a> {
    schema_version: u32,
    product: Cow<'a, str>,
    name: Cow<'a, str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    author: Option<Cow<'a, str>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<Cow<'a, str>>,
    parameters: BTreeMap<u32, f32>,
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

    fn wrap_framed(body_after_magic: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + body_after_magic.len() + 4);
        buf.extend_from_slice(&STATE_MAGIC);
        buf.extend_from_slice(body_after_magic);
        let crc = crc32_ieee(&buf);
        buf.extend_from_slice(&crc.to_le_bytes());
        buf
    }

    #[test]
    fn test_project_state_unsupported_future_version() {
        let future_version = CURRENT_SCHEMA_VERSION + 1;
        let mut body = Vec::new();
        body.extend_from_slice(&future_version.to_le_bytes());
        body.extend_from_slice(&6u32.to_le_bytes());
        body.extend_from_slice(b"RomuEq");
        body.extend_from_slice(&0u32.to_le_bytes());

        let result = ProjectState::deserialize(&wrap_framed(&body));
        assert_eq!(
            result,
            Err(StateError::UnsupportedFutureVersion {
                version: future_version
            })
        );
    }

    #[test]
    fn test_project_state_malformed_input_truncated() {
        let data = vec![1, 0, 0, 0, 5, 0, 0, 0, b'R']; // missing magic / truncated
        let result = ProjectState::deserialize(&data);
        assert_eq!(result, Err(StateError::MalformedInput));
    }

    #[test]
    fn test_project_state_unknown_product() {
        let body = vec![
            1, 0, 0, 0, // schema_version = 1
            5, 0, 0, 0, // product length = 5
            b'X', b'x', b'x', b'x', b'x', // unknown product
            0, 0, 0, 0, // param_count = 0
        ];
        let result = ProjectState::deserialize(&wrap_framed(&body));
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
        let body = vec![
            99, 0, 0, 0, // schema_version = 99 (future)
            5, 0, 0, 0, // product length = 5
            b'R', b'o', b'm', b'u', b'E', // "RomuEq"
            0, 0, 0, 0, // name length = 0
            0, // no author
            0, 0, 0, 0, // tag_count = 0
            0, 0, 0, 0, // param_count = 0
        ];
        let result = Preset::deserialize(&wrap_framed(&body));
        assert_eq!(
            result,
            Err(StateError::UnsupportedFutureVersion { version: 99 })
        );
    }

    #[test]
    fn test_preset_malformed_input() {
        let data = vec![1, 0, 0, 0, 5, 0, 0, 0, b'R']; // missing magic / truncated
        let result = Preset::deserialize(&data);
        assert_eq!(result, Err(StateError::MalformedInput));
    }

    #[test]
    fn test_preset_unknown_product() {
        let body = vec![
            1, 0, 0, 0, // schema_version = 1
            5, 0, 0, 0, // product length = 5
            b'X', b'x', b'x', b'x', b'x', // unknown product
            0, 0, 0, 0, // name length = 0
            0, // no author
            0, 0, 0, 0, // tag_count = 0
            0, 0, 0, 0, // param_count = 0
        ];
        let result = Preset::deserialize(&wrap_framed(&body));
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

    // -----------------------------------------------------------------------
    // Integrity & persistence tests: CRC32, JSON, atomic_write
    // -----------------------------------------------------------------------

    #[test]
    fn test_crc32_ieee_standard_vector() {
        assert_eq!(crc32_ieee(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32_ieee(b""), 0x0000_0000);
    }

    #[test]
    fn test_project_state_magic_and_crc32_corruption_detection() {
        let state = ProjectState::new();
        let serialized = state.serialize();

        // Starts with "EQFR" magic header
        assert_eq!(&serialized[..4], &STATE_MAGIC);

        // Flip a bit in the payload -> ChecksumMismatch
        let mut corrupted_payload = serialized.clone();
        let mid = serialized.len() / 2;
        corrupted_payload[mid] ^= 0x01;
        assert!(matches!(
            ProjectState::deserialize(&corrupted_payload),
            Err(StateError::ChecksumMismatch { .. })
        ));

        // Flip a bit in the trailing CRC32 -> ChecksumMismatch
        let mut corrupted_crc = serialized.clone();
        let last = corrupted_crc.len() - 1;
        corrupted_crc[last] ^= 0x80;
        assert!(matches!(
            ProjectState::deserialize(&corrupted_crc),
            Err(StateError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn test_preset_crc32_corruption_detection() {
        let mut preset = Preset::new("Integrity Check");
        preset.author = Some("Audio Lab".to_string());
        preset.tags = vec!["mastering".to_string()];
        preset.parameter_values.set(102, 1000.0);

        let mut serialized = preset.serialize();
        assert_eq!(&serialized[..4], &STATE_MAGIC);

        serialized[12] ^= 0xFF;
        assert!(matches!(
            Preset::deserialize(&serialized),
            Err(StateError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn test_preset_json_roundtrip_and_errors() {
        let mut preset = Preset::new("JSON Mastering");
        preset.author = Some("Equaform".to_string());
        preset.tags = vec!["master".to_string(), "clean".to_string()];
        preset.parameter_values.set(global::OUTPUT_GAIN_DB, -1.5);
        preset.parameter_values.set(102, 80.0);
        preset.parameter_values.set(103, 2.5);

        let json = preset.to_json().unwrap();
        assert!(json.contains("\"Equaform-8\""));
        assert!(json.contains("\"JSON Mastering\""));

        let decoded = Preset::from_json(&json).unwrap();
        assert_eq!(decoded, preset);

        // Malformed JSON
        assert_eq!(
            Preset::from_json("{not valid json"),
            Err(StateError::InvalidJson)
        );

        // Future schema version in JSON
        let future_json = r#"{
            "schema_version": 99,
            "product": "Equaform-8",
            "name": "Future",
            "parameters": {}
        }"#;
        assert_eq!(
            Preset::from_json(future_json),
            Err(StateError::UnsupportedFutureVersion { version: 99 })
        );

        // Unknown product in JSON
        let unknown_prod = r#"{
            "schema_version": 1,
            "product": "OtherEQ",
            "name": "Foreign",
            "parameters": {}
        }"#;
        assert_eq!(
            Preset::from_json(unknown_prod),
            Err(StateError::UnknownProduct)
        );
    }

    #[test]
    fn test_atomic_write_and_overwrite_no_tmp_leftover() {
        let dir = std::env::temp_dir().join(format!("equaform_atomic_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file_path = dir.join("state.bin");

        atomic_write(&file_path, b"first payload").unwrap();
        assert_eq!(std::fs::read(&file_path).unwrap(), b"first payload");

        atomic_write(&file_path, b"second payload").unwrap();
        assert_eq!(std::fs::read(&file_path).unwrap(), b"second payload");

        // Ensure no leftover .tmp files exist in the directory
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["state.bin".to_string()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_fuzz_adversarial_corpus_never_panics() {
        // 1. Hand-crafted adversarial inputs with valid CRC32 framing
        let mut oom_param_count = Vec::new();
        oom_param_count.extend_from_slice(&CURRENT_SCHEMA_VERSION.to_le_bytes());
        oom_param_count.extend_from_slice(&6u32.to_le_bytes());
        oom_param_count.extend_from_slice(b"RomuEq");
        oom_param_count.extend_from_slice(&u32::MAX.to_le_bytes());

        let mut oom_tag_count = Vec::new();
        oom_tag_count.extend_from_slice(&CURRENT_SCHEMA_VERSION.to_le_bytes());
        oom_tag_count.extend_from_slice(&6u32.to_le_bytes());
        oom_tag_count.extend_from_slice(b"RomuEq");
        oom_tag_count.extend_from_slice(&1u32.to_le_bytes());
        oom_tag_count.push(b'A');
        oom_tag_count.push(0); // no author
        oom_tag_count.extend_from_slice(&u32::MAX.to_le_bytes()); // tag_count = u32::MAX

        let mut invalid_utf8_name = Vec::new();
        invalid_utf8_name.extend_from_slice(&CURRENT_SCHEMA_VERSION.to_le_bytes());
        invalid_utf8_name.extend_from_slice(&6u32.to_le_bytes());
        invalid_utf8_name.extend_from_slice(b"RomuEq");
        invalid_utf8_name.extend_from_slice(&2u32.to_le_bytes());
        invalid_utf8_name.extend_from_slice(&[0xFF, 0xFE]);

        let mut nan_param = Vec::new();
        nan_param.extend_from_slice(&CURRENT_SCHEMA_VERSION.to_le_bytes());
        nan_param.extend_from_slice(&6u32.to_le_bytes());
        nan_param.extend_from_slice(b"RomuEq");
        nan_param.extend_from_slice(&1u32.to_le_bytes());
        nan_param.extend_from_slice(&1u32.to_le_bytes());
        nan_param.extend_from_slice(&f32::NAN.to_le_bytes());

        let crafted: Vec<Vec<u8>> = vec![
            vec![],
            vec![0; 4],
            vec![0; 8],
            STATE_MAGIC.to_vec(),
            wrap_framed(&[]),
            wrap_framed(&[0xFF; 4]),
            wrap_framed(&oom_param_count),
            wrap_framed(&oom_tag_count),
            wrap_framed(&invalid_utf8_name),
            wrap_framed(&nan_param),
        ];

        for payload in &crafted {
            assert!(ProjectState::deserialize(payload).is_err());
            assert!(Preset::deserialize(payload).is_err());
        }

        // 2. Deterministic PRNG corpus (raw + valid-CRC32 framed)
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next_u8 = || -> u8 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed & 0xFF) as u8
        };

        for idx in 0..500usize {
            let len = idx % 128;
            let raw: Vec<u8> = (0..len).map(|_| next_u8()).collect();
            let framed = wrap_framed(&raw);

            let _ = ProjectState::deserialize(&raw);
            let _ = Preset::deserialize(&raw);
            let _ = ProjectState::deserialize(&framed);
            let _ = Preset::deserialize(&framed);

            if let Ok(text) = std::str::from_utf8(&raw) {
                let _ = Preset::from_json(text);
            }
        }

        // 3. Adversarial JSON payloads (non-finite / oversized / wrong types)
        let bad_jsons = [
            "",
            "null",
            "[]",
            "{}",
            r#"{"schema_version":1,"product":"Equaform-8","name":"X","parameters":{"1":null}}"#,
            r#"{"schema_version":1,"product":"Equaform-8","name":"X","parameters":{"bad":1.0}}"#,
            r#"{"schema_version":0,"product":"Equaform-8","name":"X","parameters":{}}"#,
        ];
        for json in bad_jsons {
            assert!(Preset::from_json(json).is_err());
        }
    }

    #[test]
    fn test_read_to_string_bounded_enforces_limit_and_utf8()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!("equaform_bounded_{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;

        let exact_path = dir.join("exact.txt");
        atomic_write(&exact_path, b"abcd")?;
        assert_eq!(read_to_string_bounded(&exact_path, 4)?, "abcd");

        let over_path = dir.join("over.txt");
        atomic_write(&over_path, b"abcde")?;
        let over_err = read_to_string_bounded(&over_path, 4).unwrap_err();
        assert_eq!(over_err.kind(), std::io::ErrorKind::InvalidData);

        let utf8_path = dir.join("invalid_utf8.txt");
        atomic_write(&utf8_path, &[0xFF, 0xFE, 0xFD])?;
        let utf8_err = read_to_string_bounded(&utf8_path, 16).unwrap_err();
        assert_eq!(utf8_err.kind(), std::io::ErrorKind::InvalidData);

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
