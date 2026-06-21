# Nemotron Voice Input - egui GUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Add egui main window + floating overlay to the existing nemotron-voice-input system-tray app.

**Architecture:** Existing Win32 main thread (hotkeys, tray, injection, state) continues unchanged. A separate thread runs eframe (egui main window). A third thread runs winit+egui (floating overlay). Crossbeam channels connect them. Existing modules (audio, asr, injector, config, convert, hotkey) are untouched.

**Tech Stack:** Rust, eframe 0.31, egui 0.31, egui-winit 0.31, egui-wgpu 0.31, winit 0.30, crossbeam

**Spec Reference:** docs/superpowers/specs/2026-06-22-nemotron-egui-gui-design.md

---

## File Map

### New Files

| File | Responsibility |
|------|---------------|
| src/ui/gui/mod.rs | Module declarations for gui submodule |
| src/ui/gui/state.rs | GuiSharedState, GuiAction enum, TranscriptEntry struct, channel setup |
| src/ui/gui/app.rs | eframe::App implementation: main window layout, all panels |
| src/ui/overlay/mod.rs | winit EventLoop + Window creation, overlay thread entry point |
| src/ui/overlay/ui.rs | egui overlay rendering: transparent always-on-top text display |

### Modified Files

| File | Change |
|------|--------|
| Cargo.toml | Add eframe, egui-winit, egui-wgpu, winit dependencies |
| src/main.rs | Spawn eframe thread + overlay thread; route GuiAction back to main loop |
| src/ui/mod.rs | Add pub mod gui; pub mod overlay; |
| src/ui/tray.rs | Add Show Main Window and Show/Hide Overlay menu items |
| src/ui/strings.rs | Add bilingual strings for new menu items |

### Untouched Files (core modules)

audio/, asr/, injector/, config/, convert/, hotkey/, src/ui/config_window.rs

---

## Phase 1: egui Main Window Basics (P0)

### Task 1.1: Add Dependencies

**Files:**
- Modify: Cargo.toml

Step 1: Add egui dependencies to Cargo.toml after the existing windows dependency block:

`
# GUI framework (egui)
eframe = { version = "0.31", features = ["wgpu"] }
egui-winit = "0.31"
egui-wgpu = "0.31"
winit = "0.30"
egui = "0.31"
`

Step 2: Verify build - run cargo check. Expected: Build succeeds (new deps download + compile).

Step 3: Commit:
`
git add Cargo.toml Cargo.lock
git commit -m "deps: add eframe, egui-winit, egui-wgpu, winit for GUI"
`

### Task 1.2: Create GUI Module Structure

**Files:**
- Create: src/ui/gui/mod.rs
- Create: src/ui/gui/state.rs
- Create: src/ui/overlay/mod.rs (placeholder)
- Modify: src/ui/mod.rs

Step 1: Create src/ui/gui/mod.rs:
`
pub mod app;
pub mod state;
`

Step 2: Create src/ui/gui/state.rs with:
- GuiAction enum: ToggleRecording, CycleLanguage, Flush, SetLanguage(String), SaveConfig(AppConfig), ShowOverlay(bool), DeleteHistoryEntry(usize), ClearHistory, Exit
- TranscriptEntry struct: text: String, timestamp: String, language: String
- GuiSnapshot struct: is_recording: bool, current_language: String, conversion_mode: String, latest_final_text: String, latest_partial_text: String, history: Vec<TranscriptEntry>

Step 3: Create placeholder src/ui/overlay/mod.rs:
`
// Placeholder - will be implemented in Phase 3
`

Step 4: Modify src/ui/mod.rs to add:
`
pub mod gui;
pub mod overlay;
`

Step 5: Verify build with cargo check.

Step 6: Commit:
`
git add src/ui/gui/mod.rs src/ui/gui/state.rs src/ui/overlay/mod.rs src/ui/mod.rs
git commit -m "feat(gui): create GUI/overlay module structure with state types"
`

### Task 1.3: Implement eframe Application Skeleton

**Files:**
- Create: src/ui/gui/app.rs

Step 1: Create app.rs with eframe::App implementation.

The file should contain:

- GuiSharedState struct with: snapshot Arc<Mutex<GuiSnapshot>>, gui_rx Receiver<GuiSnapshot>, action_tx Sender<GuiAction>, show_overlay Arc<AtomicBool>
- GuiApp struct with: state, current_snapshot, show_settings: bool, show_overlay_local: bool
- GuiApp::new(state) constructor
- process_incoming() - drain gui_rx channel into current_snapshot
- send_action() - send GuiAction to action_tx

eframe::App::update() implementation with four panels:
1. TopPanel "status_bar": recording indicator, language label, conversion mode, Settings/Overlay buttons (right-aligned)
2. CentralPanel: Live Transcript (final + partial text labels), History (ScrollArea with Copy/Del buttons per entry, Clear All button)
3. BottomPanel "controls": Start/Stop Recording, Cycle Language, Flush buttons
4. Settings Window (toggleable, placeholder content for now)

spawn_gui() function that creates a thread and runs eframe::run_native with the app settings (viewport 800x600, min 400x300).

Step 2: Verify build with cargo check.

Step 3: Commit:
`
git add src/ui/gui/app.rs
git commit -m "feat(gui): implement eframe App skeleton with status, transcript, history, controls"
`

### Task 1.4: Integrate GUI Thread into main.rs

**Files:**
- Modify: src/main.rs

Step 1: Add imports for GuiSnapshot, GuiAction, TranscriptEntry, spawn_gui, AtomicBool, Arc, Mutex.

Step 2: Add a simple_timestamp() helper function returning HH:MM:SS string.

Step 3: After tray initialization section, add GUI initialization:
- Create GuiSnapshot with initial values from app_config
- Create channels: gui_snapshot_tx/rx (bounded 256), gui_action_tx/rx (unbounded)
- Create show_overlay AtomicBool
- Call spawn_gui()

Step 4: In audio processing thread (where transcript results are sent), add snapshot update:
- Lock gui_snapshot mutex
- Update latest_final_text (if is_final) or latest_partial_text
- Push to history if is_final
- Send updated snapshot via gui_snapshot_tx

Step 5: In main loop, after tray action handler, add GUI action handler:
- Match on GuiAction variants and dispatch accordingly (ToggleRecording, CycleLanguage, Flush, SetLanguage, SaveConfig (placeholder), ShowOverlay, DeleteHistoryEntry, ClearHistory, Exit)

Step 6: Verify build with cargo check.

Step 7: Commit:
`
git add src/main.rs
git commit -m "feat(gui): integrate eframe GUI thread into main event loop"
`

### Task 1.5: Add Tray Menu Items for Main Window

**Files:**
- Modify: src/ui/tray.rs
- Modify: src/ui/strings.rs

Step 1: Add ShowMainWindow variant to TrayAction enum.

Step 2: Add menu constant IDM_SHOW_MAIN_WINDOW = 1006, menu item, and WM_TRAYICON handler.

Step 3: Add bilingual strings in strings.rs (English "Show Main Window" / Chinese "显示主视窗").

Step 4: Handle in main.rs tray handler - log the action (eframe window is always visible).

Step 5: Commit:
`
git add src/ui/tray.rs src/ui/strings.rs src/main.rs
git commit -m "feat(gui): add Show Main Window tray menu item"
`

**Phase 1 Verification:**
Run cargo run. Verify: main window appears with status bar, recording via hotkey shows transcript, history accumulates, copy/delete works.

---

## Phase 2: Settings Panel (P0)

### Task 2.1: Implement Settings Panel

**Files:**
- Modify: src/ui/gui/app.rs
- Modify: src/main.rs

Step 1: Add settings state fields to GuiApp struct (settings_language, settings_provider, settings_num_threads, settings_use_vad, settings_decoding_method, settings_inject_strategy, settings_key_delay_ms, settings_restore_clipboard, settings_conversion_mode, settings_ui_lang). Initialize with defaults in new().

Step 2: Replace the placeholder settings window with a full Grid layout containing all settings fields:
- UI Language: ComboBox (English/Chinese)
- ASR Language: ComboBox (zh/en/ja/de/fr/es/ko)
- Provider: ComboBox (cpu/cuda)
- Num Threads: DragValue (1-16)
- VAD: Checkbox
- Decoding: ComboBox (greedy_search/modified_beam_search)
- Inject: ComboBox (sendinput/clipboard/auto)
- Key Delay: DragValue (0-100ms)
- Restore Clipboard: Checkbox
- Text Conversion: ComboBox (none/s2t/t2s)
- Hotkeys: read-only labels
- Save: builds AppConfig from fields, sends GuiAction::SaveConfig(cfg)
- Cancel: closes window

Step 3: In main.rs, implement the SaveConfig handler:
- Call cfg.save() to write config.toml
- Update runtime settings: current_language, RUNTIME_VAD_ENABLED, RUNTIME_CONVERSION_MODE
- Update gui_snapshot current_language and conversion_mode

Step 4: Verify - run cargo check, then cargo run and test settings save/load.

Step 5: Commit:
`
git add src/ui/gui/app.rs src/main.rs
git commit -m "feat(gui): implement full settings panel replacing Win32 dialog"
`

---

## Phase 3: Floating Overlay (P1)

### Task 3.1: Create Overlay Module

**Files:**
- Create: src/ui/overlay/ui.rs
- Modify: src/ui/overlay/mod.rs (replace placeholder)

Step 1: Create src/ui/overlay/ui.rs with render_overlay_frame(ctx, text, hovered):
- Uses egui::Frame with dark semi-transparent background (alpha 0.85, 0.95 when hovered)
- Renders text with white color, 20pt font, centered
- Returns nothing (draws directly to ctx)

Step 2: Replace src/ui/overlay/mod.rs with full implementation:
- run_overlay(text_rx, stop): creates winit EventLoop, Window with decorations(false), always_on_top(true), transparent(true); positions at bottom-center of monitor; runs event loop receiving text from channel; renders using egui-winit state
- spawn_overlay(text_rx, stop): spawns thread to call run_overlay

Note: egui-wgpu rendering integration is needed. Use egui_winit + egui_wgpu renderer setup. See egui_wgpu::WgpuSetup for context creation.

Step 3: In main.rs, after GUI initialization, add overlay initialization:
- Create overlay_tx/rx channel (bounded 8)
- Create overlay_stop AtomicBool
- Call spawn_overlay

Step 4: In audio processing thread, after updating gui_snapshot, send final text to overlay:
`
if result.is_final { let _ = overlay_tx.send(trimmed.clone()); }
`

Step 5: Verify build with cargo check.

Step 6: Commit:
`
git add src/ui/overlay/ src/main.rs
git commit -m "feat(overlay): implement floating transcript overlay with winit+egui"
`

### Task 3.2: Add Overlay Toggle to Tray Menu

**Files:**
- Modify: src/ui/tray.rs
- Modify: src/ui/strings.rs
- Modify: src/main.rs

Step 1: Add ToggleOverlay to TrayAction, menu constant 1007, menu item, handler, bilingual strings.

Step 2: Handle in main.rs tray handler: toggle show_overlay flag and send GuiAction::ShowOverlay.

Step 3: Commit:
`
git add src/ui/tray.rs src/ui/strings.rs src/main.rs
git commit -m "feat(overlay): add overlay toggle to system tray menu"
`

---

## Phase 4: Polish (P2)

### Task 4.1: Window Position Memory
Save position of main window and overlay window to config.toml, restore on startup.

### Task 4.2: egui Theme Toggle
Add theme ComboBox (Dark/Light) to settings panel. Apply via ctx.set_visuals().

### Task 4.3: Overlay Auto-Fade
Track idle time in overlay event loop. After 5s of no new text, reduce alpha to 0.3. Reset on text or hover.

### Task 4.4: Remove Old Config Window
After Phase 2 is stable, remove src/ui/config_window.rs and its mod declaration.

---

## Rollback Strategy

| Phase | Rollback | Impact |
|-------|----------|--------|
| 1/2 | git checkout previous commit | No data migration needed |
| 3 | Remove overlay thread spawn in main.rs | No user data impact |
| 4.4 | Restore config_window.rs from git | No dependencies on its removal |