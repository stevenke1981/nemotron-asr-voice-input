/// Tri-lingual UI strings (Traditional Chinese / Simplified Chinese / English).
/// All user-facing text is defined here to enable runtime language switching.

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLang {
    English,
    ChineseSimplified,
    ChineseTraditional,
}

impl UiLang {
    /// Parse from config code: "en" → English, "zh-CN" → ChineseSimplified,
    /// "zh-TW" or "zh" → ChineseTraditional (backward compat).
    pub fn from_code(code: &str) -> Self {
        match code {
            "en" => UiLang::English,
            "zh-CN" | "zh_Hans" | "hans" => UiLang::ChineseSimplified,
            "zh-TW" | "zh_Hant" | "hant" | "zh" => UiLang::ChineseTraditional,
            _ => UiLang::ChineseTraditional, // safe default
        }
    }

    /// Serialize to config code.
    pub fn code(&self) -> &'static str {
        match self {
            UiLang::English => "en",
            UiLang::ChineseSimplified => "zh-CN",
            UiLang::ChineseTraditional => "zh-TW",
        }
    }

    /// Human-readable display name in the language itself.
    pub fn display_name(&self) -> &'static str {
        match self {
            UiLang::English => "English",
            UiLang::ChineseSimplified => "简体中文",
            UiLang::ChineseTraditional => "繁體中文",
        }
    }
}

impl Default for UiLang {
    /// Default to Traditional Chinese.
    fn default() -> Self {
        UiLang::ChineseTraditional
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
            UiLang::ChineseSimplified => "Nemotron 语音输入",
            UiLang::ChineseTraditional => "Nemotron 語音輸入",
        }
    }

    pub fn settings_title(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Settings",
            UiLang::ChineseSimplified => "设置",
            UiLang::ChineseTraditional => "設定",
        }
    }

    // ── Tray context menu ──
    pub fn tray_toggle_recording(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Toggle Recording",
            UiLang::ChineseSimplified => "切换录音",
            UiLang::ChineseTraditional => "切換錄音",
        }
    }

    pub fn tray_cycle_language(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Cycle Language",
            UiLang::ChineseSimplified => "切换语言",
            UiLang::ChineseTraditional => "切換語言",
        }
    }

    pub fn tray_flush(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Flush Buffer",
            UiLang::ChineseSimplified => "清除缓冲",
            UiLang::ChineseTraditional => "清除緩衝",
        }
    }

    pub fn tray_settings(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Settings...",
            UiLang::ChineseSimplified => "设置...",
            UiLang::ChineseTraditional => "設定...",
        }
    }

    pub fn tray_show_main_window(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Show Main Window",
            UiLang::ChineseSimplified => "显示主窗口",
            UiLang::ChineseTraditional => "顯示主視窗",
        }
    }

    pub fn tray_toggle_overlay(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Toggle Overlay",
            UiLang::ChineseSimplified => "切换浮窗",
            UiLang::ChineseTraditional => "切換浮窗",
        }
    }

    pub fn tray_exit(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Exit",
            UiLang::ChineseSimplified => "退出",
            UiLang::ChineseTraditional => "離開",
        }
    }

    // ── Tray tooltip ──
    pub fn tray_tip_idle(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Nemotron Voice Input - Idle",
            UiLang::ChineseSimplified => "Nemotron 语音输入 - 待命中",
            UiLang::ChineseTraditional => "Nemotron 語音輸入 - 待命中",
        }
    }

    pub fn tray_tip_recording(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Nemotron Voice Input - Recording...",
            UiLang::ChineseSimplified => "Nemotron 语音输入 - 录音中...",
            UiLang::ChineseTraditional => "Nemotron 語音輸入 - 錄音中...",
        }
    }

    // ── Balloon notifications ──
    pub fn notification_ready(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Ready (see Settings for hotkey bindings)",
            UiLang::ChineseSimplified => "就绪（设置窗口可查看快捷键）",
            UiLang::ChineseTraditional => "就緒（設定視窗可查看快捷鍵）",
        }
    }

    pub fn notification_recording_started(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Recording started",
            UiLang::ChineseSimplified => "录音开始",
            UiLang::ChineseTraditional => "錄音開始",
        }
    }

    pub fn notification_recording_stopped(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Recording stopped",
            UiLang::ChineseSimplified => "录音停止",
            UiLang::ChineseTraditional => "錄音停止",
        }
    }

    pub fn notification_flushed(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Buffer cleared",
            UiLang::ChineseSimplified => "缓冲已清除",
            UiLang::ChineseTraditional => "緩衝區已清除",
        }
    }

    pub fn notification_language_switched_to(&self, lang: &str) -> String {
        match self.lang {
            UiLang::English => format!("Switched to {}", lang),
            UiLang::ChineseSimplified => format!("已切换至 {}", lang),
            UiLang::ChineseTraditional => format!("已切換至 {}", lang),
        }
    }

    // ── Settings window ──
    pub fn settings_ui_language(&self) -> &'static str {
        match self.lang {
            UiLang::English => "UI Language",
            UiLang::ChineseSimplified => "界面语言",
            UiLang::ChineseTraditional => "介面語言",
        }
    }

    pub fn settings_asr_section(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Speech Recognition",
            UiLang::ChineseSimplified => "语音识别",
            UiLang::ChineseTraditional => "語音辨識",
        }
    }

    pub fn settings_asr_language(&self) -> &'static str {
        match self.lang {
            UiLang::English => "ASR Language",
            UiLang::ChineseSimplified => "识别语言",
            UiLang::ChineseTraditional => "辨識語言",
        }
    }

    pub fn settings_provider(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Execution Provider",
            UiLang::ChineseSimplified => "执行提供者",
            UiLang::ChineseTraditional => "執行提供者",
        }
    }

    pub fn settings_decoding(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Decoding Method",
            UiLang::ChineseSimplified => "解码方式",
            UiLang::ChineseTraditional => "解碼方式",
        }
    }

    pub fn settings_threads(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Num Threads",
            UiLang::ChineseSimplified => "线程数",
            UiLang::ChineseTraditional => "執行緒數",
        }
    }

    pub fn settings_vad(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Voice Activity Detection (VAD)",
            UiLang::ChineseSimplified => "语音活动检测 (VAD)",
            UiLang::ChineseTraditional => "語音活動偵測 (VAD)",
        }
    }

    pub fn settings_vad_threshold(&self) -> &'static str {
        match self.lang {
            UiLang::English => "VAD Threshold",
            UiLang::ChineseSimplified => "VAD 阈值",
            UiLang::ChineseTraditional => "VAD 閥值",
        }
    }

    pub fn settings_injection_section(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Text Injection",
            UiLang::ChineseSimplified => "文字注入",
            UiLang::ChineseTraditional => "文字注入",
        }
    }

    pub fn settings_inject_strategy(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Injection Strategy",
            UiLang::ChineseSimplified => "注入策略",
            UiLang::ChineseTraditional => "注入策略",
        }
    }

    pub fn settings_key_delay(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Key Delay (ms)",
            UiLang::ChineseSimplified => "按键延迟 (毫秒)",
            UiLang::ChineseTraditional => "按鍵延遲 (毫秒)",
        }
    }

    pub fn settings_restore_clipboard(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Restore clipboard after injection",
            UiLang::ChineseSimplified => "注入后还原剪贴板",
            UiLang::ChineseTraditional => "注入後還原剪貼簿",
        }
    }

    pub fn settings_hotkeys_section(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Hotkeys",
            UiLang::ChineseSimplified => "快捷键",
            UiLang::ChineseTraditional => "快捷鍵",
        }
    }

    #[allow(dead_code)]
    pub fn settings_hotkey_line(&self, action: &str, key: &str) -> String {
        match self.lang {
            UiLang::English => format!("{}: {}", action, key),
            UiLang::ChineseSimplified => format!("{}：{}", action, key),
            UiLang::ChineseTraditional => format!("{}：{}", action, key),
        }
    }

    pub fn settings_save(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Save Settings",
            UiLang::ChineseSimplified => "保存设置",
            UiLang::ChineseTraditional => "儲存設定",
        }
    }

    pub fn settings_cancel(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Cancel",
            UiLang::ChineseSimplified => "取消",
            UiLang::ChineseTraditional => "取消",
        }
    }

    #[allow(dead_code)]
    pub fn settings_saved(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Settings saved",
            UiLang::ChineseSimplified => "设置已保存",
            UiLang::ChineseTraditional => "設定已儲存",
        }
    }

    pub fn settings_general_section(&self) -> &'static str {
        match self.lang {
            UiLang::English => "General",
            UiLang::ChineseSimplified => "常规",
            UiLang::ChineseTraditional => "一般設定",
        }
    }

    pub fn settings_conversion_section(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Chinese Conversion",
            UiLang::ChineseSimplified => "简繁转换",
            UiLang::ChineseTraditional => "簡繁轉換",
        }
    }

    pub fn settings_conversion_mode(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Conversion Mode",
            UiLang::ChineseSimplified => "转换方向",
            UiLang::ChineseTraditional => "轉換方向",
        }
    }

    /// Full hotkey display string for the settings window.
    pub fn hotkey_toggle_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Toggle Recording",
            UiLang::ChineseSimplified => "切换录音",
            UiLang::ChineseTraditional => "切換錄音",
        }
    }

    pub fn hotkey_lang_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Cycle Language",
            UiLang::ChineseSimplified => "切换语言",
            UiLang::ChineseTraditional => "切換語言",
        }
    }

    pub fn hotkey_flush_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Flush Buffer",
            UiLang::ChineseSimplified => "清除缓冲",
            UiLang::ChineseTraditional => "清除緩衝",
        }
    }

    pub fn hotkey_ptt_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Push-to-Talk (hold)",
            UiLang::ChineseSimplified => "按住说话（松开即送）",
            UiLang::ChineseTraditional => "按住說話（放開即送）",
        }
    }

    // ── Main window GUI (egui) ──
    pub fn status_recording(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Recording",
            UiLang::ChineseSimplified => "录音中",
            UiLang::ChineseTraditional => "錄音中",
        }
    }

    pub fn status_idle(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Idle",
            UiLang::ChineseSimplified => "待命中",
            UiLang::ChineseTraditional => "待命中",
        }
    }

    pub fn lang_label(&self, lang: &str) -> String {
        let prefix = match self.lang {
            UiLang::English => "Lang",
            UiLang::ChineseSimplified => "语言",
            UiLang::ChineseTraditional => "語言",
        };
        format!("{}: {}", prefix, lang)
    }

    pub fn convert_label(&self, mode: &str) -> String {
        match self.lang {
            UiLang::English => format!("Convert: {}", mode),
            UiLang::ChineseSimplified => format!("转换：{}", mode),
            UiLang::ChineseTraditional => format!("轉換：{}", mode),
        }
    }

    pub fn hide_overlay(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Hide Overlay",
            UiLang::ChineseSimplified => "隐藏浮窗",
            UiLang::ChineseTraditional => "隱藏浮窗",
        }
    }

    pub fn show_overlay(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Show Overlay",
            UiLang::ChineseSimplified => "显示浮窗",
            UiLang::ChineseTraditional => "顯示浮窗",
        }
    }

    pub fn live_transcript(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Live Transcript",
            UiLang::ChineseSimplified => "实时转录",
            UiLang::ChineseTraditional => "即時轉錄",
        }
    }

    pub fn final_prefix(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Final",
            UiLang::ChineseSimplified => "最终",
            UiLang::ChineseTraditional => "最終",
        }
    }

    pub fn partial_prefix(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Partial",
            UiLang::ChineseSimplified => "实时",
            UiLang::ChineseTraditional => "即時",
        }
    }

    pub fn history_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "History",
            UiLang::ChineseSimplified => "历史记录",
            UiLang::ChineseTraditional => "歷史記錄",
        }
    }

    pub fn clear_all(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Clear All",
            UiLang::ChineseSimplified => "清除全部",
            UiLang::ChineseTraditional => "清除全部",
        }
    }

    pub fn copy_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Copy",
            UiLang::ChineseSimplified => "复制",
            UiLang::ChineseTraditional => "複製",
        }
    }

    pub fn delete_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Delete",
            UiLang::ChineseSimplified => "删除",
            UiLang::ChineseTraditional => "刪除",
        }
    }

    pub fn stop_recording_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "■ Stop",
            UiLang::ChineseSimplified => "■ 停止",
            UiLang::ChineseTraditional => "■ 停止",
        }
    }

    pub fn start_recording_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "● Start",
            UiLang::ChineseSimplified => "● 开始",
            UiLang::ChineseTraditional => "● 開始",
        }
    }

    pub fn cycle_language_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Cycle Lang",
            UiLang::ChineseSimplified => "切换语言",
            UiLang::ChineseTraditional => "切換語言",
        }
    }

    pub fn flush_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Flush",
            UiLang::ChineseSimplified => "清除缓冲",
            UiLang::ChineseTraditional => "清除緩衝",
        }
    }

    pub fn settings_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "⚙ Settings",
            UiLang::ChineseSimplified => "⚙ 设置",
            UiLang::ChineseTraditional => "⚙ 設定",
        }
    }

    pub fn settings_enabled(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Enabled",
            UiLang::ChineseSimplified => "启用",
            UiLang::ChineseTraditional => "啟用",
        }
    }

    #[allow(dead_code)]
    pub fn settings_yes(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Yes",
            UiLang::ChineseSimplified => "是",
            UiLang::ChineseTraditional => "是",
        }
    }

    pub fn hotkey_display(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Registered Hotkeys",
            UiLang::ChineseSimplified => "已注册快捷键",
            UiLang::ChineseTraditional => "已註冊快捷鍵",
        }
    }

    pub fn theme_label(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Theme",
            UiLang::ChineseSimplified => "主题",
            UiLang::ChineseTraditional => "主題",
        }
    }

    // ── Startup panel ──
    pub fn startup_checking(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Checking model files...",
            UiLang::ChineseSimplified => "正在检查模型文件...",
            UiLang::ChineseTraditional => "正在檢查模型檔案...",
        }
    }

    pub fn startup_downloading(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Downloading model files...",
            UiLang::ChineseSimplified => "正在下载模型文件...",
            UiLang::ChineseTraditional => "正在下載模型檔案...",
        }
    }

    pub fn startup_extracting(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Extracting model package...",
            UiLang::ChineseSimplified => "正在解压模型包...",
            UiLang::ChineseTraditional => "正在解壓模型包...",
        }
    }

    pub fn startup_failed(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Model download failed",
            UiLang::ChineseSimplified => "模型下载失败",
            UiLang::ChineseTraditional => "模型下載失敗",
        }
    }

    pub fn startup_retry(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Retry",
            UiLang::ChineseSimplified => "重试",
            UiLang::ChineseTraditional => "重試",
        }
    }

    pub fn startup_continue_without_models(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Continue without models",
            UiLang::ChineseSimplified => "不下载直接启动",
            UiLang::ChineseTraditional => "不下載直接啟動",
        }
    }

    pub fn startup_hint(&self) -> &'static str {
        match self.lang {
            UiLang::English => "Models will be downloaded from GitHub. This may take a few minutes.",
            UiLang::ChineseSimplified => "模型将从 GitHub 下载，可能需要几分钟。",
            UiLang::ChineseTraditional => "模型將從 GitHub 下載，可能需要幾分鐘。",
        }
    }

    // ── Model status ──
    #[allow(dead_code)]
    pub fn settings_model_status(&self, ok: usize, total: usize) -> String {
        match self.lang {
            UiLang::English => format!("Model files: {}/{} available", ok, total),
            UiLang::ChineseSimplified => format!("模型文件：{}/{} 可用", ok, total),
            UiLang::ChineseTraditional => format!("模型檔案：{}/{} 可用", ok, total),
        }
    }

    /// Language display names for the combobox.
    #[allow(dead_code)]
    pub fn language_display_name(&self, code: &str) -> String {
        match code {
            "en" => match self.lang {
                UiLang::English => "English",
                UiLang::ChineseSimplified => "英文",
                UiLang::ChineseTraditional => "英文",
            },
            "zh" | "zh-CN" | "zh_Hans" => match self.lang {
                UiLang::English => "Chinese (Simplified)",
                UiLang::ChineseSimplified => "简体中文",
                UiLang::ChineseTraditional => "簡體中文",
            },
            "zh-TW" | "zh_Hant" => match self.lang {
                UiLang::English => "Chinese (Traditional)",
                UiLang::ChineseSimplified => "繁体中文",
                UiLang::ChineseTraditional => "繁體中文",
            },
            "ja" => match self.lang {
                UiLang::English => "Japanese",
                UiLang::ChineseSimplified => "日语",
                UiLang::ChineseTraditional => "日文",
            },
            "ko" => match self.lang {
                UiLang::English => "Korean",
                UiLang::ChineseSimplified => "韩语",
                UiLang::ChineseTraditional => "韓文",
            },
            "de" => match self.lang {
                UiLang::English => "German",
                UiLang::ChineseSimplified => "德语",
                UiLang::ChineseTraditional => "德文",
            },
            "fr" => match self.lang {
                UiLang::English => "French",
                UiLang::ChineseSimplified => "法语",
                UiLang::ChineseTraditional => "法文",
            },
            "es" => match self.lang {
                UiLang::English => "Spanish",
                UiLang::ChineseSimplified => "西班牙语",
                UiLang::ChineseTraditional => "西班牙文",
            },
            "it" => match self.lang {
                UiLang::English => "Italian",
                UiLang::ChineseSimplified => "意大利语",
                UiLang::ChineseTraditional => "義大利文",
            },
            "pt" => match self.lang {
                UiLang::English => "Portuguese",
                UiLang::ChineseSimplified => "葡萄牙语",
                UiLang::ChineseTraditional => "葡萄牙文",
            },
            "ru" => match self.lang {
                UiLang::English => "Russian",
                UiLang::ChineseSimplified => "俄语",
                UiLang::ChineseTraditional => "俄文",
            },
            "ar" => match self.lang {
                UiLang::English => "Arabic",
                UiLang::ChineseSimplified => "阿拉伯语",
                UiLang::ChineseTraditional => "阿拉伯文",
            },
            "hi" => match self.lang {
                UiLang::English => "Hindi",
                UiLang::ChineseSimplified => "印地语",
                UiLang::ChineseTraditional => "印地文",
            },
            "vi" => match self.lang {
                UiLang::English => "Vietnamese",
                UiLang::ChineseSimplified => "越南语",
                UiLang::ChineseTraditional => "越南文",
            },
            "th" => match self.lang {
                UiLang::English => "Thai",
                UiLang::ChineseSimplified => "泰语",
                UiLang::ChineseTraditional => "泰文",
            },
            "tr" => match self.lang {
                UiLang::English => "Turkish",
                UiLang::ChineseSimplified => "土耳其语",
                UiLang::ChineseTraditional => "土耳其文",
            },
            "nl" => match self.lang {
                UiLang::English => "Dutch",
                UiLang::ChineseSimplified => "荷兰语",
                UiLang::ChineseTraditional => "荷蘭文",
            },
            "pl" => match self.lang {
                UiLang::English => "Polish",
                UiLang::ChineseSimplified => "波兰语",
                UiLang::ChineseTraditional => "波蘭文",
            },
            "sv" => match self.lang {
                UiLang::English => "Swedish",
                UiLang::ChineseSimplified => "瑞典语",
                UiLang::ChineseTraditional => "瑞典文",
            },
            "el" => match self.lang {
                UiLang::English => "Greek",
                UiLang::ChineseSimplified => "希腊语",
                UiLang::ChineseTraditional => "希臘文",
            },
            "he" => match self.lang {
                UiLang::English => "Hebrew",
                UiLang::ChineseSimplified => "希伯来语",
                UiLang::ChineseTraditional => "希伯來文",
            },
            "id" => match self.lang {
                UiLang::English => "Indonesian",
                UiLang::ChineseSimplified => "印尼语",
                UiLang::ChineseTraditional => "印尼文",
            },
            "auto" => match self.lang {
                UiLang::English => "Auto-detect",
                UiLang::ChineseSimplified => "自动检测",
                UiLang::ChineseTraditional => "自動偵測",
            },
            _ => code,
        }
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── UiLang ───────────────────────────────────────────────────────

    #[test]
    fn test_ui_lang_from_code() {
        assert_eq!(UiLang::from_code("en"), UiLang::English);
        assert_eq!(UiLang::from_code("zh-CN"), UiLang::ChineseSimplified);
        assert_eq!(UiLang::from_code("zh_Hans"), UiLang::ChineseSimplified);
        assert_eq!(UiLang::from_code("zh-TW"), UiLang::ChineseTraditional);
        assert_eq!(UiLang::from_code("zh_Hant"), UiLang::ChineseTraditional);
        assert_eq!(UiLang::from_code("zh"), UiLang::ChineseTraditional); // backward compat
        assert_eq!(UiLang::from_code("hans"), UiLang::ChineseSimplified);
        assert_eq!(UiLang::from_code("hant"), UiLang::ChineseTraditional);
    }

    #[test]
    fn test_ui_lang_code_roundtrip() {
        assert_eq!(UiLang::English.code(), "en");
        assert_eq!(UiLang::ChineseSimplified.code(), "zh-CN");
        assert_eq!(UiLang::ChineseTraditional.code(), "zh-TW");
    }

    #[test]
    fn test_ui_lang_display_name() {
        assert_eq!(UiLang::English.display_name(), "English");
        assert_eq!(UiLang::ChineseSimplified.display_name(), "简体中文");
        assert_eq!(UiLang::ChineseTraditional.display_name(), "繁體中文");
    }

    #[test]
    fn test_ui_lang_default_is_traditional_chinese() {
        assert_eq!(UiLang::default(), UiLang::ChineseTraditional);
    }

    #[test]
    fn test_from_code_unknown_fallback() {
        assert_eq!(UiLang::from_code("fr"), UiLang::ChineseTraditional);
        assert_eq!(UiLang::from_code(""), UiLang::ChineseTraditional);
        assert_eq!(UiLang::from_code("xx"), UiLang::ChineseTraditional);
    }

    // ── Strings basic methods ────────────────────────────────────────

    #[test]
    fn test_strings_app_name_tri_lingual() {
        let en = Strings::new(UiLang::English);
        let cn = Strings::new(UiLang::ChineseSimplified);
        let tw = Strings::new(UiLang::ChineseTraditional);
        assert_eq!(en.app_name(), "Nemotron Voice Input");
        assert_eq!(cn.app_name(), "Nemotron 语音输入");
        assert_eq!(tw.app_name(), "Nemotron 語音輸入");
        assert_ne!(en.app_name(), cn.app_name());
        assert_ne!(cn.app_name(), tw.app_name());
    }

    #[test]
    fn test_strings_settings_title_tri_lingual() {
        let en = Strings::new(UiLang::English);
        let cn = Strings::new(UiLang::ChineseSimplified);
        let tw = Strings::new(UiLang::ChineseTraditional);
        assert_eq!(en.settings_title(), "Settings");
        assert_eq!(cn.settings_title(), "设置");
        assert_eq!(tw.settings_title(), "設定");
    }

    #[test]
    fn test_strings_tray_toggle_tri_lingual() {
        let en = Strings::new(UiLang::English);
        let cn = Strings::new(UiLang::ChineseSimplified);
        let tw = Strings::new(UiLang::ChineseTraditional);
        assert_eq!(en.tray_toggle_recording(), "Toggle Recording");
        assert_eq!(cn.tray_toggle_recording(), "切换录音");
        assert_eq!(tw.tray_toggle_recording(), "切換錄音");
    }

    #[test]
    fn test_strings_language_display_name_known_codes() {
        let en = Strings::new(UiLang::English);
        let cn = Strings::new(UiLang::ChineseSimplified);
        let tw = Strings::new(UiLang::ChineseTraditional);

        assert_eq!(en.language_display_name("en"), "English");
        assert_eq!(en.language_display_name("zh-CN"), "Chinese (Simplified)");
        assert_eq!(en.language_display_name("zh-TW"), "Chinese (Traditional)");
        assert_eq!(en.language_display_name("ja"), "Japanese");

        assert_eq!(cn.language_display_name("en"), "英文");
        assert_eq!(cn.language_display_name("zh-CN"), "简体中文");
        assert_eq!(cn.language_display_name("zh-TW"), "繁体中文");

        assert_eq!(tw.language_display_name("en"), "英文");
        assert_eq!(tw.language_display_name("zh-CN"), "簡體中文");
        assert_eq!(tw.language_display_name("zh-TW"), "繁體中文");

        // Unknown code should pass through
        assert_eq!(en.language_display_name("xx"), "xx");
    }

    #[test]
    fn test_startup_strings_tri_lingual() {
        let en = Strings::new(UiLang::English);
        let cn = Strings::new(UiLang::ChineseSimplified);
        let tw = Strings::new(UiLang::ChineseTraditional);

        assert_eq!(en.startup_checking(), "Checking model files...");
        assert_eq!(cn.startup_checking(), "正在检查模型文件...");
        assert_eq!(tw.startup_checking(), "正在檢查模型檔案...");
    }
}
