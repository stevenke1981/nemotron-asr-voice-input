use std::collections::HashMap;
use tracing::info;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};

/// Identifier for a registered hotkey action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    ToggleRecording,
    CycleLanguage,
    Flush,
}

impl HotkeyAction {
    pub fn id(&self) -> i32 {
        match self {
            Self::ToggleRecording => 1,
            Self::CycleLanguage => 2,
            Self::Flush => 3,
        }
    }
}

/// Manager for Windows global hotkeys.
pub struct HotkeyManager {
    registered: HashMap<i32, HotkeyAction>,
}

impl HotkeyManager {
    /// Create a new hotkey manager.
    pub fn new() -> Self {
        Self {
            registered: HashMap::new(),
        }
    }

    /// Register a hotkey with the given modifier and virtual key.
    pub fn register(
        &mut self,
        action: HotkeyAction,
        modifiers: u32,
        vk: u32,
    ) -> Result<(), String> {
        let id = action.id();

        // Remove previous registration if any
        let _ = self.unregister(action);

        let mods = HOT_KEY_MODIFIERS(modifiers);

        unsafe {
            if RegisterHotKey(None, id, mods, vk).is_ok() {
                self.registered.insert(id, action);
                info!("Registered hotkey {:?} (mods={:#x}, vk={:#x})", action, modifiers, vk);
                Ok(())
            } else {
                Err(format!("Failed to register hotkey {:?}", action))
            }
        }
    }

    /// Unregister a hotkey.
    pub fn unregister(&mut self, action: HotkeyAction) -> Result<(), String> {
        let id = action.id();
        if self.registered.remove(&id).is_some() {
            unsafe {
                if UnregisterHotKey(None, id).is_ok() {
                    info!("Unregistered hotkey {:?}", action);
                    Ok(())
                } else {
                    Err(format!("Failed to unregister hotkey {:?}", action))
                }
            }
        } else {
            Ok(())
        }
    }

    /// Unregister all hotkeys.
    pub fn unregister_all(&mut self) {
        for (&id, &action) in &self.registered {
            unsafe {
                let _ = UnregisterHotKey(None, id);
            }
            info!("Unregistered hotkey {:?}", action);
        }
        self.registered.clear();
    }

    /// Process incoming Windows messages and return the action if a hotkey was pressed.
    /// This should be called in the main message loop.
    pub fn process_message(&self, msg: &windows::Win32::UI::WindowsAndMessaging::MSG) -> Option<HotkeyAction> {
        const WM_HOTKEY: u32 = 0x0312;
        if msg.message == WM_HOTKEY {
            let id = msg.wParam.0 as i32;
            let action = self.registered.get(&id).copied();
            tracing::trace!("WM_HOTKEY id={} -> {:?}", id, action);
            action
        } else {
            None
        }
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        self.unregister_all();
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new()
    }
}
