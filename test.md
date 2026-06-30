# Nemotron ASR Voice Input — 測試計畫

> 基於 CBM 完整專案分析（55 檔案、481 符號、55 檔案結構）於 2026-06-30 產生

---

## 現有測試概況

```
cargo test 目前結果：17 tests passed（全數通過）

src/audio/ringbuf.rs  — 4 tests (test_push_pop, test_wraparound, test_push_slice, test_clear)
src/audio/resampler.rs — 3 tests (passthrough_preserves_samples, downsampling_preserves_duration_and_speech_band, downsampling_rejects_alias_frequency)
src/asr/mod.rs        — 1 test  (complete_decode_flushes_before_reading_final_result)
src/convert/mod.rs    — 8 tests (s2t, t2s, convert_text s2t/t2s/none/empty, from_config, index_roundtrip)
src/main.rs           — 1 test  (wav_parser_finds_data_after_nonstandard_chunks)
```

**現有不足**：
- ❌ 無整合測試（integration tests）
- ❌ 無注入模組測試（injector 完全無測試）
- ❌ 無 UI 模組測試（strings, gui, tray, overlay）
- ❌ 無設定模組測試（config 解析與寫回）
- ❌ 無下載模組測試（download 邏輯）
- ❌ 無邊界值與壓力測試

---

## 第一階段：補強單元測試（Unit Tests）

### 1.1 Audio Capture 模組（`src/audio/capture.rs`）

```rust
// tests/audio_tests.rs 或直接寫在 capture.rs 底部

#[test]
fn test_ringbuf_capacity_rounds_to_power_of_two() {
    // 驗證 new(100) → capacity 128
}

#[test]
fn test_ringbuf_push_pop_underflow_empty() {
    // 從空 buffer pop 應回傳 None
}

#[test]
fn test_ringbuf_peek_does_not_consume() {
    // peek_slice 不應移動 read_pos
}

#[test]
fn test_ringbuf_clear_during_use() {
    // 清除後 len() == 0
}

#[test]
fn test_capture_list_devices_returns_non_empty() {
    // 至少列出一個裝置
}
```

### 1.2 ASR 模組強化（`src/asr/`）

```rust
// 現有 FakeEngine 可擴充

#[test]
fn test_create_asr_engine_with_missing_model_returns_error() {
    // 指定不存在的模型目錄應回傳 Err
}

#[test]
fn test_language_name_to_code_full_coverage() {
    // 驗證所有 40+ 語言的 forward/backward mapping
}

#[test]
fn test_decode_complete_utterance_empty_input() {
    // 空音頻不會 panic，回傳 empty TranscriptResult
}

#[test]
fn test_decode_complete_utterance_engine_reset_on_error() {
    // FakeEngine 在 decode 失敗時 reset 仍應被呼叫
}
```

### 1.3 Injector 模組（`src/injector/`）

```rust
// 新增 src/injector/tests.rs

#[test]
fn test_sendinput_inject_empty_text() {
    // 空字串應直接 Ok(())，不產生任何 INPUT event
}

#[test]
fn test_sendinput_inject_ascii_builds_correct_input_count() {
    // "abc" → 6 個 INPUT (3 keydown + 3 keyup)
}

#[test]
fn test_sendinput_inject_unicode_uses_unicode_flag() {
    // 中文字元使用 KEYEVENTF_UNICODE
}

#[test]
fn test_composite_injector_fallback_order() {
    // CompositeInjector 按 UIA→SendInput→Clipboard 順序嘗試
}

#[test]
fn test_composite_injector_all_fail_returns_error() {
    // 當所有策略皆不可用應回傳 AllStrategiesFailed
}
```

### 1.4 Convert 模組強化（`src/convert/mod.rs`）

```rust
// 現有 8 tests 已完整；建議補充：

#[test]
fn test_convert_text_thread_safety() {
    // 多執行緒同時呼叫 convert_text 不會 panic
}

#[test]
fn test_convert_text_mixed_content() {
    // 中英混合文字：s2t 只影響中文部分
}
```

### 1.5 Config 模組（`src/config/settings.rs`）

```rust
// 新增 tests 模組

#[test]
fn test_app_config_load_default_writes_toml() {
    // 檔案不存在時應建立預設 config.toml
}

#[test]
fn test_app_config_load_existing_parses_correctly() {
    // 用已知 TOML 字串驗證所有欄位反序列化正確
}

#[test]
fn test_app_config_save_roundtrip() {
    // 寫出 → 讀回 → 比較每個欄位
}

#[test]
fn test_runtime_conversion_mode_mapping() {
    // RUNTIME_CONVERSION_MODE atomic → ConversionMode 對應正確
}
```

### 1.6 Hotkey 模組（`src/hotkey/register.rs`）

```rust
#[test]
fn test_format_hotkey_all_modifiers() {
    // Ctrl+Alt+Shift+Win+F1 格式正確
}

#[test]
fn test_format_hotkey_letter_key() {
    // Ctrl+A 格式正確
}

#[test]
fn test_format_hotkey_special_keys() {
    // Space, Enter, Backspace 等顯示名稱正確
}
```

### 1.7 UI Strings 模組（`src/ui/strings.rs`）

```rust
#[test]
fn test_ui_lang_from_code() {
    // "zh" → Chinese, "en" → English, 其他 → English
}

#[test]
fn test_strings_app_name_bilingual() {
    // English 與 Chinese 下 app_name 不同
}

#[test]
fn test_strings_language_display_name_all_codes() {
    // 所有在 cycle_order 中出現的語言代碼都有對應顯示名稱
}
```

---

## 第二階段：整合測試（Integration Tests）

### 2.1 測試架構

在 `tests/` 目錄下建立：

```
tests/
├── mod.rs              # 測試輔助工具
├── audio_to_asr.rs     # 音頻 → ASR 管線
├── injector_test.rs    # TextInjector 鏈
├── config_loading.rs   # 組態載入整合
└── e2e_pipeline.rs     # 端到端模擬
```

### 2.2 `tests/audio_to_asr.rs`

```rust
// 使用 FakeEngine 測試音頻資料從 ringbuf 到 ASR 引擎的流程

#[test]
fn test_audio_pipeline_push_pop_cycle() {
    // 模擬音頻執行緒：push 模擬資料 → pop → feed ASR
}

#[test]
fn test_resampler_then_asr_integration() {
    // 48kHz → resample → 16kHz → feed FakeEngine → 驗證資料正確
}

#[test]
fn test_full_audio_accumulation_on_stop() {
    // 錄音開始 → 累積音頻 → 停止 → 驗證 full_audio 包含預期樣本數
}
```

### 2.3 `tests/injector_test.rs`

```rust
// 注意：SendInput 實際呼叫 Win32 API，在 CI 環境可能無法執行
// 應使用 mock 或條件編譯

#[test]
fn test_composite_injector_all_fail_cleanup() {
    // 所有策略失敗後 CompositeInjector 狀態一致
}

#[test]
fn test_composite_injector_caches_working_strategy() {
    // 模擬第一個策略成功後，後續呼叫優先使用同一策略
}
```

### 2.4 `tests/config_loading.rs`

```rust
#[test]
fn test_config_full_roundtrip_with_tempfile() {
    // 建立暫存目錄 → 寫入 config → 讀回 → 比較
}

#[test]
fn test_config_load_corrupted_toml_returns_error() {
    // 損壞的 TOML 檔案應回傳 Err 而非 panic
}

#[test]
fn test_config_default_values_are_valid() {
    // 預設值的 sample_rate=16000, channels=1 等符合模型需求
}
```

---

## 第三階段：邊界值與壓力測試

### 3.1 邊界值測試

| 測試 | 說明 | 預期結果 |
|------|------|---------|
| `empty_text_injection` | 注入空字串 | Ok(())，無副作用 |
| `very_long_text_injection` | 注入 10000 字元的文字 | 不 panic，正確分割 |
| `zero_chunk_size` | chunk_size_ms = 0 | 不 panic，使用最小值 |
| `ringbuf_exact_capacity` | 填入恰好 capacity 個樣本 | 最後一個 push 成功 |
| `ringbuf_overfill_by_one` | 填入 capacity+1 個樣本 | 最後一個 push Err |
| `concurrent_start_stop` | 快速連續按下開始/停止 50 次 | 無 crash、無 deadlock |
| `rapid_language_switch` | 1 秒內切換語言 20 次 | 最終語言正確 |

### 3.2 壓力測試

```rust
#[test]
fn test_ringbuf_high_throughput() {
    // 100 萬次 push/pop 循環，驗證無資料遺失
}

#[test]
fn test_resampler_long_stream() {
    // 10 分鐘的模擬音頻（48kHz → 16kHz），驗證無 drift
}

#[test]
fn test_concurrent_channel_backpressure() {
    // 多生產者/消費者 channel 在高負載下不 deadlock
}
```

### 3.3 多執行緒安全性測試

```rust
#[test]
fn test_audio_thread_concurrent_state_access() {
    // 模擬 audio thread + main thread 同時讀寫 is_recording
    // 使用 loom/shuttle 或純 loop 驗證無 data race
}
```

---

## 第四階段：手動測試腳本

### 4.1 CLI 參數驗證

```bash
# 測試所有 CLI 參數的正常與錯誤路徑
cargo run -- --help
cargo run -- --list-devices
cargo run -- --model-status
cargo run -- --model-status --model-dir non_existent
cargo run -- --file non_existent.wav
cargo run -- --language invalid_code
cargo run -- --provider invalid_provider
cargo run -- --inject invalid_strategy
```

### 4.2 核心操作流程

```bash
# 1. 啟動應用（無系統匣）
cargo run -- --no-tray

# 2. 啟動應用（指定語言）
cargo run -- --language en

# 3. 啟動應用（CUDA 模式）——若硬體支援
cargo run -- --provider cuda

# 4. 測試 PTT 模式
#    - 按住 Ctrl+Shift+F2 開始錄音
#    - 放開後自動注入
```

### 4.3 應用相容性測試

| 應用 | SendInput | UIAutomation | Clipboard |
|------|-----------|--------------|-----------|
| 記事本 | ✅ | ❓ | ❓ |
| Chrome/Edge | ✅ | ❓ | ❓ |
| VS Code | ✅ | ❓ | ❓ |
| Word | ✅ | ❓ | ❓ |
| 終端機/PowerShell | ✅ | ❓ | ❓ |
| 遊戲（全螢幕） | ❓ | ❓ | ❓ |
| 密碼/敏感欄位 | ⚠️ | ❓ | ❓ |

---

## 第五階段：自動化 CI/CD 測試

### 5.1 GitHub Actions 設定

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - name: Build
        run: cargo build
      - name: Unit tests
        run: cargo test --lib
      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings
      - name: Format check
        run: cargo fmt --check
```

### 5.2 品質閘道

| 檢查 | 閘道條件 |
|------|---------|
| `cargo build` | ✅ 通過 |
| `cargo test --lib` | 100% 通過，新測試 ≥ 80% |
| `cargo clippy --all-targets -- -D warnings` | 0 warnings |
| `cargo fmt --check` | 通過 |
| 檔案大小 | main.rs ≤ 600 行（目前 1286） |

---

## 執行指令彙整

```bash
# 執行所有測試
cargo test

# 僅執行單元測試
cargo test --lib

# 僅執行整合測試（若建立 tests/ 目錄）
cargo test --test '*'

# 特定測試名稱過濾
cargo test ringbuf
cargo test resampler
cargo test inject
cargo test convert

# 顯示詳細輸出
cargo test -- --nocapture

# 測試建置（不執行）
cargo test --no-run

# 所有 targets（包含 tests, benches）
cargo test --all-targets

# Clippy 檢查
cargo clippy --all-targets -- -D warnings
```

---

## 測試優先級矩陣

| 測試 | 優先級 | 難度 | 影響範圍 | 備註 |
|------|--------|------|---------|------|
| Config 儲存正確性 | P0 | 低 | 設定功能 | 防止使用者資料遺失 |
| Inject 空殼實作 | P0 | 中 | 注入可靠性 | 需 COM 知識 |
| 設定視窗初始值 | P0 | 低 | UX | 簡單修復 |
| Ringbuf 壓力測試 | P1 | 低 | 音頻可靠性 | 防回歸 |
| main.rs 拆分後測試 | P1 | 中 | 可維護性 | 先重構再測試 |
| 多執行緒安全性 | P2 | 高 | 穩定性 | 需 loom/shuttle |
| CI/CD 自動化 | P2 | 中 | DevOps | 需 GitHub 權限 |

---

## 變更歷史

| 日期 | 摘要 |
|------|------|
| 2026-06-30 | 初始測試計畫 — 基於 CBM 完整專案分析 |
