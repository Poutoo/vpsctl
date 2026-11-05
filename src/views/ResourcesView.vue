<script setup lang="ts">
/**
 * Ressources du serveur (écran IV, planche VII). Tant que l'écran est
 * affiché et la fenêtre visible, le backend fait un relevé complet toutes
 * les 5 s (`resources_watch`) ; sinon, seuls la mémoire et le disque sont
 * relevés chaque minute, pour le repère de navigation.
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { watchWindowVisibility } from "../api/window";
import type { ContainerUsage } from "../bindings/ContainerUsage";
import ContainerUsageTable from "../components/features/ContainerUsageTable.vue";
import DiskPanel from "../components/features/DiskPanel.vue";
import ResourceAlerts from "../components/features/ResourceAlerts.vue";
import ResourceCards from "../components/features/ResourceCards.vue";
import ThresholdPanel from "../components/features/ThresholdPanel.vue";
import PageHeader from "../components/layout/PageHeader.vue";
import AppIcon from "../components/ui/AppIcon.vue";
import StatusIcon from "../components/ui/StatusIcon.vue";
import { formatTime, resourceAlerts } from "../resourceText";
import { useConnectionStore } from "../stores/connection";
import { useResourcesStore } from "../stores/resources";

/** Avance de l'axe des courbes (« maintenant ») et de la zone sans relevé. */
const TICK_MS = 1_000;

const store = useResourcesStore();
const connection = useConnectionStore();

const now = ref(Date.now());
let ticker: ReturnType<typeof setInterval> | null = null;
let mounted = false;
let stopWatching: (() => void) | null = null;

const update = computed(() => store.update);
const alerts = computed(() => resourceAlerts(update.value));

onMounted(() => {
  mounted = true;
  ticker = setInterval(() => (now.value = Date.now()), TICK_MS);
  void store.ensureListening();
  void store.loadThresholds();
  void store.setOnScreen(true);
  void watchWindowVisibility((visible) => void store.setOnScreen(visible)).then(
    (watch) => {
      if (!mounted) {
        watch.stop();
        return;
      }
      stopWatching = watch.stop;
      if (!watch.visible) void store.setOnScreen(false);
    },
  );
});

onBeforeUnmount(() => {
  mounted = false;
  if (ticker) clearInterval(ticker);
  stopWatching?.();
  void store.setOnScreen(false);
});

/** « Voir <conteneur> » : défilement jusqu'à sa ligne, surlignée. */
async function show(container: ContainerUsage) {
  store.highlight(container.id);
  await nextTick();
  document
    .querySelector(`[data-container-id="${container.id}"]`)
    ?.scrollIntoView({ behavior: "smooth", block: "center" });
}
</script>

<template>
  <div class="resources">
    <PageHeader section="resources" title="Ressources du serveur">
      <p
        class="survey"
        :class="{ frozen: connection.lost }"
        data-testid="survey-status"
      >
        <template v-if="connection.lost && update">
          <StatusIcon name="triangle" />
          <span
            >Valeurs figées · dernier relevé {{ formatTime(update.at) }}</span
          >
        </template>
        <template v-else-if="update">
          <AppIcon name="clock" />
          <span>Relevé {{ formatTime(update.at) }} · toutes les 5 s</span>
        </template>
        <template v-else>
          <AppIcon name="clock" />
          <span>Première lecture en cours…</span>
        </template>
        <span class="caps memory">30 dernières minutes · en mémoire</span>
      </p>
    </PageHeader>

    <ResourceAlerts :alerts="alerts" :frozen="connection.lost" @show="show" />
    <ResourceCards :now="now" />
    <DiskPanel />
    <ContainerUsageTable />
    <ThresholdPanel />
  </div>
</template>

<style scoped>
.resources {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}

.survey {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2) var(--sp-4);
  padding: var(--sp-2) var(--sp-3);
  border: var(--border-width) solid var(--color-border-strong);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
}

.survey.frozen {
  border-color: var(--color-warning);
  color: var(--color-warning);
}

.memory {
  color: var(--color-text-muted);
}
</style>
