<script setup lang="ts">
/**
 * Bouton : principal (ambre plein), secondaire (contour) ou destructif
 * (contour et texte rouge). Désactivé : contour pointillé. En cours : chrono
 * `T+02` à côté du libellé, jamais de spinner.
 *
 * Avec `to`, rend un lien de navigation ; désactivé, il redevient un bouton
 * inactif (un lien ne peut pas être désactivé).
 */
import { computed, toRef } from "vue";
import type { RouteLocationRaw } from "vue-router";
import { formatElapsed, useElapsed } from "../../composables/useElapsed";

const props = withDefaults(
  defineProps<{
    variant?: "primary" | "secondary" | "danger";
    type?: "button" | "submit";
    disabled?: boolean;
    /** Début de l'action en cours (`performance.now()`) : affiche le chrono. */
    busySince?: number | null;
    to?: RouteLocationRaw;
  }>(),
  {
    variant: "secondary",
    type: "button",
    disabled: false,
    busySince: null,
    to: undefined,
  },
);

const elapsed = useElapsed(toRef(props, "busySince"));
const chrono = computed(() =>
  elapsed.value === null ? null : formatElapsed(elapsed.value, "s"),
);
</script>

<template>
  <RouterLink v-if="to && !disabled" :to="to" class="button" :class="variant">
    <slot />
  </RouterLink>
  <button
    v-else
    :type="type"
    class="button"
    :class="variant"
    :disabled="disabled"
    :aria-busy="chrono !== null || undefined"
  >
    <slot />
    <span v-if="chrono" class="chrono">{{ chrono }}</span>
  </button>
</template>

<style scoped>
.button {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  gap: var(--sp-2);
  min-height: var(--row-height);
  padding: var(--sp-2) var(--sp-4);
  border: var(--border-width) solid transparent;
  border-radius: var(--r-1);
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--fs-md);
  font-weight: 600;
  line-height: 1;
  text-decoration: none;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    border-color var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}

.primary {
  background: var(--color-action);
  border-color: var(--color-action);
  color: var(--color-on-action);
}

.primary:hover {
  background: var(--color-action-hover);
  border-color: var(--color-action-hover);
}

.primary:active {
  background: var(--color-action-pressed);
  border-color: var(--color-action-pressed);
}

.secondary {
  border-color: var(--color-border-strong);
  color: var(--color-text);
}

.secondary:hover {
  background: var(--color-surface-hover);
}

.secondary:active {
  background: var(--color-surface-raised);
}

.danger {
  border-color: var(--color-danger);
  color: var(--color-danger-text);
}

.danger:hover {
  background: var(--color-danger-tint);
}

.danger:active {
  background: var(--color-danger-tint-strong);
}

/* Désactivé : contour pointillé, pas seulement grisé. */
.button:disabled:not([aria-busy]) {
  background: transparent;
  border-color: var(--color-border-strong);
  border-style: dashed;
  color: var(--color-text-subtle);
  cursor: not-allowed;
}

/* En cours : garde l'apparence de sa variante, sans réagir au survol. */
.button[aria-busy]:disabled {
  pointer-events: none;
}

.chrono {
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  font-weight: 400;
}
</style>
