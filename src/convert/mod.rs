/// Chinese text conversion between Simplified and Traditional variants.
///
/// Wraps the [`ferrous_opencc`] crate behind a simplified interface
/// that is safe to call from any thread after a one-time initialization.

use ferrous_opencc::{config::BuiltinConfig, error::OpenCCError, OpenCC};
use std::sync::OnceLock;

/// Conversion direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionMode {
    /// No conversion applied.
    None,
    /// Simplified Chinese → Traditional Chinese.
    SimplifiedToTraditional,
    /// Traditional Chinese → Simplified Chinese.
    TraditionalToSimplified,
}

impl ConversionMode {
    /// Parse from a config string (stored in config.toml).
    pub fn from_config(s: &str) -> Self {
        match s {
            "s2t" => Self::SimplifiedToTraditional,
            "t2s" => Self::TraditionalToSimplified,
            _ => Self::None,
        }
    }

    /// Serialize to a config string.
    #[allow(dead_code)]
    pub fn to_config(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SimplifiedToTraditional => "s2t",
            Self::TraditionalToSimplified => "t2s",
        }
    }

    /// Human-readable display name.
    #[allow(dead_code)]
    pub fn display_name(&self, ui_lang: &str) -> &'static str {
        match (self, ui_lang) {
            (Self::None, _) => "None",
            (Self::SimplifiedToTraditional, "zh") => "簡體 → 繁體",
            (Self::SimplifiedToTraditional, _) => "Simplified → Traditional",
            (Self::TraditionalToSimplified, "zh") => "繁體 → 簡體",
            (Self::TraditionalToSimplified, _) => "Traditional → Simplified",
        }
    }

    /// All modes for dropdown population.
    #[allow(dead_code)]
    pub fn all() -> &'static [Self] {
        static ALL: [ConversionMode; 3] = [
            ConversionMode::None,
            ConversionMode::SimplifiedToTraditional,
            ConversionMode::TraditionalToSimplified,
        ];
        &ALL
    }

    /// Index in the `all()` list.
    #[allow(dead_code)]
    pub fn index(&self) -> usize {
        match self {
            Self::None => 0,
            Self::SimplifiedToTraditional => 1,
            Self::TraditionalToSimplified => 2,
        }
    }

    /// Get mode from index.
    #[allow(dead_code)]
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => Self::SimplifiedToTraditional,
            2 => Self::TraditionalToSimplified,
            _ => Self::None,
        }
    }
}

// ── Thread-safe OpenCC instance cache ──────────────────────────────────

struct Converters {
    s2t: OpenCC,
    t2s: OpenCC,
}

static CONVERTERS: OnceLock<Converters> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s2t_conversion() {
        let opencc = OpenCC::from_config(BuiltinConfig::S2t).unwrap();
        let result = opencc.convert("这是测试");
        assert_eq!(result, "這是測試", "S2T failed: got '{}'", result);
    }

    #[test]
    fn test_t2s_conversion() {
        let opencc = OpenCC::from_config(BuiltinConfig::T2s).unwrap();
        let result = opencc.convert("這是測試");
        assert_eq!(result, "这是测试", "T2S failed: got '{}'", result);
    }

    #[test]
    fn test_convert_text_s2t() {
        init_converters().unwrap();
        let result = convert_text("这是测试", ConversionMode::SimplifiedToTraditional);
        assert_eq!(result, "這是測試", "convert_text S2T failed: got '{}'", result);
    }

    #[test]
    fn test_convert_text_t2s() {
        init_converters().unwrap();
        let result = convert_text("這是測試", ConversionMode::TraditionalToSimplified);
        assert_eq!(result, "这是测试", "convert_text T2S failed: got '{}'", result);
    }

    #[test]
    fn test_convert_text_none() {
        let result = convert_text("这是测试", ConversionMode::None);
        assert_eq!(result, "这是测试");
    }

    #[test]
    fn test_convert_text_empty() {
        let result = convert_text("", ConversionMode::SimplifiedToTraditional);
        assert_eq!(result, "");
    }

    #[test]
    fn test_conversion_mode_from_config() {
        assert_eq!(ConversionMode::from_config("s2t"), ConversionMode::SimplifiedToTraditional);
        assert_eq!(ConversionMode::from_config("t2s"), ConversionMode::TraditionalToSimplified);
        assert_eq!(ConversionMode::from_config("none"), ConversionMode::None);
        assert_eq!(ConversionMode::from_config("invalid"), ConversionMode::None);
    }

    #[test]
    fn test_conversion_mode_index_roundtrip() {
        for mode in ConversionMode::all() {
            let idx = mode.index();
            let back = ConversionMode::from_index(idx);
            assert_eq!(*mode, back, "Index roundtrip failed for {:?}", mode);
        }
    }
}

/// Initialize converters (call once before any conversion).
pub fn init_converters() -> Result<(), OpenCCError> {
    if CONVERTERS.get().is_none() {
        let s2t = OpenCC::from_config(BuiltinConfig::S2t)?;
        let t2s = OpenCC::from_config(BuiltinConfig::T2s)?;
        let _ = CONVERTERS.set(Converters { s2t, t2s });
    }
    Ok(())
}

/// Convert text according to the specified mode.
/// Returns the original text if mode is `None` or conversion fails.
pub fn convert_text(text: &str, mode: ConversionMode) -> String {
    if text.is_empty() || mode == ConversionMode::None {
        return text.to_string();
    }

    let converters = match CONVERTERS.get() {
        Some(c) => c,
        None => return text.to_string(), // not initialized, pass through
    };

    let result = match mode {
        ConversionMode::SimplifiedToTraditional => converters.s2t.convert(text),
        ConversionMode::TraditionalToSimplified => converters.t2s.convert(text),
        ConversionMode::None => return text.to_string(),
    };

    result
}
