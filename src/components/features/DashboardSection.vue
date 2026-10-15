<script setup lang="ts">
/**
 * Section du tableau de bord : titre, compteurs et ses propres états
 * (chargement, erreur, liste vide). Le contenu n'est affiché que s'il existe.
 */
import { computed } from "vue";
import { errorMessage } from "../../api/errors";
import type { RunState } from "../../bindings/RunState";
import type { Section } from "../../stores/services";
import { formatCounts } from "../ui/runState";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  title: string;
  section: Section<{ state: RunState }>;
  emptyText: string;
}>();

const counts = computed(() =>
  formatCounts(props.section.items.map((item) => item.state)),
);

/** Avertissement si des éléments illisibles ont été exclus de la liste. */
const skippedWarning = computed(() => {
  const { loaded, error, skipped } = props.section;
  if (!loaded || error || skipped === 0) return null;
  return skipped === 1
    ? "1 élément illisible dans la réponse du serveur a été ignoré."
    : `${skipped} éléments illisibles dans la réponse du serveur ont été ignorés.`;
});
</script>

<template>
  <section class="section" :aria-label="title">
    <header class="header">
      <h2 class="title">{{ title }}</h2>
      <span v-if="counts" class="counts">{{ counts }}</span>
    </header>
    <p v-if="skippedWarning" class="warning" role="status">
      <StatusIcon class="warning-shape" name="triangle" />
      {{ skippedWarning }}
    </p>
    <p v-if="!section.loaded" class="state">Chargement…</p>
    <p v-else-if="section.error" class="state error" role="alert">
      {{ errorMessage(section.error) }}
    </p>
    <p v-else-if="section.items.length === 0" class="state">
      {{ emptyText }}
    </p>
    <slot v-else />
  </section>
</template>

<style scoped>
.section {
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
  border-radius: var(--r-2);
}

.header {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
}

.title {
  margin: 0;
  font-size: var(--fs-xl);
  font-weight: 600;
}

.counts {
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

.state {
  margin: 0;
  padding: var(--sp-4);
  color: var(--color-text-muted);
  font-size: var(--fs-md);
}

.error {
  color: var(--color-danger-text);
}

.warning {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  margin: 0;
  padding: var(--sp-2) var(--sp-4);
  font-size: var(--fs-md);
  border-bottom: var(--border-width) solid var(--color-border);
}

.warning-shape {
  color: var(--color-warning);
}
</style>
