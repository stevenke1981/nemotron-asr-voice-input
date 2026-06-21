# 實作計畫：Nemotron ASR 語音輸入法

## 1. 總體策略

**核心選擇**：使用 Rust + sherpa-onnx 方案 — 最少開發量、最高成熟度、純 Rust 無 Python 依賴。

**路線圖階段**：MVP → 強化 → 發行

**預計總工時**：約 120-160 小時 (全職 3-4 週)

---

## 2. 技術決策

### 2.1 關鍵決策記錄

| 決策 | 選擇 | 理由 |
|------|------|------|
| ASR 引擎 | sherpa-onnx (而非 ORT GenAI 或自幹) | 內建 Mel + RNNT + VAD；C/Rust API 完整 |
| 語言 | Rust | 記憶體安全、生態豐富、跨平台潛力 |
| 音頻庫 | cpal (WASAPI) | 純 Rust、事件驅動、Windows 原生支援 |
| 文字注入 | SendInput + UIAutomation 混合 | 廣泛相容、無需權限提升 |
| 模型格式 | ONNX INT4 (onnx-community) | 較小容量、較快推論 |
| 執行提供者 | CPU (預設) + CUDA (選項) | 相容性優先 |

### 2.2 備選方案

若 sherpa-onnx Rust binding 不足：
- **備選 A**：sherpa-onnx C API 透過 `cc` + FFI 呼叫
- **備選 B**：ONNX Runtime C++ GenAI API 編譯為 DLL，Rust 透過 FFI 呼叫
- **備選 C**：ort crate 自行實作 RNNT 解碼

---

## 3. 階段一：MVP (最小可行產品)

**目標**：可啟動、可轉錄、可注入。驗證核心流程。

### 3.1 里程碑 1：專案骨架

**預計工時**：4-6 小時
**預計工時**：4-6 小時

| 任務 | 檔案 | 說明 |
|------|------|------|
| 初始化 Cargo 專案 | `Cargo.toml` | 加入依賴：sherpa-onnx, cpal, windows, serde |
| 建立模組結構 | `src/main.rs` + 各 mod | 目錄架構、模組宣告 |
| 組態系統 | `src/config/settings.rs` | TOML 設定檔載入 |
| 日誌系統 | `src/main.rs` | tracing/env_logger 初始化 |
| **驗證** | `cargo build` | 成功編譯 |

### 3.2 里程碑 2：音頻擷取

**預計工時**：6-8 小時

| 任務 | 檔案 | 說明 |
|------|------|------|
| 麥克風裝置列舉 | `src/audio/capture.rs` | 列出裝置、選擇預設麥克風 |
| PCM 串流回呼 | `src/audio/capture.rs` | cpal::Stream 事件驅動回呼 |
| 環形緩衝區 | `src/audio/ringbuf.rs` | lock-free SPSC queue, 8960 samples |
| 格式轉換 | `src/audio/capture.rs` | 確保 16kHz mono f32 輸出 |
| **驗證** | `cargo run -- --dump-audio` | 將 PCM dump 為 WAV 驗證正確性 |

### 3.3 里程碑 3：ASR 轉錄

**預計工時**：8-12 小時

| 任務 | 檔案 | 說明 |
|------|------|------|
| sherpa-onnx 引擎初始化 | `src/asr/sherpa.rs` | 載入 encoder/decoder/joint/tokens |
| 模型下載腳本 | `build.rs` 或獨立腳本 | 從 HuggingFace 下載 ONNX 模型 |
| 音頻餵入串流 | `src/asr/sherpa.rs` | 從 ringbuf 取樣 -> accept_waveform |
| 解碼觸發 | `src/asr/sherpa.rs` | 定時呼叫 decode |
| 結果擷取 | `src/asr/sherpa.rs` | get_result -> 文字輸出 |
| VAD 整合 | `src/asr/sherpa.rs` | 啟用 Silero VAD |
| 語言設定 | `src/asr/sherpa.rs` | set_option("language", ...) |
| 抽象層 | `src/asr/mod.rs` | AsrEngine trait 實作 |
| **驗證** | `cargo run -- --file test.wav` | 轉錄已知 WAV 比對結果 |

### 3.4 里程碑 4：文字注入

**預計工時**：6-8 小時

| 任務 | 檔案 | 說明 |
|------|------|------|
| SendInput 注入 | `src/injector/sendinput.rs` | 字元逐一/WM_CHAR 注入 |
| Unicode 支援 | `src/injector/sendinput.rs` | KEYEVENTF_UNICODE 標記 |
| UIAutomation 優先 | `src/injector/uiautomation.rs` | 獲取聚焦元素 ValuePattern |
| 降級策略 | `src/injector/mod.rs` | SendInput -> Clipboard fallback |
| **驗證** | 手動測試 | 開啟記事本，注入中英文混合文字 |

### 3.5 里程碑 5：MVP 整合

**預計工時**：6-8 小時

| 任務 | 說明 |
|------|------|
| 主事件迴圈 | 音頻擷取執行緒 + ASR 執行緒 + 注入執行緒 |
| 執行緒間通訊 | crossbeam channel 傳遞轉錄文字 |
| 熱鍵觸發 | Ctrl+Alt+R 開始/停止錄音 |
| 錯誤處理 | 模型缺失、麥克風錯誤的適當回應 |
| **驗證** | 完整 MVP 功能測試 |

---

## 4. 階段二：強化

**目標**：產品級品質、多語言支援、使用者體驗優化。

### 4.1 系統匣 UI

| 任務 | 預計工時 |
|------|----------|
| 系統匣圖示與選單 | 8-10 小時 |
| 語言選擇子選單 | 3-4 小時 |
| 設定視窗 | 6-8 小時 |
| 轉錄狀態指示燈 | 2-3 小時 |

### 4.2 延遲優化

| 任務 | 預計工時 |
|------|----------|
| 環形緩衝區大小調校 | 2-3 小時 |
| 編碼器執行緒優先級 | 1-2 小時 |
| 低延遲 WASAPI 模式 | 4-6 小時 |
| CUDA 支援 (可選) | 6-8 小時 |

### 4.3 多語言支援

| 任務 | 預計工時 |
|------|----------|
| 語言 ID 對照表 | 2-3 小時 |
| 自動語言偵測測試 | 3-4 小時 |
| 使用者語言切換 UX | 4-6 小時 |

### 4.4 穩定性

| 任務 | 預計工時 |
|------|----------|
| 崩潰復原 (watchdog) | 4-6 小時 |
| 記憶體使用監控 | 2-3 小時 |
| 長時間運作測試 | 4-6 小時 |

---

## 5. 階段三：發行準備

**目標**：可發布的 Windows 應用程式。

| 任務 | 預計工時 |
|------|----------|
| 安裝程式 (MSI/InnoSetup) | 6-8 小時 |
| 模型下載引導程式 | 4-6 小時 |
| 數位簽章 | 2-3 小時 |
| 文件與使用說明 | 6-8 小時 |
| 自動更新機制 | 8-10 小時 |

---

## 6. 相依性一覽

### Cargo.toml

```toml
[package]
name = "nemotron-voice-input"
version = "0.1.0"
edition = "2024"

[dependencies]
# ASR 引擎
sherpa-onnx = { version = "1.10", features = ["online"] }

# 音頻擷取
cpal = "0.18"

# Windows API (文字注入 + 系統匣)
windows = { version = "0.58", features = [
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_Shell",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation",
] }

# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# 日誌
tracing = "0.1"
tracing-subscriber = "0.3"

# 執行緒
crossbeam = "0.8"
once_cell = "1"

# 快捷鍵
# 使用 windows crate 的 RegisterHotKey

# 錯誤處理
anyhow = "1"
thiserror = "1"

[build-dependencies]
# 模型下載 (可選)
# ureq = { version = "2", features = ["tls"] }
```

---

## 7. 建置與執行

### 7.1 建置需求

- Rust 1.85+
- CMake 3.20+ (sherpa-onnx 建置需求)
- Windows 10/11 SDK
- Visual Studio 2022 建置工具 (含 C++ toolchain)

### 7.2 模型準備

```bash
# 下載 ONNX INT4 模型 (約 800MB)
git lfs install
git clone https://huggingface.co/onnx-community/nemotron-3.5-asr-streaming-0.6b-onnx-int4 ./models

# 或手動下載所需檔案
mkdir -p models
# 下載 encoder.onnx + encoder.onnx.data
# 下載 decoder.onnx + decoder.onnx.data
# 下載 joint.onnx + joint.onnx.data
# 下載 silero_vad.onnx
# 下載 tokenizer.json
```

### 7.3 建置與執行

```bash
cargo build --release
./target/release/nemotron-voice-input.exe
```

### 7.4 測試指令

```bash
# 測試 WAV 檔案轉錄
cargo run --release -- --file test.wav --language zh

# 測試麥克風即時轉錄
cargo run --release

# 測試特定語言
cargo run --release -- --language ja
```

---

## 8. 風險與應對

| 風險 | 機率 | 影響 | 應對 |
|------|------|------|------|
| sherpa-onnx Rust binding 不完整 | 低 | 高 | 備選 A：直接使用 C API FFI |
| cpal WASAPI 延遲過高 | 低 | 中 | 使用 WASAPI 獨佔模式或 ASIO |
| ONNX 模型無法載入 (version mismatch) | 中 | 高 | 固定 ort/sherpa-onnx 版本、使用 ONNX Runtime 1.18+ |
| 文字注入在特定應用失效 | 中 | 中 | 多重注入策略、可設定的注入方式 |
| 中文 WER 過高 | 中 | 中 | 考慮 fine-tune 或後處理語言模型 |
