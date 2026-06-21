use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Model directory path
    pub model_dir: PathBuf,
    /// Audio capture configuration
    pub audio: AudioConfig,
    /// ASR engine configuration
    pub asr: AsrProviderConfig,
    /// Text injector configuration
    pub injector: InjectorConfig,
    /// Hotkey configuration
    pub hotkey: HotkeyConfig,
    /// Language settings
    pub language: LanguageConfig,
    /// UI settings
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Sample rate (Hz) - model requires 16000
    pub sample_rate: u32,
    /// Number of channels (1 = mono)
    pub channels: u16,
    /// Chunk size in milliseconds
    pub chunk_size_ms: u32,
    /// Ring buffer capacity in samples
    pub ringbuf_capacity: usize,
    /// Device name (empty = default)
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AsrProviderConfig {
    /// Execution provider: "cpu" or "cuda"
    pub provider: String,
    /// Number of inference threads
    pub num_threads: u32,
    /// Enable Silero VAD
    pub use_vad: bool,
    /// Decoding method: "greedy_search" or "modified_beam_search"
    pub decoding_method: String,
    /// Max active paths for beam search
    pub max_active_paths: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InjectorConfig {
    /// Injection strategy: "auto", "sendinput", "uiautomation", "clipboard"
    pub strategy: String,
    /// Delay between keystrokes in milliseconds
    pub key_delay_ms: u64,
    /// Restore clipboard after clipboard injection
    pub restore_clipboard: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    /// Modifier keys for toggle (MOD_ALT | MOD_CONTROL | MOD_NOREPEAT, etc.)
    pub toggle_modifiers: u32,
    /// Virtual key for toggle
    pub toggle_vk: u32,
    /// Modifier keys for language switch
    pub lang_modifiers: u32,
    /// Virtual key for language switch
    pub lang_vk: u32,
    /// Modifier keys for flush
    pub flush_modifiers: u32,
    /// Virtual key for flush
    pub flush_vk: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LanguageConfig {
    /// Language code: "en", "zh", "ja", "de", "fr", "es", "ko", "auto", etc.
    pub language: String,
    /// Ordered list for cycling
    pub cycle_order: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// UI language: "en" (English) or "zh" (Traditional Chinese)
    pub language: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            model_dir: PathBuf::from("models"),
            audio: AudioConfig::default(),
            asr: AsrProviderConfig::default(),
            injector: InjectorConfig::default(),
            hotkey: HotkeyConfig::default(),
            language: LanguageConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: "en".into(),
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            chunk_size_ms: 560,
            ringbuf_capacity: 8960 * 4, // ~2.24 seconds
            device_name: String::new(),
        }
    }
}

impl Default for AsrProviderConfig {
    fn default() -> Self {
        Self {
            provider: "cpu".into(),
            num_threads: 4,
            use_vad: true,
            decoding_method: "greedy_search".into(),
            max_active_paths: 4,
        }
    }
}

impl Default for InjectorConfig {
    fn default() -> Self {
        Self {
            strategy: "auto".into(),
            key_delay_ms: 5,
            restore_clipboard: true,
        }
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            // MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT = 0x4007
            toggle_modifiers: 0x4007,
            // 'R' virtual key
            toggle_vk: 0x52,
            // MOD_ALT | MOD_CONTROL | MOD_NOREPEAT = 0x4003
            lang_modifiers: 0x4003,
            // 'L' virtual key
            lang_vk: 0x4C,
            // MOD_ALT | MOD_CONTROL | MOD_NOREPEAT = 0x4003
            flush_modifiers: 0x4003,
            // VK_SPACE = 0x20
            flush_vk: 0x20,
        }
    }
}

impl Default for LanguageConfig {
    fn default() -> Self {
        Self {
            language: "zh".into(),
            cycle_order: vec![
                "zh".into(), "en".into(), "ja".into(), "de".into(),
                "fr".into(), "es".into(), "ko".into(),
            ],
        }
    }
}

impl AppConfig {
    /// Load configuration from a TOML file.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: AppConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            let config = AppConfig::default();
            // Save default config for user reference
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let toml_str = toml::to_string_pretty(&config)?;
            let _ = std::fs::write(path, toml_str);
            Ok(config)
        }
    }

    /// Get the model directory path.
    #[allow(dead_code)]
    pub fn model_dir(&self) -> &std::path::Path {
        &self.model_dir
    }

    /// Get the chunk size in samples.
    #[allow(dead_code)]
    pub fn chunk_samples(&self) -> usize {
        (self.audio.sample_rate as u64 * self.audio.chunk_size_ms as u64 / 1000) as usize
    }

    /// Save configuration to a TOML file.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        let toml_str = toml::to_string_pretty(self)?;
        std::fs::write(path.as_ref(), toml_str)?;
        Ok(())
    }
}
