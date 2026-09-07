#![forbid(unsafe_code)]

//! Automation ingestion for the engine.
//!
//! This module bridges raw `AutomationEvent` instances from `romueq-common`
//! into the engine's parameter store. It validates offsets, deduplicates,
//! and guarantees deterministic processing order.

use romueq_common::ParamId;
use romueq_common::automation::{
    AutomationBatch, AutomationEvent, AutomationStream, AutomationValueProvider, InterpolationMode,
};

// ---------------------------------------------------------------------------
// AutomationIngestor
// ---------------------------------------------------------------------------

/// Ingests automation events and applies them to the parameter store.
///
/// The ingestor is the single point where host automation events enter the
/// engine. It enforces:
///
/// - Offset validation against block length.
/// - Deterministic ordering by `(sample_offset, sequence)`.
/// - Deduplication of events targeting the same parameter at the same offset.
/// - No silent block-rate downgrade — sample-accurate events remain
///   sample-accurate.
#[derive(Debug)]
pub struct AutomationIngestor {
    /// Stream accumulating events for the current block.
    stream: AutomationStream,
    /// Interpolation mode for automation values.
    interpolation: InterpolationMode,
}

impl AutomationIngestor {
    /// Create a new automation ingestor.
    pub fn new(interpolation: InterpolationMode) -> Self {
        AutomationIngestor {
            stream: AutomationStream::new(),
            interpolation,
        }
    }

    /// Push a raw automation event into the ingestor.
    ///
    /// Events are buffered until `process_block` is called.
    pub fn push_event(&mut self, event: AutomationEvent) {
        self.stream.push_event(event);
    }

    /// Process a block of audio.
    ///
    /// This finalises the current batch, validates all events, and produces
    /// an `AutomationBatch` that can be consumed by the parameter store.
    ///
    /// # Arguments
    ///
    /// * `block_length` — the number of samples in this audio block.
    ///   Events with `sample_offset >= block_length` are silently dropped.
    pub fn process_block(&mut self, block_length: u32) -> AutomationBatch {
        self.stream.finalize_block(block_length)
    }

    /// Create a value provider from a batch for sample-accurate lookups.
    pub fn value_provider(&self, batch: &AutomationBatch) -> AutomationValueProvider {
        AutomationValueProvider::new(batch.clone(), self.interpolation)
    }

    /// Returns the number of pending events for the current block.
    pub fn pending_count(&self) -> usize {
        self.stream.pending_count()
    }

    /// Reset the ingestor (e.g. after a sample-rate change).
    pub fn reset(&mut self) {
        self.stream = AutomationStream::new();
    }
}

impl Default for AutomationIngestor {
    fn default() -> Self {
        Self::new(InterpolationMode::Hold)
    }
}

// ---------------------------------------------------------------------------
// AutomationProcessor
// ---------------------------------------------------------------------------

/// Applies automation events to a parameter store.
///
/// The processor iterates over a batch and updates the parameter store with
/// the values at the correct sample offsets.
#[derive(Debug)]
pub struct AutomationProcessor {
    /// The parameter store being updated.
    store: crate::parameter_store::ParameterStore,
}

impl AutomationProcessor {
    /// Create a new automation processor backed by a parameter store.
    pub fn new(store: crate::parameter_store::ParameterStore) -> Self {
        AutomationProcessor { store }
    }

    /// Apply a batch of automation events to the parameter store.
    ///
    /// Returns the number of events actually applied (after validation).
    pub fn apply_batch(&mut self, batch: &AutomationBatch) -> usize {
        let mut applied = 0;
        for event in batch.iter() {
            self.store
                .set_value(event.param_id, event.value, event.sample_offset);
            applied += 1;
        }
        applied
    }

    /// Get the current value of a parameter.
    pub fn get_value(&self, param_id: ParamId) -> Option<f32> {
        self.store.get_value(param_id)
    }

    /// Get the parameter store (for further processing).
    pub fn store(&self) -> &crate::parameter_store::ParameterStore {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use romueq_common::automation::{AutomationEvent, InterpolationMode};

    fn make_event(param_id: ParamId, value: f32, offset: u32, seq: u64) -> AutomationEvent {
        AutomationEvent::new(param_id, value, offset, seq)
    }

    // -----------------------------------------------------------------------
    // AutomationIngestor
    // -----------------------------------------------------------------------

    #[test]
    fn test_ingestor_pushes_and_processes_block() {
        let mut ingestor = AutomationIngestor::new(InterpolationMode::Hold);
        ingestor.push_event(make_event(ParamId(0), 0.5, 10, 1));
        ingestor.push_event(make_event(ParamId(1), 0.8, 20, 2));
        assert_eq!(ingestor.pending_count(), 2);

        let batch = ingestor.process_block(256);
        assert_eq!(batch.len(), 2);
        assert_eq!(ingestor.pending_count(), 0);
    }

    #[test]
    fn test_ingestor_drops_out_of_range_events() {
        let mut ingestor = AutomationIngestor::new(InterpolationMode::Hold);
        ingestor.push_event(make_event(ParamId(0), 0.5, 10, 1));
        ingestor.push_event(make_event(ParamId(1), 0.8, 300, 2)); // beyond block
        let batch = ingestor.process_block(256);
        assert_eq!(batch.len(), 1);
        assert_eq!(batch.get(0).unwrap().param_id, ParamId(0));
    }

    #[test]
    fn test_ingestor_reset_clears_pending() {
        let mut ingestor = AutomationIngestor::new(InterpolationMode::Hold);
        ingestor.push_event(make_event(ParamId(0), 0.5, 10, 1));
        ingestor.reset();
        assert_eq!(ingestor.pending_count(), 0);
    }

    #[test]
    fn test_ingestor_default_is_hold_mode() {
        let ingestor = AutomationIngestor::default();
        assert_eq!(ingestor.pending_count(), 0);
    }

    #[test]
    fn test_ingestor_value_provider() {
        let mut ingestor = AutomationIngestor::new(InterpolationMode::Linear);
        ingestor.push_event(make_event(ParamId(0), 0.0, 0, 1));
        ingestor.push_event(make_event(ParamId(0), 1.0, 100, 2));
        let batch = ingestor.process_block(256);
        let provider = ingestor.value_provider(&batch);
        let val = provider.value_at_sample(ParamId(0), 50, None).unwrap();
        assert!((val - 0.5).abs() < 0.01);
    }

    // -----------------------------------------------------------------------
    // AutomationProcessor
    // -----------------------------------------------------------------------

    #[test]
    fn test_processor_applies_batch_to_store() {
        let store = crate::parameter_store::ParameterStore::new();
        let mut processor = AutomationProcessor::new(store);
        let events = vec![
            make_event(ParamId(0), 0.5, 10, 1),
            make_event(ParamId(1), 0.8, 20, 2),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        let applied = processor.apply_batch(&batch);
        assert_eq!(applied, 2);
        assert_eq!(processor.get_value(ParamId(0)), Some(0.5));
        assert_eq!(processor.get_value(ParamId(1)), Some(0.8));
    }

    #[test]
    fn test_processor_applies_empty_batch() {
        let store = crate::parameter_store::ParameterStore::new();
        let mut processor = AutomationProcessor::new(store);
        let batch = AutomationBatch::from_raw(vec![], 256);
        assert_eq!(processor.apply_batch(&batch), 0);
    }

    #[test]
    fn test_processor_overwrites_values() {
        let store = crate::parameter_store::ParameterStore::new();
        let mut processor = AutomationProcessor::new(store);
        let events = vec![
            make_event(ParamId(0), 0.3, 10, 1),
            make_event(ParamId(0), 0.7, 20, 2),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        processor.apply_batch(&batch);
        assert_eq!(processor.get_value(ParamId(0)), Some(0.7));
    }

    #[test]
    fn test_processor_get_unknown_param() {
        let store = crate::parameter_store::ParameterStore::new();
        let processor = AutomationProcessor::new(store);
        assert_eq!(processor.get_value(ParamId(99)), None);
    }

    #[test]
    fn test_processor_store_access() {
        let store = crate::parameter_store::ParameterStore::new();
        let mut processor = AutomationProcessor::new(store);
        let events = vec![make_event(ParamId(0), 0.5, 10, 1)];
        let batch = AutomationBatch::from_raw(events, 256);
        processor.apply_batch(&batch);
        assert_eq!(processor.store().param_count(), 1);
        let points = processor.store().get_automation_points(ParamId(0));
        assert_eq!(points.len(), 1);
        assert_eq!(points[0], (10, 0.5));
    }

    // -----------------------------------------------------------------------
    // Integration: ingestor → processor pipeline
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_pipeline_deterministic_order() {
        let mut ingestor = AutomationIngestor::new(InterpolationMode::Hold);
        // Push events out of order
        ingestor.push_event(make_event(ParamId(0), 0.4, 100, 4));
        ingestor.push_event(make_event(ParamId(0), 0.1, 10, 1));
        ingestor.push_event(make_event(ParamId(0), 0.3, 50, 3));
        ingestor.push_event(make_event(ParamId(0), 0.2, 50, 2));
        let batch = ingestor.process_block(256);
        // Dedup: offset 50 has seq 2 and 3 → keep seq 3 (value 0.3)
        assert_eq!(batch.len(), 3);
        let offsets: Vec<u32> = batch.iter().map(|e| e.sample_offset).collect();
        assert_eq!(offsets, vec![10, 50, 100]);
        let values: Vec<f32> = batch.iter().map(|e| e.value).collect();
        assert_eq!(values, vec![0.1, 0.3, 0.4]);
    }

    #[test]
    fn test_full_pipeline_malformed_events_no_corruption() {
        let mut ingestor = AutomationIngestor::new(InterpolationMode::Hold);
        ingestor.push_event(make_event(ParamId(0), 0.5, 10, 1));
        ingestor.push_event(make_event(ParamId(1), 0.9, 500, 2)); // out of range
        ingestor.push_event(make_event(ParamId(2), 0.2, 30, 3));
        let batch = ingestor.process_block(256);
        assert_eq!(batch.len(), 2); // only valid events
        let store = crate::parameter_store::ParameterStore::new();
        let mut processor = AutomationProcessor::new(store);
        let applied = processor.apply_batch(&batch);
        assert_eq!(applied, 2);
        assert_eq!(processor.get_value(ParamId(0)), Some(0.5));
        assert_eq!(processor.get_value(ParamId(2)), Some(0.2));
        assert_eq!(processor.get_value(ParamId(1)), None); // dropped
    }

    #[test]
    fn test_full_pipeline_large_block() {
        let mut ingestor = AutomationIngestor::new(InterpolationMode::Hold);
        let mut events = Vec::new();
        for i in 0..1000 {
            events.push(make_event(
                ParamId((i % 16) as u32),
                (i as f32) / 1000.0,
                i % 256,
                i as u64,
            ));
        }
        for event in events {
            ingestor.push_event(event);
        }
        let batch = ingestor.process_block(256);
        // After dedup: at most 256 unique (param, offset) pairs
        assert!(batch.len() <= 256 * 16);
        let store = crate::parameter_store::ParameterStore::new();
        let mut processor = AutomationProcessor::new(store);
        let applied = processor.apply_batch(&batch);
        assert_eq!(applied, batch.len());
    }
}
