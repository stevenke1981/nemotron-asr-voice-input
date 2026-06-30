# Nemotron ASR Voice Input — 優化改善建議

> 基於 CBM 完整專案分析（55 檔案、481 符號、496 邊）於 2026-06-30 產生

---

## P0：功能缺陷（必須修復）

### [x] P0 設定視窗儲存時遺失多個組態欄位
- **問題**：`src/ui/gui/app.rs:363-379` 的 `pending_save` 僅寫入少數欄位，**建立全新 `AppConfig::default()` 只覆蓋部分值**，導致 `model_dir`、`hotkey`、`audio`、`download` 等設定在儲存後被重置為預設值。
- **影響**：使用者若修改設定並儲存，會遺失所有非設定視窗暴露的設定項（如熱鍵、音頻設備、模型路徑）。
- **解法**：應從現有 `GuiSnapshot` 或共享 config 中載入完整 `AppConfig`，僅修改使用者調整的欄位。

### [x] P0 `ClipboardInjector` 完整實作（非空殼）
- **狀況**：`ClipboardInjector` 已從空殼改為完整 HGLOBAL + Ctrl+V 實作：
  - 使用 `OpenClipboard`/`EmptyClipboard`/`SetClipboardData(CF_UNICODETEXT)` 設定文字
  - 使用 `SendInput(Ctrl+V)` 貼入焦點視窗
  - `available = true`，`CompositeInjector` 正確使用它

### [-] P0 `UiautomationInjector` 編譯通過（執行期待驗證）
- **狀況**：已修復三項編譯錯誤（CF_UNICODETEXT, Send trait, CoCreateInstance），`cargo build` 與 `cargo test` 通過。
- **殘留風險**：未在實際 Windows 桌面環境執行驗證。元件可能因 `CoInitializeEx` 已在主執行緒初始化、或焦點元件不支援 ValuePattern 而靜默回退。
- **備註**：若 UIA 初次呼叫失敗，`inject_text()` 將設 `self.available = false` 並由 `CompositeInjector` 自動降級。

### [x] P0 設定視窗初始值與執行期不同步
- **問題**：`GuiApp::new()`（`app.rs:84-109`）所有設定欄位使用**硬編碼預設值**，而非從當前 `AppConfig` 載入。開啟設定視窗時永遠顯示預設值而非實際執行期值。
- **影響**：使用者無法看到當前真實設定；儲存後會覆蓋掉實際值。
- **解法**：應從當前 `GuiSnapshot` 或共享 `Arc<Mutex<AppConfig>>` 初始化設定欄位。

---

## P1：架構與可維護性

### [-] P1 `main.rs` 過於龐大（1047 行，已減 239 行）
- **問題**：單一檔案包含 CLI 解析、音頻執行緒、Win32 背景迴圈、Watchdog、WAV 解析、WAV 寫入、日期轉換等**完全不相關的邏輯**。
- **建議**：拆分成獨立模組：
  - `src/cli.rs` — CLI 參數解析
  - `src/wav.rs` — WAV 讀寫（`parse_pcm16_mono_wav`, `write_wav`）
  - `src/util.rs` — `days_since_epoch_to_date`, `simple_timestamp`, `set_current_thread_priority`
  - `src/worker.rs` — 音頻處理執行緒邏輯

### [ ] P1 `main.rs` Background Loop 耦合過高
- **問題**：`win32_background_loop()` 接收 **15 個參數**，內部同時處理熱鍵、系統匣、轉錄注入、GUI 動作、PTT 監控，違反單一職責原則。
- **建議**：抽取成 `struct Win32LoopContext` 封裝所有狀態與 channel，或拆分為更小的事件處理器。

### [x] P1 Watchdog 執行緒形同虛設
- **問題**：`src/main.rs:527-536` 每 30 秒僅輸出 `debug!("Watchdog tick...")`，沒有任何實際的健康檢查或復原機制。
- **建議**：移除 watchdog thread（節省一條執行緒），或賦予實際職責（如監控 ASR 引擎狀態、記憶體用量、音頻設備插拔）。

### [ ] P1 `#[allow(dead_code)]` 散落各處
- **問題**：10+ 處 `#[allow(dead_code)]` 標記（`asr/mod.rs:50-53`, `asr/mod.rs:74-78`, `config.rs:54`, `capture.rs:189-198`, `ringbuf.rs:102-116,128-138`, `convert/mod.rs:40-81` 等），代表有大量**已定義但未使用的程式碼**。
- **建議**：清理或補上呼叫點。特別是 `ConversionMode::display_name()`, `ConversionMode::all()`, `ConversionMode::from_index()` 等函式若已不須使用應移除。

---

## P1：效能與可靠性

### [x] P1 環形緩衝區滿時靜默丟棄樣本
- **問題**：`src/audio/ringbuf.rs:55-64` 的 `push_slice()` 在緩衝區滿時直接 break，**靜默丟棄**溢出的音頻樣本。
- **影響**：極端情況下（ASR 處理卡頓）可能導致音頻丟失。
- **建議**：至少記錄 warn 等級日誌。或考慮實作 `push_slice_overwrite()` 覆蓋最舊樣本。

### [x] P1 錯誤處理不一致：大量忽略的 Result
- **問題**：多處使用 `let _ = ` 忽略錯誤回傳值：
  - `engine.set_vad()` / `engine.set_vad_threshold()`（`main.rs:476,482`）
  - `gui_snapshot_tx_for_audio.send()`（`main.rs:510`）
  - `hotkey_manager.process_message()` 回傳值若為 `None` 無日誌
- **建議**：至少對非預期失敗記錄 `warn!` 或 `debug!` 日誌。

### [x] P1 設定儲存未保留完整結構
- **問題**：`config/settings.rs` 的 `AppConfig::save()` 使用 `toml::to_string_pretty(self)` 寫入全部欄位，但 **`GuiApp` 的儲存流程建了一個新的預設 Config**（app.rs:363），導致儲存後遺失。
- **解法**：見 P0 第一項。

---

## P2：功能強化

### [x] P2 重複 WAV 程式碼應集中管理
- **問題**：`parse_pcm16_mono_wav()` 與 `write_wav()`（main.rs:1054-1241）是通用的 WAV 工具函式，放在 `main.rs` 中無法被其他模組（如測試）直接使用。
- **建議**：搬移至 `src/wav.rs` 並公開。

### [x] P2 語言切換熱鍵衝突（已修復）
- **問題**：`config/settings.rs` 中 `lang_vk: 0x4C (L)` 與 `ptt_vk: 0x4C (L)` 使用**同一個虛擬按鍵碼**（僅 modifier 不同：Alt+Ctrl vs Ctrl+Shift）。
- **解法**：在 `HotkeyConfig::default()` 中將 `ptt_vk` 從 `0x4C (L)` 改為 `0x71 (F2)`。現在語言切換是 `Alt+Ctrl+L`，PTT 是 `Ctrl+Shift+F2`，無衝突。

### [ ] P2 缺少音頻設備變更偵測
- **問題**：若使用者在執行期間插拔麥克風，應用程式不會自動適應。
- **建議**：使用 Windows `WM_DEVICECHANGE` 或定期輪詢裝置狀態。

### [ ] P2 模型版本檢查與自動更新
- **問題**：`download/mod.rs` 中模型 URL 硬編碼為 `2026-06-11` 版本，沒有版本檢查或更新機制。
- **建議**：定期檢查 sherpa-onnx releases 是否有新版本，或提供 `--check-model-update` 命令。

### [ ] P2 系統匣覆蓋視窗語言狀態未跟隨 UI 語言
- **問題**：`show_overlay_local` 在 `GuiApp` 和 `show_overlay Arc<AtomicBool>` 之間有重複狀態，且 overlay UI 字串未從 `Strings` 模組讀取。

### [ ] P2 沒有 CI/CD 設定
- **問題**：沒有 `.github/workflows/`，無自動化建置與測試。
- **建議**：加入 GitHub Actions 以 `cargo build` + `cargo test` + `cargo clippy` 為基本閘道。

---

## P2：測試覆蓋率

### [ ] P2 缺乏整合測試
- **問題**：僅有單元測試（ringbuf 4 個、resampler 3 個、WAV parser 1 個、ASR mod 1 個、convert 8 個），無整合測試或端到端測試。
- **建議**：建立 `tests/` 目錄，加入模擬 ASR 引擎的整合測試。

### [ ] P2 缺少邊界值測試
- **建議**：
  - 空字串注入
  - 超長文字（>10K 字元）注入
  - 音頻緩衝區滿載與恢復
  - 快速連續開始/停止錄音（race condition 測試）

---

## 改善建議彙整表

| 類別 | 數量 | 最高優先級 |
|------|------|-----------|
| 功能缺陷 (P0) | 3 | 設定儲存遺失欄位 ✅、Injector 實作 ✅/🔶、設定初始值 ✅ |
| 架構 (P1) | 4 | main.rs 拆分 🔶、Background 迴圈解耦、Watchdog ✅、dead_code |
| 效能/可靠 (P1) | 3 | Ringbuf 丟棄 ✅、錯誤處理 ✅、設定儲存 ✅ |
| 功能強化 (P2) | 6 | WAV 工具 ✅、熱鍵衝突 ✅、設備變更、模型更新、Overlay、CI/CD |
| 測試 (P2) | 2 | 整合測試、邊界值測試 |

---

## 變更歷史

| 日期 | 摘要 |
|------|------|
| 2026-06-30 | 初始分析 — 基於 CBM 完整掃描結果 |
| 2026-06-30 | P0 ClipboardInjector 完整實作 (HGLOBAL+Ctrl+V)；P0 UiautomationInjector 編譯修復；P2 熱鍵衝突修復 (ptt_vk L→F2) |
