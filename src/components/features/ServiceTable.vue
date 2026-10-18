<script setup lang="ts">
/** Services systemd utilisateur et leurs actions. */
import { computed } from "vue";
import type { RunState } from "../../bindings/RunState";
import type { Service } from "../../bindings/Service";
import {
  serviceTarget,
  targetKey,
  useServicesStore,
} from "../../stores/services";
import DataTable from "../ui/DataTable.vue";
import IconButton from "../ui/IconButton.vue";
import RunStateLabel from "../ui/RunStateLabel.vue";
import {
  OPERATION_PROGRESS,
  serviceAvailability,
  type ServiceOperation,
} from "./availability";

const props = defineProps<{
  services: Service[];
  /** Premier chargement : registre tracé et vide. */
  loading?: boolean;
}>();

const emit = defineEmits<{
  logs: [service: Service];
  operate: [service: Service, operation: ServiceOperation];
}>();

const store = useServicesStore();

/** Démarrer, arrêter, redémarrer : icône et libellé. */
const RUN_ACTIONS = [
  { operation: "start", icon: "play", label: "Démarrer" },
  { operation: "stop", icon: "stop", label: "Arrêter" },
  { operation: "restart", icon: "refresh", label: "Redémarrer" },
] as const;

/**
 * Affichage d'une ligne : pendant une action, « en transition » avec le
 * libellé et le chrono de l'action, et toutes les actions indisponibles
 * sauf les logs. Une unité masquée ou non pilotable n'a que les logs.
 */
function view(service: Service) {
  const key = targetKey(serviceTarget(service));
  const pending = store.pending[key] ?? null;
  const state: RunState = pending ? "transition" : service.state;
  const reasons = serviceAvailability(service);
  return {
    state,
    pending,
    arrival: store.arrivals[key] ?? null,
    controllable: service.controllable && !service.masked,
    reason: (operation: ServiceOperation) =>
      pending ? "action en cours" : reasons[operation],
    busySince: (operation: ServiceOperation) =>
      pending?.operation === operation ? pending.since : null,
  };
}

const rows = computed(() =>
  props.services.map((service) => ({ service, row: view(service) })),
);

/**
 * Nom hors du format accepté (`^[a-zA-Z0-9@._-]+\.service$`) : le backend
 * le refuse dans toute commande, logs compris. Une unité masquée, elle, garde
 * ses logs.
 */
function logsUnavailable(service: Service): string | null {
  return !service.controllable && !service.masked
    ? "nom hors du format accepté"
    : null;
}

/** Lignes vides tracées pendant le premier chargement. */
const PLACEHOLDER_ROWS = 3;

function rawState(service: Service): string {
  return `${service.load ?? "non chargé"} · ${service.active}/${service.sub}`;
}
</script>

<template>
  <DataTable>
    <colgroup>
      <col class="unit" />
      <col class="state" />
      <col class="description" />
      <col class="actions" />
    </colgroup>
    <thead>
      <tr>
        <th scope="col">Unité</th>
        <th scope="col">État</th>
        <th scope="col">Description</th>
        <th scope="col" class="actions-head">Actions</th>
      </tr>
    </thead>
    <tbody v-if="loading">
      <tr v-for="n in PLACEHOLDER_ROWS" :key="n" class="placeholder">
        <td>—</td>
        <td />
        <td />
        <td />
      </tr>
    </tbody>
    <TransitionGroup v-else name="row-in" tag="tbody" appear>
      <tr
        v-for="{ service, row } in rows"
        :key="service.unit"
        :class="{ failed: row.state === 'failed' }"
      >
        <td>
          <div class="unit-cell">
            <span class="truncate" :title="service.unit">{{
              service.unit
            }}</span>
            <span
              v-if="service.masked"
              class="caps tag"
              title="Unité masquée : elle ne peut pas être démarrée."
              >masqué</span
            >
            <span
              v-else-if="!service.controllable"
              class="caps tag"
              title="Nom hors du format accepté : aucune action possible sur ce service."
              >non pilotable</span
            >
          </div>
        </td>
        <td>
          <RunStateLabel
            :state="row.state"
            :raw="rawState(service)"
            :label="
              row.pending ? OPERATION_PROGRESS[row.pending.operation] : null
            "
            :since="row.pending?.since ?? null"
            :arrival="row.arrival"
          />
        </td>
        <td
          v-if="service.description !== null"
          class="truncate description-cell"
          :title="service.description"
        >
          {{ service.description }}
        </td>
        <td v-else class="muted" title="Unité non chargée par systemd.">—</td>
        <td>
          <div class="row-actions">
            <template v-if="row.controllable">
              <IconButton
                v-for="action in RUN_ACTIONS"
                :key="action.operation"
                :icon="action.icon"
                :label="`${action.label} ${service.unit}`"
                :unavailable="row.reason(action.operation)"
                :busy-since="row.busySince(action.operation)"
                @click="emit('operate', service, action.operation)"
              />
            </template>
            <IconButton
              icon="logs"
              :label="`Télémétrie de ${service.unit}`"
              :unavailable="logsUnavailable(service)"
              @click="emit('logs', service)"
            />
            <span class="separator" aria-hidden="true" />
            <IconButton
              icon="trash"
              tone="danger"
              :label="`Supprimer ${service.unit}`"
              :unavailable="row.reason('remove')"
              :busy-since="row.busySince('remove')"
              @click="emit('operate', service, 'remove')"
            />
          </div>
        </td>
      </tr>
    </TransitionGroup>
  </DataTable>
</template>

<style scoped>
.unit {
  width: 34%;
}

.state {
  width: 20%;
}

/* La description prend la largeur restante. */

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

.unit-cell {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
}

.tag {
  flex-shrink: 0;
  padding: 0 var(--sp-1);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
  color: var(--color-text-muted);
}

/* Description : texte courant, pas une donnée technique. */
.description-cell {
  font-family: var(--font-ui);
}
</style>
