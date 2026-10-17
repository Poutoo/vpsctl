<script setup lang="ts">
/**
 * Console d'un flux du store `telemetry` (panneau ancré ou onglet) :
 * identification de la source dans l'en-tête, légende selon la source.
 * Les boutons propres à l'emplacement passent par le slot `actions`.
 */
import { computed } from "vue";
import type { TelemetryStream } from "../../stores/telemetry";
import LogConsole from "../ui/LogConsole.vue";

const props = defineProps<{ stream: TelemetryStream }>();

defineSlots<{ actions?(): unknown }>();

const ident = computed(() => {
  const target = props.stream.target;
  return {
    kind: target.kind === "systemd" ? "Service" : "Conteneur",
    name: target.name,
    command: target.command,
    legend:
      target.kind === "systemd" ? ("journal" as const) : ("output" as const),
  };
});
</script>

<template>
  <LogConsole
    :stream="stream.session"
    :legend="ident.legend"
    :label="`Télémétrie de ${ident.name}`"
  >
    <template #title>
      <span class="caps kind">{{ ident.kind }}</span>
      <span class="name">{{ ident.name }}</span>
      <span class="command">{{ ident.command }}</span>
    </template>
    <template v-if="$slots.actions" #actions>
      <slot name="actions" />
    </template>
  </LogConsole>
</template>

<style scoped>
.kind {
  color: var(--color-text-muted);
}

.name {
  overflow: hidden;
  font-family: var(--font-mono);
  font-size: var(--fs-md);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command {
  overflow: hidden;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
