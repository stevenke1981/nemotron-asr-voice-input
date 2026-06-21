# 技術分析報告：Nemotron 3.5 ASR Streaming 非 Python 語音輸入法

## 1. 概述

本報告分析如何在不使用 Python 的前提下，利用 **Rust / C / C++** 建立基於 **NVIDIA Nemotron 3.5 ASR Streaming** 模型的語音轉錄應用，並將轉錄結果注入焦點視窗，實現類似「語音輸入法」的功能。

### 1.1 模型摘要

| 項目 | 說明 |
|------|------|
| **原始模型** | `nvidia/nemotron-3.5-asr-streaming-0.6b` |
| **ONNX 量化版** | `onnx-community/nemotron-3.5-asr-streaming-0.6b-onnx-int4` |
| **架構** | FastConformer-CacheAware-RNNT + Language-ID Prompt |
| **參數量** | 600M |
| **支援語言** | 40 語言區域 (19 立即可用 + 13 廣泛覆蓋 + 8 適配就緒) |
| **取樣率** | 16 kHz |
| **麥克風輸入** | 128 維 Mel 頻譜 (FFT 512, Hop 160, Window 400) |
| **區塊大小** | 80 / 160 / 320 / 560 / 1120 ms (ONNX 版優化於 560ms) |
| **權重格式** | INT4 量化 (ONNX) |
| **模型容量** | 編碼器 24 層 + 解碼器 2 層 LSTM + Joiner |
| **授權** | MIT |

### 1.2 ONNX 模型檔案結構

```
nemotron-3.5-asr-streaming-0.6b-onnx-int4/
├── encoder.onnx (+ encoder.onnx.data)   # FastConformer 編碼器 ~690MB
├── decoder.onnx (+ decoder.onnx.data)   # RNN-T 解碼器 (2層LSTM) ~60MB
├── joint.onnx (+ joint.onnx.data)       # Joiner 網路 ~38MB
├── silero_vad.onnx                      # 語音活動偵測 (VAD) ~2.2MB
├── tokenizer.json / tokenizer_config.json
├── vocab.txt                            # 詞彙表 (13088 tokens)
├── audio_processor_config.json          # 音頻前處理參數
├── genai_config.json                    # ONNX Runtime GenAI 設定
└── model_config.json
```

---

## 2. 技術可行性分析

### 2.1 非 Python 推論方案比較

| 方案 | 語言 | 成熟度 | 維護狀態 | 難度 |
|------|------|--------|---------|------|
| **ONNX Runtime GenAI C/C++ API** | C/C++ | ✅ 正式支援 | Microsoft 維護 | 中 |
| **ONNX Runtime C++ API (直接)** | C/C++ | ✅ 穩定 | Microsoft 維護 | 高 |
| **ort crate (Rust bindings)** | Rust | ✅ RC2 (2.0.0-rc.12) | 活躍開源 | 中 |
| **sherpa-onnx C/Rust API** | C/Rust | ✅ 已合併 Nemotron 支援 | k2-fsa 維護 | 低 |
| **ONNX Runtime GenAI Rust (尚不存在)** | Rust | ❌ 無官方 bindings | — | 極高 |

**結論：**
- **捷徑方案**：使用 **sherpa-onnx** 的 C API 或 Rust binding — 只需配置模型路徑即可，無需自行實作 RNNT 解碼迴圈。
- **低階方案**：使用 **ONNX Runtime C++ API** 自行載入 encoder/decoder/joint 並實作 streaming RNNT 解碼 — 彈性最大但開發量高。
- **純 Rust 方案**：使用 **ort crate** (pykeio/ort) 直接呼叫 ONNX Runtime — 需自行處理 Mel 前處理與 RNNT 解碼。

### 2.2 sherpa-onnx 方案優勢

sherpa-onnx 在 2026 年 6 月已合併 multilingual Nemotron 3.5 支援 (PR #3671)，提供：
- C API：`SherpaOnnxCreateOnlineRecognizer` + `SherpaOnnxOnlineStreamAcceptWaveform`
- C++ API：`OnlineRecognizer` + `OnlineStream`
- Rust binding：`sherpa_onnx::OnlineRecognizer`
- 內建 Mel 前處理（無需自行實作）
- 內建 RNNT greedy search 解碼
- 內建 VAD (Silero VAD)
- 支援語言提示：`stream.set_option("language", "zh")`

### 2.3 ONNX Runtime GenAI 方案

Microsoft 已在 `onnxruntime-genai` 中合併 Nemotron Speech streaming ASR 支援 (PR #1997, #2171)：
- C++ API：`OgaConfig` → `OgaModel` → `OgaStreamingProcessor`
- 支援 CUDA 加速
- 支援語言 ID (`lang_id`) 輸入
- 支援 VAD 設定
- 範例：`examples/python/nemotron_speech.py`

但 ONNX Runtime GenAI**無官方 Rust binding**，需透過 C API 封裝。

### 2.4 自家實作 RNNT 解碼

若使用 `ort` crate 直接推論，需自行實作：
1. **Mel 頻譜前處理**：16kHz PCM → 128-dim Mel (預加重、STFT、Mel 濾波、log)
2. **編碼器推論**：送入音頻區塊 + cache 狀態 → 取得編碼輸出
3. **RNNT 解碼迴圈**：greedy search：將編碼輸出逐幀餵給 decoder + joiner
4. **LSTM 狀態管理**：decoder 的 hidden/cell state 需跨步傳遞
5. **快取管理**：encoder 的 cache_last_channel / cache_last_time 需跨區塊維護

> **估計開發量**：自家實作完整 RNNT 解碼約需 600-1000 行 Rust 程式碼，並需深入理解 RNNT 演算法。

---

## 3. 系統架構設計

### 3.1 整體架構

```
┌─────────────────────────────────────────────────────────────────┐
│                    nemotron-voice-input                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────┐    ┌──────────────┐    ┌─────────────────────┐   │
│  │ Audio    │───▶│ Audio       │───▶│ ASR Engine           │   │
│  │ Capture  │    │ Ring Buffer │    │ (sherpa-onnx / ort)  │   │
│  └──────────┘    └──────────────┘    └──────────┬──────────┘   │
│       ▲                                         │              │
│       │ 16kHz PCM f32                           │ text         │
│       │                                         ▼              │
│  ┌────┴─────┐                           ┌──────────────────┐   │
│  │ WASAPI   │                           │ Text Injector    │   │
│  │ Loopback │                           │ (SendInput /     │   │
│  │ or Mic   │                           │  UIAutomation /  │   │
│  └──────────┘                           │  Clipboard)      │   │
│                                         └────────┬─────────┘   │
│                                                  │              │
│                                                  ▼              │
│                                           ┌─────────────┐      │
│                                           │  Focused     │      │
│                                           │  Application │      │
│                                           └─────────────┘      │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 音頻擷取層

| 方法 | 說明 | 適用場景 |
|------|------|---------|
| **WASAPI Microphone** | 從麥克風即時捕捉 PCM | 麥克風語音輸入 |
| **WASAPI Loopback** | 捕捉系統音效卡輸出 | 會議錄音、系統音頻 |
| **檔案輸入** | 讀取 WAV/MP3 檔案 | 批次處理、測試 |

- **推薦庫**：
  - **Rust**: `cpal` (純 Rust, WASAPI 後端)
  - **C++**: `RtAudio` 或直接 WASAPI COM API (`IAudioCaptureClient`)
  - **C**: 直接呼叫 Windows Core Audio API

### 3.3 音頻前處理

ASR 引擎需要 16kHz 單聲道 f32 PCM 輸入：
- 若來源非 16kHz → 需重取樣 (resample)
- 若來源為立體聲 → 需混音為單聲道
- 若來源為整數格式 → 需轉為 f32

### 3.4 ASR 引擎層

#### 方案 A: sherpa-onnx（推薦）

```rust
use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, Wave};

let config = OnlineRecognizerConfig {
    model_config: OnlineModelConfig {
        transducer: OnlineTransducerModelConfig {
            encoder: Some("encoder.onnx".into()),
            decoder: Some("decoder.onnx".into()),
            joiner: Some("joint.onnx".into()),
        },
        tokens: Some("tokens.txt".into()),
        provider: Some("cpu".into()),
        num_threads: Some(4),
        ..Default::default()
    },
    decoding_method: Some("greedy_search".into()),
    enable_endpoint: true,
    ..Default::default()
};

let recognizer = OnlineRecognizer::create(&config)?;
let stream = recognizer.create_stream();
stream.set_option("language", "zh")?;
// 餵入音頻
stream.accept_waveform(16000, &pcm_data);
recognizer.decode(&stream);
let result = recognizer.get_result(&stream);
```

#### 方案 B: ONNX Runtime C++ GenAI

```cpp
#include "ort_genai.h"

auto config = OgaConfig::Create(model_path);
auto model = OgaModel::Create(*config);
auto processor = OgaStreamingProcessor::Create(*model);

processor->SetOption("use_vad", "true");
// 逐區塊處理
auto mel = processor->Process(audio_chunk.data(), audio_chunk.size());
// 推論 (透過 generator)
while (true) {
    speech_state->StepToken();
    // 獲取轉錄文字
}
```

#### 方案 C: ort crate (純 Rust，自行實作解碼)

```rust
use ort::{Session, SessionBuilder, inputs, GraphOptimizationLevel};

let encoder = Session::builder()?
    .commit_from_file("encoder.onnx")?;
let decoder = Session::builder()?
    .commit_from_file("decoder.onnx")?;
let joiner = Session::builder()?
    .commit_from_file("joint.onnx")?;

// 自行實作 RNNT greedy search 演算法
```

### 3.5 文字注入層

| 方法 | 可靠性 | 速度 | 備註 |
|------|--------|------|------|
| **SendInput + WM_CHAR** | ⭐⭐⭐ | 逐字元 | Windows API, 中等速度 |
| **SendInput + Unicode** | ⭐⭐⭐ | 逐字元 | 支援多語言 |
| **UIAutomation ValuePattern** | ⭐⭐⭐⭐ | 一次性 | 僅支援 UIA 相容應用 |
| **剪貼簿 + Ctrl+V** | ⭐⭐⭐⭐ | 一次性 | 會污染剪貼簿 |
| **SendMessage/PostMessage WM_SETTEXT** | ⭐⭐ | 一次性 | 部分應用不支援 |

**建議策略**：`UIAutomation` (優先) → `SendInput` (預設) → `Clipboard` (後備)。

### 3.6 支援語言設定

透過 language-ID prompt conditioning，支援 40 個語言區域：

| 層級 | 數量 | 說明 |
|------|------|------|
| 立即可用 (Transcription-ready) | 19 | 最高準確度，開箱即用 |
| 廣泛覆蓋 (Broad-coverage) | 13 | 生產級 ASR |
| 適配就緒 (Adaptation-ready) | 8 | 需領域微調 |

---

## 4. 非 Python 技術風險評估

| 風險 | 等級 | 緩解措施 |
|------|------|---------|
| **RNNT 解碼演算法複雜** | 🟡 中 | 使用 sherpa-onnx，其已封裝完整解碼 |
| **Mel 前處理實作** | 🟡 中 | sherpa-onnx 內建處理；或參考 NeMo 原始碼 |
| **ONNX 權重容量大 (.data 690MB)** | 🟢 低 | INT4 量化已很小；可延遲載入 |
| **Windows 權限隔離 (UIPI)** | 🟡 中 | SendInput 在 UAC 提升視窗需特殊處理 |
| **CPU 即時性** | 🟡 中 | INT4 可接受；可選 CUDA 加速 |
| **LSTM 狀態管理** | 🟡 中 | sherpa-onnx 已封裝 |
| **多語言提示支援** | 🟢 低 | sherpa-onnx 支援 `set_option("language")` |
| **無官方 Rust GenAI binding** | 🟡 中 | 可選 C API FFI 或改用 sherpa-onnx |

---

## 5. 建議技術棧

### 5.1 推薦方案（Rust）

| 層級 | 技術選擇 |
|------|---------|
| **語言** | Rust 1.85+ |
| **ASR 引擎** | `sherpa_onnx` crate（內建 Mel 前處理 + RNNT 解碼 + VAD） |
| **音頻擷取** | `cpal` crate (WASAPI 後端) |
| **文字注入** | Windows `SendInput` / `UIAutomation` via `windows` crate |
| **音頻重取樣** | `rubato` crate（若需要） |
| **組態管理** | `serde` + `serde_json` |
| **多執行緒** | `std::thread` + `crossbeam` channel |

### 5.2 替代方案（C++）

| 層級 | 技術選擇 |
|------|---------|
| **ASR 引擎** | `onnxruntime-genai` C++ API 或 `sherpa-onnx` C++ API |
| **音頻擷取** | WASAPI COM 直接呼叫 或 `RtAudio` |
| **文字注入** | Win32 `SendInput` + `UIAutomation` |
| **建置工具** | CMake + vcpkg / Conan |

### 5.3 純 C 方案

| 層級 | 技術選擇 |
|------|---------|
| **ASR 引擎** | `sherpa-onnx` C API |
| **音頻擷取** | WASAPI C COM API |
| **文字注入** | Win32 `SendInput` |
| **建置工具** | CMake + MSVC / GCC |

---

## 6. 效能預估

| 情境 | RTFx (Real-Time Factor) | 延遲 | 記憶體 |
|------|------------------------|------|--------|
| CPU INT4 (x86-64 AVX2) | ~0.5x | ~1.1s (560ms 音頻 → 500ms 處理) | ~800MB |
| CPU INT4 (ARM64) | ~0.3x | ~1.8s | ~800MB |
| CUDA INT4 | ~3-5x | ~0.7s | ~1.2GB (含 GPU) |
| CUDA FP32 | ~2-3x | ~0.8s | ~2.5GB (含 GPU) |

> **說明**：560ms 的 chunk 約 8960 個 samples (16kHz)，處理時間約 500-600ms 時可達即時門檻。

---

## 7. 結論

1. **完全可行** — 不需 Python。Rust 方案是最實用的選擇。
2. **sherpa-onnx 為最成熟方案** — 已封裝 RNNT 解碼、Mel 前處理、VAD、語言提示。
3. **最低開發量路徑**：sherpa-onnx Rust binding → 約 300-500 行主程式碼。
4. **中等開發量路徑**：ort crate + 自行實作 RNNT → 約 1500-2000 行程式碼。
5. **最高彈性路徑**：ONNX Runtime C++ GenAI API → 約 500-800 行程式碼。
