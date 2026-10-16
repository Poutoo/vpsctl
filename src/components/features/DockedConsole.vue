<script setup lang="ts">
/**
 * Console ancrée en bas du tableau de bord, sur la source choisie par
 * l'action Logs d'une ligne. Changer de source arrête le flux précédent ;
 * fermer le panneau le démonte, ce qui arrête le flux (`useLogStream`).
 */
import { computed, reactive } from "vue";
import { useLogStream } from "../../composables/useLogStream";
import { useConsoleStore, type LogTarget } from "../../stores/console";
import AppButton from "../ui/AppButton.vue";
import LogConsole from "../ui/LogConsole.vue";

const props = defineProps<{ target: LogTarget }>();

const store = useConsoleStore();
const stream = reactive(useLogStream(computed(() => props.target.source)));
</script>

<template>
  <LogConsole
    :stream="stream"
    :legend="target.kind === 'systemd' ? 'journal' : 'output'"
    :label="`Télémétrie de ${target.name}`"
  >
    <template #title>
      <span class="caps kind">{{
        target.kind === "systemd" ? "Service" : "Conteneur"
      }}</span>
      <span class="name">{{ target.name }}</span>
      <span class="command">{{ target.command }}</span>
    </template>
    <template #actions>
      <AppButton @click="store.close()">Fermer</AppButton>
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
