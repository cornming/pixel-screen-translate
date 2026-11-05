<script setup lang="ts">
import { ref, onMounted } from "vue";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import MainDashboard from "./views/MainDashboard.vue";
import OverlayView from "./views/OverlayView.vue";

const isOverlay = ref(false);

onMounted(() => {
  try {
    const current = getCurrentWebviewWindow();
    if (current && current.label === "overlay") {
      isOverlay.value = true;
      return;
    }
  } catch (e) {
    console.warn("Failed to get webview window label:", e);
  }

  const params = new URLSearchParams(window.location.search);
  if (params.get("mode") === "overlay" || window.location.hash.includes("overlay")) {
    isOverlay.value = true;
  }
});
</script>

<template>
  <OverlayView v-if="isOverlay" />
  <MainDashboard v-else />
</template>

<style>
/* Reset global defaults for seamless edge-to-edge transparent overlay */
html, body, #app {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  box-sizing: border-box;
}

body {
  background: transparent;
  user-select: none;
}
</style>
