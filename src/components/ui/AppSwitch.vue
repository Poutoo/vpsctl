<script setup lang="ts">
/**
 * Interrupteur : état lisible sans la couleur (position du curseur, et
 * étiquette ACTIVÉ / DÉSACTIVÉ si `showState`). Le curseur glisse en
 * `--dur-fast` sur `transform`. Désactivé : contour pointillé.
 */
withDefaults(
  defineProps<{
    label: string;
    disabled?: boolean;
    /** Affiche ACTIVÉ / DÉSACTIVÉ sous le libellé. */
    showState?: boolean;
  }>(),
  { disabled: false, showState: false },
);

const checked = defineModel<boolean>({ required: true });
</script>

<template>
  <button
    type="button"
    role="switch"
    class="switch"
    :aria-checked="checked"
    :disabled="disabled"
    @click="checked = !checked"
  >
    <span class="track" :class="{ on: checked }" aria-hidden="true">
      <span class="knob" />
    </span>
    <span class="text">
      <span class="label">{{ label }}</span>
      <span v-if="showState" class="caps state">{{
        checked ? "Activé" : "Désactivé"
      }}</span>
    </span>
  </button>
</template>

<style scoped>
.switch {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  padding: 0;
  background: none;
  border: none;
  border-radius: var(--r-1);
  color: var(--color-text);
  font-size: var(--fs-md);
  font-weight: 600;
  cursor: pointer;
}

.track {
  position: relative;
  flex-shrink: 0;
  width: var(--switch-width);
  height: var(--switch-height);
  background: var(--color-field);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-1);
  transition:
    background-color var(--dur-fast) var(--ease-out),
    border-color var(--dur-fast) var(--ease-out);
}

.track.on {
  background: var(--color-action);
  border-color: var(--color-action);
}

.knob {
  position: absolute;
  top: calc(var(--sp-1) - var(--border-width));
  left: calc(var(--sp-1) - var(--border-width));
  width: calc(var(--switch-height) - 2 * var(--sp-1));
  height: calc(var(--switch-height) - 2 * var(--sp-1));
  background: var(--color-text-muted);
  border-radius: var(--r-1);
  transition: transform var(--dur-fast) var(--ease-out);
}

.on .knob {
  background: var(--color-on-action);
  transform: translateX(calc(var(--switch-width) - var(--switch-height)));
}

.text {
  display: inline-flex;
  flex-direction: column;
  align-items: flex-start;
  line-height: var(--lh-title);
}

.state {
  color: var(--color-text-muted);
}

.on + .text .state {
  color: var(--color-success);
}

.switch:disabled {
  cursor: not-allowed;
}

.switch:disabled .track {
  background: transparent;
  border-style: dashed;
}

.switch:disabled .label {
  color: var(--color-text-subtle);
}
</style>
