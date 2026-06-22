# Nemotron ASR Voice Input

> Real-time streaming speech recognition + text injection for Windows.
> NVIDIA Nemotron-3.5-ASR model + sherpa-onnx + Rust.
>
> 即時語音辨識 + 文字注入工具，給 Windows 使用。
> NVIDIA Nemotron-3.5-ASR 模型 + sherpa-onnx + Rust。

---

## Features / 功能特色

- **Real-time streaming ASR** — speech-to-text as you speak, with full-audio decode on stop for maximum accuracy
- **Push-to-talk (PTT)** — hold Ctrl+Shift+L, speak, release; injects transcribed text
- **Toggle mode** — Ctrl+Shift+R to toggle recording on/off
- **Auto language detection** — supports 40+ languages (Nemotron model built-in language ID)
- **Multiple text injection strategies** — SendInput (Unicode), clipboard + Ctrl+V, UIAutomation
- **Modern native GUI** — settings panel, floating overlay, system tray, theme toggle (Dark/Light)
- **GUI VAD threshold slider** — adjust voice activity detection sensitivity at runtime
- **Simplified / Traditional Chinese conversion** — auto-convert based on config
- **System tray** — background operation with notification support
- **Bilingual UI** — English and Traditional Chinese interface

---

- **即時串流 ASR** — 邊說邊轉錄，停止時全音頻解碼以達最高準確度
- **按鍵發話 (PTT)** — 按住 Ctrl+Shift+L，說話，放開；自動注入轉錄文字
- **切換模式** — Ctrl+Shift+R 切換錄音開關
- **自動語言偵測** — 支援 40+ 種語言（Nemotron 內建語言辨識）
- **多種文字注入策略** — SendInput（Unicode）、剪貼簿 + Ctrl+V、UIAutomation
- **現代原生 GUI** — 設定面板、浮動 overlay、系統匣、主題切換（深色/淺色）
- **GUI VAD 閾值滑桿** — 執行中調整語音活動偵測敏感度
- **繁簡轉換** — 依設定自動轉換
- **系統匣** — 背景執行，支援通知
- **雙語介面** — 英文與繁體中文

---

## Quick Start / 快速開始

### Prerequisites / 前置需求

- Windows 10 or later (64-bit)
- A working microphone
- Rust toolchain (2024 edition) — only if building from source

### Download Model / 下載模型

The model is downloaded automatically on first run. It requires ~600 MB.

模型會在首次執行時自動下載，約需 600 MB。

### Build & Run / 編譯與執行

```powershell
# Release build (recommended)
cargo build --release

# First run — downloads model automatically
.\target\release\nemotron-voice-input.exe
```

### CLI Options / 命令列選項

| Flag / 參數 | Description / 說明 | Default / 預設 |
|---|---|---|
| `-c, --config <PATH>` | Config file path / 設定檔路徑 | `config.toml` |
| `-m, --model-dir <DIR>` | Model directory / 模型目錄 | `models/` |
| `-l, --language <CODE>` | Language code / 語言代碼 | `auto` |
| `--provider <cpu\|cuda>` | Execution provider / 執行提供者 | `cpu` |
| `--list-devices` | List audio devices / 列出音訊裝置 | — |
| `--dump-audio` | Dump audio to WAV for debugging / 輸出音訊除錯 | — |
| `--file <PATH>` | Transcribe a WAV file / 轉錄 WAV 檔案 | — |

### Hotkeys / 快速鍵

| Hotkey / 按鍵 | Action / 動作 |
|---|---|
| **Ctrl+Shift+L** | Push-to-talk: hold to record, release to inject / 按住錄音，放開注入 |
| **Ctrl+Shift+R** | Toggle recording on/off / 切換錄音開關 |
| **Ctrl+Alt+L** | Cycle language (zh→en→ja→de→...) / 循環切換語言 |
| **Ctrl+Alt+Space** | Flush/reset ASR engine / 重設 ASR 引擎 |

---

## Configuration / 設定

Configuration is in `config.toml` (auto-created on first run):

設定檔為 `config.toml`（首次執行自動建立）：

```toml
model_dir = "models"

[audio]
sample_rate = 16000
chunk_size_ms = 700
ringbuf_capacity = 448000

[asr]
provider = "cpu"
num_threads = 4
use_vad = true
vad_threshold = 0.1
decoding_method = "greedy_search"

[language]
language = "auto"
cycle_order = ["zh", "en", "ja", "de", "fr", "es", "ko"]

[ui]
language = "zh"
theme = "Dark"

[conversion]
mode = "s2t"    # s2t = Simplified→Traditional, t2s = Traditional→Simplified
```

---

## Architecture / 架構

```
┌─────────────────────────────────────────────────┐
│  main()                                          │
│  ├─ win32_background_loop() — hotkeys + tray     │
│  └─ audio processing thread                      │
│       ├─ cpal WASAPI capture (48kHz → 16kHz)     │
│       ├─ rubato band-limited resampler            │
│       ├─ ring buffer (lock-free SPSC)            │
│       ├─ sherpa-onnx streaming decode            │
│       └─ full-audio decode on stop               │
├─ eframe GUI thread (settings, overlay, history)  │
└─ injector (SendInput / clipboard / UIAutomation) │
└─────────────────────────────────────────────────┘
```

### Key modules / 主要模組

| Module / 模組 | Path / 路徑 | Purpose / 用途 |
|---|---|---|
| **audio** | `src/audio/` | Capture, ring buffer, band-limited resampling |
| **asr** | `src/asr/` | ASR engine trait + sherpa-onnx implementation |
| **config** | `src/config/` | TOML config loading, runtime settings |
| **injector** | `src/injector/` | Text injection strategies |
| **hotkey** | `src/hotkey/` | Win32 hotkey registration + dispatch |
| **ui** | `src/ui/` | System tray, strings, eframe GUI, overlay |
| **convert** | `src/convert/` | Simplified / Traditional Chinese conversion |
| **download** | `src/download/` | Model download from HuggingFace |

### ASR Engine / ASR 引擎

- **Model**: NVIDIA Nemotron-3.5-ASR-Streaming-0.6B (INT4 quantized)
- **Framework**: sherpa-onnx 1.13 (online streaming transducer)
- **Chunk size**: 700ms (T_ = 65 frames receptive field + overhead)
- **Decoder**: greedy search (default) or modified beam search
- **Features**: streaming decode + full-audio reset on stop for complete context
- **VAD**: Silero VAD with runtime-adjustable threshold

---

## Development / 開發

### Building / 建置

```powershell
# Check (fast)
cargo check

# Tests
cargo test

# Lint
cargo clippy --all-targets

# Release build
cargo build --release
```

### Project files / 專案文件

| File / 檔案 | Purpose / 用途 |
|---|---|
| `docs/plan.md` | Implementation plan / 實作計畫 |
| `docs/spec.md` | Specification / 規格書 |
| `docs/todos.md` | Task tracking / 工作追蹤 |
| `docs/lessons.md` | Reusable lessons learned / 經驗教訓 |

### For AI agents / 給 AI 代理

- **Role conventions**: see `~/.config/opencode/TEAM.md` for agent roles (plan/build/architect/reviewer/fixer/fast-researcher)
- **Session memory**: `docs/lessons.md` stores reusable technical lessons
- **Task tracking**: `docs/todos.md` tracks all pending/completed work
- **Styling rules**: team operates under Controlled Workflow v4 — see `~/.config/opencode/AGENTS.md`
- **Code of conduct**:
  - Prefer small reversible changes
  - Always verify with tests before declaring done
  - Record root cause analysis in `docs/lessons.md`
  - Keep `docs/todos.md` up to date with task status
  - Use Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`)
  - Run `cargo check` → `cargo test` → `cargo clippy` before committing

---

## License / 授權

MIT License — see [LICENSE](LICENSE).

---

## Disclaimer / 免責聲明

This project uses the NVIDIA Nemotron-3.5-ASR model which has its own license terms.
Please refer to [NVIDIA's model card](https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b) for details.

本專案使用 NVIDIA Nemotron-3.5-ASR 模型，該模型有其獨立授權條款。
請參考 [NVIDIA 模型卡](https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b)。
