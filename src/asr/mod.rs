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

    /// Reset the engine state.
    fn reset(&mut self) -> Result<(), AsrError>;

    /// Set the recognition language.
    fn set_language(&mut self, lang: &str) -> Result<(), AsrError>;

    /// Enable or disable VAD at runtime.
    fn set_vad(&mut self, enabled: bool) -> Result<(), AsrError>;
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
