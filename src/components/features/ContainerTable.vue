<script setup lang="ts">
/** Conteneurs Docker et leurs actions. */
import type { Container } from "../../bindings/Container";
import DataTable from "../ui/DataTable.vue";
import IconButton from "../ui/IconButton.vue";
import RunStateLabel from "../ui/RunStateLabel.vue";

defineProps<{
  containers: Container[];
  /** Premier chargement : registre tracé et vide. */
  loading?: boolean;
}>();

const emit = defineEmits<{ logs: [container: Container] }>();

/** Lignes vides tracées pendant le premier chargement. */
const PLACEHOLDER_ROWS = 3;

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
    <TransitionGroup v-else name="row-in" tag="tbody" appear>
      <tr
        v-for="container in containers"
        :key="container.id"
        :class="{ failed: container.state === 'failed' }"
      >
        <td class="truncate" :title="container.name">{{ container.name }}</td>
        <td class="truncate muted" :title="container.image">
          {{ container.image }}
        </td>
        <td>
          <RunStateLabel
            :state="container.state"
            :detail="exitDetail(container)"
            :raw="`${container.dockerState} · ${container.status}`"
          />
        </td>
        <td class="truncate duration-cell" :title="container.status">
          {{ container.duration ?? "—" }}
        </td>
        <td>
          <div class="row-actions">
            <IconButton
              icon="logs"
              :label="`Télémétrie de ${container.name}`"
              @click="emit('logs', container)"
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

.duration-cell {
  color: var(--color-text-muted);
}

.failed .duration-cell {
  color: var(--color-danger-text);
}
</style>
