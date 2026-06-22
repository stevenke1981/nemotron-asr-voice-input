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
    /// The actual sample rate the device is running at (may differ from target).
    pub(crate) capture_sample_rate: u32,
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
            capture_sample_rate: default_config.sample_rate(),
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

        // Attempt 1: target config (16000 Hz, 1 ch)
        let target_cfg = StreamConfig {
            channels: 1,
            sample_rate: self.sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let result = {
            let cap = is_capturing.clone();
            let buf = ringbuf.clone();
            self._device.build_input_stream(
                target_cfg,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !cap.load(Ordering::SeqCst) { return; }
                    let _ = buf.push_slice(data);
                },
                move |err| error!("Audio stream error: {}", err),
                None,
            )
        };

        let (stream, actual_rate, actual_channels) = match result {
            Ok(s) => {
                info!("Audio stream opened at {} Hz / 1 ch (target rate)", self.sample_rate);
                (s, self.sample_rate, 1u16)
            }
            Err(_) => {
                // Fall back to device default config (e.g. 48000 Hz, 2 ch)
                let default_cfg = self._device.default_input_config()
                    .context("No default input config available")?;
                let dev_rate: u32 = default_cfg.sample_rate();
                let dev_channels: u16 = default_cfg.channels();
                info!("Falling back to device default: {} Hz, {} ch", dev_rate, dev_channels);

                let cap2 = is_capturing.clone();
                let buf2 = ringbuf.clone();
                let ch = dev_channels;
                let fallback_cfg = StreamConfig {
                    channels: dev_channels,
                    sample_rate: dev_rate,
                    buffer_size: cpal::BufferSize::Default,
                };

                let s = self._device.build_input_stream(
                    fallback_cfg,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !cap2.load(Ordering::SeqCst) { return; }
                        if ch > 1 {
                            let frame_count = data.len() / ch as usize;
                            for frame in 0..frame_count {
                                let sum: f32 = (0..ch as usize)
                                    .map(|c| data[frame * ch as usize + c])
                                    .sum();
                                let _ = buf2.push(sum / ch as f32);
                            }
                        } else {
                            let _ = buf2.push_slice(data);
                        }
                    },
                    move |err| error!("Audio stream error: {}", err),
                    None,
                )?;

                (s, dev_rate, dev_channels)
            }
        };

        // Signal the callback before play() so the first chunk isn't discarded.
        self.is_capturing.store(true, Ordering::SeqCst);
        stream.play().context("Failed to start audio stream")?;
        self.stream = Some(stream);
        self.capture_sample_rate = actual_rate;
        self.channels = actual_channels;
        info!(
            "Audio capture started ({} Hz, {} ch, target {} Hz)",
            actual_rate, actual_channels, self.sample_rate
        );
        Ok(())
    }

    /// Stop capturing audio.
    pub fn stop(&mut self) -> Result<()> {
        if !self.is_capturing.load(Ordering::SeqCst) {
            return Ok(());
        }

        // CRITICAL: Drop the stream FIRST. This blocks until the current
        // cpal callback finishes. The callback still sees is_capturing=true
        // and pushes its data to the ring buffer. After drop() returns, no
        // more callbacks will fire.
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        // Only now mark as stopped — no more callbacks can run.
        self.is_capturing.store(false, Ordering::SeqCst);
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

    /// Get the target audio sample rate (what the ASR expects).
    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the actual capture sample rate (what the device provides).
    /// May differ from the target rate — use for resampling.
    pub fn capture_rate(&self) -> u32 {
        self.capture_sample_rate
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
