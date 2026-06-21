use super::{InjectorError, TextInjector};
use tracing::debug;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::GetMessageExtraInfo;

/// Text injector using Win32 SendInput with Unicode support.
pub struct SendInputInjector {
    available: bool,
}

impl SendInputInjector {
    pub fn new() -> Self {
        Self { available: true }
    }
}

impl TextInjector for SendInputInjector {
    fn inject_text(&mut self, text: &str) -> Result<(), InjectorError> {
        debug!("SendInput injecting {} chars", text.chars().count());

        // Build inputs for each Unicode character
        let mut inputs: Vec<INPUT> = Vec::with_capacity(text.chars().count() * 2);

        for ch in text.chars() {
            let code_unit = ch as u16;

            let extra_info = unsafe { GetMessageExtraInfo() };

            // KEYEVENTF_UNICODE: key down event
            let key_down = KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: code_unit,
                dwFlags: KEYEVENTF_UNICODE,
                time: 0,
                dwExtraInfo: extra_info.0 as usize,
            };

            // KEYEVENTF_UNICODE | KEYEVENTF_KEYUP: key up event
            let key_up = KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: code_unit,
                dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: extra_info.0 as usize,
            };

            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: key_down,
                },
            });
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: key_up,
                },
            });
        }

        unsafe {
            let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            if sent as usize != inputs.len() {
                return Err(InjectorError::SendInputFailed(format!(
                    "Only sent {} of {} inputs",
                    sent,
                    inputs.len()
                )));
            }
        }

        debug!("SendInput: successfully injected {} chars", text.chars().count());
        Ok(())
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

impl Default for SendInputInjector {
    fn default() -> Self {
        Self::new()
    }
}
