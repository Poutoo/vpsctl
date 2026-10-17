<script setup lang="ts">
/**
 * Console d'un flux du store `telemetry` (panneau ancré ou onglet) :
 * identification de la source dans l'en-tête, légende selon la source.
 * Pour un script : « Relancer » avec le GO / NO GO éventuel, sur sa
 * définition actuelle. Les boutons propres à l'emplacement passent par le
 * slot `actions`.
 */
import { computed } from "vue";
import { useScriptActions } from "../../composables/useScriptActions";
import { useScriptGuards } from "../../composables/useScriptGuards";
import { useScriptsStore } from "../../stores/scripts";
import type { TelemetryStream } from "../../stores/telemetry";
import LogConsole from "../ui/LogConsole.vue";

const props = defineProps<{ stream: TelemetryStream }>();

defineSlots<{ actions?(): unknown }>();

const scripts = useScriptsStore();
const { run } = useScriptActions();
const { confirmStop } = useScriptGuards();

/** Première ligne d'une commande, suivie de « … » s'il y en a d'autres. */
function firstLine(command: string): string {
  const lines = command.trim().split("\n");
  return lines.length > 1 ? `${lines[0]} …` : lines[0];
}

const ident = computed(() => {
  const stream = props.stream;
  if (stream.kind === "script") {
    return {
      kind: "Procédure",
      name: stream.script.name,
      command: firstLine(stream.script.command),
      legend: "output" as const,
    };
  }
  const target = stream.target;
  return {
    kind: target.kind === "systemd" ? "Service" : "Conteneur",
    name: target.name,
    command: target.command,
    legend:
      target.kind === "systemd" ? ("journal" as const) : ("output" as const),
  };
});

/** Interruption d'un script : après GO / NO GO. */
const requestStop = computed(() => {
  const stream = props.stream;
  if (stream.kind !== "script") return undefined;
  return async () => {
    if (await confirmStop(stream)) await stream.session.stop();
  };
});

/** Relance d'un script : sa définition enregistrée, sinon celle de l'onglet. */
const requestReplay = computed(() => {
  const stream = props.stream;
  if (stream.kind !== "script") return undefined;
  return () => void run(scripts.byId(stream.script.id) ?? stream.script);
});
</script>

<template>
  <LogConsole
    :stream="stream.session"
    :legend="ident.legend"
    :label="`Télémétrie de ${ident.name}`"
    :procedure="stream.kind === 'script'"
    :replay-label="stream.kind === 'script' ? 'Relancer' : 'Rejouer'"
    :request-stop="requestStop"
    :request-replay="requestReplay"
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
