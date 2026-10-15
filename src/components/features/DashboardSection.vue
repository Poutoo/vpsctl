<script setup lang="ts">
/**
 * Section du tableau de bord : titre, commande interrogée, compteurs par
 * état, et ses propres états. Pendant le premier chargement, le tableau est
 * tracé et vide sous un bandeau « Interrogation » avec chrono (pas de
 * spinner) ; puis les données, une erreur ou un état vide.
 *
 * Le slot reçoit `loading` pour tracer les lignes vides du registre.
 */
import { computed, onMounted, ref } from "vue";
import { errorMessage } from "../../api/errors";
import type { RunState } from "../../bindings/RunState";
import { formatElapsed, useElapsed } from "../../composables/useElapsed";
import type { Section } from "../../stores/services";
import { countsByState, RUN_STATE_SHAPE } from "../ui/runState";
import StateMessage from "../ui/StateMessage.vue";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  title: string;
  /** Commande interrogée, ex. « docker ps -a ». */
  command: string;
  section: Section<{ state: RunState }>;
  emptyKicker: string;
  /** Chemin ou commande accolé à l'étiquette de l'état vide. */
  emptyDetail: string;
  emptyTitle: string;
  emptyText: string;
}>();

defineSlots<{ default(props: { loading: boolean }): unknown }>();

const counts = computed(() =>
  countsByState(props.section.items.map((item) => item.state)),
);

const loading = computed(() => !props.section.loaded);

/** Chrono du premier chargement, depuis l'affichage de la section. */
const mountedAt = ref<number | null>(null);
onMounted(() => (mountedAt.value = performance.now()));
const elapsed = useElapsed(
  computed(() => (loading.value ? mountedAt.value : null)),
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
  <section class="section" :aria-label="title" :aria-busy="loading">
    <header class="header">
      <h2 class="title">{{ title }}</h2>
      <span class="command">
        {{ command
        }}<template v-if="section.loaded && !section.error">
          · {{ section.items.length }}</template
        >
      </span>
      <ul v-if="counts.length" class="counts">
        <li v-for="count in counts" :key="count.state" :class="count.state">
          <StatusIcon class="shape" :name="RUN_STATE_SHAPE[count.state]" />
          {{ count.label }}
        </li>
      </ul>
    </header>

    <div class="card">
      <p v-if="loading" class="probe" role="status">
        <StatusIcon class="probe-shape" name="half" />
        <span class="caps"
          >Interrogation · <span class="verbatim">{{ command }}</span></span
        >
        <span v-if="elapsed !== null" class="probe-chrono">{{
          formatElapsed(elapsed, "ms")
        }}</span>
      </p>
      <p v-if="skippedWarning" class="warning" role="status">
        <StatusIcon class="warning-shape" name="triangle" />
        {{ skippedWarning }}
      </p>

      <StateMessage
        v-if="section.error"
        tone="error"
        kicker="Échec"
        :kicker-detail="command"
      >
        {{ errorMessage(section.error) }}
      </StateMessage>
      <StateMessage
        v-else-if="section.loaded && section.items.length === 0"
        :kicker="emptyKicker"
        :kicker-detail="emptyDetail"
        :title="emptyTitle"
      >
        {{ emptyText }}
      </StateMessage>
      <slot v-else :loading="loading" />
    </div>
  </section>
</template>

<style scoped>
.section {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}

.header {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--sp-3);
}

.command {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.counts {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-4);
  margin: 0 0 0 auto;
  padding: 0;
  list-style: none;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.counts li {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
}

.nominal .shape {
  color: var(--color-success);
}

.failed .shape {
  color: var(--color-danger);
}

.transition .shape {
  color: var(--color-warning);
}

/* Surface opaque sous les données. */
.card {
  overflow: hidden;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.probe {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  padding: var(--sp-2) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
  color: var(--color-warning);
}

.probe-chrono {
  margin-left: auto;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.warning {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  padding: var(--sp-2) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
  font-size: var(--fs-sm);
}

.warning-shape {
  color: var(--color-warning);
}
</style>
