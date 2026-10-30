<script setup lang="ts">
/**
 * Curseur (planche VI) : champ `range` natif pour le clavier et les lecteurs
 * d'écran, piste et remplissage dessinés à part. Le remplissage suit le
 * centre du curseur : `taille du curseur / 2 + (piste − curseur) × valeur`.
 */
import { computed, useId } from "vue";

const props = withDefaults(
  defineProps<{
    label: string;
    /** Valeur lue à droite du libellé, ex. « 85 % ». */
    valueText: string;
    min?: number;
    max?: number;
    step?: number;
    /** Repères sous la piste, aux deux extrémités. */
    minHint?: string;
    maxHint?: string;
    disabled?: boolean;
  }>(),
  { min: 0, max: 100, step: 1, minHint: "", maxHint: "", disabled: false },
);

const value = defineModel<number>({ required: true });

const ratio = computed(() =>
  props.max === props.min
    ? 0
    : (value.value - props.min) / (props.max - props.min),
);

const id = useId();

function onInput(event: Event) {
  value.value = Number((event.target as HTMLInputElement).value);
}
</script>

<template>
  <div class="range" :class="{ disabled }">
    <div class="head">
      <label class="label" :for="id">{{ label }}</label>
      <span class="value mono" aria-hidden="true">{{ valueText }}</span>
    </div>
    <div class="control" :style="{ '--range-ratio': ratio }">
      <span class="track" aria-hidden="true"><span class="fill" /></span>
      <input
        :id="id"
        type="range"
        :min="min"
        :max="max"
        :step="step"
        :value="value"
        :disabled="disabled"
        :aria-valuetext="valueText"
        @input="onInput"
      />
    </div>
    <div v-if="minHint || maxHint" class="hints mono">
      <span>{{ minHint }}</span>
      <span>{{ maxHint }}</span>
    </div>
  </div>
</template>

<style scoped>
.range {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--sp-3);
}

.label {
  font-size: var(--fs-md);
  font-weight: 600;
}

.value {
  color: var(--color-action);
  font-size: var(--fs-sm);
}

.control {
  position: relative;
  height: var(--range-thumb-size);
}

.track {
  position: absolute;
  top: 50%;
  right: 0;
  left: 0;
  height: var(--range-track-height);
  overflow: hidden;
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
  transform: translateY(-50%);
}

.fill {
  display: block;
  width: calc(
    var(--range-thumb-size) / 2 + (100% - var(--range-thumb-size)) *
      var(--range-ratio)
  );
  height: 100%;
  background: var(--color-action);
}

input {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  margin: 0;
  background: transparent;
  cursor: pointer;
  appearance: none;
}

input:focus-visible {
  outline: none;
}

input::-webkit-slider-runnable-track {
  height: 100%;
  background: transparent;
}

input::-webkit-slider-thumb {
  width: var(--range-thumb-size);
  height: var(--range-thumb-size);
  background: var(--color-action);
  border: var(--border-width) solid var(--color-on-action);
  border-radius: 50%;
  appearance: none;
  transition: background-color var(--dur-fast) var(--ease-out);
}

input:hover::-webkit-slider-thumb {
  background: var(--color-action-hover);
}

input:focus-visible::-webkit-slider-thumb {
  outline: var(--focus-ring);
  outline-offset: var(--focus-offset);
}

.hints {
  display: flex;
  justify-content: space-between;
  gap: var(--sp-3);
  color: var(--color-text-muted);
  font-size: var(--fs-xs);
}

.disabled input {
  cursor: not-allowed;
}

.disabled .fill,
.disabled input::-webkit-slider-thumb {
  background: var(--color-text-subtle);
}
</style>
