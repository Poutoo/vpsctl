<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import ContainerTable from "../components/features/ContainerTable.vue";
import DashboardSection from "../components/features/DashboardSection.vue";
import DockedConsole from "../components/features/DockedConsole.vue";
import ServiceTable from "../components/features/ServiceTable.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import AppButton from "../components/ui/AppButton.vue";
import AppIcon from "../components/ui/AppIcon.vue";
import FilterField from "../components/ui/FilterField.vue";
import { formatElapsed, useElapsed } from "../composables/useElapsed";
import { useDockedStream } from "../composables/useDockedStream";
import { useServiceActions } from "../composables/useServiceActions";
import { useVisiblePolling } from "../composables/useVisiblePolling";
import { timeLabel } from "../linkText";
import { useConnectionStore } from "../stores/connection";
import { useServicesStore } from "../stores/services";
import { containerLogTarget, serviceLogTarget } from "../stores/telemetry";

/** Rafraîchissement automatique, uniquement sur cet écran et fenêtre visible. */
const REFRESH_INTERVAL_MS = 5_000;

const store = useServicesStore();
const connection = useConnectionStore();
const telemetry = useDockedStream();
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

const content = ref<HTMLElement | null>(null);

/** Échap dans un filtre déjà vide : le focus revient au premier tableau. */
function focusTables() {
  content.value?.querySelector<HTMLElement>("table")?.focus();
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
    <div ref="content" class="content">
      <PageHeader section="dashboard" title="Tableau de bord">
        <FilterField
          v-model="store.filter"
          label="Filtrer les tableaux"
          @leave="focusTables"
        />
        <div v-if="connection.lost" class="refresh frozen">
          <span class="refreshed" aria-live="off">
            <AppIcon name="pause" />
            Figé · dernière actualisation
            {{ store.frozenAt ? timeLabel(store.frozenAt) : "inconnue" }}
          </span>
          <AppButton disabled>
            <AppIcon name="refresh" />
            Actualiser
          </AppButton>
        </div>
        <div v-else class="refresh">
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
        :shown="store.shownContainers.length"
        :frozen-at="store.frozenAt"
        empty-kicker="0 conteneur"
        empty-detail="docker ps -a"
        empty-title="Aucun conteneur"
        empty-text="Aucun conteneur, même arrêté, sur ce serveur."
      >
        <ContainerTable
          :containers="store.shownContainers"
          :total="store.containers.items.length"
          :query="store.filter"
          :loading="loading"
          @clear-filter="store.filter = ''"
          @logs="(c) => telemetry.showLogs(containerLogTarget(c.id, c.name))"
          @operate="onContainer"
        />
      </DashboardSection>

      <DashboardSection
        title="Services systemd"
        command="systemctl --user list-units"
        :section="store.services"
        :shown="store.shownServices.length"
        :frozen-at="store.frozenAt"
        empty-kicker="0 unité gérée"
        empty-detail="~/.config/systemd/user/"
        empty-title="Aucun service utilisateur"
        empty-text="Aucun service dans ~/.config/systemd/user/ sur ce serveur."
      >
        <template #actions>
          <AppButton
            size="compact"
            :to="{ name: 'service-create' }"
            :disabled="connection.lost"
          >
            Nouveau service
          </AppButton>
        </template>
        <template #default="{ loading }">
          <ServiceTable
            :services="store.shownServices"
            :total="store.services.items.length"
            :query="store.filter"
            :loading="loading"
            @clear-filter="store.filter = ''"
            @logs="(s) => telemetry.showLogs(serviceLogTarget(s.unit))"
            @operate="onService"
          />
        </template>
      </DashboardSection>
    </div>

    <!-- Panneau ancré : réduit la zone au-dessus de lui, ne la recouvre pas. -->
    <div v-if="telemetry.dock" class="dock">
      <DockedConsole :stream="telemetry.dock" />
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

.refresh.frozen {
  border-style: dashed;
}

.frozen .refreshed {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
}
</style>
