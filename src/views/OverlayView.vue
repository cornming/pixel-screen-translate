<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface BoundingBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface TranslatedBlock {
  id: number;
  original_text: string;
  translated_text: string;
  rect: BoundingBox;
  bg_color: string;
  text_color: string;
}

interface ScreenTranslationResponse {
  screen_width: number;
  screen_height: number;
  scale_factor: number;
  blocks: TranslatedBlock[];
}

const blocks = ref<TranslatedBlock[]>([]);
const displayMode = ref<"inplace" | "sidebar">("inplace");
const isLiveScroll = ref(false);
const isScrollingNow = ref(false);

// Snipping Tool Mode State
const isSnipping = ref(false);
const isSnippingDragging = ref(false);
const isSnippingProcessing = ref(false);
const snipStartX = ref(0);
const snipStartY = ref(0);
const snipCurrentRect = ref<{ x: number; y: number; width: number; height: number } | null>(null);

const searchQuery = ref("");
const hoveredBlock = ref<TranslatedBlock | null>(null);
const activeSidebarBlockId = ref<number | null>(null);
const copiedId = ref<number | null>(null);
const copyToast = ref<{ text: string; x: number; y: number } | null>(null);

let unlistenTranslateFn: (() => void) | null = null;
let unlistenScrollFn: (() => void) | null = null;
let unlistenStatusFn: (() => void) | null = null;
let unlistenStartSnipFn: (() => void) | null = null;
let unlistenStopSnipFn: (() => void) | null = null;

const filteredBlocks = computed(() => {
  if (!searchQuery.value.trim()) return blocks.value;
  const q = searchQuery.value.toLowerCase();
  return blocks.value.filter(
    (b) =>
      b.original_text.toLowerCase().includes(q) ||
      b.translated_text.toLowerCase().includes(q)
  );
});

async function closeOverlay() {
  try {
    await invoke("hide_overlay");
  } catch (err) {
    console.error("Failed to hide overlay:", err);
  }
}

async function startSnipping() {
  isSnipping.value = true;
  snipCurrentRect.value = null;
  blocks.value = [];
}

function handleSnipMouseDown(e: MouseEvent) {
  if (!isSnipping.value || isSnippingProcessing.value) return;
  isSnippingDragging.value = true;
  snipStartX.value = e.clientX;
  snipStartY.value = e.clientY;
  snipCurrentRect.value = {
    x: e.clientX,
    y: e.clientY,
    width: 0,
    height: 0,
  };
}

function handleSnipMouseMove(e: MouseEvent) {
  if (!isSnipping.value || !isSnippingDragging.value || isSnippingProcessing.value) return;
  const x = Math.min(snipStartX.value, e.clientX);
  const y = Math.min(snipStartY.value, e.clientY);
  const width = Math.abs(e.clientX - snipStartX.value);
  const height = Math.abs(e.clientY - snipStartY.value);

  snipCurrentRect.value = { x, y, width, height };
}

async function handleSnipMouseUp() {
  if (!isSnipping.value || !isSnippingDragging.value) return;
  isSnippingDragging.value = false;

  if (snipCurrentRect.value && snipCurrentRect.value.width > 20 && snipCurrentRect.value.height > 20) {
    isSnippingProcessing.value = true;
    try {
      await invoke("translate_selected_region", {
        x: snipCurrentRect.value.x,
        y: snipCurrentRect.value.y,
        width: snipCurrentRect.value.width,
        height: snipCurrentRect.value.height,
      });
    } catch (err) {
      console.error("Region translation failed:", err);
    } finally {
      isSnippingProcessing.value = false;
      isSnipping.value = false;
    }
  } else {
    snipCurrentRect.value = null;
  }
}

async function toggleLiveScrollMode() {
  try {
    const nextState = !isLiveScroll.value;
    await invoke("set_live_scroll_mode", { enabled: nextState });
    isLiveScroll.value = nextState;
  } catch (err) {
    console.error("Failed to toggle live scroll mode:", err);
  }
}

async function copyText(text: string, event: MouseEvent, id?: number) {
  try {
    await navigator.clipboard.writeText(text);
    if (id !== undefined) copiedId.value = id;
    copyToast.value = {
      text: "已複製譯文！",
      x: event.clientX,
      y: Math.max(10, event.clientY - 30),
    };
    setTimeout(() => {
      copiedId.value = null;
      copyToast.value = null;
    }, 1500);
  } catch (err) {
    console.error("Failed to copy:", err);
  }
}

async function copyAllTranslatedText(event: MouseEvent) {
  const allText = blocks.value.map((b) => b.translated_text).join("\n\n");
  if (!allText) return;
  await copyText(allText, event);
  copyToast.value = {
    text: `已複製全部 ${blocks.value.length} 處譯文！`,
    x: window.innerWidth / 2 - 80,
    y: 80,
  };
}

function calculateFontSize(rect: BoundingBox, text: string): string {
  if (!text || text.length === 0) return "14px";
  const heightRatio = Math.max(11, Math.min(rect.height * 0.78, 26));
  const estimatedCharWidth = rect.width / (text.length * 0.9 + 1);
  const size = Math.max(11, Math.min(heightRatio, estimatedCharWidth * 1.5));
  return `${Math.round(size)}px`;
}

function scrollToSidebarItem(id: number) {
  activeSidebarBlockId.value = id;
  const el = document.getElementById(`sidebar-item-${id}`);
  if (el) {
    el.scrollIntoView({ behavior: "smooth", block: "center" });
  }
}

function toggleMode() {
  displayMode.value = displayMode.value === "inplace" ? "sidebar" : "inplace";
  try {
    localStorage.setItem("pixel_translate_mode", displayMode.value);
  } catch (_) {}
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    if (isSnipping.value) {
      isSnipping.value = false;
      snipCurrentRect.value = null;
    } else {
      closeOverlay();
    }
  } else if (e.key === "Tab") {
    e.preventDefault();
    toggleMode();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", handleKeyDown);

  try {
    const savedMode = localStorage.getItem("pixel_translate_mode");
    if (savedMode === "inplace" || savedMode === "sidebar") {
      displayMode.value = savedMode;
    }
  } catch (_) {}

  try {
    const latest: any = await invoke("get_latest_translation");
    if (latest && latest.blocks && latest.blocks.length > 0) {
      blocks.value = latest.blocks;
    }
    isLiveScroll.value = await invoke("is_live_scroll_active");
  } catch (err) {
    console.warn("Could not fetch latest translation:", err);
  }

  try {
    unlistenTranslateFn = await listen<ScreenTranslationResponse>("screen-translated-data", (event) => {
      blocks.value = event.payload.blocks;
      isScrollingNow.value = false;
    });

    unlistenScrollFn = await listen("screen-is-scrolling", () => {
      blocks.value = [];
      isScrollingNow.value = true;
    });

    unlistenStatusFn = await listen<boolean>("live-scroll-status", (event) => {
      isLiveScroll.value = event.payload;
    });

    unlistenStartSnipFn = await listen("start-snipping", () => {
      startSnipping();
    });

    unlistenStopSnipFn = await listen("stop-snipping", () => {
      isSnipping.value = false;
      isSnippingProcessing.value = false;
    });
  } catch (err) {
    console.error("Failed to listen to tauri events:", err);
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeyDown);
  if (unlistenTranslateFn) unlistenTranslateFn();
  if (unlistenScrollFn) unlistenScrollFn();
  if (unlistenStatusFn) unlistenStatusFn();
  if (unlistenStartSnipFn) unlistenStartSnipFn();
  if (unlistenStopSnipFn) unlistenStopSnipFn();
});
</script>

<template>
  <div
    class="overlay-container"
    :class="{
      'live-mode': isLiveScroll,
      'snipping-mode': isSnipping,
    }"
    @mousedown="handleSnipMouseDown"
    @mousemove="handleSnipMouseMove"
    @mouseup="handleSnipMouseUp"
    @click.self="!isSnipping && closeOverlay()"
  >
    <!-- Top Floating Pixel-style Pill Toolbar -->
    <div v-if="!isSnipping" class="floating-toolbar" :class="{ 'live-glow': isLiveScroll }">
      <div class="toolbar-brand">
        <span class="pulse-dot" :class="{ live: isLiveScroll }"></span>
        <span class="brand-text">{{ isLiveScroll ? "滾輪動態翻譯中" : "Pixel 螢幕翻譯" }}</span>
      </div>

      <!-- Live Wheel Scroll Mode Toggle -->
      <button
        class="live-toggle-btn"
        :class="{ active: isLiveScroll }"
        @click="toggleLiveScrollMode"
        title="按 Ctrl+Alt+W 開啟/關閉滾輪即時跟隨"
      >
        <span class="wheel-icon">🖱️</span>
        <span>{{ isLiveScroll ? "動態滾輪：開" : "動態滾輪：關" }}</span>
      </button>

      <!-- Snipping Mode Trigger -->
      <button class="snip-toggle-btn" @click="startSnipping" title="區域框選翻譯 (Ctrl+Alt+S / Alt+Shift+S)">
        <span>✂️ 框選翻譯</span>
      </button>

      <!-- Mode Switcher -->
      <div class="mode-switcher">
        <button
          class="mode-btn"
          :class="{ active: displayMode === 'inplace' }"
          @click="displayMode = 'inplace'"
          title="原地遮罩替換排版"
        >
          📌 原地覆蓋
        </button>
        <button
          class="mode-btn"
          :class="{ active: displayMode === 'sidebar' }"
          @click="displayMode = 'sidebar'"
          title="側邊滾動對照清單 (按 Tab 切換)"
        >
          📜 側邊清單
        </button>
      </div>

      <div class="toolbar-info">
        <span v-if="isScrollingNow" class="scrolling-hint">⚡ 滾動中...</span>
        <span v-else>共 {{ blocks.length }} 處自然段落</span>
      </div>

      <div class="toolbar-actions">
        <button
          v-if="displayMode === 'sidebar'"
          class="tool-btn copy-all"
          @click="copyAllTranslatedText($event)"
          title="複製全部翻譯段落"
        >
          📋 複製全部
        </button>
        <button class="tool-btn close-btn" @click="closeOverlay" title="關閉覆蓋層 (Esc)">
          ✕ 關閉 (Esc)
        </button>
      </div>
    </div>

    <!-- Snipping Banner Prompt -->
    <div v-if="isSnipping" class="snipping-banner">
      <span class="snip-icon">✂️</span>
      <span v-if="!isSnippingProcessing">請使用滑鼠拖曳框選需要翻譯的畫面區域（按 <kbd>Esc</kbd> 取消）</span>
      <span v-else class="processing-text">
        <span class="spinner-small"></span>
        正在對圈選區域執行 Windows OCR 與翻譯...
      </span>
      <button class="snip-cancel-btn" @click="isSnipping = false">取消</button>
    </div>

    <!-- Snipping Selection Rubber-Band Box -->
    <div
      v-if="isSnipping && snipCurrentRect"
      class="snip-selection-box"
      :style="{
        left: `${snipCurrentRect.x}px`,
        top: `${snipCurrentRect.y}px`,
        width: `${snipCurrentRect.width}px`,
        height: `${snipCurrentRect.height}px`,
      }"
    >
      <div class="snip-dimension">
        {{ Math.round(snipCurrentRect.width) }} × {{ Math.round(snipCurrentRect.height) }}
      </div>
    </div>

    <!-- Live Scroll Tip Floating Pill -->
    <div v-if="isLiveScroll && !isSnipping" class="live-status-pill">
      <span>🟢 滾輪穿透中：滾動網頁/PDF即可動態翻譯（按 <kbd>Esc</kbd> 或 <kbd>Ctrl+Alt+W</kbd> 退出）</span>
    </div>

    <!-- ================= MODE 1: 原地替換覆蓋 (In-Place Replacement) ================= -->
    <template v-if="displayMode === 'inplace' && !isSnipping">
      <div
        v-for="block in blocks"
        :key="block.id"
        class="pixel-block"
        :class="{ 'scrolling-fade': isScrollingNow }"
        :style="{
          left: `${block.rect.x}px`,
          top: `${block.rect.y}px`,
          width: `${Math.max(block.rect.width, 24)}px`,
          minHeight: `${Math.max(block.rect.height, 16)}px`,
          backgroundColor: block.bg_color,
          color: block.text_color,
          fontSize: calculateFontSize(block.rect, block.translated_text),
        }"
        @mouseenter="hoveredBlock = block"
        @mouseleave="hoveredBlock = null"
        @click="copyText(block.translated_text, $event, block.id)"
        :title="`原文: ${block.original_text}\n點擊複製譯文`"
      >
        <span class="block-content">{{ block.translated_text }}</span>

        <!-- Original Text Tooltip on Hover -->
        <div
          v-if="hoveredBlock?.id === block.id && !isLiveScroll"
          class="pixel-tooltip"
          :style="{
            bottom: `${block.rect.height + 6}px`,
          }"
        >
          <div class="tooltip-label">原文對照 (完整段落)</div>
          <div class="tooltip-text">{{ block.original_text }}</div>
          <div class="tooltip-hint">單擊複製譯文 · 按 Tab 切換側邊清單</div>
        </div>
      </div>
    </template>

    <!-- ================= MODE 2: 側邊滾動對照清單 (Scrollable Sidebar) ================= -->
    <template v-if="displayMode === 'sidebar' && !isSnipping">
      <div
        v-for="block in blocks"
        :key="'marker-' + block.id"
        class="screen-marker"
        :class="{
          active: activeSidebarBlockId === block.id || hoveredBlock?.id === block.id,
          'scrolling-fade': isScrollingNow,
        }"
        :style="{
          left: `${block.rect.x}px`,
          top: `${block.rect.y}px`,
          width: `${Math.max(block.rect.width, 24)}px`,
          height: `${Math.max(block.rect.height, 18)}px`,
        }"
        @mouseenter="hoveredBlock = block"
        @mouseleave="hoveredBlock = null"
        @click="scrollToSidebarItem(block.id)"
        :title="`#${block.id + 1} 點選在側邊清單中定位`"
      >
        <span class="marker-badge">#{{ block.id + 1 }}</span>
      </div>

      <aside class="sidebar-drawer">
        <div class="sidebar-header">
          <div class="sidebar-title-row">
            <div class="sidebar-title">
              <span class="sidebar-icon">📜</span>
              <span>閱讀對照清單</span>
              <span class="item-count-badge">{{ filteredBlocks.length }}</span>
            </div>
            <button class="icon-btn" @click="copyAllTranslatedText($event)" title="複製全部譯文">
              📋 複製全部
            </button>
          </div>

          <div class="sidebar-search">
            <input
              v-model="searchQuery"
              type="text"
              placeholder="搜尋原文或譯文內容..."
              class="search-input"
            />
            <button v-if="searchQuery" class="clear-search" @click="searchQuery = ''">✕</button>
          </div>
        </div>

        <div class="sidebar-list">
          <div
            v-for="(block, idx) in filteredBlocks"
            :id="`sidebar-item-${block.id}`"
            :key="block.id"
            class="sidebar-card"
            :class="{
              selected: activeSidebarBlockId === block.id || hoveredBlock?.id === block.id,
            }"
            @mouseenter="hoveredBlock = block"
            @mouseleave="hoveredBlock = null"
          >
            <div class="card-top">
              <span class="card-index">#{{ idx + 1 }}</span>
              <button
                class="copy-btn"
                :class="{ copied: copiedId === block.id }"
                @click="copyText(block.translated_text, $event, block.id)"
                title="複製此段譯文"
              >
                {{ copiedId === block.id ? "✓ 已複製" : "複製譯文" }}
              </button>
            </div>

            <div class="card-translation">
              {{ block.translated_text }}
            </div>

            <div class="card-original">
              <span class="original-tag">原文</span>
              <span class="original-text">{{ block.original_text }}</span>
            </div>
          </div>

          <div v-if="filteredBlocks.length === 0" class="empty-state">
            沒有符合「{{ searchQuery }}」的內容
          </div>
        </div>

        <div class="sidebar-footer">
          <span>支援滑鼠滾輪上下滾動</span>
          <span>按 <kbd>Tab</kbd> 切換回原地覆蓋模式</span>
        </div>
      </aside>
    </template>

    <!-- Copy Toast Notification -->
    <div
      v-if="copyToast"
      class="copy-toast"
      :style="{ left: `${copyToast.x}px`, top: `${copyToast.y}px` }"
    >
      {{ copyToast.text }}
    </div>
  </div>
</template>

<style scoped>
.overlay-container {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  background-color: rgba(15, 23, 42, 0.05);
  user-select: none;
  cursor: default;
  overflow: hidden;
  z-index: 99999;
}

.overlay-container.live-mode {
  background-color: transparent !important;
}

.overlay-container.snipping-mode {
  background-color: rgba(15, 23, 42, 0.45) !important;
  cursor: crosshair !important;
}

/* ================= Snipping Banner & Box ================= */
.snipping-banner {
  position: fixed;
  top: 24px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 12px;
  background: rgba(15, 23, 42, 0.94);
  border: 1px solid #38bdf8;
  color: #f1f5f9;
  padding: 8px 20px;
  border-radius: 9999px;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.4);
  font-size: 13px;
  z-index: 100000;
  animation: fadeInDown 0.2s ease-out;
}

.snip-icon {
  font-size: 16px;
}

.processing-text {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #38bdf8;
}

.spinner-small {
  width: 14px;
  height: 14px;
  border: 2px solid rgba(56, 189, 248, 0.3);
  border-top-color: #38bdf8;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.snip-cancel-btn {
  background: rgba(255, 255, 255, 0.1);
  border: none;
  color: #cbd5e1;
  padding: 3px 10px;
  border-radius: 9999px;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.snip-cancel-btn:hover {
  background: rgba(239, 68, 68, 0.85);
  color: white;
}

.snip-selection-box {
  position: absolute;
  border: 2px dashed #38bdf8;
  background: rgba(56, 189, 248, 0.12);
  box-shadow: 0 0 0 99999px rgba(0, 0, 0, 0.35);
  pointer-events: none;
  z-index: 99998;
}

.snip-dimension {
  position: absolute;
  bottom: -22px;
  left: 0;
  background: rgba(15, 23, 42, 0.9);
  color: #38bdf8;
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 4px;
  border: 1px solid rgba(56, 189, 248, 0.3);
}

/* ================= Top Pill Toolbar ================= */
.floating-toolbar {
  position: fixed;
  top: 18px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 10px;
  background: rgba(30, 41, 59, 0.94);
  backdrop-filter: blur(14px);
  -webkit-backdrop-filter: blur(14px);
  color: #f8fafc;
  padding: 6px 16px;
  border-radius: 9999px;
  box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.35), 0 8px 10px -6px rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.15);
  z-index: 100000;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  animation: fadeInDown 0.2s ease-out;
}

.floating-toolbar.live-glow {
  border-color: #10b981;
  box-shadow: 0 0 16px rgba(16, 185, 129, 0.35);
}

@keyframes fadeInDown {
  from {
    opacity: 0;
    transform: translate(-50%, -10px);
  }
  to {
    opacity: 1;
    transform: translate(-50%, 0);
  }
}

.toolbar-brand {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 13px;
  color: #38bdf8;
}

.pulse-dot {
  width: 8px;
  height: 8px;
  background-color: #38bdf8;
  border-radius: 50%;
  box-shadow: 0 0 8px #38bdf8;
}

.pulse-dot.live {
  background-color: #34d399;
  box-shadow: 0 0 10px #34d399;
  animation: pulse 1s infinite alternate;
}

@keyframes pulse {
  from { transform: scale(0.9); opacity: 0.7; }
  to { transform: scale(1.2); opacity: 1; }
}

.live-toggle-btn {
  display: flex;
  align-items: center;
  gap: 5px;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.15);
  color: #cbd5e1;
  padding: 4px 10px;
  border-radius: 9999px;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.live-toggle-btn:hover {
  background: rgba(255, 255, 255, 0.15);
  color: white;
}

.live-toggle-btn.active {
  background: rgba(16, 185, 129, 0.25);
  border-color: #10b981;
  color: #34d399;
  font-weight: 600;
}

.snip-toggle-btn {
  display: flex;
  align-items: center;
  gap: 5px;
  background: rgba(56, 189, 248, 0.15);
  border: 1px solid rgba(56, 189, 248, 0.3);
  color: #38bdf8;
  padding: 4px 10px;
  border-radius: 9999px;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.snip-toggle-btn:hover {
  background: #0284c7;
  color: white;
}

.mode-switcher {
  display: flex;
  background: rgba(15, 23, 42, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 9999px;
  padding: 2px;
  gap: 2px;
}

.mode-btn {
  background: transparent;
  border: none;
  color: #94a3b8;
  font-size: 12px;
  padding: 4px 10px;
  border-radius: 9999px;
  cursor: pointer;
  transition: all 0.15s ease;
  white-space: nowrap;
}

.mode-btn:hover {
  color: #f8fafc;
}

.mode-btn.active {
  background: #0284c7;
  color: #ffffff;
  font-weight: 600;
  box-shadow: 0 2px 6px rgba(2, 132, 199, 0.4);
}

.toolbar-info {
  font-size: 12px;
  color: #94a3b8;
  border-left: 1px solid rgba(255, 255, 255, 0.15);
  padding-left: 10px;
}

.scrolling-hint {
  color: #38bdf8;
  font-weight: 600;
  animation: blink 0.5s infinite alternate;
}

@keyframes blink {
  from { opacity: 0.5; }
  to { opacity: 1; }
}

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tool-btn {
  background: rgba(255, 255, 255, 0.1);
  border: none;
  color: #f1f5f9;
  padding: 4px 12px;
  border-radius: 9999px;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.tool-btn.copy-all {
  background: rgba(56, 189, 248, 0.2);
  color: #38bdf8;
  border: 1px solid rgba(56, 189, 248, 0.4);
}

.tool-btn.copy-all:hover {
  background: #0284c7;
  color: white;
}

.tool-btn.close-btn:hover {
  background: rgba(239, 68, 68, 0.85);
  color: white;
}

.live-status-pill {
  position: fixed;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(15, 23, 42, 0.92);
  border: 1px solid rgba(16, 185, 129, 0.4);
  color: #a7f3d0;
  padding: 6px 18px;
  border-radius: 9999px;
  font-size: 12px;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(8px);
  pointer-events: none;
  z-index: 100000;
  animation: fadeInUp 0.25s ease-out;
}

@keyframes fadeInUp {
  from { opacity: 0; transform: translate(-50%, 10px); }
  to { opacity: 1; transform: translate(-50%, 0); }
}

/* ================= MODE 1: 原地文字塊 ================= */
.pixel-block {
  position: absolute;
  display: flex;
  align-items: center;
  justify-content: flex-start;
  padding: 2px 6px;
  border-radius: 3px;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
  cursor: pointer;
  line-height: 1.35;
  white-space: pre-wrap;
  word-break: break-word;
  transition: opacity 0.15s ease, transform 0.12s ease;
  z-index: 100;
}

.pixel-block.scrolling-fade {
  opacity: 0 !important;
}

.pixel-block:hover {
  transform: scale(1.02);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.25);
  z-index: 200;
  outline: 1.5px solid #38bdf8;
}

.block-content {
  display: inline-block;
  width: 100%;
}

.pixel-tooltip {
  position: absolute;
  left: 0;
  background: rgba(15, 23, 42, 0.96);
  color: #f8fafc;
  padding: 8px 12px;
  border-radius: 6px;
  box-shadow: 0 8px 20px rgba(0, 0, 0, 0.35);
  pointer-events: none;
  font-size: 12px;
  line-height: 1.45;
  min-width: 200px;
  max-width: 420px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  z-index: 9999;
  backdrop-filter: blur(8px);
}

.tooltip-label {
  font-size: 10px;
  color: #38bdf8;
  font-weight: 600;
  text-transform: uppercase;
  margin-bottom: 3px;
}

.tooltip-text {
  font-size: 12px;
  color: #f1f5f9;
  word-break: break-all;
}

.tooltip-hint {
  font-size: 10px;
  color: #94a3b8;
  margin-top: 5px;
  text-align: right;
}

/* ================= MODE 2: 螢幕定位標記 ================= */
.screen-marker {
  position: absolute;
  border: 1.5px dashed rgba(56, 189, 248, 0.4);
  background: rgba(56, 189, 248, 0.06);
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s ease;
  z-index: 50;
}

.screen-marker.scrolling-fade {
  opacity: 0 !important;
}

.screen-marker:hover,
.screen-marker.active {
  border: 2px solid #38bdf8;
  background: rgba(56, 189, 248, 0.2);
  box-shadow: 0 0 10px rgba(56, 189, 248, 0.4);
  z-index: 100;
}

.marker-badge {
  position: absolute;
  top: -10px;
  left: 0;
  background: #0284c7;
  color: white;
  font-size: 10px;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 4px;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
}

/* ================= MODE 2: 側邊滾動對照面板 ================= */
.sidebar-drawer {
  position: fixed;
  right: 0;
  top: 0;
  width: 440px;
  height: 100vh;
  background: rgba(15, 23, 42, 0.94);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  border-left: 1px solid rgba(255, 255, 255, 0.1);
  display: flex;
  flex-direction: column;
  box-shadow: -10px 0 30px rgba(0, 0, 0, 0.4);
  z-index: 99990;
  animation: slideInRight 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes slideInRight {
  from { transform: translateX(100%); }
  to { transform: translateX(0); }
}

.sidebar-header {
  padding: 18px 20px 12px 20px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: rgba(30, 41, 59, 0.4);
}

.sidebar-title-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.sidebar-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 15px;
  font-weight: 700;
  color: #f1f5f9;
}

.sidebar-icon {
  font-size: 18px;
}

.item-count-badge {
  font-size: 11px;
  background: rgba(56, 189, 248, 0.2);
  color: #38bdf8;
  padding: 2px 7px;
  border-radius: 9999px;
  font-weight: 600;
}

.icon-btn {
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: #38bdf8;
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.icon-btn:hover {
  background: #0284c7;
  color: white;
}

.sidebar-search {
  position: relative;
  display: flex;
  align-items: center;
}

.search-input {
  width: 100%;
  padding: 8px 30px 8px 12px;
  background: #1e293b;
  border: 1px solid #334155;
  color: #f8fafc;
  border-radius: 8px;
  font-size: 13px;
  outline: none;
  transition: border-color 0.15s ease;
}

.search-input:focus {
  border-color: #38bdf8;
  box-shadow: 0 0 0 2px rgba(56, 189, 248, 0.2);
}

.clear-search {
  position: absolute;
  right: 8px;
  background: none;
  border: none;
  color: #94a3b8;
  cursor: pointer;
  font-size: 12px;
}

.sidebar-list {
  flex: 1;
  padding: 14px 16px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.2) transparent;
}

.sidebar-list::-webkit-scrollbar {
  width: 6px;
}

.sidebar-list::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
  border-radius: 9999px;
}

.sidebar-card {
  background: rgba(30, 41, 59, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  transition: all 0.15s ease;
  cursor: default;
}

.sidebar-card:hover {
  background: rgba(30, 41, 59, 0.9);
  border-color: rgba(56, 189, 248, 0.3);
  transform: translateY(-1px);
}

.sidebar-card.selected {
  border-color: #38bdf8;
  background: rgba(14, 165, 233, 0.12);
  box-shadow: 0 0 12px rgba(14, 165, 233, 0.25);
}

.card-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.card-index {
  font-size: 11px;
  font-weight: 700;
  color: #38bdf8;
}

.copy-btn {
  background: rgba(255, 255, 255, 0.06);
  border: none;
  color: #94a3b8;
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.copy-btn:hover {
  background: rgba(56, 189, 248, 0.2);
  color: #38bdf8;
}

.copy-btn.copied {
  color: #34d399;
  background: rgba(16, 185, 129, 0.15);
}

.card-translation {
  font-size: 14px;
  font-weight: 500;
  line-height: 1.45;
  color: #f8fafc;
  user-select: text;
}

.card-original {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  font-size: 12px;
  line-height: 1.4;
  color: #94a3b8;
  border-top: 1px solid rgba(255, 255, 255, 0.05);
  padding-top: 6px;
  user-select: text;
}

.original-tag {
  font-size: 10px;
  background: rgba(255, 255, 255, 0.08);
  color: #cbd5e1;
  padding: 1px 4px;
  border-radius: 3px;
  flex-shrink: 0;
}

.original-text {
  word-break: break-all;
}

.empty-state {
  text-align: center;
  color: #64748b;
  font-size: 13px;
  margin-top: 40px;
}

.sidebar-footer {
  padding: 10px 18px;
  background: rgba(15, 23, 42, 0.95);
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: #64748b;
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

.copy-toast {
  position: fixed;
  background: #10b981;
  color: white;
  padding: 5px 12px;
  border-radius: 9999px;
  font-size: 12px;
  font-weight: 600;
  pointer-events: none;
  box-shadow: 0 4px 12px rgba(16, 185, 129, 0.4);
  z-index: 100001;
  animation: popUp 0.15s ease-out;
}

@keyframes popUp {
  from { opacity: 0; transform: translateY(4px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
