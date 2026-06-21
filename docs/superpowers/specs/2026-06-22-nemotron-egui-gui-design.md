# 規格文件：Nemotron Voice Input — egui GUI 強化

## 1. 產品概述

**專案名稱**：nemotron-voice-input
**文件日期**：2026-06-22
**狀態**：設計核准，準備實作
**目標**：在現有 nemotron-voice-input 系統匣應用基礎上，增加 egui 圖形使用者介面（主視窗 + 浮窗 Overlay），提升使用者體驗。

### 現有基礎

目前已完成 MVP：音頻擷取 (cpal) -> ASR 轉錄 (sherpa-onnx + Nemotron 3.5) -> 文字注入 (SendInput/Clipboard) + 系統匣 + Win32 設定視窗。專案以 Rust 實作，可正常編譯執行。

### 強化目標

| 面向 | 現狀 | 目標 |
|------|------|------|
| 主介面 | 僅系統匣圖示 | egui 主視窗（轉錄歷史 + 即時顯示 + 設定） |
| 轉錄顯示 | 僅系統匣通知 | 即時浮窗 + 主視窗歷史記錄 |
| 設定 UI | Win32 modeless dialog | egui 設定面板（可完全取代舊視窗） |
| UI 框架 | Win32 API | egui (eframe + egui-wgpu) |

---

## 2. 系統架構

### 2.1 執行緒模型

主執行緒 (Win32 msg loop) 負責熱鍵管理、系統匣圖示、文字注入器、錄音狀態管理、設定載入/儲存、跨執行緒通訊協調。
它發送 TranscriptResult 到 gui 通道，接收 gui_action 通道（設定變更、語言切換）。

透過 crossbeam 通道連接三個子系統：
- 音頻處理執行緒 (ASR sherpa-onnx) - 現有不變
- Watchdog 執行緒 - 現有不變
- eframe GUI 執行緒（新增）- egui App 主視窗 UI 邏輯
- Overlay 執行緒（新增）- winit + egui 浮窗

### 2.2 通訊通道

| 通道 | 類型 | 方向 | 訊息 |
|------|------|------|------|
| transcript_tx/rx | 現有 bounded(64) | 音頻 -> 主執行緒 | TranscriptResult |
| gui_transcript_tx/rx | 新增 bounded(256) | 主執行緒 -> eframe | TranscriptResult |
| gui_action_tx/rx | 新增 unbounded | eframe -> 主執行緒 | GuiAction 列舉 |
| overlay_text_tx/rx | 新增 bounded(8) | eframe -> overlay | 最新轉錄字串 |

### 2.3 核心資料結構

`ust
/// eframe 收到的轉錄事件
#[derive(Clone)]
pub struct GuiTranscriptEvent {
    pub text: String,
    pub is_final: bool,
    pub segment_id: u32,
    pub timestamp: std::time::Instant,
}

/// 對話歷史記錄
#[derive(Clone)]
pub struct TranscriptEntry {
    pub text: String,
    pub timestamp: String,
    pub language: String,
}

/// GUI -> 主執行緒的行動請求
#[derive(Clone)]
pub enum GuiAction {
    ToggleRecording,
    CycleLanguage,
    Flush,
    SetLanguage(String),
    SaveConfig(AppConfig),
    ShowOverlay(bool),
    Exit,
}
`

---

## 3. egui 主視窗設計

### 3.1 整體佈局

主視窗包含四個主要區域：

1. **狀態列** - 頂部橫條
   - 綠色圓點 + 文字 = 錄音中，灰色圓點 = 閒置
   - 語言顯示當前 ASR 語言名稱
   - 轉換模式顯示（無/簡->繁/繁->簡）
   - 語言和轉換模式可點擊即時切換

2. **即時轉錄面板** - 狀態列下方
   - 顯示最近 1-3 句 is_final: true 的完整句子
   - 當前 is_final: false 的局部結果以灰色斜體顯示
   - 自動滾動至最新內容

3. **對話歷史面板** - 中間主要區域
   - egui ScrollArea::vertical() 實現
   - 按時間排序，最新在底部
   - 每行顯示：時間戳 + 文字 + 操作按鈕
   - 每行右側操作：複製到剪貼簿、刪除
   - 上限 1000 條（可在設定中調整）

4. **底部控制列** - 底部橫條
   - 開始/停止錄音（對應 Ctrl+Shift+F2）
   - 語言循環切換（對應 Ctrl+Shift+L）
   - 設定按鈕（切換到設定面板）

### 3.2 設定面板

以 egui 視窗或側邊欄呈現，取代現有 Win32 modeless dialog：

| 欄位 | 控制項 | 說明 |
|------|--------|------|
| UI 語言 | ComboBox | English / 中文 |
| ASR 語言 | ComboBox | zh / en / ja / de / fr / es / ko ... |
| Provider | ComboBox | cpu / cuda |
| 解碼方法 | ComboBox | greedy_search / modified_beam_search |
| 執行緒數 | DragValue (u32) | 1-16 |
| VAD | Checkbox | 啟用語音活動偵測 |
| 注入策略 | ComboBox | sendinput / clipboard / auto |
| 按鍵延遲 | DragValue (ms) | 0-100 |
| 還原剪貼簿 | Checkbox | 注入後還原 |
| 文字轉換 | ComboBox | 無 / 簡->繁 / 繁->簡 |
| 熱鍵資訊 | 唯讀文字 | 顯示當前綁定 |

### 3.3 視窗行為

| 操作 | 行為 |
|------|------|
| 關閉視窗（×） | 隱藏到系統匣（不退出應用） |
| 系統匣「顯示主視窗」 | 還原 eframe 視窗 |
| 設定儲存 | 寫入 config.toml + 即時通知主迴圈 |
| 語言切換 | 更新 shared state + 通知 main 更新 ASR |

---

## 4. 浮窗 Overlay 設計

### 4.1 視窗規格

| 屬性 | 值 |
|------|-----|
| 尺寸 | 自動適應文字寬度，最大 80% 螢幕寬 |
| 位置 | 螢幕底部中央，可拖曳 |
| 邊框 | 無邊框 (winit set_decorations(false)) |
| 圖層 | always-on-top |
| 工作列 | 不顯示圖示 |
| 背景 | 暗色半透明 (alpha ~0.85) |
| 文字 | 白色、大字體 (20pt+) |

### 4.2 行為

- 僅顯示最新一句 is_final: true 的轉錄文字
- 局部結果不顯示在浮窗中（避免閃爍）
- 滑鼠懸停時 alpha 提升至 0.95（幾乎不透明）
- 閒置 N 秒後可選淡出（P2 功能）
- 可從系統匣選單或主視窗切換顯示/隱藏
- 位置儲存在 config.toml（跨會話記憶）

### 4.3 實作方式

使用 egui-winit + egui-wgpu 直接建立第二個視窗，透過 winit::window::WindowBuilder 設定 always-on-top 和無邊框屬性。在獨立執行緒中運行 winit EventLoop，每幀從通道接收最新文字並繪製。

---

## 5. 模組邊界與檔案結構

### 5.1 新增檔案

`
src/ui/
  mod.rs              # 增加 pub mod gui; pub mod overlay;
  gui/
    mod.rs            # 模組宣告
    state.rs          # GuiSharedState, GuiAction, TranscriptEntry
    app.rs            # eframe::App 實作（主視窗 UI 邏輯）
  overlay/
    mod.rs            # overlay 執行緒啟動 + winit 事件迴圈
    ui.rs             # egui overlay UI 繪製邏輯
  tray.rs             # 修改：增加「顯示主視窗」「顯示浮窗」選單
`

### 5.2 修改檔案

| 檔案 | 修改內容 |
|------|---------|
| Cargo.toml | 新增 eframe, egui-winit, egui-wgpu, winit 依賴 |
| src/main.rs | 啟動 eframe 執行緒 + overlay 執行緒；設定通道；處理 GuiAction |
| src/ui/tray.rs | 增加「顯示主視窗」「顯示/隱藏浮窗」選單項目 |
| src/ui/mod.rs | 匯出 gui 和 overlay 子模組 |
| src/ui/config_window.rs | 階段三後標記 deprecated，可選移除 |

### 5.3 不修改檔案（維持不變）

audio/, asr/, injector/, config/, convert/, hotkey/ 模組完全不修改。

---

## 6. 新增依賴

`	oml
[dependencies]
eframe = { version = "0.31", features = ["wgpu"] }
egui-winit = "0.31"
egui-wgpu = "0.31"
winit = "0.30"
egui = "0.31"
`

所有 egui 相關 crate 版本鎖定 0.31，版本不匹配會造成編譯錯誤。

---

## 7. 錯誤處理與邊界

| 情境 | 處理方式 |
|------|---------|
| eframe 初始化失敗 | 退回純系統匣模式，記錄錯誤，不影響核心功能 |
| overlay 初始化失敗 | 不影響主應用，僅記錄錯誤 |
| 通道滿載 | 捨棄最舊事件，保留最新 N 條 |
| 設定寫入失敗 | GUI 顯示錯誤提示，不崩潰 |
| GUI 執行緒崩潰 | 不影響音頻處理、ASR、注入等核心管線 |
| 模型未下載 | 設定面板顯示引導提示 |

**降級策略**：egui GUI 是選配層。若 egui 初始化失敗，應用退回純系統匣模式，所有現有功能（熱鍵、轉錄、注入）完全不受影響。

---

## 8. 實作階段

### 階段一：egui 主視窗基礎（P0，3-5 工時）

| 任務 | 檔案 | 說明 |
|------|------|------|
| 1.1 新增依賴 | Cargo.toml | 加入 eframe, egui-winit, egui-wgpu, winit |
| 1.2 建立 GUI 模組 | src/ui/gui/mod.rs | 模組宣告 |
| 1.3 GUI 共享狀態 | src/ui/gui/state.rs | GuiSharedState + 通道類型 |
| 1.4 eframe 應用入口 | src/ui/gui/app.rs | egui::App 實作，載入主視窗佈局 |
| 1.5 狀態列面板 | src/ui/gui/app.rs | 錄音狀態、語言、轉換模式 |
| 1.6 即時轉錄面板 | src/ui/gui/app.rs | 最新轉錄文字顯示 |
| 1.7 底部控制列 | src/ui/gui/app.rs | 開始/停止、語言切換、設定按鈕 |
| 1.8 通道串接 | src/main.rs | 主迴圈發送 TranscriptResult 到 gui 通道 |
| 1.9 系統匣整合 | src/ui/tray.rs | 增加「顯示主視窗」選單項目 |
| 驗證 | cargo run | 主視窗顯示、轉錄文字即時更新 |

**交付門檻**：cargo run -> 主視窗顯示，說話後轉錄文字出現在即時轉錄區

### 階段二：對話歷史面板（P0，2-3 工時）

| 任務 | 檔案 | 說明 |
|------|------|------|
| 2.1 歷史資料結構 | src/ui/gui/state.rs | TranscriptEntry 結構 + Vec 儲存 |
| 2.2 歷史面板 UI | src/ui/gui/app.rs | ScrollArea 列表、時間戳、操作按鈕 |
| 2.3 複製功能 | src/ui/gui/app.rs | 複製到剪貼簿 |
| 2.4 刪除功能 | src/ui/gui/app.rs | 刪除單條/清除全部 |
| 2.5 面板整合 | src/ui/gui/app.rs | 歷史面板嵌入主佈局 |
| 驗證 | cargo run | 多次錄音後歷史正確顯示、可複製刪除 |

**交付門檻**：多次錄音後歷史正確累積，複製和刪除功能正常

### 階段三：設定面板（P0，2-3 工時）

| 任務 | 檔案 | 說明 |
|------|------|------|
| 3.1 設定面板 UI | src/ui/gui/app.rs | 所有設定欄位 |
| 3.2 設定讀寫 | src/ui/gui/app.rs | 呼叫 AppConfig::load/save |
| 3.3 GuiAction 處理 | src/main.rs | 接收設定變更通道訊息並應用 |
| 3.4 config_window 標記 | src/ui/config_window.rs | 標記為 deprecated |
| 驗證 | cargo run | 設定可讀寫、即時生效 |

**交付門檻**：設定修改後寫入 config.toml，重啟後保留

### 階段四：浮窗 Overlay（P1，4-6 工時）

| 任務 | 檔案 | 說明 |
|------|------|------|
| 4.1 overlay 模組 | src/ui/overlay/mod.rs | winit 視窗建立 + 事件迴圈 |
| 4.2 浮窗 UI | src/ui/overlay/ui.rs | egui 即時模式繪製 |
| 4.3 文字通道 | src/ui/overlay/mod.rs | 從 eframe 接收最新文字 |
| 4.4 always-on-top | src/ui/overlay/mod.rs | winit 設定 |
| 4.5 透明度控制 | src/ui/overlay/ui.rs | 滑鼠懸停/閒置切換 |
| 4.6 系統匣整合 | src/ui/tray.rs | 「顯示/隱藏浮窗」選單 |
| 4.7 主視窗整合 | src/ui/gui/app.rs | 浮窗切換按鈕 |
| 驗證 | cargo run | 浮窗顯示、即時更新、可拖曳 |

**交付門檻**：浮窗顯示轉錄文字，always-on-top，可切換顯示/隱藏

### 階段五：打磨與發行（P2，2-3 工時）

| 任務 | 說明 |
|------|------|
| 5.1 視窗位置記憶 | 主視窗 + 浮窗位置儲存到 config |
| 5.2 egui 主題切換 | 亮/暗主題支援 |
| 5.3 浮窗淡出計時器 | 閒置數秒後自動淡出 |
| 5.4 效能優化 | egui 渲染調校、減少不必要重繪 |
| 5.5 清理舊程式碼 | 移除 config_window.rs、清理未使用依賴 |
| 驗證 | cargo run | 完整功能測試 + 回歸測試 |

---

## 9. 驗證標準

| 門檻 | 驗證方式 |
|------|---------|
| 主視窗啟動 | cargo run -> 視窗正常顯示 |
| 轉錄即時更新 | 按下錄音 -> 文字出現在即時轉錄區 |
| 歷史記錄 | 多次錄音 -> 歷史區域正確累積 |
| 設定讀寫 | 修改設定 -> 關閉 -> 重啟 -> 設定保留 |
| 浮窗顯示 | 開啟浮窗 -> 自動顯示轉錄文字 |
| 系統匣整合 | 關閉視窗 -> 系統匣圖示保留 |
| 無回歸 | 熱鍵、注入、系統匣原有功能正常 |
| 編譯無警告 | cargo build + cargo clippy |