use anyhow::{Context, Result};
use rubato::{FftFixedInOut, Resampler};

/// Stateful, band-limited mono resampler for the microphone-to-ASR path.
///
/// The previous linear interpolator became plain sample decimation for the
/// common 48 kHz -> 16 kHz ratio. Without an anti-alias filter, frequencies
/// above 8 kHz folded into Nemotron's speech band and caused dropped or
/// substituted tokens. `FftFixedInOut` applies the required low-pass filter
/// while preserving state across capture callbacks.
pub struct StreamingResampler {
    from_rate: u32,
    to_rate: u32,
    inner: Option<FftFixedInOut<f32>>,
    input_chunk: usize,
    pending: Vec<f32>,
    delay_remaining: usize,
    total_input: usize,
    total_output: usize,
}

impl StreamingResampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Result<Self> {
        anyhow::ensure!(from_rate > 0, "input sample rate must be greater than zero");
        anyhow::ensure!(to_rate > 0, "output sample rate must be greater than zero");

        if from_rate == to_rate {
            return Ok(Self {
                from_rate,
                to_rate,
                inner: None,
                input_chunk: 0,
                pending: Vec::new(),
                delay_remaining: 0,
                total_input: 0,
                total_output: 0,
            });
        }

        // A ~20 ms FFT block keeps live latency low while providing a proper
        // anti-alias transition band. Rubato rounds this to a valid rate ratio.
        let requested_chunk = (from_rate as usize / 50).max(64);
        let inner =
            FftFixedInOut::<f32>::new(from_rate as usize, to_rate as usize, requested_chunk, 1)
                .context("failed to create band-limited audio resampler")?;
        let input_chunk = inner.input_frames_next();
        let delay_remaining = inner.output_delay();

        Ok(Self {
            from_rate,
            to_rate,
            inner: Some(inner),
            input_chunk,
            pending: Vec::with_capacity(input_chunk * 2),
            delay_remaining,
            total_input: 0,
            total_output: 0,
        })
    }

    /// Resample all complete internal blocks and append them to `output`.
    pub fn process_into(&mut self, input: &[f32], output: &mut Vec<f32>) -> Result<()> {
        self.total_input += input.len();

        if self.inner.is_none() {
            output.extend_from_slice(input);
            self.total_output += input.len();
            return Ok(());
        }

        self.pending.extend_from_slice(input);
        let complete_frames = self.pending.len() / self.input_chunk * self.input_chunk;
        if complete_frames == 0 {
            return Ok(());
        }

        let mut emitted = Vec::new();
        {
            let inner = self.inner.as_mut().expect("resampler checked above");
            for start in (0..complete_frames).step_by(self.input_chunk) {
                let channel = &self.pending[start..start + self.input_chunk];
                let resampled = inner
                    .process(&[channel], None)
                    .context("band-limited audio resampling failed")?;
                emitted.extend_from_slice(&resampled[0]);
            }
        }
        self.pending.drain(..complete_frames);
        self.append_emitted(&emitted, output, None);
        Ok(())
    }

    /// Flush the final partial block and delayed filter samples.
    ///
    /// The emitted stream is trimmed to the exact input/output duration, so
    /// saving or decoding an utterance does not gain artificial audio length.
    pub fn flush_into(&mut self, output: &mut Vec<f32>) -> Result<()> {
        if self.inner.is_none() {
            return Ok(());
        }

        let expected_total =
            ((self.total_input as u128 * self.to_rate as u128) / self.from_rate as u128) as usize;
        let pending = std::mem::take(&mut self.pending);

        if !pending.is_empty() {
            let emitted = self
                .inner
                .as_mut()
                .expect("resampler checked above")
                .process_partial(Some(&[pending]), None)
                .context("failed to flush partial resampler input")?;
            self.append_emitted(&emitted[0], output, Some(expected_total));
        }

        // Push the FFT overlap tail out of the filter. Stop once the exact
        // duration has been emitted; the remaining samples are zero padding.
        if self.total_output < expected_total {
            let emitted = self
                .inner
                .as_mut()
                .expect("resampler checked above")
                .process_partial::<Vec<f32>>(None, None)
                .context("failed to flush resampler delay")?;
            self.append_emitted(&emitted[0], output, Some(expected_total));
        }

        Ok(())
    }

    /// Reset filter history between utterances.
    pub fn reset(&mut self) {
        if let Some(inner) = self.inner.as_mut() {
            inner.reset();
            self.delay_remaining = inner.output_delay();
        }
        self.pending.clear();
        self.total_input = 0;
        self.total_output = 0;
    }

    fn append_emitted(
        &mut self,
        emitted: &[f32],
        output: &mut Vec<f32>,
        max_total_output: Option<usize>,
    ) {
        let skip = self.delay_remaining.min(emitted.len());
        self.delay_remaining -= skip;
        let available = &emitted[skip..];
        let take = max_total_output
            .map(|max| max.saturating_sub(self.total_output).min(available.len()))
            .unwrap_or(available.len());
        output.extend_from_slice(&available[..take]);
        self.total_output += take;
    }
}

#[cfg(test)]
mod tests {
    use super::StreamingResampler;

    fn sine(rate: u32, hz: f32, seconds: f32) -> Vec<f32> {
        let len = (rate as f32 * seconds) as usize;
        (0..len)
            .map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / rate as f32).sin())
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn passthrough_preserves_samples() {
        let input = vec![0.1, -0.2, 0.3];
        let mut output = Vec::new();
        let mut resampler = StreamingResampler::new(16_000, 16_000).unwrap();
        resampler.process_into(&input, &mut output).unwrap();
        resampler.flush_into(&mut output).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn downsampling_preserves_duration_and_speech_band() {
        let input = sine(48_000, 1_000.0, 1.0);
        let mut output = Vec::new();
        let mut resampler = StreamingResampler::new(48_000, 16_000).unwrap();
        for chunk in input.chunks(7_777) {
            resampler.process_into(chunk, &mut output).unwrap();
        }
        resampler.flush_into(&mut output).unwrap();

        assert_eq!(output.len(), 16_000);
        assert!(
            rms(&output) > 0.6,
            "speech-band tone was attenuated too much"
        );
    }

    #[test]
    fn downsampling_rejects_alias_frequency() {
        // 12 kHz is above the 8 kHz Nyquist limit at the target rate. Direct
        // 3:1 decimation aliases it to 4 kHz at full volume; the fixed path
        // must strongly reject it.
        let input = sine(48_000, 12_000.0, 1.0);
        let mut output = Vec::new();
        let mut resampler = StreamingResampler::new(48_000, 16_000).unwrap();
        resampler.process_into(&input, &mut output).unwrap();
        resampler.flush_into(&mut output).unwrap();

        assert_eq!(output.len(), 16_000);
        assert!(
            rms(&output) < 0.02,
            "out-of-band tone aliased into ASR audio"
        );
    }
}
