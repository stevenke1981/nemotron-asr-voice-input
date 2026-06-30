use crate::config::AppConfig;

/// Actions sent from the egui GUI back to the main Win32 thread.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum GuiAction {
    ToggleRecording,
    CycleLanguage,
    Flush,
    SetLanguage(String),
    SaveConfig(AppConfig),
    ShowOverlay(bool),
    DeleteHistoryEntry(usize),
    ClearHistory,
    Exit,
}

/// A single entry in the transcript history panel.
#[derive(Debug, Clone)]
pub struct TranscriptEntry {
    pub text: String,
    pub timestamp: String,
    #[allow(dead_code)]
    pub language: String,
}

/// Shared state snapshot sent from main thread to GUI.
#[derive(Debug, Clone, Default)]
pub struct GuiSnapshot {
    pub is_recording: bool,
    pub current_language: String,
    pub conversion_mode: String,
    pub latest_final_text: String,
    pub latest_partial_text: String,
    pub history: Vec<TranscriptEntry>,
    /// Set by main when tray "Settings" is clicked; consumed by GUI to open settings panel.
    pub show_settings_requested: bool,
    /// Set when tray Exit or GUI Exit is triggered; consumed by GUI to close the window.
    pub exit_requested: bool,
}

/// Status of the model check/download phase during startup.
#[derive(Debug, Clone)]
pub enum ModelStatus {
    /// Checking if model files already exist.
    Checking,
    /// Downloading: (current_bytes, total_bytes_estimate).
    Downloading(u64, u64),
    /// Extracting model package.
    Extracting,
    /// Download failed with an error message.
    Failed(String),
    /// Models are ready — normal UI can be shown.
    Ready,
}

impl Default for ModelStatus {
    fn default() -> Self {
        Self::Checking
    }
}
