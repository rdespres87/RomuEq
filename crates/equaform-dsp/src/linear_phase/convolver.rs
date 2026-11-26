//! Uniformly Partitioned Overlap-Save (UPOLS) linear-phase stereo convolver.
//!
//! All memory is pre-allocated in [`LinearPhaseConvolver::new`] so kernel
//! updates, resets, and audio processing execute without heap allocations,
//! locks, or panics on real-time audio threads.

use super::synthesis::{
    KernelSynthesizer, PartitionBank, UPOLS_BLOCK_SIZE, UPOLS_FFT_SIZE, UPOLS_MAX_PARTITIONS,
};
use super::worker::AsyncKernelWorker;
use equaform_common::{EqParameters, ProcessingMode};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

/// Pre-allocated, non-panicking FIFO ring buffer using contiguous slice copies.
#[derive(Debug)]
struct AudioRingFifo {
    buf: Vec<f32>,
    read_pos: usize,
    write_pos: usize,
    len: usize,
}

impl AudioRingFifo {
    fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0.0; capacity.max(1)],
            read_pos: 0,
            write_pos: 0,
            len: 0,
        }
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.read_pos = 0;
        self.write_pos = 0;
        self.len = 0;
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }

    fn push_slice(&mut self, data: &[f32]) {
        let cap = self.buf.len();
        let count = data.len().min(cap.saturating_sub(self.len));
        if count == 0 {
            return;
        }
        let first = count.min(cap - self.write_pos);
        self.buf[self.write_pos..self.write_pos + first].copy_from_slice(&data[..first]);
        if count > first {
            self.buf[..count - first].copy_from_slice(&data[first..count]);
        }
        self.write_pos = (self.write_pos + count) % cap;
        self.len += count;
    }

    fn push_zeros(&mut self, count: usize) {
        let cap = self.buf.len();
        let n = count.min(cap.saturating_sub(self.len));
        if n == 0 {
            return;
        }
        let first = n.min(cap - self.write_pos);
        self.buf[self.write_pos..self.write_pos + first].fill(0.0);
        if n > first {
            self.buf[..n - first].fill(0.0);
        }
        self.write_pos = (self.write_pos + n) % cap;
        self.len += n;
    }

    fn pop_slice(&mut self, dst: &mut [f32]) {
        let cap = self.buf.len();
        let count = dst.len().min(self.len);
        if count > 0 {
            let first = count.min(cap - self.read_pos);
            dst[..first].copy_from_slice(&self.buf[self.read_pos..self.read_pos + first]);
            if count > first {
                dst[first..count].copy_from_slice(&self.buf[..count - first]);
            }
            self.read_pos = (self.read_pos + count) % cap;
            self.len -= count;
        }
        if dst.len() > count {
            dst[count..].fill(0.0);
        }
    }
}

/// Shared FFT plans and complex scratch buffers for UPOLS block processing.
struct ConvolutionBuffers {
    fft_forward: Arc<dyn Fft<f32>>,
    fft_inverse: Arc<dyn Fft<f32>>,
    scratch: Vec<Complex<f32>>,
    fft_time_buf: Vec<Complex<f32>>,
    acc_l: Vec<Complex<f32>>,
    acc_r: Vec<Complex<f32>>,
}

/// Core UPOLS engine state isolated from mono scratch so `process_mono` can
/// reuse the stereo path with zero extra allocations.
struct ConvolverEngine {
    synthesizer: KernelSynthesizer,
    banks: [PartitionBank; 2],
    active_bank: usize,
    crossfade_pending: bool,
    current_mode: ProcessingMode,
    bufs: ConvolutionBuffers,
    overlap_in: [[f32; UPOLS_BLOCK_SIZE]; 2],
    fdl: [Vec<Complex<f32>>; 2],
    fdl_head: usize,
    in_fifo: [AudioRingFifo; 2],
    out_fifo: [AudioRingFifo; 2],
    block_in_l: Vec<f32>,
    block_in_r: Vec<f32>,
    out_block_l: Vec<f32>,
    out_block_r: Vec<f32>,
    old_block_l: Vec<f32>,
    old_block_r: Vec<f32>,
}

impl ConvolverEngine {
    fn reset(&mut self) {
        let zero = Complex::new(0.0, 0.0);
        for ch in 0..2 {
            self.overlap_in[ch].fill(0.0);
            self.fdl[ch].fill(zero);
            self.in_fifo[ch].clear();
            self.out_fifo[ch].clear();
            self.out_fifo[ch].push_zeros(UPOLS_BLOCK_SIZE);
        }
        self.fdl_head = 0;

        if self.crossfade_pending {
            self.active_bank = 1 - self.active_bank;
            self.crossfade_pending = false;
        }
    }

    fn process_stereo_chunk(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.in_fifo[0].push_slice(left);
        self.in_fifo[1].push_slice(right);

        while self.in_fifo[0].len() >= UPOLS_BLOCK_SIZE && self.in_fifo[1].len() >= UPOLS_BLOCK_SIZE
        {
            self.process_upols_block();
        }

        self.out_fifo[0].pop_slice(left);
        self.out_fifo[1].pop_slice(right);
    }

    fn process_upols_block(&mut self) {
        self.in_fifo[0].pop_slice(&mut self.block_in_l);
        self.in_fifo[1].pop_slice(&mut self.block_in_r);

        let fdl_offset = self.fdl_head * UPOLS_FFT_SIZE;
        push_channel_fft(
            &mut self.bufs,
            &mut self.overlap_in[0],
            &self.block_in_l,
            &mut self.fdl[0][fdl_offset..fdl_offset + UPOLS_FFT_SIZE],
        );
        push_channel_fft(
            &mut self.bufs,
            &mut self.overlap_in[1],
            &self.block_in_r,
            &mut self.fdl[1][fdl_offset..fdl_offset + UPOLS_FFT_SIZE],
        );

        if self.crossfade_pending {
            let old_idx = self.active_bank;
            let new_idx = 1 - self.active_bank;

            convolve_bank(
                &self.banks[old_idx],
                &self.fdl,
                self.fdl_head,
                &mut self.bufs,
                &mut self.old_block_l,
                &mut self.old_block_r,
            );
            convolve_bank(
                &self.banks[new_idx],
                &self.fdl,
                self.fdl_head,
                &mut self.bufs,
                &mut self.out_block_l,
                &mut self.out_block_r,
            );

            crossfade_block(&self.old_block_l, &mut self.out_block_l);
            crossfade_block(&self.old_block_r, &mut self.out_block_r);

            self.active_bank = new_idx;
            self.crossfade_pending = false;
        } else {
            convolve_bank(
                &self.banks[self.active_bank],
                &self.fdl,
                self.fdl_head,
                &mut self.bufs,
                &mut self.out_block_l,
                &mut self.out_block_r,
            );
        }

        self.out_fifo[0].push_slice(&self.out_block_l);
        self.out_fifo[1].push_slice(&self.out_block_r);
        self.fdl_head = (self.fdl_head + 1) % UPOLS_MAX_PARTITIONS;
    }
}

fn push_channel_fft(
    bufs: &mut ConvolutionBuffers,
    overlap: &mut [f32; UPOLS_BLOCK_SIZE],
    block_in: &[f32],
    fdl_slot: &mut [Complex<f32>],
) {
    for (dst, &src) in bufs.fft_time_buf[..UPOLS_BLOCK_SIZE]
        .iter_mut()
        .zip(overlap.iter())
    {
        *dst = Complex::new(src, 0.0);
    }
    for (dst, &src) in bufs.fft_time_buf[UPOLS_BLOCK_SIZE..UPOLS_FFT_SIZE]
        .iter_mut()
        .zip(block_in.iter())
    {
        *dst = Complex::new(src, 0.0);
    }
    overlap.copy_from_slice(block_in);
    bufs.fft_forward
        .process_with_scratch(&mut bufs.fft_time_buf, &mut bufs.scratch);
    fdl_slot.copy_from_slice(&bufs.fft_time_buf);
}

fn crossfade_block(old_block: &[f32], new_block: &mut [f32]) {
    let inv_len = 1.0 / UPOLS_BLOCK_SIZE as f32;
    for (i, (out_s, &old_s)) in new_block.iter_mut().zip(old_block.iter()).enumerate() {
        let alpha = i as f32 * inv_len;
        *out_s = old_s * (1.0 - alpha) + *out_s * alpha;
    }
}

fn convolve_bank(
    bank: &PartitionBank,
    fdl: &[Vec<Complex<f32>>; 2],
    fdl_head: usize,
    bufs: &mut ConvolutionBuffers,
    out_l: &mut [f32],
    out_r: &mut [f32],
) {
    let zero = Complex::new(0.0, 0.0);
    bufs.acc_l.fill(zero);
    bufs.acc_r.fill(zero);

    for p in 0..bank.num_partitions {
        let slot = (fdl_head + UPOLS_MAX_PARTITIONS - p) % UPOLS_MAX_PARTITIONS;
        let fdl_off = slot * UPOLS_FFT_SIZE;
        let h_off = p * UPOLS_FFT_SIZE;

        let x_l = &fdl[0][fdl_off..fdl_off + UPOLS_FFT_SIZE];
        let x_r = &fdl[1][fdl_off..fdl_off + UPOLS_FFT_SIZE];
        let h_ll = &bank.ll[h_off..h_off + UPOLS_FFT_SIZE];
        let h_rr = &bank.rr[h_off..h_off + UPOLS_FFT_SIZE];

        if bank.has_cross_terms {
            let h_lr = &bank.lr[h_off..h_off + UPOLS_FFT_SIZE];
            let h_rl = &bank.rl[h_off..h_off + UPOLS_FFT_SIZE];

            for ((((al, ar), &xl), &xr), ((&ll, &rr), (&lr, &rl))) in bufs
                .acc_l
                .iter_mut()
                .zip(bufs.acc_r.iter_mut())
                .zip(x_l.iter())
                .zip(x_r.iter())
                .zip(
                    h_ll.iter()
                        .zip(h_rr.iter())
                        .zip(h_lr.iter().zip(h_rl.iter())),
                )
            {
                *al += xl * ll + xr * lr;
                *ar += xl * rl + xr * rr;
            }
        } else {
            for ((((al, ar), &xl), &xr), (&ll, &rr)) in bufs
                .acc_l
                .iter_mut()
                .zip(bufs.acc_r.iter_mut())
                .zip(x_l.iter())
                .zip(x_r.iter())
                .zip(h_ll.iter().zip(h_rr.iter()))
            {
                *al += xl * ll;
                *ar += xr * rr;
            }
        }
    }

    bufs.fft_inverse
        .process_with_scratch(&mut bufs.acc_l, &mut bufs.scratch);
    bufs.fft_inverse
        .process_with_scratch(&mut bufs.acc_r, &mut bufs.scratch);

    let inv_fft = 1.0 / UPOLS_FFT_SIZE as f32;
    for ((dst_l, src_l), (dst_r, src_r)) in out_l
        .iter_mut()
        .zip(bufs.acc_l[UPOLS_BLOCK_SIZE..UPOLS_FFT_SIZE].iter())
        .zip(
            out_r
                .iter_mut()
                .zip(bufs.acc_r[UPOLS_BLOCK_SIZE..UPOLS_FFT_SIZE].iter()),
        )
    {
        *dst_l = src_l.re * inv_fft;
        *dst_r = src_r.re * inv_fft;
    }
}

/// Real-time-safe dual-bank UPOLS linear-phase stereo convolver.
///
/// Supports arbitrary host block sizes (`>= 1`) while maintaining an invariant
/// algorithmic latency of `1024` samples in [`ProcessingMode::LinearPhaseLow`]
/// and `2048` samples in [`ProcessingMode::LinearPhaseHigh`].
pub struct LinearPhaseConvolver {
    engine: ConvolverEngine,
    worker: AsyncKernelWorker,
    max_chunk_size: usize,
    mono_scratch: Vec<f32>,
}

impl std::fmt::Debug for LinearPhaseConvolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinearPhaseConvolver")
            .field("current_mode", &self.engine.current_mode)
            .field("active_bank", &self.engine.active_bank)
            .field("crossfade_pending", &self.engine.crossfade_pending)
            .field("async_worker_available", &self.worker.is_available())
            .field("max_chunk_size", &self.max_chunk_size)
            .finish()
    }
}

impl LinearPhaseConvolver {
    /// Creates a new linear-phase convolver pre-allocated for `max_host_block_size`
    /// and initialized with a unity [`ProcessingMode::LinearPhaseLow`] kernel.
    pub fn new(max_host_block_size: usize) -> Self {
        let max_chunk_size = max_host_block_size.max(UPOLS_BLOCK_SIZE);
        let in_cap = max_chunk_size + UPOLS_BLOCK_SIZE;
        let out_cap = max_chunk_size + 2 * UPOLS_BLOCK_SIZE;

        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(UPOLS_FFT_SIZE);
        let fft_inverse = planner.plan_fft_inverse(UPOLS_FFT_SIZE);
        let scratch_len = fft_forward
            .get_inplace_scratch_len()
            .max(fft_inverse.get_inplace_scratch_len());

        let zero = Complex::new(0.0, 0.0);
        let mut synthesizer = KernelSynthesizer::new();
        let mut bank0 = PartitionBank::new();
        synthesizer.synthesize(
            &EqParameters::default(),
            48_000.0,
            ProcessingMode::LinearPhaseLow,
            &mut bank0,
        );
        let bank1 = PartitionBank::new();

        let mut out_fifo_l = AudioRingFifo::new(out_cap);
        let mut out_fifo_r = AudioRingFifo::new(out_cap);
        out_fifo_l.push_zeros(UPOLS_BLOCK_SIZE);
        out_fifo_r.push_zeros(UPOLS_BLOCK_SIZE);

        let fdl_len = UPOLS_MAX_PARTITIONS * UPOLS_FFT_SIZE;

        Self {
            engine: ConvolverEngine {
                synthesizer,
                banks: [bank0, bank1],
                active_bank: 0,
                crossfade_pending: false,
                current_mode: ProcessingMode::LinearPhaseLow,
                bufs: ConvolutionBuffers {
                    fft_forward,
                    fft_inverse,
                    scratch: vec![zero; scratch_len],
                    fft_time_buf: vec![zero; UPOLS_FFT_SIZE],
                    acc_l: vec![zero; UPOLS_FFT_SIZE],
                    acc_r: vec![zero; UPOLS_FFT_SIZE],
                },
                overlap_in: [[0.0; UPOLS_BLOCK_SIZE]; 2],
                fdl: [vec![zero; fdl_len], vec![zero; fdl_len]],
                fdl_head: 0,
                in_fifo: [AudioRingFifo::new(in_cap), AudioRingFifo::new(in_cap)],
                out_fifo: [out_fifo_l, out_fifo_r],
                block_in_l: vec![0.0; UPOLS_BLOCK_SIZE],
                block_in_r: vec![0.0; UPOLS_BLOCK_SIZE],
                out_block_l: vec![0.0; UPOLS_BLOCK_SIZE],
                out_block_r: vec![0.0; UPOLS_BLOCK_SIZE],
                old_block_l: vec![0.0; UPOLS_BLOCK_SIZE],
                old_block_r: vec![0.0; UPOLS_BLOCK_SIZE],
            },
            worker: AsyncKernelWorker::new(),
            max_chunk_size,
            mono_scratch: vec![0.0; max_chunk_size],
        }
    }

    /// Re-synthesizes the EQ filter kernel synchronously into the inactive bank.
    ///
    /// Invalidates any older in-flight asynchronous synthesis result. If `mode`
    /// differs from the active mode, swaps immediately and resets all delay lines;
    /// otherwise schedules a click-free linear crossfade over the next 256-sample
    /// UPOLS output block.
    pub fn update_kernel(&mut self, params: &EqParameters, sample_rate: f32, mode: ProcessingMode) {
        self.worker.invalidate_in_flight();

        let inactive = 1 - self.engine.active_bank;
        self.engine.synthesizer.synthesize(
            params,
            sample_rate,
            mode,
            &mut self.engine.banks[inactive],
        );

        if mode != self.engine.current_mode {
            self.engine.active_bank = inactive;
            self.engine.current_mode = mode;
            self.engine.crossfade_pending = false;
            self.engine.reset();
        } else {
            self.engine.crossfade_pending = true;
        }
    }

    /// Requests a steady-state EQ filter kernel update.
    ///
    /// When `mode` matches the active mode and the background worker is
    /// available, publishes the request wait-free with zero FFT work on the
    /// audio thread. Falls back to [`LinearPhaseConvolver::update_kernel`] on
    /// mode switches or if the worker thread could not be spawned.
    pub fn request_kernel_update(
        &mut self,
        params: &EqParameters,
        sample_rate: f32,
        mode: ProcessingMode,
    ) {
        if mode != self.engine.current_mode || !self.worker.is_available() {
            self.update_kernel(params, sample_rate, mode);
            return;
        }
        self.worker.request_update(params, sample_rate, mode);
    }

    /// Polls the background synthesis worker for a completed partition bank.
    ///
    /// Returns `true` if a matching bank was swapped into the inactive slot and
    /// queued for a click-free crossfade. Never overwrites a pending crossfade.
    pub fn poll_async_kernel(&mut self) -> bool {
        if self.engine.crossfade_pending || !self.engine.current_mode.is_linear_phase() {
            return false;
        }
        let inactive = 1 - self.engine.active_bank;
        if self
            .worker
            .poll_ready(&mut self.engine.banks[inactive], self.engine.current_mode)
        {
            self.engine.crossfade_pending = true;
            true
        } else {
            false
        }
    }

    /// Clears all overlap buffers, FDL history, and FIFOs, restoring the
    /// pre-filled output latency offset without allocating.
    pub fn reset(&mut self) {
        self.engine.reset();
    }

    /// Processes stereo planar audio buffers in place with zero heap allocations.
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let _ = self.poll_async_kernel();
        if self.engine.current_mode == ProcessingMode::MinimumPhase {
            return;
        }
        let total = left.len().min(right.len());
        if total == 0 {
            return;
        }

        for (l_chunk, r_chunk) in left[..total]
            .chunks_mut(self.max_chunk_size)
            .zip(right[..total].chunks_mut(self.max_chunk_size))
        {
            self.engine.process_stereo_chunk(l_chunk, r_chunk);
        }
    }

    /// Processes a mono audio buffer in place with zero heap allocations.
    pub fn process_mono(&mut self, samples: &mut [f32]) {
        let _ = self.poll_async_kernel();
        if self.engine.current_mode == ProcessingMode::MinimumPhase || samples.is_empty() {
            return;
        }

        for chunk in samples.chunks_mut(self.max_chunk_size) {
            let scratch = &mut self.mono_scratch[..chunk.len()];
            scratch.copy_from_slice(chunk);
            self.engine.process_stereo_chunk(chunk, scratch);
        }
    }

    /// Returns the total algorithmic latency in samples for the active mode.
    #[inline]
    pub fn latency_samples(&self) -> u32 {
        self.engine.current_mode.latency_samples()
    }

    /// Returns the active [`ProcessingMode`].
    #[inline]
    pub fn current_mode(&self) -> ProcessingMode {
        self.engine.current_mode
    }

    #[cfg(test)]
    pub(super) fn worker_next_generation(&self) -> u64 {
        self.worker.next_generation()
    }

    #[cfg(test)]
    pub(super) fn worker_last_published_generation(&self) -> u64 {
        self.worker.last_published_generation()
    }

    #[cfg(test)]
    pub(super) fn worker_last_polled_generation(&self) -> u64 {
        self.worker.last_polled_generation()
    }

    #[cfg(test)]
    pub(super) fn worker_synthesis_count(&self) -> u64 {
        self.worker.synthesis_count()
    }

    #[cfg(test)]
    pub(super) fn is_crossfade_pending(&self) -> bool {
        self.engine.crossfade_pending
    }
}
