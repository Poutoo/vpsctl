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
  | "signal-lost"
  | "play"
  | "stop"
  | "pause"
  | "logs"
  | "edit"
  | "copy"
  | "trash"
  | "arrow-down"
  | "arrow-right"
  | "success"
  | "failure"
  | "folder"
  | "file"
  | "link"
  | "special"
  | "more"
  | "download"
  | "upload"
  | "search"
  | "lock"
  | "info"
  | "check-circle"
  | "check"
  | "never";

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
    <!-- Actions sur un conteneur ou un service. -->
    <path
      v-else-if="name === 'play'"
      d="M5 3.5l7.5 4.5L5 12.5z"
      fill="currentColor"
    />
    <!-- Données figées (LOS). -->
    <path v-else-if="name === 'pause'" d="M6 4v8M10 4v8" />
    <rect
      v-else-if="name === 'stop'"
      x="4"
      y="4"
      width="8"
      height="8"
      fill="currentColor"
      stroke="none"
    />
    <!-- Télémétrie : lignes de journal. -->
    <path v-else-if="name === 'logs'" d="M3 4.5h10M3 8h10M3 11.5h6" />
    <!-- Copier : deux feuilles décalées. -->
    <g v-else-if="name === 'copy'">
      <rect x="5.5" y="5.5" width="7.5" height="8" rx="1" />
      <path d="M3 10.5V3.5a1 1 0 0 1 1-1h6" />
    </g>
    <!-- Modifier : crayon. -->
    <g v-else-if="name === 'edit'">
      <path d="M10.5 3l2.5 2.5-7 7H3.5V10z" />
      <path d="M9 4.5l2.5 2.5" />
    </g>
    <g v-else-if="name === 'trash'">
      <path d="M2.5 4.5h11" />
      <path d="M6 4.5V3h4v1.5" />
      <path d="M4 4.5l.7 8.5h6.6l.7-8.5" />
      <path d="M6.7 7v3.5M9.3 7v3.5" />
    </g>
    <g v-else-if="name === 'arrow-down'">
      <path d="M8 3v10" />
      <path d="M4 9l4 4 4-4" />
    </g>
    <g v-else-if="name === 'arrow-right'">
      <path d="M3 8h10" />
      <path d="M9 4l4 4-4 4" />
    </g>
    <!-- Notifications : succès dans un cercle, erreur dans un carré. -->
    <g v-else-if="name === 'success'" stroke-width="1">
      <circle cx="8" cy="8" r="7" />
      <path d="M5 8.2l2 2 4-4.4" stroke-width="1.5" />
    </g>
    <g v-else-if="name === 'failure'" stroke-width="1">
      <rect x="1.5" y="1.5" width="13" height="13" />
      <path d="M5.5 5.5l5 5M10.5 5.5l-5 5" stroke-width="1.5" />
    </g>
    <!-- Explorateur : genres d'entrée et actions. -->
    <path v-else-if="name === 'folder'" d="M2 4h4.5l1.5 1.5H14V13H2z" />
    <g v-else-if="name === 'file'">
      <path d="M4 2h5l3 3v9H4z" />
      <path d="M9 2v3h3" />
    </g>
    <g v-else-if="name === 'link'">
      <path d="M6.5 9.5l3-3" />
      <path d="M7.5 4.5l1-1a2.5 2.5 0 0 1 3.5 3.5l-1 1" />
      <path d="M8.5 11.5l-1 1A2.5 2.5 0 0 1 4 9l1-1" />
    </g>
    <g v-else-if="name === 'special'">
      <rect x="3" y="3" width="10" height="10" />
      <circle cx="8" cy="8" r="2" />
    </g>
    <g v-else-if="name === 'more'" fill="currentColor" stroke="none">
      <circle cx="3.5" cy="8" r="1.1" />
      <circle cx="8" cy="8" r="1.1" />
      <circle cx="12.5" cy="8" r="1.1" />
    </g>
    <g v-else-if="name === 'download'">
      <path d="M8 2.5v8" />
      <path d="M4.5 7l3.5 3.5L11.5 7" />
      <path d="M3 13.5h10" />
    </g>
    <g v-else-if="name === 'upload'">
      <path d="M8 13V3" />
      <path d="M4 7l4-4 4 4" />
    </g>
    <g v-else-if="name === 'search'">
      <circle cx="7" cy="7" r="4" />
      <path d="M10 10l3.5 3.5" />
    </g>
    <g v-else-if="name === 'lock'">
      <rect x="4" y="7.5" width="8" height="6" rx="1" />
      <path d="M6 7.5V5.5a2 2 0 0 1 4 0v2" />
    </g>
    <g v-else-if="name === 'info'">
      <circle cx="8" cy="8" r="6" />
      <path d="M8 7.5v3.5M8 5v.01" />
    </g>
    <g v-else-if="name === 'check-circle'">
      <circle cx="8" cy="8" r="6" />
      <path d="M5.5 8.2l1.8 1.8 3.2-3.5" />
    </g>
    <path v-else-if="name === 'check'" d="M3.5 8.5l3 3 6-6.5" />
    <!-- Jamais : cercle barré. -->
    <g v-else-if="name === 'never'">
      <circle cx="8" cy="8" r="5.5" />
      <path d="M4.2 11.8l7.6-7.6" />
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
