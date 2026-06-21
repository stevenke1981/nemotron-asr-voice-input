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
