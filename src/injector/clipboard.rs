use super::{InjectorError, TextInjector};
use tracing::debug;

/// Text injector using clipboard + Ctrl+V fallback.
/// For MVP, this reports as not available, falling through to SendInput.
/// A full clipboard implementation requires proper Win32 HGLOBAL/HANDLE
/// handling and will be implemented in Phase 2.
pub struct ClipboardInjector {
    available: bool,
}

impl ClipboardInjector {
    pub fn new() -> Self {
        Self { available: false }
    }
}

impl TextInjector for ClipboardInjector {
    fn inject_text(&mut self, _text: &str) -> Result<(), InjectorError> {
        if !self.available {
            return Err(InjectorError::NotAvailable);
        }
        // Full clipboard implementation TBD in Phase 2
        debug!("Clipboard injection not yet implemented");
        Err(InjectorError::NotAvailable)
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

impl Default for ClipboardInjector {
    fn default() -> Self {
        Self::new()
    }
}
