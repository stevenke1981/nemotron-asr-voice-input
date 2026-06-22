# 工作項目：Nemotron ASR 語音輸入法

## 說明

- 狀態：`[ ]` 待辦 / `[x]` 完成 / `[-]` 擱置 / `[!]` 受阻
- 優先級：P0 = 必須 (MVP) / P1 = 重要 / P2 = 加分

---

## 2026-06-22：語音轉錄缺字根因與修正（P0）

### 根因（已用 `target/release/voices` 實錄音檔重現）

1. **主要根因：48 kHz 麥克風降採樣到 16 kHz 時沒有 anti-alias low-pass。**
   - 實際裝置紀錄為 Realtek `48,000 Hz / 2 ch / F32`，程式會 fallback 到 48 kHz，再送往 16 kHz Nemotron。
   - 舊 `resample_into()` 對 48k→16k 的比例為整數 3，linear interpolation 會退化成每 3 個 sample 直接取 1 個（decimation）。8 kHz 以上能量因此 alias 回語音頻帶，破壞子音與音節特徵，Nemotron 會漏掉句中詞或以錯字替代。
   - 代表錄音交叉驗證（Whisper 僅用來確認音檔內容完整）：

| 錄音 | 音檔內容對照 | 修正前 Nemotron |
|------|--------------|-----------------|
| `20260622-055807-909164.wav` | 許／去測試看能不能正常 | 去測試看能不能（漏「正常」） |
| `20260622-055813-907448.wav` | 繼續測試有沒有正常 | 一序有沒有正常（漏「測試」） |
| `20260622-055832-052184.wav` | 謝謝你，我會不會覺得很累 | 所以我大西花（整句聲學特徵失真） |
| `20260622-055839-702566.wav` | 所以聽得到我在講什麼 | 所以講什麼（漏「聽得到我在」） |

2. **次要根因：finalize 順序錯誤且即時／batch 各寫一套。**
   - `--file` 路徑在 `input_finished()` 前先 `engine.reset()`，把已有 hypothesis 清空後才讀結果；修正前可穩定重現 partial 有字、`=== Full Transcript ===` 為空。
   - `input_finished()` 本身不足以替 Nemotron 的 `T=65` encoder 提供最後 token 所需的 future context；`20260622-055839-702566.wav` 在無 context 時只得到「所以聽得到我在」，補 800 ms silence 後得到完整「所以聽得到我在講什麼」。
   - VAD 開關 A/B 輸出完全相同，證實「VAD 跳過 padding」不是根因。正確順序統一為：fresh stream → feed 全部 audio → feed 800 ms trailing context → `input_finished()` → `while is_ready()` decode → 讀 final result → fresh stream。

3. **診斷工具問題：WAV reader 把 header 寫死為 44 bytes。**
   - 含 `JUNK`／extended `fmt` chunk 的標準 WAV 會把 header bytes 當 PCM，讓 A/B 結果受到額外雜訊干擾。

### 修正項目

- [x] 以 `rubato::FftFixedInOut` stateful band-limited resampler 取代無 low-pass 的 linear decimation。
- [x] 每段錄音開始時清除 resampler filter history；停止時 flush partial block 與 filter delay，並維持精確音訊長度。
- [x] 新增 anti-alias 回歸測試：48 kHz 的 12 kHz tone 降到 16 kHz 後 RMS 必須 `< 0.02`，避免 alias 成 4 kHz。
- [x] 新增 speech-band／長度回歸測試：1 kHz tone 保留且 1 秒輸出恰為 16,000 samples。
- [x] 新增共用 `decode_complete_utterance()`，即時與 batch 使用同一個 sherpa-onnx canonical finalize 流程。
- [x] 加入符合 `T=65` receptive field 的 800 ms trailing context；移除未經證實的 VAD toggle 與固定 60 次 decode 上限。
- [x] 改為走訪 RIFF chunks 的 PCM16 mono WAV parser，並加入 non-44-byte header 測試。
- [x] 完整驗證：17 tests 全過、`cargo clippy --all-targets` 成功（僅 23 個既有 warnings）、release build 成功、30 檔 batch 中 29 檔產生 final。
- [!] 嚴格 `-D warnings` 仍被本任務前既有的 UI/dead-code/clippy warnings 阻擋；本次新增的 resampler／finalize／WAV parser 沒有新增 clippy warning。
- [x] git commit + push，確認 local/remote SHA 一致。

---

## 階段一：MVP 最小可行產品

### Milestone 1: 專案骨架 ✅

- [x] P0 `cargo init` 建立專案，設定 `Cargo.toml` 所有依賴
- [x] P0 建立模組目錄結構 (`src/audio/`, `src/asr/`, `src/injector/`, `src/config/`, `src/ui/`, `src/hotkey/`, `src/download/`)
- [x] P0 實作 `src/config/settings.rs` — TOML 設定檔載取 + 寫入
- [x] P0 實作模型下載 `src/download/mod.rs` — ureq HuggingFace 下載
- [x] P0 設定 tracing/logging
- [x] P0 驗證：`cargo build` 成功

### Milestone 2: 音頻擷取

- [ ] P0 實作 `src/audio/capture.rs` — 列舉裝置、選擇預設麥克風
- [ ] P0 實作 cpal WASAPI 串流回呼，捕獲 16kHz mono f32 PCM
- [ ] P0 實作 `src/audio/ringbuf.rs` — lock-free SPSC 環形緩衝區 (8960 samples)
- [ ] P1 處理取樣率轉換 (若裝置非 16kHz)
- [ ] P1 處理立體聲混音為單聲道
- [ ] P0 驗證：`cargo run -- --dump-audio` 輸出 WAV 檔案確認正確

### Milestone 3: ASR 轉錄

- [x] P0 實作模型下載/路徑組態 (`src/download/mod.rs`)，準備 `models/` 目錄
- [ ] P0 實作 `src/asr/sherpa.rs` — sherpa-onnx OnlineRecognizer 初始化
- [ ] P0 實作音頻餵入：ringbuf → `stream.accept_waveform()`
- [ ] P0 實作解碼循環：定時 `recognizer.decode()`
- [ ] P0 實作 `get_transcript()` — 擷取轉錄文字
- [ ] P0 實作 `src/asr/mod.rs` — `AsrEngine` trait 與 `AsrConfig`
- [ ] P0 實作 VAD 設定：`stream.set_option("use_vad", "true")`
- [ ] P0 實作語言設定：`stream.set_option("language", "zh")`
- [ ] P0 實作引擎重置與狀態清理
- [ ] P0 驗證：`cargo run -- --file test.wav --language en` 比對轉錄結果

### Milestone 4: 文字注入

- [ ] P0 實作 `src/injector/sendinput.rs` — Win32 SendInput, KEYEVENTF_UNICODE
- [ ] P0 處理 Unicode 字元 (中/日/韓等多位元組)
- [ ] P0 實作 `src/injector/clipboard.rs` — 剪貼簿 + Ctrl+V 後備方案
- [ ] P1 實作 `src/injector/uiautomation.rs` — UIA ValuePattern 注入
- [ ] P0 實作 `src/injector/mod.rs` — `TextInjector` trait + 策略切換
- [ ] P1 實作注入前後保留/恢復剪貼簿
- [ ] P0 驗證：在記事本、瀏覽器、IDE 中注入中英文混合文字

### Milestone 5: MVP 整合

- [ ] P0 實作主事件迴圈 (音頻 → ASR → 注入管線)
- [ ] P0 跨執行緒通訊：`crossbeam::channel` 傳遞 `TranscriptResult`
- [ ] P0 實作 `src/hotkey/register.rs` — `RegisterHotKey` (Ctrl+Alt+R)
- [ ] P0 基本錯誤處理：模型缺失提示、麥克風錯誤恢復
- [ ] P0 狀態管理：閒置/錄音中/轉錄中/暫停
- [ ] P0 CLI 參數解析：`--language`, `--model-dir`, `--list-devices`
- [ ] P0 MVP 端到端測試：開啟記事本 → 說話 → 文字出現

---

## 階段二：強化

### 系統匣與 UI

- [x] P1 實作 `src/ui/tray.rs` — Windows 系統匣圖示 (內建雙語選單)
- [x] P1 右鍵選單：啟用/停用、語言切換、強制結束、設定、離開
- [x] P1 轉錄狀態提示 (系統匣 balloon notification，雙語字串)
- [x] P2 設定視窗 (Win32 modeless dialog，雙語介面) ✅
- [x] P1 中英文雙語字串模組 `src/ui/strings.rs` (60+ 字串) ✅
- [x] P1 設定寫入 `config.toml` (`AppConfig::save()`) ✅
- [x] P1 `set_ui_lang()` 系統匣語言靜態切換 ✅
- [x] P1 程式化圖示 (16×16 GDI 彩色圓形) ✅
- [ ] P2 轉錄浮窗 overlay (可選)

### 延遲與效能優化

- [ ] P1 環形緩衝區大小調校與延遲測量
- [ ] P1 WASAPI 獨佔模式測試 (降低延遲)
- [ ] P1 編碼器執行緒設定即時優先級
- [ ] P2 CUDA 執行提供者支援 (`--provider cuda`)
- [ ] P2 ASYNC 解碼模式 (run_async)

### 多語言支援

- [x] P1 完整 85+ 種語言 ID 對照表 (`src/asr/config.rs`)
- [x] P1 語言快捷切換 (Ctrl+Alt+L 循環切換)
- [ ] P2 自動語言偵測支援 (不設定 language，讓模型自動判斷)
- [ ] P2 每視窗/每應用程式語言記憶

### 穩定性

- [x] P1 watchdog 線程：30 秒健康檢查記號 (logging tick)
- [x] P1 音頻執行緒即時優先級 (THREAD_PRIORITY_HIGHEST)
- [ ] P1 記憶體使用監控與日誌
- [ ] P1 模型熱重載 (更新模型不需重啟)
- [ ] P2 長時間運作測試 (24h+)
- [ ] P2 崩潰報告 (panic hook → 日誌檔案)

---

## 階段三：發行準備

### 安裝與發布

- [ ] P2 安裝程式 (InnoSetup / MSI)
- [ ] P2 模型下載引導程式 (首次執行時)
- [ ] P2 自動更新機制
- [ ] P2 數位簽章與程式碼簽章
- [ ] P2 使用者文件 (README, 使用手冊)

### 測試

- [ ] P1 單元測試：各模組獨立測試
- [ ] P1 整合測試：完整管線測試
- [ ] P2 WER 評測：FLEURS 資料集 (英文、中文)
- [ ] P2 相容性測試：Windows 10/11 多版本
- [ ] P2 應用相容性測試：記事本、Chrome、Word、VS Code、終端機

---

## 技術研究項目

- [ ] P0 研究：sherpa-onnx Rust binding 的 API 完整性 (特別是 multilingual SetOption)
- [ ] P0 研究：確認 sherpa-onnx 版本是否已合併 PR #3671
- [ ] P0 研究：Win32 `SendInput` Unicode 路徑與 UIPI 限制測試
- [ ] P0 研究：cpal WASAPI 獨佔模式延遲表現
- [ ] P1 研究：ONNX Runtime CUDA EP 在 INT4 模型上的效能
- [ ] P1 研究：Windows 系統匣實作 (windows crate 方式)
- [ ] P2 研究：輕量級語言模型輔助標點符號恢復

---

## 決策記錄

| 日期 | 決策 | 理由 |
|------|------|------|
| TBD | 使用 sherpa-onnx 而非自幹 RNNT | 節省數百行複雜解碼程式碼 |
| TBD | 使用 cpal 而非直接 WASAPI COM | 純 Rust、跨平台潛力 |
| TBD | ONNX INT4 而非 FP32 | INT4 約 1/4 大小、速度更快 |
| TBD | 預設 CPU provider | 確保最大相容性，CUDA 為選項 |
| 2026-06-21 | 設定視窗使用 Win32 modeless dialog (CreateWindowExW) 而非 DialogBox | 不需 .rc 資源檔，純 Rust 可編譯 |
| 2026-06-21 | 雙語系統使用簡單 match self.lang 模式而非 i18n crate | 兩語言時最輕量、零依賴 |
| 2026-06-21 | `CONFIG_HWND` 使用 `AtomicIsize` 儲存 HWND | 避免 `OnceLock<HWND>` 的 Send/Sync 問題 |
| 2026-06-21 | 控制項列舉使用 `FindWindowExW` + `GetDlgCtrlID` | windows 0.62 中 GetDlgItem 傳回型別問題 |
