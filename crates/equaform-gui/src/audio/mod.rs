//! Real-time audio backend using CPAL.
//!
//! This module provides a real-time, lock-free audio output stream and input capture
//! that processes audio through `equaform_engine::EngineLifecycle`.
//!
//! The audio callback performs zero allocations, zero mutex locks, zero syscalls,
//! and provides deterministic fallback behavior.

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos_virtual_rate_sync;
mod render;
mod resampler;
mod stream;

#[cfg(test)]
mod tests;

pub use render::{
    AudioRenderContext, AudioScratchBuffers, AudioTelemetry, StreamFormatSpec, soft_limit,
};
pub use resampler::ContinuousPiDriftController;
pub use stream::AudioStream;

/// Maximum number of scratch frames allocated per block (pre-allocated up-front, never resized).
pub const MAX_SCRATCH_FRAMES: usize = 8192;

/// Source of audio input for the equalizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSourceMode {
    /// Internal harmonic audio generator (ideal for auditioning EQ on any PC without cables).
    TestSignal = 0,
    /// Live input pass-through (captures from system loopback or microphone).
    PassThrough = 1,
}

impl From<u8> for AudioSourceMode {
    fn from(val: u8) -> Self {
        match val {
            0 => AudioSourceMode::TestSignal,
            _ => AudioSourceMode::PassThrough,
        }
    }
}
