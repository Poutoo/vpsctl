<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import ContainerTable from "../components/features/ContainerTable.vue";
import DashboardSection from "../components/features/DashboardSection.vue";
import DockedConsole from "../components/features/DockedConsole.vue";
import ServiceTable from "../components/features/ServiceTable.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import AppButton from "../components/ui/AppButton.vue";
import AppIcon from "../components/ui/AppIcon.vue";
import { formatElapsed, useElapsed } from "../composables/useElapsed";
import { useServiceActions } from "../composables/useServiceActions";
import { useVisiblePolling } from "../composables/useVisiblePolling";
import {
  containerLogTarget,
  serviceLogTarget,
  useConsoleStore,
} from "../stores/console";
import { useServicesStore } from "../stores/services";

/** Rafraîchissement automatique, uniquement sur cet écran et fenêtre visible. */
const REFRESH_INTERVAL_MS = 5_000;

const store = useServicesStore();
const consoleStore = useConsoleStore();
const { onContainer, onService } = useServiceActions();
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

/** Au-delà, un rafraîchissement automatique est signalé comme lent. */
const SLOW_REFRESH_MS = 500;

// Chrono de l'interrogation en cours (pas de spinner).
const refreshStartedAt = ref<number | null>(null);
/** L'interrogation en cours a été demandée (ou rejointe) par le bouton. */
const manual = ref(false);
watch(
  () => store.refreshing,
  (refreshing) => {
    refreshStartedAt.value = refreshing ? performance.now() : null;
    if (!refreshing) manual.value = false;
  },
  { immediate: true },
);
const refreshElapsed = useElapsed(refreshStartedAt);

/**
 * Les rafraîchissements automatiques sont silencieux : « Interrogation · T+ »
 * n'apparaît qu'au premier chargement, sur actualisation manuelle, ou quand
 * un rafraîchissement automatique dépasse `SLOW_REFRESH_MS`.
 */
const probing = computed(() => {
  const elapsed = refreshElapsed.value;
  if (elapsed === null) return null;
  const shown =
    store.refreshedAt === null || manual.value || elapsed >= SLOW_REFRESH_MS;
  return shown ? elapsed : null;
});

function refreshNow() {
  manual.value = true;
  void trigger();
}

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
    <div class="content">
      <PageHeader section="dashboard" title="Tableau de bord">
        <div class="refresh">
          <span
            v-if="probing !== null"
            class="refreshed probing"
            aria-live="off"
          >
            Interrogation · {{ formatElapsed(probing, "ms") }}
          </span>
          <span v-else class="refreshed" aria-live="off">{{
            refreshedLabel
          }}</span>
          <AppButton @click="refreshNow">
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
        empty-kicker="0 conteneur"
        empty-detail="docker ps -a"
        empty-title="Aucun conteneur"
        empty-text="Aucun conteneur, même arrêté, sur ce serveur."
      >
        <ContainerTable
          :containers="store.containers.items"
          :loading="loading"
          @logs="(c) => consoleStore.open(containerLogTarget(c.id, c.name))"
          @operate="onContainer"
        />
      </DashboardSection>

      <DashboardSection
        v-slot="{ loading }"
        title="Services systemd"
        command="systemctl --user list-units"
        :section="store.services"
        empty-kicker="0 unité gérée"
        empty-detail="~/.config/systemd/user/"
        empty-title="Aucun service utilisateur"
        empty-text="Aucun service dans ~/.config/systemd/user/ sur ce serveur."
      >
        <ServiceTable
          :services="store.services.items"
          :loading="loading"
          @logs="(s) => consoleStore.open(serviceLogTarget(s.unit))"
          @operate="onService"
        />
      </DashboardSection>
    </div>

    <!-- Panneau ancré : réduit la zone au-dessus de lui, ne la recouvre pas. -->
    <div v-if="consoleStore.target" class="dock">
      <DockedConsole :target="consoleStore.target" />
    </div>
  </div>
</template>

<style scoped>
/* Écran plein cadre : la zone des tableaux défile au-dessus de la console. */
.dashboard {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.content {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-6);
  min-height: 0;
  overflow: auto;
  padding: var(--sp-6);
}

.dock {
  flex-shrink: 0;
  height: var(--console-dock-height);
  padding: 0 var(--sp-6) var(--sp-4);
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
