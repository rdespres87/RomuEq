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

/// Creates a new single-producer single-consumer lock-free triple buffer initialized
/// by invoking `init` once for each of the three pre-allocated slots.
pub fn create_triple_buffer_with<T>(
    mut init: impl FnMut() -> T,
) -> (TripleBufferProducer<T>, TripleBufferConsumer<T>) {
    let shared = Arc::new(TripleBufferShared {
        slots: [
            UnsafeCell::new(init()),
            UnsafeCell::new(init()),
            UnsafeCell::new(init()),
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

/// Creates a new single-producer single-consumer lock-free triple buffer initialized with `initial`.
pub fn create_triple_buffer<T: Copy>(
    initial: T,
) -> (TripleBufferProducer<T>, TripleBufferConsumer<T>) {
    create_triple_buffer_with(|| initial)
}

impl<T> TripleBufferProducer<T> {
    /// Mutates the exclusive producer slot in place via `write` and atomically publishes it.
    ///
    /// This operation is wait-free $O(1)$ and performs zero internal allocations.
    #[inline]
    pub fn publish_with<F: FnOnce(&mut T)>(&mut self, write: F) {
        // 1. Mutate exclusive producer slot in place.
        // SAFETY: `self.producer_idx` is in `0..3` and exclusively owned by this single producer
        // until the subsequent `AcqRel` swap publishes it.
        unsafe {
            write(&mut *self.shared.slots[self.producer_idx].get());
        }

        // 2. Atomically swap producer slot into shared state, setting DIRTY_FLAG.
        let new_state = (self.producer_idx as u8) | DIRTY_FLAG;
        let old_state = self.shared.state.swap(new_state, Ordering::AcqRel);

        // 3. The previous shared slot becomes our new exclusive producer slot.
        self.producer_idx = (old_state & INDEX_MASK) as usize;
    }
}

impl<T: Copy> TripleBufferProducer<T> {
    /// Publish a new value to the triple buffer.
    ///
    /// This operation is wait-free $O(1)$, performs zero allocations, and never blocks.
    #[inline]
    pub fn publish(&mut self, value: T) {
        self.publish_with(|slot| *slot = value);
    }
}

impl<T> TripleBufferConsumer<T> {
    /// Checks for a new published value and swaps it with `dst` if available.
    ///
    /// Returns `true` if a new value was acquired and swapped into `dst`, or `false`
    /// if no unread published value was available.
    ///
    /// This operation is lock-free, performs zero allocations, zero deallocations, and is strictly RT-safe.
    #[inline]
    pub fn swap_latest(&mut self, dst: &mut T) -> bool {
        let mut current_state = self.shared.state.load(Ordering::Acquire);
        loop {
            if (current_state & DIRTY_FLAG) == 0 {
                // No unread published value available
                return false;
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
                    // (in `0..3`) to this single consumer and synchronized with the producer's
                    // write. Both pointers are valid, properly aligned, and non-overlapping
                    // because `dst` is uniquely borrowed by the caller outside `TripleBufferShared`.
                    unsafe {
                        std::ptr::swap(self.shared.slots[self.consumer_idx].get(), dst);
                    }
                    return true;
                }
                Err(actual) => {
                    current_state = actual;
                }
            }
        }
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

    #[test]
    fn test_triple_buffer_with_non_copy_vec_preserves_capacity_and_coalesces() {
        let (mut producer, mut consumer) =
            create_triple_buffer_with(|| Vec::<u32>::with_capacity(16));
        let mut dst = Vec::<u32>::with_capacity(16);
        let mut seen_ptrs = vec![dst.as_ptr()];

        assert!(!consumer.swap_latest(&mut dst));

        // Multiple publishes before a single swap coalesce to the latest slice.
        producer.publish_with(|slot| {
            slot.clear();
            slot.push(1);
        });
        producer.publish_with(|slot| {
            slot.clear();
            slot.extend_from_slice(&[2, 3]);
        });
        producer.publish_with(|slot| {
            slot.clear();
            slot.extend_from_slice(&[10, 20, 30]);
        });

        assert!(consumer.swap_latest(&mut dst));
        assert_eq!(dst.as_slice(), &[10, 20, 30]);
        assert_eq!(dst.capacity(), 16);
        assert!(!consumer.swap_latest(&mut dst));
        if !seen_ptrs.contains(&dst.as_ptr()) {
            seen_ptrs.push(dst.as_ptr());
        }

        for round in 0..20_u32 {
            producer.publish_with(|slot| {
                slot.clear();
                for j in 0..8 {
                    slot.push(round * 10 + j);
                }
            });
            assert!(consumer.swap_latest(&mut dst));
            assert_eq!(dst.capacity(), 16);
            assert_eq!(dst.len(), 8);
            assert_eq!(dst[0], round * 10);
            if !seen_ptrs.contains(&dst.as_ptr()) {
                seen_ptrs.push(dst.as_ptr());
            }
        }

        // Only the 4 pre-allocated heap buffers (3 slots + dst) may ever be observed.
        assert!(
            seen_ptrs.len() <= 4,
            "unexpected heap reallocations detected: {} distinct buffer pointers",
            seen_ptrs.len()
        );
    }
}
