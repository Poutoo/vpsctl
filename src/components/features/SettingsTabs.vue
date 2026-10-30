<script setup lang="ts">
/** Sélecteur d'onglet des Réglages (planche VI) : Profils ou Préférences. */
defineProps<{ current: "profiles" | "preferences" }>();
</script>

<template>
  <nav class="tabs" aria-label="Onglets des réglages">
    <RouterLink
      class="tab"
      :to="{ name: 'settings' }"
      :aria-current="current === 'profiles' ? 'page' : undefined"
    >
      Profils
    </RouterLink>
    <RouterLink
      class="tab"
      :to="{ name: 'settings', params: { tab: 'preferences' } }"
      :aria-current="current === 'preferences' ? 'page' : undefined"
    >
      Préférences
    </RouterLink>
  </nav>
</template>

<style scoped>
.tabs {
  display: inline-flex;
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
}

.tab {
  display: inline-flex;
  align-items: center;
  min-height: var(--row-height);
  padding: var(--sp-1) var(--sp-4);
  background: var(--color-surface);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
  font-weight: 600;
  text-decoration: none;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}

.tab + .tab {
  border-left: var(--border-width) solid var(--color-border-strong);
}

.tab:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.tab[aria-current="page"] {
  background: var(--color-selection);
  box-shadow: inset 0 0 0 var(--border-width) var(--color-action);
  color: var(--color-action);
}
</style>
