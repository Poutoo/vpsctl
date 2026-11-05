<script setup lang="ts">
/**
 * État d'une mesure (planche VII) : forme SVG et libellé en capitales. La
 * forme double la couleur : coche nominal, triangle attention, cercle à
 * point d'exclamation critique, cercle pointillé lecture, cercle barré
 * indisponible.
 */
import { computed } from "vue";
import type { Level } from "../../bindings/Level";
import StatusIcon from "../ui/StatusIcon.vue";

const props = defineProps<{
  state: Level | "reading" | "unavailable";
}>();

const shape = computed(
  () =>
    (
      ({
        nominal: "check",
        attention: "triangle",
        critical: "alert",
        reading: "reading",
        unavailable: "unavailable",
      }) as const
    )[props.state],
);
const label = computed(
  () =>
    ({
      nominal: "Nominal",
      attention: "Attention",
      critical: "Critique",
      reading: "Lecture",
      unavailable: "Indisponible",
    })[props.state],
);
</script>

<template>
  <span
    class="state caps"
    :class="state"
    :data-testid="`resource-state-${state}`"
  >
    <StatusIcon :name="shape" />
    {{ label }}
  </span>
</template>

<style scoped>
.state {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
  color: var(--color-text-muted);
  white-space: nowrap;
}

.nominal {
  color: var(--color-success);
}

.attention {
  color: var(--color-warning);
}

.critical {
  color: var(--color-danger-text);
}
</style>
