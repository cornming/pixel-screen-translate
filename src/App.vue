<script setup lang="ts">
import { computed } from "vue";
import MainDashboard from "./views/MainDashboard.vue";
import OverlayView from "./views/OverlayView.vue";

const isOverlayMode = computed(() => {
  const params = new URLSearchParams(window.location.search);
  return params.get("mode") === "overlay" || window.location.hash.includes("overlay");
});
</script>

<template>
  <OverlayView v-if="isOverlayMode" />
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
