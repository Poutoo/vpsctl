<script setup lang="ts">
/**
 * Ligne d'un conteneur : nom, image, état, durée et actions. Pendant une
 * action, « en transition » avec le libellé et le chrono de l'action, et
 * toutes les actions indisponibles sauf les logs ; de même pendant une
 * commande compose sur son projet. Dans un projet compose, un trait relie la
 * ligne à l'en-tête du groupe.
 */
import { computed } from "vue";
import type { Container } from "../../bindings/Container";
import type { RunState } from "../../bindings/RunState";
import { containerProgress } from "../../composeActions";
import type { ComposeRunning } from "../../stores/compose";
import { useConnectionStore } from "../../stores/connection";
import {
  containerTarget,
  targetKey,
  useServicesStore,
  type Operation,
} from "../../stores/services";
import HighlightText from "../ui/HighlightText.vue";
import IconButton from "../ui/IconButton.vue";
import RunStateLabel from "../ui/RunStateLabel.vue";
import { containerAvailability, OPERATION_PROGRESS } from "./availability";
import { containerStateDetail, containerStateHelp } from "./containerState";

const props = withDefaults(
  defineProps<{
    container: Container;
    query: string;
    /** Position dans un groupe : trait de rattachement à l'en-tête. */
    branch?: "middle" | "last" | null;
    /** Commande compose en cours sur le projet du conteneur. */
    composeRun?: ComposeRunning | null;
  }>(),
  { branch: null, composeRun: null },
);

const emit = defineEmits<{
  logs: [container: Container];
  operate: [container: Container, operation: Operation];
}>();

const store = useServicesStore();
const connection = useConnectionStore();

/** Démarrer, arrêter, redémarrer : icône et libellé. */
const RUN_ACTIONS = [
  { operation: "start", icon: "play", label: "Démarrer" },
  { operation: "stop", icon: "stop", label: "Arrêter" },
  { operation: "restart", icon: "refresh", label: "Redémarrer" },
] as const;

const key = computed(() => targetKey(containerTarget(props.container)));
const pending = computed(() => store.pending[key.value] ?? null);
/** Libellé de transition pendant une commande compose (« Démarrage »). */
const progress = computed(() =>
  props.composeRun
    ? containerProgress(props.composeRun.action, props.container)
    : null,
);
const state = computed<RunState>(() =>
  pending.value || progress.value ? "transition" : props.container.state,
);
const label = computed(() =>
  pending.value ? OPERATION_PROGRESS[pending.value.operation] : progress.value,
);
const since = computed(
  () =>
    pending.value?.since ??
    (progress.value ? (props.composeRun?.since ?? null) : null),
);
const detail = computed(() =>
  progress.value && props.composeRun
    ? `via docker compose ${props.composeRun.action}`
    : (props.container.duration ?? "—"),
);
const reasons = computed(() => containerAvailability(props.container));

function reason(operation: Operation): string | null {
  if (connection.lost) return "liaison perdue";
  if (pending.value) return "action en cours";
  if (props.composeRun) return "commande compose en cours sur son projet";
  return reasons.value[operation];
}

function busySince(operation: Operation): number | null {
  return pending.value?.operation === operation ? pending.value.since : null;
}
</script>

<template>
  <tr
    :class="{
      failed: state === 'failed',
      member: branch !== null,
      'compose-busy': progress !== null,
    }"
    :data-testid="`container-${container.name}`"
  >
    <td class="truncate name-cell" :title="container.name">
      <span v-if="branch" class="branch" :class="branch" aria-hidden="true" />
      <HighlightText :text="container.name" :query="query" />
      <span
        v-if="container.oneoff"
        class="caps run-tag"
        title="Lancé par docker compose run : non compté dans l'état du projet"
        >run</span
      >
    </td>
    <td class="truncate muted" :title="container.image">
      <HighlightText :text="container.image" :query="query" />
    </td>
    <td class="state-cell" data-testid="container-state">
      <RunStateLabel
        :state="state"
        :detail="containerStateDetail(container)"
        :tag="container.oomKilled && state !== 'transition' ? 'Mémoire' : null"
        :raw="containerStateHelp(container)"
        :label="label"
        :since="since"
        :arrival="store.arrivals[key] ?? null"
      />
      <span class="stacked-detail" :title="container.status">
        {{ detail }}
      </span>
    </td>
    <td
      class="truncate detail-cell"
      data-testid="container-detail"
      :title="container.status"
    >
      {{ detail }}
    </td>
    <td>
      <div class="row-actions">
        <IconButton
          v-for="action in RUN_ACTIONS"
          :key="action.operation"
          :icon="action.icon"
          :label="`${action.label} ${container.name}`"
          :unavailable="reason(action.operation)"
          :busy-since="busySince(action.operation)"
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
          :unavailable="reason('remove')"
          :busy-since="busySince('remove')"
          @click="emit('operate', container, 'remove')"
        />
      </div>
    </td>
  </tr>
</template>

<style scoped>
.name-cell {
  position: relative;
}

/* Ligne d'un projet : décalée, reliée à l'en-tête par un trait. */
.member .name-cell {
  padding-left: calc(var(--sp-4) + var(--sp-5));
}

.branch {
  position: absolute;
  top: 0;
  left: calc(var(--sp-4) + var(--sp-2));
  width: var(--sp-2);
  height: 50%;
  border-bottom: var(--border-width) solid var(--color-border-strong);
  border-left: var(--border-width) solid var(--color-border-strong);
}

.branch.middle::after {
  position: absolute;
  top: 100%;
  left: calc(-1 * var(--border-width));
  height: 100%;
  border-left: var(--border-width) solid var(--color-border-strong);
  content: "";
}

.run-tag {
  margin-left: var(--sp-2);
  padding: 0 var(--sp-1);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
  color: var(--color-text-muted);
  font-size: var(--fs-xs);
  font-weight: 400;
}

.detail-cell,
.stacked-detail {
  color: var(--color-text-muted);
}

.failed .detail-cell,
.failed .stacked-detail {
  color: var(--color-danger-text);
}

.compose-busy .detail-cell,
.compose-busy .stacked-detail {
  color: var(--color-warning);
}

/* Fenêtre étroite : la durée passe sous l'état (colonne « État · Détail »). */
.stacked-detail {
  display: none;
}

@media (max-width: 1100px) {
  .state-cell {
    padding-top: var(--sp-1);
    padding-bottom: var(--sp-1);
  }

  .stacked-detail {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail-cell {
    padding: 0;
    font-size: 0;
  }
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
</style>
