use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8};

/// Runtime VAD toggle — shared between main thread and audio processing thread.
/// Updated from settings window at runtime (no restart required).
pub static RUNTIME_VAD_ENABLED: AtomicBool = AtomicBool::new(true);

/// Runtime VAD threshold (0.0–1.0, stored as f32 bits via to_bits/from_bits).
/// Updated from settings window at runtime.
pub static RUNTIME_VAD_THRESHOLD: AtomicU32 = AtomicU32::new(0.1f32.to_bits());

/// Runtime conversion mode (0=None, 1=S2T, 2=T2S).
/// Updated from settings window at runtime.
pub static RUNTIME_CONVERSION_MODE: AtomicU8 = AtomicU8::new(0);

/// Helper to decode RUNTIME_CONVERSION_MODE to a crate::convert::ConversionMode.
#[inline]
pub fn runtime_conversion_mode() -> crate::convert::ConversionMode {
    match RUNTIME_CONVERSION_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => crate::convert::ConversionMode::SimplifiedToTraditional,
        2 => crate::convert::ConversionMode::TraditionalToSimplified,
        _ => crate::convert::ConversionMode::None,
    }
}

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
    /// Chinese text conversion mode (none/s2t/t2s)
    pub conversion: ConversionConfig,
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
    /// Silero VAD threshold (0.0–1.0, default 0.5). Lower = more sensitive.
    pub vad_threshold: f32,
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
    /// Modifier keys for push-to-talk (hold to record, release to inject)
    pub ptt_modifiers: u32,
    /// Virtual key for push-to-talk
    pub ptt_vk: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConversionConfig {
    /// Conversion mode: "none", "s2t", "t2s"
    pub mode: String,
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
    /// egui theme: "Dark" or "Light"
    pub theme: String,
    /// eframe main window X position (None = system default)
    pub window_x: Option<f32>,
    /// eframe main window Y position (None = system default)
    pub window_y: Option<f32>,
    /// eframe main window width (None = default 800)
    pub window_width: Option<f32>,
    /// eframe main window height (None = default 600)
    pub window_height: Option<f32>,
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
            conversion: ConversionConfig::default(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: "zh-TW".into(),
            theme: "Dark".into(),
            window_x: None,
            window_y: None,
            window_width: None,
            window_height: None,
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            chunk_size_ms: 700,
            ringbuf_capacity: 11200 * 40, // ~28s @ 16kHz / ~9.3s @ 48kHz — handles ASR CPU latency + high sample rate capture
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
            vad_threshold: 0.1,
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
            // MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT = 0x4006 (no Alt)
            toggle_modifiers: 0x4006,
            // VK_F2 = 0x71 (rarely conflicts with other apps)
            toggle_vk: 0x71,
            // MOD_ALT | MOD_CONTROL | MOD_NOREPEAT = 0x4003
            lang_modifiers: 0x4003,
            // 'L' virtual key
            lang_vk: 0x4C,
            // MOD_ALT | MOD_CONTROL | MOD_NOREPEAT = 0x4003
            flush_modifiers: 0x4003,
            // VK_SPACE = 0x20
            flush_vk: 0x20,
            // MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT = 0x4006
            ptt_modifiers: 0x4006,
            // 'L' virtual key (Ctrl+Shift+L — different modifiers from lang which uses Alt)
            ptt_vk: 0x4C,
        }
    }
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            mode: "none".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_default_values_are_valid() {
        let config = AppConfig::default();
        assert_eq!(config.audio.sample_rate, 16000);
        assert_eq!(config.audio.channels, 1);
        assert!(config.audio.chunk_size_ms > 0);
        assert_eq!(config.asr.num_threads, 4);
        assert!(config.hotkey.lang_vk > 0);
        assert!(config.hotkey.ptt_vk > 0);
    }

    #[test]
    fn test_app_config_toml_roundtrip() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).expect("serialize");
        let deserialized: AppConfig = toml::from_str(&toml_str).expect("deserialize");

        // Compare key fields
        assert_eq!(deserialized.audio.sample_rate, config.audio.sample_rate);
        assert_eq!(deserialized.audio.channels, config.audio.channels);
        assert_eq!(deserialized.asr.num_threads, config.asr.num_threads);
        assert_eq!(deserialized.asr.provider, config.asr.provider);
        assert_eq!(deserialized.hotkey.lang_vk, config.hotkey.lang_vk);
        assert_eq!(deserialized.hotkey.ptt_vk, config.hotkey.ptt_vk);
        assert_eq!(deserialized.language.language, config.language.language);
        assert_eq!(deserialized.ui.language, config.ui.language);
        assert_eq!(deserialized.injector.strategy, config.injector.strategy);
    }

    #[test]
    fn test_app_config_load_existing_parses_correctly() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).expect("serialize");

        let dir = std::env::temp_dir().join("nemotron_config_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_config.toml");
        std::fs::write(&path, &toml_str).expect("write");

        let loaded = AppConfig::load(&path).expect("load existing");
        assert_eq!(loaded.audio.sample_rate, 16000);
        assert_eq!(loaded.asr.num_threads, 4);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_app_config_load_corrupted_toml_returns_error() {
        let dir = std::env::temp_dir().join("nemotron_config_test_corrupt");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("corrupt.toml");
        std::fs::write(&path, "this is not valid toml {").expect("write");

        let result = AppConfig::load(&path);
        assert!(result.is_err(), "corrupted TOML should return error");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_chunk_samples_calculation() {
        let config = AppConfig::default();
        let samples = config.chunk_samples();
        let expected = (config.audio.sample_rate as u64 * config.audio.chunk_size_ms as u64 / 1000) as usize;
        assert_eq!(samples, expected);
    }

    #[test]
    fn test_runtime_conversion_mode_mapping() {
        use std::sync::atomic::Ordering;

        RUNTIME_CONVERSION_MODE.store(0, Ordering::SeqCst);
        assert_eq!(runtime_conversion_mode(), crate::convert::ConversionMode::None);

        RUNTIME_CONVERSION_MODE.store(1, Ordering::SeqCst);
        assert_eq!(runtime_conversion_mode(), crate::convert::ConversionMode::SimplifiedToTraditional);

        RUNTIME_CONVERSION_MODE.store(2, Ordering::SeqCst);
        assert_eq!(runtime_conversion_mode(), crate::convert::ConversionMode::TraditionalToSimplified);

        RUNTIME_CONVERSION_MODE.store(99, Ordering::SeqCst);
        assert_eq!(runtime_conversion_mode(), crate::convert::ConversionMode::None);

        // Restore default
        RUNTIME_CONVERSION_MODE.store(0, Ordering::SeqCst);
    }
}
