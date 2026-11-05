<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

const targetLang = ref("zh-TW");
const defaultMode = ref("inplace");
const isTranslating = ref(false);
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
    statusMessage.value = "正在截取螢幕並執行本機離線 OCR 辨識與翻譯...";

    await syncTargetLanguage();

    const res: any = await invoke("trigger_screen_translation", {
      targetLang: targetLang.value,
    });

    detectedCount.value = res.blocks.length;
    statusMessage.value = `翻譯完成！共辨識並原地替換 ${res.blocks.length} 處文字（覆蓋層已啟動）`;
  } catch (err: any) {
    statusMessage.value = `發生錯誤: ${err}`;
  } finally {
    isTranslating.value = false;
  }
}

async function showOverlayDirectly() {
  try {
    await invoke("show_overlay");
  } catch (err) {
    console.error("Failed to show overlay:", err);
  }
}

onMounted(() => {
  syncTargetLanguage();
  try {
    const saved = localStorage.getItem("pixel_translate_mode");
    if (saved === "inplace" || saved === "sidebar") {
      defaultMode.value = saved;
    }
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
          <p class="logo-sub">原地覆蓋排版 · 側邊滾動對照 · 全域快捷鍵 Ctrl+Alt+T</p>
        </div>
      </div>
      <div class="badge-online">離線 OCR 運作中</div>
    </header>

    <!-- Main Content -->
    <main class="dashboard-main">
      <!-- Configuration Cards Grid -->
      <div class="grid-2col">
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
          <p class="hint-text">自動偵測來源語言，並將其翻譯為所選語言。</p>
        </section>

        <!-- Default Presentation Mode Card -->
        <section class="card config-card">
          <label class="section-label">預設呈現模式</label>
          <div class="select-wrapper">
            <select v-model="defaultMode" class="lang-select" @change="onModeChange">
              <option value="inplace">📌 原地覆蓋（Pixel 貼平遮罩）</option>
              <option value="sidebar">📜 側邊滾動對照（滑鼠滾輪瀏覽）</option>
            </select>
          </div>
          <p class="hint-text">覆蓋層中按 <kbd>Tab</kbd> 可隨時在兩種模式間切換。</p>
        </section>
      </div>

      <!-- Action Button Area -->
      <section class="action-area">
        <button
          class="primary-btn"
          :disabled="isTranslating"
          @click="startScreenTranslation"
        >
          <span v-if="!isTranslating" class="btn-icon">⚡</span>
          <span v-else class="spinner"></span>
          <span>{{ isTranslating ? "正在辨識與翻譯螢幕..." : "立即進行全螢幕翻譯" }}</span>
        </button>

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
          <div class="card-icon blue">🔍</div>
          <div class="card-info">
            <div class="card-title">Windows 原生 OCR</div>
            <div class="card-desc">Windows.Media.Ocr 離線辨識，毫秒級座標擷取</div>
          </div>
        </div>

        <div class="status-card">
          <div class="card-icon green">📜</div>
          <div class="card-info">
            <div class="card-title">側邊滾動對照</div>
            <div class="card-desc">支援滑鼠滾輪上下滾動，長篇閱讀最直覺</div>
          </div>
        </div>

        <div class="status-card">
          <div class="card-icon purple">⌨️</div>
          <div class="card-info">
            <div class="card-title">全域快捷鍵</div>
            <div class="card-desc">隨時在任何畫面按下 <kbd>Ctrl + Alt + T</kbd> 即時翻譯</div>
          </div>
        </div>

        <div class="status-card">
          <div class="card-icon amber">🔄</div>
          <div class="card-info">
            <div class="card-title">雙模式切換</div>
            <div class="card-desc">覆蓋層中按 <kbd>Tab</kbd> 切換，按 <kbd>Esc</kbd> 退出</div>
          </div>
        </div>
      </section>
    </main>

    <!-- Footer -->
    <footer class="dashboard-footer">
      <span>Pixel Screen Translate · v0.2.0</span>
      <span>隨時按下 Ctrl+Alt+T 即可直接翻譯當前螢幕</span>
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

.grid-2col {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
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

.primary-btn {
  width: 100%;
  padding: 12px 18px;
  background: linear-gradient(135deg, #0284c7 0%, #2563eb 100%);
  color: white;
  border: none;
  border-radius: 10px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  box-shadow: 0 4px 14px rgba(37, 99, 235, 0.35);
  transition: all 0.2s ease;
}

.primary-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 6px 18px rgba(37, 99, 235, 0.45);
  filter: brightness(1.08);
}

.primary-btn:disabled {
  opacity: 0.65;
  cursor: not-allowed;
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
  to {
    transform: rotate(360deg);
  }
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
