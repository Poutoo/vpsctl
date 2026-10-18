<script setup lang="ts">
/**
 * Console d'un flux du store `telemetry` (panneau ancré ou onglet) :
 * identification de la source dans l'en-tête, légende selon la source.
 * Pour un script : « Relancer » avec le GO / NO GO éventuel, sur sa
 * définition actuelle. Les boutons propres à l'emplacement passent par le
 * slot `actions`.
 *
 * La commande équivalente se coupe avec « … » quand la place manque (texte
 * complet en infobulle) ; le bouton à côté la copie en entier.
 */
import { computed } from "vue";
import { copyText } from "../../api/local";
import { useScriptActions } from "../../composables/useScriptActions";
import { useNotify } from "../../composables/useNotify";
import { useScriptGuards } from "../../composables/useScriptGuards";
import { useScriptsStore } from "../../stores/scripts";
import type { TelemetryStream } from "../../stores/telemetry";
import IconButton from "../ui/IconButton.vue";
import LogConsole from "../ui/LogConsole.vue";

const props = defineProps<{ stream: TelemetryStream }>();

defineSlots<{ actions?(): unknown }>();

const scripts = useScriptsStore();
const { run } = useScriptActions();
const { confirmStop } = useScriptGuards();
const { notify } = useNotify();

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
      fullCommand: stream.script.command,
      legend: "output" as const,
    };
  }
  const target = stream.target;
  return {
    kind: target.kind === "systemd" ? "Service" : "Conteneur",
    name: target.name,
    command: target.command,
    fullCommand: target.command,
    legend:
      target.kind === "systemd" ? ("journal" as const) : ("output" as const),
  };
});

/** Copie la commande complète (toutes les lignes, pour un script). */
async function copyCommand() {
  const { command, fullCommand } = ident.value;
  try {
    await copyText(fullCommand);
    notify({ tone: "success", title: "Commande copiée", detail: command });
  } catch {
    notify({
      tone: "error",
      title: "Copie impossible",
      detail: command,
      message: "Le presse-papiers du système n'est pas accessible.",
    });
  }
}

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
      <span class="name" :title="ident.name">{{ ident.name }}</span>
      <span class="command" :title="ident.fullCommand">{{
        ident.command
      }}</span>
      <IconButton icon="copy" label="Copier la commande" @click="copyCommand" />
    </template>
    <template v-if="$slots.actions" #actions>
      <slot name="actions" />
    </template>
  </LogConsole>
</template>

<style scoped>
.kind {
  flex-shrink: 0;
  color: var(--color-text-muted);
}

/* Le nom ne se coupe qu'en dernier recours ; la commande, d'abord. */
.name {
  flex-shrink: 0;
  max-width: 50%;
  overflow: hidden;
  font-family: var(--font-mono);
  font-size: var(--fs-md);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: var(--fs-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
