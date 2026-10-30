<script setup lang="ts">
/**
 * Image de fond plein écran, sous toute l'interface. Pilotée par les tokens
 * `--bg-image`, `--bg-opacity`, `--bg-blend` et `--bg-filter` : n'importe
 * quelle image doit pouvoir être utilisée, le contraste étant porté par les
 * surfaces opaques.
 *
 * Parallaxe : l'image se décale de `--bg-parallax-shift` par section, en
 * `--dur-slow`. Désactivée en mouvement réduit.
 *
 * Préférences : `--bg-opacity` (assombrissement) et `--bg-filter` (couleurs
 * d'origine) sont posés sur la racine du document, pour que les aperçus des
 * Réglages suivent le fond. Rien n'est affiché avant leur lecture, pour ne
 * pas montrer un fond qui changerait aussitôt.
 */
import { computed, watchEffect } from "vue";
import { useRoute } from "vue-router";
import { SECTIONS, sectionOf } from "../../navigation";
import { usePreferencesStore } from "../../stores/preferences";

const route = useRoute();
const preferences = usePreferencesStore();

watchEffect(() => {
  const root = document.documentElement.style;
  root.setProperty(
    "--bg-opacity",
    String(preferences.ready ? preferences.backgroundOpacity : 0),
  );
  if (preferences.originalColors) {
    root.setProperty("--bg-filter", "var(--bg-filter-original)");
  } else {
    root.removeProperty("--bg-filter");
  }
});

const style = computed(() => {
  const section = sectionOf(route.name);
  return {
    "--bg-index": section ? SECTIONS.indexOf(section) : 0,
    "--bg-max-index": SECTIONS.length - 1,
  };
});
</script>

<template>
  <div class="background" :style="style" aria-hidden="true" />
</template>

<style scoped>
.background {
  position: fixed;
  inset: 0 calc(-1 * var(--bg-max-index) * var(--bg-parallax-shift)) 0 0;
  z-index: var(--z-background);
  background: var(--bg-image) center / cover no-repeat;
  opacity: var(--bg-opacity);
  mix-blend-mode: var(--bg-blend);
  filter: var(--bg-filter);
  pointer-events: none;
  transform: translate3d(
    calc(-1 * var(--bg-index) * var(--bg-parallax-shift)),
    0,
    0
  );
  transition: transform var(--dur-slow) var(--ease-in-out);
}

@media (prefers-reduced-motion: reduce) {
  .background {
    inset: 0;
    transform: none;
  }
}
</style>
