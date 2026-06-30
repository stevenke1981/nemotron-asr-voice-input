use super::{InjectorError, TextInjector};
use tracing::debug;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::GetMessageExtraInfo;
use windows::Win32::Foundation::HANDLE;

/// Clipboard format: Unicode text (CF_UNICODETEXT = 13).
const CF_UNICODETEXT: u32 = 13;

/// Text injector using clipboard + Ctrl+V.
///
/// Strategy:
/// 1. Open clipboard, empty it, set new text as CF_UNICODETEXT.
/// 2. Send Ctrl+V to paste into the focused window.
pub struct ClipboardInjector {
    available: bool,
}

impl ClipboardInjector {
    pub fn new() -> Self {
        Self { available: true }
    }
}

impl TextInjector for ClipboardInjector {
    fn inject_text(&mut self, text: &str) -> Result<(), InjectorError> {
        if !self.available {
            return Err(InjectorError::NotAvailable);
        }
        if text.is_empty() {
            return Ok(());
        }

        // Open clipboard and set text
        unsafe {
            if OpenClipboard(None).is_err() {
                return Err(InjectorError::ClipboardFailed(
                    "OpenClipboard failed".into(),
                ));
            }

            let set_result = (|| -> Result<(), InjectorError> {
                let _ = EmptyClipboard();

                // Allocate global memory for UTF-16 text
                let utf16: Vec<u16> = text.encode_utf16().collect();
                let byte_size = (utf16.len() + 1) * 2; // include null terminator
                let h_global = GlobalAlloc(GHND, byte_size);

                if h_global.is_err() {
                    return Err(InjectorError::ClipboardFailed(
                        "GlobalAlloc failed".into(),
                    ));
                }
                let h_global = h_global.unwrap();

                let locked = GlobalLock(h_global);
                if locked.is_null() {
                    let _ = CloseClipboard();
                    return Err(InjectorError::ClipboardFailed(
                        "GlobalLock failed".into(),
                    ));
                }

                // Copy UTF-16 data into global memory
                let dst = locked as *mut u16;
                std::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len());
                dst.add(utf16.len()).write(0); // null terminator

                let _ = GlobalUnlock(h_global);

                // Set clipboard data (take ownership of HGLOBAL)
                if SetClipboardData(CF_UNICODETEXT, Some(HANDLE(h_global.0))).is_err() {
                    let _ = CloseClipboard();
                    return Err(InjectorError::ClipboardFailed(
                        "SetClipboardData failed".into(),
                    ));
                }

                Ok(())
            })();

            let _ = CloseClipboard();

            if let Err(e) = set_result {
                return Err(e);
            }
        }

        // Send Ctrl+V paste
        send_ctrl_v()?;

        debug!(
            "ClipboardInjector: injected {} chars via clipboard",
            text.chars().count()
        );
        Ok(())
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

// ── Helpers ──────────────────────────────────────────────────────────

/// Send Ctrl+V (VK_CONTROL down, 'V' down/up, VK_CONTROL up).
fn send_ctrl_v() -> Result<(), InjectorError> {
    unsafe {
        let extra = GetMessageExtraInfo();

        let ctrl_down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0x11), // VK_CONTROL
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0), // key down
                    time: 0,
                    dwExtraInfo: extra.0 as usize,
                },
            },
        };
        let v_down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0x56), // V
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: extra.0 as usize,
                },
            },
        };
        let v_up = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0x56),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: extra.0 as usize,
                },
            },
        };
        let ctrl_up = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0x11),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: extra.0 as usize,
                },
            },
        };

        let inputs = [ctrl_down, v_down, v_up, ctrl_up];
        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if sent as usize != inputs.len() {
            return Err(InjectorError::SendInputFailed(format!(
                "Ctrl+V: only sent {} of {} inputs",
                sent,
                inputs.len()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_injector_empty_text() {
        let mut injector = ClipboardInjector::new();
        assert!(injector.is_available());
        assert!(injector.inject_text("").is_ok());
    }
}
