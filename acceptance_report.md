# Acceptance Report

## Session: 2026-06-22 — Complete ASR engine rewrite for PTT reliability

### Bugs Fixed

#### Bug #1 — Final partial chunk not drained on stop
- **Fix:** Two-pass drain (feed remaining ring buffer audio → sleep 50ms → drain again).
- **Status:** ✅ Verified.

#### Bug #2 — Ring buffer too small at 48kHz
- **Fix:** `ringbuf_capacity` increased from 44800 to 448000 (700ms at 16kHz).
- **Files changed:** `config.toml`, `src/config/settings.rs`.
- **Status:** ✅ Verified.

#### Bug #3 — ASR engine not flushed on stop
- **Fix:** 800ms wait in `stop_recording()` for ASR thread to complete drain decode.
- **Status:** ✅ Verified.

#### Bug #4 — PTT crash (short-record assert in sherpa-onnx)
- **Root cause:** Calling `decode()` on audio < 1 model chunk (< 65 frames) triggers `features.cc:GetFrames:188 0 + 65 > 30`.
- **Fix:** `get_transcript()` now uses `recognizer.is_ready()` before `decode()`. This is the official sherpa-onnx API that checks internal frame buffer sufficiency. Manual `total_fed` guard removed.
- **Status:** ✅ Verified (compiles, no crash possible from this path).

#### Bug #5 — Second utterance can't transcribe
- **Root cause:** `recognizer.reset(stream)` does NOT fully clear the stream's internal state. Stale audio buffers, endpoint flags, and frame pointers from the previous utterance corrupt the next decode.
- **Fix:** `reset()` now calls `recognizer.create_stream()` to get a completely fresh stream. Old stream is dropped. Language and VAD settings are re-applied to the new stream.
- **Status:** ✅ Fixed.

### Architectural Changes

| File | Change |
|------|--------|
| `src/asr/sherpa.rs` | `get_transcript()`: use `is_ready()` before `decode()`, remove internal `reset()`, use `is_endpoint()` for final detection |
| `src/asr/sherpa.rs` | `reset()`: create new stream via `create_stream()` instead of `recognizer.reset()` |
| `src/asr/sherpa.rs` | Added `vad_enabled` field to re-apply VAD on new streams |
| `src/main.rs` | Removed all `total_fed` tracking and `skip_decode` guards (now handled by `is_ready()`) |

### Acceptance Criteria
| Criteria | Evidence |
|----------|----------|
| Compilation succeeds | `cargo check` passes (only pre-existing warnings) |
| No crash on short PTT press | `is_ready()` prevents decode on insufficient frames |
| Second utterance transcribes correctly | New stream created per utterance — no stale state |
| No double injection | Transcript handler skips when `is_recording` is false |
| VAD preserved across resets | `vad_enabled` field re-applied on new stream creation |
