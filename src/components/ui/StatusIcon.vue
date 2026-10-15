<script setup lang="ts">
/**
 * Symboles d'état en SVG, dessinés en `currentColor` : la couleur vient du
 * parent. Décoratifs (`aria-hidden`) : le parent porte le libellé accessible.
 *
 * Formes des états de service (docs/06-design.md) : `disc` nominal, `circle`
 * arrêté, `triangle` en échec, `half` en transition (statique).
 */
defineProps<{
  name: "check" | "cross" | "circle" | "disc" | "triangle" | "half";
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
    <circle
      v-else-if="name === 'disc'"
      cx="8"
      cy="8"
      r="4.5"
      fill="currentColor"
    />
    <template v-else-if="name === 'triangle'">
      <path d="M8 2.5l6 10.5H2z" />
      <path d="M8 6.5v2.5M8 11v.01" />
    </template>
    <template v-else-if="name === 'half'">
      <circle cx="8" cy="8" r="4.5" />
      <path d="M8 3.5a4.5 4.5 0 0 0 0 9z" fill="currentColor" stroke="none" />
    </template>
    <circle v-else cx="8" cy="8" r="4.5" />
  </svg>
</template>

<style scoped>
.icon {
  display: inline-block;
  width: 1em;
  height: 1em;
  vertical-align: -0.125em;
  flex-shrink: 0;
}
</style>
