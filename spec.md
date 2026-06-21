# 規格文件：Nemotron ASR 語音輸入法

## 1. 產品概述

**專案名稱**：Nemotron Voice Input (nemotron-voice-input)  
**目標**：建立一個常駐背景的 Windows 語音輸入法應用，透過 NVIDIA Nemotron 3.5 ASR 模型即時轉錄麥克風語音，並將轉錄文字注入當前焦點視窗。  
**技術限制**：不使用 Python。僅使用 Rust 或 C/C++。  
**目標使用者**：需要高效率語音輸入的繁體中文 / 多語使用者。

---

## 2. 功能需求

### 2.1 核心功能

| ID | 功能 | 說明 | 優先級 |
|----|------|------|--------|
| F-01 | 麥克風即時音頻擷取 | 從預設麥克風捕捉 16kHz 單聲道 PCM | P0 |
| F-02 | 即時 ASR 轉錄 | 使用 Nemotron 3.5 模型將語音轉為文字 | P0 |
| F-03 | 文字注入焦點視窗 | 將轉錄文字自動輸入當前活動視窗 | P0 |
| F-04 | 語言切換 | 支援在 40 種語言間切換 | P1 |
| F-05 | 推文/即時模式切換 | 連續辨識 vs. 手動觸發 | P1 |
| F-06 | 自動語言偵測 | 自動偵測語音語言 (模型支援) | P2 |
| F-07 | VAD 語音活動偵測 | 自動判斷語音開始/結束 | P1 |
| F-08 | 系統匣圖示 | 右下角系統匣常駐圖示 + 選單 | P1 |

### 2.2 非功能需求

| ID | 需求 | 目標值 | 優先級 |
|----|------|--------|--------|
| NF-01 | 端到端延遲 | < 1.5 秒 (從語音到文字出現) | P0 |
| NF-02 | CPU 使用率 | 非轉錄時 < 1%，轉錄時 < 30% | P1 |
| NF-03 | 記憶體使用 | < 1 GB (峰值) | P1 |
| NF-04 | 啟動時間 | < 5 秒 (含模型載入) | P1 |
| NF-05 | 無 Python 依賴 | 僅使用 Rust/C/C++ 原生程式碼 | P0 |
| NF-06 | Windows 相容 | Windows 10 20H2+, Windows 11 | P0 |
| NF-07 | 中文字元輸入 | 正確注入 Unicode 中文字 | P0 |

---

## 3. 系統架構

### 3.1 模組劃分

```
nemotron-voice-input/
├── src/
│   ├── main.rs                    # 入口、系統匣、事件迴圈
│   ├── audio/
│   │   ├── mod.rs                 # 音頻模組
│   │   ├── capture.rs             # WASAPI 麥克風擷取 (cpal)
│   │   ├── resampler.rs           # 重取樣 (若需要)
│   │   └── ringbuf.rs             # 音頻環形緩衝區
│   ├── asr/
│   │   ├── mod.rs                 # ASR 抽象層
│   │   ├── sherpa.rs              # sherpa-onnx 引擎封裝
│   │   └── config.rs              # 模型組態
│   ├── injector/
│   │   ├── mod.rs                 # 文字注入模組
│   │   ├── sendinput.rs           # SendInput 實作
│   │   ├── uiautomation.rs        # UIAutomation 實作
│   │   └── clipboard.rs           # 剪貼簿後備方案
│   ├── ui/
│   │   ├── mod.rs                 # 使用者介面
│   │   ├── tray.rs                # 系統匣圖示 (雙語選單、通知)
│   │   ├── strings.rs             # 中英文雙語字串模組 (60+ 字串)
│   │   ├── overlay.rs             # 轉錄浮窗 (可選)
│   │   └── config_window.rs       # 設定視窗 (Win32 modeless dialog)
│   ├── config/
│   │   ├── mod.rs                 # 設定管理
│   │   └── settings.rs            # 設定結構與序列化
│   └── hotkey/
│       ├── mod.rs                 # 快捷鍵管理
│       └── register.rs            # 全域熱鍵註冊
├── models/                        # ONNX 模型檔案 (下載後)
│   ├── encoder.onnx
│   ├── decoder.onnx
│   ├── joint.onnx
│   ├── silero_vad.onnx
│   ├── tokenizer.json
│   └── tokens.txt
├── Cargo.toml
└── build.rs                       # 建置腳本 (模型下載等)
```

### 3.2 資料流

```
麥克風類比音頻
    │
    ▼
WASAPI 驅動程式 (16kHz, mono, f32)
    │
    ▼
cpal 音頻串流回呼
    │
    ▼
Ring Buffer (鎖定自由佇列, 8960 samples = 560ms 區塊)
    │
    ▼
sherpa-onnx OnlineStream.accept_waveform()
    │
    ▼
sherpa-onnx OnlineRecognizer.decode()
    ├── Mel 頻譜前處理 (內建)
    ├── Encoder 推論 (FastConformer)
    ├── RNNT Greedy Search 解碼
    │   ├── Decoder LSTM 推論
    │   └── Joiner 推論
    └── 文字輸出
    │
    ▼
轉錄文字
    │
    ▼
TextInjector
    ├── [優先] UIAutomation → SetValue
    ├── [預設] SendInput → 模擬鍵盤輸入
    └── [後備] 剪貼簿 → Ctrl+V
    │
    ▼
焦點應用程式視窗
```

---

## 4. 介面設計

### 4.1 ASR 引擎抽象層

```rust
/// ASR 引擎統一介面
pub trait AsrEngine: Send {
    /// 初始化引擎，載入模型
    fn initialize(&mut self, config: &AsrConfig) -> Result<(), AsrError>;

    /// 餵入音頻資料 (16kHz, mono, f32 PCM)
    fn feed_audio(&mut self, samples: &[f32]) -> Result<(), AsrError>;

    /// 取得當前轉錄結果 (部分/最終)
    fn get_transcript(&mut self) -> Result<TranscriptResult, AsrError>;

    /// 重置引擎狀態
    fn reset(&mut self) -> Result<(), AsrError>;

    /// 設定語言
    fn set_language(&mut self, lang: &str) -> Result<(), AsrError>;
}

#[derive(Debug, Clone)]
pub struct TranscriptResult {
    pub text: String,
    pub is_final: bool,
    pub segment_id: u32,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct AsrConfig {
    pub model_dir: PathBuf,
    pub provider: String,       // "cpu" | "cuda"
    pub num_threads: u32,
    pub chunk_size_ms: u32,     // 560 (default)
    pub use_vad: bool,
    pub language: String,       // "en", "zh", "de", ... 或 "auto"
}
```

### 4.2 文字注入器抽象層

```rust
/// 文字注入器統一介面
pub trait TextInjector: Send {
    /// 注入文字到焦點視窗
    fn inject_text(&mut self, text: &str) -> Result<(), InjectorError>;

    /// 檢查目前是否可用
    fn is_available(&self) -> bool;
}

/// 注入策略
pub enum InjectStrategy {
    Uiautomation,   // 優先
    SendInput,      // 預設
    Clipboard,      // 後備
    Auto,           // 自動選擇
}
```

### 4.3 系統匣圖示

- 右鍵選單 (雙語：支援中/英文即時切換)：
  - 啟用/停用轉錄 (Toggle Recording)
  - 語言選擇 (子選單：40 種語言)
  - 切換語言 (Cycle Language)
  - 強制結束語句 (Flush)
  - 設定 (Settings) → 開啟設定視窗
  - 離開
- 狀態指示圖示：程式化繪製 16×16 彩色圓形（綠 = 錄音中，灰 = 閒置）
- 氣球通知：`send_tray_notification()` 公開輔助函式，字串自動依 `UiLang` 本地化

### 4.4 設定視窗

- **實作方式**：Win32 modeless dialog (`CreateWindowExW` + 自訂 WNDPROC)
- **無資源檔依賴**：所有控制項在程式碼中以 `CreateWindowExW` 建立
- **按鈕**：Save (儲存) / Cancel (取消)
- **設定欄位**：

| 欄位 | 控制項類型 | 說明 |
|------|-----------|------|
| UI Language | Combo box (英/中) | 切換整個介面語言，即時重繪 |
| ASR Language | Combo box (22 種語言代碼) | 指定 ASR 轉錄語言 |
| Provider | Combo box (cpu/cuda) | 推論執行提供者 |
| Decoding method | Combo box (greedy_search / modified_beam_search) | 解碼方法 |
| Num Threads | Edit box (數字) | 推論執行緒數 |
| VAD | Checkbox | 啟用/停用語音活動偵測 |
| Injection strategy | Combo box (sendinput / clipboard / auto) | 文字注入策略 |
| Key delay (ms) | Edit box (數字) | 按鍵模擬延遲 |
| Restore clipboard | Checkbox | 注入後還原剪貼簿內容 |
| Hotkeys | Static text (顯示按鍵綁定) | Ctrl+Alt+R / L / Space |
| Model status | Static text | 模型檔案存在狀態 |

- **儲存行為**：寫入 `config.toml`，部分設定需重啟應用程式生效
- **單例保護**：`CONFIG_HWND` `AtomicIsize` 防止重複開啟
- **背景顏色**：`GetSysColorBrush(COLOR_WINDOW)` 跟隨系統主題
- **控制項列舉**：`FindWindowExW` + `GetDlgCtrlID` 取得子控制項 HWND

### 4.5 雙語字串系統

- **UiLang 列舉**：`English` / `Chinese`
- **Strings 結構**：60+ 方法，每個方法以 `match self.lang` 回傳對應語言字串
- **涵蓋範圍**：
  - 系統匣選單項目
  - 通知提示文字
  - 設定視窗標籤與按鈕
  - 快捷鍵名稱
  - 語言顯示名稱 (language_display_name 函式)
- **設計原則**：無 i18n 框架，純 Rust match pattern，兩語言時最輕量方案

### 4.6 快捷鍵

| 快捷鍵 | 功能 |
|--------|------|
| `Ctrl+Alt+R` | 開始/停止轉錄 (Toggle) |
| `Ctrl+Alt+L` | 切換語言 (循環) |
| `Ctrl+Alt+Space` | 強制結束當前語句 (Flush) |

---

## 5. 模型組態

### 5.1 genai_config.json (ONNX Runtime GenAI 格式)

```json
{
  "model": {
    "type": "nemotron_speech",
    "vocab_size": 13088,
    "num_mels": 128,
    "sample_rate": 16000,
    "chunk_samples": 8960,
    "blank_id": 13087,
    "max_symbols_per_step": 10,
    "encoder": {
      "filename": "encoder.onnx",
      "hidden_size": 1024,
      "inputs": {
        "audio_features": "audio_signal",
        "cache_last_channel": "cache_last_channel",
        "cache_last_time": "cache_last_time",
        "lang_id": "lang_id"
      },
      "outputs": {
        "encoder_outputs": "outputs",
        "cache_last_channel_next": "cache_last_channel_next",
        "cache_last_time_next": "cache_last_time_next"
      }
    },
    "decoder": {
      "filename": "decoder.onnx",
      "hidden_size": 640,
      "inputs": {
        "targets": "targets",
        "lstm_hidden_state": "h_in",
        "lstm_cell_state": "c_in"
      },
      "outputs": {
        "outputs": "decoder_output",
        "lstm_hidden_state": "h_out",
        "lstm_cell_state": "c_out"
      }
    },
    "joiner": {
      "filename": "joint.onnx",
      "inputs": {
        "encoder_outputs": "encoder_output",
        "decoder_outputs": "decoder_output"
      },
      "outputs": {
        "logits": "joint_output"
      }
    },
    "vad": {
      "filename": "silero_vad.onnx",
      "threshold": 0.3,
      "silence_duration_ms": 3360,
      "prefix_padding_ms": 560
    }
  }
}
```

### 5.2 語言 ID 對照表 (部分)

| 語言 | 代碼 | lang_id |
|------|------|---------|
| English | en | 0 |
| Mandarin Chinese | zh | 9 |
| German | de | 8 |
| Japanese | ja | 17 |
| Korean | ko | 18 |
| French | fr | 5 |
| Spanish | es | 3 |
| ... (共 40 種) | | |

---

## 6. 錯誤處理

| 錯誤情境 | 處理方式 |
|----------|---------|
| 模型載入失敗 | 系統匣顯示錯誤提示，無法啟動轉錄 |
| 麥克風無權限 | 提示使用者開啟麥克風權限 (Windows 設定) |
| 音頻緩衝區溢出 | 捨棄最舊音頻，記錄警告 |
| ASR 引擎逾時 | 重置引擎狀態，重新開始 |
| 文字注入失敗 | 自動降級至下一策略 (SendInput → Clipboard) |
| GPU 記憶體不足 | 自動降級至 CPU 執行 |

---

## 7. 效能目標

| 指標 | 目標值 | 測量方式 |
|------|--------|---------|
| 轉錄延遲 (p50) | < 800ms | 音頻結束 → 文字出現 |
| 轉錄延遲 (p99) | < 1500ms | 同上 |
| CPU 使用率 | < 25% (4 核) | 工作管理員 |
| 記憶體使用 | < 600MB (閒置) / < 900MB (轉錄) | Process Explorer |
| 模型載入時間 | < 3 秒 | 首次初始化 |
| WER (英文) | < 8% | FLEURS 測試集 |
| WER (中文) | < 15% | 自建測試集 |
| 穩定運行時間 | 連續 24 小時無崩潰 | 壓力測試 |

---

## 8. 安全與權限

- **最小權限原則**：不需要管理員權限即可執行
- **麥克風權限**：需 `microphone` 功能 (Windows App Capability)
- **網路權限**：不需要（模型為本地推論）
- **UIPI 考量**：若目標為管理員權限視窗，需特殊處理注入方式
- **無資料收集**：所有音頻資料在本地處理，不上傳

---

## 9. 交付產物

| 產物 | 說明 |
|------|------|
| `nemotron-voice-input.exe` | 可執行檔 (Windows x86-64) |
| `models/` | 模型目錄 (使用者自行下載) |
| `config.toml` | 使用者設定檔 |
| 安裝腳本 | 模型下載 + 安裝輔助 |
