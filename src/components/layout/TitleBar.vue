<script setup lang="ts">
/**
 * Barre de titre personnalisée (la barre native est désactivée :
 * `decorations: false`). Toute la barre déplace la fenêtre et l'agrandit au
 * double-clic (`data-tauri-drag-region="deep"`), sauf les éléments
 * interactifs, que Tauri exclut d'office, et l'indicateur AOS.
 */
import { onBeforeUnmount, onMounted, ref } from "vue";
import {
  closeWindow,
  minimizeWindow,
  toggleMaximizeWindow,
  watchMaximized,
} from "../../api/window";
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
  align-items: baseline;
  gap: var(--sp-3);
  align-self: center;
  padding-left: var(--sp-4);
  white-space: nowrap;
}

.name {
  font-family: var(--font-title);
  font-size: var(--fs-lg);
}

.product {
  color: var(--color-text-muted);
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
