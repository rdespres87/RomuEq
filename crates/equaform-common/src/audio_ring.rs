use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

#[repr(align(64))]
struct CachePadded<T> {
    value: T,
}

impl<T> CachePadded<T> {
    const fn new(value: T) -> Self {
        CachePadded { value }
    }
}

/// Shared backing storage for the lock-free stereo audio ring buffer.
struct AudioRingShared {
    left: Box<[AtomicU32]>,
    right: Box<[AtomicU32]>,
    capacity: usize,
    mask: usize,
    write_idx: CachePadded<AtomicUsize>,
    read_idx: CachePadded<AtomicUsize>,
}

/// Producer handle for the lock-free stereo audio ring buffer.
///
/// Can be sent to the audio input callback thread.
pub struct AudioRingProducer {
    shared: Arc<AudioRingShared>,
}

/// Consumer handle for the lock-free stereo audio ring buffer.
///
/// Can be sent to the audio output callback thread.
pub struct AudioRingConsumer {
    shared: Arc<AudioRingShared>,
}

/// Create a single-producer single-consumer lock-free audio ring buffer.
///
/// # Panics
/// Panics if `capacity` is not a power of two or is 0.
pub fn create_audio_ring_buffer(capacity: usize) -> (AudioRingProducer, AudioRingConsumer) {
    assert!(capacity > 0, "Audio ring buffer capacity must be > 0");
    assert!(
        capacity.is_power_of_two(),
        "Audio ring buffer capacity must be a power of two"
    );

    let mut left = Vec::with_capacity(capacity);
    let mut right = Vec::with_capacity(capacity);
    for _ in 0..capacity {
        left.push(AtomicU32::new(0));
        right.push(AtomicU32::new(0));
    }

    let shared = Arc::new(AudioRingShared {
        left: left.into_boxed_slice(),
        right: right.into_boxed_slice(),
        capacity,
        mask: capacity - 1,
        write_idx: CachePadded::new(AtomicUsize::new(0)),
        read_idx: CachePadded::new(AtomicUsize::new(0)),
    });

    (
        AudioRingProducer {
            shared: Arc::clone(&shared),
        },
        AudioRingConsumer { shared },
    )
}

impl AudioRingProducer {
    /// Capacity of the ring buffer in frames.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.shared.capacity
    }

    /// Available space for writing in frames.
    #[inline]
    pub fn available_write(&self) -> usize {
        let write = self.shared.write_idx.value.load(Ordering::Relaxed);
        let read = self.shared.read_idx.value.load(Ordering::Acquire);
        let occupied = write.wrapping_sub(read);
        self.shared.capacity.saturating_sub(occupied)
    }

    /// Push a single stereo frame. Returns `true` if enqueued, `false` if full.
    #[inline]
    pub fn push(&self, left: f32, right: f32) -> bool {
        let write = self.shared.write_idx.value.load(Ordering::Relaxed);
        let read = self.shared.read_idx.value.load(Ordering::Acquire);
        if write.wrapping_sub(read) >= self.shared.capacity {
            return false;
        }

        let idx = write & self.shared.mask;
        self.shared.left[idx].store(left.to_bits(), Ordering::Relaxed);
        self.shared.right[idx].store(right.to_bits(), Ordering::Relaxed);
        self.shared
            .write_idx
            .value
            .store(write.wrapping_add(1), Ordering::Release);
        true
    }

    /// Push interleaved multi-channel or stereo frames.
    ///
    /// For mono (`channels == 1`), `left` is duplicated into `right`.
    /// For stereo or multi-channel (`channels >= 2`), channels 0 and 1 are captured.
    ///
    /// Returns the number of frames actually enqueued.
    pub fn push_interleaved(&self, data: &[f32], channels: usize) -> usize {
        if channels == 0 || data.is_empty() {
            return 0;
        }

        let write = self.shared.write_idx.value.load(Ordering::Relaxed);
        let read = self.shared.read_idx.value.load(Ordering::Acquire);
        let occupied = write.wrapping_sub(read);
        let available = self.shared.capacity.saturating_sub(occupied);

        let total_frames = data.len() / channels;
        let frames_to_write = total_frames.min(available);

        for i in 0..frames_to_write {
            let offset = i * channels;
            let l = data[offset];
            let r = if channels > 1 { data[offset + 1] } else { l };

            let idx = (write.wrapping_add(i)) & self.shared.mask;
            self.shared.left[idx].store(l.to_bits(), Ordering::Relaxed);
            self.shared.right[idx].store(r.to_bits(), Ordering::Relaxed);
        }

        self.shared
            .write_idx
            .value
            .store(write.wrapping_add(frames_to_write), Ordering::Release);
        frames_to_write
    }
}

impl AudioRingConsumer {
    /// Capacity of the ring buffer in frames.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.shared.capacity
    }

    /// Number of frames ready to be read.
    #[inline]
    pub fn available_read(&self) -> usize {
        let read = self.shared.read_idx.value.load(Ordering::Relaxed);
        let write = self.shared.write_idx.value.load(Ordering::Acquire);
        write.wrapping_sub(read)
    }

    /// Try to pop a single stereo frame. Returns `None` if empty.
    #[inline]
    pub fn pop(&self) -> Option<(f32, f32)> {
        let read = self.shared.read_idx.value.load(Ordering::Relaxed);
        let write = self.shared.write_idx.value.load(Ordering::Acquire);
        if write.wrapping_sub(read) == 0 {
            return None;
        }

        let idx = read & self.shared.mask;
        let l = f32::from_bits(self.shared.left[idx].load(Ordering::Relaxed));
        let r = f32::from_bits(self.shared.right[idx].load(Ordering::Relaxed));
        self.shared
            .read_idx
            .value
            .store(read.wrapping_add(1), Ordering::Release);
        Some((l, r))
    }

    /// Pop frames into planar left and right destination slices.
    ///
    /// Returns the number of frames actually popped.
    pub fn pop_slice(&self, left: &mut [f32], right: &mut [f32]) -> usize {
        let read = self.shared.read_idx.value.load(Ordering::Relaxed);
        let write = self.shared.write_idx.value.load(Ordering::Acquire);
        let available = write.wrapping_sub(read);

        let frames_to_read = left.len().min(right.len()).min(available);

        for i in 0..frames_to_read {
            let idx = (read.wrapping_add(i)) & self.shared.mask;
            left[i] = f32::from_bits(self.shared.left[idx].load(Ordering::Relaxed));
            right[i] = f32::from_bits(self.shared.right[idx].load(Ordering::Relaxed));
        }

        self.shared
            .read_idx
            .value
            .store(read.wrapping_add(frames_to_read), Ordering::Release);
        frames_to_read
    }

    /// Discard `count` pending frames from the buffer.
    ///
    /// Returns the number of frames actually discarded.
    pub fn discard(&self, count: usize) -> usize {
        let read = self.shared.read_idx.value.load(Ordering::Relaxed);
        let write = self.shared.write_idx.value.load(Ordering::Acquire);
        let available = write.wrapping_sub(read);
        let to_discard = count.min(available);
        self.shared
            .read_idx
            .value
            .store(read.wrapping_add(to_discard), Ordering::Release);
        to_discard
    }

    /// Discard all pending frames, aligning read position with current write position.
    pub fn clear(&self) {
        let write = self.shared.write_idx.value.load(Ordering::Acquire);
        self.shared.read_idx.value.store(write, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_basic_push_pop() {
        let (producer, consumer) = create_audio_ring_buffer(8);
        assert_eq!(producer.capacity(), 8);
        assert_eq!(consumer.available_read(), 0);
        assert_eq!(producer.available_write(), 8);

        assert!(producer.push(0.5, -0.5));
        assert!(producer.push(0.25, -0.25));
        assert_eq!(consumer.available_read(), 2);
        assert_eq!(producer.available_write(), 6);

        let f1 = consumer.pop().unwrap();
        assert_eq!(f1, (0.5, -0.5));
        let f2 = consumer.pop().unwrap();
        assert_eq!(f2, (0.25, -0.25));
        assert_eq!(consumer.pop(), None);
    }

    #[test]
    fn test_ring_interleaved_push_and_slice_pop() {
        let (producer, consumer) = create_audio_ring_buffer(16);
        let interleaved = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 3 stereo frames
        let written = producer.push_interleaved(&interleaved, 2);
        assert_eq!(written, 3);
        assert_eq!(consumer.available_read(), 3);

        let mut left = [0.0; 4];
        let mut right = [0.0; 4];
        let read = consumer.pop_slice(&mut left, &mut right);
        assert_eq!(read, 3);
        assert_eq!(&left[..3], &[1.0, 3.0, 5.0]);
        assert_eq!(&right[..3], &[2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_ring_mono_interleaved_duplication() {
        let (producer, consumer) = create_audio_ring_buffer(8);
        let mono = [0.1, 0.2, 0.3];
        let written = producer.push_interleaved(&mono, 1);
        assert_eq!(written, 3);

        let mut left = [0.0; 3];
        let mut right = [0.0; 3];
        let read = consumer.pop_slice(&mut left, &mut right);
        assert_eq!(read, 3);
        assert_eq!(left, [0.1, 0.2, 0.3]);
        assert_eq!(right, [0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_ring_clear() {
        let (producer, consumer) = create_audio_ring_buffer(8);
        producer.push(1.0, 1.0);
        producer.push(2.0, 2.0);
        assert_eq!(consumer.available_read(), 2);
        consumer.clear();
        assert_eq!(consumer.available_read(), 0);
        assert_eq!(producer.available_write(), 8);
    }

    #[test]
    fn test_ring_wrap_around() {
        let (producer, consumer) = create_audio_ring_buffer(4);
        for i in 0..100 {
            let val = i as f32;
            assert!(producer.push(val, -val));
            let popped = consumer.pop().unwrap();
            assert_eq!(popped, (val, -val));
        }
    }
}
