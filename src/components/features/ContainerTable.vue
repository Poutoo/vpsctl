<script setup lang="ts">
/** Conteneurs Docker et leurs actions. */
import { computed } from "vue";
import type { Container } from "../../bindings/Container";
import type { RunState } from "../../bindings/RunState";
import { useConnectionStore } from "../../stores/connection";
import {
  containerTarget,
  targetKey,
  useServicesStore,
  type Operation,
} from "../../stores/services";
import DataTable from "../ui/DataTable.vue";
import FilterNoMatch from "../ui/FilterNoMatch.vue";
import HighlightText from "../ui/HighlightText.vue";
import IconButton from "../ui/IconButton.vue";
import RunStateLabel from "../ui/RunStateLabel.vue";
import { containerAvailability, OPERATION_PROGRESS } from "./availability";

const props = withDefaults(
  defineProps<{
    /** Conteneurs affichés, déjà filtrés. */
    containers: Container[];
    /** Premier chargement : registre tracé et vide. */
    loading?: boolean;
    /** Filtre en cours, surligné dans les noms et les images. */
    query?: string;
    /** Conteneurs de la liste complète, avant le filtre. */
    total?: number;
  }>(),
  { loading: false, query: "", total: 0 },
);

const emit = defineEmits<{
  logs: [container: Container];
  operate: [container: Container, operation: Operation];
  clearFilter: [];
}>();

const store = useServicesStore();
const connection = useConnectionStore();

/** Lignes vides tracées pendant le premier chargement. */
const PLACEHOLDER_ROWS = 3;

/** Démarrer, arrêter, redémarrer : icône et libellé. */
const RUN_ACTIONS = [
  { operation: "start", icon: "play", label: "Démarrer" },
  { operation: "stop", icon: "stop", label: "Arrêter" },
  { operation: "restart", icon: "refresh", label: "Redémarrer" },
] as const;

/**
 * Affichage d'une ligne : pendant une action, « en transition » avec le
 * libellé et le chrono de l'action, et toutes les actions indisponibles
 * sauf les logs.
 */
function view(container: Container) {
  const key = targetKey(containerTarget(container));
  const pending = store.pending[key] ?? null;
  const state: RunState = pending ? "transition" : container.state;
  const reasons = containerAvailability(container);
  return {
    state,
    pending,
    arrival: store.arrivals[key] ?? null,
    reason: (operation: Operation) =>
      connection.lost
        ? "liaison perdue"
        : pending
          ? "action en cours"
          : reasons[operation],
    busySince: (operation: Operation) =>
      pending?.operation === operation ? pending.since : null,
  };
}

const rows = computed(() =>
  props.containers.map((container) => ({ container, row: view(container) })),
);

/** Code de sortie, affiché quand le conteneur est arrêté ou en échec. */
function exitDetail(container: Container): string | null {
  const shown = container.state === "stopped" || container.state === "failed";
  return shown && container.exitCode !== null
    ? `code ${container.exitCode}`
    : null;
}
</script>

<template>
  <DataTable>
    <colgroup>
      <col class="name" />
      <col class="image" />
      <col class="state" />
      <col class="duration" />
      <col class="actions" />
    </colgroup>
    <thead>
      <tr>
        <th scope="col">Nom</th>
        <th scope="col">Image</th>
        <th scope="col">État</th>
        <th scope="col">Durée</th>
        <th scope="col" class="actions-head">Actions</th>
      </tr>
    </thead>
    <tbody v-if="loading">
      <tr v-for="n in PLACEHOLDER_ROWS" :key="n" class="placeholder">
        <td>—</td>
        <td />
        <td />
        <td />
        <td />
      </tr>
    </tbody>
    <tbody v-else-if="containers.length === 0 && total > 0">
      <tr class="no-match-row">
        <td colspan="5">
          <FilterNoMatch
            :total="total"
            :query="query"
            @clear="emit('clearFilter')"
          />
        </td>
      </tr>
    </tbody>
    <TransitionGroup v-else name="row-in" tag="tbody" appear>
      <tr
        v-for="{ container, row } in rows"
        :key="container.id"
        :class="{ failed: row.state === 'failed' }"
      >
        <td class="truncate" :title="container.name">
          <HighlightText :text="container.name" :query="query" />
        </td>
        <td class="truncate muted" :title="container.image">
          <HighlightText :text="container.image" :query="query" />
        </td>
        <td>
          <RunStateLabel
            :state="row.state"
            :detail="exitDetail(container)"
            :raw="`${container.dockerState} · ${container.status}`"
            :label="
              row.pending ? OPERATION_PROGRESS[row.pending.operation] : null
            "
            :since="row.pending?.since ?? null"
            :arrival="row.arrival"
          />
        </td>
        <td class="truncate duration-cell" :title="container.status">
          {{ container.duration ?? "—" }}
        </td>
        <td>
          <div class="row-actions">
            <IconButton
              v-for="action in RUN_ACTIONS"
              :key="action.operation"
              :icon="action.icon"
              :label="`${action.label} ${container.name}`"
              :unavailable="row.reason(action.operation)"
              :busy-since="row.busySince(action.operation)"
              @click="emit('operate', container, action.operation)"
            />
            <IconButton
              icon="logs"
              :label="`Télémétrie de ${container.name}`"
              :unavailable="connection.lost ? 'liaison perdue' : null"
              @click="emit('logs', container)"
            />
            <span class="separator" aria-hidden="true" />
            <IconButton
              icon="trash"
              tone="danger"
              :label="`Supprimer ${container.name}`"
              :unavailable="row.reason('remove')"
              :busy-since="row.busySince('remove')"
              @click="emit('operate', container, 'remove')"
            />
          </div>
        </td>
      </tr>
    </TransitionGroup>
  </DataTable>
</template>

<style scoped>
/* Le nom prend la largeur restante. */
.image {
  width: 24%;
}

.state {
  width: 20%;
}

.duration {
  width: 18%;
}

/* Cinq actions, un séparateur, et la place d'un chrono T+. */
.actions {
  width: calc(
    5 * var(--icon-button-size) + 2 * var(--sp-4) + var(--sp-3) + var(--sp-7)
  );
}

.actions-head {
  text-align: right;
}

.row-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--sp-1);
}

/* Isole la suppression des autres actions. */
.separator {
  align-self: stretch;
  margin: var(--sp-1) var(--sp-1);
  border-left: var(--border-width) solid var(--color-border-strong);
}

.duration-cell {
  color: var(--color-text-muted);
}

.failed .duration-cell {
  color: var(--color-danger-text);
}
</style>
