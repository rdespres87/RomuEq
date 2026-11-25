//! Wait-free, zero-allocation Lock-Free Triple Buffer.
//!
//! Provides a bounded, single-producer single-consumer triple buffer designed
//! for real-time audio parameter exchange.
//!
//! - Producer (GUI thread): Wait-free $O(1)$ write via atomic swap.
//! - Consumer (Audio thread): Wait-free $O(1)$ read via atomic CAS.
//! - Memory footprint: 3 pre-allocated contiguous slots, zero heap allocations,
//!   zero heap deallocations, and zero mutex locks.

#![allow(unsafe_code)]

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const DIRTY_FLAG: u8 = 0b100;
const INDEX_MASK: u8 = 0b011;

/// Shared internal storage for the 3-slot buffer.
struct TripleBufferShared<T> {
    slots: [UnsafeCell<T>; 3],
    /// Packed atomic state:
    /// - Bits 0..1: Index of the shared slot (0, 1, or 2).
    /// - Bit 2 (DIRTY_FLAG): 1 = new published value available, 0 = clean/consumed.
    state: AtomicU8,
}

// SAFETY: `TripleBufferShared<T>` grants mutually exclusive slot ownership via atomic state
// transitions so no two threads ever concurrently access the same `UnsafeCell<T>` slot.
unsafe impl<T: Send> Send for TripleBufferShared<T> {}
// SAFETY: Shared references only touch `state: AtomicU8` or disjoint slots owned exclusively
// by the single producer or single consumer, with `AcqRel`/`Acquire` synchronization.
unsafe impl<T: Send> Sync for TripleBufferShared<T> {}

/// Producer handle for the lock-free triple buffer (typically owned by the GUI thread).
pub struct TripleBufferProducer<T> {
    shared: Arc<TripleBufferShared<T>>,
    producer_idx: usize,
}

// SAFETY: `TripleBufferProducer<T>` is the sole writer to its exclusive `producer_idx` slot
// and transfers `T: Send` values to the consumer via atomic release semantics.
unsafe impl<T: Send> Send for TripleBufferProducer<T> {}

/// Consumer handle for the lock-free triple buffer (typically owned by the audio callback thread).
pub struct TripleBufferConsumer<T> {
    shared: Arc<TripleBufferShared<T>>,
    consumer_idx: usize,
}

// SAFETY: `TripleBufferConsumer<T>` is the sole reader of its exclusive `consumer_idx` slot
// and acquires published `T: Send` values via atomic acquire semantics.
unsafe impl<T: Send> Send for TripleBufferConsumer<T> {}

/// Creates a new single-producer single-consumer lock-free triple buffer initialized with `initial`.
pub fn create_triple_buffer<T: Copy>(
    initial: T,
) -> (TripleBufferProducer<T>, TripleBufferConsumer<T>) {
    let shared = Arc::new(TripleBufferShared {
        slots: [
            UnsafeCell::new(initial),
            UnsafeCell::new(initial),
            UnsafeCell::new(initial),
        ],
        // Slot 2 is initially the shared slot, not dirty
        state: AtomicU8::new(2),
    });

    let producer = TripleBufferProducer {
        shared: Arc::clone(&shared),
        producer_idx: 0,
    };

    let consumer = TripleBufferConsumer {
        shared,
        consumer_idx: 1,
    };

    (producer, consumer)
}

impl<T: Copy> TripleBufferProducer<T> {
    /// Publish a new value to the triple buffer.
    ///
    /// This operation is wait-free $O(1)$, performs zero allocations, and never blocks.
    #[inline]
    pub fn publish(&mut self, value: T) {
        // 1. Write value into exclusive producer slot
        // SAFETY: `self.producer_idx` is in `0..3` and exclusively owned by this single producer
        // until the subsequent `AcqRel` swap publishes it.
        unsafe {
            *self.shared.slots[self.producer_idx].get() = value;
        }

        // 2. Atomically swap producer slot into shared state, setting DIRTY_FLAG
        let new_state = (self.producer_idx as u8) | DIRTY_FLAG;
        let old_state = self.shared.state.swap(new_state, Ordering::AcqRel);

        // 3. The previous shared slot becomes our new exclusive producer slot
        self.producer_idx = (old_state & INDEX_MASK) as usize;
    }
}

impl<T: Copy> TripleBufferConsumer<T> {
    /// Checks for a new published value and swaps it into the local consumer slot if available.
    ///
    /// Returns `Some(value)` if a new value was acquired, or `None` if no new value was published.
    ///
    /// This operation is lock-free, performs zero allocations, zero deallocations, and is strictly RT-safe.
    #[inline]
    pub fn read_latest_copy(&mut self) -> Option<T> {
        let mut current_state = self.shared.state.load(Ordering::Acquire);
        loop {
            if (current_state & DIRTY_FLAG) == 0 {
                // No unread published value available
                return None;
            }

            let shared_idx = (current_state & INDEX_MASK) as usize;
            let next_state = self.consumer_idx as u8; // DIRTY_FLAG cleared to 0

            match self.shared.state.compare_exchange_weak(
                current_state,
                next_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.consumer_idx = shared_idx;
                    // SAFETY: The `AcqRel` CAS transferred exclusive ownership of `shared_idx`
                    // (in `0..3`) to this single consumer and synchronized with the producer's write.
                    let val = unsafe { *self.shared.slots[self.consumer_idx].get() };
                    return Some(val);
                }
                Err(actual) => {
                    current_state = actual;
                }
            }
        }
    }

    /// Read the current value residing in the consumer slot without polling for updates.
    #[inline]
    pub fn current_copy(&self) -> T {
        // SAFETY: `self.consumer_idx` (in `0..3`) is exclusively owned by this consumer
        // and never written by the producer while held here.
        unsafe { *self.shared.slots[self.consumer_idx].get() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triple_buffer_initial_value() {
        let (_producer, consumer) = create_triple_buffer(42);
        assert_eq!(consumer.current_copy(), 42);
    }

    #[test]
    fn test_triple_buffer_single_publish_and_read() {
        let (mut producer, mut consumer) = create_triple_buffer(100);
        assert_eq!(consumer.read_latest_copy(), None);

        producer.publish(200);
        assert_eq!(consumer.read_latest_copy(), Some(200));
        assert_eq!(consumer.current_copy(), 200);

        // Next read without new publish returns None, keeping current value
        assert_eq!(consumer.read_latest_copy(), None);
        assert_eq!(consumer.current_copy(), 200);
    }

    #[test]
    fn test_triple_buffer_multiple_publishes_keeps_latest() {
        let (mut producer, mut consumer) = create_triple_buffer(0);

        producer.publish(1);
        producer.publish(2);
        producer.publish(3);

        assert_eq!(consumer.read_latest_copy(), Some(3));
        assert_eq!(consumer.read_latest_copy(), None);
        assert_eq!(consumer.current_copy(), 3);
    }

    #[test]
    fn test_triple_buffer_threaded_stress() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let (mut producer, mut consumer) = create_triple_buffer(0u64);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_reader = Arc::clone(&stop_flag);

        let writer_thread = std::thread::spawn(move || {
            for i in 1..=100_000 {
                producer.publish(i);
            }
            stop_flag.store(true, Ordering::Release);
        });

        let reader_thread = std::thread::spawn(move || {
            let mut last_seen = 0;
            while !stop_reader.load(Ordering::Acquire) {
                if let Some(val) = consumer.read_latest_copy() {
                    assert!(
                        val >= last_seen,
                        "Monotonic ordering violation: val={val} < last_seen={last_seen}"
                    );
                    last_seen = val;
                }
            }
            // Drain final value
            if let Some(val) = consumer.read_latest_copy() {
                assert!(val >= last_seen);
            }
        });

        writer_thread.join().unwrap();
        reader_thread.join().unwrap();
    }
}
