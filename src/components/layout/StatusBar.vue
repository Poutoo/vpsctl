<script setup lang="ts">
/** Barre d'état : système du serveur pendant une liaison, version de l'application. */
import { computed, onMounted, ref } from "vue";
import { appVersion } from "../../api/app";
import { useConnectionStore } from "../../stores/connection";

const connection = useConnectionStore();
const version = ref<string | null>(null);

onMounted(async () => {
  try {
    version.value = await appVersion();
  } catch {
    version.value = null;
  }
});

const serverSummary = computed(() => {
  const info = connection.serverInfo;
  if (!connection.isConnected || !info) return null;
  return [
    info.os,
    info.docker && `Docker ${info.docker}`,
    info.systemd && `systemd ${info.systemd}`,
  ]
    .filter(Boolean)
    .join(" · ");
});
</script>

<template>
  <footer class="status-bar">
    <span class="summary">
      <template v-if="connection.isConnected">{{
        serverSummary ?? ""
      }}</template>
      <template v-else>
        Empreintes mémorisées dans les profils · connexion par clé uniquement
      </template>
    </span>
    <span v-if="version">Atlas {{ version }}</span>
  </footer>
</template>

<style scoped>
.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-4);
  padding: var(--sp-1) var(--sp-4);
  background: var(--color-bg);
  border-top: var(--border-width) solid var(--color-border);
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  white-space: nowrap;
}

.summary {
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
