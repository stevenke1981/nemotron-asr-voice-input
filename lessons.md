---
## Lesson #1 — 2026-06-21
**Trigger:** First build of nemotron-voice-input from spec/plan/todos
**Rule:** Before using any crate's API in Rust code, check the actual published crate version + API signatures via `cargo tree`, `cargo doc`, or reading the source. The plan's assumed APIs (sherpa-onnx `online` feature, `FastFixedIn` in rubato, `SampleRate` as tuple struct in cpal) were all outdated.
**Source:** MVP implementation of Nemotron ASR Voice Input
---
## Lesson #2 — 2026-06-21
**Trigger:** Windows crate version mismatch between cpal 0.18 (windows 0.62) and our windows 0.58, causing HMENU/HWND trait conflicts with Param/CanInto from different windows-core versions.
**Rule:** Before adding a dependency that contains windows types (cpal, sherpa-onnx-sys, etc.), use `cargo tree -i windows` and `cargo tree -i windows-core` to check what versions already exist in the dependency closure. Pin our `windows` crate to match the highest version used by any dependency to avoid version conflicts in type traits.
**Source:** Phase 2 - model download + system tray implementation

## Lesson #3 — 2026-06-21
**Trigger:** Windows NOTIFYICONDATAW struct layout changed significantly between windows 0.58 and 0.62 — szInfo/szInfoTitle/dwInfoFlags went from being behind unions to being direct fields, and uFlags changed from u32 to NOTIFY_ICON_DATA_FLAGS bitflag struct.
**Rule:** When upgrading the windows crate version, re-check the struct layout of all Win32 types used (especially union-heavy types like NOTIFYICONDATAW) by grepping the generated bindings. These types are auto-generated from metadata and can change structure between versions.
**Source:** Phase 2 - system tray reimplementation

## Lesson #4 — 2026-06-21
**Trigger:** Rust 2024 edition changed `unsafe_op_in_unsafe_fn` from an allow-by-default lint to a warn-by-default lint (will become deny in future). The project uses edition 2024.
**Rule:** In any `unsafe fn`, wrap every call to an unsafe function in an inner `unsafe {}` block. This is now required by Rust 2024 edition. Use `cargo build 2>&1 | Select-String "unsafe_op_in_unsafe_fn"` to check compliance.
**Source:** Phase 2 - tray.rs compilation with Rust 2024 edition
---
## Lesson #5 — 2026-06-21
**Trigger:** `cargo fix` auto-applied unsafe blocks around every function call in config_window.rs, creating many `unnecessary unsafe` blocks. Manual cleanup was needed.
**Rule:** Use `cargo fix` first to auto-fix 70-80% of `unsafe_op_in_unsafe_fn` warnings, but accept that `cargo fix` will over-wrap safe functions inside `unsafe {}`. After `cargo fix`, manually convert `unsafe fn` helpers that only call Win32 API to `fn` with targeted `unsafe {}` inside. Only then remove redundant `unsafe {}` wrappers around safe helper calls. The pattern: `fn helper() { unsafe { Win32Api() } }` — callers use `helper()` directly without `unsafe {}`.
**Source:** fix: clean all Rust 2024 unsafe warnings, connect bilingual notifications, improve ASR init error handling
---
## Lesson #6 — 2026-06-21
**Trigger:** Settings window Save button was invisible because `create_controls` used the return value of `add_groupbox()` (the bottom edge of the groupbox) as the y-coordinate for inner controls, placing them below the groupbox frame instead of inside it.
**Rule:** In Win32 dialog layout, `add_groupbox()` returns `y + h` (bottom edge). Save the groupbox top before calling it (`let gb = y`), use `gb + padding` for inner controls, and set `y = gb + h + gap` for the next section. Never use the return value of `add_groupbox` as the inner-control origin.
**Source:** fix(ui): repair settings layout so Save/Cancel buttons are visible, show GUI on startup
---
## Lesson #7 — 2026-06-21
**Trigger:** Settings window language change was lost on reopen because `show_config_window` always used the original `app_config` from `main()`. A shared mutable config (`Arc<Mutex<>>`) was needed.
**Rule:** When the settings dialog can modify config at runtime and needs to reflect those changes on reopen, use a shared state (e.g. `OnceLock<Mutex<AppConfig>>` in the config_window module). Seed it from `main()` with `init_shared_config()`, have the save handler update it via `store_shared_config()`, and have the open handler read from it via `load_shared_config()`.
**Source:** fix: settings now persist across window re-opens via shared config; add hotkey tracing
