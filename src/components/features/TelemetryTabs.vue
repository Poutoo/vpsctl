<script setup lang="ts">
/**
 * Onglets de l'écran Télémétrie : un par flux ouvert, avec sa forme d'état
 * et un bouton de fermeture. L'onglet actif est celui de l'URL.
 */
import { streamName, type TelemetryTab } from "../../stores/telemetry";
import AppIcon from "../ui/AppIcon.vue";
import AppTooltip from "../ui/AppTooltip.vue";
import StatusIcon from "../ui/StatusIcon.vue";
import { streamState } from "./streamState";

defineProps<{
  tabs: TelemetryTab[];
  activeId: string | null;
}>();

const emit = defineEmits<{ close: [tab: TelemetryTab] }>();
</script>

<template>
  <nav class="tabs" aria-label="Flux ouverts">
    <div
      v-for="tab in tabs"
      :key="tab.id"
      class="tab"
      :class="{ active: tab.id === activeId }"
    >
      <RouterLink
        :to="{ name: 'telemetry', params: { tab: tab.id } }"
        class="link"
        :aria-current="tab.id === activeId ? 'page' : undefined"
      >
        <StatusIcon
          class="shape"
          :class="streamState(tab).tone"
          :name="streamState(tab).shape"
        />
        <span class="name" :title="streamName(tab)">{{ streamName(tab) }}</span>
        <span class="visually-hidden">, {{ streamState(tab).label }}</span>
      </RouterLink>
      <AppTooltip text="Fermer l'onglet">
        <button
          type="button"
          class="close"
          :aria-label="`Fermer l'onglet ${streamName(tab)}`"
          @click="emit('close', tab)"
        >
          <AppIcon name="close" />
        </button>
      </AppTooltip>
    </div>
  </nav>
</template>

<style scoped>
.tabs {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.tab {
  display: flex;
  align-items: center;
  min-width: 0;
  border-right: var(--border-width) solid var(--color-border);
  color: var(--color-text-muted);
  transition:
    background-color var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}

.tab:last-child {
  border-right: none;
}

.tab:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.tab.active {
  background: var(--color-action);
  color: var(--color-on-action);
}

.link {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
  padding: var(--sp-2) var(--sp-1) var(--sp-2) var(--sp-3);
  color: inherit;
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-decoration: none;
}

.link:focus-visible,
.close:focus-visible {
  outline-offset: calc(-1 * var(--focus-offset));
}

.name {
  overflow: hidden;
  max-width: 24ch;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Couleur d'état hors de l'onglet actif ; dans l'onglet actif, la forme seule. */
.tab:not(.active) .success {
  color: var(--color-success);
}

.tab:not(.active) .warning {
  color: var(--color-warning);
}

.tab:not(.active) .danger {
  color: var(--color-danger);
}

.close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--icon-button-size);
  height: var(--icon-button-size);
  margin-right: var(--sp-1);
  padding: 0;
  background: none;
  border: none;
  color: inherit;
  cursor: pointer;
}

.close:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.active .close:hover {
  background: var(--color-action-hover);
  color: var(--color-on-action);
}
</style>
