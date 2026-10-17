<script setup lang="ts">
/**
 * Indicateur discret de la navigation, hors de l'écran Télémétrie :
 * procédures en cours (forme « En transition » et chrono T+ de la plus
 * récente), ou à défaut le nombre de flux ouverts. Mène à l'onglet
 * correspondant. En fenêtre étroite, la forme seule, avec une infobulle.
 */
import { computed } from "vue";
import { useRoute } from "vue-router";
import { formatClock, useElapsed } from "../../composables/useElapsed";
import { useTelemetryStore } from "../../stores/telemetry";
import AppTooltip from "../ui/AppTooltip.vue";
import StatusIcon from "../ui/StatusIcon.vue";

defineProps<{ narrow: boolean }>();

const route = useRoute();
const telemetry = useTelemetryStore();

/** Procédure en cours la plus récente, sinon le premier onglet. */
const target = computed(() => {
  const running = telemetry.runningScripts;
  return running[running.length - 1] ?? telemetry.tabs[0] ?? null;
});

const runningCount = computed(() => telemetry.runningScripts.length);

const since = computed(() =>
  runningCount.value > 0 ? (target.value?.session.startedAt ?? null) : null,
);
const elapsed = useElapsed(since, { clock: () => Date.now(), tickMs: 1_000 });

const text = computed(() => {
  const count = runningCount.value;
  if (count > 0) {
    return `${count} ${count > 1 ? "procédures en cours" : "procédure en cours"}`;
  }
  const tabs = telemetry.tabs.length;
  return `Télémétrie · ${tabs} flux`;
});

const visible = computed(
  () => route.name !== "telemetry" && target.value !== null,
);
</script>

<template>
  <AppTooltip
    v-if="visible && target"
    :text="elapsed !== null ? `${text} · T+${formatClock(elapsed)}` : text"
    side="right"
    :disabled="!narrow"
  >
    <RouterLink
      :to="{ name: 'telemetry', params: { tab: target.id } }"
      class="indicator"
      :class="{ running: runningCount > 0, narrow }"
    >
      <StatusIcon class="shape" :name="runningCount > 0 ? 'half' : 'disc'" />
      <span :class="{ 'visually-hidden': narrow }">
        {{ text }}
        <span v-if="elapsed !== null" class="chrono">
          · T+{{ formatClock(elapsed) }}
        </span>
      </span>
    </RouterLink>
  </AppTooltip>
</template>

<style scoped>
.indicator {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-decoration: none;
  transition: color var(--dur-fast) var(--ease-out);
}

.indicator:hover {
  color: var(--color-text);
}

.indicator.narrow {
  justify-content: center;
}

.shape {
  color: var(--color-success);
}

.running .shape {
  color: var(--color-warning);
}

.chrono {
  white-space: nowrap;
}
</style>
