<script setup lang="ts">
import { computed, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import ConfirmDialog from "./components/ui/ConfirmDialog.vue";
import { useConnectionStore } from "./stores/connection";
import { useProfilesStore } from "./stores/profiles";

const connection = useConnectionStore();
const profiles = useProfilesStore();
const route = useRoute();
const router = useRouter();

void profiles.ensureLoaded().catch(() => undefined);

const linkedProfile = computed(() => profiles.byId(connection.profileId));

const serverSummary = computed(() => {
  const info = connection.serverInfo;
  if (!info) return null;
  return [
    info.os,
    info.docker && `Docker ${info.docker}`,
    info.systemd && `systemd ${info.systemd}`,
  ]
    .filter(Boolean)
    .join(" · ");
});

// Liaison perdue ou fermée : retour à l'écran de connexion.
watch(
  () => connection.isConnected,
  (connected) => {
    if (!connected && route.meta.requiresConnection) {
      void router.push({ name: "connect" });
    }
  },
);

async function disconnect() {
  await connection.disconnect();
}
</script>

<template>
  <!-- Navigation provisoire, sans design : la mise en page sera définie avec les maquettes. -->
  <nav v-if="connection.isConnected" class="nav">
    <RouterLink :to="{ name: 'dashboard' }">Tableau de bord</RouterLink>
    <RouterLink :to="{ name: 'files' }">Fichiers</RouterLink>
    <RouterLink :to="{ name: 'scripts' }">Scripts</RouterLink>
    <RouterLink :to="{ name: 'settings' }">Réglages</RouterLink>
    <span class="status">
      {{ linkedProfile?.name ?? "Liaison" }}
      <template v-if="connection.info?.latencyMs != null">
        · {{ connection.info.latencyMs }} ms
      </template>
      <template v-if="serverSummary"> · {{ serverSummary }}</template>
    </span>
    <button type="button" @click="disconnect">Déconnexion</button>
  </nav>
  <main class="main">
    <RouterView />
  </main>
  <ConfirmDialog />
</template>

<style scoped>
.nav {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  border-bottom: var(--border-width) solid var(--color-border);
}

.status {
  margin-left: auto;
  color: var(--color-text-muted);
  font-size: var(--font-size-sm);
}

.main {
  padding: var(--space-4);
}
</style>
