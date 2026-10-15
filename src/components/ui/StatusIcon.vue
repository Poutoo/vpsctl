<script setup lang="ts">
/**
 * Symboles d'état en SVG, dessinés en `currentColor` : la couleur vient du
 * parent. Décoratifs (`aria-hidden`) : le parent porte le libellé accessible.
 */
defineProps<{
  name: "check" | "cross" | "circle";
}>();
</script>

<template>
  <svg
    class="icon"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    <path v-if="name === 'check'" d="M3 8.5l3 3 7-7" />
    <path v-else-if="name === 'cross'" d="M4 4l8 8M12 4l-8 8" />
    <circle v-else cx="8" cy="8" r="4.5" />
  </svg>
</template>

<style scoped>
.icon {
  display: inline-block;
  width: 1em;
  height: 1em;
  vertical-align: -0.125em;
}
</style>
