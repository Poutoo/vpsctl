<script setup lang="ts">
/**
 * Aperçu en direct de la lisibilité (planche VI) : une miniature de
 * l'interface posée sur l'image de fond, avec l'assombrissement et les
 * couleurs choisis (mêmes variables que le fond réel). Purement décoratif.
 */
const SECTIONS = ["Tableau de bord", "Fichiers", "Scripts", "Réglages"];
const NUMERALS = ["I", "II", "III", "IV"];
const FILES = [
  { name: "docker-compose.yml", size: "1,2 Ko" },
  { name: "Caddyfile", size: "418 o", selected: true },
  { name: ".env", size: "412 o" },
  { name: "notes.txt", size: "612 o" },
];
</script>

<template>
  <div class="preview" aria-hidden="true">
    <span class="image" />
    <ul class="nav">
      <li
        v-for="(label, index) in SECTIONS"
        :key="label"
        :class="{ active: index === 1 }"
      >
        <span class="numeral">{{ NUMERALS[index] }}</span>
        <span class="section">{{ label }}</span>
      </li>
    </ul>
    <ul class="files mono">
      <li
        v-for="file in FILES"
        :key="file.name"
        :class="{ selected: file.selected }"
      >
        <span class="name">{{ file.name }}</span>
        <span>{{ file.size }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.preview {
  position: relative;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  height: var(--dimming-preview-height);
  overflow: hidden;
  background: var(--color-bg);
  border: var(--border-width) solid var(--color-border-strong);
}

.image {
  position: absolute;
  inset: 0;
  background: var(--bg-image) center / cover no-repeat;
  opacity: var(--bg-opacity);
  mix-blend-mode: var(--bg-blend);
  filter: var(--bg-filter);
}

.nav,
.files {
  position: relative;
  margin: 0;
  padding: var(--sp-2) 0;
  list-style: none;
}

.nav {
  background: var(--color-surface);
  border-right: var(--border-width) solid var(--color-border);
  font-size: var(--fs-sm);
}

.nav li {
  display: flex;
  gap: var(--sp-2);
  padding: var(--sp-1) var(--sp-3);
  overflow: hidden;
  color: var(--color-text-muted);
  white-space: nowrap;
}

.nav .numeral {
  font-family: var(--font-title);
}

.nav .section {
  overflow: hidden;
  text-overflow: ellipsis;
}

.nav .active {
  background: var(--color-surface-raised);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-action);
  color: var(--color-text);
}

.nav .active .numeral {
  color: var(--color-action);
}

.files {
  padding: var(--sp-2) var(--sp-3);
  font-size: var(--fs-sm);
}

.files li {
  display: flex;
  justify-content: space-between;
  gap: var(--sp-3);
  padding: var(--sp-1) var(--sp-2);
  border-bottom: var(--border-width) solid var(--color-border);
}

.files .name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.files .selected {
  background: var(--color-selection);
}
</style>
