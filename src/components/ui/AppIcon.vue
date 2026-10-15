<script setup lang="ts">
/**
 * Icônes d'interface en SVG, dessinées en `currentColor`. Décoratives
 * (`aria-hidden`) : le parent porte le libellé accessible.
 *
 * Les formes d'état des services sont dans `StatusIcon`.
 */
export type AppIconName =
  | "minimize"
  | "maximize"
  | "restore"
  | "close"
  | "refresh"
  | "signal"
  | "dish"
  | "signal-lost";

defineProps<{
  name: AppIconName;
}>();
</script>

<template>
  <svg
    class="icon"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    <!-- Boutons de fenêtre : traits fins, alignés sur la grille de pixels. -->
    <path
      v-if="name === 'minimize'"
      d="M3 8.5h10"
      stroke-width="1"
      stroke-linecap="butt"
    />
    <rect
      v-else-if="name === 'maximize'"
      x="3.5"
      y="3.5"
      width="9"
      height="9"
      stroke-width="1"
    />
    <g v-else-if="name === 'restore'" stroke-width="1">
      <rect x="3.5" y="5.5" width="7" height="7" />
      <path d="M5.5 5.5v-2h7v7h-2" />
    </g>
    <path
      v-else-if="name === 'close'"
      d="M3.5 3.5l9 9M12.5 3.5l-9 9"
      stroke-width="1"
    />
    <g v-else-if="name === 'refresh'">
      <path d="M13 8a5 5 0 1 1-1.5-3.6" />
      <path d="M12 2v3h-3" />
    </g>
    <!-- Signal acquis : ondes au-dessus d'un émetteur. -->
    <g v-else-if="name === 'signal'">
      <path d="M2 6.5a8.5 8.5 0 0 1 12 0" />
      <path d="M4.2 9a5.3 5.3 0 0 1 7.6 0" />
      <path d="M6.4 11.4a2.2 2.2 0 0 1 3.2 0" />
      <circle cx="8" cy="13.5" r="0.6" fill="currentColor" />
    </g>
    <!-- Acquisition : antenne parabolique orientée vers le ciel. -->
    <g v-else-if="name === 'dish'">
      <path d="M3 6.5a5 5 0 0 0 6.5 6.5z" />
      <path d="M6.2 9.8l4.3-4.3" />
      <circle cx="11.2" cy="4.8" r="1" />
      <path d="M5 13.5h4" />
    </g>
    <!-- Signal perdu : antenne barrée. -->
    <g v-else>
      <path d="M3 6.5a5 5 0 0 0 6.5 6.5z" />
      <path d="M5 13.5h4" />
      <path d="M2.5 2.5l11 11" />
    </g>
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
