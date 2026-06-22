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
    /// Silero VAD threshold (0.0–1.0, default 0.5). Lower = more sensitive.
    pub vad_threshold: f32,
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
            vad_threshold: 0.1,
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
/// Reference: https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b
#[allow(dead_code)]
pub fn language_to_lang_id(language: &str) -> Option<i32> {
    match language {
        "en" => Some(0),   // English
        "de" => Some(8),   // German
        "zh" => Some(9),   // Mandarin Chinese
        "es" => Some(3),   // Spanish
        "fr" => Some(5),   // French
        "it" => Some(6),   // Italian
        "ja" => Some(17),  // Japanese
        "ko" => Some(18),  // Korean
        "pt" => Some(4),   // Portuguese
        "ru" => Some(10),  // Russian
        "ar" => Some(34),  // Arabic
        "hi" => Some(14),  // Hindi
        "vi" => Some(49),  // Vietnamese
        "th" => Some(64),  // Thai
        "tr" => Some(29),  // Turkish
        "nl" => Some(48),  // Dutch
        "pl" => Some(50),  // Polish
        "sv" => Some(43),  // Swedish
        "da" => Some(65),  // Danish
        "fi" => Some(44),  // Finnish
        "cs" => Some(55),  // Czech
        "hu" => Some(46),  // Hungarian
        "ro" => Some(47),  // Romanian
        "el" => Some(45),  // Greek
        "he" => Some(51),  // Hebrew
        "id" => Some(62),  // Indonesian
        "ms" => Some(63),  // Malay
        "uk" => Some(53),  // Ukrainian
        "hr" => Some(60),  // Croatian
        "sk" => Some(56),  // Slovak
        "sl" => Some(57),  // Slovenian
        "bg" => Some(54),  // Bulgarian
        "sr" => Some(52),  // Serbian
        "ca" => Some(61),  // Catalan
        "tl" => Some(66),  // Tagalog / Filipino
        "bn" => Some(31),  // Bengali
        "ta" => Some(23),  // Tamil
        "te" => Some(24),  // Telugu
        "mr" => Some(42),  // Marathi
        "ur" => Some(37),  // Urdu
        // Additional languages
        "af" => Some(7),   // Afrikaans
        "az" => Some(41),  // Azerbaijani
        "be" => Some(39),  // Belarusian
        "bs" => Some(59),  // Bosnian
        "cy" => Some(69),  // Welsh
        "eo" => Some(2),   // Esperanto
        "et" => Some(40),  // Estonian
        "eu" => Some(1),   // Basque
        "fa" => Some(35),  // Persian / Farsi
        "fil" => Some(66), // Filipino
        "ga" => Some(68),  // Irish
        "gl" => Some(11),  // Galician
        "gu" => Some(30),  // Gujarati
        "ha" => Some(74),  // Hausa
        "hy" => Some(38),  // Armenian
        "is" => Some(67),  // Icelandic
        "jv" => Some(75),  // Javanese
        "ka" => Some(36),  // Georgian
        "kk" => Some(33),  // Kazakh
        "km" => Some(70),  // Khmer
        "kn" => Some(22),  // Kannada
        "ku" => Some(73),  // Kurdish
        "ky" => Some(32),  // Kyrgyz
        "la" => Some(76),  // Latin
        "lo" => Some(71),  // Lao
        "lt" => Some(58),  // Lithuanian
        "mg" => Some(77),  // Malagasy
        "mk" => Some(78),  // Macedonian
        "ml" => Some(21),  // Malayalam
        "mn" => Some(79),  // Mongolian
        "mt" => Some(80),  // Maltese
        "my" => Some(72),  // Burmese
        "ne" => Some(15),  // Nepali
        "or" => Some(26),  // Odia / Oriya
        "pa" => Some(27),  // Punjabi
        "ps" => Some(28),  // Pashto
        "si" => Some(25),  // Sinhala
        "sq" => Some(12),  // Albanian
        "su" => Some(81),  // Sundanese
        "sw" => Some(16),  // Swahili
        "tk" => Some(82),  // Turkmen
        "uz" => Some(19),  // Uzbek
        "xh" => Some(13),  // Xhosa
        "yi" => Some(83),  // Yiddish
        "yo" => Some(84),  // Yoruba
        "zu" => Some(20),  // Zulu
        _ => None,
    }
}
