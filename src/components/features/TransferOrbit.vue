<script setup lang="ts">
/**
 * Orbite de transfert : arc entre la machine locale (carré) et le serveur
 * (cercle), parcouru dans le sens du transfert. Le point suit l'avancement
 * réel (`transform`, `--dur-base` linéaire entre deux mesures) ; le trajet
 * parcouru est tracé plein, le reste en pointillé. Terminé : coche sur le
 * nœud d'arrivée. Erreur : croix dans le point, figé à la position de la
 * perte. Décorative : l'état est donné en texte à côté.
 */
import { computed } from "vue";
import type { TransferDirection, TransferStatus } from "../../stores/transfers";

const props = defineProps<{
  direction: TransferDirection;
  status: TransferStatus;
  /** Avancement, de 0 à 1. */
  progress: number;
  /** Nom du serveur, sous son nœud. */
  server: string;
}>();

// Géométrie de l'arc, en unités du `viewBox` (180 × 40).
const LOCAL = { x: 12, y: 30 };
const REMOTE = { x: 168, y: 30 };
const APEX = { x: 90, y: -10 };

const outbound = computed(() => props.direction === "upload");
const from = computed(() => (outbound.value ? LOCAL : REMOTE));
const to = computed(() => (outbound.value ? REMOTE : LOCAL));

/** Arc tracé dans le sens du transfert : le trajet parcouru part du début. */
const arc = computed(
  () =>
    `M${from.value.x} ${from.value.y} Q${APEX.x} ${APEX.y} ${to.value.x} ${to.value.y}`,
);

const travelled = computed(() =>
  props.status === "done" ? 1 : Math.min(Math.max(props.progress, 0), 1),
);

/** Position du point sur la courbe de Bézier quadratique. */
const point = computed(() => {
  const t = travelled.value;
  const u = 1 - t;
  return {
    x: u * u * from.value.x + 2 * u * t * APEX.x + t * t * to.value.x,
    y: u * u * from.value.y + 2 * u * t * APEX.y + t * t * to.value.y,
  };
});

const showDot = computed(
  () =>
    props.status === "active" ||
    props.status === "error" ||
    props.status === "cancelled",
);
</script>

<template>
  <div class="orbit" :class="status" aria-hidden="true">
    <svg class="arc" viewBox="0 0 180 40">
      <path class="track" :d="arc" />
      <path
        v-if="status !== 'queued'"
        class="travel"
        :d="arc"
        pathLength="1"
        :stroke-dasharray="`${travelled} 1`"
      />
      <rect
        class="node"
        :class="{ arrived: status === 'done' && !outbound }"
        :x="LOCAL.x - 8"
        :y="LOCAL.y - 4"
        width="8"
        height="8"
      />
      <circle
        class="node"
        :class="{ arrived: status === 'done' && outbound }"
        :cx="REMOTE.x + 4"
        :cy="REMOTE.y"
        r="4"
      />
      <path
        v-if="status === 'done'"
        class="check"
        :d="`M${to.x - 4} 14l3 3 5-6`"
      />
      <g
        v-if="showDot"
        class="dot"
        :style="{ transform: `translate(${point.x}px, ${point.y}px)` }"
      >
        <circle r="3.5" />
        <path
          v-if="status === 'error'"
          class="cross"
          d="M-2 -2l4 4M2 -2l-4 4"
        />
      </g>
    </svg>
    <div class="caps labels">
      <span>Local</span>
      <span class="verbatim server">{{ server }}</span>
    </div>
  </div>
</template>

<style scoped>
.orbit {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  width: var(--orbit-width);
  color: var(--color-text-subtle);
}

.orbit.active {
  color: var(--color-warning);
}

.orbit.done {
  color: var(--color-success);
}

.orbit.error {
  color: var(--color-danger);
}

.arc {
  width: var(--orbit-width);
  height: var(--orbit-height);
  overflow: visible;
  fill: none;
  stroke-linecap: round;
  stroke-linejoin: round;
}

/* Trajet restant : pointillé neutre. */
.track {
  stroke: var(--color-border-strong);
  stroke-width: 1.5;
  stroke-dasharray: 1 4;
}

/* Trajet parcouru : plein, à la couleur de l'état. */
.travel {
  stroke: currentColor;
  stroke-width: 1.5;
}

.node {
  stroke: var(--color-text-muted);
  stroke-width: 1.5;
  fill: var(--color-surface);
}

.node.arrived {
  stroke: currentColor;
  fill: currentColor;
}

.check {
  stroke: currentColor;
  stroke-width: 1.5;
}

.dot {
  transition: transform var(--dur-base) linear;
}

.dot circle {
  fill: currentColor;
  stroke: var(--color-surface);
  stroke-width: 1;
}

.cancelled .dot circle {
  fill: var(--color-surface);
  stroke: currentColor;
}

.cross {
  stroke: var(--color-on-danger);
  stroke-width: 1.2;
}

.labels {
  display: flex;
  justify-content: space-between;
  gap: var(--sp-2);
  color: var(--color-text-muted);
}

.server {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
