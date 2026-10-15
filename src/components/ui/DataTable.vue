<script setup lang="ts">
/**
 * Tableau de données : lignes de hauteur `--row-height`, données en
 * monospace. Une cellule `.truncate` coupe son texte avec des points de
 * suspension (le texte complet doit être donné en infobulle).
 */
</script>

<template>
  <table class="table">
    <slot />
  </table>
</template>

<style scoped>
.table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
  font-family: var(--font-mono);
  font-size: var(--fs-data);
}

.table :deep(th) {
  padding: var(--sp-2) var(--sp-4);
  color: var(--color-text-muted);
  font-family: var(--font-ui);
  font-size: var(--fs-sm);
  font-weight: 500;
  text-align: left;
  border-bottom: var(--border-width) solid var(--color-border);
}

.table :deep(td) {
  height: var(--row-height);
  padding: 0 var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
  vertical-align: middle;
}

.table :deep(tbody tr:last-child td) {
  border-bottom: none;
}

.table :deep(.truncate) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.table :deep(.muted) {
  color: var(--color-text-muted);
}
</style>
