use std::path::PathBuf;

/// ASR engine configuration.
#[derive(Debug, Clone)]
pub struct AsrConfig {
    /// Directory containing model files
    pub model_dir: PathBuf,
    /// Execution provider: "cpu" or "cuda"
    pub provider: String,
    /// Number of threads for inference
    pub num_threads: u32,
    /// Chunk size in milliseconds (560 default)
    pub chunk_size_ms: u32,
    /// Enable Silero VAD
    pub use_vad: bool,
    /// Language code: "en", "zh", "de", etc. or "auto"
    pub language: String,
    /// Decoding method: "greedy_search" or "modified_beam_search"
    pub decoding_method: String,
    /// Max active paths for beam search
    pub max_active_paths: i32,
    /// Sample rate expected by the model
    pub sample_rate: u32,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            model_dir: PathBuf::from("models"),
            provider: "cpu".into(),
            num_threads: 4,
            chunk_size_ms: 560,
            use_vad: true,
            language: "zh".into(),
            decoding_method: "greedy_search".into(),
            max_active_paths: 4,
            sample_rate: 16000,
        }
    }
}

impl AsrConfig {
    /// Get the chunk size in samples.
    pub fn chunk_samples(&self) -> usize {
        (self.sample_rate as u64 * self.chunk_size_ms as u64 / 1000) as usize
    }
}

/// Language code to numeric ID mapping for the Nemotron model.
pub fn language_to_lang_id(language: &str) -> Option<i32> {
    match language {
        "en" => Some(0),
        "de" => Some(8),
        "zh" => Some(9),
        "es" => Some(3),
        "fr" => Some(5),
        "it" => Some(6),
        "ja" => Some(17),
        "ko" => Some(18),
        "pt" => Some(4),
        "ru" => Some(10),
        "ar" => Some(34),
        "hi" => Some(14),
        "vi" => Some(49),
        "th" => Some(64),
        "tr" => Some(29),
        "nl" => Some(48),
        "pl" => Some(50),
        "sv" => Some(43),
        "da" => Some(65),
        "fi" => Some(44),
        "cs" => Some(55),
        "hu" => Some(46),
        "ro" => Some(47),
        "el" => Some(45),
        "he" => Some(51),
        "id" => Some(62),
        "ms" => Some(63),
        "uk" => Some(53),
        "hr" => Some(60),
        "sk" => Some(56),
        "sl" => Some(57),
        "bg" => Some(54),
        "sr" => Some(52),
        "ca" => Some(61),
        "tl" => Some(66),
        "bn" => Some(31),
        "ta" => Some(53),
        "te" => Some(54),
        "mr" => Some(42),
        "ur" => Some(37),
        _ => None,
    }
}
