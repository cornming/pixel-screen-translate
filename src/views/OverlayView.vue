<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
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
const hoveredBlock = ref<TranslatedBlock | null>(null);
const copiedId = ref<number | null>(null);
const copyToast = ref<{ text: string; x: number; y: number } | null>(null);

let unlistenFn: (() => void) | null = null;

async function closeOverlay() {
  try {
    await invoke("hide_overlay");
  } catch (err) {
    console.error("Failed to hide overlay:", err);
  }
}

async function copyText(block: TranslatedBlock, event: MouseEvent) {
  try {
    await navigator.clipboard.writeText(block.translated_text);
    copiedId.value = block.id;
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

function calculateFontSize(rect: BoundingBox, text: string): string {
  if (!text || text.length === 0) return "14px";
  const heightRatio = Math.max(11, Math.min(rect.height * 0.78, 26));
  const estimatedCharWidth = rect.width / (text.length * 0.9 + 1);
  const size = Math.max(11, Math.min(heightRatio, estimatedCharWidth * 1.5));
  return `${Math.round(size)}px`;
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    closeOverlay();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", handleKeyDown);

  // 1. Immediately fetch latest translation if already available
  try {
    const latest: any = await invoke("get_latest_translation");
    if (latest && latest.blocks && latest.blocks.length > 0) {
      blocks.value = latest.blocks;
    }
  } catch (err) {
    console.warn("Could not fetch latest translation:", err);
  }

  // 2. Listen to translation events for live updates
  try {
    unlistenFn = await listen<ScreenTranslationResponse>("screen-translated-data", (event) => {
      blocks.value = event.payload.blocks;
    });
  } catch (err) {
    console.error("Failed to listen to screen-translated-data:", err);
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeyDown);
  if (unlistenFn) {
    unlistenFn();
  }
});
</script>

<template>
  <div class="overlay-container" @click.self="closeOverlay">
    <!-- Top Floating Pixel-style Pill Toolbar -->
    <div class="floating-toolbar">
      <div class="toolbar-brand">
        <span class="pulse-dot"></span>
        <span class="brand-text">Pixel 原地翻譯</span>
      </div>
      <div class="toolbar-info">
        <span>已偵測 {{ blocks.length }} 個文字區塊</span>
      </div>
      <div class="toolbar-actions">
        <button class="tool-btn" @click="closeOverlay" title="關閉覆蓋層 (Esc)">
          ✕ 關閉 (Esc)
        </button>
      </div>
    </div>

    <!-- Translated Text Blocks (In-Place Replacement) -->
    <div
      v-for="block in blocks"
      :key="block.id"
      class="pixel-block"
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
      @click="copyText(block, $event)"
      :title="`原文: ${block.original_text}\n點擊複製譯文`"
    >
      <span class="block-content">{{ block.translated_text }}</span>

      <!-- Original Text Tooltip on Hover -->
      <div
        v-if="hoveredBlock?.id === block.id"
        class="pixel-tooltip"
        :style="{
          bottom: `${block.rect.height + 6}px`,
        }"
      >
        <div class="tooltip-label">原文對照</div>
        <div class="tooltip-text">{{ block.original_text }}</div>
        <div class="tooltip-hint">單擊複製譯文</div>
      </div>
    </div>

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

.floating-toolbar {
  position: fixed;
  top: 18px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 16px;
  background: rgba(30, 41, 59, 0.94);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  color: #f8fafc;
  padding: 8px 18px;
  border-radius: 9999px;
  box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.35), 0 8px 10px -6px rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.15);
  z-index: 100000;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  animation: fadeInDown 0.2s ease-out;
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

.toolbar-info {
  font-size: 12px;
  color: #94a3b8;
  border-left: 1px solid rgba(255, 255, 255, 0.15);
  padding-left: 14px;
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

.tool-btn:hover {
  background: rgba(239, 68, 68, 0.85);
  color: white;
}

.pixel-block {
  position: absolute;
  display: flex;
  align-items: center;
  justify-content: flex-start;
  padding: 1px 4px;
  border-radius: 3px;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
  cursor: pointer;
  line-height: 1.25;
  white-space: pre-wrap;
  word-break: break-word;
  transition: transform 0.12s ease, box-shadow 0.12s ease;
  z-index: 100;
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
  background: rgba(15, 23, 42, 0.95);
  color: #f8fafc;
  padding: 8px 12px;
  border-radius: 6px;
  box-shadow: 0 8px 20px rgba(0, 0, 0, 0.35);
  pointer-events: none;
  font-size: 12px;
  line-height: 1.4;
  min-width: 160px;
  max-width: 320px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  z-index: 9999;
  backdrop-filter: blur(8px);
}

.tooltip-label {
  font-size: 10px;
  color: #38bdf8;
  font-weight: 600;
  text-transform: uppercase;
  margin-bottom: 2px;
}

.tooltip-text {
  font-size: 12px;
  color: #f1f5f9;
  word-break: break-all;
}

.tooltip-hint {
  font-size: 10px;
  color: #94a3b8;
  margin-top: 4px;
  text-align: right;
}

.copy-toast {
  position: fixed;
  background: #10b981;
  color: white;
  padding: 4px 10px;
  border-radius: 9999px;
  font-size: 11px;
  font-weight: 600;
  pointer-events: none;
  box-shadow: 0 4px 12px rgba(16, 185, 129, 0.4);
  z-index: 100001;
  animation: popUp 0.15s ease-out;
}

@keyframes popUp {
  from {
    opacity: 0;
    transform: translateY(4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
