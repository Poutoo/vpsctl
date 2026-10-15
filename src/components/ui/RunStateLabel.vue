<script setup lang="ts">
/** État d'un service : forme et libellé, jamais la couleur seule. */
import type { RunState } from "../../bindings/RunState";
import { RUN_STATE_LABEL } from "./runState";
import StatusIcon from "./StatusIcon.vue";

defineProps<{
  state: RunState;
  /** Complément affiché après le libellé, ex. « code 137 ». */
  detail?: string | null;
  /** État brut, en infobulle. */
  raw: string;
}>();

const SHAPE = {
  nominal: "disc",
  stopped: "circle",
  failed: "triangle",
  transition: "half",
} as const;
</script>

<template>
  <span class="run-state" :class="state" :title="raw">
    <StatusIcon class="shape" :name="SHAPE[state]" />
    <span>{{ RUN_STATE_LABEL[state] }}</span>
    <span v-if="detail" class="detail">· {{ detail }}</span>
  </span>
</template>

<style scoped>
.run-state {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  white-space: nowrap;
}

.detail {
  color: var(--color-text-muted);
}

.nominal .shape {
  color: var(--color-success);
}

.stopped .shape {
  color: var(--color-text-muted);
}

.failed .shape {
  color: var(--color-danger);
}

.transition .shape {
  color: var(--color-warning);
}
</style>
