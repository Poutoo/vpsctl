<script setup lang="ts">
/**
 * Action en icône dans une ligne de tableau, avec infobulle et libellé
 * accessible. Indisponible (`unavailable`) : grisé, et l'infobulle en donne
 * la raison. Le bouton reste focalisable (`aria-disabled` plutôt que
 * `disabled`) pour que l'infobulle s'affiche au survol comme au clavier.
 * En cours (`busySince`) : chrono `T+02` à côté de l'icône, jamais de spinner.
 */
import { computed, toRef } from "vue";
import { formatElapsed, useElapsed } from "../../composables/useElapsed";
import AppIcon, { type AppIconName } from "./AppIcon.vue";
import AppTooltip from "./AppTooltip.vue";

const props = withDefaults(
  defineProps<{
    icon: AppIconName;
    label: string;
    /** Raison de l'indisponibilité, ou `null` si l'action est possible. */
    unavailable?: string | null;
    tone?: "default" | "danger";
    /** Début de l'action en cours (`performance.now()`). */
    busySince?: number | null;
  }>(),
  { unavailable: null, tone: "default", busySince: null },
);

const emit = defineEmits<{ click: [] }>();

const elapsed = useElapsed(toRef(props, "busySince"));
const chrono = computed(() =>
  elapsed.value === null ? null : formatElapsed(elapsed.value, "s"),
);
const inactive = computed(
  () => props.unavailable !== null || props.busySince !== null,
);
const description = computed(() =>
  props.unavailable === null
    ? props.label
    : `${props.label} : ${props.unavailable}`,
);

function onClick() {
  if (!inactive.value) emit("click");
}
</script>

<template>
  <AppTooltip :text="description" side="top">
    <button
      type="button"
      class="icon-button"
      :class="[tone, { unavailable: unavailable !== null, busy: chrono }]"
      :aria-label="description"
      :aria-disabled="inactive || undefined"
      :aria-busy="chrono !== null || undefined"
      @click="onClick"
    >
      <AppIcon :name="icon" />
      <span v-if="chrono" class="chrono">{{ chrono }}</span>
    </button>
  </AppTooltip>
</template>

<style scoped>
.icon-button {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  gap: var(--sp-1);
  min-width: var(--icon-button-size);
  height: var(--icon-button-size);
  padding: 0;
  background: none;
  border: var(--border-width) solid transparent;
  border-radius: var(--r-1);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
  cursor: pointer;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}

.icon-button:hover:not([aria-disabled]) {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

/* Forme rouge : --color-danger (icône), jamais du texte. */
.danger {
  color: var(--color-danger);
}

.danger:hover:not([aria-disabled]) {
  background: var(--color-danger-tint);
  color: var(--color-danger);
}

.unavailable {
  color: var(--color-text-subtle);
  cursor: not-allowed;
}

.busy {
  padding: 0 var(--sp-1);
  border-color: var(--color-border-strong);
  color: var(--color-warning);
  cursor: progress;
}

.chrono {
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
}
</style>
