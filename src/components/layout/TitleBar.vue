<script setup lang="ts">
/**
 * Barre de titre personnalisée (la barre native est désactivée :
 * `decorations: false`). Toute la barre déplace la fenêtre et l'agrandit au
 * double-clic (`data-tauri-drag-region="deep"`), sauf les éléments
 * interactifs, que Tauri exclut d'office, et l'indicateur AOS.
 *
 * Marque (planche V) : symbole de 32 px sur toute la hauteur de la barre,
 * décoratif car le mot « Atlas » le nomme. Il n'est jamais transformé en URI
 * `data:` (`?no-inline`), que la CSP bloquerait, et ne se glisse pas comme
 * une image : le glisser déplace la fenêtre. En fenêtre étroite, « Contrôle
 * VPS » disparaît pour laisser la place à l'indicateur de liaison.
 */
import { onBeforeUnmount, onMounted, ref } from "vue";
import {
  closeWindow,
  minimizeWindow,
  toggleMaximizeWindow,
  watchMaximized,
} from "../../api/window";
import symbol from "../../assets/logo/atlas-symbole.svg?no-inline";
import AppIcon from "../ui/AppIcon.vue";
import AppTooltip from "../ui/AppTooltip.vue";
import AosIndicator from "./AosIndicator.vue";

const maximized = ref(false);
let stopWatching: (() => void) | null = null;
let mounted = false;

onMounted(async () => {
  mounted = true;
  const stop = await watchMaximized((value) => (maximized.value = value));
  if (mounted) stopWatching = stop;
  else stop();
});

onBeforeUnmount(() => {
  mounted = false;
  stopWatching?.();
});
</script>

<template>
  <header class="titlebar" data-tauri-drag-region="deep">
    <p class="brand">
      <img class="symbol" :src="symbol" alt="" draggable="false" />
      <span class="name">Atlas</span>
      <span class="caps product">Contrôle VPS</span>
    </p>

    <div class="signal">
      <AosIndicator />
    </div>

    <div class="controls">
      <AppTooltip text="Réduire">
        <button
          type="button"
          class="control"
          aria-label="Réduire"
          @click="minimizeWindow"
        >
          <AppIcon name="minimize" />
        </button>
      </AppTooltip>
      <AppTooltip :text="maximized ? 'Restaurer' : 'Agrandir'">
        <button
          type="button"
          class="control"
          :aria-label="maximized ? 'Restaurer' : 'Agrandir'"
          @click="toggleMaximizeWindow"
        >
          <AppIcon :name="maximized ? 'restore' : 'maximize'" />
        </button>
      </AppTooltip>
      <AppTooltip text="Fermer">
        <button
          type="button"
          class="control close"
          aria-label="Fermer"
          @click="closeWindow"
        >
          <AppIcon name="close" />
        </button>
      </AppTooltip>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  align-items: stretch;
  height: var(--titlebar-height);
  background: var(--color-bg);
  border-bottom: var(--border-width) solid var(--color-border);
  user-select: none;
  -webkit-user-select: none;
}

.brand {
  display: flex;
  align-items: center;
  padding-left: var(--sp-2);
  white-space: nowrap;
}

.symbol {
  flex-shrink: 0;
  width: var(--titlebar-height);
  height: var(--titlebar-height);
}

.name {
  margin-left: var(--sp-1);
  font-family: var(--font-title);
  font-size: var(--fs-brand);
}

.product {
  margin-left: var(--sp-3);
  color: var(--color-text-muted);
}

/* Fenêtre étroite (seuil de NARROW_QUERY) : la place va à l'indicateur. */
@media (max-width: 1100px) {
  .product {
    display: none;
  }
}

.signal {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: flex-end;
  min-width: 0;
  padding: 0 var(--sp-4);
}

.controls {
  display: flex;
}

.control {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--titlebar-button-width);
  padding: 0;
  background: transparent;
  border: none;
  color: var(--color-text-muted);
  font-size: var(--fs-md);
  cursor: default;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}

.control:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.control:active {
  background: var(--color-surface-raised);
}

.control:focus-visible {
  outline-offset: calc(-1 * var(--focus-offset) - var(--border-width-accent));
}

/* Fermer : survol rouge, comme les fenêtres natives. */
.close:hover,
.close:active {
  background: var(--color-danger);
  color: var(--color-on-danger);
}
</style>
