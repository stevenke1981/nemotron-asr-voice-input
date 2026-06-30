use super::{InjectorError, TextInjector};
use windows::core::s;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationValuePattern, UIA_ValuePatternId,
};

/// Text injector using Windows UIAutomation ValuePattern.
///
/// This is the preferred strategy as it's faster and more reliable
/// for applications that support UIA (most modern Windows apps).
/// Falls through to SendInput if UIA is not available.
pub struct UiautomationInjector {
    available: bool,
}

impl UiautomationInjector {
    pub fn new() -> Self {
        let available = check_uia_available();
        Self { available }
    }
}

impl TextInjector for UiautomationInjector {
    fn inject_text(&mut self, text: &str) -> Result<(), InjectorError> {
        if !self.available {
            return Err(InjectorError::NotAvailable);
        }
        if text.is_empty() {
            return Ok(());
        }

        let uia = match create_uiautomation() {
            Some(uia) => uia,
            None => {
                self.available = false;
                return Err(InjectorError::NotAvailable);
            }
        };

        let result = inject_with_uia(&uia, text);
        // Drop the IUIAutomation before we might signal unavailability
        drop(uia);

        match result {
            Ok(()) => {
                tracing::debug!(
                    "UIAutomation: injected {} chars via ValuePattern",
                    text.chars().count()
                );
                Ok(())
            }
            Err(e) => {
                // If UIA fails once, mark as unavailable for future calls
                self.available = false;
                Err(e)
            }
        }
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

// ── Internal helpers ─────────────────────────────────────────────────

/// Check if UIAutomationCore.dll is available on this system.
fn check_uia_available() -> bool {
    unsafe { GetModuleHandleA(s!("UIAutomationCore.dll")).is_ok() }
}

/// Create an IUIAutomation instance. Returns None if COM/UIA is not available.
fn create_uiautomation() -> Option<IUIAutomation> {
    // Try to initialize COM (may already be initialized by the host)
    let com_init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let needs_com_uninit = com_init.is_ok();

    let result = unsafe {
        CoCreateInstance::<_, IUIAutomation>(
            &CUIAutomation,
            None, // No outer unknown
            CLSCTX_INPROC_SERVER,
        )
    };

    match result {
        Ok(uia) => {
            // Keep COM initialized for the lifetime of the IUIAutomation
            // (COM will be released when the IUIAutomation is dropped)
            Some(uia)
        }
        Err(e) => {
            tracing::warn!("Failed to create IUIAutomation: {}", e);
            if needs_com_uninit {
                unsafe {
                    CoUninitialize();
                }
            }
            None
        }
    }
}

/// Attempt to inject text using IUIAutomation ValuePattern on the focused element.
fn inject_with_uia(uia: &IUIAutomation, text: &str) -> Result<(), InjectorError> {
    unsafe {
        let focused = uia
            .GetFocusedElement()
            .map_err(|e| InjectorError::UiautomationFailed(format!("GetFocusedElement: {}", e)))?;

        // Try to get the ValuePattern from the focused element
        let value_pattern: IUIAutomationValuePattern = match focused.GetCurrentPatternAs(UIA_ValuePatternId)
        {
            Ok(p) => p,
            Err(e) => {
                return Err(InjectorError::UiautomationFailed(format!(
                    "No ValuePattern on focused element: {}",
                    e
                )));
            }
        };

        // Convert Rust string to BSTR
        let bstr = windows::core::BSTR::from(text);

        // Set the value
        value_pattern
            .SetValue(&bstr)
            .map_err(|e| InjectorError::UiautomationFailed(format!("SetValue failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uiautomation_injector_creation() {
        // In test environment, UIA may or may not be available.
        // Just verify the injector doesn't panic on construction.
        let injector = UiautomationInjector::new();
        // Note: `available` may be false in test/CI environments
        let _ = injector.available;
    }
}
