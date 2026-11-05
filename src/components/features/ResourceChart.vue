<script setup lang="ts">
/**
 * Courbe des 30 dernières minutes en SVG (tracés de `resourceChart.ts`) :
 * séries en trait plein, tireté ou pointillé, surface sous la première,
 * seuils, zones hachurées « sans relevé » (pertes de liaison). Le cadre
 * suit la taille réelle de l'élément : les traits et les hachures ne sont
 * jamais déformés.
 */
import { computed, onBeforeUnmount, onMounted, ref, useId } from "vue";
import {
  areaPath,
  frameAt,
  linePath,
  spanRects,
  yOf,
  type Point,
  type Span,
} from "../../resourceChart";

export interface ChartLine {
  points: readonly Point[];
  style: "solid" | "dashed" | "dotted";
  /** Surface sous la ligne. */
  area?: boolean;
}

export interface ChartThreshold {
  value: number;
  level: "attention" | "critical";
}

const props = withDefaults(
  defineProps<{
    lines: readonly ChartLine[];
    max: number;
    /** Étiquette du sommet de l'échelle (« 100 % »). */
    maxLabel: string;
    now: number;
    /** Écart au-delà duquel la ligne est interrompue. */
    maxGapMs: number;
    gaps?: readonly Span[];
    thresholds?: readonly ChartThreshold[];
    /** Message au centre, à la place des courbes. */
    message?: string | null;
    /** Mesure indisponible : cadre hachuré. */
    unavailable?: boolean;
  }>(),
  { gaps: () => [], thresholds: () => [], message: null, unavailable: false },
);

const HEIGHT = 64;
const host = ref<HTMLElement | null>(null);
const width = ref(240);
let observer: ResizeObserver | null = null;

onMounted(() => {
  if (!host.value) return;
  width.value = Math.max(1, Math.round(host.value.clientWidth));
  observer = new ResizeObserver(([entry]) => {
    width.value = Math.max(1, Math.round(entry.contentRect.width));
  });
  observer.observe(host.value);
});
onBeforeUnmount(() => observer?.disconnect());

const hatchId = `hatch-${useId()}`;
const frame = computed(() =>
  frameAt(props.now, width.value, HEIGHT, props.max),
);
const drawn = computed(() =>
  props.message || props.unavailable
    ? []
    : props.lines.map((line) => ({
        style: line.style,
        line: linePath(line.points, frame.value, props.maxGapMs),
        area: line.area
          ? areaPath(line.points, frame.value, props.maxGapMs)
          : "",
      })),
);
const thresholdLines = computed(() =>
  props.thresholds
    .filter((t) => t.value <= props.max)
    .map((t) => ({ level: t.level, y: yOf(t.value, frame.value) })),
);
const gapRects = computed(() =>
  props.unavailable
    ? [{ x: 0, width: width.value }]
    : spanRects(props.gaps, frame.value),
);
/** Étiquette « sans relevé » au centre de la dernière zone assez large. */
const gapLabel = computed(() => {
  if (props.unavailable || props.message) return null;
  const rect = [...gapRects.value].reverse().find((r) => r.width >= 36);
  return rect
    ? { left: `${((rect.x + rect.width / 2) / width.value) * 100}%` }
    : null;
});
</script>

<template>
  <div class="chart">
    <div ref="host" class="plot">
      <svg
        class="svg"
        :viewBox="`0 0 ${width} ${HEIGHT}`"
        :width="width"
        :height="HEIGHT"
        aria-hidden="true"
        focusable="false"
      >
        <defs>
          <pattern
            :id="hatchId"
            width="6"
            height="6"
            patternUnits="userSpaceOnUse"
            patternTransform="rotate(45)"
          >
            <line class="hatch-line" x1="0" y1="0" x2="0" y2="6" />
          </pattern>
        </defs>
        <line class="axis" x1="0.5" y1="0" x2="0.5" :y2="HEIGHT" />
        <line
          class="axis"
          x1="0"
          :y1="HEIGHT - 0.5"
          :x2="width"
          :y2="HEIGHT - 0.5"
        />
        <line class="guide" x1="0" y1="0.5" :x2="width" y2="0.5" />
        <rect
          v-for="(rect, index) in gapRects"
          :key="index"
          class="gap"
          :x="rect.x"
          y="0"
          :width="rect.width"
          :height="HEIGHT"
          :fill="`url(#${hatchId})`"
          data-testid="chart-gap"
        />
        <template v-for="(line, index) in drawn" :key="index">
          <path v-if="line.area" class="area" :d="line.area" />
          <path class="line" :class="line.style" :d="line.line" />
        </template>
        <line
          v-for="(t, index) in thresholdLines"
          :key="`t${index}`"
          class="threshold"
          :class="t.level"
          x1="0"
          :y1="t.y"
          :x2="width"
          :y2="t.y"
        />
      </svg>
      <span class="max mono">{{ maxLabel }}</span>
      <span v-if="gapLabel" class="gap-label mono" :style="gapLabel"
        >sans relevé</span
      >
      <p v-if="message" class="message" data-testid="chart-message">
        {{ message }}
      </p>
    </div>
    <div class="ticks mono" aria-hidden="true">
      <span>−30 min</span><span>−15</span><span>maintenant</span>
    </div>
  </div>
</template>

<style scoped>
.chart {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
}

.plot {
  position: relative;
}

.svg {
  display: block;
  width: 100%;
  overflow: visible;
}

.axis {
  stroke: var(--color-border-strong);
  stroke-width: var(--border-width);
}

.guide {
  stroke: var(--color-border-strong);
  stroke-width: var(--border-width);
  stroke-dasharray: 1 3;
}

.line {
  fill: none;
  stroke: var(--color-text);
  stroke-width: var(--border-width-accent);
  stroke-linejoin: round;
}

.line.dashed {
  stroke: var(--color-text-muted);
  stroke-dasharray: 5 3;
}

.line.dotted {
  stroke: var(--color-text-muted);
  stroke-dasharray: 1 2;
}

.area {
  fill: var(--color-surface-hover);
  stroke: none;
}

.threshold {
  stroke-width: var(--border-width);
}

.threshold.attention {
  stroke: var(--color-warning);
  stroke-dasharray: 6 4;
}

.threshold.critical {
  stroke: var(--color-danger);
  stroke-dasharray: 1 3;
}

.gap {
  stroke: none;
}

.hatch-line {
  stroke: var(--color-border-strong);
  stroke-width: var(--border-width);
}

.max {
  position: absolute;
  top: var(--sp-1);
  left: var(--sp-1);
  color: var(--color-text-subtle);
  font-size: var(--fs-xs);
}

.gap-label {
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  padding: 0 var(--sp-1);
  background: var(--color-surface);
  color: var(--color-text-muted);
  font-size: var(--fs-xs);
  text-align: center;
}

.message {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 var(--sp-3);
  color: var(--color-text-muted);
  font-size: var(--fs-sm);
  text-align: center;
}

.ticks {
  display: flex;
  justify-content: space-between;
  color: var(--color-text-subtle);
  font-size: var(--fs-xs);
}
</style>
