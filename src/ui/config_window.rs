/// Settings dialog window — modeless Win32 window with bilingual Chinese/English UI.
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::atomic::AtomicIsize;
use std::sync::{Mutex, OnceLock};
use tracing::{info, warn};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::{GetSysColorBrush, COLOR_WINDOW};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::strings::{Strings, UiLang};
use crate::config::AppConfig;

/// Shared config so settings changes persist across window re-opens.
static SHARED_CONFIG: OnceLock<Mutex<AppConfig>> = OnceLock::new();

/// Initialize the shared config (call once from main).
pub fn init_shared_config(config: AppConfig) {
    let _ = SHARED_CONFIG.set(Mutex::new(config));
}

/// Load the current config from the shared mutex (falls back to `AppConfig::default()`).
fn load_shared_config() -> AppConfig {
    match SHARED_CONFIG.get() {
        Some(m) => m.lock().ok().map(|g| g.clone()).unwrap_or_default(),
        None => AppConfig::default(),
    }
}

/// Update the shared config (called from on_save).
fn store_shared_config(config: &AppConfig) {
    if let Some(m) = SHARED_CONFIG.get() {
        if let Ok(mut guard) = m.lock() {
            *guard = config.clone();
        }
    }
}

// ── Control IDs ───────────────────────────────────────────────────────
const IDC_UI_LANG: u32 = 2001;
const IDC_ASR_LANG: u32 = 2002;
const IDC_PROVIDER: u32 = 2003;
const IDC_DECODING: u32 = 2004;
const IDC_THREADS: u32 = 2005;
const IDC_VAD: u32 = 2006;
const IDC_STRATEGY: u32 = 2007;
const IDC_KEY_DELAY: u32 = 2008;
const IDC_RESTORE_CLIP: u32 = 2009;
const IDC_SAVE: u32 = 2010;
const IDC_CANCEL: u32 = 2011;

// ── Button/Control style constants (windows 0.62 uses i32 for these) ──
const BS_GROUPBOX: i32 = 0x0000_0007;
const BS_AUTOCHECKBOX: i32 = 0x0000_0003;
const BS_PUSHBUTTON: i32 = 0x0000_0000;
const CBS_DROPDOWNLIST: i32 = 0x0000_0003;
const ES_LEFT: i32 = 0x0000_0000;
const ES_NUMBER: i32 = 0x0000_2000;
const SS_LEFT: i32 = 0x0000_0000;

// ── ComboBox / Button messages ────────────────────────────────────────
const CB_ADDSTRING: u32 = 0x0143;
const CB_SETCURSEL: u32 = 0x014E;
const CB_GETCURSEL: u32 = 0x0147;
const CB_GETLBTEXTLEN: u32 = 0x0149;
const CB_GETLBTEXT: u32 = 0x0148;
const CB_GETCOUNT: u32 = 0x0146;
const BM_SETCHECK: u32 = 0x00F1;
const BM_GETCHECK: u32 = 0x00F0;

/// Guard to prevent opening the settings window twice.
static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);
/// Store HWND as isize (isize is Send+Sync).
static CONFIG_HWND: AtomicIsize = AtomicIsize::new(0);

// ── Helpers ───────────────────────────────────────────────────────────

/// Build a WINDOW_STYLE from the base WS_* flags and i32-style flags.
fn ws(base: WINDOW_STYLE, extra: i32) -> WINDOW_STYLE {
    WINDOW_STYLE(base.0 | extra as u32)
}

/// Send a window message (windows 0.62 requires Option<WPARAM/LPARAM>).
#[inline]
fn send(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { SendMessageW(hwnd, msg, Some(wparam), Some(lparam)) }
}

// ── Window procedure ─────────────────────────────────────────────────

unsafe extern "system" fn config_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT { unsafe {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 as u32) & 0xFFFF;
            let code = (wparam.0 as u32) >> 16;
            match id {
                IDC_SAVE if code == 0 => {
                    on_save(hwnd);
                    LRESULT(0)
                }
                IDC_CANCEL if code == 0 => {
                    destroy(hwnd);
                    LRESULT(0)
                }
                IDC_UI_LANG if code == 1 => {
                    on_ui_lang_changed(hwnd);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
        WM_CLOSE => {
            destroy(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            SETTINGS_OPEN.store(false, Ordering::SeqCst);
            CONFIG_HWND.store(0, Ordering::SeqCst);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                let _ = Box::from_raw(ptr as *mut ConfigDialogData);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}}

// ── Context data ──────────────────────────────────────────────────────

struct ConfigDialogData {
    config: AppConfig,
    strings: Strings,
}

// ── Public entry point ────────────────────────────────────────────────

pub fn show_config_window(parent_hwnd: HWND, _config: &AppConfig) {
    if SETTINGS_OPEN.load(Ordering::SeqCst) {
        let hwnd = CONFIG_HWND.load(Ordering::SeqCst);
        if hwnd != 0 {
            unsafe {
                let _ = SetForegroundWindow(HWND(hwnd as *mut _));
            }
        }
        return;
    }

    // Always load from shared config so saved changes are reflected
    let config = load_shared_config();
    let strings = Strings::new(UiLang::from_code(&config.ui.language));
    create_window(parent_hwnd, config, strings);
}

fn create_window(parent: HWND, config: AppConfig, strings: Strings) { unsafe {
    let hinstance = match GetModuleHandleA(None) {
        Ok(h) => h,
        Err(e) => { warn!("GetModuleHandle failed: {}", e); return; }
    };

    let class_name = w!("NemotronConfigWndCls");
    let brush = GetSysColorBrush(COLOR_WINDOW);
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(config_wndproc),
        hInstance: hinstance.into(),
        hbrBackground: brush,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    RegisterClassW(&wc);

    let title = format!("{} - {}", strings.app_name(), strings.settings_title());
    let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();

    let hwnd = match CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        class_name,
        PCWSTR(title_wide.as_ptr()),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
        CW_USEDEFAULT, CW_USEDEFAULT, 460, 570,
        Some(parent),
        None,
        Some(hinstance.into()),
        None,
    ) {
        Ok(w) => w,
        Err(e) => { warn!("Failed to create config window: {}", e); return; }
    };

    create_controls(hwnd, &config, &strings);

    let data = Box::new(ConfigDialogData { config, strings });
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(data) as isize);

    SETTINGS_OPEN.store(true, Ordering::SeqCst);
    CONFIG_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

    let _ = ShowWindow(hwnd, SW_SHOW);
    info!("Config window opened");
}}

// ── Control creation helpers ─────────────────────────────────────────

fn model_status(config: &AppConfig) -> (usize, usize) {
    let total = 6usize;
    let ok = match std::fs::read_dir(&config.model_dir) {
        Ok(entries) => entries.filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "onnx" || ext == "json" || ext == "txt"))
            .count(),
        Err(_) => 0,
    };
    (ok, total)
}

fn add_static(hwnd: HWND, text: &str, x: i32, y: i32, w: i32, h: i32) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(), w!("Static"), PCWSTR(wide.as_ptr()),
            ws(WS_CHILD | WS_VISIBLE, SS_LEFT),
            x, y, w, h, Some(hwnd), None, None, None,
        );
    }
}

fn add_groupbox(hwnd: HWND, text: &str, x: i32, y: i32, w: i32, h: i32) -> i32 {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(), w!("Button"), PCWSTR(wide.as_ptr()),
            ws(WS_CHILD | WS_VISIBLE, BS_GROUPBOX),
            x, y, w, h, Some(hwnd), None, None, None,
        );
    }
    y + h
}

fn add_combobox(hwnd: HWND, id: u32, x: i32, y: i32, w: i32, drop_h: i32) -> HWND {
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(), w!("ComboBox"), PCWSTR::null(),
            ws(WS_CHILD | WS_VISIBLE | WS_TABSTOP, CBS_DROPDOWNLIST),
            x, y, w, drop_h, Some(hwnd),
            Some(HMENU(id as isize as *mut _)), None, None,
        )
        .unwrap_or(HWND(ptr::null_mut()))
    }
}

fn add_edit(hwnd: HWND, id: u32, x: i32, y: i32, w: i32, h: i32) -> HWND {
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(), w!("Edit"), PCWSTR::null(),
            ws(WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER, ES_LEFT | ES_NUMBER),
            x, y, w, h, Some(hwnd),
            Some(HMENU(id as isize as *mut _)), None, None,
        )
        .unwrap_or(HWND(ptr::null_mut()))
    }
}

fn add_checkbox(hwnd: HWND, id: u32, text: &str, x: i32, y: i32, w: i32, h: i32, checked: bool) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let chk = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(), w!("Button"), PCWSTR(wide.as_ptr()),
            ws(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_AUTOCHECKBOX),
            x, y, w, h, Some(hwnd),
            Some(HMENU(id as isize as *mut _)), None, None,
        )
        .unwrap_or(HWND(ptr::null_mut()))
    };
    send(chk, BM_SETCHECK, WPARAM(if checked { 1 } else { 0 }), LPARAM(0));
}

fn add_button(hwnd: HWND, id: u32, text: &str, x: i32, y: i32, w: i32, h: i32) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(), w!("Button"), PCWSTR(wide.as_ptr()),
            ws(WS_CHILD | WS_VISIBLE | WS_TABSTOP, BS_PUSHBUTTON),
            x, y, w, h, Some(hwnd),
            Some(HMENU(id as isize as *mut _)), None, None,
        );
    }
}

// ── ComboBox helpers ──────────────────────────────────────────────────

fn combo_add(combo: HWND, items: &[&str]) {
    if combo.0.is_null() { return; }
    for item in items {
        let w: Vec<u16> = item.encode_utf16().chain(std::iter::once(0)).collect();
        send(combo, CB_ADDSTRING, WPARAM(0), LPARAM(w.as_ptr() as isize));
    }
}

fn combo_sel(combo: HWND, idx: i32) {
    if combo.0.is_null() || idx < 0 { return; }
    send(combo, CB_SETCURSEL, WPARAM(idx as usize), LPARAM(0));
}

fn combo_sel_str(combo: HWND, target: &str) {
    if combo.0.is_null() { return; }
    let count = send(combo, CB_GETCOUNT, WPARAM(0), LPARAM(0)).0 as i32;
    for i in 0..count {
        let len = send(combo, CB_GETLBTEXTLEN, WPARAM(i as usize), LPARAM(0)).0 as usize;
        if len == 0 { continue; }
        let mut buf = vec![0u16; len + 1];
        send(combo, CB_GETLBTEXT, WPARAM(i as usize), LPARAM(buf.as_mut_ptr() as isize));
        buf.truncate(buf.iter().position(|&c| c == 0).unwrap_or(buf.len()));
        if let Ok(s) = String::from_utf16(&buf) { if s == target { send(combo, CB_SETCURSEL, WPARAM(i as usize), LPARAM(0)); return; } }
    }
}

fn combo_text(combo: HWND) -> String {
    if combo.0.is_null() { return String::new(); }
    let sel = send(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
    if sel < 0 { return String::new(); }
    let len = send(combo, CB_GETLBTEXTLEN, WPARAM(sel as usize), LPARAM(0)).0 as usize;
    if len == 0 { return String::new(); }
    let mut buf = vec![0u16; len + 1];
    send(combo, CB_GETLBTEXT, WPARAM(sel as usize), LPARAM(buf.as_mut_ptr() as isize));
    buf.truncate(buf.iter().position(|&c| c == 0).unwrap_or(buf.len()));
    String::from_utf16(&buf).unwrap_or_default()
}

fn is_checked(chk: HWND) -> bool {
    if chk.0.is_null() { return false; }
    send(chk, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 != 0
}

fn edit_text(edit: HWND) -> String {
    if edit.0.is_null() { return String::new(); }
    let len = unsafe { GetWindowTextLengthW(edit) };
    if len == 0 { return String::new(); }
    let mut buf = vec![0u16; (len as usize) + 1];
    unsafe { GetWindowTextW(edit, &mut buf); }
    buf.truncate(buf.iter().position(|&c| c == 0).unwrap_or(buf.len()));
    String::from_utf16(&buf).unwrap_or_default()
}

fn edit_set(edit: HWND, text: &str) {
    if edit.0.is_null() { return; }
    let w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { let _ = SetWindowTextW(edit, PCWSTR(w.as_ptr())); }
}

// ── Language combo ────────────────────────────────────────────────────

const ASR_LANG: &[&str] = &[
    "auto", "en", "zh", "ja", "ko", "de", "fr", "es",
    "it", "pt", "ru", "ar", "hi", "vi", "th", "tr",
    "nl", "pl", "sv", "el", "he", "id",
];

fn fill_asr_lang(combo: HWND, s: &Strings) {
    for &code in ASR_LANG {
        let item = format!("{} ({})", s.language_display_name(code), code);
        let w: Vec<u16> = item.encode_utf16().chain(std::iter::once(0)).collect();
        send(combo, CB_ADDSTRING, WPARAM(0), LPARAM(w.as_ptr() as isize));
    }
}

fn pick_asr_lang(combo: HWND, target: &str) {
    if combo.0.is_null() { return; }
    let count = send(combo, CB_GETCOUNT, WPARAM(0), LPARAM(0)).0 as i32;
    for i in 0..count {
        let len = send(combo, CB_GETLBTEXTLEN, WPARAM(i as usize), LPARAM(0)).0 as usize;
        if len == 0 { continue; }
        let mut buf = vec![0u16; len + 1];
        send(combo, CB_GETLBTEXT, WPARAM(i as usize), LPARAM(buf.as_mut_ptr() as isize));
        buf.truncate(buf.iter().position(|&c| c == 0).unwrap_or(buf.len()));
        if let Ok(text) = String::from_utf16(&buf) {
            if let Some(p) = text.rfind('(') {
                if text[p + 1..].trim_end_matches(')') == target {
                    send(combo, CB_SETCURSEL, WPARAM(i as usize), LPARAM(0));
                    return;
                }
            }
        }
    }
}

fn parse_lang(text: &str) -> &str {
    text.rfind('(').map(|p| text[p+1..].trim_end_matches(')')).unwrap_or(text)
}

// ── Control layout ────────────────────────────────────────────────────
//
// Layout strategy: `gb` tracks each groupbox top edge. `add_groupbox` draws
// the frame and returns the bottom edge. Inner controls use `cy` (starting
// at `gb + padding`) so they render inside the groupbox. `y` advances to
// the groupbox bottom + gap for the next section.

fn create_controls(hwnd: HWND, config: &AppConfig, s: &Strings) {
    let mut y = 12;
    let gap = 4;

    // ── General section ──
    let gb = y;
    add_groupbox(hwnd, s.settings_general_section(), 8, gb, 432, 44);
    add_static(hwnd, s.settings_ui_language(), 16, gb + 14, 130, 20);
    let ui_lang = add_combobox(hwnd, IDC_UI_LANG, 150, gb + 12, 280, 180);
    combo_add(ui_lang, &["English", "中文"]);
    combo_sel(ui_lang, if config.ui.language == "zh" { 1 } else { 0 });
    y = gb + 44 + gap;

    // ── ASR section (4 columns + checkbox) ──
    let gb = y;
    add_groupbox(hwnd, s.settings_asr_section(), 8, gb, 432, 166);
    let mut cy = gb + 16;
    add_static(hwnd, s.settings_asr_language(), 16, cy, 130, 20);
    let asr_lang = add_combobox(hwnd, IDC_ASR_LANG, 150, cy - 2, 280, 200);
    fill_asr_lang(asr_lang, s);
    pick_asr_lang(asr_lang, &config.language.language);
    cy += 26;

    add_static(hwnd, s.settings_provider(), 16, cy, 130, 20);
    let provider = add_combobox(hwnd, IDC_PROVIDER, 150, cy - 2, 280, 100);
    combo_add(provider, &["cpu", "cuda"]);
    combo_sel_str(provider, &config.asr.provider);
    cy += 26;

    add_static(hwnd, s.settings_decoding(), 16, cy, 130, 20);
    let decoding = add_combobox(hwnd, IDC_DECODING, 150, cy - 2, 280, 100);
    combo_add(decoding, &["greedy_search", "modified_beam_search"]);
    combo_sel_str(decoding, &config.asr.decoding_method);
    cy += 26;

    add_static(hwnd, s.settings_threads(), 16, cy, 130, 20);
    let threads = add_edit(hwnd, IDC_THREADS, 150, cy - 2, 60, 22);
    edit_set(threads, &config.asr.num_threads.to_string());
    cy += 26;

    add_checkbox(hwnd, IDC_VAD, s.settings_vad(), 16, cy, 400, 22, config.asr.use_vad);
    y = gb + 166 + gap;

    // ── Injection section (2 columns + checkbox) ──
    let gb = y;
    add_groupbox(hwnd, s.settings_injection_section(), 8, gb, 432, 104);
    let mut cy = gb + 16;
    add_static(hwnd, s.settings_inject_strategy(), 16, cy, 130, 20);
    let strategy = add_combobox(hwnd, IDC_STRATEGY, 150, cy - 2, 280, 120);
    combo_add(strategy, &["auto", "sendinput", "uiautomation", "clipboard"]);
    combo_sel_str(strategy, &config.injector.strategy);
    cy += 26;

    add_static(hwnd, s.settings_key_delay(), 16, cy, 130, 20);
    let key_delay = add_edit(hwnd, IDC_KEY_DELAY, 150, cy - 2, 60, 22);
    edit_set(key_delay, &config.injector.key_delay_ms.to_string());
    cy += 26;

    add_checkbox(hwnd, IDC_RESTORE_CLIP, s.settings_restore_clipboard(), 16, cy, 400, 22, config.injector.restore_clipboard);
    y = gb + 104 + gap;

    // ── Hotkeys section (3 lines) ──
    let gb = y;
    add_groupbox(hwnd, s.settings_hotkeys_section(), 8, gb, 432, 82);
    let mut cy = gb + 16;
    let hotkeys = [
        (s.hotkey_toggle_label(), "Ctrl+Alt+R"),
        (s.hotkey_lang_label(), "Ctrl+Alt+L"),
        (s.hotkey_flush_label(), "Ctrl+Alt+Space"),
    ];
    for (action, key) in &hotkeys {
        add_static(hwnd, &s.settings_hotkey_line(action, key), 22, cy, 400, 20);
        cy += 20;
    }
    y = gb + 82 + gap;

    // ── Model status ──
    let (ok, total) = model_status(config);
    add_static(hwnd, &s.settings_model_status(ok, total), 16, y, 400, 20);
    y += 28;

    // ── Buttons ──
    add_button(hwnd, IDC_SAVE, s.settings_save(), 150, y, 90, 28);
    add_button(hwnd, IDC_CANCEL, s.settings_cancel(), 260, y, 90, 28);
}

// ── Handlers ──────────────────────────────────────────────────────────

fn on_save(hwnd: HWND) {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    if ptr == 0 { return; }
    let data = unsafe { &mut *(ptr as *mut ConfigDialogData) };

    // Get control HWNDs via child traversal (not GetDlgItem)
    let find = |id: u32| -> HWND {
        let mut child = HWND(ptr::null_mut());
        loop {
            child = unsafe { FindWindowExW(Some(hwnd), Some(child), PCWSTR::null(), PCWSTR::null()).unwrap_or(HWND(ptr::null_mut())) };
            if child.0.is_null() { break; }
            let cid = unsafe { GetDlgCtrlID(child) };
            if cid == id as i32 { return child; }
        }
        HWND(ptr::null_mut())
    };

    let u = find(IDC_UI_LANG);
    let a = find(IDC_ASR_LANG);
    let p = find(IDC_PROVIDER);
    let d = find(IDC_DECODING);
    let t = find(IDC_THREADS);
    let v = find(IDC_VAD);
    let s = find(IDC_STRATEGY);
    let k = find(IDC_KEY_DELAY);
    let r = find(IDC_RESTORE_CLIP);

    data.config.ui.language = if combo_text(u) == "中文" { "zh".into() } else { "en".into() };
    data.config.language.language = parse_lang(&combo_text(a)).to_string();
    data.config.asr.provider = combo_text(p);
    data.config.asr.decoding_method = combo_text(d);
    data.config.asr.use_vad = is_checked(v);
    if let Ok(n) = edit_text(t).parse::<u32>() { data.config.asr.num_threads = n; }
    data.config.injector.strategy = combo_text(s);
    if let Ok(n) = edit_text(k).parse::<u64>() { data.config.injector.key_delay_ms = n; }
    data.config.injector.restore_clipboard = is_checked(r);

    store_shared_config(&data.config);

    match data.config.save("config.toml") {
        Ok(()) => {
            info!("Settings saved");
            crate::ui::tray::send_tray_notification(&data.strings.settings_title(), &data.strings.settings_saved());
        }
        Err(e) => {
            warn!("Failed to save: {}", e);
            crate::ui::tray::send_tray_notification("Error", &format!("Save failed: {}", e));
        }
    }

    destroy(hwnd);
}

fn on_ui_lang_changed(hwnd: HWND) {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    if ptr == 0 { return; }
    let data = unsafe { &mut *(ptr as *mut ConfigDialogData) };

    let mut child = HWND(ptr::null_mut());
    let u = loop {
        child = unsafe { FindWindowExW(Some(hwnd), Some(child), PCWSTR::null(), PCWSTR::null()).unwrap_or(HWND(ptr::null_mut())) };
        if child.0.is_null() { return; }
        if unsafe { GetDlgCtrlID(child) } == IDC_UI_LANG as i32 { break child; }
    };

    let text = combo_text(u);
    let new = if text == "中文" { UiLang::Chinese } else { UiLang::English };
    if new == data.strings.lang { return; }

    data.config.ui.language = new.code().to_string();
    store_shared_config(&data.config);
    let parent = unsafe { GetAncestor(hwnd, GA_PARENT) };

    destroy(hwnd);
    // show_config_window will re-read from shared config
    show_config_window(parent, &data.config);
}

fn destroy(hwnd: HWND) {
    unsafe { let _ = DestroyWindow(hwnd); }
}
