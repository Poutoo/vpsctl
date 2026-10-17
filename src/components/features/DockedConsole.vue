<script setup lang="ts">
/**
 * Console ancrée en bas du tableau de bord, sur la source choisie par
 * l'action Logs d'une ligne. Son flux appartient au store `telemetry` :
 * arrêté quand on quitte le tableau de bord (`useDockedStream`), arrêté par
 * Fermer. « Plein écran » le transfère dans un onglet de l'écran
 * Télémétrie, sans le redémarrer ni perdre la mémoire tampon.
 */
import { useRouter } from "vue-router";
import { useTelemetryStore, type LogsStream } from "../../stores/telemetry";
import AppButton from "../ui/AppButton.vue";
import StreamConsole from "./StreamConsole.vue";

defineProps<{ stream: LogsStream }>();

const store = useTelemetryStore();
const router = useRouter();

function fullScreen() {
  const tab = store.promoteDock();
  if (tab) void router.push({ name: "telemetry", params: { tab: tab.id } });
}
</script>

<template>
  <StreamConsole :stream="stream">
    <template #actions>
      <AppButton @click="fullScreen">Plein écran</AppButton>
      <AppButton @click="store.closeDock()">Fermer</AppButton>
    </template>
  </StreamConsole>
</template>
