<script setup lang="ts">
/**
 * Carte d'une mesure (planche VII) : en-tête (titre, état), valeur
 * principale, lignes de détail, courbe et légende. En première lecture, la
 * valeur et les détails sont remplacés par un squelette.
 */
defineProps<{
  title: string;
  /** Squelette à la place de la valeur et des détails. */
  skeleton?: boolean;
}>();
</script>

<template>
  <section class="card">
    <header class="head">
      <h2 class="caps title">{{ title }}</h2>
      <slot name="state" />
    </header>
    <div class="body">
      <div v-if="skeleton" class="skeleton" aria-hidden="true">
        <span class="block value-block" />
        <span class="block line-block" />
      </div>
      <template v-else>
        <div class="value"><slot name="value" /></div>
        <div class="details mono"><slot name="details" /></div>
      </template>
      <slot name="chart" />
      <div v-if="$slots.legend" class="legend mono"><slot name="legend" /></div>
    </div>
  </section>
</template>

<style scoped>
.card {
  display: flex;
  flex-direction: column;
  min-width: 0;
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-2);
  min-height: var(--row-height);
  padding: var(--sp-2) var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
}

.title {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  font-weight: 400;
}

.body {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-3);
  padding: var(--sp-4);
}

.value {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--sp-1) var(--sp-3);
  font-family: var(--font-title);
  font-size: var(--fs-2xl);
  line-height: var(--lh-title);
}

.details {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  min-height: calc(2 * var(--fs-md));
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}

.block {
  display: block;
  background: var(--color-surface-hover);
}

.value-block {
  width: 40%;
  height: var(--fs-2xl);
}

.line-block {
  width: 70%;
  height: var(--fs-sm);
}

.legend {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1) var(--sp-4);
  color: var(--color-text-muted);
  font-size: var(--fs-xs);
}
</style>
