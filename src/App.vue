<script setup lang="ts">
import { useConnectionStore } from "./stores/connection";

const connection = useConnectionStore();
</script>

<template>
  <!-- Navigation provisoire, sans design : la mise en page sera définie avec les maquettes. -->
  <nav v-if="connection.isConnected" class="nav">
    <RouterLink :to="{ name: 'dashboard' }">Dashboard</RouterLink>
    <RouterLink :to="{ name: 'files' }">Files</RouterLink>
    <RouterLink :to="{ name: 'scripts' }">Scripts</RouterLink>
    <RouterLink :to="{ name: 'settings' }">Settings</RouterLink>
  </nav>
  <main class="main">
    <RouterView />
  </main>
</template>

<style scoped>
.nav {
  display: flex;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  border-bottom: var(--border-width) solid var(--color-border);
}

.main {
  padding: var(--space-4);
}
</style>
