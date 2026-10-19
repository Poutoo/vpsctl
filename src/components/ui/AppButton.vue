<script setup lang="ts">
/**
 * Bouton : principal (ambre plein), secondaire (contour) ou destructif
 * (contour et texte rouge). Désactivé : contour pointillé. En cours : chrono
 * `T+02` à côté du libellé, jamais de spinner.
 *
 * Dans une ligne de tableau : taille `compact` (moins haute que la ligne),
 * et variantes `quiet` / `quiet-danger` (libellé seul, contour au survol)
 * pour les actions secondaires.
 *
 * Avec `to`, rend un lien de navigation ; désactivé, il redevient un bouton
 * inactif (un lien ne peut pas être désactivé).
 */
import { computed, toRef } from "vue";
import type { RouteLocationRaw } from "vue-router";
import { formatElapsed, useElapsed } from "../../composables/useElapsed";

const props = withDefaults(
  defineProps<{
    variant?: "primary" | "secondary" | "danger" | "quiet" | "quiet-danger";
    size?: "default" | "compact";
    type?: "button" | "submit";
    disabled?: boolean;
    /** Début de l'action en cours (`performance.now()`) : affiche le chrono. */
    busySince?: number | null;
    to?: RouteLocationRaw;
  }>(),
  {
    variant: "secondary",
    size: "default",
    type: "button",
    disabled: false,
    busySince: null,
    to: undefined,
  },
);

// Pas de classe pour la taille par défaut : le style local d'un parent
// s'applique aussi à la racine de ce composant, et une classe aussi
// générique que `default` y reprenait ses règles (texte ambre).
const classes = computed(() => [
  props.variant,
  { compact: props.size === "compact" },
]);

const elapsed = useElapsed(toRef(props, "busySince"));
const chrono = computed(() =>
  elapsed.value === null ? null : formatElapsed(elapsed.value, "s"),
);
</script>

<template>
  <RouterLink v-if="to && !disabled" :to="to" class="button" :class="classes">
    <slot />
  </RouterLink>
  <button
    v-else
    :type="type"
    class="button"
    :class="classes"
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

/* Action secondaire d'une ligne : libellé seul, contour au survol. */
.quiet {
  color: var(--color-text-muted);
}

.quiet:hover {
  border-color: var(--color-border-strong);
  color: var(--color-text);
}

.quiet-danger {
  color: var(--color-danger-text);
}

.quiet-danger:hover {
  border-color: var(--color-danger);
  background: var(--color-danger-tint);
}

/* Dans une ligne de tableau : moins haut que la ligne, qui garde de l'air. */
.compact {
  min-height: var(--button-height-compact);
  padding: var(--sp-1) var(--sp-3);
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
