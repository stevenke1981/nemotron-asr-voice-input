use super::{InjectorError, TextInjector};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::core::s;

/// Text injector using Windows UIAutomation ValuePattern.
/// This is the preferred strategy as it's faster and more reliable
/// for applications that support UIA (most modern Windows apps).
pub struct UiautomationInjector {
    available: bool,
}

impl UiautomationInjector {
    pub fn new() -> Self {
        // Check if UIA is available on this system
        let available = uia_supports_automation();
        Self { available }
    }
}

impl TextInjector for UiautomationInjector {
    fn inject_text(&mut self, _text: &str) -> Result<(), InjectorError> {
        if !self.available {
            return Err(InjectorError::NotAvailable);
        }

        // For now, fall back to a simpler approach.
        // Full UIAutomation implementation requires COM interop
        // which is complex. We mark it as not available for MVP
        // and let the composite injector fall through to SendInput.
        self.available = false;
        Err(InjectorError::NotAvailable)
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

impl Default for UiautomationInjector {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if UIAutomation is supported on this system.
///
/// # Safety
/// Calls Win32 API.
fn uia_supports_automation() -> bool {
    unsafe {
        GetModuleHandleA(s!("UIAutomationCore.dll")).is_ok()
    }
}
