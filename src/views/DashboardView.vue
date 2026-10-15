<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import ContainerTable from "../components/features/ContainerTable.vue";
import DashboardSection from "../components/features/DashboardSection.vue";
import ServiceTable from "../components/features/ServiceTable.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import AppButton from "../components/ui/AppButton.vue";
import AppIcon from "../components/ui/AppIcon.vue";
import { formatElapsed, useElapsed } from "../composables/useElapsed";
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

// Chrono de l'interrogation en cours (pas de spinner).
const refreshStartedAt = ref<number | null>(null);
watch(
  () => store.refreshing,
  (refreshing) => {
    refreshStartedAt.value = refreshing ? performance.now() : null;
  },
  { immediate: true },
);
const refreshElapsed = useElapsed(refreshStartedAt);

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
    <PageHeader section="dashboard" title="Tableau de bord">
      <div class="refresh">
        <span
          v-if="refreshElapsed !== null"
          class="refreshed probing"
          aria-live="off"
        >
          Interrogation · {{ formatElapsed(refreshElapsed, "ms") }}
        </span>
        <span v-else class="refreshed" aria-live="off">{{
          refreshedLabel
        }}</span>
        <AppButton @click="trigger">
          <AppIcon name="refresh" />
          Actualiser
        </AppButton>
      </div>
    </PageHeader>

    <DashboardSection
      v-slot="{ loading }"
      title="Conteneurs Docker"
      command="docker ps -a"
      :section="store.containers"
      empty-kicker="0 conteneur · docker ps -a"
      empty-title="Aucun conteneur"
      empty-text="Aucun conteneur, même arrêté, sur ce serveur."
    >
      <ContainerTable :containers="store.containers.items" :loading="loading" />
    </DashboardSection>

    <DashboardSection
      v-slot="{ loading }"
      title="Services systemd"
      command="systemctl --user list-units"
      :section="store.services"
      empty-kicker="0 unité gérée · ~/.config/systemd/user/"
      empty-title="Aucun service utilisateur"
      empty-text="Aucun service dans ~/.config/systemd/user/ sur ce serveur."
    >
      <ServiceTable :services="store.services.items" :loading="loading" />
    </DashboardSection>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  gap: var(--sp-6);
}

/* Bloc d'actualisation : surface opaque, posé sur l'image de fond. */
.refresh {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-1) var(--sp-1) var(--sp-1) var(--sp-3);
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-border);
}

.refreshed {
  min-width: 32ch;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.probing {
  color: var(--color-warning);
}
</style>
