/// System tray icon and context menu implementation using Win32 API.
/// Uses Shell_NotifyIconW, NOTIFYICONDATAW, and context menu.
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;
use tracing::{info, warn};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::strings::{Strings, UiLang};

/// WM_APP + 1 — custom message sent by Shell_NotifyIcon to our hidden window.
const WM_TRAYICON: u32 = WM_APP + 1;

/// Our tray icon unique ID within the hidden window.
const TRAY_ICON_ID: u32 = 1;

// ── Context menu item IDs ────────────────────────────────────────────
const IDM_TOGGLE_RECORDING: u32 = 1001;
const IDM_CYCLE_LANGUAGE: u32 = 1002;
const IDM_FLUSH: u32 = 1004;
const IDM_EXIT: u32 = 1003;
const IDM_SETTINGS: u32 = 1005;

// ── Tray action channel ──────────────────────────────────────────────

/// Actions that the tray sends to the main loop.
#[derive(Debug, Clone)]
pub enum TrayAction {
    ToggleRecording,
    CycleLanguage,
    Flush,
    OpenSettings,
    Exit,
}

static TRAY_TX: OnceLock<crossbeam::channel::Sender<TrayAction>> = OnceLock::new();

/// UI language for tray context menu (set from config).
static UI_LANG: AtomicU8 = AtomicU8::new(0);

/// Set the UI language for the tray context menu.
pub fn set_ui_lang(code: &str) {
    match code {
        "zh" => UI_LANG.store(1, Ordering::Relaxed),
        _ => UI_LANG.store(0, Ordering::Relaxed),
    }
}

/// Get the current UI strings for the tray.
pub fn tray_strings() -> Strings {
    let lang = match UI_LANG.load(Ordering::Relaxed) {
        1 => UiLang::Chinese,
        _ => UiLang::English,
    };
    Strings::new(lang)
}

/// Get a clone of the tray sender channel (for use in the window proc).
pub fn tray_sender() -> Option<crossbeam::channel::Sender<TrayAction>> {
    TRAY_TX.get().cloned()
}

/// Set the tray sender channel (called once from main).
pub fn set_tray_sender(tx: crossbeam::channel::Sender<TrayAction>) -> Result<(), String> {
    TRAY_TX
        .set(tx)
        .map_err(|_| "TRAY_TX already set".to_string())
}

/// Send a balloon notification from anywhere (used by config window).
pub fn send_tray_notification(title: &str, message: &str) {
    let hwnd = HIDDEN_HWND.load(Ordering::Relaxed);
    if hwnd == 0 {
        return;
    }
    let hwnd = HWND(hwnd as *mut _);

    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_ID;
        nid.uFlags = NIF_INFO;

        let msg_wide = message.encode_utf16().chain(std::iter::once(0));
        for (i, c) in msg_wide.enumerate() {
            if i >= 256 {
                break;
            }
            nid.szInfo[i] = c;
        }

        let title_wide = title.encode_utf16().chain(std::iter::once(0));
        for (i, c) in title_wide.enumerate() {
            if i >= 64 {
                break;
            }
            nid.szInfoTitle[i] = c;
        }

        nid.dwInfoFlags = NIIF_INFO;
        nid.Anonymous.uTimeout = 5000;

        let _ = Shell_NotifyIconW(NIM_MODIFY, &mut nid);
    }
}

/// Store the hidden window HWND for notification access.
static HIDDEN_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

// ── Window procedure for the hidden tray window ──────────────────────

/// The window procedure that handles tray callbacks and context menu.
unsafe extern "system" fn tray_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let lp = lparam.0 as u32;
            match lp {
                WM_LBUTTONUP | WM_RBUTTONUP => {
                    unsafe { show_context_menu(hwnd); }
                }
                NIN_BALLOONUSERCLICK => {
                    if let Some(tx) = tray_sender() {
                        let _ = tx.send(TrayAction::ToggleRecording);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            // LOWORD of wparam
            let id = (wparam.0 as u32) & 0xFFFF;
            match id {
                IDM_TOGGLE_RECORDING => {
                    if let Some(tx) = tray_sender() {
                        let _ = tx.send(TrayAction::ToggleRecording);
                    }
                }
                IDM_CYCLE_LANGUAGE => {
                    if let Some(tx) = tray_sender() {
                        let _ = tx.send(TrayAction::CycleLanguage);
                    }
                }
                IDM_FLUSH => {
                    if let Some(tx) = tray_sender() {
                        let _ = tx.send(TrayAction::Flush);
                    }
                }
                IDM_SETTINGS => {
                    if let Some(tx) = tray_sender() {
                        let _ = tx.send(TrayAction::OpenSettings);
                    }
                }
                IDM_EXIT => {
                    if let Some(tx) = tray_sender() {
                        let _ = tx.send(TrayAction::Exit);
                    }
                }
                _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0); }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Show the context menu at the current cursor position, using bilingual strings.
unsafe fn show_context_menu(hwnd: HWND) {
    let s = tray_strings();
    let menu = match unsafe { CreatePopupMenu() } {
        Ok(m) => m,
        Err(_) => return,
    };

    unsafe {
        append_menu_item(menu, MENU_ITEM_FLAGS(0), IDM_TOGGLE_RECORDING, s.tray_toggle_recording());
        append_menu_item(menu, MENU_ITEM_FLAGS(0), IDM_CYCLE_LANGUAGE, s.tray_cycle_language());
        append_menu_item(menu, MENU_ITEM_FLAGS(0), IDM_FLUSH, s.tray_flush());
        AppendMenuW(menu, MENU_ITEM_FLAGS(0x800), 0, w!("")).ok();
        append_menu_item(menu, MENU_ITEM_FLAGS(0), IDM_SETTINGS, s.tray_settings());
        AppendMenuW(menu, MENU_ITEM_FLAGS(0x800), 0, w!("")).ok();
        append_menu_item(menu, MENU_ITEM_FLAGS(0), IDM_EXIT, s.tray_exit());
    }

    let mut pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);

        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_LEFTALIGN,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        );

        let _ = DestroyMenu(menu);
    }
}

/// Append a menu item with Unicode text.
unsafe fn append_menu_item(menu: HMENU, flags: MENU_ITEM_FLAGS, id: u32, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = AppendMenuW(menu, flags, id as usize, PCWSTR(wide.as_ptr()));
    }
}

// ── Icon creation helpers ────────────────────────────────────────────

/// Create a simple 16×16 icon filled with the given color using GDI.
unsafe fn create_icon(r: u8, g: u8, b: u8) -> HICON {
    let and_mask = [0u8; 32];
    let mut xor_mask = [0u8; 1024];

    for row in 0..16u32 {
        for col in 0..16u32 {
            let offset = ((row * 16 + col) * 4) as usize;
            let dx = col as i32 - 7;
            let dy = row as i32 - 7;
            if dx * dx + dy * dy <= 49 {
                xor_mask[offset] = b;
                xor_mask[offset + 1] = g;
                xor_mask[offset + 2] = r;
                xor_mask[offset + 3] = 255;
            }
        }
    }

    match unsafe {
        CreateIcon(
            None,
            16,
            16,
            1,
            32,
            and_mask.as_ptr(),
            xor_mask.as_ptr(),
        )
    } {
        Ok(icon) => icon,
        Err(e) => {
            warn!("Failed to create tray icon: {}", e);
            unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON(std::ptr::null_mut())) }
        }
    }
}

// ── TrayManager ──────────────────────────────────────────────────────

/// Manages the system tray icon, context menu, and balloon notifications.
pub struct TrayManager {
    hwnd: HWND,
    icon_initialized: bool,
}

impl TrayManager {
    pub fn new() -> Self {
        Self {
            hwnd: HWND(std::ptr::null_mut()),
            icon_initialized: false,
        }
    }

    /// Initialize: create hidden window, register icon, set up the tray sender.
    pub fn initialize(&mut self, action_tx: crossbeam::channel::Sender<TrayAction>) -> Result<(), String> {
        set_tray_sender(action_tx).ok();
        self.create_hidden_window()?;
        self.add_tray_icon()?;
        self.icon_initialized = true;
        info!("System tray initialized");
        Ok(())
    }

    /// Register the window class and create a hidden window.
    fn create_hidden_window(&mut self) -> Result<(), String> {
        unsafe {
            let hinstance = GetModuleHandleA(None)
                .map_err(|e| format!("GetModuleHandleA failed: {}", e))?;

            let class_name = w!("NemotronTrayWndCls");

            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(tray_wndproc),
                hInstance: hinstance.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };

            let atom = RegisterClassW(&wc);
            if atom == 0 {
                warn!("RegisterClassW returned 0 (may already be registered)");
            }

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                w!("NemotronTrayWindow"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .map_err(|e| format!("CreateWindowExW failed: {}", e))?;

            self.hwnd = hwnd;
            HIDDEN_HWND.store(hwnd.0 as isize, std::sync::atomic::Ordering::Relaxed);
            info!("Hidden tray window created (HWND={:?})", hwnd);
            Ok(())
        }
    }

    /// Add the tray icon via Shell_NotifyIconW.
    fn add_tray_icon(&mut self) -> Result<(), String> {
        unsafe {
            let icon = create_icon(64, 128, 255);

            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = TRAY_ICON_ID;
            nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;
            nid.hIcon = icon;

            let tip = "Nemotron Voice Input\0";
            for (i, c) in tip.encode_utf16().enumerate() {
                if i < 128 {
                    nid.szTip[i] = c;
                }
            }

            let result = Shell_NotifyIconW(NIM_ADD, &mut nid);
            if !result.as_bool() {
                return Err("Shell_NotifyIconW NIM_ADD failed".into());
            }

            info!("Tray icon added");
            Ok(())
        }
    }

    /// Show a balloon notification (NIF_INFO).
    pub fn show_notification(&self, title: &str, message: &str) {
        if !self.icon_initialized {
            return;
        }

        unsafe {
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = TRAY_ICON_ID;
            nid.uFlags = NIF_INFO;

            let msg_wide = message.encode_utf16().chain(std::iter::once(0));
            for (i, c) in msg_wide.enumerate() {
                if i >= 256 {
                    break;
                }
                nid.szInfo[i] = c;
            }

            let title_wide = title.encode_utf16().chain(std::iter::once(0));
            for (i, c) in title_wide.enumerate() {
                if i >= 64 {
                    break;
                }
                nid.szInfoTitle[i] = c;
            }

            nid.dwInfoFlags = NIIF_INFO;
            nid.Anonymous.uTimeout = 5000;

            let _ = Shell_NotifyIconW(NIM_MODIFY, &mut nid);
        }
    }

    /// Update the tray icon and tooltip to reflect recording state.
    pub fn set_recording_state(&self, is_recording: bool) {
        if !self.icon_initialized {
            return;
        }

        unsafe {
            let icon = if is_recording {
                create_icon(0, 200, 0)
            } else {
                create_icon(64, 128, 255)
            };

            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = TRAY_ICON_ID;
            nid.uFlags = NIF_ICON | NIF_TIP;
            nid.hIcon = icon;

            let s = tray_strings();
            let tip = if is_recording {
                s.tray_tip_recording()
            } else {
                s.tray_tip_idle()
            };
            let tip_wide: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
            for (i, c) in tip_wide.iter().enumerate() {
                if i < 128 {
                    nid.szTip[i] = *c;
                }
            }

            let _ = Shell_NotifyIconW(NIM_MODIFY, &mut nid);
        }
    }

    /// Return the HWND of the hidden window.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }
}

impl Drop for TrayManager {
    fn drop(&mut self) {
        if !self.icon_initialized {
            return;
        }
        unsafe {
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = TRAY_ICON_ID;
            let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
            info!("Tray icon removed");

            if !self.hwnd.0.is_null() {
                let _ = DestroyWindow(self.hwnd);
            }
            info!("Tray window destroyed");
        }
    }
}

impl Default for TrayManager {
    fn default() -> Self {
        Self::new()
    }
}
