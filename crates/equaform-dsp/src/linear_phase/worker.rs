//! Lock-free asynchronous FIR kernel synthesis worker thread.
//!
//! Offloads linear-phase frequency-sampling and FFT partition synthesis from
//! the real-time audio thread onto a dedicated background thread using
//! lock-free triple buffers and lost-wakeup-free `park`/`unpark` signaling.

use super::synthesis::{KernelSynthesizer, PartitionBank};
use equaform_common::{
    EqParameters, ProcessingMode, TripleBufferConsumer, TripleBufferProducer, create_triple_buffer,
    create_triple_buffer_with,
};
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle, Thread};

/// Snapshot of EQ parameters and mode requested by the audio thread.
#[derive(Clone, Copy)]
pub(super) struct KernelRequest {
    pub params: EqParameters,
    pub sample_rate: f32,
    pub mode: ProcessingMode,
    pub generation: u64,
}

/// Pre-allocated synthesized UPOLS partition bank tagged with source mode and generation.
pub(super) struct SynthesizedBank {
    pub bank: PartitionBank,
    pub mode: ProcessingMode,
    pub generation: u64,
}

impl SynthesizedBank {
    /// Allocates a full-capacity staging bank for lock-free exchange with the worker thread.
    pub(super) fn new() -> Self {
        Self {
            bank: PartitionBank::new(),
            mode: ProcessingMode::LinearPhaseLow,
            generation: 0,
        }
    }
}

/// Dedicated background worker that synthesizes FIR partition banks without
/// blocking or allocating on the real-time audio thread.
pub(super) struct AsyncKernelWorker {
    req_tx: TripleBufferProducer<KernelRequest>,
    bank_rx: TripleBufferConsumer<SynthesizedBank>,
    staging: SynthesizedBank,
    next_generation: u64,
    min_accepted_generation: u64,
    shutdown: Arc<AtomicBool>,
    wake_pending: Arc<AtomicBool>,
    worker_thread: Option<Thread>,
    join_handle: Option<JoinHandle<()>>,
    #[cfg(test)]
    last_published_generation: Arc<AtomicU64>,
    #[cfg(test)]
    synthesis_count: Arc<AtomicU64>,
    #[cfg(test)]
    last_polled_generation: u64,
}

impl AsyncKernelWorker {
    /// Spawns the asynchronous FIR synthesis worker thread.
    ///
    /// If thread creation fails, [`AsyncKernelWorker::is_available`] returns `false`
    /// so callers can transparently fall back to synchronous synthesis.
    pub(super) fn new() -> Self {
        let (req_tx, mut req_rx) = create_triple_buffer(KernelRequest {
            params: EqParameters::default(),
            sample_rate: 48_000.0,
            mode: ProcessingMode::LinearPhaseLow,
            generation: 0,
        });
        let (mut bank_tx, bank_rx) = create_triple_buffer_with(SynthesizedBank::new);
        let shutdown = Arc::new(AtomicBool::new(false));
        let wake_pending = Arc::new(AtomicBool::new(false));

        #[cfg(test)]
        let last_published_generation = Arc::new(AtomicU64::new(0));
        #[cfg(test)]
        let synthesis_count = Arc::new(AtomicU64::new(0));

        let worker_shutdown = Arc::clone(&shutdown);
        let worker_wake = Arc::clone(&wake_pending);
        #[cfg(test)]
        let worker_last_gen = Arc::clone(&last_published_generation);
        #[cfg(test)]
        let worker_synth_count = Arc::clone(&synthesis_count);

        let spawn_result = thread::Builder::new()
            .name("equaform-fir-synth".to_string())
            .stack_size(256 * 1024)
            .spawn(move || {
                let mut synthesizer = KernelSynthesizer::new();
                loop {
                    if worker_shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    worker_wake.store(false, Ordering::SeqCst);

                    if let Some(req) = req_rx.read_latest_copy() {
                        bank_tx.publish_with(|slot| {
                            synthesizer.synthesize(
                                &req.params,
                                req.sample_rate,
                                req.mode,
                                &mut slot.bank,
                            );
                            slot.mode = req.mode;
                            slot.generation = req.generation;
                        });
                        #[cfg(test)]
                        {
                            worker_synth_count.fetch_add(1, Ordering::Release);
                            worker_last_gen.store(req.generation, Ordering::Release);
                        }
                        continue;
                    }

                    if !worker_shutdown.load(Ordering::SeqCst)
                        && !worker_wake.load(Ordering::SeqCst)
                    {
                        thread::park();
                    }
                }
            });

        let (worker_thread, join_handle) = match spawn_result {
            Ok(handle) => (Some(handle.thread().clone()), Some(handle)),
            Err(_) => (None, None),
        };

        Self {
            req_tx,
            bank_rx,
            staging: SynthesizedBank::new(),
            next_generation: 0,
            min_accepted_generation: 1,
            shutdown,
            wake_pending,
            worker_thread,
            join_handle,
            #[cfg(test)]
            last_published_generation,
            #[cfg(test)]
            synthesis_count,
            #[cfg(test)]
            last_polled_generation: 0,
        }
    }

    /// Returns `true` if the background worker thread was spawned successfully.
    #[inline]
    pub(super) fn is_available(&self) -> bool {
        self.worker_thread.is_some()
    }

    /// Advances the accepted generation threshold so any older in-flight or
    /// unread background synthesis result is discarded.
    #[inline]
    pub(super) fn invalidate_in_flight(&mut self) {
        self.next_generation = self.next_generation.wrapping_add(1);
        self.min_accepted_generation = self.next_generation;
    }

    /// Publishes an asynchronous kernel synthesis request and wakes the worker
    /// thread without blocking or allocating on the audio thread.
    pub(super) fn request_update(
        &mut self,
        params: &EqParameters,
        sample_rate: f32,
        mode: ProcessingMode,
    ) {
        self.next_generation = self.next_generation.wrapping_add(1);
        let req = KernelRequest {
            params: *params,
            sample_rate,
            mode,
            generation: self.next_generation,
        };
        self.req_tx.publish(req);
        self.wake_pending.store(true, Ordering::SeqCst);
        if let Some(thread) = &self.worker_thread {
            thread.unpark();
        }
    }

    /// Polls for a completed bank matching `expected_mode` and at least
    /// `min_accepted_generation`, swapping it into `dst` with zero allocations.
    pub(super) fn poll_ready(
        &mut self,
        dst: &mut PartitionBank,
        expected_mode: ProcessingMode,
    ) -> bool {
        if !self.bank_rx.swap_latest(&mut self.staging) {
            return false;
        }
        if self.staging.mode == expected_mode
            && self.staging.generation >= self.min_accepted_generation
        {
            std::mem::swap(&mut self.staging.bank, dst);
            #[cfg(test)]
            {
                self.last_polled_generation = self.staging.generation;
            }
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub(super) fn next_generation(&self) -> u64 {
        self.next_generation
    }

    #[cfg(test)]
    pub(super) fn last_published_generation(&self) -> u64 {
        self.last_published_generation.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(super) fn last_polled_generation(&self) -> u64 {
        self.last_polled_generation
    }

    #[cfg(test)]
    pub(super) fn synthesis_count(&self) -> u64 {
        self.synthesis_count.load(Ordering::Acquire)
    }
}

impl Drop for AsyncKernelWorker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(thread) = self.worker_thread.take() {
            thread.unpark();
        }
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}
