<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import ContainerTable from "../components/features/ContainerTable.vue";
import DashboardSection from "../components/features/DashboardSection.vue";
import ServiceTable from "../components/features/ServiceTable.vue";
import { useVisiblePolling } from "../composables/useVisiblePolling";
import { useServicesStore } from "../stores/services";

/** Rafraîchissement automatique, uniquement sur cet écran et fenêtre visible. */
const REFRESH_INTERVAL_MS = 5_000;

const store = useServicesStore();
const { trigger } = useVisiblePolling(
  () => store.refresh(),
  REFRESH_INTERVAL_MS,
);

// Horloge de l'indication « il y a N s ».
const now = ref(Date.now());
let clock: ReturnType<typeof setInterval> | null = null;
onMounted(() => {
  clock = setInterval(() => (now.value = Date.now()), 1_000);
});
onBeforeUnmount(() => {
  if (clock) clearInterval(clock);
});

const refreshedLabel = computed(() => {
  const at = store.refreshedAt;
  if (at === null) return "Actualisation…";
  const time = new Date(at).toLocaleTimeString("fr-FR", { hour12: false });
  const ago = Math.max(0, Math.floor((now.value - at) / 1000));
  return `Actualisé à ${time} · il y a ${ago} s`;
});
</script>

<template>
  <div class="dashboard">
    <header class="header">
      <h1>Tableau de bord</h1>
      <div class="refresh">
        <span class="refreshed" aria-live="off">{{ refreshedLabel }}</span>
        <button type="button" @click="trigger">Actualiser</button>
      </div>
    </header>

    <DashboardSection
      title="Conteneurs Docker"
      :section="store.containers"
      empty-text="Aucun conteneur sur ce serveur."
    >
      <ContainerTable :containers="store.containers.items" />
    </DashboardSection>

    <DashboardSection
      title="Services systemd"
      :section="store.services"
      empty-text="Aucun service dans ~/.config/systemd/user/."
    >
      <ServiceTable :services="store.services.items" />
    </DashboardSection>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  gap: var(--sp-5);
}

.header {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--sp-3);
}

.header h1 {
  margin: 0;
}

.refresh {
  display: flex;
  align-items: baseline;
  gap: var(--sp-3);
}

.refreshed {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}
</style>
