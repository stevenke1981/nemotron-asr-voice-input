use tracing::info;

/// System tray icon and context menu manager.
/// For MVP, this is a stub that will be fully implemented in Phase 2.
pub struct TrayManager {
    initialized: bool,
}

impl TrayManager {
    /// Create a new tray manager.
    pub fn new() -> Self {
        Self { initialized: false }
    }

    /// Initialize the system tray icon.
    pub fn initialize(&mut self) -> Result<(), String> {
        info!("Tray manager initialized (MVP stub)");
        self.initialized = true;
        Ok(())
    }

    /// Show a balloon notification.
    pub fn show_notification(&self, title: &str, message: &str) {
        info!("Tray notification: [{}] {}", title, message);
        // TODO: Phase 2 - implement actual balloon notification
    }

    /// Check if initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Set the current recording state icon.
    pub fn set_recording_state(&self, _is_recording: bool) {
        // TODO: Phase 2 - implement tray icon change
    }
}

impl Default for TrayManager {
    fn default() -> Self {
        Self::new()
    }
}
