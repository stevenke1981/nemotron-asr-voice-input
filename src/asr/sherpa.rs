use super::config::AsrConfig;
use super::{AsrEngine, AsrError, TranscriptResult};
use std::path::Path;
use tracing::{debug, info, warn};

/// Sherpa-onnx based ASR engine implementation.
pub struct SherpaAsrEngine {
    recognizer: Option<sherpa_onnx::OnlineRecognizer>,
    stream: Option<sherpa_onnx::OnlineStream>,
    language: String,
    initialized: bool,
    sample_rate: i32,
    /// Track VAD state so it can be re-applied when a new stream is created.
    vad_enabled: bool,
    /// VAD threshold (0.0–1.0). Lower = more sensitive to quiet speech.
    vad_threshold: f32,
}

impl SherpaAsrEngine {
    /// Create a new Sherpa ASR engine (not yet initialized).
    pub fn new() -> Self {
        Self {
            recognizer: None,
            stream: None,
            language: "zh".into(),
            initialized: false,
            sample_rate: 16000,
            vad_enabled: false,
            vad_threshold: 0.1,
        }
    }

    /// Check if all required model files exist.
    fn check_model_files(model_dir: &Path) -> Result<(), AsrError> {
        let required = [
            "encoder.int8.onnx",
            "decoder.int8.onnx",
            "joiner.int8.onnx",
            "tokens.txt",
        ];
        let optional = ["silero_vad.onnx"];

        for file in &required {
            let path = model_dir.join(file);
            if !path.exists() {
                return Err(AsrError::ModelLoadError(format!(
                    "Required model file not found: {}",
                    path.display()
                )));
            }
        }

        for file in &optional {
            let path = model_dir.join(file);
            if !path.exists() {
                warn!("Optional model file not found: {}", path.display());
            }
        }

        info!("All required model files present in {:?}", model_dir);
        Ok(())
    }
}

impl AsrEngine for SherpaAsrEngine {
    fn initialize(&mut self, config: &AsrConfig) -> Result<(), AsrError> {
        Self::check_model_files(&config.model_dir)?;

        let model_dir_str = config.model_dir.to_string_lossy().to_string();

        // Build the recognizer configuration for Nemotron model
        let recognizer_config = sherpa_onnx::OnlineRecognizerConfig {
            feat_config: sherpa_onnx_sys::online_asr::FeatureConfig {
                sample_rate: config.sample_rate as i32,
                feature_dim: 128,
            },
            model_config: sherpa_onnx::OnlineModelConfig {
                transducer: sherpa_onnx::OnlineTransducerModelConfig {
                    encoder: Some(format!("{}/encoder.int8.onnx", model_dir_str)),
                    decoder: Some(format!("{}/decoder.int8.onnx", model_dir_str)),
                    joiner: Some(format!("{}/joiner.int8.onnx", model_dir_str)),
                },
                tokens: Some(format!("{}/tokens.txt", model_dir_str)),
                num_threads: config.num_threads as i32,
                provider: Some(config.provider.clone()),
                ..Default::default()
            },
            decoding_method: Some(config.decoding_method.clone()),
            max_active_paths: config.max_active_paths,
            enable_endpoint: true,
            rule1_min_trailing_silence: 2.4,
            rule2_min_trailing_silence: 1.2,
            rule3_min_utterance_length: 30.0,
            ..Default::default()
        };

        let recognizer = sherpa_onnx::OnlineRecognizer::create(&recognizer_config)
            .ok_or_else(|| AsrError::ModelLoadError(
                "Failed to create recognizer - returned None (check model files)".into()
            ))?;

        let stream = recognizer.create_stream();
        self.recognizer = Some(recognizer);
        self.stream = Some(stream);
        self.language = config.language.clone();
        self.initialized = true;
        self.sample_rate = config.sample_rate as i32;

        info!("Sherpa ASR engine initialized successfully");

        // Set language if specified
        if config.language != "auto" {
            self.set_language(&config.language)?;
        }

        // Enable VAD if requested
        if config.use_vad {
            self.vad_enabled = true;
            self.vad_threshold = config.vad_threshold;
            if let Some(ref stream) = self.stream {
                stream.set_option("use_vad", "true");
                // Set VAD threshold (lower = more sensitive to quiet speech)
                let thresh_str = format!("{:.2}", self.vad_threshold);
                stream.set_option("silero_vad_threshold", &thresh_str);
                info!("VAD enabled (threshold: {})", thresh_str);
            }
        }

        Ok(())
    }

    fn feed_audio(&mut self, samples: &[f32]) -> Result<(), AsrError> {
        if !self.initialized {
            return Err(AsrError::NotInitialized);
        }

        if let Some(ref stream) = self.stream {
            stream.accept_waveform(self.sample_rate, samples);
            debug!("Fed {} audio samples to ASR engine", samples.len());
            Ok(())
        } else {
            Err(AsrError::NotInitialized)
        }
    }

    fn get_transcript(&mut self) -> Result<TranscriptResult, AsrError> {
        if !self.initialized {
            return Err(AsrError::NotInitialized);
        }

        let (recognizer, stream) = match (&self.recognizer, &self.stream) {
            (Some(r), Some(s)) => (r, s),
            _ => return Err(AsrError::NotInitialized),
        };

        // Only decode if enough audio has accumulated. sherpa-onnx's is_ready()
        // checks the internal feature buffer against the model's minimum frame
        // requirement (T_ = 65 for Nemotron). This is the official safe alternative
        // to our manual total_fed >= chunk_target guard, preventing the
        // "features.cc:GetFrames:188 0 + 65 > 30" assert crash.
        if !recognizer.is_ready(stream) {
            return Ok(TranscriptResult::empty());
        }

        // Decode one step
        recognizer.decode(stream);

        // Check endpoint detection — if triggered, mark result as final.
        // We do NOT call recognizer.reset() here. Utterance boundaries are
        // managed externally (via PTT press/release), and reset() creates a
        // completely new stream to avoid internal state corruption.
        let is_endpoint = recognizer.is_endpoint(stream);

        // Get result
        let result = recognizer.get_result(stream);

        match result {
            Some(r) => {
                let text = r.text.clone();
                let is_final = is_endpoint || r.is_final;

                debug!("Transcript: '{}' (final: {}, endpoint: {})", text, is_final, is_endpoint);

                Ok(TranscriptResult {
                    text,
                    is_final,
                    segment_id: r.segment.unwrap_or(0) as u32,
                    confidence: 0.0, // sherpa-onnx doesn't provide confidence
                })
            }
            None => {
                if is_endpoint {
                    // Endpoint detected but no result text — return empty final
                    Ok(TranscriptResult {
                        text: String::new(),
                        is_final: true,
                        segment_id: 0,
                        confidence: 0.0,
                    })
                } else {
                    Ok(TranscriptResult::empty())
                }
            }
        }
    }

    fn is_ready(&self) -> bool {
        match (&self.recognizer, &self.stream) {
            (Some(recognizer), Some(stream)) => recognizer.is_ready(stream),
            _ => false,
        }
    }

    fn decode_final(&mut self) -> Result<TranscriptResult, AsrError> {
        if !self.initialized {
            return Err(AsrError::NotInitialized);
        }
        let (recognizer, stream) = match (&self.recognizer, &self.stream) {
            (Some(r), Some(s)) => (r, s),
            _ => return Err(AsrError::NotInitialized),
        };

        // SAFE: Only get_result() without decode(). After input_finished() and
        // the main decode loop, calling decode() when !is_ready() can trigger
        // C++ asserts in sherpa-onnx. We just read the current hypothesis.
        let result = recognizer.get_result(stream);

        match result {
            Some(r) => {
                let text = r.text.clone();
                let is_final = r.is_final;
                debug!("Decode final: '{}' (final: {})", text, is_final);
                Ok(TranscriptResult {
                    text,
                    is_final,
                    segment_id: r.segment.unwrap_or(0) as u32,
                    confidence: 0.0,
                })
            }
            None => Ok(TranscriptResult::empty()),
        }
    }

    fn input_finished(&mut self) -> Result<(), AsrError> {
        if !self.initialized {
            return Err(AsrError::NotInitialized);
        }
        if let Some(ref stream) = self.stream {
            stream.input_finished();
            debug!("ASR input_finished signaled");
            Ok(())
        } else {
            Err(AsrError::NotInitialized)
        }
    }

    fn reset(&mut self) -> Result<(), AsrError> {
        if !self.initialized {
            return Err(AsrError::NotInitialized);
        }

        if let Some(ref recognizer) = self.recognizer {
            // Create a completely new stream instead of calling recognizer.reset().
            // The old stream may have stale internal state (partially decoded audio,
            // endpoint flags, frame buffer pointers) that can corrupt the next
            // utterance and cause "second utterance can't transcribe" failures.
            let new_stream = recognizer.create_stream();
            self.stream = Some(new_stream);

            // Re-apply runtime settings to the fresh stream
            if let Some(ref new_stream) = self.stream {
                if self.language != "auto" {
                    new_stream.set_option("language", &self.language);
                }
                if self.vad_enabled {
                    new_stream.set_option("use_vad", "true");
                    let thresh_str = format!("{:.2}", self.vad_threshold);
                    new_stream.set_option("silero_vad_threshold", &thresh_str);
                }
            }

            info!("ASR engine reset: new stream created");
            Ok(())
        } else {
            Err(AsrError::NotInitialized)
        }
    }

    fn set_language(&mut self, lang: &str) -> Result<(), AsrError> {
        if !self.initialized {
            return Err(AsrError::NotInitialized);
        }

        if let Some(ref stream) = self.stream {
            stream.set_option("language", lang);
            self.language = lang.to_string();
            info!("Language set to: {}", lang);
            Ok(())
        } else {
            Err(AsrError::NotInitialized)
        }
    }

    fn set_vad(&mut self, enabled: bool) -> Result<(), AsrError> {
        if !self.initialized {
            return Err(AsrError::NotInitialized);
        }

        self.vad_enabled = enabled;
        if let Some(ref stream) = self.stream {
            stream.set_option("use_vad", if enabled { "true" } else { "false" });
            info!("VAD runtime toggled: {}", if enabled { "enabled" } else { "disabled" });
            Ok(())
        } else {
            Err(AsrError::NotInitialized)
        }
    }

    fn set_vad_threshold(&mut self, threshold: f32) -> Result<(), AsrError> {
        if !self.initialized {
            return Err(AsrError::NotInitialized);
        }

        let clamped = threshold.clamp(0.0, 1.0);
        self.vad_threshold = clamped;
        let thresh_str = format!("{:.2}", clamped);
        if let Some(ref stream) = self.stream {
            stream.set_option("silero_vad_threshold", &thresh_str);
            info!("VAD threshold updated to: {:.2}", clamped);
        }
        Ok(())
    }
}

impl Default for SherpaAsrEngine {
    fn default() -> Self {
        Self::new()
    }
}
