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
---
## Lesson #8 — 2026-06-21
**Trigger:** ASR engine crash at startup — `'vocab_size' does not exist in the metadata` from sherpa-onnx `online-transducer-nemo-model.cc`.
**Rule:** Always use sherpa-onnx's official pre-exported model packages (from their GitHub Releases) rather than community-exported ONNX models from HuggingFace. Community models often lack required metadata (`vocab_size`, `context_size`) and use Xet split format (`.onnx` + `.onnx.data`) that sherpa-onnx does not fully support. Check https://k2-fsa.github.io/sherpa/onnx/nemo/nemotron-streaming.html for the official multilingual model packages.
**Source:** fix: switch to sherpa-onnx official Nemotron model to fix ASR crash
---
## Lesson #9 — 2026-06-21
**Trigger:** sherpa-onnx C++ assertion crash `features.cc:GetFrames:188 0 + 65 > 55` on first recording hotkey press — model metadata T_=65 frames (650ms) but chunk_size_ms=560ms only provides 56 frames.
**Rule:** When feeding audio to sherpa-onnx streaming transducer models, ensure `chunk_size_ms` provides enough frames for the model's `T_` (total receptive field) metadata value plus snip_edges overhead. For zipformer2 models: `T_` = model metadata `"T"` in frames (10ms each), and snip_edges=true adds ~25ms frame_length overhead. Formula: `chunk_ms >= (T_ * 10 + 25) * 1000 / 16000`. For Nemotron (T_=65): chunk_ms >= 675, round up to 700. Before changing, check model metadata by running with `config_.debug = true` to see `T_` and `decode_chunk_len_` values.
**Source:** fix: increase chunk_size_ms from 560 to 700 to prevent sherpa-onnx crash
---
## Lesson #10 — 2026-06-22
**Trigger:** PTT mode crash (Ctrl+Shift+L 閃退) — sherpa-onnx assert `features.cc:GetFrames:188 0 + 65 > 30` when calling `decode()` multiple times on audio shorter than one model chunk.
**Rule:** Never call sherpa-onnx `recognizer.decode()` or `recognizer.get_result()` more than once on the same stream data without feeding new audio first. The internal feature buffer expects at least `T_` frames (65 for Nemotron, ~650ms) of audio per decode operation. Calling decode multiple times on short audio (< one chunk) will assert-crash because the frame computation `offset + requested > available` (e.g., `0 + 65 > 30`) fails. If multiple decode passes are needed, either (a) accumulate enough audio first, or (b) feed the same audio again via `accept_waveform` before each decode, or (c) use endpoint detection instead.
**Source:** fix: remove multi-cycle decode loop to prevent PTT crash
---
## Lesson #11 — 2026-06-22
**Trigger:** Second utterance fails to transcribe (silent failure) after `recognizer.reset(stream)` between PTT utterances.
**Rule:** `recognizer.reset(&stream)` in sherpa-onnx does NOT fully clear the stream's internal state. Stale frame buffers, endpoint detection flags, and feature extraction state leak into the next utterance. Instead of calling `reset()`, ALWAYS call `recognizer.create_stream()` and replace the old stream. This gives a completely fresh processing context. Re-apply runtime settings (language, VAD) to the new stream after creation. This is also the pattern used in sherpa-onnx's official microphone examples.
**Source:** fix: complete ASR engine rewrite — create_stream on reset, is_ready guard, remove total_fed
---
## Lesson #12 — 2026-06-22
**Trigger:** Manual `total_fed >= chunk_target` guard was fragile — didn't account for resampling, different model T_ values, or the exact internal frame buffer state.
**Rule:** Use `recognizer.is_ready(&stream)` instead of manually tracking samples fed. This is the official sherpa-onnx API that checks the internal feature frame buffer against the model's `T_` minimum. It works correctly regardless of sample rate, chunk size, or resampling. The official example (`streaming_zipformer.rs`) uses this pattern: `while recognizer.is_ready(&stream) { recognizer.decode(&stream); ... }`.
**Source:** fix: replace total_fed guard with recognizer.is_ready()
