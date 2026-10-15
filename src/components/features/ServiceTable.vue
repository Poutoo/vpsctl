<script setup lang="ts">
/** Services systemd utilisateur, en lecture seule. */
import type { Service } from "../../bindings/Service";
import DataTable from "../ui/DataTable.vue";
import RunStateLabel from "../ui/RunStateLabel.vue";

defineProps<{
  services: Service[];
  /** Premier chargement : registre tracé et vide. */
  loading?: boolean;
}>();

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
    </colgroup>
    <thead>
      <tr>
        <th scope="col">Unité</th>
        <th scope="col">État</th>
        <th scope="col">Description</th>
      </tr>
    </thead>
    <tbody v-if="loading">
      <tr v-for="n in PLACEHOLDER_ROWS" :key="n" class="placeholder">
        <td>—</td>
        <td />
        <td />
      </tr>
    </tbody>
    <TransitionGroup v-else name="row-in" tag="tbody" appear>
      <tr
        v-for="service in services"
        :key="service.unit"
        :class="{ failed: service.state === 'failed' }"
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
          <RunStateLabel :state="service.state" :raw="rawState(service)" />
        </td>
        <td
          v-if="service.description !== null"
          class="truncate description-cell"
          :title="service.description"
        >
          {{ service.description }}
        </td>
        <td v-else class="muted" title="Unité non chargée par systemd.">—</td>
      </tr>
    </TransitionGroup>
  </DataTable>
</template>

<style scoped>
.unit {
  width: 35%;
}

.state {
  width: 22%;
}

.description {
  width: 43%;
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
