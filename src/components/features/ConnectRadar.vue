<script setup lang="ts">
/**
 * Radar de l'écran de connexion : trois anneaux de latence (33, 67, 100 ms).
 * Pendant l'acquisition, un secteur balaie le cadran tant que dure la
 * connexion réelle ; en mouvement réduit, il reste statique. En AOS, un plot
 * marque le serveur à la distance de sa latence.
 *
 * Les libellés sont en HTML par-dessus le SVG pour garder les tailles de
 * texte des tokens quelle que soit la taille du cadran.
 */
import { computed } from "vue";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  sweeping: boolean;
  /** Plot du serveur en AOS : nom et latence mesurée. */
  target: { name: string; latencyMs: number | null } | null;
}>();

/** Rayon du cadran (100 ms) dans le repère du SVG, centré sur 0. */
const R = 100;
const VIEW = 110;
const RINGS = [
  { ms: 33, r: 33 },
  { ms: 67, r: 67 },
  { ms: 100, r: 100 },
];
/** Graduations tous les 10 degrés, plus longues tous les 30. */
const TICKS = Array.from({ length: 36 }, (_, i) => {
  const angle = (i * 10 * Math.PI) / 180;
  const inner = i % 3 === 0 ? R - 8 : R - 4;
  return {
    x1: Math.cos(angle) * inner,
    y1: Math.sin(angle) * inner,
    x2: Math.cos(angle) * (R + 4),
    y2: Math.sin(angle) * (R + 4),
  };
});

/** Secteur de 40 degrés, bord d'attaque vers le haut. */
const SWEEP_ANGLE = (40 * Math.PI) / 180;
const SECTOR = `M0 0 L0 ${-R} A${R} ${R} 0 0 0 ${-Math.sin(SWEEP_ANGLE) * R} ${-Math.cos(SWEEP_ANGLE) * R} Z`;

/** Position en % du cadran pour un point du repère SVG. */
function percent(value: number): string {
  return `${50 + (value / (2 * VIEW)) * 100}%`;
}

const ringLabels = RINGS.map((ring) => ({
  text: `${ring.ms} ms`,
  top: percent(-ring.r),
}));

/** Plot en haut à gauche du cadran, à la distance de la latence. */
const blip = computed(() => {
  if (!props.target) return null;
  const ms = props.target.latencyMs;
  const r = Math.min(R, Math.max(12, ms ?? 50));
  const angle = (-135 * Math.PI) / 180;
  return {
    left: percent(Math.cos(angle) * r),
    top: percent(Math.sin(angle) * r),
    label: ms != null ? `${props.target.name} · ${ms} ms` : props.target.name,
  };
});
</script>

<template>
  <div class="radar" aria-hidden="true">
    <svg class="dial" :viewBox="`${-VIEW} ${-VIEW} ${2 * VIEW} ${2 * VIEW}`">
      <circle class="face" :r="R" />
      <circle v-for="ring in RINGS" :key="ring.ms" class="ring" :r="ring.r" />
      <path class="cross" :d="`M${-R - 8} 0H${R + 8}M0 ${-R - 8}V${R + 8}`" />
      <line
        v-for="(tick, index) in TICKS"
        :key="index"
        class="tick"
        v-bind="tick"
      />
      <circle class="center" r="1.6" />
    </svg>

    <svg
      v-if="sweeping"
      class="sweep"
      :viewBox="`${-VIEW} ${-VIEW} ${2 * VIEW} ${2 * VIEW}`"
    >
      <path class="sector" :d="SECTOR" />
      <path class="edge" :d="`M0 0V${-R}`" />
    </svg>

    <span
      v-for="label in ringLabels"
      :key="label.text"
      class="ring-label"
      :style="{ top: label.top }"
      >{{ label.text }}</span
    >

    <Transition name="blip" appear>
      <span
        v-if="blip"
        class="blip"
        :style="{ left: blip.left, top: blip.top }"
      >
        <span class="marker"><StatusIcon name="disc" /></span>
        <span class="blip-label">{{ blip.label }}</span>
      </span>
    </Transition>
  </div>
</template>

<style scoped>
.radar {
  position: relative;
  aspect-ratio: 1;
  height: 100%;
  max-width: 100%;
}

.dial,
.sweep {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  overflow: visible;
}

.face {
  fill: var(--color-surface);
  stroke: var(--color-border-strong);
  stroke-width: var(--border-width);
  vector-effect: non-scaling-stroke;
}

.ring,
.tick {
  fill: none;
  stroke: var(--color-border-strong);
  stroke-width: var(--border-width);
  vector-effect: non-scaling-stroke;
}

.cross {
  fill: none;
  stroke: var(--color-border);
  stroke-width: var(--border-width);
  stroke-dasharray: var(--sp-1);
  vector-effect: non-scaling-stroke;
}

.center {
  fill: var(--color-text-muted);
}

/* Balayage : rotation (transform) pendant la connexion réelle. */
.sweep {
  animation: sweep var(--dur-sweep) linear infinite;
}

.sector {
  fill: var(--color-selection);
}

.edge {
  stroke: var(--color-action);
  stroke-width: var(--border-width);
  vector-effect: non-scaling-stroke;
}

@keyframes sweep {
  to {
    transform: rotate(1turn);
  }
}

/* Mouvement réduit : secteur statique. */
@media (prefers-reduced-motion: reduce) {
  .sweep {
    animation: none;
    transform: rotate(-30deg);
  }
}

.ring-label {
  position: absolute;
  left: 50%;
  padding: 0 var(--sp-1);
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  transform: translateY(-100%);
}

.blip {
  position: absolute;
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  transform: translate(calc(-1 * var(--sp-3)), -50%);
}

.marker {
  display: inline-flex;
  padding: var(--sp-1);
  background: var(--color-surface);
  border: var(--border-width) solid var(--color-success);
  color: var(--color-success);
  font-size: var(--fs-sm);
}

.blip-label {
  padding: var(--sp-1) var(--sp-2);
  background: var(--color-surface);
  font-family: var(--font-mono);
  font-size: var(--fs-xs);
  white-space: nowrap;
}

.blip-enter-active {
  transition: opacity var(--dur-slow) var(--ease-out);
}

.blip-enter-from {
  opacity: 0;
}
</style>
