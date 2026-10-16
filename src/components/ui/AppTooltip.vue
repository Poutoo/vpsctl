<script setup lang="ts">
/**
 * Infobulle : s'ouvre au survol et au focus clavier (comportement Reka UI).
 * Enveloppe un seul élément interactif, qui doit aussi porter son propre
 * libellé accessible (`aria-label`) : l'infobulle n'en est qu'un complément.
 * Nécessite un `TooltipProvider` en amont (monté dans `App.vue`).
 */
import {
  TooltipContent,
  TooltipPortal,
  TooltipRoot,
  TooltipTrigger,
} from "reka-ui";

withDefaults(
  defineProps<{
    text: string;
    side?: "top" | "right" | "bottom" | "left";
    disabled?: boolean;
  }>(),
  { side: "bottom", disabled: false },
);
</script>

<template>
  <TooltipRoot :disabled="disabled">
    <TooltipTrigger as-child>
      <slot />
    </TooltipTrigger>
    <TooltipPortal>
      <!-- Décalage en px imposé par Reka UI (nombre) : égal à --sp-1. -->
      <TooltipContent class="tooltip" :side="side" :side-offset="4">
        {{ text }}
      </TooltipContent>
    </TooltipPortal>
  </TooltipRoot>
</template>

<style scoped>
.tooltip {
  z-index: var(--z-tooltip);
  max-width: calc(5 * var(--sp-8));
  padding: var(--sp-1) var(--sp-2);
  background: var(--color-surface-raised);
  border: var(--border-width) solid var(--color-border-strong);
  border-radius: var(--r-2);
  box-shadow: var(--shadow-overlay);
  color: var(--color-text);
  font-family: var(--font-ui);
  font-size: var(--fs-sm);
  animation: tooltip-in var(--dur-fast) var(--ease-out);
}

@keyframes tooltip-in {
  from {
    opacity: 0;
  }
}
</style>
