# Pixel 螢幕即時翻譯 (Pixel Screen Translate)

這是一款專為 Windows 打造的桌面螢幕即時翻譯工具，設計概念源自 Google Pixel 手機的「智慧鏡頭／螢幕即時翻譯」功能：按下觸發後，程式會自動擷取螢幕畫面，以毫秒級的速度完成文字座標辨識，並採樣文字外框周圍的背景色彩覆蓋原文，直接在螢幕原處渲染翻譯文字，完美保留原始版面配置。

---

## ✨ 核心特色

1. **Pixel 風格原地文字替換（In-Place Replacement）**
   - 提取文字框精確座標（Bounding Box）。
   - 智慧邊界色彩採樣（Border Color Sampling），無痕覆蓋原始文字。
   - 計算背景感知亮度（Luminance），自動套用黑／白高對比清晰文字與字級縮放。
2. **本機離線極速 OCR（Windows.Media.Ocr）**
   - 採用 Windows 10/11 內建的硬體加速離線 OCR API。
   - 無需下載幾百 MB 的龐大深度學習權重，毫秒級辨識、高隱私、零 API 費用。
3. **開箱即用免設定翻譯**
   - 預設整合高速免金鑰翻譯端點，首次開啟即可直接翻譯。
   - 支援多語系互翻（預設翻譯至正體中文 `zh-TW`，可選英文、日文、韓文、法文、德文、西班牙文）。
4. **互動式覆蓋層體驗**
   - **原文對照 Tooltip**：滑鼠移至任一翻譯色塊上方，即刻浮動顯示原文。
   - **單擊複製譯文**：滑鼠點選文字色塊即可將翻譯結果複製至剪貼簿。
   - **隨時退出**：按下鍵盤 `Esc` 或點擊空白處即可立刻關閉覆蓋層。
5. **輕量高性能技術架構**
   - **後端**：Tauri 2 ＋ Rust（記憶體佔用極低，無縫銜接 Windows API 與多螢幕 DPI 縮放）。
   - **前端**：Vue 3 ＋ TypeScript ＋ Vite。

---

## 🚀 啟動與開發

### 前置需求
- Windows 10 或 Windows 11
- [Rust](https://www.rust-lang.org/) (rustc & cargo)
- [Node.js](https://nodejs.org/) (v18+)

### 開發模式啟動
在專案根目錄下執行：
```powershell
cd pixel-screen-translate

# 啟動開發伺服器與桌面視窗
npm run tauri dev
```

### 建置正式發布版（Release）
```powershell
npm run tauri build
```
建置完成之輕量化安裝檔與執行檔將產生於 `src-tauri/target/release/`。

---

## 📂 專案架構目錄

```text
pixel-screen-translate/
├── src-tauri/                     # Tauri 2 (Rust) 後端
│   ├── Cargo.toml                 # Rust 依賴配置
│   ├── tauri.conf.json            # 雙視窗配置（控制中心 + 透明全螢幕覆蓋層）
│   └── src/
│       ├── main.rs                # 程式進入點
│       ├── lib.rs                 # Tauri Commands 與狀態調度
│       ├── capture.rs             # 螢幕擷取模組（多螢幕與 DPI 感知）
│       ├── ocr.rs                 # Windows 原生離線 OCR 核心
│       ├── color.rs               # 智慧背景色彩採樣與高對比文字計算
│       └── translate.rs           # 高速開箱即用翻譯服務
├── src/                           # Vue 3 前端
│   ├── App.vue                    # 路由調度（依據 mode 切換控制中心與覆蓋層）
│   ├── views/
│   │   ├── MainDashboard.vue      # 控制中心儀表板（語言選擇、狀態指標、即時觸發）
│   │   └── OverlayView.vue        # Pixel 風格全螢幕透明覆蓋層
│   └── main.ts
└── package.json
```

---

## ⌨️ 快捷鍵與操作

| 操作 | 說明 |
| :--- | :--- |
| **點擊「立即進行全螢幕原地翻譯」** | 控制中心一鍵觸發全螢幕截圖、辨識與原地覆蓋翻譯 |
| **滑鼠懸停（Hover）於文字區塊** | 顯示原文對照 Tooltip 浮動小標籤 |
| **單擊（Click）文字區塊** | 自動複製譯文至系統剪貼簿 |
| **按下 <kbd>Esc</kbd>** | 立即關閉退出透明覆蓋層 |
