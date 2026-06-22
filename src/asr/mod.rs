pub mod config;
pub mod sherpa;

pub use config::AsrConfig;
pub use sherpa::SherpaAsrEngine;

use thiserror::Error;

/// ASR engine unified interface.
pub trait AsrEngine: Send {
    /// Initialize the engine and load models.
    fn initialize(&mut self, config: &AsrConfig) -> Result<(), AsrError>;

    /// Feed audio data (16kHz, mono, f32 PCM).
    fn feed_audio(&mut self, samples: &[f32]) -> Result<(), AsrError>;

    /// Get the current transcript result.
    fn get_transcript(&mut self) -> Result<TranscriptResult, AsrError>;

    /// Check if the engine has enough audio accumulated to decode another step.
    fn is_ready(&self) -> bool;

    /// Signal that no more audio will be fed (flushes internal buffers).
    fn input_finished(&mut self) -> Result<(), AsrError>;

    /// Finalize decoding and get the complete result.
    /// Must be called after input_finished(). Unlike get_transcript(),
    /// this does NOT check is_ready() first, so it captures the final
    /// hypothesis that may only be available after all decode steps.
    fn decode_final(&mut self) -> Result<TranscriptResult, AsrError>;

    /// Reset the engine state.
    fn reset(&mut self) -> Result<(), AsrError>;

    /// Set the recognition language.
    fn set_language(&mut self, lang: &str) -> Result<(), AsrError>;

    /// Enable or disable VAD at runtime.
    fn set_vad(&mut self, enabled: bool) -> Result<(), AsrError>;

    /// Update VAD threshold at runtime (0.0–1.0, lower = more sensitive).
    fn set_vad_threshold(&mut self, threshold: f32) -> Result<(), AsrError>;
}

/// Result of ASR transcription.
#[derive(Debug, Clone)]
pub struct TranscriptResult {
    pub text: String,
    pub is_final: bool,
    #[allow(dead_code)]
    pub segment_id: u32,
    #[allow(dead_code)]
    pub confidence: f32,
}

impl TranscriptResult {
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            is_final: false,
            segment_id: 0,
            confidence: 0.0,
        }
    }
}

/// ASR engine errors.
#[derive(Error, Debug)]
pub enum AsrError {
    #[error("Model loading failed: {0}")]
    ModelLoadError(String),

    #[error("Audio feed error: {0}")]
    #[allow(dead_code)]
    AudioFeedError(String),

    #[error("Decode error: {0}")]
    #[allow(dead_code)]
    DecodeError(String),

    #[error("Engine not initialized")]
    NotInitialized,

    #[error("Language not supported: {0}")]
    #[allow(dead_code)]
    UnsupportedLanguage(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    #[allow(dead_code)]
    Other(String),
}

/// Create an ASR engine based on the configuration.
pub fn create_asr_engine(config: &AsrConfig) -> Result<Box<dyn AsrEngine>, AsrError> {
    let mut engine = SherpaAsrEngine::new();
    engine.initialize(config)?;
    Ok(Box::new(engine))
}

/// Decode one complete utterance using sherpa-onnx's canonical flush order.
///
/// A fresh stream is created first, all audio is fed, `input_finished()`
/// exposes the trailing context, and decoding continues until `is_ready()` is
/// false. The stream is replaced again before returning so callers cannot
/// accidentally reuse an input-finished stream.
pub fn decode_complete_utterance(
    engine: &mut dyn AsrEngine,
    samples: &[f32],
) -> Result<TranscriptResult, AsrError> {
    // Nemotron's encoder has a 650 ms receptive field (T=65). In practice,
    // input_finished() alone does not expose enough future frames for the last
    // tokens, so provide one full trailing-context window before flushing.
    const TRAILING_CONTEXT_SAMPLES: usize = 16_000 * 800 / 1_000;
    let trailing_context = vec![0.0; TRAILING_CONTEXT_SAMPLES];

    let decode_result = (|| {
        engine.reset()?;
        engine.feed_audio(samples)?;
        engine.feed_audio(&trailing_context)?;
        engine.input_finished()?;

        let mut latest = TranscriptResult::empty();
        while engine.is_ready() {
            let result = engine.get_transcript()?;
            if !result.text.is_empty() {
                latest = result;
            }
        }

        let final_result = engine.decode_final()?;
        if !final_result.text.is_empty() {
            latest = final_result;
        }
        latest.is_final = !latest.text.is_empty();
        Ok(latest)
    })();

    // Always replace the input-finished stream. Preserve the decode error if
    // both decoding and cleanup fail because it is the actionable root cause.
    let reset_result = engine.reset();
    match (decode_result, reset_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(result), Ok(())) => Ok(result),
    }
}

/// Language ID mapping for Nemotron model.
/// Reference: https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b
#[allow(dead_code)]
pub fn language_name_to_code(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "english" | "en" | "en_us" => Some("en"),
        "mandarin chinese" | "chinese" | "zh" | "zh_cn" => Some("zh"),
        "german" | "de" | "de_de" => Some("de"),
        "japanese" | "ja" | "ja_jp" => Some("ja"),
        "korean" | "ko" | "ko_kr" => Some("ko"),
        "french" | "fr" | "fr_fr" => Some("fr"),
        "spanish" | "es" | "es_es" => Some("es"),
        "italian" | "it" | "it_it" => Some("it"),
        "portuguese" | "pt" | "pt_br" => Some("pt"),
        "russian" | "ru" | "ru_ru" => Some("ru"),
        "arabic" | "ar" => Some("ar"),
        "vietnamese" | "vi" => Some("vi"),
        "thai" | "th" => Some("th"),
        "turkish" | "tr" => Some("tr"),
        "dutch" | "nl" => Some("nl"),
        "polish" | "pl" => Some("pl"),
        "swedish" | "sv" => Some("sv"),
        "danish" | "da" => Some("da"),
        "finnish" | "fi" => Some("fi"),
        "norwegian" | "no" => Some("no"),
        "greek" | "el" => Some("el"),
        "hebrew" | "he" => Some("he"),
        "indonesian" | "id" => Some("id"),
        "malay" | "ms" => Some("ms"),
        "romanian" | "ro" => Some("ro"),
        "czech" | "cs" => Some("cs"),
        "hungarian" | "hu" => Some("hu"),
        "ukrainian" | "uk" => Some("uk"),
        "croatian" | "hr" => Some("hr"),
        "slovak" | "sk" => Some("sk"),
        "slovenian" | "sl" => Some("sl"),
        "bulgarian" | "bg" => Some("bg"),
        "serbian" | "sr" => Some("sr"),
        "catalan" | "ca" => Some("ca"),
        "tagalog" | "tl" => Some("tl"),
        "hindi" | "hi" => Some("hi"),
        "bengali" | "bn" => Some("bn"),
        "tamil" | "ta" => Some("ta"),
        "telugu" | "te" => Some("te"),
        "marathi" | "mr" => Some("mr"),
        "urdu" | "ur" => Some("ur"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_complete_utterance, AsrEngine, AsrError, TranscriptResult};
    use std::cell::Cell;

    struct FakeEngine {
        calls: Vec<&'static str>,
        ready: Cell<usize>,
        decode_count: usize,
        feed_count: usize,
    }

    impl FakeEngine {
        fn result(text: &str) -> TranscriptResult {
            TranscriptResult {
                text: text.to_string(),
                is_final: false,
                segment_id: 0,
                confidence: 0.0,
            }
        }
    }

    impl AsrEngine for FakeEngine {
        fn initialize(&mut self, _: &super::AsrConfig) -> Result<(), AsrError> {
            Ok(())
        }

        fn feed_audio(&mut self, samples: &[f32]) -> Result<(), AsrError> {
            self.feed_count += 1;
            if self.feed_count == 1 {
                assert_eq!(samples, [0.25, -0.25]);
                self.calls.push("feed");
            } else {
                assert_eq!(samples.len(), 12_800);
                assert!(samples.iter().all(|sample| *sample == 0.0));
                self.calls.push("trailing-context");
            }
            Ok(())
        }

        fn get_transcript(&mut self) -> Result<TranscriptResult, AsrError> {
            self.calls.push("decode");
            self.ready.set(self.ready.get().saturating_sub(1));
            self.decode_count += 1;
            Ok(Self::result(if self.decode_count == 1 {
                "不完整"
            } else {
                "較完整"
            }))
        }

        fn is_ready(&self) -> bool {
            self.ready.get() > 0
        }

        fn input_finished(&mut self) -> Result<(), AsrError> {
            self.calls.push("finished");
            self.ready.set(2);
            Ok(())
        }

        fn decode_final(&mut self) -> Result<TranscriptResult, AsrError> {
            self.calls.push("final");
            Ok(Self::result("完整句"))
        }

        fn reset(&mut self) -> Result<(), AsrError> {
            self.calls.push("reset");
            Ok(())
        }

        fn set_language(&mut self, _: &str) -> Result<(), AsrError> {
            Ok(())
        }

        fn set_vad(&mut self, _: bool) -> Result<(), AsrError> {
            Ok(())
        }

        fn set_vad_threshold(&mut self, _: f32) -> Result<(), AsrError> {
            Ok(())
        }
    }

    #[test]
    fn complete_decode_flushes_before_reading_final_result() {
        let mut engine = FakeEngine {
            calls: Vec::new(),
            ready: Cell::new(0),
            decode_count: 0,
            feed_count: 0,
        };

        let result = decode_complete_utterance(&mut engine, &[0.25, -0.25]).unwrap();

        assert_eq!(result.text, "完整句");
        assert!(result.is_final);
        assert_eq!(
            engine.calls,
            [
                "reset",
                "feed",
                "trailing-context",
                "finished",
                "decode",
                "decode",
                "final",
                "reset"
            ]
        );
    }
}
