<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const targetLang = ref("zh-TW");
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

async function startScreenTranslation() {
  try {
    isTranslating.value = true;
    statusMessage.value = "正在截取螢幕並執行本機離線 OCR 辨識...";

    const res: any = await invoke("trigger_screen_translation", {
      targetLang: targetLang.value,
    });

    detectedCount.value = res.blocks.length;
    statusMessage.value = `翻譯完成！共辨識並原地替換 ${res.blocks.length} 處文字`;
  } catch (err: any) {
    statusMessage.value = `發生錯誤: ${err}`;
  } finally {
    isTranslating.value = false;
  }
}
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
          <p class="logo-sub">Google Pixel 風格 · 原地覆蓋排版 · 本機離線 OCR</p>
        </div>
      </div>
      <div class="badge-online">離線 OCR 運作中</div>
    </header>

    <!-- Main Content -->
    <main class="dashboard-main">
      <!-- Target Language Card -->
      <section class="card config-card">
        <label class="section-label">目標翻譯語言</label>
        <div class="select-wrapper">
          <select v-model="targetLang" class="lang-select">
            <option v-for="lang in languageOptions" :key="lang.code" :value="lang.code">
              {{ lang.label }}
            </option>
          </select>
        </div>
        <p class="hint-text">自動偵測來源語言，並將其原地翻譯為所選語言。</p>
      </section>

      <!-- Action Button Area -->
      <section class="action-area">
        <button
          class="primary-btn"
          :disabled="isTranslating"
          @click="startScreenTranslation"
        >
          <span v-if="!isTranslating" class="btn-icon">⚡</span>
          <span v-else class="spinner"></span>
          <span>{{ isTranslating ? "正在辨識與翻譯螢幕..." : "立即進行全螢幕原地翻譯" }}</span>
        </button>

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
          <div class="card-icon green">🎨</div>
          <div class="card-info">
            <div class="card-title">智慧邊緣色彩採樣</div>
            <div class="card-desc">自動提取背景色平鋪覆蓋，自適應高對比文字</div>
          </div>
        </div>

        <div class="status-card">
          <div class="card-icon purple">🌐</div>
          <div class="card-info">
            <div class="card-title">開箱即用免金鑰</div>
            <div class="card-desc">預設直接連線翻譯，支援自訂進階 API</div>
          </div>
        </div>

        <div class="status-card">
          <div class="card-icon amber">⌨️</div>
          <div class="card-info">
            <div class="card-title">快捷鍵操作提示</div>
            <div class="card-desc">全螢幕覆蓋層按 <kbd>Esc</kbd> 即可退出</div>
          </div>
        </div>
      </section>
    </main>

    <!-- Footer -->
    <footer class="dashboard-footer">
      <span>Pixel Screen Translate · v0.1.0</span>
      <span>提示：按 Esc 或點擊螢幕空白處可隨時關閉翻譯覆蓋</span>
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
  padding: 20px 24px;
  background: rgba(30, 41, 59, 0.6);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.logo-area {
  display: flex;
  align-items: center;
  gap: 12px;
}

.logo-icon {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  background: linear-gradient(135deg, #0284c7 0%, #38bdf8 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  box-shadow: 0 4px 12px rgba(14, 165, 233, 0.35);
}

.logo-title {
  font-size: 17px;
  font-weight: 700;
  margin: 0;
  color: #f1f5f9;
}

.logo-sub {
  font-size: 12px;
  color: #94a3b8;
  margin: 2px 0 0 0;
}

.badge-online {
  font-size: 11px;
  font-weight: 600;
  color: #34d399;
  background: rgba(16, 185, 129, 0.15);
  border: 1px solid rgba(16, 185, 129, 0.3);
  padding: 4px 10px;
  border-radius: 9999px;
}

.dashboard-main {
  flex: 1;
  padding: 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow-y: auto;
}

.card {
  background: rgba(30, 41, 59, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  padding: 16px 20px;
}

.section-label {
  display: block;
  font-size: 13px;
  font-weight: 600;
  color: #cbd5e1;
  margin-bottom: 8px;
}

.select-wrapper {
  position: relative;
}

.lang-select {
  width: 100%;
  padding: 10px 14px;
  background: #1e293b;
  border: 1px solid #334155;
  color: #f8fafc;
  border-radius: 8px;
  font-size: 14px;
  outline: none;
  transition: border-color 0.15s ease;
}

.lang-select:focus {
  border-color: #38bdf8;
  box-shadow: 0 0 0 2px rgba(56, 189, 248, 0.2);
}

.hint-text {
  font-size: 12px;
  color: #64748b;
  margin: 8px 0 0 0;
}

.action-area {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.primary-btn {
  width: 100%;
  padding: 14px 20px;
  background: linear-gradient(135deg, #0284c7 0%, #2563eb 100%);
  color: white;
  border: none;
  border-radius: 10px;
  font-size: 15px;
  font-weight: 600;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  box-shadow: 0 6px 18px rgba(37, 99, 235, 0.35);
  transition: all 0.2s ease;
}

.primary-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 8px 22px rgba(37, 99, 235, 0.45);
  filter: brightness(1.08);
}

.primary-btn:disabled {
  opacity: 0.65;
  cursor: not-allowed;
}

.spinner {
  width: 18px;
  height: 18px;
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
  padding: 10px 14px;
  border-radius: 8px;
  background: rgba(14, 165, 233, 0.12);
  border: 1px solid rgba(14, 165, 233, 0.25);
  color: #38bdf8;
  font-size: 13px;
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
  gap: 12px;
}

.status-card {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  background: rgba(30, 41, 59, 0.4);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 10px;
  padding: 12px 14px;
}

.card-icon {
  font-size: 18px;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(255, 255, 255, 0.05);
}

.card-title {
  font-size: 13px;
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
  padding: 1px 6px;
  border-radius: 4px;
  font-size: 10px;
  font-family: inherit;
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.dashboard-footer {
  display: flex;
  justify-content: space-between;
  padding: 12px 24px;
  background: rgba(15, 23, 42, 0.8);
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  font-size: 11px;
  color: #64748b;
}
</style>
