<script setup lang="ts">
/**
 * État d'un service : forme et libellé, jamais la couleur seule.
 *
 * Pendant une action, l'état « en transition » porte le libellé de l'action
 * et son chrono (« Démarrage · T+02 »). Après une action vérifiée,
 * `arrival` joue une fois la trajectoire d'allumage ou d'extinction de la
 * forme (`transform` et `opacity` uniquement, `--dur-slow`) ; jamais lors
 * d'un rafraîchissement automatique.
 */
import { computed, toRef } from "vue";
import type { RunState } from "../../bindings/RunState";
import { formatElapsed, useElapsed } from "../../composables/useElapsed";
import { RUN_STATE_LABEL, RUN_STATE_SHAPE } from "./runState";
import StatusIcon from "./StatusIcon.vue";

const props = withDefaults(
  defineProps<{
    state: RunState;
    /** Complément affiché après le libellé, ex. « code 137 ». */
    detail?: string | null;
    /** Mention encadrée après le libellé, ex. « Mémoire ». */
    tag?: string | null;
    /** État brut, en infobulle. */
    raw: string;
    /** Libellé à la place de celui de l'état, ex. « Démarrage ». */
    label?: string | null;
    /** Début de l'action en cours (`performance.now()`) : chrono T+. */
    since?: number | null;
    /** Animation de changement d'état à jouer une fois. */
    arrival?: { direction: "on" | "off"; token: number } | null;
  }>(),
  { detail: null, tag: null, label: null, since: null, arrival: null },
);

const elapsed = useElapsed(toRef(props, "since"));
const chrono = computed(() =>
  elapsed.value === null ? null : formatElapsed(elapsed.value, "s"),
);
</script>

<template>
  <span class="run-state" :class="state" :title="raw">
    <span
      :key="arrival?.token ?? 0"
      class="shape-wrap"
      :class="arrival ? `arrive-${arrival.direction}` : null"
    >
      <StatusIcon class="shape" :name="RUN_STATE_SHAPE[state]" />
    </span>
    <span class="caps label">{{ label ?? RUN_STATE_LABEL[state] }}</span>
    <span v-if="chrono" class="chrono">· {{ chrono }}</span>
    <span v-else-if="detail" class="detail">· {{ detail }}</span>
    <span v-if="tag && !chrono" class="caps tag">{{ tag }}</span>
  </span>
</template>

<style scoped>
.run-state {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  max-width: 100%;
  white-space: nowrap;
}

.shape-wrap {
  display: inline-flex;
}

.label {
  font-weight: 600;
}

.detail {
  overflow: hidden;
  color: var(--color-text-muted);
  text-overflow: ellipsis;
}

/* Mention encadrée : contour (forme) rouge, texte lisible. */
.tag {
  padding: 0 var(--sp-1);
  border: var(--border-width) solid var(--color-danger);
  border-radius: var(--r-1);
}

.chrono {
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
}

/* Forme : --color-danger ; texte : --color-danger-text (contraste). */
.nominal {
  color: var(--color-success);
}

.stopped {
  color: var(--color-text);
}

.stopped .shape {
  color: var(--color-text-muted);
}

.failed {
  color: var(--color-danger-text);
}

.failed .shape {
  color: var(--color-danger);
}

.transition {
  color: var(--color-warning);
}

/*
 * Trajectoire d'allumage : la forme arrive d'en bas à gauche en décrivant un
 * arc, et grandit jusqu'à sa taille. Extinction : elle redescend de l'arc en
 * se contractant. Jouées une seule fois.
 */
.arrive-on {
  animation: arrive-on var(--dur-slow) var(--ease-out);
}

.arrive-off {
  animation: arrive-off var(--dur-slow) var(--ease-out);
}

@keyframes arrive-on {
  0% {
    opacity: 0;
    transform: translate(calc(-1 * var(--sp-2)), var(--sp-2)) scale(0.4);
  }

  60% {
    opacity: 1;
    transform: translate(calc(-1 * var(--sp-1)), calc(-1 * var(--sp-1)))
      scale(1.15);
  }
}

@keyframes arrive-off {
  0% {
    transform: translate(calc(-1 * var(--sp-1)), calc(-1 * var(--sp-1)))
      scale(1.3);
  }

  50% {
    opacity: 0.4;
    transform: translate(0, var(--sp-1)) scale(0.7);
  }
}
</style>
