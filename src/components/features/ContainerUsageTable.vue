<script setup lang="ts">
/**
 * Conteneurs triés par mémoire (`docker stats`, relevé uniquement quand
 * l'écran est affiché) : CPU en % d'un cœur, mémoire et limite. Une ligne
 * désignée par « Voir <conteneur> » est surlignée quelques secondes.
 */
import { computed } from "vue";
import { formatBytes } from "../../format";
import { formatCpu } from "../../resourceText";
import { useResourcesStore } from "../../stores/resources";
import ResourceState from "./ResourceState.vue";

const store = useResourcesStore();

const measure = computed(() => store.update?.containers ?? null);
const containers = computed(() =>
  measure.value?.status === "ok" ? measure.value.value : [],
);
const reading = computed(
  () =>
    !measure.value ||
    measure.value.status === "pending" ||
    measure.value.status === "notRead",
);

function percent(value: number, of: number): string {
  return `${of > 0 ? Math.min(100, Math.max(0, (value / of) * 100)) : 0}%`;
}
</script>

<template>
  <section class="panel" data-testid="containers-panel">
    <header class="head">
      <h2 class="caps">Conteneurs · triés par mémoire</h2>
      <span class="caps aside">{{
        reading ? "Lecture en cours" : "docker stats"
      }}</span>
    </header>
    <div class="row columns caps" aria-hidden="true">
      <span>Nom</span>
      <span>CPU · % d'un cœur</span>
      <span>Mémoire · limite</span>
    </div>

    <p
      v-if="measure?.status === 'unavailable'"
      class="row message"
      data-testid="containers-unavailable"
    >
      <ResourceState state="unavailable" />
      <span>{{ measure.reason }}. Les autres mesures restent à jour.</span>
    </p>
    <div v-else-if="reading" class="row skeleton" aria-hidden="true">
      <span class="block" /><span class="block" /><span class="block" />
    </div>
    <p v-else-if="containers.length === 0" class="row message">
      Aucun conteneur en marche.
    </p>
    <div
      v-for="container in containers"
      :key="container.id"
      class="row container"
      :class="{ highlighted: store.highlighted === container.id }"
      :data-container-id="container.id"
      :data-testid="`container-${container.name}`"
    >
      <span class="name mono">{{ container.name }}</span>
      <span class="metric">
        <span class="bar" aria-hidden="true">
          <span
            class="fill"
            :style="{ width: percent(container.cpuPercent, 100) }"
          />
        </span>
        <span class="mono value">{{ formatCpu(container.cpuPercent) }}</span>
      </span>
      <span class="metric">
        <span
          class="bar"
          :class="{ unlimited: container.memoryLimit === null }"
          aria-hidden="true"
        >
          <span
            v-if="container.memoryLimit !== null"
            class="fill"
            :style="{
              width: percent(container.memoryUsed, container.memoryLimit),
            }"
          />
        </span>
        <span class="mono value">
          <strong>{{ formatBytes(container.memoryUsed) }}</strong>
          <span class="limit">{{
            container.memoryLimit === null
              ? " · sans limite"
              : ` / ${formatBytes(container.memoryLimit)}`
          }}</span>
        </span>
      </span>
    </div>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  min-height: var(--row-height);
  padding: var(--sp-2) var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.head h2,
.aside,
.columns {
  color: var(--color-text-muted);
}

.row {
  display: grid;
  grid-template-columns: minmax(0, 3fr) minmax(0, 3fr) minmax(0, 5fr);
  align-items: center;
  gap: var(--sp-4);
  min-height: var(--row-height);
  padding: var(--sp-2) var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
}

.row:last-child {
  border-bottom: none;
}

.message {
  display: flex;
  gap: var(--sp-3);
  color: var(--color-text-muted);
}

.container {
  transition: background-color var(--dur-slow) var(--ease-out);
}

.container.highlighted {
  background: var(--color-selection);
  box-shadow: inset var(--border-width-accent) 0 0 var(--color-action);
}

.name {
  overflow: hidden;
  font-size: var(--fs-data);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.metric {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
}

.bar {
  position: relative;
  flex: 1;
  height: var(--sp-1);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border-strong);
}

.bar.unlimited {
  border-style: dashed;
}

.fill {
  position: absolute;
  inset: 0 auto 0 0;
  background: var(--color-text-muted);
}

.value {
  font-size: var(--fs-data);
  white-space: nowrap;
}

.limit {
  color: var(--color-text-muted);
}

.skeleton .block {
  display: block;
  height: var(--sp-2);
  background: var(--color-surface-hover);
}
</style>
