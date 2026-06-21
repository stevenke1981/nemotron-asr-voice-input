use std::collections::HashMap;
use tracing::{info, warn};
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
    /// The actually-registered (modifiers, vk) for each action.
    actual_keys: HashMap<HotkeyAction, (u32, u32)>,
}

impl HotkeyManager {
    /// Create a new hotkey manager.
    pub fn new() -> Self {
        Self {
            registered: HashMap::new(),
            actual_keys: HashMap::new(),
        }
    }

    /// Register a hotkey with the given modifier and virtual key.
    /// If the combination is already in use by another app, tries fallback
    /// combinations: F5, F6, F7, F8, F9, F10 (+same modifiers).
    pub fn register(
        &mut self,
        action: HotkeyAction,
        modifiers: u32,
        vk: u32,
    ) -> Result<(), String> {
        // Remove previous registration if any
        let _ = self.unregister(action);

        let id = action.id();
        let mods = HOT_KEY_MODIFIERS(modifiers);

        // Try the primary key first
        if let Err(e) = self.try_register(id, action, mods, vk) {
            warn!("Primary hotkey {} conflict: {}", format_hotkey(modifiers, vk), e);
            // Try fallback keys (function keys F5-F12 are rarely conflicted)
            let fallbacks = [0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B]; // F5..F12
            let mut fallback_ok = false;
            for &fallback_vk in &fallbacks {
                if let Ok(()) = self.try_register(id, action, mods, fallback_vk) {
                    info!("  → Registered with fallback key: {}", format_hotkey(modifiers, fallback_vk));
                    fallback_ok = true;
                    break;
                }
            }
            if !fallback_ok {
                return Err(format!(
                    "Failed to register hotkey {:?}: primary conflict and no fallback available",
                    action
                ));
            }
        }

        Ok(())
    }

    /// Internal: attempt a single registration.
    fn try_register(
        &mut self,
        id: i32,
        action: HotkeyAction,
        modifiers: HOT_KEY_MODIFIERS,
        vk: u32,
    ) -> Result<(), String> {
        unsafe {
            match RegisterHotKey(None, id, modifiers, vk) {
                Ok(()) => {
                    self.registered.insert(id, action);
                    self.actual_keys.insert(action, (modifiers.0, vk));
                    info!("Registered hotkey {:?} (mods={:#x}, vk={:#x})", action, modifiers.0, vk);
                    Ok(())
                }
                Err(e) => {
                    Err(format!("{} (code={})", e, e.code().0))
                }
            }
        }
    }

    /// Get the (modifiers, vk) that were actually registered for an action.
    pub fn actual_key(&self, action: HotkeyAction) -> Option<(u32, u32)> {
        self.actual_keys.get(&action).copied()
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
        self.actual_keys.clear();
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

/// Convert Windows VK code + modifier flags to a human-readable hotkey string.
/// Example: (0x4007, 0x71) → "Ctrl+Alt+Shift+F2"
pub fn format_hotkey(modifiers: u32, vk: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    if modifiers & 0x0002 != 0 { parts.push("Ctrl".into()); }
    if modifiers & 0x0001 != 0 { parts.push("Alt".into()); }
    if modifiers & 0x0004 != 0 { parts.push("Shift".into()); }
    if modifiers & 0x0008 != 0 { parts.push("Win".into()); }

    match vk {
        0x08 => parts.push("Backspace".into()),
        0x09 => parts.push("Tab".into()),
        0x0D => parts.push("Enter".into()),
        0x1B => parts.push("Escape".into()),
        0x20 => parts.push("Space".into()),
        0x21 => parts.push("PageUp".into()),
        0x22 => parts.push("PageDown".into()),
        0x23 => parts.push("End".into()),
        0x24 => parts.push("Home".into()),
        0x25 => parts.push("Left".into()),
        0x26 => parts.push("Up".into()),
        0x27 => parts.push("Right".into()),
        0x28 => parts.push("Down".into()),
        0x2E => parts.push("Delete".into()),
        0x6A => parts.push("*".into()),
        0x6B => parts.push("+".into()),
        0x6D => parts.push("-".into()),
        0x6E => parts.push(".".into()),
        0x6F => parts.push("/".into()),
        0x90 => parts.push("NumLock".into()),
        0x91 => parts.push("ScrollLock".into()),
        0x70..=0x87 => {
            parts.push(format!("F{}", vk - 0x70 + 1));
        }
        0x30..=0x39 => {
            parts.push(format!("{}", (b'0' + (vk - 0x30) as u8) as char));
        }
        0x41..=0x5A => {
            parts.push(format!("{}", (b'A' + (vk - 0x41) as u8) as char));
        }
        _ => {
            parts.push(format!("VK=0x{:X}", vk));
        }
    }

    parts.join("+")
}
