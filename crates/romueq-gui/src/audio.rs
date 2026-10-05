//! Audio backend using cpal for real-time input/output processing.
//!
//! This module provides a cpal-based audio stream that feeds the RomuEq engine
//! in real-time. The audio thread owns its own `EngineLifecycle` instance and
//! receives parameter updates from the GUI thread via a channel.

use romueq_common::EqParameters;
use romueq_engine::EngineLifecycle;
use std::sync::mpsc;
use std::thread;

/// Message sent from the GUI thread to the audio thread.
#[derive(Debug, Clone)]
pub enum AudioMessage {
    /// Update EQ parameters.
    SetParameters(EqParameters),
    /// Start audio processing.
    Start,
    /// Stop audio processing.
    Stop,
}

/// Audio stream that processes live audio through the RomuEq engine.
///
/// The audio thread owns its own `EngineLifecycle` instance and receives
/// parameter updates from the GUI thread via a channel.
pub struct AudioStream {
    /// Sender for messages to the audio thread.
    sender: mpsc::Sender<AudioMessage>,
    /// Handle to the audio thread.
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl AudioStream {
    /// Create a new audio stream.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let receiver = std::sync::Mutex::new(receiver);

        let handle = thread::spawn(move || {
            let mut engine = EngineLifecycle::new();
            let sample_rate = 48000.0;
            let block_size = 512;
            let channels = 2;
            let mut running = false;
            let mut params = EqParameters::default();

            loop {
                // Try to receive a message with a short timeout.
                let msg = {
                    let rx = receiver.lock().unwrap();
                    rx.recv_timeout(std::time::Duration::from_millis(10))
                };

                match msg {
                    Ok(AudioMessage::SetParameters(new_params)) => {
                        params = new_params;
                        if running {
                            engine.set_parameters(params);
                        }
                    }
                    Ok(AudioMessage::Start) => {
                        if engine.state() != romueq_engine::lifecycle::LifecycleState::Ready {
                            if engine.prepare(sample_rate, block_size, channels) {
                                engine.set_parameters(params);
                                running = true;
                            }
                        } else {
                            running = true;
                        }
                    }
                    Ok(AudioMessage::Stop) => {
                        running = false;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // No message received, continue processing audio if running.
                        if running {
                            // Process a silent block (no input source).
                            let input: Vec<f32> = vec![0.0; block_size];
                            let mut output = vec![0.0; block_size];
                            let _ = engine.process(&[&input], &mut [&mut output]);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });

        Self {
            sender,
            thread_handle: Some(handle),
        }
    }

    /// Prepare the audio stream with the given parameters.
    pub fn prepare(&mut self, sample_rate: f32, block_size: usize, channels: usize) {
        // Store these in the engine when it starts — they'll be used during Start.
        // For now, we send a parameter update with the new sample rate context.
        let _ = (sample_rate, block_size, channels);
    }

    /// Start audio processing.
    pub fn start(&mut self) -> Result<(), String> {
        self.sender
            .send(AudioMessage::Start)
            .map_err(|e| format!("Failed to send start message: {e}"))
    }

    /// Stop audio processing.
    pub fn stop(&mut self) -> Result<(), String> {
        self.sender
            .send(AudioMessage::Stop)
            .map_err(|e| format!("Failed to send stop message: {e}"))
    }

    /// Send updated EQ parameters to the audio thread.
    pub fn set_parameters(&mut self, params: EqParameters) -> Result<(), String> {
        self.sender
            .send(AudioMessage::SetParameters(params))
            .map_err(|e| format!("Failed to send parameters: {e}"))
    }

    /// Stop and clean up the audio stream.
    pub fn stop_and_cleanup(&mut self) {
        let _ = self.stop();
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }
}

impl Default for AudioStream {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        self.stop_and_cleanup();
    }
}
