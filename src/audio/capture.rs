use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn};

use super::ringbuf::AudioRingBuffer;

/// Audio capture from microphone using cpal (WASAPI on Windows).
pub struct AudioCapture {
    _device: Device,
    stream: Option<Stream>,
    ringbuf: Arc<AudioRingBuffer>,
    is_capturing: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
}

impl AudioCapture {
    /// Create a new audio capture instance.
    pub fn new(
        device_name: &str,
        target_sample_rate: u32,
        target_channels: u16,
        ringbuf_capacity: usize,
    ) -> Result<Self> {
        let host = cpal::default_host();
        let device = if device_name.is_empty() {
            host.default_input_device()
                .context("No default input device found")?
        } else {
            host.input_devices()?
                .find(|d| {
                    d.description()
                        .map(|desc| desc.name().contains(device_name))
                        .unwrap_or(false)
                })
                .context(format!("Device '{}' not found", device_name))?
        };

        let desc = device.description()?;
        let device_name_str = desc.name().to_string();
        info!("Using audio input device: {}", device_name_str);

        let default_config = device.default_input_config()?;
        info!(
            "Default input config: {} Hz, {} channels, {:?}",
            default_config.sample_rate(),
            default_config.channels(),
            default_config.sample_format()
        );

        let _config = StreamConfig {
            channels: target_channels,
            sample_rate: target_sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        Ok(Self {
            _device: device,
            stream: None,
            ringbuf: Arc::new(AudioRingBuffer::new(ringbuf_capacity)),
            is_capturing: Arc::new(AtomicBool::new(false)),
            sample_rate: target_sample_rate,
            channels: target_channels,
        })
    }

    /// Start capturing audio.
    pub fn start(&mut self) -> Result<()> {
        if self.is_capturing.load(Ordering::SeqCst) {
            warn!("Audio capture already started");
            return Ok(());
        }

        let ringbuf = self.ringbuf.clone();
        let is_capturing = self.is_capturing.clone();
        let channels = self.channels;

        let err_fn = move |err| {
            error!("Audio stream error: {}", err);
        };

        let config = StreamConfig {
            channels: self.channels,
            sample_rate: self.sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = self._device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if !is_capturing.load(Ordering::SeqCst) {
                    return;
                }

                // If the device provides multi-channel, mix to mono
                if channels > 1 {
                    let frame_count = data.len() / channels as usize;
                    for frame in 0..frame_count {
                        let sum: f32 = (0..channels as usize)
                            .map(|ch| data[frame * channels as usize + ch])
                            .sum();
                        let mono_sample = sum / channels as f32;
                        let _ = ringbuf.push(mono_sample);
                    }
                } else {
                    let _ = ringbuf.push_slice(data);
                }
            },
            err_fn,
            None,
        )?;

        stream.play().context("Failed to start audio stream")?;
        self.stream = Some(stream);
        self.is_capturing.store(true, Ordering::SeqCst);
        info!("Audio capture started");
        Ok(())
    }

    /// Stop capturing audio.
    pub fn stop(&mut self) -> Result<()> {
        if !self.is_capturing.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.is_capturing.store(false, Ordering::SeqCst);
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        info!("Audio capture stopped");
        Ok(())
    }

    /// Get a reference to the ring buffer.
    pub fn ringbuf(&self) -> &Arc<AudioRingBuffer> {
        &self.ringbuf
    }

    /// Check if currently capturing.
    #[allow(dead_code)]
    pub fn is_capturing(&self) -> bool {
        self.is_capturing.load(Ordering::SeqCst)
    }

    /// Get the audio sample rate.
    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// List available audio input devices.
    pub fn list_devices() -> Result<Vec<String>> {
        let host = cpal::default_host();
        let devices = host.input_devices()?;
        let names: Vec<String> = devices
            .filter_map(|d| d.description().ok().map(|desc| desc.name().to_string()))
            .collect();
        Ok(names)
    }

    /// Clear the ring buffer.
    pub fn clear_ringbuf(&self) {
        self.ringbuf.clear();
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
