#![forbid(unsafe_code)]

//! Automation event types for host automation event ingestion.
//!
//! This module defines the core types used to represent automation events
//! flowing from the host into the engine. Events carry a parameter identifier,
//! a logical value, a sample offset (relative to the start of the current
//! audio block), and a deterministic sequence number.
//!
//! # Ingestion contract
//!
//! - Events are **always** sorted by `(sample_offset, sequence)` before
//!   application.
//! - Offsets outside `[0, block_length)` are rejected — they cannot corrupt
//!   state.
//! - Duplicate offsets for the same parameter are resolved by keeping the
//!   **last** event (highest sequence number).
//! - The host event path is kept separate from GUI/control commands.

use crate::ParamId;

// ---------------------------------------------------------------------------
// Automation event
// ---------------------------------------------------------------------------

/// A single automation event from the host.
///
/// Fields:
/// - `param_id`: which parameter to change
/// - `value`: the logical (normalised) value
/// - `sample_offset`: relative to the start of the current block
/// - `sequence`: monotonically increasing, assigned by the producer
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutomationEvent {
    /// Parameter identifier.
    pub param_id: ParamId,
    /// Normalised parameter value in `[0.0, 1.0]`.
    pub value: f32,
    /// Sample offset relative to the start of the current block.
    pub sample_offset: u32,
    /// Deterministic sequence number (monotonically increasing).
    pub sequence: u64,
}

impl AutomationEvent {
    /// Create a new automation event.
    pub fn new(param_id: ParamId, value: f32, sample_offset: u32, sequence: u64) -> Self {
        AutomationEvent {
            param_id,
            value,
            sample_offset,
            sequence,
        }
    }

    /// Returns `true` if this event is valid for a block of the given length.
    pub fn is_valid_for_block(&self, block_length: u32) -> bool {
        self.sample_offset < block_length
    }
}

/// Comparison key for deterministic ordering: `(sample_offset, sequence)`.
///
/// Events are sorted by this key before application.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SortKey {
    pub sample_offset: u32,
    pub sequence: u64,
}

impl SortKey {
    pub fn from_event(event: &AutomationEvent) -> Self {
        SortKey {
            sample_offset: event.sample_offset,
            sequence: event.sequence,
        }
    }
}

// ---------------------------------------------------------------------------
// Automation event batch
// ---------------------------------------------------------------------------

/// A batch of automation events received from the host for one block.
///
/// The batch is immutable once created — ingestion validates and sorts
/// before producing an `AutomationBatch`.
#[derive(Clone, Debug)]
pub struct AutomationBatch {
    /// Sorted, deduplicated events.
    events: Vec<AutomationEvent>,
}

impl AutomationBatch {
    /// Create a new batch from raw events.
    ///
    /// Validation and sorting are applied:
    /// 1. Events with `sample_offset >= block_length` are dropped.
    /// 2. Events are sorted by `(sample_offset, sequence)`.
    /// 3. Duplicate `(param_id, sample_offset)` pairs keep the event with
    ///    the highest sequence number.
    pub fn from_raw(events: Vec<AutomationEvent>, block_length: u32) -> Self {
        let mut filtered: Vec<AutomationEvent> = events
            .into_iter()
            .filter(|e| e.is_valid_for_block(block_length))
            .collect();

        // Sort by (sample_offset, sequence)
        filtered.sort_by_key(SortKey::from_event);

        // Deduplicate: for same (param_id, sample_offset), keep highest sequence
        let mut deduped: Vec<AutomationEvent> = Vec::with_capacity(filtered.len());
        let mut i = 0;
        while i < filtered.len() {
            let mut j = i + 1;
            while j < filtered.len()
                && filtered[i].param_id == filtered[j].param_id
                && filtered[i].sample_offset == filtered[j].sample_offset
            {
                j += 1;
            }
            // Keep the last (highest sequence) since events are sorted by sequence
            // within same offset (because SortKey includes sequence)
            deduped.push(filtered[j - 1]);
            i = j;
        }

        AutomationBatch { events: deduped }
    }

    /// Returns the number of events in the batch.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns `true` if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Iterate over events in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = &AutomationEvent> {
        self.events.iter()
    }

    /// Get an event by index.
    pub fn get(&self, index: usize) -> Option<&AutomationEvent> {
        self.events.get(index)
    }
}

// ---------------------------------------------------------------------------
// Automation event stream (bounded queue consumer)
// ---------------------------------------------------------------------------

/// A consumer-side view of the automation event stream.
///
/// Events arrive via the bounded SPSC queue and are
/// accumulated into batches per block. The consumer is responsible for
/// producing `AutomationBatch` instances from raw events.
#[derive(Debug)]
pub struct AutomationStream {
    /// Events accumulated for the current block.
    pending: Vec<AutomationEvent>,
}

impl AutomationStream {
    /// Create a new automation stream.
    pub fn new() -> Self {
        AutomationStream {
            pending: Vec::new(),
        }
    }

    /// Push a raw event into the pending buffer.
    ///
    /// This is called by the RT consumer after dequeuing from the SPSC queue.
    pub fn push_event(&mut self, event: AutomationEvent) {
        self.pending.push(event);
    }

    /// Finalise the current block and produce a sorted, deduplicated batch.
    ///
    /// After calling this, `pending` is cleared and a new batch is ready
    /// for the engine to consume.
    pub fn finalize_block(&mut self, block_length: u32) -> AutomationBatch {
        let events = std::mem::take(&mut self.pending);
        AutomationBatch::from_raw(events, block_length)
    }

    /// Returns the number of pending events for the current block.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for AutomationStream {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Parameter value smoothing (sample-accurate interpolation)
// ---------------------------------------------------------------------------

/// Interpolation mode for automation values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterpolationMode {
    /// Linear interpolation between consecutive automation points.
    Linear,
    /// Hold the last value (step interpolation).
    Hold,
}

/// Sample-accurate automation value provider.
///
/// Given a sequence of automation events and a block length, this struct
/// can produce the value for any sample within the block.
#[derive(Debug)]
pub struct AutomationValueProvider {
    /// Sorted events for the current block.
    events: Vec<AutomationEvent>,
    /// Interpolation mode.
    interpolation: InterpolationMode,
}

impl AutomationValueProvider {
    /// Create a new value provider from a batch.
    pub fn new(events: AutomationBatch, interpolation: InterpolationMode) -> Self {
        AutomationValueProvider {
            events: events.events,
            interpolation,
        }
    }

    /// Get the value for a specific parameter at a specific sample.
    ///
    /// Returns the value of the event whose sample_offset matches, or the
    /// interpolated/hold value for the interval between events.
    /// If no event has ever been seen for the parameter, returns `None`.
    ///
    /// **RT-safe:** performs zero heap allocations.
    pub fn value_at_sample(
        &self,
        param_id: ParamId,
        sample: u32,
        last_known: Option<f32>,
    ) -> Option<f32> {
        let mut prev: Option<&AutomationEvent> = None;
        let mut next: Option<&AutomationEvent> = None;

        for e in self.events.iter().filter(|e| e.param_id == param_id) {
            if e.sample_offset <= sample {
                prev = Some(e);
            } else {
                next = Some(e);
                break;
            }
        }

        match prev {
            Some(prev_ev) => {
                if prev_ev.sample_offset == sample || self.interpolation == InterpolationMode::Hold
                {
                    return Some(prev_ev.value);
                }

                if let Some(next_ev) = next {
                    let range =
                        (next_ev.sample_offset as f64 - prev_ev.sample_offset as f64).max(1.0);
                    let t = (sample as f64 - prev_ev.sample_offset as f64) / range;
                    let prev_v = prev_ev.value as f64;
                    let curr_v = next_ev.value as f64;
                    return Some((prev_v + t * (curr_v - prev_v)) as f32);
                }

                Some(prev_ev.value)
            }
            None => {
                // All events are after the sample — return the first one (or last_known)
                match next {
                    Some(first_ev) => last_known.or(Some(first_ev.value)),
                    None => last_known,
                }
            }
        }
    }

    /// Returns the last automation value for the given parameter within the block,
    /// or `None` if no automation event exists for this parameter in the block.
    ///
    /// This method performs zero allocations, operates in RT-safe O(E) time where E
    /// is the number of events in the block, and avoids looping over all samples.
    pub fn last_value_in_block(&self, param_id: ParamId, block_length: u32) -> Option<f32> {
        self.events
            .iter()
            .rev()
            .find(|e| e.param_id == param_id && e.sample_offset < block_length)
            .map(|e| e.value)
    }

    /// Iterate over events for a specific parameter.
    pub fn events_for_param(&self, param_id: ParamId) -> impl Iterator<Item = &AutomationEvent> {
        self.events.iter().filter(move |e| e.param_id == param_id)
    }

    /// Returns the total number of events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(param_id: ParamId, value: f32, offset: u32, seq: u64) -> AutomationEvent {
        AutomationEvent::new(param_id, value, offset, seq)
    }

    // -----------------------------------------------------------------------
    // AutomationEvent
    // -----------------------------------------------------------------------

    #[test]
    fn test_event_is_valid_for_block() {
        let event = make_event(ParamId(0), 0.5, 0, 0);
        assert!(event.is_valid_for_block(256));
        assert!(event.is_valid_for_block(1));
        assert!(!event.is_valid_for_block(0));
    }

    #[test]
    fn test_event_offset_at_block_length_is_invalid() {
        let event = make_event(ParamId(0), 0.5, 256, 0);
        assert!(!event.is_valid_for_block(256));
    }

    // -----------------------------------------------------------------------
    // SortKey
    // -----------------------------------------------------------------------

    #[test]
    fn test_sort_key_ordering() {
        let e1 = make_event(ParamId(0), 0.1, 10, 5);
        let e2 = make_event(ParamId(1), 0.2, 10, 3);
        let e3 = make_event(ParamId(0), 0.3, 5, 10);

        let k1 = SortKey::from_event(&e1);
        let k2 = SortKey::from_event(&e2);
        let k3 = SortKey::from_event(&e3);

        // Same offset: higher sequence sorts later
        assert!(k1 > k2);
        // Lower offset sorts earlier regardless of sequence
        assert!(k3 < k2);
    }

    // -----------------------------------------------------------------------
    // AutomationBatch — filtering
    // -----------------------------------------------------------------------

    #[test]
    fn test_batch_drops_out_of_range_offsets() {
        let events = vec![
            make_event(ParamId(0), 0.5, 0, 1),
            make_event(ParamId(0), 0.6, 255, 2),
            make_event(ParamId(0), 0.7, 256, 3), // out of range
            make_event(ParamId(0), 0.8, 512, 4), // out of range
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn test_batch_empty_block_length_rejects_all() {
        let events = vec![make_event(ParamId(0), 0.5, 0, 1)];
        let batch = AutomationBatch::from_raw(events, 0);
        assert!(batch.is_empty());
    }

    // -----------------------------------------------------------------------
    // AutomationBatch — sorting
    // -----------------------------------------------------------------------

    #[test]
    fn test_batch_sorts_by_offset_then_sequence() {
        let events = vec![
            make_event(ParamId(0), 0.1, 100, 5),
            make_event(ParamId(0), 0.2, 50, 3),
            make_event(ParamId(0), 0.3, 100, 2),
            make_event(ParamId(0), 0.4, 10, 1),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        // Dedup keeps only the highest-sequence event per (param, offset):
        // offset 100 → seq 5 wins over seq 2, so 3 events remain
        assert_eq!(batch.len(), 3);

        let offsets: Vec<u32> = batch.iter().map(|e| e.sample_offset).collect();
        assert_eq!(offsets, vec![10, 50, 100]);

        // Same offset: sorted by sequence ascending
        let seqs: Vec<u64> = batch.iter().map(|e| e.sequence).collect();
        assert_eq!(seqs, vec![1, 3, 5]);
    }

    // -----------------------------------------------------------------------
    // AutomationBatch — deduplication
    // -----------------------------------------------------------------------

    #[test]
    fn test_batch_deduplicates_same_param_same_offset() {
        let events = vec![
            make_event(ParamId(0), 0.1, 50, 1),
            make_event(ParamId(0), 0.5, 50, 2),
            make_event(ParamId(0), 0.9, 50, 3),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        assert_eq!(batch.len(), 1);
        // Keeps the last (highest sequence)
        assert_eq!(batch.get(0).unwrap().value, 0.9);
        assert_eq!(batch.get(0).unwrap().sequence, 3);
    }

    #[test]
    fn test_batch_keeps_different_params_same_offset() {
        let events = vec![
            make_event(ParamId(0), 0.1, 50, 1),
            make_event(ParamId(1), 0.5, 50, 2),
            make_event(ParamId(2), 0.9, 50, 3),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        assert_eq!(batch.len(), 3);
    }

    #[test]
    fn test_batch_dedup_different_offset_same_param() {
        let events = vec![
            make_event(ParamId(0), 0.1, 50, 1),
            make_event(ParamId(0), 0.5, 100, 2),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        assert_eq!(batch.len(), 2);
    }

    // -----------------------------------------------------------------------
    // AutomationBatch — deterministic ordering
    // -----------------------------------------------------------------------

    #[test]
    fn test_batch_deterministic_ordering() {
        // Same events in different input order should produce same batch
        let events_a = vec![
            make_event(ParamId(0), 0.1, 100, 5),
            make_event(ParamId(0), 0.2, 50, 3),
            make_event(ParamId(0), 0.3, 100, 2),
        ];
        let events_b = vec![
            make_event(ParamId(0), 0.3, 100, 2),
            make_event(ParamId(0), 0.1, 100, 5),
            make_event(ParamId(0), 0.2, 50, 3),
        ];
        let batch_a = AutomationBatch::from_raw(events_a, 256);
        let batch_b = AutomationBatch::from_raw(events_b, 256);

        assert_eq!(batch_a.len(), batch_b.len());
        for (ea, eb) in batch_a.iter().zip(batch_b.iter()) {
            assert_eq!(ea.sample_offset, eb.sample_offset);
            assert_eq!(ea.sequence, eb.sequence);
            assert_eq!(ea.value, eb.value);
        }
    }

    // -----------------------------------------------------------------------
    // AutomationBatch — large event counts
    // -----------------------------------------------------------------------

    #[test]
    fn test_batch_large_event_count() {
        let mut events = Vec::new();
        for i in 0..10_000 {
            events.push(make_event(
                ParamId(i % 8),
                (i as f32) / 10_000.0,
                i % 256,
                i as u64,
            ));
        }
        let batch = AutomationBatch::from_raw(events, 256);
        // All 10k events are valid (offsets < 256), but duplicates are
        // deduplicated: 8 params * 256 offsets = 2048 unique (param, offset)
        // pairs, so at most 2048 events remain.
        assert!(batch.len() <= 2048);
        assert!(!batch.is_empty());
    }

    // -----------------------------------------------------------------------
    // AutomationStream
    // -----------------------------------------------------------------------

    #[test]
    fn test_stream_accumulates_and_finalizes() {
        let mut stream = AutomationStream::new();
        stream.push_event(make_event(ParamId(0), 0.1, 10, 1));
        stream.push_event(make_event(ParamId(1), 0.2, 20, 2));
        assert_eq!(stream.pending_count(), 2);

        let batch = stream.finalize_block(256);
        assert_eq!(batch.len(), 2);
        assert_eq!(stream.pending_count(), 0);
    }

    #[test]
    fn test_stream_drops_out_of_range_on_finalize() {
        let mut stream = AutomationStream::new();
        stream.push_event(make_event(ParamId(0), 0.1, 10, 1));
        stream.push_event(make_event(ParamId(0), 0.2, 300, 2)); // out of range

        let batch = stream.finalize_block(256);
        assert_eq!(batch.len(), 1);
        assert_eq!(batch.get(0).unwrap().sample_offset, 10);
    }

    // -----------------------------------------------------------------------
    // AutomationValueProvider
    // -----------------------------------------------------------------------

    #[test]
    fn test_value_provider_hold_mode() {
        let events = vec![
            make_event(ParamId(0), 0.0, 0, 1),
            make_event(ParamId(0), 1.0, 100, 2),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        let provider = AutomationValueProvider::new(batch, InterpolationMode::Hold);

        assert_eq!(provider.value_at_sample(ParamId(0), 0, None), Some(0.0));
        assert_eq!(provider.value_at_sample(ParamId(0), 50, None), Some(0.0));
        assert_eq!(provider.value_at_sample(ParamId(0), 100, None), Some(1.0));
        assert_eq!(provider.value_at_sample(ParamId(0), 200, None), Some(1.0));
    }

    #[test]
    fn test_value_provider_linear_interpolation() {
        let events = vec![
            make_event(ParamId(0), 0.0, 0, 1),
            make_event(ParamId(0), 1.0, 100, 2),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        let provider = AutomationValueProvider::new(batch, InterpolationMode::Linear);

        // At midpoint (sample 50), value should be ~0.5
        let val = provider.value_at_sample(ParamId(0), 50, None).unwrap();
        assert!((val - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_value_provider_no_events_returns_last_known() {
        let events: Vec<AutomationEvent> = vec![];
        let batch = AutomationBatch::from_raw(events, 256);
        let provider = AutomationValueProvider::new(batch, InterpolationMode::Hold);

        assert_eq!(
            provider.value_at_sample(ParamId(0), 50, Some(0.75)),
            Some(0.75)
        );
        assert_eq!(provider.value_at_sample(ParamId(0), 50, None), None);
    }

    #[test]
    fn test_value_provider_events_for_param() {
        let events = vec![
            make_event(ParamId(0), 0.1, 10, 1),
            make_event(ParamId(1), 0.2, 20, 2),
            make_event(ParamId(0), 0.3, 30, 3),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        let provider = AutomationValueProvider::new(batch, InterpolationMode::Hold);

        let band0_count = provider.events_for_param(ParamId(0)).count();
        assert_eq!(band0_count, 2);

        let band1_count = provider.events_for_param(ParamId(1)).count();
        assert_eq!(band1_count, 1);
    }

    #[test]
    fn test_value_provider_last_value_in_block() {
        let events = vec![
            make_event(ParamId(0), 0.1, 10, 1),
            make_event(ParamId(0), 0.4, 50, 2),
            make_event(ParamId(0), 0.8, 100, 3),
            make_event(ParamId(1), 0.5, 20, 4),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        let provider = AutomationValueProvider::new(batch, InterpolationMode::Hold);

        assert_eq!(provider.last_value_in_block(ParamId(0), 256), Some(0.8));
        assert_eq!(provider.last_value_in_block(ParamId(0), 100), Some(0.4));
        assert_eq!(provider.last_value_in_block(ParamId(0), 50), Some(0.1));
        assert_eq!(provider.last_value_in_block(ParamId(0), 10), None);
        assert_eq!(provider.last_value_in_block(ParamId(1), 256), Some(0.5));
        assert_eq!(provider.last_value_in_block(ParamId(2), 256), None);
    }

    // -----------------------------------------------------------------------
    // Malformed input — cannot corrupt state
    // -----------------------------------------------------------------------

    #[test]
    fn test_malformed_offsets_cannot_corrupt_state() {
        // Events with offsets >= block_length are silently dropped,
        // never stored or applied.
        let events = vec![
            make_event(ParamId(0), f32::NAN, 999, 1),
            make_event(ParamId(0), f32::INFINITY, 999, 2),
            make_event(ParamId(0), f32::NEG_INFINITY, 999, 3),
        ];
        let batch = AutomationBatch::from_raw(events, 256);
        assert!(batch.is_empty());
    }

    #[test]
    fn test_random_event_sequences_produce_deterministic_batches() {
        // Generate the same set of events in two different orders
        let mut rng_state = 42u64;
        let mut events_a = Vec::new();
        let mut events_b = Vec::new();

        for i in 0..500 {
            let param = ParamId((rng_state % 8) as u32);
            let value = ((rng_state >> 3) % 1000) as f32 / 1000.0;
            let offset = (rng_state >> 10) % 256;
            let seq = i as u64;
            let event = make_event(param, value, offset as u32, seq);
            events_a.push(event);
            events_b.push(event);
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        }

        // Shuffle events_b differently
        events_b.reverse();

        let batch_a = AutomationBatch::from_raw(events_a, 256);
        let batch_b = AutomationBatch::from_raw(events_b, 256);

        assert_eq!(batch_a.len(), batch_b.len());
        for (ea, eb) in batch_a.iter().zip(batch_b.iter()) {
            assert_eq!(ea.sample_offset, eb.sample_offset);
            assert_eq!(ea.sequence, eb.sequence);
        }
    }
}
