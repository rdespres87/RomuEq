#![forbid(unsafe_code)]

//! Common types and traits for RomuEq.

pub mod audio_buffer;
pub mod audio_ring;

/// Maximum number of channels for analysis data (RMS, peak, zero-crossings).
pub const ANALYSIS_MAX_CHANNELS: usize = 8;

pub mod automation;

pub mod plugin;

pub mod parameter;

pub mod state;

pub use audio_buffer::AudioBuffer;
pub use audio_buffer::EngineLifecycle as CommonEngineLifecycle;
pub use audio_buffer::LifecycleState as CommonLifecycleState;
pub use audio_buffer::MAX_BLOCK_SIZE;
pub use audio_buffer::MAX_CHANNELS;
pub use audio_ring::{AudioRingConsumer, AudioRingProducer, create_audio_ring_buffer};
pub use parameter::BandParameters;
pub use parameter::EqParameters;
pub use parameter::FilterType;
pub use parameter::GlobalParameters;
pub use parameter::MAX_BANDS;
pub use parameter::ParamId;
pub use parameter::ParameterCommand;
pub use parameter::ProcessingMode;
pub use parameter::Slope;
pub use parameter::StereoMode;

// ---------------------------------------------------------------------------
// M2-T001/M5-T001: Bounded SPSC transport (exported for analysis handoff)
// ---------------------------------------------------------------------------

// OverflowPolicy and SpscRingBuffer are already pub in this module.

// ---------------------------------------------------------------------------
// M2-T001: Bounded SPSC parameter transport prototypes
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicUsize, Ordering};

/// Overflow policy for bounded SPSC queues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// Drop the new value when the queue is full.
    DropNew,
    /// Overwrite the oldest value when the queue is full.
    OverwriteOldest,
}

/// Cache-line size for padding to avoid false sharing.
const CACHE_LINE: usize = 64;

/// Padding struct to align a field to a cache line.
#[allow(dead_code)]
struct CachePadded<T> {
    value: T,
    pad: [usize; CACHE_LINE / std::mem::size_of::<usize>() - 1],
}

impl<T> CachePadded<T> {
    const fn new(value: T) -> Self {
        CachePadded {
            value,
            pad: [0; CACHE_LINE / std::mem::size_of::<usize>() - 1],
        }
    }
}

impl CachePadded<AtomicUsize> {
    fn load(&self, order: Ordering) -> usize {
        self.value.load(order)
    }

    fn store(&self, val: usize, order: Ordering) {
        self.value.store(val, order)
    }
}

// ===========================================================================
// Design 1: Classic lock-free SPSC ring buffer
// ===========================================================================

/// A bounded single-producer single-consumer ring buffer.
///
/// Uses separate atomic head and tail indices with cache-line padding to
/// prevent false sharing. The producer and consumer never block.
///
/// Overflow behavior is controlled by `OverflowPolicy`:
/// - `DropNew`: the producer silently discards the value when full.
/// - `OverwriteOldest`: the consumer reads the overwritten value, then the
///   new value on the next dequeue.
///
/// **Invariants:**
/// - Exactly one producer thread and one consumer thread.
/// - `capacity` must be a power of two.
/// - No heap allocation after construction.
/// - No blocking syscalls in either direction.
pub struct SpscRingBuffer<T> {
    /// Slots for queued values. None means "empty slot".
    slots: Box<[Option<T>]>,
    /// Capacity (power of two).
    capacity: usize,
    /// Mask for fast modulo: `index & mask == index % capacity`.
    mask: usize,
    /// Tail index (producer writes here). Cache-line padded.
    tail: CachePadded<AtomicUsize>,
    /// Head index (consumer reads here). Cache-line padded.
    head: CachePadded<AtomicUsize>,
    /// Overflow policy.
    policy: OverflowPolicy,
    /// Number of items currently in the buffer.
    count: CachePadded<AtomicUsize>,
}

impl<T> SpscRingBuffer<T> {
    /// Create a new ring buffer with the given capacity (power of two).
    ///
    /// # Panics
    /// Panics if `capacity` is zero or not a power of two.
    pub fn new(capacity: usize, policy: OverflowPolicy) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        assert!(
            capacity.is_power_of_two(),
            "capacity must be a power of two"
        );

        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(None);
        }

        SpscRingBuffer {
            slots: slots.into_boxed_slice(),
            capacity,
            mask: capacity - 1,
            tail: CachePadded::new(AtomicUsize::new(0)),
            head: CachePadded::new(AtomicUsize::new(0)),
            policy,
            count: CachePadded::new(AtomicUsize::new(0)),
        }
    }

    /// Try to enqueue a value.
    ///
    /// Returns `true` if the value was enqueued, `false` if it was dropped
    /// (overflow policy is `DropNew` and the buffer is full).
    ///
    /// **RT-safe:** no allocation, no blocking, no syscalls.
    pub fn enqueue(&mut self, value: T) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        let count = tail.wrapping_sub(head);

        if count >= self.capacity {
            // Buffer full
            if self.policy == OverflowPolicy::OverwriteOldest {
                // Overwrite: advance head to make room, consumer will see
                // the overwritten slot as "consumed".
                let new_head = head + 1;
                self.head.store(new_head, Ordering::Release);
                // count stays the same — we're replacing, not adding
            } else {
                return false;
            }
        } else {
            self.count.store(count + 1, Ordering::Relaxed);
        }

        self.slots[tail & self.mask] = Some(value);
        self.tail.store(tail + 1, Ordering::Release);

        true
    }

    /// Try to dequeue a value.
    ///
    /// Returns `Some(value)` if the buffer was non-empty, `None` otherwise.
    ///
    /// **RT-safe:** no allocation, no blocking, no syscalls.
    pub fn dequeue(&mut self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let count = tail.wrapping_sub(head);

        if count == 0 {
            return None;
        }

        let value = self.slots[head & self.mask].take();
        self.head.store(head + 1, Ordering::Release);
        self.count.store(count - 1, Ordering::Relaxed);

        value
    }

    /// Current number of items in the buffer.
    pub fn len(&self) -> usize {
        self.count.value.load(Ordering::Relaxed)
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if the buffer is full.
    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity
    }

    /// Capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Overflow policy in use.
    pub fn policy(&self) -> OverflowPolicy {
        self.policy
    }
}

// ===========================================================================
// Design 2: Double-buffered SPSC ring buffer
// ===========================================================================

/// A double-buffered SPSC ring buffer.
///
/// Uses two ring buffers. The producer writes to buffer B while the consumer
/// reads from buffer A. When buffer B is full or the producer decides to
/// swap, an atomic swap makes B the read buffer and A the write buffer.
///
/// This design allows the producer to batch writes without contending with
/// the consumer, at the cost of up to 2x memory usage.
///
/// **Invariants:**
/// - Exactly one producer thread and one consumer thread.
/// - `buffer_capacity` must be a power of two.
/// - No heap allocation after construction.
/// - No blocking syscalls in either direction.
pub struct DoubleBufferedSpsc<T> {
    /// Write buffer.
    write_buf: SpscRingBuffer<T>,
    /// Read buffer.
    read_buf: SpscRingBuffer<T>,
    /// Which buffer is currently being written to (0 = write_buf, 1 = read_buf).
    write_index: AtomicUsize,
}

impl<T: Clone> DoubleBufferedSpsc<T> {
    /// Create a new double-buffered SPSC with the given per-buffer capacity.
    ///
    /// Total capacity is `2 * buffer_capacity`.
    ///
    /// # Panics
    /// Panics if `buffer_capacity` is zero or not a power of two.
    pub fn new(buffer_capacity: usize, policy: OverflowPolicy) -> Self {
        assert!(buffer_capacity > 0, "buffer_capacity must be > 0");
        assert!(
            buffer_capacity.is_power_of_two(),
            "buffer_capacity must be a power of two"
        );

        DoubleBufferedSpsc {
            write_buf: SpscRingBuffer::new(buffer_capacity, policy),
            read_buf: SpscRingBuffer::new(buffer_capacity, policy),
            write_index: AtomicUsize::new(0),
        }
    }

    /// Try to enqueue a value.
    ///
    /// If the write buffer is full, attempts a swap with the read buffer.
    /// If the read buffer is also full, the value is dropped (or overwrites
    /// oldest depending on policy).
    ///
    /// Returns `true` if the value was enqueued, `false` if dropped.
    pub fn enqueue(&mut self, value: T) -> bool {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        let write_buf = if write_idx == 0 {
            &mut self.write_buf
        } else {
            &mut self.read_buf
        };

        if write_buf.enqueue(value.clone()) {
            return true;
        }

        // Write buffer full — try to swap
        // Use compare-and-swap to avoid race with consumer swap
        let new_idx = 1 - write_idx;
        if self
            .write_index
            .compare_exchange(write_idx, new_idx, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            // Swap succeeded: new write buffer is now the old read buffer
            // which may have unconsumed data — that's fine, consumer will
            // drain it.
            let new_write_buf = if new_idx == 0 {
                &mut self.write_buf
            } else {
                &mut self.read_buf
            };
            new_write_buf.enqueue(value)
        } else {
            // Swap failed: another thread swapped, retry with new write buf
            let write_idx = self.write_index.load(Ordering::Relaxed);
            let write_buf = if write_idx == 0 {
                &mut self.write_buf
            } else {
                &mut self.read_buf
            };
            write_buf.enqueue(value)
        }
    }

    /// Try to dequeue a value.
    ///
    /// Drains from the current read buffer. If empty, tries to swap with
    /// the write buffer (if it has data).
    pub fn dequeue(&mut self) -> Option<T> {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        let read_idx = 1 - write_idx;

        // Try to read from current read buffer
        let read_buf = if read_idx == 0 {
            &mut self.write_buf
        } else {
            &mut self.read_buf
        };

        if let Some(value) = read_buf.dequeue() {
            return Some(value);
        }

        // Read buffer empty — try to swap with write buffer
        let new_read_idx = write_idx;
        if self
            .write_index
            .compare_exchange(write_idx, new_read_idx, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            // Swap succeeded: the old write buffer is now the read buffer
            let new_read_buf = if new_read_idx == 0 {
                &mut self.write_buf
            } else {
                &mut self.read_buf
            };
            new_read_buf.dequeue()
        } else {
            // Swap failed: retry
            let write_idx = self.write_index.load(Ordering::Relaxed);
            let read_idx = 1 - write_idx;
            let read_buf = if read_idx == 0 {
                &mut self.write_buf
            } else {
                &mut self.read_buf
            };
            read_buf.dequeue()
        }
    }

    /// Total capacity across both buffers.
    pub fn total_capacity(&self) -> usize {
        self.write_buf.capacity() * 2
    }

    /// Number of items currently in the read buffer.
    pub fn read_len(&self) -> usize {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        let read_idx = 1 - write_idx;
        if read_idx == 0 {
            self.write_buf.len()
        } else {
            self.read_buf.len()
        }
    }

    /// Number of items currently in the write buffer.
    pub fn write_len(&self) -> usize {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        if write_idx == 0 {
            self.write_buf.len()
        } else {
            self.read_buf.len()
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // SpscRingBuffer tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_ring_basic_enqueue_dequeue() {
        let mut buf = SpscRingBuffer::new(4, OverflowPolicy::DropNew);
        assert!(buf.is_empty());
        assert_eq!(buf.capacity(), 4);

        buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(1),
            value: 0.5,
        });
        buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(2),
            value: 0.7,
        });

        assert_eq!(buf.len(), 2);
        assert!(!buf.is_empty());
        assert!(!buf.is_full());

        let v1 = buf.dequeue().unwrap();
        let v2 = buf.dequeue().unwrap();

        match v1 {
            ParameterCommand::SetParameter { id, value } => {
                assert_eq!(id.0, 1);
                assert!((value - 0.5).abs() < f32::EPSILON);
            }
            _ => panic!("unexpected command"),
        }

        match v2 {
            ParameterCommand::SetParameter { id, value } => {
                assert_eq!(id.0, 2);
                assert!((value - 0.7).abs() < f32::EPSILON);
            }
            _ => panic!("unexpected command"),
        }

        assert!(buf.is_empty());
        assert!(buf.dequeue().is_none());
    }

    #[test]
    fn test_ring_full_drop_new() {
        let mut buf = SpscRingBuffer::new(2, OverflowPolicy::DropNew);
        buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(1),
            value: 1.0,
        });
        buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(2),
            value: 2.0,
        });

        assert!(buf.is_full());

        // Should be dropped
        let result = buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(3),
            value: 3.0,
        });
        assert!(!result);
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn test_ring_full_overwrite_oldest() {
        let mut buf = SpscRingBuffer::new(2, OverflowPolicy::OverwriteOldest);
        buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(1),
            value: 1.0,
        });
        buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(2),
            value: 2.0,
        });

        assert!(buf.is_full());

        // Should overwrite oldest
        let result = buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(3),
            value: 3.0,
        });
        assert!(result);
        assert_eq!(buf.len(), 2);

        // Consumer should see ParamId(2) then ParamId(3)
        let v1 = buf.dequeue().unwrap();
        let v2 = buf.dequeue().unwrap();

        match v1 {
            ParameterCommand::SetParameter { id, .. } => assert_eq!(id.0, 2),
            _ => panic!("unexpected command"),
        }
        match v2 {
            ParameterCommand::SetParameter { id, .. } => assert_eq!(id.0, 3),
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_ring_wraparound() {
        let mut buf = SpscRingBuffer::new(4, OverflowPolicy::DropNew);

        // Fill and drain once
        for i in 0..4 {
            buf.enqueue(ParameterCommand::SetParameter {
                id: ParamId(i),
                value: i as f32,
            });
        }
        for _ in 0..4 {
            buf.dequeue().unwrap();
        }
        assert!(buf.is_empty());

        // Fill again — tests wraparound
        for i in 4..8 {
            buf.enqueue(ParameterCommand::SetParameter {
                id: ParamId(i),
                value: i as f32,
            });
        }

        for i in 4..8 {
            let v = buf.dequeue().unwrap();
            match v {
                ParameterCommand::SetParameter { id, value } => {
                    assert_eq!(id.0, i);
                    assert!((value - i as f32).abs() < f32::EPSILON);
                }
                _ => panic!("unexpected command"),
            }
        }
    }

    #[test]
    fn test_ring_setall_command() {
        let mut buf = SpscRingBuffer::new(2, OverflowPolicy::DropNew);

        let params = EqParameters {
            global: GlobalParameters {
                bypass: false,
                output_gain_db: 0.0,
                auto_gain: false,
                processing_mode: ProcessingMode::MinimumPhase,
                stereo_mode: StereoMode::Stereo,
            },
            bands: [BandParameters {
                enabled: false,
                filter_type: FilterType::Bell,
                frequency_hz: 440.0,
                gain_db: 0.0,
                q: 1.0,
                slope: Slope::Db12,
            }; MAX_BANDS],
        };

        buf.enqueue(ParameterCommand::SetAll(params));
        let v = buf.dequeue().unwrap();
        match v {
            ParameterCommand::SetAll(p) => {
                assert!(!p.global.bypass);
                assert_eq!(p.bands.len(), MAX_BANDS);
            }
            _ => panic!("expected SetAll"),
        }
    }

    // -----------------------------------------------------------------------
    // DoubleBufferedSpsc tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_double_buffer_basic() {
        let mut buf = DoubleBufferedSpsc::new(4, OverflowPolicy::DropNew);
        assert_eq!(buf.total_capacity(), 8);
        assert!(buf.read_len() == 0);
        assert!(buf.write_len() == 0);

        buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(1),
            value: 0.5,
        });
        buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(2),
            value: 0.7,
        });

        assert_eq!(buf.write_len(), 2);

        let v1 = buf.dequeue().unwrap();
        let v2 = buf.dequeue().unwrap();

        match v1 {
            ParameterCommand::SetParameter { id, .. } => assert_eq!(id.0, 1),
            _ => panic!("unexpected command"),
        }
        match v2 {
            ParameterCommand::SetParameter { id, .. } => assert_eq!(id.0, 2),
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_double_buffer_swap_on_full() {
        let mut buf = DoubleBufferedSpsc::new(2, OverflowPolicy::DropNew);

        // Fill write buffer
        buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(1),
            value: 1.0,
        });
        buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(2),
            value: 2.0,
        });
        assert!(buf.write_len() == 2);

        // This should trigger a swap and write to the other buffer
        let result = buf.enqueue(ParameterCommand::SetParameter {
            id: ParamId(3),
            value: 3.0,
        });
        assert!(result);

        // Drain all
        let v1 = buf.dequeue().unwrap();
        let v2 = buf.dequeue().unwrap();
        let v3 = buf.dequeue().unwrap();

        match v1 {
            ParameterCommand::SetParameter { id, .. } => assert_eq!(id.0, 1),
            _ => panic!("unexpected command"),
        }
        match v2 {
            ParameterCommand::SetParameter { id, .. } => assert_eq!(id.0, 2),
            _ => panic!("unexpected command"),
        }
        match v3 {
            ParameterCommand::SetParameter { id, .. } => assert_eq!(id.0, 3),
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_double_buffer_interleaved_produce_consume() {
        let mut buf = DoubleBufferedSpsc::new(4, OverflowPolicy::DropNew);

        // Produce 3, consume 1, produce 2, consume 2
        for i in 0..3 {
            buf.enqueue(ParameterCommand::SetParameter {
                id: ParamId(i),
                value: i as f32,
            });
        }

        assert_eq!(
            buf.dequeue().unwrap(),
            ParameterCommand::SetParameter {
                id: ParamId(0),
                value: 0.0,
            }
        );

        for i in 3..5 {
            buf.enqueue(ParameterCommand::SetParameter {
                id: ParamId(i),
                value: i as f32,
            });
        }

        for i in 1..5 {
            let v = buf.dequeue().unwrap();
            match v {
                ParameterCommand::SetParameter { id, value } => {
                    assert_eq!(id.0, i);
                    assert!((value - i as f32).abs() < f32::EPSILON);
                }
                _ => panic!("unexpected command"),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Latency / contention benchmark helpers
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    /// Measure enqueue latency for a single value.
    ///
    /// Returns the median latency in nanoseconds over `iterations` enqueues.
    pub fn measure_enqueue_latency<T>(
        buf: &mut SpscRingBuffer<T>,
        value: &T,
        iterations: usize,
    ) -> u64
    where
        T: Clone,
    {
        let mut latencies = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = std::time::Instant::now();
            // Drain first to ensure buffer is not full
            while buf.dequeue().is_some() {}
            buf.enqueue(value.clone());
            let elapsed = start.elapsed();
            latencies.push(elapsed.as_nanos() as u64);
        }
        latencies.sort();
        latencies[iterations / 2]
    }

    #[allow(dead_code)]
    /// Measure dequeue latency for a single value.
    ///
    /// Returns the median latency in nanoseconds over `iterations` dequeues.
    pub fn measure_dequeue_latency<T>(buf: &mut SpscRingBuffer<T>, iterations: usize) -> u64
    where
        T: Default + Clone,
    {
        // Fill the buffer first
        for _ in 0..buf.capacity() {
            buf.enqueue(T::default());
        }

        let mut latencies = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = std::time::Instant::now();
            buf.dequeue();
            let elapsed = start.elapsed();
            latencies.push(elapsed.as_nanos() as u64);
        }
        latencies.sort();
        latencies[iterations / 2]
    }
}
