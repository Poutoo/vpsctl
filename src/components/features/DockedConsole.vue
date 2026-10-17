<script setup lang="ts">
/**
 * Console ancrée en bas du tableau de bord, sur la source choisie par
 * l'action Logs d'une ligne. Son flux appartient au store `telemetry` :
 * arrêté quand on quitte le tableau de bord (`useDockedStream`), arrêté par
 * Fermer.
 */
import { useTelemetryStore, type LogsStream } from "../../stores/telemetry";
import AppButton from "../ui/AppButton.vue";
import LogConsole from "../ui/LogConsole.vue";

defineProps<{ stream: LogsStream }>();

const store = useTelemetryStore();
</script>

<template>
  <LogConsole
    :stream="stream.session"
    :legend="stream.target.kind === 'systemd' ? 'journal' : 'output'"
    :label="`Télémétrie de ${stream.target.name}`"
  >
    <template #title>
      <span class="caps kind">{{
        stream.target.kind === "systemd" ? "Service" : "Conteneur"
      }}</span>
      <span class="name">{{ stream.target.name }}</span>
      <span class="command">{{ stream.target.command }}</span>
    </template>
    <template #actions>
      <AppButton @click="store.closeDock()">Fermer</AppButton>
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
