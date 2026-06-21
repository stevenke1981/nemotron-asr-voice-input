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
        }
    }

    /// Check if all required model files exist.
    fn check_model_files(model_dir: &Path) -> Result<(), AsrError> {
        let required = [
            "encoder.onnx",
            "decoder.onnx",
            "joint.onnx",
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
                    encoder: Some(format!("{}/encoder.onnx", model_dir_str)),
                    decoder: Some(format!("{}/decoder.onnx", model_dir_str)),
                    joiner: Some(format!("{}/joint.onnx", model_dir_str)),
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
            if let Some(ref stream) = self.stream {
                stream.set_option("use_vad", "true");
                info!("VAD enabled");
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

        // Decode
        recognizer.decode(stream);

        // Get result
        let result = recognizer.get_result(stream);

        match result {
            Some(r) => {
                let text = r.text.clone();
                let is_final = r.is_final;

                // If endpoint detected, reset for next utterance
                if is_final && !text.is_empty() {
                    recognizer.reset(stream);
                }

                debug!("Transcript: '{}' (final: {})", text, is_final);

                Ok(TranscriptResult {
                    text,
                    is_final,
                    segment_id: r.segment.unwrap_or(0) as u32,
                    confidence: 0.0, // sherpa-onnx doesn't provide confidence
                })
            }
            None => Ok(TranscriptResult::empty()),
        }
    }

    fn reset(&mut self) -> Result<(), AsrError> {
        if !self.initialized {
            return Err(AsrError::NotInitialized);
        }

        if let (Some(recognizer), Some(stream)) = (&self.recognizer, &self.stream) {
            recognizer.reset(stream);
            info!("ASR engine reset");
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
}

impl Default for SherpaAsrEngine {
    fn default() -> Self {
        Self::new()
    }
}
