<script setup lang="ts">
/** Services systemd utilisateur, en lecture seule. */
import type { Service } from "../../bindings/Service";
import DataTable from "../ui/DataTable.vue";
import RunStateLabel from "../ui/RunStateLabel.vue";

defineProps<{
  services: Service[];
}>();

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
    <tbody>
      <tr v-for="service in services" :key="service.unit">
        <td>
          <div class="unit-cell">
            <span class="truncate" :title="service.unit">{{
              service.unit
            }}</span>
            <span
              v-if="service.masked"
              class="tag"
              title="Unité masquée : elle ne peut pas être démarrée."
              >masqué</span
            >
            <span
              v-else-if="!service.controllable"
              class="tag"
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
          class="truncate"
          :title="service.description"
        >
          {{ service.description }}
        </td>
        <td v-else class="muted" title="Unité non chargée par systemd.">—</td>
      </tr>
    </tbody>
  </DataTable>
</template>

<style scoped>
.unit {
  width: 35%;
}

.state {
  width: 20%;
}

.description {
  width: 45%;
}

.unit-cell {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
}

.tag {
  flex-shrink: 0;
  color: var(--color-text-muted);
  font-size: var(--fs-xs);
  text-transform: uppercase;
}
</style>
