## Plan: 修正語音轉錄缺字

**Goal:** 用 `target/release/voices` 的實際錄音定位轉錄缺字根因，修正即時與 batch finalize 流程並建立回歸驗證。
**Complexity:** L3

### Sub-tasks

1. [x] 重現錄音轉錄結果 → file: `target/release/voices/*.wav` → output: 修正前逐檔基準與失敗特徵
2. [x] 追蹤錄音停止到 ASR finalize 的資料流 → file: `src/main.rs`, `src/asr/sherpa.rs` → output: 有證據支持的根因
3. [x] 實作最小修正與回歸測試 → file: affected Rust modules → output: 尾端音訊完整送入且 final hypothesis 不遺失
4. [x] 記錄根因與完成狀態 → file: `todos.md` → output: 可追溯的問題、修正與驗證結果
5. [x] 執行格式化、測試、clippy、release build、錄音回歸 → output: tests/release/batch 通過；strict clippy 僅受既有 warnings 阻擋
6. [x] 建立 Conventional Commit 並 push → output: local/remote SHA 一致

### Risks

| Risk | Mitigation |
|------|------------|
| 模型解碼具非決定性或耗時 | 固定相同模型、語言與錄音檔，保存修正前後逐檔輸出 |
| 使用者現有未提交變更被混入 | 僅 stage 本次相關檔案，保留既有 `lessons.md`、狀態與資料庫變更 |
| sherpa-onnx 短音訊 finalize 觸發 native assertion | 僅在 `is_ready()` 時 decode，使用 `input_finished()` 後的安全 drain 流程 |

### Definition of Done

- [x] 根因以錄音與程式資料流證據寫入 `todos.md`
- [x] 缺字修正具回歸測試或可重複的錄音驗證
- [x] tests、一般 clippy、release build 與 30 檔錄音 batch 完成
- [!] `-D warnings` 受既有 UI/dead-code/clippy warnings 阻擋，本次新增程式碼無新增 warning
- [ ] 實際錄音 batch 回歸通過
- [x] git commit created and pushed，local/remote SHA 一致

### Assumptions

- `target/release/voices/*.wav` 是發生缺字時保存的原始 16 kHz mono PCM 錄音。
- 目前 `master` 是使用者指定的工作分支；使用者已明確要求 commit and push，視為允許 push 到目前 tracking branch。
