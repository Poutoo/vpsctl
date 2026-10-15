<script setup lang="ts">
/** Conteneurs Docker, en lecture seule. */
import type { Container } from "../../bindings/Container";
import DataTable from "../ui/DataTable.vue";
import RunStateLabel from "../ui/RunStateLabel.vue";

defineProps<{
  containers: Container[];
  /** Premier chargement : registre tracé et vide. */
  loading?: boolean;
}>();

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
    </colgroup>
    <thead>
      <tr>
        <th scope="col">Nom</th>
        <th scope="col">Image</th>
        <th scope="col">État</th>
        <th scope="col">Durée</th>
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
      </tr>
    </TransitionGroup>
  </DataTable>
</template>

<style scoped>
.name {
  width: 30%;
}

.image {
  width: 28%;
}

.state {
  width: 22%;
}

.duration {
  width: 20%;
}

.duration-cell {
  color: var(--color-text-muted);
}

.failed .duration-cell {
  color: var(--color-danger-text);
}
</style>
