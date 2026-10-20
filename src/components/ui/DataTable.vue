<script setup lang="ts">
/**
 * Tableau de données sur surface opaque : lignes de hauteur `--row-height`,
 * données en monospace à `--fs-data`. Conventions pour le contenu :
 *
 * - cellule `.truncate` : texte coupé avec points de suspension (le texte
 *   complet doit être donné en infobulle) ;
 * - ligne `.failed` : fond teinté de rouge (l'état reste porté par sa forme) ;
 * - ligne `.placeholder` : registre tracé et vide pendant un chargement ;
 * - `<TransitionGroup name="row-in" tag="tbody" appear>` : fondu des lignes
 *   à leur apparition uniquement.
 *
 * Le tableau peut recevoir le focus par programme (Échap dans le filtre).
 */
</script>

<template>
  <table class="table" tabindex="-1">
    <slot />
  </table>
</template>

<style scoped>
.table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
  background: var(--color-surface);
  font-family: var(--font-mono);
  font-size: var(--fs-data);
}

.table :deep(th) {
  height: var(--row-height);
  padding: 0 var(--sp-4);
  background: var(--color-surface-raised);
  border-bottom: var(--border-width) solid var(--color-border);
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  font-weight: 400;
  letter-spacing: var(--tracking-caps);
  text-align: left;
  text-transform: uppercase;
  vertical-align: middle;
}

.table :deep(td) {
  height: var(--row-height);
  padding: 0 var(--sp-4);
  border-bottom: var(--border-width) solid var(--color-border);
  vertical-align: middle;
}

.table :deep(td:first-child) {
  font-weight: 500;
}

.table :deep(tbody tr:last-child td) {
  border-bottom: none;
}

.table :deep(tbody tr) {
  transition: background-color var(--dur-fast) var(--ease-out);
}

.table :deep(tbody tr.no-match-row:hover) {
  background: none;
}

.table :deep(tbody tr:hover) {
  background: var(--color-surface-hover);
}

.table :deep(tbody tr.failed) {
  background: var(--color-danger-tint);
}

.table :deep(tbody tr.placeholder td) {
  color: var(--color-text-subtle);
}

.table :deep(.truncate) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.table :deep(.muted) {
  color: var(--color-text-muted);
}

/* Apparition des données : fondu, joué une seule fois par ligne. */
.table :deep(.row-in-enter-active) {
  transition: opacity var(--dur-base) var(--ease-out);
}

.table :deep(.row-in-enter-from) {
  opacity: 0;
}
</style>
