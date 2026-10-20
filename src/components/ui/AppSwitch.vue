<script setup lang="ts">
/**
 * Interrupteur (planche II) : état lisible sans la couleur (position du
 * curseur, et étiquette ACTIVÉ / DÉSACTIVÉ si `showState`). Le curseur glisse
 * en `--dur-fast` sur `transform`.
 *
 * - Désactivé : contour pointillé.
 * - En cours (`busyLabel`) : curseur à mi-course, contour pointillé, étiquette
 *   au participe (ACTIVATION, DÉSACTIVATION).
 * - Non modifiable (`locked`) : contour pointillé, cadenas et cause (MASQUÉ,
 *   STATIQUE), explication en infobulle.
 * - Indisponible (`unavailable`, ex. liaison perdue) : contour pointillé, la
 *   raison en infobulle.
 *
 * Sauf `disabled`, l'interrupteur inactif reste focalisable (`aria-disabled`)
 * pour que son infobulle s'affiche au survol comme au clavier.
 */
import { computed } from "vue";
import AppIcon from "./AppIcon.vue";
import AppTooltip from "./AppTooltip.vue";
import StatusIcon from "./StatusIcon.vue";

const props = withDefaults(
  defineProps<{
    label: string;
    disabled?: boolean;
    /** Affiche ACTIVÉ / DÉSACTIVÉ sous le libellé. */
    showState?: boolean;
    /** Libellé lu par les lecteurs d'écran seulement (cellule de tableau). */
    hideLabel?: boolean;
    /** Changement en cours : étiquette au participe. */
    busyLabel?: string | null;
    /** Non modifiable : cause affichée avec un cadenas. */
    locked?: string | null;
    /** Raison de l'indisponibilité, en infobulle. */
    unavailable?: string | null;
    /** Explication en infobulle (cause d'un état non modifiable). */
    help?: string | null;
  }>(),
  {
    disabled: false,
    showState: false,
    hideLabel: false,
    busyLabel: null,
    locked: null,
    unavailable: null,
    help: null,
  },
);

const checked = defineModel<boolean>({ required: true });

const busy = computed(() => props.busyLabel !== null);
const inactive = computed(
  () =>
    props.disabled ||
    busy.value ||
    props.locked !== null ||
    props.unavailable !== null,
);
const tooltip = computed(() => props.unavailable ?? props.help ?? null);
const description = computed(() =>
  tooltip.value === null ? props.label : `${props.label} : ${tooltip.value}`,
);

function toggle() {
  if (!inactive.value) checked.value = !checked.value;
}
</script>

<template>
  <AppTooltip :text="tooltip ?? ''" side="top" :disabled="tooltip === null">
    <button
      type="button"
      role="switch"
      class="switch"
      :class="{ inactive, busy, locked: locked !== null }"
      :aria-checked="checked"
      :aria-label="hideLabel ? description : undefined"
      :aria-disabled="(inactive && !disabled) || undefined"
      :aria-busy="busy || undefined"
      :disabled="disabled"
      @click="toggle"
    >
      <span class="track" :class="{ on: checked }" aria-hidden="true">
        <span class="knob" />
      </span>
      <span class="text">
        <span v-if="!hideLabel" class="label">{{ label }}</span>
        <span v-if="busy" class="caps state progress">
          <StatusIcon name="half" />{{ busyLabel }}
        </span>
        <span v-else-if="locked !== null" class="caps state">
          <AppIcon name="lock" />{{ locked }}
        </span>
        <span v-else-if="showState" class="caps state">{{
          checked ? "Activé" : "Désactivé"
        }}</span>
      </span>
    </button>
  </AppTooltip>
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
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
  color: var(--color-text-muted);
}

.on + .text .state {
  color: var(--color-success);
}

.switch:disabled,
.switch.inactive {
  cursor: not-allowed;
}

.switch:disabled .track,
.switch.inactive .track {
  background: transparent;
  border-style: dashed;
}

.switch:disabled .label {
  color: var(--color-text-subtle);
}

/* En cours : curseur à mi-course, contour ambre pointillé. */
.switch.busy {
  cursor: progress;
}

.switch.busy .track {
  border-color: var(--color-warning);
}

.switch.busy .knob {
  background: var(--color-warning);
  transform: translateX(calc((var(--switch-width) - var(--switch-height)) / 2));
}

.switch.busy .state,
.switch.busy .on + .text .state {
  color: var(--color-warning);
}

/* Non modifiable : curseur et cause en texte discret. */
.switch.locked .knob,
.switch.inactive:not(.busy) .on .knob {
  background: var(--color-text-subtle);
}

.switch.locked .state,
.switch.locked .on + .text .state {
  color: var(--color-text-muted);
}
</style>
