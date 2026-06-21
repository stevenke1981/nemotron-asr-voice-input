use anyhow::Result;

/// Simple audio resampler.
/// For MVP, this handles basic cases. If no resampling is needed it's a no-op.
pub struct AudioResampler {
    from_rate: u32,
    to_rate: u32,
    channels: u16,
}

impl AudioResampler {
    pub fn new(from_rate: u32, to_rate: u32, channels: u16) -> Self {
        Self {
            from_rate,
            to_rate,
            channels,
        }
    }

    /// Check if resampling is needed.
    pub fn needs_resampling(&self) -> bool {
        self.from_rate != self.to_rate
    }

    /// Check if channel mixing is needed.
    pub fn needs_mixing(&self) -> bool {
        self.channels > 1
    }

    /// Convert stereo/multi-channel to mono by averaging.
    pub fn mix_to_mono(input: &[f32], channels: u16) -> Vec<f32> {
        if channels <= 1 {
            return input.to_vec();
        }
        let frames = input.len() / channels as usize;
        let mut mono = Vec::with_capacity(frames);
        for frame in 0..frames {
            let sum: f32 = (0..channels as usize)
                .map(|ch| input[frame * channels as usize + ch])
                .sum();
            mono.push(sum / channels as f32);
        }
        mono
    }

    /// Simple linear interpolation resampling.
    pub fn resample(&self, input: &[f32]) -> Vec<f32> {
        if !self.needs_resampling() {
            return input.to_vec();
        }

        let ratio = self.to_rate as f64 / self.from_rate as f64;
        let output_len = (input.len() as f64 * ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_idx = i as f64 / ratio;
            let src_floor = src_idx.floor() as usize;
            let src_ceil = (src_idx.ceil() as usize).min(input.len() - 1);
            let frac = src_idx - src_idx.floor();

            if src_floor < input.len() {
                let sample = if src_floor == src_ceil {
                    input[src_floor]
                } else {
                    input[src_floor] * (1.0 - frac as f32) + input[src_ceil] * frac as f32
                };
                output.push(sample);
            }
        }

        output
    }

    /// Process audio: first mix to mono if needed, then resample.
    pub fn process(&self, input: &[f32]) -> Result<Vec<f32>> {
        let mono = if self.needs_mixing() {
            Self::mix_to_mono(input, self.channels)
        } else {
            input.to_vec()
        };

        Ok(self.resample(&mono))
    }
}
