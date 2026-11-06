<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

const targetLang = ref("zh-TW");
const defaultMode = ref("inplace");
const ocrLang = ref<string>("");
const installedOcrLanguages = ref<string[]>([]);
const isTranslating = ref(false);
const isLiveScrollActive = ref(false);
const statusMessage = ref("");
const detectedCount = ref<number | null>(null);

const languageOptions = [
  { code: "zh-TW", label: "繁體中文 (正體)" },
  { code: "en", label: "English (英文)" },
  { code: "ja", label: "日本語 (日文)" },
  { code: "ko", label: "한국어 (韓文)" },
  { code: "es", label: "Español (西班牙文)" },
  { code: "de", label: "Deutsch (德文)" },
  { code: "fr", label: "Français (法文)" },
];

function formatOcrLanguageLabel(tag: string): string {
  const map: Record<string, string> = {
    "en-US": "English (美國英文)",
    "en-GB": "English (英國英文)",
    "zh-Hant-TW": "繁體中文 (台灣)",
    "zh-Hans-CN": "簡體中文 (中國)",
    "ja-JP": "日本語 (日本)",
    "ja": "日本語",
    "ko-KR": "한국어 (韓國)",
    "ko": "한국어",
    "de-DE": "Deutsch (德國)",
    "fr-FR": "Français (法國)",
    "es-ES": "Español (西班牙)",
  };
  return map[tag] || tag;
}

async function loadOcrLanguages() {
  try {
    const list: string[] = await invoke("get_installed_ocr_languages");
    installedOcrLanguages.value = list;
    const current: string | null = await invoke("get_current_ocr_language");
    ocrLang.value = current || "";
  } catch (err) {
    console.warn("Failed to get installed OCR languages:", err);
  }
}

async function syncOcrLanguage() {
  try {
    await invoke("set_ocr_language", { lang: ocrLang.value ? ocrLang.value : null });
  } catch (err) {
    console.warn("Failed to set OCR language:", err);
  }
}

async function syncTargetLanguage() {
  try {
    await invoke("set_target_language", { lang: targetLang.value });
  } catch (err) {
    console.warn("Failed to set target language:", err);
  }
}

function onModeChange() {
  try {
    localStorage.setItem("pixel_translate_mode", defaultMode.value);
  } catch (_) {}
}

async function startScreenTranslation() {
  try {
    isTranslating.value = true;
    statusMessage.value = "正在截取螢幕並執行多行段落智慧合併與翻譯...";

    await syncTargetLanguage();

    const res: any = await invoke("trigger_screen_translation", {
      targetLang: targetLang.value,
    });

    detectedCount.value = res.blocks.length;
    statusMessage.value = `翻譯完成！已智慧合併並原地替換 ${res.blocks.length} 處自然段落`;
  } catch (err: any) {
    statusMessage.value = `發生錯誤: ${err}`;
  } finally {
    isTranslating.value = false;
  }
}

async function startSnippingTranslation() {
  try {
    await syncTargetLanguage();
    await invoke("trigger_snipping_mode");
    statusMessage.value = "已開啟框選模式！請在畫面上拖曳滑鼠圈選欲翻譯的區域。";
  } catch (err: any) {
    statusMessage.value = `發生錯誤: ${err}`;
  }
}

async function toggleLiveScroll() {
  try {
    await syncTargetLanguage();
    const nextState = !isLiveScrollActive.value;
    await invoke("set_live_scroll_mode", { enabled: nextState });
    isLiveScrollActive.value = nextState;
    if (nextState) {
      statusMessage.value = "已啟用滾輪即時跟隨模式！直接在網頁/PDF上滾動滑鼠滾輪即可動態翻譯。";
    } else {
      statusMessage.value = "已關閉滾輪即時跟隨模式。";
    }
  } catch (err: any) {
    statusMessage.value = `發生錯誤: ${err}`;
  }
}

async function showOverlayDirectly() {
  try {
    await invoke("show_overlay");
  } catch (err) {
    console.error("Failed to show overlay:", err);
  }
}

onMounted(async () => {
  syncTargetLanguage();
  loadOcrLanguages();
  try {
    const saved = localStorage.getItem("pixel_translate_mode");
    if (saved === "inplace" || saved === "sidebar") {
      defaultMode.value = saved;
    }
    isLiveScrollActive.value = await invoke("is_live_scroll_active");
  } catch (_) {}
});
</script>

<template>
  <div class="dashboard-wrap">
    <!-- Header -->
    <header class="dashboard-header">
      <div class="logo-area">
        <div class="logo-icon">
          <svg viewBox="0 0 24 24" width="22" height="22" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="2" y1="12" x2="22" y2="12"></line>
            <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>
          </svg>
        </div>
        <div>
          <h1 class="logo-title">Pixel 螢幕即時翻譯</h1>
          <p class="logo-sub">多行段落智慧合併 · 0ms 快取 · 滾輪動態跟隨 · 區域框選</p>
        </div>
      </div>
      <div class="badge-online">離線 OCR 運作中</div>
    </header>

    <!-- Main Content -->
    <main class="dashboard-main">
      <!-- Configuration Cards Grid -->
      <div class="grid-config">
        <!-- Target Language Card -->
        <section class="card config-card">
          <label class="section-label">目標翻譯語言</label>
          <div class="select-wrapper">
            <select v-model="targetLang" class="lang-select" @change="syncTargetLanguage">
              <option v-for="lang in languageOptions" :key="lang.code" :value="lang.code">
                {{ lang.label }}
              </option>
            </select>
          </div>
          <p class="hint-text">翻譯輸出之目標語言。</p>
        </section>

        <!-- OCR Source Language Card -->
        <section class="card config-card">
          <label class="section-label">OCR 來源語系 (離線辨識)</label>
          <div class="select-wrapper">
            <select v-model="ocrLang" class="lang-select" @change="syncOcrLanguage">
              <option value="">自動（系統預設語系）</option>
              <option v-for="tag in installedOcrLanguages" :key="tag" :value="tag">
                {{ formatOcrLanguageLabel(tag) }}
              </option>
            </select>
          </div>
          <p class="hint-text">Windows 內建之 OCR 語系包。</p>
        </section>

        <!-- Default Presentation Mode Card -->
        <section class="card config-card">
          <label class="section-label">預設呈現模式</label>
          <div class="select-wrapper">
            <select v-model="defaultMode" class="lang-select" @change="onModeChange">
              <option value="inplace">📌 原地覆蓋（貼平遮罩）</option>
              <option value="sidebar">📜 側邊滾動對照（條列閱讀）</option>
            </select>
          </div>
          <p class="hint-text">按 <kbd>Tab</kbd> 可隨時在兩模式切換。</p>
        </section>
      </div>

      <!-- Action Button Area -->
      <section class="action-area">
        <!-- Button 1: Live Wheel Follow Mode -->
        <button
          class="live-scroll-btn"
          :class="{ active: isLiveScrollActive }"
          @click="toggleLiveScroll"
        >
          <span class="live-btn-icon">🖱️</span>
          <div class="btn-text-col">
            <span class="btn-main-title">
              {{ isLiveScrollActive ? "🟢 滾輪即時跟隨翻譯中 (點擊停用)" : "⚡ 啟動「滾輪即時跟隨翻譯」模式" }}
            </span>
            <span class="btn-sub-title">自由滾動底層網頁/PDF，停頓自動即時翻譯，LRU 0ms 快取（Ctrl+Alt+W）</span>
          </div>
        </button>

        <!-- Button Row: Snapshot & Snipping -->
        <div class="button-row">
          <!-- Button 2: Snapshot Fullscreen Translate -->
          <button
            class="action-btn-half snapshot"
            :disabled="isTranslating"
            @click="startScreenTranslation"
          >
            <span v-if="!isTranslating" class="btn-icon">📸</span>
            <span v-else class="spinner"></span>
            <span>{{ isTranslating ? "正在翻譯..." : "全螢幕快照翻譯 (Ctrl+Alt+T)" }}</span>
          </button>

          <!-- Button 3: Snipping Tool Mode -->
          <button
            class="action-btn-half snipping"
            @click="startSnippingTranslation"
          >
            <span class="btn-icon">✂️</span>
            <span>區域框選翻譯 (Ctrl+Alt+S / Alt+Shift+S)</span>
          </button>
        </div>

        <div v-if="detectedCount !== null" class="secondary-actions">
          <button class="secondary-btn" @click="showOverlayDirectly">
            🔍 開啟 / 檢視目前翻譯覆蓋層
          </button>
        </div>

        <div v-if="statusMessage" class="status-box" :class="{ error: statusMessage.startsWith('發生錯誤') }">
          {{ statusMessage }}
        </div>
      </section>

      <!-- Architecture & Status Badges -->
      <section class="grid-cards">
        <div class="status-card">
          <div class="card-icon green">🧠</div>
          <div class="card-info">
            <div class="card-title">智慧段落合併</div>
            <div class="card-desc">跨行自動拼合完整長句，翻譯語意最通順</div>
          </div>
        </div>

        <div class="status-card">
          <div class="card-icon blue">⚡</div>
          <div class="card-info">
            <div class="card-title">LRU 記憶體快取</div>
            <div class="card-desc">重複詞句 0 毫秒極速秒回，省流抗延遲</div>
          </div>
        </div>

        <div class="status-card">
          <div class="card-icon purple">✂️</div>
          <div class="card-info">
            <div class="card-title">滑鼠拖曳框選</div>
            <div class="card-desc">按 <kbd>Ctrl + Alt + S</kbd> 或 <kbd>Alt + Shift + S</kbd> 圈選局部翻譯</div>
          </div>
        </div>

        <div class="status-card">
          <div class="card-icon amber">❌</div>
          <div class="card-info">
            <div class="card-title">全域隨時退出</div>
            <div class="card-desc">任何焦點下按 <kbd>Esc</kbd> 立即關閉所有覆蓋</div>
          </div>
        </div>
      </section>
    </main>

    <!-- Footer -->
    <footer class="dashboard-footer">
      <span>Pixel Screen Translate · v0.4.0</span>
      <span>滾動：Ctrl+Alt+W · 框選：Ctrl+Alt+S / Alt+Shift+S · 快照：Ctrl+Alt+T</span>
    </footer>
  </div>
</template>

<style scoped>
.dashboard-wrap {
  width: 100vw;
  height: 100vh;
  background-color: #0f172a;
  color: #f8fafc;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-sizing: border-box;
}

.dashboard-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  background: rgba(30, 41, 59, 0.6);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.logo-area {
  display: flex;
  align-items: center;
  gap: 12px;
}

.logo-icon {
  width: 38px;
  height: 38px;
  border-radius: 10px;
  background: linear-gradient(135deg, #0284c7 0%, #38bdf8 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  box-shadow: 0 4px 12px rgba(14, 165, 233, 0.35);
}

.logo-title {
  font-size: 16px;
  font-weight: 700;
  margin: 0;
  color: #f1f5f9;
}

.logo-sub {
  font-size: 11px;
  color: #94a3b8;
  margin: 2px 0 0 0;
}

.badge-online {
  font-size: 11px;
  font-weight: 600;
  color: #34d399;
  background: rgba(16, 185, 129, 0.15);
  border: 1px solid rgba(16, 185, 129, 0.3);
  padding: 3px 8px;
  border-radius: 9999px;
}

.dashboard-main {
  flex: 1;
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow-y: auto;
}

.grid-config {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
}

@media (max-width: 620px) {
  .grid-config {
    grid-template-columns: 1fr;
  }
}

.card {
  background: rgba(30, 41, 59, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  padding: 14px 16px;
}

.section-label {
  display: block;
  font-size: 13px;
  font-weight: 600;
  color: #cbd5e1;
  margin-bottom: 6px;
}

.select-wrapper {
  position: relative;
}

.lang-select {
  width: 100%;
  padding: 8px 12px;
  background: #1e293b;
  border: 1px solid #334155;
  color: #f8fafc;
  border-radius: 8px;
  font-size: 13px;
  outline: none;
  transition: border-color 0.15s ease;
}

.lang-select:focus {
  border-color: #38bdf8;
  box-shadow: 0 0 0 2px rgba(56, 189, 248, 0.2);
}

.hint-text {
  font-size: 11px;
  color: #64748b;
  margin: 6px 0 0 0;
}

.action-area {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.live-scroll-btn {
  width: 100%;
  padding: 12px 18px;
  background: linear-gradient(135deg, #059669 0%, #10b981 100%);
  color: white;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 12px;
  box-shadow: 0 4px 14px rgba(16, 185, 129, 0.35);
  transition: all 0.2s ease;
  text-align: left;
}

.live-scroll-btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 6px 18px rgba(16, 185, 129, 0.45);
  filter: brightness(1.08);
}

.live-scroll-btn.active {
  background: linear-gradient(135deg, #dc2626 0%, #ef4444 100%);
  box-shadow: 0 4px 14px rgba(239, 68, 68, 0.35);
}

.live-btn-icon {
  font-size: 24px;
}

.btn-text-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.btn-main-title {
  font-size: 14px;
  font-weight: 700;
}

.btn-sub-title {
  font-size: 11px;
  opacity: 0.9;
}

.button-row {
  display: flex;
  gap: 10px;
}

.action-btn-half {
  flex: 1;
  padding: 10px 14px;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  transition: all 0.2s ease;
}

.action-btn-half.snapshot {
  background: rgba(30, 41, 59, 0.8);
  border: 1px solid rgba(56, 189, 248, 0.3);
  color: #38bdf8;
}

.action-btn-half.snapshot:hover:not(:disabled) {
  background: rgba(14, 165, 233, 0.15);
  border-color: #38bdf8;
  color: white;
}

.action-btn-half.snipping {
  background: rgba(30, 41, 59, 0.8);
  border: 1px solid rgba(168, 85, 247, 0.4);
  color: #c084fc;
}

.action-btn-half.snipping:hover {
  background: rgba(168, 85, 247, 0.15);
  border-color: #c084fc;
  color: white;
}

.secondary-actions {
  display: flex;
  justify-content: center;
}

.secondary-btn {
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.12);
  color: #38bdf8;
  padding: 6px 14px;
  border-radius: 8px;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.secondary-btn:hover {
  background: rgba(56, 189, 248, 0.15);
  border-color: #38bdf8;
}

.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.status-box {
  padding: 8px 12px;
  border-radius: 8px;
  background: rgba(14, 165, 233, 0.12);
  border: 1px solid rgba(14, 165, 233, 0.25);
  color: #38bdf8;
  font-size: 12px;
  text-align: center;
}

.status-box.error {
  background: rgba(239, 68, 68, 0.12);
  border-color: rgba(239, 68, 68, 0.25);
  color: #f87171;
}

.grid-cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.status-card {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  background: rgba(30, 41, 59, 0.4);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 8px;
  padding: 10px 12px;
}

.card-icon {
  font-size: 16px;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(255, 255, 255, 0.05);
}

.card-title {
  font-size: 12px;
  font-weight: 600;
  color: #e2e8f0;
}

.card-desc {
  font-size: 11px;
  color: #94a3b8;
  margin-top: 2px;
}

kbd {
  background: #334155;
  color: #f1f5f9;
  padding: 1px 5px;
  border-radius: 4px;
  font-size: 10px;
  font-family: inherit;
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.dashboard-footer {
  display: flex;
  justify-content: space-between;
  padding: 10px 20px;
  background: rgba(15, 23, 42, 0.8);
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  font-size: 11px;
  color: #64748b;
}
</style>
