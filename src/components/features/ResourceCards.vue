<script setup lang="ts">
/**
 * Cartes CPU, mémoire, charge et réseau (planche VII). Première lecture :
 * squelette ; CPU et réseau, mesurés par différence, annoncent « relevé 1
 * sur 2 ». Après un rétablissement, ils restent figés sur leur dernière
 * valeur jusqu'au deuxième relevé. Pendant un LOS, toutes les valeurs sont
 * figées avec l'heure du dernier relevé.
 */
import { computed } from "vue";
import type { Level } from "../../bindings/Level";
import type { Measure } from "../../bindings/Measure";
import { formatBytes } from "../../format";
import { niceMax, peak, type Point } from "../../resourceChart";
import {
  formatLoad,
  formatLoadThreshold,
  formatPercent,
  formatRate,
  formatTime,
  interfacesLabel,
} from "../../resourceText";
import { useConnectionStore } from "../../stores/connection";
import { useResourcesStore } from "../../stores/resources";
import AppIcon from "../ui/AppIcon.vue";
import ResourceCard from "./ResourceCard.vue";
import ResourceChart from "./ResourceChart.vue";
import ResourceState from "./ResourceState.vue";

const props = defineProps<{ now: number }>();

const store = useResourcesStore();
const connection = useConnectionStore();

/** Ligne interrompue au-delà de 3 relevés manqués ; 90 s pour la mémoire, relevée chaque minute hors de l'écran. */
const GAP_FOREGROUND_MS = 15_000;
const GAP_MEMORY_MS = 90_000;
const FIRST_READING = "Première lecture en cours · relevé 1 sur 2";

const update = computed(() => store.update);
const series = computed(() => store.series);
const thresholds = computed(
  () => store.thresholds ?? update.value?.thresholds ?? null,
);
const frozenAt = computed(() =>
  connection.lost && update.value ? formatTime(update.value.at) : null,
);
const cores = computed(() => update.value?.cores ?? null);
const from = computed(() => props.now - 30 * 60_000);

type Mode = "value" | "frozen" | "skeleton" | "unavailable";

/**
 * Affichage d'une mesure. Par différence (`points` fourni) : en attente
 * après un rétablissement, la dernière valeur reste figée.
 */
function modeOf(
  measure: Measure<unknown> | undefined,
  points?: readonly Point[],
): Mode {
  if (!measure) return "skeleton";
  if (measure.status === "ok") return "value";
  if (measure.status === "unavailable") return "unavailable";
  if (points && store.resumed && points.length > 0) return "frozen";
  return "skeleton";
}

function reasonOf(measure: Measure<unknown> | undefined): string {
  return measure?.status === "unavailable" ? measure.reason : "";
}

function last(points: readonly Point[]): Point | null {
  return points.length > 0 ? points[points.length - 1] : null;
}

function cardState(level: Level, measure: Measure<unknown> | undefined) {
  if (
    !measure ||
    measure.status === "pending" ||
    measure.status === "notRead"
  ) {
    return "reading" as const;
  }
  if (measure.status === "unavailable") return "unavailable" as const;
  return level;
}

// --- CPU -------------------------------------------------------------------

const cpuMode = computed(() => modeOf(update.value?.cpu, series.value.cpu));
const cpuValue = computed(() => {
  const measure = update.value?.cpu;
  if (measure?.status === "ok") return measure.value;
  return last(series.value.cpu)?.value ?? null;
});
const cpuPeak = computed(() => peak(series.value.cpu, from.value));

// --- Mémoire ---------------------------------------------------------------

const memory = computed(() => {
  const measure = update.value?.memory;
  return measure?.status === "ok" ? measure.value : null;
});
const memoryMode = computed(() => modeOf(update.value?.memory));
const memoryLevel = computed(() => update.value?.levels.memory ?? "nominal");
const swapText = computed(() => {
  const swap = memory.value?.swap;
  if (swap === undefined) return "";
  if (swap === null) return "Swap : non mesurable";
  if (swap.total === 0) return "Swap : aucun";
  return `Swap ${formatBytes(swap.used)} sur ${formatBytes(swap.total)}`;
});

// --- Charge ----------------------------------------------------------------

const load = computed(() => {
  const measure = update.value?.load;
  return measure?.status === "ok" ? measure.value : null;
});
const loadMode = computed(() => modeOf(update.value?.load));
const loadLevel = computed(() => update.value?.levels.load ?? "nominal");
const loadMax = computed(() =>
  niceMax(
    [
      ...series.value.load5.map((p) => p.value),
      ...series.value.load1.map((p) => p.value),
      (thresholds.value?.load.critical ?? 2) * 1.1,
    ],
    1,
  ),
);

// --- Réseau ----------------------------------------------------------------

const networkMode = computed(() =>
  modeOf(update.value?.network, series.value.received),
);
const network = computed(() => {
  const measure = update.value?.network;
  if (measure?.status === "ok") return measure.value;
  return null;
});
const received = computed(
  () => network.value?.received ?? last(series.value.received)?.value ?? 0,
);
const sent = computed(
  () => network.value?.sent ?? last(series.value.sent)?.value ?? 0,
);
const lastInterfaces = computed(() => network.value?.interfaces ?? []);
const networkMax = computed(() =>
  niceMax(
    [...series.value.received, ...series.value.sent].map((p) => p.value),
    100 * 1024,
  ),
);
const frozenNetworkAt = computed(() => {
  const point = last(series.value.received);
  return point ? formatTime(point.at) : null;
});
const frozenCpuAt = computed(() => {
  const point = last(series.value.cpu);
  return point ? formatTime(point.at) : null;
});

function message(mode: Mode, points: readonly Point[]): string | null {
  if (mode === "unavailable") return "Aucune courbe";
  if (mode === "skeleton" && points.length === 0) return FIRST_READING;
  return null;
}
</script>

<template>
  <div class="cards">
    <!-- CPU -->
    <ResourceCard
      title="CPU"
      :skeleton="cpuMode === 'skeleton'"
      data-testid="card-cpu"
    >
      <template v-if="cpuMode === 'unavailable'" #state>
        <ResourceState state="unavailable" />
      </template>
      <template #value>
        <template v-if="cpuMode === 'unavailable'">
          <span class="muted">Indisponible</span>
        </template>
        <template v-else-if="cpuValue !== null">
          <span data-testid="cpu-value">{{ Math.round(cpuValue) }}</span>
          <span class="unit mono">%</span>
        </template>
      </template>
      <template #details>
        <span v-if="cpuMode === 'unavailable'" class="reason">
          {{ reasonOf(update?.cpu) }}. Les autres mesures restent à jour.
        </span>
        <template v-else>
          <span>
            {{ cores ?? "?" }} {{ (cores ?? 0) > 1 ? "cœurs" : "cœur" }}
            <template v-if="cpuPeak !== null">
              · pic {{ formatPercent(cpuPeak) }} sur 30 min</template
            >
          </span>
          <span v-if="frozenAt" class="frozen">Figé à {{ frozenAt }}</span>
          <span v-else-if="cpuMode === 'frozen'" class="frozen"
            >Figé à {{ frozenCpuAt }}</span
          >
        </template>
      </template>
      <template #chart>
        <ResourceChart
          :lines="[{ points: series.cpu, style: 'solid', area: true }]"
          :max="100"
          max-label="100 %"
          :now="now"
          :max-gap-ms="GAP_FOREGROUND_MS"
          :gaps="store.gaps"
          :message="message(cpuMode, series.cpu)"
          :unavailable="cpuMode === 'unavailable'"
        />
      </template>
      <template #legend>
        <span class="key"><span class="sample solid" />Utilisation</span>
      </template>
    </ResourceCard>

    <!-- Mémoire -->
    <ResourceCard
      title="Mémoire"
      :skeleton="memoryMode === 'skeleton'"
      data-testid="card-memory"
    >
      <template #state>
        <ResourceState :state="cardState(memoryLevel, update?.memory)" />
      </template>
      <template #value>
        <span v-if="memoryMode === 'unavailable'" class="muted"
          >Indisponible</span
        >
        <template v-else-if="memory">
          <span :class="memoryLevel" data-testid="memory-value">{{
            formatBytes(memory.used).split(" ")[0]
          }}</span>
          <span class="unit mono">{{
            formatBytes(memory.used).split(" ")[1]
          }}</span>
        </template>
      </template>
      <template #details>
        <span v-if="memoryMode === 'unavailable'" class="reason">
          {{ reasonOf(update?.memory) }}. Les autres mesures restent à jour.
        </span>
        <template v-else-if="memory">
          <span
            >sur {{ formatBytes(memory.total) }} ·
            {{ formatPercent(memory.percent) }}</span
          >
          <span>{{ swapText }}</span>
          <span v-if="frozenAt" class="frozen">Figé à {{ frozenAt }}</span>
        </template>
      </template>
      <template #chart>
        <ResourceChart
          :lines="[{ points: series.memory, style: 'solid', area: true }]"
          :max="100"
          max-label="100 %"
          :now="now"
          :max-gap-ms="GAP_MEMORY_MS"
          :gaps="store.gaps"
          :thresholds="
            thresholds
              ? [
                  { value: thresholds.memory.attention, level: 'attention' },
                  { value: thresholds.memory.critical, level: 'critical' },
                ]
              : []
          "
          :message="message(memoryMode, series.memory)"
          :unavailable="memoryMode === 'unavailable'"
        />
      </template>
      <template #legend>
        <span class="key"><span class="sample solid" />Utilisée</span>
        <template v-if="thresholds">
          <span class="key"
            ><span class="sample attention" />seuil
            {{ thresholds.memory.attention }} %</span
          >
          <span class="key"
            ><span class="sample critical" />seuil
            {{ thresholds.memory.critical }} %</span
          >
        </template>
      </template>
    </ResourceCard>

    <!-- Charge -->
    <ResourceCard
      title="Charge moyenne"
      :skeleton="loadMode === 'skeleton'"
      data-testid="card-load"
    >
      <template #state>
        <ResourceState :state="cardState(loadLevel, update?.load)" />
      </template>
      <template #value>
        <span v-if="loadMode === 'unavailable'" class="muted"
          >Indisponible</span
        >
        <template v-else-if="load">
          <span :class="loadLevel" data-testid="load-value">{{
            formatLoad(load.perCore[1])
          }}</span>
          <span class="unit mono">par cœur · 5 min</span>
        </template>
      </template>
      <template #details>
        <span v-if="loadMode === 'unavailable'" class="reason">
          {{ reasonOf(update?.load) }}. Les autres mesures restent à jour.
        </span>
        <template v-else-if="load">
          <span
            >1 min {{ formatLoad(load.perCore[0]) }} · 15 min
            {{ formatLoad(load.perCore[2]) }}</span
          >
          <span v-if="thresholds"
            >{{ cores }} {{ (cores ?? 0) > 1 ? "cœurs" : "cœur" }} · seuils
            {{ formatLoadThreshold(thresholds.load.attention) }} et
            {{ formatLoadThreshold(thresholds.load.critical) }} · brute
            {{ formatLoadThreshold(load.raw[1]) }}</span
          >
          <span v-if="frozenAt" class="frozen">Figé à {{ frozenAt }}</span>
        </template>
      </template>
      <template #chart>
        <ResourceChart
          :lines="[
            { points: series.load5, style: 'solid', area: true },
            { points: series.load1, style: 'dotted' },
          ]"
          :max="loadMax"
          :max-label="formatLoadThreshold(loadMax)"
          :now="now"
          :max-gap-ms="GAP_FOREGROUND_MS"
          :gaps="store.gaps"
          :thresholds="
            thresholds
              ? [
                  { value: thresholds.load.attention, level: 'attention' },
                  { value: thresholds.load.critical, level: 'critical' },
                ]
              : []
          "
          :message="message(loadMode, series.load5)"
          :unavailable="loadMode === 'unavailable'"
        />
      </template>
      <template #legend>
        <span class="key"><span class="sample solid" />5 min</span>
        <span class="key"><span class="sample dotted" />1 min</span>
        <template v-if="thresholds">
          <span class="key"
            ><span class="sample attention" />seuil
            {{ formatLoadThreshold(thresholds.load.attention) }}</span
          >
          <span class="key"
            ><span class="sample critical" />seuil
            {{ formatLoadThreshold(thresholds.load.critical) }}</span
          >
        </template>
      </template>
    </ResourceCard>

    <!-- Réseau -->
    <ResourceCard
      title="Réseau"
      :skeleton="networkMode === 'skeleton'"
      data-testid="card-network"
    >
      <template v-if="networkMode === 'unavailable'" #state>
        <ResourceState state="unavailable" />
      </template>
      <template #value>
        <span v-if="networkMode === 'unavailable'" class="muted"
          >Indisponible</span
        >
        <template v-else>
          <span class="rate" data-testid="network-received">
            <AppIcon name="arrow-down" class="arrow" />
            <span class="visually-hidden">Reçu</span>
            {{ formatRate(received).split(" ")[0] }}
            <span class="unit mono">{{
              formatRate(received).split(" ")[1]
            }}</span>
          </span>
          <span class="rate" data-testid="network-sent">
            <AppIcon name="arrow-up" class="arrow" />
            <span class="visually-hidden">Émis</span>
            {{ formatRate(sent).split(" ")[0] }}
            <span class="unit mono">{{ formatRate(sent).split(" ")[1] }}</span>
          </span>
        </template>
      </template>
      <template #details>
        <span v-if="networkMode === 'unavailable'" class="reason">
          {{ reasonOf(update?.network) }}. Les autres mesures restent à jour.
        </span>
        <template v-else>
          <span
            v-if="lastInterfaces.length > 0"
            data-testid="network-interfaces"
            >{{ interfacesLabel(lastInterfaces) }}</span
          >
          <span v-if="frozenAt" class="frozen">Figé à {{ frozenAt }}</span>
          <span v-else-if="networkMode === 'frozen'" class="frozen"
            >Figé à {{ frozenNetworkAt }}</span
          >
        </template>
      </template>
      <template #chart>
        <ResourceChart
          :lines="[
            { points: series.received, style: 'solid', area: true },
            { points: series.sent, style: 'dashed' },
          ]"
          :max="networkMax"
          :max-label="formatRate(networkMax)"
          :now="now"
          :max-gap-ms="GAP_FOREGROUND_MS"
          :gaps="store.gaps"
          :message="message(networkMode, series.received)"
          :unavailable="networkMode === 'unavailable'"
        />
      </template>
      <template #legend>
        <span class="key"><span class="sample solid" />Reçu</span>
        <span class="key"><span class="sample dashed" />Émis</span>
      </template>
    </ResourceCard>
  </div>
</template>

<style scoped>
.cards {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--sp-3);
}

@media (max-width: 1100px) {
  .cards {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

.unit {
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
}

.muted {
  color: var(--color-text-muted);
}

.attention {
  color: var(--color-warning);
}

.critical {
  color: var(--color-danger-text);
}

.rate {
  display: inline-flex;
  align-items: baseline;
  gap: var(--sp-1);
}

.arrow {
  font-size: var(--fs-md);
}

.frozen {
  color: var(--color-warning);
}

.reason {
  font-family: var(--font-ui);
}

.key {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
}

.sample {
  display: inline-block;
  width: var(--sp-4);
  border-top: var(--border-width-accent) solid var(--color-text);
}

.sample.dashed {
  border-top-style: dashed;
  border-top-color: var(--color-text-muted);
}

.sample.dotted {
  border-top-style: dotted;
  border-top-color: var(--color-text-muted);
}

.sample.attention {
  border-top: var(--border-width) dashed var(--color-warning);
}

.sample.critical {
  border-top: var(--border-width) dotted var(--color-danger);
}
</style>
