<script setup lang="ts">
/**
 * État agrégé d'un projet compose : forme, libellé et décompte
 * (« Partiel · 2/4 »), jamais la couleur seule. Pendant une commande, le
 * verbe et son chrono remplacent l'état (« Up · T+2.1 »), sur la forme
 * « en transition ».
 */
import { computed, toRef } from "vue";
import type { ProjectState } from "../../bindings/ProjectState";
import { formatElapsed, useElapsed } from "../../composables/useElapsed";
import { PROJECT_STATE_LABEL, PROJECT_STATE_SHAPE } from "../../composeView";
import StatusIcon from "./StatusIcon.vue";

const props = withDefaults(
  defineProps<{
    state: ProjectState;
    /** « 2/4 » : conteneurs en marche sur le total. */
    count: string;
    /** Verbe de la commande en cours, à la place de l'état. */
    label?: string | null;
    /** Début de la commande en cours (`performance.now()`). */
    since?: number | null;
  }>(),
  { label: null, since: null },
);

const elapsed = useElapsed(toRef(props, "since"));
const chrono = computed(() =>
  elapsed.value === null ? null : formatElapsed(elapsed.value, "s"),
);
const tone = computed(() => (props.label ? "transition" : props.state));
</script>

<template>
  <span class="project-state" :class="tone">
    <StatusIcon
      class="shape"
      :name="label ? 'half' : PROJECT_STATE_SHAPE[state]"
    />
    <span class="caps label">{{ label ?? PROJECT_STATE_LABEL[state] }}</span>
    <span v-if="chrono" class="chrono">· {{ chrono }}</span>
    <span v-else class="count">· {{ count }}</span>
  </span>
</template>

<style scoped>
.project-state {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  max-width: 100%;
  white-space: nowrap;
}

.label {
  font-weight: 600;
}

.count,
.chrono {
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
}

/* Forme : --color-danger ; texte : --color-danger-text (contraste). */
.nominal {
  color: var(--color-success);
}

.stopped,
.down {
  color: var(--color-text);
}

.stopped .shape,
.down .shape {
  color: var(--color-text-muted);
}

.failed {
  color: var(--color-danger-text);
}

.failed .shape {
  color: var(--color-danger);
}

.partial,
.transition {
  color: var(--color-warning);
}
</style>
