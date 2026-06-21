/// Bilingual (Traditional Chinese / English) UI strings for the application.
/// All user-facing text is defined here to enable runtime language switching.

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLang {
    English,
    Chinese,
}

impl UiLang {
    pub fn from_code(code: &str) -> Self {
        match code {
            "zh" => UiLang::Chinese,
            _ => UiLang::English,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            UiLang::English => "en",
            UiLang::Chinese => "zh",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            UiLang::English => "English",
            UiLang::Chinese => "中文",
        }
    }
}

impl Default for UiLang {
    fn default() -> Self {
        UiLang::English
    }
}

/// All application UI strings, localized to the selected language.
pub struct Strings {
    pub lang: UiLang,
}

impl Strings {
    pub fn new(lang: UiLang) -> Self {
        Self { lang }
    }

    // ── Application identity ──
    pub fn app_name(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Nemotron Voice Input",
            UiLang::Chinese => "Nemotron 語音輸入法",
        }
    }

    pub fn settings_title(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Settings",
            UiLang::Chinese => "設定",
        }
    }

    // ── Tray context menu ──
    pub fn tray_toggle_recording(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Toggle Recording",
            UiLang::Chinese => "切換錄音",
        }
    }

    pub fn tray_cycle_language(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Cycle Language",
            UiLang::Chinese => "切換語言",
        }
    }

    pub fn tray_flush(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Flush",
            UiLang::Chinese => "清除緩衝",
        }
    }

    pub fn tray_settings(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Settings...",
            UiLang::Chinese => "設定...",
        }
    }

    pub fn tray_exit(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Exit",
            UiLang::Chinese => "離開",
        }
    }

    // ── Tray tooltip ──
    pub fn tray_tip_idle(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Nemotron Voice Input - Idle",
            UiLang::Chinese => "Nemotron 語音輸入法 - 待命中",
        }
    }

    pub fn tray_tip_recording(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Nemotron Voice Input - Recording...",
            UiLang::Chinese => "Nemotron 語音輸入法 - 錄音中...",
        }
    }

    // ── Balloon notifications ──
    pub fn notification_ready(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Ready. Press Ctrl+Alt+R to toggle recording.",
            UiLang::Chinese => "就緒。按 Ctrl+Alt+R 切換錄音。",
        }
    }

    pub fn notification_recording_started(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Recording started",
            UiLang::Chinese => "錄音開始",
        }
    }

    pub fn notification_recording_stopped(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Recording stopped",
            UiLang::Chinese => "錄音停止",
        }
    }

    pub fn notification_flushed(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Buffer cleared",
            UiLang::Chinese => "緩衝區已清除",
        }
    }

    pub fn notification_language_switched_to(&self, lang: &str) -> String {
        match self.lang {
            UiLang::English => format!("Switched to {}", lang),
            UiLang::Chinese => format!("已切換至 {}", lang),
        }
    }

    // ── Settings window ──
    pub fn settings_ui_language(&self) -> &'static str {
        match self.lang {
            UiLang::English => "UI Language:",
            UiLang::Chinese => "介面語言：",
        }
    }

    pub fn settings_asr_section(&self) -> &'static str {
        match self.lang {
            UiLang::English => "ASR Settings",
            UiLang::Chinese => "語音辨識設定",
        }
    }

    pub fn settings_asr_language(&self) -> &'static str {
        match self.lang {
            UiLang::English => "ASR Language:",
            UiLang::Chinese => "辨識語言：",
        }
    }

    pub fn settings_provider(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Provider:",
            UiLang::Chinese => "執行提供者：",
        }
    }

    pub fn settings_decoding(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Decoding:",
            UiLang::Chinese => "解碼方式：",
        }
    }

    pub fn settings_threads(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Num Threads:",
            UiLang::Chinese => "執行緒數：",
        }
    }

    pub fn settings_vad(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Enable Voice Activity Detection (VAD)",
            UiLang::Chinese => "啟用語音活動偵測 (VAD)",
        }
    }

    pub fn settings_injection_section(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Injection Settings",
            UiLang::Chinese => "文字注入設定",
        }
    }

    pub fn settings_inject_strategy(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Strategy:",
            UiLang::Chinese => "注入策略：",
        }
    }

    pub fn settings_key_delay(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Key Delay (ms):",
            UiLang::Chinese => "按鍵延遲 (毫秒)：",
        }
    }

    pub fn settings_restore_clipboard(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Restore clipboard after injection",
            UiLang::Chinese => "注入後還原剪貼簿",
        }
    }

    pub fn settings_hotkeys_section(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Hotkeys",
            UiLang::Chinese => "快捷鍵",
        }
    }

    pub fn settings_hotkey_line(&self, action: &str, key: &str) -> String {
        match self.lang {
            UiLang::English => format!("{}: {}", action, key),
            UiLang::Chinese => format!("{}：{}", action, key),
        }
    }

    pub fn settings_save(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Save",
            UiLang::Chinese => "儲存",
        }
    }

    pub fn settings_cancel(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Cancel",
            UiLang::Chinese => "取消",
        }
    }

    pub fn settings_saved(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Settings saved",
            UiLang::Chinese => "設定已儲存",
        }
    }

    pub fn settings_model_status(&self, ok: usize, total: usize) -> String {
        match self.lang {
            UiLang::English => format!("Model files: {}/{} available", ok, total),
            UiLang::Chinese => format!("模型檔案：{}/{} 可用", ok, total),
        }
    }

    pub fn settings_general_section(&self) -> &'static str {
        match self.lang {
            UiLang::English => "General",
            UiLang::Chinese => "一般設定",
        }
    }

    /// Full hotkey display string for the settings window.
    pub fn hotkey_toggle_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Toggle Recording",
            UiLang::Chinese => "切換錄音",
        }
    }

    pub fn hotkey_lang_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Cycle Language",
            UiLang::Chinese => "切換語言",
        }
    }

    pub fn hotkey_flush_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Flush",
            UiLang::Chinese => "清除緩衝",
        }
    }

    /// Language display names for the combobox.
    pub fn language_display_name(&self, code: &str) -> String {
        match code {
            "en" => match self.lang {
                UiLang::English => "English",
                UiLang::Chinese => "英文",
            },
            "zh" => match self.lang {
                UiLang::English => "Chinese (Mandarin)",
                UiLang::Chinese => "中文（國語）",
            },
            "ja" => match self.lang {
                UiLang::English => "Japanese",
                UiLang::Chinese => "日文",
            },
            "ko" => match self.lang {
                UiLang::English => "Korean",
                UiLang::Chinese => "韓文",
            },
            "de" => match self.lang {
                UiLang::English => "German",
                UiLang::Chinese => "德文",
            },
            "fr" => match self.lang {
                UiLang::English => "French",
                UiLang::Chinese => "法文",
            },
            "es" => match self.lang {
                UiLang::English => "Spanish",
                UiLang::Chinese => "西班牙文",
            },
            "it" => match self.lang {
                UiLang::English => "Italian",
                UiLang::Chinese => "義大利文",
            },
            "pt" => match self.lang {
                UiLang::English => "Portuguese",
                UiLang::Chinese => "葡萄牙文",
            },
            "ru" => match self.lang {
                UiLang::English => "Russian",
                UiLang::Chinese => "俄文",
            },
            "ar" => match self.lang {
                UiLang::English => "Arabic",
                UiLang::Chinese => "阿拉伯文",
            },
            "hi" => match self.lang {
                UiLang::English => "Hindi",
                UiLang::Chinese => "印地文",
            },
            "vi" => match self.lang {
                UiLang::English => "Vietnamese",
                UiLang::Chinese => "越南文",
            },
            "th" => match self.lang {
                UiLang::English => "Thai",
                UiLang::Chinese => "泰文",
            },
            "tr" => match self.lang {
                UiLang::English => "Turkish",
                UiLang::Chinese => "土耳其文",
            },
            "nl" => match self.lang {
                UiLang::English => "Dutch",
                UiLang::Chinese => "荷蘭文",
            },
            "pl" => match self.lang {
                UiLang::English => "Polish",
                UiLang::Chinese => "波蘭文",
            },
            "sv" => match self.lang {
                UiLang::English => "Swedish",
                UiLang::Chinese => "瑞典文",
            },
            "el" => match self.lang {
                UiLang::English => "Greek",
                UiLang::Chinese => "希臘文",
            },
            "he" => match self.lang {
                UiLang::English => "Hebrew",
                UiLang::Chinese => "希伯來文",
            },
            "id" => match self.lang {
                UiLang::English => "Indonesian",
                UiLang::Chinese => "印尼文",
            },
            "auto" => match self.lang {
                UiLang::English => "Auto-detect",
                UiLang::Chinese => "自動偵測",
            },
            _ => code,
        }
        .to_string()
    }
}
